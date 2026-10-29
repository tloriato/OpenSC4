// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Construction, simulator tuning, and saved style settings.

use super::*;
use core::mem::MaybeUninit;

game_fn! { #[allow(dead_code)] fn allocate(bytes: u32) -> u32 = "C" 0x0090_CF54; }
game_fn! { #[allow(dead_code)] fn allocate_object(bytes: u32) -> u32 = "C" 0x005E_55E0; }
game_fn! { #[allow(dead_code)] fn construct_context(this: u32) -> u32 = "thiscall" 0x0070_6390; }
game_fn! { #[allow(dead_code)] fn construct_developer(this: u32) -> u32 = "thiscall" 0x0070_C900; }
game_fn! { #[allow(dead_code)] fn query_interface(this: u32, iid: u32, out: u32) -> u32 = "thiscall" 0x0070_45B0; }
game_fn! { #[allow(dead_code)] fn property_float(holder: u32, id: u32, out: u32) -> u32 = "C" 0x005F_D4F0; }
game_fn! { #[allow(dead_code)] fn property_bool(holder: u32, id: u32, out: u32) -> u32 = "C" 0x005F_D390; }
game_fn! { #[allow(dead_code)] fn property_int(holder: u32, id: u32, out: u32) -> u32 = "C" 0x005F_D450; }
game_fn! { #[allow(dead_code)] fn insert_styles(this: u32, at: u32, begin: u32, end: u32, temporary: u32) -> u32 = "thiscall" 0x0041_4040; }
game_fn! { #[allow(dead_code)] fn copy_styles(this: u32, source: u32) -> u32 = "thiscall" 0x0046_2C10; }
game_fn! { #[allow(dead_code)] fn read_bool(stream: u32, out: u32) -> u32 = "C" 0x005D_9FA0; }
game_fn! { #[allow(dead_code)] fn resize_styles(this: u32, count: u32, value: u32) -> u32 = "thiscall" 0x006B_DF10; }
game_data! { static resource_manager: u32 = 0x00B4_3CA8; }

layout! {
    /// Growth context embedded at +0x80 in the developer.
    struct GrowthContext {
        0x4C => lots / set_lots: u32,
        0x50 => lot_count / set_lot_count: u32,
        0x58 => configurations / set_configurations: u32,
        0x5C => configuration_count / set_configuration_count: u32,
        0x64 => candidates / set_candidates: u32,
        0x68 => candidates_end / set_candidates_end: u32,
        0x6C => candidates_limit / set_candidates_limit: u32,
        0x8C => suitability / set_suitability: u32,
        0x90 => suitability_count / set_suitability_count: u32,
    }
}

register_port! {
    /// Construct empty lot, configuration and suitability trees and an empty candidate vector.
    0x0070_6390, fc4p_00706390, new_context, 0, 0, "ConstructTractGrowthContext", |r| {
        let this = r.ecx;
        store(this, 0x4C, 0);
        invoke(r, Conv::Cdecl, crate::abi::target(0x0090_CF54), &[0x14]);
        store(this, 0x4C, r.eax); store(this, 0x50, 0);
        *(r.eax as *mut u8) = 0;
        r.eax = word(this, 0x4C); store(r.eax, 4, 0);
        r.eax = word(this, 0x4C); store(r.eax, 8, r.eax);
        r.eax = word(this, 0x4C); store(r.eax, 12, r.eax);
        store(this, 0x58, 0);
        invoke(r, Conv::Cdecl, crate::abi::target(0x0090_CF54), &[0x14]);
        store(this, 0x58, r.eax); store(this, 0x5C, 0);
        *(r.eax as *mut u8) = 0;
        r.ecx = word(this, 0x58); store(r.ecx, 4, 0);
        r.eax = word(this, 0x58); store(r.eax, 8, r.eax);
        r.eax = word(this, 0x58); store(r.eax, 12, r.eax);
        for offset in [0x64, 0x68, 0x6C, 0x8C] { store(this, offset, 0); }
        invoke(r, Conv::Cdecl, crate::abi::target(0x0090_CF54), &[0x18]);
        store(this, 0x8C, r.eax); store(this, 0x90, 0);
        *(r.eax as *mut u8) = 0;
        r.edx = word(this, 0x8C); store(r.edx, 4, 0);
        r.eax = word(this, 0x8C); store(r.eax, 8, r.eax);
        r.eax = word(this, 0x8C); store(r.eax, 12, r.eax);
        r.eax = this;
    }
}

register_port! {
    /// Initialize interfaces, tuning defaults, context and empty style/filter containers.
    0x0070_C900, fc4p_0070c900, new_developer, 0, 0, "cSC4TractDeveloper::Construct", |r| {
        let this = r.ecx;
        store(this, 4, 0x00A8_1174); store(this, 8, 0x00A8_0784);
        r.eax = 0x3FC0_0000; store(this, 0x40, r.eax); store(this, 0x4C, r.eax);
        r.eax = 0x3E4C_CCCD; store(this, 0x54, r.eax); store(this, 0x60, r.eax);
        r.eax = 0x3DCC_CCCD;
        store(this, 0, 0x00AB_3118); store(this, 4, 0x00AB_3104); store(this, 8, 0x00AB_30EC);
        *(this.wrapping_add(12) as *mut u8) = 0;
        for offset in [0x10, 0x14, 0x18, 0x1C, 0x20, 0x24, 0x28, 0x2C, 0x30, 0x34, 0x38, 0x3C, 0x44] {
            store(this, offset, 0);
        }
        store(this, 0x48, 0x3F99_999A); store(this, 0x50, 0x3F80_0000);
        store(this, 0x58, 0x4120_0000); store(this, 0x5C, 0x40A0_0000);
        store(this, 0x64, r.eax); store(this, 0x68, r.eax);
        *(this.wrapping_add(0x6C) as *mut u8) = 1;
        for offset in [0x70, 0x74, 0x78, 0x7C] { store(this, offset, 0); }
        r.ecx = this.wrapping_add(0x80);
        invoke(r, Conv::Thiscall, crate::abi::target(0x0070_6390), &[]);
        for offset in [0x118, 0x11C, 0x120, 0x124, 0x128, 0x130] { store(this, offset, 0); }
        *(this.wrapping_add(0x12C) as *mut u8) = 1;
        invoke(r, Conv::Cdecl, crate::abi::target(0x0090_CF54), &[0x1C]);
        store(this, 0x130, r.eax); store(this, 0x134, 0); *(r.eax as *mut u8) = 0;
        r.eax = word(this, 0x130); store(r.eax, 4, 0);
        r.eax = word(this, 0x130); store(r.eax, 8, r.eax);
        r.eax = word(this, 0x130); store(r.eax, 12, r.eax);
        store(this, 0x13C, 0x00A8_27D8); store(this, 0x140, 0); store(this, 0x16C, 0);
        invoke(r, Conv::Cdecl, crate::abi::target(0x0090_CF54), &[0x3C]);
        store(r.eax, 0, r.eax); store(r.eax, 4, r.eax); store(this, 0x16C, r.eax);
        for offset in [0x170, 0x174, 0x178, 0x17C] { store(this, offset, 0); }
        invoke(r, Conv::Cdecl, crate::abi::target(0x0090_CF54), &[0x20]);
        store(this, 0x17C, r.eax); store(this, 0x180, 0); *(r.eax as *mut u8) = 0;
        r.ecx = word(this, 0x17C); store(r.ecx, 4, 0);
        r.eax = word(this, 0x17C); store(r.eax, 8, r.eax);
        r.eax = word(this, 0x17C); store(r.eax, 12, r.eax);
        r.eax = this;
    }
}

register_port! {
    /// Allocate a developer and construct it when allocation succeeds.
    0x0070_CAA0, fc4p_0070caa0, create_developer, 0, 0, "CreateTractDeveloper", |r| {
        invoke(r, Conv::Cdecl, crate::abi::target(0x005E_55E0), &[0x188]);
        if r.eax != 0 {
            r.ecx = r.eax;
            invoke(r, Conv::Thiscall, crate::abi::target(0x0070_C900), &[]);
        } else { r.eax = 0; }
    }
}

port! {
    /// Query interfaces through the message interface, retaining the caller's arguments.
    #[port(address = 0x0070_C890, owner = "tractdeveloper", name = "TractDeveloperMessageQueryInterface", naked)]
    pub unsafe extern "C" fn fc4p_0070c890() {
        core::arch::naked_asm!("sub ecx, 4", "push 0x007045b0", "ret");
    }
}
port! {
    /// Query interfaces through the persistence interface, retaining the caller's arguments.
    #[port(address = 0x0070_C8A0, owner = "tractdeveloper", name = "TractDeveloperPersistenceQueryInterface", naked)]
    pub unsafe extern "C" fn fc4p_0070c8a0() {
        core::arch::naked_asm!("sub ecx, 8", "push 0x007045b0", "ret");
    }
}

register_port! {
    /// Restore versioned style settings. The version reader writes into the caller's argument word.
    0x0070_7950, fc4p_00707950, read_settings, 1, 4, "cSC4TractDeveloper::Read", |r| {
        let this = r.ecx;
        let stream = r.args[0];
        r.eax = word(stream, 0); r.ecx = r.args.as_mut_ptr() as u32;
        let version_slot = r.ecx; r.ecx = stream;
        invoke(r, Conv::Thiscall, word(r.eax, 0x1C) as usize, &[version_slot]);
        if r.args[0] as u16 > 3 { r.eax &= !0xFF; return; }
        r.edx = word(stream, 0); r.eax = this.wrapping_add(0x11C); r.ecx = stream;
        invoke(r, Conv::Thiscall, word(r.edx, 0x20) as usize, &[r.eax]);
        r.edx = word(stream, 0); r.eax = this.wrapping_add(0x120); r.ecx = stream;
        invoke(r, Conv::Thiscall, word(r.edx, 0x20) as usize, &[r.eax]);
        let mut temporary = MaybeUninit::<u32>::uninit();
        if r.args[0] as u16 >= 2 {
            r.ecx = this.wrapping_add(0x124);
            invoke(r, Conv::Cdecl, crate::abi::target(0x005D_9FA0), &[stream, r.ecx]);
            let mut count = MaybeUninit::<u32>::uninit();
            r.edx = word(stream, 0); r.eax = count.as_mut_ptr() as u32; r.ecx = stream;
            invoke(r, Conv::Thiscall, word(r.edx, 0x24) as usize, &[r.eax]);
            r.edx = count.assume_init(); r.ecx = temporary.as_mut_ptr() as u32;
            let value = r.ecx; r.ecx = this.wrapping_add(0x110); temporary.write(0);
            invoke(r, Conv::Thiscall, crate::abi::target(0x006B_DF10), &[r.edx, value]);
            let mut index = 0u32;
            while index < count.assume_init() {
                r.ecx = word(this, 0x110); r.eax = word(stream, 0);
                r.edx = r.ecx.wrapping_add(index.wrapping_mul(4)); r.ecx = stream;
                invoke(r, Conv::Thiscall, word(r.eax, 0x24) as usize, &[r.edx]);
                r.eax = count.assume_init(); index = index.wrapping_add(1);
            }
        } else {
            r.eax = temporary.as_mut_ptr() as u32; r.ecx = this.wrapping_add(0x110);
            *(this.wrapping_add(0x124) as *mut u8) = 1; temporary.write(0);
            invoke(r, Conv::Thiscall, crate::abi::target(0x006B_DF10), &[3, r.eax]);
            r.ecx = word(this, 0x110); store(r.ecx, 0, 0x2000);
            r.edx = word(this, 0x110); store(r.edx, 4, 0x2001);
            r.eax = word(this, 0x110); store(r.eax, 8, 0x2002);
        }
        if r.args[0] as u16 >= 3 {
            r.edx = word(stream, 0); r.ecx = stream;
            invoke(r, Conv::Thiscall, word(r.edx, 0x20) as usize, &[this.wrapping_add(0x74)]);
        }
        r.eax = word(stream, 0); r.ecx = stream;
        invoke(r, Conv::Thiscall, word(r.eax, 0x48) as usize, &[]);
        r.eax = u32::from(r.eax as u8 == 0);
    }
}

/// Read an optional pair of float words, preserving the register setup of each virtual call.
unsafe fn tune_pair(r: &mut Entry<0>, holder: u32, this: u32, id: u32, offset: u32, edx_table: bool) {
    // SAFETY: holder is the tuning property holder and all returned interfaces are live.
    unsafe {
        let table = word(holder, 0);
        if edx_table {
            r.edx = table;
        } else {
            r.eax = table;
        }
        r.ecx = holder;
        invoke(r, Conv::Thiscall, word(table, 0x24) as usize, &[id]);
        if r.eax != 0 {
            r.edx = word(r.eax, 0);
            r.ecx = r.eax;
            invoke(r, Conv::Thiscall, word(r.edx, 0x14) as usize, &[]);
            r.edx = word(r.eax, 0);
            r.ecx = r.eax;
            invoke(r, Conv::Thiscall, word(r.edx, 0x1BC) as usize, &[]);
            r.ecx = word(r.eax, 0);
            store(this, offset, r.ecx);
            r.edx = word(r.eax, 4);
            store(this, offset + 4, r.edx);
        }
    }
}

register_port! {
    /// Read tuning defaults and optional arrays, then maintain the active style list and index.
    0x0070_6FC0, fc4p_00706fc0, read_tuning, 0, 0, "cSC4TractDeveloper::ReadSimulatorTunableValues", |r| {
        let this = r.ecx;
        let mut resource = MaybeUninit::<u32>::uninit();
        let key = [0x6534_284Au32, 0xE7E2_C2DB, 0xE8DA_7677];
        r.edx = key.as_ptr() as u32; r.ecx = *resource_manager(); r.eax = word(r.ecx, 0);
        invoke(r, Conv::Thiscall, word(r.eax, 0xC) as usize,
            &[r.edx, 0xA521_60F5, resource.as_mut_ptr() as u32, 0, 0]);
        if r.eax as u8 == 0 { return; }
        r.ecx = resource.assume_init(); r.eax = word(r.ecx, 0);
        invoke(r, Conv::Thiscall, word(r.eax, 0x1C) as usize, &[]);
        let holder = r.eax;
        r.eax = this.wrapping_add(0x40); store(r.eax, 0, 0x3FC0_0000);
        invoke(r, Conv::Cdecl, crate::abi::target(0x005F_D4F0), &[holder, 0x47E2_C500, r.eax]);
        store(this, 0x44, 0); store(this, 0x48, 0x3F99_999A);
        tune_pair(r, holder, this, 0x47E2_C501, 0x44, true);
        r.eax = this.wrapping_add(0x4C); store(r.eax, 0, 0x3FC0_0000);
        invoke(r, Conv::Cdecl, crate::abi::target(0x005F_D4F0), &[holder, 0x47E2_C502, r.eax]);
        store(this, 0x50, 0x3F80_0000); store(this, 0x54, 0x3E4C_CCCD);
        tune_pair(r, holder, this, 0x47E2_C510, 0x50, false);
        store(this, 0x58, 0x4120_0000); store(this, 0x5C, 0x40A0_0000);
        tune_pair(r, holder, this, 0x47E2_C511, 0x58, false);
        r.eax = this.wrapping_add(0x60); store(r.eax, 0, 0x3E4C_CCCD);
        invoke(r, Conv::Cdecl, crate::abi::target(0x005F_D4F0), &[holder, 0x47E2_C530, r.eax]);
        store(this, 0x64, 0x3C23_D70A); store(this, 0x68, 0x3DCC_CCCD);
        tune_pair(r, holder, this, 0x47E2_C531, 0x64, false);
        r.eax = this.wrapping_add(0x6C); *(r.eax as *mut u8) = 1;
        invoke(r, Conv::Cdecl, crate::abi::target(0x005F_D390), &[holder, 0x47E2_C540, r.eax]);
        r.eax = word(this, 0x74); r.ecx = word(this, 0x70); r.eax = r.ecx; store(this, 0x74, r.eax);
        r.eax = word(holder, 0); r.ecx = holder;
        invoke(r, Conv::Thiscall, word(r.eax, 0x24) as usize, &[0x47E2_C550]);
        if r.eax != 0 {
            r.edx = word(r.eax, 0); r.ecx = r.eax;
            invoke(r, Conv::Thiscall, word(r.edx, 0x14) as usize, &[]);
            let variant = r.eax;
            r.eax = word(variant, 0); r.ecx = variant;
            invoke(r, Conv::Thiscall, word(r.eax, 0x19C) as usize, &[]);
            let begin = r.eax;
            r.edx = word(variant, 0); r.ecx = variant;
            invoke(r, Conv::Thiscall, word(r.edx, 0x10) as usize, &[]);
            let mut temporary = MaybeUninit::<u8>::uninit();
            r.ecx = begin; r.edx = begin.wrapping_add(r.eax.wrapping_mul(4)); r.eax = word(this, 0x74);
            let args = [r.eax, r.ecx, r.edx, temporary.as_mut_ptr() as u32];
            r.ecx = this.wrapping_add(0x70);
            invoke(r, Conv::Thiscall, crate::abi::target(0x0041_4040), &args);
        }
        r.ecx = word(this, 0x118); r.eax = word(this, 0x11C);
        if r.ecx == r.eax {
            r.ecx = this.wrapping_add(0x118); store(this, 0x124, 0);
            invoke(r, Conv::Thiscall, crate::abi::target(0x0046_2C10), &[this.wrapping_add(0x70)]);
        }
        r.edx = word(this, 0x11C); r.eax = word(this, 0x124);
        r.edx = (r.edx.wrapping_sub(word(this, 0x118)) as i32 >> 2) as u32;
        if r.eax >= r.edx { store(this, 0x124, 0); }
        r.eax = word(this, 0x7C);
        if r.eax == 0 {
            store(this, 0x7C, 40);
            invoke(r, Conv::Cdecl, crate::abi::target(0x005F_D450), &[holder, 0x47E2_C551, this.wrapping_add(0x7C)]);
        }
        r.ecx = resource.assume_init(); r.eax = word(r.ecx, 0);
        invoke(r, Conv::Thiscall, word(r.eax, 8) as usize, &[]);
    }
}

/// This file's replacements.
pub const PORTS: &[Port] = &[
    fc4p_00706390::PORT,
    fc4p_00706fc0::PORT,
    fc4p_00707950::PORT,
    fc4p_0070c890::PORT,
    fc4p_0070c8a0::PORT,
    fc4p_0070c900::PORT,
    fc4p_0070caa0::PORT,
];
