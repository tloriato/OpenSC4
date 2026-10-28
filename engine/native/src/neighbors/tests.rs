// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Native-layout versions of the core's interface, connection-query, and accounting cases.

use super::queries::*;
use super::surface::*;
use super::*;
use crate::abi::{call_raw, Conv, RawCall};
use crate::testing::{fake, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    AddRef(usize, usize),
    Find(usize, u32, i32, i32),
    Compare(usize, usize),
    Shutdown(usize),
    Destroy(usize, u32),
    Property(usize, u32),
    Value(usize),
    Output(usize, usize),
    Kind(usize),
    Cell(usize),
    Visited(usize),
    City(usize),
    Clear(usize),
    Copy(usize, u32, usize, usize),
}

thread_local! {
    static CALLS: RefCell<Vec<Call>> = const { RefCell::new(Vec::new()) };
    static INTERFACE_OUT: RefCell<usize> = const { RefCell::new(0) };
    static NEXT_TABLE: RefCell<usize> = const { RefCell::new(0) };
}

fn record(call: Call) {
    CALLS.with(|c| c.borrow_mut().push(call));
}

fn take_calls() -> Vec<Call> {
    CALLS.with(|c| core::mem::take(&mut *c.borrow_mut()))
}

unsafe fn run(entry: usize, this: *mut u8, args: &[u32], eax: u32, edx: u32) -> u64 {
    let mut c = RawCall::new(Conv::Thiscall, entry, this as u32, args);
    c.eax = eax;
    c.edx = edx;
    unsafe { call_raw(&mut c) };
    pair(c.eax, c.edx)
}

unsafe fn run_c(entry: usize, args: &[u32]) -> u64 {
    let mut c = RawCall::new(Conv::Cdecl, entry, 0, args);
    unsafe { call_raw(&mut c) };
    pair(c.eax, c.edx)
}

extern "thiscall" fn add_ref(this: *mut u8) -> u64 {
    let output = INTERFACE_OUT.with(|o| *o.borrow());
    let published = if output == 0 {
        0
    } else {
        unsafe { *(output as *const usize) }
    };
    record(Call::AddRef(this as usize, published));
    0x1234_5678_AABB_CCDD
}

extern "thiscall" fn find(this: *mut u8, kind: u32, x: i32, z: i32) -> u64 {
    record(Call::Find(this as usize, kind, x, z));
    unsafe { core::ptr::read_unaligned(this.add(0x40).cast()) }
}

extern "thiscall" fn compare(this: *mut u8, other: *mut u8) -> u64 {
    record(Call::Compare(this as usize, other as usize));
    unsafe { core::ptr::read_unaligned(this.add(4).cast()) }
}

extern "thiscall" fn shutdown(this: *mut u8) -> u64 {
    record(Call::Shutdown(this as usize));
    NEXT_TABLE.with(|v| unsafe { this.cast::<usize>().write(*v.borrow()) });
    0xAAAA_BBBB_CCCC_DDDD
}

extern "thiscall" fn destroy(this: *mut u8, flags: u32) -> u64 {
    record(Call::Destroy(this as usize, flags));
    0xFEED_CAFE_9876_5432
}

