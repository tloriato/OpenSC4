// SPDX-License-Identifier: GPL-3.0-or-later
//! Visual-effect vectors moved from core pollution, including visibility, empty effects and worst blocks.

use super::*;
use crate::testing::{fake, fake_data, Fake, GameFpu, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq)]
enum Call {
    Width,
    Height,
    CountX,
    Reset,
    Next,
    Cell(u32, i32, i32),
    Curve(u32, u32),
    Random(u64, u64),
    Terrain(u32, u32),
    Create(u32),
    Transform(u32, [u32; 3]),
    Parameter(u32, u32, u32),
    Start(u32),
    Started(u32),
    Stop(u32),
    Release(u32),
}
struct State {
    calls: Vec<Call>,
    blocks: Vec<(i32, i32)>,
    cursor: usize,
    values: Vec<[u32; 4]>,
    strengths: [f64; 4],
    effects: [*mut u8; 4],
    fail: bool,
    sim: usize,
    random: usize,
}
thread_local! {static STATE:RefCell<State>=const{RefCell::new(State{
    calls:Vec::new(),blocks:Vec::new(),cursor:0,values:Vec::new(),strengths:[0.0;4],
    effects:[core::ptr::null_mut();4],fail:false,sim:0,random:0,
})};}
fn log(call: Call) {
    STATE.with(|s| s.borrow_mut().calls.push(call));
}
fn calls() -> Vec<Call> {
    STATE.with(|s| std::mem::take(&mut s.borrow_mut().calls))
}
#[repr(C)]
struct Grid {
    table: *const usize,
    kind: u32,
}
#[repr(C)]
struct Effect {
    table: *const usize,
    id: u32,
    started: bool,
    slot: *mut *mut u8,
    replacement: *mut u8,
}
extern "thiscall" fn width(_this: *mut u8) -> f64 {
    log(Call::Width);
    16.0
}
extern "thiscall" fn height(_this: *mut u8) -> f64 {
    log(Call::Height);
    32.0
}
extern "thiscall" fn count_x(_this: *mut u8) -> i32 {
    log(Call::CountX);
    32
}
extern "thiscall" fn reset(this: *mut u8, shuffle: u32) {
    assert_eq!(shuffle, 1);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.sim + 0x78);
        s.cursor = 0;
    });
    log(Call::Reset);
}
extern "thiscall" fn next(this: *mut u8, x: *mut i32, z: *mut i32) -> u32 {
    log(Call::Next);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.sim + 0x78);
        if let Some(&(bx, bz)) = s.blocks.get(s.cursor) {
            s.cursor += 1;
            unsafe {
                x.write(bx);
                z.write(bz)
            };
            0xAABB_CC01
        } else {
            0x7654_3200
        }
    })
}
extern "thiscall" fn cell(this: *mut Grid, x: i32, z: i32) -> u32 {
    let kind = unsafe { (*this).kind };
    log(Call::Cell(kind, x, z));
    STATE.with(|s| s.borrow().values[(x >> 3) as usize][kind as usize])
}
extern "thiscall" fn curve(this: *mut u8, input: f32) -> f64 {
    let kind = this as u32 - 1;
    log(Call::Curve(kind, input.to_bits()));
    STATE.with(|s| s.borrow().strengths[kind as usize])
}
extern "thiscall" fn random(this: *mut u8, low: f64, high: f64) -> f64 {
    STATE.with(|s| assert_eq!(this as usize, s.borrow().random));
    log(Call::Random(low.to_bits(), high.to_bits()));
    low
}
extern "thiscall" fn terrain(_this: *mut u8, x: f32, z: f32) -> f64 {
    log(Call::Terrain(x.to_bits(), z.to_bits()));
    20.0
}
extern "thiscall" fn create(_this: *mut u8, id: u32, kind: u32, output: *mut *mut u8) {
    assert_eq!(id, 0x0A78_747A);
    log(Call::Create(kind));
    STATE.with(|s| {
        let s = s.borrow();
        if !s.fail {
            unsafe { output.write(s.effects[kind as usize - 1]) }
        }
    });
}
extern "thiscall" fn transform(this: *mut Effect, value: *const Transform) {
    unsafe {
        let t = &*value;
        assert_eq!(t.flags & 0xFFFF, 0x0101);
        assert_eq!(&t.matrix[..9], &[1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]);
        assert_eq!(t.matrix[12], 1.0);
        log(Call::Transform(
            (*this).id,
            [t.matrix[9].to_bits(), t.matrix[10].to_bits(), t.matrix[11].to_bits()],
        ));
        if !(*this).replacement.is_null() {
            (*this).slot.write((*this).replacement);
        }
    }
}
extern "thiscall" fn parameter(this: *mut Effect, id: u32, value: *mut f32, count: u32) {
    assert_eq!(count, 1);
    log(Call::Parameter(
        unsafe { (*this).id },
        id,
        unsafe { value.read_unaligned() }.to_bits(),
    ));
}
extern "thiscall" fn start(this: *mut Effect, flags: u32) {
    assert_eq!(flags, 0);
    log(Call::Start(unsafe { (*this).id }));
}
extern "thiscall" fn started(this: *mut Effect) -> u32 {
    log(Call::Started(unsafe { (*this).id }));
    u32::from(unsafe { (*this).started })
}
extern "thiscall" fn stop(this: *mut Effect, flags: u32) {
    assert_eq!(flags, 0);
    log(Call::Stop(unsafe { (*this).id }));
}
extern "thiscall" fn release(this: *mut Effect) {
    log(Call::Release(unsafe { (*this).id }));
}
fn table(slots: &[(usize, usize)]) -> Box<[usize; 100]> {
    let mut t = Box::new([0; 100]);
    for &(slot, f) in slots {
        t[slot / 4] = f;
    }
    t
}
struct World {
    sim: Object,
    city: Box<[u32; 1]>,
    city_pointer: Box<*mut u8>,
    terrain: Box<[u32; 1]>,
    terrain_pointer: Box<*mut u8>,
    manager: Box<[u32; 1]>,
    grids: [Box<Grid>; 4],
    effects: [Box<Effect>; 4],
    slots: [Box<[*mut u8; 4]>; 4],
    curve_pointers: Box<[*mut u8; 4]>,
    random: Box<u8>,
    constants: Box<[f32; 4]>,
    // Vtables retain their addresses while the owner moves.
    #[allow(clippy::vec_box)]
    _tables: Vec<Box<[usize; 100]>>,
    _fakes: Vec<Fake>,
}
impl World {
    fn new(blocks: &[(i32, i32)]) -> Self {
        let tables = vec![
            table(&[
                (0x16C, width as *const () as usize),
                (0x170, height as *const () as usize),
                (0x174, count_x as *const () as usize),
            ]),
            table(&[(0x34, terrain as *const () as usize)]),
            table(&[(0x18, create as *const () as usize)]),
            table(&[(0x1C, cell as *const () as usize)]),
            table(&[
                (0x20, transform as *const () as usize),
                (0x30, parameter as *const () as usize),
                (0x0C, start as *const () as usize),
                (0x14, started as *const () as usize),
                (0x10, stop as *const () as usize),
                (0x08, release as *const () as usize),
            ]),
        ];
        let mut w = Self {
            sim: Object::new(0x240),
            city: Box::new([tables[0].as_ptr() as u32]),
            city_pointer: Box::new(core::ptr::null_mut()),
            terrain: Box::new([tables[1].as_ptr() as u32]),
            terrain_pointer: Box::new(core::ptr::null_mut()),
            manager: Box::new([tables[2].as_ptr() as u32]),
            grids: core::array::from_fn(|i| {
                Box::new(Grid {
                    table: tables[3].as_ptr(),
                    kind: i as u32,
                })
            }),
            effects: core::array::from_fn(|i| {
                Box::new(Effect {
                    table: tables[4].as_ptr(),
                    id: i as u32 + 1,
                    started: true,
                    slot: core::ptr::null_mut(),
                    replacement: core::ptr::null_mut(),
                })
            }),
            slots: core::array::from_fn(|_| Box::new([core::ptr::null_mut(); 4])),
            curve_pointers: Box::new(core::array::from_fn(|i| (i + 1) as *mut u8)),
            random: Box::new(0),
            constants: Box::new([8.0, f32::from_bits(0x3800_0100), 0.05, 3.0]),
            _tables: tables,
            _fakes: vec![],
        };
        *w.city_pointer = w.city.as_mut_ptr().cast();
        *w.terrain_pointer = w.terrain.as_mut_ptr().cast();
        let sim = unsafe { VisualView::from_ptr(w.sim.ptr()) };
        sim.set_city(*w.city_pointer);
        sim.set_effects(w.manager.as_mut_ptr().cast());
        sim.set_visible(1);
        sim.set_water_effects(w.slots[0].as_mut_ptr());
        sim.set_air_effects(w.slots[1].as_mut_ptr());
        sim.set_radiation_effects(w.slots[2].as_mut_ptr());
        sim.set_garbage_effects(w.slots[3].as_mut_ptr());
        let stats = unsafe { PollutionSim::from_ptr(w.sim.ptr()) };
        stats.set_water((&mut *w.grids[0] as *mut Grid).cast());
        stats.set_air((&mut *w.grids[1] as *mut Grid).cast());
        stats.set_radiation_source((&mut *w.grids[2] as *mut Grid).cast());
        stats.set_garbage((&mut *w.grids[3] as *mut Grid).cast());
        STATE.with(|s| {
            *s.borrow_mut() = State {
                calls: vec![],
                blocks: blocks.to_vec(),
                cursor: 0,
                values: vec![[0; 4]; 2],
                strengths: [0.0; 4],
                effects: core::array::from_fn(|i| (&mut *w.effects[i] as *mut Effect).cast()),
                fail: false,
                sim: w.sim.ptr() as usize,
                random: (&mut *w.random as *mut u8) as usize,
            }
        });
        w._fakes = vec![
            fake(0x0049_D1C0, reset as *const () as usize),
            fake(0x0049_CB10, next as *const () as usize),
            fake(0x005F_4250, curve as *const () as usize),
            fake(0x0091_3A46, random as *const () as usize),
            fake_data(0x00B4_3CEC, (&mut *w.city_pointer as *mut *mut u8).cast()),
            fake_data(0x00B4_3CF4, (&mut *w.terrain_pointer as *mut *mut u8).cast()),
            fake_data(0x00B4_9BD0, (&mut *w.random as *mut u8).cast()),
        ];
        for (i, address) in [0x00B4_9BB0, 0x00B4_9BB4, 0x00B4_9BB8, 0x00B4_9BBC]
            .into_iter()
            .enumerate()
        {
            w._fakes.push(fake_data(address, unsafe {
                w.curve_pointers.as_mut_ptr().add(i).cast()
            }));
        }
        for (i, address) in [0x00A8_E178, 0x00AB_25E4, 0x00A8_80D8, 0x00A8_5074]
            .into_iter()
            .enumerate()
        {
            w._fakes
                .push(fake_data(address, unsafe { w.constants.as_mut_ptr().add(i).cast() }));
        }
        w
    }
    fn run(&mut self) -> u32 {
        unsafe { fc4p_006e63c0(self.sim.ptr()) }
    }
    fn stats(&mut self) -> PollutionSim {
        unsafe { PollutionSim::from_ptr(self.sim.ptr()) }
    }
    fn existing(&mut self) {
        for i in 0..4 {
            self.slots[i][0] = (&mut *self.effects[i] as *mut Effect).cast();
        }
    }
}

