// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Ordered reload of the thirteen process-wide network type records.

use super::*;

game_fn! { fn clear(this: *mut u8) = "thiscall" 0x0064_21D0; }
game_fn! { fn evaluator(this: *mut u8, key: *const u8, a: u32, b: u32) = "thiscall" 0x0063_D360; }
game_fn! { fn bridges(this: *mut u8, ini: *mut u8, section: *const u8, instance: u32, width: u32) = "thiscall" 0x0064_37A0; }
game_fn! { fn supports(this: *mut u8, group: u32) = "thiscall" 0x0063_F780; }
game_fn! { fn placement(this: *mut u8, instance: u32) = "thiscall" 0x0063_D460; }
game_fn! { fn complete(this: *mut u8) = "thiscall" 0x0063_BF90; }
game_fn! { fn verifiers(ini: *mut u8, section: *const u8) -> u64 = "C" 0x0063_FAF0; }

layout! {
    /// The metadata written between clearing a type record and loading its resources.
    struct TypeMetadata {
        0x00 => layer / set_layer: u32,
        0x08 => occupant / set_occupant: u32,
        0x0C => model / set_model: u32,
        0x10 => underground / set_underground: u32,
        0x14 => preview / set_preview: u32,
        0x18 => alternate / set_alternate: u32,
        0x1C => width / set_width: u32,
        0x20 => depth / set_depth: u32,
        0x75 => straight / set_straight: u8,
    }
}

game_data! { static road: [u8; 0x114] = 0x00B4_52C8; }
game_data! { static rail: [u8; 0x114] = 0x00B4_53DC; }
game_data! { static highway: [u8; 0x114] = 0x00B4_54F0; }
game_data! { static street: [u8; 0x114] = 0x00B4_5604; }
game_data! { static pipe: [u8; 0x114] = 0x00B4_5718; }
game_data! { static power: [u8; 0x114] = 0x00B4_582C; }
game_data! { static avenue: [u8; 0x114] = 0x00B4_5940; }
game_data! { static subway: [u8; 0x114] = 0x00B4_5A54; }
game_data! { static light_rail: [u8; 0x114] = 0x00B4_5B68; }
game_data! { static monorail: [u8; 0x114] = 0x00B4_5C7C; }
game_data! { static one_way: [u8; 0x114] = 0x00B4_5D90; }
game_data! { static dirt: [u8; 0x114] = 0x00B4_5EA4; }
game_data! { static ground_highway: [u8; 0x114] = 0x00B4_5FB8; }

// Resource keys and INI section names are passed at their game addresses, including their identity.
game_data! { static road_key: [u8; 24] = 0x00B0_B418; }
game_data! { static pipe_key: [u8; 24] = 0x00B0_B430; }
game_data! { static rail_key: [u8; 24] = 0x00B0_B448; }
game_data! { static highway_key: [u8; 24] = 0x00B0_B460; }
game_data! { static street_key: [u8; 24] = 0x00B0_B478; }
game_data! { static subway_key: [u8; 24] = 0x00B0_B490; }
game_data! { static avenue_key: [u8; 24] = 0x00B0_B4A8; }
game_data! { static light_rail_key: [u8; 24] = 0x00B0_B4C0; }
game_data! { static one_way_key: [u8; 24] = 0x00B0_B4D8; }
game_data! { static dirt_key: [u8; 24] = 0x00B0_B4F0; }
game_data! { static monorail_key: [u8; 24] = 0x00B0_B508; }
game_data! { static ground_highway_key: [u8; 24] = 0x00B0_B520; }
game_data! { static road_section: u8 = 0x00AA_86D4; }
game_data! { static one_way_section: u8 = 0x00AA_86B8; }
game_data! { static dirt_section: u8 = 0x00AA_86A0; }
game_data! { static rail_section: u8 = 0x00AA_868C; }
game_data! { static light_rail_section: u8 = 0x00AA_8670; }
game_data! { static monorail_section: u8 = 0x00AA_8658; }
game_data! { static street_section: u8 = 0x00AA_8640; }
game_data! { static highway_section: u8 = 0x00AA_8628; }
game_data! { static ground_highway_section: u8 = 0x00AA_8608; }
game_data! { static avenue_section: u8 = 0x00AA_85F0; }
game_data! { static pipe_section: u8 = 0x00AA_85DC; }
game_data! { static subway_section: u8 = 0x00AA_85C4; }
game_data! { static verifier_section: u8 = 0x00AA_85B4; }

