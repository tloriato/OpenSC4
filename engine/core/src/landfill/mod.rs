// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The landfill developer (`cSC4LandfillDeveloper`, class `0xEA09D4AE`), its
//! landfill zones (`cSC4LandfillZone`) and the landfill tile occupant
//! (`cSC4LandfillOccupant`, class `0x8A22BEBE`): `docs/specs/landfill.md`.
//! Section references (`§n`) point at that spec.
//!
//! # Structure
//!
//! - [`LandfillDeveloper`]: the city's one landfill developer (component slot
//!   12). It holds the zone list and the recomputed totals of §2.1, the
//!   lifecycle of §1.1–§1.2, `DoMessage` (§3.2), zoning and dezoning (§4.4),
//!   the terrain handling (§4.5), the capacity totals (§4.6), the monthly
//!   pass (§4.7), the budget and cost slots (§4.9), the posted history (§5),
//!   the save record (§6.1) and the interface slots of §7.1.
//! - [`LandfillZone`]: one zone (§2.2) — its [`CellRegion`], the heap grid,
//!   the connected flag, the zot, `levelAltitude` and the sticky
//!   `completelyDezoned` cache — with `AddRegionToZone` (§4.4.4),
//!   `PopulateZone` (§4.4.5), `DezoneCell` (§4.4.8), the terrain response
//!   (§4.5.2), `LevelRect` (§4.5.3), capacity (§4.6.1–§4.6.4), `UpdateZone`
//!   (§4.7.2), the tile models and their random draws (§4.8) and the inline
//!   zone record (§6.2). Zones are `Rc`-held, refcounted objects, as in the
//!   original.
//! - [`Heap`]: one 2×2 block of zoned cells (§2.2).
//! - [`LandfillOccupant`]: the tile occupant (§2.3) with its record (§6.4).
//! - [`Tuning`]: the exemplar of §4.1 with its code defaults and the shipped
//!   values ([`shipped_tuning`]).
//! - `algo.rs`: the pure arithmetic of §4.3, §4.6.1, §4.7.3, §4.7.4 and §4.8
//!   ([`magic_round`], [`altitude_variance`], [`zoning_cost`],
//!   [`heap_capacity`], [`decay_amount`], [`redistribute`], [`tile_of`],
//!   [`neighbour_mask`]).
//! - `levelling.rs`: what the lot developer's `AttemptAreaLevelling` does for
//!   `LevelRect` (§4.5.3, conformance test 22), as a pure function over the
//!   three slots it calls ([`attempt_area_levelling`], [`LevellingSlots`]).
//! - [`LandfillHost`]: everything the developer and its zones read from, or
//!   do to, the rest of the game (§8): the city and its managers, terrain,
//!   the pollution simulator, the budget and the sanitation department, the
//!   zone manager, the traffic simulator and network map, the lot developer's
//!   levelling, the 3D view, occupants, props, history, messages and agents.
//!   Objects outside these three classes are host tokens (`u64`).
//!
//! # Choices made where the spec is open (`[choice]`)
//!
//! - **Arithmetic** follows foundations §18 (24-bit x87): every float add,
//!   subtract, multiply and divide is one `f32` rounding
//!   ([`crate::x87::r24`]), an integer or `double` operand enters that one
//!   operation unrounded, and every `trunc` is the CRT `_ftol`
//!   ([`crate::x87::ftol`]/[`crate::x87::ftol32`]). `round(v)` in §4.3,
//!   §4.6.1 and §4.7.3 is the class's **magic-number round**
//!   `bits(f32(v + 12582912.0)) − 0x4B400000`, which is
//!   [`crate::x87::rhe`] and is wrong above 2²² (§4, quirk 14).
//!   [`zoning_cost`] returns the int64 whose **high dword is always 0**
//!   (§4.3 step 4 [compat]): the 32-bit round result zero-extended.
//! - **The C `rand()` of §4.8** is the developer's own [`CrtRand`], at the
//!   process-start state 1 by default and overridable by the host
//!   (foundations §12.6 *[choice]*). Landfill has **no** `cRZRandom`, so
//!   foundations §6.3's `derive_seed` does not apply here: §6.3's last bullet
//!   keeps the CRT generator unseeded. One draw per redrawn cell whose four
//!   surrounding heaps are all active, 0–4 per heap state change (§4.8
//!   *[compat]*); nothing else in these classes draws.
//! - **No re-entrancy in §4.4.8 step 4** *[compat]* (spec question 4,
//!   answered): `DezoneCell` with `changeZone` calls
//!   [`LandfillHost::place_zone`] and nothing else; the zone manager
//!   **posts** `0x07699455`, which reaches [`LandfillDeveloper::do_message`]
//!   on a later message-server tick, after the whole `RemoveZone` /
//!   `DoTerrainChanged` pass has completed. The core never calls
//!   `DoZoneChange` from inside a dezone.
//! - **`LevelRect(…, false)`** (§4.5.3) calls the lot developer's slot 21
//!   through [`LandfillHost::attempt_area_levelling`] and ignores the result.
//!   What that slot does — level to the **vertex** average, or refuse when
//!   the area slope exceeds the lot developer's tuning (40) — is
//!   [`attempt_area_levelling`], which a host may use with its own
//!   [`LevellingSlots`].
//! - **The zot spiral of §4.7.5** is the plumbing §7.2 `SpiralIterator` in
//!   its clockwise mode over the zone rect (LF4): [`spiral_cells`] lists the
//!   centre, then ring r from `(cx + r, cz − r + 1)` running +z, −x, −z, +x
//!   to `(cx + r, cz − r)`, out-of-rect cells skipped one by one. The first
//!   mask cell wins; with no mask cell the centre cell is used.
//! - **The zot's orientation** comes from a global view service (§4.7.5,
//!   spec open question 6): [`LandfillHost::zot_orientation`].
//! - **The zone effects of §4.10** are rendering only and affect no
//!   simulation value, so the core only tracks the two automaton tokens and
//!   the seagull token plus its started flag, and reproduces the *gating*
//!   the messages specify (§3.2 FundsChanged needs a department and a truck
//!   automaton; PreViewLevelChange needs a non-empty zone list and a seagull
//!   effect). Placement (`GetFirstRoadTileNeighbor`, the centre cell 64 m up)
//!   and the Lua names are the host's.
//! - **Null pointers the original would dereference** are guarded: no
//!   department before PostCityInit reads as 0 current expenses and 1.0
//!   efficiency (§4.9.2, quirk 12); a missing line item as 0; a failed
//!   `AddLotProp` for the zot leaves no zot (§4.7.5, quirk: the original
//!   crashes); a missing tile exemplar still creates the occupant (§4.8.1,
//!   quirk) with no parent exemplar; a failed `GetSubnetworksInRegion` or
//!   `GetOccupantCountForAllSubnetworks` reads as no subnetwork / no
//!   occupants, so the zone is not connected.
//! - **`HandleZoneOverlap` step 3** (§4.4.2) uses the intersection `I`, which
//!   the original leaves uninitialised when step 1's test failed. *[choice]*
//!   `RedisplayTerrain` is then skipped (it is display only).
//! - **`DoTerrainChanged`** (§4.5.1) walks the zone list once, in order,
//!   and removes every zone whose `IsCompletelyDezoned()` is true. Nothing
//!   re-enters, so the list only shrinks during the pass.
//! - **Heaps are addressed by vector index** `hz × W + hx` (§2.2) in
//!   `A(i, j)` (§4.8) and `InvalidateSurroundingHeaps` (§4.4.8). The stored
//!   `hx`, `hz` only decide which cells a heap's redraw visits (§4.8).
//! - **Landfill tiles are core objects**: §4.8.1 step 3 constructs the
//!   [`LandfillOccupant`] in the core, sets its flags, position and `Init`
//!   on it, and hands it to the occupant manager through
//!   [`LandfillHost::insert_landfill_occupant`] (slot 20), which gives it a
//!   token. Every other occupant (found by `occupant_at`, possibly an original
//!   one) is reached through host calls.
//! - **`RedistributeGarbage`** (§4.7.4): `share := cap / v` is a signed
//!   32-bit division and `min(share, rem)` compares both as signed, matching
//!   the spec's "signed integer division"; the result is stored as the
//!   heap's `u32` `contained`.
//! - **`AddRegionToZone` step 4** copies each old heap, with all its fields
//!   (including its old local `hx`, `hz`), to index `old.hz × newW + old.hx`
//!   (§4.4.4, quirk 4). The union rect only grows, so the index is always in
//!   range; an out-of-range copy is skipped *[choice: guard]*.
//! - **`PopulateZone`** tests "not a mask cell **at the start**" (§4.4.5)
//!   against a snapshot of the mask taken before the pass, so a cell the pass
//!   itself dropped is not also invalidated.
//! - **Saves** (§6.1): `Read` sets `loaded` before the version check and
//!   returns **true** regardless of stream errors [compat]; a failed count
//!   read uses 0 (the original leaves it uninitialised, quirk) and at most
//!   [`MAX_SAVED_ZONES`] zones are read. The zone list is cleared without
//!   releasing the old zones [quirk]: the core drops its `Rc`s, which is the
//!   same thing once nothing else holds one — the original's *leak* is not
//!   reproduced because it has no observable effect on simulation state.
//!   A zone whose heap record has a version other than 2 fails the zone read
//!   and is skipped (§6.2).
//! - **`levelAltitude` is not saved** (§2.2, §4.4.9): a loaded zone gets the
//!   centre-cell altitude at `Init` (quirk 3).
//! - **`GetLandfillZoneFromCell`** (§7.1 slot 12) is half-open, so the last
//!   row and column of a zone are never found (quirk 1). It returns the zone
//!   with no reference taken; the core returns an `Rc` clone and the plugin
//!   decides what to hand out (the city situation manager's raw-memory read
//!   of §9 is the plugin's problem).
//! - **`Release`** from a count ≤ 1 calls `Shutdown`, clears the zone list and
//!   marks the object destroyed (§1); the plugin drops it.
//!
//! Host callbacks must not call back into the landfill developer.

