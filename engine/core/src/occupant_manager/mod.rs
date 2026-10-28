// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The occupant manager (`cSC4OccupantManager`, class `0x098F964D`),
//! `docs/specs/occupant-manager.md`. Section references (`§n`) point at that
//! spec.
//!
//! The city's spatial index of occupants: a fixed 192 × 192 grid of 64 m
//! world cells, each holding a newest-first list of the occupants whose
//! manager box touches it (§3, §4).
//!
//! # Structure
//!
//! - `cells.rs`: the coordinate systems and every conversion of §3 with the
//!   exact single-precision formulas (slot 12's bug included, §15 quirk 1),
//!   the occupant box of §5.1 steps 5–6, the overlap test of §8.2 and
//!   `GetBoundingCells` (§6.2).
//! - `manager.rs`: the object: lifecycle (§2), the grid (§4), insert /
//!   remove / move (§5), the cell helpers (§6), the queries (§7–§10) and the
//!   save record (§13).
//! - `tests.rs`: a fake occupant world and the §18 tests.
//!
//! The occupants are host objects: opaque `u64` tokens (0 = null) with a
//! bounding box, a type, flags, a stored manager box and the base `Init` /
//! `Shutdown` / `PostOccupantMessage` behaviour of §5.6. Everything the
//! manager asks of them goes through [`OccupantHost`]. Occupant filters are
//! host objects too ([`Filter::Host`]), except the manager's own shared
//! one-type filter ([`Filter::Type`], §4.4).
//!
//! # Threading (§14), for hosts
//!
//! The original guards the grid with one recursive critical section. The
//! core reproduces **where** it is held by calling [`OccupantHost::lock`] /
//! [`OccupantHost::unlock`] at exactly the points of the §14 table (see
//! [`slot_locking`]):
//!
//! - slots 1–5, 20–22, 24, 27, 28, 30, 32, 34, 36 and 38 hold it for the
//!   whole call. Insert (§5.1) takes it after the flag-1 check and drops it
//!   before `SetFlag` and the Insert message; Remove (§5.2) holds it only
//!   around the cell update; Move (§5.4) drops it before the Move message;
//!   RemoveOccupants (§5.3) drops it before the messages, `Shutdown` and
//!   `Release` calls;
//! - slot 42 (`IterateOccupantsByBBox`) takes it per cell and **releases it
//!   around the callbacks**;
//! - slot 44 (`IterateOccupants`) holds the outer lock for the whole walk,
//!   callbacks included; each cell's batch is collected under an inner
//!   hold that is dropped before the callbacks (OM7), so callbacks run at
//!   depth 1; the lock must be re-entrant;
//! - slots 40, 47 and the typed thunks take none of their own (they reach a
//!   locking slot); slots 6–19, 25, 26, 48, `Write` and `Read` take none in
//!   the original (OM8). [choice] The core takes it for slot 25
//!   (`IsCellEmpty` reads the grid) and for `Write` / `Read`: the grid's
//!   `RefCell` must only be touched under the lock, and OM8 notes the extra
//!   hold is not observable (same thread, no callbacks inside).
//!
//! The host implements the hooks with a recursive lock (the capi does) or
//! leaves them empty (a single-threaded host). The grid lives in a
//! `RefCell` that the core touches only between `lock` and `unlock`; the
//! state that unlocked slots read (the initialised flag, the sizes and
//! counts, the shared filter's type) is atomic. A host that calls the
//! manager from several threads therefore needs only the recursive lock.
//!
//! Other host-facing notes:
//!
//! - No `RefCell` borrow is held across a host call, so occupant callbacks
//!   (a synchronous Move handler, a filter, an iteration callback) may call
//!   back into the manager.
//! - [choice] The typed overloads (slots 23, 29–35, 37–41, 43, 45, 46) write
//!   their type into the shared filter's state (§10: the value persists and
//!   is observable) but pass the type to the walk **by value**. A typed call
//!   nested in a typed walk's callback therefore does not change the outer
//!   walk's type: the logged deviation of §14.
//! - The duplicate sets of §8.2, §9.1 and §13 are local to each call, so
//!   nested queries are safe.
//! - Every conversion uses the 24-bit (`float`) arithmetic of the game's
//!   main thread (foundations §18). In the original, a query from a 53-bit
//!   worker thread (OM1) could round differently; the core does not model
//!   that.
//!
//! # Choices (`[choice]`) and literal readings (`[question]`)
//!
//! - **Crash paths** of the original (§5.7, OM6: the original crashes or
//!   reads wild memory; the choices below were accepted) are given safe
//!   results:
//!   - slot 23 before `Init` / after `Shutdown` (null shared filter; the
//!     original writes to address `0xC`): nothing is removed and the result
//!     is true;
//!   - Insert before `Init` (the original crashes on the null row array
//!     after Create and the stored box): it inserts no nodes and succeeds;
//!   - a cell index outside the grid (Insert's unclamped bytes from 192 to
//!     255 inside a running loop, Remove / Move / slots 28 and 32 out of the
//!     grid, a smaller grid after a `Read`): the cell is skipped. Insert
//!     still sets flag 1 and posts its message, like a cell whose holder
//!     could not be allocated (§5.1 step 9);
//!   - FindOccupant without a property holder (an uninitialised guess,
//!     §6.3): no guess; the full scan runs when asked.
//!
//!   Defined by the original and reproduced (OM6 MATCHES): a wrapped box
//!   (low byte > high byte, e.g. a low corner below −4096 m stored as 255)
//!   visits no cells: Insert succeeds with no nodes, sets flag 1 and posts
//!   Insert; Remove returns true and posts Remove (§18 test 22). After
//!   `Shutdown` the rows exist and Insert / Remove / Move work normally.
//! - **`Read`** is atomic: on a stream error nothing changes and the
//!   references kept so far are released; the "was read" flag is set
//!   either way (§13 step 1). The old grid is replaced without releasing its
//!   entries (`Read` runs on a fresh object). A grid of any size is read;
//!   rows and cells are allocated as they are read, so a corrupt count
//!   cannot allocate more than the stream holds.
//! - **The 0.1 m shrink and the 1.0 m width** (§5.1 step 5, §7 slots 32 /
//!   34, §8.4) are `float` constants and every shrunk bound is a `float`
//!   (OM4); slot 34 clamps a given corner to `0..=count − 1` and turns a
//!   null max into (count, count) (OM5).
//! - **The single-cell bit** (§4.2) is kept exactly like the original:
//!   multi-cell queries and walks skip the duplicate check for such nodes,
//!   and slot 44's single-cell walk has no duplicate check (OM7).

