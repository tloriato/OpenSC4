// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{call_thiscall_with_eax, fake, fake_data, GameFpu, Object};
use std::cell::RefCell;

thread_local! {
    static CALLS: RefCell<Vec<(usize, u32, Vec<u32>)>> = const { RefCell::new(Vec::new()) };
    static ANSWERS: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
}

fn record(this: *mut u8, slot: u32, args: &[u32]) -> u32 {
    CALLS.with(|calls| calls.borrow_mut().push((this as usize, slot, args.to_vec())));
    ANSWERS.with(|answers| {
        let mut answers = answers.borrow_mut();
        if answers.is_empty() {
            0xABCD_EF00
        } else {
            answers.remove(0)
        }
    })
}

fn calls() -> Vec<(usize, u32, Vec<u32>)> {
    CALLS.with(|calls| core::mem::take(&mut *calls.borrow_mut()))
}

extern "thiscall" fn add_ref(this: *mut u8) -> u32 {
    record(this, 4, &[])
}
extern "thiscall" fn shutdown(this: *mut u8) -> u32 {
    // Change the vtable to ensure Release fetches the destructor from the current table.
    let replacement = unsafe { read_unaligned(this.add(0x18).cast::<usize>()) };
    if replacement != 0 {
        unsafe { write_unaligned(this.cast::<usize>(), replacement) };
    }
    record(this, 0x10, &[])
}
extern "thiscall" fn destroy(this: *mut u8, flags: u32) -> u32 {
    record(this, 0x28, &[flags])
}
extern "thiscall" fn network_type(this: *mut u8, kind: u32) -> u32 {
    record(this, 0x58, &[kind])
}
extern "thiscall" fn network_cell(this: *mut u8, x: i32, z: i32, flags: u32) -> u32 {
    record(this, 0x20, &[x as u32, z as u32, flags])
}
extern "thiscall" fn zone_cell(this: *mut u8, x: i32, z: i32) -> u32 {
    // A zone query may change which manager and zone the developer uses next.
    let owner = unsafe { read_unaligned(this.add(4).cast::<*mut u8>()) };
    if !owner.is_null() {
        let developer = unsafe { ZoneDeveloper::from_ptr(owner) };
        let replacement = unsafe { read_unaligned(this.add(8).cast::<*mut u8>()) };
        developer.set_zone_manager(replacement);
        developer.set_current_zone(0);
    }
    record(this, 0x30, &[x as u32, z as u32])
}
extern "thiscall" fn is_rci(this: *mut u8, zone: u32) -> u32 {
    record(this, 0x34, &[zone])
}
extern "thiscall" fn is_water(this: *mut u8, x: i32, z: i32) -> u32 {
    record(this, 0x88, &[x as u32, z as u32])
}

#[test]
fn density_table_and_unsigned_boundaries() {
    // The zone-density table from the core zone tests, including types outside the RCI range.
    for (zone, density) in [
        (1, 1),
        (4, 1),
        (2, 2),
        (5, 2),
        (3, 3),
        (6, 3),
        (7, 3),
        (8, 3),
        (9, 3),
        (0, 0),
        (10, 0),
        (16, 0),
        (u32::MAX, 0),
        (0x8000_0000, 0),
    ] {
        assert_eq!(unsafe { fc4p_00458960(zone) }, density);
    }
}

#[test]
fn class_identifier() {
    let result = unsafe {
        call_thiscall_with_eax(
            fc4p_00729eb0 as *const () as usize,
            core::ptr::null_mut(),
            &[],
            0xABCD_EF12,
        )
    };
    assert_eq!(result.0, 0x498F_9B01);
}

#[test]
fn nibble_masks_include_byte_carries() {
    // Masks beyond 0x0F retain the arithmetic's byte carries and alternating nibble selection.
    for (mask, value, expected) in [
        (0, 0x1234_5678, 1),
        (1, 1, 1),
        (1, 0x11, 1),
        (1, 2, 0),
        (0x10, 0, 1),
        (0x10, 1, 0),
        (0x20, 5, 1),
        (0x100, 0x100, 1),
        (0x100, 0, 0),
        (0, u32::MAX, 1),
        (0x0100_0200, 0xF1AB_02CD, 1),
        (0x0100_0200, 0xF2AB_02CD, 0),
        (0x2020_2020, 0xAFAF_AFAF, 1),
        (u32::MAX, 0x1234_5678, 1),
        (0xF0F0_F0F0, 0, 1),
        (0x1010_1010, 0x1010_1011, 0),
    ] {
        assert_eq!(
            unsafe { fc4p_00729ec0(mask, value) },
            expected,
            "{mask:08x}, {value:08x}"
        );
    }
}

