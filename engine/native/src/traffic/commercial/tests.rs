// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, fake_data, Object};
use crate::traffic::test_support::invoke;
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Fill(u32, u32, u16),
    Free(u32),
}
thread_local! {static VALUES: RefCell<[u16;6]>=const{RefCell::new([0;6])};static CALLS:RefCell<Vec<Call>>=const{RefCell::new(Vec::new())};}
extern "thiscall" fn fill_fake(this: *mut u8, position: u32, count: u32, initial: *const u16) -> u32 {
    unsafe {
        CALLS.with(|v| {
            v.borrow_mut()
                .push(Call::Fill(position, count, initial.read_unaligned()))
        });
        VALUES.with(|v| {
            let mut v = v.borrow_mut();
            v.fill(0);
            let p = v.as_mut_ptr();
            store(this, 0, p as u32);
            store(this, 4, p.add(6) as u32);
            store(this, 8, p.add(6) as u32);
        });
    }
    0
}
extern "C" fn free_fake(ptr: u32) -> u64 {
    CALLS.with(|v| v.borrow_mut().push(Call::Free(ptr)));
    0x1234_5678_ABCD_9876
}

/// The core's commercial cell vectors: road sums wrap to u16 and compete with elevated rail and
/// monorail, while pedestrian, rail and subway usage is excluded.
#[test]
fn commercial_cell_selects_wrapping_network_totals_and_preserves_free_result_upper_half() {
    unsafe {
        let _fill = fake(0x0046_5760, fill_fake as *const () as usize);
        let _free = fake(0x0090_CF63, free_fake as *const () as usize);
        let mut networks = [0i32, 1, 1, 2, 1, 2, 3, 4, 5];
        let _net = fake_data(0x00AB_31A8, networks.as_mut_ptr().cast());
        for (usage, want) in [
            ([0u16; 9], 0),
            ([99, 40, 20, 500, 5, 1000, 600, 64, 63], 65),
            ([0, 65535, 1, 0, 0, 0, 0, 2, 3], 3),
            ([0, 100, 200, 0, 300, 0, 0, 700, 900], 900),
        ] {
            let mut sim = Object::new(0xD60);
            let p = sim.ptr();
            let mut nodes = [[0u32; 19]; 10];
            let mut buckets = [0u32; 5];
            let mut previous = 0u32;
            // Deliberate collisions force searches past unrelated keys before reaching each match.
            for i in 0..10 {
                let n = nodes[i].as_mut_ptr().cast::<u8>();
                store(n, 0, previous);
                let travel = if i == 9 { 99 } else { i };
                store(n, 4, 0x0300_0000 | ((travel as u32) << 16) | 0x0000_FEFD);
                write_unaligned(n.add(40).cast::<u16>(), if i == 9 { 999 } else { usage[i] });
                previous = n as u32;
            }
            // All keys end in the same bucket modulo one; the larger storage remains a canary.
            buckets[0] = previous;
            store(p, 0x17C, buckets.as_mut_ptr() as u32);
            store(p, 0x180, buckets.as_mut_ptr().add(1) as u32);
            let before = nodes;
            let r = invoke(
                fc4p_00714940 as *const () as usize,
                p,
                &[0xABCD_FFFE, 0x1234_FFFD, 0x9999_0003],
                0x11,
                0x22,
                false,
            );
            assert_eq!(r.eax, 0xABCD_0000 | want as u32);
            assert_eq!(nodes, before);
            let pointer = VALUES.with(|v| v.borrow().as_ptr() as u32);
            assert_eq!(
                CALLS.with(|v| core::mem::take(&mut *v.borrow_mut())),
                [Call::Fill(0, 6, 0), Call::Free(pointer)]
            );
            let r = invoke(fc4p_00714940 as *const () as usize, p, &[0xFE, 0xFD, 2], 0, 0, false);
            assert_eq!(r.eax, 0xABCD_0000);
            CALLS.with(|v| v.borrow_mut().clear());
        }
    }
}
