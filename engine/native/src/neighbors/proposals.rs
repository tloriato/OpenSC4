// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Automatic deal proposals in city order, buy before sell for each commodity.

use super::{pair, pointer, virtual_call, word, NeighborsSim};
use crate::abi::{call_raw, vslot, Conv, RawCall};
use crate::port::Port;
use crate::x87::F80;

game_fn! { fn framework() -> u64 = "C" 0x0087_93EC; }
game_fn! { fn release_local(local: *mut *mut u8, edx: u32) -> u64 = "fastcall" 0x0041_1870; }
game_fn! { fn property_float(holder: *mut u8, id: u32, output: *mut f32) -> u64 = "C" 0x005F_D4F0; }
game_fn! { fn clear_deals(list: *mut *mut u8, edx: u32) -> u64 = "fastcall" 0x0042_1660; }
game_fn! { fn destroy_cities(list: *mut *mut u8, edx: u32) -> u64 = "fastcall" 0x0041_1FE0; }
game_fn! { fn erase_city(list: *mut *mut u8, edx: *mut *mut u8, output: *mut *mut u8, node: *mut u8) -> u64 = "fastcall" 0x004A_4A50; }
game_fn! { #[allow(dead_code)] fn create_proposition(this: *mut u8, kind: u32, flags: u32, city: *mut u8, output: *mut u8, add: u32) -> u64 = "thiscall" 0x006D_0520; }
game_data! { static current_city: *mut u8 = 0x00B4_3CEC; }
game_data! { static region_manager: *mut u8 = 0x00B4_3C94; }
game_data! { static city_allocator: *mut u8 = 0x00B2_07E0; }
game_data! { static deal_allocator: *mut u8 = 0x00B2_0D58; }
game_data! { static one_float: f32 = 0x00A8_1228; }

unsafe fn allocator(cache: *mut *mut u8, edx: u32) -> (*mut u8, u32) {
    unsafe {
        let mut service = cache.read();
        let mut edx = edx;
        if service.is_null() {
            let mut local = core::ptr::null_mut();
            let result = framework();
            let framework = result as u32 as *mut u8;
            if !framework.is_null() {
                virtual_call(
                    framework,
                    0x14,
                    &[0x3AE4_BEA3, 0x3AE4_BEAB, &mut local as *mut *mut u8 as u32],
                    framework as u32,
                    word(framework, 0),
                );
            }
            cache.write(local);
            let result = release_local(&mut local, local as u32);
            edx = (result >> 32) as u32;
            service = cache.read();
        }
        (service, edx)
    }
}

unsafe fn new_list(cache: *mut *mut u8, edx: u32) -> (*mut u8, u32) {
    unsafe {
        let (service, edx) = allocator(cache, edx);
        let result = virtual_call(service, 0x0C, &[12], word(service, 0), edx);
        let header = result as u32 as *mut u8;
        header.cast::<*mut u8>().write_unaligned(header);
        header.add(4).cast::<*mut u8>().write_unaligned(header);
        (header, (result >> 32) as u32)
    }
}

unsafe fn delete_deal_list(header: &mut *mut u8, edx: u32) -> u64 {
    unsafe {
        let result = clear_deals(header, edx);
        let node = *header;
        let (service, _) = allocator(deal_allocator(), (result >> 32) as u32);
        virtual_call(service, 0x10, &[node as u32], word(service, 0), node as u32)
    }
}

unsafe fn floating(object: *mut u8, slot: usize, args: &[u32], eax: u32, edx: u32) -> RawCall {
    unsafe {
        let mut call = RawCall::new(Conv::Thiscall, vslot(object, slot), object as u32, args);
        call.eax = eax;
        call.edx = edx;
        call.take_st0 = 1;
        call_raw(&mut call);
        call
    }
}

fn multiply_float_by_double(value: f32, operand: f64) -> F80 {
    let mut result = F80::ZERO;
    unsafe {
        core::arch::asm!(
            "fld dword ptr [{value}]", "fmul qword ptr [{operand}]", "fstp tbyte ptr [{result}]",
            value=in(reg)&value, operand=in(reg)&operand, result=in(reg)&mut result, out("st(0)") _,
        );
    }
    result
}

/// Every rejection leaves the other direction eligible. Existing deals are checked only after
/// feasibility, and budgets are fetched only for feasible directions without an existing deal.
#[allow(clippy::too_many_arguments)]
unsafe fn propose_direction(
    this: *mut u8,
    stack: *mut u32,
    city: *mut u8,
    buyer: *mut u8,
    kind: u32,
    base: f32,
    minimum_budget: f32,
    ratio: f32,
    list: &mut *mut u8,
) -> u32 {
    unsafe {
        let result = virtual_call(this, 0x5C, &[kind, city as u32], kind, word(this, 0));
        let mut edx = (result >> 32) as u32;
        if result as u32 != 0 {
            let result = virtual_call(
                this,
                0x74,
                &[kind, city as u32, list as *mut *mut u8 as u32],
                list as *mut *mut u8 as u32,
                word(this, 0),
            );
            edx = (result >> 32) as u32;
            if pointer(*list, 0) == *list {
                let budget = floating(buyer, 0xC8, &[], *list as u32, word(buyer, 0));
                let funds = budget.st0.to_f64();
                let maximum = floating(this, 0x90, &[kind, city as u32], word(this, 0), budget.edx);
                let maximum_stored = maximum.st0.to_f64();
                let base_wide = F80::from_f32(base);
                let amount = if base_wide.compare(F80::from_f64(maximum_stored)).less() {
                    base_wide.to_f64()
                } else {
                    maximum_stored
                };
                edx = maximum.edx;
                let cmp = F80::from_f64(amount).compare(F80::from_f32(base));
                if !cmp.less() && !cmp.unordered() {
                    let bits = amount.to_bits();
                    let cost = floating(
                        this,
                        0x8C,
                        &[kind, bits as u32, (bits >> 32) as u32, city as u32],
                        kind,
                        word(this, 0),
                    );
                    let price = cost.st0.to_f64();
                    edx = cost.edx;
                    if F80::from_f32(minimum_budget).compare(F80::from_f64(funds)).less() {
                        let cmp = F80::from_f64(funds).compare(F80::from_f64(price));
                        if !cmp.less()
                            && !cmp.equal()
                            && !cmp.unordered()
                            && multiply_float_by_double(ratio, price)
                                .compare(F80::from_f64(funds))
                                .less()
                        {
                            let output = stack.read();
                            let args = [kind, 0, city as u32, output, stack.add(1).read()];
                            let mut call = RawCall::new(
                                Conv::Thiscall,
                                crate::abi::target(0x006D_0520),
                                this as u32,
                                &args,
                            );
                            call.eax = kind;
                            call.edx = output;
                            call_raw(&mut call);
                            edx = call.edx;
                        }
                    }
                }
            }
        }
        // Cleanup receives the last call's high word, including when a comparison rejected it.
        edx
    }
}

unsafe fn publish(this: *mut u8, edx: u32) -> u32 {
    unsafe {
        let sim = NeighborsSim::from_ptr(this);
        sim.set_deals_dirty(1);
        let (mut header, _) = new_list(city_allocator(), edx);
        let result = virtual_call(
            this,
            0x50,
            &[&mut header as *mut *mut u8 as u32, 0],
            &mut header as *mut *mut u8 as u32,
            word(this, 0),
        );
        let manager = region_manager().read();
        let result = virtual_call(manager, 0x90, &[], result as u32, word(manager, 0));
        let current = result as u32 as *mut u8;
        let mut edx = (result >> 32) as u32;
        while pointer(header, 0) != header {
            let city = pointer(pointer(header, 4), 8);
            if !city.is_null() {
                let result = virtual_call(
                    city,
                    0x114,
                    &[this.add(0x20) as u32],
                    this.add(0x20) as u32,
                    word(city, 0),
                );
                let result = virtual_call(city, 0x11C, &[], result as u32, word(city, 0));
                edx = (result >> 32) as u32;
            }
            let node = pointer(header, 4);
            let previous = pointer(node, 4);
            let next = pointer(node, 0);
            previous.cast::<*mut u8>().write_unaligned(next);
            next.add(4).cast::<*mut u8>().write_unaligned(previous);
            let object = pointer(node, 8);
            if !object.is_null() {
                let result = virtual_call(object, 8, &[], word(object, 0), edx);
                edx = (result >> 32) as u32;
            }
            let (service, edx_after_cache) = allocator(city_allocator(), edx);
            let result = virtual_call(service, 0x10, &[node as u32], word(service, 0), edx_after_cache);
            edx = (result >> 32) as u32;
        }
        let result = virtual_call(
            current,
            0x114,
            &[this.add(0x20) as u32],
            this.add(0x20) as u32,
            word(current, 0),
        );
        let result = virtual_call(current, 0x11C, &[], result as u32, word(current, 0));
        sim.set_deals_dirty(0);
        sim.set_stats_dirty(0);
        let result = destroy_cities(&mut header, (result >> 32) as u32);
        (result >> 32) as u32
    }
}

port! {
    /// Clears the caller's output, tests both directions for power, water and garbage for each
    /// adjacent city, and creates only affordable full-base proposals. Adding nonempty output
    /// publishes deals and commodity changes to adjacent cities and the current regional city.
    /// An uninitialized simulator leaves the output intact. AL is one; other return bytes survive.
    #[port(address = 0x006D_21A0, owner = "neighbors", name = "cSC4NeighborsSimulator::ProposeDeals", naked)]
    pub unsafe extern "C" fn fc4p_006d21a0() {
        core::arch::naked_asm!("push eax", "push edx", "lea edx, [esp + 12]", "call {body}", "ret 8", body=sym propose_deals);
    }
}

unsafe extern "fastcall" fn propose_deals(
    this: *mut u8,
    stack: *mut u32,
    entry_edx: u32,
    entry_eax: u32,
) -> u64 {
    unsafe {
        let sim = NeighborsSim::from_ptr(this);
        if sim.initialized() == 0 {
            return pair((entry_eax & !255) | 1, entry_edx);
        }
        let result = virtual_call(
            this,
            0x88,
            &[current_city().read() as u32],
            word(this, 0),
            entry_edx,
        );
        let result = clear_deals(stack.read() as *mut *mut u8, (result >> 32) as u32);
        let manager = region_manager().read();
        let result = virtual_call(manager, 0x90, &[], result as u32, word(manager, 0));
        let current = result as u32 as *mut u8;
        let result = virtual_call(this, 0x94, &[], 0, word(this, 0));
        let exemplar = result as u32 as *mut u8;
        let result = virtual_call(exemplar, 0x18, &[], result as u32, word(exemplar, 0));
        let holder = result as u32 as *mut u8;
        let mut bases = [0.0f32; 3];
        for (id, base) in [0xEA46_932C, 0x6A46_9337, 0x8A46_933D]
            .into_iter()
            .zip(&mut bases)
        {
            property_float(holder, id, base);
        }
        let mut maximum_markup = 1.0f32;
        property_float(holder, 0xCA46_9346, &mut maximum_markup);
        let mut sum = F80::ZERO;
        let one = one_float().read();
        core::arch::asm!("fld dword ptr [{value}]", "fadd dword ptr [{one}]", "fstp tbyte ptr [{sum}]",
            value=in(reg)&maximum_markup, one=in(reg)&one, sum=in(reg)&mut sum, out("st(0)") _);
        let _maximum_multiplier = sum.to_f32();
        let mut minimum_budget = 0.0f32;
        property_float(holder, 0xEA4A_E9EF, &mut minimum_budget);
        let mut ratio = 1.0f32;
        let result = property_float(holder, 0xCA46_931F, &mut ratio);
        let (mut header, _) = new_list(city_allocator(), (result >> 32) as u32);
        let result = virtual_call(
            this,
            0x50,
            &[&mut header as *mut *mut u8 as u32, 0],
            &mut header as *mut *mut u8 as u32,
            word(this, 0),
        );
        let mut edx = (result >> 32) as u32;
        while pointer(header, 0) != header {
            let city = pointer(pointer(header, 0), 8);
            for (index, base) in bases.into_iter().enumerate() {
                let kind = 0x4773_FFE0 + index as u32 * 2;
                let (mut buys, next_edx) = new_list(deal_allocator(), edx);
                let (mut sells, _) = new_list(deal_allocator(), next_edx);
                propose_direction(
                    this,
                    stack,
                    city,
                    current,
                    kind,
                    base,
                    minimum_budget,
                    ratio,
                    &mut buys,
                );
                edx = propose_direction(
                    this,
                    stack,
                    city,
                    city,
                    kind + 1,
                    base,
                    minimum_budget,
                    ratio,
                    &mut sells,
                );
                let result = delete_deal_list(&mut sells, edx);
                let result = delete_deal_list(&mut buys, (result >> 32) as u32);
                edx = (result >> 32) as u32;
            }
            let mut iterator = core::ptr::null_mut();
            let node = pointer(header, 0);
            let result = erase_city(&mut header, &mut iterator, &mut iterator, node);
            edx = (result >> 32) as u32;
        }
        let output = stack.read() as *mut u8;
        let output_header = pointer(output, 0);
        if pointer(output_header, 0) != output_header && stack.add(1).read() as u8 != 0 {
            edx = publish(this, edx);
        }
        let result = destroy_cities(&mut header, edx);
        (result & !255) | 1
    }
}

pub const PORTS: &[Port] = &[fc4p_006d21a0::PORT];

#[cfg(test)]
mod tests;