#[test]
fn scans_x_before_z_in_grid_order_and_keeps_first_worst_tie() {
    let _fpu = GameFpu::new();
    let mut w = World::new(&[(0, 0), (1, 0)]);
    STATE.with(|s| s.borrow_mut().values = vec![[17, 21, 0xFFFF_FFFE, 0xAABB_801F]; 2]);
    assert_eq!(w.run(), 0x7654_3200);
    let c = calls();
    assert_eq!(
        &c[..6],
        &[
            Call::Width,
            Call::Height,
            Call::CountX,
            Call::Reset,
            Call::Next,
            Call::Cell(0, 0, 0)
        ]
    );
    let cells: Vec<_> = c.into_iter().filter(|c| matches!(c, Call::Cell(..))).collect();
    let mut expected = vec![];
    for x in 0..16 {
        for z in 0..8 {
            for kind in 0..4 {
                expected.push(Call::Cell(kind, x, z));
            }
        }
    }
    assert_eq!(cells, expected);
    let stats = w.stats();
    assert_eq!(
        (stats.worst_air_x(), stats.worst_air_z(), stats.worst_air_value()),
        (4, 4, 21)
    );
    assert_eq!(
        (stats.worst_water_x(), stats.worst_water_z(), stats.worst_water_value()),
        (4, 4, 17)
    );
    assert_eq!(
        (
            stats.worst_garbage_x(),
            stats.worst_garbage_z(),
            stats.worst_garbage_value()
        ),
        (4, 4, 31)
    );
}

