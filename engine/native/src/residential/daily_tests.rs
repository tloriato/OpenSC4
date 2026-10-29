// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, fake_data, Fake, GameFpu, Object};
use std::cell::RefCell;
use std::collections::VecDeque;

#[derive(Debug, PartialEq, Eq)]
struct Call(&'static str, Vec<usize>);
fn c(name: &'static str, args: &[usize]) -> Call {
    Call(name, args.to_vec())
}
fn record(name: &'static str, args: &[usize]) {
    STATE.with(|s| s.borrow_mut().calls.push(c(name, args)));
}
fn take() -> Vec<Call> {
    STATE.with(|s| core::mem::take(&mut s.borrow_mut().calls))
}
fn names(calls: &[Call]) -> Vec<&'static str> {
    calls.iter().map(|c| c.0).collect()
}
fn id(this: *mut u8) -> usize {
    unsafe { this.add(4).cast::<usize>().read() }
}

#[derive(Default)]
struct State {
    calls: Vec<Call>,
    sim: usize,
    scratch: usize,
    temporary: usize,
    service: usize,
    framework: usize,
    nodes: VecDeque<usize>,
    counter: u32,
    step: u32,
    stopped: u8,
    expired: VecDeque<u32>,
    proximity: VecDeque<u32>,
    tracts: VecDeque<(u32, u32)>,
    scan_count: u32,
    scan_end_after_change: bool,
    budget: usize,
    change_step: Option<u32>,
    end_on_expiry: bool,
    grow_coverage: bool,
    local_auto: u8,
    ideal: u32,
    mutate_history_population: Option<i32>,
    resize_swap: Option<usize>,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }

