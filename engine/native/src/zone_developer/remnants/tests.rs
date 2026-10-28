// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, fake_data, Fake, Object};
use std::cell::RefCell;
game_fn! {fn invoke(this:*mut u8,x:i32,z:i32,w:i32,h:i32,facing:u32)->u32 = "thiscall" 0x0072_D440;}

#[derive(Debug, PartialEq, Clone)]
enum Call {
    Allocate(u32),
    Append(usize, i32),
    Includes(i32, i32),
    Water(i32, i32),
    Zone(i32, i32),
    Network(i32, i32, u32),
    Occupied(i32, i32),
    Create([i32; 4], Vec<i32>, usize),
    Release,
    Free(u32),
    Destroy,
}
#[derive(Default)]
struct State {
    calls: Vec<Call>,
    #[allow(clippy::vec_box)] // Circular links retain stable node addresses.
    allocations: Vec<Box<[u32; 3]>>,
    restriction: bool,
    water: Vec<(i32, i32)>,
    wrong_zone: bool,
    networks: Vec<(i32, i32)>,
    occupied: Vec<(i32, i32)>,
    lot: usize,
    create_results: Vec<u32>,
    created: usize,
}
thread_local! {static STATE:RefCell<State>=RefCell::new(State{restriction:true,..State::default()});}

fn alloc(s: &mut State, payload: u32) -> *mut u8 {
    let mut node = Box::new([0xA5A5_A5A5, 0xA5A5_A5A5, payload]);
    let p = node.as_mut_ptr().cast();
    s.allocations.push(node);
    p
}
unsafe fn link(header: *mut u8, node: *mut u8) {
    unsafe {
        let previous = read_unaligned(header.add(4).cast::<*mut u8>());
        write_unaligned(node.cast::<*mut u8>(), header);
        write_unaligned(node.add(4).cast::<*mut u8>(), previous);
        write_unaligned(previous.cast::<*mut u8>(), node);
        write_unaligned(header.add(4).cast::<*mut u8>(), node);
    }
}
extern "C" fn allocate_fake(size: u32) -> *mut u8 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Allocate(size));
        alloc(&mut s, 0xA5A5_A5A5)
    })
}
extern "C" fn free_fake(node: *mut u8) {
    let payload = unsafe { read_unaligned(node.add(8).cast::<u32>()) };
    if payload == 0xA5A5_A5A5 {
        assert_eq!(unsafe { read_unaligned(node.cast::<*mut u8>()) }, node);
        assert_eq!(unsafe { read_unaligned(node.add(4).cast::<*mut u8>()) }, node);
    }
    STATE.with(|s| s.borrow_mut().calls.push(Call::Free(payload)));
}
extern "thiscall" fn append_fake(list: *mut *mut u8, value: *const i32) {
    let facing = unsafe { read_unaligned(value) };
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Append(value as usize, facing));
        let node = alloc(&mut s, facing as u32);
        unsafe { link(read_unaligned(list), node) };
    });
}
extern "thiscall" fn destroy_fake(list: *mut *mut u8) -> u64 {
    let header = unsafe { read_unaligned(list) };
    assert!(!header.is_null());
    assert_eq!(unsafe { read_unaligned(header.add(8).cast::<u32>()) }, 0xA5A5_A5A5);
    STATE.with(|s| s.borrow_mut().calls.push(Call::Destroy));
    0xDEAD_BEEF_1234_5678
}
extern "thiscall" fn includes_fake(_this: *mut u8, x: i32, z: i32) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Includes(x, z));
        0xAAAA_0000 | u32::from(s.restriction)
    })
}
extern "thiscall" fn water_fake(_this: *mut u8, x: i32, z: i32) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Water(x, z));
        0xAAAA_0000 | u32::from(s.water.contains(&(x, z)))
    })
}
extern "thiscall" fn zone_fake(_this: *mut u8, x: i32, z: i32) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Zone(x, z));
        0xAAAA_0000 | u32::from(!s.wrong_zone)
    })
}
extern "thiscall" fn network_fake(_this: *mut u8, x: i32, z: i32, mask: u32) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Network(x, z, mask));
        0xAAAA_0000 | u32::from(s.networks.contains(&(x, z)))
    })
}
extern "thiscall" fn occupied_fake(_this: *mut u8, x: i32, z: i32) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Occupied(x, z));
        0xAAAA_0000 | u32::from(s.occupied.contains(&(x, z)))
    })
}
extern "thiscall" fn release_fake(_this: *mut u8) {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Release));
}
extern "thiscall" fn face_fake(
    _this: *mut u8,
    x: i32,
    z: i32,
    w: i32,
    h: i32,
    list: *mut *mut u8,
    out: *mut *mut u8,
) -> u32 {
    let header = unsafe { read_unaligned(list) };
    let mut node = unsafe { read_unaligned(header.cast::<*mut u8>()) };
    let mut values = Vec::new();
    while node != header {
        values.push(unsafe { read_unaligned(node.add(8).cast::<i32>()) });
        node = unsafe { read_unaligned(node.cast::<*mut u8>()) };
    }
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Create(
            [x, z, w, h],
            values.clone(),
            unsafe { read_unaligned(out) } as usize,
        ));
        if values.is_empty() {
            for facing in [0, 1] {
                let node = alloc(&mut s, facing);
                unsafe { link(header, node) };
            }
        }
        unsafe { write_unaligned(out, s.lot as *mut u8) };
        let result = s.create_results.get(s.created).copied().unwrap_or(0xAABB_CC80);
        s.created += 1;
        result
    })
}

