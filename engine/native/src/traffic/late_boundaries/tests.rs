// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, Object};
use crate::traffic::test_support::{bytes, invoke, table};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Allocate(u32),
    Insert(u16),
    Release(u32),
    Detach(u32),
    Free(u32),
    Erase(u32),
    Prune(u32),
}
#[derive(Default)]
struct State {
    calls: Vec<Call>,
    header: u32,
    temp_nodes: Vec<u32>,
    used: usize,
    pool_nodes: Vec<u32>,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn take() -> Vec<Call> {
    STATE.with(|s| core::mem::take(&mut s.borrow_mut().calls))
}
extern "C" fn allocate_fake(size: u32) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Allocate(size));
        0x1234_5678_0000_0000 | u64::from(s.header)
    })
}
extern "C" fn free_fake(ptr: u32) -> u64 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Free(ptr)));
    0x1234_5678_AABB_CCDD
}
extern "thiscall" fn release_fake(this: *mut u8) -> u32 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Release(this as u32)));
    0
}
extern "thiscall" fn prune_fake(this: *mut u8) -> u32 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Prune(this as u32)));
    0
}
extern "thiscall" fn erase_fake(_this: *mut u8, node: u32) -> u32 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Erase(node)));
    0
}
extern "thiscall" fn insert_fake(this: *mut u8, result: *mut u32, key: *const u16) -> u32 {
    unsafe {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            let key = key.read_unaligned();
            s.calls.push(Call::Insert(key));
            let h = word(this, 0) as *mut u8;
            let mut parent = h;
            let mut current = word(h, 4) as *mut u8;
            while !current.is_null() {
                if read_unaligned(current.add(16).cast::<u16>()) == key {
                    result.write_unaligned(current as u32);
                    result.add(1).write_unaligned(0);
                    return result as u32;
                }
                parent = current;
                current = word(current, 12) as *mut u8;
            }
            let n = s.temp_nodes[s.used] as *mut u8;
            s.used += 1;
            store(n, 4, parent as u32);
            store(n, 8, 0);
            store(n, 12, 0);
            write_unaligned(n.add(16).cast::<u16>(), key);
            if parent == h {
                store(h, 4, n as u32);
                store(h, 8, n as u32);
            } else {
                store(parent, 12, n as u32);
            }
            store(h, 12, n as u32);
            store(this, 4, word(this, 4) + 1);
            result.write_unaligned(n as u32);
            result.add(1).write_unaligned(1);
            result as u32
        })
    }
}
extern "C" fn detach_fake(node: u32, root: *mut u32, first: *mut u32, last: *mut u32) -> u32 {
    unsafe {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.calls.push(Call::Detach(node));
            s.pool_nodes.retain(|&n| n != node);
            let h = root.cast::<u8>().sub(4);
            root.write_unaligned(s.pool_nodes.first().copied().unwrap_or(0));
            first.write_unaligned(s.pool_nodes.first().copied().unwrap_or(h as u32));
            last.write_unaligned(s.pool_nodes.last().copied().unwrap_or(h as u32));
            for (i, &n) in s.pool_nodes.iter().enumerate() {
                store(n as *mut u8, 4, if i == 0 { h as u32 } else { s.pool_nodes[i - 1] });
                store(n as *mut u8, 8, 0);
                store(n as *mut u8, 12, s.pool_nodes.get(i + 1).copied().unwrap_or(0));
            }
            node
        })
    }
}

