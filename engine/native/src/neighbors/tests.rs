// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Native-layout versions of the core's interface, connection-query, and accounting cases.

use super::accounting::*;
use super::lists::*;
use super::proposal::*;
use super::queries::*;
use super::surface::*;
use super::*;
use crate::abi::{Conv, RawCall};
use crate::testing::{fake, fake_data, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Call {
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
    Flags(usize, u32),
    DealKind(usize),
    Cost(usize),
    Version(u32),
    Count(u32),
    Reference(usize, usize),
    Error,
    Erase(usize, u32),
    Convert(i32, i32),
    Release(usize),
    Allocate(u32),
    Framework,
    Service(u32, u32),
    ReleaseLocal(usize, usize),
    Construct(usize),
    Suitable(usize, u32, u32),
    OtherPosition(usize, i32, i32),
    OurPosition(usize, i32, i32),
    SetKind(usize, u32),
    SetAmount(usize, u32),
    CurrentCity(usize),
    Position(usize),
    Size(usize),
    Width,
    Height,
    Region,
    RegionalCity(i32, i32),
}

thread_local! {
    static CALLS: RefCell<Vec<Call>> = const { RefCell::new(Vec::new()) };
    static INTERFACE_OUT: RefCell<usize> = const { RefCell::new(0) };
    static NEXT_TABLE: RefCell<usize> = const { RefCell::new(0) };
    static CONVERSION: RefCell<(usize, u32)> = const { RefCell::new((0, 0)) };
    // Each issued node address stays fixed when the ownership vector grows.
    #[allow(clippy::vec_box)]
    static ALLOCATIONS: RefCell<Vec<Box<[u32; 3]>>> = const { RefCell::new(Vec::new()) };
    static ALLOCATOR: RefCell<usize> = const { RefCell::new(0) };
    static CACHED_ALLOCATOR: RefCell<usize> = const { RefCell::new(0) };
    static DEAL_STORAGE: RefCell<(usize, usize)> = const { RefCell::new((0, 0)) };
    static SUITABLE: RefCell<usize> = const { RefCell::new(0) };
}

pub(super) fn record(call: Call) {
    CALLS.with(|c| c.borrow_mut().push(call));
}

pub(super) fn take_calls() -> Vec<Call> {
    CALLS.with(|c| core::mem::take(&mut *c.borrow_mut()))
}

unsafe fn run(entry: usize, this: *mut u8, args: &[u32], eax: u32, edx: u32) -> u64 {
    let mut args = args.to_vec();
    unsafe { checked_call(entry, this, &mut args, eax, edx, Conv::Thiscall) }
}

unsafe fn run_c(entry: usize, args: &[u32]) -> u64 {
    let mut args = args.to_vec();
    unsafe { checked_call(entry, core::ptr::null_mut(), &mut args, 0, 0, Conv::Cdecl) }
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

#[unsafe(naked)]
unsafe extern "C" fn untouched_range_registers() {
    core::arch::naked_asm!("mov eax, [esp + 12]", "ret 16");
}

#[test]
fn all_connections_empty_range_retains_the_destination_header_in_edx() {
    let _clear = fake(0x0041_1D70, clear as *const () as usize);
    let _copy = fake(0x004A_86F0, untouched_range_registers as *const () as usize);
    let mut source = List::new(&[]);
    let mut destination = List::new(&[]);
    let mut object = Object::new(0x48);
    let p = object.ptr();
    unsafe { NeighborsSim::from_ptr(p) }.set_connections(source.ptr());
    assert_eq!(
        unsafe {
            run(
                fc4p_006cff00 as *const () as usize,
                p,
                &[destination.ptr() as u32],
                0,
                0xDEAD_BEEF,
            )
        },
        pair(source.ptr() as u32, destination.nodes[0][0])
    );
    assert_eq!(take_calls(), [Call::Clear(destination.ptr() as usize)]);
}

extern "C" fn allocate_deal(size: u32) -> *mut u8 {
    record(Call::Allocate(size));
    DEAL_STORAGE.with(|s| s.borrow().0 as *mut u8)
}

extern "thiscall" fn construct_deal(this: *mut u8) -> *mut u8 {
    record(Call::Construct(this as usize));
    DEAL_STORAGE.with(|s| s.borrow().1 as *mut u8)
}

extern "thiscall" fn suitable(this: *mut u8, kind: u32, city: u32) -> u64 {
    record(Call::Suitable(this as usize, kind, city));
    pair(SUITABLE.with(|s| *s.borrow() as u32), 0x1234_5678)
}

extern "thiscall" fn set_other_position(this: *mut u8, x: i32, z: i32) -> u64 {
    record(Call::OtherPosition(this as usize, x, z));
    0
}

extern "thiscall" fn set_our_position(this: *mut u8, x: i32, z: i32) -> u64 {
    record(Call::OurPosition(this as usize, x, z));
    0
}

extern "thiscall" fn set_kind(this: *mut u8, kind: u32) -> u64 {
    record(Call::SetKind(this as usize, kind));
    0
}

extern "thiscall" fn set_amount(this: *mut u8, amount: u32) -> u64 {
    record(Call::SetAmount(this as usize, amount));
    0
}

extern "thiscall" fn current_city(this: *mut u8) -> u64 {
    record(Call::CurrentCity(this as usize));
    pair(unsafe { word(this, 4) }, 0)
}

#[test]
fn single_proposal_calls_constructor_before_suitability_and_sets_positions_type_and_zero_amount() {
    let _allocate = fake(0x005E_55E0, allocate_deal as *const () as usize);
    let _construct = fake(0x006C_F270, construct_deal as *const () as usize);
    let mut deal_table = [0usize; 12];
    deal_table[1] = add_ref as *const () as usize;
    deal_table[2] = list_release as *const () as usize;
    deal_table[0x0C / 4] = set_our_position as *const () as usize;
    deal_table[0x14 / 4] = set_other_position as *const () as usize;
    deal_table[0x24 / 4] = set_kind as *const () as usize;
    deal_table[0x2C / 4] = set_amount as *const () as usize;
    let mut deal = Object::new(0x80);
    let dp = deal.ptr();
    unsafe { dp.cast::<usize>().write(deal_table.as_ptr() as usize) };
    let mut sim_table = [0usize; 24];
    sim_table[0x5C / 4] = suitable as *const () as usize;
    let mut sim = Object::new(0x48);
    let p = sim.ptr();
    unsafe { p.cast::<usize>().write(sim_table.as_ptr() as usize) };
    let mut connection_table = connection_table();
    connection_table[0x20 / 4] = connection_cell as *const () as usize;
    let mut connection = Connection {
        table: connection_table.as_ptr(),
        kind: 5,
        x: 4,
        z: 3,
        city: 0,
        visited: 0,
    };
    let cp = &mut connection as *mut _ as usize;
    let mut regional_table = [0usize; 6];
    regional_table[0x14 / 4] = connection_cell as *const () as usize;
    let mut regional = Connection {
        table: regional_table.as_ptr(),
        kind: 0,
        x: 3,
        z: 3,
        city: 0,
        visited: 0,
    };
    let rp = &mut regional as *mut _ as u32;
    let mut manager_table = [0usize; 37];
    manager_table[0x90 / 4] = current_city as *const () as usize;
    let mut manager = [manager_table.as_ptr() as u32, rp];
    let mp = manager.as_mut_ptr() as usize;
    let mut manager_ptr = mp;
    let _manager = fake_data(0x00B4_3C94, (&mut manager_ptr as *mut usize).cast());
    for (storage, constructed, matched) in [
        (dp as usize, dp as usize, true),
        (dp as usize, dp as usize, false),
        (0, 0, false),
        (dp as usize, 0, false),
    ] {
        DEAL_STORAGE.with(|s| *s.borrow_mut() = (storage, constructed));
        SUITABLE.with(|s| *s.borrow_mut() = if matched { cp } else { 0 });
        let mut out = 0xDEAD_BEEFusize;
        INTERFACE_OUT.with(|o| *o.borrow_mut() = &mut out as *mut usize as usize);
        let out_address = &mut out as *mut usize as u32;
        let mut args = [0x4773_FFE0, 7, out_address];
        let result = unsafe { run_live(fc4p_006cf780 as *const () as usize, p, &mut args, 0) };
        let mut expected = vec![Call::Allocate(0x80)];
        if storage != 0 {
            expected.push(Call::Construct(storage));
        }
        if constructed != 0 {
            expected.push(Call::AddRef(dp as usize, 0xDEAD_BEEF));
        }
        expected.push(Call::Suitable(p as usize, 0x4773_FFE0, 7));
        if matched {
            assert_eq!(result, 0x7788_99AA_AABB_CC01);
            assert_eq!(args, [3, 3, out_address]);
            assert_eq!(out, dp as usize);
            expected.extend([
                Call::Cell(cp),
                Call::OtherPosition(dp as usize, 4, 3),
                Call::CurrentCity(mp),
                Call::Cell(rp as usize),
                Call::OurPosition(dp as usize, 3, 3),
                Call::SetKind(dp as usize, 0x4773_FFE0),
                Call::SetAmount(dp as usize, 0),
                Call::AddRef(dp as usize, dp as usize),
                Call::Release(dp as usize),
            ]);
        } else {
            assert_eq!(args, [0x4773_FFE0, 7, out_address]);
            assert_eq!(out, 0xDEAD_BEEF);
            if constructed != 0 {
                expected.push(Call::Release(dp as usize));
                assert_eq!(result, 0x7788_99AA_AABB_CC00);
            } else {
                assert_eq!(result, pair(0, 0x1234_5678));
            }
        }
        assert_eq!(take_calls(), expected);
    }
    INTERFACE_OUT.with(|o| *o.borrow_mut() = 0);
}

extern "thiscall" fn list_clear(list: *mut u8) -> u64 {
    record(Call::Clear(list as usize));
    unsafe {
        let header = pointer(list, 0);
        header.cast::<*mut u8>().write(header);
        header.add(4).cast::<*mut u8>().write(header);
    }
    0x5566_7788_1234_5678
}

extern "thiscall" fn list_addref(this: *mut u8) -> u64 {
    record(Call::AddRef(this as usize, 0));
    0x3344_5566_AABB_CCDD
}

extern "thiscall" fn list_release(this: *mut u8) -> u64 {
    record(Call::Release(this as usize));
    0x7788_99AA_AABB_CCDD
}

extern "thiscall" fn allocate(_this: *mut u8, size: u32) -> u64 {
    record(Call::Allocate(size));
    ALLOCATIONS.with(|a| {
        let mut node = Box::new([0u32; 3]);
        let address = node.as_mut_ptr() as u32;
        a.borrow_mut().push(node);
        pair(address, 0xABCD_EF01)
    })
}

extern "C" fn get_framework() -> *mut u8 {
    record(Call::Framework);
    ALLOCATOR.with(|a| *a.borrow() as *mut u8)
}

extern "thiscall" fn service(_this: *mut u8, class: u32, iid: u32, out: *mut *mut u8) -> u32 {
    record(Call::Service(class, iid));
    ALLOCATOR.with(|a| unsafe { out.write(*a.borrow() as *mut u8) });
    0
}

extern "thiscall" fn release_local(this: *mut *mut u8) -> u32 {
    let local = unsafe { this.read() } as usize;
    let cache = CACHED_ALLOCATOR.with(|c| unsafe { *(*c.borrow() as *const usize) });
    record(Call::ReleaseLocal(local, cache));
    0
}

#[test]
fn filtered_lists_clear_match_in_order_and_balance_temporary_references() {
    let _clear_connections = fake(0x0041_1D70, list_clear as *const () as usize);
    let _clear_deals = fake(0x0042_1660, list_clear as *const () as usize);
    let _framework = fake(0x0087_93EC, get_framework as *const () as usize);
    let _release_local = fake(0x0041_1870, release_local as *const () as usize);
    let mut table = connection_table();
    table[1] = list_addref as *const () as usize;
    table[2] = list_release as *const () as usize;
    // The shared test object supplies the deal-type/city slots as well as the connection slots.
    table[0x20 / 4] = connection_kind as *const () as usize;
    table[0x1C / 4] = connection_city as *const () as usize;
    let mut allocator_table = [0usize; 6];
    allocator_table[0x0C / 4] = allocate as *const () as usize;
    allocator_table[0x14 / 4] = service as *const () as usize;
    let mut allocator = allocator_table.as_ptr();
    let ap = &mut allocator as *mut _ as usize;
    ALLOCATOR.with(|a| *a.borrow_mut() = ap);
    let mut cache = ap;
    let _cache_connections = fake_data(0x00B2_07DC, (&mut cache as *mut usize).cast());
    let _cache_deals = fake_data(0x00B2_0D58, (&mut cache as *mut usize).cast());
    CACHED_ALLOCATOR.with(|a| *a.borrow_mut() = &mut cache as *mut usize as usize);
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
            visited: 1,
        },
        Connection {
            table: table.as_ptr(),
            kind: 2,
            x: -1,
            z: 5,
            city: 7,
            visited: 1,
        },
    ];
    let values: Vec<_> = connections.iter_mut().map(|c| c as *mut _ as u32).collect();
    let mut source = List::new(&values);
    let mut destination = List::new(&[123, 456]);
    let mut list_header = destination.ptr();
    let lp = &mut list_header as *mut _ as *mut u8;
    let mut object = Object::new(0x48);
    let p = object.ptr();
    let sim = unsafe { NeighborsSim::from_ptr(p) };
    sim.set_connections(source.ptr());
    sim.set_deals(source.ptr());
    let cases = [
        (fc4p_006cff30 as *const () as usize, false),
        (fc4p_006d0010 as *const () as usize, true),
        (fc4p_006d0760 as *const () as usize, false),
        (fc4p_006d0840 as *const () as usize, true),
    ];
    for (entry, with_city) in cases {
        for lazy in [false, true] {
            cache = if lazy { 0 } else { ap };
            assert_eq!(cache, if lazy { 0 } else { ap });
            let mut args = vec![2];
            if with_city {
                args.push(7);
            }
            args.push(lp as u32);
            assert_eq!(unsafe { run(entry, p, &args, 0, 0) }, 0x7788_99AA_AABB_CCDD);
            assert_eq!(cache, ap);
            let mut expected = vec![Call::Clear(lp as usize)];
            let mut matched = Vec::new();
            for (i, &value) in values.iter().enumerate() {
                expected.push(Call::Kind(value as usize));
                if i == 1 {
                    continue;
                }
                if with_city {
                    expected.push(Call::City(value as usize));
                    if i == 2 {
                        continue;
                    }
                }
                expected.push(Call::AddRef(value as usize, 0));
                if lazy && matched.is_empty() {
                    expected.extend([
                        Call::Framework,
                        Call::Service(0x3AE4_BEA3, 0x3AE4_BEAB),
                        Call::ReleaseLocal(ap, ap),
                    ]);
                }
                expected.extend([
                    Call::Allocate(12),
                    Call::AddRef(value as usize, 0),
                    Call::Release(value as usize),
                ]);
                matched.push(value);
            }
            assert_eq!(take_calls(), expected);
            let header = destination.ptr();
            let mut node = unsafe { pointer(header, 0) };
            let mut previous = header;
            let mut actual = Vec::new();
            while node != header {
                assert_eq!(unsafe { pointer(node, 4) }, previous);
                actual.push(unsafe { word(node, 8) });
                previous = node;
                node = unsafe { pointer(node, 0) };
            }
            assert_eq!(actual, matched);
            assert_eq!(unsafe { pointer(header, 4) }, previous);
            ALLOCATIONS.with(|a| a.borrow_mut().clear());
        }
    }
}

