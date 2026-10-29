// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! These map accessors have no equivalent in the core traffic simulator's network model.
//! Fixtures retain the original object offsets and record every virtual call.

use super::*;
use crate::testing::{call_thiscall_with_eax, fake_data, GameFpu, Object};
use std::cell::RefCell;

// Keep the logical-result assertions concise; the register tests below call the exports directly.
unsafe fn fc4p_004adf00(occupant: *mut u8) -> u32 {
    unsafe {
        call_thiscall_with_eax(
            super::fc4p_004adf00 as *const () as usize,
            core::ptr::null_mut(),
            &[occupant as u32],
            0,
        )
    }
    .0
}
unsafe fn fc4p_004ae0c0(this: *mut u8, x: i32, z: i32) -> *mut u8 {
    unsafe {
        call_thiscall_with_eax(
            super::fc4p_004ae0c0 as *const () as usize,
            this,
            &[x as u32, z as u32],
            0,
        )
    }
    .0 as *mut u8
}
unsafe fn fc4p_004ae030(this: *mut u8, x: i32, z: i32, a: u32, b: u32, c: u32) -> u32 {
    unsafe { super::fc4p_004ae030(this, x, z, a, b, c) as u32 }
}
unsafe fn fc4p_004ae140(this: *mut u8, x: i32, z: i32) -> *mut u8 {
    unsafe { super::fc4p_004ae140(this, x, z) as u32 as *mut u8 }
}
unsafe fn fc4p_004ae190(this: *mut u8, point: *const f32, a: u32, b: u32) -> u32 {
    unsafe { super::fc4p_004ae190(this, point, a, b) as u32 }
}

unsafe fn fc4p_004ae460(begin: *mut u8, end: *mut u8, key: *const u32) -> *mut u8 {
    let f: unsafe extern "C" fn(*mut u8, *mut u8, *const u32) -> u64 =
        unsafe { core::mem::transmute(super::fc4p_004ae460 as *const ()) };
    unsafe { f(begin, end, key) as u32 as *mut u8 }
}

#[derive(Debug, PartialEq)]
struct Call(usize, usize, Vec<u32>);

#[derive(Default)]
struct State {
    calls: Vec<Call>,
    head: usize,
    answers: Vec<u32>,
    coordinates: (i32, i32),
    replacement_link: Option<(usize, usize)>,
    secondary: u32,
    query_output: Option<(usize, usize)>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

fn reset() {
    STATE.with(|s| *s.borrow_mut() = State::default());
}

fn record(this: *mut u8, slot: usize, args: &[u32]) {
    STATE.with(|s| s.borrow_mut().calls.push(Call(this as usize, slot, args.to_vec())));
}

fn calls() -> Vec<Call> {
    STATE.with(|s| std::mem::take(&mut s.borrow_mut().calls))
}

extern "thiscall" fn cell(this: *mut u8, x: i32, z: i32) -> u64 {
    record(this, 0x38, &[x as u32, z as u32]);
    STATE.with(|s| words(s.borrow().head as u32, s.borrow().secondary))
}

extern "thiscall" fn safe_cell(this: *mut u8, x: i32, z: i32) -> u64 {
    record(this, 0x34, &[x as u32, z as u32]);
    STATE.with(|s| words(s.borrow().head as u32, s.borrow().secondary))
}

extern "thiscall" fn predicate(this: *mut u8, network: *mut u8, a: u32, b: u32, c: u32) -> u64 {
    record(this, 0x70, &[network as u32, a, b, c]);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if let Some((tile, next)) = s.replacement_link.take() {
            unsafe { TileInfo::from_ptr(tile as *mut u8) }.set_next(next as *mut u8);
        }
        words(s.answers.remove(0), s.secondary)
    })
}

extern "thiscall" fn world(this: *mut u8, x: i32, z: i32, a: u32, b: u32) -> u64 {
    record(this, 0x20, &[x as u32, z as u32, a, b]);
    words(0xCAFE_BABE, 0xE190_CAFE)
}

