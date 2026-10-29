// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, fake_data, Fake, GameFpu, Object};
use std::cell::RefCell;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Call {
    Allocate(u32),
    Free(usize),
    CurveNew(usize),
    Holder,
    ReadFloat(u32),
    ReadUint(u32, u32),
    WriteFloat(u32, u32),
    WriteUint(u32, u32),
    Efficiency(u32, u32),
    Base(u32),
    PropertyCurves(u32),
    ClearEffects,
    Select(u32, usize),
    Modifiers,
    Geometry(u32),
    Covered(u32, u32, u32, u32),
    Served(u32, u32, u32),
    Coverage(u32, u32),
    GradeRead(u32, u32),
    GradeWrite(u32, u32, u32),
    Lot,
    Position(bool),
    Transform(u32, u32),
    EffectCurves(u32),
    CopyCurve(usize),
    CurveOutput(u32),
    RadiusEffect(usize, usize, u32, u32, u32, u32, u32, u32, u32),
    ClearCurves,
}

struct State {
    calls: Vec<Call>,
    buffers: Vec<Box<[u32]>>,
    curves: Vec<usize>,
    radius: Option<f32>,
    capacity: Option<u32>,
    boost: Option<f32>,
    staff: f32,
    vehicle: f32,
    base: f32,
    modifiers: [f32; 3],
    served: u32,
    tracts: Vec<(u32, u32, u32, f32)>,
    grades: Vec<(u32, u32, u32)>,
    sim: usize,
    occupant: usize,
    holder: usize,
    coverage: usize,
    grade_grid: usize,
    lot: usize,
    wealth: [f32; 3],
    expected_school: u32,
    extended_grade: Option<f64>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            calls: Vec::new(),
            buffers: Vec::new(),
            curves: Vec::new(),
            radius: Some(10.0),
            capacity: Some(1000),
            boost: None,
            staff: 1.0,
            vehicle: 1.0,
            base: 1.0,
            modifiers: [1.0; 3],
            served: 2000,
            tracts: vec![(2, 3, 1, 0.0)],
            grades: Vec::new(),
            sim: 0,
            occupant: 0,
            holder: 0,
            coverage: 0,
            grade_grid: 0,
            lot: 0,
            wealth: [100.0; 3],
            expected_school: 0,
            extended_grade: None,
        }
    }
}
thread_local! { static STATE:RefCell<State> = RefCell::new(State::default()); }
fn log(call: Call) {
    STATE.with(|s| s.borrow_mut().calls.push(call));
}
fn calls() -> Vec<Call> {
    STATE.with(|s| std::mem::take(&mut s.borrow_mut().calls))
}
fn storage(size: u32) -> *mut u8 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let mut block = vec![0xCCCC_CCCC; size.div_ceil(4) as usize].into_boxed_slice();
        let ptr = block.as_mut_ptr().cast();
        s.buffers.push(block);
        ptr
    })
}
extern "C" fn allocate_fake(size: u32) -> *mut u8 {
    log(Call::Allocate(size));
    storage(size)
}
extern "C" fn free_fake(ptr: *mut u8) -> u32 {
    log(Call::Free(ptr as usize));
    assert!(STATE.with(|s| s.borrow().buffers.iter().any(|b| b.as_ptr().cast::<u8>() == ptr)));
    0x1234_5678
}
extern "thiscall" fn curve_new_fake(this: *mut u8) -> u32 {
    let index = STATE.with(|s| {
        let mut s = s.borrow_mut();
        let i = s.curves.len();
        s.curves.push(this as usize);
        i
    });
    log(Call::CurveNew(index));
    unsafe {
        this.cast::<[u32; 4]>().write([0; 4]);
    }
    this as u32
}
extern "thiscall" fn holder_fake(this: *mut u8) -> *mut u8 {
    log(Call::Holder);
    STATE.with(|s| {
        let s = s.borrow();
        assert_eq!(this as usize, s.occupant);
        s.holder as *mut u8
    })
}
extern "C" fn read_float_fake(holder: *mut u8, id: u32, out: *mut f32) -> u32 {
    log(Call::ReadFloat(id));
    STATE.with(|s| {
        let s = s.borrow();
        assert_eq!(holder as usize, s.holder);
        let value = if matches!(id, 0x691B_42B3 | 0x091B_42A2) {
            s.radius
        } else {
            s.boost
        };
        if let Some(value) = value {
            unsafe {
                out.write(value);
            }
            0x1234_5601
        } else {
            0x1234_5600
        }
    })
}
extern "C" fn read_uint_fake(holder: *mut u8, id: u32, out: *mut u32) -> u32 {
    log(Call::ReadUint(id, unsafe { *out }));
    STATE.with(|s| {
        let s = s.borrow();
        assert_eq!(holder as usize, s.holder);
        if let Some(value) = s.capacity {
            unsafe {
                out.write(value);
            }
            1
        } else {
            0
        }
    })
}
extern "C" fn write_float_fake(holder: *mut u8, id: u32, value: f32) -> u32 {
    assert_eq!(holder as usize, STATE.with(|s| s.borrow().holder));
    log(Call::WriteFloat(id, value.to_bits()));
    0
}
extern "C" fn write_uint_fake(holder: *mut u8, id: u32, value: u32) -> u32 {
    assert_eq!(holder as usize, STATE.with(|s| s.borrow().holder));
    log(Call::WriteUint(id, value));
    0
}
extern "thiscall" fn efficiency_fake(
    this: *mut u8,
    occupant: *mut u8,
    staff: *mut f32,
    vehicle: *mut f32,
    school: u32,
    ignore: u32,
) {
    log(Call::Efficiency(school, ignore));
    STATE.with(|s| {
        let s = s.borrow();
        assert_eq!((this as usize, occupant as usize), (s.sim, s.occupant));
        assert_eq!(school, s.expected_school);
        unsafe {
            assert_eq!(*vehicle, 0.0);
            *staff = s.staff;
            *vehicle = s.vehicle;
        }
    });
}
extern "thiscall" fn base_fake(_this: *mut u8, out: *mut f32, _occupant: *mut u8, school: u32) -> *mut f32 {
    log(Call::Base(school));
    unsafe {
        *out = STATE.with(|s| s.borrow().base);
    }
    out
}
extern "thiscall" fn property_curves_fake(
    _this: *mut u8,
    holder: *mut u8,
    age: *mut u8,
    wealth: *mut u8,
    distance: *mut u8,
    school: u32,
) {
    log(Call::PropertyCurves(school));
    STATE.with(|s| {
        let s = s.borrow();
        assert_eq!(holder as usize, s.holder);
        assert_eq!([age as usize, wealth as usize, distance as usize], s.curves.as_slice());
    });
    for curve in [age, wealth, distance] {
        unsafe {
            Curve::from_ptr(curve).set_storage(storage(16));
        }
    }
}
extern "thiscall" fn clear_effects_fake(list: *mut *mut u8) -> u32 {
    log(Call::ClearEffects);
    unsafe {
        let head = *list;
        assert_eq!(Node::from_ptr(head).next(), head);
        assert_eq!(Node::from_ptr(head).previous(), head);
    }
    0
}
extern "thiscall" fn select_fake(
    _this: *mut u8,
    _occupant: *mut u8,
    kind: u32,
    source: *mut *mut u8,
    _out: *mut *mut u8,
) -> u32 {
    let offset = source as usize - STATE.with(|s| s.borrow().sim);
    log(Call::Select(kind, offset));
    0
}
extern "thiscall" fn modifiers_fake(
    _this: *mut u8,
    _occupant: *mut u8,
    _list: *mut *mut u8,
    radius: *mut f32,
    capacity: *mut f32,
    strength: *mut f32,
) -> u32 {
    log(Call::Modifiers);
    let m = STATE.with(|s| s.borrow().modifiers);
    unsafe {
        *radius = m[0];
        *capacity = m[1];
        *strength = m[2];
    }
    0
}
extern "thiscall" fn width_fake(_this: *mut u8) -> u32 {
    log(Call::Geometry(0x30));
    7
}
extern "thiscall" fn height_fake(_this: *mut u8) -> u32 {
    log(Call::Geometry(0x34));
    9
}
unsafe extern "thiscall" fn scale_fake(_this: *mut u8) {
    log(Call::Geometry(0x38));
    unsafe {
        core::arch::asm!("fld1", options(nostack));
    }
}
extern "thiscall" fn covered_fake(
    _this: *mut u8,
    _occupant: *mut u8,
    radius: f32,
    scale: f32,
    x: u32,
    z: u32,
    list: *mut *mut u8,
) -> u32 {
    log(Call::Covered(radius.to_bits(), scale.to_bits(), x, z));
    let tracts = STATE.with(|s| s.borrow().tracts.clone());
    unsafe {
        let head = *list;
        for (x, z, _, _) in tracts {
            let node = storage(16);
            let tail = Node::from_ptr(head).previous();
            let n = Node::from_ptr(node);
            n.set_next(head);
            n.set_previous(tail);
            n.set_x(x);
            n.set_z(z);
            Node::from_ptr(tail).set_next(node);
            Node::from_ptr(head).set_previous(node);
        }
    }
    0
}
#[allow(clippy::too_many_arguments)]
extern "thiscall" fn served_fake(
    _this: *mut u8,
    _occupant: *mut u8,
    radius: f32,
    capacity: u32,
    age: *mut u8,
    wealth: *mut u8,
    distance: *mut u8,
    _effects: *mut *mut u8,
    coverage: *mut u8,
    chunk: u32,
    _tracts: *mut *mut u8,
    unused: *mut u8,
) -> u32 {
    log(Call::Served(radius.to_bits(), capacity, chunk));
    STATE.with(|s| {
        let s = s.borrow();
        assert_eq!([age as usize, wealth as usize, distance as usize], s.curves.as_slice());
        assert_eq!((coverage as usize, unused as usize), (s.coverage, s.grade_grid));
        s.served
    })
}
extern "thiscall" fn coverage_fake(this: *mut u8, x: u32, z: u32) -> u32 {
    log(Call::Coverage(x, z));
    STATE.with(|s| {
        let s = s.borrow();
        assert_eq!(this as usize, s.coverage);
        s.tracts.iter().find(|t| t.0 == x && t.1 == z).unwrap().2
    })
}
unsafe extern "thiscall" fn grade_value(this: *mut u8, x: u32, z: u32) {
    log(Call::GradeRead(x, z));
    let value = STATE.with(|s| {
        let s = s.borrow();
        assert_eq!(this as usize, s.grade_grid);
        s.tracts.iter().find(|t| t.0 == x && t.1 == z).unwrap().3
    });
    let extended = STATE.with(|s| s.borrow().extended_grade);
    if let Some(extended) = extended {
        unsafe {
            core::arch::asm!("fld qword ptr [{value}]",value=in(reg)&extended,options(nostack));
        }
    } else {
        unsafe {
            core::arch::asm!("fld dword ptr [{value}]",value=in(reg)&value,options(nostack));
        }
    }
}
extern "thiscall" fn grade_write_fake(this: *mut u8, x: u32, z: u32, value: f32) -> u32 {
    assert_eq!(this as usize, STATE.with(|s| s.borrow().grade_grid));
    log(Call::GradeWrite(x, z, value.to_bits()));
    STATE.with(|s| s.borrow_mut().grades.push((x, z, value.to_bits())));
    0
}
extern "thiscall" fn lot_fake(_this: *mut u8, occupant: *mut u8) -> *mut u8 {
    log(Call::Lot);
    STATE.with(|s| {
        let s = s.borrow();
        assert_eq!(occupant as usize, s.occupant);
        s.lot as *mut u8
    })
}
extern "thiscall" fn position_fake(this: *mut u8, out: *mut f32) -> u32 {
    log(Call::Position(this as usize == STATE.with(|s| s.borrow().lot)));
    unsafe {
        out.cast::<[f32; 3]>().write([32.0, 80.0, -48.0]);
    }
    0
}
extern "thiscall" fn transform_fake(_this: *mut u8, x: f32, z: f32, first: *mut u32, second: *mut u32) -> u32 {
    log(Call::Transform(x.to_bits(), z.to_bits()));
    unsafe {
        *first = 99;
        *second = 88;
    }
    0
}
extern "thiscall" fn effect_curves_fake(
    _this: *mut u8,
    _occupant: *mut u8,
    kind: u32,
    _effects: *mut *mut u8,
    first: *mut *mut u8,
    second: *mut *mut u8,
) -> u32 {
    log(Call::EffectCurves(kind));
    unsafe {
        for list in [first, second] {
            let head = *list;
            assert_eq!(Node::from_ptr(head).next(), head);
        }
    }
    0
}
extern "thiscall" fn copy_curve_fake(this: *mut u8, source: *mut u8) -> u32 {
    let index = STATE.with(|s| s.borrow().curves.iter().position(|&c| c == source as usize).unwrap());
    log(Call::CopyCurve(index));
    unsafe {
        this.cast::<[u32; 4]>().write([0; 4]);
    }
    0
}
unsafe extern "thiscall" fn curve_output_fake(this: *mut u8, input: f32) {
    assert_eq!(this as usize, STATE.with(|s| s.borrow().curves[1]));
    log(Call::CurveOutput(input.to_bits()));
    let index = F80::from_f32(input).ftol() as usize;
    let output = STATE.with(|s| s.borrow().wealth[index]);
    unsafe {
        core::arch::asm!("fld dword ptr [{value}]",value=in(reg)&output,options(nostack));
    }
}
#[allow(clippy::too_many_arguments)]
extern "C" fn radius_effect_fake(
    age: *mut u8,
    output: *mut u8,
    age_curves: *mut *mut u8,
    x: f32,
    z: f32,
    radius: f32,
    distance_curves: *mut *mut u8,
    intensity: f32,
    context: u32,
    units: u32,
    coverage: *mut u8,
    population: *mut u8,
    extra: u32,
) -> u32 {
    log(Call::RadiusEffect(
        age as usize,
        output as usize,
        x.to_bits(),
        z.to_bits(),
        radius.to_bits(),
        intensity.to_bits(),
        context,
        units,
        extra,
    ));
    STATE.with(|s| {
        let s = s.borrow();
        assert_eq!(coverage as usize, s.coverage);
        assert_ne!(population as usize, 0);
    });
    unsafe {
        for list in [age_curves, distance_curves] {
            let head = *list;
            let node = Node::from_ptr(head).next();
            assert_ne!(head, node);
            assert_eq!(Node::from_ptr(node).next(), head);
        }
    }
    0
}
extern "thiscall" fn clear_curves_fake(list: *mut *mut u8) -> u32 {
    log(Call::ClearCurves);
    unsafe {
        let head = *list;
        let mut node = Node::from_ptr(head).next();
        while node != head {
            let next = Node::from_ptr(node).next();
            free_fake(node);
            node = next;
        }
        Node::from_ptr(head).set_next(head);
        Node::from_ptr(head).set_previous(head);
    }
    0
}

