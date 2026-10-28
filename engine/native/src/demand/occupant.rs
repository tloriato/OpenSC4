// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Updating a building's percentage-based agent effects.

use super::*;

game_fn! {
    #[allow(dead_code)]
    fn add_agent_effect(this: *mut u8, kind: u32, add: u32, percentage: u32) -> u64 = "thiscall" 0x006A_5920;
}

fn compare(old: f32, new: f32) -> u16 {
    let status;
    // SAFETY: both float operands are live and both pushed registers are popped.
    unsafe {
        core::arch::asm!(
            "fld dword ptr [{new}]", "fld dword ptr [{old}]", "fucompp", "fnstsw ax",
            new = in(reg) &new, old = in(reg) &old, out("ax") status,
            out("st(0)") _, out("st(1)") _,
        );
    }
    status
}

port! {
    /// Remove the old effects and add the new effects when the percentages differ or are unordered.
    /// Read the new percentage again after removing the old effects. Equal values retain the entry EDX
    /// and the upper EAX half, with the comparison status in AX.
    #[port(address = 0x006A_6C90, owner = "demand", name = "cSC4DemandSimulator::UpdateOccupantEffects", naked)]
    pub unsafe extern "C" fn fc4p_006a6c90() {
        core::arch::naked_asm!(
            "push edx", "push eax", "mov edx, esp", "call {body}", "add esp, 8", "ret 12",
            body = sym update,
        )
    }
}

unsafe extern "fastcall" fn update(this: *mut u8, entry: *mut Entry) -> u64 {
    // SAFETY: the original arguments name readable percentages and a live simulator.
    unsafe {
        let entry = &*entry;
        let old = entry.arg(1) as *const f32;
        let new = entry.arg(2) as *const f32;
        let status = compare(read_unaligned(old), read_unaligned(new));
        if status & 0x4400 == 0x4000 {
            return registers((entry.eax & 0xFFFF_0000) | u32::from(status), entry.edx);
        }
        let kind = entry.arg(0);
        let bits = read_unaligned(old.cast::<u32>());
        let args = [kind, 0, bits];
        let mut call = RawCall::new(Conv::Thiscall, crate::abi::target(0x006A_5920), this as u32, &args);
        call.eax = bits;
        call.edx = entry.edx;
        call_raw(&mut call);
        let args = [kind, 1, read_unaligned(new.cast::<u32>())];
        let mut next = RawCall::new(Conv::Thiscall, crate::abi::target(0x006A_5920), this as u32, &args);
        next.eax = call.eax;
        next.edx = call.edx;
        call_raw(&mut next);
        registers(next.eax, next.edx)
    }
}

pub(super) const PORTS: &[Port] = &[fc4p_006a6c90::PORT];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demand::test_support::Probe;
    use crate::testing::{fake, GameFpu};
    use std::cell::RefCell;

    #[derive(Default)]
    struct State {
        calls: Vec<(u32, u32, u32, u32)>,
        change: usize,
    }
    thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }

    extern "thiscall" fn effect(this: *mut u8, kind: u32, add: u32, bits: u32) -> u64 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.calls.push((this as u32, kind, add, bits));
            if add == 0 && s.change != 0 {
                unsafe {
                    *(s.change as *mut f32) = 0.75;
                }
            }
        });
        0xAABB_CCDD_1122_3344
    }

    #[test]
    fn changed_percentages_remove_then_add_and_reload_the_new_value() {
        let _fpu = GameFpu::new();
        let _fake = fake(0x006A_5920, effect as *const () as usize);
        let old = 0.25f32;
        let mut new = 0.5f32;
        STATE.with(|s| {
            *s.borrow_mut() = State {
                calls: Vec::new(),
                change: &mut new as *mut f32 as usize,
            }
        });
        let args = [0xABC, &old as *const f32 as u32, &new as *const f32 as u32];
        let mut call = Probe::new(fc4p_006a6c90 as *const (), 0x1234usize as *mut u8, args);
        unsafe {
            call.run();
        }
        assert_eq!(call.after, args);
        assert_eq!((call.eax, call.edx), (0x1122_3344, 0xAABB_CCDD));
        STATE.with(|s| {
            assert_eq!(
                s.borrow().calls,
                [(0x1234, 0xABC, 0, old.to_bits()), (0x1234, 0xABC, 1, 0.75f32.to_bits())]
            )
        });
    }

    #[test]
    fn equal_percentages_skip_both_calls_and_nan_percentages_apply_both() {
        let _fpu = GameFpu::new();
        let _fake = fake(0x006A_5920, effect as *const () as usize);
        for (old, new, changed) in [
            (0.0f32, -0.0f32, false),
            (2.0, 2.0, false),
            (f32::INFINITY, f32::INFINITY, false),
            (f32::NAN, 1.0, true),
            (1.0, f32::NAN, true),
            (f32::from_bits(0x7F80_0001), 1.0, true),
        ] {
            STATE.with(|s| *s.borrow_mut() = State::default());
            let mut call = Probe::new(
                fc4p_006a6c90 as *const (),
                core::ptr::null_mut(),
                [42, &old as *const f32 as u32, &new as *const f32 as u32],
            );
            unsafe {
                call.run();
            }
            STATE.with(|s| assert_eq!(s.borrow().calls.len(), if changed { 2 } else { 0 }));
            if !changed {
                assert_eq!(call.eax & 0xFFFF_4400, 0x1122_4000);
                assert_eq!(call.edx, 0x5566_7788);
            }
        }
    }
}
