// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The fire and police protection simulators (`docs/specs/protection.md`):
//! the shared base `cSC4ProtectionSimulator` and its two classes
//! `cSC4FireProtectionSimulator` (`0x2990C05A`) and `cSC4PoliceSimulator`
//! (`0x6990C04F`). Section references (`§n`) point at that spec.
//!
//! # Structure
//!
//! - [`ProtectionSim`]: one simulator object; its [`ProtectionKind`] selects
//!   the fire or the police variant. It holds the state of §3, the lifecycle
//!   of §4, the messages and grid updates of §5, the formulas of §6, the
//!   station and dispatch effects of §7, the dispatch source of §8, the
//!   police jails of §9, the save records of §11, the posted messages of §12
//!   and the interface slots of §15 (portable parts).
//! - [`KindParams`] ([`ProtectionKind::params`]): every ID that differs
//!   between the two classes (class, purpose, grids, properties, messages,
//!   history series, period, dispatch type).
//! - [`ProtectionTuning`] (§6.7): code defaults, the shipped exemplars
//!   ([`shipped_tuning`]) and the "default" curve pattern of foundations
//!   §4.1.
//! - Pure arithmetic in `algo.rs` ([`adjusted_value`], [`scale_effect`],
//!   [`occupant_coverage`], [`combine_strike_chance`], the dispatch-mask
//!   operations, [`choose_base_call_number`], the jail formulas).
//! - [`ProtectionHost`]: everything the simulator reads from, or does to,
//!   the rest of the game (occupants and their properties, lots, the budget
//!   and the departments, the dispatch manager, the automata controllers,
//!   terrain, zones, history, the air dispatch sources, messages, agents).
//!   Its supertraits [`GridProvider<u8>`] and [`GridProvider<i16>`] create
//!   the adjusted (`uint8`) and unadjusted (`int16`) coverage grids and
//!   resolve grid references read from a save (in game: the game's own
//!   `cSC4SimGrid` objects, simgrid §6.4). Grids are held as
//!   `Rc<dyn TractGrid<T>>` and saved by reference.
//! - [`ProtectionRandom`]: the simulator's own `cRZRandom` (§10), behind a
//!   trait so tests can script it ([`ProtectionRng`] is the real one). The
//!   iterator reset draws from the simulator's own [`CrtRand`] (foundations
//!   §12.6).
//!
//! The crime simulator uses the police simulator through four slots (crime
//! §6.4, §6.6): [`ProtectionSim::police_grid`] (slot 5, `+0x14`),
//! [`ProtectionSim::jail_population`] (slot 30, `+0x78`),
//! [`ProtectionSim::reset_unjailed_criminals`] (slot 27, `+0x6C`) and
//! [`ProtectionSim::process_monthly_crime_stats`] (slot 38, `+0x98`); the
//! plugin wires `CrimeHost` to them.
//!
//! Host handles are opaque tokens. Objects the simulator keeps (stations,
//! jails, departments, the air dispatch source) are [`ObjRef`]s: a token
//! plus the class ID a save writes for the reference. Host callbacks must
//! not call back into the simulator; the simulator holds no borrow across a
//! callback.
//!
//! # Choices made where the spec is open (`[choice]`)
//!
//! - Arithmetic follows foundations §18 (24-bit x87): every float operation
//!   is one `f32` rounding; an integer operand enters unrounded (the
//!   operation is computed in `f64` and rounded once). Every `× 100` and
//!   `× 0.01` of §6, §7, §9 and §11 uses the float32 constants `100.0f` and
//!   `0.01f` (§6.2 PR3, [compat]), and so does the "pct" tuning getter:
//!   shipped 10 → 0.099999994, 120 → 1.1999999. `trunc` is the CRT `_ftol`
//!   ([`crate::x87::ftol32`]: low 32 bits, NaN/∞ → 0; `ftol` for the int64
//!   upkeep cost). The jails' `roundHalfEven` is the `1.5 × 2^23` trick of
//!   §9.1 ([`crate::crime::rhe`]).
//! - Occupant properties are read from their first element (`s32`/`u32`:
//!   the integer value, floats truncated; `f32`: the numeric value); a
//!   property with no elements counts as absent.
//! - `GetStationEffect` (§7.1, PR2) is [`ProtectionSim::station_effect_into`]
//!   on a caller's record, exactly as the original (fire: all three reads,
//!   fails on a missing centre or radius; police: fails at once on a zero or
//!   missing centre, a missing outer or radius leaves the caller's field).
//!   Every FreeCity4 caller but `UpdateUnadjusted` passes a zeroed record, so
//!   an absent police outer or radius reads as 0 ([choice] of §7.1), and
//!   `UpdateUnadjusted` (§5.1) skips a station whose effect fails
//!   ([choice]). A prop station without a department uses department
//!   funding 1.0.
//! - `GetDispatchEffect` (§7.2, corrected by PR14) is
//!   [`ProtectionSim::dispatch_effect_into`]: it fails only on an absent or
//!   zero centre, an absent outer or radius keeps the caller's field, and the
//!   effect is scaled by `GetStationEfficiency` (§6.2). Its two callers are
//!   `UpdateUnadjusted` (§5.1) and the dispatch active / inactive handler
//!   (§5), both with the record's `+0x20` (the source station).
//!   `UpdateUnadjusted` keeps **one** record across its station and dispatch
//!   loops, as the original does (PR17, PR18, [choice] to match): it ignores
//!   the AL and always draws, so a dispatch whose station lacks the dispatch
//!   centre draws an inverted cone from the previous dispatch's (else the
//!   last station's) outer and radius. The handler passes a zeroed record
//!   (PR18: the original's is a fresh, uninitialised one) and skips the
//!   update when the effect fails ([choice] of §5, PR14; the original uses a
//!   stack leftover as the radius).
//! - The "PR14"/"PR15" labels in this module, `algo.rs` and the `pr18_*` jail
//!   vectors are the protection spec's older test numbering, not the
//!   QUESTIONS entries; PR19 (glue only) leaves them as they are.
//! - `UpdateBothGridsByPosition` (§5.2) copies the box with the adjusted
//!   grid's counts; `UpdateUnadjusted` (§5.1) draws with the unadjusted
//!   grid's counts and inverse widths. Without a grid the step is skipped.
//! - The daily step (§5.3) tests "zoned" over the cells
//!   `[tx·S, (tx+1)·S) × [tz·S, (tz+1)·S)` with S the simulator's own tract
//!   size constant 2 (not the grid's), through the host's zone-grid read;
//!   cells with a negative coordinate are skipped and the host reads cells
//!   beyond the zone grid as 0 ([choice] of §5.3). The counters add the
//!   adjusted (clamped) value. One day visits at most one full pass
//!   (`min(tracts per day, countX × countZ)`), and a second consecutive empty
//!   `GetNext` ends the day without a second end of cycle, so a hostile
//!   tracts-per-day or a degenerate iterator cannot loop forever.
//! - Shift counts of `1 << n` (availability masks, §5.2, §8) are taken
//!   modulo 32 like the x86 `shl`.
//! - A station without the unit-count property gets n = 0 at insert (§5.2;
//!   the original uses stack garbage).
//! - Base call numbers (§8.2, PR10): the other stations' ranges come from
//!   `GetDispatchesAtStation` with locals that carry over between stations
//!   (starting at 0, 0; the original's start values are uninitialised) as in
//!   the original; the smallest free run is found with interval arithmetic,
//!   call IDs at or beyond `nextCall` counting as free (no bit vector).
//!   `nextCall := max(nextCall, base + n)` saturates `base + n` at
//!   `2^32 − 1` instead of wrapping.
//! - The fire end of cycle reads the police percent coverage through the
//!   host; without a police simulator it reads 0.
//! - A crime message whose `IGZUnknown` is not a lot (or whose location
//!   cannot be read) is ignored; the original would dereference null.
//! - Police removal (§9.4 step 2): `RemoveLocallyFundedObject` is skipped
//!   when the jail item's department does not exist (the original
//!   dereferences null).
//! - Saves (§11): station and jail lists keep only references to host
//!   objects (null entries are dropped). The base record, and the police
//!   lists and counters, are committed only when read completely; on failure
//!   the references read so far are released and the simulator keeps its
//!   previous state. A re-read releases what the simulator held before. An
//!   unresolvable grid reference reads as no grid (`Init` then creates one).
//!   The air dispatch source follows the original (PR13, foundations §2.7):
//!   a fire v3 or police v8 record releases the current source first, and a
//!   failed reference leaves none.
//! - The station statistics vector (§9.2, PR12): absent → four zeros; a
//!   shorter vector is padded with zeros to four before it is updated or
//!   read, and written back padded; a longer one keeps its extra elements.
//! - `GetJailCapacity(occ)` reads an absent capacity property as 0
//!   ([compat], PR14); `GetJailCurrentUpkeepCost` truncates with `_ftol`
//!   (NaN or out of range → `i64::MIN`).
//! - `ProcessMonthlyCrimeStats` returns true (§15 marks it a choice; the
//!   original returns garbage); `EndJailbreak` returns false (quirk 13).
//! - The copies of the station and jail lists the getters return are plain
//!   values; the core takes no reference for them (the plugin AddRefs what
//!   it hands to game code).
//! - `Release` from a count ≤ 1 calls `Shutdown`, returns 0 and marks the
//!   object destroyed (the plugin then drops it); the `QueryInterface` on a
//!   null `this` quirk of §1 is the plugin's.