// The fake globals keep stable addresses while these vectors grow.
#[allow(clippy::vec_box)]
struct World {
    _guards: Vec<Fake>,
    _tables: Vec<Box<[usize]>>,
    _objects: Vec<Object>,
    _globals: Vec<Box<*mut u8>>,
    _floats: Vec<Box<f32>>,
    sim: Object,
    occupant: *mut u8,
    coverage: *mut u8,
    grade: *mut u8,
    boost: [*mut u8; 3],
}

impl World {
    fn new() -> Self {
        STATE.with(|s| *s.borrow_mut() = State::default());
        let mut world = Self {
            _guards: Vec::new(),
            _tables: Vec::new(),
            _objects: Vec::new(),
            _globals: Vec::new(),
            _floats: Vec::new(),
            sim: Object::new(0x400),
            occupant: core::ptr::null_mut(),
            coverage: core::ptr::null_mut(),
            grade: core::ptr::null_mut(),
            boost: [0x1010 as *mut u8, 0x2020 as *mut u8, 0x3030 as *mut u8],
        };
        let holder = world.object(&[]);
        world.occupant = world.object(&[
            (0x18, holder_fake as *const () as usize),
            (0x20, position_fake as *const () as usize),
        ]);
        world.coverage = world.object(&[(0x58, coverage_fake as *const () as usize)]);
        world.grade = world.object(&[
            (0x58, grade_value as *const () as usize),
            (0x68, grade_write_fake as *const () as usize),
        ]);
        let population = world.object(&[
            (0x30, width_fake as *const () as usize),
            (0x34, height_fake as *const () as usize),
            (0x38, scale_fake as *const () as usize),
        ]);
        let lots = world.object(&[(0x38, lot_fake as *const () as usize)]);
        let terrain = world.object(&[(0x17C, transform_fake as *const () as usize)]);
        world.data_pointer(0x00B4_3D08, lots);
        world.data_pointer(0x00B4_3CEC, terrain);
        for (address, value) in [
            (0x00A8_0AA8, 4294967296.0),
            (0x00B0_D440, 1.0),
            (0x00B4_AF90, 0.0),
            (0x00A8_1054, 0.0),
            (0x00A8_1228, 1.0),
            (0x00A8_23A4, 0.01),
        ] {
            world.data_float(address, value);
        }
        for (address, function) in [
            (0x0090_CF54, allocate_fake as *const () as usize),
            (0x0090_CF63, free_fake as *const () as usize),
            (0x007D_0410, curve_new_fake as *const () as usize),
            (0x005F_D4F0, read_float_fake as *const () as usize),
            (0x005F_D480, read_uint_fake as *const () as usize),
            (0x005F_E3C0, write_uint_fake as *const () as usize),
            (0x005F_E4F0, write_float_fake as *const () as usize),
            (0x006F_8710, efficiency_fake as *const () as usize),
            (0x006F_85F0, base_fake as *const () as usize),
            (0x006F_C400, property_curves_fake as *const () as usize),
            (0x006F_CB40, clear_effects_fake as *const () as usize),
            (0x006F_D070, select_fake as *const () as usize),
            (0x006F_9340, modifiers_fake as *const () as usize),
            (0x006F_7E00, covered_fake as *const () as usize),
            (0x006F_8F60, served_fake as *const () as usize),
            (0x006F_CD70, effect_curves_fake as *const () as usize),
            (0x005F_4430, copy_curve_fake as *const () as usize),
            (0x005F_4250, curve_output_fake as *const () as usize),
            (0x005E_48F0, radius_effect_fake as *const () as usize),
            (0x006F_CBD0, clear_curves_fake as *const () as usize),
        ] {
            world._guards.push(fake(address, function));
        }
        unsafe {
            let sim = GradeSim::from_ptr(world.sim.ptr());
            sim.set_population_low(population);
            sim.set_age(0x4040 as *mut u8);
            sim.set_slop(1.5);
            world.sim.ptr().add(0x4C).cast::<u32>().write(0x5050);
            world.sim.ptr().add(0x50).cast::<u32>().write(0x6060);
        }
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.sim = world.sim.ptr() as usize;
            s.occupant = world.occupant as usize;
            s.holder = holder as usize;
            s.coverage = world.coverage as usize;
            s.grade_grid = world.grade as usize;
        });
        world
    }
    fn object(&mut self, slots: &[(usize, usize)]) -> *mut u8 {
        let mut table = vec![0usize; 0x180 / 4].into_boxed_slice();
        for &(offset, f) in slots {
            table[offset / 4] = f;
        }
        let mut object = Object::new(32);
        unsafe {
            object.ptr().cast::<*const usize>().write(table.as_ptr());
        }
        let pointer = object.ptr();
        self._tables.push(table);
        self._objects.push(object);
        pointer
    }
    fn data_pointer(&mut self, address: u32, value: *mut u8) {
        let mut data = Box::new(value);
        self._guards
            .push(fake_data(address, (&mut *data as *mut *mut u8).cast()));
        self._globals.push(data);
    }
    fn data_float(&mut self, address: u32, value: f32) {
        let mut data = Box::new(value);
        self._guards.push(fake_data(address, (&mut *data as *mut f32).cast()));
        self._floats.push(data);
    }
    fn run(&mut self, school: u32, chunk: u32) -> u32 {
        STATE.with(|s| s.borrow_mut().expected_school = school);
        let before = unsafe { std::slice::from_raw_parts(self.sim.ptr(), 0x400) }.to_vec();
        let result = unsafe {
            fc4p_006fd970(
                self.sim.ptr(),
                self.occupant,
                self.coverage,
                chunk,
                self.grade,
                self.boost.as_mut_ptr(),
                0x1234_5678,
                school,
                0x90AB_CDEF,
            )
        };
        assert_eq!(unsafe { std::slice::from_raw_parts(self.sim.ptr(), 0x400) }, before);
        result
    }
}

