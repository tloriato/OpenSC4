// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Coverage geometry and service-selection vectors from the core's occupant_coverage_geometry test.

use super::*;
use crate::testing::{fake_data, GameFpu};

#[derive(Default)]
struct Config {
    school: bool,
    hospital: bool,
    radius: Option<f32>,
    vehicle: VecDeque<Option<f32>>,
    effects: VecDeque<Vec<f32>>,
    points: Vec<(f32, f32)>,
    modifier: f32,
    curve_storage: Vec<usize>,
    point_storage: usize,
    list: usize,
}
thread_local! {
    static CONFIG: RefCell<Config> = RefCell::new(Config::default());
}
fn game(name: &'static str, args: &[u32]) {
    log(Call::Game(name, args.to_vec()));
}
fn owned(size: usize) -> *mut u8 {
    STATE.with(|s| {
        let mut object = Object::new(size);
        let p = object.ptr();
        s.borrow_mut().allocations.push(object);
        p
    })
}
unsafe extern "thiscall" fn efficiency_fake(
    this: *mut u8,
    occupant: *mut u8,
    staff: *mut f32,
    vehicle: *mut f32,
    school: u32,
    ignore: u32,
) -> u32 {
    unsafe {
        game(
            "efficiency",
            &[this as u32, occupant as u32, school, ignore, vehicle.read().to_bits()],
        );
        staff.write(0.75);
        if let Some(value) = CONFIG.with(|c| c.borrow_mut().vehicle.pop_front().unwrap_or(Some(1.0))) {
            vehicle.write(value);
        }
    }
    0
}
unsafe extern "thiscall" fn select_fake(
    this: *mut u8,
    occupant: *mut u8,
    kind: u32,
    source: *mut *mut u8,
    out: *mut *mut u8,
) -> u32 {
    unsafe {
        game(
            "select",
            &[this as u32, occupant as u32, kind, source as u32, out as u32],
        );
        let head = *out;
        CONFIG.with(|c| c.borrow_mut().list = head as usize);
        let effects = CONFIG.with(|c| c.borrow_mut().effects.pop_front().unwrap_or_default());
        for value in effects {
            let n = owned(0x3C);
            n.add(8).cast::<f32>().write(value);
            let tail = Link::from_ptr(head).previous();
            Link::from_ptr(n).set_next(head);
            Link::from_ptr(n).set_previous(tail);
            Link::from_ptr(tail).set_next(n);
            Link::from_ptr(head).set_previous(n);
        }
    }
    0
}
unsafe extern "thiscall" fn mark_fake(
    this: *mut u8,
    grids: *mut *mut *mut u8,
    index: u32,
    occupant: *mut u8,
    radius_id: u32,
    multiplier: f32,
    capacity_id: u32,
    chunk: u32,
    served: *mut u32,
) -> u32 {
    game(
        "mark",
        &[
            this as u32,
            grids as u32,
            index,
            occupant as u32,
            radius_id,
            multiplier.to_bits(),
            capacity_id,
            chunk,
            served as u32,
        ],
    );
    unsafe {
        served.write(served.read().wrapping_add(13));
    }
    0
}
extern "thiscall" fn fill(grid: *mut u8, value: u32) -> u32 {
    game("fill", &[grid as u32, value]);
    0
}
unsafe extern "C" fn uint_fake(holder: *mut u8, id: u32, out: *mut u32) -> u64 {
    game("uint", &[holder as u32, id]);
    let exists = CONFIG.with(|c| {
        if id == 0x8922_041B {
            c.borrow().school
        } else {
            assert_eq!(id, 0x6922_0415);
            c.borrow().hospital
        }
    });
    if exists {
        unsafe { out.write(1000) };
    }
    0xCAFE_BABE_ABCD_1200 | u64::from(exists)
}
unsafe extern "C" fn float_fake(holder: *mut u8, id: u32, out: *mut f32) -> u32 {
    game("float", &[holder as u32, id, unsafe { out.read() }.to_bits()]);
    let value = CONFIG.with(|c| c.borrow().radius);
    if let Some(value) = value {
        unsafe { out.write(value) };
    }
    u32::from(value.is_some())
}
unsafe extern "thiscall" fn new_curve(this: *mut u8) -> u32 {
    game("new", &[this as u32]);
    unsafe { this.write_bytes(0, 16) };
    this as u32
}
unsafe extern "thiscall" fn curves_fake(
    this: *mut u8,
    holder: *mut u8,
    a: *mut u8,
    b: *mut u8,
    c: *mut u8,
    school: u32,
) {
    game(
        "curves",
        &[this as u32, holder as u32, a as u32, b as u32, c as u32, school],
    );
    for curve in [a, b, c] {
        let storage = owned(16);
        unsafe { curve.cast::<*mut u8>().write(storage) };
        CONFIG.with(|c| c.borrow_mut().curve_storage.push(storage as usize));
    }
}
unsafe extern "thiscall" fn modifiers_fake(
    this: *mut u8,
    occupant: *mut u8,
    list: *mut *mut u8,
    radius: *mut f32,
    capacity: *mut u32,
    effect: *mut u32,
) -> u32 {
    unsafe {
        game(
            "modifiers",
            &[
                this as u32,
                occupant as u32,
                list as u32,
                radius.read().to_bits(),
                capacity.read(),
                effect.read(),
            ],
        );
        radius.write(CONFIG.with(|c| c.borrow().modifier));
        capacity.write(2);
        effect.write(3);
    }
    0
}
unsafe extern "thiscall" fn points_fake(curve: *mut u8, vector: *mut u8) -> u32 {
    game("points", &[curve as u32]);
    let points = CONFIG.with(|c| c.borrow().points.clone());
    let storage = if points.is_empty() {
        core::ptr::null_mut()
    } else {
        owned(points.len() * 8).cast::<f32>()
    };
    unsafe {
        for (i, &(x, y)) in points.iter().enumerate() {
            storage.add(2 * i).write(x);
            storage.add(2 * i + 1).write(y);
        }
        let vector = Points::from_ptr(vector);
        assert!(vector.begin().is_null());
        assert!(vector.end().is_null());
        assert!(vector.capacity().is_null());
        vector.set_begin(storage);
        vector.set_end(storage.wrapping_add(points.len() * 2));
        vector.set_capacity(storage.wrapping_add(points.len() * 2));
    }
    CONFIG.with(|c| c.borrow_mut().point_storage = storage as usize);
    0
}
extern "C" fn free_fake(storage: *mut u8) -> u64 {
    game("free", &[storage as u32]);
    0xC123_4567_89AB_CDEF
}
unsafe extern "thiscall" fn holder_fake(building: *mut u8) -> *mut u8 {
    game("holder", &[building as u32]);
    unsafe { building.add(4).cast::<*mut u8>().read() }
}

