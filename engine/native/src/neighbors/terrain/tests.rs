// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Terrain marker cases from the core, including inclusive bounds and the stored height rounding.

use super::*;
use crate::neighbors::tests::checked_call;
use crate::testing::{fake, fake_data, Fake};
use std::cell::RefCell;

#[derive(Debug, PartialEq)]
enum Call {
    Width,
    Height,
    Cell(u32),
    Centre(i32, i32),
    Surface(i32, i32),
    Kind(u32),
    Framework,
    Service,
    ReleaseLocal,
    Allocate(u32),
    Props([i32; 4]),
    Holder(u32),
    Marker(u32),
    Move(u32, [u32; 3]),
    Update(u32, u32),
    Submit,
    Clear,
    Free,
}
#[derive(Default)]
struct State {
    calls: Vec<Call>,
    width: i32,
    height: i32,
    surface: f64,
    allocator: usize,
    framework: usize,
    props: Vec<usize>,
    invalidate: bool,
    #[allow(clippy::vec_box)]
    nodes: Vec<Box<[u32; 3]>>,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn record(c: Call) {
    STATE.with(|s| s.borrow_mut().calls.push(c));
}
fn take() -> Vec<Call> {
    STATE.with(|s| core::mem::take(&mut s.borrow_mut().calls))
}
fn node() -> *mut u8 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.nodes.push(Box::new([0xA5A5_A5A5; 3]));
        s.nodes.last_mut().unwrap().as_mut_ptr().cast()
    })
}

#[repr(C)]
struct Connection {
    table: *const usize,
    kind: u32,
    x: i32,
    z: i32,
    id: u32,
}
#[repr(C)]
struct Prop {
    table: *const usize,
    id: u32,
    marker: u8,
    property_exists: bool,
}