extern "C" fn alloc(size: u32) -> *mut u8 {
    record("allocate", &[size as usize]);
    assert_eq!(size, 8);
    STATE.with(|s| s.borrow().scratch as *mut u8)
}
extern "C" fn dealloc(p: *mut u8) -> u32 {
    record("free", &[]);
    STATE.with(|s| assert_eq!(p as usize, s.borrow().scratch));
    assert_eq!(
        unsafe { core::slice::from_raw_parts(p, 8) },
        &[0, 0xA5, 0xA5, 0xA5, 0xA5, 0xA5, 0xA5, 0xA5]
    );
    0xFADE_BE80
}
extern "thiscall" fn strike(this: *mut u8, department: *mut u8, lists: *mut u8, staff: u32, grade: u32, group: u32) {
    record(
        "strike",
        &[
            (department as usize).wrapping_sub(this as usize),
            (lists as usize).wrapping_sub(this as usize),
            staff as usize,
            grade as usize,
            group as usize,
        ],
    );
    let sim = unsafe { DailySim::from_ptr(this) };
    assert_eq!((sim.school_sum(), sim.hospital_sum()), (0, 0));
}
extern "thiscall" fn effects(this: *mut u8) {
    let offset = STATE.with(|s| (this as usize).wrapping_sub(s.borrow().sim));
    record("effects", &[offset]);
}
extern "thiscall" fn ordinances(this: *mut u8, hospital: *mut u8, school: *mut u8) {
    record(
        "ordinances",
        &[(hospital as usize) - this as usize, (school as usize) - this as usize],
    );
}
extern "thiscall" fn node_alloc(this: *mut u8, count: u32, hint: u32) -> *mut u8 {
    record("node", &[this as usize, count as usize, hint as usize]);
    STATE.with(|s| s.borrow_mut().nodes.pop_front().expect("node allocation") as *mut u8)
}
extern "thiscall" fn clear(this: *mut u8) {
    let offset = STATE.with(|s| (this as usize) - s.borrow().sim);
    record("clear", &[offset]);
}
extern "C" fn get_framework() -> *mut u8 {
    record("framework", &[]);
    STATE.with(|s| s.borrow().framework as *mut u8)
}
extern "thiscall" fn factory(this: *mut u8, a: u32, b: u32, out: *mut *mut u8) {
    record("factory", &[id(this), a as usize, b as usize]);
    assert!(unsafe { *out }.is_null());
    unsafe { *out = STATE.with(|s| s.borrow().service as *mut u8) };
}
extern "thiscall" fn release_ref(this: *mut *mut u8) {
    record("release_reference", &[id(unsafe { *this })]);
}
extern "thiscall" fn service_alloc(this: *mut u8, size: u32) -> *mut u8 {
    record("service_allocate", &[id(this), size as usize]);
    STATE.with(|s| s.borrow_mut().nodes.pop_front().expect("service allocation") as *mut u8)
}
extern "C" fn grid_alloc(size: u32) -> *mut u8 {
    record("allocate_grid", &[size as usize]);
    STATE.with(|s| s.borrow().temporary as *mut u8)
}
extern "thiscall" fn new_grid(this: *mut u8, kind: u32) -> *mut u8 {
    record("grid_new", &[id(this), kind as usize]);
    this
}
extern "thiscall" fn init(this: *mut u8) {
    record("init", &[id(this)]);
}
extern "thiscall" fn add_ref(this: *mut u8) {
    record("add_ref", &[id(this)]);
}
extern "thiscall" fn shutdown(this: *mut u8) {
    record("shutdown", &[id(this)]);
}
extern "thiscall" fn release(this: *mut u8) {
    record("release", &[id(this)]);
}
extern "thiscall" fn size(this: *mut u8) -> u32 {
    record("size", &[id(this)]);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if let Some(replacement) = s.resize_swap.take() {
            unsafe {
                let temporary = DailySim::from_ptr(s.sim as *mut u8).temporary();
                let table = temporary.cast::<*mut usize>().read();
                table.add(0x24 / 4).write(resize_after_size as *const () as usize);
                temporary.cast::<usize>().write(replacement);
            }
        }
    });
    64
}
extern "thiscall" fn resize(this: *mut u8, size: u32) {
    record("resize", &[id(this), size as usize]);
}
extern "thiscall" fn resize_after_size(this: *mut u8, size: u32) {
    record("resize_after_size", &[id(this), size as usize]);
}
extern "thiscall" fn key(this: *mut u8, purpose: u32, wealth: u32) -> u32 {
    record("key", &[id(this), purpose as usize, wealth as usize]);
    700 + wealth
}
extern "thiscall" fn source(this: *mut u8) -> *mut u8 {
    record("source", &[id(this)]);
    0x1234usize as *mut u8
}
extern "C" fn start(destination: *mut u8, source: *mut u8, key: u32) {
    record("start", &[id(destination), source as usize, key as usize]);
}
extern "C" fn start_high(destination: *mut u8, source: *mut u8, key: u32) {
    record("start_high", &[id(destination), source as usize, key as usize]);
}
extern "thiscall" fn x(this: *mut u8) -> u32 {
    record("x", &[id(this)]);
    3
}
extern "thiscall" fn z(this: *mut u8) -> u32 {
    record("z", &[id(this)]);
    5
}
extern "thiscall" fn data(this: *mut u8) -> *mut u8 {
    record("data", &[id(this)]);
    0x4567usize as *mut u8
}
extern "C" fn proximity(data: *mut u8, x: u32, z: u32) -> u32 {
    record("proximity", &[data as usize, x as usize, z as usize]);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if let Some(step) = s.change_step.take() {
            unsafe {
                Budget::from_ptr(s.budget as *mut u8).set_step(step);
            }
        }
        s.proximity.pop_front().expect("proximity result")
    })
}
extern "C" fn copy(destination: *mut u8, source: *mut u8, callback: u32, context: u32) {
    record(
        "copy",
        &[id(destination), id(source), callback as usize, context as usize],
    );
}
extern "thiscall" fn new_budget(this: *mut u8, milliseconds: u32, pacing: *mut [u32; 2]) {
    let values = unsafe { *pacing };
    record(
        "budget",
        &[
            milliseconds as usize,
            pacing as usize,
            values[0] as usize,
            values[1] as usize,
        ],
    );
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.budget = this as usize;
        let b = unsafe { Budget::from_ptr(this) };
        b.set_counter(s.counter);
        b.set_step(s.step);
        b.set_stopped(s.stopped);
    });
}
extern "thiscall" fn expired(this: *mut u8) -> u32 {
    record("expired", &[unsafe { Budget::from_ptr(this) }.counter() as usize]);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if s.end_on_expiry {
            unsafe {
                *((s.sim as *mut u8).add(0x328)) = 2;
            }
        }
        s.expired.pop_front().unwrap_or(1)
    })
}
extern "thiscall" fn end_budget(this: *mut u8) {
    let b = unsafe { Budget::from_ptr(this) };
    record(
        "budget_end",
        &[b.counter() as usize, b.step() as usize, b.stopped() as usize],
    );
}
extern "thiscall" fn reset(this: *mut u8) {
    record("population_reset", &[]);
    assert_eq!(unsafe { DailySim::from_ptr(this) }.tuning_read(), 1);
}
extern "thiscall" fn tuning(this: *mut u8, out: *mut u8) {
    record("tuning", &[(out as usize) - this as usize]);
    unsafe {
        DailySim::from_ptr(this).set_tuning_read(1);
    }
}
extern "thiscall" fn next(this: *mut u8, x: *mut u32, z: *mut u32) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.sim + 0x328);
        if let Some((px, pz)) = s.tracts.pop_front() {
            unsafe {
                *x = px;
                *z = pz;
                *this = 0;
            }
            s.calls.push(c("next", &[px as usize, pz as usize]));
        } else {
            unsafe {
                *this = 2;
            }
            s.calls.push(c("scan_done", &[]));
        }
        s.scan_count += 1;
    });
}
extern "thiscall" fn change(this: *mut u8, tuning: *mut u8, x: u32, z: u32) {
    assert_eq!(tuning as usize, this as usize + 0x350);
    record("change", &[x as usize, z as usize]);
    STATE.with(|s| {
        if s.borrow().scan_end_after_change {
            unsafe {
                *this.add(0x328) = 2;
            }
        }
    });
}
extern "thiscall" fn reset_scan(this: *mut u8, x: u32, z: u32) {
    assert_eq!(this as usize, STATE.with(|s| s.borrow().sim + 0x328));
    record("reset_scan", &[x as usize, z as usize]);
}
extern "thiscall" fn post(this: *mut u8) {
    record("post", &[]);
    assert_eq!(unsafe { DailySim::from_ptr(this) }.state(), 12);
}
extern "thiscall" fn cover(
    this: *mut u8,
    maps: *mut u8,
    index: u32,
    list: *mut *mut u8,
    effects: *mut u8,
    school: u32,
) {
    let sim = unsafe { DailySim::from_ptr(this) };
    let list_base = unsafe { Lists::from_ptr(this.add(if school != 0 { 0x1A8 } else { 0x1B4 })) };
    assert_eq!(list, list_base.begin().wrapping_add(index as usize));
    record(
        "coverage",
        &[
            (maps as usize) - this as usize,
            index as usize,
            (effects as usize) - this as usize,
            school as usize,
        ],
    );
    let covered = if school != 0 {
        sim.school_population()
    } else {
        sim.hospital_population()
    };
    assert_eq!(covered, index as i32);
    if school != 0 {
        sim.set_school_population(covered + 1);
    } else {
        sim.set_hospital_population(covered + 1);
    }
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if s.grow_coverage {
            list_base.set_end(list_base.begin().wrapping_add(2));
            s.grow_coverage = false;
        }
    });
}
extern "thiscall" fn fill(this: *mut u8, value: u32) {
    record("fill", &[id(this), value as usize]);
}
extern "thiscall" fn holder(this: *mut u8) -> *mut u8 {
    record("holder", &[id(this)]);
    this
}
extern "C" fn property(this: *mut u8, property: u32, out: *mut u8) {
    record("bool", &[id(this), property as usize, unsafe { *out } as usize]);
    unsafe { *out = STATE.with(|s| s.borrow().local_auto) };
}
extern "thiscall" fn get_funding(_this: *mut u8, occupant: *mut u8, staff: *mut u32, vehicle: *mut u32, school: u32) {
    assert_eq!(unsafe { (*staff, *vehicle) }, (0, 0));
    record("get_funding", &[id(occupant), school as usize]);
    unsafe {
        *staff = 11;
        *vehicle = 22;
    }
}
extern "thiscall" fn ideal(_this: *mut u8, occupant: *mut u8, staff: *mut u32) -> u32 {
    record("ideal", &[id(occupant), unsafe { *staff } as usize]);
    unsafe {
        *staff = 33;
    }
    STATE.with(|s| s.borrow().ideal)
}
extern "thiscall" fn set_funding(_this: *mut u8, occupant: *mut u8, staff: *mut u32, vehicle: *mut u32, school: u32) {
    record(
        "set_funding",
        &[
            id(occupant),
            unsafe { *staff } as usize,
            unsafe { *vehicle } as usize,
            school as usize,
        ],
    );
}
#[allow(clippy::too_many_arguments)]
extern "thiscall" fn grade_building(
    this: *mut u8,
    occupant: *mut u8,
    coverage: *mut u8,
    curve: *mut u8,
    effect: *mut u8,
    grids: *mut u8,
    maximum: f32,
    school: u32,
    sum: *mut u8,
) {
    record(
        "grade",
        &[
            id(occupant),
            coverage as usize,
            curve as usize,
            id(effect),
            (grids as usize) - this as usize,
            maximum.to_bits() as usize,
            school as usize,
            (sum as usize) - this as usize,
        ],
    );
}
extern "thiscall" fn eq(_this: *mut u8) -> f32 {
    record("eq", &[]);
    -12.75
}
extern "thiscall" fn hq(_this: *mut u8) -> f32 {
    record("hq", &[]);
    23.875
}
extern "thiscall" fn write_history(this: *mut u8, series: u32, value: u32) {
    record("history", &[series as usize, value as usize]);
    if series == 0xAA1A_2CCA {
        STATE.with(|s| {
            if let Some(p) = s.borrow().mutate_history_population {
                unsafe {
                    DailySim::from_ptr(this).set_population(p);
                }
            }
        });
    }
}
extern "thiscall" fn group(this: *mut u8, group: u32) -> u32 {
    record("group", &[id(this), group as usize]);
    // Building one belongs to both school groups: the first wins.
    u32::from(match id(this) {
        1 => group == 1000 || group == 1001,
        2 => group == 1001,
        3 => group == 2001,
        _ => false,
    }) | 0xAB00
}

