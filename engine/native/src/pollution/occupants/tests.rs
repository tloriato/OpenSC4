// SPDX-License-Identifier: GPL-3.0-or-later
//! Insertion/removal vectors from core pollution, with each source and facility call recorded.

use super::*;
use crate::testing::{fake, fake_data, Fake, GameFpu, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq)]
enum Call {
    Holder,
    Excluded,
    Property(u32),
    Variant(u32),
    Array(u32),
    Bounds,
    Cell(i32, i32),
    CountX,
    CountZ,
    Shift,
    Get(i32, i32),
    Put(i32, i32, i32),
    Position,
    Width(u32),
    Height(u32),
    Tract(u32),
    Size(u32),
    Data(u32),
    Cone(u32, i32, i32, i32, i32, [u32; 3]),
    List(usize, u32),
    Ensure(u32),
    Signed(u32, i32),
    Unsigned(u32, u32),
    Current,
    Clear(u32, u32, u32),
    Key,
    Register(bool, u32),
    Cost(i64),
}
struct State {
    calls: Vec<Call>,
    excluded: Option<u8>,
    properties: Vec<(u32, *mut u8)>,
    current: Option<u32>,
    key_type: Option<u32>,
    node: *mut u8,
}
thread_local! {static STATE:RefCell<State>=const{RefCell::new(State{calls:Vec::new(),excluded:None,properties:Vec::new(),current:None,key_type:None,node:core::ptr::null_mut()})};}
fn log(call: Call) {
    STATE.with(|s| s.borrow_mut().calls.push(call));
}
fn calls() -> Vec<Call> {
    STATE.with(|s| std::mem::take(&mut s.borrow_mut().calls))
}
#[repr(C)]
struct Occupant {
    table: *const usize,
    holder: *mut u8,
    minimum: [f32; 3],
    maximum: [f32; 3],
    position: [f32; 3],
}
#[repr(C)]
struct Property {
    table: *const usize,
    id: u32,
    data: *mut u8,
}
#[repr(C)]
struct Grid {
    table: *const usize,
    id: u32,
}
extern "thiscall" fn holder(this: *mut Occupant) -> *mut u8 {
    log(Call::Holder);
    unsafe { (*this).holder }
}
extern "C" fn excluded(_holder: *mut u8, id: u32, output: *mut u8) -> u32 {
    assert_eq!(id, 0x0292_44DB);
    log(Call::Excluded);
    STATE.with(|s| {
        if let Some(value) = s.borrow().excluded {
            unsafe { output.write(value) };
            0x1234_5601
        } else {
            0x1234_5600
        }
    })
}
extern "thiscall" fn property(_this: *mut u8, id: u32) -> *mut u8 {
    log(Call::Property(id));
    STATE.with(|s| {
        s.borrow()
            .properties
            .iter()
            .find(|&&(key, _)| key == id)
            .map_or(core::ptr::null_mut(), |&(_, p)| p)
    })
}
extern "thiscall" fn variant(this: *mut Property) -> *mut u8 {
    log(Call::Variant(unsafe { (*this).id }));
    this.cast()
}
extern "thiscall" fn array(this: *mut Property) -> *mut u8 {
    log(Call::Array(unsafe { (*this).id }));
    unsafe { (*this).data }
}
extern "thiscall" fn bounds(this: *mut Occupant, minimum: *mut [f32; 3], maximum: *mut [f32; 3]) {
    log(Call::Bounds);
    unsafe {
        minimum.write((*this).minimum);
        maximum.write((*this).maximum)
    }
}
extern "thiscall" fn position(this: *mut Occupant, output: *mut [f32; 3]) {
    log(Call::Position);
    unsafe { output.write((*this).position) }
}
extern "thiscall" fn cell(_this: *mut u8, x: f32, z: f32, out_x: *mut i32, out_z: *mut i32) {
    log(Call::Cell(x as i32, z as i32));
    unsafe {
        out_x.write(x as i32);
        out_z.write(z as i32)
    }
}
extern "thiscall" fn count_x(_this: *mut u8) -> i32 {
    log(Call::CountX);
    4
}
extern "thiscall" fn count_z(_this: *mut u8) -> i32 {
    log(Call::CountZ);
    4
}
extern "thiscall" fn shift(_this: *mut u8) -> u32 {
    log(Call::Shift);
    1
}
extern "thiscall" fn get(_this: *mut u8, x: i32, z: i32) -> u32 {
    log(Call::Get(x, z));
    0xAABB_FFFE
}
extern "thiscall" fn put(_this: *mut u8, x: i32, z: i32, value: i32) {
    log(Call::Put(x, z, value))
}
extern "thiscall" fn width(this: *mut Grid) -> i32 {
    log(Call::Width(unsafe { (*this).id }));
    2
}
extern "thiscall" fn height(this: *mut Grid) -> i32 {
    log(Call::Height(unsafe { (*this).id }));
    3
}
extern "thiscall" fn tract(this: *mut Grid, _x: f32, _z: f32, x: *mut i32, z: *mut i32) {
    log(Call::Tract(unsafe { (*this).id }));
    unsafe {
        x.write(7);
        z.write(-8)
    }
}
extern "thiscall" fn size(this: *mut Grid) -> u32 {
    log(Call::Size(unsafe { (*this).id }));
    2
}
extern "thiscall" fn data(this: *mut Grid) -> *mut u8 {
    log(Call::Data(unsafe { (*this).id }));
    this.cast()
}
extern "C" fn cone(data: *mut Grid, width: i32, height: i32, x: i32, z: i32, shape: f32, amount: f32, radius: f32) {
    log(Call::Cone(
        unsafe { (*data).id },
        width,
        height,
        x,
        z,
        [shape.to_bits(), amount.to_bits(), radius.to_bits()],
    ));
}
extern "thiscall" fn change_list(this: *mut u8, list: *mut *mut u8, occupant: *mut u8, insert: u32) {
    let offset = list as usize - this as usize;
    log(Call::List(offset, insert));
    if offset == 0x74 {
        unsafe {
            let end = *list;
            let node = STATE.with(|s| s.borrow().node);
            if insert as u8 != 0 {
                ListNode::from_ptr(end).set_next(node);
                ListNode::from_ptr(end).set_previous(node);
                ListNode::from_ptr(node).set_next(end);
                ListNode::from_ptr(node).set_previous(end);
                ListNode::from_ptr(node).set_occupant(occupant);
            } else {
                ListNode::from_ptr(end).set_next(end);
                ListNode::from_ptr(end).set_previous(end);
            }
        }
    }
}
extern "thiscall" fn ensure(_this: *mut u8, id: u32, data: u32, count: u32, flags: u32) {
    assert_eq!((data, count, flags), (0, 1, 0));
    log(Call::Ensure(id))
}
extern "C" fn signed(_this: *mut u8, id: u32, value: i32) {
    log(Call::Signed(id, value))
}
extern "C" fn unsigned(_this: *mut u8, id: u32, value: u32) {
    log(Call::Unsigned(id, value))
}
extern "C" fn current(_this: *mut u8, id: u32, output: *mut u32) -> u32 {
    assert_eq!(id, 0x2781_2852);
    log(Call::Current);
    STATE.with(|s| {
        if let Some(v) = s.borrow().current {
            unsafe { output.write(v) };
            1
        } else {
            0
        }
    })
}
extern "thiscall" fn clear(_this: *mut u8, id: u32, value: u32, flags: u32) {
    log(Call::Clear(id, value, flags))
}
extern "thiscall" fn key(_this: *mut u8, _holder: *mut u8, id: u32, output: *mut u32) -> u32 {
    assert_eq!(id, 0xCA58_EDC7);
    log(Call::Key);
    assert_eq!(unsafe { output.add(7).read() }, 0);
    STATE.with(|s| {
        if let Some(kind) = s.borrow().key_type {
            unsafe {
                output.add(1).write(kind);
                output.add(4).cast::<i64>().write_unaligned(-1000);
                output.add(6).write(42);
            }
            0x1122_3301
        } else {
            0x1122_3300
        }
    })
}
extern "thiscall" fn register(_this: *mut u8, _holder: *mut u8, id: u32) {
    log(Call::Register(true, id))
}
extern "thiscall" fn unregister(_this: *mut u8, _holder: *mut u8, id: u32) {
    log(Call::Register(false, id))
}
extern "C" fn cost(_this: *mut u8, id: u32, value: i64) -> u32 {
    assert_eq!(id, 0x2A58_D350);
    log(Call::Cost(value));
    0xCAFE_BABE
}
fn table(slots: &[(usize, usize)]) -> Box<[usize; 120]> {
    let mut t = Box::new([0; 120]);
    for &(slot, f) in slots {
        t[slot / 4] = f;
    }
    t
}
struct World {
    sim: Object,
    occupant: Box<Occupant>,
    holder: Box<[u32; 1]>,
    city: Box<[u32; 1]>,
    city_pointer: Box<*mut u8>,
    budget: Box<[u32; 1]>,
    budget_pointer: Box<*mut u8>,
    grids: [Box<Grid>; 4],
    emissions: Box<[i32; 4]>,
    radii: Box<[i32; 4]>,
    shapes: Box<[f32; 4]>,
    default_radii: Box<[i32; 4]>,
    capacity: Box<i32>,
    sentinel: Box<[u32; 3]>,
    node: Box<[u32; 3]>,
    // Every stored table/property address remains stable.
    #[allow(clippy::vec_box)]
    tables: Vec<Box<[usize; 120]>>,
    properties: Vec<(*mut u8, Box<Property>)>,
    fakes: Vec<Fake>,
}
impl World {
    fn new() -> Self {
        let tables = vec![
            table(&[
                (0x18, holder as *const () as usize),
                (0x20, position as *const () as usize),
                (0x28, bounds as *const () as usize),
            ]),
            table(&[
                (0x24, property as *const () as usize),
                (0x28, ensure as *const () as usize),
                (0x34, clear as *const () as usize),
            ]),
            table(&[
                (0x14, variant as *const () as usize),
                (0x124, array as *const () as usize),
                (0x1A4, array as *const () as usize),
                (0x1BC, array as *const () as usize),
            ]),
            table(&[
                (0x17C, cell as *const () as usize),
                (0x174, count_x as *const () as usize),
                (0x178, count_z as *const () as usize),
            ]),
            table(&[
                (0x2C, shift as *const () as usize),
                (0x1C, get as *const () as usize),
                (0x68, put as *const () as usize),
                (0x30, width as *const () as usize),
                (0x34, height as *const () as usize),
                (0x4C, tract as *const () as usize),
                (0x28, size as *const () as usize),
                (0x60, data as *const () as usize),
            ]),
            table(&[
                (0xCC, key as *const () as usize),
                (0xDC, register as *const () as usize),
                (0xE0, unregister as *const () as usize),
            ]),
        ];
        let mut w = Self {
            sim: Object::new(0x240),
            occupant: Box::new(Occupant {
                table: tables[0].as_ptr(),
                holder: core::ptr::null_mut(),
                minimum: [-1.0, 0.0, 1.0],
                maximum: [1.0, 0.0, 2.0],
                position: [16.0, 9.0, 32.0],
            }),
            holder: Box::new([tables[1].as_ptr() as u32]),
            city: Box::new([tables[3].as_ptr() as u32]),
            city_pointer: Box::new(core::ptr::null_mut()),
            budget: Box::new([tables[5].as_ptr() as u32]),
            budget_pointer: Box::new(core::ptr::null_mut()),
            grids: core::array::from_fn(|i| {
                Box::new(Grid {
                    table: tables[4].as_ptr(),
                    id: i as u32,
                })
            }),
            emissions: Box::new([100, -7, 5, 20]),
            radii: Box::new([4, -3, 0, 8]),
            shapes: Box::new([1.0, 0.0, f32::NAN, 2.0]),
            default_radii: Box::new([6, 7, 8, 9]),
            capacity: Box::new(500),
            sentinel: Box::new([0; 3]),
            node: Box::new([0; 3]),
            tables,
            properties: vec![],
            fakes: vec![],
        };
        w.occupant.holder = w.holder.as_mut_ptr().cast();
        *w.city_pointer = w.city.as_mut_ptr().cast();
        *w.budget_pointer = w.budget.as_mut_ptr().cast();
        let sim = unsafe { ChangedView::from_ptr(w.sim.ptr()) };
        sim.set_city(*w.city_pointer);
        sim.set_air((&mut *w.grids[0] as *mut Grid).cast());
        sim.set_water((&mut *w.grids[1] as *mut Grid).cast());
        sim.set_garbage((&mut *w.grids[2] as *mut Grid).cast());
        sim.set_radiation((&mut *w.grids[3] as *mut Grid).cast());
        let end = w.sentinel.as_mut_ptr() as u32;
        *w.sentinel = [end, end, 0];
        sim.set_recycling(end as *mut u8);
        STATE.with(|s| {
            *s.borrow_mut() = State {
                calls: vec![],
                excluded: None,
                properties: vec![],
                current: None,
                key_type: None,
                node: w.node.as_mut_ptr().cast(),
            }
        });
        w.fakes = vec![
            fake(0x005F_D390, excluded as *const () as usize),
            fake(0x005F_D480, current as *const () as usize),
            fake(0x005F_E330, signed as *const () as usize),
            fake(0x005F_E3C0, unsigned as *const () as usize),
            fake(0x005F_E450, cost as *const () as usize),
            fake(0x006E_45F0, change_list as *const () as usize),
            fake(0x005E_4500, cone as *const () as usize),
            fake_data(0x00B4_A598, w.default_radii.as_mut_ptr().cast()),
            fake_data(0x00B4_3CEC, (&mut *w.city_pointer as *mut *mut u8).cast()),
            fake_data(0x00B4_3D64, (&mut *w.budget_pointer as *mut *mut u8).cast()),
        ];
        w
    }
    fn add(&mut self, id: u32, data: *mut u8) {
        let mut property = Box::new(Property {
            table: self.tables[2].as_ptr(),
            id,
            data,
        });
        let ptr = (&mut *property as *mut Property).cast();
        STATE.with(|s| s.borrow_mut().properties.push((id, ptr)));
        self.properties.push((ptr, property));
    }
    fn pollution(&mut self, custom: bool) {
        let p = self.emissions.as_mut_ptr().cast();
        self.add(0x2781_2851, p);
        if custom {
            let r = self.radii.as_mut_ptr().cast();
            self.add(0xAA58_32F3, r);
            let s = self.shapes.as_mut_ptr().cast();
            self.add(0x68EE_9764, s);
        }
    }
    fn run(&mut self, insert: u32) -> u32 {
        unsafe { fc4p_006e4680(self.sim.ptr(), (&mut *self.occupant as *mut Occupant).cast(), insert) }
    }
}

