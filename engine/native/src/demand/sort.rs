// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Insertion into a sequence ordered by census-map values.

use super::*;
use crate::x87::F80;

game_fn! { fn score(map: *mut u8, key: *const u32) -> u64 = "thiscall" 0x006A_3170; }
game_fn! {
    #[allow(dead_code)]
    fn destroy_subtree(map: *mut u8, node: *mut u8) -> u64 = "thiscall" 0x006A_25C0;
}
game_fn! {
    #[allow(dead_code)]
    fn free(pointer: *mut u8) -> u64 = "C" 0x0090_CF63;
}

port! {
    /// Shift preceding keys while their scores exceed the inserted key's score, then destroy the by-value map.
    /// Arguments are position, inserted key, map header and map count. The caller supplies the lower-bound sentinel.
    #[port(address = 0x006A_3470, owner = "demand", name = "InsertKeyByCensusValue", naked)]
    pub unsafe extern "C" fn fc4p_006a3470() {
        core::arch::naked_asm!(
            "push edx", "push eax", "mov edx, esp", "call {body}", "mov ecx, dword ptr [esp]", "add esp, 8", "ret",
            body = sym insert,
        )
    }
}

unsafe extern "fastcall" fn insert(this: *mut u8, entry: *mut Entry) -> u64 {
    unsafe {
        let entry = &mut *entry;
        let mut position = entry.arg(0) as *mut u32;
        let value = entry.arg(1);
        entry.put(0, value);
        let map = entry.args.as_mut_ptr().add(2).cast::<u8>();
        let mut result;
        loop {
            let previous = read_unaligned(position.sub(1));
            entry.put(1, previous);
            let current = score(map, entry.args.as_ptr());
            let current = read_unaligned((current as u32 as *const u32).cast::<f32>());
            result = score(map, entry.args.as_ptr().add(1));
            let previous_score = read_unaligned((result as u32 as *const u32).cast::<f32>());
            if !F80::from_f32(current).compare(F80::from_f32(previous_score)).less() {
                break;
            }
            write_unaligned(position, read_unaligned(position.sub(1)));
            position = position.sub(1);
        }
        write_unaligned(position, value);
        if entry.arg(3) != 0 {
            let header = entry.arg(2) as *mut u8;
            let args = [TreeNode::from_ptr(header).parent() as u32];
            let mut call = RawCall::new(Conv::Thiscall, crate::abi::target(0x006A_25C0), map as u32, &args);
            call.eax = args[0];
            call.edx = header as u32;
            call_raw(&mut call);
            result = registers(call.eax, call.edx);
            TreeNode::from_ptr(entry.arg(2) as *mut u8).set_left(entry.arg(2) as *mut u8);
            TreeNode::from_ptr(entry.arg(2) as *mut u8).set_parent(core::ptr::null_mut());
            TreeNode::from_ptr(entry.arg(2) as *mut u8).set_right(entry.arg(2) as *mut u8);
            entry.put(3, 0);
        }
        let header = entry.arg(2);
        entry.eax = this as u32;
        if header != 0 {
            let args = [header];
            let mut call = RawCall::new(Conv::Cdecl, crate::abi::target(0x0090_CF63), 0, &args);
            call.eax = header;
            call.edx = (result >> 32) as u32;
            call_raw(&mut call);
            return registers(call.eax, call.edx);
        }
        registers(0, (result >> 32) as u32)
    }
}

pub(super) const PORTS: &[Port] = &[fc4p_006a3470::PORT];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demand::{interface::tests::Map, test_support::Probe};
    use crate::testing::{fake, GameFpu};
    use std::cell::RefCell;

    #[derive(Default)]
    struct State {
        values: [u32; 5],
        calls: Vec<(u32, u32)>,
    }
    thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
    extern "thiscall" fn value(_map: *mut u8, key: *const u32) -> u64 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            let key = unsafe { *key };
            s.calls.push((0x006A_3170, key));
            registers(&mut s.values[key as usize] as *mut u32 as u32, 0xAA55_1234)
        })
    }
    extern "thiscall" fn destroy(_map: *mut u8, root: u32) -> u64 {
        STATE.with(|s| s.borrow_mut().calls.push((0x006A_25C0, root)));
        0xDEAD_BEEF_8765_4321
    }
    extern "C" fn release(header: u32) -> u64 {
        STATE.with(|s| s.borrow_mut().calls.push((0x0090_CF63, header)));
        0x1234_5678_90AB_CDEF
    }

    #[test]
    fn insertion_orders_finite_scores_stops_on_equal_or_nan_and_destroys_the_map_copy() {
        let _fpu = GameFpu::new();
        let _a = fake(0x006A_3170, value as *const () as usize);
        let _b = fake(0x006A_25C0, destroy as *const () as usize);
        let _c = fake(0x0090_CF63, release as *const () as usize);
        for (score, expected, keys) in [
            (0.5, [0, 4, 1, 2, 3], vec![4, 3, 4, 2, 4, 1, 4, 0]),
            (2.0, [0, 1, 2, 4, 3], vec![4, 3, 4, 2]),
            (f32::NAN, [0, 1, 2, 3, 4], vec![4, 3]),
        ] {
            for count in [0, 3] {
                let mut map = Map::new(&[(1, 1), (2, 2), (3, 3)]);
                let header = map.header();
                let root = unsafe { TreeNode::from_ptr(header).parent() };
                STATE.with(|s| {
                    *s.borrow_mut() = State {
                        values: [
                            f32::NEG_INFINITY.to_bits(),
                            1.0f32.to_bits(),
                            2.0f32.to_bits(),
                            3.0f32.to_bits(),
                            score.to_bits(),
                        ],
                        calls: Vec::new(),
                    }
                });
                let mut array = [0u32, 1, 2, 3, 99];
                let mut call = Probe::new(
                    fc4p_006a3470 as *const (),
                    0x8888usize as *mut u8,
                    [unsafe { array.as_mut_ptr().add(4) } as u32, 4, header as u32],
                );
                call.extra_word = count;
                unsafe {
                    call.run();
                }
                assert_eq!(array, expected);
                assert_eq!((call.eax, call.edx, call.ecx), (0x90AB_CDEF, 0x1234_5678, 0x8888));
                assert_eq!(call.after[0], 4);
                assert_eq!(call.after[1], *keys.last().unwrap());
                let mut calls: Vec<_> = keys.iter().map(|&key| (0x006A_3170, key)).collect();
                if count != 0 {
                    calls.push((0x006A_25C0, root as u32));
                }
                calls.push((0x0090_CF63, header as u32));
                assert_eq!(STATE.with(|s| std::mem::take(&mut s.borrow_mut().calls)), calls);
                if count != 0 {
                    let h = unsafe { TreeNode::from_ptr(header) };
                    assert_eq!(
                        (h.parent(), h.left(), h.right()),
                        (core::ptr::null_mut(), header, header)
                    );
                }
            }
        }
    }
}
