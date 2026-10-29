// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{call_thiscall_with_eax, fake, fake_data, Fake, GameFpu, Object};
use std::cell::RefCell;

type Event = (u32, Vec<u32>);
#[derive(Default)]
struct State {
    events: Vec<Event>,
    occupant: *mut u8,
    interface: *mut u8,
    resource: *mut u8,
    allocation: *mut u8,
    manager: *mut u8,
    replacement_table: *mut u8,
    support: u32,
    support_cell: u32,
    flags: u32,
    probes: Vec<bool>,
    probe: usize,
    query: usize,
    final_query: bool,
    final_resource_null: bool,
    adjacent: bool,
    reversed: u32,
    translation: [u32; 3],
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn event(id: u32, args: &[u32]) {
    STATE.with(|s| s.borrow_mut().events.push((id, args.to_vec())));
}

extern "thiscall" fn support(this: *mut u8, piece: u32) -> u32 {
    event(1, &[this as u32, piece]);
    STATE.with(|s| s.borrow().support)
}
extern "thiscall" fn support_cell_fake(this: *mut u8, cell: *mut u8) -> u32 {
    event(2, &[this as u32, cell as u32]);
    STATE.with(|s| s.borrow().support_cell)
}
extern "thiscall" fn supports(this: *mut u8, occupant: *mut u8, cell: *mut u8, support: u32, info: u32) {
    event(3, &[this as u32, occupant as u32, cell as u32, support, info]);
}
extern "thiscall" fn clear(this: *mut u8, x: u32, z: u32, filter: u32) {
    event(4, &[this as u32, x, z, filter]);
}
extern "thiscall" fn resource(this: *mut u8, tgi: *mut u8, iid: u32, output: *mut *mut u8, a: u32, b: u32) -> u32 {
    unsafe {
        event(5, &[this as u32, read(tgi, 0), read(tgi, 4), read(tgi, 8), iid, a, b]);
    }
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let answer = s.probes.get(s.probe).copied().unwrap_or(false);
        s.probe += 1;
        unsafe {
            output.write(if s.final_resource_null && s.probe > 1 {
                core::ptr::null_mut()
            } else {
                s.resource
            });
        }
        u32::from(answer)
    })
}
extern "thiscall" fn release(this: *mut u8) -> u64 {
    event(8, &[this as u32]);
    0x1234_5678_AABB_CCDD
}
extern "thiscall" fn addref(this: *mut u8) {
    event(9, &[this as u32]);
}
extern "thiscall" fn create(this: *mut u8, cell: *mut u8, output: *mut *mut u8, bbox: *mut u8, iid: u32) {
    event(6, &[this as u32, cell as u32, iid]);
    STATE.with(|s| unsafe {
        output.write(s.borrow().occupant);
    });
    for (i, value) in [1.0f32, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0]
        .iter()
        .enumerate()
    {
        unsafe {
            write(bbox, 4 * i, value.to_bits());
        }
    }
}
extern "thiscall" fn piece(this: *mut u8, piece: u32, orientation: u32) {
    event(7, &[this as u32, piece, orientation]);
}
extern "thiscall" fn set_flags(this: *mut u8, mask: u32) -> u64 {
    event(0x28, &[this as u32, mask]);
    STATE.with(|s| s.borrow_mut().flags |= mask);
    0x1122_3344_5566_7788
}
extern "thiscall" fn clear_flags(this: *mut u8, mask: u32) -> u64 {
    event(0x2C, &[this as u32, mask]);
    STATE.with(|s| s.borrow_mut().flags &= !mask);
    0x8877_6655_4433_2211
}
extern "thiscall" fn get_flags(this: *mut u8) -> u32 {
    event(0x30, &[this as u32]);
    STATE.with(|s| s.borrow().flags)
}
extern "thiscall" fn has_flags(this: *mut u8, mask: u32) -> u32 {
    event(0x20, &[this as u32, mask]);
    STATE.with(|s| u32::from(s.borrow().flags & mask != 0))
}
extern "thiscall" fn configure(this: *mut u8, occupant: *mut u8, connections: *mut u8) {
    event(10, &[this as u32, occupant as u32, connections as u32]);
}
extern "thiscall" fn as_occupant(this: *mut u8) -> *mut u8 {
    event(0xC0, &[this as u32]);
    STATE.with(|s| {
        let s = s.borrow();
        if !s.replacement_table.is_null() {
            unsafe {
                write(s.manager, 0, s.replacement_table);
            }
        }
    });
    this.wrapping_add(0x14)
}
extern "thiscall" fn changed_manager_insert(this: *mut u8, object: *mut u8, data: u32) -> u32 {
    event(23, &[this as u32, object as u32, data]);
    0xFEDC_BA98
}
extern "thiscall" fn manager_insert(this: *mut u8, object: *mut u8, data: u32) -> u32 {
    event(11, &[this as u32, object as u32, data]);
    0x1234_56A7
}
extern "C" fn alloc(size: u32) -> u64 {
    event(12, &[size]);
    STATE.with(|s| registers(s.borrow().allocation as u32, 0xDEAD_BEEF))
}
extern "thiscall" fn query(this: *mut u8, iid: u32, output: *mut *mut u8) -> u32 {
    event(13, &[this as u32, iid]);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.query += 1;
        unsafe {
            output.write(s.interface);
        }
        u32::from(s.query == 1 || s.final_query)
    })
}
extern "thiscall" fn origin(this: *mut u8, output: *mut u8) {
    event(14, &[this as u32]);
    STATE.with(|s| {
        for (i, value) in s.borrow().translation.iter().enumerate() {
            unsafe {
                write(output, i * 4, *value);
            }
        }
    });
}
extern "thiscall" fn recompute(this: *mut u8) {
    event(15, &[this as u32]);
}
extern "thiscall" fn reverse(this: *mut u8, output: *mut u8) {
    unsafe {
        event(16, &[this as u32, read(this, 0)]);
    }
    STATE.with(|s| unsafe {
        write(output, 0, s.borrow().reversed);
    });
}
extern "thiscall" fn adjacent_x_fake(this: *mut u8, x: u32, z: u32, mask: u32) -> u32 {
    event(17, &[this as u32, x, z, mask]);
    STATE.with(|s| u32::from(s.borrow().adjacent))
}
extern "thiscall" fn adjacent_z_fake(this: *mut u8, x: u32, z: u32, mask: u32) -> u32 {
    event(18, &[this as u32, x, z, mask]);
    STATE.with(|s| u32::from(s.borrow().adjacent))
}
extern "thiscall" fn transform(this: *mut u8, output: *mut u8, x: u32, z: u32) -> *mut u8 {
    event(19, &[this as u32, x, z]);
    unsafe {
        write(output, 0, x);
        write(output, 4, 2.0f32);
        write(output, 8, z);
    }
    output
}
extern "thiscall" fn altitude(this: *mut u8, x: u32, z: u32) -> f32 {
    event(20, &[this as u32, x, z]);
    120.0
}
extern "thiscall" fn pose(this: *mut u8, model: u32, angle: u32, x: u32, y: u32, z: u32) {
    event(21, &[this as u32, model, angle, x, y, z]);
}
extern "thiscall" fn model(this: *mut u8, resource: *mut u8, origin: *mut u8, orientation: u32) {
    unsafe {
        event(
            22,
            &[
                this as u32,
                resource as u32,
                read(origin, 0),
                read(origin, 4),
                read(origin, 8),
                orientation,
            ],
        );
    }
}

