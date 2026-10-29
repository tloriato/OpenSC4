// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, Fake};
use std::cell::RefCell;

const EAX: u32 = 0xAABB_CCDD;
const EDX: u32 = 0x1234_5678;
const FREE_EAX: u32 = 0xCAFE_2468;
const FREE_EDX: u32 = 0xDEAD_1357;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Free(u32),
    Copy(u32, u32, u32, u32, u32),
    Assign(u32, u32),
    Resize(u32, u32),
    Allocate(u32),
    Construct(u32, u32),
}
#[derive(Default)]
struct State {
    calls: Vec<Call>,
    copy_end: u32,
    scratch_value: u32,
    mutation: Option<(usize, usize, u32)>,
    allocation: u32,
    resize_buckets: Option<(u32, u32)>,
    table: u32,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn reset() {
    STATE.with(|s| *s.borrow_mut() = State::default());
}
fn take_calls() -> Vec<Call> {
    STATE.with(|s| core::mem::take(&mut s.borrow_mut().calls))
}

extern "C" fn free(p: u32) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Free(p));
        if let Some((object, offset, value)) = s.mutation.take() {
            unsafe { write(object as *mut u8, offset, value) };
        }
    });
    registers(FREE_EAX, FREE_EDX)
}

fn free_fake() -> Fake {
    fake(0x0090_CF63, free as *const () as usize)
}

unsafe fn invoke(entry: *const (), this: *mut u8, args: &[u32], convention: Conv) -> RawCall {
    let mut c = RawCall::new(convention, entry as usize, this as u32, args);
    c.eax = EAX;
    c.edx = EDX;
    unsafe { call_registers(&mut c) };
    c
}

#[test]
fn vector_destructors_free_members_in_order_then_reread_storage_without_clearing_fields() {
    let _f = free_fake();
    for (entry, stride, member) in [
        (fc4p_0062be40 as *const (), 0x28, 0x1C),
        (fc4p_0062bf20 as *const (), 0x50, 0x44),
    ] {
        reset();
        let mut bytes = vec![0xA5u8; stride * 3];
        let begin = bytes.as_mut_ptr();
        unsafe {
            write(begin, member, 0x1111u32);
            write(begin, stride + member, 0u32);
            write(begin, 2 * stride + member, 0x3333u32);
        }
        let before = bytes.clone();
        let mut vector = [begin as u32, begin as u32 + bytes.len() as u32, 0xFEED_BEEF];
        let vp = vector.as_mut_ptr().cast::<u8>();
        STATE.with(|s| s.borrow_mut().mutation = Some((vp as usize, 0, 0x5555)));
        let c = unsafe { invoke(entry, vp, &[], Conv::Thiscall) };
        assert_eq!((c.eax, c.edx), (FREE_EAX, FREE_EDX));
        assert_eq!(
            take_calls(),
            [Call::Free(0x1111), Call::Free(0x3333), Call::Free(0x5555)]
        );
        assert_eq!(bytes, before);
        assert_eq!(vector, [0x5555, begin as u32 + bytes.len() as u32, 0xFEED_BEEF]);

        for begin in [0u32, 0x1234] {
            reset();
            let mut vector = [begin, begin, 0x1122_3344];
            let c = unsafe { invoke(entry, vector.as_mut_ptr().cast(), &[], Conv::Thiscall) };
            let expected = if begin == 0 { (EAX, EDX) } else { (FREE_EAX, FREE_EDX) };
            assert_eq!((c.eax, c.edx), expected);
            assert_eq!(take_calls(), if begin == 0 { vec![] } else { vec![Call::Free(begin)] });
            assert_eq!(vector, [begin, begin, 0x1122_3344]);
        }
    }
}

extern "C" fn copy(first: u32, last: u32, output: u32, scratch: *mut u32, flags: u32) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls
            .push(Call::Copy(first, last, output, unsafe { *scratch }, flags));
        unsafe { *scratch = s.scratch_value };
        registers(s.copy_end, 0x5678_9876)
    })
}

