// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::abi::{call_raw, Conv, RawCall};
use crate::testing::{fake, fake_data, GameFpu, Object};
use crate::traffic::test_support::{bytes, invoke, object, table};
use std::cell::RefCell;

#[test]
fn intersection_dirty_bounds_initialization_union_and_only_expected_bytes() {
    let mut o = Object::new(0xD00);
    let s = unsafe { TrafficSim::from_ptr(o.ptr()) };
    let mut expected = unsafe { bytes(o.ptr(), 0xD00) };
    unsafe {
        fc4p_0070eb60(o.ptr(), 5, 6, 7, 8);
    }
    expected[0x81C] = 1;
    for (offset, value) in [(0x820, 5i32), (0x824, 6), (0x828, 7), (0x82C, 8)] {
        expected[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    assert_eq!(unsafe { bytes(o.ptr(), 0xD00) }, expected);
    s.set_intersection_dirty(0xA5);
    unsafe {
        fc4p_0070eb60(o.ptr(), -1, 10, 10, -1);
    }
    assert_eq!(
        (
            s.dirty_x0(),
            s.dirty_z0(),
            s.dirty_x1(),
            s.dirty_z1(),
            s.intersection_dirty()
        ),
        (-1, 6, 10, 8, 0xA5)
    );
    s.set_intersection_dirty(0);
    unsafe {
        fc4p_0070eb60(o.ptr(), i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    }
    assert_eq!(
        (s.dirty_x0(), s.dirty_z0(), s.dirty_x1(), s.dirty_z1()),
        (i32::MAX, i32::MAX, i32::MIN, i32::MIN)
    );
}

#[test]
fn intersection_link_marks_exact_words_shift_modulo_32_and_neighbor_mask() {
    let mut classes = [1i32; 9];
    classes[2] = 2;
    let mut dx = [-1i32, 0, 1, 0];
    let mut dz = [0i32, -1, 0, 1];
    let mut masks = [0x1111u16, 0x2222, 0x4444, 0x8888];
    let _a = fake_data(0x00AB_31A8, classes.as_mut_ptr().cast());
    let _b = fake_data(0x00AB_325C, dx.as_mut_ptr().cast());
    let _c = fake_data(0x00AB_326C, dz.as_mut_ptr().cast());
    let _d = fake_data(0x00B0_D470, masks.as_mut_ptr().cast());
    let mut o = Object::new(0xD00);
    let s = unsafe { TrafficSim::from_ptr(o.ptr()) };
    let mut cells = vec![0xA5u8; 256 * 256 * 46];
    s.set_intersections(cells.as_mut_ptr());
    s.set_min_cell_x(0);
    s.set_min_cell_z(0);
    s.set_max_cell_x(3);
    s.set_max_cell_z(3);
    for (x, z, enter, exit, travel) in [(1, 1, 0, 2, 1), (0, 0, 1, 0, 1), (1, 1, 1, 3, 2), (1, 1, 8, 1, 2)] {
        cells.fill(0xA5);
        let mut expected = cells.clone();
        let offset = (((x << 8) | z) * 46 + travel * 2) as usize;
        let bits = 1u32.wrapping_shl((enter * 4 + exit) as u32) as u16;
        expected[offset..offset + 2].copy_from_slice(&(0xA5A5 | bits).to_le_bytes());
        let (nx, nz) = (x + dx[exit as usize], z + dz[exit as usize]);
        let propagate = travel != 2 && nx >= 0 && nz >= 0 && nx <= 3 && nz <= 3;
        if propagate {
            let offset = (((nx << 8) | nz) * 46 + ((exit + 2) % 4) * 2 + 0x24) as usize;
            expected[offset..offset + 2].copy_from_slice(&(0xA5A5 | masks[enter as usize]).to_le_bytes());
        }
        let r = unsafe {
            invoke(
                fc4p_0070eab0 as *const () as usize,
                o.ptr(),
                &[x as u32, z as u32, enter as u32, exit as u32, travel as u32],
                0,
                0,
                false,
            )
        };
        assert_eq!(cells, expected);
        assert_eq!(
            r.ecx,
            if travel == 2 {
                (enter * 4 + exit) as u32
            } else if propagate {
                u32::from(masks[enter as usize])
            } else {
                nz as u32
            }
        );
    }
}

thread_local! {
    static CALLS:RefCell<Vec<(usize,Vec<u32>)>>=const{RefCell::new(Vec::new())};
    static POPULATIONS:RefCell<Option<[u32;12]>>=const{RefCell::new(None)};
}
fn log(this: *mut u8, args: Vec<u32>) {
    CALLS.with(|c| c.borrow_mut().push((this as usize, args)));
}
fn take() -> Vec<(usize, Vec<u32>)> {
    CALLS.with(|c| core::mem::take(&mut *c.borrow_mut()))
}
extern "thiscall" fn water(
    this: *mut u8,
    x0: u32,
    z0: u32,
    x1: u32,
    z1: u32,
    out: *mut u32,
    route: u32,
    flags: u32,
) -> u64 {
    log(this, vec![x0, z0, x1, z1, route, flags]);
    assert_eq!(unsafe { out.read() }, route);
    unsafe {
        out.write(0xCAFE_BABE);
    }
    0x1234_5678_8765_4321
}
#[test]
fn water_route_forwards_coordinates_and_original_fifth_argument_address() {
    let _f = fake(0x0072_5170, water as *const () as usize);
    let mut o = Object::new(0xD00);
    let s = unsafe { TrafficSim::from_ptr(o.ptr()) };
    s.set_water_paths(0x1234usize as *mut u8);
    let mut stack = [0u32, 1, 2, 3, 4, 5];
    take();
    assert_eq!(
        unsafe { get_water_route(o.ptr(), stack.as_mut_ptr()) },
        0x1234_5678_8765_4321
    );
    assert_eq!(stack, [0, 1, 2, 3, 4, 0xCAFE_BABE]);
    assert_eq!(take(), [(0x1234, vec![1, 2, 3, 4, 5, 0])]);
    let r = unsafe {
        invoke(
            fc4p_0070ec10 as *const () as usize,
            o.ptr(),
            &[6, 7, 8, 9, 10],
            0,
            0,
            false,
        )
    };
    assert_eq!((r.eax, r.edx), (0x8765_4321, 0x1234_5678));
    assert_eq!(take(), [(0x1234, vec![6, 7, 8, 9, 10, 0])]);
}

extern "thiscall" fn developer(this: *mut u8, id: u32) -> *mut u8 {
    log(this, vec![id]);
    unsafe {
        store(this, 4, id);
    }
    this
}
extern "thiscall" fn grid(this: *mut u8) -> *mut u8 {
    log(this, vec![0x3C]);
    this
}
extern "thiscall" fn population(this: *mut u8, x: u32, z: u32) -> u64 {
    log(this, vec![0x1C, x, z]);
    let id = unsafe { word(this, 4) };
    let index = match id {
        0x1010 => 0,
        0x1020 => 1,
        0x1030 => 2,
        0x3110 => 3,
        0x3120 => 4,
        0x3130 => 5,
        0x3320 => 6,
        0x3330 => 7,
        0x4100 => 8,
        0x4200 => 9,
        0x4300 => 10,
        _ => 11,
    };
    let value = POPULATIONS.with(|p| p.borrow().map_or((index as u32 + 1) * 10, |values| values[index]));
    (0xABCD_1234u64 << 32) | u64::from(value)
}

#[test]
fn core_background_example_signed_words_and_extended_exponent_range() {
    use crate::traffic::test_support::float_environment;
    let _fpu = GameFpu::new();
    let vt = table(&[
        (0x80, developer as *const () as usize),
        (0x3C, grid as *const () as usize),
        (0x1C, population as *const () as usize),
    ]);
    let mut manager = object(8, &vt);
    let mut manager_ptr = manager.ptr();
    let _f = fake_data(0x00B4_3D68, (&mut manager_ptr as *mut *mut u8).cast());
    let mut o = Object::new(0xD00);
    for (weights, populations, expected) in [
        ([0.05, 0.2, 0.2], [100, 0, 0, 10, 0, 0, 0, 0, 0, 0, 0, 0], 7.0f32),
        (
            [1.0; 3],
            [0xFFFF_FFFF, 0x1234_8000, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            -32769.0,
        ),
        ([f32::MAX, -f32::MAX, 1.0], [2, 0, 0, 2, 0, 0, 0, 0, 1, 0, 0, 0], 1.0),
    ] {
        POPULATIONS.with(|p| *p.borrow_mut() = Some(populations));
        unsafe {
            for (i, weight) in weights.into_iter().enumerate() {
                store(o.ptr(), 0xCE8 + i * 4, weight.to_bits());
            }
        }
        let sentinel = 123.25f64;
        let mut restored = 0.0f64;
        unsafe {
            core::arch::asm!("fld qword ptr [{p}]", p = in(reg) &sentinel, out("st(0)") _);
        }
        let before = float_environment();
        let args = [2u32, 3];
        let mut call = RawCall::new(
            Conv::Thiscall,
            fc4p_0070e870 as *const () as usize,
            o.ptr() as u32,
            &args,
        );
        call.take_st0 = 1;
        unsafe {
            call_raw(&mut call);
        }
        let after = float_environment();
        unsafe {
            core::arch::asm!("fstp qword ptr [{p}]", p = in(reg) &mut restored, out("st(0)") _);
        }
        assert_eq!(call.st0.to_f32().to_bits(), expected.to_bits());
        assert_eq!(after, before);
        assert_eq!(restored.to_bits(), sentinel.to_bits());
        take();
    }
    POPULATIONS.with(|p| *p.borrow_mut() = None);
}
#[test]
fn t11_background_traffic_queries_all_developers_in_order_and_returns_unrounded_st0() {
    let _fpu = GameFpu::new();
    let vt = table(&[
        (0x80, developer as *const () as usize),
        (0x3C, grid as *const () as usize),
        (0x1C, population as *const () as usize),
    ]);
    let mut manager = object(8, &vt);
    let mut manager_ptr = manager.ptr();
    let _f = fake_data(0x00B4_3D68, (&mut manager_ptr as *mut *mut u8).cast());
    let mut o = Object::new(0xD00);
    unsafe {
        store(o.ptr(), 0xCE8, 0.5f32.to_bits());
        store(o.ptr(), 0xCEC, 0.25f32.to_bits());
        store(o.ptr(), 0xCF0, 0.125f32.to_bits());
    }
    take();
    let args = [2u32, 3];
    let mut c = RawCall::new(
        Conv::Thiscall,
        fc4p_0070e870 as *const () as usize,
        o.ptr() as u32,
        &args,
    );
    c.take_st0 = 1;
    unsafe {
        call_raw(&mut c);
    }
    assert_eq!(c.st0.to_f32(), 157.5);
    assert_eq!((c.eax, c.edx), (o.ptr() as u32, 0xABCD_1234));
    let expected: Vec<_> = [
        0x1010, 0x1020, 0x1030, 0x3110, 0x3120, 0x3130, 0x3320, 0x3330, 0x4100, 0x4200, 0x4300, 0x4400,
    ]
    .into_iter()
    .flat_map(|id| {
        [
            (manager.ptr() as usize, vec![id]),
            (manager.ptr() as usize, vec![0x3C]),
            (manager.ptr() as usize, vec![0x1C, 2, 3]),
        ]
    })
    .collect();
    assert_eq!(take(), expected);
}
