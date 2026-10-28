// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Ordering city keys by the values in an owned map copy.

use super::*;
use crate::x87::F80;
use core::mem::MaybeUninit;

game_fn! { fn score(map: *mut u8, key: *const u32) -> u64 = "thiscall" 0x006A_3170; }
game_fn! { fn copy_map(out: *mut u8, source: *const u32) -> u64 = "thiscall" 0x006A_2DB0; }
game_fn! { fn move_words(to: *mut u32, from: *const u32, bytes: u32) -> u64 = "C" 0x009E_EB70; }
game_fn! {
    #[allow(dead_code)]
    fn destroy(map: *mut u8, root: *mut u8) -> u64 = "thiscall" 0x006A_25C0;
}
game_fn! {
    #[allow(dead_code)]
    fn free(header: *mut u8) -> u64 = "C" 0x0090_CF63;
}
game_fn! {
    #[allow(dead_code)]
    fn insert(pos: *mut u32, key: u32, header: u32, count: u32) -> u64 = "C" 0x006A_3470;
}
game_fn! {
    #[allow(dead_code)]
    fn insert_first(first: *mut u32, pos: *mut u32, key: u32, header: u32, count: u32) -> u64 = "C" 0x006A_6CD0;
}
game_fn! {
    #[allow(dead_code)]
    fn insertion(first: *mut u32, last: *mut u32, header: u32, count: u32) -> u64 = "C" 0x006A_7A40;
}
game_fn! {
    #[allow(dead_code)]
    fn insertion_tail(first: *mut u32, last: *mut u32, header: u32, count: u32) -> u64 = "C" 0x006A_7AF0;
}
game_fn! {
    #[allow(dead_code)]
    fn final_insertion(first: *mut u32, last: *mut u32, header: u32, count: u32) -> u64 = "C" 0x006A_7F90;
}
game_fn! {
    #[allow(dead_code)]
    fn median(a: *mut u32, b: *mut u32, c: *mut u32, header: u32, count: u32) -> u64 = "C" 0x006A_31F0;
}
game_fn! {
    #[allow(dead_code)]
    fn partition(a: *mut u32, b: *mut u32, key: u32, header: u32, count: u32) -> u64 = "C" 0x006A_3370;
}
game_fn! {
    #[allow(dead_code)]
    fn heap_sort(a: *mut u32, b: *mut u32, c: *mut u32, header: u32, count: u32) -> u64 = "C" 0x006A_8710;
}
game_fn! {
    #[allow(dead_code)]
    fn intro_sort(a: *mut u32, b: *mut u32, unused: u32, depth: u32, header: u32, count: u32) -> u64 = "C" 0x006A_8AE0;
}

/// Construct a by-value map in the argument area, then invoke the selected algorithm at its game entry.
unsafe fn with_copy(address: u32, map: *mut u32, args: &[u32]) -> u64 {
    unsafe {
        let mut words = [MaybeUninit::<u32>::uninit(); 7];
        let p = words.as_mut_ptr().cast::<u32>();
        for (i, &arg) in args.iter().enumerate() {
            write_unaligned(p.add(i), arg);
        }
        let copied = p.add(args.len());
        let copied_registers = copy_map(copied.cast(), map);
        // Only the two initialized map words are arguments; the third reserved word is unused.
        let arguments = core::slice::from_raw_parts(p, args.len() + 2);
        let mut call = RawCall::new(Conv::Cdecl, crate::abi::target(address), copied as u32, arguments);
        call.edx = (copied_registers >> 32) as u32;
        call_raw(&mut call);
        registers(call.eax, call.edx)
    }
}

/// Destroy the by-value map and reset its caller-owned count. Some callers retain the header in EDX.
enum DestroyEdx {
    Root,
    Header,
    Keep,
}