struct Globals {
    allocator: *mut u8,
    flags: u32,
    pacing: [[u32; 2]; 4],
    school: [u32; 14],
    hospital: [u32; 6],
    scale: f32,
    bias: f32,
}
// Each boxed object keeps its address when the owning vectors grow.
#[allow(clippy::vec_box)]
struct Fixture {
    sim: Object,
    scratch: Box<[u8; 8]>,
    table: Box<[usize; 60]>,
    service_table: Box<[usize; 4]>,
    objects: Vec<Box<[usize; 2]>>,
    lists: Vec<Box<[usize; 3]>>,
    vectors: Vec<Box<[*mut u8]>>,
    globals: Box<Globals>,
    _guards: Vec<Fake>,
    _fpu: GameFpu,
}
impl Fixture {
    fn new(state: u32) -> Self {
        STATE.with(|s| {
            *s.borrow_mut() = State {
                step: 1024,
                ideal: 0xAA01,
                ..State::default()
            }
        });
        let mut f = Self {
            sim: Object::new(0x3D0),
            scratch: Box::new([0xA5; 8]),
            table: Box::new([0; 60]),
            service_table: Box::new([0; 4]),
            objects: Vec::new(),
            lists: Vec::new(),
            vectors: Vec::new(),
            globals: Box::new(Globals {
                allocator: null_mut(),
                flags: 0,
                pacing: [[99, 77]; 4],
                school: core::array::from_fn(|i| 1000 + i as u32),
                hospital: core::array::from_fn(|i| 2000 + i as u32),
                scale: 1000.0,
                bias: 12582912.0,
            }),
            _guards: Vec::new(),
            _fpu: GameFpu::new(),
        };
        macro_rules! slot {
            ($offset:expr, $function:ident) => {
                f.table[$offset / 4] = $function as *const () as usize;
            };
        }
        slot!(4, add_ref);
        slot!(8, release);
        slot!(12, init);
        slot!(16, shutdown);
        slot!(20, factory);
        slot!(24, holder);
        slot!(0x24, resize);
        slot!(0x28, size);
        slot!(0x30, x);
        slot!(0x34, z);
        slot!(0x4C, set_funding);
        slot!(0x50, get_funding);
        slot!(0x58, group);
        slot!(0x60, data);
        slot!(0x6C, fill);
        slot!(0x80, eq);
        slot!(0x84, hq);
        slot!(0xA4, source);
        slot!(0xA8, key);
        slot!(0xE8, ideal);
        f.service_table[3] = service_alloc as *const () as usize;
        let p = f.sim.ptr();
        unsafe {
            p.cast::<*const usize>().write(f.table.as_ptr());
        }
        let sim = unsafe { DailySim::from_ptr(p) };
        sim.set_state(state);
        let proximity_grids = [f.object(100), f.object(101), f.object(102)];
        sim.set_proximity(proximity_grids);
        sim.set_lots(f.object(300));
        sim.set_school_effect(f.object(400));
        sim.set_hospital_effect(f.object(401));
        let temp = f.object(200);
        let framework = f.object(600);
        let mut service = Box::new([f.service_table.as_ptr() as usize, 500]);
        let service_ptr = service.as_mut_ptr() as usize;
        f.objects.push(service);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.sim = p as usize;
            s.scratch = f.scratch.as_mut_ptr() as usize;
            s.temporary = temp as usize;
            s.service = service_ptr;
            s.framework = framework as usize;
        });
        macro_rules! function {
            ($address:expr, $function:ident) => {
                f._guards.push(fake($address, $function as *const () as usize));
            };
        }
        function!(0x0090_CF54, alloc);
        function!(0x0090_CF63, dealloc);
        function!(0x006F_AFF0, strike);
        function!(0x006F_CB40, effects);
        function!(0x006F_E0F0, ordinances);
        function!(0x0041_3DD0, node_alloc);
        function!(0x0041_3E60, clear);
        function!(0x0087_93EC, get_framework);
        function!(0x0041_1870, release_ref);
        function!(0x005E_55E0, grid_alloc);
        function!(0x0045_DDB0, new_grid);
        function!(0x005E_4FA0, start);
        function!(0x005E_5130, start_high);
        function!(0x005E_4E80, proximity);
        function!(0x006F_8970, copy);
        function!(0x0068_CC20, new_budget);
        function!(0x0068_BF80, expired);
        function!(0x0068_B960, end_budget);
        function!(0x006F_6EE0, reset);
        function!(0x006F_C0C0, tuning);
        function!(0x0060_C8D0, next);
        function!(0x006F_A450, change);
        function!(0x0060_C860, reset_scan);
        function!(0x006F_8070, post);
        function!(0x006F_D0F0, cover);
        function!(0x005F_D390, property);
        function!(0x006F_D970, grade_building);
        function!(0x006F_7D00, write_history);
        fn data_pointer<T>(value: &mut T) -> *mut u8 {
            (value as *mut T).cast()
        }
        macro_rules! global {
            ($address:expr, $value:expr) => {
                f._guards.push(fake_data($address, data_pointer($value)));
            };
        }
        global!(0x00B2_07E4, &mut f.globals.allocator);
        global!(0x00B4_AFB8, &mut f.globals.flags);
        for (i, address) in [0x00B4_AFB0, 0x00B4_AFA8, 0x00B4_AFA0, 0x00B4_AF98]
            .into_iter()
            .enumerate()
        {
            global!(address, &mut f.globals.pacing[i]);
        }
        global!(0x00AB_2B60, &mut f.globals.school);
        global!(0x00AB_2B98, &mut f.globals.hospital);
        global!(0x00A8_FAF0, &mut f.globals.scale);
        global!(0x00AB_2BB0, &mut f.globals.bias);
        f
    }
    fn object(&mut self, id: usize) -> *mut u8 {
        let mut obj = Box::new([self.table.as_ptr() as usize, id]);
        let p = obj.as_mut_ptr().cast();
        self.objects.push(obj);
        p
    }
    fn sim(&mut self) -> DailySim {
        unsafe { DailySim::from_ptr(self.sim.ptr()) }
    }
    fn list(&mut self, occupants: &[*mut u8]) -> *mut u8 {
        let mut head = Box::new([0; 3]);
        let hp = head.as_mut_ptr().cast::<u8>();
        head[0] = hp as usize;
        head[1] = hp as usize;
        self.lists.push(head);
        for &occupant in occupants {
            let mut node = Box::new([0; 3]);
            let p = node.as_mut_ptr().cast();
            self.lists.push(node);
            unsafe {
                append(p, hp, occupant);
            }
        }
        hp
    }
    fn vector(&mut self, offset: usize, heads: &[*mut u8]) {
        let mut entries = heads.to_vec().into_boxed_slice();
        let view = unsafe { Lists::from_ptr(self.sim.ptr().add(offset)) };
        view.set_begin(entries.as_mut_ptr());
        view.set_end(entries.as_mut_ptr().wrapping_add(entries.len()));
        self.vectors.push(entries);
    }
    fn allocation(&mut self) {
        let mut node = Box::new([0xCDCD_CDCD; 3]);
        STATE.with(|s| s.borrow_mut().nodes.push_back(node.as_mut_ptr() as usize));
        self.lists.push(node);
    }
    fn run(&mut self) -> (bool, Vec<Call>) {
        *self.scratch = [0xA5; 8];
        let result = unsafe { fc4p_006ff520(self.sim.ptr(), 5) };
        assert_eq!(result & !255, 0xFADE_BE00);
        assert!(result as u8 <= 1);
        (result as u8 != 0, take())
    }
}