pub mod archive;
mod algo;
mod levelling;
mod occupant;
mod simulator;
#[cfg(test)]
mod tests;
mod tuning;
mod zone;

pub use algo::{
    altitude_variance, decay_amount, heap_capacity, magic_round, neighbour_mask, redistribute, redistribute_share,
    redrawn_cells, spiral_cells, tile_of, zoning_cost, Variance, TILE_RULES,
};
pub use levelling::{
    attempt_area_levelling, LevellingSlots, ModifyTerrainHeight, FLT_MAX, LEVELLING_REASON, SHIPPED_MAX_AREA_SLOPE,
    SLOPE_TEST_LIMIT,
};
pub use occupant::{
    LandfillOccupant, OccupantData, CLSID_LANDFILL_OCCUPANT, IID_LANDFILL_OCCUPANT, OCCUPANT_RECORD_VERSION,
    OCCUPANT_SLOTS, OCCUPANT_TYPE, TILE_EXEMPLAR,
};
pub use simulator::{answers_iid, DevState, LandfillDeveloper, DEVELOPER_RECORD_VERSION, MAX_SAVED_ZONES};
pub use tuning::{prop, shipped_tile_costs, shipped_tile_exemplar, shipped_tuning, Tuning, TUNING_KEY};
pub use zone::{Heap, LandfillZone, ZoneData, HEAP_RECORD_VERSION, ZONE_RECORD_VERSION};