mod algo;
mod simulator;
#[cfg(test)]
mod tests;
mod tuning;
pub mod archive;

pub use algo::{
    adjusted_value, choose_base_call_number, combine_strike_chance, dispatch_from_station_mask, escapees,
    jail_capacity, jailbreak_ends, jailbreak_starts, jailed_percentage, occupant_coverage, pct, ratio_share,
    rehabilitation, reset_unjailed, revoke_dispatch_mask, scale_effect, unit_mask, DEFAULT_NAN,
};
pub use simulator::{
    CoverageOutputs, CycleCounters, JailState, ProtectionData, ProtectionSim, BASE_RECORD_VERSION, FIRE_RECORD_VERSION,
    POLICE_RECORD_VERSION,
};
pub use tuning::{shipped_tuning, ProtectionTuning};

use std::rc::Rc;

use crate::grid::{GridProvider, TractGrid};
use crate::property::PropertyHolder;
use crate::rng::{RzRandom, SeedSource, CLOCK_SEED};
use crate::variant::Variant;

pub use crate::rng::CrtRand;

/// The two concrete classes (§1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProtectionKind {
    Fire,
    Police,
}

/// Agent registration (§4.1): levels 4 (day) and 6 (month), flags 2, the
/// same name for both classes.
pub const AGENT_NAME: &str = "cSC4ProtectionSimulator";
pub const AGENT_LEVELS: [u32; 2] = [4, 6];
pub const AGENT_FLAGS: u32 = 2;

