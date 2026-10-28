// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake_data, GameFpu, Object};
use std::cell::RefCell;

thread_local! {
    static FLAGS: RefCell<Vec<usize>> = const { RefCell::new(Vec::new()) };
}

#[repr(C)]
struct FloatObject {
    vtable: *const usize,
    flags: u32,
    value: F80,
    padding: u16,
    eax: u32,
    edx: u32,
    calls: u32,
    x: u32,
    z: u32,
}

extern "thiscall" fn flags(this: *mut FloatObject) -> u32 {
    FLAGS.with(|f| f.borrow_mut().push(this as usize));
    unsafe { (*this).flags }
}

#[unsafe(naked)]
unsafe extern "thiscall" fn lot_altitude(_this: *mut u8) {
    core::arch::naked_asm!(
        "inc dword ptr [ecx + 28]",
        "fld tbyte ptr [ecx + 8]",
        "mov eax, [ecx + 20]",
        "mov edx, [ecx + 24]",
        "ret"
    );
}

#[unsafe(naked)]
unsafe extern "thiscall" fn terrain_altitude(_this: *mut u8, _x: f32, _z: f32) {
    core::arch::naked_asm!(
        "inc dword ptr [ecx + 28]",
        "mov eax, [esp + 4]",
        "mov [ecx + 32], eax",
        "mov eax, [esp + 8]",
        "mov [ecx + 36], eax",
        "fld tbyte ptr [ecx + 8]",
        "mov eax, [ecx + 20]",
        "mov edx, [ecx + 24]",
        "ret 8"
    );
}

unsafe fn invoke(this: *mut u8, x: f32, z: f32, ok: *mut u8) -> RawCall {
    let args = [x.to_bits(), z.to_bits(), ok as u32];
    let mut call = RawCall::new(Conv::Thiscall, fc4p_0072a9f0 as *const () as usize, this as u32, &args);
    call.take_st0 = 1;
    unsafe { call_raw(&mut call) };
    call
}

fn float_object(table: &[usize], value: F80) -> FloatObject {
    FloatObject {
        vtable: table.as_ptr(),
        flags: 0x1234_5601,
        value,
        padding: 0xA5A5,
        eax: 0xABCD_1234,
        edx: 0x5678_90AB,
        calls: 0,
        x: 0xCCCC_CCCC,
        z: 0xCCCC_CCCC,
    }
}

#[test]
fn near_boundary_rounding_clamping_flag_bit_and_unrounded_altitude() {
    let _fpu = GameFpu::new();
    let (mut scale, mut offset, mut bias) = (0.0625f32, f32::from_bits(0x3EFF_7CEE), 12_582_912.0f32);
    let _scale = fake_data(0x00B0_D4AC, (&mut scale as *mut f32).cast());
    let _offset = fake_data(0x00A8_F7BC, (&mut offset as *mut f32).cast());
    let _bias = fake_data(0x00AB_36D4, (&mut bias as *mut f32).cast());
    let mut table = [0usize; 17];
    table[12] = flags as *const () as usize;
    table[16] = lot_altitude as *const () as usize;
    let mut terrain_table = [0usize; 14];
    terrain_table[13] = terrain_altitude as *const () as usize;
    let wide = F80::from_bytes([0x34, 0x12, 0, 0, 0, 0, 0, 0x80, 0x00, 0x41]);
    let mut lots: Vec<_> = (0..16).map(|_| float_object(&table, wide)).collect();
    let mut terrain = float_object(
        &terrain_table,
        F80::from_bytes([0xEF, 0xCD, 0xAB, 0, 0, 0, 0, 0x80, 0x00, 0x40]),
    );
    let mut cells = [[0usize; 4]; 4];
    for x in 0..4 {
        for z in 0..4 {
            cells[x][z] = (&mut lots[x * 4 + z] as *mut FloatObject) as usize;
        }
    }
    let mut rows = [[0usize; 3]; 4];
    for x in 0..4 {
        rows[x][0] = cells[x].as_mut_ptr() as usize;
    }
    let mut object = Object::new(0x60);
    let ptr = object.ptr();
    let manager = unsafe { LotManager::from_ptr(ptr) };
    manager.set_cells_x(4);
    manager.set_cells_z(4);
    manager.set_rows(rows.as_mut_ptr().cast());
    unsafe { write_unaligned(ptr.add(0x30).cast::<*mut FloatObject>(), &mut terrain) };
    for (position, expected) in [
        (15.98f32, 0usize),
        (15.99, 1),
        (16.0, 1),
        (-8.0, 0),
        (-100.0, 0),
        (0.0, 0),
        (31.99, 2),
        (48.0, 3),
        (10000.0, 3),
        (f32::INFINITY, 3),
        (f32::NEG_INFINITY, 0),
        (f32::from_bits(0x7FC1_2345), 3),
        (f32::from_bits(0xFFC1_2345), 0),
        (f32::from_bits(0x7F81_2345), 3),
    ] {
        for z in [0usize, 2, 3] {
            let mut ok = [0xA5u8; 3];
            let call = unsafe { invoke(ptr, position, z as f32 * 16.0, ok.as_mut_ptr().wrapping_add(1)) };
            assert_eq!(call.st0, wide);
            assert_eq!((call.eax, call.edx), (0xABCD_1234, 0x5678_90AB));
            assert_eq!(ok, [0xA5, 1, 0xA5]);
            assert_eq!(
                FLAGS.with(|f| core::mem::take(&mut *f.borrow_mut())),
                [cells[expected][z]]
            );
        }
    }
    let index = 6;
    for flag in [0u32, 2, 0xFFFF_FFFE] {
        lots[index].flags = flag;
        let mut ok = [0xA5u8; 3];
        let call = unsafe { invoke(ptr, 16.0, 32.0, ok.as_mut_ptr().wrapping_add(1)) };
        assert_eq!(call.st0, terrain.value);
        assert_eq!(ok, [0xA5, 0, 0xA5]);
        assert_eq!((terrain.x, terrain.z), (16.0f32.to_bits(), 32.0f32.to_bits()));
        assert_eq!(FLAGS.with(|f| core::mem::take(&mut *f.borrow_mut())), [cells[1][2]]);
    }
    let previous = terrain.calls;
    unsafe { write_unaligned(cells[1].as_mut_ptr().add(2), 0) };
    let call = unsafe { invoke(ptr, 16.0, 32.0, core::ptr::null_mut()) };
    assert_eq!(call.st0, terrain.value);
    assert_eq!(terrain.calls, previous + 1);
    assert!(FLAGS.with(|f| f.borrow().is_empty()));
    unsafe {
        write_unaligned(
            cells[1].as_mut_ptr().add(2),
            (&mut lots[index] as *mut FloatObject) as usize,
        )
    };
    lots[index].flags = 1;
    assert_eq!(unsafe { invoke(ptr, 16.0, 32.0, core::ptr::null_mut()) }.st0, wide);
    FLAGS.with(|f| f.borrow_mut().clear());
}