#[test]
fn erase_passes_all_five_copy_arguments_and_returns_the_modified_argument_only_for_a_tail() {
    let _f = free_fake();
    let _c = fake(0x0062_7220, copy as *const () as usize);
    for tail in [0usize, 1, 3] {
        reset();
        let mut bytes = [0xA5u8; 0x28 * 4];
        let begin = bytes.as_mut_ptr();
        let end = begin as u32 + bytes.len() as u32;
        let new_end = end - (tail * 0x28) as u32;
        for i in 0..4 {
            unsafe { write(begin, i * 0x28 + 0x1C, if i == 2 { 0 } else { 0x1000 + i as u32 }) };
        }
        let before = bytes;
        let mut vector = [begin as u32, end, end + 0x80];
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.copy_end = new_end;
            s.scratch_value = 0xABCD_4321;
        });
        let c = unsafe {
            invoke(
                fc4p_0062be80 as *const (),
                vector.as_mut_ptr().cast(),
                &[begin as u32, begin as u32 + 0x28],
                Conv::Thiscall,
            )
        };
        assert_eq!(c.eax, if tail == 0 { begin as u32 } else { 0xABCD_4321 });
        let mut expected = vec![Call::Copy(begin as u32 + 0x28, end, begin as u32, begin as u32, 0)];
        for i in 4 - tail..4 {
            if i != 2 {
                expected.push(Call::Free(0x1000 + i as u32));
            }
        }
        assert_eq!(take_calls(), expected);
        assert_eq!(c.edx, if tail == 0 { 0x5678_9876 } else { FREE_EDX });
        assert_eq!(vector, [begin as u32, new_end, end + 0x80]);
        assert_eq!(bytes, before);
    }
}

extern "thiscall" fn assign(this: *mut u8, source: *mut u8) -> u64 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Assign(this as u32, source as u32)));
    unsafe {
        for offset in [0, 4, 8] {
            write(this, offset, read::<u32>(source, offset));
        }
    }
    registers(0xAA55_AA55, 0x1234_FEDC)
}