#[test]
fn idle_and_unknown_states_only_allocate_clear_one_byte_and_free() {
    for state in [10, 16, u32::MAX, 0x8000_0000] {
        let mut f = Fixture::new(state);
        let before = unsafe { core::slice::from_raw_parts(f.sim.ptr(), 0x3D0).to_vec() };
        assert_eq!(f.run(), (true, vec![c("allocate", &[8]), c("free", &[])]));
        assert_eq!(unsafe { core::slice::from_raw_parts(f.sim.ptr(), 0x3D0) }, before);
    }
}

#[test]
fn seed_all_wealths_constructs_once_and_preserves_call_order() {
    for wealth in 0..3 {
        for existing in [false, true] {
            let mut f = Fixture::new(13 + wealth);
            if existing {
                f.sim().set_temporary(STATE.with(|s| s.borrow().temporary as *mut u8));
            }
            let (done, calls) = f.run();
            assert!(done);
            assert_eq!(f.sim().state(), wealth + 1);
            let mut expected = vec![c("allocate", &[8])];
            if !existing {
                expected.extend([
                    c("allocate_grid", &[0x44]),
                    c("grid_new", &[200, 0x49B9_E600]),
                    c("init", &[200]),
                    c("add_ref", &[200]),
                ]);
            }
            expected.extend([
                c("size", &[100 + wealth as usize]),
                c("resize", &[200, 64]),
                c("key", &[300, 1, wealth as usize + 1]),
                c("source", &[300]),
                c(
                    if wealth == 2 { "start_high" } else { "start" },
                    &[200, 0x1234, 701 + wealth as usize],
                ),
                c("free", &[]),
            ]);
            assert_eq!(calls, expected);
        }
    }
}