struct World {
    dev: Object,
    _table: Box<[usize; 10]>,
    _lot_table: Box<[usize; 3]>,
    _lot: Box<[usize; 1]>,
    constants: Box<[i32; 4]>,
    _fakes: Vec<Fake>,
}
impl World {
    fn new(zone: u32) -> Self {
        STATE.with(|s| {
            *s.borrow_mut() = State {
                restriction: true,
                ..State::default()
            }
        });
        let mut table = Box::new([0usize; 10]);
        table[8] = network_fake as *const () as usize;
        table[9] = water_fake as *const () as usize;
        let mut lot_table = Box::new([0usize; 3]);
        lot_table[2] = release_fake as *const () as usize;
        let lot = Box::new([lot_table.as_ptr() as usize]);
        let mut constants = Box::new([0, 1, 2, 3]);
        let mut dev = Object::new(0xEC);
        let d = unsafe { ZoneDeveloper::from_ptr(dev.ptr()) };
        d.set_primary_vtable(table.as_ptr() as u32);
        d.set_cells_x(64);
        d.set_cells_z(64);
        d.set_real_mode(1);
        d.set_current_zone(zone);
        STATE.with(|s| s.borrow_mut().lot = lot.as_ptr() as usize);
        let mut fakes = vec![
            fake(0x0090_CF54, allocate_fake as *const () as usize),
            fake(0x0090_CF63, free_fake as *const () as usize),
            fake(0x0042_7640, append_fake as *const () as usize),
            fake(0x0056_50A0, destroy_fake as *const () as usize),
            fake(0x0072_D200, face_fake as *const () as usize),
            fake(0x0046_6610, includes_fake as *const () as usize),
            fake(0x0072_A040, zone_fake as *const () as usize),
            fake(0x0072_C050, occupied_fake as *const () as usize),
        ];
        for i in 0..4 {
            fakes.push(fake_data(
                0x00AB_36C4 + i * 4,
                unsafe { constants.as_mut_ptr().add(i as usize) }.cast(),
            ));
        }
        Self {
            dev,
            _table: table,
            _lot_table: lot_table,
            _lot: lot,
            constants,
            _fakes: fakes,
        }
    }
    fn run(&mut self, rect: [i32; 4], facing: u32) -> (u32, Vec<Call>) {
        let before = unsafe { core::slice::from_raw_parts(self.dev.ptr(), 0xEC) }.to_vec();
        let result = unsafe { invoke(self.dev.ptr(), rect[0], rect[1], rect[2], rect[3], facing) };
        assert_eq!(unsafe { core::slice::from_raw_parts(self.dev.ptr(), 0xEC) }, before);
        (result, STATE.with(|s| core::mem::take(&mut s.borrow_mut().calls)))
    }
}

