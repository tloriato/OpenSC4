// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Random corner and interior rubble props, fitted back inside the supplied horizontal bounds.

use super::*;

game_data! { static small_extent: f32 = 0x00A9_4D50; }
game_data! { static large_extent: f32 = 0x00AA_209C; }
game_data! { static density: f64 = 0x00AB_1B68; }
game_data! { static random_state: u8 = 0x00B4_9AF0; }
game_data! { static prop_manager: *mut u8 = 0x00B4_3D10; }
game_data! { static ordinary_types: [u32; 6] = 0x00AB_1AD4; }
game_data! { static burnt_corners: [u32; 2] = 0x00AB_1AF4; }
game_data! { static burnt_interiors: [u32; 2] = 0x00AB_1AEC; }
game_data! { static corner_x: [u32; 4] = 0x00AB_1B54; }
game_data! { static corner_z: [u32; 4] = 0x00AB_1B44; }

game_fn! { #[allow(dead_code)] fn uniform(this: *mut u8, bound: u32) -> u64 = "thiscall" 0x0091_36B1; }
game_fn! {
    /// The two bounds occupy four words containing their double-precision storage bits.
    #[allow(dead_code)] fn gaussian(this: *mut u8, min_lo: u32, min_hi: u32, max_lo: u32, max_hi: u32) -> St0 = "thiscall" 0x0091_381F;
}

unsafe fn direct(address: u32, args: &[u32], eax: u32, edx: u32, floating: bool) -> RawCall {
    let mut call = RawCall::new(Conv::Thiscall, crate::abi::target(address), random_state() as u32, args);
    call.eax = eax;
    call.edx = edx;
    call.take_st0 = u32::from(floating);
    unsafe { call_raw(&mut call) };
    call
}

fn compare(value: F80, operand: f32) -> u16 {
    let status: u16;
    unsafe {
        core::arch::asm!("fld tbyte ptr [{value}]", "fcomp dword ptr [{operand}]", "fnstsw ax",
            value = in(reg) &value, operand = in(reg) &operand, out("ax") status, out("st(0)") _,);
    }
    status
}

unsafe fn gaussian_position(min: *const f32, max: *const f32, call: RawCall) -> RawCall {
    unsafe {
        let high = F80::from_f32(max.read_unaligned()).to_f64().to_bits();
        let low = F80::from_f32(min.read_unaligned()).to_f64().to_bits();
        direct(
            0x0091_381F,
            &[low as u32, (low >> 32) as u32, high as u32, (high >> 32) as u32],
            call.eax,
            call.edx,
            true,
        )
    }
}

unsafe fn fit_axis(
    position: *mut f32,
    lo: *const f32,
    hi: *const f32,
    actual_lo: *const f32,
    actual_hi: *const f32,
    call: &mut RawCall,
) {
    unsafe {
        let status = compare(F80::from_f32(actual_lo.read_unaligned()), lo.read_unaligned());
        call.eax = call.eax & 0xFFFF_0000 | u32::from(status);
        let offset = if status & 0x0500 == 0x0100 {
            Some(terrain::subtract_float(
                F80::from_f32(lo.read_unaligned()),
                actual_lo.read_unaligned(),
            ))
        } else {
            let status = compare(F80::from_f32(actual_hi.read_unaligned()), hi.read_unaligned());
            call.eax = call.eax & 0xFFFF_0000 | u32::from(status);
            if status & 0x4100 == 0 {
                Some(terrain::subtract_float(
                    F80::from_f32(hi.read_unaligned()),
                    actual_hi.read_unaligned(),
                ))
            } else {
                None
            }
        };
        if let Some(offset) = offset {
            position.write_unaligned(terrain::add_float(offset, position.read_unaligned()).to_f32());
        }
    }
}

port! {
    /// This static routine takes four stack words and pops sixteen bytes. Its local vectors retain
    /// the original eight-byte alignment so prop callbacks receive the original scratch addresses.
    #[port(address = 0x006C_4BD0, owner = "lotdeveloper", name = "cSC4LotDeveloper::AddLotRubbleProps", naked)]
    pub unsafe extern "C" fn fc4p_006c4bd0() {
        core::arch::naked_asm!("mov ecx, esp", "sub esp, 128", "call {}", "add esp, 128", "ret 16", sym rubble);
    }
}

unsafe extern "fastcall" fn rubble(stack: *mut u32, _entry_edx: u32) -> u64 {
    unsafe {
        let lot = stack.add(1).read_unaligned();
        let min = stack.add(2).read_unaligned() as *const f32;
        let max = stack.add(3).read_unaligned() as *const f32;
        let burnt = stack.add(4).read_unaligned() as u8 != 0;
        let frame = ((stack as usize).wrapping_sub(4) & !7).wrapping_sub(0x40) as *mut u8;
        let position = frame.add(0x1C).cast::<f32>();
        position
            .add(1)
            .cast::<u32>()
            .write_unaligned(min.add(1).cast::<u32>().read_unaligned());
        let dx = terrain::subtract_float(F80::from_f32(max.read_unaligned()), min.read_unaligned());
        let dz = terrain::subtract_float(F80::from_f32(max.add(2).read_unaligned()), min.add(2).read_unaligned());
        let depth = dz.to_f32();
        let area = (dz * dx).to_f32();
        let small = compare(dx, small_extent().read()) & 0x0500 == 0x0100
            || compare(F80::from_f32(depth), small_extent().read()) & 0x0500 == 0x0100;
        let large = compare(dx, large_extent().read()) & 0x4100 == 0
            && compare(F80::from_f32(depth), large_extent().read()) & 0x4100 == 0;
        let mut weighted = F80::ZERO;
        core::arch::asm!("fld dword ptr [{area}]", "fmul qword ptr [{density}]", "fstp tbyte ptr [{weighted}]",
            area = in(reg) &area, density = in(reg) density(), weighted = in(reg) &mut weighted, out("st(0)") _,);
        let converted = weighted.ftol();
        let count = (converted as i32).wrapping_add(4);
        let mut call = RawCall::new(Conv::Thiscall, 0, 0, &[]);
        call.eax = count as u32;
        call.edx = (converted as u64 >> 32) as u32;
        let mut index = 0i32;
        while index < count {
            let mut orientation = index as u32;
            let kind;
            if index < 4 {
                if small {
                    index = index.wrapping_add(1);
                    continue;
                }
                call = direct(0x0091_36B1, &[2], call.eax, call.edx, false);
                if call.eax == 0 {
                    index = index.wrapping_add(1);
                    continue;
                }
                call.eax = 0;
                if large {
                    call = direct(0x0091_36B1, &[if burnt { 2 } else { 6 }], 0, call.edx, false);
                }
                if call.eax == 0 {
                    orientation = (orientation + 2) % 4;
                }
                let types = if burnt {
                    burnt_corners().cast::<u32>()
                } else {
                    ordinary_types().cast::<u32>()
                };
                kind = types.wrapping_add(call.eax as usize).read_unaligned();
                let x_max = corner_x().cast::<u32>().add(index as usize).read_unaligned() != 0;
                let z_max = corner_z().cast::<u32>().add(index as usize).read_unaligned() != 0;
                position
                    .cast::<u32>()
                    .write_unaligned(if x_max { max } else { min }.cast::<u32>().read_unaligned());
                position
                    .add(2)
                    .cast::<u32>()
                    .write_unaligned(if z_max { max } else { min }.add(2).cast::<u32>().read_unaligned());
            } else {
                call = direct(0x0091_36B1, &[4], call.eax, call.edx, false);
                orientation = call.eax;
                call = direct(0x0091_36B1, &[if burnt { 2 } else { 6 }], call.eax, call.edx, false);
                let types = if burnt {
                    burnt_interiors().cast::<u32>()
                } else {
                    ordinary_types().cast::<u32>()
                };
                kind = types.wrapping_add(call.eax as usize).read_unaligned();
                call = gaussian_position(min, max, call);
                position.write_unaligned(call.st0.to_f32());
                call = gaussian_position(min.add(2), max.add(2), call);
                position.add(2).write_unaligned(call.st0.to_f32());
            }
            if kind != 0 {
                let output = frame.add(0x10).cast::<*mut u8>();
                let manager = prop_manager().read();
                call = virtual_call(
                    manager,
                    0x3C,
                    &[kind, output as u32],
                    output as u32,
                    table(manager),
                    false,
                );
                if call.eax as u8 != 0 {
                    let prop = output.read_unaligned();
                    call = virtual_call(prop, 0x0C, &[], call.eax, table(prop), false);
                    let occupant = call.eax as *mut u8;
                    call = virtual_call(occupant, 0x0C, &[], call.eax, table(occupant), false);
                    if call.eax as u8 != 0 {
                        let prop = output.read_unaligned();
                        call = virtual_call(prop, 0x0C, &[], table(prop), call.edx, false);
                        let occupant = call.eax as *mut u8;
                        call = virtual_call(occupant, 0x74, &[0x40], call.eax, table(occupant), false);
                        let prop = output.read_unaligned();
                        call = virtual_call(prop, 0x28, &[orientation], table(prop), call.edx, false);
                        let prop = output.read_unaligned();
                        call = virtual_call(prop, 0x0C, &[], call.eax, table(prop), false);
                        let occupant = call.eax as *mut u8;
                        call = virtual_call(occupant, 0x24, &[position as u32], call.eax, table(occupant), false);
                        let prop = output.read_unaligned();
                        call = virtual_call(prop, 0x0C, &[], call.eax, table(prop), false);
                        let occupant = call.eax as *mut u8;
                        let actual_lo = frame.add(0x28).cast::<f32>();
                        let actual_hi = frame.add(0x34).cast::<f32>();
                        call = virtual_call(
                            occupant,
                            0x28,
                            &[actual_lo as u32, actual_hi as u32],
                            call.eax,
                            table(occupant),
                            false,
                        );
                        fit_axis(position, min, max, actual_lo, actual_hi, &mut call);
                        fit_axis(
                            position.add(2),
                            min.add(2),
                            max.add(2),
                            actual_lo.add(2),
                            actual_hi.add(2),
                            &mut call,
                        );
                        let prop = output.read_unaligned();
                        call = virtual_call(prop, 0x0C, &[], call.eax, table(prop), false);
                        let occupant = call.eax as *mut u8;
                        call = virtual_call(occupant, 0x24, &[position as u32], call.eax, table(occupant), false);
                        let prop = output.read_unaligned();
                        let manager = prop_manager().read();
                        let manager_table = table(manager);
                        call = virtual_call(prop, 0x0C, &[], call.eax, table(prop), false);
                        let args = [lot, call.eax];
                        let mut insert = RawCall::new(
                            Conv::Thiscall,
                            (manager_table as *const usize).add(0x64 / 4).read_unaligned(),
                            manager as u32,
                            &args,
                        );
                        insert.eax = lot;
                        insert.edx = call.edx;
                        call_raw(&mut insert);
                        call = insert;
                    }
                    let prop = output.read_unaligned();
                    call = virtual_call(prop, 8, &[], call.eax, table(prop), false);
                }
            }
            index = index.wrapping_add(1);
        }
        call.eax = count as u32;
        result(call)
    }
}

pub const PORTS: &[Port] = &[fc4p_006c4bd0::PORT];
