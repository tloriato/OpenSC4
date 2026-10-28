// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The terrain (`docs/specs/terrain.md`, `cSTETerrain`, CLSID `0xE98F9525`)
//! and the simulation side of its surface water (`cSTESurfaceWater`, CLSID
//! `0x0B8DA7DB`): the height field, the per-vertex water depths, the height
//! and water queries, the height setters with their depth bookkeeping, the
//! change messages, kind-0 brushes, navigability and the bodies of water.
//! Section references (`§n`) point at that spec.
//!
//! # Structure
//!
//! - [`Terrain`] (`field.rs`): the grid and state of §2, the bounds of §3, the
//!   height queries of §4, the water queries of §5, the setters of §6, the
//!   flags of §7, `RedisplayTerrain` and the messages of §8, and the records
//!   of §13 (`records.rs`).
//! - Brushes (`brush.rs`): the descriptions of §9.1 with their walk and
//!   stroke angles, `ApplyBrush` (§9.2, §9.3) with the scan-converted
//!   footprint (`ComputeSpansForRectangle`), and the component kinds 0, 2, 5
//!   and 7 (§9.4, §9.4.1).
//! - The volcano lava (`lava.rs`): the start, the flow step, the fire
//!   thresholds and the burning draws of §11.
//! - Surface water (`water.rs`): the cell queries and navigability of §10.2
//!   and §10.4 over a [`NavigationHost`], and [`BodiesOfWater`] (§10.3).
//!
//! Messages are not sent from inside an edit: every call that posts appends
//! a [`Post`] to the terrain's outbox, which the caller drains with
//! [`Terrain::take_posts`] and forwards with `MessagePost` (queued, §8.1), in
//! order. This lets a C ABI wrapper post them after it released its lock.
//!
//! # Precision (foundations §18)
//!
//! Every float operation the spec writes as `r24` is computed in `f64` from
//! `f32` operands and rounded once with [`crate::x87::r24`]. Truncations are
//! `x87::ftol32`.
//!
//! # Choices made where the spec is open (`[choice]`)
//!
//! Each is tagged in the code. Round 2 of the spec (re-answers17, TR3-TR18)
//! settled most of the first round's; what remains: a flat index past the
//! arrays in the unchecked slots of §10.5 reads 0.0 (0, a zero vector) and
//! a write does nothing (accepted by TR12); the texture message's altitude
//! band is the plain minimum and maximum over its rectangle (§8.1, allowed
//! for a view-less replacement); the brush descriptions are kept per terrain
//! and the host carries their angles across cities (TR8); and the open
//! points TR19-TR21 (kind 2's out rectangle, kind 7, the vertex of
//! `GetSedimentValue`).

mod brush;
mod field;
mod lava;
mod records;
mod view;
mod water;

#[cfg(test)]
mod tests;

pub use brush::{
    shipped_brush, BrushBitmap, BrushComponent, BrushDescription, BrushParams, ADD_BRUSH_MAX_COMPONENTS,
    DEFAULT_TALUS_ANGLE, DEFAULT_WALK_STEP, MAX_COMPONENT_KIND, TWO_PI_F32,
};
pub use field::{Post, Terrain, TerrainTuning, VertRect, WaterVertex, MAX_CELLS};
pub use lava::{FireMode, Lava, LavaHost, LavaTuning, BURN_DRAWS_PER_TICK, BURN_HALF_WIDTH, NEIGHBOURS};
pub use records::{RecordError, ALTITUDES_VERSION, EDGE_VERSION, MISC_VERSION, PROPERTIES_VERSION};
pub use records::{SurfaceWaterRecord, TerrainRecord, TerrainViewRecord};
pub use view::{
    cell_triangle_indices, PlainViewRow, TerrainViewHeights, TerrainViewIndices, ViewHeightError, ViewRowCall,
    ViewVertexBuffer,
};
pub use view::{
    coordinate_bounds, edge_triangle_indices, plumbing_calls, row_visibility, widen_row_intervals, ExtensionBuffer,
    ExtensionFlushCall, PlumbingCall, TerrainViewLifecycle, TerrainViewShared, TextureCoordinateCache,
    ViewColumnInterval, ViewVertex,
};
pub use water::{compute_bodies, BodiesOfWater, Bridge, NavigationHost, OpenWater};

