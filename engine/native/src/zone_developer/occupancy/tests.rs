// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, Object};
use std::cell::RefCell;

type Call = (u32, usize, Vec<u32>);
thread_local! {
    static CALLS: RefCell<Vec<Call>> = const { RefCell::new(Vec::new()) };
    static ANSWERS: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
}
fn answer(code: u32, this: *mut u8, args: &[u32]) -> u32 {
    CALLS.with(|c| c.borrow_mut().push((code, this as usize, args.to_vec())));
    ANSWERS.with(|a| {
        let mut a = a.borrow_mut();
        if a.is_empty() {
            0
        } else {
            a.remove(0)
        }
    })
}
fn calls() -> Vec<Call> {
    CALLS.with(|c| core::mem::take(&mut *c.borrow_mut()))
}
fn answers(values: &[u32]) {
    ANSWERS.with(|a| *a.borrow_mut() = values.to_vec());
}
extern "thiscall" fn lookup(this: *mut u8, cell: *const i32) -> *mut u8 {
    let cell = unsafe { read_unaligned(cell.cast::<[u32; 2]>()) };
    answer(0x0072_AC60, this, &cell) as *mut u8
}
extern "thiscall" fn get_lot(this: *mut u8, x: i32, z: i32, origin: u32) -> u32 {
    answer(0x18, this, &[x as u32, z as u32, origin])
}
extern "thiscall" fn delete_tree(this: *mut u8, root: *mut u8) -> u32 {
    answer(0x0072_ABE0, this, &[root as u32])
}
extern "thiscall" fn reset(this: *mut u8) -> u32 {
    answer(0x14, this, &[])
}
extern "thiscall" fn clear_view(this: *mut u8) -> u32 {
    answer(0x24, this, &[])
}
extern "thiscall" fn region(this: *mut u8, x: i32, z: i32) -> u32 {
    answer(0x0046_6610, this, &[x as u32, z as u32])
}
extern "thiscall" fn zone(this: *mut u8, x: i32, z: i32) -> u32 {
    answer(0x0072_A040, this, &[x as u32, z as u32])
}
extern "thiscall" fn network(this: *mut u8, x: i32, z: i32) -> u32 {
    answer(0x0072_A010, this, &[x as u32, z as u32])
}
extern "thiscall" fn is_occupied(this: *mut u8, x: i32, z: i32) -> u32 {
    answer(0x0072_C050, this, &[x as u32, z as u32])
}
extern "thiscall" fn water(this: *mut u8, x: i32, z: i32) -> u32 {
    answer(0x24, this, &[x as u32, z as u32])
}
extern "thiscall" fn config(this: *mut u8) -> u32 {
    answer(0x38, this, &[])
}
extern "thiscall" fn code(this: *mut u8, x: i32, z: i32, zone: u32, water: u32) -> i32 {
    answer(0x0072_C200, this, &[x as u32, z as u32, zone, water]) as i32
}

#[test]
fn occupancy_real_and_highlight_modes_keep_lookup_eax_bytes() {
    let _lookup = fake(0x0072_AC60, lookup as *const () as usize);
    let mut table = [0usize; 7];
    table[6] = get_lot as *const () as usize;
    let mut object = Object::new(0x100);
    let ptr = object.ptr();
    let developer = unsafe { ZoneDeveloper::from_ptr(ptr) };
    developer.set_lot_manager_vtable(table.as_ptr() as u32);
    developer.set_deleted_lot_header(0xABCD_1200usize as *mut u8);
    developer.set_virtual_lot_header(0x8765_4300usize as *mut u8);
    for (real, deleted, lot, created) in [
        (true, false, 0, false),
        (true, false, 0x1122_3300, false),
        (false, false, 0, false),
        (false, false, 0, true),
        (false, false, 0x1122_3300, false),
        (false, true, 0, false),
        (false, true, 0, true),
    ] {
        developer.set_real_mode(u8::from(real));
        let deleted_node = if deleted { 0xABCD_1255 } else { 0xABCD_1200 };
        let created_node = if created { 0x8765_43FF } else { 0x8765_4300 };
        let mut script = vec![];
        let mut expected = vec![];
        if !real {
            script.push(deleted_node);
            expected.push((0x0072_AC60, ptr.wrapping_add(0xE4) as usize, vec![2, 3]));
        }
        if real || !deleted {
            script.push(lot);
            expected.push((0x18, ptr.wrapping_add(4) as usize, vec![2, 3, 0]));
        }
        let result = if (real || !deleted) && lot != 0 {
            (lot & !255) | 1
        } else if real {
            0
        } else {
            script.push(created_node);
            expected.push((0x0072_AC60, ptr.wrapping_add(0xD8) as usize, vec![2, 3]));
            (created_node & !255) | u32::from(created)
        };
        answers(&script);
        let before = unsafe { core::slice::from_raw_parts(ptr, 0x100) }.to_vec();
        assert_eq!(unsafe { fc4p_0072c050(ptr, 2, 3) }, result);
        assert_eq!(calls(), expected);
        assert_eq!(unsafe { core::slice::from_raw_parts(ptr, 0x100) }, before);
    }
}

