// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Ordinance selection vectors from the core's ordinance_effect_lists test, with explicit game calls.

use super::*;
use crate::testing::{fake, Fake, Object};
use std::cell::RefCell;
use std::collections::VecDeque;

/// Call the exports and copy their caller-owned argument words immediately after the callee pop.
unsafe fn invoke(function: usize, this: *mut u8, args: &[u32]) -> (u64, Vec<u32>) {
    let mut words = vec![0; args.len()];
    let mut frame = [
        function as u32,
        this as u32,
        args.as_ptr() as u32,
        args.len() as u32,
        words.as_mut_ptr() as u32,
        0,
        0,
    ];
    unsafe {
        core::arch::asm!(
            "mov ecx,[edi+12]","mov eax,[edi+8]",
            "2:","test ecx,ecx","jz 3f","dec ecx","push dword ptr [eax+ecx*4]","jmp 2b",
            "3:","mov dword ptr [esp-0x68],0xDEADBE55","mov ecx,[edi+4]","call dword ptr [edi]",
            "mov [edi+20],eax","mov [edi+24],edx",
            "mov ecx,[edi+12]","mov eax,ecx","shl eax,2","mov edx,esp","sub edx,eax","mov eax,[edi+16]",
            "4:","test ecx,ecx","jz 5f","dec ecx","mov ebx,[edx+ecx*4]","mov [eax+ecx*4],ebx","jmp 4b","5:",
            in("edi") frame.as_mut_ptr(),out("ebx") _,clobber_abi("C"),
        );
    }
    ((u64::from(frame[6]) << 32) | u64::from(frame[5]), words)
}
unsafe fn select(this: *mut u8, occupant: *mut u8, kind: u32, source: *mut *mut u8, out: *mut *mut u8) -> u32 {
    let expected = unsafe { *source };
    let (result, words) = unsafe {
        invoke(
            super::fc4p_006fd070 as *const () as usize,
            this,
            &[occupant as u32, kind, source as u32, out as u32],
        )
    };
    assert_eq!(words, [occupant as u32, kind, expected as u32, out as u32]);
    result as u32
}
unsafe fn copy(
    this: *mut u8,
    occupant: *mut u8,
    kind: u32,
    source: *mut *mut u8,
    first: *mut *mut u8,
    second: *mut *mut u8,
) -> u32 {
    let expected = unsafe { *source };
    let (result, words) = unsafe {
        invoke(
            super::fc4p_006fcd70 as *const () as usize,
            this,
            &[occupant as u32, kind, source as u32, first as u32, second as u32],
        )
    };
    assert_eq!(
        words,
        [occupant as u32, kind, expected as u32, first as u32, second as u32]
    );
    result as u32
}

#[derive(Debug, PartialEq)]
enum Call {
    ClearEffects(usize),
    ClearCurves(usize),
    Affects(usize, usize, usize, u32),
    Allocate(u32),
    CopyEffect(usize, usize),
    CopyCurve(usize, usize),
    Game(&'static str, Vec<u32>),
}

mod coverage;

#[derive(Default)]
struct State {
    calls: Vec<Call>,
    allocations: Vec<Object>,
    accepted: VecDeque<u32>,
    clear_swap: Option<(usize, usize)>,
    allocate_swap: Option<(usize, usize)>,
}
thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}
fn log(call: Call) {
    STATE.with(|s| s.borrow_mut().calls.push(call));
}
fn calls() -> Vec<Call> {
    STATE.with(|s| std::mem::take(&mut s.borrow_mut().calls))
}

unsafe fn reset_links(list: *mut *mut u8) {
    unsafe {
        let head = *list;
        Link::from_ptr(head).set_next(head);
        Link::from_ptr(head).set_previous(head);
        STATE.with(|s| {
            if let Some((location, value)) = s.borrow_mut().clear_swap.take() {
                (location as *mut *mut u8).write(value as *mut u8);
            }
        });
    }
}
unsafe extern "thiscall" fn effects_clear(list: *mut *mut u8) -> u32 {
    log(Call::ClearEffects(list as usize));
    unsafe { reset_links(list) };
    0x1234_5678
}
unsafe extern "thiscall" fn curves_clear(list: *mut *mut u8) -> u32 {
    log(Call::ClearCurves(list as usize));
    unsafe { reset_links(list) };
    0x1234_5678
}
extern "thiscall" fn affects(this: *mut u8, occupant: *mut u8, effect: *mut u8, kind: u32) -> u32 {
    log(Call::Affects(this as usize, occupant as usize, effect as usize, kind));
    STATE.with(|s| s.borrow_mut().accepted.pop_front().unwrap())
}
extern "C" fn alloc(size: u32) -> *mut u8 {
    log(Call::Allocate(size));
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let mut allocation = Object::new(size as usize);
        let p = allocation.ptr();
        unsafe { p.write_bytes(0xA5, size as usize) };
        s.allocations.push(allocation);
        if let Some((location, value)) = s.allocate_swap.take() {
            unsafe { (location as *mut *mut u8).write(value as *mut u8) };
        }
        p
    })
}
unsafe extern "thiscall" fn effect_copy(this: *mut u8, source: *mut u8) -> u32 {
    log(Call::CopyEffect(this as usize, source as usize));
    unsafe { core::ptr::copy_nonoverlapping(source, this, 0x34) };
    0
}
unsafe extern "thiscall" fn curve_copy(this: *mut u8, source: *mut u8) -> u32 {
    log(Call::CopyCurve(this as usize, source as usize));
    unsafe { core::ptr::copy_nonoverlapping(source, this, 16) };
    0
}