/// Interface IDs (§1).
pub const IID_UNKNOWN: u32 = 1;
pub const IID_FIRE_SIMULATOR: u32 = 0x88F5_4C7B;
pub const IID_POLICE_SIMULATOR: u32 = 0x6765_9895;
pub const IID_MESSAGE_TARGET2: u32 = 0x4522_94AA;
pub const IID_DISPATCH_SOURCE: u32 = 0xC9C0_0435;
pub const IID_SERIALIZABLE: u32 = 0xE4FD_A3D4;
/// The interface a department reference is read with (§11).
pub const IID_DEPARTMENT_BUDGET: u32 = 0x223E_2B67;
/// The interface the crime message's `IGZUnknown` is queried for (§9.2).
pub const IID_LOT: u32 = 0xE818_FCED;

/// Grid class IDs and tract size (§2).
pub const CLSID_ADJUSTED_GRID: u32 = 0x49B9_E602;
pub const CLSID_UNADJUSTED_GRID: u32 = 0x49B9_E605;
pub const TRACT_SIZE: i32 = 2;

/// The occupant type of buildings (stations, §5.2).
pub const OCCUPANT_TYPE_BUILDING: u32 = 0x2781_28A0;
/// The budget department group and the name-key group (§4.1).
pub const DEPARTMENT_GROUP: u32 = 0x4A35_7B40;
pub const DEPARTMENT_NAME_GROUP: u32 = 0x6A3F_F01C;
/// The tuning exemplar group and type (§6.7).
pub const TUNING_GROUP: u32 = 0xE7E2_C2DB;
pub const TUNING_TYPE: u32 = 0x6534_284A;

/// Messages handled (§5).
pub mod msg {
    pub const SIM_BEGIN: u32 = 0x6695_6810;
    pub const SIM_NEW_DAY: u32 = 0x6695_6814;
    pub const SIM_NEW_MONTH: u32 = 0x6695_6816;
    pub const OCCUPANT_INSERTED: u32 = 0x99EF_1142;
    pub const OCCUPANT_REMOVED: u32 = 0x99EF_1143;
    pub const OCCUPANT_STATE_CHANGED: u32 = 0xC8DA_C052;
    pub const FUNDING_CHANGED: u32 = 0x08DA_C3E7;
    pub const DISPATCH_BEGIN: u32 = 0xE946_71EB;
    pub const DISPATCH_ACTIVE: u32 = 0xE946_71EC;
    pub const DISPATCH_INACTIVE: u32 = 0xE946_71ED;
    pub const DISPATCH_END: u32 = 0x8946_71F6;
    pub const RELOAD_TUNING: u32 = 0xC53D_10AA;
    /// Police only: a crime was committed (crime §10).
    pub const CRIME: u32 = 0x0A15_9D95;
    /// The nine subscriptions of the base `Init`, in the original's order
    /// (§4.1 step 4, [compat]).
    pub const SUBSCRIPTIONS: [u32; 9] = [
        OCCUPANT_INSERTED,
        OCCUPANT_REMOVED,
        OCCUPANT_STATE_CHANGED,
        FUNDING_CHANGED,
        DISPATCH_BEGIN,
        DISPATCH_END,
        DISPATCH_ACTIVE,
        DISPATCH_INACTIVE,
        RELOAD_TUNING,
    ];
    /// The nine unsubscriptions of the base shutdown, in the original's
    /// order (§4.2 step 3, [compat], PR15: `0x006F5744`–`0x006F57BA`): the
    /// active / inactive pair comes **before** the dispatch end, unlike the
    /// subscriptions.
    pub const UNSUBSCRIPTIONS: [u32; 9] = [
        OCCUPANT_INSERTED,
        OCCUPANT_REMOVED,
        OCCUPANT_STATE_CHANGED,
        FUNDING_CHANGED,
        DISPATCH_BEGIN,
        DISPATCH_ACTIVE,
        DISPATCH_INACTIVE,
        DISPATCH_END,
        RELOAD_TUNING,
    ];

    /// Posted (§12).
    pub const FIRE_CYCLE: u32 = 0x68F5_4CE0;
    pub const FIRE_STRIKE_BEGIN: u32 = 0x68F5_4CE1;
    pub const FIRE_STRIKE_END: u32 = 0x68F5_4CE2;
    pub const POLICE_CYCLE: u32 = 0xA8DA_F510;
    pub const POLICE_STRIKE_BEGIN: u32 = 0xA8DA_F511;
    pub const POLICE_STRIKE_END: u32 = 0xA8DA_F512;
    pub const JAILBREAK_BEGIN: u32 = 0x2A61_4891;
    pub const JAILBREAK_END: u32 = 0x2A61_4892;
    pub const POLICE_STATION_CHANGED: u32 = 0xA949_569A;
}