extern "thiscall" fn width(_this: *mut u8) -> u64 {
    record(Call::Width);
    pair(STATE.with(|s| s.borrow().width) as u32, 1)
}
extern "thiscall" fn height(_this: *mut u8) -> u64 {
    record(Call::Height);
    pair(STATE.with(|s| s.borrow().height) as u32, 2)
}
extern "thiscall" fn cell(this: *mut Connection, x: *mut i32, z: *mut i32) -> u64 {
    record(Call::Cell(unsafe { (*this).id }));
    unsafe {
        x.write((*this).x);
        z.write((*this).z);
    }
    pair(3, 4)
}
extern "thiscall" fn centre(_this: *mut u8, x: i32, z: i32, wx: *mut u32, wz: *mut u32) -> u64 {
    record(Call::Centre(x, z));
    let x = (F80::from_i32(x) * F80::from_f32(16.0) + F80::from_f32(8.0))
        .to_f32()
        .to_bits();
    let z = (F80::from_i32(z) * F80::from_f32(16.0) + F80::from_f32(8.0))
        .to_f32()
        .to_bits();
    unsafe {
        wx.write(x);
        wz.write(z);
    }
    pair(5, 6)
}
extern "thiscall" fn surface(_this: *mut u8, x: i32, z: i32) -> f64 {
    record(Call::Surface(x, z));
    STATE.with(|s| s.borrow().surface)
}
extern "thiscall" fn kind(this: *mut Connection) -> u64 {
    record(Call::Kind(unsafe { (*this).id }));
    pair(unsafe { (*this).kind }, 7)
}
extern "C" fn framework() -> u64 {
    record(Call::Framework);
    pair(STATE.with(|s| s.borrow().framework) as u32, 8)
}
extern "thiscall" fn service(_this: *mut u8, service: u32, iid: u32, out: *mut usize) -> u64 {
    assert_eq!((service, iid), (0x3AE4_BEA3, 0x3AE4_BEAB));
    record(Call::Service);
    unsafe {
        out.write(STATE.with(|s| s.borrow().allocator));
    }
    pair(1, 9)
}
extern "thiscall" fn release_local(local: *mut usize) -> u64 {
    record(Call::ReleaseLocal);
    assert_eq!(unsafe { prop_allocator().read() } as usize, unsafe { local.read() });
    unsafe {
        local.write(0);
    }
    pair(10, 11)
}
extern "thiscall" fn allocate(_this: *mut u8, bytes: u32) -> u64 {
    record(Call::Allocate(bytes));
    pair(node() as u32, 12)
}
extern "thiscall" fn props(_this: *mut u8, list: *mut *mut u8, bounds: *const [i32; 4]) -> u64 {
    record(Call::Props(unsafe { bounds.read() }));
    let objects = STATE.with(|s| s.borrow().props.clone());
    unsafe {
        let header = list.read();
        assert_eq!(pointer(header, 0), header);
        assert_eq!(pointer(header, 4), header);
        assert_eq!(word(header, 8), 0xA5A5_A5A5);
        for object in objects {
            let n = node();
            let prev = pointer(header, 4);
            n.cast::<*mut u8>().write(header);
            n.add(4).cast::<*mut u8>().write(prev);
            n.add(8).cast::<usize>().write(object);
            prev.cast::<*mut u8>().write(n);
            header.add(4).cast::<*mut u8>().write(n);
        }
    }
    pair(13, 14)
}
extern "thiscall" fn holder(this: *mut Prop) -> u64 {
    record(Call::Holder(unsafe { (*this).id }));
    pair(this as u32, 15)
}
extern "C" fn marker(this: *mut Prop, id: u32, out: *mut u8) -> u64 {
    assert_eq!(id, 0xCAA4_6595);
    assert_eq!(unsafe { out.read() }, 0);
    record(Call::Marker(unsafe { (*this).id }));
    if unsafe { (*this).property_exists } {
        unsafe {
            out.write((*this).marker);
        }
    }
    pair(0xAABB_CC80, 16)
}
extern "thiscall" fn move_prop(this: *mut Prop, position: *const [u32; 3]) -> u64 {
    record(Call::Move(unsafe { (*this).id }, unsafe { position.read() }));
    pair(17, 18)
}
extern "thiscall" fn update(_this: *mut u8, prop: *mut Prop, flag: u32) -> u64 {
    record(Call::Update(unsafe { (*prop).id }, flag));
    pair(19, 20)
}
extern "thiscall" fn submit(_this: *mut u8, _list: *mut *mut u8) -> u64 {
    record(Call::Submit);
    pair(21, 22)
}
extern "thiscall" fn clear(list: *mut *mut u8) -> u64 {
    record(Call::Clear);
    unsafe {
        let h = list.read();
        h.cast::<*mut u8>().write(h);
        h.add(4).cast::<*mut u8>().write(h);
    }
    if STATE.with(|s| s.borrow().invalidate) {
        unsafe {
            prop_allocator().write(core::ptr::null_mut());
        }
    }
    pair(23, 24)
}
extern "thiscall" fn free(_this: *mut u8, header: *mut u8) -> u64 {
    record(Call::Free);
    unsafe {
        assert_eq!(pointer(header, 0), header);
        assert_eq!(pointer(header, 4), header);
    }
    pair(0xDEAD_BEEF, 0xCAFE_BABE)
}