#[test]
fn clear_highlight_all_presence_combinations_only_reset_nonempty_maps() {
    let _clear = fake(0x0072_ABE0, delete_tree as *const () as usize);
    let mut table = [0usize; 10];
    table[5] = reset as *const () as usize;
    table[9] = clear_view as *const () as usize;
    let mut street = [table.as_ptr() as usize];
    let mut road = [table.as_ptr() as usize];
    let mut renderer = [table.as_ptr() as usize];
    let (street, road, renderer) = (
        street.as_mut_ptr().cast::<u8>(),
        road.as_mut_ptr().cast::<u8>(),
        renderer.as_mut_ptr().cast::<u8>(),
    );
    for presence in 0u32..32 {
        let mut object = Object::new(0x100);
        let ptr = object.ptr();
        let developer = unsafe { ZoneDeveloper::from_ptr(ptr) };
        let mut a = [0x1122_3344u32, 0xAABB_CCDD, 0x5566_7788, 0x1234_5678, 0xC0FF_EE11];
        let mut b = [0x1122_3344u32, 0x8765_4321, 0x5566_7788, 0x1234_5678, 0xC0FF_EE11];
        developer.set_virtual_lot_header(a.as_mut_ptr().cast());
        developer.set_deleted_lot_header(b.as_mut_ptr().cast());
        developer.set_virtual_lot_count(if presence & 1 != 0 { 99 } else { 0 });
        developer.set_deleted_lot_count(if presence & 2 != 0 { 99 } else { 0 });
        developer.set_street_tool(if presence & 4 != 0 {
            street
        } else {
            core::ptr::null_mut()
        });
        developer.set_road_tool(if presence & 8 != 0 { road } else { core::ptr::null_mut() });
        developer.set_renderer(if presence & 16 != 0 {
            renderer
        } else {
            core::ptr::null_mut()
        });
        let mut expected = vec![];
        if presence & 1 != 0 {
            expected.push((0x0072_ABE0, ptr.wrapping_add(0xD8) as usize, vec![0xAABB_CCDD]));
        }
        if presence & 2 != 0 {
            expected.push((0x0072_ABE0, ptr.wrapping_add(0xE4) as usize, vec![0x8765_4321]));
        }
        if presence & 4 != 0 {
            expected.push((0x14, street as usize, vec![]));
        }
        if presence & 8 != 0 {
            expected.push((0x14, road as usize, vec![]));
        }
        if presence & 16 != 0 {
            expected.push((0x24, renderer as usize, vec![]));
        }
        answers(&vec![0xABCD_1234; expected.len()]);
        let mut before = unsafe { core::slice::from_raw_parts(ptr, 0x100) }.to_vec();
        before[0xDC..0xE0].fill(0);
        before[0xE8..0xEC].fill(0);
        assert_eq!(
            unsafe { fc4p_0072c0e0(ptr) },
            if presence & 16 != 0 { 0xABCD_1234 } else { 0 }
        );
        assert_eq!(calls(), expected);
        assert_eq!(unsafe { core::slice::from_raw_parts(ptr, 0x100) }, before);
        for (index, header, root) in [(0, &a, 0xAABB_CCDD), (1, &b, 0x8765_4321)] {
            assert_eq!(header[0], 0x1122_3344);
            assert_eq!(header[4], 0xC0FF_EE11);
            if presence & (1 << index) != 0 {
                assert_eq!(header[1..4], [0, header.as_ptr() as u32, header.as_ptr() as u32]);
            } else {
                assert_eq!(header[1..4], [root, 0x5566_7788, 0x1234_5678]);
            }
        }
    }
}