#[test]
fn query_interface_answers_four_iids_and_publishes_before_addref() {
    let mut table = [0usize; 3];
    table[1] = add_ref as *const () as usize;
    let mut object = Object::new(0x48);
    let p = object.ptr();
    unsafe { p.cast::<usize>().write(table.as_ptr() as usize) };
    for (iid, offset) in [(1, 0), (0x0773_FEF0, 0), (0x4522_94AA, 4), (0xE4FD_A3D4, 8)] {
        let mut output = 0xDEAD_BEEFusize;
        INTERFACE_OUT.with(|o| *o.borrow_mut() = &mut output as *mut usize as usize);
        let result = unsafe {
            run(
                fc4p_006cdf00 as *const () as usize,
                p,
                &[iid, &mut output as *mut usize as u32],
                0,
                0,
            )
        };
        assert_eq!(result, 0x1234_5678_AABB_CC01);
        assert_eq!(output, p as usize + offset);
        assert_eq!(take_calls(), [Call::AddRef(p as usize, output)]);
    }
    INTERFACE_OUT.with(|o| *o.borrow_mut() = 0);
    for iid in [0u32, 2, 0x0773_FEEF, 0x0773_FEF1, 0x4522_94A9, 0x4522_94AB, u32::MAX] {
        let mut output = 0xDEAD_BEEFusize;
        let result = unsafe {
            run(
                fc4p_006cdf00 as *const () as usize,
                p,
                &[iid, &mut output as *mut usize as u32],
                0,
                0xABCD_EF01,
            )
        };
        let expected = if iid > 0x4522_94AA {
            iid
        } else {
            iid.wrapping_sub(0x0773_FEF0)
        };
        assert_eq!(result, pair(expected & !0xFF, 0xABCD_EF01));
        assert_eq!(output, 0xDEAD_BEEF);
        assert_eq!(take_calls(), []);
    }
}

#[test]
fn identity_and_dirty_marker_keep_registers_and_unrelated_bytes() {
    let mut object = [0xA5u8; 0x48];
    let p = object.as_mut_ptr();
    unsafe {
        assert_eq!(
            run(fc4p_006cdef0 as *const () as usize, p, &[], 7, 0xFACE_CAFE),
            pair(0xC990_C0A4, 0xFACE_CAFE)
        );
        assert_eq!(
            run(fc4p_006cfd70 as *const () as usize, p, &[], 0xABCD_EF12, 0xFACE_CAFE),
            pair(0xABCD_EF12, 0xFACE_CAFE)
        );
    }
    let mut expected = [0xA5; 0x48];
    expected[0x3C] = 1;
    assert_eq!(object, expected);
}

#[test]
fn has_connection_normalizes_the_full_pointer_and_forwards_arguments() {
    let mut table = [0usize; 12];
    table[0x2C / 4] = find as *const () as usize;
    let mut object = Object::new(0x48);
    let p = object.ptr();
    unsafe { p.cast::<usize>().write(table.as_ptr() as usize) };
    for result in [0, 1, 0x100, 0x8000_0000, u32::MAX] {
        unsafe { p.add(0x40).cast::<u64>().write_unaligned(pair(result, 0xF00D_BEEF)) };
        let actual = unsafe { fc4p_006cdf80(p, 0x8000_0001, i32::MIN, i32::MAX) };
        assert_eq!(actual, pair(u32::from(result != 0), 0xF00D_BEEF));
        assert_eq!(take_calls(), [Call::Find(p as usize, 0x8000_0001, i32::MIN, i32::MAX)]);
    }
}

#[test]
fn serializable_comparison_adjusts_both_objects_and_inverts_full_word() {
    let mut table = [0usize; 28];
    table[0x6C / 4] = compare as *const () as usize;
    let mut left = Object::new(0x20);
    let p = left.ptr();
    let mut right = Object::new(0x20);
    unsafe { p.add(8).cast::<usize>().write(table.as_ptr() as usize) };
    for other in [core::ptr::null_mut(), right.ptr()] {
        for result in [0, 1, 0x100, u32::MAX] {
            unsafe { p.add(12).cast::<u64>().write_unaligned(pair(result, 0x1234_5678)) };
            assert_eq!(
                unsafe { fc4p_006ce6a0(p, other) },
                pair(u32::from(result == 0), 0x1234_5678)
            );
            assert_eq!(
                take_calls(),
                [Call::Compare(
                    p as usize + 8,
                    if other.is_null() { 0 } else { other as usize + 8 }
                )]
            );
        }
    }
}