/// Class and interface IDs (§1).
pub const CLSID_TERRAIN: u32 = 0xE98F_9525;
pub const IID_TERRAIN: u32 = 0xAB95_3253;
/// `cISTETerrain`, the component IID.
pub const IID_TERRAIN_COMPONENT: u32 = 0xC70B_D37E;
pub const CLSID_SURFACE_WATER: u32 = 0x0B8D_A7DB;
/// The terrain is city component 49 (§1).
pub const CITY_COMPONENT: u32 = 49;

/// Cell width in metres (`CellWidth`, slot 6).
pub const CELL_WIDTH: f32 = 16.0;
/// `GetDefaultTerrainRaiseLowerAmount` (slot 47).
pub const DEFAULT_RAISE_LOWER_AMOUNT: f32 = 20.0;
/// `HasDraftClearance`: the cell depth must exceed this (§10.2).
pub const DRAFT_CLEARANCE: f32 = 5.0;
/// `IsNavigable` step 4: the bridge deck clearance (§10.4).
pub const BRIDGE_CLEARANCE: f32 = 20.0;

/// Messages (§8).
pub mod msg {
    /// Terrain heights changed (`RedisplayTerrain` with the terrain flag).
    pub const TERRAIN_CHANGED: u32 = 0x278E_6003;
    /// Water heights changed (`RedisplayTerrain` with the water flag).
    pub const WATER_CHANGED: u32 = 0x678E_6E3A;
    /// Texture redisplay (render only; always posted when the view is ready).
    pub const TEXTURE_REDISPLAY: u32 = 0x870B_C918;
    pub const BEGIN_LONG_OPERATION: u32 = 0xEA3B_0AF1;
    pub const END_LONG_OPERATION: u32 = 0x6A3B_0AFD;
    pub const SEA_LEVEL_CHANGED: u32 = 0x8917_1496;
    pub const TERRAIN_TYPE_CHANGED: u32 = 0xC922_EB91;
    pub const WATER_DISPLAY_STATE: u32 = 0x6AB9_7795;
    pub const TERRAIN_RECONCILIATION: u32 = 0xCA63_D517;
    /// PostCityInit: `DoPostCityInit` sets the view-ready flag (§12.1).
    pub const POST_CITY_INIT: u32 = 0x26D3_1EC1;
    pub const SIM_NEW_DAY: u32 = 0x6695_6814;
}

/// The `data4` kinds of `RedisplayTerrain` (§8.2).
pub mod kind {
    /// Internal and non-height changes (sea level, terrain type, ...).
    pub const INTERNAL: u32 = 0;
    /// The terraforming tool in god mode, `ChangeTerrainLevel`, edge reconciliation.
    pub const GOD: u32 = 1;
    /// The terraforming tool in mayor mode, `ApplyTerrainBrush`.
    pub const MAYOR: u32 = 2;
    /// Disasters and water; the `tBrushParameters` default.
    pub const DISASTER: u32 = 3;
    /// The network height constraint solver (road and rail grading).
    pub const GRADING: u32 = 4;
    /// Levelling (demolition reason 5, lot developer, landfill).
    pub const LEVELLING: u32 = 5;
    /// The network tool's tunnel cut.
    pub const TUNNEL: u32 = 6;
}

/// Vertex flags (`+0x22`, §7).
pub mod flag {
    pub const CLIFF: u16 = 0x0008;
    pub const SHORE_OR_CLIFF: u16 = 0x0010;
    pub const EVEN_TRIANGLE_HIDDEN: u16 = 0x0020;
    pub const ODD_TRIANGLE_HIDDEN: u16 = 0x0040;
    pub const CELL_FLAG_4: u16 = 0x0200;
    pub const BRUSH_MARK: u16 = 0x0400;
    pub const FLIPPED_BY_CLIFF: u16 = 0x0800;
    pub const FLIPPED_INTERNAL: u16 = 0x1000;
    pub const FLIPPED_EXTERNAL: u16 = 0x2000;
    /// Every height setter skips a locked vertex (slot 52).
    pub const LOCKED: u16 = 0x4000;
    /// `IsTriangulationFlipped` (slot 70) tests these.
    pub const FLIPPED_MASK: u16 = FLIPPED_INTERNAL | FLIPPED_EXTERNAL;
}

