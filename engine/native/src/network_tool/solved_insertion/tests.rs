// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{call_thiscall_with_eax, fake, fake_data, Fake, GameFpu, Object};
use std::cell::RefCell;

type Event = (u32, Vec<u32>);

#[derive(Default)]
struct State {
    cells: Vec<(u32, *mut u8)>,
    events: Vec<Event>,
    manager: *mut u8,
    resource: *mut u8,
    resource_success: bool,
    existing_model: u32,
    old_direction: u32,
    same: bool,
    separate: bool,
    replace_manager_slots: bool,
}

thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }

fn event(id: u32, args: &[u32]) {
    STATE.with(|s| s.borrow_mut().events.push((id, args.to_vec())));
}

extern "thiscall" fn lookup(this: *mut u8, key: u32) -> *mut u8 {
    event(1, &[this as u32, key]);
    STATE.with(|s| {
        s.borrow()
            .cells
            .iter()
            .find(|c| c.0 == key)
            .map_or(core::ptr::null_mut(), |c| c.1)
    })
}
extern "C" fn manager() -> *mut u8 {
    event(2, &[]);
    STATE.with(|s| s.borrow().manager)
}
extern "thiscall" fn height(this: *mut u8, vertex: u32) -> f32 {
    event(3, &[this as u32, vertex]);
    // Signed zero and a payload exercise the ST0-to-f32 store without Rust arithmetic.
    match vertex % 4 {
        0 => -0.0,
        1 => 1.25,
        2 => f32::from_bits(0x7FC1_2345),
        _ => f32::INFINITY,
    }
}
extern "thiscall" fn preview(this: *mut u8, cell: *mut u8, request: *mut u8) {
    unsafe {
        assert_eq!(read::<u32>(cell, 0x58), 0x8000_0000);
    }
    event(4, &[this as u32, cell as u32, request as u32]);
}
extern "thiscall" fn insert(this: *mut u8, cell: *mut u8, request: *mut u8, output: u32) {
    unsafe {
        assert_eq!(read::<u8>(cell, 0x51), 1);
    }
    event(5, &[this as u32, cell as u32, request as u32, output]);
}
extern "thiscall" fn flags(this: *mut u8, mask: u32) -> u32 {
    event(0x20, &[this as u32, mask]);
    0xAABB_CC00 | u32::from(unsafe { read::<u32>(this, 4) } & mask != 0)
}
extern "thiscall" fn direction(this: *mut u8) -> u32 {
    event(0x114, &[this as u32]);
    STATE.with(|s| s.borrow().old_direction)
}
extern "thiscall" fn set_direction(this: *mut u8, direction: u32) {
    event(0x118, &[this as u32, direction]);
}
extern "thiscall" fn usable(this: *mut u8, usable: u32) {
    event(0x50, &[this as u32, usable]);
}
extern "thiscall" fn set_flags(this: *mut u8, mask: u32) {
    event(0x28, &[this as u32, mask]);
}
extern "thiscall" fn clear_flags(this: *mut u8, mask: u32) {
    event(0x2C, &[this as u32, mask]);
}
extern "thiscall" fn connections(this: *mut u8, kind: u32, connections: u32) {
    event(0x94, &[this as u32, kind, connections]);
}
extern "thiscall" fn release(this: *mut u8) {
    event(8, &[this as u32]);
}
extern "thiscall" fn resource(this: *mut u8, tgi: *mut u8, iid: u32, output: *mut *mut u8, a: u32, b: u32) -> u32 {
    unsafe {
        event(0xC, &[this as u32, read(tgi, 0), read(tgi, 4), read(tgi, 8), iid, a, b]);
    }
    STATE.with(|s| {
        let s = s.borrow();
        unsafe {
            output.write(s.resource);
        }
        u32::from(s.resource_success)
    })
}
extern "thiscall" fn model(this: *mut u8) -> u32 {
    event(0x3C, &[this as u32]);
    STATE.with(|s| s.borrow().existing_model)
}
extern "thiscall" fn model_direction(this: *mut u8) -> u32 {
    event(0x68, &[this as u32]);
    0x1234_5678
}
extern "thiscall" fn set_piece(this: *mut u8, piece: u32, direction: u32) {
    event(0x78, &[this as u32, piece, direction]);
}
extern "thiscall" fn get_tgi(this: *mut u8, tgi: *mut u8, index: *mut u32) -> u32 {
    event(0x34, &[this as u32]);
    unsafe {
        write(tgi, 0, 9u32);
        write(tgi, 4, 8u32);
        write(tgi, 8, 7u32);
    }
    STATE.with(|s| unsafe {
        let s = s.borrow();
        index.write(if s.same { read(s.cells[0].1, 0x18) } else { 0 });
    });
    0x1234_5601
}
extern "thiscall" fn set_model(this: *mut u8, piece: u32) {
    event(0xCC, &[this as u32, piece]);
}
extern "thiscall" fn as_occupant(this: *mut u8) -> *mut u8 {
    event(0xC0, &[this as u32]);
    STATE.with(|s| {
        let s = s.borrow();
        if s.replace_manager_slots {
            unsafe {
                let table: *mut u8 = read(s.manager, 0);
                write(table, 0x54, remove_changed as *const () as usize);
                write(table, 0x50, reinsert_changed as *const () as usize);
            }
        }
    });
    this.wrapping_add(0x10)
}
extern "thiscall" fn remove_changed(this: *mut u8, object: *mut u8, a: u32, b: u32) {
    event(0x154, &[this as u32, object as u32, a, b]);
}
extern "thiscall" fn reinsert_changed(this: *mut u8, object: *mut u8, data: u32) {
    event(0x155, &[this as u32, object as u32, data]);
}
extern "thiscall" fn remove(this: *mut u8, object: *mut u8, a: u32, b: u32) {
    event(0x54, &[this as u32, object as u32, a, b]);
}
extern "thiscall" fn reinsert(this: *mut u8, object: *mut u8, data: u32) {
    event(0x150, &[this as u32, object as u32, data]);
}
extern "thiscall" fn separate_fake(
    this: *mut u8,
    cell: *mut u8,
    kind: u32,
    piece: *mut u32,
    rotation: *mut u8,
    flip: *mut u8,
) -> u32 {
    unsafe {
        event(
            6,
            &[
                this as u32,
                read(cell, 0),
                read(cell, 4),
                kind,
                u32::from(rotation.read()),
                read(flip, 0),
            ],
        );
        piece.write(23);
        rotation.write(2);
        flip.write(1);
    }
    STATE.with(|s| u32::from(s.borrow().separate))
}
extern "thiscall" fn texture(this: *mut u8, piece: u32, rotation: *mut u8) -> u32 {
    event(7, &[this as u32, piece, unsafe { u32::from(rotation.read()) }]);
    unsafe {
        rotation.write(3);
    }
    0x1357_2468
}
extern "thiscall" fn untextured_fake(
    this: *mut u8,
    cell: *mut u8,
    color: u32,
    heights: *mut u8,
    layer: u32,
    flags: u32,
) {
    unsafe {
        event(
            9,
            &[
                this as u32,
                read(cell, 0),
                read(cell, 4),
                color,
                heights as u32,
                layer,
                flags,
            ],
        );
    }
}
extern "thiscall" fn textured_fake(
    this: *mut u8,
    cell: *mut u8,
    color: u32,
    heights: *mut u8,
    tgi: *mut u8,
    rotation: u32,
    flip: u32,
    layer: u32,
    flags: u32,
) {
    unsafe {
        event(
            10,
            &[
                this as u32,
                read(cell, 0),
                read(cell, 4),
                color,
                heights as u32,
                read(tgi, 0),
                read(tgi, 4),
                read(tgi, 8),
                rotation,
                flip,
                layer,
                flags,
            ],
        );
    }
}