#[test]
fn proximity_completes_without_deadline_check_and_releases_high_grid() {
    for state in 1..=3 {
        let mut f = Fixture::new(state);
        f.sim().set_temporary(STATE.with(|s| s.borrow().temporary as *mut u8));
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.proximity.push_back(0);
            s.stopped = 1;
        });
        let (done, calls) = f.run();
        assert!(done);
        assert_eq!(f.globals.flags, 1 << (state - 1));
        assert_eq!(f.globals.pacing[state as usize - 1], [1024, 0]);
        let mut expected = vec![
            c("allocate", &[8]),
            c(
                "budget",
                &[
                    5,
                    (&mut f.globals.pacing[state as usize - 1] as *mut [u32; 2]) as usize,
                    1024,
                    0,
                ],
            ),
            c("z", &[200]),
            c("x", &[200]),
            c("data", &[200]),
            c("proximity", &[0x4567, 3, 5]),
            c("copy", &[99 + state as usize, 200, 0x006F_6F80, 0]),
        ];
        if state == 3 {
            expected.extend([c("shutdown", &[200]), c("release", &[200])]);
            assert!(f.sim().temporary().is_null());
        }
        expected.extend([c("budget_end", &[0, 1024, 1]), c("free", &[])]);
        assert_eq!(calls, expected);
        assert_eq!(f.sim().state(), if state == 3 { 11 } else { state + 13 });
    }
}

#[test]
fn proximity_budget_resumes_and_only_low_wealth_reloads_the_step() {
    for state in 1..=3 {
        let mut f = Fixture::new(state);
        f.sim().set_temporary(STATE.with(|s| s.borrow().temporary as *mut u8));
        f.globals.flags = 0xFFFF_FFFF;
        f.globals.pacing[state as usize - 1] = [32, 9];
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.counter = u32::MAX - 15;
            s.step = 16;
            s.change_step = Some(32);
            s.proximity.extend([1, 0]);
            s.stopped = 1;
        });
        let (done, calls) = f.run();
        assert!(!done);
        assert_eq!(f.sim().state(), state);
        let expected_counter = if state == 1 { 16 } else { 0 };
        assert!(calls.contains(&c("budget_end", &[expected_counter, 32, 1])));
        assert_eq!(
            names(&calls),
            if state == 1 {
                vec![
                    "allocate",
                    "budget",
                    "z",
                    "x",
                    "data",
                    "proximity",
                    "budget_end",
                    "free",
                ]
            } else {
                vec![
                    "allocate",
                    "budget",
                    "z",
                    "x",
                    "data",
                    "proximity",
                    "expired",
                    "budget_end",
                    "free",
                ]
            }
        );
        assert_eq!(f.globals.pacing[state as usize - 1], [32, 9]);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.stopped = 0;
        });
        assert!(f.run().0);
    }
}