struct Fixture {
    tool: Object,
    cell: Object,
    occupant: Object,
    interface: Object,
    manager: Object,
    resource: Object,
    allocation: Object,
    type_info: Object,
    scratch: [u32; 53],
    models: Vec<u32>,
    _tables: Vec<Vec<usize>>,
    _data: Vec<Box<[u32]>>,
    _fakes: Vec<Fake>,
}

impl Fixture {
    fn new() -> Self {
        STATE.with(|s| {
            *s.borrow_mut() = State {
                final_query: true,
                translation: [1.0f32.to_bits(), 100.0f32.to_bits(), 2.0f32.to_bits()],
                ..State::default()
            }
        });
        let mut f = Self {
            tool: Object::new(0x380),
            cell: Object::new(0xC0),
            occupant: Object::new(0x30),
            interface: Object::new(0x20),
            manager: Object::new(4),
            resource: Object::new(4),
            allocation: Object::new(12),
            type_info: Object::new(0x10),
            scratch: [0xCDEF_AB99; 53],
            models: vec![0xABC, 0xDEF],
            _tables: vec![],
            _data: vec![],
            _fakes: vec![
                fake(0x0062_5860, support as *const () as usize),
                fake(0x0062_3BB0, support_cell_fake as *const () as usize),
                fake(0x0064_3FB0, supports as *const () as usize),
                fake(0x0062_72F0, clear as *const () as usize),
                fake(0x0090_CF54, alloc as *const () as usize),
                fake(0x0061_D230, recompute as *const () as usize),
                fake(0x0062_1210, reverse as *const () as usize),
                fake(0x0062_4510, adjacent_x_fake as *const () as usize),
                fake(0x0062_4590, adjacent_z_fake as *const () as usize),
                fake(0x0061_D340, pose as *const () as usize),
                fake(0x0061_D120, model as *const () as usize),
            ],
        };
        let tool = f.tool.ptr();
        let cell = f.cell.ptr();
        let occupant = f.occupant.ptr();
        let interface = f.interface.ptr();
        let manager = f.manager.ptr();
        let resource_ptr = f.resource.ptr();
        let mut table = vec![0; 0x144 / 4];
        table[0xD8 / 4] = create as *const () as usize;
        table[0x140 / 4] = configure as *const () as usize;
        f.table(tool, table);
        let mut table = vec![0; 0xE8 / 4];
        for (slot, target) in [
            (0, query as *const () as usize),
            (4, addref as *const () as usize),
            (8, release as *const () as usize),
            (0x78, piece as *const () as usize),
            (0x28, set_flags as *const () as usize),
            (0x2C, clear_flags as *const () as usize),
            (0x30, get_flags as *const () as usize),
            (0xC0, as_occupant as *const () as usize),
            (0xE0, altitude as *const () as usize),
            (0x20, has_flags as *const () as usize),
            (0xE4, transform as *const () as usize),
        ] {
            table[slot / 4] = target;
        }
        f.table(occupant, table.clone());
        f.table(interface, table);
        let mut table = vec![0; 9];
        table[8] = origin as *const () as usize;
        f.table(unsafe { interface.add(4) }, table);
        let mut table = vec![0; 0x54 / 4];
        table[0x50 / 4] = manager_insert as *const () as usize;
        f.table(manager, table);
        f.table(
            resource_ptr,
            vec![0, 0, release as *const () as usize, resource as *const () as usize],
        );
        f.data(0x00B4_3CA8, vec![resource_ptr as u32]);
        f.data(0x00AA_835C, (0..13).map(|i| 1 << i).collect());
        f.data(0x00B4_52D4, vec![0x2222_3333; 14 * 0x45]);
        let mut model_types = vec![0u32; 14 * 0x45];
        for kind in 0..14 {
            model_types[kind * 0x45 + 0xA0 / 4] = f.models.as_ptr() as u32;
            model_types[kind * 0x45 + 0xA4 / 4] = f.models.as_ptr().wrapping_add(2) as u32;
        }
        f.data(0x00B4_5318, model_types);
        for (address, bits) in [
            (0x00B4_52AC, 0),
            (0x00A8_4D2C, 0x3F00_0000),
            (0x00AA_8708, 0x3E49_0FDB),
            (0x00AA_870C, 0x3F26_6666),
            (0x00AA_8710, 0xC080_0000),
            (0x00A8_E17C, 0x4080_0000),
        ] {
            f.data(address, vec![bits]);
        }
        unsafe {
            let tool = NetworkTool::from_ptr(tool);
            tool.set_type_info(f.type_info.ptr());
            tool.set_occupant_manager(manager);
            tool.set_insertion_data(1);
            tool.set_network_type(2);
            InsertionTool::from_ptr(tool.as_ptr()).set_filter(0x1234_5678);
            write(f.type_info.ptr(), 0xC, 0x1111_2222u32);
            write(cell, 0, 7u32);
            write(cell, 4, 9u32);
            write(cell, 8, 1u32);
            write(cell, 0x5C, 110.0f32);
            write(cell, 0x64, 112.0f32);
        }
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.occupant = occupant;
            s.interface = interface;
            s.resource = resource_ptr;
            s.manager = manager;
            s.allocation = f.allocation.ptr();
            s.flags = 1;
        });
        f
    }
    fn table(&mut self, object: *mut u8, table: Vec<usize>) {
        unsafe {
            write(object, 0, table.as_ptr());
        }
        self._tables.push(table);
    }
    fn data(&mut self, address: u32, data: Vec<u32>) {
        let mut data = data.into_boxed_slice();
        self._fakes.push(fake_data(address, data.as_mut_ptr().cast()));
        self._data.push(data);
    }
    fn prepare(&mut self, request: *mut u32, output: *mut u8) {
        self.scratch[0xC8 / 4] = self.cell.ptr() as u32;
        self.scratch[0xCC / 4] = request as u32;
        self.scratch[0xD0 / 4] = output as u32;
    }
    fn run(&mut self, request: &mut [u32; 3], output: *mut u8) -> u64 {
        self.prepare(request.as_mut_ptr(), output);
        unsafe { insert_occupant(self.tool.ptr(), self.scratch.as_mut_ptr().cast()) }
    }
    fn prebuilt(&mut self, kind: usize, edges: u32) {
        unsafe {
            write(self.cell.ptr(), 8, 1u32 << kind);
            write(self.cell.ptr(), 0x18 + 4 * kind, edges);
        }
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.flags = 1 << kind;
            s.probes = vec![true, true];
        });
    }
    fn events(&self) -> Vec<Event> {
        STATE.with(|s| s.borrow().events.clone())
    }
    fn ids(&self) -> Vec<u32> {
        self.events().iter().map(|e| e.0).collect()
    }
}

