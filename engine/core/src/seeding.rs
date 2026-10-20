// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The session seed (`docs/specs/foundations.md` §6.3 *[choice]*).
//!
//! The host gives the core one 32-bit session seed `S` (`fc4.h`: every
//! create function that owns a generator takes it). Every generator's seed is
//! `derive(K) = S XOR K`, with 0 and `0xFFFFFFFF` (the values that make the
//! original fall back to the clock, §6.1) moved by XOR 1 ([`derive_seed`]).
//! `K` depends on the generator's kind of §6.2 ([`SeedKey`]); [`keys`] lists
//! the `K` of every generator a merged core owns, and every core seeds
//! through [`seed`] / [`clocks`] with one of them.
//!
//! **Process-wide generators (RN3).** A file-local generator of the §6.3
//! table is a static of its source file: it is seeded once per process and
//! keeps its stream across city loads, whichever object draws from it. The
//! budget's and the pollution effects' `cRZRandomSuper` are shared that way
//! through [`process_super`] (one per thread, keyed by `K`; the first
//! session to ask seeds it). The crime generator is process-wide too, kept
//! by its own [`crate::crime::CrimeShared`]. A core's plain constructor
//! still builds a generator of its own (tests, conformance vectors,
//! replays); the C ABI's create functions take the process-wide one.
//!
//! Not covered here, by §6.3: coordinate-seeded temporaries keep their
//! coordinate seed ([`coordinate_seed`] only moves the tract developer's
//! (0, 0) case off 0), and the C runtime `rand()` ([`crate::rng::CrtRand`])
//! stays unseeded (state 1, §12.6).

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use crate::rng::{derive_seed, file_seed, FixedSeeds, RzRandomSuper};

/// Which `K` a generator uses (foundations §6.3, by the kinds of §6.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SeedKey {
    /// A file-local generator seeded once at static initialisation with the
    /// clock XOR a constant: `K` = that constant (§6.3 table,
    /// [`file_seed`]).
    FileLocal(u32),
    /// A member generator seeded from the clock (`Seed(0xFFFFFFFF)` at
    /// construction, or the clock at `Init`): `K` = the owning class's
    /// GZCLSID, XOR the instance's creation index (from 0) for a class with
    /// several instances (MySims).
    Member { clsid: u32, instance: u32 },
    /// A member seeded with a constant whose zero multiplicative state later
    /// reseeds from the clock (§6.1): that reseed uses `K` = the owning
    /// class's GZCLSID (flammability).
    ZeroStateReseed(u32),
    /// A temporary generator seeded from the clock: `K` = the owner's
    /// GZCLSID XOR `n`, the number of earlier creations at that site by the
    /// owning object since its construction (not saved; QUESTIONS CB14).
    Temporary { clsid: u32, n: u32 },
}

impl SeedKey {
    /// A member generator of a class with one instance.
    pub const fn member(clsid: u32) -> SeedKey {
        SeedKey::Member { clsid, instance: 0 }
    }

    /// The constant `K` of foundations §6.3.
    pub const fn k(self) -> u32 {
        match self {
            SeedKey::FileLocal(k) => k,
            SeedKey::Member { clsid, instance } => clsid ^ instance,
            SeedKey::ZeroStateReseed(clsid) => clsid,
            SeedKey::Temporary { clsid, n } => clsid ^ n,
        }
    }
}

/// The seed of the generator `key` in the session `session`:
/// `derive(K)` of foundations §6.3. Never 0 and never `0xFFFFFFFF`.
pub fn seed(session: u32, key: SeedKey) -> u32 {
    derive_seed(session, key.k())
}

/// The clock values to build the generator `key` with: both are
/// [`seed`]`(session, key)`. A generator seeded with a derived seed never
/// reads them (the seed is neither `0xFFFFFFFF` nor 0, and its
/// multiplicative state stays odd-multiplied and non-zero); a generator
/// seeded with a constant 0 reads the process clock once, and §6.3 makes
/// that reseed `derive(GZCLSID)` ([`SeedKey::ZeroStateReseed`]).
pub fn clocks(session: u32, key: SeedKey) -> FixedSeeds {
    let s = seed(session, key);
    FixedSeeds {
        wall_clock: s,
        process_clock: s,
    }
}