#[test]
fn proximity_deadline_uses_low_byte_and_accumulates_multiple_passes() {
    let mut f = Fixture::new(1);
    f.sim().set_temporary(STATE.with(|s| s.borrow().temporary as *mut u8));
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.step = 512;
        s.proximity.extend([1, 1, 1, 1]);
        s.expired.extend([0x100, 0xAB01]);
    });
    let (done, calls) = f.run();
    assert!(!done);
    assert_eq!(f.sim().state(), 1);
    assert_eq!(
        calls.iter().filter(|c| c.0 == "expired").collect::<Vec<_>>(),
        vec![&c("expired", &[1024]), &c("expired", &[2048])]
    );
    assert!(calls.contains(&c("budget_end", &[2048, 512, 1])));
}

#[test]
fn population_slice_resumes_at_next_tract_then_posts() {
    let mut f = Fixture::new(4);
    STATE.with(|s| s.borrow_mut().tracts.extend([(0, 0), (1, 0), (0, 1), (1, 1)]));
    for (x, z) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
        let (done, calls) = f.run();
        assert!(!done);
        assert_eq!(f.sim().state(), 4);
        assert!(calls.contains(&c("next", &[x, z])));
        assert!(calls.contains(&c("change", &[x, z])));
        assert_eq!(calls.iter().filter(|c| c.0 == "change").count(), 1);
        assert!(calls.contains(&c("budget_end", &[1024, 1024, 1])));
        assert_eq!(
            calls.iter().filter(|c| c.0 == "tuning").count(),
            usize::from(x == 0 && z == 0)
        );
    }
    let (done, calls) = f.run();
    assert!(done);
    assert_eq!(f.sim().state(), 12);
    assert_eq!(
        names(&calls),
        vec!["allocate", "budget", "scan_done", "budget_end", "free"]
    );
    assert_eq!(
        f.run(),
        (
            true,
            vec![
                c("allocate", &[8]),
                c("reset_scan", &[0, 0]),
                c("post", &[]),
                c("free", &[])
            ]
        )
    );
    assert_eq!(f.sim().state(), 5);
}

#[test]
fn population_completion_before_or_during_deadline_wins() {
    for finish_in_change in [false, true] {
        let mut f = Fixture::new(4);
        f.sim().set_tuning_read(1);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.tracts.push_back((7, 9));
            s.scan_end_after_change = finish_in_change;
            s.end_on_expiry = !finish_in_change;
        });
        let (done, calls) = f.run();
        assert!(done);
        assert_eq!(f.sim().state(), 12);
        assert_eq!(
            calls.iter().filter(|c| c.0 == "expired").count(),
            usize::from(!finish_in_change)
        );
    }
    let mut f = Fixture::new(11);
    f.sim().set_tuning_read(1);
    assert_eq!(
        f.run(),
        (
            true,
            vec![c("allocate", &[8]), c("population_reset", &[]), c("free", &[])]
        )
    );
    assert_eq!((f.sim().state(), f.sim().tuning_read()), (4, 0));
}

#[test]
fn coverage_clears_only_selected_count_and_reloads_vector_length() {
    for school in [false, true] {
        for grow in [false, true] {
            let mut f = Fixture::new(if school { 5 } else { 7 });
            let sim = f.sim();
            sim.set_school_population(99);
            sim.set_hospital_population(88);
            let a = f.list(&[]);
            let b = f.list(&[]);
            let offset = if school { 0x1A8 } else { 0x1B4 };
            f.vector(offset, &[a, b]);
            if grow {
                let v = unsafe { Lists::from_ptr(f.sim.ptr().add(offset)) };
                v.set_end(v.begin().wrapping_add(1));
                STATE.with(|s| s.borrow_mut().grow_coverage = true);
            }
            let (done, calls) = f.run();
            assert!(done);
            let (maps, effects) = if school { (0x58, 0x198) } else { (0x64, 0x194) };
            assert_eq!(
                calls,
                vec![
                    c("allocate", &[8]),
                    c("coverage", &[maps, 0, effects, usize::from(school)]),
                    c("coverage", &[maps, 1, effects, usize::from(school)]),
                    c("free", &[])
                ]
            );
            assert_eq!(
                (sim.school_population(), sim.hospital_population()),
                if school { (2, 88) } else { (99, 2) }
            );
            assert_eq!(sim.state(), if school { 6 } else { 8 });
        }
    }
}

#[test]
fn auto_budget_precedes_grading_for_both_service_types() {
    for school in [false, true] {
        for (global, local, ideal_result) in [(0, 0, 1), (0, 1, 1), (1, 0, 1), (1, 0, 0x100)] {
            let mut f = Fixture::new(if school { 6 } else { 8 });
            let sim = f.sim();
            if school {
                sim.set_school_auto(global);
            } else {
                sim.set_hospital_auto(global);
            }
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                s.local_auto = local;
                s.ideal = ideal_result;
            });
            let a = f.object(1);
            let b = f.object(2);
            let h = f.list(&[a, b]);
            let empty = f.list(&[]);
            f.vector(if school { 0x1A8 } else { 0x1B4 }, &[h, empty]);
            let mut maps = [0x111usize as *mut u8, 0x112usize as *mut u8];
            let mut curves = [0x221usize as *mut u8, 0x222usize as *mut u8];
            if school {
                sim.set_school_coverage(maps.as_mut_ptr());
                sim.set_school_curves(curves.as_mut_ptr());
            } else {
                sim.set_hospital_coverage(maps.as_mut_ptr());
                sim.set_hospital_curves(curves.as_mut_ptr());
            }
            let (done, calls) = f.run();
            assert!(done);
            assert_eq!(sim.state(), if school { 7 } else { 9 });
            let effect = if school { 400 } else { 401 };
            let flag = usize::from(school);
            let mut expected = vec![c("allocate", &[8]), c("fill", &[effect, 0])];
            for id in [1, 2] {
                if global == 0 {
                    expected.extend([c("holder", &[id]), c("bool", &[id, 0x6C05_FC4D, 0])]);
                }
                if global != 0 || local != 0 {
                    expected.extend([c("get_funding", &[id, flag]), c("ideal", &[id, 11])]);
                    if ideal_result as u8 != 0 {
                        expected.push(c("set_funding", &[id, 33, 22, flag]));
                    }
                }
                expected.push(c(
                    "grade",
                    &[
                        id,
                        0x111,
                        0x221,
                        effect,
                        if school { 0x7C } else { 0x88 },
                        200f32.to_bits() as usize,
                        flag,
                        if school { 0x314 } else { 0x318 },
                    ],
                ));
            }
            expected.push(c("free", &[]));
            assert_eq!(calls, expected);
        }
    }
}