use std::rc::Rc;

pub use crate::grid::CellRect;
use crate::property::PropertyHolder;
pub use crate::rng::CrtRand;
use crate::stream::RefValue;

/// Class ID (`GetGZCLSID`, §1).
pub const CLSID_LANDFILL_DEVELOPER: u32 = 0xEA09_D4AE;
/// Interface IDs answered by `QueryInterface` (§1).
pub const IID_UNKNOWN: u32 = 0x0000_0001;
pub const IID_LANDFILL_DEVELOPER: u32 = 0x8A09_D502;
pub const IID_MESSAGE_TARGET2: u32 = 0x4522_94AA;
pub const IID_SERIALIZABLE: u32 = 0xE4FD_A3D4;
/// `GetSimulatorType()` (slot 6, §1).
pub const SIMULATOR_TYPE: u32 = 0x2A39_8ED6;
/// The city component slot the developer occupies (foundations §3.6, §1).
pub const CITY_COMPONENT_SLOT: u32 = 12;
/// `cISC4City` slot 78 (`+0x138`), the getter other systems use (§1).
pub const CITY_GETTER_SLOT: u32 = 78;

/// The landfill zone type (§1).
pub const ZONE_TYPE_LANDFILL: i32 = 14;

/// Agent registration (§1.1 step 2, §3.1): one month-level agent, flags 2
/// (enabled once the city is established).
pub const AGENT_NAME: &str = "cSC4LandfillDeveloper";
pub const AGENT_LEVEL: u32 = 6;
pub const AGENT_FLAGS: u32 = 2;

/// Messages (§3.2, §5).
pub mod msg {
    /// Zoning: the zone manager after `PlaceZone` (`GetVoid2` = the region,
    /// `GetData3` = the zone type). Runs for every zone type.
    pub const ZONING_CHANGE: u32 = 0x0769_9455;
    /// PostCityInit.
    pub const POST_CITY_INIT: u32 = 0x26D3_1EC1;
    /// FundsChanged.
    pub const FUNDS_CHANGED: u32 = 0x0772_FAD4;
    /// Terrain modified (`data4` ∈ {1, 2, 3} are the terraforming tools).
    pub const TERRAIN_MODIFIED: u32 = 0x278E_6003;
    /// PreViewLevelChange (the 3D view already stores the new level).
    pub const PRE_VIEW_LEVEL_CHANGE: u32 = 0xC6CD_BF1A;
    /// SimNewMonth (the month agent's message).
    pub const SIM_NEW_MONTH: u32 = 0x6695_6816;
    /// Posted (§5): a history record, sent synchronously (`MessageSend`).
    pub const HISTORY_RECORD: u32 = 0x89EF_A536;
    /// The five subscriptions, in the order of §1.1 step 4 [compat].
    pub const SUBSCRIPTIONS: [u32; 5] = [
        ZONING_CHANGE,
        POST_CITY_INIT,
        FUNDS_CHANGED,
        TERRAIN_MODIFIED,
        PRE_VIEW_LEVEL_CHANGE,
    ];
    /// Occupant display notification of §4.5.2 (slot 15).
    pub const OCCUPANT_CHANGED: u32 = 0x99EF_1145;
}

/// History series posted each month (§5).
pub mod history {
    /// Landfill capacity (`capacityTotal`).
    pub const CAPACITY: u32 = 0x2A8C_4696;
    /// Total garbage: landfilled + uncollected + waiting.
    pub const TOTAL_GARBAGE: u32 = 0x2A8C_4697;
}

/// Occupant types (§4.4.5, §2.3).
pub const OCCUPANT_TYPE_LANDFILL: u32 = 0x6A11_BD2B;
pub const OCCUPANT_TYPE_BASE_TEXTURE: u32 = 0x497F_6D9D;
pub const OCCUPANT_TYPE_FLORA: u32 = 0x7475_8926;