unsafe fn finish(map: *mut u32, mut edx: u32, header_in_edx: bool, input: DestroyEdx) -> u64 {
    unsafe {
        let header = read_unaligned(map) as *mut u8;
        if read_unaligned(map.add(1)) != 0 {
            let root = TreeNode::from_ptr(header).parent();
            let args = [root as u32];
            let mut call = RawCall::new(Conv::Thiscall, crate::abi::target(0x006A_25C0), map as u32, &args);
            call.edx = match input {
                DestroyEdx::Root => root as u32,
                DestroyEdx::Header => header as u32,
                DestroyEdx::Keep => edx,
            };
            call_raw(&mut call);
            edx = call.edx;
            TreeNode::from_ptr(read_unaligned(map) as *mut u8).set_left(read_unaligned(map) as *mut u8);
            let h = read_unaligned(map) as *mut u8;
            if header_in_edx {
                edx = h as u32;
            }
            TreeNode::from_ptr(h).set_parent(core::ptr::null_mut());
            TreeNode::from_ptr(read_unaligned(map) as *mut u8).set_right(read_unaligned(map) as *mut u8);
            write_unaligned(map.add(1), 0);
        }
        let h = read_unaligned(map);
        if h != 0 {
            let args = [h];
            let mut call = RawCall::new(Conv::Cdecl, crate::abi::target(0x0090_CF63), 0, &args);
            call.eax = h;
            call.edx = edx;
            call_raw(&mut call);
            registers(call.eax, call.edx)
        } else {
            registers(0, edx)
        }
    }
}

port! {
    /// Insert a key before the first element or use the sentinel-based insertion helper. Consume the map copy.
    #[port(address = 0x006A_6CD0, owner = "demand", name = "InsertKeyWithFirstSentinel", naked)]
    pub unsafe extern "C" fn fc4p_006a6cd0() {
        core::arch::naked_asm!(
            "push edx", "push eax", "mov edx, esp", "call {body}",
            "mov ecx, dword ptr [esp]", "add esp, 8", "ret", body = sym insert_with_first,
        )
    }
}
unsafe extern "fastcall" fn insert_with_first(this: *mut u8, entry: *mut Entry) -> u64 {
    unsafe {
        let e = &mut *entry;
        let first = e.arg(0) as *mut u32;
        let first_key = read_unaligned(first);
        let key = e.arg(2);
        e.put(0, key);
        let map = e.args.as_mut_ptr().add(3);
        let a = score(map.cast(), e.args.as_ptr());
        e.put(0, read_unaligned((a as u32 as *const u32).cast()));
        let b = score(map.cast(), &first_key);
        let mut edx = (b >> 32) as u32;
        if F80::from_f32(f32::from_bits(e.arg(0)))
            .compare(F80::from_f32(read_unaligned((b as u32 as *const u32).cast())))
            .less()
        {
            let bytes = e.arg(1).wrapping_sub(first as u32) as i32;
            if bytes > 0 {
                edx = (move_words(first.wrapping_add(1), first, bytes as u32) >> 32) as u32;
            }
            write_unaligned(first, key);
        } else {
            edx = (with_copy(0x006A_3470, map, &[e.arg(1), key]) >> 32) as u32;
        }
        e.eax = this as u32;
        finish(map, edx, false, DestroyEdx::Root)
    }
}

port! {
    /// Sort a range by repeated insertion, copying the score map for each insertion.
    #[port(address = 0x006A_7A40, owner = "demand", name = "InsertionSortCityKeys", naked)]
    pub unsafe extern "C" fn fc4p_006a7a40() {
        core::arch::naked_asm!("push edx", "push eax", "mov edx, esp", "call {body}", "add esp, 8", "ret", body = sym insertion_sort)
    }
}
unsafe extern "fastcall" fn insertion_sort(_this: *mut u8, entry: *mut Entry) -> u64 {
    unsafe {
        let e = &mut *entry;
        let (first, last) = (e.arg(0), e.arg(1));
        let map = e.args.as_mut_ptr().add(2);
        let mut edx = e.edx;
        if first != last {
            let mut pos = first.wrapping_add(4);
            while pos != last {
                edx = (with_copy(0x006A_6CD0, map, &[first, pos, read_unaligned(pos as *const u32)]) >> 32) as u32;
                pos = pos.wrapping_add(4);
            }
        }
        finish(
            map,
            edx,
            first == last,
            if first == last {
                DestroyEdx::Keep
            } else {
                DestroyEdx::Header
            },
        )
    }
}

