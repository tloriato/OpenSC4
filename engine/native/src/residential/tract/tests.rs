// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Tract vectors moved from the core residential tests, with calls at the original object layout.

use super::*;
use crate::testing::{fake, fake_data, Fake, GameFpu, Object};
use std::cell::RefCell;
use std::collections::VecDeque;

#[derive(Clone, Debug, PartialEq)]
enum Call {
    String,
    Chars,
    Developer(usize),
    Size(usize),
    Pollution(u32, i32, i32, i32),
    Curve(usize, f32),
    Read(usize, i32, i32),
    Write(usize, i32, i32, u32),
    Random([u32; 4]),
    Create,
    Extent(usize, bool),
    Altitude(f32, f32),
    Transform([f32; 3]),
    Start(u32),
    Release,
    Month,
    Life(i32, i32),
    Free,
}

struct State {
    calls: Vec<Call>,
    tuning: usize,
    sim: usize,
    draws: VecDeque<f64>,
    samples: [u32; 3],
    curves: [f64; 3],
    month: u32,
    life: u32,
    effect: usize,
    create: bool,
    swap: Option<(usize, usize, usize)>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            calls: Vec::new(),
            tuning: 0,
            sim: 0,
            draws: VecDeque::new(),
            samples: [0; 3],
            curves: [0.0; 3],
            month: 11,
            life: 100,
            effect: 0,
            create: true,
            swap: None,
        }
    }
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

fn log(call: Call) {
    STATE.with(|state| state.borrow_mut().calls.push(call));
}
fn take() -> Vec<Call> {
    STATE.with(|state| std::mem::take(&mut state.borrow_mut().calls))
}
fn state(f: impl FnOnce(&mut State)) {
    STATE.with(|state| f(&mut state.borrow_mut()));
}

#[repr(C)]
struct Grid {
    table: *const usize,
    id: usize,
    integer: u32,
    float: f64,
    size: i32,
    width: f64,
    depth: f64,
    map: *mut u8,
}

unsafe extern "thiscall" fn string_range(this: *mut u8, begin: *const u8, end: *const u8, _tag: *const u8) {
    assert_eq!(end as usize - begin as usize, 8);
    assert_eq!(unsafe { std::slice::from_raw_parts(begin, 8) }, b"buzzards");
    assert_eq!(unsafe { std::slice::from_raw_parts(this.cast::<u32>(), 3) }, [0; 3]);
    let bytes = Box::into_raw(Box::new(*b"buzzards\0")).cast::<u8>();
    unsafe {
        this.cast::<*mut u8>().write(bytes);
        this.add(4).cast::<*mut u8>().write(bytes.add(8));
        this.add(8).cast::<*mut u8>().write(bytes.add(9));
    }
    log(Call::String);
}

unsafe extern "thiscall" fn string_chars(this: *mut EffectName) -> *const u8 {
    log(Call::Chars);
    unsafe {
        assert_eq!((*this).table, 0x00A8_0810);
        assert_eq!((*this).extra, 0);
        (*this).begin
    }
}

unsafe extern "C" fn free(pointer: *mut u8) {
    unsafe {
        drop(Box::from_raw(pointer.cast::<[u8; 9]>()));
    }
    log(Call::Free);
}

extern "thiscall" fn pollution(this: *mut u8, kind: u32, x: i32, z: i32, size: i32) -> u32 {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        assert_eq!(this as usize, state.sim);
        state.calls.push(Call::Pollution(kind, x, z, size));
        state.samples[kind as usize]
    })
}

extern "thiscall" fn curve(this: *mut u8, input: f32) -> f64 {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let index = (this as usize - state.tuning - 0x40) / 16;
        state.calls.push(Call::Curve(index, input));
        state.curves[index]
    })
}

extern "thiscall" fn random(this: *mut u8, a: u32, b: u32, c: u32, d: u32) -> f64 {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        assert_eq!(this as usize, state.sim + 0xB8);
        state.calls.push(Call::Random([a, b, c, d]));
        state.draws.pop_front().expect("a queued random result")
    })
}

