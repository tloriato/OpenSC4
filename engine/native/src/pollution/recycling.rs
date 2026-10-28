// SPDX-License-Identifier: GPL-3.0-or-later
//! Recycling-center ordering and population-weighted effect.

use super::*;

game_fn! { fn application() -> *mut u8 = "C" 0x0087_93EC; }
game_fn! { fn release_service(reference: *mut *mut u8) = "thiscall" 0x0041_1870; }
game_fn! { fn assign_list(list: *mut *mut u8, source: *mut *mut u8) = "thiscall" 0x006E_5660; }
game_fn! { fn destroy_list(list: *mut *mut u8) = "thiscall" 0x0041_3FE0; }
game_fn! { fn property_float(holder: *mut u8, id: u32, output: *mut f32) -> u32 = "C" 0x005F_D4F0; }
game_data! { static allocator: *mut u8 = 0x00B2_07E4; }
game_data! { static default_effect: u32 = 0x00B4_9BC4; }

layout! {
    /// Fields used by the population-weighted recycling calculation.
    struct RecyclingView {
        0x2C => residential / set_residential: *mut u8,
        0x74 => recycling / set_recycling: *mut u8,
        0xFC => count / set_count: u32,
        0x134 => effect / set_effect: f32,
    }
}

macro_rules! stored_op {
    ($name:ident, $instruction:literal) => {
        fn $name(value: F80, operand: f32) -> F80 {
            let input = value.to_bytes();
            let mut output = [0u8; 10];
            // SAFETY: both operands are live; one x87 register is pushed and popped.
            unsafe {
                core::arch::asm!("fld tbyte ptr [{input}]", $instruction, "fstp tbyte ptr [{output}]",
                    input = in(reg) &input, operand = in(reg) &operand,
                    output = in(reg) &mut output, out("st(0)") _);
            }
            F80::from_bytes(output)
        }
    };
}
stored_op!(add_float, "fadd dword ptr [{operand}]");
stored_op!(multiply_float, "fmul dword ptr [{operand}]");
stored_op!(divide_float, "fdiv dword ptr [{operand}]");

unsafe fn allocate() -> *mut u8 {
    // SAFETY: the original allocator service and its acquisition interfaces.
    unsafe {
        if (*allocator()).is_null() {
            let mut service = core::ptr::null_mut();
            let app = application();
            if !app.is_null() {
                vcall!(app, 0x14 => fn(u32,u32,*mut *mut u8), 0x3AE4_BEA3,0x3AE4_BEAB,&mut service);
            }
            *allocator() = service;
            release_service(&mut service);
        }
        vcall!(*allocator(), 0x0C => fn(u32) -> *mut u8, 12)
    }
}

port! {
    /// Sorts recycling centers by descending current capacity through a temporary game-allocated list.
    /// Equal capacities insert before existing entries. Counts full served populations until the city's
    /// population is reached, then divides their weighted capacities by the larger served/city population.
    #[port(address = 0x006E_6DA0, owner = "pollution", name = "cSC4PollutionSimulator::CalculateRecyclingCenterEffect")]
    pub unsafe extern "thiscall" fn fc4p_006e6da0(this: *mut u8) {
        // SAFETY: the original simulator, list nodes, allocator, properties and residential service.
        unsafe {
            let sim = RecyclingView::from_ptr(this);
            let population = F80::from_i32(vcall!(sim.residential(), 0x40 => fn() -> i32)).to_f32();
            let mut capacity = 0.0;
            let sentinel = allocate();
            ListNode::from_ptr(sentinel).set_next(sentinel);
            ListNode::from_ptr(sentinel).set_previous(sentinel);
            let mut head = sentinel;
            if sim.count() == 0 {
                write_unaligned(this.add(0x134).cast::<u32>(), *default_effect());
                destroy_list(&mut head);
                return;
            }
            let end = sim.recycling();
            let mut node = ListNode::from_ptr(end).next();
            while node != end {
                if vcall!(this, 0xDC => fn(*mut u8,*mut f32) -> u32, ListNode::from_ptr(node).occupant(), &mut capacity) as u8 != 0 {
                    let mut position = ListNode::from_ptr(sentinel).next();
                    while position != sentinel {
                        let mut other = 0.0;
                        let success = vcall!(this, 0xDC => fn(*mut u8,*mut f32) -> u32, ListNode::from_ptr(position).occupant(), &mut other);
                        if success as u8 != 0 {
                            let comparison = F80::from_f32(other).compare(F80::from_f32(capacity));
                            if comparison.less() || comparison.equal() { break; }
                        }
                        position = ListNode::from_ptr(position).next();
                    }
                    let inserted = allocate();
                    let value = inserted.wrapping_add(8);
                    if !value.is_null() { write_unaligned(value.cast(), ListNode::from_ptr(node).occupant()); }
                    let previous = ListNode::from_ptr(position).previous();
                    ListNode::from_ptr(inserted).set_next(position);
                    ListNode::from_ptr(inserted).set_previous(previous);
                    ListNode::from_ptr(previous).set_next(inserted);
                    ListNode::from_ptr(position).set_previous(inserted);
                }
                node = ListNode::from_ptr(node).next();
            }
            assign_list(this.add(0x74).cast(), &mut head);
            let mut served = 0.0f32;
            let mut weighted = 0.0f32;
            let mut population_served = 0.0f32;
            let end = sim.recycling();
            let mut node = ListNode::from_ptr(end).next();
            let mut finished = node == end;
            while !finished && F80::from_f32(served).compare(F80::from_f32(population)).less() {
                let occupant = ListNode::from_ptr(node).occupant();
                let holder = vcall!(occupant, 0x18 => fn() -> *mut u8);
                if property_float(holder, 0x68ED_1736, &mut population_served) as u8 != 0 {
                    served = add_float(F80::from_f32(population_served), served).to_f32();
                    let mut capacity = 0.0;
                    if vcall!(this, 0xDC => fn(*mut u8,*mut f32) -> u32, occupant, &mut capacity) as u8 != 0 {
                        weighted = add_float(multiply_float(F80::from_f32(capacity), population_served), weighted).to_f32();
                    }
                }
                node = ListNode::from_ptr(node).next();
                finished = node == sim.recycling();
            }
            let positive = F80::from_f32(served).compare(F80::ZERO);
            if positive.less() || positive.equal() || positive.unordered() {
                write_unaligned(this.add(0x134).cast::<u32>(), *default_effect());
            } else {
                let denominator = if F80::from_f32(served).compare(F80::from_f32(population)).less() {
                    population
                } else { served };
                sim.set_effect(divide_float(F80::from_f32(weighted), denominator).to_f32());
            }
            destroy_list(&mut head);
        }
    }
}

/// This file's replacements.
pub const PORTS: &[Port] = &[fc4p_006e6da0::PORT];

#[cfg(test)]
mod tests;