/// Occupant and tuning properties (§6–§9).
pub mod prop {
    /// Preview occupants (foundations §5.2).
    pub const PREVIEW_OCCUPANT: u32 = 0x0292_44DB;
    /// Strike start day (u32; 0 or absent = not striking, §6.3).
    pub const STRIKE_START: u32 = 0xEAA6_9F83;
    /// Jail capacity (u32), jail population (written), overcapacity cost.
    pub const JAIL_CAPACITY: u32 = 0x48D7_1EE0;
    pub const JAIL_POPULATION: u32 = 0x48D7_1EE1;
    pub const JAIL_OVERCAPACITY_COST: u32 = 0x48D7_1EE4;
    /// Police station statistics, a u32 vector (§9.2).
    pub const STATION_STATS: u32 = 0x8A70_A0B9;

    /// Fire tuning (§6.7).
    pub const FIRE_FUNDING_CURVE: u32 = 0x2923_3E8D;
    pub const FIRE_STRIKE_CURVE: u32 = 0x2923_3E98;
    pub const FIRE_MAX_FUNDING: u32 = 0x2923_3E8F;
    pub const FIRE_STRIKE_EFFICIENCY: u32 = 0x2923_3E94;
    pub const FIRE_MIN_FUNDING_TO_END: u32 = 0x2923_3E96;
    pub const FIRE_MAX_STRIKE_DURATION: u32 = 0x2923_3E95;
    pub const FIRE_NO_ROAD_MULTIPLIER: u32 = 0x69B4_25DB;

    /// Police tuning (§6.7).
    pub const POLICE_FUNDING_CURVE: u32 = 0x68DD_AE93;
    pub const JAIL_FUNDING_CURVE: u32 = 0x68DD_AEA0;
    pub const POLICE_STRIKE_CURVE: u32 = 0x68DD_AE9E;
    pub const JAILBREAK_CURVE: u32 = 0xEA61_4F63;
    pub const POLICE_MAX_FUNDING: u32 = 0x68DD_AE95;
    pub const REHAB_RATE: u32 = 0x68DD_AE97;
    pub const POLICE_STRIKE_EFFICIENCY: u32 = 0x68DD_AE9A;
    pub const POLICE_MAX_STRIKE_DURATION: u32 = 0x68DD_AE9B;
    pub const POLICE_MIN_FUNDING_TO_END: u32 = 0x68DD_AE9C;
    pub const ARREST_THRESHOLD: u32 = 0x68DD_AE9F;
    pub const ESCAPEE_RANGE: u32 = 0xEA61_4F64;
    pub const END_JAILBREAK_CAPACITY: u32 = 0xEA61_4F65;
    pub const OVERFLOW_FACTOR: u32 = 0x68DD_AE92;
}

/// History series (§5.4, §9.3).
pub mod history {
    pub const FIRE_PCT: u32 = 0x2A11_D203;
    pub const FIRE_AVG: u32 = 0x2A11_D204;
    pub const SAFETY: u32 = 0xAAAD_4D87;
    pub const POLICE_PCT: u32 = 0x4A11_D354;
    pub const POLICE_AVG: u32 = 0x4A11_D355;
    pub const CRIMES: u32 = 0x0A5C_C155;
    pub const ARRESTS: u32 = 0x0A5C_C156;
}

/// The jail budget purpose, department and name (§4.1, §9).
pub const JAIL_PURPOSE: u32 = 0xEA56_768A;
pub const JAIL_DEPARTMENT: u32 = 0xA296_3984;
pub const JAIL_DEPARTMENT_NAME: u32 = 0x4A4F_C955;
/// The automata controller group of a jailbreak (§9.5).
pub const JAILBREAK_GROUP: &str = "jailbreak";

/// `GetDispatchAutomataProperties` values (§8.2).
pub const AUTOMATA_FIRE: u32 = 0x4202;
pub const AUTOMATA_POLICE_DISASTER: u32 = 0x4207;
pub const AUTOMATA_POLICE: u32 = 0x4228;
pub const AUTOMATA_D: u32 = 0x11882;

/// Every ID that differs between the two classes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KindParams {
    pub clsid: u32,
    pub iid: u32,
    /// The budget item purpose of a station.
    pub purpose: u32,
    /// The daily cycle period (§3).
    pub period: i32,
    /// The dispatch type (§15.3 slot 13).
    pub dispatch_type: u32,
    pub adjusted_instance: u32,
    pub unadjusted_instance: u32,
    pub tuning_instance: u32,
    pub department: u32,
    pub department_name: u32,
    /// Station effect: centre (s32), outer (s32), radius (f32) (§7.1).
    pub effect_props: [u32; 3],
    /// Dispatch effect of the dispatch's source station (§7.2, PR2).
    pub dispatch_effect_props: [u32; 3],
    /// Unit count, availability mask and base call number (§8.1).
    pub unit_count_prop: u32,
    pub mask_prop: u32,
    pub base_prop: u32,
    pub strike_group: &'static str,
    pub msg_cycle: u32,
    pub msg_strike_begin: u32,
    pub msg_strike_end: u32,
    pub air_source_clsid: u32,
    pub history_pct: u32,
    pub history_avg: u32,
    /// Tuning properties: funding curve, strike curve, max funding, strike
    /// efficiency, min funding to end, max strike duration.
    pub funding_curve: u32,
    pub strike_curve: u32,
}