#[test]
fn ordinary_insert_uses_current_data_clears_the_cell_then_sets_translucency() {
    for usable in [false, true] {
        for data in [0, 1, 0xDEAD_BEEF] {
            let mut f = Fixture::new();
            unsafe {
                NetworkTool::from_ptr(f.tool.ptr()).set_insertion_data(data);
            }
            STATE.with(|s| s.borrow_mut().flags |= if usable { 0x1000_0000 } else { 0 });
            let before = unsafe { core::slice::from_raw_parts(f.cell.ptr(), 0xC0).to_vec() };
            assert_eq!(
                f.run(&mut [0x1234, 0xDEAD_BE82, 7], core::ptr::null_mut()),
                0x1234_5678_AABB_CCDD
            );
            assert_eq!(
                f.ids(),
                [
                    1,
                    5,
                    6,
                    7,
                    10,
                    0x2C,
                    4,
                    0xC0,
                    11,
                    0x30,
                    if usable { 0x2C } else { 0x28 },
                    8
                ]
            );
            let events = f.events();
            assert_eq!(events[2].1[2], 0);
            assert_eq!(events[3].1[1..], [0x1234, 0x82]);
            assert_eq!(events[4].1[2], f.cell.ptr() as u32 + 0x20);
            assert_eq!(events[6].1, [f.tool.ptr() as u32, 7, 9, 0x1234_5678]);
            assert_eq!(
                events[8].1,
                [f.manager.ptr() as u32, f.occupant.ptr() as u32 + 0x14, data]
            );
            assert_eq!(STATE.with(|s| s.borrow().flags & 0x4000 != 0), !usable);
            let mut after = unsafe { core::slice::from_raw_parts(f.cell.ptr(), 0xC0).to_vec() };
            assert_eq!(after[0x54], 0xA7);
            after[0x54] = before[0x54];
            assert_eq!(before, after);
        }
    }
}