extern "thiscall" fn addref(this: *mut u8) -> u32 {
    record(this, 4, &[]);
    STATE.with(|s| {
        if let Some((slot, expected)) = s.borrow().query_output {
            assert_eq!(unsafe { (slot as *const usize).read() }, expected);
        }
    });
    0xABCD_EF80
}

struct Map {
    memory: Object,
    _vtable: Box<[usize; 0x74 / 4]>,
}

impl Map {
    fn new() -> Self {
        reset();
        let mut memory = Object::new(0x80);
        let mut vtable = Box::new([0; 0x74 / 4]);
        vtable[1] = addref as *const () as usize;
        vtable[0x20 / 4] = world as *const () as usize;
        vtable[0x34 / 4] = safe_cell as *const () as usize;
        vtable[0x38 / 4] = cell as *const () as usize;
        vtable[0x70 / 4] = predicate as *const () as usize;
        let map = unsafe { TrafficNetworkMap::from_ptr(memory.ptr()) };
        map.set_vtable(vtable.as_ptr());
        map.set_width(5);
        map.set_height(3);
        Self {
            memory,
            _vtable: vtable,
        }
    }

    fn ptr(&mut self) -> *mut u8 {
        self.memory.ptr()
    }
}

fn tile(network: *mut u8, next: *mut u8) -> Object {
    let mut memory = Object::new(0x58);
    let tile = unsafe { TileInfo::from_ptr(memory.ptr()) };
    tile.set_network(network);
    tile.set_next(next);
    memory
}

#[test]
fn safe_cell_signed_boundaries_and_empty_dimensions() {
    let mut map = Map::new();
    let p = map.ptr();
    STATE.with(|s| s.borrow_mut().head = 0x1234);
    for (x, z, allowed) in [
        (0, 0, true),
        (4, 2, true),
        (-1, 0, false),
        (0, -1, false),
        (5, 0, false),
        (0, 3, false),
        (i32::MIN, i32::MAX, false),
    ] {
        let before = unsafe { core::slice::from_raw_parts(p, 0x80) }.to_vec();
        assert_eq!(
            unsafe { fc4p_004ae0c0(p, x, z) } as usize,
            if allowed { 0x1234 } else { 0 }
        );
        assert_eq!(
            calls(),
            if allowed {
                vec![Call(p as usize, 0x34, vec![x as u32, z as u32])]
            } else {
                vec![]
            }
        );
        assert_eq!(unsafe { core::slice::from_raw_parts(p, 0x80) }, before);
    }
    for dimension in [0, -1, i32::MIN] {
        let view = unsafe { TrafficNetworkMap::from_ptr(p) };
        view.set_width(dimension);
        assert!(unsafe { fc4p_004ae0c0(p, 0, 0) }.is_null());
        view.set_width(5);
        view.set_height(dimension);
        assert!(unsafe { fc4p_004ae0c0(p, 0, 0) }.is_null());
    }
    assert!(calls().is_empty());
}

#[test]
fn chain_predicate_preserves_return_bytes_and_reloads_next_after_callback() {
    let mut map = Map::new();
    let p = map.ptr();
    let mut last = tile(0x3333usize as *mut u8, core::ptr::null_mut());
    let mut skipped = tile(0x2222usize as *mut u8, core::ptr::null_mut());
    let mut first = tile(0x1111usize as *mut u8, skipped.ptr());
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.head = first.ptr() as usize;
        s.answers = vec![0xAABB_CC00, 0xDD11_22FE];
        s.replacement_link = Some((first.ptr() as usize, last.ptr() as usize));
    });
    assert_eq!(unsafe { fc4p_004ae030(p, -2, 8, 11, 22, 33) }, 0xDD11_2201);
    assert_eq!(
        calls(),
        [
            Call(p as usize, 0x38, vec![(-2i32) as u32, 8]),
            Call(p as usize, 0x70, vec![0x1111, 11, 22, 33]),
            Call(p as usize, 0x70, vec![0x3333, 11, 22, 33]),
        ]
    );
    STATE.with(|s| s.borrow_mut().answers = vec![0x1122_3300, 0x9988_7700]);
    assert_eq!(unsafe { fc4p_004ae030(p, 1, 2, 0, 0, 0) }, 0x9988_7700);
    assert_eq!(calls().len(), 3);
    STATE.with(|s| s.borrow_mut().head = 0);
    assert_eq!(unsafe { fc4p_004ae030(p, 0, 0, 0, 0, 0) }, 0);
    assert_eq!(calls().len(), 1);
}

