// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Per-building agent effects.

use super::queries::multiply_float;
use super::*;
use crate::x87::F80;

game_data! { static effects_map: *mut u8 = 0x00B4_9008; }
game_data! { static zero: f32 = 0x00A8_1054; }
game_fn! { fn find_effect(map: *mut u8, output: *mut *mut u8, key: *const u32) -> u64 = "thiscall" 0x0049_7520; }
game_fn! { fn node_value(map: *mut u8, key: *const u32) -> u64 = "thiscall" 0x0049_7A00; }

layout! {
    /// Circular effect-list links and payload.
    struct Effect {
        0x00 => next / set_next: *mut u8,
        0x08 => key / set_key: u32,
        0x0C => supply / set_supply: f32,
        0x10 => demand / set_demand: f32,
    }
}

fn compare_zero(value: f32, zero: f32) -> u16 {
    let status;
    unsafe {
        core::arch::asm!(
            "fld dword ptr [{zero}]", "fld dword ptr [{value}]", "fucompp", "fnstsw ax",
            zero = in(reg) &zero, value = in(reg) &value, out("ax") status, out("st(0)") _, out("st(1)") _,
        );
    }
    status
}

port! {
    /// Apply each nonzero (or unordered) effect, supply before demand, with percentage multiplication before sign.
    /// Retain the upper EAX bytes and comparison status that remain when the last effect has a zero demand term.
    #[port(address = 0x006A_5920, owner = "demand", name = "cSC4DemandSimulator::AddAgentEffect", naked)]
    pub unsafe extern "C" fn fc4p_006a5920() {
        core::arch::naked_asm!(
            "push edx", "push eax", "mov edx, esp", "call {body}", "mov ecx, dword ptr [esp]", "add esp, 8", "ret 12",
            body = sym add_effect,
        )
    }
}

unsafe extern "fastcall" fn add_effect(this: *mut u8, entry: *mut Entry) -> u64 {
    unsafe {
        let entry = &mut *entry;
        entry.eax = this as u32;
        let mut found = core::ptr::null_mut();
        let result = find_effect(effects_map().cast(), &mut found, entry.args.as_ptr());
        let mut edx = (result >> 32) as u32;
        if found == read_unaligned(effects_map()) {
            return registers(found as u32, edx);
        }
        let head = TreeNode::from_ptr(found).value() as *mut u8;
        entry.put(0, head as u32);
        let mut link = Effect::from_ptr(head).next();
        let mut eax = head as u32;
        while link != head {
            let effect = Effect::from_ptr(link);
            let result = node_value(this.add(0x50), link.add(8).cast());
            eax = result as u32;
            edx = (result >> 32) as u32;
            let object = read_unaligned(eax as *const *mut u8);
            for (slot, offset) in [(0x40, 0x0C), (0x44, 0x10)] {
                let coefficient = read_unaligned(link.add(offset).cast::<f32>());
                let status = compare_zero(coefficient, read_unaligned(zero()));
                eax = (eax & 0xFFFF_0000) | u32::from(status);
                if status & 0x4400 == 0x4000 {
                    continue;
                }
                let add = entry.arg(1) as u8;
                eax = (eax & !0xFF) | u32::from(add);
                let value = multiply_float(F80::from_f32(f32::from_bits(entry.arg(2))), coefficient);
                let value = if add == 0 { -value } else { value };
                if add != 0 {
                    edx = read_unaligned(object.cast());
                } else {
                    eax = read_unaligned(object.cast());
                }
                let result = virtual_call(object, slot, eax, edx, &[value.to_f32().to_bits()]);
                eax = result.eax;
                edx = result.edx;
            }
            link = effect.next();
        }
        registers(eax, edx)
    }
}