impl ProtectionKind {
    pub const fn params(self) -> KindParams {
        match self {
            ProtectionKind::Fire => KindParams {
                clsid: 0x2990_C05A,
                iid: IID_FIRE_SIMULATOR,
                purpose: 0xEA56_7BC3,
                period: 50,
                dispatch_type: 1,
                adjusted_instance: 0x49D5_B8F9,
                unadjusted_instance: 0x89D5_B91A,
                tuning_instance: 0x68F5_524A,
                department: 0x28F5_5A9F,
                department_name: 0xAA49_35FE,
                effect_props: [0x08F5_5DC0, 0x08F5_5DC1, 0x08F5_5DC2],
                dispatch_effect_props: [0x2923_3E9A, 0x2923_3E9B, 0x2923_3E9C],
                unit_count_prop: 0x08F5_5DC5,
                mask_prop: 0x69B4_25DE,
                base_prop: 0x69B4_25DD,
                strike_group: "striking_fire",
                msg_cycle: msg::FIRE_CYCLE,
                msg_strike_begin: msg::FIRE_STRIKE_BEGIN,
                msg_strike_end: msg::FIRE_STRIKE_END,
                air_source_clsid: 0x4C0A_E114,
                history_pct: history::FIRE_PCT,
                history_avg: history::FIRE_AVG,
                funding_curve: prop::FIRE_FUNDING_CURVE,
                strike_curve: prop::FIRE_STRIKE_CURVE,
            },
            ProtectionKind::Police => KindParams {
                clsid: 0x6990_C04F,
                iid: IID_POLICE_SIMULATOR,
                purpose: 0x0A56_7BAA,
                period: 40,
                dispatch_type: 0,
                adjusted_instance: 0x49D5_BBB9,
                unadjusted_instance: 0x49D5_BBCE,
                tuning_instance: 0x28DF_CF89,
                department: 0xA296_3983,
                department_name: 0xAA49_3855,
                effect_props: [0x48D7_1ED0, 0x48D7_1ED1, 0x48D7_1ED2],
                dispatch_effect_props: [0x48D7_1ED9, 0x48D7_1EDA, 0x48D7_1EDB],
                unit_count_prop: 0x48D7_1ED5,
                mask_prop: 0x48D7_1ED8,
                base_prop: 0x48D7_1ED7,
                strike_group: "striking_police",
                msg_cycle: msg::POLICE_CYCLE,
                msg_strike_begin: msg::POLICE_STRIKE_BEGIN,
                msg_strike_end: msg::POLICE_STRIKE_END,
                air_source_clsid: 0x0C0B_06E8,
                history_pct: history::POLICE_PCT,
                history_avg: history::POLICE_AVG,
                funding_curve: prop::POLICE_FUNDING_CURVE,
                strike_curve: prop::POLICE_STRIKE_CURVE,
            },
        }
    }
}

/// Which sub-object `QueryInterface` returns (§1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProtectionInterface {
    /// The class's primary interface (also `cIGZUnknown`).
    Primary,
    MessageTarget,
    DispatchSource,
    Serializable,
}

/// `QueryInterface(iid)` (§1); `None` = false.
pub fn query_interface(kind: ProtectionKind, iid: u32) -> Option<ProtectionInterface> {
    match iid {
        IID_UNKNOWN => Some(ProtectionInterface::Primary),
        i if i == kind.params().iid => Some(ProtectionInterface::Primary),
        IID_MESSAGE_TARGET2 => Some(ProtectionInterface::MessageTarget),
        IID_DISPATCH_SOURCE => Some(ProtectionInterface::DispatchSource),
        IID_SERIALIZABLE => Some(ProtectionInterface::Serializable),
        _ => None,
    }
}

/// How a compiled slot returns its result (§15).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotReturn {
    /// Only AL is meaningful.
    Al,
    /// EAX (a count, an integer or a pointer).
    Eax,
    /// ST0 (a float).
    St0,
    /// An `SC4Percentage` by value through a hidden first stack argument;
    /// EAX = that pointer.
    HiddenPtr,
    /// A 64-bit integer in EDX:EAX.
    EdxEax,
    /// Nothing meaningful.
    None,
}