#[test]
fn missing_radius_writes_only_service_results_and_cleans_up() {
    let _fpu = GameFpu::new();
    for (school, radius, served, capacity, grade) in [
        (0, 0x091B_42A2, 0xC922_015C, 0x4924_4B21, 0x0924_53CB),
        (0xFFFF_0001, 0x691B_42B3, 0x0922_0161, 0xE924_4B2E, 0x2924_53C5),
        (0x100, 0x091B_42A2, 0xC922_015C, 0x4924_4B21, 0x0924_53CB),
    ] {
        let mut world = World::new();
        STATE.with(|s| s.borrow_mut().radius = None);
        assert_eq!(world.run(school, 0), 0x1234_5678);
        let all = calls();
        let head = STATE.with(|s| s.borrow().buffers[0].as_ptr() as usize);
        assert_eq!(
            all,
            [
                Call::Allocate(0x3C),
                Call::CurveNew(0),
                Call::CurveNew(1),
                Call::CurveNew(2),
                Call::Holder,
                Call::Holder,
                Call::ReadFloat(radius),
                Call::WriteUint(served, 0),
                Call::WriteUint(capacity, 0),
                Call::WriteFloat(grade, 0),
                Call::ClearEffects,
                Call::Free(head)
            ]
        );
    }
}