struct Fixture {
    tool: Object,
    cell: Object,
    occupant: Object,
    manager: Object,
    resource: Object,
    scratch: [u32; 22],
    _tables: Vec<Vec<usize>>,
    _data: Vec<Box<[u32]>>,
    _fakes: Vec<Fake>,
}

impl Fixture {
    fn new() -> Self {
        STATE.with(|s| *s.borrow_mut() = State::default());
        let mut f = Self {
            tool: Object::new(0x380),
            cell: Object::new(0xC0),
            occupant: Object::new(0x20),
            manager: Object::new(4),
            resource: Object::new(4),
            scratch: [0xCDEF_AB99; 22],
            _tables: Vec::new(),
            _data: Vec::new(),
            _fakes: vec![
                fake(0x0063_3220, lookup as *const () as usize),
                fake(0x0060_2680, manager as *const () as usize),
                fake(0x0063_3FA0, separate_fake as *const () as usize),
                fake(0x0062_0820, untextured_fake as *const () as usize),
                fake(0x0062_0BC0, textured_fake as *const () as usize),
            ],
        };
        let mut table = vec![0; 0x150 / 4 + 1];
        table[0xE8 / 4] = preview as *const () as usize;
        table[0x120 / 4] = insert as *const () as usize;
        table[0x124 / 4] = height as *const () as usize;
        let tool = f.tool.ptr();
        f.table(tool, table);
        let mut table = vec![0; 0x120 / 4];
        for (slot, target) in [
            (0x20, flags as *const () as usize),
            (0x114, direction as *const () as usize),
            (0x118, set_direction as *const () as usize),
            (0x50, usable as *const () as usize),
            (0x28, set_flags as *const () as usize),
            (0x2C, clear_flags as *const () as usize),
            (0x94, connections as *const () as usize),
            (0x3C, model as *const () as usize),
            (0x68, model_direction as *const () as usize),
            (0x78, set_piece as *const () as usize),
            (0x34, get_tgi as *const () as usize),
            (0xCC, set_model as *const () as usize),
            (0xC0, as_occupant as *const () as usize),
        ] {
            table[slot / 4] = target;
        }
        let occupant = f.occupant.ptr();
        f.table(occupant, table);
        let mut table = vec![0; 0x58 / 4];
        table[0x54 / 4] = remove as *const () as usize;
        table[0x50 / 4] = reinsert as *const () as usize;
        table[0x34 / 4] = texture as *const () as usize;
        let manager_ptr = f.manager.ptr();
        f.table(manager_ptr, table);
        let mut table = vec![0; 4];
        table[2] = release as *const () as usize;
        table[3] = resource as *const () as usize;
        let resource_ptr = f.resource.ptr();
        f.table(resource_ptr, table);
        let bits: Vec<u32> = (0..13).map(|i| 1 << i).collect();
        f.data(0x00AA_835C, bits);
        f.data(0x00AA_83F0, (100..113).collect());
        f.data(0x00B4_52D4, vec![0x8765_4321; 13 * 0x45]);
        f.data(0x00B4_3CA8, vec![resource_ptr as u32]);
        f.data(0x00B4_3D0C, vec![manager_ptr as u32]);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.cells = vec![(7, f.cell.ptr())];
            s.manager = f.manager.ptr();
            s.resource = f.resource.ptr();
        });
        unsafe {
            write(f.cell.ptr(), 0xA8, 0u32);
            write(f.cell.ptr(), 0xAC, 1u32);
            write(f.cell.ptr(), 0xB0, 2u32);
            write(f.cell.ptr(), 0xB4, 3u32);
        }
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
    fn run(&mut self, requests: &mut [[u32; 3]], preview: u32) -> u32 {
        let p = requests.as_mut_ptr() as u32;
        let vector = [p, p.wrapping_add((requests.len() * 12) as u32), 0];
        let mut args = [vector.as_ptr() as u32, 0x1234_5678, preview];
        unsafe { insert_solved(self.tool.ptr(), self.scratch.as_mut_ptr().cast(), args.as_mut_ptr()) }
    }
    fn events(&self) -> Vec<Event> {
        STATE.with(|s| s.borrow().events.clone())
    }
    fn ids(&self) -> Vec<u32> {
        self.events().iter().map(|e| e.0).collect()
    }
}