#[test]
fn optional_manager_and_retained_list_have_distinct_reference_effects() {
    let mut f = Fixture::new();
    unsafe {
        NetworkTool::from_ptr(f.tool.ptr()).set_occupant_manager(core::ptr::null_mut());
        write(f.cell.ptr(), 0x54, 0xEFu8);
    }
    let mut head = [0u32; 3];
    let p = head.as_mut_ptr() as u32;
    head[0] = p;
    head[1] = p;
    let mut output = [p];
    f.run(&mut [0x1234, 2, 7], output.as_mut_ptr().cast());
    assert_eq!(f.ids(), [1, 5, 6, 7, 10, 0x2C, 4, 0x30, 0x28, 9, 12, 8]);
    unsafe {
        let node = f.allocation.ptr();
        assert_eq!((head[0], head[1]), (node as u32, node as u32));
        assert_eq!(
            (read::<u32>(node, 0), read::<u32>(node, 4), read::<u32>(node, 8)),
            (p, p, f.occupant.ptr() as u32)
        );
        assert_eq!(read::<u8>(f.cell.ptr(), 0x54), 0xEF);
    }
}

#[test]
fn support_calls_precede_connection_configuration_and_city_insertion() {
    for found in [false, true] {
        let mut f = Fixture::new();
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.support = 0x4567;
            s.support_cell = if found { 0x9876 } else { 0 };
        });
        f.run(&mut [55, 0, 7], core::ptr::null_mut());
        let events = f.events();
        assert_eq!(events[4].0, 2);
        if found {
            assert_eq!(
                events[5],
                (
                    3,
                    vec![
                        f.tool.ptr() as u32,
                        f.occupant.ptr() as u32,
                        f.cell.ptr() as u32,
                        0x4567,
                        0x9876
                    ]
                )
            );
            assert_eq!(events[6].1[1], 0x4000_0000);
            assert_eq!(events[7].1[1], 0x0800_0000);
            assert_eq!(events[8].0, 10);
        } else {
            assert_eq!(events[5].0, 10);
        }
    }
}

