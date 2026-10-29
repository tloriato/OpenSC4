// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, Object};
use crate::traffic::test_support::{bytes, invoke};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Allocate(u32, u32),
    Free(u32),
    Erase(u32, u32),
    Assign(u32, u32),
    Construct(u32, u32),
    Walk([u32; 8]),
    Blocks(u32, u32, u32),
}
thread_local! {
    static CALLS: RefCell<Vec<Call>> = const { RefCell::new(Vec::new()) };
    static ALLOCATION: RefCell<(u32, u32)> = const { RefCell::new((0, 0)) };
}
fn log(c: Call) {
    CALLS.with(|v| v.borrow_mut().push(c));
}
fn take() -> Vec<Call> {
    CALLS.with(|v| core::mem::take(&mut *v.borrow_mut()))
}
const FREE_RESULT: u64 = 0x1234_5678_AABB_CCDD;
extern "C" fn allocate_fake(size: u32) -> u64 {
    let (ptr, owner) = ALLOCATION.with(|a| *a.borrow());
    log(Call::Allocate(size, unsafe { word(owner as *mut u8, 0) }));
    0x8765_4321_0000_0000 | u64::from(ptr)
}
extern "C" fn free_fake(p: u32) -> u64 {
    log(Call::Free(p));
    FREE_RESULT
}
extern "thiscall" fn erase_fake(this: *mut u8, root: u32) -> u64 {
    log(Call::Erase(this as u32, root));
    FREE_RESULT
}
extern "thiscall" fn assign_fake(this: *mut u8, source: *mut u8) -> u64 {
    log(Call::Assign(this as u32, source as u32));
    unsafe {
        store(this, 0, word(source, 0));
    }
    FREE_RESULT
}
extern "thiscall" fn construct_fake(this: *mut u8, source: *mut u8) -> u64 {
    log(Call::Construct(this as u32, source as u32));
    unsafe {
        store(this, 0, word(source, 0));
    }
    FREE_RESULT
}
extern "C" fn walk_fake(a: u32, b: u32, c: u32, d: u32, e: u32, f: u32, g: u32, h: u32, scratch: *mut u8) -> u64 {
    log(Call::Walk([a, b, c, d, e, f, g, h]));
    unsafe {
        scratch.write(0x91);
    }
    FREE_RESULT
}
extern "thiscall" fn blocks_fake(this: *mut u8, begin: u32, end: u32) -> u64 {
    log(Call::Blocks(this as u32, begin, end));
    FREE_RESULT
}

#[test]
fn tree_sentinel_construction_and_destruction_retain_padding_and_owner_pointer() {
    unsafe {
        let _alloc = fake(0x0090_CF54, allocate_fake as *const () as usize);
        let _free = fake(0x0090_CF63, free_fake as *const () as usize);
        let _erase = fake(0x0042_6A60, erase_fake as *const () as usize);
        let mut owner = Object::new(16);
        let p = owner.ptr();
        let mut header = [0xA5u8; 24];
        let h = header.as_mut_ptr();
        store(p, 0, 0xDEAD);
        store(p, 4, 9);
        store(p, 8, 0x1234);
        ALLOCATION.with(|a| *a.borrow_mut() = (h as u32, p as u32));
        let r = invoke(fc4p_00713620 as *const () as usize, p, &[], 0x11, 0x22, false);
        assert_eq!((r.eax, r.edx), (p as u32, 0x8765_4321));
        assert_eq!(take(), [Call::Allocate(24, 0)]);
        let mut expected = [0xA5u8; 24];
        expected[0] = 0;
        expected[4..8].copy_from_slice(&0u32.to_le_bytes());
        expected[8..12].copy_from_slice(&(h as u32).to_le_bytes());
        expected[12..16].copy_from_slice(&(h as u32).to_le_bytes());
        assert_eq!(header, expected);
        assert_eq!(word(p, 8), 0x1234);
        for count in [0, 3] {
            store(p, 4, count);
            store(h, 4, 0x7777);
            let r = invoke(fc4p_00713660 as *const () as usize, p, &[], 0x11, 0x22, false);
            assert_eq!((r.eax, r.edx), (FREE_RESULT as u32, (FREE_RESULT >> 32) as u32));
            let want = if count == 0 {
                vec![Call::Free(h as u32)]
            } else {
                vec![Call::Erase(p as u32, 0x7777), Call::Free(h as u32)]
            };
            assert_eq!(take(), want);
            assert_eq!(word(p, 0), h as u32);
            assert_eq!(word(p, 4), 0);
            if count != 0 {
                assert_eq!(word(h, 4), 0);
            }
        }
        store(p, 0, 0);
        let r = invoke(fc4p_00713660 as *const () as usize, p, &[], 0x11, 0x22, false);
        assert_eq!((r.eax, r.edx), (0, 0x22));
        assert!(take().is_empty());
    }
}

