// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Geometry cases from the core's neighbor tests, with the original output and argument storage.

use super::tests::*;
use super::*;
use crate::testing::{fake_data, Fake};

#[repr(C)]
struct City {
    table: *const usize,
    x: i32,
    z: i32,
    sx: i32,
    sz: i32,
}

extern "thiscall" fn position(this: *mut City, x: *mut i32, z: *mut i32) -> u64 {
    record(Call::Position(this as usize));
    unsafe {
        x.write_unaligned((*this).x);
        z.write_unaligned((*this).z);
    }
    pair(0x1234_5678, 0x1414_1414)
}

extern "thiscall" fn size(this: *mut City, x: *mut i32, z: *mut i32) -> u64 {
    record(Call::Size(this as usize));
    unsafe {
        x.write_unaligned((*this).sx);
        z.write_unaligned((*this).sz);
    }
    pair(0x1234_5678, 0x1C1C_1C1C)
}

extern "thiscall" fn width(this: *mut City) -> u64 {
    record(Call::Width);
    pair(unsafe { (*this).sx } as u32, 0x7474_7474)
}

extern "thiscall" fn height(this: *mut City) -> u64 {
    record(Call::Height);
    pair(unsafe { (*this).sz } as u32, 0x7878_7878)
}

#[repr(C)]
struct Region {
    table: *const usize,
    city: *mut City,
    missing_reference: bool,
}

extern "thiscall" fn regional_city(this: *mut Region, x: i32, z: i32) -> u64 {
    record(Call::RegionalCity(x, z));
    let reference = if unsafe { (*this).missing_reference } {
        0
    } else {
        unsafe { &mut (*this).city as *mut _ as u32 }
    };
    pair(reference, 0x2C2C_2C2C)
}

#[repr(C)]
struct Manager {
    table: *const usize,
    ours: *mut City,
    region: *mut Region,
}

extern "thiscall" fn current(this: *mut Manager) -> u64 {
    record(Call::CurrentCity(this as usize));
    pair(unsafe { (*this).ours } as u32, 0x9090_9090)
}

extern "thiscall" fn region(this: *mut Manager) -> u64 {
    record(Call::Region);
    pair(unsafe { (*this).region } as u32, 0x8C8C_8C8C)
}

struct World {
    ours: Box<City>,
    neighbor: Box<City>,
    region: Box<Region>,
    manager: Box<Manager>,
    _current: Box<City>,
    _manager_global: Box<*mut u8>,
    _city_global: Box<*mut u8>,
    _city_table: Box<[usize; 95]>,
    _region_table: Box<[usize; 12]>,
    _manager_table: Box<[usize; 37]>,
    _globals: [Fake; 2],
}

impl World {
    fn new() -> Self {
        let mut city_table = Box::new([0usize; 95]);
        city_table[0x14 / 4] = position as *const () as usize;
        city_table[0x1C / 4] = size as *const () as usize;
        city_table[0x174 / 4] = width as *const () as usize;
        city_table[0x178 / 4] = height as *const () as usize;
        let mut region_table = Box::new([0usize; 12]);
        region_table[0x2C / 4] = regional_city as *const () as usize;
        let mut manager_table = Box::new([0usize; 37]);
        manager_table[0x8C / 4] = region as *const () as usize;
        manager_table[0x90 / 4] = current as *const () as usize;
        let mut ours = Box::new(City {
            table: city_table.as_ptr(),
            x: 3,
            z: 3,
            sx: 1,
            sz: 1,
        });
        let mut neighbor = Box::new(City {
            table: city_table.as_ptr(),
            x: 1,
            z: 3,
            sx: 2,
            sz: 2,
        });
        let mut region = Box::new(Region {
            table: region_table.as_ptr(),
            city: &mut *neighbor,
            missing_reference: false,
        });
        let mut manager = Box::new(Manager {
            table: manager_table.as_ptr(),
            ours: &mut *ours,
            region: &mut *region,
        });
        let mut city = Box::new(City {
            table: city_table.as_ptr(),
            x: 0,
            z: 0,
            sx: 64,
            sz: 64,
        });
        let mut manager_global = Box::new((&mut *manager as *mut Manager).cast::<u8>());
        let mut city_global = Box::new((&mut *city as *mut City).cast::<u8>());
        let globals = [
            fake_data(0x00B4_3C94, (&mut *manager_global as *mut *mut u8).cast()),
            fake_data(0x00B4_3CEC, (&mut *city_global as *mut *mut u8).cast()),
        ];
        Self {
            ours,
            neighbor,
            region,
            manager,
            _current: city,
            _manager_global: manager_global,
            _city_global: city_global,
            _city_table: city_table,
            _region_table: region_table,
            _manager_table: manager_table,
            _globals: globals,
        }
    }
}