#[test]
fn ordinary_cells_keep_request_order_skip_missing_and_pass_the_output_set() {
    let mut f = Fixture::new();
    unsafe {
        NetworkTool::from_ptr(f.tool.ptr()).set_real(1);
    }
    let before = unsafe { core::slice::from_raw_parts(f.cell.ptr(), 0xC0).to_vec() };
    let mut requests = [[12, 2, 7], [99, 0, 8]];
    let end = requests.as_mut_ptr().wrapping_add(2) as u32;
    assert_eq!(f.run(&mut requests, 0), end);
    assert_eq!(f.ids(), [1, 5, 1]);
    assert_eq!(
        f.events()[1].1,
        [
            f.tool.ptr() as u32,
            f.cell.ptr() as u32,
            requests.as_ptr() as u32,
            0x1234_5678
        ]
    );
    let mut after = unsafe { core::slice::from_raw_parts(f.cell.ptr(), 0xC0).to_vec() };
    assert_eq!(after[0x51], 1);
    after[0x51] = before[0x51];
    assert_eq!(before, after);
}

#[test]
fn preview_refreshes_each_corner_and_marks_only_when_preview_only_is_false() {
    let _fpu = GameFpu::new();
    for real in [0, 1] {
        for preview_only in [0, 1, 0x100, 0x102] {
            if real != 0 && preview_only as u8 == 0 {
                continue;
            }
            let mut f = Fixture::new();
            unsafe {
                NetworkTool::from_ptr(f.tool.ptr()).set_real(real);
                write(f.cell.ptr(), 0x56, 1u8);
            }
            assert_eq!(f.run(&mut [[1, 0x80, 7]], preview_only), f.manager.ptr() as u32);
            assert_eq!(f.ids(), [1, 3, 3, 3, 3, 4, 2]);
            let events = f.events();
            assert_eq!(
                events[1..5].iter().map(|e| e.1[1]).collect::<Vec<_>>(),
                [100_000, 100_001, 100_002, 100_003]
            );
            unsafe {
                assert_eq!(read::<u8>(f.cell.ptr(), 0x51), u8::from(preview_only as u8 == 0));
                assert_eq!(
                    (0..4)
                        .map(|i| read::<u32>(f.cell.ptr(), 0x58 + i * 4))
                        .collect::<Vec<_>>(),
                    [0x8000_0000, 1.25f32.to_bits(), 0x7FC1_2345, f32::INFINITY.to_bits()]
                );
            }
        }
    }
}