#[test]
fn city_insert_keeps_the_manager_table_saved_before_the_occupant_interface_call() {
    let mut f = Fixture::new();
    let mut table = vec![0usize; 0x54 / 4];
    table[0x50 / 4] = changed_manager_insert as *const () as usize;
    STATE.with(|s| s.borrow_mut().replacement_table = table.as_mut_ptr().cast());
    f.run(&mut [9, 2, 7], core::ptr::null_mut());
    let events = f.events();
    assert_eq!(events.iter().filter(|e| e.0 == 11).count(), 1);
    assert!(!events.iter().any(|e| e.0 == 23));
    assert_eq!(unsafe { read::<u8>(f.cell.ptr(), 0x54) }, 0xA7);
}

#[test]
fn rail_model_uses_average_corners_and_model_style_fallback() {
    let _fpu = GameFpu::new();
    for style in [0, 1, 2, u32::MAX] {
        let mut f = Fixture::new();
        f.prebuilt(2, 0x0103_0103);
        unsafe {
            InsertionTool::from_ptr(f.tool.ptr()).set_style(style);
        }
        f.run(&mut [77, 0x1234_5683, 7], core::ptr::null_mut());
        assert_eq!(
            f.ids(),
            [
                1, 5, 8, 6, 7, 0x2C, 0x2C, 0x28, 13, 14, 0x30, 0x28, 15, 21, 13, 5, 22, 8, 8, 8, 0x2C, 4, 0xC0, 11,
                0x30, 0x28, 8
            ]
        );
        let events = f.events();
        assert_eq!(events[3].1[2], 0x49C1_A034);
        assert_eq!(
            events[13].1,
            [
                f.interface.ptr() as u32,
                if style == 1 { 0xDEF } else { 0xABC },
                0x3F49_0FD0,
                0,
                11.0f32.to_bits(),
                0
            ]
        );
        assert_eq!(
            events[16].1,
            [
                f.interface.ptr() as u32,
                f.resource.ptr() as u32,
                0,
                0,
                0,
                (f.resource.ptr() as u32 & !255) | 0x83
            ]
        );
    }
}