#[test]
fn core_neighboring_region_cells_edges_interior_corners_and_signed_division() {
    let mut w = World::new();
    let ours = &mut *w.ours as *mut _ as u32;
    let manager = &mut *w.manager as *mut _ as usize;
    for ((x, z), (rx, rz), eax, edx) in [
        ((-1, 5), (2, 3), 0u32, 0u32),
        ((64, 5), (4, 3), 0, 4),
        ((5, -1), (3, 2), 2, 63),
        ((5, 64), (3, 4), 4, 1),
        ((10, 20), (3, 3), 0, 0),
        ((-1, -1), (2, 3), 0, 63),
        ((-65, -65), (2, 2), u32::MAX, 63),
        ((127, 0), (4, 3), 0, 4),
        ((0, 127), (3, 4), 4, 1),
        ((i32::MIN, i32::MIN), (2, -33_554_429), (-33_554_432i32) as u32, 63),
    ] {
        for default in [false, true] {
            let mut out = [0x5A5A_5A5Ai32; 2];
            let mut args = [
                x as u32,
                z as u32,
                out.as_mut_ptr() as u32,
                unsafe { out.as_mut_ptr().add(1) } as u32,
                if default { 0 } else { ours },
            ];
            assert_eq!(
                unsafe { run_live_c(geometry::fc4p_006ce820 as *const () as usize, &mut args) },
                pair(eax, edx)
            );
            assert_eq!(out, [rx, rz]);
            assert_eq!(args[0..2], [x as u32, z as u32]);
            assert_eq!(args[4], 1);
            let mut calls = Vec::new();
            if default {
                calls.push(Call::CurrentCity(manager));
            }
            calls.extend([Call::Size(ours as usize), Call::Position(ours as usize)]);
            assert_eq!(take_calls(), calls);
        }
    }
}