fn setup_coverage() -> Vec<Fake> {
    let mut fakes = setup();
    CONFIG.with(|c| {
        *c.borrow_mut() = Config {
            school: true,
            radius: Some(200.0),
            modifier: 1.0,
            ..Config::default()
        }
    });
    fakes.extend([
        fake(0x006F_8710, efficiency_fake as *const () as usize),
        fake(0x006F_D070, select_fake as *const () as usize),
        fake(0x006F_9BF0, mark_fake as *const () as usize),
        fake(0x005F_D480, uint_fake as *const () as usize),
        fake(0x005F_D4F0, float_fake as *const () as usize),
        fake(0x007D_0410, new_curve as *const () as usize),
        fake(0x006F_C400, curves_fake as *const () as usize),
        fake(0x006F_9340, modifiers_fake as *const () as usize),
        fake(0x005F_4760, points_fake as *const () as usize),
        fake(0x0090_CF63, free_fake as *const () as usize),
    ]);
    fakes
}
fn names(calls: &[Call]) -> Vec<&'static str> {
    calls
        .iter()
        .map(|c| match c {
            Call::ClearEffects(_) => "clear",
            Call::Allocate(_) => "allocate",
            Call::Game(name, _) => name,
            _ => panic!("unexpected {c:?}"),
        })
        .collect()
}
fn arguments<'a>(calls: &'a [Call], name: &str) -> Vec<&'a [u32]> {
    calls
        .iter()
        .filter_map(|c| match c {
            Call::Game(n, a) if *n == name => Some(a.as_slice()),
            _ => None,
        })
        .collect()
}