#[test]
fn filtered_lists_empty_and_missing_matches_keep_original_return_words() {
    let _clear_connections = fake(0x0041_1D70, clear as *const () as usize);
    let _clear_deals = fake(0x0042_1660, clear as *const () as usize);
    let mut source = List::new(&[]);
    let mut destination = List::new(&[]);
    let mut object = Object::new(0x48);
    let p = object.ptr();
    let sim = unsafe { NeighborsSim::from_ptr(p) };
    sim.set_connections(source.ptr());
    sim.set_deals(source.ptr());
    for (entry, city) in [
        (fc4p_006cff30 as *const () as usize, false),
        (fc4p_006d0010 as *const () as usize, true),
        (fc4p_006d0760 as *const () as usize, false),
        (fc4p_006d0840 as *const () as usize, true),
    ] {
        let mut args = vec![2];
        if city {
            args.push(7);
        }
        args.push(destination.ptr() as u32);
        assert_eq!(
            unsafe { run(entry, p, &args, 0, 0) },
            pair(source.ptr() as u32, 0xAABB_CCDD)
        );
        assert_eq!(take_calls(), [Call::Clear(destination.ptr() as usize)]);
    }
    let mut table = connection_table();
    table[0x20 / 4] = connection_kind as *const () as usize;
    table[0x1C / 4] = connection_city as *const () as usize;
    let mut connection = Connection {
        table: table.as_ptr(),
        kind: 5,
        x: 0,
        z: 0,
        city: 8,
        visited: 0,
    };
    let cp = &mut connection as *mut _ as u32;
    let mut source = List::new(&[cp]);
    sim.set_connections(source.ptr());
    sim.set_deals(source.ptr());
    assert_eq!(
        unsafe { fc4p_006cff30(p, 0, 2, destination.ptr()) },
        pair(5, 0x1111_2222)
    );
    assert_eq!(
        take_calls(),
        [Call::Clear(destination.ptr() as usize), Call::Kind(cp as usize)]
    );
    assert_eq!(
        unsafe { fc4p_006d0840(p, 0, 5, 7 as *mut u8, destination.ptr()) },
        pair(8, 0x3333_4444)
    );
    assert_eq!(
        take_calls(),
        [
            Call::Clear(destination.ptr() as usize),
            Call::Kind(cp as usize),
            Call::City(cp as usize)
        ]
    );
}

