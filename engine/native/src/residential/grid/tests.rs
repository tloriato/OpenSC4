// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Grid(usize, u32),
    Visit(usize, i32, usize),
}
thread_local! {
    static CALLS:RefCell<Vec<Call>>=const{RefCell::new(vec![])};
    static MUTATE:RefCell<Option<(*mut usize,*mut u8)>>=const{RefCell::new(None)};
}
fn log(c: Call) {
    CALLS.with(|v| v.borrow_mut().push(c));
}
fn take() -> Vec<Call> {
    CALLS.with(|v| core::mem::take(&mut *v.borrow_mut()))
}
struct Storage {
    values: Vec<Vec<u8>>,
    rows: Vec<[usize; 3]>,
    vector: Box<[usize; 3]>,
}
impl Storage {
    fn new(values: Vec<Vec<u8>>) -> Self {
        let mut values = values;
        let rows: Vec<_> = values.iter_mut().map(|r| [r.as_mut_ptr() as usize, 0, 0]).collect();
        let vector = Box::new([rows.as_ptr() as usize, 0, 0]);
        Self { values, rows, vector }
    }
    fn ptr(&mut self) -> *mut u8 {
        self.vector.as_mut_ptr().cast()
    }
    fn cell(&mut self, x: usize, z: usize) -> usize {
        (unsafe { self.values[x].as_mut_ptr().add(z) }) as usize
    }
}
#[repr(C)]
struct Grid {
    vt: *const usize,
    shift: i32,
    x: i32,
    z: i32,
    storage: *mut u8,
}
extern "thiscall" fn shift(p: *mut Grid) -> i32 {
    log(Call::Grid(p as usize, 0x2C));
    unsafe { (*p).shift }
}
extern "thiscall" fn x(p: *mut Grid) -> i32 {
    log(Call::Grid(p as usize, 0x30));
    unsafe { (*p).x }
}
extern "thiscall" fn z(p: *mut Grid) -> i32 {
    log(Call::Grid(p as usize, 0x34));
    unsafe { (*p).z }
}
extern "thiscall" fn dest(p: *mut Grid) -> *mut u8 {
    log(Call::Grid(p as usize, 0x60));
    unsafe { (*p).storage }
}
extern "thiscall" fn source(p: *mut Grid) -> *mut u8 {
    log(Call::Grid(p as usize, 0x64));
    unsafe { (*p).storage }
}
extern "C" fn visit(p: *mut u8, value: i32, context: *mut u8) {
    log(Call::Visit(p as usize, value, context as usize));
    unsafe {
        *p = value as u8;
    }
    MUTATE.with(|m| {
        if let Some((location, pointer)) = m.borrow_mut().take() {
            unsafe {
                *location = pointer as usize;
            }
        }
    });
}
fn vt() -> [usize; 26] {
    let mut v = [0usize; 26];
    for (slot, f) in [
        (0x2C, shift as *const () as usize),
        (0x30, x as *const () as usize),
        (0x34, z as *const () as usize),
        (0x60, dest as *const () as usize),
        (0x64, source as *const () as usize),
    ] {
        v[slot / 4] = f;
    }
    v
}
fn run(a: &mut Grid, b: &mut Grid) -> u32 {
    unsafe {
        fc4p_006f8970(
            (a as *mut Grid).cast(),
            (b as *mut Grid).cast(),
            visit,
            0x1234usize as *mut u8,
        )
    }
}
fn geometry(a: &Grid, b: &Grid) -> Vec<Call> {
    let a = a as *const Grid as usize;
    let b = b as *const Grid as usize;
    vec![
        Call::Grid(a, 0x2C),
        Call::Grid(a, 0x30),
        Call::Grid(a, 0x34),
        Call::Grid(a, 0x60),
        Call::Grid(b, 0x2C),
        Call::Grid(b, 0x30),
        Call::Grid(b, 0x34),
        Call::Grid(b, 0x64),
    ]
}
#[test]
fn equal_grids_copy_unsigned_bytes_in_x_z_order_and_reload_rows_after_callbacks() {
    let vt = vt();
    let mut a = Storage::new(vec![vec![0; 2]; 2]);
    let mut b = Storage::new(vec![vec![255, 128], vec![7, 3]]);
    let mut ga = Grid {
        vt: vt.as_ptr(),
        shift: 4,
        x: 2,
        z: 2,
        storage: a.ptr(),
    };
    let mut gb = Grid {
        vt: vt.as_ptr(),
        shift: 4,
        x: -1,
        z: -2,
        storage: b.ptr(),
    };
    let mut expected = geometry(&ga, &gb);
    for (x, row) in [[255, 128], [7, 3]].into_iter().enumerate() {
        for (z, n) in row.into_iter().enumerate() {
            expected.push(Call::Visit(a.cell(x, z), n, 0x1234));
        }
    }
    assert_eq!(run(&mut ga, &mut gb), 0);
    assert_eq!(take(), expected);
    assert_eq!(a.values, b.values);
    let mut replacement = vec![21u8, 22];
    MUTATE.with(|m| *m.borrow_mut() = Some((b.rows[0..].as_mut_ptr().cast(), replacement.as_mut_ptr())));
    run(&mut ga, &mut gb);
    let calls = take();
    assert_eq!(calls[9], Call::Visit(a.cell(0, 1), 22, 0x1234));
}
#[test]
fn coarser_destination_averages_blocks_and_finer_destination_broadcasts_cached_bytes() {
    let vt = vt();
    let mut a = Storage::new(vec![vec![0; 1]; 2]);
    let mut b = Storage::new(vec![vec![1, 2], vec![3, 5], vec![128, 255], vec![254, 255]]);
    let mut ga = Grid {
        vt: vt.as_ptr(),
        shift: 2,
        x: 2,
        z: 1,
        storage: a.ptr(),
    };
    let mut gb = Grid {
        vt: vt.as_ptr(),
        shift: 1,
        x: 4,
        z: 2,
        storage: b.ptr(),
    };
    assert_eq!(run(&mut ga, &mut gb), 2);
    let mut expected = geometry(&ga, &gb);
    expected.extend([
        Call::Visit(a.cell(0, 0), 2, 0x1234),
        Call::Visit(a.cell(1, 0), 223, 0x1234),
    ]);
    assert_eq!(take(), expected);
    let mut a = Storage::new(vec![vec![0; 4]; 2]);
    let mut b = Storage::new(vec![vec![255, 128]]);
    let mut ga = Grid {
        vt: vt.as_ptr(),
        shift: 0,
        x: 2,
        z: 4,
        storage: a.ptr(),
    };
    let mut gb = Grid {
        vt: vt.as_ptr(),
        shift: 1,
        x: 1,
        z: 2,
        storage: b.ptr(),
    };
    let mut expected = geometry(&ga, &gb);
    for (z, value) in [255, 128].into_iter().enumerate() {
        for x in 0..2 {
            for dz in z * 2..z * 2 + 2 {
                expected.push(Call::Visit(a.cell(x, dz), value, 0x1234));
            }
        }
    }
    assert_eq!(run(&mut ga, &mut gb), 1);
    assert_eq!(take(), expected);
    assert_eq!(a.values, vec![vec![255, 255, 128, 128]; 2]);
}
#[test]
fn empty_negative_and_modulo_shift_geometry_keeps_branch_specific_results() {
    let vt = vt();
    let mut a = Storage::new(vec![vec![0]]);
    let mut b = Storage::new(vec![vec![200]]);
    for (sa, sb, ax, bx, expected) in [
        (0, 0, -1, 1, u32::MAX),
        (1, 0, -1, 1, 0),
        (0, 1, 1, -2, (-2i32) as u32),
        (0, 0, 0, 1, 0),
    ] {
        let mut ga = Grid {
            vt: vt.as_ptr(),
            shift: sa,
            x: ax,
            z: 1,
            storage: a.ptr(),
        };
        let mut gb = Grid {
            vt: vt.as_ptr(),
            shift: sb,
            x: bx,
            z: 1,
            storage: b.ptr(),
        };
        assert_eq!(run(&mut ga, &mut gb), expected);
        assert_eq!(take(), geometry(&ga, &gb));
    }
    let mut ga = Grid {
        vt: vt.as_ptr(),
        shift: 32,
        x: 1,
        z: 1,
        storage: a.ptr(),
    };
    let mut gb = Grid {
        vt: vt.as_ptr(),
        shift: 0,
        x: 1,
        z: 1,
        storage: b.ptr(),
    };
    assert_eq!(run(&mut ga, &mut gb), 1);
    assert_eq!(take().last(), Some(&Call::Visit(a.cell(0, 0), 200, 0x1234)));
}