#[repr(C)]
struct Occupant {
    vtable: *const usize,
    kind: u32,
    flag2: u32,
    flag3: u32,
}

extern "thiscall" fn kind(this: *mut Occupant) -> u64 {
    record(this.cast(), 0x1C, &[]);
    words(unsafe { (*this).kind }, 0xA1A2_A3A4)
}

extern "thiscall" fn flag(this: *mut Occupant, id: u32) -> u64 {
    record(this.cast(), 0x58, &[id]);
    words(
        unsafe {
            if id == 0x5002 {
                (*this).flag2
            } else {
                (*this).flag3
            }
        },
        0xB1B2_B3B4,
    )
}

extern "thiscall" fn connection(this: *mut Occupant, a: u32, b: u32, c: u32) -> u64 {
    record(this.cast(), 0xD4, &[a, b, c]);
    words(unsafe { (*this).flag2 }, 0xC1C2_C3C4)
}

extern "thiscall" fn coordinates(this: *mut u8, x: *mut i32, z: *mut i32) {
    record(this, 0xEC, &[]);
    STATE.with(|s| {
        let s = s.borrow();
        unsafe {
            x.write(s.coordinates.0);
            z.write(s.coordinates.1);
        }
    });
}

#[test]
fn prop_type_and_flags_short_circuit_and_normalize_only_al() {
    reset();
    let mut vt = [0usize; 0x5C / 4];
    vt[0x1C / 4] = kind as *const () as usize;
    vt[0x58 / 4] = flag as *const () as usize;
    let mut obj = Occupant {
        vtable: vt.as_ptr(),
        kind: 0x1234_5678,
        flag2: 0,
        flag3: 0,
    };
    let p = (&mut obj as *mut Occupant).cast();
    assert_eq!(unsafe { fc4p_004adf00(p) }, 0x1234_5600);
    assert_eq!(calls(), [Call(p as usize, 0x1C, vec![])]);
    unsafe {
        (*p.cast::<Occupant>()).kind = 0xA823_821E;
    }
    for (flag2, flag3, expected, slots) in [
        (0x1234_56FF, 0, 0x1234_5601, vec![0x1C, 0x58]),
        (0x1234_5600, 0xCDEF_1280, 0xCDEF_1201, vec![0x1C, 0x58, 0x58]),
        (0, 0xFFEE_DD00, 0xFFEE_DD00, vec![0x1C, 0x58, 0x58]),
    ] {
        unsafe {
            (*p.cast::<Occupant>()).flag2 = flag2;
            (*p.cast::<Occupant>()).flag3 = flag3;
        }
        assert_eq!(unsafe { fc4p_004adf00(p) }, expected);
        let observed = calls();
        assert_eq!(observed.iter().map(|c| c.1).collect::<Vec<_>>(), slots);
        assert_eq!(observed[1].2, [0x5002]);
        if observed.len() == 3 {
            assert_eq!(observed[2].2, [0x5003]);
        }
    }
}

#[test]
fn connection_stdcall_stack_order_and_null_eax() {
    reset();
    let mut vt = [0usize; 0xD8 / 4];
    vt[0xD4 / 4] = connection as *const () as usize;
    let mut obj = Occupant {
        vtable: vt.as_ptr(),
        kind: 0,
        flag2: 0xAABB_CC80,
        flag3: 0,
    };
    let p = (&mut obj as *mut Occupant).cast::<u8>();
    // RawCall verifies the original four-word callee cleanup, including the stdcall null path.
    for (network, expected) in [(p, 0xAABB_CC01), (core::ptr::null_mut(), 0x1234_5600)] {
        let result = unsafe {
            call_thiscall_with_eax(
                fc4p_004ae090 as *const () as usize,
                0x8765usize as *mut u8,
                &[network as u32, 11, 22, 33],
                0x1234_56FF,
            )
        }
        .0;
        assert_eq!(result, expected);
    }
    assert_eq!(calls(), [Call(p as usize, 0xD4, vec![33, 11, 22])]);
    unsafe {
        (*p.cast::<Occupant>()).flag2 = 0xAABB_CC00;
    }
    assert_eq!(
        unsafe {
            call_thiscall_with_eax(
                fc4p_004ae090 as *const () as usize,
                core::ptr::null_mut(),
                &[p as u32, 1, 2, 3],
                0,
            )
        }
        .0,
        0xAABB_CC00
    );
}