#[test]
fn release_uses_signed_count_and_reloads_the_destructor_table() {
    let mut table = [0usize; 39];
    table[0x10 / 4] = shutdown as *const () as usize;
    let mut replacement = [0usize; 39];
    replacement[0x98 / 4] = destroy as *const () as usize;
    NEXT_TABLE.with(|v| *v.borrow_mut() = replacement.as_ptr() as usize);
    let mut object = Object::new(0x48);
    let p = object.ptr();
    let sim = unsafe { NeighborsSim::from_ptr(p) };
    for count in [i32::MIN, -1, 0, 1, 2, i32::MAX] {
        sim.set_primary_vtable(table.as_ptr() as u32);
        sim.set_references(count);
        let before = unsafe { core::slice::from_raw_parts(p, 0x48) }.to_vec();
        let result = unsafe { run(fc4p_006ce770 as *const () as usize, p, &[], 0, 0x1234_5678) };
        if count > 1 {
            assert_eq!(result, pair((count - 1) as u32, 0x1234_5678));
            assert_eq!(sim.references(), count - 1);
            let mut expected = before;
            expected[0x10..0x14].copy_from_slice(&(count - 1).to_le_bytes());
            assert_eq!(unsafe { core::slice::from_raw_parts(p, 0x48) }, expected);
            assert_eq!(take_calls(), []);
        } else {
            assert_eq!(result, pair(0, 0xFEED_CAFE));
            assert_eq!(sim.references(), count);
            assert_eq!(take_calls(), [Call::Shutdown(p as usize), Call::Destroy(p as usize, 1)]);
        }
    }
    for (entry, offset) in [
        (fc4p_006cfda0 as *const () as usize, 4),
        (fc4p_006cfdb0 as *const () as usize, 8),
    ] {
        sim.set_references(3);
        assert_eq!(
            unsafe { run(entry, p.add(offset), &[], 0, 0x1234_5678) },
            pair(2, 0x1234_5678)
        );
        assert_eq!(sim.references(), 2);
        sim.set_primary_vtable(table.as_ptr() as u32);
        sim.set_references(1);
        assert_eq!(unsafe { run(entry, p.add(offset), &[], 0, 0) }, pair(0, 0xFEED_CAFE));
        assert_eq!(take_calls(), [Call::Shutdown(p as usize), Call::Destroy(p as usize, 1)]);
    }
}

#[repr(C)]
struct PropertyObject {
    table: *const usize,
    result: u64,
}

extern "thiscall" fn property(this: *mut PropertyObject, id: u32) -> u64 {
    record(Call::Property(this as usize, id));
    unsafe { (*this).result }
}

extern "thiscall" fn value(this: *mut PropertyObject) -> u64 {
    record(Call::Value(this as usize));
    unsafe { (*this).result }
}

extern "thiscall" fn output(this: *mut PropertyObject, stream: *mut u8) -> u64 {
    record(Call::Output(this as usize, stream as usize));
    unsafe { (*this).result }
}

#[test]
fn property_writer_skips_missing_values_and_returns_the_output_registers() {
    let mut holder_table = [0usize; 10];
    holder_table[0x24 / 4] = property as *const () as usize;
    let mut property_table = [0usize; 6];
    property_table[0x14 / 4] = value as *const () as usize;
    let mut value_table = [0usize; 41];
    value_table[0xA0 / 4] = output as *const () as usize;
    let mut v = PropertyObject {
        table: value_table.as_ptr(),
        result: 0x1234_5678_AABB_CCDD,
    };
    let mut prop = PropertyObject {
        table: property_table.as_ptr(),
        result: &mut v as *mut _ as u64,
    };
    let mut holder = PropertyObject {
        table: holder_table.as_ptr(),
        result: pair(0, 0xABCD_EF01),
    };
    let h = &mut holder as *mut _ as u32;
    assert_eq!(
        unsafe { run_c(fc4p_005fd530 as *const () as usize, &[h, 0xFF00, 0x1234]) },
        pair(0, 0xABCD_EF01)
    );
    assert_eq!(take_calls(), [Call::Property(h as usize, 0xFF00)]);
    holder.result = &mut prop as *mut _ as u64;
    assert_ne!(holder.result, 0);
    assert_eq!(
        unsafe { run_c(fc4p_005fd530 as *const () as usize, &[h, 0xFF00, 0x1234]) },
        v.result
    );
    assert_eq!(
        take_calls(),
        [
            Call::Property(h as usize, 0xFF00),
            Call::Value(&prop as *const _ as usize),
            Call::Output(&v as *const _ as usize, 0x1234)
        ]
    );
}