#[test]
fn float_environment_and_existing_stack_value_survive() {
    let _fpu = GameFpu::new();
    let (mut scale, mut offset, mut bias) = (0.0625f32, f32::from_bits(0x3EFF_7CEE), 12_582_912.0f32);
    let _scale = fake_data(0x00B0_D4AC, (&mut scale as *mut f32).cast());
    let _offset = fake_data(0x00A8_F7BC, (&mut offset as *mut f32).cast());
    let _bias = fake_data(0x00AB_36D4, (&mut bias as *mut f32).cast());
    let mut table = [0usize; 14];
    table[13] = terrain_altitude as *const () as usize;
    let mut terrain = float_object(&table, F80::from_f32(123.5));
    let mut cells = [0usize];
    let mut rows = [cells.as_mut_ptr() as usize, 0, 0];
    let mut object = Object::new(0x60);
    let ptr = object.ptr();
    let manager = unsafe { LotManager::from_ptr(ptr) };
    manager.set_cells_x(1);
    manager.set_cells_z(1);
    manager.set_rows(rows.as_mut_ptr().cast());
    unsafe { write_unaligned(ptr.add(0x30).cast::<*mut FloatObject>(), &mut terrain) };
    let bottom = F80::from_f32(37.25);
    let mut after = F80::ZERO;
    let before_cw = crate::x87::control_word();
    let mut before_mxcsr = 0u32;
    let mut after_mxcsr = 0u32;
    unsafe {
        core::arch::asm!("stmxcsr [{mx}]","fld tbyte ptr [{bottom}]",
        mx=in(reg) &mut before_mxcsr,bottom=in(reg) &bottom,out("st(0)") _)
    };
    let call = unsafe { invoke(ptr, 1.0, 2.0, core::ptr::null_mut()) };
    unsafe {
        core::arch::asm!("fstp tbyte ptr [{bottom}]","stmxcsr [{mx}]",
        mx=in(reg) &mut after_mxcsr,bottom=in(reg) &mut after,out("st(0)") _)
    };
    assert_eq!(call.st0, terrain.value);
    assert_eq!(bottom, after);
    assert_eq!(crate::x87::control_word(), before_cw);
    assert_eq!(before_mxcsr, after_mxcsr);
}

thread_local! {
    static STATES: RefCell<Vec<usize>> = const { RefCell::new(Vec::new()) };
}

extern "fastcall" fn state(this: *mut u8, entry_edx: u32) -> u64 {
    STATES.with(|s| s.borrow_mut().push(this as usize));
    assert_eq!(entry_edx, unsafe { read_unaligned(this.cast::<u32>()) });
    let state = unsafe { read_unaligned(this.add(4).cast::<u32>()) };
    let edx = unsafe { read_unaligned(this.add(8).cast::<u32>()) };
    (u64::from(edx) << 32) | u64::from(state)
}