/// The prop developer's seed (§6.3: the original's × 28 of the clock value
/// is applied to `derive(K)` instead): `derive(S, 0x89C48F47) × 28 mod
/// 2^32`. Never `0xFFFFFFFF` (the product is even); it may be 0, which a
/// `cRZRandomSuper` seeds as 1 like any other value (bit 0 is ignored, RN1).
pub fn prop_developer_seed(session: u32) -> u32 {
    seed(session, keys::PROP_DEVELOPER).wrapping_mul(28)
}

/// A coordinate-seeded temporary's seed (§6.3): unchanged, except that the
/// one case whose seed is 0 (tract developer `UpdatePopulation` for tract
/// (0, 0), whose multiplicative path the original reseeds from the clock)
/// uses 1 *[choice]* (tract-developer.md §4.18).
pub fn coordinate_seed(seed: u32) -> u32 {
    if seed == 0 {
        1
    } else {
        seed
    }
}

/// A `cRZRandomSuper` several objects draw from in turn (RN3).
pub type SharedSuper = Rc<RefCell<RzRandomSuper>>;

thread_local! {
    /// The process-wide file-local generators made so far, by `K`.
    static PROCESS_SUPERS: RefCell<BTreeMap<u32, SharedSuper>> = const { RefCell::new(BTreeMap::new()) };
}

/// The process-wide file-local `cRZRandomSuper` `key` (RN3): seeded with
/// `derive(S, K)` of the session that first asks for it, then shared, its
/// stream continuing across city loads (a later `session` is ignored). One
/// per thread: the plugin creates its simulators on the main thread.
pub fn process_super(session: u32, key: SeedKey) -> SharedSuper {
    PROCESS_SUPERS.with(|m| {
        m.borrow_mut()
            .entry(key.k())
            .or_insert_with(|| {
                Rc::new(RefCell::new(RzRandomSuper::new(
                    seed(session, key),
                    &clocks(session, key),
                )))
            })
            .clone()
    })
}

/// The `K` of every generator a merged core owns (foundations §6.2, §6.3).
///
/// Generators the merged cores do not draw from are left out: the power
/// simulator's and the zone manager's members (never drawn, power.md,
/// zones.md), the traffic network map's (no draws in traffic-network.md).
/// Cores still in flight that own generators (§6.2, §6.3 table) adopt the
/// helper when their glue lands: prop developer (member, × 28), advisors
/// and MySims (members, MySims per instance), the `cSC4Lot`, wealth
/// requester, network construction crew (no core yet), construction
/// / base-texture / retaining-wall / prop / fire occupants, automata, region
/// creation and advice file-local generators (keys below where the spec
/// gives `K`). The disaster layer's and the city situation manager's cores
/// are merged (their glue is not): their generators are listed here.
pub mod keys {
    use super::{file_seed, SeedKey};

    // ---- file-local generators (the §6.3 table) --------------------------