pub mod archive;
mod cells;
mod manager;

#[cfg(test)]
mod tests;

pub use cells::{
    bounding_cells, cell_box_of, occupant_manager_cell_to_position, occupant_manager_cell_to_world_cell, overlaps,
    position_to_occupant_manager_cell, position_to_standard_city_cell, position_to_world_cell, shrink,
    standard_city_cell_to_position, standard_city_cell_to_world_cell, world_cell_to_occupant_manager_cell,
    world_cell_to_position, world_cell_to_standard_city_cell, BBox, CellBox, Vec3,
};
pub use manager::{query_interface, Interface, Node, OccupantManager, OccupantSnapshot};

use crate::stream::RefValue;

/// Class ID (§1).
pub const CLSID_OCCUPANT_MANAGER: u32 = 0x098F_964D;
/// Interfaces answered by `QueryInterface` (§1).
pub const IID_UNKNOWN: u32 = 0x0000_0001;
pub const IID_OCCUPANT_MANAGER: u32 = 0xC6BB_8334;
pub const IID_SERIALIZABLE: u32 = 0xE4FD_A3D4;
pub const IID_MESSAGE_TARGET2: u32 = 0x4522_94AA;
/// Saved occupants are read with this IID (§13).
pub const IID_OCCUPANT: u32 = 0x26BB_767E;
/// The shared one-type filter (§4.4): its class and the interface it answers.
pub const CLSID_TYPE_FILTER: u32 = 0xE9C7_5ED1;
pub const IID_OCCUPANT_FILTER: u32 = 0x46A8_0026;

/// City component and Init order (§1).
pub const CITY_COMPONENT: u32 = 4;

/// The fixed grid (§3.1): 192 × 192 world cells of 64 m; world cell 64 is
/// the city's first.
pub const WORLD_CELLS: i32 = 192;
pub const CELL_SIZE: f32 = 64.0;
pub const CITY_ORIGIN_CELL: i32 = 64;
/// Standard city cells are 16 m (§3.1).
pub const STANDARD_CELL_SIZE: f32 = 16.0;

/// The record version (§13).
pub const RECORD_VERSION: u16 = 1;

/// Occupant flag 1: "in the occupant manager" (§5.6).
pub const FLAG_INSERTED: u32 = 1;