#[repr(C)]
struct Deal {
    table: *const usize,
    kind: u32,
    flags: u32,
    cost: u64,
}

extern "thiscall" fn deal_flags(this: *mut Deal, mask: u32) -> u64 {
    record(Call::Flags(this as usize, mask));
    pair(
        0xABCD_EF00 | u32::from(unsafe { (*this).flags } & mask != 0),
        0x1111_2222,
    )
}

extern "thiscall" fn deal_kind(this: *mut Deal) -> u64 {
    record(Call::DealKind(this as usize));
    pair(unsafe { (*this).kind }, 0x2222_3333)
}

extern "thiscall" fn deal_cost(this: *mut Deal) -> u64 {
    record(Call::Cost(this as usize));
    unsafe { (*this).cost }
}

fn deal_table() -> [usize; 40] {
    let mut table = [0; 40];
    table[0x20 / 4] = deal_kind as *const () as usize;
    table[0x58 / 4] = deal_cost as *const () as usize;
    table[0x9C / 4] = deal_flags as *const () as usize;
    table
}

#[test]
fn core_monthly_totals_flag_cases_and_call_order() {
    let table = deal_table();
    let mut deals: Vec<_> = [
        (0x4773_FFE0, 10),
        (0x4773_FFE2, 20),
        (0x4773_FFE4, 40),
        (0x4773_FFE1, 100),
        (0x4773_FFE3, 200),
        (0x4773_FFE5, 400),
        (0x4773_FFE6, 900),
    ]
    .into_iter()
    .map(|(kind, cost)| Deal {
        table: table.as_ptr(),
        kind,
        cost,
        flags: 2,
    })
    .collect();
    let values: Vec<_> = deals.iter_mut().map(|d| d as *mut _ as u32).collect();
    let mut list = List::new(&values);
    let mut object = Object::new(0x48);
    let p = object.ptr();
    unsafe { NeighborsSim::from_ptr(p) }.set_deals(list.ptr());
    for flag in [0, 1, 4, 8, 0x10, 0x20, 0x40, 0x80, 0x100, 0x200, 0x8000_0000] {
        deals[0].flags = 2 | flag;
        deals[3].flags = 2 | flag;
        assert_eq!(deals[0].flags, 2 | flag);
        let excluded = flag & 0x3D != 0;
        for (entry, first, expected) in [
            (
                fc4p_006cf860 as *const () as usize,
                0x4773_FFE1u32,
                if excluded { 600 } else { 700 },
            ),
            (
                fc4p_006cf8d0 as *const () as usize,
                0x4773_FFE0,
                if excluded { 60 } else { 70 },
            ),
        ] {
            let before = unsafe { core::slice::from_raw_parts(p, 0x48) }.to_vec();
            assert_eq!(unsafe { run(entry, p, &[], 0, 0xDEAD_BEEF) }, expected);
            let mut expected_calls = Vec::new();
            for (d, &address) in deals.iter().zip(&values) {
                expected_calls.push(Call::Flags(address as usize, 0x3D));
                if d.flags & 0x3D == 0 {
                    expected_calls.push(Call::DealKind(address as usize));
                    if [0, 2, 4].contains(&d.kind.wrapping_sub(first)) {
                        expected_calls.push(Call::Cost(address as usize));
                    }
                }
            }
            assert_eq!(take_calls(), expected_calls);
            assert_eq!(unsafe { core::slice::from_raw_parts(p, 0x48) }, before);
        }
    }
}