unsafe extern "thiscall" fn developer(this: *mut Grid) -> *mut u8 {
    log(Call::Developer(unsafe { (*this).id }));
    unsafe { (*this).map }
}

unsafe extern "thiscall" fn size(this: *mut Grid) -> i32 {
    log(Call::Size(unsafe { (*this).id }));
    unsafe { (*this).size }
}

unsafe extern "thiscall" fn read_integer(this: *mut Grid, x: i32, z: i32) -> u32 {
    log(Call::Read(unsafe { (*this).id }, x, z));
    unsafe { (*this).integer }
}

unsafe extern "thiscall" fn read_float(this: *mut Grid, x: i32, z: i32) -> f64 {
    unsafe {
        log(Call::Read((*this).id, x, z));
        state(|state| {
            if let Some((id, offset, next)) = state.swap {
                if id == (*this).id {
                    ((state.sim + offset) as *mut *mut u8).write_unaligned(next as *mut u8);
                    state.swap = None;
                }
            }
        });
        (*this).float
    }
}

unsafe extern "thiscall" fn write(this: *mut Grid, x: i32, z: i32, value: u32) {
    unsafe {
        log(Call::Write((*this).id, x, z, value));
        if (3..9).contains(&(*this).id) || (*this).id == 16 {
            (*this).float = f64::from(f32::from_bits(value));
        } else {
            (*this).integer = if (*this).id == 9 { value & 0xFF } else { value & 0xFFFF };
        }
    }
}

unsafe extern "thiscall" fn width(this: *mut Grid) -> f64 {
    log(Call::Extent(unsafe { (*this).id }, false));
    unsafe { (*this).width }
}

unsafe extern "thiscall" fn depth(this: *mut Grid) -> f64 {
    log(Call::Extent(unsafe { (*this).id }, true));
    unsafe { (*this).depth }
}

extern "thiscall" fn altitude(_this: *mut u8, x: f32, z: f32) -> f64 {
    log(Call::Altitude(x, z));
    7.0
}

unsafe extern "thiscall" fn create(_this: *mut u8, name: *const u8, out: *mut *mut u8) {
    assert_eq!(unsafe { std::slice::from_raw_parts(name, 9) }, b"buzzards\0");
    assert!(unsafe { out.read() }.is_null());
    log(Call::Create);
    STATE.with(|state| {
        let state = state.borrow();
        unsafe {
            out.write(if state.create {
                state.effect as *mut u8
            } else {
                core::ptr::null_mut()
            })
        };
    });
}

unsafe extern "thiscall" fn transform(_this: *mut u8, transform: *mut u8) {
    unsafe {
        assert_eq!(transform.read(), 1);
        assert_eq!(transform.add(1).read(), 1);
        assert_eq!(transform.add(2).cast::<u16>().read_unaligned(), 0xA1B2);
        for (i, value) in [1.0f32, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0].iter().enumerate() {
            assert_eq!(transform.add(4 + i * 4).cast::<f32>().read_unaligned(), *value);
        }
        assert_eq!(transform.add(52).cast::<f32>().read_unaligned(), 1.0);
        log(Call::Transform(std::array::from_fn(|i| {
            transform.add(40 + i * 4).cast::<f32>().read_unaligned()
        })));
    }
}

extern "thiscall" fn start(_this: *mut u8, argument: u32) {
    log(Call::Start(argument));
}
extern "thiscall" fn release(_this: *mut u8) {
    log(Call::Release);
}

unsafe extern "thiscall" fn month(_this: *mut u8, a: u32, out: *mut u32, b: u32, c: u32, d: u32) {
    assert_eq!([a, b, c, d], [0; 4]);
    log(Call::Month);
    STATE.with(|state| unsafe { out.write(state.borrow().month) });
}

extern "thiscall" fn life(this: *mut u8, x: i32, z: i32) -> u32 {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        assert_eq!(this as usize, state.sim);
        state.calls.push(Call::Life(x, z));
        state.life
    })
}