#[test]
fn placement_codes_branch_priority_low_bytes_and_bypass_flags() {
    let _region = fake(0x0046_6610, region as *const () as usize);
    let _zone = fake(0x0072_A040, zone as *const () as usize);
    let _network = fake(0x0072_A010, network as *const () as usize);
    let _occupied = fake(0x0072_C050, is_occupied as *const () as usize);
    let mut table = [0usize; 10];
    table[9] = water as *const () as usize;
    let mut object = Object::new(0x100);
    let ptr = object.ptr();
    let developer = unsafe { ZoneDeveloper::from_ptr(ptr) };
    developer.set_primary_vtable(table.as_ptr() as u32);
    developer.set_cells_x(10);
    developer.set_cells_z(12);
    developer.set_restriction(0x1234usize as *mut u8);
    for (x, z) in [(-1, 2), (10, 2), (1, -1), (1, 12), (i32::MIN, 0), (i32::MAX, 0)] {
        assert_eq!(unsafe { fc4p_0072c200(ptr, x, z, 0, 0) }, 0);
        assert!(calls().is_empty());
    }
    // The core placement-code cases, expanded across all bypasses and service results.
    for real in [false, true] {
        for skip_zone in [false, true] {
            for allow_water in [false, true] {
                for failure in 0..6 {
                    developer.set_real_mode(u8::from(real));
                    let mut script = vec![if failure == 0 { 0xABCD_0000 } else { 0xABCD_0002 }];
                    let mut expected = vec![(0x0046_6610, 0x1234usize, vec![2, 3])];
                    let result = if failure == 0 {
                        0
                    } else {
                        if !allow_water {
                            script.push(if failure == 1 { 0xABCD_0002 } else { 0xABCD_0000 });
                            expected.push((0x24, ptr as usize, vec![2, 3]));
                        }
                        if !allow_water && failure == 1 {
                            1
                        } else {
                            if real && !skip_zone {
                                script.push(if failure == 2 { 0xABCD_0000 } else { 0xABCD_0002 });
                                expected.push((0x0072_A040, ptr as usize, vec![2, 3]));
                            }
                            if real && !skip_zone && failure == 2 {
                                2
                            } else {
                                script.push(if failure == 3 { 0xABCD_0002 } else { 0xABCD_0000 });
                                expected.push((0x0072_A010, ptr as usize, vec![2, 3]));
                                if failure == 3 {
                                    3
                                } else {
                                    script.push(if failure == 4 { 0xABCD_0002 } else { 0xABCD_0000 });
                                    expected.push((0x0072_C050, ptr as usize, vec![2, 3]));
                                    if failure == 4 {
                                        7
                                    } else {
                                        8
                                    }
                                }
                            }
                        }
                    };
                    answers(&script);
                    assert_eq!(
                        unsafe {
                            fc4p_0072c200(
                                ptr,
                                2,
                                3,
                                0xFF00_0000 | u32::from(skip_zone),
                                0xFF00_0000 | u32::from(allow_water),
                            )
                        },
                        result
                    );
                    assert_eq!(calls(), expected);
                }
            }
        }
    }
    developer.set_restriction(core::ptr::null_mut());
    developer.set_real_mode(0);
    answers(&[0xFFFF_FF00, 0xFFFF_FF00]);
    assert_eq!(unsafe { fc4p_0072c200(ptr, 2, 3, 0, 1) }, 8);
    assert_eq!(
        calls(),
        [
            (0x0072_A010, ptr as usize, vec![2, 3]),
            (0x0072_C050, ptr as usize, vec![2, 3])
        ]
    );
}

#[test]
fn rectangle_codes_visit_all_cells_keep_argument_words_and_query_config_before_empty_scan() {
    let _code = fake(0x0072_C200, code as *const () as usize);
    let mut table = [0usize; 15];
    table[14] = config as *const () as usize;
    let mut cfg = [table.as_ptr() as usize];
    let cfg = cfg.as_mut_ptr().cast::<u8>();
    let mut object = Object::new(0x100);
    let ptr = object.ptr().wrapping_add(4);
    for (water_word, config_word) in [(0xABCD_0000, 0xFFFF_FF00), (0xABCD_0000, 0xFFFF_FF02), (0xABCD_0003, 0)] {
        let mut expected = vec![];
        let mut script = vec![];
        let water = if water_word as u8 != 0 {
            0xABCD_0001
        } else {
            expected.push((0x38, cfg as usize, vec![]));
            script.push(config_word);
            0xABCD_0000 | (config_word & 255)
        };
        script.extend([8, 0, 7, 3, 2, u32::MAX]);
        for x in 2..4 {
            for z in 3..6 {
                expected.push((
                    0x0072_C200,
                    ptr.wrapping_sub(4) as usize,
                    vec![x, z, 0xAABB_CC02, water],
                ));
            }
        }
        answers(&script);
        assert_eq!(
            unsafe { fc4p_0072c2d0(ptr, 2, 3, 2, 3, 0xAABB_CC02, water_word, cfg) },
            -1
        );
        assert_eq!(calls(), expected);
    }
    for (x, z, width, height) in [
        (2, 3, 0, 1),
        (2, 3, 1, 0),
        (2, 3, -1, 2),
        (i32::MAX, 3, 1, 2),
        (2, i32::MAX, 2, 1),
    ] {
        answers(&[1]);
        assert_eq!(unsafe { fc4p_0072c2d0(ptr, x, z, width, height, 0, 0, cfg) }, 8);
        assert_eq!(calls(), [(0x38, cfg as usize, vec![])]);
    }
    answers(&[8]);
    assert_eq!(
        unsafe { fc4p_0072c2d0(ptr, 1, 2, 1, 1, 0, 0, core::ptr::null_mut()) },
        8
    );
    assert_eq!(calls(), [(0x0072_C200, ptr.wrapping_sub(4) as usize, vec![1, 2, 0, 0])]);
}