/// Landfill tile exemplars (type `0x6534284A`, group `0x07BDDF1C`, §4.1).
pub mod tile {
    pub const TYPE: u32 = 0x6534_284A;
    pub const GROUP: u32 = 0x07BD_DF1C;
    /// Empty: never instantiated (§4.8.1 step 2).
    pub const EMPTY: u32 = 0x02F1_0000;
    /// One active heap at a corner.
    pub const CORNER: u32 = 0x034E_0000;
    /// Two adjacent active heaps (edge).
    pub const EDGE: u32 = 0x02F9_0000;
    /// Two diagonal active heaps.
    pub const DIAGONAL: u32 = 0x034F_0000;
    /// Three active heaps.
    pub const THREE: u32 = 0x02FB_0000;
    /// Four active heaps (interior): the only tile that draws `rand()`.
    pub const INTERIOR: u32 = 0x02FA_0000;
}

/// The budget item cost property of a tile exemplar (§4.1, §4.9.1).
pub const PROP_ITEM_COST: u32 = 0xEA54_D286;

/// Budget IDs (§1, §4.9).
pub mod budget_ids {
    /// The sanitation department, fetched on PostCityInit (§3.2).
    pub const SANITATION_DEPARTMENT: u32 = 0x6A42_A8F5;
    /// The landfill line item of that department (created by the pollution
    /// simulator, pollution §7).
    pub const LANDFILL_LINE: u32 = 30;
}

/// The not-connected warning prop (§4.7.5).
pub const ZOT_PROP: u32 = 0x107A_0000;
/// The zot's height offset above the terrain (§4.7.5, §4.1).
pub const ZOT_HEIGHT: f32 = 3.0;
/// The seagull effect's height above the terrain (§4.10).
pub const SEAGULL_HEIGHT: f32 = 64.0;

/// `FindNetworkTile`'s flag mask and flag (§4.4.5; the mask is flora §6.4.1's).
pub const NETWORK_TILE_MASK: u32 = 0x1F4F;

/// The height change above which a tile moves with the terrain (§4.5.2, §4.1).
pub const TERRAIN_MOVE_THRESHOLD: f32 = 0.25;

/// `GetEfficiency` with no maintenance (§4.9.2, §4.1 "default efficiency").
pub const DEFAULT_EFFICIENCY: f32 = 1.0;

/// The developer types `IsZoneConnected` asks the traffic simulator about,
/// in the original's order (§4.7.2): R$, R$$, R$$$, CS$, CS$$, CS$$$, CO$$,
/// CO$$$, ID, IM, IR, IHT.
pub const CONNECTIVITY_DEVELOPER_TYPES: [u32; 12] = [
    0x1010, 0x1020, 0x1030, 0x3110, 0x3120, 0x3130, 0x3320, 0x3330, 0x4200, 0x4300, 0x4100, 0x4400,
];

/// The `0x278E6003` `data4` reasons that are the terraforming tools (§3.2
/// *[inferred]*). The landfill's own levelling uses reason 5, which is
/// ignored.
pub const TERRAIN_TOOL_REASONS: [u32; 3] = [1, 2, 3];
/// The reason the landfill's own levelling passes (§4.4.2 step 3, §4.5.3).
pub const TERRAIN_REASON_LANDFILL: u32 = 5;

/// An inclusive cell rectangle plus a per-cell bit mask: the game's
/// `SC4CellRegion<long>` (§2.2). A cell is part of the region iff its mask
/// bit is set ("mask cells").
///
/// The mask is `(x1 − x0 + 1) × (z1 − z0 + 1)` cells, indexed
/// `(z − z0) × width + (x − x0)`. A rectangle with a non-positive side has no
/// cells.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CellRegion {
    pub x0: i32,
    pub z0: i32,
    pub x1: i32,
    pub z1: i32,
    mask: Vec<bool>,
}

impl CellRegion {
    /// A region over `rect` with every cell set (§6.2 read).
    pub fn all_set(rect: CellRect) -> CellRegion {
        let (w, h) = (Self::side(rect.x1, rect.x2), Self::side(rect.z1, rect.z2));
        CellRegion {
            x0: rect.x1,
            z0: rect.z1,
            x1: rect.x2,
            z1: rect.z2,
            mask: vec![true; (w * h) as usize],
        }
    }

    /// A region over `rect` with no cell set.
    pub fn cleared(rect: CellRect) -> CellRegion {
        let mut r = Self::all_set(rect);
        r.mask.iter_mut().for_each(|m| *m = false);
        r
    }

    /// A region from an explicit mask (row-major, `(z − z0) × width +
    /// (x − x0)`); a short mask is padded with cleared cells and a long one
    /// truncated.
    pub fn from_mask(rect: CellRect, mask: Vec<bool>) -> CellRegion {
        let mut r = Self::cleared(rect);
        for (d, s) in r.mask.iter_mut().zip(mask) {
            *d = s;
        }
        r
    }

    fn side(lo: i32, hi: i32) -> i32 {
        (hi - lo + 1).max(0)
    }

    pub fn rect(&self) -> CellRect {
        CellRect::new(self.x0, self.z0, self.x1, self.z1)
    }

