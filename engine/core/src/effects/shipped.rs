// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The brush and game records the shipped effect directory carries
//! (`docs/specs/effects.md` §8.4, §9.4).
//!
//! These are the **tables**, not the effect descriptions that point at them: a
//! component of type 3 with `description_index = i` is brush record `i`, and a
//! component of type 5 with `description_index = i` is game record `i`
//! (§3). The counts are measured against the shipped `SimCity_1.dat` effect
//! directory — 25 brush and 57 game records — and are pinned by
//! [`tests`](#) below.
//!
//! The values here are transcribed from the spec's tables. Where a table gives
//! a single value and §8.1 / §9.1 give a range, the range is degenerate: §8.4
//! says "All target heights are 100, all ranges are degenerate (the strength
//! and radius columns give the single value)".

/// A terrain brush's ID (terrain.md §9.1).
pub mod brush_id {
    /// CraterBrush.
    pub const CRATER: u32 = 0x8A33_E17A;
    /// VolcanoBrush.
    pub const VOLCANO: u32 = 0xCA31_3035;
    /// ErosionBrush.
    pub const EROSION: u32 = 0x6A4B_AAFA;
    /// SmoothingBrush.
    pub const SMOOTHING: u32 = 0xEA31_3BB9;
}

/// One shipped brush record (§8.4), as the brush component is built from it
/// (§8.1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BrushRecord {
    /// The terrain brush the stroke applies.
    pub brush_id: u32,
    /// Applications per effect-second, or the count of a one-shot.
    pub rate: f32,
    /// Stroke duration in effect-seconds; 0 means no limit.
    pub length: f32,
    /// The one-shot flag (`+0x18`). A one-shot applies `trunc(rate)` times on
    /// its first update and then goes inactive (§8.3).
    pub one_shot: bool,
    /// `tBrushParameters.level`.
    pub level: u32,
    /// The strength range. Every shipped record is degenerate.
    pub strength: [f32; 2],
    /// The radius-scale range. Every shipped record is degenerate.
    pub radius_scale: [f32; 2],
    /// The target height. Every shipped record uses 100.
    pub target_height: f32,
}