#[test]
fn coverage_updates_select_service_and_multiply_factors_before_efficiency() {
    let _fpu = GameFpu::new();
    for school in [0x1234_5601u32, 0x1234_5600] {
        let _fakes = setup_coverage();
        let mut one = 1.0f32;
        let _one = fake_data(0x00A8_1228, (&mut one as *mut f32).cast());
        let mut sim = Object::new(0x3C4);
        let s = sim.ptr();
        let mut chunks = [123u32, 500];
        let mut other_chunks = [456u32, 700];
        let mut lists = List::new(&[0, 0]);
        let buildings = lists.nodes();
        unsafe {
            buildings[0].add(8).cast::<u32>().write(0xABC0);
            buildings[1].add(8).cast::<u32>().write(0xABC4);
        }
        let mut vt = [0usize; 28];
        vt[0x6C / 4] = fill as *const () as usize;
        let mut grid = vt.as_ptr();
        let g = (&mut grid as *mut *const usize).cast::<u8>();
        let mut grids_array = [g, g];
        let mut grids = grids_array.as_mut_ptr();
        let grids_ptr = &mut grids;
        let sim_view = unsafe { OccupantSim::from_ptr(s) };
        sim_view.set_school_chunks(chunks.as_mut_ptr());
        sim_view.set_hospital_chunks(other_chunks.as_mut_ptr());
        sim_view.set_school_covered(3);
        sim_view.set_hospital_covered(4);
        CONFIG.with(|c| {
            let mut c = c.borrow_mut();
            c.vehicle.extend([Some(0.25), None]);
            c.effects.extend([vec![2.0, 0.5], vec![]]);
        });
        let (result, words) = unsafe {
            invoke(
                fc4p_006fd0f0 as *const () as usize,
                s,
                &[grids_ptr as *mut _ as u32, 1, lists.ptr() as u32, 0xCAFE_BABE, school],
            )
        };
        let head = CONFIG.with(|c| c.borrow().list as u32);
        assert_eq!(words, [grids_ptr as *mut _ as u32, 1, head, 0xCAFE_BABE, school]);
        assert_eq!(result as u32, 0x89AB_CDEF);
        let calls = calls();
        assert_eq!(
            names(&calls),
            [
                "fill",
                "allocate",
                "efficiency",
                "clear",
                "select",
                "mark",
                "efficiency",
                "clear",
                "select",
                "mark",
                "clear",
                "free"
            ]
        );
        assert_eq!(arguments(&calls, "fill"), [&[g as u32, 0][..]]);
        let efficiency = arguments(&calls, "efficiency");
        assert_eq!(
            efficiency,
            [
                &[s as u32, 0xABC0, school, 0, 0][..],
                &[s as u32, 0xABC4, school, 0, 0.25f32.to_bits()][..]
            ]
        );
        let is_school = school as u8 != 0;
        for (i, entry) in arguments(&calls, "select").iter().enumerate() {
            assert_eq!(
                &entry[..4],
                &[
                    s as u32,
                    0xABC0 + i as u32 * 4,
                    if is_school { 0xA92D_8E40 } else { 0x292D_8F9B },
                    s as u32 + if is_school { 0x198 } else { 0x194 }
                ]
            );
        }
        for (i, entry) in arguments(&calls, "mark").iter().enumerate() {
            assert_eq!(
                *entry,
                &[
                    s as u32,
                    grids_ptr as *mut _ as u32,
                    1,
                    0xABC0 + i as u32 * 4,
                    if is_school { 0x691B_42B3 } else { 0x091B_42A2 },
                    0.25f32.to_bits(),
                    if is_school { 0x8922_041B } else { 0x6922_0415 },
                    if is_school { 500 } else { 700 },
                    s as u32 + if is_school { 0xAC } else { 0xB0 }
                ]
            );
        }
        assert_eq!(
            (sim_view.school_covered(), sim_view.hospital_covered()),
            if is_school { (29, 4) } else { (3, 30) }
        );
        assert_eq!(arguments(&calls, "free"), [&[head][..]]);
    }
}

#[test]
fn empty_coverage_list_still_clears_the_grid_and_allocates_and_frees_its_effect_head() {
    let _fakes = setup_coverage();
    let mut sim = Object::new(0x3C4);
    let s = sim.ptr();
    let mut chunk = 500u32;
    unsafe { OccupantSim::from_ptr(s) }.set_school_chunks(&mut chunk);
    let mut list = List::new(&[]);
    let mut vt = [0usize; 28];
    vt[27] = fill as *const () as usize;
    let mut grid = vt.as_ptr();
    let mut pointer = (&mut grid as *mut *const usize).cast::<u8>();
    let mut array = &mut pointer as *mut *mut u8;
    let (_, words) = unsafe {
        invoke(
            fc4p_006fd0f0 as *const () as usize,
            s,
            &[&mut array as *mut _ as u32, 0, list.ptr() as u32, 42, 1],
        )
    };
    let calls = calls();
    assert_eq!(names(&calls), ["fill", "allocate", "clear", "free"]);
    assert_eq!(arguments(&calls, "free"), [&[words[2]][..]]);
}

