// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Grid-layout tests, including the core's distance-cap and edge-seed examples.

use super::*;
use crate::abi::{call_raw, Conv, RawCall};
use crate::testing::{fake, fake_data, GameFpu, Object};
use std::cell::RefCell;

#[repr(C)]
struct Row {
    begin: *mut u8,
    end: *mut u8,
    limit: *mut u8,
}
struct Grid {
    data: Vec<Vec<u8>>,
    rows: Vec<Row>,
    vector: [u32; 3],
}
impl Grid {
    fn new(data: Vec<Vec<u8>>) -> Self {
        let mut grid = Self {
            data,
            rows: Vec::new(),
            vector: [0; 3],
        };
        grid.rows = grid
            .data
            .iter_mut()
            .map(|r| Row {
                begin: r.as_mut_ptr(),
                end: r.as_mut_ptr().wrapping_add(r.len()),
                limit: r.as_mut_ptr().wrapping_add(r.len()),
            })
            .collect();
        grid.vector[0] = grid.rows.as_mut_ptr() as u32;
        grid
    }
    fn pass(&mut self, nx: i32, nz: i32) -> u32 {
        unsafe { fc4p_005e4e80(self.vector.as_mut_ptr().cast(), nx, nz) }
    }
}

#[test]
fn distance_cap_and_change_mask() {
    let mut g = Grid::new(vec![vec![255]; 300]);
    g.data[0][0] = 0;
    assert_ne!(g.pass(300, 1), 0);
    assert_eq!((g.data[200][0], g.data[280][0]), (200, 255));
    assert_eq!(g.pass(300, 1), 0);
    let mut g = Grid::new(vec![vec![0, 255]]);
    assert_eq!(g.pass(1, 2), 254);
    assert_eq!(g.data, [vec![0, 1]]);
}

#[test]
fn seeded_edge_worked_example_and_empty_dimensions() {
    let mut g = Grid::new((0..4).map(|x| vec![if x == 1 { 0 } else { 255 }, 0, 0, 0]).collect());
    assert_ne!(g.pass(4, 4), 0);
    assert_eq!(g.pass(4, 4), 0);
    assert_eq!(
        g.data,
        [vec![1, 0, 0, 0], vec![0, 0, 0, 0], vec![1, 0, 0, 0], vec![1, 0, 0, 0]]
    );
    for (nx, nz) in [(0, 0), (-1, 3), (3, 0), (3, -1)] {
        let mut g = Grid::new(vec![vec![255; 4]; 4]);
        let before = g.data.clone();
        assert_eq!(g.pass(nx, nz), 0);
        assert_eq!(g.data, before);
    }
}