/// The 25 shipped brush records (§8.4). This answers terrain.md TR2.
pub const BRUSH_RECORDS: [BrushRecord; 25] = [
    // 0: mushroomcloud.
    BrushRecord {
        brush_id: brush_id::CRATER,
        rate: 1.0,
        length: 0.0,
        one_shot: true,
        level: 2,
        strength: [4.0, 4.0],
        radius_scale: [0.65, 0.65],
        target_height: 100.0,
    },
    // 1: powerplant_explosion.
    BrushRecord {
        brush_id: brush_id::CRATER,
        rate: 2.0,
        length: 0.5,
        one_shot: false,
        level: 3,
        strength: [3.0, 3.0],
        radius_scale: [0.4, 0.4],
        target_height: 100.0,
    },
    // 2: volcanocreator1a.
    BrushRecord {
        brush_id: brush_id::VOLCANO,
        rate: 30.0,
        length: 3.0,
        one_shot: false,
        level: 1,
        strength: [1.0, 1.0],
        radius_scale: [0.5, 0.5],
        target_height: 100.0,
    },
    // 3: volcanocreator1b.
    BrushRecord {
        brush_id: brush_id::VOLCANO,
        rate: 40.0,
        length: 4.0,
        one_shot: false,
        level: 1,
        strength: [1.0, 1.0],
        radius_scale: [0.5, 0.5],
        target_height: 100.0,
    },
    // 4: volcanocreator2a.
    BrushRecord {
        brush_id: brush_id::VOLCANO,
        rate: 45.0,
        length: 3.5,
        one_shot: false,
        level: 1,
        strength: [1.0, 1.0],
        radius_scale: [0.6, 0.6],
        target_height: 100.0,
    },
    // 5: volcanocreator2b.
    BrushRecord {
        brush_id: brush_id::VOLCANO,
        rate: 50.0,
        length: 3.5,
        one_shot: false,
        level: 1,
        strength: [1.0, 1.0],
        radius_scale: [0.6, 0.6],
        target_height: 100.0,
    },
    // 6: volcanocreator3a.
    BrushRecord {
        brush_id: brush_id::VOLCANO,
        rate: 55.0,
        length: 3.0,
        one_shot: false,
        level: 1,
        strength: [1.0, 1.0],
        radius_scale: [0.8, 0.8],
        target_height: 100.0,
    },
    // 7: volcanocreator3b.
    BrushRecord {
        brush_id: brush_id::VOLCANO,
        rate: 65.0,
        length: 3.0,
        one_shot: false,
        level: 1,
        strength: [1.0, 1.0],
        radius_scale: [0.8, 0.8],
        target_height: 100.0,
    },
    // 8: volcanocreator4a -- the creator a fresh manager's first select draw
    // picks (§17 test 3).
    BrushRecord {
        brush_id: brush_id::VOLCANO,
        rate: 65.0,
        length: 3.0,
        one_shot: false,
        level: 1,
        strength: [1.0, 1.0],
        radius_scale: [1.0, 1.0],
        target_height: 100.0,
    },
    // 9: volcanocreator4b.
    BrushRecord {
        brush_id: brush_id::VOLCANO,
        rate: 80.0,
        length: 3.5,
        one_shot: false,
        level: 1,
        strength: [1.0, 1.0],
        radius_scale: [1.0, 1.0],
        target_height: 100.0,
    },
    // 10: volcanocreator5.
    BrushRecord {
        brush_id: brush_id::VOLCANO,
        rate: 130.0,
        length: 2.0,
        one_shot: false,
        level: 1,
        strength: [1.0, 1.0],
        radius_scale: [1.3, 1.3],
        target_height: 100.0,
    },
    // 11-15: meteorcrater_a .. _e (one-shot, 2 applications).
    BrushRecord {
        brush_id: brush_id::CRATER,
        rate: 2.0,
        length: 0.0,
        one_shot: true,
        level: 3,
        strength: [4.0, 4.0],
        radius_scale: [0.5, 0.5],
        target_height: 100.0,
    },
    BrushRecord {
        brush_id: brush_id::CRATER,
        rate: 2.0,
        length: 0.0,
        one_shot: true,
        level: 3,
        strength: [6.0, 6.0],
        radius_scale: [0.7, 0.7],
        target_height: 100.0,
    },
    BrushRecord {
        brush_id: brush_id::CRATER,
        rate: 2.0,
        length: 0.0,
        one_shot: true,
        level: 3,
        strength: [3.0, 3.0],
        radius_scale: [0.8, 0.8],
        target_height: 100.0,
    },
    BrushRecord {
        brush_id: brush_id::CRATER,
        rate: 2.0,
        length: 0.0,
        one_shot: true,
        level: 3,
        strength: [2.0, 2.0],
        radius_scale: [0.6, 0.6],
        target_height: 100.0,
    },
    BrushRecord {
        brush_id: brush_id::CRATER,
        rate: 2.0,
        length: 0.0,
        one_shot: true,
        level: 3,
        strength: [5.0, 5.0],
        radius_scale: [0.9, 0.9],
        target_height: 100.0,
    },
    // 16-20: meteorcraterwater_a .. _e.
    BrushRecord {
        brush_id: brush_id::CRATER,
        rate: 2.0,
        length: 0.0,
        one_shot: true,
        level: 3,
        strength: [4.0, 4.0],
        radius_scale: [0.5, 0.5],
        target_height: 100.0,
    },
    BrushRecord {
        brush_id: brush_id::CRATER,
        rate: 2.0,
        length: 0.0,
        one_shot: true,
        level: 3,
        strength: [6.0, 6.0],
        radius_scale: [0.8, 0.8],
        target_height: 100.0,
    },
    BrushRecord {
        brush_id: brush_id::CRATER,
        rate: 2.0,
        length: 0.0,
        one_shot: true,
        level: 3,
        strength: [3.0, 3.0],
        radius_scale: [1.0, 1.0],
        target_height: 100.0,
    },
    BrushRecord {
        brush_id: brush_id::CRATER,
        rate: 2.0,
        length: 0.0,
        one_shot: true,
        level: 3,
        strength: [2.0, 2.0],
        radius_scale: [0.6, 0.6],
        target_height: 100.0,
    },
    BrushRecord {
        brush_id: brush_id::CRATER,
        rate: 2.0,
        length: 0.0,
        one_shot: true,
        level: 3,
        strength: [5.0, 5.0],
        radius_scale: [0.7, 0.7],
        target_height: 100.0,
    },
    // 21: lasercraterwater_a.
    BrushRecord {
        brush_id: brush_id::CRATER,
        rate: 2.0,
        length: 0.0,
        one_shot: true,
        level: 3,
        strength: [4.0, 4.0],
        radius_scale: [0.5, 0.5],
        target_height: 100.0,
    },
    // 22: erodefx_terrain.
    BrushRecord {
        brush_id: brush_id::EROSION,
        rate: 1.0,
        length: 0.0,
        one_shot: true,
        level: 2,
        strength: [1.0, 1.0],
        radius_scale: [1.0, 1.0],
        target_height: 100.0,
    },
    // 23: erodefx_terrain.
    BrushRecord {
        brush_id: brush_id::SMOOTHING,
        rate: 1.0,
        length: 0.0,
        one_shot: true,
        level: 2,
        strength: [1.0, 1.0],
        radius_scale: [1.0, 1.0],
        target_height: 100.0,
    },
    // 24: stream_cut_fx_terrain.
    BrushRecord {
        brush_id: brush_id::EROSION,
        rate: 1.0,
        length: 0.0,
        one_shot: true,
        level: 4,
        strength: [0.3, 0.3],
        radius_scale: [0.25, 0.25],
        target_height: 100.0,
    },
];