struct List {
    head: Box<*mut u8>,
    storage: Vec<Object>,
}
impl List {
    fn new(payloads: &[u8]) -> Self {
        let mut sentinel = Object::new(0x3C);
        let p = sentinel.ptr();
        let mut list = Self {
            head: Box::new(p),
            storage: vec![sentinel],
        };
        unsafe {
            let head = Link::from_ptr(p);
            head.set_next(p);
            head.set_previous(p);
            for &byte in payloads {
                let mut node = Object::new(0x3C);
                let n = node.ptr();
                n.add(8).write_bytes(byte, 0x34);
                let tail = head.previous();
                Link::from_ptr(n).set_next(p);
                Link::from_ptr(n).set_previous(tail);
                Link::from_ptr(tail).set_next(n);
                head.set_previous(n);
                list.storage.push(node);
            }
        }
        list
    }
    fn ptr(&mut self) -> *mut *mut u8 {
        &mut *self.head
    }
    fn nodes(&self) -> Vec<*mut u8> {
        let mut out = Vec::new();
        unsafe {
            let mut n = Link::from_ptr(*self.head).next();
            let mut previous = *self.head;
            while n != *self.head {
                assert!(out.len() < 20, "list closes at its sentinel");
                assert_eq!(Link::from_ptr(n).previous(), previous);
                out.push(n);
                previous = n;
                n = Link::from_ptr(n).next();
            }
            assert_eq!(Link::from_ptr(*self.head).previous(), previous);
        }
        out
    }
}
fn setup() -> Vec<Fake> {
    STATE.with(|s| *s.borrow_mut() = State::default());
    vec![
        fake(0x006F_CB40, effects_clear as *const () as usize),
        fake(0x006F_CBD0, curves_clear as *const () as usize),
        fake(0x006F_94F0, affects as *const () as usize),
        fake(0x006F_B9F0, effect_copy as *const () as usize),
        fake(0x005F_4430, curve_copy as *const () as usize),
        fake(0x0090_CF54, alloc as *const () as usize),
    ]
}

#[test]
fn ordinance_selection_preserves_order_and_tests_only_the_low_byte() {
    let _fakes = setup();
    let mut source = List::new(&[0x11, 0x22, 0x33]);
    let src = source.nodes();
    let mut out = List::new(&[0x44]);
    let mut sim = Object::new(0x3C4);
    let mut building = Object::new(4);
    let (s, b, input, output) = (sim.ptr(), building.ptr(), source.ptr(), out.ptr());
    STATE.with(|s| s.borrow_mut().accepted.extend([0x1234_5601, 0xABCD_0000, 0x8080_80FF]));
    unsafe { select(s, b, 0xA92D_8E40, input, output) };
    let dst = out.nodes();
    assert_eq!(dst.len(), 2);
    unsafe {
        assert_eq!(core::slice::from_raw_parts(dst[0].add(8), 0x34), &[0x11; 0x34]);
        assert_eq!(core::slice::from_raw_parts(dst[1].add(8), 0x34), &[0x33; 0x34]);
    }
    assert_eq!(
        calls(),
        vec![
            Call::ClearEffects(output as usize),
            Call::Affects(s as usize, b as usize, src[0] as usize + 8, 0xA92D_8E40),
            Call::Allocate(0x3C),
            Call::CopyEffect(dst[0] as usize + 8, src[0] as usize + 8),
            Call::Affects(s as usize, b as usize, src[1] as usize + 8, 0xA92D_8E40),
            Call::Affects(s as usize, b as usize, src[2] as usize + 8, 0xA92D_8E40),
            Call::Allocate(0x3C),
            Call::CopyEffect(dst[1] as usize + 8, src[2] as usize + 8),
        ]
    );
    assert_eq!(source.nodes(), src);
}