    /// Building development simulator (`0x00B485F0`, `cRZRandom`).
    pub const BUILDING_DEVELOPMENT: SeedKey = SeedKey::FileLocal(file_seed::BUILDING_DEVELOPMENT);
    /// Growth developers (`0x00B49080`, `cRZRandom`, shared).
    pub const GROWTH_DEVELOPER: SeedKey = SeedKey::FileLocal(file_seed::GROWTH_DEVELOPER);
    /// Tract developers (`0x00B4AFD8`, `cRZRandom`, shared).
    pub const TRACT_DEVELOPER: SeedKey = SeedKey::FileLocal(file_seed::TRACT_DEVELOPER);
    /// Lot developer (`0x00B49AF0`, `cRZRandom`).
    pub const LOT_DEVELOPER: SeedKey = SeedKey::FileLocal(file_seed::LOT_DEVELOPER);
    /// Zone developer (`0x00B4B398`, `cRZRandom`).
    pub const ZONE_DEVELOPER: SeedKey = SeedKey::FileLocal(file_seed::ZONE_DEVELOPER);
    /// Budget simulator `DoMessage` (`0x00B47B78`, `cRZRandomSuper`).
    pub const BUDGET: SeedKey = SeedKey::FileLocal(file_seed::BUDGET);
    /// Crime simulator (`0x00B48630`, `cRZRandomSuper`, process-wide).
    pub const CRIME: SeedKey = SeedKey::FileLocal(file_seed::CRIME);
    /// Pollution simulator visual effects (`0x00B49BD0`, `cRZRandomSuper`).
    pub const POLLUTION_EFFECTS: SeedKey = SeedKey::FileLocal(file_seed::POLLUTION);
    /// Earthquake disaster (`0x00B22AE8`, `cRZRandom`; disasters.md).
    pub const EARTHQUAKE: SeedKey = SeedKey::FileLocal(file_seed::EARTHQUAKE);
    /// Pipe-burst disaster (`0x00B22B60`, `cRZRandom`; disasters.md).
    pub const PIPE_BURST: SeedKey = SeedKey::FileLocal(file_seed::PIPE_BURST);
    /// Riot disaster (`0x00B22B80`, `cRZRandom`; disasters.md).
    pub const RIOT: SeedKey = SeedKey::FileLocal(file_seed::RIOT);

    // ---- member generators seeded from the clock (K = GZCLSID) -----------

    /// Residential simulator (`cRZRandom`, wall clock at construction).
    pub const RESIDENTIAL: SeedKey = SeedKey::member(crate::residential::CLSID_RESIDENTIAL_SIMULATOR);
    /// Aura simulator (`cRZRandom`, wall clock at construction; aura.md §9).
    pub const AURA: SeedKey = SeedKey::member(crate::aura::CLSID_AURA_SIMULATOR);
    /// Weather simulator (`cRZRandom`, wall clock at construction).
    pub const WEATHER: SeedKey = SeedKey::member(crate::weather::CLSID_WEATHER_SIMULATOR);
    /// Flora simulator (`cRZRandom`, wall clock at construction).
    pub const FLORA: SeedKey = SeedKey::member(crate::flora::CLSID_FLORA_SIMULATOR);
    /// Fire protection simulator (`cRZRandom`, wall clock at construction;
    /// GZCLSID `0x2990C05A`, protection.md §1).
    pub const FIRE_PROTECTION: SeedKey = SeedKey::member(crate::protection::ProtectionKind::Fire.params().clsid);
    /// Police simulator (`cRZRandom`, wall clock at construction; GZCLSID
    /// `0x6990C04F`, protection.md §1).
    pub const POLICE: SeedKey = SeedKey::member(crate::protection::ProtectionKind::Police.params().clsid);
    /// Plumbing simulator (`cRZRandom`, wall clock at `Init`).
    pub const PLUMBING: SeedKey = SeedKey::member(crate::plumbing::CLSID_PLUMBING_SIMULATOR);
    /// Traffic simulator (`cRZRandomSuper`, wall clock at `Init`).
    pub const TRAFFIC_SIMULATOR: SeedKey = SeedKey::member(crate::traffic_simulator::RNG_K);
    /// Park manager (`cRZRandomSuper`, wall clock at construction; parks.md
    /// §5).
    pub const PARK_MANAGER: SeedKey = SeedKey::member(crate::parks::CLSID_PARK_MANAGER);
    /// The disaster fire instance's own `cRZRandom` (flammability.md §13.1;
    /// K = the fire instance class `0x69C1718F`, disasters lane choice, QUESTIONS DS1).
    pub const DISASTER_FIRE: SeedKey = SeedKey::member(crate::disasters::instance_class::FIRE);
    /// City situation manager (`cRZRandomSuper`, wall clock at construction,
    /// drawn only during a mission; city-situation.md §5).
    pub const CITY_SITUATION: SeedKey = SeedKey::member(crate::city_situation::CLSID_CITY_SITUATION);

    // ---- members seeded with a constant -----------------------------------