#[test]
fn prebuilt_direction_changes_preserve_the_other_argument_bytes() {
    for flags in [0, 0x400] {
        for old in [2, 3] {
            let mut f = Fixture::new();
            unsafe {
                NetworkTool::from_ptr(f.tool.ptr()).set_real(1);
                write(f.cell.ptr(), 0xC, f.occupant.ptr());
                write(f.cell.ptr(), 8, flags);
                write(f.cell.ptr(), 0x50, 2u8);
                write(f.occupant.ptr(), 4, 0x0040_0000u32);
            }
            STATE.with(|s| s.borrow_mut().old_direction = old);
            f.run(&mut [[10, 0, 7]], 0);
            let expected = if flags == 0 {
                vec![1, 0x20, 0x20]
            } else if old == 2 {
                vec![1, 0x20, 0x20, 0x114]
            } else {
                vec![1, 0x20, 0x20, 0x114, 0x118, 0x50, 0x50]
            };
            assert_eq!(f.ids(), expected);
            if flags != 0 && old != 2 {
                assert_eq!(f.events()[4].1[1], 0xCDEF_AB02);
                assert_eq!(f.events()[5].1[1], 0);
                assert_eq!(f.events()[6].1[1], 1);
            }
        }
    }
}

#[test]
fn connection_refresh_reinserts_with_zero_data_and_keeps_byte_argument_padding() {
    for model in [0, 0x1234] {
        for success in [false, true] {
            let mut f = Fixture::new();
            unsafe {
                NetworkTool::from_ptr(f.tool.ptr()).set_real(1);
                write(f.cell.ptr(), 0xC, f.occupant.ptr());
                write(f.cell.ptr(), 8, 1u32);
                write(f.cell.ptr(), 0x18, 0x1020_3040u32);
                write(f.occupant.ptr(), 4, 0x0020_0000u32);
            }
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                s.existing_model = model;
                s.same = success;
                s.resource_success = success;
            });
            f.run(&mut [[0x1020_3040, 0xFE12_3482, 7]], 0);
            let mut expected = vec![1, 0x20, 0x94, 0xC];
            if success {
                expected.push(8);
            }
            expected.extend([0x34, 0x3C]);
            if model != 0 {
                expected.push(0x68);
            }
            expected.extend([0x78, 0x28, 0x2C, 0x2C, 0x3C]);
            if model == 0 {
                expected.push(0xCC);
            }
            expected.extend([0xC0, 0x54, 0x50, 0xC0, 0x150, 0x50]);
            assert_eq!(f.ids(), expected);
            let events = f.events();
            assert_eq!(
                events.iter().find(|e| e.0 == 0xC).unwrap().1,
                [
                    f.resource.ptr() as u32,
                    0x6534_284A,
                    0x8765_4321,
                    0x1020_3040,
                    0xA521_60F5,
                    0,
                    0
                ]
            );
            assert_eq!(
                events.iter().find(|e| e.0 == 0x78).unwrap().1[1..],
                [0x1020_3040, if model != 0 { 0x1234_5678 } else { 0x82 }]
            );
            assert_eq!(events.iter().find(|e| e.0 == 0x150).unwrap().1[2], 0);
            assert_eq!(events.last().unwrap().1[1], 0xCDEF_AB00 | u32::from(success));
            if model == 0 {
                assert_eq!(
                    events.iter().find(|e| e.0 == 0xCC).unwrap().1[1],
                    if success { 0x1020_3040 } else { 0 }
                );
            }
        }
    }
}