/// The game-description flags (§9.1). Re-exported so the table reads as the
/// text options do.
pub mod game_flags {
    pub const NO_NETWORKS: u32 = 1;
    pub const NO_FLORA: u32 = 2;
    pub const DEZONE: u32 = 4;
    pub const SINGLE: u32 = 8;
    pub const PAUSE_SIM: u32 = 0x10;
    pub const PAUSE_SIM_HIDDEN: u32 = 0x20;
    pub const PAUSE_CLOCK: u32 = 0x40;
}

/// One shipped game record (§9.4).
///
/// Only the fields §9.4's table measures are named; the rest are the record's
/// defaults (no message unless shown, no effect map unless shown, no
/// demolish-size limits unless shown).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GameRecord {
    /// `+0x0C`: [`game_flags`].
    pub flags: u32,
    /// `+0x14`: the demolish radius in cells; 0 = no demolish action.
    pub demolish_radius: u32,
    /// `+0x18`: the demolition flags (demolition.md §4.9). `1` and `2` are the
    /// cause byte; `0x1301` / `0x1302` add `0x300` (explode) with `0x1000`.
    pub demolish_flags: u32,
    /// `+0x20`: `-maxDemolishSize` in metres; 0 = no limit.
    pub max_demolish_size: f32,
    /// `+0x24`: the burn radius; 0 = no burn action.
    pub burn_radius: u32,
    /// `+0x28`: `-toxic n`. §9.3 quirk 6: the amount is ignored, the spill is
    /// always 120.
    pub toxic_amount: u32,
    /// `+0x2C`: `-extinguishFire u`.
    pub extinguish_amount: u32,
    /// `+0x30`: `-message type`; 0 = no message.
    pub message_type: u32,
    /// `+0x4C`: the `-pauseSim` duration in effect-seconds; 0 = none.
    pub duration: f32,
    /// `+0x38` is non-zero: the record applies an effect map (§9.3 step 6).
    /// The map's shape is render data and out of scope; only the fact that one
    /// is applied is recorded here.
    pub has_map: bool,
}

/// A game record with no action at all.
const NO_ACTION: GameRecord = GameRecord {
    flags: 0,
    demolish_radius: 0,
    demolish_flags: 0,
    max_demolish_size: 0.0,
    burn_radius: 0,
    toxic_amount: 0,
    extinguish_amount: 0,
    message_type: 0,
    duration: 0.0,
    has_map: false,
};