#[test]
fn nonrail_pose_transforms_the_selected_box_diagonal_and_applies_the_larger_slope() {
    let _fpu = GameFpu::new();
    let mut f = Fixture::new();
    f.prebuilt(0, 0x0100);
    f.run(&mut [77, 2, 7], core::ptr::null_mut());
    let events = f.events();
    let transforms: Vec<_> = events.iter().filter(|e| e.0 == 19).map(|e| e.1.clone()).collect();
    assert_eq!(
        transforms,
        [
            [f.interface.ptr() as u32, 1.0f32.to_bits(), 3.0f32.to_bits()],
            [f.interface.ptr() as u32, 7.0f32.to_bits(), 9.0f32.to_bits()]
        ]
    );
    let pose = &events.iter().find(|e| e.0 == 21).unwrap().1;
    assert_eq!(
        pose[1..],
        [
            0xABC,
            0x3F49_0FD0,
            1.4000001f32.to_bits(),
            14.8f32.to_bits(),
            (-6.6f32).to_bits()
        ]
    );
    let model = &events.iter().find(|e| e.0 == 22).unwrap().1;
    assert_eq!(model[2..], [0, 0, 0, 0x4100_0002]);
    assert_eq!(
        events.iter().find(|e| e.0 == 20).unwrap().1[1..],
        [2.4f32.to_bits(), (-4.6f32).to_bits()]
    );
}

#[test]
fn multi_type_models_skip_pose_but_keep_both_interface_releases() {
    let mut f = Fixture::new();
    f.prebuilt(0, 0x103);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.flags = 3;
        s.final_query = false;
    });
    f.run(&mut [99, 0x81, 7], core::ptr::null_mut());
    assert_eq!(
        f.ids(),
        [1, 5, 8, 6, 7, 0x2C, 0x2C, 0x28, 13, 14, 0x30, 13, 8, 8, 0x2C, 4, 0xC0, 11, 0x30, 0x28, 8]
    );
}

#[test]
fn failed_or_null_final_resources_skip_model_addition_and_release_both_interfaces() {
    for (success, null) in [(false, false), (true, true)] {
        let mut f = Fixture::new();
        f.prebuilt(0, 0x103);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.probes = vec![true, success];
            s.final_resource_null = null;
        });
        f.run(&mut [99, 0x81, 7], core::ptr::null_mut());
        let events = f.events();
        assert!(!events.iter().any(|e| e.0 == 22));
        assert_eq!(
            events
                .iter()
                .filter(|e| e.0 == 8 && e.1[0] == f.interface.ptr() as u32)
                .count(),
            2
        );
        assert_eq!(
            events
                .iter()
                .filter(|e| e.0 == 8 && e.1[0] == f.resource.ptr() as u32)
                .count(),
            1
        );
    }
}

#[test]
fn rail_height_keeps_the_extended_exponent_and_preserves_an_existing_x87_value() {
    let _fpu = GameFpu::new();
    let mut f = Fixture::new();
    f.prebuilt(2, 0x0103_0103);
    unsafe {
        write(f.cell.ptr(), 0x5C, f32::MAX);
        write(f.cell.ptr(), 0x64, f32::MAX);
    }
    let saved = 123.5f32;
    unsafe {
        core::arch::asm!("fld dword ptr [{value}]", value = in(reg) &saved);
    }
    f.run(&mut [77, 2, 7], core::ptr::null_mut());
    let mut retained = 0f32;
    unsafe {
        core::arch::asm!("fstp dword ptr [{value}]", value = in(reg) &mut retained);
    }
    assert_eq!(retained.to_bits(), saved.to_bits());
    assert_eq!(crate::x87::control_word(), crate::x87::GAME_CONTROL_WORD);
    assert_eq!(f.events().iter().find(|e| e.0 == 21).unwrap().1[4], f32::MAX.to_bits());
}