#[test]
fn range_copy_truncates_signed_distances_and_keeps_record_padding() {
    let _f = fake(0x0062_C1E0, assign as *const () as usize);
    for distance in [-81i32, -40, -1, 0, 1, 39, 40, 79, 80, 81] {
        reset();
        let mut source = core::array::from_fn::<_, 120, _>(|i| i as u8);
        let mut destination = [0xA5u8; 120];
        let p = source.as_mut_ptr();
        let d = destination.as_mut_ptr();
        let c = unsafe {
            invoke(
                fc4p_0062c510 as *const (),
                core::ptr::null_mut(),
                &[p as u32, p.wrapping_offset(distance as isize) as u32, d as u32],
                Conv::Cdecl,
            )
        };
        let count = (distance / 40).max(0) as usize;
        let mut expected = [0xA5u8; 120];
        for i in 0..count {
            expected[i * 40..i * 40 + 25].copy_from_slice(&source[i * 40..i * 40 + 25]);
            expected[i * 40 + 28..i * 40 + 40].copy_from_slice(&source[i * 40 + 28..i * 40 + 40]);
        }
        assert_eq!(destination, expected);
        assert_eq!(c.eax, d as u32 + count as u32 * 40);
        let product_high = (((i64::from(distance) * 0x6666_6667) >> 32) as i32) >> 4;
        assert_eq!(c.edx, if count == 0 { product_high as u32 } else { 0x1234_FEDC });
        assert_eq!(
            take_calls(),
            (0..count)
                .map(|i| Call::Assign(d as u32 + i as u32 * 40 + 28, p as u32 + i as u32 * 40 + 28))
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn range_copy_supports_self_assignment_and_forward_overlapping_words() {
    let _f = fake(0x0062_C1E0, assign as *const () as usize);
    for displacement in [0usize, 4, 8] {
        reset();
        let mut bytes = core::array::from_fn::<_, 120, _>(|i| i as u8);
        let mut expected = bytes;
        for i in 0..2 {
            for offset in [0, 4, 8, 12, 16, 20] {
                let start = i * 40 + offset;
                let word: [u8; 4] = expected[start..start + 4].try_into().unwrap();
                expected[start + displacement..start + displacement + 4].copy_from_slice(&word);
            }
            expected[i * 40 + 24 + displacement] = expected[i * 40 + 24];
            for offset in [28, 32, 36] {
                let start = i * 40 + offset;
                let word: [u8; 4] = expected[start..start + 4].try_into().unwrap();
                expected[start + displacement..start + displacement + 4].copy_from_slice(&word);
            }
        }
        let p = bytes.as_mut_ptr();
        unsafe {
            invoke(
                fc4p_0062c510 as *const (),
                core::ptr::null_mut(),
                &[p as u32, p as u32 + 80, p as u32 + displacement as u32],
                Conv::Cdecl,
            )
        };
        assert_eq!(bytes, expected);
        assert_eq!(take_calls().len(), 2);
    }
}

#[test]
fn nested_destruction_releases_inner_members_and_storage_but_keeps_outer_storage() {
    reset();
    let _f = free_fake();
    let mut pieces = [0xA5u8; 0x50 * 3];
    let pp = pieces.as_mut_ptr();
    unsafe {
        write(pp, 0x44, 0x1111u32);
        write(pp, 0x50 + 0x44, 0u32);
        write(pp, 0xA0 + 0x44, 0x3333u32);
    }
    let mut records = [0xA5u8; 0x28 * 3];
    let p = records.as_mut_ptr();
    unsafe {
        write(p, 0x1C, pp);
        write(p, 0x20, pp.add(0xF0));
        write(p, 0x28 + 0x1C, 0u32);
        write(p, 0x28 + 0x20, 0u32);
        write(p, 0x50 + 0x1C, 0x7777u32);
        write(p, 0x50 + 0x20, 0x7777u32);
    }
    let before = records;
    let before_pieces = pieces;
    let c = unsafe {
        invoke(
            fc4p_0062c590 as *const (),
            core::ptr::null_mut(),
            &[p as u32, p as u32 + 120, 0xFFFF_FFFF],
            Conv::Cdecl,
        )
    };
    assert_eq!(
        take_calls(),
        [
            Call::Free(0x1111),
            Call::Free(0x3333),
            Call::Free(pp as u32),
            Call::Free(0x7777)
        ]
    );
    assert_eq!((c.eax, c.edx), (FREE_EAX, FREE_EDX));
    assert_eq!(records, before);
    assert_eq!(pieces, before_pieces);
    let c = unsafe {
        invoke(
            fc4p_0062c590 as *const (),
            core::ptr::null_mut(),
            &[p as u32, p as u32],
            Conv::Cdecl,
        )
    };
    assert_eq!((c.eax, c.edx), (EAX, EDX));
    assert!(take_calls().is_empty());
}

extern "thiscall" fn resize(this: *mut u8, count: u32) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Resize(this as u32, count));
        if let Some((begin, end)) = s.resize_buckets {
            let t = unsafe { CellTable::from_ptr(this) };
            t.set_buckets(begin as *mut u8);
            t.set_buckets_end(end as *mut u8);
        }
    });
    registers(0x9999_8888, 0x7777_6666)
}
extern "C" fn allocate_node(size: u32) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Allocate(size));
        registers(s.allocation, 0x3333_4444)
    })
}
extern "thiscall" fn construct(this: *mut u8, source: *mut u8) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Construct(this as u32, source as u32));
        assert_eq!(unsafe { read::<u32>(this.sub(8), 0) }, 0);
        assert_eq!(unsafe { read::<u32>(this.sub(4), 0) }, unsafe {
            read::<u32>(source.sub(4), 0)
        });
        unsafe { write(s.table as *mut u8, 0x10, 70u32) };
    });
    unsafe { write(this, 0, read::<u32>(source, 0)) };
    registers(0xEEEE_FFFF, 0x4321_ABCD)
}