fn creations(c: &[Call]) -> Vec<[i32; 4]> {
    c.iter()
        .filter_map(|c| match c {
            Call::Create(r, ..) => Some(*r),
            _ => None,
        })
        .collect()
}

#[test]
fn industry_and_agriculture_place_every_cell_with_fresh_lists_release_and_free_nodes_in_order() {
    for zone in [7, 8, 9] {
        let mut w = World::new(zone);
        let (r, c) = w.run([10, 20, 2, 2], u32::MAX);
        assert_eq!(r, 1);
        let mut expected = Vec::new();
        for z in 20..22 {
            for x in 10..12 {
                expected.extend([
                    Call::Allocate(12),
                    Call::Create([x, z, 1, 1], vec![], 0),
                    Call::Release,
                    Call::Free(0),
                    Call::Free(1),
                    Call::Free(0xA5A5_A5A5),
                ]);
            }
        }
        assert_eq!(c, expected);
    }
}

#[test]
fn strips_follow_facing_order_and_flush_before_blocked_cells_and_at_each_line_end() {
    for (facing, expected) in [
        (3, vec![[10, 20, 2, 1], [13, 20, 1, 1], [10, 21, 1, 1], [12, 21, 2, 1]]),
        (1, vec![[13, 20, 1, 1], [10, 20, 2, 1], [12, 21, 2, 1], [10, 21, 1, 1]]),
        (0, vec![[10, 20, 1, 2], [11, 20, 1, 1], [12, 21, 1, 1], [13, 20, 1, 2]]),
        (2, vec![[10, 20, 1, 2], [11, 20, 1, 1], [12, 21, 1, 1], [13, 20, 1, 2]]),
    ] {
        let mut w = World::new(1);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.water.push((12, 20));
            s.occupied.push((11, 21));
        });
        let (result, c) = w.run([10, 20, 4, 2], facing);
        assert_eq!(result, 0x1234_5680);
        assert_eq!(creations(&c), expected);
        assert_eq!(c[0], Call::Allocate(12));
        assert_eq!(
            c[1],
            Call::Append(
                unsafe { w.constants.as_ptr().add(facing as usize) } as usize,
                facing as i32
            )
        );
        assert_eq!(c.last(), Some(&Call::Destroy));
        for (i, call) in c.iter().enumerate() {
            if let Call::Create(_, values, previous) = call {
                assert_eq!(values, &[facing as i32]);
                assert_eq!(*previous, 0);
                assert_eq!(c[i + 1], Call::Release);
            }
        }
    }
}

#[test]
fn buildability_short_circuits_in_bounds_restriction_water_zone_network_occupancy_order() {
    for reason in 0..6 {
        let mut w = World::new(1);
        let d = unsafe { ZoneDeveloper::from_ptr(w.dev.ptr()) };
        if reason == 0 {
            d.set_cells_x(10);
        } else {
            d.set_restriction(core::ptr::dangling_mut());
        }
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            match reason {
                1 => s.restriction = false,
                2 => s.water.push((10, 20)),
                3 => s.wrong_zone = true,
                4 => s.networks.push((10, 20)),
                5 => s.occupied.push((10, 20)),
                _ => (),
            }
        });
        let (result, c) = w.run([10, 20, 1, 1], 0);
        assert_eq!(result, 0x1234_5600);
        assert!(creations(&c).is_empty());
        let mut checks = vec![];
        if reason >= 1 {
            checks.push(Call::Includes(10, 20));
        }
        if reason >= 2 {
            checks.push(Call::Water(10, 20));
        }
        if reason >= 3 {
            checks.push(Call::Zone(10, 20));
        }
        if reason >= 4 {
            checks.push(Call::Network(10, 20, 0x1F4F));
        }
        if reason >= 5 {
            checks.push(Call::Occupied(10, 20));
        }
        assert_eq!(&c[2..c.len() - 1], checks);
    }
}