/// The core grading case: 1000 effective places / 2000 served = grade 0.5, with one contribution per wealth.
#[test]
fn grading_calls_dependencies_in_order_and_applies_all_wealth_effects() {
    let _fpu = GameFpu::new();
    let mut world = World::new();
    assert_eq!(world.run(1, 1000), 0x1234_5678);
    let all = calls();
    assert_eq!(
        &all[..21],
        [
            Call::Allocate(0x3C),
            Call::CurveNew(0),
            Call::CurveNew(1),
            Call::CurveNew(2),
            Call::Holder,
            Call::Holder,
            Call::ReadFloat(0x691B_42B3),
            Call::Efficiency(1, 0),
            Call::Base(1),
            Call::PropertyCurves(1),
            Call::ClearEffects,
            Call::Select(0xA92D_8E40, 0x198),
            Call::Modifiers,
            Call::Allocate(0x10),
            Call::Geometry(0x34),
            Call::Geometry(0x30),
            Call::Geometry(0x38),
            Call::Covered(10f32.to_bits(), 1f32.to_bits(), 7, 9),
            Call::ReadUint(0x8922_041B, 0x691B_42B3),
            Call::Served(10f32.to_bits(), 1000, 1000),
            Call::WriteFloat(0x2A97_6C20, 0.5f32.to_bits())
        ]
    );
    assert!(all.contains(&Call::GradeWrite(2, 3, 0.5f32.to_bits())));
    assert!(all.contains(&Call::WriteUint(0x0922_0161, 2000)));
    assert!(all.contains(&Call::WriteUint(0xE924_4B2E, 1000)));
    assert!(all.contains(&Call::WriteFloat(0x2924_53C5, 0.5f32.to_bits())));
    let boost: Vec<_> = all
        .iter()
        .filter(|c| matches!(c, Call::RadiusEffect(..)))
        .cloned()
        .collect();
    assert_eq!(
        boost,
        world
            .boost
            .iter()
            .map(|&output| Call::RadiusEffect(
                0x4040,
                output as usize,
                32f32.to_bits(),
                (-48f32).to_bits(),
                15f32.to_bits(),
                0.5f32.to_bits(),
                0x1234_5678,
                1,
                0x90AB_CDEF
            ))
            .collect::<Vec<_>>()
    );
    let copy: Vec<_> = all
        .iter()
        .filter(|c| matches!(c, Call::CopyCurve(..)))
        .cloned()
        .collect();
    assert_eq!(copy, [Call::CopyCurve(0), Call::CopyCurve(2)]);
    assert_eq!(all.iter().filter(|c| matches!(c, Call::ClearCurves)).count(), 2);
    let allocations = STATE.with(|s| {
        s.borrow()
            .buffers
            .iter()
            .map(|b| b.as_ptr() as usize)
            .collect::<Vec<_>>()
    });
    let tail = &all[all.iter().position(|c| *c == Call::ClearCurves).unwrap()..];
    assert_eq!(
        tail,
        [
            Call::ClearCurves,
            Call::Free(allocations[8]),
            Call::Free(allocations[7]),
            Call::ClearCurves,
            Call::Free(allocations[9]),
            Call::Free(allocations[6]),
            Call::Free(allocations[5]),
            Call::Free(allocations[4]),
            Call::WriteUint(0x0922_0161, 2000),
            Call::WriteUint(0xE924_4B2E, 1000),
            Call::WriteFloat(0x2924_53C5, 0.5f32.to_bits()),
            Call::Free(allocations[3]),
            Call::Free(allocations[2]),
            Call::Free(allocations[1]),
            Call::ClearEffects,
            Call::Free(allocations[0]),
        ]
    );
}