    /// Flammability simulator: its `cRZRandom` is seeded with 0 and keeps
    /// it; the double path's clock reseed uses `derive(GZCLSID)`.
    pub const FLAMMABILITY_RESEED: SeedKey =
        SeedKey::ZeroStateReseed(crate::flammability::CLSID_FLAMMABILITY_SIMULATOR);

    // ---- generators of cores still in flight (§6.3 table, §6.2) ----------
    //
    // Not used by a merged core yet: their glue adopts them.

    /// `cSC4Lot` (`0x00B49120`, `cRZRandomSuper`).
    pub const LOT: SeedKey = SeedKey::FileLocal(file_seed::LOT);
    /// Wealth requester prop selection (`0x00B4A5C0`, `cRZRandomSuper`).
    pub const WEALTH_REQUESTER: SeedKey = SeedKey::FileLocal(file_seed::WEALTH_REQUESTER);
    /// Network lot manager (`0x00B44870`, `cRZRandomSuper`); adopted by the network lot core (network_manager).
    pub const NETWORK_LOT_MANAGER: SeedKey = SeedKey::FileLocal(file_seed::NETWORK_LOT_MANAGER);
    /// Network construction crew (`0x00B43E88`, `cRZRandomSuper`).
    pub const NETWORK_CONSTRUCTION_CREW: SeedKey = SeedKey::FileLocal(file_seed::NETWORK_CONSTRUCTION_CREW);
    /// Construction occupant (`0x00B22FB0`, `cRZRandom`): `K` = 0, not the
    /// static's `0xEC09C32A` ([`file_seed::CONSTRUCTION_OCCUPANT`]), because
    /// the first `StaticInit` reseeds it with the plain clock before any draw
    /// (§6.3 table), so `derive(0)` = `S` (moved off 0 / `0xFFFFFFFF`).
    pub const CONSTRUCTION_OCCUPANT: SeedKey = SeedKey::FileLocal(0);
    /// Lot base-texture occupant (`0x00B23078`, `cRZRandom`).
    pub const BASE_TEXTURE_OCCUPANT: SeedKey = SeedKey::FileLocal(file_seed::BASE_TEXTURE_OCCUPANT);
    /// Lot retaining-wall occupant (`0x00B230C8`, `cRZRandom`).
    pub const RETAINING_WALL_OCCUPANT: SeedKey = SeedKey::FileLocal(file_seed::RETAINING_WALL_OCCUPANT);
    /// Prop occupant (`0x00B231B0`, `cRZRandomSuper`).
    pub const PROP_OCCUPANT: SeedKey = SeedKey::FileLocal(file_seed::PROP_OCCUPANT);
    /// Fire occupant, flames (`0x00B21648`, `cRZRandom`).
    pub const FIRE_OCCUPANT: SeedKey = SeedKey::FileLocal(file_seed::FIRE_OCCUPANT);
    /// Automata (`0x00B21D78`, `cRZRandomSuper`).
    pub const AUTOMATA: SeedKey = SeedKey::FileLocal(file_seed::AUTOMATA);
    /// Region creation (`0x00B217C0`, `cRZRandom`).
    pub const REGION_CREATION: SeedKey = SeedKey::FileLocal(file_seed::REGION_CREATION);
    /// Advice (`0x00B22E58`, `cRZRandom`).
    pub const ADVICE: SeedKey = SeedKey::FileLocal(file_seed::ADVICE);
    /// Prop developer (`cRZRandomSuper` member seeded with the wall clock
    /// × 28 at construction; GZCLSID `0x89C48F47`, prop-developer.md §1):
    /// seed it with [`super::prop_developer_seed`].
    pub const PROP_DEVELOPER: SeedKey = SeedKey::member(0x89C4_8F47);

    // ---- temporaries seeded from the clock (K = GZCLSID XOR n) -----------

    /// The civic building simulator's monthly temporary, the `n`-th made by
    /// this simulator object (civic-buildings.md §5).
    pub const fn civic_monthly(n: u32) -> SeedKey {
        SeedKey::Temporary {
            clsid: crate::civic::CLSID_CIVIC_BUILDING_SIMULATOR,
            n,
        }
    }