#[test]
fn empty_and_rejected_ordinance_lists_still_clear_the_output() {
    for accepted in [None, Some(0xCAFE_0000)] {
        let _fakes = setup();
        let mut source = List::new(if accepted.is_some() { &[1] } else { &[] });
        let src = source.nodes();
        let mut out = List::new(&[2]);
        let (input, output) = (source.ptr(), out.ptr());
        if let Some(value) = accepted {
            STATE.with(|s| s.borrow_mut().accepted.push_back(value));
        }
        let result = unsafe { select(core::ptr::null_mut(), core::ptr::null_mut(), 7, input, output) };
        assert!(out.nodes().is_empty());
        let mut expected = vec![Call::ClearEffects(output as usize)];
        if let Some(value) = accepted {
            expected.push(Call::Affects(0, 0, src[0] as usize + 8, 7));
            assert_eq!(result, value);
        } else {
            assert_eq!(result, input as u32);
        }
        assert_eq!(calls(), expected);
    }
}

#[test]
fn curve_lists_copy_pairs_in_source_order_after_clearing_both() {
    let _fakes = setup();
    let mut source = List::new(&[0x12, 0x34]);
    let src = source.nodes();
    let mut first = List::new(&[0x56]);
    let mut second = List::new(&[0x78]);
    let (input, a, b) = (source.ptr(), first.ptr(), second.ptr());
    let result = unsafe { copy(core::ptr::null_mut(), core::ptr::null_mut(), 0, input, a, b) };
    assert_eq!(result, *source.head as u32);
    let (an, bn) = (first.nodes(), second.nodes());
    assert_eq!((an.len(), bn.len()), (2, 2));
    let mut expected = vec![Call::ClearCurves(a as usize), Call::ClearCurves(b as usize)];
    for i in 0..2 {
        expected.extend([
            Call::Allocate(0x18),
            Call::CopyCurve(an[i] as usize + 8, src[i] as usize + 0x0C),
            Call::Allocate(0x18),
            Call::CopyCurve(bn[i] as usize + 8, src[i] as usize + 0x24),
        ]);
        unsafe {
            assert_eq!(
                core::slice::from_raw_parts(an[i].add(8), 16),
                &[if i == 0 { 0x12 } else { 0x34 }; 16]
            );
            assert_eq!(
                core::slice::from_raw_parts(bn[i].add(8), 16),
                &[if i == 0 { 0x12 } else { 0x34 }; 16]
            );
        }
    }
    assert_eq!(calls(), expected);
}

#[test]
fn empty_curve_source_clears_both_and_retains_the_second_clear_result() {
    let _fakes = setup();
    let mut source = List::new(&[]);
    let mut first = List::new(&[1]);
    let mut second = List::new(&[2]);
    let (input, a, b) = (source.ptr(), first.ptr(), second.ptr());
    assert_eq!(
        unsafe { copy(core::ptr::null_mut(), core::ptr::null_mut(), 0, input, a, b) },
        0x1234_5678
    );
    assert!(first.nodes().is_empty());
    assert!(second.nodes().is_empty());
    assert_eq!(calls(), [Call::ClearCurves(a as usize), Call::ClearCurves(b as usize)]);
}

#[test]
fn curve_source_is_captured_before_clearing_and_output_head_before_allocating() {
    let _fakes = setup();
    let mut source = List::new(&[0x42]);
    let src = source.nodes()[0];
    let replacement = List::new(&[]);
    let mut first = List::new(&[]);
    let mut second = List::new(&[]);
    let alternate = List::new(&[]);
    let original_first = *first.head;
    let (input, a, b) = (source.ptr(), first.ptr(), second.ptr());
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.clear_swap = Some((input as usize, *replacement.head as usize));
        s.allocate_swap = Some((a as usize, *alternate.head as usize));
    });
    unsafe { copy(core::ptr::null_mut(), core::ptr::null_mut(), 0, input, a, b) };
    assert_eq!(second.nodes().len(), 1);
    assert!(alternate.nodes().is_empty());
    unsafe {
        let inserted = Link::from_ptr(original_first).next();
        assert_ne!(inserted, original_first);
        assert_eq!(*inserted.add(8), 0x42);
    }
    assert!(calls()
        .iter()
        .any(|c| matches!(c,Call::CopyCurve(_,p) if *p==src as usize+0x0C)));
    // Restore the source handle so the original source links can still be checked.
    *source.head = source.storage[0].ptr();
    assert_eq!(source.nodes(), [src]);
}