/// The core's neighbor-pool cleanup: the union of four boundary ID arrays retains used holders.
/// The final coordinate is excluded on every side; duplicate IDs still call the insertion helper.
#[test]
fn pool_pruning_keeps_used_ids_releases_unused_holders_and_destroys_the_temporary_tree() {
    unsafe {
        let _allocate = fake(0x0090_CF54, allocate_fake as *const () as usize);
        let _free = fake(0x0090_CF63, free_fake as *const () as usize);
        let _insert = fake(0x0071_09B0, insert_fake as *const () as usize);
        let _detach = fake(0x0040_DAF0, detach_fake as *const () as usize);
        let _erase = fake(0x0070_EF50, erase_fake as *const () as usize);
        let hv = table(&[(8, release_fake as *const () as usize)]);
        for retain in [false, true] {
            let mut sim = Object::new(0xD60);
            let p = sim.ptr();
            let ts = TrafficSim::from_ptr(p);
            let bs = BoundarySim::from_ptr(p);
            ts.set_max_cell_x(2);
            ts.set_max_cell_z(999);
            let mut edges = [[0u16; 12]; 4];
            if retain {
                edges[0][1] = 10;
                edges[1][1] = 30;
                edges[1][5] = 10;
            }
            for (i, side) in edges.iter_mut().enumerate() {
                side[9] = 999;
                store(p, 0x88 + i * 12, side.as_mut_ptr() as u32);
            }
            let mut header = [0xA5u8; 24];
            let h = header.as_mut_ptr();
            let mut nodes = [[0u32; 6]; 3];
            let ns: Vec<_> = nodes.iter_mut().map(|n| n.as_mut_ptr() as u32).collect();
            let mut holders = [[hv.as_ptr() as u32]; 3];
            let hs: Vec<_> = holders.iter_mut().map(|h| h.as_mut_ptr() as u32).collect();
            for i in 0..3 {
                store(ns[i] as *mut u8, 4, if i == 0 { h as u32 } else { ns[i - 1] });
                store(ns[i] as *mut u8, 12, ns.get(i + 1).copied().unwrap_or(0));
                write_unaligned((ns[i] as *mut u8).add(16).cast::<u16>(), 10 * (i as u16 + 1));
                store(ns[i] as *mut u8, 20, hs[i]);
            }
            store(h, 4, ns[0]);
            store(h, 8, ns[0]);
            store(h, 12, ns[2]);
            bs.set_pool_header(h);
            bs.set_pool_count(3);
            let mut temp_header = [0xA5u8; 20];
            let th = temp_header.as_mut_ptr();
            let mut temp_nodes = [[0xA5u8; 20]; 2];
            let tn: Vec<_> = temp_nodes.iter_mut().map(|n| n.as_mut_ptr() as u32).collect();
            STATE.with(|s| {
                *s.borrow_mut() = State {
                    header: th as u32,
                    temp_nodes: tn.clone(),
                    pool_nodes: ns.clone(),
                    ..State::default()
                }
            });
            let r = invoke(fc4p_007138b0 as *const () as usize, p, &[], 0x11, 0x22, false);
            assert_eq!(r.eax, 0xAABB_CCDD);
            assert_eq!(bs.pool_count(), if retain { 2 } else { 0 });
            let mut expected = vec![Call::Allocate(20)];
            if retain {
                expected.extend([
                    Call::Insert(10),
                    Call::Insert(30),
                    Call::Insert(10),
                    Call::Release(hs[1]),
                    Call::Detach(ns[1]),
                    Call::Free(ns[1]),
                    Call::Erase(tn[1]),
                    Call::Free(tn[0]),
                ]);
            } else {
                for i in 0..3 {
                    expected.extend([Call::Release(hs[i]), Call::Detach(ns[i]), Call::Free(ns[i])]);
                }
            }
            expected.push(Call::Free(th as u32));
            assert_eq!(take(), expected);
            assert_eq!(temp_header[0], 0);
            assert_eq!(&temp_header[1..4], &[0xA5; 3]);
            assert_eq!(word(th, 4), 0);
            assert_eq!(word(th, 8), th as u32);
            assert_eq!(word(th, 12), th as u32);
        }
    }
}

#[test]
fn boundary_removal_clears_only_the_requested_bit_then_prunes_and_updates_destination_counts() {
    unsafe {
        let _prune = fake(0x0071_38B0, prune_fake as *const () as usize);
        for (x, z, side, index) in [(-1i32, 1i32, 0, 1), (4, 1, 2, 1), (1, -1, 1, 1), (1, 4, 3, 1)] {
            for (kind, mask, cleared) in [(0u32, 1u16, true), (1, 3, false), (16, 1, false), (32, 1, true)] {
                let mut sim = Object::new(0xD60);
                let p = sim.ptr();
                let ts = TrafficSim::from_ptr(p);
                let bs = BoundarySim::from_ptr(p);
                ts.set_max_cell_x(3);
                let mut edges = [[0u16; 24]; 4];
                for (i, edge) in edges.iter_mut().enumerate() {
                    store(p, 0x88 + i * 12, edge.as_mut_ptr() as u32);
                }
                edges[side][index * 4] = mask;
                edges[side][index * 4 + 1] = 77;
                let mut rows = [0u32; 24];
                let mut cells = [0xA5u8; 8];
                cells[(z + 1) as usize] = 0xAF;
                rows[(x * 3 + 3) as usize] = cells.as_mut_ptr() as u32;
                bs.set_destination_rows(rows.as_mut_ptr().cast());
                bs.set_free_destinations([0, 1, 2, u32::MAX]);
                let before = bytes(p, 0xD60);
                let r = invoke(
                    fc4p_00713ab0 as *const () as usize,
                    p,
                    &[kind, x as u32, z as u32],
                    0x11,
                    0x22,
                    false,
                );
                assert_eq!(edges[side][index * 4], mask & !1u32.wrapping_shl(kind) as u16);
                assert_eq!(edges[side][index * 4 + 1], if cleared { 0 } else { 77 });
                if cleared {
                    assert_eq!(take(), [Call::Prune(p as u32)]);
                    assert_eq!(bs.free_destinations(), [u32::MAX, 0, 1, u32::MAX - 1]);
                    assert_eq!(cells[(z + 1) as usize], 0);
                    assert_eq!(r.eax, x.wrapping_mul(12) as u32);
                } else {
                    assert!(take().is_empty());
                    assert_eq!(bytes(p, 0xD60), before);
                    assert_eq!(cells[(z + 1) as usize], 0xAF);
                }
            }
        }
        let mut sim = Object::new(0xD60);
        let p = sim.ptr();
        let r = invoke(
            fc4p_00713ab0 as *const () as usize,
            p,
            &[0, u32::MAX, 0],
            0x11,
            0x22,
            false,
        );
        assert_eq!(r.eax, 0);
        assert!(take().is_empty());
    }
}
