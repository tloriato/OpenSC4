// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Power, water and travel checks and their shared grace counters.

use super::terrain::{add_float, divide_from_float};
use super::*;

game_fn! { #[allow(dead_code)] fn add_zot(this: *mut u8, lot: *mut u8, kind: u32, zot: u32) -> u64 = "thiscall" 0x006C_98C0; }
game_fn! { #[allow(dead_code)] fn remove_zot(this: *mut u8, lot: *mut u8, zot: u32) -> u64 = "thiscall" 0x006C_9A40; }
game_fn! { #[allow(dead_code)] fn remove_prop(this: *mut u8, lot: *mut u8, zot: u32) -> u64 = "thiscall" 0x006C_9780; }
game_data! { static zero: f32 = 0x00A8_1054; }

layout! {
    /// Services used by the three checks, from the primary developer interface.
    pub struct ServiceDeveloper {
        0x2C => water / set_water: *mut u8,
        0x34 => power / set_power: *mut u8,
        0x54 => travel / set_travel: *mut u8,
    }
}

// These checks reuse the low byte of their first stack argument. Keep the caller's argument words and
// the original scratch area available to the Rust body, including the other bytes of the power flag.
macro_rules! service_port {
    ($address:literal, $export:ident, $name:literal, $body:ident, $pop:literal) => {
        port! {
            /// Retains the original integer results and caller argument and scratch-byte writes.
            #[port(address = $address, owner = "lotdeveloper", name = $name, naked)]
            pub unsafe extern "C" fn $export() {
                core::arch::naked_asm!(
                    "sub esp, 128", "mov dword ptr [esp], ebp", "lea ebp, [esp + 128]",
                    "mov dword ptr [esp + 4], eax", "mov dword ptr [esp + 8], edx",
                    "mov edx, ebp", "lea eax, [esp + 4]", "push eax", "call {body}",
                    "mov eax, dword ptr [esp + 4]", "mov edx, dword ptr [esp + 8]",
                    "mov ebp, dword ptr [esp]", "add esp, 128",
                    concat!("ret ", stringify!($pop)), body = sym $body,
                )
            }
        }
    };
}

pub(super) use service_port;

/// Reads or creates a counter property and returns its variant and current word.
unsafe fn counter(lot: *mut u8, id: u32, edx: u32) -> (*mut u8, u32, RawCall) {
    unsafe {
        let mut call = virtual_call(lot, 0x0C, &[], table(lot), edx, false);
        let holder = call.eax as *mut u8;
        call = virtual_call(holder, 0x24, &[id], call.eax, table(holder), false);
        if call.eax == 0 {
            call = virtual_call(holder, 0x34, &[id, 0, 0], table(holder), call.edx, false);
            call = virtual_call(holder, 0x24, &[id], call.eax, table(holder), false);
        }
        let property = call.eax as *mut u8;
        call = virtual_call(property, 0x14, &[], call.eax, table(property), false);
        let variant = call.eax as *mut u8;
        call = virtual_call(variant, 0x124, &[], table(variant), call.edx, false);
        (variant, (call.eax as *const u32).read_unaligned(), call)
    }
}

/// Removes a matching zot, retaining the variant across the removal callback.
unsafe fn clear_matching_zot(this: *mut u8, lot: *mut u8, zot: u32, mut call: RawCall) -> RawCall {
    unsafe {
        let holder = call.eax as *mut u8;
        call = virtual_call(holder, 0x24, &[0xC999_C45E], call.eax, table(holder), false);
        if call.eax != 0 {
            let property = call.eax as *mut u8;
            call = virtual_call(property, 0x14, &[], call.eax, table(property), false);
            let variant = call.eax as *mut u8;
            call = virtual_call(variant, 0x124, &[], table(variant), call.edx, false);
            call.eax = (call.eax as *const u32).read_unaligned();
            if call.eax == zot {
                call = direct(0x006C_9780, this, &[lot as u32, zot], call);
                call = virtual_call(variant, 0x68, &[0], call.eax, table(variant), false);
            }
        }
        call
    }
}

unsafe fn direct(address: u32, this: *mut u8, args: &[u32], previous: RawCall) -> RawCall {
    let mut call = RawCall::new(Conv::Thiscall, crate::abi::target(address), this as u32, args);
    call.eax = previous.eax;
    call.edx = previous.edx;
    unsafe {
        call_raw(&mut call);
    }
    call
}

unsafe fn finish(output: *mut u64, returned: u64) {
    unsafe {
        output.write_unaligned(returned);
    }
}

service_port!(
    0x006C_ABB0,
    fc4p_006cabb0,
    "cSC4LotDeveloper::TestPower",
    test_power,
    12
);

unsafe extern "fastcall" fn test_power(this: *mut u8, stack: *mut u32, output: *mut u64) {
    unsafe {
        let developer = LotDeveloper::from_ptr(this);
        let lot = stack.add(1).read_unaligned() as *mut u8;
        let scratch = stack.cast::<u8>().sub(0x4C);
        let (variant, old_counter, mut call) = counter(lot, 0xE999_B11F, (output.read_unaligned() >> 32) as u32);
        call = virtual_call(lot, 0xC0, &[], call.eax, table(lot), false);
        let unit = call.eax as *mut u8;
        let needs_power = if unit.is_null() {
            true
        } else {
            call = virtual_call(lot, 0x104, &[], table(lot), call.edx, false);
            call.eax as u8 != 0
        };
        stack.add(1).cast::<u8>().write(u8::from(needs_power));
        scratch.add(0x10).write(0);
        call = virtual_call(
            lot,
            0x38,
            &[scratch.add(0x24) as u32],
            scratch.add(0x24) as u32,
            table(lot),
            false,
        );
        let service = developer.building_development();
        call = virtual_call(service, 0xDC, &[], call.eax, table(service), false);
        let cheat = call.eax as u8 != 0;
        let mut powered = cheat || !needs_power;
        if !cheat {
            call.eax = call.eax & !255 | u32::from(needs_power);
        }
        if !powered {
            if !unit.is_null() {
                call = virtual_call(unit, 0x0C, &[], table(unit), call.edx, false);
                let occupant = call.eax as *mut u8;
                for i in 0..3 {
                    scratch
                        .add(0x34 + i * 4)
                        .cast::<u32>()
                        .write_unaligned(1.0f32.to_bits());
                    scratch.add(0x40 + i * 4).cast::<u32>().write_unaligned(0);
                }
                call = virtual_call(
                    occupant,
                    0x28,
                    &[scratch.add(0x34) as u32, scratch.add(0x40) as u32],
                    call.eax,
                    table(occupant),
                    false,
                );
                let sum_x = add_float(
                    F80::from_f32(scratch.add(0x40).cast::<f32>().read_unaligned()),
                    scratch.add(0x34).cast::<f32>().read_unaligned(),
                )
                .to_f32();
                let city = developer.city();
                call = virtual_call(city, 0x16C, &[], call.eax, table(city), true);
                let size_x = call.st0;
                let x_result = divide_from_float(size_x + size_x, sum_x).ftol() as u64;
                let x = x_result as u32;
                scratch.add(0x18).cast::<u32>().write_unaligned(x);
                let sum_z = add_float(
                    F80::from_f32(scratch.add(0x48).cast::<f32>().read_unaligned()),
                    scratch.add(0x3C).cast::<f32>().read_unaligned(),
                )
                .to_f32();
                let city = developer.city();
                call = virtual_call(city, 0x170, &[], table(city), (x_result >> 32) as u32, true);
                let size_z = call.st0;
                let z = divide_from_float(size_z + size_z, sum_z).ftol() as u32;
                scratch.add(0x14).cast::<u32>().write_unaligned(z);
            } else {
                virtual_call(
                    lot,
                    0x54,
                    &[scratch.add(0x18) as u32, scratch.add(0x14) as u32],
                    scratch.add(0x14) as u32,
                    table(lot),
                    false,
                );
            }
            let x = scratch.add(0x18).cast::<u32>().read_unaligned();
            let z = scratch.add(0x14).cast::<u32>().read_unaligned();
            let service = ServiceDeveloper::from_ptr(this).power();
            call = virtual_call(service, 0x94, &[x, z], x, table(service), false);
            powered = call.eax as u8 != 0;
        }
        scratch.add(0x10).write(u8::from(powered));
        call = virtual_call(lot, 0x108, &[], call.eax, table(lot), false);
        if call.eax as u8 != u8::from(powered) {
            call = virtual_call(
                lot,
                0x10C,
                &[scratch.add(0x10).cast::<u32>().read_unaligned()],
                table(lot),
                call.edx,
                false,
            );
            call.eax = call.eax & !255 | u32::from(powered);
            call = virtual_call(lot, 0x8C, &[], call.eax, table(lot), false);
            if !powered {
                if call.eax == 0 {
                    call = virtual_call(lot, 0x90, &[1], table(lot), call.edx, false);
                }
            } else if call.eax == 1 {
                call = virtual_call(lot, 0x90, &[0], table(lot), call.edx, false);
            }
        }
        if !powered && needs_power {
            let next = old_counter.wrapping_add(1);
            call.eax = call.eax & !255 | 1;
            call = virtual_call(variant, 0x68, &[next], call.eax, table(variant), false);
            call = direct(0x006C_98C0, this, &[lot as u32, 0x0FD1_0000, 4], call);
            call.eax = u32::from(next <= 2);
        } else {
            call.eax = call.eax & !255 | u32::from(powered);
            call = virtual_call(variant, 0x68, &[0], call.eax, table(variant), false);
            if !unit.is_null() {
                call = virtual_call(lot, 0x0C, &[], table(lot), call.edx, false);
                call = clear_matching_zot(this, lot, 4, call);
            }
            call.eax = 2;
        }
        finish(output, result(call));
    }
}

service_port!(
    0x006C_AE30,
    fc4p_006cae30,
    "cSC4LotDeveloper::TestWater",
    test_water,
    12
);

unsafe extern "fastcall" fn test_water(this: *mut u8, stack: *mut u32, output: *mut u64) {
    unsafe {
        let developer = LotDeveloper::from_ptr(this);
        let lot = stack.add(1).read_unaligned() as *mut u8;
        let (variant, old_counter, mut call) = counter(lot, 0x8999_B20B, (output.read_unaligned() >> 32) as u32);
        stack.add(1).cast::<u8>().write(0);
        call = virtual_call(lot, 0x84, &[], call.eax, table(lot), false);
        let state = call.eax as i32;
        let mut needs_water = false;
        if (2..=4).contains(&state) {
            call = virtual_call(lot, 0xB8, &[0], table(lot), call.edx, false);
            let service = developer.building_development();
            call = virtual_call(service, 0x9C, &[call.eax], call.eax, table(service), false);
            let profile = call.eax as *mut u32;
            let purpose = profile.read_unaligned();
            if purpose == 0 {
                needs_water = true;
            } else {
                let wealth = profile.add(1).read_unaligned();
                let service = developer.building_development();
                call = virtual_call(service, 0x84, &[purpose, wealth], wealth, purpose, false);
                let growth = call.eax as *mut u8;
                call = virtual_call(lot, 0x24, &[], call.eax, table(lot), false);
                let config = call.eax as *mut u8;
                if !growth.is_null() && !config.is_null() {
                    call = virtual_call(config, 0x2C, &[], call.eax, table(config), false);
                    let stage = call.eax as u8;
                    call = virtual_call(growth, 0x50, &[], call.eax, table(growth), false);
                    let cap = call.eax as u8;
                    needs_water = stage > cap;
                }
            }
        }
        stack.add(1).cast::<u8>().write(u8::from(needs_water));
        let service = developer.building_development();
        call = virtual_call(service, 0xD8, &[], table(service), call.edx, false);
        let watered = if call.eax as u8 != 0 {
            true
        } else {
            let x = stack.add(2).read_unaligned();
            let z = stack.add(3).read_unaligned();
            let service = ServiceDeveloper::from_ptr(this).water();
            call = virtual_call(service, 0x2C, &[x, z], x, table(service), false);
            call.eax as u8 != 0
        };
        if !watered && needs_water {
            let next = old_counter.wrapping_add(1);
            call.eax = call.eax & !255 | 1;
            call = virtual_call(variant, 0x68, &[next], call.eax, table(variant), false);
            call = direct(0x006C_98C0, this, &[lot as u32, 0x1C43_0000, 5], call);
            call.eax = call.eax & !255 | u32::from(next <= 2);
        } else {
            call = virtual_call(variant, 0x68, &[0], table(variant), call.edx, false);
            call = virtual_call(lot, 0x0C, &[], call.eax, table(lot), false);
            call = clear_matching_zot(this, lot, 5, call);
            call.eax = call.eax & !255 | 1;
        }
        finish(output, result(call));
    }
}

service_port!(
    0x006C_AFC0,
    fc4p_006cafc0,
    "cSC4LotDeveloper::TestTravel",
    test_travel,
    16
);

unsafe extern "fastcall" fn test_travel(this: *mut u8, stack: *mut u32, output: *mut u64) {
    unsafe {
        let lot = stack.add(1).read_unaligned() as *mut u8;
        let mut call = virtual_call(
            lot,
            0x84,
            &[],
            table(lot),
            (output.read_unaligned() >> 32) as u32,
            false,
        );
        let state = call.eax;
        let mut fail = false;
        let mut road = true;
        let mut checked_access = false;
        if matches!(state, 1 | 3 | 4) {
            let purpose = (stack.add(2).read_unaligned() as *const i32).read_unaligned();
            stack.add(1).cast::<u8>().write(255);
            if purpose == 1 {
                checked_access = true;
                call = virtual_call(lot, 0x70, &[], purpose as u32, table(lot), false);
                road = call.eax as u8 != 0;
                if road {
                    call = virtual_call(lot, 0x6C, &[], table(lot), call.edx, false);
                    road = call.eax as u8 != 0;
                }
                let service = ServiceDeveloper::from_ptr(this).travel();
                call = virtual_call(service, 0x34, &[], call.eax, table(service), false);
                let grid = call.eax as *mut u8;
                call = virtual_call(
                    grid,
                    0x1C,
                    &[stack.add(3).read_unaligned(), stack.add(4).read_unaligned()],
                    call.eax,
                    table(grid),
                    false,
                );
                let trip = call.eax as u8;
                stack.add(1).cast::<u8>().write(trip);
                fail = !road || trip == 0;
            } else if (5..=8).contains(&purpose) {
                checked_access = true;
                call = virtual_call(lot, 0x74, &[purpose as u32], purpose as u32, table(lot), false);
                road = call.eax as u8 != 0;
                fail = !road;
            } else {
                call = virtual_call(
                    lot,
                    0x100,
                    &[stack.cast::<u8>().sub(16) as u32],
                    table(lot),
                    call.edx,
                    false,
                );
                let mut jobs = false;
                for wealth in 1..=3 {
                    let (eax, edx) = if wealth == 2 {
                        (table(lot), call.edx)
                    } else {
                        (call.eax, table(lot))
                    };
                    call = virtual_call(lot, 0xFC, &[wealth], eax, edx, true);
                    let count = call.st0;
                    let cmp = count.compare(F80::from_f32(zero().read()));
                    call.eax = call.eax & 0xFFFF_0000 | u32::from(cmp.status_word());
                    if !cmp.less() && !cmp.equal() && !cmp.unordered() {
                        jobs = true;
                        break;
                    }
                }
                if jobs {
                    checked_access = true;
                    call = virtual_call(lot, 0x70, &[], table(lot), call.edx, false);
                    road = call.eax as u8 != 0;
                    fail = !road;
                }
            }
            if checked_access && road {
                call.eax = call.eax & !255 | u32::from(stack.add(1).cast::<u8>().read());
            }
        }
        if fail {
            let (variant, old_counter, next_call) = counter(lot, 0x8999_B10B, call.edx);
            call = next_call;
            let next = old_counter.wrapping_add(1);
            call = virtual_call(variant, 0x68, &[next], call.eax, table(variant), false);
            if !road {
                call = direct(0x006C_98C0, this, &[lot as u32, 0x107A_0000, 6], call);
                call.eax = u32::from(next <= 2);
                finish(output, result(call));
                return;
            }
            if state == 3 && next > 2 {
                call = direct(0x006C_98C0, this, &[lot as u32, 0x1C44_0000, 7], call);
                call.eax = call.eax & !255 | u32::from(next <= 6);
                finish(output, result(call));
                return;
            }
        } else {
            call = virtual_call(lot, 0x0C, &[], call.eax, table(lot), false);
            let holder = call.eax as *mut u8;
            call = virtual_call(holder, 0x24, &[0x8999_B10B], call.eax, table(holder), false);
            if call.eax != 0 {
                let property = call.eax as *mut u8;
                call = virtual_call(property, 0x14, &[], call.eax, table(property), false);
                let variant = call.eax as *mut u8;
                call = virtual_call(variant, 0x68, &[0], call.eax, table(variant), false);
            }
        }
        call = direct(0x006C_9A40, this, &[lot as u32, 6], call);
        call = virtual_call(lot, 0x0C, &[], table(lot), call.edx, false);
        call = clear_matching_zot(this, lot, 7, call);
        call.eax = call.eax & !255 | 1;
        finish(output, result(call));
    }
}

pub const PORTS: &[Port] = &[fc4p_006cabb0::PORT, fc4p_006cae30::PORT, fc4p_006cafc0::PORT];