fn by_network(map: *mut u8, network: *mut u8) -> (u32, u32) {
    let mut frame = [fc4p_004ae0f0 as *const () as u32, map as u32, network as u32, 0, 0, 0];
    unsafe {
        core::arch::asm!(
            "push dword ptr [edi + 8]", "mov ecx, dword ptr [edi + 4]", "call dword ptr [edi]",
            "mov dword ptr [edi + 12], eax", "mov dword ptr [edi + 20], edx",
            "mov eax, dword ptr [esp - 4]", "mov dword ptr [edi + 16], eax",
            in("edi") frame.as_mut_ptr(), clobber_abi("C"),
        );
    }
    assert_eq!(frame[5], STATE.with(|s| s.borrow().secondary));
    (frame[3], frame[4])
}

#[test]
fn by_network_writes_original_argument_word_and_searches_entire_chain() {
    let mut map = Map::new();
    let p = map.ptr();
    let mut vt = [0usize; 0xF0 / 4];
    vt[0xEC / 4] = coordinates as *const () as usize;
    let mut obj = [vt.as_ptr() as usize];
    let network = obj.as_mut_ptr().cast::<u8>();
    let mut found = tile(network, core::ptr::null_mut());
    let mut first = tile(core::ptr::null_mut(), found.ptr());
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.head = first.ptr() as usize;
        s.coordinates = (-7, 12345);
        s.secondary = 0x89AB_CDEF;
    });
    assert_eq!(by_network(p, network), (found.ptr() as u32, 12345));
    assert_eq!(
        calls(),
        [
            Call(network as usize, 0xEC, vec![]),
            Call(p as usize, 0x38, vec![(-7i32) as u32, 12345])
        ]
    );
    unsafe { TileInfo::from_ptr(found.ptr()) }.set_network(core::ptr::null_mut());
    assert_eq!(by_network(p, network), (0, 12345));
    assert_eq!(calls().len(), 2);
    STATE.with(|s| s.borrow_mut().head = 0);
    assert_eq!(by_network(p, network), (0, 12345));
}

#[test]
fn network_at_cell_returns_first_network_without_searching() {
    let mut map = Map::new();
    let p = map.ptr();
    let mut second = tile(0x5555usize as *mut u8, core::ptr::null_mut());
    let mut first = tile(core::ptr::null_mut(), second.ptr());
    STATE.with(|s| s.borrow_mut().head = first.ptr() as usize);
    assert!(unsafe { fc4p_004ae140(p, 99, -3) }.is_null());
    assert_eq!(calls(), [Call(p as usize, 0x38, vec![99, (-3i32) as u32])]);
    unsafe { TileInfo::from_ptr(first.ptr()) }.set_network(0x1234usize as *mut u8);
    assert_eq!(unsafe { fc4p_004ae140(p, 0, 0) } as usize, 0x1234);
    STATE.with(|s| s.borrow_mut().head = 0);
    assert!(unsafe { fc4p_004ae140(p, 0, 0) }.is_null());
}

#[test]
fn world_point_truncates_scaled_coordinates_with_x87_helper() {
    let _fpu = GameFpu::new();
    let mut map = Map::new();
    let p = map.ptr();
    let mut scale = 0.0625f32;
    let _scale = fake_data(0x00A8_FDF0, (&mut scale as *mut f32).cast());
    for (x, z, ix, iz) in [
        (31.99, -31.99, 1, -1),
        (-15.99, 16.0, 0, 1),
        (-0.0, 0.0, 0, 0),
        (f32::INFINITY, f32::NAN, 0, 0),
        (f32::MAX, f32::MIN, 0, 0),
        (34_359_738_368.0, -34_359_738_368.0, i32::MIN, i32::MIN),
    ] {
        let point = [x, f32::NAN, z];
        assert_eq!(unsafe { fc4p_004ae190(p, point.as_ptr(), 55, 66) }, 0xCAFE_BABE);
        assert_eq!(calls(), [Call(p as usize, 0x20, vec![ix as u32, iz as u32, 55, 66])]);
    }
}