    pub fn width(&self) -> i32 {
        Self::side(self.x0, self.x1)
    }

    pub fn height(&self) -> i32 {
        Self::side(self.z0, self.z1)
    }

    fn index(&self, x: i32, z: i32) -> Option<usize> {
        let (dx, dz) = (x.checked_sub(self.x0)?, z.checked_sub(self.z0)?);
        if !(0..self.width()).contains(&dx) || !(0..self.height()).contains(&dz) {
            return None;
        }
        Some((dz * self.width() + dx) as usize)
    }

    /// Whether `(x, z)` is a mask cell. A cell outside the rectangle is not.
    pub fn is_set(&self, x: i32, z: i32) -> bool {
        self.index(x, z).is_some_and(|i| self.mask[i])
    }

    pub fn set(&mut self, x: i32, z: i32, on: bool) {
        if let Some(i) = self.index(x, z) {
            self.mask[i] = on;
        }
    }

    /// Every cell of the rectangle, `x` outer and `z` inner (§4.2's iteration
    /// order, used by §4.3, §4.4.4, §4.4.5 and §6.2).
    pub fn cells(&self) -> impl Iterator<Item = (i32, i32)> + '_ {
        (self.x0..=self.x1).flat_map(move |x| (self.z0..=self.z1).map(move |z| (x, z)))
    }

    /// The mask cells, in [`CellRegion::cells`] order.
    pub fn mask_cells(&self) -> impl Iterator<Item = (i32, i32)> + '_ {
        self.cells().filter(move |&(x, z)| self.is_set(x, z))
    }

    /// The number of mask cells (§4.6.4).
    pub fn mask_count(&self) -> i32 {
        self.mask.iter().filter(|&&m| m).count() as i32
    }

    /// The mask as a plain vector, in [`CellRegion::cells`] order.
    pub fn mask(&self) -> &[bool] {
        &self.mask
    }

    /// Whether the two rectangles intersect, inclusively (§4.2: diagonal
    /// neighbours count).
    pub fn intersects(a: CellRect, b: CellRect) -> bool {
        a.x1 <= b.x2 && a.x2 >= b.x1 && a.z1 <= b.z2 && a.z2 >= b.z1
    }

    /// The inclusive intersection of two rectangles, or `None` when they do
    /// not intersect (§4.2).
    pub fn intersection(a: CellRect, b: CellRect) -> Option<CellRect> {
        Self::intersects(a, b).then(|| CellRect::new(a.x1.max(b.x1), a.z1.max(b.z1), a.x2.min(b.x2), a.z2.min(b.z2)))
    }

    /// The union of two rectangles (§4.4.4 step 2).
    pub fn union(a: CellRect, b: CellRect) -> CellRect {
        CellRect::new(a.x1.min(b.x1), a.z1.min(b.z1), a.x2.max(b.x2), a.z2.max(b.z2))
    }

    /// `rect` inflated by one cell on each side (§4.4.1 step 1).
    pub fn inflate(rect: CellRect) -> CellRect {
        CellRect::new(rect.x1 - 1, rect.z1 - 1, rect.x2 + 1, rect.z2 + 1)
    }

    /// The centre cell of a rectangle: `((x0 + x1) / 2, (z0 + z1) / 2)` with
    /// C integer division, toward zero (§4.2).
    pub fn centre_cell(rect: CellRect) -> (i32, i32) {
        ((rect.x1 + rect.x2) / 2, (rect.z1 + rect.z2) / 2)
    }
}

/// How a compiled slot returns its result (§7.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotReturn {
    /// Only AL is meaningful.
    Al,
    /// EAX (a count, an integer or a pointer).
    Eax,
    /// A 64-bit integer in EDX:EAX.
    EdxEax,
    /// An `SC4Percentage` by value through a hidden first stack argument;
    /// EAX = that pointer (§7.4).
    HiddenPtr,
    /// Nothing meaningful (the outputs are the caller's references).
    None,
}

/// One row of a compiled slot table: (slot, method, callee-pop bytes, return
/// channel).
pub type SlotRow = (u32, &'static str, u32, SlotReturn);

/// `cISC4LandfillDeveloper` (§7.1, vtable `0x00AB157C`, 13 slots). Slot 9's
/// `int64` return and slot 5's hidden pointer are **SDK errors** (§7.1,
/// §7.4): the SDK declares `int32 GetZoningCost` and
/// `SC4Percentage GetEfficiency()`.
pub const DEVELOPER_SLOTS: [SlotRow; 13] = {
    use SlotReturn::*;
    [
        (0, "QueryInterface", 8, Al),
        (1, "AddRef", 0, Eax),
        (2, "Release", 0, Eax),
        (3, "Init", 0, Al),
        (4, "Shutdown", 0, Al),
        (5, "GetEfficiency", 4, HiddenPtr),
        (6, "GetSimulatorType", 0, Eax),
        (7, "GetMaintenanceCosts", 0, EdxEax),
        (8, "GetCurrentBudget", 0, EdxEax),
        (9, "GetZoningCost", 4, EdxEax),
        (10, "GetLandfillUsage", 8, None),
        (11, "GetLandfillTileCount", 0, Eax),
        (12, "GetLandfillZoneFromCell", 8, Eax),
    ]
};

/// A message as `DoMessage` sees it (§3.2).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LandfillMessage {
    pub msg_type: u32,
    pub data1: u32,
    pub data2: u32,
    pub data3: u32,
    pub data4: u32,
    /// `GetVoid2()` of the zoning message: the zoning region. The region is
    /// the message's; the core neither keeps nor frees it.
    pub region: Option<CellRegion>,
}