#[test]
fn trip_erase_unlinks_then_frees_second_first_and_node_without_changing_count() {
    unsafe {
        let _free = fake(0x0090_CF63, free_fake as *const () as usize);
        for (first, second) in [(0, 0), (0x1000, 0), (0, 0x2000), (0x1000, 0x2000)] {
            let mut node = Object::new(48);
            let n = node.ptr();
            let mut next = Object::new(16);
            let nx = next.ptr();
            let mut prev = Object::new(16);
            let pr = prev.ptr();
            store(n, 0, nx as u32);
            store(n, 4, pr as u32);
            store(n, 20, first);
            store(n, 32, second);
            let before = bytes(n, 48);
            let mut result = 0u32;
            let r = invoke(
                fc4p_007145e0 as *const () as usize,
                pr,
                &[&mut result as *mut u32 as u32, n as u32],
                0,
                0,
                false,
            );
            assert_eq!(r.eax, &mut result as *mut u32 as u32);
            assert_eq!(r.edx, 0x1234_5678);
            assert_eq!(result, nx as u32);
            assert_eq!(word(pr, 0), nx as u32);
            assert_eq!(word(nx, 4), pr as u32);
            assert_eq!(bytes(n, 48), before);
            let mut want = Vec::new();
            if second != 0 {
                want.push(Call::Free(second));
            }
            if first != 0 {
                want.push(Call::Free(first));
            }
            want.push(Call::Free(n as u32));
            assert_eq!(take(), want);
        }
    }
}

#[test]
fn clear_trip_list_handles_empty_and_multiple_nodes_and_leaves_other_fields_alone() {
    unsafe {
        let _free = fake(0x0090_CF63, free_fake as *const () as usize);
        let mut owner = Object::new(16);
        let p = owner.ptr();
        let mut head = Object::new(48);
        let h = head.ptr();
        let mut a = Object::new(48);
        let a = a.ptr();
        let mut b = Object::new(48);
        let b = b.ptr();
        store(p, 0, h as u32);
        store(p, 4, 123);
        for empty in [true, false] {
            store(h, 0, if empty { h as u32 } else { a as u32 });
            store(h, 4, b as u32);
            store(a, 0, b as u32);
            store(a, 20, 0x1000);
            store(a, 32, 0x2000);
            store(b, 0, h as u32);
            let r = invoke(fc4p_00714660 as *const () as usize, p, &[], 0x11, 0x22, false);
            assert_eq!(r.eax, h as u32);
            assert_eq!(word(h, 0), h as u32);
            assert_eq!(word(h, 4), h as u32);
            assert_eq!(word(p, 4), 123);
            assert_eq!(
                take(),
                if empty {
                    vec![]
                } else {
                    vec![
                        Call::Free(0x2000),
                        Call::Free(0x1000),
                        Call::Free(a as u32),
                        Call::Free(b as u32),
                    ]
                }
            );
        }
    }
}