#[test]
fn creates_all_effects_with_original_positions_parameters_and_call_order() {
    let _fpu = GameFpu::new();
    let mut w = World::new(&[(0, 0)]);
    STATE.with(|s| s.borrow_mut().strengths = [0.5; 4]);
    w.run();
    let c: Vec<_> = calls().into_iter().filter(|c| !matches!(c, Call::Cell(..))).collect();
    assert_eq!(
        c,
        [
            Call::Width,
            Call::Height,
            Call::CountX,
            Call::Reset,
            Call::Next,
            Call::Curve(0, 0),
            Call::Random((-32.0f64).to_bits(), 32.0f64.to_bits()),
            Call::Width,
            Call::Random((-64.0f64).to_bits(), 64.0f64.to_bits()),
            Call::Height,
            Call::Create(1),
            Call::Transform(1, [(-32.0f32).to_bits(), 0, (-64.0f32).to_bits()]),
            Call::Parameter(1, 3, 0.5f32.to_bits()),
            Call::Start(1),
            Call::Curve(1, 0),
            Call::Width,
            Call::Height,
            Call::Terrain(64.0f32.to_bits(), 128.0f32.to_bits()),
            Call::Create(2),
            Call::Transform(2, [64.0f32.to_bits(), 20.0f32.to_bits(), 128.0f32.to_bits()]),
            Call::Parameter(2, 2, 0.5f32.to_bits()),
            Call::Start(2),
            Call::Curve(2, 0),
            Call::Width,
            Call::Height,
            Call::Terrain(64.0f32.to_bits(), 128.0f32.to_bits()),
            Call::Create(3),
            Call::Transform(3, [64.0f32.to_bits(), 23.0f32.to_bits(), 128.0f32.to_bits()]),
            Call::Parameter(3, 2, 0.5f32.to_bits()),
            Call::Start(3),
            Call::Curve(3, 0),
            Call::Width,
            Call::Height,
            Call::Terrain(64.0f32.to_bits(), 128.0f32.to_bits()),
            Call::Create(4),
            Call::Transform(4, [64.0f32.to_bits(), 20.0f32.to_bits(), 128.0f32.to_bits()]),
            Call::Parameter(4, 2, 0.5f32.to_bits()),
            Call::Start(4),
            Call::Next,
        ]
    );
    for slots in &w.slots {
        assert!(!slots[0].is_null());
    }
}

