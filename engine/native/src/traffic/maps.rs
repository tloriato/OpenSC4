// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Intersection flags, dirty bounds, water routing and background demand.

use super::*;

game_data! { static travel_class: [i32; 9] = 0x00AB_31A8; }
game_data! { static neighbor_x: [i32; 4] = 0x00AB_325C; }
game_data! { static neighbor_z: [i32; 4] = 0x00AB_326C; }
game_data! { static side_masks: [u16; 4] = 0x00B0_D470; }
game_data! { static growth: *mut u8 = 0x00B4_3D68; }
game_fn! { #[allow(clippy::too_many_arguments)] fn water_route(this: *mut u8, x0: u32, z0: u32, x1: u32, z1: u32,
output: *mut u32, route: u32, flags: u32) -> u64 = "thiscall" 0x0072_5170; }

unsafe fn intersection(s: TrafficSim, x: i32, z: i32) -> *mut u8 {
    s.intersections()
        .wrapping_add(((x as u32).wrapping_shl(8) | z as u32).wrapping_mul(46) as usize)
}

port! {
    /// Marks a same-mode intersection link, and for surface modes marks its neighbor's incoming mask.
    #[port(address = 0x0070_EAB0, owner = "traffic", name = "MarkIntersectionLink", naked)]
    pub unsafe extern "C" fn fc4p_0070eab0() {
        core::arch::naked_asm!("sub esp, 12", "mov edx, esp",
            "push [esp + 32]", "push [esp + 32]", "push [esp + 32]", "push [esp + 32]", "push [esp + 32]", "call {}",
            "mov eax, [esp]", "mov ecx, [esp + 4]", "mov edx, [esp + 8]", "add esp, 12", "ret 20", sym mark_link);
    }
}
unsafe extern "fastcall" fn mark_link(
    this: *mut u8,
    out: *mut Registers,
    x: i32,
    z: i32,
    enter: i32,
    exit: i32,
    travel: i32,
) {
    unsafe {
        let s = TrafficSim::from_ptr(this);
        let cell = intersection(s, x, z);
        let shift = enter.wrapping_mul(4).wrapping_add(exit) as u32;
        let value = cell.wrapping_add(travel.wrapping_mul(2) as usize).cast::<u16>();
        value.write_unaligned(value.read_unaligned() | 1u32.wrapping_shl(shift) as u16);
        let mut result = Registers {
            eax: cell as u32,
            ecx: shift,
            edx: this as u32,
        };
        if travel_class()
            .cast::<i32>()
            .wrapping_offset(travel as isize)
            .read_unaligned()
            == 1
        {
            let nx = neighbor_x()
                .cast::<i32>()
                .wrapping_offset(exit as isize)
                .read_unaligned()
                .wrapping_add(x);
            let nz = neighbor_z()
                .cast::<i32>()
                .wrapping_offset(exit as isize)
                .read_unaligned()
                .wrapping_add(z);
            result.eax = nx as u32;
            result.ecx = nz as u32;
            if nx >= s.min_cell_x() && nz >= s.min_cell_z() && nx <= s.max_cell_x() && nz <= s.max_cell_z() {
                let next = intersection(s, nx, nz);
                let opposite = exit.wrapping_add(2) % 4;
                let value = next
                    .wrapping_add(opposite.wrapping_mul(2).wrapping_add(0x24) as usize)
                    .cast::<u16>();
                let mask = side_masks()
                    .cast::<u16>()
                    .wrapping_offset(enter as isize)
                    .read_unaligned();
                result.ecx = (result.ecx & 0xFFFF_0000) | u32::from(mask);
                result.edx = enter as u32;
                value.write_unaligned(value.read_unaligned() | mask);
                result.eax = value as u32;
            }
        }
        out.write(result);
    }
}
port! {
    /// Initializes or expands the pending intersection rectangle without clipping or normalizing its bounds.
    #[port(address = 0x0070_EB60, owner = "traffic", name = "cSC4TrafficSimulator::InvalidateIntersectionData")]
    pub unsafe extern "thiscall" fn fc4p_0070eb60(this: *mut u8, x0: i32, z0: i32, x1: i32, z1: i32) -> u32 {
        let s = unsafe { TrafficSim::from_ptr(this) };
        if s.intersection_dirty() != 0 {
            s.set_dirty_x0(x0.min(s.dirty_x0()));
            s.set_dirty_z0(z0.min(s.dirty_z0()));
            s.set_dirty_x1(x1.max(s.dirty_x1()));
            s.set_dirty_z1(z1.max(s.dirty_z1()));
            this.wrapping_add(0x82C) as u32
        } else {
            s.set_dirty_x0(x0); s.set_dirty_z0(z0); s.set_dirty_x1(x1); s.set_dirty_z1(z1);
            s.set_intersection_dirty(1);
            x1 as u32
        }
    }
}
port! {
    /// Forwards five arguments to the water pathfinder, which also receives the address of the fifth argument word.
    #[port(address = 0x0070_EC10, owner = "traffic", name = "cSC4TrafficSimulator::GetWaterRoute", naked)]
    pub unsafe extern "C" fn fc4p_0070ec10() {
        core::arch::naked_asm!("mov edx, esp", "call {}", "ret 20", sym get_water_route);
    }
}
unsafe extern "fastcall" fn get_water_route(this: *mut u8, stack: *mut u32) -> u64 {
    unsafe {
        let s = TrafficSim::from_ptr(this);
        water_route(
            s.water_paths(),
            stack.add(1).read(),
            stack.add(2).read(),
            stack.add(3).read(),
            stack.add(4).read(),
            stack.add(5),
            stack.add(5).read(),
            0,
        )
    }
}