#[test]
fn core_neighbor_connection_cells_and_success_return_words() {
    let mut w = World::new();
    let ours = &mut *w.ours as *mut _ as usize;
    let neighbor = &mut *w.neighbor as *mut _ as usize;
    let manager = &mut *w.manager as *mut _ as usize;
    for ((x, z), (rx, rz), (px, pz), (nx, nz), edge) in [
        ((-1, 5), (2, 3), (1, 3), (128, 5), 3),
        ((64, 5), (4, 3), (4, 2), (-1, 69), 1),
        ((5, -1), (3, 2), (2, 1), (69, 128), 0),
        ((5, 64), (3, 4), (3, 4), (5, -1), 2),
        ((-65, -65), (2, 2), (1, 1), (128, 63), 3),
        ((64, -1), (4, 2), (3, 1), (64, 128), 0),
    ] {
        w.neighbor.x = px;
        w.neighbor.z = pz;
        let mut output_city = 0xDEAD_BEEFu32;
        let mut region = [0x5A5A_5A5Ai32; 2];
        let mut cell = [0x5A5A_5A5Ai32; 2];
        let mut args = [
            x as u32,
            z as u32,
            region.as_mut_ptr() as u32,
            unsafe { region.as_mut_ptr().add(1) } as u32,
            &mut output_city as *mut u32 as u32,
            cell.as_mut_ptr() as u32,
            unsafe { cell.as_mut_ptr().add(1) } as u32,
        ];
        let (x_out, z_out) = (args[5], args[6]);
        let result = unsafe { run_live_c(geometry::fc4p_006ce920 as *const () as usize, &mut args) };
        let (eax, edx) = match edge {
            1 | 3 => (z_out, x_out),
            0 => (z_out, 128),
            _ => (nx as u32, z_out),
        };
        assert_eq!(result, pair((eax & !0xFF) | 1, edx));
        assert_eq!(region, [rx, rz]);
        assert_eq!(cell, [nx, nz]);
        assert_eq!(output_city, neighbor as u32);
        assert_eq!(args[0], (z % 64) as u32);
        assert_eq!(args[4], px as u32);
        assert_eq!(
            take_calls(),
            [
                Call::Width,
                Call::Height,
                Call::CurrentCity(manager),
                Call::Position(ours),
                Call::Size(ours),
                Call::Region,
                Call::RegionalCity(rx, rz),
                Call::Position(neighbor),
                Call::Size(neighbor)
            ]
        );
    }
    // An interior cell still resolves our regional record and returns a zero cell.
    w.region.city = &mut *w.ours;
    let mut city = 123u32;
    let (mut rx, mut rz, mut nx, mut nz) = (123, 123, 123, 123);
    let mut args = [
        10,
        20,
        &mut rx as *mut i32 as u32,
        &mut rz as *mut i32 as u32,
        &mut city as *mut u32 as u32,
        &mut nx as *mut i32 as u32,
        &mut nz as *mut i32 as u32,
    ];
    assert_eq!(
        unsafe { run_live_c(geometry::fc4p_006ce920 as *const () as usize, &mut args) },
        pair(1, 0x1C1C_1C1C)
    );
    assert_eq!((city, rx, rz, nx, nz), (ours as u32, 3, 3, 0, 0));
    assert_eq!((args[0], args[4]), (20, 3));
    take_calls();
}

#[test]
fn neighbor_cell_clears_outputs_before_lookup_and_stops_at_each_missing_service() {
    for failure in 0..4 {
        let mut w = World::new();
        match failure {
            0 => w.manager.ours = core::ptr::null_mut(),
            1 => w.manager.region = core::ptr::null_mut(),
            2 => w.region.missing_reference = true,
            _ => w.region.city = core::ptr::null_mut(),
        }
        let mut city = 123u32;
        let (mut rx, mut rz, mut nx, mut nz) = (123, 123, 123, 123);
        let mut args = [
            64,
            5,
            &mut rx as *mut i32 as u32,
            &mut rz as *mut i32 as u32,
            &mut city as *mut u32 as u32,
            &mut nx as *mut i32 as u32,
            &mut nz as *mut i32 as u32,
        ];
        let result = unsafe { run_live_c(geometry::fc4p_006ce920 as *const () as usize, &mut args) };
        let expected = match failure {
            0 => pair(0, 0x9090_9090),
            1 => pair(0, 0x8C8C_8C8C),
            2 => pair(0, 0x2C2C_2C2C),
            _ => pair((&mut w.region.city as *mut _ as u32) & !0xFF, 0x2C2C_2C2C),
        };
        assert_eq!(result, expected);
        assert_eq!((city, nx, nz), (0, 0, 0));
        assert_eq!((rx, rz), if failure == 0 { (123, 123) } else { (4, 3) });
        assert_eq!(args[0], if failure == 0 { 64 } else { 5 });
        let calls = take_calls();
        assert_eq!(
            &calls[..3],
            [
                Call::Width,
                Call::Height,
                Call::CurrentCity(&mut *w.manager as *mut _ as usize)
            ]
        );
        assert_eq!(
            calls.len(),
            match failure {
                0 => 3,
                1 => 6,
                _ => 7,
            }
        );
    }
}