#[test]
fn preview_occupant_skips_every_grid_list_and_budget_operation() {
    let mut w = World::new();
    w.pollution(false);
    STATE.with(|s| s.borrow_mut().excluded = Some(0x80));
    let before = unsafe { std::slice::from_raw_parts(w.sim.ptr(), 0x240) }.to_vec();
    assert_eq!(w.run(1), 0x1234_5680);
    assert_eq!(calls(), [Call::Holder, Call::Excluded]);
    assert_eq!(unsafe { std::slice::from_raw_parts(w.sim.ptr(), 0x240) }, before);
}

#[test]
fn source_rectangles_and_signed_cone_arguments_follow_insertion_and_removal() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    w.pollution(true);
    for (insert, sign) in [(1, 1), (0, -1)] {
        assert_eq!(w.run(insert), 0x1122_3300);
        let c = calls();
        assert_eq!(
            &c[..9],
            &[
                Call::Holder,
                Call::Excluded,
                Call::Property(0x2781_2851),
                Call::Variant(0x2781_2851),
                Call::Array(0x2781_2851),
                Call::Property(0xAA58_32F3),
                Call::Variant(0xAA58_32F3),
                Call::Array(0xAA58_32F3),
                Call::Bounds
            ]
        );
        let cells: Vec<_> = c
            .iter()
            .filter(|c| matches!(c, Call::Get(..) | Call::Put(..)))
            .collect();
        assert_eq!(
            cells,
            vec![
                &Call::Get(0, 1),
                &Call::Put(0, 0, -2 + 5 * sign),
                &Call::Get(0, 2),
                &Call::Put(0, 1, -2 + 5 * sign),
                &Call::Get(1, 1),
                &Call::Put(0, 0, -2 + 5 * sign),
                &Call::Get(1, 2),
                &Call::Put(0, 1, -2 + 5 * sign)
            ]
        );
        let cones: Vec<_> = c.iter().filter(|c| matches!(c, Call::Cone(..))).collect();
        assert_eq!(
            cones,
            vec![
                &Call::Cone(
                    0,
                    2,
                    3,
                    7,
                    -8,
                    [
                        1.0f32.to_bits(),
                        ((100 * sign) as f32).to_bits(),
                        ((4 * sign) as f32).to_bits()
                    ]
                ),
                &Call::Cone(
                    3,
                    2,
                    3,
                    7,
                    -8,
                    [
                        2.0f32.to_bits(),
                        ((20 * sign) as f32).to_bits(),
                        ((8 * sign) as f32).to_bits()
                    ]
                )
            ]
        );
        assert_eq!(unsafe { ChangedView::from_ptr(w.sim.ptr()) }.dirty_garbage(), 1);
    }
}