struct Query {
    sim: Object,
    building: Object,
    _holder: Object,
    _vt: Box<[usize; 7]>,
    _fakes: Vec<Fake>,
    _fpu: GameFpu,
    _globals: Box<[u32; 3]>,
}
impl Query {
    fn new(points: &[(f32, f32)]) -> Self {
        let mut fakes = setup_coverage();
        CONFIG.with(|c| c.borrow_mut().points = points.to_vec());
        let mut globals = Box::new([0, 100.0f32.to_bits(), 0.01f32.to_bits()]);
        for (i, a) in [0x00B4_AF90, 0x00A8_0AA0, 0x00A8_23A4].into_iter().enumerate() {
            fakes.push(fake_data(a, (&mut globals[i] as *mut u32).cast()));
        }
        let mut vt = Box::new([0usize; 7]);
        vt[6] = holder_fake as *const () as usize;
        let mut building = Object::new(8);
        let mut holder = Object::new(4);
        unsafe {
            building.ptr().cast::<*const usize>().write(vt.as_ptr());
            building.ptr().add(4).cast::<*mut u8>().write(holder.ptr());
        }
        Self {
            sim: Object::new(0x3C4),
            building,
            _holder: holder,
            _vt: vt,
            _fakes: fakes,
            _fpu: GameFpu::new(),
            _globals: globals,
        }
    }
    fn run(&mut self, level: *mut f32, distance: *mut f32, radius: *mut f32) -> (u64, Vec<u32>) {
        unsafe {
            invoke(
                fc4p_006fd260 as *const () as usize,
                self.sim.ptr(),
                &[self.building.ptr() as u32, level as u32, distance as u32, radius as u32],
            )
        }
    }
}

#[test]
fn occupant_coverage_geometry_preserves_the_unscaled_previous_x_term() {
    for (input, want) in [(0.5, 10037.5), (0.9, 50.0), (0.0, 10100.0)] {
        let mut q = Query::new(&[(0.0, 100.0), (50.0, 80.0), (100.0, 0.0)]);
        let (mut level, mut distance, mut radius) = (input, -123.0, -456.0);
        let (result, words) = q.run(&mut level, &mut distance, &mut radius);
        assert_eq!((distance, radius), (want, 200.0));
        assert_eq!(result, 0xC123_4567_89AB_CD01);
        let list = CONFIG.with(|c| c.borrow().list as u32);
        assert_eq!(
            words,
            [
                list,
                &mut level as *mut _ as u32,
                &mut distance as *mut _ as u32,
                &mut radius as *mut _ as u32
            ]
        );
        let calls = calls();
        assert_eq!(
            names(&calls),
            [
                "holder",
                "uint",
                "uint",
                "allocate",
                "new",
                "new",
                "new",
                "float",
                "efficiency",
                "curves",
                "clear",
                "select",
                "modifiers",
                "points",
                "free",
                "free",
                "free",
                "free",
                "clear",
                "free"
            ]
        );
        let news = arguments(&calls, "new");
        let curve = arguments(&calls, "curves")[0];
        assert_eq!(&curve[2..5], &[news[0][0], news[1][0], news[2][0]]);
        assert_eq!(curve[5], 0xDEAD_BE01);
        assert_eq!(arguments(&calls, "points"), [&[news[2][0]][..]]);
        CONFIG.with(|c| {
            let c = c.borrow();
            let expected = [
                c.point_storage as u32,
                c.curve_storage[2] as u32,
                c.curve_storage[1] as u32,
                c.curve_storage[0] as u32,
                c.list as u32,
            ];
            assert_eq!(
                arguments(&calls, "free").iter().map(|a| a[0]).collect::<Vec<_>>(),
                expected
            );
            unsafe {
                assert_eq!(*(c.point_storage as *const f32), if input == 0.9 { 0.0 } else { 50.0 });
            }
        });
    }
}