#[test]
fn grade_callback_stays_unrounded_until_its_multiply() {
    let _fpu = GameFpu::new();
    let mut world = World::new();
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.radius = Some(0.0);
        s.capacity = Some(0);
        s.served = 1;
        s.tracts = vec![(0, 0, 4, 0.0)];
        s.extended_grade = Some(0.25 + 3.0 / 134217728.0);
    });
    world.run(0, 1);
    assert_eq!(STATE.with(|s| s.borrow().grades.clone()), [(0, 0, 0x3F40_0001)]);
}

fn environment() -> (u16, u16, u32, u32) {
    let (mut cw, mut mxcsr) = (0u16, 0u32);
    let (sw, flags): (u16, u32);
    unsafe {
        core::arch::asm!("fnstcw word ptr [{cw}]", "fnstsw ax", "stmxcsr dword ptr [{mxcsr}]",
            "pushfd", "pop {flags:e}", cw=in(reg)&mut cw, mxcsr=in(reg)&mut mxcsr,
            flags=lateout(reg) flags, out("ax") sw);
    }
    (cw, sw & 0x3800, mxcsr, flags & 0x400)
}

#[test]
fn grading_preserves_existing_x87_values_control_word_mxcsr_and_direction() {
    let _fpu = GameFpu::new();
    let mut world = World::new();
    let sentinel = 123.25f64;
    let mut restored = 0f64;
    unsafe {
        core::arch::asm!("fld qword ptr [{p}]",p=in(reg)&sentinel,out("st(0)") _);
    }
    let before = environment();
    world.run(1, 1000);
    let after = environment();
    unsafe {
        core::arch::asm!("fstp qword ptr [{p}]",p=in(reg)&mut restored,out("st(0)") _);
    }
    assert_eq!(restored, sentinel);
    assert_eq!(after, before);
}

