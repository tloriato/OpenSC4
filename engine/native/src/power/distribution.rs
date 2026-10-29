// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Select one wired network and distribute its plants and neighbor deals across eight-cell blocks.

use super::changes::{allocator, new_head, pointer, selected, ChangeSim};
use super::*;

layout! {
    /// Working totals, source rows, and bitmap pointers used for one network.
    struct DistributionSim {
        0x18 => city / set_city: *mut u8,
        0x30 => neighbors / set_neighbors: *mut u8,
        0x3C => production / set_production: u32,
        0x40 => consumption / set_consumption: u32,
        0x44 => imports / set_imports: u32,
        0x48 => exports / set_exports: u32,
        0x4C => demand / set_demand: u32,
        0x6C => powered_count / set_powered_count: u32,
        0x84 => rows / set_rows: *mut u8,
        0x298 => plants / set_plants: *mut u8,
        0x2AC => factor / set_factor: f32,
        0x2C4 => selected / set_selected: *mut u32,
        0x2C8 => wire / set_wire: *mut u32,
        0x2D0 => next_powered / set_next_powered: *mut u32,
    }
}

game_fn! { fn bitmap_clear(bits: *mut u32, width: u32) = "C" 0x0060_3E00; }
game_fn! { fn grow(out: *mut u32, source: *mut u32, width: u32, limit: u32, low: *mut u32, high: *mut u32) = "C" 0x0060_4570; }
game_fn! { fn rect_any(bits: *mut u32, width: u32, rect: *mut Rect) -> u32 = "C" 0x0060_4350; }
game_fn! { fn rect_or(out: *mut u32, source: *mut u32, width: u32, rect: *mut Rect) = "C" 0x0060_4440; }
game_fn! { fn produced(this: *mut u8, occupant: *mut u8) -> u32 = "thiscall" 0x006E_B1C0; }
game_fn! { fn find_connection(this: *mut u8, city: u32, visit: u32, power: u32) -> u32 = "thiscall" 0x006E_DB40; }
game_fn! { fn mark_plants(this: *mut u8, used: u32, divisor: u32) = "thiscall" 0x006E_BC10; }
game_fn! { fn iterator_reset(this: *mut u8, mode: u32) = "thiscall" 0x0049_D1C0; }
game_fn! { fn iterator_next(this: *mut u8, x: *mut u32, z: *mut u32) -> u32 = "thiscall" 0x0049_CB10; }
game_fn! { fn clear_deals(this: *mut u32) = "thiscall" 0x0042_1660; }
game_fn! { fn free(p: *mut u8) = "C" 0x0090_CF63; }
game_data! { static deal_allocator: *mut u8 = 0x00B2_0D58; }
game_data! { static unsigned_bias: f32 = 0x00A8_0AA8; }
game_data! { static negative_production_ratio: f32 = 0x00AB_2888; }
stored_op!(divide_float, "fdiv dword ptr [{operand}]");

unsafe fn get(frame: *mut u8, offset: usize) -> u32 {
    unsafe { frame.add(offset).cast::<u32>().read_unaligned() }
}
unsafe fn put(frame: *mut u8, offset: usize, value: u32) {
    unsafe {
        frame.add(offset).cast::<u32>().write_unaligned(value);
    }
}
unsafe fn unsigned_float(value: u32) -> F80 {
    let wide = F80::from_i32(value as i32);
    if (value as i32) < 0 {
        unsafe { add_float(wide, *unsigned_bias()) }
    } else {
        wide
    }
}

port! {
    /// Grow a selected network, visit plants, reserve exports, then power complete blocks in iterator order.
    /// Shortfall consumes the remaining availability. Totals and signed consumption cells wrap as words.
    /// Plant usage divides by the signed greater of network production and consumption after imports.
    #[port(address = 0x006E_E110, owner = "power", name = "cSC4PowerSimulator::UpdatePowerGrid", naked)]
    pub unsafe extern "C" fn fc4p_006ee110() {
        core::arch::naked_asm!(
            "sub esp, 0x94", "push ebx", "push ebp", "push esi", "push edi",
            "mov edx, esp", "call {body}",
            "pop edi", "pop esi", "pop ebp", "pop ebx", "add esp, 0x94", "ret 8",
            body = sym distribute,
        );
    }
}