/// One row of a compiled slot table: (slot, method, callee-pop bytes,
/// return channel).
pub type SlotRow = (u32, &'static str, u32, SlotReturn);

/// `cISC4FireProtectionSimulator` (§15.1).
pub const FIRE_SLOTS: [SlotRow; 26] = {
    use SlotReturn::*;
    [
        (0, "QueryInterface", 8, Al),
        (1, "AddRef", 0, Eax),
        (2, "Release", 0, Eax),
        (3, "Init", 0, Al),
        (4, "Shutdown", 0, Al),
        (5, "GetFireProtectionGrid", 0, Eax),
        (6, "GetDepartmentBudget", 0, Eax),
        (7, "IsFireStation", 4, Al),
        (8, "GetStationFunding", 12, Al),
        (9, "SetStationFunding", 8, Al),
        (10, "GetStationEfficiency", 8, HiddenPtr),
        (11, "IsOnStrike", 0, Al),
        (12, "GetChanceOfStrike", 4, HiddenPtr),
        (13, "BeginStrike", 4, Al),
        (14, "EndStrike", 0, Al),
        (15, "GetStrikeStartDate", 0, Eax),
        (16, "GetStrikeLength", 0, Eax),
        (17, "GetStrikingStations", 4, Eax),
        (18, "IsStationOnStrike", 4, Al),
        (19, "GetAverageCoverage", 0, St0),
        (20, "GetPercentCoverage", 0, Eax),
        (21, "GetOccupantCoverage", 16, Al),
        (22, "GetFireStations", 4, Eax),
        (23, "GetFireStationCount", 0, Eax),
        (24, "GetStationDispatchRadius", 4, St0),
        (25, "GetTotalDispatchCount", 0, Eax),
    ]
};

/// `cISC4PoliceSimulator` (§15.2).
pub const POLICE_SLOTS: [SlotRow; 45] = {
    use SlotReturn::*;
    [
        (0, "QueryInterface", 8, Al),
        (1, "AddRef", 0, Eax),
        (2, "Release", 0, Eax),
        (3, "Init", 0, Al),
        (4, "Shutdown", 0, Al),
        (5, "GetPolicePowerGrid", 0, Eax),
        (6, "GetDepartmentBudget", 0, Eax),
        (7, "GetStationFunding", 12, Al),
        (8, "SetStationFunding", 8, Al),
        (9, "GetJailDepartmentBudget", 0, Eax),
        (10, "IsOnStrike", 0, Al),
        (11, "GetChanceOfStrike", 4, HiddenPtr),
        (12, "BeginStrike", 4, Al),
        (13, "EndStrike", 0, Al),
        (14, "GetStrikeStartDate", 0, Eax),
        (15, "GetStrikeLength", 0, Eax),
        (16, "GetStrikingStations", 4, Eax),
        (17, "IsStationOnStrike", 4, Al),
        (18, "GetAverageCoverage", 0, St0),
        (19, "GetPercentCoverage", 0, Eax),
        (20, "GetOccupantCoverage", 16, Al),
        (21, "GetPoliceStations", 4, Eax),
        (22, "IsPoliceStation", 4, Al),
        (23, "GetStationDispatchRadius", 4, St0),
        (24, "GetPoliceStationCount", 0, Eax),
        (25, "GetStationEfficiency", 8, HiddenPtr),
        (26, "GetTotalDispatchCount", 0, Eax),
        (27, "ResetUnjailedCriminals", 0, Eax),
        (28, "GetJailCapacity(occ)", 4, Eax),
        (29, "GetJailCapacity()", 0, Eax),
        (30, "GetJailPopulation", 0, Eax),
        (31, "GetCriminalCount", 0, Eax),
        (32, "GetJails", 4, Eax),
        (33, "IsJail", 4, Al),
        (34, "GetArrestCount", 0, Eax),
        (35, "GetJailCount", 0, Eax),
        (36, "GetJailedPercentage", 4, HiddenPtr),
        (37, "GetJailCurrentUpkeepCost", 4, EdxEax),
        (38, "ProcessMonthlyCrimeStats", 0, None),
        (39, "IsInJailbreak", 4, Al),
        (40, "BeginJailbreak", 4, Al),
        (41, "EndJailbreak", 4, Al),
        (42, "GetJailbreakBuildings", 4, Eax),
        (43, "GetPoliceStationArrestCount", 4, Eax),
        (44, "GetPoliceStationCrimeCount", 4, Eax),
    ]
};

/// An object the simulator holds a reference on: a host token and the
/// class ID a save writes for its reference.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ObjRef {
    pub token: u64,
    pub class_id: u32,
}

/// A station or dispatch effect (§7): position (metres), radius (metres),
/// centre and outer values, and the centre before the efficiency scaling.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Effect {
    pub x: f32,
    pub z: f32,
    pub radius: f32,
    pub centre: i32,
    pub outer: i32,
    pub original_centre: i32,
}

/// What `GetBudgetItemForPurpose(holder, purpose, &item)` gives (§6.1, §6.6),
/// with the item's department looked up by its ID.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BudgetItem {
    /// `IsLocallyFundedObjectInDepartment(holder, purpose)` asked of the
    /// item's department (slot 28); only used when that department exists.
    pub locally_funded: bool,
    /// The item's local funding percentage (a fraction).
    pub local_funding: f32,
    /// The item's cost.
    pub cost: i64,
    /// `budget.GetDepartmentBudget(item.departmentID)` (a host token);
    /// `None` when the lookup returns null. "The item's department exists"
    /// (§6.1, §5.2, §9.4) means `Some`; the simulator's own department
    /// pointer is not used for these calls.
    pub department: Option<u64>,
}

/// `cISC4DispatchManager::GetDispatchInfo` (§5, §5.1; PR1).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DispatchInfo {
    /// The record's `+0x20`: the source station the unit left from (host
    /// token, borrowed). Its dispatch effect is the cone's (§7.2, PR2).
    pub station: u64,
    /// The record's `+0x08`: the unit's call ID (`base + i`).
    pub call_id: u32,
    /// The record's `+0x10` byte: the dispatch state is 5.
    pub active: bool,
    /// The dispatch's position (metres).
    pub x: f32,
    pub z: f32,
}

