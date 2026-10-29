// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Applying a compressed route to network volumes, switches, edge densities and ferry counters.

use super::create_maps::cell;
use super::late_containers::call;
use super::late_passes::virtual_call;
use super::*;
use crate::abi::{Conv, RawCall};

layout! {
    /// Maps and tuning used while applying a route.
    pub struct ApplyRouteSim {
        0x7C => network_rows / set_network_rows: *mut u8,
        0xE0 => departures / set_departures: *mut u8,
        0xE4 => departures_end / set_departures_end: *mut u8,
        0x164 => volumes / set_volumes: *mut u8,
        0x704 => switches / set_switches: *mut u8,
        0xD08 => fare / set_fare: f32,
    }
}
game_fn! { #[allow(dead_code)] fn route_begin(this: *mut u8, route: *mut u8) = "thiscall" 0x006D_64B0; }
game_fn! { #[allow(dead_code)] fn route_next(this: *mut u8, step: *mut u8) -> u32 = "thiscall" 0x006D_6510; }
game_fn! { #[allow(dead_code)] fn direction(x_word: u32, z_word: u32, target_x: i32, target_z: i32) -> u32 = "C" 0x0070_D780; }
game_fn! { #[allow(dead_code)] fn find_arrival(this: *mut u8, out: *mut u32, key: *mut u32) -> *mut u32 = "thiscall" 0x0060_D170; }
game_fn! { #[allow(dead_code)] fn neighbor_link(this: *mut u8, terminal: *mut u8, x_word: u32, z_word: u32) -> *mut u8 = "thiscall" 0x0070_F9F0; }
game_fn! { #[allow(dead_code)] fn terminal_link(this: *mut u8, terminal: *mut u8, key: u32) -> *mut u8 = "thiscall" 0x0070_F9C0; }
game_fn! { #[allow(dead_code)] fn transit_switch(this: *mut u8, key: *mut u32) -> *mut u8 = "thiscall" 0x006D_90F0; }
game_fn! { #[allow(dead_code)] fn edge_density(this: *mut u8, key: *mut u32) -> *mut u8 = "thiscall" 0x006D_7820; }
game_fn! { #[allow(dead_code)] fn destination(this: *mut u8, count: u32, x: u32, z: u32, travel: u32, kind: u32) = "thiscall" 0x0071_4740; }
// C's double argument occupies two stack words; the result must retain the whole ST0 value.
game_fn! { #[allow(dead_code)] fn ceiling(low: u32, high: u32) -> St0 = "C" 0x009F_0A20; }
game_data! { static ordinance: *mut u8 = 0x00B4_3D90; }
game_data! { static lot_manager: *mut u8 = 0x00B4_3D08; }
game_data! { static unsigned_bias: f32 = 0x00A8_0AA8; }
game_data! { static carpool_factor: f32 = 0x00AB_3158; }
game_data! { static rounding_bias: f32 = 0x00AB_32D8; }
game_data! { static travel_network: [u32; 9] = 0x00AB_31A8; }

fn low(value: u32, byte: u8) -> u32 {
    (value & !255) | u32::from(byte)
}
unsafe fn float(p: *mut u8, offset: usize) -> f32 {
    unsafe { f32::from_bits(word(p, offset)) }
}
unsafe fn add_short(p: *mut u8, offset: usize, n: u32) {
    unsafe {
        let p = p.add(offset).cast::<u16>();
        write_unaligned(p, read_unaligned(p).wrapping_add(n as u16));
    }
}
unsafe fn rounded(n: f32) -> u32 {
    unsafe {
        add_float(F80::from_f32(n), read_unaligned(rounding_bias()))
            .to_f32()
            .to_bits()
            .wrapping_add(0xB4C0_0000)
    }
}
fn inside(sim: TrafficSim, x: i32, z: i32) -> bool {
    x >= sim.min_cell_x() && z >= sim.min_cell_z() && x <= sim.max_cell_x() && z <= sim.max_cell_z()
}

fn add_double(value: F80, operand: f64) -> F80 {
    let mut out = F80::ZERO;
    unsafe {
        core::arch::asm!("fld tbyte ptr [{value}]", "fadd qword ptr [{operand}]", "fstp tbyte ptr [{out}]",
            value = in(reg) &value, operand = in(reg) &operand, out = in(reg) &mut out, out("st(0)") _);
    }
    out
}

/// Take ST0 and the integer registers together: an empty route can expose the ceiling call's EDX.
unsafe fn ceiling_value(bits: u64, r: &mut Registers) -> F80 {
    let args = [bits as u32, (bits >> 32) as u32];
    let mut c = RawCall::new(Conv::Cdecl, crate::abi::target(0x009F_0A20), r.ecx, &args);
    c.eax = r.eax;
    c.edx = r.edx;
    unsafe {
        core::arch::asm!(
            "mov eax, [edi + 16]", "push dword ptr [eax + 4]", "push dword ptr [eax]",
            "mov eax, [edi + 4]", "mov ecx, [edi + 8]", "mov edx, [edi + 12]", "call dword ptr [edi]",
            "mov [edi + 4], eax", "mov [edi + 8], ecx", "mov [edi + 12], edx", "lea esp, [esp + 8]",
            "fstp tbyte ptr [edi + 32]", in("edi") &mut c, clobber_abi("C"), out("st(0)") _,
        );
    }
    *r = Registers {
        eax: c.eax,
        ecx: c.ecx,
        edx: c.edx,
    };
    c.st0
}

unsafe fn bump_neighbor(link: *mut u8, mode: u32, n: u32) {
    unsafe {
        if mode == 0 || mode == 2 {
            add_short(link, 0x26, n);
        }
        if mode == 1 || mode == 2 {
            add_short(link, 0x2A, n);
        }
    }
}

/// DIV retains its hardware exception for an empty bucket array.
fn remainder(key: u32, count: u32) -> u32 {
    let rem: u32;
    unsafe {
        core::arch::asm!("xor edx, edx", "div {count:e}", count = in(reg) count,
            inout("eax") key => _, out("edx") rem, options(nomem, nostack));
    }
    rem
}

port! {
    /// Applies every decoded step, retaining partial volume and switch updates on an invalid intermediate
    /// side. Income and tolls are committed only on normal completion. Iterator buffers keep their original
    /// stack locations, and EDX retains the last call's or direction calculation's value.
    #[port(address = 0x0071_9910, owner = "traffic", name = "cSC4TrafficSimulator::ApplyRoute", naked)]
    pub unsafe extern "C" fn fc4p_00719910() {
        core::arch::naked_asm!(
            "sub esp, 0x8c", "push ebp", "mov ebp, esp", "sub esp, 12", "lea edx, [esp]",
            "lea eax, [ebp + 4]", "push eax", "lea eax, [ebp + 148]", "push eax", "call {body}",
            "mov eax, [ebp - 12]", "mov ecx, [ebp - 8]", "mov edx, [ebp - 4]",
            "mov esp, ebp", "pop ebp", "add esp, 0x8c", "ret 0x28", body = sym apply_route,
        );
    }
}

unsafe extern "fastcall" fn apply_route(this: *mut u8, out: *mut Registers, arguments: *mut u32, frame: *mut u8) {
    unsafe {
        let args: [u32; 10] = read_unaligned(arguments.cast());
        let [route, count, x0, z0, x1, z1, destination_x, destination_z, mode, kind] = args;
        let sim = ApplyRouteSim::from_ptr(this);
        let bounds = TrafficSim::from_ptr(this);
        let iterator = frame.add(116);
        let step = frame.add(32);
        let next = frame.add(88);
        let rect = frame.add(52);
        let key = frame.add(68).cast::<u32>();
        let result = frame.add(68);
        let arrival_result = frame.add(108);
        let mut r = Registers {
            eax: route,
            ecx: iterator as u32,
            edx: 0,
        };
        call(0x006D_64B0, Conv::Thiscall, &[route], &mut r);
        r.edx = word(sim.switches(), 0);
        virtual_call(sim.switches(), 0x60, &[], &mut r);
        let switches = r.eax as *mut u8;
        let mut n = F80::from_i32(count as i32);
        if (count as i32) < 0 {
            n = add_float(n, read_unaligned(unsigned_bias()));
        }
        let ordinary = n.to_f32();
        let mut cars = n.to_f32();
        let service = read_unaligned(ordinance());
        r.eax = count;
        r.edx = word(service, 0);
        virtual_call(service, 0x20, &[0xA2BF_1DDC], &mut r);
        if r.eax as u8 != 0 {
            let bits = multiply_float(F80::from_f32(ordinary), read_unaligned(carpool_factor()))
                .to_f64()
                .to_bits();
            cars = ceiling_value(bits, &mut r).to_f32();
        }
        let mut income = 0.0f32;
        let mut fees = 0.0f32;
        r.eax = step as u32;
        r.ecx = iterator as u32;
        call(0x006D_6510, Conv::Thiscall, &[step as u32], &mut r);
        if r.eax as u8 == 0 {
            out.write(r);
            return;
        }
        if mode != 1 {
            write_unaligned(rect.cast::<[u32; 4]>(), [x0, z0, x1, z1]);
        } else {
            let (x, z) = (i32::from(destination_x as i16), i32::from(destination_z as i16));
            write_unaligned(rect.cast::<[i32; 4]>(), [x, z, x, z]);
            if inside(bounds, x, z) {
                let manager = read_unaligned(lot_manager());
                r.edx = z as u32;
                virtual_call(manager, 0x18, &[x as u32, z as u32, 0], &mut r);
                if r.eax != 0 {
                    let lot = r.eax as *mut u8;
                    r.edx = word(lot, 0);
                    r.ecx = rect as u32;
                    virtual_call(lot, 0x38, &[rect as u32], &mut r);
                }
            }
        }
        let rectangle: [i32; 4] = read_unaligned(rect.cast());
        let mut sx = i32::from(read_unaligned(step.cast::<i16>()));
        let mut sz = i32::from(read_unaligned(step.add(2).cast::<i16>()));
        let beside_x = sx == rectangle[0].wrapping_sub(1) || sx == rectangle[2].wrapping_add(1);
        let beside_z = sz == rectangle[1].wrapping_sub(1) || sz == rectangle[3].wrapping_add(1);
        if beside_x == beside_z {
            let first_key = frame.add(28).cast::<u32>();
            first_key.write(((sx as u32 & 255) << 8) | sz as u32);
            r.ecx = this.add(0xF0) as u32;
            call(0x0060_D170, Conv::Thiscall, &[result as u32, first_key as u32], &mut r);
            let node = word(result, 0) as *mut u8;
            if !node.is_null() {
                let terminal = (word(node, 8) as *mut u8).add(8);
                r.edx = rectangle[0] as u32;
                r.ecx = this as u32;
                call(
                    0x0070_F9F0,
                    Conv::Thiscall,
                    &[terminal as u32, rectangle[0] as u32, rectangle[1] as u32],
                    &mut r,
                );
                if r.eax != 0 {
                    let q = if step.add(4).read() == 1 { cars } else { ordinary };
                    income = multiply_float(F80::from_f32(q), sim.fare()).to_f32();
                    let n = rounded(q);
                    r.ecx = n;
                    r.edx = if mode <= 1 { 0 } else { mode.wrapping_sub(2) };
                    bump_neighbor(r.eax as *mut u8, mode, n);
                }
            }
        }
        if sx < rectangle[0] {
            sx = sx.wrapping_add(1);
        } else if sx > rectangle[2] {
            sx = sx.wrapping_sub(1);
        } else if sz < rectangle[1] {
            sz = sz.wrapping_add(1);
        } else {
            sz = sz.wrapping_sub(1);
        }
        r.ecx = word(step, 2);
        r.edx = word(step, 0);
        call(
            0x0070_D780,
            Conv::Cdecl,
            &[word(step, 0), word(step, 2), sx as u32, sz as u32],
            &mut r,
        );
        step.add(7).write(r.eax as u8);
        let mut last_switch = 0u32;
        loop {
            let x = i32::from(read_unaligned(step.cast::<i16>()));
            let z = i32::from(read_unaligned(step.add(2).cast::<i16>()));
            let travel = step.add(4).read();
            // Only the low byte is assigned; the destination call pushes this whole scratch word.
            frame.add(48).write(travel);
            let q = if travel == 1 { cars } else { ordinary };
            let n = rounded(q);
            let in_city = inside(bounds, x, z);
            if travel != 1 {
                r.edx = ordinary.to_bits();
            }
            r.eax = n;
            // GetNext leaves its output untouched at the end, so the previous record remains available.
            r.ecx = iterator as u32;
            call(0x006D_6510, Conv::Thiscall, &[next as u32], &mut r);
            let last = r.eax as u8 == 0;
            r.eax = low(r.eax, u8::from(in_city));
            if in_city {
                let row = word(sim.network_rows(), x.wrapping_mul(12) as usize) as *mut u8;
                let mut info = word(row, z.wrapping_mul(4) as usize) as *mut u8;
                if !info.is_null() {
                    let network = read_unaligned(travel_network().cast::<u32>().add(usize::from(travel)));
                    while !info.is_null() && u32::from(info.add(6).read()) != network {
                        info = word(info, 0x14) as *mut u8;
                    }
                    if !info.is_null() && this.add(0xC34 + usize::from(travel)).read() != 0 {
                        let grid = word(sim.volumes(), network.wrapping_mul(4) as usize) as *mut u8;
                        r.eax = word(grid, 0);
                        r.edx = sim.volumes() as u32;
                        virtual_call(grid, 0x60, &[], &mut r);
                        let volume = cell::<f32>(r.eax as *mut u8, x, z);
                        let total = add_float(F80::from_f32(q), read_unaligned(volume));
                        write_unaligned(volume, total.to_f32());
                        if mode == 2 {
                            write_unaligned(volume, add_float(total, q).to_f32());
                        }
                    }
                }
                let switch_rows = word(switches, 0) as *mut u8;
                let switch_row = word(switch_rows, x.wrapping_mul(12) as usize) as *mut u8;
                let id = word(switch_row, z.wrapping_mul(4) as usize);
                r.edx = switches as u32;
                r.ecx = switch_row as u32;
                if id != last_switch {
                    if id != 0 {
                        let switch_key = frame.add(16).cast::<u32>();
                        switch_key.write(id);
                        r.edx = switch_key as u32;
                        r.ecx = this.add(0x70C) as u32;
                        call(0x006D_90F0, Conv::Thiscall, &[switch_key as u32], &mut r);
                        if travel != 6 || next.add(4).read() != 6 {
                            let sw = r.eax as *mut u8;
                            store(sw, 4, add_float(F80::from_f32(q), float(sw, 4)).to_f32().to_bits());
                            fees = add_float(multiply_float(F80::from_f32(q), float(sw, 0x2C)), fees).to_f32();
                        }
                    }
                    last_switch = id;
                }
                let entry = u32::from(step.add(7).read());
                let mut exit = u32::from(step.add(8).read());
                r.ecx = low(r.ecx, u8::from(last));
                if last {
                    if mode == 1 {
                        exit = if x < x0 as i32 {
                            2
                        } else if x > x1 as i32 {
                            0
                        } else {
                            1 + 2 * u32::from(z < z0 as i32)
                        };
                        if x >= x0 as i32 && x <= x1 as i32 {
                            r.edx = z as u32;
                            r.ecx = z0;
                        }
                    } else {
                        let dx = destination_x.wrapping_sub(x as u32) as i16;
                        let dz = destination_z.wrapping_sub(z as u32) as i16;
                        exit = match (dx, dz) {
                            (0, -1) => 1,
                            (0, 1) => 3,
                            (-1, 0) => 0,
                            (1, 0) => 2,
                            _ => u32::MAX,
                        };
                        r.edx = exit;
                    }
                }
                let valid = entry != 255 && exit != 255 && exit != u32::MAX;
                r.eax = low(exit, u8::from(valid));
                r.ecx = low(r.ecx, u8::from(last));
                if !last && !valid {
                    out.write(r);
                    return;
                }
                if valid {
                    let mut packed = (u32::from(mode == 1) << 24)
                        | (u32::from(travel) << 16)
                        | ((x as u32 & 255) << 8)
                        | (z as u32 & 255);
                    key.write(packed);
                    r.eax = packed;
                    r.edx = low(r.edx, u8::from(mode == 1));
                    r.ecx = this.add(0x178) as u32;
                    call(0x006D_7820, Conv::Thiscall, &[key as u32], &mut r);
                    let index = exit.wrapping_add(entry.wrapping_mul(4));
                    let density = (r.eax as *mut u8)
                        .wrapping_add(0x22 + index.wrapping_mul(2) as usize)
                        .cast::<u16>();
                    r.edx = index;
                    r.ecx = density as u32;
                    let value = u32::from(read_unaligned(density)).wrapping_add(n).min(65535);
                    write_unaligned(density, value as u16);
                    r.eax = value;
                    if mode == 2 {
                        packed |= 1 << 24;
                        key.write(packed);
                        r.eax = key as u32;
                        r.ecx = this.add(0x178) as u32;
                        call(0x006D_7820, Conv::Thiscall, &[key as u32], &mut r);
                        let index = entry.wrapping_add(exit.wrapping_mul(4));
                        let density = (r.eax as *mut u8)
                            .wrapping_add(0x22 + index.wrapping_mul(2) as usize)
                            .cast::<u16>();
                        r.edx = exit;
                        r.ecx = density as u32;
                        let value = u32::from(read_unaligned(density)).wrapping_add(n).min(65535);
                        write_unaligned(density, value as u16);
                        r.eax = value;
                    }
                }
                if !last && next.add(9).read() != 0 {
                    let packed = ((x as u32 & 255) << 8) | z as u32;
                    let count = (sim.departures_end() as u32).wrapping_sub(sim.departures() as u32) as i32 >> 2;
                    let bucket = remainder(packed & 65535, count as u32);
                    let mut node = word(sim.departures(), bucket.wrapping_mul(4) as usize) as *mut u8;
                    while !node.is_null() && read_unaligned(node.add(4).cast::<u16>()) != packed as u16 {
                        node = word(node, 0) as *mut u8;
                    }
                    r.edx = node as u32;
                    if !node.is_null() {
                        let terminal = (word(node, 8) as *mut u8).add(8);
                        add_short(terminal, 0x1A, n);
                        income = add_float(multiply_float(F80::from_f32(q), sim.fare()), income).to_f32();
                        if mode == 2 {
                            add_short(terminal, 0x1A, n);
                        }
                        key.write((u32::from(next.read()) << 8) | word(next, 2));
                        r.ecx = this.add(0xF0) as u32;
                        r.edx = key as u32;
                        call(
                            0x0060_D170,
                            Conv::Thiscall,
                            &[arrival_result as u32, key as u32],
                            &mut r,
                        );
                        let target_node = word(arrival_result, 0) as *mut u8;
                        if !target_node.is_null() {
                            let target = (word(target_node, 8) as *mut u8).add(8);
                            let target_key = (u32::from(target.read()) << 8) | u32::from(target.add(1).read());
                            r.ecx = this as u32;
                            call(0x0070_F9C0, Conv::Thiscall, &[terminal as u32, target_key], &mut r);
                            if r.eax != 0 {
                                if mode == 1 {
                                    r.edx = (r.edx & !65535) | (n & 65535);
                                    add_short(r.eax as *mut u8, 0x1E, n);
                                } else {
                                    add_short(r.eax as *mut u8, 0x1A, n);
                                }
                            }
                            if mode == 2 {
                                r.ecx = this as u32;
                                call(0x0070_F9C0, Conv::Thiscall, &[target as u32, packed], &mut r);
                                if r.eax != 0 {
                                    r.edx = (r.edx & !65535) | (n & 65535);
                                    add_short(r.eax as *mut u8, 0x1E, n);
                                }
                            }
                        } else {
                            r.eax = word(next, 0);
                            r.ecx = this as u32;
                            call(
                                0x0070_F9F0,
                                Conv::Thiscall,
                                &[terminal as u32, word(next, 0), word(next, 2)],
                                &mut r,
                            );
                            if r.eax != 0 {
                                bump_neighbor(r.eax as *mut u8, mode, n);
                            }
                        }
                    }
                }
            }
            income = add_float(
                multiply_float(F80::from_f32(q), float(this, 0xBA4 + usize::from(travel) * 4)),
                income,
            )
            .to_f32();
            // The income table index zero-extends the travel byte before AL becomes the end flag.
            r.eax = u32::from(last);
            if last {
                if mode != 1 {
                    r.ecx = this as u32;
                    r.edx = count;
                    r.eax = destination_z;
                    call(
                        0x0071_4740,
                        Conv::Thiscall,
                        &[count, destination_x, destination_z, word(frame, 48), kind],
                        &mut r,
                    );
                }
                bounds
                    .set_transit_accumulator(add_double(F80::from_f32(income), bounds.transit_accumulator()).to_f64());
                bounds.set_toll_accumulator(add_double(F80::from_f32(fees), bounds.toll_accumulator()).to_f64());
                out.write(r);
                return;
            }
            core::ptr::copy_nonoverlapping(next, step, 10);
            r.edx = word(next, 4);
        }
    }
}

pub const PORTS: &[Port] = &[fc4p_00719910::PORT];

#[cfg(test)]
mod tests;