#[test]
fn layout_fields_are_disjoint() {
    let mut spans: Vec<_> = NeighborsSim::FIELDS
        .iter()
        .map(|&(_, offset, size)| (offset, offset + size))
        .collect();
    spans.sort();
    assert!(spans.windows(2).all(|w| w[0].1 <= w[1].0));
}

/// Copies the callee's popped argument words back before any subsequent call can reuse that stack.
unsafe fn run_live(entry: usize, this: *mut u8, args: &mut [u32], edx: u32) -> u64 {
    let mut call = RawCall::new(Conv::Thiscall, entry, this as u32, args);
    call.edx = edx;
    unsafe {
        core::arch::asm!(
            "push esi",
            "mov ecx, [edi + 20]",
            "mov eax, [edi + 16]",
            "2:", "test ecx, ecx", "jz 3f", "dec ecx", "push dword ptr [eax + ecx*4]", "jmp 2b",
            "3:", "mov eax, [edi + 4]", "mov ecx, [edi + 8]", "mov edx, [edi + 12]", "call dword ptr [edi]",
            "mov [edi + 4], eax", "mov [edi + 12], edx",
            "mov ecx, [edi + 20]", "mov edx, ecx", "shl edx, 2", "neg edx", "add edx, esp",
            "mov esi, [edi + 16]",
            "4:", "test ecx, ecx", "jz 5f", "dec ecx", "mov eax, [edx + ecx*4]", "mov [esi + ecx*4], eax", "jmp 4b",
            "5:", "pop esi",
            in("edi") &mut call,
            clobber_abi("C"),
        );
    }
    pair(call.eax, call.edx)
}

#[repr(C)]
struct Connection {
    table: *const usize,
    kind: u32,
    x: i32,
    z: i32,
    city: u32,
    visited: u32,
}

extern "thiscall" fn connection_kind(this: *mut Connection) -> u64 {
    record(Call::Kind(this as usize));
    pair(unsafe { (*this).kind }, 0x1111_2222)
}

extern "thiscall" fn connection_cell(this: *mut Connection, x: *mut i32, z: *mut i32) -> u64 {
    record(Call::Cell(this as usize));
    unsafe {
        x.write_unaligned((*this).x);
        z.write_unaligned((*this).z);
    }
    0x2222_3333_4444_5555
}

extern "thiscall" fn connection_city(this: *mut Connection) -> u64 {
    record(Call::City(this as usize));
    pair(unsafe { (*this).city }, 0x3333_4444)
}

extern "thiscall" fn connection_visited(this: *mut Connection) -> u64 {
    record(Call::Visited(this as usize));
    pair(unsafe { (*this).visited }, 0x4444_5555)
}

fn connection_table() -> [usize; 12] {
    let mut table = [0; 12];
    table[0x14 / 4] = connection_kind as *const () as usize;
    table[0x18 / 4] = connection_cell as *const () as usize;
    table[0x24 / 4] = connection_city as *const () as usize;
    table[0x2C / 4] = connection_visited as *const () as usize;
    table
}

struct List {
    nodes: Vec<[u32; 3]>,
}

impl List {
    fn new(values: &[u32]) -> Self {
        let mut nodes = vec![[0u32; 3]; values.len() + 1];
        let count = nodes.len();
        let base = nodes.as_mut_ptr();
        for i in 0..count {
            nodes[i] = [
                unsafe { base.add((i + 1) % count) } as u32,
                unsafe { base.add((i + count - 1) % count) } as u32,
                if i == 0 { 0 } else { values[i - 1] },
            ];
        }
        Self { nodes }
    }