#[test]
fn history_per_mille_rounds_ties_and_orders_empty_population_series() {
    for (population, school, hospital, expected_school, expected_hospital) in [
        (2000, 3, 1, 2, 0),
        (5000, 1234, 0, 247, 0),
        (2000, -3, -1, -2, 0),
        (1, 0, 1, 0, 1000),
        (0, 1234, 1, 0, 0),
        (-1, 4, 4, 0, 0),
    ] {
        let mut f = Fixture::new(9);
        let sim = f.sim();
        sim.set_population(population);
        sim.set_school_population(school);
        sim.set_hospital_population(hospital);
        let (done, calls) = f.run();
        assert!(done);
        assert_eq!(sim.state(), 10);
        let mut expected = vec![
            c("allocate", &[8]),
            c("eq", &[]),
            c("history", &[0xAA1A_2864, (-12i32) as u32 as usize]),
            c("hq", &[]),
            c("history", &[0xAA1A_2866, 23]),
            c("history", &[0xAA1A_2CCA, population as u32 as usize]),
        ];
        if population > 0 {
            expected.extend([
                c("history", &[0x4A8A_3FE5, expected_school as u32 as usize]),
                c("history", &[0x4A8A_3FE2, expected_hospital as u32 as usize]),
            ]);
        } else {
            expected.extend([c("history", &[0x4A8A_3FE2, 0]), c("history", &[0x4A8A_3FE5, 0])]);
        }
        expected.push(c("free", &[]));
        assert_eq!(calls, expected);
    }
}

#[test]
fn history_rereads_population_after_population_history_call() {
    let mut f = Fixture::new(9);
    let sim = f.sim();
    sim.set_population(2000);
    sim.set_school_population(3);
    sim.set_hospital_population(1);
    STATE.with(|s| s.borrow_mut().mutate_history_population = Some(0));
    let (_, calls) = f.run();
    assert!(calls.contains(&c("history", &[0xAA1A_2CCA, 2000])));
    assert_eq!(
        &calls[calls.len() - 3..],
        &[
            c("history", &[0x4A8A_3FE2, 0]),
            c("history", &[0x4A8A_3FE5, 0]),
            c("free", &[])
        ]
    );
}

#[test]
fn monthly_setup_resets_totals_distributes_first_match_and_appends_other() {
    let mut f = Fixture::new(0);
    let sim = f.sim();
    sim.set_school_sum(0x1234);
    sim.set_hospital_sum(0x5678);
    let a = f.object(1);
    let b = f.object(2);
    let hospital = f.object(3);
    let unmatched = f.object(4);
    let pending_school = f.list(&[a, b, unmatched]);
    let pending_hospital = f.list(&[hospital]);
    let pending_other = f.list(&[unmatched, a]);
    sim.set_new_schools(pending_school);
    sim.set_new_hospitals(pending_hospital);
    sim.set_new_other(pending_other);
    let school0 = f.list(&[]);
    let school1 = f.list(&[]);
    let hospital0 = f.list(&[]);
    let hospital1 = f.list(&[]);
    let other = f.list(&[hospital]);
    sim.set_other(other);
    f.vector(0x1A8, &[school0, school1]);
    f.vector(0x1B4, &[hospital0, hospital1]);
    for _ in 0..5 {
        f.allocation();
    }
    let school_slots = unsafe { Lists::from_ptr(f.sim.ptr().add(0x1A8)) }.begin();
    let hospital_slots = unsafe { Lists::from_ptr(f.sim.ptr().add(0x1B4)) }.begin();
    let (done, calls) = f.run();
    assert!(done);
    assert_eq!(sim.state(), 13);
    assert_eq!(
        calls,
        vec![
            c("allocate", &[8]),
            c("strike", &[0x134, 0x1A8, 0x2A97_6C20, 0x2924_53C5, 0x1A03]),
            c("strike", &[0xD4, 0x1B4, 0xAA97_6BE7, 0x0924_53CB, 0x1A02]),
            c("effects", &[0x198]),
            c("effects", &[0x194]),
            c("ordinances", &[0x194, 0x198]),
            c("group", &[1, 1000]),
            c("node", &[school_slots as usize, 1, 0]),
            c("group", &[2, 1000]),
            c("group", &[2, 1001]),
            c("node", &[school_slots.wrapping_add(1) as usize, 1, 0]),
            c("group", &[4, 1000]),
            c("group", &[4, 1001]),
            c("clear", &[0x19C]),
            c("group", &[3, 2000]),
            c("group", &[3, 2001]),
            c("node", &[hospital_slots.wrapping_add(1) as usize, 1, 0]),
            c("clear", &[0x1A0]),
            c("framework", &[]),
            c("factory", &[600, 0x3AE4_BEA3, 0x3AE4_BEAB]),
            c("release_reference", &[500]),
            c("service_allocate", &[500, 12]),
            c("service_allocate", &[500, 12]),
            c("clear", &[0x1A4]),
            c("free", &[])
        ]
    );
    assert_eq!(list_ids(school0), vec![1]);
    assert_eq!(list_ids(school1), vec![2]);
    assert!(list_ids(hospital0).is_empty());
    assert_eq!(list_ids(hospital1), vec![3]);
    assert_eq!(list_ids(other), vec![3, 4, 1]);
    assert_eq!(f.globals.allocator as usize, STATE.with(|s| s.borrow().service));
}