#[test]
fn packed_orientation_covers_bends_extensions_rotation_and_mirroring() {
    let _fpu = GameFpu::new();
    for (edges, mode, reversed, expected_angle) in [
        (0x0000_0A02, 0, 0, 0x4023_5CE2u32),
        (0x0000_020A, 0x80, 0x0000_0A02, 0x4023_5CE2),
        (0x0000_0A02, 1, 0, 0x4083_F268),
    ] {
        let mut f = Fixture::new();
        f.prebuilt(0, edges);
        STATE.with(|s| s.borrow_mut().reversed = reversed);
        f.run(&mut [9, mode, 7], core::ptr::null_mut());
        let events = f.events();
        let pose = &events.iter().find(|e| e.0 == 21).unwrap().1;
        assert_eq!(pose[2], expected_angle, "edges={edges:08X} mode={mode:02X}");
        assert_eq!(pose[3], (-4.0f32).to_bits());
        assert_eq!(pose[5], (-4.0f32).to_bits());
        assert_eq!(
            events.iter().filter(|e| e.0 == 16).count(),
            usize::from(mode & 0x80 != 0)
        );
    }
}

#[test]
fn straight_and_rail_adjacent_probes_respect_coordinate_parity() {
    let _fpu = GameFpu::new();
    for (kind, edges, axis, parity) in [
        (0, 0x0200_0200, 18, 4),
        (0, 0x0002_0002, 17, 0),
        (2, 0x0204_0200, 18, 4),
        (2, 0x0402_0002, 17, 0),
    ] {
        for odd in [false, true] {
            for adjacent in [false, true] {
                let mut f = Fixture::new();
                f.prebuilt(kind, edges);
                unsafe {
                    write(f.cell.ptr(), parity, if odd { 7u32 } else { 6u32 });
                }
                STATE.with(|s| s.borrow_mut().adjacent = adjacent);
                f.run(&mut [9, 0, 7], core::ptr::null_mut());
                let events = f.events();
                assert_eq!(events.iter().filter(|e| e.0 == axis).count(), usize::from(!odd));
                assert_eq!(
                    events.iter().filter(|e| e.0 == 21).count(),
                    usize::from(odd || adjacent)
                );
            }
        }
    }
}

#[test]
fn resource_probe_order_and_zero_group_fallback_are_preserved() {
    let mut f = Fixture::new();
    f.prebuilt(0, 0x103);
    unsafe {
        write(f.cell.ptr(), 8, 5u32);
    }
    f.data(0x00B4_52D4, vec![0; 14 * 0x45]);
    STATE.with(|s| s.borrow_mut().probes = vec![false, true, true]);
    f.run(&mut [91, 0, 7], core::ptr::null_mut());
    let resources: Vec<_> = f.events().into_iter().filter(|e| e.0 == 5).collect();
    assert_eq!(resources.len(), 3);
    assert_eq!(
        resources.iter().map(|e| e.1[2]).collect::<Vec<_>>(),
        [0, 0, 0x1111_2222]
    );
    assert!(resources
        .iter()
        .all(|e| e.1[1] == 0x6534_284A && e.1[3] == 91 && e.1[4] == 0xA521_60F5));
}

#[test]
fn exported_entry_returns_partial_release_registers_and_pops_three_words() {
    #[unsafe(naked)]
    unsafe extern "thiscall" fn partial_release(_this: *mut u8) {
        core::arch::naked_asm!("mov al, 0x5A", "ret");
    }
    let mut f = Fixture::new();
    let vtable = unsafe { read::<*mut usize>(f.occupant.ptr(), 0) };
    unsafe {
        vtable.add(2).write(partial_release as *const () as usize);
    }
    let request = [1u32, 2, 7];
    let result = unsafe {
        call_thiscall_with_eax(
            fc4p_006278a0 as *const () as usize,
            f.tool.ptr(),
            &[f.cell.ptr() as u32, request.as_ptr() as u32, 0],
            0xFEED_1234,
        )
    };
    assert_eq!(result, ((vtable as u32 & !255) | 0x5A, 0x1122_3344));
}