#[test]
fn totals_wrap_all_64_bits_and_do_not_require_active() {
    let table = deal_table();
    let mut deals = [
        Deal {
            table: table.as_ptr(),
            kind: 0x4773_FFE1,
            cost: u64::MAX,
            flags: 0,
        },
        Deal {
            table: table.as_ptr(),
            kind: 0x4773_FFE3,
            cost: 0x8000_0000_0000_0002,
            flags: 0x40,
        },
        Deal {
            table: table.as_ptr(),
            kind: 0x4773_FFE5,
            cost: 0x8000_0000_0000_0000,
            flags: 0x80,
        },
    ];
    let values: Vec<_> = deals.iter_mut().map(|d| d as *mut _ as u32).collect();
    let mut list = List::new(&values);
    let mut object = Object::new(0x48);
    let p = object.ptr();
    let sim = unsafe { NeighborsSim::from_ptr(p) };
    sim.set_deals(list.ptr());
    assert_eq!(unsafe { fc4p_006cf860(p, 0) }, 1);
    take_calls();
    assert_eq!(unsafe { fc4p_006cf8d0(p, 0) }, 0);
    take_calls();
    let mut empty = List::new(&[]);
    sim.set_deals(empty.ptr());
    assert_eq!(unsafe { fc4p_006cf860(p, 0xDEAD_BEEF) }, 0);
    assert_eq!(unsafe { fc4p_006cf8d0(p, 0xDEAD_BEEF) }, 0);
    assert_eq!(take_calls(), []);
}

