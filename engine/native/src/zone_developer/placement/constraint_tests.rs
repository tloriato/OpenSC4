// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use crate::testing::{fake, fake_data, Fake, GameFpu, Object};
use std::cell::RefCell;

game_fn! {fn invoke(this:*mut u8,x:i32,z:i32,w:i32,h:i32,facing:i32,configuration:*mut u8)->u32 = "thiscall" 0x0072_B480;}

#[derive(Debug, PartialEq)]
enum Call {
    Records,
    Size,
    Cell(u32, u32),
    Rotate(i32, i32, i32, [i32; 4]),
    Water(i32, i32),
    Land(i32, i32),
    Bounds(i32, i32),
    Navigate(i32, i32, u32, u32),
    PrimaryWater(i32, i32),
}
#[derive(Default)]
struct State {
    calls: Vec<Call>,
    records: usize,
    not_water: Vec<(i32, i32)>,
    land: Vec<(i32, i32)>,
    out_of_bounds: Vec<(i32, i32)>,
    not_navigable: Vec<(i32, i32)>,
    wet: Vec<(i32, i32)>,
    shrink_vector: usize,
    shrink_after_water: bool,
    extent: Option<(i32, i32)>,
}
thread_local! {static STATE:RefCell<State>=RefCell::new(State::default());}

extern "thiscall" fn records(_this: *mut u8) -> *mut u8 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Records);
        s.records as *mut u8
    })
}
extern "thiscall" fn size(_this: *mut u8, w: *mut i32, h: *mut i32) {
    let (width, height) = STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Size);
        s.extent.unwrap_or((4, 5))
    });
    unsafe {
        w.write(width);
        h.write(height);
    }
}
extern "thiscall" fn cell(_this: *mut u8, x: f32, z: f32, cx: *mut i32, cz: *mut i32) {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Cell(x.to_bits(), z.to_bits())));
    unsafe {
        cx.write((x / 16.0).floor() as i32);
        cz.write((z / 16.0).floor() as i32);
    }
}
extern "C" fn rotate(f: i32, w: i32, h: i32, input: *const i32, output: *mut i32) {
    let r = unsafe { input.cast::<[i32; 4]>().read() };
    STATE.with(|s| s.borrow_mut().calls.push(Call::Rotate(f, w, h, r)));
    let [x, z, x2, z2] = r;
    let r = match f {
        0 => [w - 1 - x2, h - 1 - z2, w - 1 - x, h - 1 - z],
        1 => [z, w - 1 - x2, z2, w - 1 - x],
        2 => r,
        3 => [h - 1 - z2, x, h - 1 - z, x2],
        _ => return,
    };
    unsafe {
        output.cast::<[i32; 4]>().write(r);
    }
}
extern "thiscall" fn water(_this: *mut u8, x: i32, z: i32) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Water(x, z));
        if s.shrink_after_water {
            s.shrink_after_water = false;
            unsafe {
                let v = s.shrink_vector as *mut u32;
                v.add(1).write(v.read().wrapping_add(52));
            }
        }
        0xAABB_0000 | u32::from(!s.not_water.contains(&(x, z)))
    })
}
extern "thiscall" fn land(_this: *mut u8, x: i32, z: i32) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Land(x, z));
        0xAABB_0000 | u32::from(s.land.contains(&(x, z)))
    })
}
extern "thiscall" fn bounds(_this: *mut u8, x: i32, z: i32) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Bounds(x, z));
        0xAABB_0000 | u32::from(!s.out_of_bounds.contains(&(x, z)))
    })
}
extern "thiscall" fn navigate(_this: *mut u8, x: i32, z: i32, a: u32, b: u32) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Navigate(x, z, a, b));
        0xAABB_0000 | u32::from(!s.not_navigable.contains(&(x, z)))
    })
}
extern "thiscall" fn primary_water(_this: *mut u8, x: i32, z: i32) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::PrimaryWater(x, z));
        0xAABB_0000 | u32::from(s.wet.contains(&(x, z)))
    })
}