fn list_ids(head: *mut u8) -> Vec<usize> {
    unsafe {
        let mut result = Vec::new();
        let mut p = Node::from_ptr(head).next();
        let mut previous = head;
        while p != head {
            let n = Node::from_ptr(p);
            assert_eq!(n.previous(), previous);
            result.push(id(n.occupant()));
            previous = p;
            p = n.next();
        }
        assert_eq!(Node::from_ptr(head).previous(), previous);
        result
    }
}

#[test]
fn empty_lists_complete_coverage_and_grading() {
    for (state, next_state) in [(5, 6), (6, 7), (7, 8), (8, 9)] {
        let mut f = Fixture::new(state);
        f.sim().set_school_population(99);
        f.sim().set_hospital_population(88);
        let (done, calls) = f.run();
        assert!(done);
        assert_eq!(f.sim().state(), next_state);
        let expected = match state {
            6 => vec![c("allocate", &[8]), c("fill", &[400, 0]), c("free", &[])],
            8 => vec![c("allocate", &[8]), c("fill", &[401, 0]), c("free", &[])],
            _ => vec![c("allocate", &[8]), c("free", &[])],
        };
        assert_eq!(calls, expected);
        assert_eq!(f.sim().school_population(), if state == 5 { 0 } else { 99 });
        assert_eq!(f.sim().hospital_population(), if state == 7 { 0 } else { 88 });
    }
}

#[test]
fn month_advances_one_state_per_call_in_pipeline_order() {
    let mut f = Fixture::new(0);
    let schools = f.list(&[]);
    let hospitals = f.list(&[]);
    let other = f.list(&[]);
    f.sim().set_new_schools(schools);
    f.sim().set_new_hospitals(hospitals);
    f.sim().set_new_other(other);
    for expected in [13, 1, 14, 2, 15, 3, 11, 4, 12, 5, 6, 7, 8, 9, 10] {
        STATE.with(|s| s.borrow_mut().proximity.push_back(0));
        if f.sim().state() == 11 {
            f.sim().set_tuning_read(1);
        }
        let (done, _) = f.run();
        assert!(done);
        assert_eq!(f.sim().state(), expected);
    }
    assert_eq!(f.globals.flags, 15);
}

#[test]
fn monthly_setup_reuses_allocator_and_retains_list_order() {
    let mut f = Fixture::new(0);
    let a = f.object(4);
    let empty = f.list(&[]);
    let pending = f.list(&[a]);
    let destination = f.list(&[]);
    f.sim().set_new_schools(empty);
    f.sim().set_new_hospitals(empty);
    f.sim().set_new_other(pending);
    f.sim().set_other(destination);
    f.globals.allocator = STATE.with(|s| s.borrow().service as *mut u8);
    f.allocation();
    let (_, calls) = f.run();
    assert_eq!(
        names(&calls),
        vec![
            "allocate",
            "strike",
            "strike",
            "effects",
            "effects",
            "ordinances",
            "clear",
            "clear",
            "service_allocate",
            "clear",
            "free"
        ]
    );
    assert_eq!(list_ids(destination), vec![4]);
}

// An integer that cannot be represented in a single-precision return value.
static EXTENDED_EQ: F80 = F80::from_bytes([0, 0, 0, 0, 0x80, 0, 0, 0x80, 0x17, 0x40]);
#[unsafe(naked)]
unsafe extern "thiscall" fn extended_eq(_this: *mut u8) -> f32 {
    core::arch::naked_asm!("fld tbyte ptr [{value}]", "ret", value = sym EXTENDED_EQ);
}

#[test]
fn history_consumes_unrounded_st0() {
    let mut f = Fixture::new(9);
    f.table[0x80 / 4] = extended_eq as *const () as usize;
    let (_, calls) = f.run();
    assert_eq!(calls[1], c("history", &[0xAA1A_2864, 16_777_217]));
}

#[test]
fn seed_retains_destination_table_but_reads_resize_slot_after_size() {
    let mut f = Fixture::new(13);
    f.sim().set_temporary(STATE.with(|s| s.borrow().temporary as *mut u8));
    let replacement = f.table.clone();
    STATE.with(|s| s.borrow_mut().resize_swap = Some(replacement.as_ptr() as usize));
    let (done, calls) = f.run();
    assert!(done);
    assert!(calls.contains(&c("resize_after_size", &[200, 64])));
    assert!(!calls.iter().any(|call| call.0 == "resize"));
}