/// A message as `DoMessage` sees it (§5).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProtectionMessage {
    pub msg_type: u32,
    pub data1: u32,
    pub data2: u32,
    /// `GetVoid1()` of the occupant messages.
    pub occupant: Option<ObjRef>,
    /// `GetIGZUnknown()` of the crime message (a host token).
    pub unknown: Option<u64>,
}

impl ProtectionMessage {
    pub fn of_type(msg_type: u32) -> ProtectionMessage {
        ProtectionMessage {
            msg_type,
            ..ProtectionMessage::default()
        }
    }
}

/// Everything the protection simulators read from, or do to, the rest of
/// the game (§14). Occupants, lots, departments and air sources are host
/// tokens.
pub trait ProtectionHost: GridProvider<u8> + GridProvider<i16> {
    // ---- lifecycle (§4) ----
    /// §4.1 base init steps 1 and 4: holds the city, the budget simulator
    /// and the zone manager, and the message server, each only if not held
    /// yet. [choice] (PR12) The simulator (the original reads a cache global
    /// each time) and fire's flammability / police's crime simulator (held
    /// by the original but never called) need not be held: only reference
    /// counts differ.
    fn acquire_services(&self, kind: ProtectionKind);
    /// Releases what [`ProtectionHost::acquire_services`] holds.
    fn release_services(&self, kind: ProtectionKind);
    /// `cISC4Simulator::AddAgent` (slot 19) once per level, in order.
    fn add_agent(&self, name: &str, levels: &[u32], flags: u32);
    /// `RemoveAgent(target)` from every level.
    fn remove_agent(&self);
    /// `AddNotification` for each type, in order.
    fn subscribe(&self, msg_types: &[u32]);
    /// `RemoveNotification` for each type, in order.
    fn unsubscribe(&self, msg_types: &[u32]);
    /// `RegisterDispatchSource` / `UnregisterDispatchSource` with the
    /// dispatch-source sub-object (`0xC9C00435`).
    fn register_dispatch_source(&self);
    fn unregister_dispatch_source(&self);
    /// `SetInstanceID` on a grid (§2), also on grids loaded from a save.
    fn set_adjusted_instance(&self, grid: &Rc<dyn TractGrid<u8>>, instance: u32);
    fn set_unadjusted_instance(&self, grid: &Rc<dyn TractGrid<i16>>, instance: u32);
    /// `Shutdown` on a grid before the simulator drops it (§4.2).
    fn shutdown_adjusted(&self, grid: &Rc<dyn TractGrid<u8>>);
    fn shutdown_unadjusted(&self, grid: &Rc<dyn TractGrid<i16>>);
    /// `AddRef` / `Release` on a held object (stations, jails, departments,
    /// air sources).
    fn add_ref(&self, token: u64);
    fn release(&self, token: u64);

    // ---- resources ----
    /// The tuning exemplar (group `0xE7E2C2DB`, type `0x6534284A`, the
    /// kind's instance), if it loads.
    fn tuning(&self, kind: ProtectionKind) -> Option<Rc<PropertyHolder>>;

    // ---- budget (§4.1, §6.1, §6.6) ----
    /// `CreateDepartmentBudget(id, group)`, `AddRef`,
    /// `SetDepartmentName(name_group, name_instance)`,
    /// `SetMaxAllowedFundingPercentage(max)`, `CreateLineItem(1, true)` and
    /// `GetLineItem(1).SetDisplayFlag(1, false)`; the held department.
    fn create_department(&self, id: u32, group: u32, name: (u32, u32), max_funding: f32) -> Option<ObjRef>;
    /// `GetDepartmentID()`.
    fn department_id(&self, dept: u64) -> u32;
    /// `*GetFundingPercentage()` (a fraction).
    fn department_funding(&self, dept: u64) -> f32;
    /// `AddLocallyFundedObject(holder, purpose)`.
    fn add_locally_funded(&self, dept: u64, occupant: u64, purpose: u32);
    /// `RemoveLocallyFundedObject(holder, purpose)`.
    fn remove_locally_funded(&self, dept: u64, occupant: u64, purpose: u32);
    /// `SetLocalFundingPercent(holder, p, purpose)`.
    fn set_local_funding(&self, dept: u64, occupant: u64, p: f32, purpose: u32) -> bool;
    /// `GetBudgetItemForPurpose(holder, purpose, &item)`; `None` = false.
    fn budget_item(&self, occupant: u64, purpose: u32) -> Option<BudgetItem>;

    // ---- occupants ----
    /// `GetType()`.
    fn occupant_type(&self, occupant: u64) -> u32;
    /// A property of the occupant's property holder; `None` when absent.
    fn occupant_property(&self, occupant: u64, id: u32) -> Option<Variant>;
    /// Sets a `u32` property (the game's property helper).
    fn set_occupant_u32(&self, occupant: u64, id: u32, value: u32);
    /// Sets a `u32` vector property.
    fn set_occupant_u32s(&self, occupant: u64, id: u32, values: &[u32]);
    /// `GetPosition()` (world x, y, z).
    fn occupant_position(&self, occupant: u64) -> [f32; 3];
    /// Whether the occupant answers `cISC4PropOccupant` (`0xE9793A65`).
    fn is_prop_occupant(&self, occupant: u64) -> bool;
    /// `GetOccupantLot(occ)`: `None` without a lot, else
    /// `HasRoadAccess()`.
    fn lot_road_access(&self, occupant: u64) -> Option<bool>;