#[repr(C)]
struct BackgroundResult {
    regs: Registers,
    value: F80,
}

port! {
    /// Weighted signed demand from residential, commercial and industrial developer grids, returned unrounded in ST0.
    #[port(address = 0x0070_E870, owner = "traffic", name = "cSC4TrafficSimulator::GetBackgroundTraffic", naked)]
    pub unsafe extern "C" fn fc4p_0070e870() {
        core::arch::naked_asm!("sub esp, 24", "mov edx, esp", "push [esp + 32]", "push [esp + 32]", "call {}",
            "mov eax, [esp]", "mov edx, [esp + 8]", "fld tbyte ptr [esp + 12]", "add esp, 24", "ret 8", sym background);
    }
}
unsafe extern "fastcall" fn background(this: *mut u8, out: *mut BackgroundResult, x: i32, z: i32) {
    unsafe {
        let manager = *growth();
        let mut sums = [0i32; 3];
        let mut edx = 0;
        for (group, ids) in [
            (0, &[0x1010, 0x1020, 0x1030][..]),
            (1, &[0x3110, 0x3120, 0x3130, 0x3320, 0x3330][..]),
            (2, &[0x4100, 0x4200, 0x4300, 0x4400][..]),
        ] {
            for &id in ids {
                let developer = vcall!(manager, 0x80 => fn(u32) -> *mut u8, id);
                let grid = vcall!(developer, 0x3C => fn() -> *mut u8);
                let result = virtual_registers(grid, 0x1C, &[x as u32, z as u32], word(grid, 0), word(grid, 0));
                sums[group] = sums[group].wrapping_add(i32::from(result.eax as i16));
                edx = result.edx;
            }
        }
        let a = multiply_float(F80::from_i32(sums[0]), f32::from_bits(word(this, 0xCE8)));
        let b = multiply_float(F80::from_i32(sums[1]), f32::from_bits(word(this, 0xCEC)));
        let sum = a + b;
        let c = multiply_float(F80::from_i32(sums[2]), f32::from_bits(word(this, 0xCF0)));
        out.write(BackgroundResult {
            regs: Registers {
                eax: this as u32,
                ecx: 0,
                edx,
            },
            value: sum + c,
        });
    }
}

pub const PORTS: &[Port] = &[
    fc4p_0070e870::PORT,
    fc4p_0070eab0::PORT,
    fc4p_0070eb60::PORT,
    fc4p_0070ec10::PORT,
];

#[cfg(test)]
mod tests;
