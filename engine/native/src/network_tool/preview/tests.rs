// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::abi::{call_raw, Conv, RawCall};
use crate::testing::{fake, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Interface(usize),
    Allocate(u32),
    Erase(usize, usize, bool),
    Free(usize),
    Release(usize, usize),
    Destroy(usize),
    Delete(usize),
}
#[derive(Default)]
struct State {
    calls: Vec<Call>,
    allocations: Vec<usize>,
    swap: Option<(usize, usize)>,
    preview: usize,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn calls() -> Vec<Call> {
    STATE.with(|s| core::mem::take(&mut s.borrow_mut().calls))
}
extern "thiscall" fn interface(this: *mut u8) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(unsafe { read::<u32>(this.sub(4), 0) }, 0x00A8_81C0);
        s.calls.push(Call::Interface(this as usize));
        unsafe {
            write(this, 4, 0xCAFE_BABEu32);
        }
    });
}
extern "C" fn alloc(size: u32) -> *mut u8 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Allocate(size));
        s.allocations.remove(0) as *mut u8
    })
}
extern "thiscall" fn erase_a(this: *mut u8, root: *mut u8) {
    erase(this, root, false);
}
extern "thiscall" fn erase_b(this: *mut u8, root: *mut u8) {
    erase(this, root, true);
}
fn erase(this: *mut u8, root: *mut u8, second: bool) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Erase(this as usize, root as usize, second));
        if let Some((location, next)) = s.swap.take() {
            unsafe {
                (location as *mut usize).write(next);
            }
        }
    });
}
extern "C" fn free(this: *mut u8) {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Free(this as usize)));
}
extern "thiscall" fn release(this: *mut u8) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let pointer = if s.preview == 0 {
            0
        } else {
            unsafe { read::<usize>(s.preview as *mut u8, 0xC) }
        };
        s.calls.push(Call::Release(this as usize, pointer));
    });
    registers(0xABCD_EF67, 0x1234_5678)
}
extern "thiscall" fn destroy_interface(this: *mut u8) -> u64 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Interface(this as usize)));
    registers(0x1111_2222, 0x3333_4444)
}
extern "thiscall" fn destroy(this: *mut u8) -> u64 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Destroy(this as usize)));
    registers(0x1111_2222, 0x3333_4444)
}
extern "C" fn delete(this: *mut u8) -> u64 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Delete(this as usize)));
    registers(0x5555_6666, 0x7777_8888)
}
fn invoke(function: usize, this: *mut u8, args: &[u32]) -> (u32, u32) {
    let mut c = RawCall::new(Conv::Thiscall, function, this as u32, args);
    c.eax = 0xA1B2_C3D4;
    c.edx = 0xB1C2_D3E4;
    unsafe {
        call_raw(&mut c);
    }
    (c.eax, c.edx)
}
#[test]
fn constructor_preserves_padding_initializes_headers_and_calls_in_order() {
    let _a = fake(0x0090_DA1E, interface as *const () as usize);
    let _b = fake(0x0090_CF54, alloc as *const () as usize);
    let mut obj = [0xA5u8; 0x60];
    let mut first = [0xC3u8; 0x7C];
    let mut second = [0xC3u8; 0x80];
    let p = obj.as_mut_ptr();
    let a = first.as_mut_ptr();
    let b = second.as_mut_ptr();
    STATE.with(|s| {
        *s.borrow_mut() = State {
            allocations: vec![a as usize, b as usize],
            ..State::default()
        }
    });
    assert_eq!(unsafe { fc4p_00620ad0(p) }, p);
    assert_eq!(
        calls(),
        [
            Call::Interface(p as usize + 4),
            Call::Allocate(0x7C),
            Call::Allocate(0x80)
        ]
    );
    let mut expected = [0xA5u8; 0x60];
    for (offset, value) in [
        (0, 0xAA8314u32),
        (4, 0xAA82FC),
        (8, 0xCAFE_BABE),
        (0xC, 0),
        (0x10, a as u32),
        (0x14, 0),
        (0x1C, b as u32),
        (0x20, 0),
    ] {
        expected[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    expected[0x28..0x48].fill(0);
    expected[0x48..0x55].copy_from_slice(&[1, 0, 3, 1, 1, 0, 3, 1, 0, 0, 0, 0, 0]);
    assert_eq!(obj, expected);
    for (head, pointer) in [(&first[..], a), (&second[..], b)] {
        let mut expected = vec![0xC3u8; head.len()];
        expected[0] = 0;
        expected[4..8].fill(0);
        expected[8..12].copy_from_slice(&(pointer as u32).to_le_bytes());
        expected[12..16].copy_from_slice(&(pointer as u32).to_le_bytes());
        assert_eq!(head, expected);
    }
}
#[test]
fn service_shutdown_skips_inactive_clears_before_release_and_keeps_registers() {
    let table = [0usize, 0, release as *const () as usize];
    let mut service = [table.as_ptr() as usize];
    let mut obj = [0xA5u8; 0x60];
    let p = obj.as_mut_ptr();
    let view = unsafe { Preview::from_ptr(p) };
    for active in [0, 1, 0xFF] {
        for present in [false, true] {
            view.set_initialized(active);
            let service = if present {
                service.as_mut_ptr().cast()
            } else {
                core::ptr::null_mut()
            };
            view.set_service(service);
            STATE.with(|s| {
                *s.borrow_mut() = State {
                    preview: p as usize,
                    ..State::default()
                }
            });
            let before = obj;
            let result = invoke(fc4p_0061fbc0 as *const () as usize, p, &[]);
            let mut expected = before;
            if active != 0 {
                expected[0x54] = 0;
            }
            if active != 0 && present {
                expected[0xC..0x10].fill(0);
            }
            assert_eq!(obj, expected);
            if active != 0 && present {
                assert_eq!(result, (0xABCD_EF67, 0x1234_5678));
                assert_eq!(calls(), [Call::Release(service as usize, 0)]);
            } else {
                assert_eq!(result, (0xA1B2_C300 | u32::from(active), 0xB1C2_D3E4));
                assert!(calls().is_empty());
            }
        }
    }
}
#[test]
fn clear_rules_visits_first_then_second_and_resets_only_nonempty_headers() {
    let _a = fake(0x0061_FCD0, erase_a as *const () as usize);
    let _b = fake(0x0062_03A0, erase_b as *const () as usize);
    for nonempty_a in [false, true] {
        for nonempty_b in [false, true] {
            let mut obj = Object::new(0x60);
            let p = obj.ptr();
            let mut a = [0xA5A5_A5A5u32, 0x1234, 5, 6, 7];
            let mut b = a;
            let ap = a.as_mut_ptr().cast::<u8>();
            let bp = b.as_mut_ptr().cast::<u8>();
            let v = unsafe { Preview::from_ptr(p) };
            v.set_first_tree(ap);
            v.set_second_tree(bp);
            v.set_first_count(u32::from(nonempty_a));
            v.set_second_count(u32::from(nonempty_b));
            STATE.with(|s| *s.borrow_mut() = State::default());
            let result = invoke(fc4p_006208d0 as *const () as usize, p, &[]);
            let mut expected = vec![];
            if nonempty_a {
                expected.push(Call::Erase(p as usize + 0x10, 0x1234, false));
            }
            if nonempty_b {
                expected.push(Call::Erase(p as usize + 0x1C, 0x1234, true));
            }
            assert_eq!(calls(), expected);
            for (head, nonempty, ptr) in [(a, nonempty_a, ap), (b, nonempty_b, bp)] {
                assert_eq!(
                    head,
                    if nonempty {
                        [0xA5A5_A5A5, 0, ptr as u32, ptr as u32, 7]
                    } else {
                        [0xA5A5_A5A5, 0x1234, 5, 6, 7]
                    }
                );
            }
            assert_eq!((v.first_count(), v.second_count()), (0, 0));
            assert_eq!(
                result,
                (
                    if nonempty_b { bp as u32 } else { 0 },
                    if nonempty_b {
                        bp as u32
                    } else if nonempty_a {
                        ap as u32
                    } else {
                        0xB1C2_D3E4
                    }
                )
            );
        }
    }
}
#[test]
fn clear_reloads_the_header_replaced_by_the_erase_callback() {
    let _a = fake(0x0061_FCD0, erase_a as *const () as usize);
    let mut obj = Object::new(0x60);
    let p = obj.ptr();
    let mut a = [0u32, 0x1234, 5, 6];
    let mut b = [0u32, 0x5678, 7, 8];
    let ap = a.as_mut_ptr().cast::<u8>();
    let bp = b.as_mut_ptr().cast::<u8>();
    let v = unsafe { Preview::from_ptr(p) };
    v.set_first_tree(ap);
    v.set_first_count(1);
    STATE.with(|s| {
        *s.borrow_mut() = State {
            swap: Some((p as usize + 0x10, bp as usize)),
            ..State::default()
        }
    });
    assert_eq!(invoke(fc4p_006208d0 as *const () as usize, p, &[]).1, bp as u32);
    assert_eq!(a, [0, 0x1234, 5, 6]);
    assert_eq!(b, [0, 0, bp as u32, bp as u32]);
    assert_eq!(calls(), [Call::Erase(p as usize + 0x10, 0x1234, false)]);
}
#[test]
fn destruction_reverses_tree_order_frees_headers_then_releases_service_and_interface() {
    let _a = fake(0x0061_FCD0, erase_a as *const () as usize);
    let _b = fake(0x0062_03A0, erase_b as *const () as usize);
    let _c = fake(0x0090_CF63, free as *const () as usize);
    let _d = fake(0x0090_D990, destroy_interface as *const () as usize);
    for nonempty in [false, true] {
        for present in [false, true] {
            let mut obj = Object::new(0x60);
            let p = obj.ptr();
            let mut a = [0u32, 0x1234, 5, 6];
            let mut b = [0u32, 0x5678, 7, 8];
            let ap = a.as_mut_ptr().cast::<u8>();
            let bp = b.as_mut_ptr().cast::<u8>();
            let table = [0usize, 0, release as *const () as usize];
            let mut service = [table.as_ptr() as usize];
            let sp = service.as_mut_ptr().cast();
            let v = unsafe { Preview::from_ptr(p) };
            v.set_first_tree(if present { ap } else { core::ptr::null_mut() });
            v.set_second_tree(if present { bp } else { core::ptr::null_mut() });
            v.set_first_count(u32::from(present && nonempty));
            v.set_second_count(u32::from(present && nonempty));
            v.set_service(if present { sp } else { core::ptr::null_mut() });
            STATE.with(|s| {
                *s.borrow_mut() = State {
                    preview: p as usize,
                    ..State::default()
                }
            });
            assert_eq!(unsafe { fc4p_00620770(p) }, registers(0x1111_2222, 0x3333_4444));
            let mut expected = vec![];
            if present {
                if nonempty {
                    expected.push(Call::Erase(p as usize + 0x1C, 0x5678, true));
                }
                expected.push(Call::Free(bp as usize));
                if nonempty {
                    expected.push(Call::Erase(p as usize + 0x10, 0x1234, false));
                }
                expected.push(Call::Free(ap as usize));
                expected.push(Call::Release(sp as usize, sp as usize));
            }
            expected.push(Call::Interface(p as usize + 4));
            assert_eq!(calls(), expected);
            assert_eq!((v.primary_vtable(), v.secondary_vtable()), (0xAA8314, 0xAA82FC));
            assert_eq!(v.service(), if present { sp } else { core::ptr::null_mut() });
        }
    }
}
#[test]
fn deleting_destructor_low_flag_bit_and_secondary_adjustment() {
    let _a = fake(0x0062_0770, destroy as *const () as usize);
    let _b = fake(0x005E_5620, delete as *const () as usize);
    let mut object = Object::new(0x60);
    let p = object.ptr();
    for flags in [0u32, 1, 2, 3, 0x8000_0000, u32::MAX] {
        STATE.with(|s| *s.borrow_mut() = State::default());
        let expected = (p as u32, if flags & 1 != 0 { 0x7777_8888 } else { 0x3333_4444 });
        assert_eq!(
            invoke(fc4p_00620810 as *const () as usize, unsafe { p.add(4) }, &[flags]),
            expected
        );
        let mut order = vec![Call::Destroy(p as usize)];
        if flags & 1 != 0 {
            order.push(Call::Delete(p as usize));
        }
        assert_eq!(calls(), order);
    }
}