#[test]
fn query_interfaces_write_adjusted_pointer_before_addref_and_keep_upper_result() {
    let mut map = Map::new();
    let p = map.ptr();
    for (iid, offset) in [(0xCA0F_81B9, 0), (1, 4), (0x4522_94AA, 8), (0xE4FD_A3D4, 12)] {
        let mut out = core::ptr::null_mut();
        STATE.with(|s| {
            s.borrow_mut().query_output = Some(((&mut out as *mut *mut u8) as usize, p.wrapping_add(offset) as usize))
        });
        assert_eq!(unsafe { fc4p_004ae350(p, iid, &mut out) }, 0xABCD_EF01);
        assert_eq!(out, p.wrapping_add(offset));
        assert_eq!(calls(), [Call(p as usize, 4, vec![])]);
        STATE.with(|s| s.borrow_mut().query_output = None);
    }
    for (iid, expected) in [
        (0, 0xBADD_6B00),
        (2, 0xBADD_6B00),
        (0xCA0F_81B8, 0x84EC_ED00),
        (0xCA0F_81BA, 0xCA0F_8100),
        (u32::MAX, 0xFFFF_FF00),
    ] {
        let mut out = 0x5678usize as *mut u8;
        assert_eq!(unsafe { fc4p_004ae350(p, iid, &mut out) }, expected);
        assert_eq!(out as usize, 0x5678);
        assert!(calls().is_empty());
    }
}

#[test]
fn record_search_covers_unrolled_groups_tail_and_first_duplicate() {
    let mut records = [[0u32; 4]; 13];
    for (i, record) in records.iter_mut().enumerate() {
        *record = [i as u32 + 10, 101, 102, 103];
    }
    let before = records;
    let start = records.as_mut_ptr().cast::<u8>();
    for count in 0..=13usize {
        let end = start.wrapping_add(count * 16);
        for key in 9..24u32 {
            let expected = if key >= 10 && (key - 10) < count as u32 {
                start.wrapping_add((key - 10) as usize * 16)
            } else {
                end
            };
            assert_eq!(unsafe { fc4p_004ae460(start, end, &key) }, expected);
        }
    }
    assert_eq!(records, before);
    unsafe {
        start.add(3 * 16).cast::<u32>().write(10);
    }
    assert_eq!(unsafe { fc4p_004ae460(start, start.wrapping_add(13 * 16), &10) }, start);
    // Reversed ranges never dereference their key or records.
    assert_eq!(
        unsafe { fc4p_004ae460(start.wrapping_add(16), start, core::ptr::null()) },
        start
    );
}