#[test]
fn support_previews_lookup_both_hash_chains_and_keep_flip_padding() {
    let _fpu = GameFpu::new();
    for (solved, skipped) in [false, true]
        .into_iter()
        .flat_map(|solved| (0..8).map(move |skipped| (solved, skipped)))
    {
        let mut f = Fixture::new();
        let mut definition = Object::new(0xE8);
        let mut value_node = [0u32, 7, 0, 0x83];
        let mut wrong_value = [value_node.as_mut_ptr() as u32, 6, 0, 0];
        let mut values = [wrong_value.as_mut_ptr() as u32];
        let mut wrong_definition = [definition.ptr() as u32, 99];
        let mut definitions = [wrong_definition.as_mut_ptr() as u32];
        let mut grid = [7u8];
        let mut keys = [8u32, 7];
        let mut support = [
            5u32,
            10,
            20,
            0,
            0,
            0,
            0,
            keys.as_mut_ptr() as u32,
            keys.as_mut_ptr().wrapping_add(2) as u32,
            0,
        ];
        unsafe {
            write(definition.ptr(), 4, 5u32);
            let d = definition.ptr().add(8);
            write(d, 0x18, 2u32);
            write(d, 0x1C, 3u32);
            write(d, 0xC4, 1u32);
            write(d, 0xB8, grid.as_mut_ptr());
            write(d, 0xBC, grid.as_mut_ptr().wrapping_add(1));
            write(d, 0xD4, values.as_mut_ptr());
            write(d, 0xD8, values.as_mut_ptr().wrapping_add(1));
            let cell = InsertionCell::from_ptr(f.cell.ptr());
            cell.set_x(12);
            cell.set_z(23);
            cell.set_connections(1);
            write(f.cell.ptr(), 0x18 + 8, 0x100u32);
            let tool = NetworkTool::from_ptr(f.tool.ptr());
            tool.set_support_begin(support.as_mut_ptr().cast());
            tool.set_support_end(support.as_mut_ptr().wrapping_add(10).cast());
            tool.set_preview(0x1122_3344usize as *mut u8);
            write(f.tool.ptr(), 0xFC, 100u32);
            match skipped {
                1 => cell.set_x(11),
                2 => cell.set_x(13),
                3 => cell.set_z(22),
                4 => write(value_node.as_mut_ptr().cast(), 4, 8u32),
                5 => cell.set_connections(0),
                6 => write(f.cell.ptr(), 0x20, 0u32),
                7 => write(support.as_mut_ptr().cast(), 0x20, keys.as_mut_ptr()),
                _ => (),
            }
        }
        f.data(0x00B4_52B4, vec![definitions.as_mut_ptr() as u32]);
        f.data(0x00B4_52B8, vec![definitions.as_mut_ptr().wrapping_add(1) as u32]);
        STATE.with(|s| s.borrow_mut().separate = solved);
        assert_eq!(f.run(&mut [], 0), support.as_mut_ptr().wrapping_add(10) as u32);
        let events = f.events();
        if skipped != 0 {
            assert_eq!(
                f.ids(),
                if skipped == 7 {
                    vec![2]
                } else {
                    vec![2, 1, 1, 3, 3, 3, 3]
                }
            );
            assert_eq!(
                unsafe { InsertionCell::from_ptr(f.cell.ptr()).placed() },
                u8::from(skipped != 7)
            );
            continue;
        }
        assert_eq!(
            f.ids(),
            if solved {
                vec![2, 1, 1, 3, 3, 3, 3, 6, 7, 10]
            } else {
                vec![2, 1, 1, 3, 3, 3, 3, 6, 9]
            }
        );
        assert_eq!(events[7].1[1..], [12, 23, 2, 3, 0xCDEF_AB01]);
        if solved {
            assert_eq!(events[8].1[1..], [123, 2]);
            assert_eq!(
                events[9].1,
                [
                    0x1122_3344,
                    12,
                    23,
                    0xFF00_00FF,
                    f.cell.ptr() as u32 + 0x58,
                    0x7AB5_0E44,
                    0x1ABE_787D,
                    0x1357_2468,
                    3,
                    0xCDEF_AB01,
                    102,
                    0
                ]
            );
        } else {
            assert_eq!(
                events[8].1,
                [0x1122_3344, 12, 23, 0xFF80_80FF, f.cell.ptr() as u32 + 0x58, 102, 0]
            );
        }
    }
}