#[test]
fn copy_fill_and_construct_subnetworks_use_original_tree_callees_and_copy_only_tail_words() {
    unsafe {
        let _assign = fake(0x0071_0FF0, assign_fake as *const () as usize);
        let _construct = fake(0x0071_2280, construct_fake as *const () as usize);
        let mut source = [
            0x1111_1111u32,
            2,
            3,
            12,
            16,
            20,
            24,
            0x2222_2222,
            8,
            9,
            112,
            116,
            120,
            124,
        ];
        let s = source.as_mut_ptr().cast::<u8>();
        for mode in 0..3 {
            let mut destination = [0xA5A5_A5A5u32; 14];
            let d = destination.as_mut_ptr().cast::<u8>();
            let function = [
                fc4p_00714000 as *const () as usize,
                fc4p_007146b0 as *const () as usize,
                fc4p_007146f0 as *const () as usize,
            ][mode];
            let args = if mode == 0 {
                [s as u32, s.add(56) as u32, d as u32]
            } else if mode == 1 {
                [d as u32, d.add(56) as u32, s as u32]
            } else {
                [d as u32, 2, s as u32]
            };
            let r = invoke(function, core::ptr::null_mut(), &args, 0x44, 0x55, true);
            assert_eq!(r.eax, if mode == 1 { 24 } else { d.add(56) as u32 });
            if mode != 2 {
                assert_eq!(r.edx, if mode == 0 { 120 } else { 20 });
            }
            assert_eq!(&destination[3..7], &source[3..7]);
            assert_eq!(
                &destination[10..14],
                if mode == 0 { &source[10..14] } else { &source[3..7] }
            );
            assert_eq!(destination[1], 0xA5A5_A5A5);
            assert_eq!(destination[2], 0xA5A5_A5A5);
            let ctor = |dest: u32, src: u32| {
                if mode == 2 {
                    Call::Construct(dest, src)
                } else {
                    Call::Assign(dest, src)
                }
            };
            assert_eq!(
                take(),
                [
                    ctor(d as u32, s as u32),
                    ctor(d.add(28) as u32, if mode == 0 { s.add(28) as u32 } else { s as u32 })
                ]
            );
        }
        let r = invoke(
            fc4p_00714000 as *const () as usize,
            core::ptr::null_mut(),
            &[s.add(28) as u32, s as u32, s as u32],
            0x44,
            0x55,
            true,
        );
        assert_eq!((r.eax, r.edx), (s as u32, (-2i32) as u32));
        let r = invoke(
            fc4p_007146b0 as *const () as usize,
            0x9999 as *mut u8,
            &[s as u32, s as u32, 0],
            0x44,
            0x55,
            true,
        );
        assert_eq!(
            r,
            Registers {
                eax: 0x44,
                ecx: 0x9999,
                edx: 0x55
            }
        );
        let r = invoke(
            fc4p_007146f0 as *const () as usize,
            core::ptr::null_mut(),
            &[0, 1, 0],
            0x44,
            0x55,
            true,
        );
        assert_eq!(r.eax, 28);
        assert!(take().is_empty());
    }
}

#[test]
fn segmented_range_cleanup_passes_both_iterators_by_value_and_reloads_allocation_after_blocks() {
    unsafe {
        let _walk = fake(0x0070_FEC0, walk_fake as *const () as usize);
        let _blocks = fake(0x006D_A8D0, blocks_fake as *const () as usize);
        let _free = fake(0x0090_CF63, free_fake as *const () as usize);
        for allocation in [0, 0xCAFE] {
            let mut object = [1u32, 2, 3, 4, 5, 6, 7, 8, allocation];
            let p = object.as_mut_ptr().cast();
            let before = object;
            let r = invoke(fc4p_00713e90 as *const () as usize, p, &[], 0x44, 0x55, false);
            let mut want = vec![Call::Walk([1, 2, 3, 4, 5, 6, 7, 8])];
            if allocation != 0 {
                want.extend([Call::Blocks(p as u32, 4, 12), Call::Free(allocation)]);
            }
            assert_eq!(take(), want);
            assert_eq!(object, before);
            assert_eq!(r.eax, if allocation == 0 { 0 } else { FREE_RESULT as u32 });
        }
    }
}