#[test]
fn coverage_uses_hospital_radius_and_effects_while_retaining_the_school_flag() {
    for school in [true, false] {
        let mut q = Query::new(&[(0.0, 100.0), (50.0, 80.0), (100.0, 0.0)]);
        CONFIG.with(|c| {
            let mut c = c.borrow_mut();
            c.hospital = true;
            c.school = school;
            c.radius = Some(100.0);
            c.modifier = 0.5;
            c.vehicle.push_back(Some(0.5));
        });
        let (mut level, mut distance, mut radius) = (0.5, 9.0, 8.0);
        q.run(&mut level, &mut distance, &mut radius);
        assert_eq!((distance, radius), (1254.6875, 25.0));
        let calls = calls();
        let holder = q._holder.ptr() as u32;
        assert_eq!(arguments(&calls, "float"), [&[holder, 0x091B_42A2, 0][..]]);
        assert_eq!(arguments(&calls, "efficiency")[0][2], 0xDEAD_BE00 | u32::from(school));
        let select = arguments(&calls, "select")[0];
        assert_eq!(
            &select[..4],
            &[
                q.sim.ptr() as u32,
                q.building.ptr() as u32,
                0x292D_8F9B,
                q.sim.ptr() as u32 + 0x194
            ]
        );
    }
}

#[test]
fn a_nonservice_building_preserves_outputs_the_occupant_argument_and_property_registers() {
    let mut q = Query::new(&[]);
    CONFIG.with(|c| c.borrow_mut().school = false);
    let (mut level, mut distance, mut radius) = (0.5, 12.0, 34.0);
    let (result, words) = q.run(&mut level, &mut distance, &mut radius);
    assert_eq!(result, 0xCAFE_BABE_ABCD_1200);
    assert_eq!((level, distance, radius), (0.5, 12.0, 34.0));
    assert_eq!(words[0], q.building.ptr() as u32);
    assert_eq!(names(&calls()), ["holder", "uint", "uint"]);
}

#[test]
fn empty_and_single_point_curves_have_different_partial_writes() {
    for points in [&[][..], &[(0.0, 100.0)][..]] {
        let mut q = Query::new(points);
        let (mut level, mut distance, mut radius) = (0.5, 123.0, 456.0);
        q.run(&mut level, &mut distance, &mut radius);
        assert_eq!(radius, 200.0);
        assert_eq!(distance, if points.is_empty() { 123.0 } else { 0.0 });
        let calls = calls();
        assert_eq!(arguments(&calls, "free").len(), if points.is_empty() { 4 } else { 5 });
    }
}

#[test]
fn missing_radius_and_unwritten_efficiency_keep_the_initialized_defaults() {
    for missing_radius in [true, false] {
        let mut q = Query::new(&[(0.0, 100.0), (100.0, 0.0)]);
        CONFIG.with(|c| {
            let mut c = c.borrow_mut();
            if missing_radius {
                c.radius = None;
            } else {
                c.vehicle.push_back(None);
            }
        });
        let (mut level, mut distance, mut radius) = (0.5, 1.0, 2.0);
        q.run(&mut level, &mut distance, &mut radius);
        assert_eq!((distance, radius), (0.0, 0.0));
    }
}

#[test]
fn unordered_levels_flat_segments_and_unreached_levels_follow_the_float_branches() {
    for (points, input, is_nan) in [
        (vec![(0.0, 100.0), (100.0, 0.0)], f32::NAN, true),
        (vec![(0.0, 100.0), (100.0, 100.0)], 1.0, true),
        (vec![(0.0, 100.0), (100.0, 0.0)], -1.0, false),
    ] {
        let mut q = Query::new(&points);
        let (mut level, mut distance, mut radius) = (input, 123.0, 456.0);
        q.run(&mut level, &mut distance, &mut radius);
        assert_eq!(distance.is_nan(), is_nan);
        if !is_nan {
            assert_eq!(distance, 0.0);
        }
    }
}

#[test]
fn output_aliases_are_observed_before_level_reads() {
    // Initializing distance overwrites the requested level before scanning.
    let mut q = Query::new(&[(0.0, 100.0), (100.0, 0.0)]);
    let (mut level, mut radius) = (0.9f32, 0.0);
    let ptr = &mut level as *mut f32;
    q.run(ptr, ptr, &mut radius);
    assert_eq!((level, radius), (200.0, 200.0));
    // Writing radius changes the level before distance is initialized.
    let mut q = Query::new(&[(0.0, 100.0), (100.0, 0.0)]);
    let (mut level, mut distance) = (0.9f32, 0.0);
    let ptr = &mut level as *mut f32;
    q.run(ptr, &mut distance, ptr);
    assert_eq!((level, distance), (200.0, -39800.0));
}