impl LandfillMessage {
    pub fn of_type(msg_type: u32) -> LandfillMessage {
        LandfillMessage {
            msg_type,
            ..LandfillMessage::default()
        }
    }
}

/// The zone effects `InitZoneEffects` created (§4.10). Rendering only; the
/// core keeps the tokens so the message gating of §3.2 is reproduced.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ZoneEffects {
    /// The garbage-truck automaton (`landfillzone_out`), if any.
    pub truck: Option<u64>,
    /// The "flock_of_seagulls" effect, if any.
    pub seagull: Option<u64>,
    /// Whether the seagull effect is started (§3.2 PreViewLevelChange).
    pub seagull_started: bool,
}

/// Everything the landfill developer and its zones read from, or do to, the
/// rest of the game (§8). Objects outside the three landfill classes are
/// host tokens. Re-entrancy: callbacks must not call back into the developer.
pub trait LandfillHost {
    // ---- lifecycle (§1.1, §1.2) ----
    /// `Init` step 3: acquire (with `AddRef`) the city, terrain, pollution
    /// simulator, budget simulator and message server.
    fn acquire_services(&self);
    /// `Shutdown`: release the message server, the department, the budget,
    /// pollution, terrain and the city (§1.2).
    fn release_services(&self);
    /// Simulator slot 19 `AddAgent(target, 6, "cSC4LandfillDeveloper", 2)`.
    fn add_agent(&self, level: u32, name: &str, flags: u32);
    /// Simulator slot 20 `RemoveAgent(target)`; the simulator is taken from
    /// the city (`cISC4City` slot 50).
    fn remove_agent(&self);
    /// Message-server subscribe / unsubscribe, one type at a time (§1.1
    /// step 4, §1.2).
    fn subscribe(&self, msg_type: u32);
    fn unsubscribe(&self, msg_type: u32);
    /// The "LandfillDeveloper" tuning exemplar ([`TUNING_KEY`]), if loaded
    /// (§4.1).
    fn tuning(&self) -> Option<Rc<PropertyHolder>>;
    /// An exemplar by key (`cISCResExemplar` as a property holder): the tile
    /// exemplars of §4.1 and §4.8.1 step 3.
    fn exemplar(&self, type_id: u32, group: u32, instance: u32) -> Option<Rc<PropertyHolder>>;

    // ---- city (§8) ----
    /// City slots 93 / 94: the cell counts.
    fn cell_counts(&self) -> (i32, i32);
    /// City slot 97 `CellCenterToPosition(x, z)` (metres).
    fn cell_center_to_position(&self, x: i32, z: i32) -> (f32, f32);

    // ---- terrain (`cISTETerrain`) ----
    /// Slot 13 `GetAltitude(px, pz)` at the cell centre of `(x, z)` (§4.2).
    fn cell_altitude(&self, x: i32, z: i32) -> f32;
    /// Slot 19 `GetAltitudeAtNearestGrid(px, pz)` at a world position
    /// (§4.5.2).
    fn altitude_at_nearest_grid(&self, px: f32, pz: f32) -> f32;
    /// Slot 19 at the cell centre of `(x, z)` (§4.7.5: the zot sits at the
    /// cell centre).
    fn cell_altitude_at_nearest_grid(&self, x: i32, z: i32) -> f32;
    /// Slot 29 `GetSeaLevel()`.
    fn sea_level(&self) -> f32;
    /// Slot 49 `SetAltitude(rect, altitude, 1, 5)` (the `direct` path of
    /// §4.5.3, which the shipped code never takes).
    fn set_altitude(&self, rect: CellRect, altitude: f32);
    /// Slot 68 `RedisplayTerrain(1, 1, &rect, 5)` (§4.4.2 step 3; display
    /// only).
    fn redisplay_terrain(&self, rect: CellRect);

    // ---- pollution simulator (`cISC4PollutionSimulator`) ----
    /// Slot 20 `GetGarbageSupply()`.
    fn garbage_supply(&self) -> u32;
    /// Slot 21 `GetLandfillTileCapacity()` (pollution tuning `0x4A65267A`,
    /// shipped 30000); read each time it is needed (§4.1).
    fn landfill_tile_capacity(&self) -> u32;
    /// Slot 24 `GetTotalGarbageUncollected()` (signed; §4.7.1 clamps it at 0).
    fn total_garbage_uncollected(&self) -> i32;
    /// Slot 29 `AddToGarbageSupply(delta)`: the monthly intake (negative) and
    /// the spill of §4.6.2 (positive).
    fn add_to_garbage_supply(&self, delta: i32);
    /// Slot 30 `SetGarbageCapacity(avail)` (§4.7.1 step 5).
    fn set_garbage_capacity(&self, available: i32);
    /// Slot 50 `SetIsActiveLandfill(x, z, on)`: bit 15 of the garbage cell
    /// (§4.8.2). It is set only on cells of zone type 14.
    fn set_is_active_landfill(&self, x: i32, z: i32, on: bool);
    /// Slot 51 `GarbagePutInLandfill(n)`; the result is ignored. Called with
    /// `n = 0` too (§4.7.2 step 3.3).
    fn garbage_put_in_landfill(&self, n: i32);