#[test]
fn distance_pass_matches_manhattan_distance_after_convergence() {
    let seeds = [(1, 5), (6, 2), (9, 9)];
    let mut g = Grid::new(vec![vec![255; 11]; 12]);
    for (x, z) in seeds {
        g.data[x][z] = 0;
    }
    while g.pass(12, 11) != 0 {}
    for x in 0usize..12 {
        for z in 0usize..11 {
            let distance = seeds
                .iter()
                .map(|&(sx, sz)| x.abs_diff(sx) + z.abs_diff(sz))
                .min()
                .unwrap();
            assert_eq!(usize::from(g.data[x][z]), distance);
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Call {
    X(usize),
    Z(usize),
    WidthX(usize),
    WidthZ(usize),
    Rows(usize, i32),
    Row(usize, i32, u32),
}
thread_local! { static CALLS: RefCell<Vec<Call>> = const { RefCell::new(Vec::new()) }; }
fn log(c: Call) {
    CALLS.with(|v| v.borrow_mut().push(c));
}
fn take() -> Vec<Call> {
    CALLS.with(|v| core::mem::take(&mut *v.borrow_mut()))
}
#[repr(C)]
struct City {
    vtable: *const usize,
    x: i32,
    z: i32,
    wx: f64,
    wz: f64,
}
unsafe extern "thiscall" fn count_x(p: *mut City) -> i32 {
    log(Call::X(p as usize));
    unsafe { (*p).x }
}
unsafe extern "thiscall" fn count_z(p: *mut City) -> i32 {
    log(Call::Z(p as usize));
    unsafe { (*p).z }
}
unsafe extern "thiscall" fn width_x(p: *mut City) -> f64 {
    log(Call::WidthX(p as usize));
    unsafe { (*p).wx }
}
unsafe extern "thiscall" fn width_z(p: *mut City) -> f64 {
    log(Call::WidthZ(p as usize));
    unsafe { (*p).wz }
}
extern "thiscall" fn rows(p: *mut u8, n: i32) -> u64 {
    log(Call::Rows(p as usize, n));
    0x1234_5678_9999_0000
}
unsafe extern "thiscall" fn fill(p: *mut u8, n: i32, v: *const u32) -> u64 {
    log(Call::Row(p as usize, n, unsafe { *v }));
    0xAABB_CCDD_9876_5432
}

// This entry keeps the original argument's address so the test can observe its overwritten value.
#[unsafe(naked)]
unsafe extern "thiscall" fn invoke_resize(_this: *mut u8, _size: i32) -> u64 {
    core::arch::naked_asm!("push dword ptr [esp + 4]", "call {port}", "mov edx, dword ptr [esp - 4]", "ret 4",port=sym fc4p_00465630)
}

#[test]
fn tract_geometry_call_order_shifts_widths_and_caller_argument() {
    let _fpu = GameFpu::new();
    let _r = fake(0x0046_23B0, rows as *const () as usize);
    let _f = fake(0x0046_5710, fill as *const () as usize);
    let mut table = [0usize; 95];
    table[0x174 / 4] = count_x as *const () as usize;
    table[0x178 / 4] = count_z as *const () as usize;
    table[0x16C / 4] = width_x as *const () as usize;
    table[0x170 / 4] = width_z as *const () as usize;
    let mut city = City {
        vtable: table.as_ptr(),
        x: 20,
        z: 12,
        wx: 16.0,
        wz: 8.0,
    };
    let cp = (&mut city as *mut City).cast::<u8>();
    let mut global = cp;
    let _city = fake_data(0x00B4_3CEC, (&mut global as *mut *mut u8).cast());
    let mut obj = Object::new(0x50);
    let p = obj.ptr();
    let g = unsafe { U16Grid::from_ptr(p) };
    let mut row_storage = [0u32; 60];
    g.set_rows(row_storage.as_mut_ptr().cast());
    for (size, shift, nx, nz) in [
        (4, 2, 5, 3),
        (3, 1, 10, 6),
        (1, 0, 20, 12),
        (0, 0, 20, 12),
        (-4, 0, 20, 12),
    ] {
        g.set_tract_size(99);
        let result = unsafe { invoke_resize(p, size) };
        assert_eq!(result as u32, nx);
        assert_eq!((result >> 32) as u32, 0);
        assert_eq!(
            (g.tract_size(), g.tract_shift(), g.count_x(), g.count_z()),
            (size, shift, nx as i32, nz)
        );
        assert_eq!(
            g.width_x().to_bits(),
            (F80::from_i32(size) * F80::from_f32(16.0)).to_f32().to_bits()
        );
        let calls = take();
        assert_eq!(
            &calls[..5],
            &[
                Call::X(cp as usize),
                Call::Z(cp as usize),
                Call::WidthX(cp as usize),
                Call::WidthZ(cp as usize),
                Call::Rows(p.wrapping_add(0x38) as usize, nx as i32)
            ]
        );
        for x in 0..nx as usize {
            assert_eq!(
                calls[x + 5],
                Call::Row(row_storage.as_mut_ptr().wrapping_add(x * 3) as usize, nz, 0)
            );
        }
        assert_eq!(calls.len(), 5 + nx as usize);
    }
    // Unchanged size never accesses the city or row helpers, and keeps EDX.
    g.set_tract_size(4);
    let args = [4];
    let mut c = RawCall::new(Conv::Thiscall, fc4p_00465630 as *const () as usize, p as u32, &args);
    c.edx = 0xDEAD_BEEF;
    unsafe {
        call_raw(&mut c);
    }
    assert_eq!((c.eax, c.edx), (4, 0xDEAD_BEEF));
    assert!(take().is_empty());
}

#[test]
fn zero_rows_keep_the_argument_and_geometry_uses_signed_shifts() {
    let _fpu = GameFpu::new();
    let _r = fake(0x0046_23B0, rows as *const () as usize);
    let _f = fake(0x0046_5710, fill as *const () as usize);
    let mut table = [0usize; 95];
    table[0x174 / 4] = count_x as *const () as usize;
    table[0x178 / 4] = count_z as *const () as usize;
    table[0x16C / 4] = width_x as *const () as usize;
    table[0x170 / 4] = width_z as *const () as usize;
    let mut city = City {
        vtable: table.as_ptr(),
        x: -1,
        z: -5,
        wx: 0.0,
        wz: 1e-30,
    };
    let mut global = (&mut city as *mut City).cast::<u8>();
    let _city = fake_data(0x00B4_3CEC, (&mut global as *mut *mut u8).cast());
    let mut obj = Object::new(0x50);
    let p = obj.ptr();
    let g = unsafe { U16Grid::from_ptr(p) };
    let result = unsafe { invoke_resize(p, 4) };
    assert_eq!(result, 0x0000_0004_FFFF_FFFF);
    assert_eq!((g.count_x(), g.count_z()), (-1, -2));
    assert_eq!(g.inverse_x(), f32::INFINITY);
    assert_eq!(take().len(), 5);
}