#[test]
fn interfaces_preserve_result_bytes_and_unknown_output() {
    let mut table = [0usize; 2];
    table[1] = add_ref as *const () as usize;
    let mut object = Object::new(0x100);
    let ptr = object.ptr();
    let developer = unsafe { ZoneDeveloper::from_ptr(ptr) };
    developer.set_primary_vtable(table.as_ptr() as u32);
    for (iid, offset) in [
        (1, 0),
        (0xE818_ED2B, 0),
        (0x6819_05BD, 4),
        (0x4522_94AA, 8),
        (0xE4FD_A3D4, 12),
    ] {
        let mut out = core::ptr::null_mut();
        assert_eq!(unsafe { fc4p_00729ef0(ptr, iid, &mut out) }, 0xABCD_EF01);
        assert_eq!(out, ptr.wrapping_add(offset));
        assert_eq!(calls(), [(ptr as usize, 4, vec![])]);
    }
    for iid in [0u32, 2, 0x4522_94A9, 0x4522_94AB, 0x6819_05BC, 0x6819_05BE, u32::MAX] {
        let mut out = 0x1234usize as *mut u8;
        let eax = if iid <= 0x6819_05BD {
            iid.wrapping_sub(0x4522_94AA)
        } else {
            iid
        };
        assert_eq!(unsafe { fc4p_00729ef0(ptr, iid, &mut out) }, eax & !255);
        assert_eq!(out as usize, 0x1234);
        assert!(calls().is_empty());
    }
}

#[test]
fn reference_count_signed_boundary_and_destroy_order() {
    let mut first = [0usize; 11];
    let mut second = [0usize; 11];
    first[4] = shutdown as *const () as usize;
    second[10] = destroy as *const () as usize;
    let mut object = Object::new(0x100);
    let ptr = object.ptr();
    let developer = unsafe { ZoneDeveloper::from_ptr(ptr) };
    for refs in [2, 3, i32::MAX] {
        developer.set_references(refs);
        assert_eq!(unsafe { fc4p_0072a270(ptr) }, (refs - 1) as u32);
        assert_eq!(developer.references(), refs - 1);
        assert!(calls().is_empty());
    }
    for refs in [1, 0, -1, i32::MIN] {
        developer.set_primary_vtable(first.as_ptr() as u32);
        developer.set_references(refs);
        unsafe { write_unaligned(ptr.add(0x18).cast::<usize>(), second.as_ptr() as usize) };
        assert_eq!(unsafe { fc4p_0072a270(ptr) }, 0);
        assert_eq!(developer.references(), refs);
        assert_eq!(calls(), [(ptr as usize, 0x10, vec![]), (ptr as usize, 0x28, vec![1])]);
    }
}

#[unsafe(naked)]
unsafe extern "thiscall" fn invoke_esi(_this: *mut u8) -> u32 {
    core::arch::naked_asm!("push esi", "mov esi, ecx", "call {port}", "pop esi", "ret",
        port = sym fc4p_00729f90);
}

