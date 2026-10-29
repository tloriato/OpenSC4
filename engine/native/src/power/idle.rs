// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! One step of the power recomputation cycle per idle notification.

use super::messages::{put, word, Registers};
use super::*;
use crate::abi::{vslot, Conv};
use core::ptr::null_mut;

layout! {
    /// Cycle services, counters and pending work, relative to the primary interface.
    struct IdleSim {
        0x14 => enabled / set_enabled: u8,
        0x18 => city / set_city: *mut u8,
        0x1C => budget / set_budget: *mut u8,
        0x20 => department / set_department: *mut u8,
        0x30 => neighbors / set_neighbors: *mut u8,
        0x38 => ring_index / set_ring_index: i32,
        0x3C => production / set_production: i32,
        0x40 => consumption / set_consumption: i32,
        0x44 => imports / set_imports: u32,
        0x48 => exports / set_exports: u32,
        0x4C => demand / set_demand: u32,
        0x64 => deficit / set_deficit: u32,
        0x68 => consumers / set_consumers: i32,
        0x6C => powered / set_powered: i32,
        0x70 => unpowered / set_unpowered: i32,
        0x78 => infrastructure_cost / set_infrastructure_cost: u64,
        0x298 => plants / set_plants: *mut u8,
        0x2AC => factor / set_factor: f32,
        0x2D0 => next_powered / set_next_powered: *mut u8,
        0x2D4 => current_powered / set_current_powered: *mut u8,
        0x2D9 => changed / set_changed: u8,
        0x308 => overworked / set_overworked: u32,
        0x30C => aged / set_aged: u32,
        0x310 => distressed / set_distressed: u32,
        0x338 => recompute / set_recompute: u8,
        0x339 => age_plants / set_age_plants: u8,
        0x33A => wire_dirty / set_wire_dirty: u8,
        0x34C => state / set_state: u32,
    }
}