unsafe fn metadata(record: *mut u8, words: [u32; 8]) {
    unsafe {
        let view = TypeMetadata::from_ptr(record);
        view.set_occupant(words[1]);
        view.set_model(words[2]);
        view.set_underground(words[3]);
        view.set_preview(words[4]);
        view.set_alternate(words[5]);
        view.set_width(words[6]);
        view.set_depth(words[7]);
        view.set_layer(words[0]);
    }
}

macro_rules! load {
    ($ini:ident, $record:ident, $key:ident, $section:ident, $words:expr, $instance:literal, $placement:literal, $supports:literal) => {{
        let record = $record().cast::<u8>();
        clear(record);
        metadata(record, $words);
        evaluator(record, $key().cast::<u8>().add(12), 0, 0);
        evaluator(record, $key().cast::<u8>(), 0, 0);
        bridges(record, $ini, $section(), $instance, $words[6]);
        if $supports {
            supports(record, 0x8974_F80F);
        }
        placement(record, $placement);
        if $instance == 0x1003 {
            TypeMetadata::from_ptr(record).set_straight(1);
        }
        complete(record);
    }};
}

port! {
    /// Clear, publish metadata, and load each type in the game's resource order. Power lines have
    /// metadata only. Street's straight-only byte is set after placement loading, before completion.
    /// The final call reloads the tile verifiers from the caller's INI reader.
    #[port(address = 0x0062_3310, owner = "networktool", name = "cSC4NetworkTool::ReloadNetworkTypeInfo")]
    pub unsafe extern "C" fn fc4p_00623310(ini: *mut u8) -> u64 {
        unsafe {
            load!(ini, road, road_key, road_section,
                [1, 0xA766_C101, 0x2821_ED93, 0, 0xE17B_B266, 0x49E1_3260, 1, 1], 0x1000, 0x0781_214F, true);
            load!(ini, one_way, one_way_key, one_way_section,
                [1, 0x2BE0_8438, 0xCBE0_84CB, 0, 0xABE0_91A8, 0, 1, 1], 0x100A, 0x8BDA_0C2B, true);
            load!(ini, dirt, dirt_key, dirt_section,
                [1, 0x4BE0_8AFB, 0x6BE0_8658, 0, 0x4BE0_91B2, 0, 1, 1], 0x100B, 0xEBE0_8CDA, true);
            load!(ini, rail, rail_key, rail_section,
                [2, 0xC843_3ECB, 0xE834_7989, 0, 0x217B_B308, 0, 1, 1], 0x1001, 0x0814_E612, true);
            load!(ini, light_rail, light_rail_key, light_rail_section,
                [2, 0x0B79_E2E1, 0x2B79_DFFB, 0, 0xAB79_E761, 0, 1, 1], 0x1008, 0xCB79_E089, true);
            load!(ini, monorail, monorail_key, monorail_section,
                [2, 0xABE0_8433, 0xEBE0_84C2, 0, 0xEBE0_91B6, 0, 1, 1], 0x1009, 0xEBE0_8882, true);
            load!(ini, street, street_key, street_section,
                [1, 0x092A_047D, 0xA92A_02EA, 0, 0x2A92_0BF9, 0, 1, 1], 0x1003, 0x492A_03A3, true);
            load!(ini, highway, highway_key, highway_section,
                [0, 0x0843_3F47, 0xA843_4037, 0, 0x8BF3_0A8B, 0, 2, 2], 0x1002, 0xA843_3F6B, false);
            load!(ini, ground_highway, ground_highway_key, ground_highway_section,
                [0, 0x6BE0_843B, 0xEBE0_84D1, 0, 0x6BE0_91B4, 0, 2, 2], 0x100C, 0x0BE0_8C87, false);
            load!(ini, avenue, avenue_key, avenue_section,
                [0, 0x0B73_0A26, 0xCB73_0FAC, 0, 0x6B73_0B83, 0, 2, 2], 0x1006, 0x2B73_0A17, false);
            load!(ini, pipe, pipe_key, pipe_section,
                [1, 0x0951_1847, 0xA951_171C, 0xF0, 0x82C2_FF49, 0, 1, 1], 0x1004, 0x0951_17DB, false);
            load!(ini, subway, subway_key, subway_section,
                [1, 0x8A15_EE13, 0x8A15_F3F2, 0xF1, 0xC17B_B329, 0, 1, 1], 0x1007, 0x0A36_7CA7, false);
            let record = power().cast::<u8>();
            clear(record);
            metadata(record, [0, 0xA88E_1592, 0x088E_1962, 0, 0xA16F_1463, 0x69E1_3770, 1, 2]);
            complete(record);
            verifiers(ini, verifier_section())
        }
    }
}

pub const PORTS: &[Port] = &[fc4p_00623310::PORT];

#[cfg(test)]
mod tests;