#[test]
fn hash_insertion_resizes_first_uses_unsigned_key_keeps_collisions_and_returns_old_head_in_ecx() {
    let _r = fake(0x004F_0940, resize as *const () as usize);
    let _a = fake(0x0090_CF54, allocate_node as *const () as usize);
    let _c = fake(0x0062_A1D0, construct as *const () as usize);
    for key in [0u32, 3, 0x8000_0000, u32::MAX] {
        reset();
        let mut buckets = [0x1111u32, 0x2222, 0x3333];
        let bp = buckets.as_mut_ptr().cast::<u8>();
        let mut table = [0xA5A5_A5A5u32; 5];
        let tp = table.as_mut_ptr().cast::<u8>();
        let t = unsafe { CellTable::from_ptr(tp) };
        t.set_count(u32::MAX);
        t.set_buckets(core::ptr::null_mut());
        t.set_buckets_end(core::ptr::null_mut());
        let mut node = [0xA5u8; 0x110];
        let np = node.as_mut_ptr();
        let mut source = [key, 0x1234_5678];
        let sp = source.as_mut_ptr().cast::<u8>();
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.allocation = np as u32;
            s.resize_buckets = Some((bp as u32, bp as u32 + 12));
            s.table = tp as u32;
        });
        let old_head = buckets[(key % 3) as usize];
        let c = unsafe { invoke(fc4p_0062c160 as *const (), tp, &[sp as u32], Conv::Thiscall) };
        assert_eq!((c.eax, c.ecx, c.edx), (np as u32 + 4, old_head, 0x4321_ABCD));
        assert_eq!(
            take_calls(),
            [
                Call::Resize(tp as u32, 0),
                Call::Allocate(0x104),
                Call::Construct(np as u32 + 8, sp as u32 + 4)
            ]
        );
        assert_eq!(unsafe { read::<u32>(np, 0) }, old_head);
        assert_eq!(unsafe { read::<u32>(np, 4) }, key);
        assert_eq!(unsafe { read::<u32>(np, 8) }, source[1]);
        assert_eq!(&node[12..], &[0xA5; 0x110 - 12]);
        assert_eq!(t.count(), 71);
        let mut expected = [0x1111u32, 0x2222, 0x3333];
        expected[(key % 3) as usize] = np as u32;
        assert_eq!(buckets, expected);
    }
}

#[unsafe(naked)]
unsafe extern "C" fn keep_registers() {
    core::arch::naked_asm!("ret")
}
#[unsafe(naked)]
unsafe extern "C" fn keep_registers_pop_word() {
    core::arch::naked_asm!("ret 4")
}

#[repr(C)]
struct Probe {
    call: RawCall,
    before_esp: u32,
    after_esp: u32,
    ebx: u32,
    esi: u32,
    ebp: u32,
}

unsafe fn probe(call: RawCall) {
    let mut probe = Probe {
        call,
        before_esp: 0,
        after_esp: 0,
        ebx: 0,
        esi: 0,
        ebp: 0,
    };
    assert_eq!(core::mem::offset_of!(Probe, before_esp), 44);
    unsafe {
        core::arch::asm!(
            "push ebx", "push esi", "push ebp", "mov [edi + 44], esp",
            "mov ebx, 0x12345678", "mov esi, 0x87654321", "mov ebp, 0xabcdef12",
            "mov ecx, [edi + 20]", "mov eax, [edi + 16]",
            "2:", "test ecx, ecx", "jz 3f", "dec ecx", "push dword ptr [eax + ecx*4]", "jmp 2b",
            "3:", "mov eax, [edi + 4]", "mov ecx, [edi + 8]", "mov edx, [edi + 12]",
            "call dword ptr [edi]",
            "cmp dword ptr [edi + 24], 0", "je 4f", "mov ecx, [edi + 20]", "lea esp, [esp + ecx*4]",
            "4:", "mov [edi + 48], esp", "mov [edi + 52], ebx", "mov [edi + 56], esi", "mov [edi + 60], ebp",
            "pop ebp", "pop esi", "pop ebx",
            in("edi") &mut probe, clobber_abi("C"),
        );
    }
    assert_eq!(probe.before_esp, probe.after_esp);
    assert_eq!(
        (probe.ebx, probe.esi, probe.ebp),
        (0x1234_5678, 0x8765_4321, 0xABCD_EF12)
    );
}