    // ---- budget (§4.9) ----
    /// Slot 28 `GetDepartmentBudget(0x6A42A8F5)` (borrowed; the developer
    /// `AddRef`s it, §3.2 PostCityInit). `None` when there is none.
    fn get_department_budget(&self, department: u32) -> Option<u64>;
    /// `AddRef` / `Release` on the held department (§3.2, §1.2).
    fn add_ref(&self, token: u64);
    fn release(&self, token: u64);
    /// Department slot 10 `*GetFundingPercentage()` (a fraction; §3.2
    /// FundsChanged compares it strictly against 0.0).
    fn department_funding_percentage(&self, department: u64) -> f32;
    /// Line item slot 11 `GetCurrentExpenses()`; `None` without the line
    /// item (§4.9.2).
    fn line_item_current_expenses(&self, department: u64, line: u32) -> Option<i64>;
    /// Line item slot 12 `SetFullExpenses(v)` (§4.9.3: `GetLineItem(30)`,
    /// `AddRef`, set, `Release`).
    fn set_line_item_full_expenses(&self, department: u64, line: u32, value: i64);
    /// The first `s64` value of property [`PROP_ITEM_COST`] of an occupant's
    /// exemplar holder (§4.1, §4.9.1); `None` when absent.
    fn occupant_item_cost(&self, occupant: u64, property: u32) -> Option<i64>;

    // ---- zone manager (`cISC4ZoneManager`) ----
    /// Slot 6 `GetZoneCount(type)`: the number of zone-`type` cells in the
    /// city, not limited to connected zones or mask cells (§4.6.5 slot 11).
    fn zone_count(&self, zone_type: i32) -> i32;
    /// Slot 12 `GetZoneType(x, z)`.
    fn zone_type(&self, x: i32, z: i32) -> i32;
    /// Slot 11 `PlaceZone(region, zoneType, placeZone, skipFundsCheck,
    /// checkAreaSlope, skipZoneSizeCheck, checkLotSize, null, null, 0)`.
    /// §4.4.8 step 4 calls it for a 1×1 region with zone type 0 and
    /// `(true, true, false, true, false)`. The zone manager **posts**
    /// `0x07699455` for the cell (§4.4.8 *[compat]*): the host delivers it to
    /// [`LandfillDeveloper::do_message`] later, never from inside this call.
    fn place_zone(&self, region: &CellRegion, zone_type: i32) -> bool;

    // ---- occupant manager (`cISC4OccupantManager`) ----
    /// Slot 33 `find(&occ, x, z, type)`: the occupant of that type at the
    /// cell, if any.
    fn occupant_at(&self, x: i32, z: i32, occupant_type: u32) -> Option<u64>;
    /// Slot 20 `insert(occupant, 0)` of a landfill tile the core just built
    /// (§4.8.1 step 3); its result is `CreateLandfillOccupant`'s. The host
    /// wraps the core object for the occupant manager and gives it its token
    /// ([`LandfillOccupant::set_token`]), which later `occupant_at` calls
    /// return.
    fn insert_landfill_occupant(&self, occupant: &Rc<LandfillOccupant>, x: i32, z: i32) -> bool;
    /// Slot 21 `remove(occupant, 1, 3)` (§4.8.3).
    fn occupant_remove(&self, occupant: u64) -> bool;
    /// Slot 24 `(occupant, 1)`: re-index a moved occupant (§4.5.2
    /// *[inferred]*).
    fn occupant_changed(&self, occupant: u64);

    // ---- occupants (`cISC4Occupant`, `cISC4LandfillOccupant`) ----
    /// Slot 8 `GetPosition()` (world x, y, z).
    fn occupant_position(&self, occupant: u64) -> [f32; 3];
    /// Slot 9 `SetPosition(p)`.
    fn occupant_set_position(&self, occupant: u64, position: [f32; 3]);
    /// Slot 12 `SetRemovalFlags(flags)` (the zot, §4.7.5).
    fn occupant_set_removal_flags(&self, occupant: u64, flags: u32);
    /// Slot 15 `(message, 0)` (§4.5.2's `0x99EF1145` display notification
    /// *[inferred]*).
    fn occupant_post_message(&self, occupant: u64, message: u32);
    /// Whether the occupant answers `cISC4LandfillOccupant` (`0x2A15F262`,
    /// §4.5.2 step 2).
    fn is_landfill_occupant(&self, occupant: u64) -> bool;
    /// `cISC4LandfillOccupant` slot 9 `IsEmptyLandfillTile()` (§4.5.2, §7.3;
    /// nothing in the shipped code sets it true, spec open question 7).
    fn is_empty_landfill_tile(&self, occupant: u64) -> bool;
    /// §4.4.5 / §4.8.1: `RemoveIncompatibleOccupants(x, z)` (removes flora
    /// [`OCCUPANT_TYPE_FLORA`] in the cell).
    fn remove_incompatible_occupants(&self, x: i32, z: i32);
    /// §4.4.5 / §4.8.1: `SetEmptyCellTexture(x, z)` (inserts a
    /// `cSC4LotBaseTextureOccupant` with the landfill zone texture).
    fn set_empty_cell_texture(&self, x: i32, z: i32);
    /// §4.4.3 / §4.4.8: `RemoveBaseTexture(x, z)`.
    fn remove_base_texture(&self, x: i32, z: i32);

