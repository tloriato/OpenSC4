// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Daily checks for one lot, followed by business-unit designation and population maintenance.

use super::daily::DailyState;
use super::*;

layout! {
    /// Commute checks for empty lots use the pending network latch as an additional gate.
    pub struct CommuteChecks {
        0x5F => enabled / set_enabled: u8,
    }
}

game_data! { static rotation_pending: u8 = 0x00B4_9AE9; }
game_fn! { #[allow(dead_code)] fn rotate_lot(this: *mut u8, lot: *mut u8) -> u64 = "thiscall" 0x006C_4910; }
game_fn! { #[allow(dead_code)] fn test_power(this: *mut u8, lot: *mut u8, x: i32, z: i32) -> u64 = "thiscall" 0x006C_ABB0; }
game_fn! { #[allow(dead_code)] fn test_travel(this: *mut u8, lot: *mut u8, profile: *mut u8, x: i32, z: i32) -> u64 = "thiscall" 0x006C_AFC0; }
game_fn! { #[allow(dead_code)] fn test_desirability(this: *mut u8, lot: *mut u8, x: i32, z: i32) -> u64 = "thiscall" 0x006C_47C0; }
game_fn! { #[allow(dead_code)] fn test_water(this: *mut u8, lot: *mut u8, x: i32, z: i32) -> u64 = "thiscall" 0x006C_AE30; }
game_fn! { #[allow(dead_code)] fn test_condition(this: *mut u8, lot: *mut u8, x: i32, z: i32) -> u64 = "thiscall" 0x006C_4630; }
game_fn! { #[allow(dead_code)] fn test_coffee(this: *mut u8, lot: *mut u8) -> u64 = "thiscall" 0x006C_78D0; }
game_fn! { #[allow(dead_code)] fn profile_new(this: *mut u8) -> u64 = "thiscall" 0x0045_91E0; }
game_fn! { #[allow(dead_code)] fn profile_destroy(this: *mut u8) -> u64 = "thiscall" 0x0045_9220; }
game_fn! { #[allow(dead_code)] fn suitable_for_zot(lot: *mut u8, x: i32) -> u64 = "fastcall" 0x006C_3B10; }
game_fn! { #[allow(dead_code)] fn pick_unit(this: *mut u8, lot: *mut u8) -> u64 = "thiscall" 0x006C_B260; }

unsafe fn direct(address: u32, this: *mut u8, args: &[u32], eax: u32, edx: u32) -> RawCall {
    let mut call = RawCall::new(Conv::Thiscall, crate::abi::target(address), this as u32, args);
    call.eax = eax;
    call.edx = edx;
    unsafe { call_raw(&mut call) };
    call
}

/// Check the unit's first member through its two pointer indirections. Unit methods can replace the
/// membership list, so both the unit and the list are fetched again for the population pass.
unsafe fn first_member(unit: *mut u8, lot: *mut u8, call: &mut RawCall) -> bool {
    unsafe {
        *call = virtual_call(unit, 0x14, &[], call.eax, table(unit), false);
        call.eax = (call.eax as *const u32).read_unaligned();
        let first = (call.eax as *const *mut u8).read_unaligned();
        first.add(8).cast::<*mut u8>().read_unaligned() == lot
    }
}

port! {
    /// Supplies the caller's lot word, which the location getter replaces with z, and preserves the
    /// temporary profile, coordinate and unit output addresses used by subsequent callbacks.
    #[port(address = 0x006C_D3A0, owner = "lotdeveloper", name = "cSC4LotDeveloper::UpdateLot", naked)]
    pub unsafe extern "C" fn fc4p_006cd3a0() {
        core::arch::naked_asm!("lea edx, [esp]", "sub esp, 128", "call {}", "add esp, 128", "ret 4", sym update_lot);
    }
}

unsafe extern "fastcall" fn update_lot(this: *mut u8, stack: *mut u8) -> u64 {
    unsafe {
        let z = stack.add(4).cast::<u32>();
        let lot = z.read_unaligned() as *mut u8;
        let x = stack.sub(0x50).cast::<u32>();
        let changed = stack.sub(0x51);
        changed.write(0);
        let mut call = virtual_call(lot, 0x54, &[x as u32, z as u32], table(lot), x as u32, false);
        let developer = LotDeveloper::from_ptr(this);
        let state = DailyState::from_ptr(this);
        call.eax = call.eax & !255 | u32::from(state.roads_changed());
        if state.roads_changed() != 0 {
            call = virtual_call(lot, 0x78, &[], table(lot), call.edx, false);
            call.eax = call.eax & !255 | u32::from(rotation_pending().read());
            if call.eax as u8 != 0 {
                call = virtual_call(lot, 0xC0, &[], call.eax, table(lot), false);
                if call.eax != 0 {
                    call = virtual_call(lot, 0xEC, &[], table(lot), call.edx, false);
                    if call.eax == 0 {
                        call = virtual_call(lot, 0x70, &[], call.eax, table(lot), false);
                        if call.eax as u8 == 0 {
                            call = virtual_call(lot, 0x7C, &[], table(lot), call.edx, false);
                            if (7..=9).contains(&(call.eax as i32)) {
                                call = direct(0x006C_4910, this, &[lot as u32], call.eax, call.edx);
                            }
                        }
                    }
                }
            }
        }
        call = virtual_call(lot, 0xB8, &[0], call.eax, table(lot), false);
        if call.eax != 0 {
            let service = developer.building_development();
            call = virtual_call(service, 0x9C, &[call.eax], call.eax, table(service), false);
            let profile = call.eax as *mut u32;
            if profile.read_unaligned() == 0 {
                call = direct(
                    0x006C_ABB0,
                    this,
                    &[lot as u32, x.read_unaligned(), z.read_unaligned()],
                    z.read_unaligned(),
                    call.edx,
                );
                let powered = call.eax == 2;
                call = virtual_call(lot, 0x8C, &[], call.eax, table(lot), false);
                let condition = call.eax;
                let mut travel = powered;
                if condition == 0 && !powered {
                    call = virtual_call(lot, 0x90, &[1], table(lot), call.edx, false);
                    travel = false;
                } else if (condition == 1 && powered) || matches!(condition, 5 | 6) {
                    call = virtual_call(
                        lot,
                        0x90,
                        &[0],
                        if condition == 1 { call.eax } else { table(lot) },
                        if condition == 1 { table(lot) } else { call.edx },
                        false,
                    );
                }
                if travel {
                    call = direct(
                        0x006C_AFC0,
                        this,
                        &[lot as u32, profile as u32, x.read_unaligned(), z.read_unaligned()],
                        call.eax,
                        x.read_unaligned(),
                    );
                }
            } else {
                call = virtual_call(lot, 0x84, &[], table(lot), call.edx, false);
                let previous = call.eax;
                let mut next = previous;
                let mut reason = 0;
                call = direct(
                    0x006C_47C0,
                    this,
                    &[lot as u32, x.read_unaligned(), z.read_unaligned()],
                    call.eax,
                    x.read_unaligned(),
                );
                if call.eax as u8 == 0 {
                    next = 4;
                    reason = 1;
                }
                call = direct(
                    0x006C_ABB0,
                    this,
                    &[lot as u32, x.read_unaligned(), z.read_unaligned()],
                    z.read_unaligned(),
                    call.edx,
                );
                if call.eax == 0 {
                    next = 4;
                    reason = 2;
                }
                call = direct(
                    0x006C_AE30,
                    this,
                    &[lot as u32, x.read_unaligned(), z.read_unaligned()],
                    x.read_unaligned(),
                    z.read_unaligned(),
                );
                if call.eax as u8 == 0 {
                    next = 4;
                    reason = 3;
                }
                call = direct(
                    0x006C_AFC0,
                    this,
                    &[lot as u32, profile as u32, x.read_unaligned(), z.read_unaligned()],
                    profile as u32,
                    x.read_unaligned(),
                );
                if call.eax as u8 == 0 {
                    next = 4;
                    reason = 4;
                }
                if next != previous {
                    call = virtual_call(lot, 0x88, &[next, 0, reason], call.eax, table(lot), false);
                    changed.write(1);
                }
                call = direct(
                    0x006C_4630,
                    this,
                    &[lot as u32, x.read_unaligned(), z.read_unaligned()],
                    z.read_unaligned(),
                    call.edx,
                );
                let condition = call.eax;
                call = virtual_call(lot, 0x8C, &[], call.eax, table(lot), false);
                call = virtual_call(lot, 0x90, &[condition], table(lot), call.edx, false);
                let service = developer.building_development();
                call = virtual_call(service, 0xE8, &[], call.eax, table(service), false);
                if call.eax as u8 != 0 && profile.read_unaligned() == 2 {
                    call = direct(0x006C_78D0, this, &[lot as u32], call.eax, call.edx);
                }
            }
        } else {
            call.eax = call.eax & !255 | u32::from(state.can_check_commute());
            if state.can_check_commute() != 0 {
                call = virtual_call(lot, 0xA0, &[], call.eax, table(lot), false);
                if call.eax > 30 {
                    call = virtual_call(lot, 0x7C, &[], table(lot), call.edx, false);
                    let zone = call.eax.wrapping_sub(1);
                    call.eax = zone;
                    if zone < 6 {
                        let enabled = CommuteChecks::from_ptr(this).enabled();
                        call.eax = call.eax & !255 | u32::from(enabled);
                        if enabled != 0 {
                            let profile = stack.sub(0x40).cast::<u32>();
                            direct(0x0045_91E0, profile.cast(), &[], call.eax, call.edx);
                            profile.write_unaligned(if zone < 3 { 1 } else { 3 });
                            call = direct(
                                0x006C_AFC0,
                                this,
                                &[lot as u32, profile as u32, x.read_unaligned(), z.read_unaligned()],
                                profile as u32,
                                x.read_unaligned(),
                            );
                            direct(0x0045_9220, profile.cast(), &[], call.eax, call.edx);
                        }
                    }
                }
                call = direct(0x006C_3B10, lot, &[], z.read_unaligned(), x.read_unaligned());
                if call.eax as u8 != 0 {
                    call = direct(
                        0x006C_ABB0,
                        this,
                        &[lot as u32, x.read_unaligned(), z.read_unaligned()],
                        call.eax,
                        x.read_unaligned(),
                    );
                }
            }
        }
        call = virtual_call(lot, 0x64, &[], table(lot), call.edx, false);
        let unit_slot = stack.sub(0x4C).cast::<u32>();
        unit_slot.write_unaligned(call.eax);
        if call.eax != 0 && first_member(call.eax as *mut u8, lot, &mut call) {
            let dirty = state.units_changed();
            call.eax = call.eax & !255 | u32::from(dirty);
            if dirty != 0 {
                call = virtual_call(this, 0x88, &[unit_slot as u32], unit_slot as u32, table(this), false);
            }
        }
        call = virtual_call(lot, 0x64, &[], call.eax, table(lot), false);
        unit_slot.write_unaligned(call.eax);
        if call.eax != 0 {
            if first_member(call.eax as *mut u8, lot, &mut call) {
                call = virtual_call(
                    this,
                    0x8C,
                    &[unit_slot.read_unaligned()],
                    unit_slot.read_unaligned(),
                    table(this),
                    false,
                );
            }
        } else {
            call = virtual_call(lot, 0xE4, &[], call.eax, table(lot), false);
            call = virtual_call(lot, 0xE8, &[lot as u32], table(lot), call.edx, false);
            call = virtual_call(lot, 0x94, &[], call.eax, table(lot), false);
            if call.eax as u8 != 0 {
                call = virtual_call(lot, 0x70, &[], table(lot), call.edx, false);
                if call.eax as u8 == 0 {
                    call = direct(0x006C_B260, this, &[lot as u32], call.eax, call.edx);
                }
            }
        }
        call.eax = call.eax & !255 | u32::from(changed.read());
        result(call)
    }
}

pub const PORTS: &[Port] = &[fc4p_006cd3a0::PORT];