port! {
    /// Sort the first sixteen keys with a lower bound, then finish the remaining range using that sentinel.
    #[port(address = 0x006A_7F90, owner = "demand", name = "FinishSortingCityKeys", naked)]
    pub unsafe extern "C" fn fc4p_006a7f90() {
        core::arch::naked_asm!("push edx", "push eax", "mov edx, esp", "call {body}", "add esp, 8", "ret", body = sym finish_sort)
    }
}
unsafe extern "fastcall" fn finish_sort(_this: *mut u8, entry: *mut Entry) -> u64 {
    unsafe {
        let e = &mut *entry;
        let (first, last) = (e.arg(0), e.arg(1));
        let map = e.args.as_mut_ptr().add(2);
        let result = if (last.wrapping_sub(first) & !3) as i32 > 64 {
            with_copy(0x006A_7A40, map, &[first, first.wrapping_add(64)]);
            with_copy(0x006A_7AF0, map, &[first.wrapping_add(64), last])
        } else {
            with_copy(0x006A_7A40, map, &[first, last])
        };
        finish(map, (result >> 32) as u32, true, DestroyEdx::Keep)
    }
}

port! {
    /// Partition ranges larger than sixteen keys, recursing on the upper half. At depth zero use heap sorting.
    #[port(address = 0x006A_8AE0, owner = "demand", name = "PartitionCityKeys", naked)]
    pub unsafe extern "C" fn fc4p_006a8ae0() {
        core::arch::naked_asm!("push edx", "push eax", "mov edx, esp", "call {body}", "add esp, 8", "ret", body = sym partition_sort)
    }
}
unsafe extern "fastcall" fn partition_sort(_this: *mut u8, entry: *mut Entry) -> u64 {
    unsafe {
        let e = &mut *entry;
        let first = e.arg(0);
        let mut last = e.arg(1);
        let mut depth = e.arg(3);
        let map = e.args.as_mut_ptr().add(4);
        let mut edx = e.edx;
        while (last.wrapping_sub(first) & !3) as i32 > 64 {
            if depth == 0 {
                edx = (with_copy(0x006A_8710, map, &[first, last, last]) >> 32) as u32;
                break;
            }
            depth = depth.wrapping_sub(1);
            let mut outer = [MaybeUninit::<u32>::uninit(); 3];
            copy_map(outer.as_mut_ptr().cast(), map);
            let elements = last.wrapping_sub(first) as i32 >> 2;
            let middle = first.wrapping_add(((elements - (elements >> 31)) >> 1).wrapping_mul(4) as u32);
            let selected = with_copy(0x006A_31F0, map, &[first, middle, last.wrapping_sub(4)]) as u32;
            let pivot = read_unaligned(selected as *const u32);
            let args = [first, last, pivot, outer[0].assume_init(), outer[1].assume_init()];
            let mut call = RawCall::new(Conv::Cdecl, crate::abi::target(0x006A_3370), 0, &args);
            call_raw(&mut call);
            let split = call.eax;
            edx = (with_copy(0x006A_8AE0, map, &[split, last, 0, depth]) >> 32) as u32;
            last = split;
        }
        finish(map, edx, false, DestroyEdx::Root)
    }
}

port! {
    /// Choose twice the binary logarithm as the partition depth, then finish with insertion sorting.
    #[port(address = 0x006A_8F40, owner = "demand", name = "SortCityKeys", naked)]
    pub unsafe extern "C" fn fc4p_006a8f40() {
        core::arch::naked_asm!("push edx", "push eax", "mov edx, esp", "call {body}", "add esp, 8", "ret", body = sym sort)
    }
}
unsafe extern "fastcall" fn sort(_this: *mut u8, entry: *mut Entry) -> u64 {
    unsafe {
        let e = &mut *entry;
        let (first, last) = (e.arg(0), e.arg(1));
        let map = e.args.as_mut_ptr().add(2);
        let mut edx = e.edx;
        if first != last {
            let mut n = last.wrapping_sub(first) as i32 >> 2;
            let mut depth = 0u32;
            while n != 1 {
                n >>= 1;
                depth = depth.wrapping_add(1);
            }
            with_copy(0x006A_8AE0, map, &[first, last, 0, depth.wrapping_mul(2)]);
            edx = (with_copy(0x006A_7F90, map, &[first, last]) >> 32) as u32;
        }
        finish(map, edx, true, DestroyEdx::Keep)
    }
}