// Pointers are installed before these owning vectors are assembled and must stay stable as they grow.
#[allow(clippy::vec_box)]
struct World {
    sim: Object,
    tuning: Object,
    grids: Vec<Grid>,
    _tables: Vec<Box<[usize; 32]>>,
    _objects: Vec<Box<[usize; 1]>>,
    _constants: Vec<Box<f64>>,
    _guards: Vec<Fake>,
    _fpu: GameFpu,
}

impl World {
    fn new() -> Self {
        state(|state| *state = State::default());
        let mut sim = Object::new(0x3D0);
        let mut tuning = Object::new(0x70);
        let mut integer_table = Box::new([0usize; 32]);
        integer_table[0x28 / 4] = size as *const () as usize;
        integer_table[0x58 / 4] = read_integer as *const () as usize;
        integer_table[0x68 / 4] = write as *const () as usize;
        let mut float_table = integer_table.clone();
        float_table[0x58 / 4] = read_float as *const () as usize;
        float_table[0x38 / 4] = width as *const () as usize;
        float_table[0x3C / 4] = depth as *const () as usize;
        let mut developer_table = Box::new([0usize; 32]);
        developer_table[0x3C / 4] = developer as *const () as usize;
        let mut grids = (0..17)
            .map(|id| Grid {
                table: if (3..9).contains(&id) || id == 16 {
                    float_table.as_ptr()
                } else if (13..16).contains(&id) {
                    developer_table.as_ptr()
                } else {
                    integer_table.as_ptr()
                },
                id,
                integer: 0,
                float: 0.0,
                size: 4,
                width: 64.0,
                depth: 64.0,
                map: core::ptr::null_mut(),
            })
            .collect::<Vec<_>>();
        for wealth in 0..3 {
            grids[13 + wealth].map = (&mut grids[10 + wealth] as *mut Grid).cast();
            for (offset, id) in [
                (0x28 + wealth * 4, 13 + wealth),
                (0x48 + wealth * 4, wealth),
                (0x7C + wealth * 4, 3 + wealth),
                (0x88 + wealth * 4, 6 + wealth),
            ] {
                unsafe {
                    sim.ptr()
                        .add(offset)
                        .cast::<*mut u8>()
                        .write((&mut grids[id] as *mut Grid).cast());
                }
            }
        }
        unsafe {
            sim.ptr()
                .add(0x54)
                .cast::<*mut u8>()
                .write((&mut grids[9] as *mut Grid).cast());
        }
        let mut sim_table = Box::new([0usize; 32]);
        sim_table[0x74 / 4] = life as *const () as usize;
        unsafe {
            sim.ptr().cast::<*const usize>().write(sim_table.as_ptr());
        }
        let mut effect_table = Box::new([0usize; 32]);
        effect_table[8 / 4] = release as *const () as usize;
        effect_table[0x0C / 4] = start as *const () as usize;
        effect_table[0x20 / 4] = transform as *const () as usize;
        let effect = Box::new([effect_table.as_ptr() as usize]);
        let mut effects_table = Box::new([0usize; 32]);
        effects_table[0x1C / 4] = create as *const () as usize;
        let effects = Box::new([effects_table.as_ptr() as usize]);
        let mut terrain_table = Box::new([0usize; 32]);
        terrain_table[0x34 / 4] = altitude as *const () as usize;
        let terrain = Box::new([terrain_table.as_ptr() as usize]);
        let mut clock_table = Box::new([0usize; 32]);
        clock_table[0x18 / 4] = month as *const () as usize;
        let clock = Box::new([clock_table.as_ptr() as usize]);
        let mut effect_global = Box::new([effects.as_ptr() as usize]);
        let mut terrain_global = Box::new([terrain.as_ptr() as usize]);
        let mut clock_global = Box::new([clock.as_ptr() as usize]);
        state(|state| {
            state.sim = sim.ptr() as usize;
            state.tuning = tuning.ptr() as usize;
            state.effect = effect.as_ptr() as usize;
        });
        let mut guards = vec![
            fake(0x0040_7FA0, string_range as *const () as usize),
            fake(0x009D_7E63, string_chars as *const () as usize),
            fake(0x0090_CF63, free as *const () as usize),
            fake(0x006F_7FF0, pollution as *const () as usize),
            fake(0x005F_4250, curve as *const () as usize),
            fake(0x0091_3761, random as *const () as usize),
            fake_data(0x00AB_2BD8, b"buzzards".as_ptr().cast_mut()),
            fake_data(0x00B4_3D1C, effect_global.as_mut_ptr().cast()),
            fake_data(0x00B4_3CF4, terrain_global.as_mut_ptr().cast()),
            fake_data(0x00B4_3D5C, clock_global.as_mut_ptr().cast()),
        ];
        let mut objects = vec![
            effect,
            effects,
            terrain,
            clock,
            effect_global,
            terrain_global,
            clock_global,
        ];
        for (address, bits) in [
            (0x00A8_1054, 0.0f32.to_bits()),
            (0x00A8_1228, 1.0f32.to_bits()),
            (0x00A8_825C, 2.0f32.to_bits()),
            (0x00A8_4D2C, 0.5f32.to_bits()),
            (0x00A9_44D8, 200.0f32.to_bits()),
            (0x00B4_AF94, 0xDEAD_BEEF),
        ] {
            let mut object = Box::new([bits as usize]);
            guards.push(fake_data(address, object.as_mut_ptr().cast()));
            objects.push(object);
        }
        // Double constants need two words of backing storage; the vectors keep their allocations stable.
        let mut hundred = Box::new(100.0f64);
        let mut low = Box::new(0.0f64);
        guards.push(fake_data(0x00A8_0AC0, (&mut *hundred as *mut f64).cast()));
        guards.push(fake_data(0x00A8_0990, (&mut *low as *mut f64).cast()));
        let mut world = Self {
            sim,
            tuning,
            grids,
            _tables: vec![
                integer_table,
                float_table,
                developer_table,
                sim_table,
                effect_table,
                effects_table,
                terrain_table,
                clock_table,
            ],
            _objects: objects,
            _constants: vec![hundred, low],
            _guards: guards,
            _fpu: GameFpu::new(),
        };
        unsafe {
            let t = Tuning::from_ptr(world.tuning.ptr());
            t.set_reset_age(40);
            t.set_eq_reset(f32::from_bits(0x3F4C_CCCC));
            t.set_hq_reset(f32::from_bits(0x3F4C_CCCC));
            for (offset, value) in [(8, 25u16), (10, 30), (12, 35)] {
                world.tuning.ptr().add(offset).cast::<u16>().write(value);
            }
            for offset in (0x18..0x30).step_by(4) {
                world.tuning.ptr().add(offset).cast::<f32>().write(20.0);
            }
        }
        world
    }