    // ---- city, simulator, zones, terrain, disasters ----
    /// The simulator date's `DayNumber()`.
    fn sim_day(&self) -> u32;
    /// `cISC4Lot::GetLocation` of the crime message's `IGZUnknown` after a
    /// QI for `cISC4Lot`; `None` when it is not a lot.
    fn lot_location(&self, unknown: u64) -> Option<(i32, i32)>;
    /// `cISC4City::CellCenterToPosition(x, z)` (metres).
    fn cell_center_to_position(&self, cell_x: i32, cell_z: i32) -> (f32, f32);
    /// A cell of the zone manager's zone grid (`+0x14`, read raw); 0 =
    /// unzoned.
    fn zone_value(&self, cell_x: i32, cell_z: i32) -> u32;
    /// `cISTETerrain::GetSeaLevel()` and `GetAltitude(x, z)`.
    fn sea_level(&self) -> f32;
    fn altitude(&self, x: f32, z: f32) -> f32;
    /// `cISC4DisasterLayer::IsADisasterOngoing()`.
    fn disaster_ongoing(&self) -> bool;

    // ---- dispatch manager (§5, §8) ----
    /// `GetDispatchCount(type)`.
    fn dispatch_count(&self, dispatch_type: u32) -> u32;
    /// `GetDispatchIDArray(type)`.
    fn dispatch_ids(&self, dispatch_type: u32) -> Vec<u32>;
    /// `GetDispatchInfo(id, &info)`; `None` = false.
    fn dispatch_info(&self, id: u32) -> Option<DispatchInfo>;

    // ---- automata controllers (§6.3, §9.5) ----
    /// `AddControllersFromOccupantGroup(name, occ, 0)`.
    fn add_controllers(&self, group: &str, occupant: u64);
    /// `RemoveControllersFromOccupantGroup(name, occ)`.
    fn remove_controllers(&self, group: &str, occupant: u64);

    // ---- other simulators, history, messages ----
    /// The city's police simulator `GetPercentCoverage()` (fire end of
    /// cycle, §5.4); `None` without one.
    fn police_percent_coverage(&self) -> Option<f32>;
    /// `RecordHistoryData(id, value)`.
    fn record_history(&self, id: u32, value: i32);
    /// `MessagePost` of `msg_type` (§12). The simulator is void1 /
    /// `IGZUnknown` as the table says; `subject` is the station (strike
    /// begin) or the jail (jailbreak messages).
    fn post(&self, msg_type: u32, subject: Option<u64>);

    // ---- air dispatch sources (§8.4) ----
    /// Creates the fire plane / police helicopter source holding one
    /// reference. The original allocates it directly; its registered GZCOM
    /// factory builds the same object (§8.4, PR13), asked for IID
    /// `0xC9C00435`.
    fn create_air_source(&self, kind: ProtectionKind) -> Option<ObjRef>;
    /// Its non-virtual `Init` / `Shutdown`.
    fn air_source_init(&self, source: u64);
    fn air_source_shutdown(&self, source: u64);
}

/// The draws the simulator makes from its `cRZRandom` (§10).
pub trait ProtectionRandom {
    /// `RandomDoubleUniform()`.
    fn uniform_f64(&mut self) -> f64;
    /// `RandomDoubleRangeUniform(a, b)`.
    fn range_f64(&mut self, a: f64, b: f64) -> f64;
}

/// The real generator: a `cRZRandom` seeded from the wall clock at
/// construction (§10).
pub struct ProtectionRng(pub RzRandom);

impl ProtectionRng {
    pub fn new(seeds: &dyn SeedSource) -> ProtectionRng {
        ProtectionRng(RzRandom::new(CLOCK_SEED, seeds))
    }

    pub fn with_seed(seed: u32, seeds: &dyn SeedSource) -> ProtectionRng {
        ProtectionRng(RzRandom::new(seed, seeds))
    }

    /// Seeded from the session seed (foundations §6.3 *[choice]*: a member
    /// seeded from the clock takes `derive(GZCLSID)`: fire `0x2990C05A`,
    /// police `0x6990C04F`).
    pub fn from_session(kind: ProtectionKind, session: u32) -> ProtectionRng {
        let key = match kind {
            ProtectionKind::Fire => crate::seeding::keys::FIRE_PROTECTION,
            ProtectionKind::Police => crate::seeding::keys::POLICE,
        };
        ProtectionRng::with_seed(
            crate::seeding::seed(session, key),
            &crate::seeding::clocks(session, key),
        )
    }
}

impl ProtectionRandom for ProtectionRng {
    fn uniform_f64(&mut self) -> f64 {
        self.0.uniform_f64()
    }
    fn range_f64(&mut self, a: f64, b: f64) -> f64 {
        self.0.range_f64(a, b)
    }
}