struct World {
    sim: [u32; 18],
    connections: Vec<Connection>,
    connection_nodes: Vec<[u32; 3]>,
    props: [Prop; 3],
    _tables: [Box<[usize; 100]>; 7],
    _objects: [Box<usize>; 6],
    _globals: [Box<*mut u8>; 4],
    _fakes: Vec<Fake>,
}
impl World {
    fn new(cached: bool) -> Self {
        let mut tables = core::array::from_fn::<_, 7, _>(|_| Box::new([0usize; 100]));
        tables[0][0x174 / 4] = width as *const () as usize;
        tables[0][0x178 / 4] = height as *const () as usize;
        tables[0][0x184 / 4] = centre as *const () as usize;
        tables[1][0x18 / 4] = cell as *const () as usize;
        tables[1][0x14 / 4] = kind as *const () as usize;
        tables[2][0x48 / 4] = surface as *const () as usize;
        tables[3][0x18 / 4] = holder as *const () as usize;
        tables[3][0x24 / 4] = move_prop as *const () as usize;
        tables[4][0x48 / 4] = props as *const () as usize;
        tables[5][0x60 / 4] = update as *const () as usize;
        tables[5][0xC0 / 4] = submit as *const () as usize;
        tables[6][3] = allocate as *const () as usize;
        tables[6][4] = free as *const () as usize;
        tables[6][0x14 / 4] = service as *const () as usize;
        let mut objects = [
            Box::new(tables[0].as_ptr() as usize),
            Box::new(tables[2].as_ptr() as usize),
            Box::new(tables[4].as_ptr() as usize),
            Box::new(tables[5].as_ptr() as usize),
            Box::new(tables[6].as_ptr() as usize),
            Box::new(tables[6].as_ptr() as usize),
        ];
        let ap = (&mut *objects[4] as *mut usize).cast::<u8>();
        STATE.with(|s| {
            *s.borrow_mut() = State {
                width: 64,
                height: 64,
                surface: 280.0,
                allocator: ap as usize,
                framework: &mut *objects[5] as *mut usize as usize,
                ..State::default()
            }
        });
        let mut globals = [
            Box::new((&mut *objects[1] as *mut usize).cast()),
            Box::new((&mut *objects[2] as *mut usize).cast()),
            Box::new((&mut *objects[3] as *mut usize).cast()),
            Box::new(if cached { ap } else { core::ptr::null_mut() }),
        ];
        let mut fakes = Vec::new();
        for (index, address) in [0x00B4_3CF4, 0x00B4_3D10, 0x00B4_3D0C, 0x00B2_07E4]
            .into_iter()
            .enumerate()
        {
            fakes.push(fake_data(address, (&mut *globals[index] as *mut *mut u8).cast()));
        }
        for (address, function) in [
            (0x0087_93EC, framework as *const () as usize),
            (0x0041_1870, release_local as *const () as usize),
            (0x005F_D390, marker as *const () as usize),
            (0x0041_3E60, clear as *const () as usize),
        ] {
            fakes.push(fake(address, function));
        }
        let mut sim = [0xA5A5_A5A5; 18];
        sim[0x14 / 4] = &mut *objects[0] as *mut usize as u32;
        Self {
            sim,
            connections: vec![],
            connection_nodes: vec![],
            props: core::array::from_fn(|i| Prop {
                table: tables[3].as_ptr(),
                id: i as u32 + 1,
                marker: if i == 0 { 0x80 } else { 0 },
                property_exists: i < 2,
            }),
            _tables: tables,
            _objects: objects,
            _globals: globals,
            _fakes: fakes,
        }
    }
    fn connection(&mut self, kind: u32, x: i32, z: i32) {
        self.connections.push(Connection {
            table: self._tables[1].as_ptr(),
            kind,
            x,
            z,
            id: self.connections.len() as u32 + 1,
        });
    }
    fn run(&mut self, rect: [i32; 4], with_props: bool) -> u64 {
        self.connection_nodes = vec![[0xA5A5_A5A5; 3]; self.connections.len() + 1];
        let count = self.connection_nodes.len();
        for i in 0..count {
            self.connection_nodes[i][0] = self.connection_nodes[(i + 1) % count].as_mut_ptr() as u32;
            self.connection_nodes[i][1] = self.connection_nodes[(i + count - 1) % count].as_mut_ptr() as u32;
            if i > 0 {
                self.connection_nodes[i][2] = &mut self.connections[i - 1] as *mut _ as u32;
            }
        }
        self.sim[0x1C / 4] = self.connection_nodes[0].as_mut_ptr() as u32;
        STATE.with(|s| {
            s.borrow_mut().props = if with_props {
                self.props.iter_mut().map(|p| p as *mut _ as usize).collect()
            } else {
                vec![]
            }
        });
        let before = self.sim;
        let before_nodes = self.connection_nodes.clone();
        let mut args = [rect.as_ptr() as u32];
        let result = unsafe {
            checked_call(
                fc4p_006d0be0 as *const () as usize,
                self.sim.as_mut_ptr().cast(),
                &mut args,
                0xDEAD_BEEF,
                0xCAFE_BABE,
                Conv::Thiscall,
            )
        };
        assert_eq!(args, [rect.as_ptr() as u32]);
        assert_eq!(self.sim, before);
        assert_eq!(self.connection_nodes, before_nodes);
        result
    }
}

#[test]
fn core_terrain_edit_moves_only_markers_at_clamped_cells_inside_the_inclusive_rectangle() {
    let mut w = World::new(true);
    w.connection(2, -1, 5);
    w.connection(3, 64, 9);
    w.connection(1, 20, -1);
    assert_eq!(w.run([0, 5, 63, 9], true), pair(0, 20));
    let mut expected = vec![Call::Width, Call::Height];
    for (id, x, z, height) in [(1, 0, 5, 280.0f32), (2, 63, 9, 295.5)] {
        let position = [
            (x as f32 * 16.0 + 8.0).to_bits(),
            height.to_bits(),
            (z as f32 * 16.0 + 8.0).to_bits(),
        ];
        expected.extend([
            Call::Cell(id),
            Call::Centre(x, z),
            Call::Surface(x, z),
            Call::Kind(id),
            Call::Allocate(12),
            Call::Props([x, z, x, z]),
            Call::Holder(1),
            Call::Marker(1),
            Call::Move(1, position),
            Call::Update(1, 1),
            Call::Holder(2),
            Call::Marker(2),
            Call::Holder(3),
            Call::Marker(3),
            Call::Submit,
            Call::Clear,
            Call::Free,
        ]);
    }
    expected.push(Call::Cell(3));
    assert_eq!(take(), expected);
}