#[test]
fn lookup_exports_preserve_secondary_results_and_null_path_registers() {
    use crate::abi::{call_raw, Conv, RawCall};
    let _fpu = GameFpu::new();
    let mut map = Map::new();
    let p = map.ptr();
    let entry_edx = 0x1122_3344;
    for (x, z, expected_edx) in [(-1, 0, entry_edx), (5, 0, entry_edx), (0, -1, u32::MAX), (0, 3, 3)] {
        let args = [x as u32, z as u32];
        let mut call = RawCall::new(
            Conv::Thiscall,
            super::fc4p_004ae0c0 as *const () as usize,
            p as u32,
            &args,
        );
        call.edx = entry_edx;
        unsafe {
            call_raw(&mut call);
        }
        assert_eq!((call.eax, call.edx), (0, expected_edx));
    }
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.secondary = 0x5566_7788;
        s.head = 0;
    });
    let args = [0, 0];
    let mut call = RawCall::new(
        Conv::Thiscall,
        super::fc4p_004ae0c0 as *const () as usize,
        p as u32,
        &args,
    );
    unsafe {
        call_raw(&mut call);
    }
    assert_eq!((call.eax, call.edx), (0, 0x5566_7788));
    assert_eq!(unsafe { super::fc4p_004ae140(p, 0, 0) }, words(0, 0x5566_7788));
    assert_eq!(unsafe { super::fc4p_004ae030(p, 0, 0, 0, 0, 0) }, words(0, 0x5566_7788));
    let mut scale = 0.0625f32;
    let _scale = fake_data(0x00A8_FDF0, (&mut scale as *mut f32).cast());
    assert_eq!(
        unsafe { super::fc4p_004ae190(p, [1.0, 0.0, 2.0].as_ptr(), 3, 4) },
        words(0xCAFE_BABE, 0xE190_CAFE)
    );
    let args = [0, 11, 22, 33];
    call = RawCall::new(Conv::Stdcall, fc4p_004ae090 as *const () as usize, 0x9988_7766, &args);
    call.eax = 0xABCD_EFFF;
    call.edx = entry_edx;
    unsafe {
        call_raw(&mut call);
    }
    assert_eq!((call.eax, call.edx), (0xABCD_EF00, entry_edx));
    let mut vt = [0usize; 0xD8 / 4];
    vt[0x1C / 4] = kind as *const () as usize;
    vt[0x58 / 4] = flag as *const () as usize;
    vt[0xD4 / 4] = connection as *const () as usize;
    let mut obj = Occupant {
        vtable: vt.as_ptr(),
        kind: 0x1234_5678,
        flag2: 0xAABB_CCFF,
        flag3: 0,
    };
    let object = (&mut obj as *mut Occupant).cast::<u8>();
    let args = [object as u32];
    call = RawCall::new(Conv::Stdcall, super::fc4p_004adf00 as *const () as usize, 0, &args);
    unsafe {
        call_raw(&mut call);
    }
    assert_eq!((call.eax, call.edx), (0x1234_5600, 0xA1A2_A3A4));
    unsafe {
        (*object.cast::<Occupant>()).kind = 0xA823_821E;
    }
    unsafe {
        call_raw(&mut call);
    }
    assert_eq!((call.eax, call.edx), (0xAABB_CC01, 0xB1B2_B3B4));
    let args = [object as u32, 11, 22, 33];
    call = RawCall::new(Conv::Stdcall, fc4p_004ae090 as *const () as usize, 0, &args);
    unsafe {
        call_raw(&mut call);
    }
    assert_eq!((call.eax, call.edx), (0xAABB_CC01, 0xC1C2_C3C4));
}

#[unsafe(naked)]
unsafe extern "C" fn return_edx_unchanged() {
    core::arch::naked_asm!("mov eax, 0x12345678", "ret 8");
}

#[test]
fn cell_virtual_call_supplies_original_edx_when_callee_keeps_it() {
    let mut map = Map::new();
    let p = map.ptr();
    map._vtable[0x34 / 4] = return_edx_unchanged as *const () as usize;
    let args = [2, 1];
    let (eax, edx) = unsafe { call_thiscall_with_eax(super::fc4p_004ae0c0 as *const () as usize, p, &args, 0) };
    assert_eq!((eax, edx), (0x1234_5678, 1));
}

#[test]
fn record_search_retains_entry_key_or_penultimate_record_in_edx() {
    use crate::abi::{call_raw, Conv, RawCall};
    let mut records = [[1u32, 0, 0, 0], [2, 0, 0, 0], [3, 0, 0, 0], [4, 0, 0, 0], [5, 0, 0, 0]];
    let start = records.as_mut_ptr().cast::<u8>();
    let absent = 99u32;
    for (count, edx) in [(0, 0xABCD_1234), (1, 0xABCD_1234), (2, 1), (3, 2), (4, 99), (5, 99)] {
        let args = [
            start as u32,
            start.wrapping_add(count * 16) as u32,
            &absent as *const u32 as u32,
        ];
        let mut call = RawCall::new(Conv::Cdecl, super::fc4p_004ae460 as *const () as usize, 0, &args);
        call.edx = 0xABCD_1234;
        unsafe {
            call_raw(&mut call);
        }
        assert_eq!((call.eax, call.edx), (start.wrapping_add(count * 16) as u32, edx));
    }
}