game_fn! { #[allow(dead_code)] fn reset_wire(this: *mut u8, rect: *mut u8) -> u64 = "thiscall" 0x006E_B020; }
game_fn! { #[allow(dead_code)] fn update_wire(this: *mut u8, rect: *mut u8, skip: u32) -> u64 = "thiscall" 0x006E_D6E0; }
game_fn! { #[allow(dead_code)] fn update_grid(this: *mut u8, x: u32, z: u32) -> u64 = "thiscall" 0x006E_E110; }
game_fn! { #[allow(dead_code)] fn connection_cell(this: *mut u8, connection: *mut u8, x: *mut u32, z: *mut u32) -> u64 = "thiscall" 0x006E_B280; }
game_fn! { #[allow(dead_code)] fn update_lots(this: *mut u8) -> u64 = "thiscall" 0x006E_BD00; }
game_fn! { #[allow(dead_code)] fn age_plant(this: *mut u8, occupant: *mut u8) -> u64 = "thiscall" 0x006E_C8F0; }
game_fn! { #[allow(dead_code)] fn framework() -> u64 = "C" 0x0087_93EC; }
game_fn! { #[allow(dead_code)] fn release_reference(this: *mut u8) -> u64 = "thiscall" 0x0041_1870; }
game_fn! { #[allow(dead_code)] fn clear_connections(this: *mut u8) -> u64 = "thiscall" 0x0041_1D70; }
game_fn! { #[allow(dead_code)] fn clear_deals(this: *mut u8) -> u64 = "thiscall" 0x0042_1660; }
game_fn! { #[allow(dead_code)] fn new_deals(this: *mut u8) -> u64 = "thiscall" 0x0042_1A90; }
game_fn! { #[allow(dead_code)] fn delete_deals(this: *mut u8) -> u64 = "thiscall" 0x0042_1760; }
game_fn! { #[allow(dead_code)] fn bitmap_clear(bits: *mut u8, width: u32) -> u64 = "C" 0x0060_3E00; }
game_fn! { #[allow(dead_code)] fn property_float(properties: *mut u8, id: u32, out: *mut u32) -> u64 = "C" 0x005F_D4F0; }
game_fn! { #[allow(dead_code)] fn property_cost(properties: *mut u8, id: u32, out: *mut u64) -> u64 = "C" 0x005F_D4B0; }
game_fn! { #[allow(dead_code)] fn property_integer(properties: *mut u8, id: u32, out: *mut u32) -> u64 = "C" 0x005F_D450; }
game_fn! { #[allow(dead_code)] fn curve_new(this: *mut u8) -> u64 = "thiscall" 0x007D_0410; }
game_fn! { #[allow(dead_code)] fn curve_data(this: *mut u8, property: *mut u8) -> u64 = "thiscall" 0x005F_4620; }
game_fn! { fn curve_output(this: *mut u8, input: f32) -> St0 = "thiscall" 0x005F_4250; }
game_fn! { #[allow(dead_code)] fn free(p: *mut u8) -> u64 = "C" 0x0090_CF63; }
game_fn! { #[allow(dead_code)] fn new_message(size: u32) -> u64 = "C" 0x0091_33DA; }
game_fn! { #[allow(dead_code)] fn message_construct(this: *mut u8) -> u64 = "thiscall" 0x0091_34D6; }
game_fn! { #[allow(dead_code)] fn divide_cost(low: u32, high: u32, divisor_low: u32, divisor_high: u32) -> u64 = "stdcall" 0x009E_FEB0; }
game_fn! { #[allow(dead_code)] fn notify(id: u32, flags: u32, a: u32, b: u32, c: u32, d: u32) -> u64 = "C" 0x005F_98C0; }

game_data! { static deal_allocator: *mut u8 = 0x00B2_0D58; }
game_data! { static connection_allocator: *mut u8 = 0x00B2_07DC; }
game_data! { static statistics: *mut u8 = 0x00B4_3DC0; }
game_data! { static global_budget: *mut u8 = 0x00B4_3D64; }
game_data! { static global_city: *mut u8 = 0x00B4_3CEC; }
game_data! { static message_server: *mut u8 = 0x00B4_3CCC; }
game_data! { static advisor: *mut u8 = 0x00B4_3D8C; }
game_data! { static zero_percent: u32 = 0x00B4_A5B0; }
game_data! { static one: f64 = 0x00A8_0AB0; }
game_data! { static hundred: f64 = 0x00A8_0AC0; }

unsafe fn qword(p: *mut u8, offset: usize) -> u64 {
    unsafe { p.add(offset).cast::<u64>().read_unaligned() }
}
unsafe fn putq(p: *mut u8, offset: usize, value: u64) {
    unsafe { p.add(offset).cast::<u64>().write_unaligned(value) };
}
unsafe fn float(p: *mut u8, offset: usize) -> f32 {
    f32::from_bits(unsafe { word(p, offset) })
}
fn converted(r: &mut Registers, value: F80) -> u64 {
    let result = value.ftol() as u64;
    r.eax = result as u32;
    r.edx = (result >> 32) as u32;
    result
}

fn divide_signed(numerator: i32, denominator: i32) -> i32 {
    let quotient: i32;
    // SAFETY: the integer division also retains the processor's faults on invalid input.
    unsafe {
        core::arch::asm!("cdq", "idiv {denominator}", denominator = in(reg) denominator,
            inlateout("eax") numerator => quotient, out("edx") _);
    }
    quotient
}

fn percentage_complement(fraction: F80) -> F80 {
    let mut result = F80::ZERO;
    // SAFETY: readable operands and one balanced x87 push/pop.
    unsafe {
        core::arch::asm!("fld tbyte ptr [{fraction}]", "fsubr qword ptr [{one}]",
            "fmul qword ptr [{hundred}]", "fstp tbyte ptr [{result}]",
            fraction = in(reg) &fraction, one = in(reg) one(), hundred = in(reg) hundred(),
            result = in(reg) &mut result, out("st(0)") _);
    }
    result
}

/// Load the cached allocator, resolving it through the framework only when the cache is null.
unsafe fn allocator(cache: *mut *mut u8, frame: *mut u8, r: &mut Registers) -> *mut u8 {
    unsafe {
        if (*cache).is_null() {
            put(frame, 0x1C, 0);
            r.game(0x0087_93EC, null_mut(), &[], Conv::Cdecl);
            if r.eax != 0 {
                r.virtual_edx(
                    r.eax as *mut u8,
                    0x14,
                    &[0x3AE4_BEA3, 0x3AE4_BEAB, frame.add(0x1C) as u32],
                );
            }
            r.edx = word(frame, 0x1C);
            *cache = r.edx as *mut u8;
            r.game(0x0041_1870, frame.add(0x1C), &[], Conv::Thiscall);
        }
        *cache
    }
}
unsafe fn head(cache: *mut *mut u8, frame: *mut u8, r: &mut Registers) -> *mut u8 {
    unsafe {
        let a = allocator(cache, frame, r);
        r.virtual_eax(a, 0x0C, &[12]);
        let p = r.eax as *mut u8;
        put(p, 0, r.eax);
        put(p, 4, r.eax);
        p
    }
}
unsafe fn clear_list(cache: *mut *mut u8, frame: *mut u8, offset: usize, clear: u32, r: &mut Registers) {
    unsafe {
        r.game(clear, frame.add(offset), &[], Conv::Thiscall);
        let p = word(frame, offset);
        let a = allocator(cache, frame, r);
        r.virtual_eax(a, 0x10, &[p]);
    }
}

port! {
    /// Process wire changes and one state of the five-state power cycle. The budget argument is ignored.
    /// Reserve the cycle's scratch words for property outputs, response curves and temporary lists.
    #[port(address = 0x006E_E960, owner = "power", name = "cSC4PowerSimulator::DoIdle", naked)]
    pub unsafe extern "C" fn fc4p_006ee960() {
        core::arch::naked_asm!(
            "sub esp, 0x70", "push ebx", "push esi", "push edi", "push ebp",
            "push edx", "push eax", "lea edx, [esp + 8]", "call {body}",
            "pop ebp", "pop edi", "pop esi", "pop ebx", "add esp, 0x70", "ret 4",
            body = sym idle_step,
        );
    }
}

unsafe extern "fastcall" fn idle_step(this: *mut u8, frame: *mut u8, entry_eax: u32, entry_edx: u32) -> u64 {
    unsafe {
        let sim = IdleSim::from_ptr(this);
        let stats = *statistics();
        let mut r = Registers {
            eax: (entry_eax & !255) | u32::from(sim.enabled()),
            edx: entry_edx,
        };
        if sim.enabled() != 0 {
            if sim.wire_dirty() != 0 {
                r.game(0x006E_B020, this, &[this.add(0x33C) as u32], Conv::Thiscall);
                r.game(0x006E_D6E0, this, &[this.add(0x33C) as u32, 0], Conv::Thiscall);
                sim.set_wire_dirty(0);
            }
            r.eax = sim.state();
            match r.eax {
                0 => start_cycle(sim, frame, &mut r),
                1 => plants(sim, frame, &mut r),
                2 => imports(sim, frame, &mut r),
                3 => finish_cycle(sim, frame, stats, &mut r),
                4 if sim.recompute() != 0 => {
                    sim.set_state(0);
                    sim.set_recompute(0);
                }
                _ => {}
            }
        }
        (u64::from(r.edx) << 32) | u64::from(r.eax)
    }
}

unsafe fn start_cycle(sim: IdleSim, frame: *mut u8, r: &mut Registers) {
    unsafe {
        let this = sim.as_ptr();
        let next = sim.ring_index().wrapping_add(1);
        sim.set_ring_index(if next < 12 { next } else { 0 });
        for i in 0..10 {
            let offset = (sim.ring_index() as u32).wrapping_mul(40).wrapping_add(0x90 + i * 4);
            let old = word(this.wrapping_add(offset as usize), 0);
            put(
                this,
                0x270 + i as usize * 4,
                word(this, 0x270 + i as usize * 4).wrapping_sub(old),
            );
            let offset = (sim.ring_index() as u32).wrapping_mul(40).wrapping_add(0x90 + i * 4);
            put(this.wrapping_add(offset as usize), 0, 0);
        }
        // Buy deals, sell deals, then power connections. Each has its own list lifetime.
        for (offset, cache, slot, kind, visit) in [
            (0x18, deal_allocator(), 0x78, 0x4773_FFE0, 0x54),
            (0x14, deal_allocator(), 0x78, 0x4773_FFE1, 0x54),
            (0x10, connection_allocator(), 0x44, 4, 0x30),
        ] {
            put(frame, offset, 0);
            let h = head(cache, frame, r);
            put(frame, offset, h as u32);
            r.eax = frame.add(offset) as u32;
            r.virtual_edx(sim.neighbors(), slot, &[kind, r.eax]);
            let mut node = word(word(frame, offset) as *mut u8, 0) as *mut u8;
            while node as u32 != word(frame, offset) {
                r.virtual_edx(word(node, 8) as *mut u8, visit, &[0]);
                node = word(node, 0) as *mut u8;
            }
        }
        clear_list(connection_allocator(), frame, 0x10, 0x0041_1D70, r);
        clear_list(deal_allocator(), frame, 0x14, 0x0042_1660, r);
        clear_list(deal_allocator(), frame, 0x18, 0x0042_1660, r);
        for offset in [0x3C, 0x40, 0x48, 0x44, 0x4C, 0x6C, 0x70] {
            put(this, offset, 0);
        }
        let mut node = word(sim.plants(), 0) as *mut u8;
        while node != sim.plants() {
            put(node, 8, 0);
            node = word(node, 0) as *mut u8;
        }
        r.virtual_edx(sim.city(), 0x174, &[]);
        let width = r.eax;
        r.eax = sim.next_powered() as u32;
        r.game(0x0060_3E00, null_mut(), &[r.eax, width], Conv::Cdecl);
        sim.set_changed(0);
        sim.set_state(1);
    }
}

unsafe fn plants(sim: IdleSim, frame: *mut u8, r: &mut Registers) {
    unsafe {
        let this = sim.as_ptr();
        let h = sim.plants();
        let mut node = word(h, 0) as *mut u8;
        putq(frame, 0x34, sim.infrastructure_cost());
        put(frame, 0x20, h as u32);
        while node != h {
            if word(node, 8) == 0 {
                r.edx = frame.add(0x44) as u32;
                r.virtual_eax(word(node, 12) as *mut u8, 0x20, &[r.edx]);
                r.eax = word(this, 0);
                r.edx = word(frame, 0x44);
                r.call(
                    vslot(this, 0x14),
                    this,
                    &[r.edx, word(frame, 0x4C), frame.add(0x28) as u32, frame.add(0x24) as u32],
                    Conv::Thiscall,
                );
                r.eax = word(frame, 0x24);
                r.game(0x006E_E110, this, &[word(frame, 0x28), r.eax], Conv::Thiscall);
            }
            putq(frame, 0x2C, 0);
            put(frame, 0x18, 0);
            put(frame, 0x1C, 0);
            put(frame, 0x14, 0);
            r.virtual_edx(word(node, 12) as *mut u8, 0x18, &[]);
            let properties = r.eax as *mut u8;
            for (address, id, offset) in [
                (0x005F_D4F0, 0xC910_B4CE, 0x18),
                (0x005F_D4B0, 0x2A58_D350, 0x2C),
                (0x005F_D450, 0x08D9_763D, 0x1C),
                (0x005F_D4F0, 0xC910_B4CF, 0x14),
            ] {
                r.game(
                    address,
                    null_mut(),
                    &[properties as u32, id, frame.add(offset) as u32],
                    Conv::Cdecl,
                );
            }
            let cost = converted(
                r,
                multiply_float(F80::from_i32(word(frame, 0x1C) as i32), float(frame, 0x18)),
            );
            putq(frame, 0x3C, cost);
            r.virtual_edx(properties, 0x24, &[0x0911_E2E7]);
            put(frame, 0x10, r.eax);
            if r.eax != 0 {
                r.game(0x007D_0410, frame.add(0x50), &[], Conv::Thiscall);
                r.eax = word(frame, 0x10);
                r.game(0x005F_4620, frame.add(0x50), &[r.eax], Conv::Thiscall);
                let output = curve_output(frame.add(0x50), float(frame, 0x14));
                let maintenance = converted(r, output * F80::from_i64(qword(frame, 0x2C) as i64));
                putq(frame, 0x2C, maintenance);
                let budget = *global_budget();
                put(frame, 0x7C, 0);
                put(frame, 0x10, budget as u32);
                r.eax = frame.add(0x60) as u32;
                r.virtual_edx(budget, 0xCC, &[properties as u32, 0xCA58_E540, r.eax]);
                if r.eax as u8 != 0 && qword(frame, 0x2C) != qword(frame, 0x70) {
                    r.edx = word(frame, 0x60);
                    r.virtual_eax(word(frame, 0x10) as *mut u8, 0x70, &[r.edx]);
                    if r.eax != 0 {
                        r.virtual_edx(
                            r.eax as *mut u8,
                            0x64,
                            &[properties as u32, word(frame, 0x2C), word(frame, 0x30), 0xCA58_E540],
                        );
                        r.eax = word(frame, 0x78);
                        r.virtual_edx(
                            word(frame, 0x10) as *mut u8,
                            0xD4,
                            &[properties as u32, r.eax, word(frame, 0x2C), word(frame, 0x30)],
                        );
                    }
                }
                r.eax = word(frame, 0x50);
                if r.eax != 0 {
                    r.game(0x0090_CF63, null_mut(), &[r.eax], Conv::Cdecl);
                }
            }
            node = word(node, 0) as *mut u8;
            putq(frame, 0x34, qword(frame, 0x34).wrapping_add(qword(frame, 0x3C)));
        }
        r.virtual_eax(sim.department(), 0x50, &[1]);
        r.virtual_edx(r.eax as *mut u8, 0x30, &[word(frame, 0x34), word(frame, 0x38)]);
        if sim.age_plants() != 0 {
            r.edx = sim.plants() as u32;
            sim.set_overworked(0);
            sim.set_aged(0);
            sim.set_distressed(0);
            sim.set_age_plants(0);
            let mut node = word(sim.plants(), 0) as *mut u8;
            while node != h {
                r.eax = word(node, 12);
                r.game(0x006E_C8F0, this, &[r.eax], Conv::Thiscall);
                node = word(node, 0) as *mut u8;
            }
        }
        sim.set_state(2);
    }
}

unsafe fn imports(sim: IdleSim, frame: *mut u8, r: &mut Registers) {
    unsafe {
        let this = sim.as_ptr();
        r.game(0x0042_1A90, frame.add(0x14), &[], Conv::Thiscall);
        r.game(0x0042_1660, frame.add(0x14), &[], Conv::Thiscall);
        r.eax = frame.add(0x14) as u32;
        r.virtual_edx(sim.neighbors(), 0x78, &[0x4773_FFE0, r.eax]);
        let mut node = word(word(frame, 0x14) as *mut u8, 0) as *mut u8;
        while node as u32 != word(frame, 0x14) {
            put(frame, 0x18, node as u32);
            let deal = word(node, 8) as *mut u8;
            r.virtual_edx(deal, 0x1C, &[]);
            let city = r.eax;
            r.virtual_eax(deal, 0x50, &[]);
            let used = r.eax as i32;
            r.virtual_edx(deal, 0x28, &[]);
            if used < r.eax as i32 {
                r.virtual_eax(sim.neighbors(), 0x3C, &[4, city]);
                while r.eax != 0 {
                    let connection = r.eax as *mut u8;
                    r.virtual_edx(connection, 0x30, &[1]);
                    r.virtual_eax(sim.neighbors(), 0x34, &[connection as u32]);
                    if r.eax != 0 {
                        r.virtual_edx(r.eax as *mut u8, 0x34, &[]);
                        if r.eax as u8 != 0 {
                            r.eax = frame.add(0x24) as u32;
                            r.game(
                                0x006E_B280,
                                this,
                                &[connection as u32, frame.add(0x20) as u32, r.eax],
                                Conv::Thiscall,
                            );
                            r.edx = word(frame, 0x24);
                            r.eax = word(frame, 0x20);
                            r.game(0x006E_E110, this, &[r.eax, r.edx], Conv::Thiscall);
                        }
                    }
                    r.virtual_edx(sim.neighbors(), 0x3C, &[4, city]);
                }
            }
            node = word(word(frame, 0x18) as *mut u8, 0) as *mut u8;
        }
        r.game(0x0042_1760, frame.add(0x14), &[], Conv::Thiscall);
        let next = sim.next_powered();
        let old = sim.current_powered();
        sim.set_current_powered(next);
        sim.set_next_powered(old);
        r.eax = old as u32;
        r.game(0x006E_BD00, this, &[], Conv::Thiscall);
        if sim.changed() != 0 {
            let server = *message_server();
            r.game(0x0091_33DA, null_mut(), &[0x2C], Conv::Cdecl);
            if r.eax != 0 {
                r.game(0x0091_34D6, r.eax as *mut u8, &[], Conv::Thiscall);
            }
            let message = r.eax as *mut u8;
            r.virtual_edx(message, 4, &[]);
            r.virtual_eax(message, 0x14, &[0x6A07_C2EE]);
            r.virtual_edx(server, 0x10, &[message as u32, 0]);
            r.virtual_eax(message, 8, &[]);
        }
        sim.set_state(3);
    }
}

unsafe fn finish_cycle(sim: IdleSim, frame: *mut u8, stats: *mut u8, r: &mut Registers) {
    unsafe {
        let this = sim.as_ptr();
        for (source, destination) in [(0x40, 0x54), (0x44, 0x58), (0x48, 0x5C), (0x3C, 0x50), (0x4C, 0x60)] {
            put(this, destination, word(this, source));
        }
        r.virtual_edx(stats, 0x30, &[0xAA11_F7CE, sim.production() as u32]);
        r.virtual_eax(stats, 0x30, &[0x0AB2_F224, sim.imports()]);
        r.eax = sim.exports();
        r.virtual_edx(stats, 0x30, &[0x4AB2_F236, r.eax]);
        let slot = vslot(stats, 0x30);
        let value = converted(
            r,
            multiply_float(
                F80::from_i32((sim.consumption() as u32).wrapping_sub(sim.exports()) as i32),
                sim.factor(),
            ),
        );
        r.call(slot, stats, &[0xAA11_F7D2, value as u32], Conv::Thiscall);
        let slot = vslot(stats, 0x30);
        let value = converted(r, multiply_float(F80::from_i32(sim.demand() as i32), sim.factor()));
        r.call(slot, stats, &[0xAA11_F7D5, value as u32], Conv::Thiscall);
        r.eax = (sim.production() as u32).wrapping_sub(sim.consumption() as u32);
        r.virtual_edx(stats, 0x30, &[0xAA11_F7D8, r.eax]);
        r.game(0x0042_1A90, frame.add(0x14), &[], Conv::Thiscall);
        r.eax = frame.add(0x14) as u32;
        r.virtual_edx(sim.neighbors(), 0x78, &[0x4773_FFE1, r.eax]);
        let mut node = word(word(frame, 0x14) as *mut u8, 0) as *mut u8;
        while node as u32 != word(frame, 0x14) {
            put(frame, 0x18, node as u32);
            let deal = word(node, 8) as *mut u8;
            r.virtual_edx(deal, 0x50, &[]);
            let used = r.eax as i32;
            r.virtual_edx(deal, 0x28, &[]);
            if used < r.eax as i32 {
                r.virtual_eax(deal, 0x94, &[0x20, 1]);
            }
            node = word(word(frame, 0x18) as *mut u8, 0) as *mut u8;
        }
        r.game(0x0042_1760, frame.add(0x14), &[], Conv::Thiscall);
        r.virtual_edx(*advisor(), 0x7C, &[]);
        r.edx = sim.department() as u32;
        r.virtual_eax(sim.budget(), 0x8C, &[r.edx]);
        let cost = (u64::from(r.edx) << 32) | u64::from(r.eax);
        if sim.consumption() > 0 {
            let slot = vslot(stats, 0x30);
            r.game(
                0x009E_FEB0,
                null_mut(),
                &[cost as u32, (cost >> 32) as u32, sim.consumption() as u32, 0],
                Conv::Stdcall,
            );
            r.call(slot, stats, &[0xAA11_F7DC, r.eax], Conv::Thiscall);
        }
        if sim.consumers() > 0 {
            let slot = vslot(stats, 0x30);
            // Keep both constants as stored double operands; the division is by an exact signed integer.
            let fraction = F80::from_i32(sim.powered()) / F80::from_i32(sim.consumers());
            let percentage = percentage_complement(fraction);
            let value = converted(r, percentage);
            r.call(slot, stats, &[0xAA11_FB59, value as u32], Conv::Thiscall);
        } else {
            r.virtual_edx(stats, 0x30, &[0xAA11_FB59, 0]);
        }
        for (index, id) in [
            0xAA12_0D5E,
            0xAA12_0D61,
            0xAA12_0D63,
            0xAA12_0D65,
            0xAA12_0D66,
            0xAA12_0D68,
            0xAA12_0D6A,
            0xAA12_0D6B,
            0xAA12_0D6D,
        ]
        .into_iter()
        .enumerate()
        {
            let offset = (sim.ring_index() as u32)
                .wrapping_mul(40)
                .wrapping_add(0x94 + index as u32 * 4);
            r.virtual_edx(stats, 0x30, &[id, word(this.wrapping_add(offset as usize), 0)]);
        }
        for index in 0..10 {
            let offset = (sim.ring_index() as u32)
                .wrapping_mul(40)
                .wrapping_add(0x90 + index as u32 * 4);
            let value = word(this.wrapping_add(offset as usize), 0);
            put(
                this,
                0x270 + index * 4,
                word(this, 0x270 + index * 4).wrapping_add(value),
            );
        }
        if sim.production() > sim.consumption() {
            let difference = sim.production().wrapping_sub(sim.consumption());
            let ratio = divide_signed(difference, sim.production());
            sim.set_deficit(F80::from_i32(ratio).to_f32().to_bits());
        } else {
            sim.set_deficit(*zero_percent());
        }
        sim.set_unpowered(sim.consumers().wrapping_sub(sim.powered()).max(0));
        put(frame, 0x18, 0);
        let h = head(connection_allocator(), frame, r);
        put(frame, 0x18, h as u32);
        r.eax = frame.add(0x18) as u32;
        r.virtual_edx(sim.neighbors(), 0x44, &[4, r.eax]);
        let mut node = word(word(frame, 0x18) as *mut u8, 0) as *mut u8;
        while node as u32 != word(frame, 0x18) {
            put(frame, 0x1C, node as u32);
            r.eax = word(node, 8);
            if r.eax != 0 {
                let connection = r.eax as *mut u8;
                r.virtual_edx(*global_city(), 0x174, &[]);
                let max_x = r.eax.wrapping_sub(1) as i32;
                r.virtual_eax(*global_city(), 0x178, &[]);
                let max_z = r.eax.wrapping_sub(1) as i32;
                r.eax = frame.add(0x10) as u32;
                r.virtual_edx(connection, 0x18, &[r.eax, frame.add(0x14) as u32]);
                put(frame, 0x10, (word(frame, 0x10) as i32).max(0).min(max_x) as u32);
                put(frame, 0x14, (word(frame, 0x14) as i32).max(0).min(max_z) as u32);
            }
            let connection = word(word(frame, 0x1C) as *mut u8, 8) as *mut u8;
            let slot = vslot(connection, 0x38);
            r.virtual_edx(this, 0x94, &[word(frame, 0x10), word(frame, 0x14)]);
            r.call(slot, connection, &[r.eax], Conv::Thiscall);
            node = word(word(frame, 0x1C) as *mut u8, 0) as *mut u8;
        }
        clear_list(connection_allocator(), frame, 0x18, 0x0041_1D70, r);
        sim.set_state(4);
        r.game(0x005F_98C0, null_mut(), &[0xEA41_8D8F, 8, 0, 0, 0, 0], Conv::Cdecl);
    }
}

pub const PORTS: &[Port] = &[fc4p_006ee960::PORT];

#[cfg(test)]
mod tests;