#[test]
fn elevated_types_and_height_store_rounding_use_the_real_x87() {
    for kind in [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, u32::MAX] {
        for (surface, ordinary, elevated) in [
            (280.0, 280.0f32.to_bits(), 295.5f32.to_bits()),
            (16_777_217.5, 16_777_218.0f32.to_bits(), 16_777_234.0f32.to_bits()),
            (f64::INFINITY, f32::INFINITY.to_bits(), f32::INFINITY.to_bits()),
            (f64::from_bits(0x7FF8_0000_2000_0000), 0x7FC0_0001, 0x7FC0_0001),
        ] {
            let mut w = World::new(true);
            w.connection(kind, 5, 5);
            STATE.with(|s| s.borrow_mut().surface = surface);
            assert_eq!(w.run([5, 5, 5, 5], true), pair(0xDEAD_BEEF, 0xCAFE_BABE));
            let calls = take();
            let height = if matches!(kind, 3 | 7 | 11) { elevated } else { ordinary };
            assert!(calls.contains(&Call::Move(1, [88.0f32.to_bits(), height, 88.0f32.to_bits()])));
        }
    }
}

#[test]
fn absent_props_still_submit_clear_and_free_and_allocator_is_reloaded_for_cleanup() {
    let mut w = World::new(false);
    w.connection(9, -1, 5);
    STATE.with(|s| s.borrow_mut().invalidate = true);
    assert_eq!(w.run([0, 5, 0, 5], false), pair(0xDEAD_BEEF, 0xCAFE_BABE));
    assert_eq!(
        take(),
        [
            Call::Width,
            Call::Height,
            Call::Cell(1),
            Call::Centre(0, 5),
            Call::Surface(0, 5),
            Call::Kind(1),
            Call::Framework,
            Call::Service,
            Call::ReleaseLocal,
            Call::Allocate(12),
            Call::Props([0, 5, 0, 5]),
            Call::Submit,
            Call::Clear,
            Call::Framework,
            Call::Service,
            Call::ReleaseLocal,
            Call::Free
        ]
    );
}

#[test]
fn empty_connections_and_rejected_cells_keep_original_return_words_without_other_calls() {
    let mut w = World::new(true);
    let h = w.run([0, 0, 63, 63], true);
    assert_eq!(h, pair(w.sim[0x1C / 4], 2));
    assert_eq!(take(), [Call::Width, Call::Height]);
    for ((x, z), rect, expected) in [
        ((-1, 5), [1, 0, 63, 63], (5, 0)),
        ((5, -1), [0, 1, 63, 63], (0, 5)),
        ((64, 5), [0, 0, 62, 63], (5, 63)),
        ((5, 64), [0, 0, 63, 62], (63, 5)),
        ((5, 5), [6, 6, 4, 4], (5, 5)),
    ] {
        let mut w = World::new(true);
        w.connection(2, x, z);
        assert_eq!(w.run(rect, true), pair(expected.0, expected.1));
        assert_eq!(take(), [Call::Width, Call::Height, Call::Cell(1)]);
    }
}

#[test]
fn signed_clamping_checks_negative_coordinates_before_negative_maximums() {
    for (dimensions, cell, expected) in [
        ([0, 0], [-1, -1], [0, 0]),
        ([0, 0], [0, 0], [-1, -1]),
        ([i32::MIN, i32::MIN], [i32::MAX, i32::MAX], [i32::MAX, i32::MAX]),
    ] {
        let mut w = World::new(true);
        w.connection(2, cell[0], cell[1]);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.width = dimensions[0];
            s.height = dimensions[1];
        });
        assert_eq!(
            w.run([1, 1, -1, -1], false),
            pair(expected[1] as u32, expected[0] as u32)
        );
        assert_eq!(take(), [Call::Width, Call::Height, Call::Cell(1)]);
    }
}