#[test]
fn existing_effects_update_only_strength_and_visibility_leaves_water_shown() {
    let _fpu = GameFpu::new();
    let mut w = World::new(&[(0, 0)]);
    w.existing();
    STATE.with(|s| s.borrow_mut().strengths = [0.5; 4]);
    w.run();
    let c = calls();
    let updates: Vec<_> = c
        .into_iter()
        .filter(|c| {
            matches!(
                c,
                Call::Parameter(..) | Call::Transform(..) | Call::Start(..) | Call::Create(..)
            )
        })
        .collect();
    assert_eq!(
        updates,
        [
            Call::Parameter(1, 3, 0.5f32.to_bits()),
            Call::Parameter(2, 2, 0.5f32.to_bits()),
            Call::Parameter(3, 2, 0.5f32.to_bits()),
            Call::Parameter(4, 2, 0.5f32.to_bits())
        ]
    );
    unsafe { VisualView::from_ptr(w.sim.ptr()) }.set_visible(0);
    w.effects[2].started = false;
    w.run();
    let c: Vec<_> = calls()
        .into_iter()
        .filter(|c| {
            !matches!(
                c,
                Call::Cell(..) | Call::Width | Call::Height | Call::CountX | Call::Reset | Call::Next | Call::Curve(..)
            )
        })
        .collect();
    assert_eq!(
        c,
        [
            Call::Parameter(1, 3, 0.5f32.to_bits()),
            Call::Started(2),
            Call::Stop(2),
            Call::Release(2),
            Call::Started(3),
            Call::Release(3),
            Call::Started(4),
            Call::Stop(4),
            Call::Release(4)
        ]
    );
    assert!(!w.slots[0][0].is_null());
    for i in 1..4 {
        assert!(w.slots[i][0].is_null());
    }
}