#[test]
fn default_shapes_radii_clamped_bounds_and_empty_rectangle() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    w.pollution(false);
    w.occupant.minimum = [7.0, 0.0, 7.0];
    w.occupant.maximum = [-1.0, 0.0, -1.0];
    w.run(1);
    let c = calls();
    assert!(!c.iter().any(|c| matches!(c, Call::Get(..) | Call::Put(..))));
    for (id, shape, radius) in [(0, 2.5f32, 6.0f32), (1, 1.5, 7.0), (2, 0.5, 8.0), (3, 4.0, 9.0)] {
        assert!(c.contains(&Call::Cone(
            id,
            2,
            3,
            7,
            -8,
            [
                shape.to_bits(),
                (w.emissions[id as usize] as f32).to_bits(),
                radius.to_bits()
            ]
        )));
    }
}

#[test]
fn facilities_create_usage_properties_recount_recycling_and_register_budget() {
    let mut w = World::new();
    let p = (&mut *w.capacity as *mut i32).cast();
    for id in [0xC8EB_D131, 0xA8EB_CF0C, 0x48EB_D0B1, 0x88ED_1059] {
        w.add(id, p);
    }
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.current = Some(77);
        s.key_type = Some(10);
    });
    assert_eq!(w.run(0xABCD_0001), 0xCAFE_BABE);
    let c = calls();
    let selected: Vec<_> = c
        .into_iter()
        .filter(|c| !matches!(c, Call::Property(_) | Call::Variant(_) | Call::Array(_)))
        .collect();
    assert_eq!(
        selected,
        [
            Call::Holder,
            Call::Excluded,
            Call::List(0x68, 0xABCD_0001),
            Call::Ensure(0x28EC_F510),
            Call::Signed(0x28EC_F510, 0),
            Call::Ensure(0x08FC_DB69),
            Call::Unsigned(0x08FC_DB69, 0),
            Call::List(0x6C, 0xABCD_0001),
            Call::Ensure(0xE8EC_F641),
            Call::Signed(0xE8EC_F641, 0),
            Call::Ensure(0xA8FC_DBA8),
            Call::Unsigned(0xA8FC_DBA8, 0),
            Call::List(0x70, 0xABCD_0001),
            Call::Ensure(0x88EC_F76C),
            Call::Signed(0x88EC_F76C, 0),
            Call::Ensure(0x28FC_DBB0),
            Call::Unsigned(0x28FC_DBB0, 0),
            Call::Current,
            Call::Ensure(0x08FD_1613),
            Call::Unsigned(0x08FD_1613, 77),
            Call::Clear(0x2781_2852, 0, 0),
            Call::List(0x74, 0xABCD_0001),
            Call::Key,
            Call::Register(true, 42),
            Call::Cost(-1000),
        ]
    );
    let sim = unsafe { ChangedView::from_ptr(w.sim.ptr()) };
    assert_eq!((sim.energy_capacity(), sim.recycling_count()), (500, 1));
    assert_eq!(w.run(0xABCD_0000), 0xCAFE_BABE);
    let c = calls();
    assert!(c.contains(&Call::Register(false, 42)));
    assert!(!c.iter().any(|c| matches!(c, Call::Ensure(_) | Call::Current)));
    assert_eq!((sim.energy_capacity(), sim.recycling_count()), (0, 0));
}

#[test]
fn energy_capacity_clamps_signed_wrap_and_budget_type_filters_registration() {
    let mut w = World::new();
    let p = (&mut *w.capacity as *mut i32).cast();
    w.add(0x48EB_D0B1, p);
    let sim = unsafe { ChangedView::from_ptr(w.sim.ptr()) };
    sim.set_energy_capacity(i32::MAX);
    w.run(1);
    assert_eq!(sim.energy_capacity(), 0);
    calls();
    sim.set_energy_capacity(10);
    w.run(0);
    assert_eq!(sim.energy_capacity(), 0);
    calls();
    STATE.with(|s| s.borrow_mut().key_type = Some(20));
    assert_eq!(w.run(1), 0x1122_3301);
    assert!(!calls().iter().any(|c| matches!(c, Call::Register(..) | Call::Cost(_))));
}