fn environment() -> (u16, u16, u32, u32) {
    let mut cw = 0u16;
    let mut mxcsr = 0u32;
    let sw: u16;
    let flags: u32;
    unsafe {
        core::arch::asm!(
            "fnstcw word ptr [{cw}]", "fnstsw ax", "stmxcsr dword ptr [{mxcsr}]", "pushfd", "pop {flags:e}",
            cw = in(reg) &mut cw, mxcsr = in(reg) &mut mxcsr,
            out("ax") sw, flags = lateout(reg) flags,
        );
    }
    (cw, sw & 0x3800, mxcsr, flags & 0x400)
}

#[test]
fn exports_keep_callee_saved_registers_stack_and_float_environment() {
    use super::super::construction::{fc4p_0062c4b0, fc4p_0062c5f0, fc4p_0062cad0, fc4p_0062caf0};
    reset();
    let _fakes: Vec<_> = [
        0x0064_8000,
        0x0066_0810,
        0x0062_6D30,
        0x0062_4F70,
        0x0066_02C0,
        0x0062_C590,
        0x0062_BE40,
        0x008E_BA60,
        0x0064_7AE0,
        0x0090_CF63,
        0x0062_7220,
    ]
    .iter()
    .map(|&address| fake(address, keep_registers as *const () as usize))
    .collect();
    let _word_fakes: Vec<_> = [0x0082_9BC0, 0x0046_76A0, 0x0062_6860, 0x004F_0940, 0x0062_A1D0]
        .iter()
        .map(|&address| fake(address, keep_registers_pop_word as *const () as usize))
        .collect();
    let _allocate = fake(0x0090_CF54, allocate_node as *const () as usize);
    let mut masks = [0u32; 13];
    let _masks = crate::testing::fake_data(0x00AA_835C, masks.as_mut_ptr().cast());
    let mut storage = [0u8; 0x110];
    STATE.with(|s| s.borrow_mut().allocation = storage.as_mut_ptr() as u32);
    let mut buckets = [0u32; 3];
    let mut table = [0, buckets.as_mut_ptr() as u32, buckets.as_mut_ptr() as u32 + 12, 0, 0];
    let mut source = [0u32; 2];
    let mut object = [0u8; 0x380];
    let p = object.as_mut_ptr();
    let calls = [
        (fc4p_0062be40 as *const (), p, vec![], Conv::Thiscall),
        (fc4p_0062be80 as *const (), p, vec![0, 0], Conv::Thiscall),
        (fc4p_0062bf20 as *const (), p, vec![], Conv::Thiscall),
        (
            fc4p_0062c160 as *const (),
            table.as_mut_ptr().cast(),
            vec![source.as_mut_ptr() as u32],
            Conv::Thiscall,
        ),
        (fc4p_0062c4b0 as *const (), p, vec![0], Conv::Thiscall),
        (fc4p_0062c510 as *const (), p, vec![0, 0, 0], Conv::Cdecl),
        (fc4p_0062c590 as *const (), p, vec![0, 0], Conv::Cdecl),
        (fc4p_0062cad0 as *const (), p, vec![0], Conv::Thiscall),
        (fc4p_0062caf0 as *const (), p, vec![], Conv::Thiscall),
        (fc4p_0062c5f0 as *const (), p, vec![0], Conv::Thiscall),
    ];
    let _fpu = crate::testing::GameFpu::new();
    for (entry, this, args, convention) in calls {
        let value = 123.25f64;
        let mut restored = 0.0f64;
        unsafe {
            core::arch::asm!("fld qword ptr [{value}]", value = in(reg) &value, out("st(0)") _);
        }
        let before = environment();
        let call = RawCall::new(convention, entry as usize, this as u32, &args);
        unsafe { probe(call) };
        let after = environment();
        unsafe {
            core::arch::asm!("fstp qword ptr [{value}]", value = in(reg) &mut restored, out("st(0)") _);
        }
        assert_eq!(before, after);
        assert_eq!(restored.to_bits(), value.to_bits());
    }
}