struct World {
    dev: Object,
    configuration: Box<[usize; 1]>,
    _configuration_table: Box<[usize; 24]>,
    _city_table: Box<[usize; 100]>,
    _terrain_table: Box<[usize; 36]>,
    _primary_table: Box<[usize; 10]>,
    _navigation_table: Box<[usize; 9]>,
    _city: Box<[usize; 1]>,
    _terrain: Box<[usize; 1]>,
    _navigation: Box<[usize; 1]>,
    _navigation_slot: Box<*mut u8>,
    _bias: Box<f32>,
    vector: Box<[u32; 2]>,
    data: Vec<[u32; 13]>,
    values: Box<[u32; 1]>,
    _fakes: Vec<Fake>,
}

impl World {
    fn new(kinds: &[u8], subtype: Option<u32>) -> Self {
        STATE.with(|s| *s.borrow_mut() = State::default());
        let mut configuration_table = Box::new([0usize; 24]);
        configuration_table[7] = size as *const () as usize;
        configuration_table[23] = records as *const () as usize;
        let configuration = Box::new([configuration_table.as_ptr() as usize]);
        let mut city_table = Box::new([0usize; 100]);
        city_table[95] = cell as *const () as usize;
        city_table[99] = bounds as *const () as usize;
        let mut terrain_table = Box::new([0usize; 36]);
        terrain_table[34] = water as *const () as usize;
        terrain_table[35] = land as *const () as usize;
        let mut primary_table = Box::new([0usize; 10]);
        primary_table[9] = primary_water as *const () as usize;
        let mut navigation_table = Box::new([0usize; 9]);
        navigation_table[8] = navigate as *const () as usize;
        let mut city = Box::new([city_table.as_ptr() as usize]);
        let mut terrain = Box::new([terrain_table.as_ptr() as usize]);
        let mut navigation = Box::new([navigation_table.as_ptr() as usize]);
        let mut navigation_slot = Box::new(navigation.as_mut_ptr().cast());
        let mut bias = Box::new(8f32);
        let mut dev = Object::new(0xEC);
        let d = unsafe { super::super::ZoneDeveloper::from_ptr(dev.ptr()) };
        d.set_city(city.as_mut_ptr().cast());
        d.set_terrain(terrain.as_mut_ptr().cast());
        d.set_primary_vtable(primary_table.as_ptr() as u32);
        let values = Box::new([subtype.unwrap_or(0)]);
        let mut data = vec![[0u32; 13]; kinds.len()];
        for (r, &kind) in data.iter_mut().zip(kinds) {
            r[0] = u32::from(kind);
            r[4] = 32f32.to_bits();
            r[5] = 32f32.to_bits();
            r[6] = 0f32.to_bits();
            r[7] = 0f32.to_bits();
            r[10] = values.as_ptr() as u32;
            r[11] = r[10] + u32::from(subtype.is_some()) * 4;
        }
        let vector = Box::new([
            data.as_ptr() as u32,
            (data.as_ptr() as u32).wrapping_add(data.len() as u32 * 52),
        ]);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.records = vector.as_ptr() as usize;
        });
        let fakes = vec![
            fake(0x004B_DFD0, rotate as *const () as usize),
            fake_data(0x00A8_E178, (&mut *bias as *mut f32).cast()),
            fake_data(0x00B4_3CF0, (&mut *navigation_slot as *mut *mut u8).cast()),
        ];
        Self {
            dev,
            configuration,
            _configuration_table: configuration_table,
            _city_table: city_table,
            _terrain_table: terrain_table,
            _primary_table: primary_table,
            _navigation_table: navigation_table,
            _city: city,
            _terrain: terrain,
            _navigation: navigation,
            _navigation_slot: navigation_slot,
            _bias: bias,
            vector,
            data,
            values,
            _fakes: fakes,
        }
    }
    fn run(&mut self, facing: i32) -> (u32, Vec<Call>) {
        let before = self.data.clone();
        let _fpu = GameFpu::new();
        let result = unsafe {
            invoke(
                self.dev.ptr(),
                10,
                20,
                i32::MIN,
                i32::MAX,
                facing,
                self.configuration.as_mut_ptr().cast(),
            )
        };
        assert_eq!(self.data, before);
        assert_eq!(self.values.len(), 1);
        assert_eq!(self.vector[0], self.data.as_ptr() as u32);
        (result, STATE.with(|s| core::mem::take(&mut s.borrow_mut().calls)))
    }
}

