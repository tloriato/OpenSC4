// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, fake_data, GameFpu, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Cell(usize, u32),
    Altitude(usize, u32),
}
#[derive(Default)]
struct State {
    calls: Vec<Call>,
    cells: Vec<usize>,
    heights: Vec<f64>,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
extern "thiscall" fn cell(this: *mut u8, key: u32) -> *mut u8 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Cell(this as usize, key));
        s.cells.remove(0) as *mut u8
    })
}
extern "thiscall" fn altitude(this: *mut u8, vertex: u32) -> f64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Altitude(this as usize, vertex));
        s.heights.remove(0)
    })
}
#[test]
fn empty_solution_returns_fixed_cost_and_biased_float_bits_in_edx() {
    let _fpu = GameFpu::new();
    let mut bias = 12_582_912.0f32;
    let _g = fake_data(0x00AA_84F8, (&mut bias as *mut f32).cast());
    let mut tool = Object::new(0x360);
    let p = tool.ptr();
    unsafe {
        write(p, 0x328, 1.0f32);
    }
    for fixed in [0u32, 17, u32::MAX] {
        unsafe {
            write(p, 0x220, fixed);
        }
        assert_eq!(
            unsafe { fc4p_00625640(p) },
            registers(fixed, 0x4B40_0000u32.wrapping_add(fixed))
        );
    }
}
#[test]
fn new_cell_price_is_unsigned_maximum_and_terrain_visits_every_existing_cell_corner() {
    let _fpu = GameFpu::new();
    let _f = fake(0x0063_3220, cell as *const () as usize);
    let mut flags = [0u32; 13];
    for (i, f) in flags.iter_mut().enumerate() {
        *f = 1 << i;
    }
    let mut prices = [0u8; 13 * 0x114];
    unsafe {
        write(prices.as_mut_ptr(), 0, 10u32);
        write(prices.as_mut_ptr(), 12 * 0x114, 0x8000_0000u32);
    }
    let mut bias = 12_582_912.0f32;
    let _a = fake_data(0x00AA_835C, flags.as_mut_ptr().cast());
    let _b = fake_data(0x00B4_53A4, prices.as_mut_ptr());
    let _c = fake_data(0x00AA_84F8, (&mut bias as *mut f32).cast());
    let mut table = [0usize; 13];
    table[12] = altitude as *const () as usize;
    let mut terrain = [table.as_ptr() as usize];
    let t = terrain.as_mut_ptr().cast::<u8>();
    let mut tool = Object::new(0x360);
    let p = tool.ptr();
    unsafe {
        NetworkTool::from_ptr(p).set_terrain(t);
        write(p, 0x328, 0.25f32);
        write(p, 0x220, 7u32);
    }
    let mut records = [[0u8; 0xB8]; 3];
    for (i, r) in records.iter_mut().enumerate() {
        unsafe {
            write(r.as_mut_ptr(), 8, 0x1001u32);
            write(r.as_mut_ptr(), 0x10, u8::from(i != 1));
            write(r.as_mut_ptr(), 0x52, u8::from(i == 2));
            for k in 0..4 {
                write(r.as_mut_ptr(), 0xA8 + k * 4, (i * 4 + k) as u32);
                write(r.as_mut_ptr(), 0x58 + k * 4, 4.0f32);
            }
        }
    }
    let mut entries = [[0u32, 0, 99], [0, 0, 100], [0, 0, 101], [0, 0, 102]];
    unsafe {
        write(p, 0x98, entries.as_mut_ptr());
        write(p, 0x9C, entries.as_mut_ptr().add(4));
    }
    STATE.with(|s| {
        *s.borrow_mut() = State {
            cells: vec![
                records[0].as_mut_ptr() as usize,
                0,
                records[1].as_mut_ptr() as usize,
                records[2].as_mut_ptr() as usize,
            ],
            heights: vec![2.; 12],
            ..State::default()
        }
    });
    let result = unsafe { fc4p_00625640(p) };
    assert_eq!(result, registers(0x8000_000D, 0x4B40_000D));
    let mut expected = vec![];
    for i in 0..4 {
        expected.push(Call::Cell(p as usize, 99 + i));
        if i != 1 {
            let record_index = if i == 0 { 0 } else { i - 1 };
            for k in 0..4 {
                expected.push(Call::Altitude(t as usize, record_index * 4 + k));
            }
        }
    }
    assert_eq!(STATE.with(|s| core::mem::take(&mut s.borrow_mut().calls)), expected);
}
#[test]
fn terrain_cost_uses_unrounded_returned_altitudes_and_ties_to_even_bias_conversion() {
    let _fpu = GameFpu::new();
    let _f = fake(0x0063_3220, cell as *const () as usize);
    let mut bias = 12_582_912.0f32;
    let _g = fake_data(0x00AA_84F8, (&mut bias as *mut f32).cast());
    let mut table = [0usize; 13];
    table[12] = altitude as *const () as usize;
    let mut terrain = [table.as_ptr() as usize];
    let mut tool = Object::new(0x360);
    let p = tool.ptr();
    let mut record = [0u8; 0xB8];
    let mut entry = [0u32; 3];
    unsafe {
        NetworkTool::from_ptr(p).set_terrain(terrain.as_mut_ptr().cast());
        write(p, 0x328, 1.0f32);
        write(p, 0x98, entry.as_mut_ptr());
        write(p, 0x9C, entry.as_mut_ptr().add(3));
    }
    for (displacement, expected) in [(0.5, 0u32), (1.5, 2), (2.5, 2), (3.5, 4)] {
        STATE.with(|s| {
            *s.borrow_mut() = State {
                cells: vec![record.as_mut_ptr() as usize],
                heights: vec![displacement, 0., 0., 0.],
                ..State::default()
            }
        });
        assert_eq!(unsafe { fc4p_00625640(p) }, registers(expected, 0x4B40_0000 + expected));
    }
    unsafe {
        write(record.as_mut_ptr(), 0x58, 1.0f32);
        write(p, 0x328, 100_000_000.0f32);
    }
    STATE.with(|s| {
        *s.borrow_mut() = State {
            cells: vec![record.as_mut_ptr() as usize],
            heights: vec![0.999_999_99, 0., 0., 0.],
            ..State::default()
        }
    });
    assert_eq!(unsafe { fc4p_00625640(p) }, registers(1, 0x4B40_0001));
}
#[test]
fn bridge_tunnel_and_support_integer_costs_wrap_and_use_the_original_record_strides() {
    let _fpu = GameFpu::new();
    let mut bias = 12_582_912.0f32;
    let _a = fake_data(0x00AA_84F8, (&mut bias as *mut f32).cast());
    let mut arrays = [0u8; 13 * 0x114];
    let mut bridge_info = [0u8; 2 * 0x178];
    unsafe {
        write(arrays.as_mut_ptr(), 12 * 0x114, bridge_info.as_mut_ptr());
        write(bridge_info.as_mut_ptr(), 0x178 + 0x150, 100u32);
        write(bridge_info.as_mut_ptr(), 0x178 + 0x154, 30u32);
    }
    let _b = fake_data(0x00B4_52F8, arrays.as_mut_ptr());
    let mut support = [0u8; 0xC0];
    unsafe {
        write(support.as_mut_ptr(), 4, u32::MAX);
        write(support.as_mut_ptr(), 0xBC, 0xFFFF_FFF0u32);
    }
    let mut buckets = [support.as_mut_ptr() as u32];
    let mut begin = buckets.as_mut_ptr() as u32;
    let mut end = begin + 4;
    let _c = fake_data(0x00B4_52B4, (&mut begin as *mut u32).cast());
    let _d = fake_data(0x00B4_52B8, (&mut end as *mut u32).cast());
    let mut bridges = [12u32, 5, 8, 1, 0, 0, 0, 0, 0, 0];
    let mut tunnels = [0u32, 0, 7, 5];
    let mut supports = [u32::MAX, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let mut tool = Object::new(0x360);
    let p = tool.ptr();
    unsafe {
        write(p, 0x328, 1.0f32);
        write(p, 0x320, 4u32);
        write(p, 0x324, 3u32);
        write(p, 0x220, 7u32);
        write(p, 0xD8, bridges.as_mut_ptr());
        write(p, 0xDC, bridges.as_mut_ptr().add(10));
        write(p, 0xE4, tunnels.as_mut_ptr());
        write(p, 0xE8, tunnels.as_mut_ptr().add(4));
        write(p, 0xCC, supports.as_mut_ptr());
        write(p, 0xD0, supports.as_mut_ptr().add(10));
    }
    // 3*(30+4)+100 - 2*3 -16 +7 =187.
    assert_eq!(unsafe { fc4p_00625640(p) }, registers(187, 0x4B40_0007));
}