#[test]
fn preview_mode_skips_zone_matching_and_null_lot_outputs_are_not_released() {
    let mut w = World::new(1);
    unsafe { ZoneDeveloper::from_ptr(w.dev.ptr()) }.set_real_mode(0);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.wrong_zone = true;
        s.lot = 0;
    });
    let (result, c) = w.run([10, 20, 1, 1], 2);
    assert_eq!(result, 0x1234_5680);
    assert_eq!(
        &c[2..c.len() - 1],
        &[
            Call::Water(10, 20),
            Call::Network(10, 20, 0x1F4F),
            Call::Occupied(10, 20),
            Call::Create([10, 20, 1, 1], vec![2], 0)
        ]
    );
}

#[test]
fn last_strip_result_is_returned_without_normalizing_or_accumulating_earlier_success() {
    let mut w = World::new(1);
    STATE.with(|s| s.borrow_mut().create_results = vec![0xFFFF_FF80, 0xAAAA_AA00]);
    let (result, c) = w.run([10, 20, 1, 2], 3);
    assert_eq!(result, 0x1234_5600);
    assert_eq!(creations(&c).len(), 2);
}

#[test]
fn invalid_facings_skip_allocation_and_nonpositive_dimensions_keep_list_lifetime() {
    for f in [4, 0x1234_56FF, u32::MAX] {
        let mut w = World::new(1);
        let (result, c) = w.run([10, 20, 1, 1], f);
        assert_eq!(result, f & !255);
        assert!(c.is_empty());
    }
    for (zone, facing, rect) in [
        (7, 99, [10, 20, 0, 1]),
        (9, 99, [10, 20, 1, -1]),
        (1, 0, [10, 20, 0, 1]),
        (1, 3, [10, 20, 1, 0]),
    ] {
        let mut w = World::new(zone);
        let (result, c) = w.run(rect, facing);
        if zone >= 7 {
            assert_eq!(result, 1);
            assert!(c.is_empty());
        } else {
            assert_eq!(result, 0x1234_5600);
            assert_eq!(c.len(), 3);
            assert_eq!(c.last(), Some(&Call::Destroy));
        }
    }
}

#[unsafe(naked)]
unsafe extern "C" fn destructor_registers() {
    core::arch::naked_asm!("mov eax, 0x12345678", "mov ecx, 0xaabbccdd", "ret");
}

#[test]
fn remnant_ecx_matches_invalid_facing_per_cell_and_destructor_exits() {
    for (zone, rect, facing, ecx) in [
        (1, [10, 20, 3, 2], 99, 3),
        (7, [10, 20, 0, 2], 99, 22),
        (8, [10, 20, 3, 0], 99, 3),
        (1, [10, 20, 0, 2], 0, 0xAABB_CCDD),
    ] {
        let mut w = World::new(zone);
        let _destroy = fake(0x0056_50A0, destructor_registers as *const () as usize);
        let args = [rect[0] as u32, rect[1] as u32, rect[2] as u32, rect[3] as u32, facing];
        let mut call = RawCall::new(
            Conv::Thiscall,
            fc4p_0072d440 as *const () as usize,
            w.dev.ptr() as u32,
            &args,
        );
        unsafe { super::super::registers::call(&mut call) };
        assert_eq!(call.ecx, ecx);
    }
}
