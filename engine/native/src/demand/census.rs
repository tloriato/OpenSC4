// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Reset the fixed resident, worker and occupied-job census entries.

use super::*;

game_fn! {
    fn census_value(map: *mut u8, key: *const u32) -> *mut u32 = "thiscall" 0x006A_3170;
}

const CENSUS_KEYS: [u32; 24] = [
    0x2111, 0x2112, 0x2113, 0x2114, 0x2121, 0x2122, 0x2123, 0x2124, 0x2131, 0x2132, 0x2133, 0x2134, 0x1011, 0x1021,
    0x1031, 0x3111, 0x3121, 0x3131, 0x3321, 0x3331, 0x4101, 0x4201, 0x4301, 0x4401,
];

port! {
    /// Set the 24 fixed census keys to positive zero through the game's map accessor, preserving other entries.
    /// The map pointer is a stack argument; ECX is unused. The argument becomes the successive key after the first.
    #[port(address = 0x006A_37A0, owner = "demand", name = "cSC4DemandSimulator::ZeroCensusInfo", naked)]
    pub unsafe extern "C" fn fc4p_006a37a0() {
        core::arch::naked_asm!(
            "push edx", "push eax", "mov edx, esp", "call {body}", "add esp, 8", "ret 4",
            body = sym zero_census,
        )
    }
}

unsafe extern "fastcall" fn zero_census(_this: *mut u8, entry: *mut Entry) -> u32 {
    unsafe {
        let entry = &mut *entry;
        let map = entry.arg(0) as *mut u8;
        let first = census_value(map, CENSUS_KEYS.as_ptr());
        write_unaligned(first, 0);
        let mut result = first;
        for key in &CENSUS_KEYS[1..] {
            entry.put(0, *key);
            result = census_value(map, entry.args.as_ptr());
            write_unaligned(result, 0);
        }
        result as u32
    }
}

pub(super) const PORTS: &[Port] = &[fc4p_006a37a0::PORT];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demand::test_support::Probe;
    use crate::testing::fake;
    use std::cell::RefCell;

    #[derive(Default)]
    struct State {
        map: u32,
        keys: Vec<u32>,
        values: [u32; 25],
    }
    thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }

    extern "thiscall" fn value(map: *mut u8, key: *const u32) -> *mut u32 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            assert_eq!(map as u32, s.map);
            let i = s.keys.len();
            assert!(s.values[..i].iter().all(|&v| v == 0));
            s.keys.push(unsafe { *key });
            &mut s.values[i] as *mut u32
        })
    }

    #[test]
    fn census_reset_orders_all_24_keys_and_leaves_unrelated_entries_untouched() {
        let _fake = fake(0x006A_3170, value as *const () as usize);
        STATE.with(|s| {
            *s.borrow_mut() = State {
                map: 0x1234_5678,
                keys: Vec::new(),
                values: [0x7FC1_2345; 25],
            }
        });
        let mut call = Probe::new(fc4p_006a37a0 as *const (), 0xDEADusize as *mut u8, [0x1234_5678, 7, 8]);
        unsafe {
            call.run();
        }
        assert_eq!(call.after, [0x4401, 7, 8]);
        STATE.with(|s| {
            let s = s.borrow();
            assert_eq!(s.keys, CENSUS_KEYS);
            assert_eq!(s.values[..24], [0; 24]);
            assert_eq!(s.values[24], 0x7FC1_2345);
        });
    }
}