#[test]
fn network_filter_all_outcomes_and_short_circuit_order() {
    let mut table = [0usize; 23];
    table[22] = network_type as *const () as usize;
    let mut object = [table.as_ptr() as usize];
    let ptr = object.as_mut_ptr().cast::<u8>();
    for broad in [0u8, 1, 0x80] {
        let mut mode = broad;
        let _global = fake_data(0x00B4_B391, &mut mode);
        for mask in 0u32..16 {
            let mut wanted = vec![3];
            let first = mask & 1 != 0;
            let second = mask & 2 != 0;
            let third = mask & 4 != 0;
            let fourth = mask & 8 != 0;
            let mut answers = vec![if first { 0xABCD_0101 } else { 0xABCD_0100 }];
            if !first {
                wanted.push(11);
                answers.push(if second { 0xABCD_0102 } else { 0xABCD_0100 });
            }
            let query_other = (broad != 0 && !first && !second) || (broad == 0 && (first || second));
            if query_other {
                wanted.push(10);
                answers.push(u32::from(third));
                if !third {
                    wanted.push(0);
                    answers.push(u32::from(fourth));
                }
            }
            ANSWERS.with(|a| *a.borrow_mut() = answers);
            let expected = if broad != 0 {
                first || second || third || fourth
            } else {
                (first || second) && !third && !fourth
            };
            assert_eq!(unsafe { invoke_esi(ptr) }, u32::from(expected));
            let expected_calls: Vec<_> = wanted
                .into_iter()
                .map(|kind| (ptr as usize, 0x58, vec![kind]))
                .collect();
            assert_eq!(calls(), expected_calls);
        }
    }
}

#[test]
fn cell_predicates_forward_arguments_and_full_eax() {
    let mut table = [0usize; 35];
    table[8] = network_cell as *const () as usize;
    table[12] = zone_cell as *const () as usize;
    table[13] = is_rci as *const () as usize;
    table[34] = is_water as *const () as usize;
    let mut manager = [table.as_ptr() as usize, 0, 0];
    let manager_ptr = manager.as_mut_ptr().cast::<u8>();
    let mut object = Object::new(0x100);
    let ptr = object.ptr();
    let developer = unsafe { ZoneDeveloper::from_ptr(ptr) };
    developer.set_primary_vtable(table.as_ptr() as u32);
    developer.set_zone_manager(manager_ptr);
    developer.set_terrain(manager_ptr);
    assert_eq!(unsafe { fc4p_0072a010(ptr, -7, i32::MAX) }, 0xABCD_EF00);
    assert_eq!(
        calls(),
        [(ptr as usize, 0x20, vec![(-7i32) as u32, i32::MAX as u32, 0x1F4F])]
    );
    assert_eq!(
        unsafe { call_thiscall_with_eax(fc4p_0072a030 as *const () as usize, ptr, &[9, (-3i32) as u32], 0x1234) }.0,
        0xABCD_EF00
    );
    assert_eq!(calls(), [(manager_ptr as usize, 0x88, vec![9, (-3i32) as u32])]);

    for selected in [0xABCD_EF00, 1, u32::MAX] {
        developer.set_current_zone(selected);
        assert_eq!(
            unsafe { fc4p_0072a040(ptr, 2, 3) },
            0xABCD_EF00 | u32::from(selected == 0xABCD_EF00)
        );
        assert_eq!(calls(), [(manager_ptr as usize, 0x30, vec![2, 3])]);
    }
    developer.set_current_zone(0);
    assert_eq!(unsafe { fc4p_0072a040(ptr, 5, 6) }, 0xABCD_EF00);
    assert_eq!(
        calls(),
        [
            (manager_ptr as usize, 0x30, vec![5, 6]),
            (manager_ptr as usize, 0x34, vec![0xABCD_EF00])
        ]
    );

    let mut replacement = [table.as_ptr() as usize];
    unsafe {
        write_unaligned(manager_ptr.add(4).cast::<usize>(), ptr as usize);
        write_unaligned(manager_ptr.add(8).cast::<usize>(), replacement.as_mut_ptr() as usize);
    }
    developer.set_current_zone(99);
    unsafe { fc4p_0072a040(ptr, 1, 4) };
    assert_eq!(
        calls(),
        [
            (manager_ptr as usize, 0x30, vec![1, 4]),
            (replacement.as_mut_ptr() as usize, 0x34, vec![0xABCD_EF00])
        ]
    );
}

extern "thiscall" fn refill(this: *mut u8) -> *mut u8 {
    record(this, 0x0090_CF9A, &[]);
    let node = unsafe { read_unaligned(this.cast::<*mut u8>()) };
    let replacement = unsafe { read_unaligned(this.add(4).cast::<*mut u8>()) };
    unsafe { write_unaligned(pool_slot(), replacement) };
    node
}