    fn tuning(&mut self) -> Tuning {
        unsafe { Tuning::from_ptr(self.tuning.ptr()) }
    }
    fn sim(&mut self) -> TractSim {
        unsafe { TractSim::from_ptr(self.sim.ptr()) }
    }
    fn run(&mut self, x: i32, z: i32) {
        unsafe {
            seeded_entry(self.sim.ptr(), self.tuning.ptr(), x, z);
        }
    }
    fn populations(&mut self, old: [u16; 3], new: [u16; 3]) {
        for wealth in 0..3 {
            self.grids[wealth].integer = u32::from(old[wealth]);
            self.grids[10 + wealth].integer = u32::from(new[wealth]);
        }
    }
    fn quotients(&mut self, eq: [f32; 3], hq: [f32; 3]) {
        for wealth in 0..3 {
            self.grids[3 + wealth].float = f64::from(eq[wealth]);
            self.grids[6 + wealth].float = f64::from(hq[wealth]);
        }
    }
    fn eq(&self) -> [f32; 3] {
        std::array::from_fn(|wealth| self.grids[3 + wealth].float as f32)
    }
    fn hq(&self) -> [f32; 3] {
        std::array::from_fn(|wealth| self.grids[6 + wealth].float as f32)
    }
    fn histogram(&mut self, bucket: usize) -> (i32, i32) {
        unsafe {
            (
                self.sim.ptr().add(0x1C4 + bucket * 4).cast::<i32>().read(),
                self.sim.ptr().add(0x214 + bucket * 4).cast::<i32>().read(),
            )
        }
    }
}