    fn ptr(&mut self) -> *mut u8 {
        self.nodes.as_mut_ptr().cast()
    }
}

#[test]
fn object_overload_forwards_cell_and_updates_its_argument_word() {
    let table = connection_table();
    let mut connection = Connection {
        table: table.as_ptr(),
        kind: 5,
        x: -1,
        z: 0x1020_3040,
        city: 0,
        visited: 0,
    };
    let cp = &mut connection as *mut _ as u32;
    let mut sim_table = [0usize; 15];
    sim_table[0x38 / 4] = find as *const () as usize;
    let mut object = Object::new(0x48);
    let p = object.ptr();
    unsafe {
        p.cast::<usize>().write(sim_table.as_ptr() as usize);
        p.add(0x40).cast::<u64>().write_unaligned(0x1234_5678_AABB_CCDD);
    }
    let mut args = [cp];
    assert_eq!(
        unsafe { run_live(fc4p_006cdfa0 as *const () as usize, p, &mut args, 0) },
        0x1234_5678_AABB_CCDD
    );
    assert_eq!(args, [0x1020_3040]);
    assert_eq!(
        take_calls(),
        [
            Call::Kind(cp as usize),
            Call::Cell(cp as usize),
            Call::Find(p as usize, 5, -1, 0x1020_3040)
        ]
    );
}

#[test]
fn connection_lookup_visits_in_order_and_keeps_the_last_cell_in_the_argument() {
    let table = connection_table();
    let mut connections = [
        Connection {
            table: table.as_ptr(),
            kind: 2,
            x: -1,
            z: 5,
            city: 7,
            visited: 0,
        },
        Connection {
            table: table.as_ptr(),
            kind: 5,
            x: -1,
            z: 6,
            city: 7,
            visited: 0,
        },
        Connection {
            table: table.as_ptr(),
            kind: 2,
            x: 64,
            z: 5,
            city: 8,
            visited: 0,
        },
        Connection {
            table: table.as_ptr(),
            kind: 2,
            x: -1,
            z: 5,
            city: 7,
            visited: 0,
        },
    ];
    let values: Vec<_> = connections.iter_mut().map(|c| c as *mut _ as u32).collect();
    let mut list = List::new(&values);
    let mut object = Object::new(0x48);
    let p = object.ptr();
    let sim = unsafe { NeighborsSim::from_ptr(p) };
    sim.set_connections(list.ptr());
    for (kind, x, z, found, visits, edx, last_x) in [
        (2, -1, 5, Some(0), 1, 5, -1),
        (5, -1, 6, Some(1), 2, 6, -1),
        (2, 64, 5, Some(2), 3, 5, 64),
        (9, -1, 5, None, 4, 0x1111_2222, -1),
        (2, -1, 6, None, 4, 6, -1),
        (2, i32::MIN, i32::MAX, None, 4, 0x1111_2222, -1),
    ] {
        let mut args = [kind, x as u32, z as u32];
        assert_eq!(
            unsafe { run_live(fc4p_006cf6b0 as *const () as usize, p, &mut args, 0xDEAD_BEEF) },
            pair(found.map_or(0, |i| values[i]), edx)
        );
        assert_eq!(args, [last_x as u32, x as u32, z as u32]);
        let expected: Vec<_> = values[..visits]
            .iter()
            .flat_map(|&c| [Call::Cell(c as usize), Call::Kind(c as usize)])
            .collect();
        assert_eq!(take_calls(), expected);
    }
    let mut empty = List::new(&[]);
    sim.set_connections(empty.ptr());
    let mut args = [2, u32::MAX, 5];
    assert_eq!(
        unsafe { run_live(fc4p_006cf6b0 as *const () as usize, p, &mut args, 0xDEAD_BEEF) },
        pair(0, 0xDEAD_BEEF)
    );
    assert_eq!(args, [2, u32::MAX, 5]);
    assert_eq!(take_calls(), []);
}