#[test]
fn pooled_node_unlinks_refills_and_wraps_count() {
    let mut next = [0xDEAD_BEEFu32; 4];
    let mut node = [next.as_mut_ptr() as usize, 0xCC, 0xCC];
    let mut first = [0usize, 0, node.as_mut_ptr() as usize, u32::MAX as usize];
    let mut second = [0usize, 0, 0, 7];
    let mut slot = first.as_mut_ptr().cast::<u8>();
    let mut storage = [node.as_mut_ptr() as usize, second.as_mut_ptr() as usize];
    let _slot = fake_data(0x00B0_D3B0, (&mut slot as *mut *mut u8).cast());
    let _pool = fake_data(0x00B0_D3AC, storage.as_mut_ptr().cast());
    let _refill = fake(0x0090_CF9A, refill as *const () as usize);
    assert_eq!(unsafe { fc4p_006bf430() }, node.as_mut_ptr().cast());
    assert_eq!(first, [0, 0, next.as_mut_ptr() as usize, 0]);
    assert!(calls().is_empty());
    first[2] = 0;
    assert_eq!(unsafe { fc4p_006bf430() }, node.as_mut_ptr().cast());
    assert_eq!(first, [0, 0, 0, 0]);
    assert_eq!(second, [0, 0, next.as_mut_ptr() as usize, 8]);
    assert_eq!(node[1..], [0xCC, 0xCC]);
    assert_eq!(calls(), [(storage.as_mut_ptr() as usize, 0x0090_CF9A, vec![])]);
}

extern "thiscall" fn position(this: *mut u8, out: *mut f32) -> u32 {
    unsafe { core::ptr::copy_nonoverlapping(this.add(4).cast::<f32>(), out, 3) };
    record(this, 0x20, &[])
}
extern "fastcall" fn occupant_lot(this: *mut u8, entry_edx: u32, x: i32, z: i32, out: u32) -> u64 {
    (u64::from(entry_edx) << 32) | u64::from(record(this, 0x18, &[x as u32, z as u32, out]))
}

#[test]
fn occupant_position_cells_correct_ties_and_forward_result() {
    let _fpu = GameFpu::new();
    let (mut scale, mut half_value, mut bias) = (0.0625f32, 0.5f32, 12_582_912.0f32);
    let _scale = fake_data(0x00AB_36C0, (&mut scale as *mut f32).cast());
    let _half = fake_data(0x00A8_4D2C, (&mut half_value as *mut f32).cast());
    let _bias = fake_data(0x00AB_36D4, (&mut bias as *mut f32).cast());
    let mut occupant_table = [0usize; 9];
    occupant_table[8] = position as *const () as usize;
    let mut developer_table = [0usize; 7];
    developer_table[6] = occupant_lot as *const () as usize;
    let mut occupant = [occupant_table.as_ptr() as u32, 0, f32::NAN.to_bits(), 0];
    let occupant_ptr = occupant.as_mut_ptr().cast::<u8>();
    let mut developer = [developer_table.as_ptr() as usize];
    let developer_ptr = developer.as_mut_ptr().cast::<u8>();
    for (x, z, cx, cz) in [
        (0.0, 16.0, 0, 1),
        (-16.0, -32.0, -1, -2),
        (-0.001, 15.999, -1, 0),
        (16.001, 32.0, 1, 2),
        (32.0, 48.0, 2, 3),
        (-32.001, -15.999, -3, -1),
        (1_000_000.0, -1_000_000.0, 62_500, -62_500),
        (f32::INFINITY, f32::NEG_INFINITY, 0x3440_0000, 0xB440_0000u32 as i32),
        (
            f32::from_bits(0x7FC1_2345),
            f32::from_bits(0xFFC1_2345),
            0x3481_2345,
            0xB481_2345u32 as i32,
        ),
        (f32::from_bits(0x7F81_2345), -0.0, 0x3481_2345, 0),
        (134_217_728.0, 16.0, 6_291_456, 1),
    ] {
        occupant[1] = f32::to_bits(x);
        occupant[3] = f32::to_bits(z);
        assert_eq!(
            unsafe { fc4p_0072a110(developer_ptr, occupant_ptr) },
            (u64::from(cz as u32) << 32) | 0xABCD_EF00
        );
        assert_eq!(
            calls(),
            [
                (occupant_ptr as usize, 0x20, vec![]),
                (developer_ptr as usize, 0x18, vec![cx as u32, cz as u32, 0])
            ]
        );
    }
}
