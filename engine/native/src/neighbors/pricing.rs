// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Deal pricing and limits from the two cities' live commodity figures.

use super::{pair, pointer, virtual_call, word, NeighborsSim};
use crate::abi::{call_raw, vslot, Conv, RawCall};
use crate::port::Port;
use crate::x87::F80;

game_fn! { fn framework() -> u64 = "C" 0x0087_93EC; }
game_fn! { fn release_local(local: *mut *mut u8, edx: u32) -> u64 = "fastcall" 0x0041_1870; }
game_fn! { #[allow(dead_code)] fn destroy_deals(list: *mut *mut u8) -> u64 = "thiscall" 0x0042_1760; }
game_fn! { fn property_float(holder: *mut u8, id: u32, out: *mut f32) -> u64 = "C" 0x005F_D4F0; }
game_fn! { fn property_double(holder: *mut u8, id: u32, out: *mut f64) -> u64 = "C" 0x005F_D530; }
game_fn! { #[allow(dead_code)] fn curve_output(curve: *mut u8, input: f32) -> St0 = "thiscall" 0x005F_4250; }
game_data! { static region_manager: *mut u8 = 0x00B4_3C94; }
game_data! { static deal_allocator: *mut u8 = 0x00B2_0D58; }
game_data! { static zero: f64 = 0x00A8_0990; }
game_data! { static one: f64 = 0x00A8_0AB0; }
game_data! { static one_float: f32 = 0x00A8_1228; }
game_data! { static half: f64 = 0x00A9_2D28; }
game_data! { static epsilon: f64 = 0x00AB_1F88; }

// Memory operands stay memory operands: loading a signaling NaN into another register first can
// change which operand supplies a result's payload.
macro_rules! stored_op {
    ($name:ident, $ty:ty, $instruction:literal) => {
        fn $name(value: F80, operand: $ty) -> F80 {
            let mut result = F80::ZERO;
            unsafe {
                core::arch::asm!(
                    "fld tbyte ptr [{value}]", $instruction, "fstp tbyte ptr [{result}]",
                    value = in(reg) &value, operand = in(reg) &operand, result = in(reg) &mut result,
                    out("st(0)") _,
                );
            }
            result
        }
    };
}
stored_op!(add_double, f64, "fadd qword ptr [{operand}]");
stored_op!(subtract_double, f64, "fsub qword ptr [{operand}]");
stored_op!(subtract_from_double, f64, "fsubr qword ptr [{operand}]");
stored_op!(multiply_double, f64, "fmul qword ptr [{operand}]");
stored_op!(divide_double, f64, "fdiv qword ptr [{operand}]");
stored_op!(add_float, f32, "fadd dword ptr [{operand}]");
stored_op!(divide_float, f32, "fdiv dword ptr [{operand}]");

unsafe fn float_call(object: *mut u8, slot: usize, commodity: u32, eax: u32, edx: u32) -> RawCall {
    unsafe {
        let args = [commodity];
        let mut call = RawCall::new(Conv::Thiscall, vslot(object, slot), object as u32, &args);
        call.eax = eax;
        call.edx = edx;
        call.take_st0 = 1;
        call_raw(&mut call);
        call
    }
}

/// Constructs only the circular sentinel. The third word is left for the game's list operations.
unsafe fn list_header(edx: u32) -> (*mut u8, u32) {
    unsafe {
        let mut allocator = deal_allocator().read();
        let mut edx = edx;
        if allocator.is_null() {
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
            deal_allocator().write(local);
            let result = release_local(&mut local, local as u32);
            edx = (result >> 32) as u32;
            allocator = deal_allocator().read();
        }
        let result = virtual_call(allocator, 0x0C, &[12], word(allocator, 0), edx);
        let header = result as u32 as *mut u8;
        header.cast::<*mut u8>().write_unaligned(header);
        header.add(4).cast::<*mut u8>().write_unaligned(header);
        (header, (result >> 32) as u32)
    }
}

unsafe fn existing(header: *mut u8, mut edx: u32) -> (f64, u32) {
    unsafe {
        let mut amount = zero().read();
        let node = pointer(header, 0);
        if node != header {
            let deal = pointer(node, 8);
            let result = virtual_call(deal, 0x28, &[], deal as u32, word(deal, 0));
            amount = F80::from_i32(result as i32).to_f64();
            edx = (result >> 32) as u32;
        }
        (amount, edx)
    }
}

unsafe fn destroy(list: *mut *mut u8, eax: u32, edx: u32) -> u64 {
    let mut call = RawCall::new(Conv::Thiscall, crate::abi::target(0x0042_1760), list as u32, &[]);
    call.eax = eax;
    call.edx = edx;
    unsafe { call_raw(&mut call) };
    pair(call.eax, call.edx)
}

fn commodity(kind: u32, fallback: u32) -> (u32, Option<u32>, bool) {
    match kind.wrapping_sub(0x4773_FFE0) {
        n @ 0..=5 => (
            n / 2 + 1,
            Some([0xEA46_932C, 0x6A46_9337, 0x8A46_933D][(n / 2) as usize]),
            n & 1 != 0,
        ),
        _ => (fallback, None, true),
    }
}

port! {
    /// Prices a deal using the first matching deal's amount and the shared response curve. Inputs below
    /// the minimum return zero after fetching the current city and amount-scale property. The entry
    /// passes writable argument storage and returns the Rust body's unrounded ST0 and integer registers.
    #[port(address = 0x006D_3250, owner = "neighbors", name = "cSC4NeighborsSimulator::GetDealCost", naked)]
    pub unsafe extern "C" fn fc4p_006d3250() {
        core::arch::naked_asm!(
            "push edx", "sub esp, 12", "lea edx, [esp + 16]", "call {body}",
            "fld tbyte ptr [esp]", "add esp, 16", "ret 16",
            body = sym deal_cost,
        );
    }
}

unsafe extern "fastcall" fn deal_cost(this: *mut u8, stack: *mut u32) -> u64 {
    unsafe {
        let args = stack.add(1);
        let out = stack.cast::<u8>().sub(16).cast::<F80>();
        let sim = NeighborsSim::from_ptr(this);
        let manager = region_manager().read();
        let result = virtual_call(manager, 0x90, &[], word(manager, 0), stack.sub(1).read());
        let current = result as u32 as *mut u8;
        let exemplar = sim.exemplar();
        let initial_other = args.add(3).read() as *mut u8;
        let result = virtual_call(exemplar, 0x1C, &[], result as u32, word(exemplar, 0));
        let holder = result as u32 as *mut u8;
        let mut scale = 1.0f32;
        let mut base = 0.0f32;
        let result = property_float(holder, 0xEA4A_D2FF, &mut scale);
        let cmp = F80::from_f64(args.add(1).cast::<f64>().read_unaligned())
            .compare(F80::from_f64(epsilon().read()));
        if cmp.less() {
            out.write_unaligned(F80::from_f64(zero().read()));
            return pair(
                (result as u32 & 0xFFFF_0000) | u32::from(cmp.status_word()),
                (result >> 32) as u32,
            );
        }
        let kind = args.read();
        let (commodity, property, selling) = commodity(kind, this as u32);
        let mut edx = (result >> 32) as u32;
        if let Some(property) = property {
            edx = (property_float(holder, property, &mut base) >> 32) as u32;
        }
        let (mut header, _) = list_header(edx);
        let find_kind = args.read();
        let result = virtual_call(
            this,
            0x74,
            &[find_kind, args.add(3).read(), &mut header as *mut *mut u8 as u32],
            find_kind,
            word(this, 0),
        );
        let (existing, mut edx) = existing(header, (result >> 32) as u32);
        let (buyer, seller) = if selling {
            (initial_other, current)
        } else {
            (current, args.add(3).read() as *mut u8)
        };

        let call = float_call(buyer, 0xF0, commodity, word(buyer, 0), edx);
        let exported = call.st0.to_f64();
        let call = float_call(buyer, 0xD8, commodity, call.eax, word(buyer, 0));
        let need = add_double(call.st0, exported).to_f64();
        let call = float_call(seller, 0xF0, commodity, word(seller, 0), call.edx);
        let exported = call.st0.to_f64();
        let call = float_call(seller, 0xD8, commodity, call.eax, word(seller, 0));
        let seller_need = subtract_double(add_double(call.st0, exported), existing).to_f64();
        let call = float_call(buyer, 0xE8, commodity, word(buyer, 0), call.edx);
        let produced = call.st0.to_f64();
        let call = float_call(buyer, 0xE0, commodity, call.eax, word(buyer, 0));
        let buyer_have = subtract_double(add_double(call.st0, produced), existing).to_f64();
        let call = float_call(seller, 0xE8, commodity, word(seller, 0), call.edx);
        let produced = call.st0.to_f64();
        let call = float_call(seller, 0xE0, commodity, call.eax, word(seller, 0));
        let mut have = add_double(call.st0, produced).to_f64();
        let floor = add_float(divide_float(F80::from_f32(base), scale), one_float().read()).to_f32();
        let need = if F80::from_f64(need).compare(F80::from_f32(floor)).less() {
            F80::from_f32(floor)
        } else {
            F80::from_f64(need)
        };
        let numerator = subtract_from_double(subtract_double(need, buyer_have), one().read()).to_f64();
        if F80::from_f64(have)
            .compare(F80::from_f64(epsilon().read()))
            .less()
        {
            have = epsilon().read();
        }
        if F80::from_f64(args.add(1).cast::<f64>().read_unaligned())
            .compare(F80::from_f64(epsilon().read()))
            .less()
        {
            args.add(1).cast::<f64>().write_unaligned(epsilon().read());
        }
        let mut maximum_markup = one().read();
        let mut minimum_markup = one().read();
        property_double(holder, 0xCA46_9346, &mut maximum_markup);
        let maximum_factor = add_double(F80::from_f64(maximum_markup), one().read()).to_f64();
        let result = property_double(holder, 0x6A46_9342, &mut minimum_markup);
        let minimum_factor = add_double(F80::from_f64(minimum_markup), one().read()).to_f64();
        let cost_call = float_call(seller, 0xF8, commodity, result as u32, word(seller, 0));
        let minimum = (F80::from_f64(minimum_factor) * cost_call.st0).to_f64();
        let maximum = multiply_double(cost_call.st0, maximum_factor).to_f64();
        let curve = this.add(0x2C);
        let input = divide_double(
            F80::from_f64(numerator),
            args.add(1).cast::<f64>().read_unaligned(),
        )
        .to_f32();
        let curve_args = [input.to_bits()];
        let mut call = RawCall::new(
            Conv::Thiscall,
            crate::abi::target(0x005F_4250),
            curve as u32,
            &curve_args,
        );
        call.eax = cost_call.eax;
        call.edx = cost_call.edx;
        call.take_st0 = 1;
        call_raw(&mut call);
        let first = call.st0.to_f64();
        let input = divide_double(
            subtract_double(
                subtract_double(F80::from_f64(have), args.add(1).cast::<f64>().read_unaligned()),
                seller_need,
            ),
            have,
        )
        .to_f32();
        let curve_args = [input.to_bits()];
        call.args = curve_args.as_ptr();
        call_raw(&mut call);
        let sum = add_double(call.st0, first);
        let spread = subtract_double(F80::from_f64(maximum), minimum);
        let raw = add_double(multiply_double(sum * spread, half().read()), minimum);
        let mut unit = raw.to_f64();
        if raw.compare(F80::from_f64(minimum)).less() {
            unit = minimum;
        }
        let cmp = F80::from_f64(unit).compare(F80::from_f64(maximum));
        if !cmp.less() && !cmp.equal() && !cmp.unordered() {
            unit = maximum;
        }
        edx = call.edx;
        let eax = (call.eax & 0xFFFF_0000) | u32::from(cmp.status_word());
        let result = destroy(&mut header, eax, edx);
        out.write_unaligned(multiply_double(
            F80::from_f64(unit),
            args.add(1).cast::<f64>().read_unaligned(),
        ));
        result
    }
}

port! {
    /// Returns the smaller of the buyer's shortfall and seller's surplus, clamped below at zero.
    /// A null partner returns zero after the current-city query. The entry retains writable argument
    /// storage and returns the Rust body's unrounded ST0 and integer registers.
    #[port(address = 0x006D_3620, owner = "neighbors", name = "cSC4NeighborsSimulator::GetDealMaxAmount", naked)]
    pub unsafe extern "C" fn fc4p_006d3620() {
        core::arch::naked_asm!(
            "push edx", "sub esp, 12", "lea edx, [esp + 16]", "call {body}",
            "fld tbyte ptr [esp]", "add esp, 16", "ret 8",
            body = sym deal_max_amount,
        );
    }
}

unsafe extern "fastcall" fn deal_max_amount(this: *mut u8, stack: *mut u32) -> u64 {
    unsafe {
        let args = stack.add(1);
        let out = stack.cast::<u8>().sub(16).cast::<F80>();
        let manager = region_manager().read();
        let result = virtual_call(manager, 0x90, &[], word(manager, 0), stack.sub(1).read());
        let current = result as u32 as *mut u8;
        let other = args.add(1).read() as *mut u8;
        if other.is_null() {
            out.write_unaligned(F80::from_f64(zero().read()));
            return pair(0, (result >> 32) as u32);
        }
        let result = virtual_call(this, 0x94, &[], other as u32, word(this, 0));
        let exemplar = result as u32 as *mut u8;
        let result = virtual_call(exemplar, 0x18, &[], result as u32, word(exemplar, 0));
        let holder = result as u32 as *mut u8;
        let mut scale = 1.0f32;
        let mut base = 0.0f32;
        let result = property_float(holder, 0xEA4A_D2FF, &mut scale);
        let kind = args.read();
        let selling_other = args.add(1).read() as *mut u8;
        let (commodity, property, selling) = commodity(kind, 1);
        let mut edx = (result >> 32) as u32;
        if let Some(property) = property {
            edx = (property_float(holder, property, &mut base) >> 32) as u32;
        }
        let (mut header, _) = list_header(edx);
        let other = args.add(1).read() as *mut u8;
        let find_kind = args.read();
        let result = virtual_call(
            this,
            0x74,
            &[find_kind, other as u32, &mut header as *mut *mut u8 as u32],
            &mut header as *mut *mut u8 as u32,
            word(this, 0),
        );
        let (existing, edx) = existing(header, (result >> 32) as u32);
        let (buyer, seller) = if selling {
            (selling_other, current)
        } else {
            (current, other)
        };
        let call = float_call(buyer, 0xF0, commodity, word(buyer, 0), edx);
        let exported = call.st0.to_f64();
        let call = float_call(buyer, 0xD8, commodity, call.eax, word(buyer, 0));
        let need = add_double(call.st0, exported);
        let floor = add_float(divide_float(F80::from_f32(base), scale), one_float().read());
        let floor_stored = floor.to_f32();
        let need = if need.compare(floor).less() {
            F80::from_f32(floor_stored)
        } else {
            need
        };
        let target = (F80::from_f32(scale) * need).to_f64();
        let call = float_call(buyer, 0xE8, commodity, word(buyer, 0), call.edx);
        let produced = call.st0.to_f64();
        let call = float_call(buyer, 0xE0, commodity, call.eax, word(buyer, 0));
        let shortfall =
            subtract_from_double(subtract_double(add_double(call.st0, produced), existing), target).to_f64();
        let call = float_call(seller, 0xE8, commodity, word(seller, 0), call.edx);
        let produced = call.st0.to_f64();
        let call = float_call(seller, 0xE0, commodity, call.eax, word(seller, 0));
        let supply = add_double(call.st0, produced).to_f64();
        let call = float_call(seller, 0xF0, commodity, word(seller, 0), call.edx);
        let exported = call.st0.to_f64();
        let call = float_call(seller, 0xD8, commodity, call.eax, word(seller, 0));
        let surplus = subtract_from_double(subtract_double(add_double(call.st0, exported), existing), supply);
        let stored = surplus.to_f64();
        let take_surplus = surplus.compare(F80::from_f64(shortfall)).less();
        let amount = if take_surplus { stored } else { shortfall };
        let cmp = F80::from_f64(amount).compare(F80::from_f64(zero().read()));
        let amount = if cmp.less() { zero().read() } else { amount };
        // The selected local's address supplies EAX's upper half before the final status-word read.
        let original_locals = ((stack as u32).wrapping_sub(4) & !7).wrapping_sub(0x40);
        let selected = original_locals.wrapping_add(if take_surplus { 0x30 } else { 0x38 });
        let result = destroy(
            &mut header,
            (selected & 0xFFFF_0000) | u32::from(cmp.status_word()),
            call.edx,
        );
        out.write_unaligned(F80::from_f64(amount));
        result
    }
}

pub const PORTS: &[Port] = &[fc4p_006d3250::PORT, fc4p_006d3620::PORT];

#[cfg(test)]
mod tests;