#[repr(C)]
struct Stream {
    table: *const usize,
    bytes: Vec<u8>,
    error: u32,
    sim: *mut u8,
    replacement_header: *mut u8,
}

extern "thiscall" fn version(this: *mut Stream, value: u32) -> u64 {
    record(Call::Version(value));
    unsafe { (*this).bytes.extend_from_slice(&(value as u16).to_le_bytes()) };
    0x1111_2222_AAAA_BBBB
}

extern "thiscall" fn count(this: *mut Stream, value: u32) -> u64 {
    record(Call::Count(value));
    unsafe {
        (*this).bytes.extend_from_slice(&value.to_le_bytes());
        if !(*this).replacement_header.is_null() {
            NeighborsSim::from_ptr((*this).sim).set_connections((*this).replacement_header);
        }
    }
    0x2222_3333_AAAA_BBBB
}

extern "C" fn reference(stream: *mut Stream, value: u32) -> u64 {
    record(Call::Reference(stream as usize, value as usize));
    unsafe { (*stream).bytes.extend_from_slice(&value.to_le_bytes()) };
    0x3333_4444_AAAA_BBBB
}

extern "thiscall" fn stream_error(this: *mut Stream) -> u64 {
    record(Call::Error);
    pair(unsafe { (*this).error }, 0xFACE_CAFE)
}