#[test]
fn adjustment_range_deduplicates_in_scan_order_and_visits_all_states() {
    let mut table = [0usize; 34];
    table[33] = state as *const () as usize;
    let mut lots = [
        [table.as_ptr() as u32, 0u32, 0xABCD_0000],
        [table.as_ptr() as u32, 1u32, 0xABCD_0001],
        [table.as_ptr() as u32, 2u32, 0xABCD_0002],
        [table.as_ptr() as u32, u32::MAX, 0xABCD_0003],
    ];
    let mut object = Object::new(0x60);
    let ptr = object.ptr();
    let manager = unsafe { LotManager::from_ptr(ptr) };
    manager.set_cells_x(2);
    manager.set_cells_z(2);
    for variant in 0u32..625 {
        let mut v = variant;
        let mut cells = [[0u32; 2]; 2];
        let mut expected = vec![];
        let mut advanced = false;
        let mut last = 0u64;
        for cell in cells.iter_mut().flatten() {
            let selected = v % 5;
            v /= 5;
            if selected != 0 {
                let index = selected as usize - 1;
                let lot = lots[index].as_mut_ptr() as u32;
                *cell = lot;
                if !expected.contains(&(lot as usize)) {
                    expected.push(lot as usize);
                    advanced |= index == 2;
                    last = (u64::from(lots[index][2]) << 32) | u64::from(lots[index][1]);
                }
            }
        }
        let mut rows = [
            [cells[0].as_mut_ptr() as u32, 0, 0],
            [cells[1].as_mut_ptr() as u32, 0, 0],
        ];
        manager.set_rows(rows.as_mut_ptr().cast());
        let mut output = [0xA5A5_A5A5u32; 4];
        let result = unsafe { fc4p_0072ae20(ptr, 1, 1, output.as_mut_ptr().add(1), output.as_mut_ptr().add(2)) };
        assert_eq!(result, last);
        let value = if advanced { 0x4200_0000 } else { 0x7F7F_FFFF };
        assert_eq!(output, [0xA5A5_A5A5, value, value, 0xA5A5_A5A5]);
        assert_eq!(STATES.with(|s| core::mem::take(&mut *s.borrow_mut())), expected);
    }
    // State comparison is signed, and a shared output reference is written with the same limit.
    for value in [i32::MIN, -1, 0, 1, 2, 3, i32::MAX] {
        lots[0][1] = value as u32;
        let mut cells = [lots[0].as_mut_ptr() as u32];
        let mut rows = [cells.as_mut_ptr() as u32, 0, 0];
        manager.set_cells_x(1);
        manager.set_cells_z(1);
        manager.set_rows(rows.as_mut_ptr().cast());
        let mut output = 0u32;
        let result = unsafe { fc4p_0072ae20(ptr, 0, 0, &mut output, &mut output) };
        assert_eq!(result, (0xABCD_0000u64 << 32) | u64::from(value as u32));
        assert_eq!(output, if value >= 2 { 0x4200_0000 } else { 0x7F7F_FFFF });
        assert_eq!(
            STATES.with(|s| core::mem::take(&mut *s.borrow_mut())),
            [lots[0].as_ptr() as usize]
        );
    }
}

#[test]
fn adjustment_range_clamps_edges_and_keeps_empty_scan_registers() {
    let mut object = Object::new(0x60);
    let ptr = object.ptr();
    let manager = unsafe { LotManager::from_ptr(ptr) };
    let mut cells = [[0u32; 2]; 2];
    let mut rows = [
        [cells[0].as_mut_ptr() as u32, 0, 0],
        [cells[1].as_mut_ptr() as u32, 0, 0],
    ];
    manager.set_rows(rows.as_mut_ptr().cast());
    for (nx, nz, x, z, eax, edx) in [
        (2, 2, 0, 0, 0, 0),
        (2, 2, 2, 2, 0, 0),
        (2, 2, -1, 0, 0, u32::MAX),
        (2, 2, 0, -1, 0, 1),
        (0, 2, 0, 0, 0, u32::MAX),
        (2, 0, 0, 0, 0, 1),
        (2, 2, 3, 1, 2, 1),
        (2, 2, i32::MIN, 0, i32::MAX as u32, i32::MIN as u32),
    ] {
        manager.set_cells_x(nx);
        manager.set_cells_z(nz);
        let mut first = 0u32;
        let mut second = 0u32;
        let result = unsafe { fc4p_0072ae20(ptr, x, z, &mut first, &mut second) };
        assert_eq!(result, (u64::from(edx) << 32) | u64::from(eax));
        assert_eq!((first, second), (0x7F7F_FFFF, 0x7F7F_FFFF));
        assert!(STATES.with(|s| s.borrow().is_empty()));
    }
}