/// Occupant messages (§5.5).
pub mod msg {
    /// Posted by the occupant's first `Init` (§5.6).
    pub const CREATE: u32 = 0x99EF_1140;
    /// Posted by the occupant's `Shutdown` (§5.6).
    pub const DESTROY: u32 = 0x99EF_1141;
    pub const INSERT: u32 = 0x99EF_1142;
    pub const REMOVE: u32 = 0x99EF_1143;
    /// Posted by callers themselves (§5.4), never by the manager.
    pub const CHANGE: u32 = 0x99EF_1145;
    pub const MOVE: u32 = 0x99EF_1146;
}

/// An occupant filter as the manager passes it on (§4.4, §10).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Filter {
    /// A null filter: everything is included.
    All,
    /// A `cISC4OccupantFilter` owned by the caller (a host token):
    /// `IsOccupantIncluded` (filter slot 3) decides.
    Host(u64),
    /// The shared one-type filter with this type: included when the
    /// occupant's `GetType()` equals it (type 0 included literally).
    Type(u32),
}

/// How a slot takes the manager's lock (§14).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Locking {
    /// The whole call.
    WholeCall,
    /// Per cell, released around the callbacks (slot 42).
    PerCell,
    /// The whole walk, callbacks included (slot 44).
    WholeWalk,
    /// No lock of its own; it reaches a locking slot.
    Delegated,
    /// None.
    None,
}

/// The §14 table: how slot `slot` of `cISC4OccupantManager` locks.
pub fn slot_locking(slot: u32) -> Locking {
    match slot {
        1..=5 | 20..=22 | 24 | 27 | 28 | 30 | 32 | 34 | 36 | 38 => Locking::WholeCall,
        42 => Locking::PerCell,
        44 => Locking::WholeWalk,
        23 | 29 | 31 | 33 | 35 | 37 | 39..=41 | 43 | 45..=47 => Locking::Delegated,
        _ => Locking::None,
    }
}

/// Everything the manager asks of the rest of the game. Occupants and
/// filters are host tokens. Callbacks may call back into the manager (no
/// borrow is held across them).
pub trait OccupantHost {
    // ---- the manager's lock (§14) ----
    /// Enter the manager's recursive lock (default: nothing).
    fn lock(&self) {}
    /// Leave it (default: nothing).
    fn unlock(&self) {}

    // ---- references ----
    fn add_ref(&self, occ: u64);
    fn release(&self, occ: u64);

    // ---- the occupant (`cISC4Occupant`) ----
    /// Slot 3 `Init`: the base posts Create on the first call, unless bit 1
    /// of `+0x2E` is set (a loaded occupant, set by its own `Read`): then it
    /// calls occupant slot 17 (0, 0) instead (§5.6, §13).
    fn init(&self, occ: u64);
    /// Slot 4 `Shutdown`: the base posts Destroy when initialised (§5.6).
    fn shutdown(&self, occ: u64);
    /// Slot 6 `AsPropertyHolder()` is non-null.
    fn has_property_holder(&self, occ: u64) -> bool;
    /// Slot 7 `GetType()`.
    fn occupant_type(&self, occ: u64) -> u32;
    /// Slot 10 `GetBoundingBox(min, max)`: the occupant's real box.
    fn bounding_box(&self, occ: u64) -> BBox;
    /// Slot 15 `PostOccupantMessage(type, data4)` (flora §9.1): queued, or
    /// sent at once for a Move of an occupant whose `+0x30` is set.
    fn post_message(&self, occ: u64, msg_type: u32, data4: u32);
    /// Slot 25 `GetOccupantManagerBBox`: `None` when it fails (the "no box"
    /// mark, §4.3).
    fn manager_bbox(&self, occ: u64) -> Option<CellBox>;
    /// Slot 26 `SetOccupantManagerBBox`.
    fn set_manager_bbox(&self, occ: u64, b: CellBox);
    /// Slots 29, 31 and 32: `SetFlag`, `ClearFlag`, `IsFlagSet`.
    fn set_flag(&self, occ: u64, flag: u32);
    fn clear_flag(&self, occ: u64, flag: u32);
    fn is_flag_set(&self, occ: u64, flag: u32) -> bool;

    // ---- filters ----
    /// `filter.IsOccupantIncluded(occ)` (filter slot 3) of a caller's filter.
    fn filter_includes(&self, filter: u64, occ: u64) -> bool;

    // ---- save references (§13) ----
    /// The reference `SC4WriteGZSerializable` writes for `occ`.
    fn object_ref(&self, occ: u64) -> RefValue;
    /// `SC4ReadGZSerializable(stream, iid, &p, 0)`: the occupant, holding one
    /// reference for the caller, or `None` when it does not resolve.
    fn resolve_ref(&self, r: &RefValue, iid: u32) -> Option<u64>;
}