pub(super) const PORTS: &[Port] = &[fc4p_006a5920::PORT];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demand::{interface::tests::Map, test_support::Probe};
    use crate::testing::{fake, fake_data, GameFpu, Object};
    use std::cell::RefCell;

    #[derive(Debug, PartialEq)]
    enum Call {
        Find(u32),
        Node(u32, u32),
        Supply(u32),
        Demand(u32),
    }
    #[derive(Default)]
    struct State {
        calls: Vec<Call>,
        found: u32,
        object: u32,
        node_slot: u32,
        change: u32,
    }
    thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
    extern "thiscall" fn find(_map: *mut u8, output: *mut *mut u8, key: *const u32) -> u64 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.calls.push(Call::Find(unsafe { *key }));
            unsafe {
                *output = s.found as *mut u8;
            }
            registers(output as u32, 0x5566_7788)
        })
    }
    extern "thiscall" fn node(map: *mut u8, key: *const u32) -> u64 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.calls.push(Call::Node(map as u32, unsafe { *key }));
            s.node_slot = s.object;
            registers(&mut s.node_slot as *mut u32 as u32, 0xAABB_CCDD)
        })
    }
    extern "thiscall" fn supply(_this: *mut u8, bits: u32) -> u64 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.calls.push(Call::Supply(bits));
            if s.change != 0 {
                unsafe {
                    write_unaligned((s.change as *mut u8).add(0x10).cast::<u32>(), 8.0f32.to_bits());
                }
            }
        });
        0x1234_5678_AABB_CCDD
    }
    extern "thiscall" fn demand(_this: *mut u8, bits: u32) -> u64 {
        STATE.with(|s| s.borrow_mut().calls.push(Call::Demand(bits)));
        0x8765_4321_9988_7766
    }

    #[test]
    fn agent_effects_multiply_before_sign_and_reload_demand_after_supply() {
        let _fpu = GameFpu::new();
        let _a = fake(0x0049_7520, find as *const () as usize);
        let _b = fake(0x0049_7A00, node as *const () as usize);
        let mut header = 0x1234usize as *mut u8;
        let _data = fake_data(0x00B4_9008, (&mut header as *mut *mut u8).cast());
        let mut zero = 0.0f32;
        let _zero = fake_data(0x00A8_1054, (&mut zero as *mut f32).cast());
        let mut table = [0; 18];
        table[0x40 / 4] = supply as *const () as usize;
        table[0x44 / 4] = demand as *const () as usize;
        let mut object = [table.as_ptr() as u32];
        let mut memory = Object::new(0xB4);
        let this = memory.ptr();
        for add in [0, 1, 0x100, 0xFF] {
            let mut links = [[0u32; 5]; 3];
            let head = links.as_mut_ptr() as u32;
            let first = links[1].as_mut_ptr() as u32;
            let second = links[2].as_mut_ptr() as u32;
            links[0][0] = first;
            links[1] = [second, 0, 9, 2.0f32.to_bits(), 3.0f32.to_bits()];
            links[2] = [head, 0, 10, 0, 0];
            assert_eq!(links[2][0], head);
            let mut map = Map::new(&[(5, head)]);
            STATE.with(|s| {
                *s.borrow_mut() = State {
                    found: map.memory[1].as_mut_ptr() as u32,
                    object: object.as_mut_ptr() as u32,
                    change: first,
                    ..State::default()
                }
            });
            let mut call = Probe::new(fc4p_006a5920 as *const (), this, [5, add, 0.5f32.to_bits()]);
            unsafe {
                call.run();
            }
            let sign = if add as u8 == 0 { -1.0f32 } else { 1.0f32 };
            let expected = vec![
                Call::Find(5),
                Call::Node(this as u32 + 0x50, 9),
                Call::Supply(sign.to_bits()),
                Call::Demand((sign * 4.0).to_bits()),
                Call::Node(this as u32 + 0x50, 10),
            ];
            assert_eq!(STATE.with(|s| std::mem::take(&mut s.borrow_mut().calls)), expected);
            assert_eq!(call.ecx, this as u32);
            assert_eq!(call.after, [head, add, 0.5f32.to_bits()]);
            assert_eq!(call.eax & 0xFF00, 0x4000);
        }
    }

    #[test]
    fn absent_and_empty_effect_lists_make_no_node_calls() {
        let _a = fake(0x0049_7520, find as *const () as usize);
        let mut map = Map::new(&[]);
        let mut header = map.header();
        let _data = fake_data(0x00B4_9008, (&mut header as *mut *mut u8).cast());
        STATE.with(|s| {
            *s.borrow_mut() = State {
                found: header as u32,
                ..State::default()
            }
        });
        let mut call = Probe::new(fc4p_006a5920 as *const (), 0x5678usize as *mut u8, [5, 1, 0]);
        unsafe {
            call.run();
        }
        assert_eq!((call.eax, call.edx, call.ecx), (header as u32, 0x5566_7788, 0x5678));
        assert_eq!(call.after, [5, 1, 0]);
        assert_eq!(
            STATE.with(|s| std::mem::take(&mut s.borrow_mut().calls)),
            [Call::Find(5)]
        );
        let mut head = [0u32; 5];
        head[0] = head.as_mut_ptr() as u32;
        let mut map = Map::new(&[(5, head.as_mut_ptr() as u32)]);
        STATE.with(|s| s.borrow_mut().found = map.memory[1].as_mut_ptr() as u32);
        unsafe {
            call.run();
        }
        assert_eq!(call.eax, head.as_mut_ptr() as u32);
        assert_eq!(call.after, [head.as_mut_ptr() as u32, 1, 0]);
        assert_eq!(
            STATE.with(|s| std::mem::take(&mut s.borrow_mut().calls)),
            [Call::Find(5)]
        );
    }
}