#[unsafe(naked)]
unsafe extern "thiscall" fn seeded_entry(_this: *mut u8, _tuning: *mut u8, _x: i32, _z: i32) {
    core::arch::naked_asm!("mov dword ptr [esp - 0x38], 0xa1b20000", "jmp {entry}", entry=sym fc4p_006fa450);
}

#[test]
fn decay_pollution_and_complete_call_order() {
    let mut world = World::new();
    world.populations([100, 100, 0], [100, 100, 0]);
    world.quotients([10.0, 0.1, 0.0], [10.0, 0.1, 0.0]);
    world.grids[9].integer = 30;
    world.tuning().set_eq_decay(0.2);
    world.tuning().set_hq_decay(0.2);
    state(|state| state.samples = [0x1234_0001, 0xABCD_0002, 0xFFFF_0003]);
    world.run(2, 3);
    assert_eq!(world.eq(), [9.8, 0.0, 0.0]);
    assert_eq!(world.hq(), [9.8, 0.0, 0.0]);
    assert_eq!(world.sim().running_population(), 200);
    assert_eq!((world.sim().life_low(), world.sim().life_high()), (20000, 0));
    assert_eq!(world.histogram(3), (200, 900));
    let mut expected = vec![
        Call::String,
        Call::Developer(13),
        Call::Developer(14),
        Call::Developer(15),
        Call::Size(0),
    ];
    for kind in 0..3 {
        expected.extend([
            Call::Pollution(kind, 8, 12, 4),
            Call::Curve(kind as usize, (kind + 1) as f32),
        ]);
    }
    for wealth in 0..3 {
        expected.extend([
            Call::Read(3 + wealth, 2, 3),
            Call::Write(3 + wealth, 2, 3, if wealth == 0 { 9.8f32.to_bits() } else { 0 }),
            Call::Read(6 + wealth, 2, 3),
            Call::Write(6 + wealth, 2, 3, if wealth == 0 { 9.8f32.to_bits() } else { 0 }),
        ]);
    }
    for wealth in 0..3 {
        expected.push(Call::Read(10 + wealth, 2, 3));
    }
    for wealth in 0..3 {
        expected.extend([
            Call::Read(wealth, 2, 3),
            Call::Write(wealth, 2, 3, if wealth < 2 { 100 } else { 0 }),
        ]);
        if wealth == 2 {
            expected.extend([Call::Write(5, 2, 3, 0), Call::Write(8, 2, 3, 0)]);
        }
    }
    expected.extend([
        Call::Read(9, 2, 3),
        Call::Month,
        Call::Size(0),
        Call::Size(0),
        Call::Life(8, 12),
        Call::Write(9, 2, 3, 30),
    ]);
    for wealth in 0..3 {
        expected.extend([Call::Read(wealth, 2, 3), Call::Read(3 + wealth, 2, 3)]);
    }
    expected.push(Call::Free);
    assert_eq!(take(), expected);
}