pub(super) const PORTS: &[Port] = &[
    fc4p_006a6cd0::PORT,
    fc4p_006a7a40::PORT,
    fc4p_006a7f90::PORT,
    fc4p_006a8ae0::PORT,
    fc4p_006a8f40::PORT,
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demand::{interface::tests::Map, test_support::environment};
    use crate::testing::{fake, GameFpu};
    use std::cell::RefCell;

    #[derive(Debug, PartialEq)]
    enum Call {
        Score(u32),
        Copy(u32, u32),
        Move(u32, u32, u32),
        Algorithm(u32, Vec<u32>),
        Destroy(u32),
        Free(u32),
    }
    #[derive(Default)]
    #[allow(clippy::vec_box)]
    struct State {
        calls: Vec<Call>,
        scores: [f32; 8],
        copies: Vec<Box<[u32; 6]>>,
        split: u32,
        splits: Vec<u32>,
    }
    thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
    extern "thiscall" fn value(_map: *mut u8, key: *const u32) -> u64 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            let key = unsafe { *key };
            s.calls.push(Call::Score(key));
            registers(&mut s.scores[key as usize] as *mut f32 as u32, 0xAABB_CCDD)
        })
    }
    extern "thiscall" fn copying(out: *mut u8, source: *const u32) -> u64 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            let (h, n) = unsafe { (*source, *source.add(1)) };
            s.calls.push(Call::Copy(h, n));
            let mut copy = Box::new([0u32; 6]);
            let p = copy.as_mut_ptr() as u32;
            copy[2] = p;
            copy[3] = p;
            unsafe {
                write_unaligned(out.cast::<u32>(), p);
                write_unaligned(out.add(4).cast::<u32>(), 0)
            };
            s.copies.push(copy);
            registers(out as u32, 0)
        })
    }
    extern "C" fn moving(to: *mut u32, from: *const u32, bytes: u32) -> u64 {
        STATE.with(|s| s.borrow_mut().calls.push(Call::Move(to as u32, from as u32, bytes)));
        unsafe { core::ptr::copy(from, to, bytes as usize / 4) };
        registers(to as u32, 0x9988_7766)
    }
    extern "thiscall" fn destroying(_map: *mut u8, root: u32) -> u64 {
        STATE.with(|s| s.borrow_mut().calls.push(Call::Destroy(root)));
        0xBBAA_9988_7766_5544
    }
    // Retain the incoming EDX to expose which cleanup path produced it.
    #[unsafe(naked)]
    unsafe extern "C" fn freeing() {
        core::arch::naked_asm!("push edx","push dword ptr [esp + 8]","call {body}","add esp, 4","pop edx","ret",body=sym freed)
    }
    extern "C" fn freed(header: u32) -> u32 {
        STATE.with(|s| s.borrow_mut().calls.push(Call::Free(header)));
        0x1234_5678
    }
    fn algorithm(address: u32, args: &[u32]) -> u64 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.calls.push(Call::Algorithm(address, args.to_vec()));
            registers(s.split, 0xDEAD_BEEF)
        })
    }
    extern "C" fn insert_fake(a: u32, b: u32, _h: u32, _n: u32) -> u64 {
        algorithm(0x006A_3470, &[a, b])
    }
    extern "C" fn insertion_fake(a: u32, b: u32, _h: u32, _n: u32) -> u64 {
        algorithm(0x006A_7A40, &[a, b])
    }
    extern "C" fn tail_fake(a: u32, b: u32, _h: u32, _n: u32) -> u64 {
        algorithm(0x006A_7AF0, &[a, b])
    }
    extern "C" fn intro_fake(a: u32, b: u32, c: u32, d: u32, _h: u32, _n: u32) -> u64 {
        algorithm(0x006A_8AE0, &[a, b, c, d])
    }
    extern "C" fn final_fake(a: u32, b: u32, _h: u32, _n: u32) -> u64 {
        algorithm(0x006A_7F90, &[a, b])
    }
    extern "C" fn median_fake(a: u32, b: u32, c: u32, _h: u32, _n: u32) -> u64 {
        algorithm(0x006A_31F0, &[a, b, c]);
        registers(b, 0xDEAD_BEEF)
    }
    extern "C" fn partition_fake(a: u32, b: u32, key: u32, _h: u32, _n: u32) -> u64 {
        let result = algorithm(0x006A_3370, &[a, b, key]);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            if s.splits.is_empty() {
                result
            } else {
                registers(s.splits.remove(0), (result >> 32) as u32)
            }
        })
    }
    extern "C" fn heap_fake(a: u32, b: u32, c: u32, _h: u32, _n: u32) -> u64 {
        algorithm(0x006A_8710, &[a, b, c])
    }

    #[repr(C)]
    struct Frame {
        target: usize,
        args: [u32; 6],
        after: [u32; 6],
        eax: u32,
        edx: u32,
        ecx: u32,
        popped: u32,
    }
    #[unsafe(naked)]
    unsafe extern "thiscall" fn invoke(_p: *mut Frame) {
        core::arch::naked_asm!(
            "push ebp",
            "push ebx",
            "push esi",
            "push edi",
            "mov ebp, esp",
            "mov edi, ecx",
            "push dword ptr [edi + 24]",
            "push dword ptr [edi + 20]",
            "push dword ptr [edi + 16]",
            "push dword ptr [edi + 12]",
            "push dword ptr [edi + 8]",
            "push dword ptr [edi + 4]",
            "mov ebx, esp",
            "mov ecx, 0x8888",
            "mov eax, 0x11223344",
            "mov edx, 0x55667788",
            "call dword ptr [edi]",
            "mov dword ptr [edi + 52], eax",
            "mov dword ptr [edi + 56], edx",
            "mov dword ptr [edi + 60], ecx",
            "mov eax, esp",
            "sub eax, ebx",
            "mov dword ptr [edi + 64], eax",
            "mov ecx, 6",
            "lea esi, [edi + 28]",
            "2:",
            "mov eax, dword ptr [ebx]",
            "mov dword ptr [esi], eax",
            "add ebx, 4",
            "add esi, 4",
            "loop 2b",
            "mov esp, ebp",
            "pop edi",
            "pop esi",
            "pop ebx",
            "pop ebp",
            "ret",
        )
    }
    fn run(target: *const (), args: [u32; 6]) -> Frame {
        let mut f = Frame {
            target: target as usize,
            args,
            after: [0; 6],
            eax: 0,
            edx: 0,
            ecx: 0,
            popped: 99,
        };
        environment(|| unsafe { invoke(&mut f) });
        assert_eq!(f.popped, 0);
        f
    }
    fn calls() -> Vec<Call> {
        STATE.with(|s| std::mem::take(&mut s.borrow_mut().calls))
    }
    fn reset() {
        STATE.with(|s| *s.borrow_mut() = State::default())
    }

    #[test]
    fn first_insertion_shifts_or_delegates_and_writes_the_score_into_the_first_argument() {
        let _fpu = GameFpu::new();
        let _a = fake(0x006A_3170, value as *const () as usize);
        let _b = fake(0x006A_2DB0, copying as *const () as usize);
        let _c = fake(0x009E_EB70, moving as *const () as usize);
        let _d = fake(0x006A_3470, insert_fake as *const () as usize);
        let _e = fake(0x006A_25C0, destroying as *const () as usize);
        let _f = fake(0x0090_CF63, freeing as *const () as usize);
        for score in [-1.0, 0.0, 1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            for distance in [0, 4, 12] {
                for count in [0, 3] {
                    reset();
                    STATE.with(|s| s.borrow_mut().scores = [0.0, score, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
                    let mut m = Map::new(&[(1, 0), (2, 0), (3, 0)]);
                    let h = m.header() as u32;
                    let root = unsafe { TreeNode::from_ptr(h as *mut u8).parent() } as u32;
                    let mut keys = [0, 2, 3, 4];
                    let first = keys.as_mut_ptr() as u32;
                    let f = run(
                        fc4p_006a6cd0 as *const (),
                        [first, first + distance, 1, h, count, 0xCAFE_BABE],
                    );
                    assert_eq!(f.after, [score.to_bits(), first + distance, 1, h, 0, 0xCAFE_BABE]);
                    assert_eq!(f.ecx, 0x8888);
                    let mut expected = vec![Call::Score(1), Call::Score(0)];
                    let edx = if score < 0.0 {
                        if distance != 0 {
                            expected.push(Call::Move(first + 4, first, distance));
                        }
                        let mut want = [0, 2, 3, 4];
                        if distance != 0 {
                            want.copy_within(..distance as usize / 4, 1);
                        }
                        want[0] = 1;
                        assert_eq!(keys, want);
                        if distance == 0 {
                            0xAABB_CCDD
                        } else {
                            0x9988_7766
                        }
                    } else {
                        expected.extend([
                            Call::Copy(h, count),
                            Call::Algorithm(0x006A_3470, vec![first + distance, 1]),
                        ]);
                        0xDEAD_BEEF
                    };
                    if count != 0 {
                        expected.push(Call::Destroy(root));
                    }
                    expected.push(Call::Free(h));
                    assert_eq!(calls(), expected);
                    assert_eq!(
                        (f.eax, f.edx),
                        (0x1234_5678, if count == 0 { edx } else { 0xBBAA_9988 })
                    );
                    if count != 0 {
                        let h = unsafe { TreeNode::from_ptr(h as *mut u8) };
                        assert_eq!(
                            (h.parent() as u32, h.left() as u32, h.right() as u32),
                            (0, h.as_ptr() as u32, h.as_ptr() as u32)
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn insertion_sort_orders_real_keys_and_destroys_each_independent_copy() {
        let _fpu = GameFpu::new();
        let _a = fake(0x006A_3170, value as *const () as usize);
        let _b = fake(0x006A_2DB0, copying as *const () as usize);
        let _c = fake(0x009E_EB70, moving as *const () as usize);
        let _d = fake(0x0090_CF63, freeing as *const () as usize);
        let _e = fake(0x006A_25C0, destroying as *const () as usize);
        for length in 0..=8 {
            reset();
            STATE.with(|s| s.borrow_mut().scores = [7.0, 6.0, 5.0, 4.0, 3.0, 2.0, 1.0, 0.0]);
            let mut keys = [0, 1, 2, 3, 4, 5, 6, 7];
            let first = keys.as_mut_ptr() as u32;
            let mut m = Map::new(&[]);
            let h = m.header() as u32;
            let f = run(
                fc4p_006a7a40 as *const (),
                [first, first + length * 4, h, 0, 0xABCD, 0xEFAB],
            );
            let mut expected = [0, 1, 2, 3, 4, 5, 6, 7];
            expected[..length as usize].reverse();
            assert_eq!(keys, expected);
            assert_eq!(f.after, f.args);
            let c = calls();
            assert_eq!(
                c.iter().filter(|c| matches!(c, Call::Copy(..))).count(),
                length.saturating_sub(1) as usize
            );
            assert_eq!(c.last(), Some(&Call::Free(h)));
        }
    }

    #[test]
    fn final_insertion_splits_only_ranges_above_sixteen_elements() {
        let _a = fake(0x006A_2DB0, copying as *const () as usize);
        let _b = fake(0x006A_7A40, insertion_fake as *const () as usize);
        let _c = fake(0x006A_7AF0, tail_fake as *const () as usize);
        let _d = fake(0x006A_25C0, destroying as *const () as usize);
        let _e = fake(0x0090_CF63, freeing as *const () as usize);
        for bytes in [0, 4, 60, 64, 65, 67, 68, 100, 0xFFFF_FFFC] {
            for count in [0, 2] {
                reset();
                let mut m = Map::new(&[(1, 1)]);
                let h = m.header() as u32;
                let root = unsafe { TreeNode::from_ptr(h as *mut u8).parent() } as u32;
                let f = run(
                    fc4p_006a7f90 as *const (),
                    [0x1000, 0x1000u32.wrapping_add(bytes), h, count, 9, 10],
                );
                let mut expected = vec![Call::Copy(h, count)];
                if (bytes & !3) as i32 > 64 {
                    expected.extend([
                        Call::Algorithm(0x006A_7A40, vec![0x1000, 0x1040]),
                        Call::Copy(h, count),
                        Call::Algorithm(0x006A_7AF0, vec![0x1040, 0x1000 + bytes]),
                    ]);
                } else {
                    expected.push(Call::Algorithm(
                        0x006A_7A40,
                        vec![0x1000, 0x1000u32.wrapping_add(bytes)],
                    ));
                }
                if count != 0 {
                    expected.push(Call::Destroy(root));
                }
                expected.push(Call::Free(h));
                assert_eq!(calls(), expected);
                assert_eq!(f.after, [f.args[0], f.args[1], h, 0, 9, 10]);
                assert_eq!(f.edx, if count == 0 { 0xDEAD_BEEF } else { h });
            }
        }
    }

    #[test]
    fn partition_depth_zero_uses_heap_and_small_ranges_only_destroy_their_map() {
        let _a = fake(0x006A_2DB0, copying as *const () as usize);
        let _b = fake(0x006A_8710, heap_fake as *const () as usize);
        let _c = fake(0x006A_25C0, destroying as *const () as usize);
        let _d = fake(0x0090_CF63, freeing as *const () as usize);
        for bytes in [0, 4, 64, 67, 68, 100] {
            for count in [0, 2] {
                reset();
                let mut m = Map::new(&[(1, 1)]);
                let h = m.header() as u32;
                let root = unsafe { TreeNode::from_ptr(h as *mut u8).parent() } as u32;
                let f = run(
                    fc4p_006a8ae0 as *const (),
                    [0x1000, 0x1000 + bytes, 0xABCD, 0, h, count],
                );
                let mut expected = vec![];
                if bytes & !3 > 64 {
                    expected.extend([
                        Call::Copy(h, count),
                        Call::Algorithm(0x006A_8710, vec![0x1000, 0x1000 + bytes, 0x1000 + bytes]),
                    ]);
                }
                if count != 0 {
                    expected.push(Call::Destroy(root));
                }
                expected.push(Call::Free(h));
                assert_eq!(calls(), expected);
                assert_eq!(f.after[5], 0);
                assert_eq!(
                    f.edx,
                    if count != 0 {
                        0xBBAA_9988
                    } else if bytes & !3 > 64 {
                        0xDEAD_BEEF
                    } else {
                        0x5566_7788
                    }
                );
            }
        }
    }

    #[test]
    fn partition_selects_the_middle_key_recurses_right_and_iterates_left() {
        let _a = fake(0x006A_2DB0, copying as *const () as usize);
        let _b = fake(0x006A_31F0, median_fake as *const () as usize);
        let _c = fake(0x006A_3370, partition_fake as *const () as usize);
        let _d = fake(0x006A_8AE0, intro_fake as *const () as usize);
        let _e = fake(0x0090_CF63, freeing as *const () as usize);
        reset();
        let mut keys = [7u32; 33];
        keys[16] = 0xCAFE;
        let first = keys.as_mut_ptr() as u32;
        let last = first + 132;
        STATE.with(|s| s.borrow_mut().split = first + 64);
        let mut m = Map::new(&[]);
        let h = m.header() as u32;
        run(fc4p_006a8ae0 as *const (), [first, last, 0xABCD, 5, h, 0]);
        assert_eq!(
            calls(),
            vec![
                Call::Copy(h, 0),
                Call::Copy(h, 0),
                Call::Algorithm(0x006A_31F0, vec![first, first + 64, last - 4]),
                Call::Algorithm(0x006A_3370, vec![first, last, 0xCAFE]),
                Call::Copy(h, 0),
                Call::Algorithm(0x006A_8AE0, vec![first + 64, last, 0, 4]),
                Call::Free(h)
            ]
        );

        reset();
        let mut keys = [7u32; 65];
        let first = keys.as_mut_ptr() as u32;
        let last = first + 260;
        STATE.with(|s| s.borrow_mut().splits = vec![first + 128, first + 64]);
        run(fc4p_006a8ae0 as *const (), [first, last, 0xABCD, 5, h, 0]);
        assert_eq!(
            calls(),
            vec![
                Call::Copy(h, 0),
                Call::Copy(h, 0),
                Call::Algorithm(0x006A_31F0, vec![first, first + 128, last - 4]),
                Call::Algorithm(0x006A_3370, vec![first, last, 7]),
                Call::Copy(h, 0),
                Call::Algorithm(0x006A_8AE0, vec![first + 128, last, 0, 4]),
                Call::Copy(h, 0),
                Call::Copy(h, 0),
                Call::Algorithm(0x006A_31F0, vec![first, first + 64, first + 124]),
                Call::Algorithm(0x006A_3370, vec![first, first + 128, 7]),
                Call::Copy(h, 0),
                Call::Algorithm(0x006A_8AE0, vec![first + 64, first + 128, 0, 3]),
                Call::Free(h)
            ]
        );
    }

    #[test]
    fn empty_and_single_key_insertions_clear_owned_maps_without_copying() {
        let _a = fake(0x006A_25C0, destroying as *const () as usize);
        let _b = fake(0x0090_CF63, freeing as *const () as usize);
        for length in [0, 4] {
            reset();
            let mut map = Map::new(&[(1, 1)]);
            let header = map.header() as u32;
            let root = unsafe { TreeNode::from_ptr(header as *mut u8).parent() } as u32;
            let f = run(fc4p_006a7a40 as *const (), [0x1000, 0x1000 + length, header, 1, 99, 88]);
            assert_eq!(calls(), vec![Call::Destroy(root), Call::Free(header)]);
            assert_eq!(f.after, [0x1000, 0x1000 + length, header, 0, 99, 88]);
            assert_eq!(f.edx, if length == 0 { header } else { 0xBBAA_9988 });
        }
    }

    #[test]
    fn a_null_subtree_retains_the_header_register_in_a_nonempty_insertion_range() {
        #[unsafe(naked)]
        unsafe extern "thiscall" fn empty_subtree() {
            core::arch::naked_asm!("ret 4");
        }
        let _a = fake(0x006A_25C0, empty_subtree as *const () as usize);
        let _b = fake(0x0090_CF63, freeing as *const () as usize);
        reset();
        let mut map = Map::new(&[]);
        let header = map.header() as u32;
        let f = run(fc4p_006a7a40 as *const (), [0x1000, 0x1004, header, 1, 99, 88]);
        assert_eq!(f.edx, header);
        assert_eq!(f.after, [0x1000, 0x1004, header, 0, 99, 88]);
        assert_eq!(calls(), vec![Call::Free(header)]);
    }

    #[test]
    fn sort_computes_twice_the_floor_logarithm_and_skips_both_passes_for_empty_ranges() {
        let _a = fake(0x006A_2DB0, copying as *const () as usize);
        let _b = fake(0x006A_8AE0, intro_fake as *const () as usize);
        let _c = fake(0x006A_7F90, final_fake as *const () as usize);
        let _d = fake(0x006A_25C0, destroying as *const () as usize);
        let _e = fake(0x0090_CF63, freeing as *const () as usize);
        for n in [0u32, 1, 2, 3, 4, 15, 16, 17, 31, 32, 33, 1000] {
            for count in [0, 2] {
                reset();
                let mut m = Map::new(&[(1, 1)]);
                let h = m.header() as u32;
                let root = unsafe { TreeNode::from_ptr(h as *mut u8).parent() } as u32;
                let f = run(fc4p_006a8f40 as *const (), [0x1000, 0x1000 + n * 4, h, count, 99, 88]);
                let mut expected = vec![];
                if n != 0 {
                    expected.extend([
                        Call::Copy(h, count),
                        Call::Algorithm(
                            0x006A_8AE0,
                            vec![0x1000, 0x1000 + n * 4, 0, (31 - n.leading_zeros()) * 2],
                        ),
                        Call::Copy(h, count),
                        Call::Algorithm(0x006A_7F90, vec![0x1000, 0x1000 + n * 4]),
                    ]);
                }
                if count != 0 {
                    expected.push(Call::Destroy(root));
                }
                expected.push(Call::Free(h));
                assert_eq!(calls(), expected);
                assert_eq!(f.after, [0x1000, 0x1000 + n * 4, h, 0, 99, 88]);
                assert_eq!(
                    f.edx,
                    if count != 0 {
                        h
                    } else if n == 0 {
                        0x5566_7788
                    } else {
                        0xDEAD_BEEF
                    }
                );
            }
        }
        reset();
        let f = run(fc4p_006a8f40 as *const (), [0x1000, 0x1000, 0, 0, 99, 88]);
        assert!(calls().is_empty());
        assert_eq!((f.eax, f.edx), (0, 0x5566_7788));
    }
}