    /// The neighbours simulator's deal-proposal generator (process-wide and
    /// lazy in the original), the `n`-th made by this simulator object
    /// (neighbors.md; one per object, so `n` = 0).
    pub const fn neighbors_proposals(n: u32) -> SeedKey {
        SeedKey::Temporary {
            clsid: crate::neighbors::CLSID_NEIGHBORS_SIMULATOR,
            n,
        }
    }

    /// The argument-less pipe burst's throwaway generator (disasters.md §8),
    /// the `n`-th made by this layer object; `K` = the pipe-burst manager
    /// class `0x4A132FDF` XOR `n`.
    pub const fn pipe_burst_start(n: u32) -> SeedKey {
        SeedKey::Temporary {
            clsid: crate::disasters::seed::PIPE_BURST_MANAGER,
            n,
        }
    }

    /// A growth developer's decline-key temporary whose coordinate seed is
    /// `0xFFFFFFFF` (growth-developer.md §7 *[quirk]*: `Seed` then reads the
    /// clock), the `n`-th such clock read by this developer object.
    /// [choice] (QUESTIONS RN2) §6.3 says coordinate-seeded temporaries are
    /// unchanged; this one case reads the clock, so it is treated as a
    /// clock-seeded temporary of the owning class.
    pub const fn growth_decline_clock(n: u32) -> SeedKey {
        SeedKey::Temporary {
            clsid: crate::growth_developer::CLSID_GROWTH_DEVELOPER,
            n,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::{RzRandom, RzRandomSuper};

    const S: u32 = 0x1234_5678;

    /// `derive(K)` for a key whose expected `K` is `k`: the key's constant,
    /// the derived seed, the fix-up and the clock values.
    fn check(key: SeedKey, k: u32) {
        assert_eq!(key.k(), k, "{key:?}");
        assert_eq!(seed(S, key), S ^ k, "{key:?}");
        // S = K gives 0, S = !K gives 0xFFFFFFFF: both move by XOR 1.
        assert_eq!(seed(k, key), 1, "{key:?}");
        assert_eq!(seed(!k, key), 0xFFFF_FFFE, "{key:?}");
        let c = clocks(S, key);
        assert_eq!((c.wall_clock, c.process_clock), (S ^ k, S ^ k), "{key:?}");
    }

    #[test]
    fn derive_fix_up() {
        assert_eq!(derive_seed(5, 5), 1);
        assert_eq!(derive_seed(0xFFFF_FFFF, 0), 0xFFFF_FFFE);
        assert_eq!(derive_seed(0, 0), 1);
        assert_eq!(derive_seed(0xA5A5_0000, 0x0000_5A5A), 0xA5A5_5A5A);
    }

    #[test]
    fn k_in_flight_file_locals() {
        check(keys::LOT, 0x0C09_C461);
        check(keys::WEALTH_REQUESTER, 0x4C09_C51F);
        check(keys::NETWORK_LOT_MANAGER, 0x8C09_C379);
        check(keys::NETWORK_CONSTRUCTION_CREW, 0x6C09_C07B);
        check(keys::BASE_TEXTURE_OCCUPANT, 0x8C09_C2D5);
        check(keys::RETAINING_WALL_OCCUPANT, 0x0C09_C347);
        check(keys::PROP_OCCUPANT, 0xAC09_C2EC);
        check(keys::FIRE_OCCUPANT, 0xEC09_C147);
        check(keys::AUTOMATA, 0x4C09_C170);
        check(keys::REGION_CREATION, 0xAC35_7AF9);
        check(keys::ADVICE, 0x2C09_C27B);
    }

    #[test]
    fn k_construction_occupant_is_zero() {
        // The static's constant is overwritten by the plain clock (§6.3).
        check(keys::CONSTRUCTION_OCCUPANT, 0);
        assert_eq!(seed(S, keys::CONSTRUCTION_OCCUPANT), S);
        assert_eq!(seed(0, keys::CONSTRUCTION_OCCUPANT), 1);
    }

    #[test]
    fn k_prop_developer() {
        check(keys::PROP_DEVELOPER, 0x89C4_8F47);
        assert_eq!(prop_developer_seed(S), (S ^ 0x89C4_8F47).wrapping_mul(28));
        // derive(K) = 0x40000000 gives 0 after × 28.
        assert_eq!(prop_developer_seed(0x4000_0000 ^ 0x89C4_8F47), 0);
        assert_eq!(prop_developer_seed(0x89C4_8F47), 28);
    }

    #[test]
    fn k_city_situation() {
        check(keys::CITY_SITUATION, 0x0BB1_4381);
    }

    #[test]
    fn k_earthquake() {
        check(keys::EARTHQUAKE, 0x6C09_C1AB);
    }

    #[test]
    fn k_pipe_burst() {
        check(keys::PIPE_BURST, 0xEC09_C1D1);
    }

    #[test]
    fn k_riot() {
        check(keys::RIOT, 0x4C09_C187);
    }

    #[test]
    fn k_disaster_fire() {
        check(keys::DISASTER_FIRE, 0x69C1_718F);
    }

    #[test]
    fn k_pipe_burst_start() {
        check(keys::pipe_burst_start(0), 0x4A13_2FDF);
        check(keys::pipe_burst_start(3), 0x4A13_2FDC);
    }

    #[test]
    fn k_building_development() {
        check(keys::BUILDING_DEVELOPMENT, 0x0C09_C0B8);
    }

    #[test]
    fn k_growth_developer() {
        check(keys::GROWTH_DEVELOPER, 0xAC09_C0E8);
    }

    #[test]
    fn k_tract_developer() {
        check(keys::TRACT_DEVELOPER, 0x4C09_C113);
    }

    #[test]
    fn k_lot_developer() {
        check(keys::LOT_DEVELOPER, 0x6C09_C0F4);
    }

    #[test]
    fn k_zone_developer() {
        check(keys::ZONE_DEVELOPER, 0x8C09_C68A);
    }

    #[test]
    fn k_budget() {
        check(keys::BUDGET, 0x6C09_C3D7);
    }

    #[test]
    fn k_crime() {
        check(keys::CRIME, 0x6C09_C3F6);
    }

    #[test]
    fn k_pollution_effects() {
        check(keys::POLLUTION_EFFECTS, 0x4C09_C4D3);
    }

    #[test]
    fn k_residential() {
        check(keys::RESIDENTIAL, 0x4990_C013);
    }

    #[test]
    fn k_aura() {
        check(keys::AURA, 0x0990_BDD8);
    }

    #[test]
    fn k_weather() {
        check(keys::WEATHER, 0x2990_C1BC);
    }

    #[test]
    fn k_flora() {
        check(keys::FLORA, 0xC990_C034);
    }

    #[test]
    fn k_fire_protection() {
        check(keys::FIRE_PROTECTION, 0x2990_C05A);
    }

    #[test]
    fn k_police() {
        check(keys::POLICE, 0x6990_C04F);
    }

    #[test]
    fn k_plumbing() {
        check(keys::PLUMBING, 0x0990_C075);
    }

    #[test]
    fn k_traffic_simulator() {
        check(keys::TRAFFIC_SIMULATOR, 0x6990_C1AA);
    }

    #[test]
    fn k_park_manager() {
        check(keys::PARK_MANAGER, 0xAA3F_E6BC);
    }

    #[test]
    fn k_flammability_reseed() {
        check(keys::FLAMMABILITY_RESEED, 0xE990_C045);
    }

    #[test]
    fn k_civic_monthly() {
        check(keys::civic_monthly(0), 0x0A41_3481);
        check(keys::civic_monthly(1), 0x0A41_3480);
        check(keys::civic_monthly(6), 0x0A41_3487);
    }

    #[test]
    fn k_neighbors_proposals() {
        check(keys::neighbors_proposals(0), 0xC990_C0A4);
        check(keys::neighbors_proposals(3), 0xC990_C0A7);
    }

    #[test]
    fn k_growth_decline_clock() {
        check(keys::growth_decline_clock(0), 0x0990_C138);
        check(keys::growth_decline_clock(2), 0x0990_C13A);
    }

    #[test]
    fn k_member_instances() {
        // MySims-style classes: GZCLSID XOR the creation index.
        let k = SeedKey::Member {
            clsid: 0xAB00_0000,
            instance: 5,
        };
        check(k, 0xAB00_0005);
        assert_eq!(
            SeedKey::member(0x10),
            SeedKey::Member {
                clsid: 0x10,
                instance: 0
            }
        );
    }

    #[test]
    fn coordinate_seeds() {
        assert_eq!(coordinate_seed(0), 1);
        assert_eq!(coordinate_seed(0x0001_0002 << 4), 0x0010_0020);
        assert_eq!(coordinate_seed(0xFFFF_FFFF), 0xFFFF_FFFF);
    }

    /// A derived generator never reads the clock values: built with any
    /// other clock values it draws the same.
    #[test]
    fn derived_generators_ignore_the_clock() {
        for key in [keys::AURA, keys::CRIME, keys::civic_monthly(3)] {
            let s = seed(S, key);
            let odd = FixedSeeds {
                wall_clock: 7,
                process_clock: 9,
            };
            let mut a = RzRandom::new(s, &clocks(S, key));
            let mut b = RzRandom::new(s, &odd);
            for _ in 0..8 {
                assert_eq!(a.next_raw(), b.next_raw());
                assert_eq!(a.uniform_f64().to_bits(), b.uniform_f64().to_bits());
            }
            let mut c = RzRandomSuper::new(s, &clocks(S, key));
            let mut d = RzRandomSuper::new(s, &odd);
            assert_eq!(c.next_u32(), d.next_u32());
        }
    }

    /// Two sessions with the same `S` give every generator the same seed;
    /// a different `S` (bit 1, RN1) changes them.
    #[test]
    fn same_session_same_seeds() {
        let all = [
            keys::BUILDING_DEVELOPMENT,
            keys::GROWTH_DEVELOPER,
            keys::TRACT_DEVELOPER,
            keys::LOT_DEVELOPER,
            keys::ZONE_DEVELOPER,
            keys::BUDGET,
            keys::CRIME,
            keys::POLLUTION_EFFECTS,
            keys::RESIDENTIAL,
            keys::AURA,
            keys::WEATHER,
            keys::FLORA,
            keys::FIRE_PROTECTION,
            keys::POLICE,
            keys::PLUMBING,
            keys::TRAFFIC_SIMULATOR,
            keys::PARK_MANAGER,
            keys::FLAMMABILITY_RESEED,
            keys::EARTHQUAKE,
            keys::PIPE_BURST,
            keys::RIOT,
            keys::DISASTER_FIRE,
            keys::CITY_SITUATION,
        ];
        for key in all {
            assert_eq!(seed(S, key), seed(S, key));
            assert_ne!(seed(S, key), seed(S ^ 2, key));
        }
        // Every K is distinct, so no two generators share a stream.
        let mut ks: Vec<u32> = all.iter().map(|k| k.k()).collect();
        ks.sort_unstable();
        ks.dedup();
        assert_eq!(ks.len(), all.len());
    }

    /// RN3: a process-wide generator is seeded once, by the first session,
    /// and every later request shares its stream.
    #[test]
    fn process_supers_are_shared() {
        let a = process_super(S, keys::BUDGET);
        let first = a.borrow_mut().next_u32();
        let b = process_super(S ^ 2, keys::BUDGET);
        assert!(Rc::ptr_eq(&a, &b));
        let mut fresh = RzRandomSuper::new(seed(S, keys::BUDGET), &clocks(S, keys::BUDGET));
        assert_eq!(fresh.next_u32(), first);
        assert_eq!(b.borrow_mut().next_u32(), fresh.next_u32());
        // Another K is another generator.
        let c = process_super(S, keys::POLLUTION_EFFECTS);
        assert!(!Rc::ptr_eq(&a, &c));
    }
}