#[test]
fn version_two_record_keeps_only_connections_and_ignores_intermediate_failures() {
    let _reference = fake(0x005D_99A0, reference as *const () as usize);
    let mut table = [0usize; 19];
    table[0x1C / 4] = version as *const () as usize;
    table[0x24 / 4] = count as *const () as usize;
    table[0x48 / 4] = stream_error as *const () as usize;
    for values in [&[][..], &[0x1111u32, 0x2222, 0x3333][..]] {
        let mut list = List::new(values);
        let mut object = Object::new(0x48);
        let p = object.ptr();
        unsafe { NeighborsSim::from_ptr(p) }.set_connections(list.ptr());
        for error in [0, 1, 0x100, 0xABCD_EF00, 0xABCD_EF80] {
            let mut stream = Stream {
                table: table.as_ptr(),
                bytes: Vec::new(),
                error,
                sim: p,
                replacement_header: core::ptr::null_mut(),
            };
            let sp = &mut stream as *mut _ as *mut u8;
            let before = unsafe { core::slice::from_raw_parts(p, 0x48) }.to_vec();
            assert_eq!(
                unsafe { fc4p_006cfb60(p.add(8), 0, sp) },
                pair(u32::from(error as u8 == 0), 0xFACE_CAFE)
            );
            let mut expected = vec![2, 0];
            expected.extend_from_slice(&(values.len() as u32).to_le_bytes());
            for v in values {
                expected.extend_from_slice(&v.to_le_bytes());
            }
            assert_eq!(stream.bytes, expected);
            let mut expected_calls = vec![Call::Version(2), Call::Count(values.len() as u32)];
            expected_calls.extend(values.iter().map(|&v| Call::Reference(sp as usize, v as usize)));
            expected_calls.push(Call::Error);
            assert_eq!(take_calls(), expected_calls);
            assert_eq!(unsafe { core::slice::from_raw_parts(p, 0x48) }, before);
        }
    }
    let mut old = List::new(&[1, 2]);
    let mut new = List::new(&[3]);
    let mut object = Object::new(0x48);
    let p = object.ptr();
    unsafe { NeighborsSim::from_ptr(p) }.set_connections(old.ptr());
    let mut stream = Stream {
        table: table.as_ptr(),
        bytes: Vec::new(),
        error: 0,
        sim: p,
        replacement_header: new.ptr(),
    };
    let sp = &mut stream as *mut _ as *mut u8;
    assert_eq!(unsafe { fc4p_006cfb60(p.add(8), 0, sp) }, pair(1, 0xFACE_CAFE));
    assert_eq!(
        take_calls(),
        [
            Call::Version(2),
            Call::Count(2),
            Call::Reference(sp as usize, 3),
            Call::Error
        ]
    );
}