#[test]
fn connection_manager_slots_are_read_after_the_occupant_interface_call() {
    let mut f = Fixture::new();
    unsafe {
        NetworkTool::from_ptr(f.tool.ptr()).set_real(1);
        write(f.cell.ptr(), 0xC, f.occupant.ptr());
        write(f.occupant.ptr(), 4, 0x0020_0000u32);
    }
    STATE.with(|s| s.borrow_mut().replace_manager_slots = true);
    f.run(&mut [[9, 2, 7]], 0);
    let events = f.events();
    assert!(!events.iter().any(|e| e.0 == 0x54 || e.0 == 0x150));
    assert_eq!(events.iter().find(|e| e.0 == 0x154).unwrap().1[2..], [0, 0]);
    assert_eq!(events.iter().find(|e| e.0 == 0x155).unwrap().1[2], 0);
}

#[test]
fn exported_entry_pops_three_words_and_returns_the_empty_vector_end() {
    let mut f = Fixture::new();
    unsafe {
        NetworkTool::from_ptr(f.tool.ptr()).set_real(1);
        let vector = [0x1234_5600u32; 3];
        let result = call_thiscall_with_eax(
            fc4p_006258c0 as *const () as usize,
            f.tool.ptr(),
            &[vector.as_ptr() as u32, 0, 0],
            0xFEED_1234,
        );
        assert_eq!(result.0, 0x1234_5600);
    }
    assert!(f.events().is_empty());
}