#[test]
fn no_constraint_records_returns_free_after_records_and_size_queries() {
    for kinds in [vec![], vec![0, 4, 7, 255]] {
        let mut w = World::new(&kinds, None);
        assert_eq!(w.run(2), (8, vec![Call::Records, Call::Size]));
    }
}

#[test]
fn water_rows_stop_on_failure_and_continue_later_rows_and_records() {
    let mut w = World::new(&[5, 5], None);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.not_water.push((10, 20));
        s.land.push((11, 21));
    });
    let (result, c) = w.run(2);
    assert_eq!(result, 5);
    let mut expected = vec![Call::Records, Call::Size];
    for _ in 0..2 {
        expected.extend([
            Call::Cell(8f32.to_bits(), 8f32.to_bits()),
            Call::Cell(40f32.to_bits(), 40f32.to_bits()),
            Call::Rotate(2, 4, 5, [0, 0, 1, 1]),
            Call::Water(10, 20),
            Call::Water(10, 21),
            Call::Land(10, 21),
            Call::Water(11, 21),
            Call::Land(11, 21),
        ]);
    }
    assert_eq!(c, expected);
}

#[test]
fn land_constraint_failure_takes_precedence_and_does_not_skip_later_records() {
    let mut w = World::new(&[5, 6, 5], Some(0));
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.not_water.push((10, 20));
        s.wet.push((10, 20));
        s.wet.push((10, 21));
    });
    let (result, c) = w.run(2);
    assert_eq!(result, 6);
    assert_eq!(c.iter().filter(|c| matches!(c, Call::Rotate(..))).count(), 3);
    assert_eq!(
        c.iter()
            .filter(|c| matches!(c, Call::PrimaryWater(..)))
            .collect::<Vec<_>>(),
        [&Call::PrimaryWater(10, 20), &Call::PrimaryWater(10, 21)]
    );
}

#[test]
fn open_water_strip_rotates_to_each_side_and_short_circuits_navigation_outside_bounds() {
    for (facing, orientation, expected) in [
        (2, 0, vec![(10, 18), (11, 18), (10, 19), (11, 19)]),
        (2, 1, vec![(12, 20), (13, 20), (12, 21), (13, 21)]),
        (2, 2, vec![(10, 22), (11, 22), (10, 23), (11, 23)]),
        (2, 3, vec![(8, 20), (9, 20), (8, 21), (9, 21)]),
    ] {
        let mut w = World::new(&[5], Some(1));
        w.data[0][0] |= orientation << 16;
        STATE.with(|s| s.borrow_mut().out_of_bounds.push(expected[0]));
        let (result, c) = w.run(facing);
        assert_eq!(result, 5);
        let visited: Vec<_> = c
            .iter()
            .filter_map(|c| match c {
                Call::Bounds(x, z) => Some((*x, *z)),
                _ => None,
            })
            .collect();
        // The first failed cell ends its row, including the second cell in that row.
        assert_eq!(visited, [expected[0], expected[2], expected[3]]);
        assert!(!c.contains(&Call::Navigate(expected[0].0, expected[0].1, 0, 0)));
    }
}