#[repr(C)]
struct RemovalList {
    header: *mut u8,
    key: *mut u32,
    change_key: bool,
}

extern "thiscall" fn erase(this: *mut RemovalList, out: *mut u32, iterator: u32) -> u64 {
    record(Call::Erase(this as usize, iterator));
    unsafe {
        let node = iterator as *mut u32;
        let next = node.read();
        let prev = node.add(1).read();
        (next as *mut u32).add(1).write(prev);
        (prev as *mut u32).write(next);
        out.write_unaligned(next);
        node.write(0);
        if (*this).change_key {
            (*this).key.write(20);
        }
    }
    0xABCD_EF01_1234_5678
}

#[test]
fn remove_all_equal_references_caches_next_but_reloads_the_key() {
    let _erase = fake(0x0041_C000, erase as *const () as usize);
    for change_key in [false, true] {
        let mut list = List::new(&[10, 20, 10, 20]);
        let mut key = 10;
        let mut owner = RemovalList {
            header: list.ptr(),
            key: &mut key,
            change_key,
        };
        let p = &mut owner as *mut _ as *mut u8;
        let mut args = [&mut key as *mut u32 as u32];
        assert_eq!(
            unsafe { run_live(fc4p_006cfd30 as *const () as usize, p, &mut args, 0) },
            pair(list.ptr() as u32, 0xABCD_EF01)
        );
        let indices: &[usize] = if change_key { &[1, 2, 4] } else { &[1, 3] };
        let expected: Vec<_> = indices
            .iter()
            .map(|&i| Call::Erase(p as usize, &mut list.nodes[i] as *mut _ as u32))
            .collect();
        assert_eq!(take_calls(), expected);
        let last_next = if change_key {
            list.ptr() as u32
        } else {
            &mut list.nodes[4] as *mut _ as u32
        };
        assert_eq!(args, [last_next]);
    }
    let mut list = List::new(&[5, 6]);
    let mut key = 7;
    let mut owner = RemovalList {
        header: list.ptr(),
        key: &mut key,
        change_key: false,
    };
    let p = &mut owner as *mut _ as *mut u8;
    let original = &mut key as *mut u32 as u32;
    let mut args = [original];
    assert_eq!(
        unsafe { run_live(fc4p_006cfd30 as *const () as usize, p, &mut args, 0xDEAD_BEEF) },
        pair(list.ptr() as u32, 0xDEAD_BEEF)
    );
    assert_eq!(args, [original]);
    assert_eq!(take_calls(), []);
}

extern "C" fn conversion(
    x: i32,
    z: i32,
    rx: *mut i32,
    rz: *mut i32,
    city: *mut *mut u8,
    nx: *mut i32,
    nz: *mut i32,
) -> u64 {
    record(Call::Convert(x, z));
    CONVERSION.with(|settings| {
        let (neighbor, success) = *settings.borrow();
        unsafe {
            rx.write(4);
            rz.write(3);
            city.write(neighbor as *mut u8);
            nx.write(-1);
            nz.write(69);
        }
        pair(success, 0xDEAD_BEEF)
    })
}

#[test]
fn coordinate_overload_uses_the_original_coordinate_slots_on_success_and_failure() {
    let _conversion = fake(0x006C_E920, conversion as *const () as usize);
    let mut table = [0usize; 21];
    table[0x50 / 4] = find as *const () as usize;
    let mut city = Object::new(0x48);
    let cp = city.ptr();
    unsafe {
        cp.cast::<usize>().write(table.as_ptr() as usize);
        cp.add(0x40).cast::<u64>().write_unaligned(0xFACE_CAFE_1234_5678);
    }
    for success in [0xABCD_EF00, 0xABCD_EF01, 0xABCD_EF80] {
        CONVERSION.with(|s| *s.borrow_mut() = (cp as usize, success));
        let mut args = [2, 64, 5];
        let result = unsafe { run_live(fc4p_006cede0 as *const () as usize, core::ptr::null_mut(), &mut args, 0) };
        assert_eq!(args, [2, u32::MAX, 69]);
        if success as u8 == 0 {
            assert_eq!(result, pair(0, 0xDEAD_BEEF));
            assert_eq!(take_calls(), [Call::Convert(64, 5)]);
        } else {
            assert_eq!(result, 0xFACE_CAFE_1234_5678);
            assert_eq!(take_calls(), [Call::Convert(64, 5), Call::Find(cp as usize, 2, -1, 69)]);
        }
    }
}