#[test]
fn pollution_curve_results_accumulate_in_order_at_x87_precision() {
    let mut world = World::new();
    world.populations([10; 3], [10; 3]);
    world.quotients([100.0; 3], [50_000_000.0; 3]);
    world.tuning().set_hq_decay(16_777_216.0);
    state(|state| state.curves = [1.0, -16_777_216.0, 0.25]);
    world.run(0, 0);
    assert_eq!(world.hq(), [50_000_000.0; 3]);
    let writes = take()
        .into_iter()
        .filter_map(|call| match call {
            Call::Write(id, _, _, bits) if (6..9).contains(&id) => Some(bits),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(writes, [50_000_000.0f32.to_bits(); 3]);
}

#[test]
fn newcomers_blend_and_age_preserve_signed_delta_and_old_population_quirks() {
    for (old, new, age, eq, want_age, want_eq) in [
        ([100, 0, 0], [150, 0, 0], 40, 50.0, 35, 45.0),
        ([100, 0, 0], [50, 10, 0], 40, 0.0, 71, 0.0),
        ([100, 0, 0], [150, 0, 0], 30, 0.0, 28, f32::from_bits(0x40D5_5555)),
        ([10_000, 0, 0], [50_000, 0, 0], 30, 50.0, -6, -7.8752),
    ] {
        let mut world = World::new();
        world.populations(old, new);
        world.quotients([eq, 0.0, 0.0], [eq, 0.0, 0.0]);
        world.grids[9].integer = age;
        world.run(0, 0);
        assert_eq!(world.grids[9].integer, want_age as u32 & 0xFF);
        assert_eq!(world.eq()[0], want_eq);
        assert_eq!(world.hq()[0], want_eq);
    }
}

#[test]
fn december_wraps_the_age_and_empty_tracts_skip_the_month_query() {
    let mut world = World::new();
    state(|state| {
        state.month = 12;
        state.life = 255;
    });
    world.populations([100, 0, 0], [100, 0, 0]);
    for (age, want) in [(40, 41), (255, 0)] {
        world.grids[9].integer = age;
        world.run(0, 0);
        assert_eq!(world.grids[9].integer, want);
        assert_eq!(take().iter().filter(|call| **call == Call::Month).count(), 1);
    }
    world.populations([100, 0, 0], [0, 0, 0]);
    world.run(0, 0);
    assert_eq!(world.grids[9].integer, 0);
    assert!(!take().contains(&Call::Month));
}

#[test]
fn reset_uses_stored_factors_then_histogram_truncates_eq() {
    let mut world = World::new();
    world.populations([200, 0, 0], [200, 0, 0]);
    world.quotients(
        [f32::from_bits(0x4248_CCCD), 0.0, 0.0],
        [f32::from_bits(0x4270_CCCD), 0.0, 0.0],
    );
    world.tuning().set_eq_decay(0.2);
    world.tuning().set_hq_decay(0.2);
    world.grids[9].integer = 70;
    state(|state| state.life = 0x1234_5641);
    world.sim().set_life_low(u32::MAX - 9999);
    world.sim().set_life_high(5);
    world.sim().set_running_population(i32::MAX - 99);
    world.run(0, 0);
    assert_eq!(world.grids[9].integer, 40);
    assert_eq!(world.eq()[0].to_bits(), 0x421F_FFFF);
    assert_eq!(world.hq()[0].to_bits(), 0x423F_FFFF);
    assert_eq!(world.histogram(6), (200, 7800));
    assert_eq!((world.sim().life_low(), world.sim().life_high()), (3000, 6));
    assert_eq!(world.sim().running_population(), i32::MIN + 100);
}

#[test]
fn age_bucket_boundaries_and_wrapping_histograms() {
    for (age, bucket) in [
        (0, 19),
        (17, 19),
        (18, 0),
        (19, 0),
        (20, 0),
        (22, 0),
        (23, 1),
        (35, 5),
        (76, 18),
        (77, 19),
        (255, 19),
    ] {
        let mut world = World::new();
        world.populations([2, 0, 0], [2, 0, 0]);
        world.quotients([3.75, 0.0, 0.0], [0.0; 3]);
        world.grids[9].integer = age;
        state(|state| state.life = 255);
        unsafe {
            world.sim.ptr().add(0x1C4 + bucket * 4).cast::<i32>().write(i32::MAX);
            world.sim.ptr().add(0x214 + bucket * 4).cast::<i32>().write(i32::MAX);
        }
        world.run(0, 0);
        assert_eq!(world.histogram(bucket), (i32::MIN + 1, i32::MIN + 5));
    }
}

#[test]
fn effects_create_transform_start_and_release_once_per_tract() {
    let mut world = World::new();
    world.populations([100, 100, 0], [100, 100, 0]);
    world.quotients([0.0; 3], [1.2, 1.2, 0.0]);
    world.tuning().set_door(2.0);
    world.tuning().set_effect_chance(1.5);
    world.tuning().set_effect_height(50.0);
    world.tuning().set_effect_delta(10.0);
    state(|state| state.draws.extend([1.2, 3.0]));
    world.run(2, 3);
    let effects = take()
        .into_iter()
        .filter(|call| {
            matches!(
                call,
                Call::Random(_)
                    | Call::Create
                    | Call::Extent(_, _)
                    | Call::Altitude(_, _)
                    | Call::Transform(_)
                    | Call::Start(_)
                    | Call::Release
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        effects,
        [
            Call::Random([0, 0, 0, 0x4059_0000]),
            Call::Create,
            Call::Extent(3, false),
            Call::Extent(3, false),
            Call::Extent(3, true),
            Call::Extent(3, true),
            Call::Altitude(160.0, 224.0),
            Call::Random([0, 0xC024_0000, 0, 0x4024_0000]),
            Call::Transform([160.0, 60.0, 224.0]),
            Call::Start(0),
            Call::Release
        ]
    );
}

#[test]
fn effects_retry_failed_creation_and_require_strict_comparisons() {
    for (chance, draw, create, want_creates) in [
        (1.5, 1.0, false, 2),
        (1.5, 1.5, true, 0),
        (f32::NAN, 1.0, true, 0),
        (1.5, f64::NAN, true, 0),
    ] {
        let mut world = World::new();
        world.populations([100, 100, 0], [100, 100, 0]);
        world.quotients([0.0; 3], [1.2, 1.2, 0.0]);
        world.tuning().set_door(2.0);
        world.tuning().set_effect_chance(chance);
        state(|state| {
            state.draws.extend([draw, draw]);
            state.create = create;
        });
        world.run(0, 0);
        let calls = take();
        assert_eq!(calls.iter().filter(|call| **call == Call::Create).count(), want_creates);
        assert_eq!(calls.iter().filter(|call| matches!(call, Call::Random(_))).count(), 2);
        assert!(!calls.iter().any(|call| matches!(call, Call::Start(_) | Call::Release)));
    }
}

#[test]
fn decay_clamps_only_ordered_negative_values_and_blend_caps_only_above_maximum() {
    let mut world = World::new();
    world.populations([1; 3], [1; 3]);
    world.quotients([f32::NAN, f32::INFINITY, -1.0], [f32::NAN, f32::INFINITY, -1.0]);
    world.run(0, 0);
    assert!(world.eq()[0].is_nan());
    assert_eq!(&world.eq()[1..], &[f32::INFINITY, 0.0]);
    let mut world = World::new();
    world.populations([0; 3], [1; 3]);
    world.quotients([f32::NAN, 500.0, 0.0], [f32::NAN, 500.0, 0.0]);
    world.run(0, 0);
    assert!(world.eq()[0].is_nan());
    assert_eq!(&world.eq()[1..], &[200.0, 20.0]);
}

#[test]
fn quotient_pointer_is_reloaded_after_a_getter_and_other_fields_stay_untouched() {
    let mut world = World::new();
    world.populations([10; 3], [10; 3]);
    world.quotients([10.0; 3], [10.0; 3]);
    world.tuning().set_eq_decay(0.2);
    let replacement = (&mut world.grids[16] as *mut Grid) as usize;
    state(|state| state.swap = Some((3, 0x7C, replacement)));
    let before = unsafe { std::slice::from_raw_parts(world.sim.ptr(), 0x3D0).to_vec() };
    world.run(i32::MAX, i32::MIN);
    assert_eq!(world.grids[3].float, 10.0);
    assert_eq!(world.grids[16].float, f64::from(9.8f32));
    let after = unsafe { std::slice::from_raw_parts(world.sim.ptr(), 0x3D0) };
    for index in 0..0x3D0 {
        if !(0x7C..0x80).contains(&index)
            && !(0xA8..0xAC).contains(&index)
            && !(0x348..0x350).contains(&index)
            && !(0x1C4..0x264).contains(&index)
        {
            assert_eq!(after[index], before[index], "byte {index:X}");
        }
    }
    assert!(take().contains(&Call::Pollution(0, -4, 0, 4)));
}