/// The 57 shipped game ("scrubber") records (§9.4), in table order.
pub const GAME_RECORDS: [GameRecord; 57] = [
    // 0: debriscorefx_terrain -- burn 1.
    GameRecord {
        burn_radius: 1,
        ..NO_ACTION
    },
    // 1: mushroomcloud -- single; demolish r = 10, flags 1.
    GameRecord {
        flags: game_flags::SINGLE,
        demolish_radius: 10,
        demolish_flags: 1,
        ..NO_ACTION
    },
    // 2: mushroomcloud -- burn 15.
    GameRecord {
        burn_radius: 15,
        ..NO_ACTION
    },
    // 3: mushroomcloud -- hidden pause, 5 s.
    GameRecord {
        flags: game_flags::PAUSE_SIM_HIDDEN,
        duration: 5.0,
        ..NO_ACTION
    },
    // 4: powerplant_explosion -- hidden pause, 5 s.
    GameRecord {
        flags: game_flags::PAUSE_SIM_HIDDEN,
        duration: 5.0,
        ..NO_ACTION
    },
    // 5: powerplant_explosion -- demolish 2.
    GameRecord {
        demolish_radius: 2,
        demolish_flags: 1,
        ..NO_ACTION
    },
    // 6: powerplant_explosion -- burn 6.
    GameRecord {
        burn_radius: 6,
        ..NO_ACTION
    },
    // 7: electrical_fire_starter_terrain -- burn 1.
    GameRecord {
        burn_radius: 1,
        ..NO_ACTION
    },
    // 8: lava_fallout_burn_terrain -- burn 1.
    GameRecord {
        burn_radius: 1,
        ..NO_ACTION
    },
    // 9: volcanocreator1a -- demolish 5, flags 2; burn 10.
    GameRecord {
        demolish_radius: 5,
        demolish_flags: 2,
        burn_radius: 10,
        ..NO_ACTION
    },
    // 10: volcanocreator1b -- demolish 5, flags 2; burn 10.
    GameRecord {
        demolish_radius: 5,
        demolish_flags: 2,
        burn_radius: 10,
        ..NO_ACTION
    },
    // 11: volcanocreator2a -- demolish 5, flags 2; burn 10.
    GameRecord {
        demolish_radius: 5,
        demolish_flags: 2,
        burn_radius: 10,
        ..NO_ACTION
    },
    // 12: volcanocreator2b -- demolish 5, flags 2; burn 10.
    GameRecord {
        demolish_radius: 5,
        demolish_flags: 2,
        burn_radius: 10,
        ..NO_ACTION
    },
    // 13: volcanocreator3a -- demolish 8, flags 2; burn 12.
    GameRecord {
        demolish_radius: 8,
        demolish_flags: 2,
        burn_radius: 12,
        ..NO_ACTION
    },
    // 14: volcanocreator3b -- demolish 8, flags 2; burn 12.
    GameRecord {
        demolish_radius: 8,
        demolish_flags: 2,
        burn_radius: 12,
        ..NO_ACTION
    },
    // 15: volcanocreator4a -- demolish 8, flags 2; burn 12.
    GameRecord {
        demolish_radius: 8,
        demolish_flags: 2,
        burn_radius: 12,
        ..NO_ACTION
    },
    // 16: volcanocreator4b -- demolish 10, flags 2; burn 12.
    GameRecord {
        demolish_radius: 10,
        demolish_flags: 2,
        burn_radius: 12,
        ..NO_ACTION
    },
    // 17: volcanocreator5 -- demolish 10, flags 2; burn 14. §17 test 6 uses it.
    GameRecord {
        demolish_radius: 10,
        demolish_flags: 2,
        burn_radius: 14,
        ..NO_ACTION
    },
    // 18: robot_walkfx_slave -- noNetworks; demolish 1, flags 0x1301.
    GameRecord {
        flags: game_flags::NO_NETWORKS,
        demolish_radius: 1,
        demolish_flags: 0x1301,
        ..NO_ACTION
    },
    // 19: robot_walkfx_slave -- message 0x2A7B582E (kSC4MessageEffectLocation).
    GameRecord {
        message_type: 0x2A7B_582E,
        ..NO_ACTION
    },
    // 20: shooting_platesfx_slave -- noNetworks; demolish 1, flags 0x1301.
    GameRecord {
        flags: game_flags::NO_NETWORKS,
        demolish_radius: 1,
        demolish_flags: 0x1301,
        ..NO_ACTION
    },
    // 21: robot_firefx_slave -- noNetworks; demolish 1, flags 0x1301.
    GameRecord {
        flags: game_flags::NO_NETWORKS,
        demolish_radius: 1,
        demolish_flags: 0x1301,
        ..NO_ACTION
    },
    // 22: robot_landfx_slave -- noNetworks; demolish 1, flags 0x1301; burn 2.
    GameRecord {
        flags: game_flags::NO_NETWORKS,
        demolish_radius: 1,
        demolish_flags: 0x1301,
        burn_radius: 2,
        ..NO_ACTION
    },
    // 23: meteor_stuff_slave -- message 0x2A7B582E.
    GameRecord {
        message_type: 0x2A7B_582E,
        ..NO_ACTION
    },
    // 24: meteor_stuff_terrain -- demolish 6, flags 2; burn 10.
    GameRecord {
        demolish_radius: 6,
        demolish_flags: 2,
        burn_radius: 10,
        ..NO_ACTION
    },
    // 25: meteor_stuff_water -- demolish 6, flags 2; burn 10.
    GameRecord {
        demolish_radius: 6,
        demolish_flags: 2,
        burn_radius: 10,
        ..NO_ACTION
    },
    // 26: tornadofx_slave -- noNetworks; demolish 4, flags 0x1302, max size
    // 48 m; message 0x0A36F1E9 (kSC4MessageTornadoLocation).
    GameRecord {
        flags: game_flags::NO_NETWORKS,
        demolish_radius: 4,
        demolish_flags: 0x1302,
        max_demolish_size: 48.0,
        message_type: 0x0A36_F1E9,
        ..NO_ACTION
    },
    // 27: earthquakefire_terrain -- noNetworks, noFlora; demolish 2, 0x1302.
    GameRecord {
        flags: game_flags::NO_NETWORKS | game_flags::NO_FLORA,
        demolish_radius: 2,
        demolish_flags: 0x1302,
        ..NO_ACTION
    },
    // 28: quaker -- noNetworks, noFlora; demolish 1, 0x1302.
    GameRecord {
        flags: game_flags::NO_NETWORKS | game_flags::NO_FLORA,
        demolish_radius: 1,
        demolish_flags: 0x1302,
        ..NO_ACTION
    },
    // 29: lightningstrike_trigger -- burn 2.
    GameRecord {
        burn_radius: 2,
        ..NO_ACTION
    },
    // 30: sparkles_on_demolish -- hidden pause, 5 s.
    GameRecord {
        flags: game_flags::PAUSE_SIM_HIDDEN,
        duration: 5.0,
        ..NO_ACTION
    },
    // 31: obliterate_finale -- hidden pause, 15 s.
    GameRecord {
        flags: game_flags::PAUSE_SIM_HIDDEN,
        duration: 15.0,
        ..NO_ACTION
    },
    // 32: lasercrater_a -- noNetworks, noFlora; demolish 1, 0x1301.
    GameRecord {
        flags: game_flags::NO_NETWORKS | game_flags::NO_FLORA,
        demolish_radius: 1,
        demolish_flags: 0x1301,
        ..NO_ACTION
    },
    // 33: biglasercrater_a -- demolish 2.
    GameRecord {
        demolish_radius: 2,
        demolish_flags: 1,
        ..NO_ACTION
    },
    // 34: mouthfirefx_slave -- noNetworks; demolish 1, 0x1301; burn 2.
    GameRecord {
        flags: game_flags::NO_NETWORKS,
        demolish_radius: 1,
        demolish_flags: 0x1301,
        burn_radius: 2,
        ..NO_ACTION
    },
    // 35: tsaurus_walkfx_slave -- noNetworks; demolish 1, 0x1301.
    GameRecord {
        flags: game_flags::NO_NETWORKS,
        demolish_radius: 1,
        demolish_flags: 0x1301,
        ..NO_ACTION
    },
    // 36: tsaurus_walkfx_slave -- message 0x2A7B582E.
    GameRecord {
        message_type: 0x2A7B_582E,
        ..NO_ACTION
    },
    // 37-39, 43: runwaysmall/medium/large, whalefx_slave -- effect maps only
    // (render), so no simulation action beyond the map.
    GameRecord {
        has_map: true,
        ..NO_ACTION
    },
    GameRecord {
        has_map: true,
        ..NO_ACTION
    },
    GameRecord {
        has_map: true,
        ..NO_ACTION
    },
    // 40: rocket_explodefx_terrain -- burn 2.
    GameRecord {
        burn_radius: 2,
        ..NO_ACTION
    },
    // 41: rocket_explodefx2_terrain -- burn 1.
    GameRecord {
        burn_radius: 1,
        ..NO_ACTION
    },
    // 42: rocket_explodefx3_terrain -- burn 1.
    GameRecord {
        burn_radius: 1,
        ..NO_ACTION
    },
    // 43: whalefx_slave -- effect maps only.
    GameRecord {
        has_map: true,
        ..NO_ACTION
    },
    // 44: no effect uses it -- demolish 1.
    GameRecord {
        demolish_radius: 1,
        demolish_flags: 1,
        ..NO_ACTION
    },
    // 45: molotov_flames -- burn 1.
    GameRecord {
        burn_radius: 1,
        ..NO_ACTION
    },
    // 46: smlmushroomcloud -- hidden pause, 5 s.
    GameRecord {
        flags: game_flags::PAUSE_SIM_HIDDEN,
        duration: 5.0,
        ..NO_ACTION
    },
    // 47: tankfire_terrain -- noNetworks; demolish 1, 0x1302; burn 2.
    GameRecord {
        flags: game_flags::NO_NETWORKS,
        demolish_radius: 1,
        demolish_flags: 0x1302,
        burn_radius: 2,
        ..NO_ACTION
    },
    // 48: water1drop_slave -- extinguish 10000.
    GameRecord {
        extinguish_amount: 10000,
        ..NO_ACTION
    },
    // 49: water1drop_terrain -- extinguish 10000.
    GameRecord {
        extinguish_amount: 10000,
        ..NO_ACTION
    },
    // 50: waterdrop -- extinguish 10000.
    GameRecord {
        extinguish_amount: 10000,
        ..NO_ACTION
    },
    // 51: helifire_terrain -- demolish 2, 0x1302; burn 2.
    GameRecord {
        demolish_radius: 2,
        demolish_flags: 0x1302,
        burn_radius: 2,
        ..NO_ACTION
    },
    // 52: jetfire_terrain -- demolish 2, 0x1302; burn 2.
    GameRecord {
        demolish_radius: 2,
        demolish_flags: 0x1302,
        burn_radius: 2,
        ..NO_ACTION
    },
    // 53: ufolasercrater_a -- noNetworks; demolish 1, 0x1302; burn 2.
    GameRecord {
        flags: game_flags::NO_NETWORKS,
        demolish_radius: 1,
        demolish_flags: 0x1302,
        burn_radius: 2,
        ..NO_ACTION
    },
    // 54: whalefire_terrain -- noNetworks; demolish 5, 0x1302; burn 2.
    GameRecord {
        flags: game_flags::NO_NETWORKS,
        demolish_radius: 5,
        demolish_flags: 0x1302,
        burn_radius: 2,
        ..NO_ACTION
    },
    // 55: p_train_toxic_slave -- toxic 20 (the spill is always 120, §9.3).
    GameRecord {
        toxic_amount: 20,
        ..NO_ACTION
    },
    // 56: collisions_active -- extinguish 4.
    GameRecord {
        extinguish_amount: 4,
        ..NO_ACTION
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    /// §3: the shipped directory holds 25 brush and 57 game records.
    #[test]
    fn the_shipped_table_counts_are_pinned() {
        assert_eq!(BRUSH_RECORDS.len(), 25, "§8.4: 25 brush records");
        assert_eq!(GAME_RECORDS.len(), 57, "§9.4: 57 game records");
    }

    /// §8.4: all target heights are 100 and every strength and radius range is
    /// degenerate.
    #[test]
    fn every_shipped_brush_range_is_degenerate() {
        for (i, r) in BRUSH_RECORDS.iter().enumerate() {
            assert_eq!(r.target_height, 100.0, "record {i}");
            assert_eq!(r.strength[0], r.strength[1], "record {i} strength");
            assert_eq!(r.radius_scale[0], r.radius_scale[1], "record {i} radius");
        }
    }

    /// §8.4: the two volcano rows conformance tests 1 and 2 use, and the
    /// crater row test 4 uses.
    #[test]
    fn the_brush_records_the_conformance_tests_use() {
        // Test 1/2: brush record 2 (rate 30, length 3.0, stroke).
        let v = BRUSH_RECORDS[2];
        assert_eq!(v.brush_id, brush_id::VOLCANO);
        assert_eq!(v.rate, 30.0);
        assert_eq!(v.length, 3.0);
        assert!(!v.one_shot, "it is a stroke");

        // Test 4: meteorcrater_b is brush record 12, one-shot with rate 2.
        let c = BRUSH_RECORDS[12];
        assert_eq!(c.brush_id, brush_id::CRATER);
        assert!(c.one_shot);
        assert_eq!(c.rate, 2.0, "two applications");
        assert_eq!(c.level, 3);
        assert_eq!(c.strength[0], 6.0);
        assert_eq!(c.radius_scale[0], 0.7);

        // §17 test 3: volcanocreator4a is brush record 8.
        let v4a = BRUSH_RECORDS[8];
        assert_eq!(v4a.brush_id, brush_id::VOLCANO);
        assert_eq!(v4a.rate, 65.0);
        assert_eq!(v4a.length, 3.0);
        assert_eq!(v4a.radius_scale[0], 1.0);
    }

    /// §9.4: the game record §17 test 6 uses, and the pause record test 8
    /// uses.
    #[test]
    fn the_game_records_the_conformance_tests_use() {
        // Test 6: game record 17 at cell (40, 40): burn 14 then demolish 10
        // cells with flags 2, and no message.
        let g = GAME_RECORDS[17];
        assert_eq!(g.burn_radius, 14);
        assert_eq!(g.demolish_radius, 10);
        assert_eq!(g.demolish_flags, 2);
        assert_eq!(g.message_type, 0);

        // Test 8: game record 3 is a hidden pause of 5 s.
        let p = GAME_RECORDS[3];
        assert_eq!(p.flags, game_flags::PAUSE_SIM_HIDDEN);
        assert_eq!(p.duration, 5.0);
        assert_eq!(p.demolish_radius, 0);
    }

    /// The action census of the table as transcribed. These are counts of the
    /// rows of §9.4, so they pin the transcription; they are not a second
    /// measurement of the shipped file.
    #[test]
    fn the_game_action_census_matches_the_table() {
        let demolish = GAME_RECORDS.iter().filter(|g| g.demolish_radius > 0).count();
        let burn = GAME_RECORDS.iter().filter(|g| g.burn_radius > 0).count();
        let pause = GAME_RECORDS
            .iter()
            .filter(|g| g.flags & (game_flags::PAUSE_SIM | game_flags::PAUSE_SIM_HIDDEN) != 0)
            .count();
        let message = GAME_RECORDS.iter().filter(|g| g.message_type != 0).count();
        let toxic = GAME_RECORDS.iter().filter(|g| g.toxic_amount > 0).count();
        let extinguish = GAME_RECORDS.iter().filter(|g| g.extinguish_amount > 0).count();
        let single = GAME_RECORDS
            .iter()
            .filter(|g| g.flags & game_flags::SINGLE != 0)
            .count();

        assert_eq!(demolish, 30, "records that demolish");
        assert_eq!(burn, 28, "records that burn");
        assert_eq!(pause, 5, "records 3, 4, 30, 31, 46");
        assert_eq!(message, 4, "records 19, 23, 26, 36");
        assert_eq!(toxic, 1, "record 55");
        assert_eq!(extinguish, 4, "records 48, 49, 50, 56");
        assert_eq!(single, 1, "record 1");
        assert_eq!(
            GAME_RECORDS.iter().filter(|g| g.has_map).count(),
            4,
            "records 37, 38, 39, 43 apply an effect map only"
        );
    }
}