#[test]
fn coverage_blend_uses_low_byte_old_value_times_count_minus_one_and_nan() {
    let _fpu = GameFpu::new();
    let mut world = World::new();
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.radius = Some(0.0);
        s.capacity = Some(100);
        s.served = 200;
        s.tracts = vec![
            (0, 0, 0x100, 0.0),
            (1, 0, 1, 0.9),
            (2, 0, 2, 0.1),
            (3, 0, 3, 0.2),
            (4, 0, 4, 0.5),
        ];
    });
    world.run(0, 100);
    let all = calls();
    let grades = STATE.with(|s| s.borrow().grades.clone());
    // A positive contribution divided by zero clamps to one.
    assert_eq!(grades[0].2, 1f32.to_bits());
    assert_eq!(
        &grades[1..],
        &[
            (1, 0, 0.5f32.to_bits()),
            (2, 0, 0.35f32.to_bits()),
            (3, 0, (0.4f32 + 1f32 / 6f32).to_bits()),
            (4, 0, 1f32.to_bits())
        ]
    );
    assert_eq!(all.iter().filter(|c| matches!(c, Call::GradeRead(..))).count(), 3);
    assert!(!all.iter().any(|c| matches!(c, Call::RadiusEffect(..))));
    let mut world = World::new();
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.radius = Some(0.0);
        s.capacity = Some(0);
        s.tracts = vec![(0, 0, 0, 0.0)];
    });
    world.run(0, 1);
    assert!(STATE.with(|s| f32::from_bits(s.borrow().grades[0].2).is_nan()));
}