#[test]
fn constraint_footprint_rotates_and_offsets_the_configuration_rectangle() {
    for (facing, expected) in [
        (0, vec![(12, 23), (13, 23), (12, 24), (13, 24)]),
        (1, vec![(10, 22), (11, 22), (10, 23), (11, 23)]),
        (2, vec![(10, 20), (11, 20), (10, 21), (11, 21)]),
        (3, vec![(13, 20), (14, 20), (13, 21), (14, 21)]),
    ] {
        let mut w = World::new(&[6], None);
        let (result, c) = w.run(facing);
        assert_eq!(result, 8);
        let actual: Vec<_> = c
            .iter()
            .filter_map(|c| match c {
                Call::PrimaryWater(x, z) => Some((*x, *z)),
                _ => None,
            })
            .collect();
        assert_eq!(actual, expected);
    }
}

#[test]
fn record_vector_count_is_read_again_after_callbacks() {
    let mut w = World::new(&[5, 6], None);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.shrink_vector = w.vector.as_mut_ptr() as usize;
        s.shrink_after_water = true;
    });
    let (result, c) = w.run(2);
    assert_eq!(result, 8);
    assert!(!c.iter().any(|c| matches!(c, Call::PrimaryWater(..))));
}

#[test]
fn empty_footprints_still_convert_both_corners_and_rotate() {
    let mut w = World::new(&[5, 6], None);
    for r in &mut w.data {
        r[4] = 0f32.to_bits();
        r[5] = 0f32.to_bits();
    }
    let (result, c) = w.run(2);
    assert_eq!(result, 8);
    assert_eq!(c.iter().filter(|c| matches!(c, Call::Rotate(..))).count(), 2);
    assert!(!c.iter().any(|c| matches!(c, Call::Water(..) | Call::PrimaryWater(..))));
}

#[test]
fn invalid_facing_uses_the_retained_original_rectangle_scratch() {
    let mut w = World::new(&[6], None);
    let _fpu = GameFpu::new();
    let mut frame = [0u32; 64];
    let stack = unsafe { frame.as_mut_ptr().add(40) };
    unsafe {
        stack.add(1).write(10);
        stack.add(2).write(20);
        stack.add(5).write(99);
        stack.add(6).write(w.configuration.as_mut_ptr() as u32);
        stack.sub(25).cast::<[i32; 4]>().write([1, 2, 1, 2]);
        assert_eq!(super::check_constraint_tiles(w.dev.ptr(), stack), 8);
        assert_eq!(stack.sub(25).cast::<[i32; 4]>().read(), [11, 22, 11, 22]);
    }
    assert_eq!(
        STATE.with(|s| s
            .borrow()
            .calls
            .iter()
            .filter_map(|c| match c {
                Call::PrimaryWater(x, z) => Some((*x, *z)),
                _ => None,
            })
            .collect::<Vec<_>>()),
        [(11, 22)]
    );
}

#[test]
fn single_constraint_record_matches_one_by_two_configuration_footprint() {
    let _fpu = GameFpu::new();
    for (facing, expected) in [(2, (30, 31)), (0, (30, 30))] {
        let mut w = World::new(&[5], None);
        w.data[0][4] = 16f32.to_bits();
        w.data[0][5] = 32f32.to_bits();
        w.data[0][6] = 0f32.to_bits();
        w.data[0][7] = 16f32.to_bits();
        STATE.with(|s| s.borrow_mut().extent = Some((1, 2)));
        assert_eq!(
            unsafe { invoke(w.dev.ptr(), 30, 30, 1, 2, facing, w.configuration.as_mut_ptr().cast()) },
            8
        );
        let cells = STATE.with(|s| {
            s.borrow()
                .calls
                .iter()
                .filter_map(|c| match c {
                    Call::Water(x, z) => Some((*x, *z)),
                    _ => None,
                })
                .collect::<Vec<_>>()
        });
        assert_eq!(cells, [expected]);
    }
}
