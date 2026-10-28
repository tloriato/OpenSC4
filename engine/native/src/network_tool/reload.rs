// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Process-wide network rule resource reload.

use super::*;
use core::mem::MaybeUninit;

game_fn! { fn resource_file(key: *const u32, output: *mut *mut u8, flags: u32) -> u64 = "C" 0x005F_9670; }
game_fn! { fn ini_new(this: *mut u8, file: *mut u8) = "thiscall" 0x0091_CF46; }
game_fn! { fn ini_destroy(this: *mut u8) -> u64 = "thiscall" 0x0091_CDE3; }
game_fn! { fn reload_types(ini: *mut u8) = "C" 0x0062_3310; }
game_fn! { fn read_intersections(ini: *mut u8) = "C" 0x0064_1D30; }
game_fn! { fn read_matrix(ini: *mut u8, values: *mut u32) = "C" 0x0063_D630; }
game_fn! { fn clear_conflicts(this: *mut u8, root: *mut u8) = "thiscall" 0x0062_4CE0; }
game_fn! { fn read_conflicts(ini: *mut u8, section: *const u8) = "C" 0x0063_D3E0; }
game_fn! { fn read_highways(ini: *mut u8, output: *mut u8) = "C" 0x0064_1B60; }
game_data! { static intersections_key: [u32; 3] = 0x00B0_B544; }
game_data! { static conflicts_key: [u32; 3] = 0x00B0_B550; }
game_data! { static highways_key: [u32; 3] = 0x00B0_B538; }
game_data! { static type_matrix: [u8; 0xE04] = 0x00B4_53C4; }
game_data! { static conflict_tree: [u32; 2] = 0x00B4_66D0; }
game_data! { static conflict_section: u8 = 0x00AA_86F0; }
game_data! { static highway_rules: u8 = 0x00B4_52B0; }

port! {
    /// Try the four rule resources in order, reusing the file output across lookups. Each successful
    /// read constructs and destroys an INI reader. Only the final file reference is released here.
    /// Intersection masks start at zero; conflict trees are cleared only after a successful lookup.
    #[port(address = 0x0062_53D0, owner = "networktool", name = "cSC4NetworkTool::ReloadRules")]
    pub unsafe extern "C" fn fc4p_006253d0() -> u64 {
        unsafe {
            let mut file = core::ptr::null_mut();
            let key = [0u32, 0x8A59_71C5, 0x8A59_93B9];
            let mut ini = MaybeUninit::<[u32; 15]>::uninit();
            let ip = ini.as_mut_ptr().cast::<u8>();
            if resource_file(key.as_ptr(), &mut file, 0) as u8 != 0 {
                ini_new(ip, file);
                reload_types(ip);
                ini_destroy(ip);
            }
            if resource_file(intersections_key().cast(), &mut file, 0) as u8 != 0 {
                ini_new(ip, file);
                read_intersections(ip);
                let mut matrix = [0u32; 13];
                read_matrix(ip, matrix.as_mut_ptr());
                for (i, mask) in matrix.into_iter().enumerate() {
                    write(type_matrix().cast(), i * 0x114, mask);
                }
                ini_destroy(ip);
            }
            if resource_file(conflicts_key().cast(), &mut file, 0) as u8 != 0 {
                ini_new(ip, file);
                let tree = conflict_tree().cast::<u8>();
                if read::<u32>(tree, 4) != 0 {
                    let head: *mut u8 = read(tree, 0);
                    clear_conflicts(tree, read(head, 4));
                    let head: *mut u8 = read(tree, 0); write(head, 8, head);
                    let head: *mut u8 = read(tree, 0); write(head, 4, 0u32);
                    let head: *mut u8 = read(tree, 0); write(head, 12, head);
                    write(tree, 4, 0u32);
                }
                read_conflicts(ip, conflict_section());
                ini_destroy(ip);
            }
            let mut result = resource_file(highways_key().cast(), &mut file, 0);
            if result as u8 != 0 {
                ini_new(ip, file);
                read_highways(ip, highway_rules());
                result = ini_destroy(ip);
            }
            if !file.is_null() { result = vcall!(file, 8 => fn() -> u64); }
            result
        }
    }
}

pub const PORTS: &[Port] = &[fc4p_006253d0::PORT];

#[cfg(test)]
mod tests;