#[test]
fn capacity_ratios_modifiers_missing_capacity_and_empty_tracts() {
    let _fpu = GameFpu::new();
    for (capacity, served, expected) in [
        (Some(0), 0, 0.0f32),
        (Some(1000), 0, 1.0),
        (Some(1000), 2000, 0.5),
        (Some(5000), 2000, 1.0),
        (None, 0, 1.0),
    ] {
        let mut world = World::new();
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.radius = Some(-1.0);
            s.capacity = capacity;
            s.served = served;
            s.tracts.clear();
        });
        world.run(0, 100);
        let all = calls();
        assert!(all.contains(&Call::WriteFloat(0x0924_53CB, expected.to_bits())));
        assert!(all.contains(&Call::Served((-1f32).to_bits(), capacity.unwrap_or(0x091B_42A2), 100)));
    }
    let mut world = World::new();
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.staff = 0.25;
        s.base = 0.5;
        s.vehicle = 0.5;
        s.modifiers = [2.0, 2.0, 1.1];
        s.boost = Some(0.5);
        s.tracts.clear();
    });
    world.run(1, 1000);
    let all = calls();
    assert!(all.contains(&Call::WriteFloat(0x2A97_6C20, 0.5f32.to_bits())));
    assert!(all.contains(&Call::WriteUint(0xE924_4B2E, 500)));
    assert!(all.contains(&Call::WriteFloat(0x2924_53C5, 0.25f32.to_bits())));
    let intensity = all
        .iter()
        .find_map(|c| {
            if let Call::RadiusEffect(_, _, _, _, _, bits, _, _, _) = c {
                Some(*bits)
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(intensity, 0.1375f32.to_bits());
}

#[test]
fn unsigned_capacity_ftol_and_service_flags_are_preserved() {
    let _fpu = GameFpu::new();
    for (capacity, staff, expected) in [
        (u32::MAX, 1.0, 0),
        (0x8000_0001, 1.0, 0x8000_0000),
        (1000, f32::NAN, 0),
        (1000, f32::INFINITY, 0),
        (1000, -0.5, (-500i32) as u32),
        (1001, 0.5, 500),
    ] {
        let mut world = World::new();
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.capacity = Some(capacity);
            s.staff = staff;
            s.radius = Some(0.0);
            s.tracts.clear();
        });
        world.run(0x100, 1);
        let all = calls();
        assert!(
            all.contains(&Call::WriteUint(0x4924_4B21, expected)),
            "capacity {capacity}, staff {staff}: {all:?}"
        );
        assert!(all.contains(&Call::Efficiency(0x100, 0)));
        assert!(all.contains(&Call::Select(0x292D_8F9B, 0x194)));
    }
}

#[test]
fn radius_branches_and_lot_centre_keep_the_hospital_curve_filter() {
    let _fpu = GameFpu::new();
    for (radius, boosts) in [
        (0.0, false),
        (-1.0, false),
        (f32::NAN, false),
        (f32::INFINITY, true),
        (1.0, true),
    ] {
        let mut world = World::new();
        let lot = world.object(&[(0x3C, position_fake as *const () as usize)]);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.radius = Some(radius);
            s.tracts.clear();
            s.lot = lot as usize;
        });
        world.run(0, 1000);
        let all = calls();
        assert!(all.contains(&Call::Position(true)));
        assert!(all.contains(&Call::Transform(32f32.to_bits(), (-48f32).to_bits())));
        assert_eq!(all.contains(&Call::EffectCurves(0xA92D_8E40)), boosts);
        assert_eq!(
            all.iter().filter(|c| matches!(c, Call::RadiusEffect(..))).count(),
            if boosts { 3 } else { 0 }
        );
    }
}