#[test]
fn threshold_uses_unrounded_curve_result_and_nan_releases_effects() {
    let _fpu = GameFpu::new();
    let mut w = World::new(&[(0, 0)]);
    STATE.with(|s| {
        s.borrow_mut().strengths = [
            f64::from(0.05f32) + 2.0f64.powi(-40),
            f64::from(0.05f32),
            -1.0,
            f64::NAN,
        ]
    });
    w.run();
    let c = calls();
    assert!(c.contains(&Call::Create(1)));
    assert!(c.contains(&Call::Parameter(1, 3, 0.05f32.to_bits())));
    for kind in 2..=4 {
        assert!(!c.contains(&Call::Create(kind)));
    }
    w.existing();
    STATE.with(|s| s.borrow_mut().strengths = [f64::NAN; 4]);
    w.run();
    let c = calls();
    for kind in 1..=4 {
        assert!(c.contains(&Call::Release(kind)));
    }
    for slots in &w.slots {
        assert!(slots[0].is_null());
    }
}

#[test]
fn failed_creation_still_visits_positions_and_reset_preserves_zero_block_coordinates() {
    let _fpu = GameFpu::new();
    let mut w = World::new(&[(0, 0)]);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.fail = true;
        s.strengths = [0.5; 4];
        s.values[0] = [3, 2000, 0, 4];
    });
    w.run();
    let c = calls();
    assert_eq!(c.iter().filter(|c| matches!(c, Call::Random(..))).count(), 2);
    assert_eq!(c.iter().filter(|c| matches!(c, Call::Create(_))).count(), 4);
    assert!(!c
        .iter()
        .any(|c| matches!(c, Call::Transform(..) | Call::Parameter(..) | Call::Start(_))));
    assert_eq!(w.stats().worst_air_value(), 2000);
    let stats = w.stats();
    stats.set_worst_air_x(123);
    stats.set_worst_air_z(456);
    STATE.with(|s| s.borrow_mut().blocks.clear());
    assert_eq!(w.run(), 0x7654_3200);
    assert_eq!(
        calls(),
        [Call::Width, Call::Height, Call::CountX, Call::Reset, Call::Next]
    );
    assert_eq!(
        (stats.worst_air_x(), stats.worst_air_z(), stats.worst_air_value()),
        (123, 456, 0)
    );
}

#[test]
fn new_effect_slot_is_reloaded_between_transform_parameter_and_start() {
    let _fpu = GameFpu::new();
    let mut w = World::new(&[(0, 0)]);
    let mut replacement = Effect {
        table: w._tables[4].as_ptr(),
        id: 99,
        started: false,
        slot: core::ptr::null_mut(),
        replacement: core::ptr::null_mut(),
    };
    w.effects[0].slot = w.slots[0].as_mut_ptr();
    w.effects[0].replacement = (&mut replacement as *mut Effect).cast();
    STATE.with(|s| s.borrow_mut().strengths = [0.5, 0.0, 0.0, 0.0]);
    w.run();
    let c = calls();
    assert!(c.contains(&Call::Transform(1, [(-32.0f32).to_bits(), 0, (-64.0f32).to_bits()])));
    assert!(c.contains(&Call::Parameter(99, 3, 0.5f32.to_bits())));
    assert!(c.contains(&Call::Start(99)));
}