/// Tuning (§2.4): exemplar `0x6534284A / 0x88CD66E9 / 1`, "Terrain Properties".
pub mod tuning_prop {
    pub const EXEMPLAR_TYPE: u32 = 0x6534_284A;
    pub const EXEMPLAR_GROUP: u32 = 0x88CD_66E9;
    pub const EXEMPLAR_INSTANCE: u32 = 1;
    pub const MAX_HEIGHT: u32 = 0x48CD_7B26;
    pub const MIN_HEIGHT: u32 = 0x48CD_7B27;
    pub const NEW_SEA_LEVEL: u32 = 0x48CD_7B21;
    pub const NEW_FLAT_HEIGHT: u32 = 0x48CD_7B2A;
    pub const IMAGE_HEIGHT_SCALE: u32 = 0x48CD_7B28;
    /// Four values: minimum, maximum and two editor values.
    pub const SEA_LEVEL_RANGE: u32 = 0x48CD_7B70;
    pub const CLIFF_SLOPE: u32 = 0x48CD_7B65;
    /// Stored at `+0x78` and saved.
    pub const SAVED_78: u32 = 0x48CD_7B20;
}

/// The brushes the simulation applies (§9.1).
pub mod brush_id {
    pub const EARTHQUAKE_FAULT: u32 = 0x11;
    pub const EARTHQUAKE_FAULT_FLIPPED: u32 = 0x12;
    pub const PIPE_SINK_HOLE: u32 = 0x14;
    pub const VOLCANO: u32 = 0xCA31_3035;
    /// Brush exemplars: type `0x6534284A`, this group, instance = brush ID.
    pub const EXEMPLAR_GROUP: u32 = 0x6A01_FC2A;
    /// Component `i` property base: `BASE + 0x100·i + k` (§9.1).
    pub const COMPONENT_PROPERTY_BASE: u32 = 0x8A02_0202;
    /// Bitmaps: grey images of this type and group.
    pub const BITMAP_TYPE: u32 = 0x7AB5_0E44;
    pub const BITMAP_GROUP: u32 = 0x891B_0E1A;
    pub const BITMAP_EARTHQUAKE_FAULT: u32 = 0x0A0B_485E;
    pub const BITMAP_EARTHQUAKE_FAULT_FLIPPED: u32 = 0xCA13_1C51;
    pub const BITMAP_PIPE_SINK_HOLE: u32 = 0x2A05_BB19;
}

/// The saved records (§13.1): type, group `0xE98F9525`, instance.
pub mod record {
    pub const GROUP: u32 = super::CLSID_TERRAIN;
    pub const MISC_TYPE: u32 = 0x8A91_E7E3;
    pub const MISC_INSTANCE: u32 = 3;
    pub const ALTITUDES_TYPE: u32 = 0xA9DD_6FF4;
    pub const ALTITUDES_INSTANCE: u32 = 1;
    pub const PROPERTIES_TYPE: u32 = 0x8A91_E7E0;
    pub const PROPERTIES_INSTANCE: u32 = 2;
    /// Edges: same type as the altitudes, instance `0x64 + side`.
    pub const EDGE_TYPE: u32 = 0xA9DD_6FF4;
    pub const EDGE_INSTANCE_BASE: u32 = 0x64;
}

/// The surface water's `GetWaterType` results (§10.2).
pub mod water_type {
    pub const LAND: u32 = 0;
    /// Fresh water above the sea (or a NaN level).
    pub const FRESH: u32 = 2;
    pub const SEA: u32 = 3;
}