    // ---- prop manager (`cISC4PropManager`) ----
    /// Slot 26 `AddLotProp(propID, position, orientation)` (the `(propID,
    /// pos, orientation)` overload): the zot occupant, `None` on failure
    /// (§4.7.5).
    fn add_lot_prop(&self, prop_type: u32, position: [f32; 3], orientation: i32) -> Option<u64>;
    /// Slot 29 `RemovePropA(occupant)` (§4.7.5, §4.4.4 step 1, §4.4.8
    /// step 1).
    fn remove_prop(&self, occupant: u64) -> bool;
    /// The zot's orientation, read from a global view service (§4.7.5, spec
    /// open question 6).
    fn zot_orientation(&self) -> i32;

    // ---- traffic (§4.4.5, §4.7.2) ----
    /// Traffic network map slot 8 `FindNetworkTile(x, z, 0x1F4F, true)`: a
    /// network tile at the cell, if any (§4.4.5).
    fn find_network_tile(&self, x: i32, z: i32, flag_mask: u32, flag: bool) -> Option<u64>;
    /// Traffic simulator slot 41 `GetSubnetworksInRegion(region, &ids)`: the
    /// subnetwork IDs touching `region` (§4.7.2).
    fn subnetworks_in_region(&self, region: &CellRegion) -> Vec<u32>;
    /// Traffic simulator slot 42 `GetOccupantCountForAllSubnetworks(type,
    /// &counts)`: the per-subnetwork developed-occupant counts of a developer
    /// type (§4.7.2).
    fn occupant_count_for_all_subnetworks(&self, developer_type: u32) -> Vec<u32>;

    // ---- lot developer (§4.5.3) ----
    /// `buildingDevelopment.GetLotDeveloper().AttemptAreaLevelling(rect, f,
    /// withWater, waterFlag, maxAreaSlope, null)` (lot developer slot 21).
    /// `LevelRect` passes `(rect, 0.0, true, true, FLT_MAX)` and ignores the
    /// result. What the slot does is [`attempt_area_levelling`] (§4.5.3
    /// *[compat]*): `f` is never read, the terrain is levelled to the vertex
    /// average, and a slope above 40 refuses.
    fn attempt_area_levelling(
        &self,
        rect: CellRect,
        f: f32,
        with_water: bool,
        water_flag: bool,
        max_area_slope: f32,
    ) -> bool;

    // ---- 3D view (§3.2) ----
    /// `cISC4View3DWin` slot 22 `GetCurrentViewLevel()`: the view stores the
    /// **new** level before sending PreViewLevelChange.
    fn current_view_level(&self) -> u32;

    // ---- zone effects (§4.10; rendering only) ----
    /// `InitZoneEffects`: the two automata controllers (named by two string
    /// statics, presumably `landfillzone_attract` / `landfillzone_out`) placed
    /// at the first road cell next to the zone, and the "flock_of_seagulls"
    /// effect at the centre cell, [`SEAGULL_HEIGHT`] m above the terrain.
    fn init_zone_effects(&self, centre: (i32, i32), rect: CellRect) -> ZoneEffects;
    /// `ShutdownZoneEffects`: stop and release them.
    fn shutdown_zone_effects(&self, effects: ZoneEffects);
    /// §3.2 FundsChanged: enable or disable the truck automaton.
    fn set_truck_enabled(&self, truck: u64, enabled: bool);
    /// §3.2 PreViewLevelChange: `Start(0)` / `Stop(1)` on the seagull effect.
    fn start_seagull(&self, seagull: u64, flag: u32);
    fn stop_seagull(&self, seagull: u64, flag: u32);

    // ---- history and messages (§5) ----
    /// §5: `MessageSend` of a new `cRZMessage2Standard` `0x89EFA536` with
    /// `data1 = series` and `data2 = value` (sent synchronously, not posted).
    fn send_history(&self, series: u32, value: u32);

    // ---- persistence (§6) ----
    /// The save reference of a zot prop (§6.2: class `cISC4Occupant`
    /// `0x26BB767E` on read).
    fn occupant_ref(&self, occupant: u64) -> RefValue;
    /// The token of a reference read with IID [`IID_OCCUPANT`], transferring
    /// the reference the stream handed over; `None` drops it (§6.2).
    fn resolve_occupant(&self, r: &RefValue) -> Option<u64>;
}

/// `cISC4Occupant`, the IID a zot reference is read with (§6.2).
pub const IID_OCCUPANT: u32 = 0x26BB_767E;