#[test]
fn unvisited_lookup_short_circuits_kind_visited_and_city_tests() {
    let table = connection_table();
    let mut connections = [
        Connection {
            table: table.as_ptr(),
            kind: 5,
            x: 0,
            z: 0,
            city: 7,
            visited: 0,
        },
        Connection {
            table: table.as_ptr(),
            kind: 2,
            x: 0,
            z: 0,
            city: 7,
            visited: 1,
        },
        Connection {
            table: table.as_ptr(),
            kind: 2,
            x: 0,
            z: 0,
            city: 8,
            visited: 0,
        },
        Connection {
            table: table.as_ptr(),
            kind: 2,
            x: 0,
            z: 0,
            city: 7,
            visited: 0x8000_0000,
        },
    ];
    let values: Vec<_> = connections.iter_mut().map(|c| c as *mut _ as u32).collect();
    let mut list = List::new(&values);
    let mut object = Object::new(0x48);
    let p = object.ptr();
    let sim = unsafe { NeighborsSim::from_ptr(p) };
    sim.set_connections(list.ptr());
    assert_eq!(
        unsafe { fc4p_006cf720(p, 0, 2, 7 as *mut u8) },
        pair(values[3], 0x3333_4444)
    );
    assert_eq!(
        take_calls(),
        [
            Call::Kind(values[0] as usize),
            Call::Kind(values[1] as usize),
            Call::Visited(values[1] as usize),
            Call::Kind(values[2] as usize),
            Call::Visited(values[2] as usize),
            Call::City(values[2] as usize),
            Call::Kind(values[3] as usize),
            Call::Visited(values[3] as usize),
            Call::City(values[3] as usize)
        ]
    );
    connections[3].visited = 1;
    assert_eq!(connections[3].visited, 1);
    assert_eq!(unsafe { fc4p_006cf720(p, 0, 2, 7 as *mut u8) }, pair(0, 0x4444_5555));
    take_calls();
    let mut empty = List::new(&[]);
    sim.set_connections(empty.ptr());
    assert_eq!(
        unsafe { fc4p_006cf720(p, 0xDEAD_BEEF, 2, core::ptr::null_mut()) },
        pair(0, 0xDEAD_BEEF)
    );
    assert_eq!(take_calls(), []);
}

extern "thiscall" fn clear(list: *mut u8) -> u64 {
    record(Call::Clear(list as usize));
    0xAABB_CCDD_1234_5678
}

extern "thiscall" fn copy(list: *mut u8, position: u32, first: *mut u8, last: *mut u8, out: *mut u32) -> u64 {
    record(Call::Copy(list as usize, position, first as usize, last as usize));
    unsafe { out.write_unaligned(0x7654_3210) };
    0xFACE_CAFE_1234_ABCD
}

#[test]
fn all_connections_always_clears_and_calls_the_range_inserter_with_original_argument_destination() {
    let _clear = fake(0x0041_1D70, clear as *const () as usize);
    let _copy = fake(0x004A_86F0, copy as *const () as usize);
    for values in [&[][..], &[1u32, 2, 3][..]] {
        let mut source = List::new(values);
        let mut destination = List::new(&[8, 9]);
        let mut object = Object::new(0x48);
        let p = object.ptr();
        unsafe { NeighborsSim::from_ptr(p) }.set_connections(source.ptr());
        let mut args = [destination.ptr() as u32];
        assert_eq!(
            unsafe { run_live(fc4p_006cff00 as *const () as usize, p, &mut args, 0) },
            0xFACE_CAFE_1234_ABCD
        );
        assert_eq!(args, [0x7654_3210]);
        assert_eq!(
            take_calls(),
            [
                Call::Clear(destination.ptr() as usize),
                Call::Copy(
                    destination.ptr() as usize,
                    destination.nodes[0][0],
                    source.nodes[0][0] as usize,
                    source.ptr() as usize
                )
            ]
        );
    }
}