/// Copies the callee's popped argument words back before any subsequent call can reuse that stack.
unsafe fn run_live(entry: usize, this: *mut u8, args: &mut [u32], edx: u32) -> u64 {
    unsafe { run_live_convention(entry, this, args, edx, Conv::Thiscall) }
}

pub(super) unsafe fn run_live_c(entry: usize, args: &mut [u32]) -> u64 {
    unsafe { run_live_convention(entry, core::ptr::null_mut(), args, 0, Conv::Cdecl) }
}

unsafe fn run_live_convention(entry: usize, this: *mut u8, args: &mut [u32], edx: u32, convention: Conv) -> u64 {
    unsafe { checked_call(entry, this, args, 0, edx, convention) }
}

#[repr(C)]
struct CheckedCall {
    call: RawCall,
    saved_registers: u32,
}

fn float_environment() -> (u16, u16, u32, u32) {
    let (mut cw, mut mxcsr) = (0u16, 0u32);
    let (sw, flags): (u16, u32);
    unsafe {
        core::arch::asm!(
            "fnstcw word ptr [{cw}]", "fnstsw ax", "stmxcsr dword ptr [{mxcsr}]", "pushfd", "pop {flags:e}",
            cw = in(reg) &mut cw, mxcsr = in(reg) &mut mxcsr,
            flags = lateout(reg) flags, out("ax") sw,
        );
    }
    (cw, sw & 0x3800, mxcsr, flags & 0x400)
}

unsafe fn checked_call(entry: usize, this: *mut u8, args: &mut [u32], eax: u32, edx: u32, convention: Conv) -> u64 {
    let _fpu = crate::testing::GameFpu::new();
    let sentinel = 123.25f64;
    let mut restored = 0.0f64;
    unsafe {
        core::arch::asm!("fld qword ptr [{p}]", p = in(reg) &sentinel, out("st(0)") _);
    }
    let before = float_environment();
    let mut call = RawCall::new(convention, entry, this as u32, args);
    call.eax = eax;
    call.edx = edx;
    let mut checked = CheckedCall {
        call,
        saved_registers: 0,
    };
    unsafe {
        core::arch::asm!(
            "push esi", "push ebx", "push ebp",
            "mov ebx, 0x12345678", "mov esi, 0x23456789", "mov ebp, 0x3456789a",
            "mov ecx, [edi + 20]",
            "mov eax, [edi + 16]",
            "2:", "test ecx, ecx", "jz 3f", "dec ecx", "push dword ptr [eax + ecx*4]", "jmp 2b",
            "3:", "mov eax, [edi + 4]", "mov ecx, [edi + 8]", "mov edx, [edi + 12]", "call dword ptr [edi]",
            "mov [edi + 4], eax", "mov [edi + 12], edx",
            "xor eax, eax", "cmp ebx, 0x12345678", "sete al",
            "xor edx, edx", "cmp esi, 0x23456789", "sete dl", "and eax, edx",
            "xor edx, edx", "cmp ebp, 0x3456789a", "sete dl", "and eax, edx",
            "mov [edi + {saved}], eax",
            "mov ecx, [edi + 20]", "mov edx, ecx", "shl edx, 2", "neg edx", "add edx, esp",
            "cmp dword ptr [edi + 24], 0", "je 6f", "mov edx, esp", "6:",
            "mov esi, [edi + 16]",
            "4:", "test ecx, ecx", "jz 5f", "dec ecx", "mov eax, [edx + ecx*4]", "mov [esi + ecx*4], eax", "jmp 4b",
            "5:", "cmp dword ptr [edi + 24], 0", "je 7f", "mov ecx, [edi + 20]", "shl ecx, 2", "add esp, ecx", "7:",
            "pop ebp", "pop ebx", "pop esi",
            saved = const core::mem::offset_of!(CheckedCall, saved_registers),
            in("edi") &mut checked,
            clobber_abi("C"),
        );
    }
    let after = float_environment();
    unsafe {
        core::arch::asm!("fstp qword ptr [{p}]", p = in(reg) &mut restored, out("st(0)") _);
    }
    assert_eq!(
        after, before,
        "floating-point environment and direction flag at {entry:08x}"
    );
    assert_eq!(
        restored.to_bits(),
        sentinel.to_bits(),
        "existing x87 value at {entry:08x}"
    );
    assert_eq!(checked.saved_registers, 1, "callee-saved registers at {entry:08x}");
    pair(checked.call.eax, checked.call.edx)
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