unsafe extern "fastcall" fn distribute(this: *mut u8, frame: *mut u8) -> u64 {
    unsafe {
        let sim = DistributionSim::from_ptr(this);
        let width = vcall!(sim.city(), 0x174 => fn() -> u32);
        bitmap_clear(sim.selected(), width);
        let width = vcall!(sim.city(), 0x174 => fn() -> i32);
        let x = get(frame, 0xA8);
        let z = get(frame, 0xAC);
        let index = (width / 32).wrapping_mul(z as i32).wrapping_add((x >> 5) as i32);
        let word = sim.selected().wrapping_offset(index as isize);
        word.write_unaligned(word.read_unaligned() | 1u32.wrapping_shl(x));
        put(frame, 0x40, z);
        put(frame, 0x2C, z.wrapping_add(1));
        let width = vcall!(sim.city(), 0x174 => fn() -> u32);
        grow(
            sim.selected(),
            sim.wire(),
            width,
            600,
            frame.add(0x40).cast(),
            frame.add(0x2C).cast(),
        );
        if (get(frame, 0x40) as i32) > get(frame, 0x2C) as i32 {
            put(frame, 0x40, z);
            put(frame, 0x2C, z.wrapping_add(1));
        }
        let height = vcall!(sim.city(), 0x178 => fn() -> i32);
        if get(frame, 0x2C) as i32 >= height {
            put(frame, 0x2C, vcall!(sim.city(), 0x178 => fn() -> u32).wrapping_sub(1));
        }
        let head = sim.plants();
        let mut node = pointer(head, 0);
        let mut production = 0u32;
        put(frame, 0x18, 0);
        while node != head {
            if node.add(8).cast::<u32>().read_unaligned() == 0 {
                let occupant = pointer(node, 12);
                vcall!(occupant, 0x28 => fn(*mut f32, *mut f32), frame.add(0x98).cast(), frame.add(0x48).cast());
                vcall!(this, 0x14 => fn(u32, u32, *mut u32, *mut u32),
                    get(frame, 0x98), get(frame, 0xA0), frame.add(0x20).cast(), frame.add(0x30).cast());
                vcall!(this, 0x14 => fn(u32, u32, *mut u32, *mut u32),
                    get(frame, 0x48), get(frame, 0x50), frame.add(0x3C).cast(), frame.add(0x18).cast());
                let rect = frame.add(0x88).cast::<Rect>();
                rect.write_unaligned(Rect {
                    x0: get(frame, 0x20),
                    z0: get(frame, 0x30),
                    x1: get(frame, 0x3C),
                    z1: get(frame, 0x18),
                });
                let width = vcall!(sim.city(), 0x174 => fn() -> u32);
                if rect_any(sim.selected(), width, rect) as u8 != 0 {
                    production = production.wrapping_add(produced(this, occupant));
                    node.add(8).cast::<u32>().write_unaligned(1);
                }
            }
            node = pointer(node, 0);
        }
        put(frame, 0x18, production);
        sim.set_production(sim.production().wrapping_add(production));
        put(frame, 0x14, 0);
        let buy = frame.add(0x24).cast::<u32>();
        buy.write(0);
        buy.write(new_head(deal_allocator(), frame.add(0x10).cast()) as u32);
        vcall!(sim.neighbors(), 0x78 => fn(u32, *mut u32), 0x4773_FFE0, buy);
        let mut node = pointer(buy.read() as *mut u8, 0);
        while node != buy.read() as *mut u8 {
            let deal = pointer(node, 8);
            let city = vcall!(deal, 0x1C => fn() -> u32);
            put(frame, 0x20, city);
            let used = vcall!(deal, 0x50 => fn() -> i32);
            let amount = vcall!(deal, 0x28 => fn() -> i32);
            if used < amount && find_connection(this, city, 1, 0) as u8 != 0 {
                let used = vcall!(deal, 0x50 => fn() -> u32);
                let amount = vcall!(deal, 0x28 => fn() -> u32);
                put(frame, 0x14, get(frame, 0x14).wrapping_add(amount.wrapping_sub(used)));
            }
            node = pointer(node, 0);
        }
        let extra = multiply_float(unsigned_float(production), *negative_production_ratio()).ftol() as u32;
        let total = get(frame, 0x14).wrapping_sub(extra);
        put(frame, 0x20, total);
        put(frame, 0x28, total);
        put(
            frame,
            0x14,
            divide_float(unsigned_float(total), sim.factor()).ftol() as u32,
        );
        let sell = frame.add(0x34).cast::<u32>();
        sell.write(0);
        sell.write(new_head(deal_allocator(), frame.add(0x10).cast()) as u32);
        vcall!(sim.neighbors(), 0x78 => fn(u32, *mut u32), 0x4773_FFE1, sell);
        let mut node = pointer(sell.read() as *mut u8, 0);
        while node != sell.read() as *mut u8 {
            let deal = pointer(node, 8);
            let city = vcall!(deal, 0x1C => fn() -> u32);
            let used = vcall!(deal, 0x50 => fn() -> i32);
            let amount = vcall!(deal, 0x28 => fn() -> i32);
            if used < amount {
                let available = get(frame, 0x14);
                let flag = (amount as u32 & !255) | u32::from(available != 0);
                if find_connection(this, city, 0, flag) as u8 != 0 {
                    let used = vcall!(deal, 0x50 => fn() -> u32);
                    let amount = vcall!(deal, 0x28 => fn() -> u32);
                    let amount = (amount.wrapping_sub(used) as i32).min(available as i32);
                    if amount > 0 {
                        sim.set_exports(sim.exports().wrapping_add(amount as u32));
                        put(frame, 0x14, get(frame, 0x14).wrapping_sub(amount as u32));
                        add_deal_used(deal, amount as u32);
                    } else {
                        vcall!(deal, 0x94 => fn(u32, u32), 0x20, 1);
                    }
                }
            }
            node = pointer(node, 0);
        }
        let width = vcall!(sim.city(), 0x174 => fn() -> u32);
        let iterator = frame.add(0x58);
        iterator.write(1);
        iterator.add(1).write(0);
        for offset in (0x6C..=0x80).step_by(4) {
            put(frame, offset, 0);
        }
        put(frame, 0x84, 1);
        put(frame, 0x5C, 0);
        let first = get(frame, 0x40) >> 3;
        put(frame, 0x60, first);
        put(frame, 0x64, ((width >> 3).wrapping_sub(1) as i32).max(0) as u32);
        put(frame, 0x68, (get(frame, 0x2C) >> 3).max(first));
        iterator.add(2).write(1);
        iterator_reset(iterator, 1);
        frame.add(0x1F).write(0);
        while iterator_next(iterator, frame.add(0x20).cast(), frame.add(0x30).cast()) as u8 != 0 {
            let x = get(frame, 0x20).wrapping_mul(8);
            let z = get(frame, 0x30).wrapping_mul(8);
            let rect = frame.add(0x48).cast::<Rect>();
            rect.write_unaligned(Rect {
                x0: x,
                z0: z,
                x1: x.wrapping_add(7),
                z1: z.wrapping_add(7),
            });
            let width = vcall!(sim.city(), 0x174 => fn() -> u32);
            if rect_any(sim.selected(), width, rect) as u8 != 0 {
                let mut need = 0u32;
                put(frame, 0x10, 0);
                put(frame, 0x38, 0);
                let mut x = get(frame, 0x48) as i32;
                while x <= get(frame, 0x50) as i32 {
                    let mut z = get(frame, 0x4C) as i32;
                    while z <= get(frame, 0x54) as i32 {
                        if selected(ChangeSim::from_ptr(this), x as u32, z as u32) {
                            let row = sim
                                .rows()
                                .wrapping_offset(x.wrapping_mul(12) as isize)
                                .cast::<*mut i16>()
                                .read_unaligned();
                            let value = row.wrapping_offset(z as isize).read_unaligned();
                            if value > 0 {
                                need = need.wrapping_add(value as u32);
                                put(frame, 0x10, need);
                                put(frame, 0x38, get(frame, 0x38).wrapping_add(1));
                            }
                        }
                        z = z.wrapping_add(1);
                    }
                    x = x.wrapping_add(1);
                }
                let available = get(frame, 0x14);
                sim.set_demand(sim.demand().wrapping_add(need));
                if available != 0 && need <= available {
                    let width = vcall!(sim.city(), 0x174 => fn() -> u32);
                    rect_or(sim.next_powered(), sim.selected(), width, rect);
                    sim.set_powered_count(sim.powered_count().wrapping_add(get(frame, 0x38)));
                    put(frame, 0x14, available.wrapping_sub(need));
                } else {
                    frame.add(0x1F).write(1);
                }
            }
        }
        if frame.add(0x1F).read() != 0 {
            put(frame, 0x14, 0);
        }
        let remaining = multiply_float(unsigned_float(get(frame, 0x14)), sim.factor()).ftol() as u32;
        let mut used = get(frame, 0x28).wrapping_sub(remaining);
        sim.set_consumption(sim.consumption().wrapping_add(used));
        put(frame, 0x14, used);
        vcall!(sim.neighbors(), 0x78 => fn(u32, *mut u32), 0x4773_FFE0, buy);
        let mut node = pointer(buy.read() as *mut u8, 0);
        while node != buy.read() as *mut u8 {
            let deal = pointer(node, 8);
            let city = vcall!(deal, 0x1C => fn() -> u32);
            let old = vcall!(deal, 0x50 => fn() -> i32);
            let amount = vcall!(deal, 0x28 => fn() -> i32);
            if old < amount && find_connection(this, city, 0, 0) as u8 != 0 {
                let old = vcall!(deal, 0x50 => fn() -> u32);
                let amount = vcall!(deal, 0x28 => fn() -> u32);
                let amount = (amount.wrapping_sub(old) as i32).min(used as i32) as u32;
                sim.set_imports(sim.imports().wrapping_add(amount));
                used = used.wrapping_sub(amount);
                put(frame, 0x14, used);
                add_deal_used(deal, amount);
            }
            node = pointer(node, 0);
        }
        mark_plants(this, used, (production as i32).max(used as i32) as u32);
        let storage = get(frame, 0x70) as *mut u8;
        if !storage.is_null() {
            free(storage);
        }
        clear_deals(sell);
        let head = sell.read();
        let alloc = allocator(deal_allocator(), frame.add(0x18).cast());
        vcall!(alloc, 0x10 => fn(u32), head);
        clear_deals(buy);
        let head = buy.read();
        let alloc = allocator(deal_allocator(), frame.add(0x18).cast());
        vcall!(alloc, 0x10 => fn(u32) -> u64, head)
    }
}

unsafe fn add_deal_used(deal: *mut u8, amount: u32) {
    unsafe {
        let table = deal.cast::<*const usize>().read_unaligned();
        let old = vcall!(deal, 0x50 => fn() -> u32);
        let set = core::mem::transmute::<usize, unsafe extern "thiscall" fn(*mut u8, u32)>(table.add(0x54 / 4).read());
        set(deal, old.wrapping_add(amount));
    }
}

pub const PORTS: &[Port] = &[fc4p_006ee110::PORT];

#[cfg(test)]
mod tests;
