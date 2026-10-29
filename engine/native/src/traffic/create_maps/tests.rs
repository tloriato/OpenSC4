// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, fake_data, Fake, GameFpu, Object};
use crate::traffic::route_map_support::Grid;
use crate::traffic::test_support::{bytes, float_environment, table};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Grid(u32),
    Size,
    Budget([u32; 4]),
    Window(u32, i32, i32),
    Curve(u32, u32),
    Accident(u32, i32, i32, u32),
    Background(u32, i32, i32),
    Switch(u32, u32),
    Roads(u32),
    Transit(u32),
    Assign(u32, u32),
}
struct State {
    calls: Vec<Call>,
    checks: u32,
    expire: u32,
    switches: Vec<(u32, u32)>,
    window: u32,
    probabilities: [f32; 2],
    wide_probability: Option<f64>,
    background: f32,
}
impl Default for State {
    fn default() -> Self {
        Self {
            calls: vec![],
            checks: 0,
            expire: 0,
            switches: vec![],
            window: 880,
            probabilities: [0.5, 0.25],
            wide_probability: None,
            background: 3.0,
        }
    }
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn log(c: Call) {
    STATE.with(|s| s.borrow_mut().calls.push(c));
}
fn calls() -> Vec<Call> {
    STATE.with(|s| std::mem::take(&mut s.borrow_mut().calls))
}
unsafe extern "thiscall" fn grid(this: *mut u8) -> u32 {
    log(Call::Grid(unsafe { word(this, 8) }));
    unsafe { word(this, 4) }
}
extern "thiscall" fn size(_this: *mut u8) -> u32 {
    log(Call::Size);
    8
}
unsafe extern "thiscall" fn expired(this: *mut u32) -> u64 {
    log(Call::Budget(unsafe { read_unaligned(this.cast()) }));
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.checks += 1;
        (0xABCD_0123u64 << 32) | u64::from(s.expire != 0 && s.checks >= s.expire)
    })
}
extern "thiscall" fn window(this: *mut u8, x: i32, z: i32) -> u32 {
    log(Call::Window(this as u32, x, z));
    STATE.with(|s| s.borrow().window)
}
extern "thiscall" fn curve(this: *mut u8, x: f32) -> f64 {
    log(Call::Curve(this as u32, x.to_bits()));
    STATE.with(|s| {
        let s = s.borrow();
        if let Some(p) = s.wide_probability {
            return p;
        }
        if s.calls.iter().filter(|c| matches!(c, Call::Curve(..))).count() & 1 == 1 {
            f64::from(s.probabilities[0])
        } else {
            f64::from(s.probabilities[1])
        }
    })
}
extern "thiscall" fn accident(this: *mut u8, x: i32, z: i32, p: f32) {
    log(Call::Accident(this as u32, x, z, p.to_bits()));
}
extern "thiscall" fn background(this: *mut u8, x: i32, z: i32) -> f32 {
    log(Call::Background(this as u32, x, z));
    STATE.with(|s| s.borrow().background)
}
unsafe extern "thiscall" fn switch(this: *mut u8, key: *mut u32) -> u32 {
    let key = unsafe { key.read() };
    log(Call::Switch(this as u32, key));
    STATE.with(|s| {
        s.borrow()
            .switches
            .iter()
            .find(|&&(k, _)| k == key)
            .expect("switch fixture")
            .1
    })
}
extern "thiscall" fn roads(this: *mut u8) {
    log(Call::Roads(this as u32));
}
extern "thiscall" fn transit(this: *mut u8) {
    log(Call::Transit(this as u32));
}
extern "thiscall" fn assign(this: *mut u8, source: *mut u8) {
    log(Call::Assign(this as u32, source as u32));
}

struct World {
    sim: Object,
    scan: Object,
    peaks: Grid<f32>,
    ratios: Grid<f32>,
    commercial: Grid<u8>,
    volumes: Vec<Grid<f32>>,
    switches: Grid<u32>,
    infos: Grid<u32>,
    pacing: Box<[u32; 2]>,
    flags: Box<u32>,
    volume_objects: Box<[u32; 6]>,
    nodes: Vec<Object>,
    constants: Box<[f32; 4]>,
    _guards: Vec<Fake>,
    _tables: Vec<Vec<usize>>,
}
impl World {
    fn new() -> Self {
        STATE.with(|s| *s.borrow_mut() = State::default());
        let tables = vec![
            table(&[(0x60, grid as *const () as usize), (0x30, size as *const () as usize)]),
            table(&[(0x2C, background as *const () as usize)]),
        ];
        let mut peaks = Grid::new(&tables[0], 2, 8, 8, 99.0);
        let mut ratios = Grid::new(&tables[0], 4, 8, 8, 98.0);
        let mut commercial = Grid::new(&tables[0], 3, 2, 2, 0xA5);
        let mut volumes: Vec<_> = (0..6).map(|i| Grid::new(&tables[0], i + 5, 8, 8, 0.0)).collect();
        let mut switches = Grid::new(&tables[0], 11, 8, 8, 0u32);
        let infos = Grid::new(&tables[0], 12, 8, 8, 0u32);
        let mut sim = crate::traffic::test_support::object(0xD50, &tables[1]);
        unsafe {
            core::ptr::write_bytes(sim.ptr().add(4), 0, 0xD4C);
        }
        let scan = Object::new(32);
        let mut volume_objects = Box::new(core::array::from_fn(|i| volumes[i].object.ptr() as u32));
        let mut pacing = Box::new([1024, 0]);
        let mut flags = Box::new(1);
        let mut constants = Box::new([4_294_967_296.0, 12_582_912.0, 0.0, 1.0]);
        let mut guards = vec![
            fake(0x0068BF80, expired as *const () as usize),
            fake(0x00714AA0, window as *const () as usize),
            fake(0x005F4250, curve as *const () as usize),
            fake(0x00716950, accident as *const () as usize),
            fake(0x006D90F0, switch as *const () as usize),
            fake(0x0070D8A0, roads as *const () as usize),
            fake(0x0070DA40, transit as *const () as usize),
            fake(0x007AD7A0, assign as *const () as usize),
            fake_data(0x00B4B348, pacing.as_mut_ptr().cast()),
            fake_data(0x00B4B350, (&mut *flags as *mut u32).cast()),
        ];
        for (i, address) in [0x00A80AA8, 0x00AB32D8, 0x00A81054, 0x00A81228].into_iter().enumerate() {
            guards.push(fake_data(address, unsafe { constants.as_mut_ptr().add(i).cast() }));
        }
        unsafe {
            let s = CreateMapsSim::from_ptr(sim.ptr());
            s.set_pollution(volumes[0].object.ptr());
            s.set_peak(peaks.object.ptr());
            s.set_commercial(commercial.object.ptr());
            s.set_ratio(ratios.object.ptr());
            s.set_volumes(volume_objects.as_mut_ptr().cast());
            s.set_switches(switches.object.ptr());
            s.set_reduction(0.25);
            s.set_commercial_scale(0.128);
            for i in 0..14 {
                put_float(sim.ptr(), 0x67C + i * 4, -1.0);
            }
        }
        let mut w = Self {
            sim,
            scan,
            peaks,
            ratios,
            commercial,
            volumes,
            switches,
            infos,
            pacing,
            flags,
            volume_objects,
            nodes: vec![],
            constants,
            _guards: guards,
            _tables: tables,
        };
        unsafe {
            CreateMapsSim::from_ptr(w.sim.ptr()).set_cursor(w.scan.ptr());
            CreateMapsSim::from_ptr(w.sim.ptr()).set_network_rows(w.infos.rows());
        }
        w.scan(1, 1, 1, 1);
        w
    }
    fn scan(&mut self, x: i32, z: i32, end_x: i32, end_z: i32) {
        unsafe {
            let s = PassScan::from_ptr(self.scan.ptr());
            s.set_state(0);
            s.set_x(x);
            s.set_z(z);
            s.set_step(1);
            s.set_minimum_x(x);
            s.set_maximum_x(end_x);
            s.set_maximum_z(end_z);
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn add(&mut self, x: usize, z: usize, network: u8, slot: u8, inverse: f32, bridge: bool, connections: u8) {
        let mut info = Object::new(24);
        unsafe {
            let e = MapNetworkInfo::from_ptr(info.ptr());
            e.set_inverse_capacity(inverse);
            e.set_connections(connections);
            e.set_statistics_slot(slot);
            e.set_network(network);
            e.set_flags(u8::from(bridge));
            e.set_next(self.infos.cells[x][z] as *mut u8);
        }
        self.infos.cells[x][z] = info.ptr() as u32;
        self.nodes.push(info);
    }
    fn switch(&mut self, x: usize, z: usize, id: u32, load: f32, capacity: f32, mask: u8) -> *mut u8 {
        let mut sw = Object::new(48);
        let p = sw.ptr();
        unsafe {
            put_float(p, 4, load);
            put_float(p, 12, capacity);
            p.add(16).write(mask);
        }
        STATE.with(|s| s.borrow_mut().switches.push((id, p as u32)));
        self.nodes.push(sw);
        self.switches.cells[x][z] = id;
        p
    }
    fn run(&mut self) -> (Registers, u32) {
        let mut deadline = [0x12345678u32, 0x87654321];
        let mut frame = [
            fc4p_0071a0c0 as *const () as u32,
            self.sim.ptr() as u32,
            deadline.as_mut_ptr() as u32,
            0,
            0,
            0,
            0,
        ];
        unsafe {
            core::arch::asm!("push dword ptr [edi + 8]", "mov ecx, [edi + 4]", "call dword ptr [edi]",
                "mov [edi + 12], eax", "mov [edi + 16], ecx", "mov [edi + 20], edx",
                "mov eax, [esp - 4]", "mov [edi + 24], eax", in("edi") frame.as_mut_ptr(), clobber_abi("C"));
        }
        let result = Registers {
            eax: frame[3],
            ecx: frame[4],
            edx: frame[5],
        };
        // Keep all independently allocated storage alive through the call.
        self.peaks.padding_untouched();
        self.ratios.padding_untouched();
        self.commercial.padding_untouched();
        assert_eq!(self.volume_objects.len(), 6);
        assert_eq!(self.constants[3], 1.0);
        (result, frame[6])
    }
}

#[test]
fn statistics_slots_bridge_background_and_publication_match_the_core_examples() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    w.scan(1, 1, 4, 1);
    w.add(1, 1, 1, 0, 0.5, false, 3);
    w.add(1, 1, 0, 0, 0.5, false, 3);
    w.add(2, 1, 1, 0, 0.25, true, 3);
    w.add(3, 1, 1, 3, 1.0, false, 3);
    w.add(4, 1, 1, 10, 1.0, false, 3);
    w.volumes[1].cells[1][1] = 4.0;
    w.volumes[0].cells[1][1] = 1.0;
    w.volumes[1].cells[2][1] = 8.0;
    w.volumes[1].cells[3][1] = 0.5;
    unsafe {
        store(w.sim.ptr(), 0x130, 0x12345678);
        store(w.sim.ptr(), 0x134, 0x12345690);
    }
    let environment = float_environment();
    assert_eq!(w.run().0.eax as u8, 1);
    assert_eq!(float_environment(), environment);
    unsafe {
        assert_eq!(
            read_unaligned(w.sim.ptr().add(0x1E4).cast::<[u32; 5]>()),
            [1.25f32.to_bits(), 1, 1, 1, 2.0f32.to_bits()]
        );
        assert_eq!(float(w.sim.ptr(), 0x1E4 + 3 * 20), 0.5);
        assert_eq!(
            read_unaligned(w.sim.ptr().add(0x2E8).cast::<[u32; 5]>()),
            [2.0f32.to_bits(), 1, 2, 1, 2.0f32.to_bits()]
        );
        assert_eq!(word(w.sim.ptr(), 0x740), 2);
        assert_eq!(word(w.sim.ptr(), 0x74C), 1);
        assert_eq!(word(w.sim.ptr(), 0x768), 1);
        assert_eq!(word(w.sim.ptr(), 0x134), 0x12345678);
    }
    assert_eq!((w.peaks.cells[1][1], w.ratios.cells[1][1]), (4.0, 2.0));
    assert_eq!((w.peaks.cells[4][1], w.ratios.cells[4][1]), (1.0, 0.0));
    assert_eq!(w.volumes[1].cells[1][1], 4.0);
    assert_eq!(w.volumes[0].cells[1][1], 0.25);
    let actual = calls();
    assert_eq!(
        &actual[..12],
        &[
            Call::Grid(5),
            Call::Grid(2),
            Call::Grid(3),
            Call::Grid(4),
            Call::Grid(5),
            Call::Grid(6),
            Call::Grid(7),
            Call::Grid(8),
            Call::Grid(9),
            Call::Grid(10),
            Call::Grid(11),
            Call::Size
        ]
    );
    assert_eq!(
        &actual[actual.len() - 3..],
        &[
            Call::Roads(w.sim.ptr() as u32),
            Call::Transit(w.sim.ptr() as u32),
            Call::Assign(w.sim.ptr() as u32 + 0x13C, w.sim.ptr() as u32 + 0x130)
        ]
    );
}

#[test]
fn commercial_rounding_ties_signed_clamp_and_low_word_of_window() {
    let _fpu = GameFpu::new();
    for (window, scale, expected) in [
        (880, 0.128, 113),
        (1, 2.5, 2),
        (1, 3.5, 4),
        (1000, 1.0, 255),
        (1, -2.0, 254),
        (0x10001, 4.0, 4),
    ] {
        let mut w = World::new();
        w.scan(0, 0, 0, 0);
        STATE.with(|s| s.borrow_mut().window = window);
        unsafe {
            CreateMapsSim::from_ptr(w.sim.ptr()).set_commercial_scale(scale);
        }
        assert_eq!(w.run().0.eax as u8, 1);
        assert_eq!(w.commercial.cells[0][0], expected);
        assert!(calls().contains(&Call::Window(w.sim.ptr() as u32, 0, 0)));
    }
}

#[test]
fn rail_and_subway_ratios_count_but_do_not_raise_peak_volume_and_accidents_keep_call_order() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    w.add(1, 1, 1, 0, 0.5, false, 4);
    w.add(1, 1, 2, 1, 0.25, false, 5);
    w.add(1, 1, 3, 7, 1.0, false, 8);
    w.volumes[1].cells[1][1] = 2.0;
    w.volumes[2].cells[1][1] = 12.0;
    w.volumes[3].cells[1][1] = 20.0;
    assert_eq!(w.run().0.eax as u8, 1);
    assert_eq!((w.peaks.cells[1][1], w.ratios.cells[1][1]), (2.0, 20.0));
    let actual: Vec<_> = calls()
        .into_iter()
        .filter(|c| matches!(c, Call::Curve(..) | Call::Accident(..) | Call::Background(..)))
        .collect();
    let p = w.sim.ptr() as u32;
    assert_eq!(
        actual,
        [
            Call::Curve(p + 0xD30, 1.0f32.to_bits()),
            Call::Curve(p + 0xD40, 2.0f32.to_bits()),
            Call::Accident(p, 1, 1, 0.125f32.to_bits()),
            Call::Background(p, 1, 1),
            Call::Curve(p + 0xD30, 3.0f32.to_bits()),
            Call::Curve(p + 0xD40, 4.0f32.to_bits()),
            Call::Accident(p, 1, 1, 0.125f32.to_bits())
        ]
    );
}

#[test]
fn switch_only_cell_has_volume_floor_ratio_cap_and_smaller_statistic_selection() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    let sw = w.switch(1, 1, 7, 3.0, 4.0, 0x81);
    unsafe {
        put_float(w.sim.ptr(), 0x588, 1.0);
        put_float(w.sim.ptr(), 0x588 + 7 * 20, 0.5);
    }
    assert_eq!(w.run().0.eax as u8, 1);
    assert_eq!((w.peaks.cells[1][1], w.ratios.cells[1][1]), (1.0, 0.75));
    unsafe {
        assert_eq!((float(sw, 0x14), float(sw, 0x18)), (3.0, 0.75));
        assert_eq!(float(w.sim.ptr(), 0x32C), 0.75);
        assert_eq!(word(w.sim.ptr(), 0x324), 1);
        assert_eq!(word(w.sim.ptr(), 0x328), 1);
        assert_eq!(float(w.sim.ptr(), 0x32C + 7 * 20), 0.5);
        assert_eq!(read_unaligned(w.sim.ptr().add(0x318).cast::<u16>()), 0);
    }
    assert!(calls().contains(&Call::Switch(w.sim.ptr() as u32 + 0x70C, 7)));
}

#[test]
fn deadline_stops_before_a_cell_resumes_and_never_publishes_partial_statistics() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    w.scan(1, 1, 3, 1);
    *w.flags = 0xA0;
    *w.pacing = [7, 8];
    STATE.with(|s| s.borrow_mut().expire = 2);
    let before = unsafe { bytes(w.sim.ptr(), 0xD50) };
    assert_eq!(w.run().0.eax as u8, 0);
    assert_eq!(*w.flags, 0xA1);
    assert_eq!(*w.pacing, [1024, u32::MAX]);
    unsafe {
        assert_eq!(PassScan::from_ptr(w.scan.ptr()).x(), 1);
        assert_eq!(bytes(w.sim.ptr(), 0xD50), before);
    }
    assert_eq!(w.peaks.cells[1][1], 0.0);
    assert_eq!(w.peaks.cells[2][1], 99.0);
    assert!(!calls().iter().any(|c| matches!(c, Call::Roads(..) | Call::Assign(..))));
    STATE.with(|s| s.borrow_mut().expire = 0);
    assert_eq!(w.run().0.eax as u8, 1);
    assert_eq!(w.peaks.cells[3][1], 0.0);
    unsafe {
        let s = PassScan::from_ptr(w.scan.ptr());
        assert_eq!((s.state(), s.x(), s.z()), (2, 1, 2));
    }
}

#[test]
fn pacing_doubles_in_ecx_on_expiration_and_eax_on_completion_and_halves_after_slow_streak() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    *w.pacing = [512, (-3i32) as u32];
    STATE.with(|s| s.borrow_mut().expire = 1);
    let r = w.run().0;
    assert_eq!((r.eax, r.ecx), (0x200, 1024));
    assert_eq!(*w.pacing, [1024, 0]);
    let mut w = World::new();
    *w.pacing = [512, (-3i32) as u32];
    let r = w.run().0;
    assert_eq!((r.eax, r.ecx), (0x401, 0));
    assert_eq!(*w.pacing, [1024, 0]);
    let mut w = World::new();
    *w.pacing = [32768, 15];
    STATE.with(|s| s.borrow_mut().expire = 1);
    assert_eq!(w.run().0.eax, 0x4000);
    assert_eq!(*w.pacing, [16384, 0]);
}

#[test]
fn nan_ratios_preserve_maximum_coordinates_and_zero_count_rows_are_copied_verbatim() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    w.add(1, 1, 0, 0, 1.0, false, 3);
    w.volumes[0].cells[1][1] = f32::NAN;
    unsafe {
        store(w.sim.ptr(), 0x440 + 5 * 20 + 8, 0xDEADBEEF);
    }
    assert_eq!(w.run().0.eax as u8, 1);
    assert_eq!((w.peaks.cells[1][1], w.ratios.cells[1][1]), (1.0, 0.0));
    unsafe {
        assert!(float(w.sim.ptr(), 0x1E4).is_nan());
        assert_eq!(word(w.sim.ptr(), 0x1EC), 0);
        assert_eq!(word(w.sim.ptr(), 0x1E4 + 5 * 20 + 8), 0xDEADBEEF);
    }
}

#[test]
fn finished_cursor_publishes_unsigned_counts_and_retains_padding_without_visiting_cells() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    unsafe {
        PassScan::from_ptr(w.scan.ptr()).set_state(2);
        store(w.sim.ptr(), 0x6B4, 0x80000001);
        put_float(w.sim.ptr(), 0x440, 64.0);
        store(w.sim.ptr(), 0x578, 0xABCD0003);
        put_float(w.sim.ptr(), 0x57C, 1.5);
    }
    let (r, scratch) = w.run();
    assert_eq!(r.eax as u8, 1);
    assert_eq!(scratch, 3);
    unsafe {
        assert_eq!(word(w.sim.ptr(), 0x1E4), 0x33000000);
        assert_eq!(word(w.sim.ptr(), 0x31C), 0xABCD0003);
        assert_eq!(float(w.sim.ptr(), 0x320), 0.5);
    }
    assert_eq!(w.peaks.cells[1][1], 99.0);
}

#[test]
fn counters_wrap_at_their_own_widths_and_accident_zero_or_nan_probabilities_stop_the_chain() {
    let _fpu = GameFpu::new();
    for probability in [0.0, -1.0, f32::NAN] {
        let mut w = World::new();
        w.add(1, 1, 1, 0, 0.5, false, 4);
        w.volumes[1].cells[1][1] = 4.0;
        STATE.with(|s| s.borrow_mut().probabilities[0] = probability);
        unsafe {
            store(w.sim.ptr(), 0x6B4, u32::MAX);
            store(w.sim.ptr(), 0x444, u32::MAX);
            write_unaligned(w.sim.ptr().add(0x558).cast::<u16>(), u16::MAX);
        }
        assert_eq!(w.run().0.eax as u8, 1);
        unsafe {
            assert_eq!(word(w.sim.ptr(), 0x6B4), 0);
            assert_eq!(word(w.sim.ptr(), 0x444), 0);
            assert_eq!(word(w.sim.ptr(), 0x740), 0);
            assert_eq!(float(w.sim.ptr(), 0x1E4), 2.0);
        }
        let actual = calls();
        assert_eq!(actual.iter().filter(|c| matches!(c, Call::Curve(..))).count(), 1);
        assert!(!actual.iter().any(|c| matches!(c, Call::Accident(..))));
    }
}

#[test]
fn map_pass_preserves_an_existing_x87_value_and_only_publishes_the_original_field_spans() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    let simulator_before = unsafe { bytes(w.sim.ptr(), 0xD50) };
    let saved = 123.25f64;
    unsafe {
        core::arch::asm!("fld qword ptr [{value}]", value=in(reg) &saved, out("st(0)") _);
    }
    let before = float_environment();
    assert_eq!(w.run().0.eax as u8, 1);
    assert_eq!(float_environment(), before);
    let mut after = 0.0f64;
    unsafe {
        core::arch::asm!("fstp qword ptr [{value}]", value=in(reg) &mut after, out("st(0)") _);
    }
    assert_eq!(after.to_bits(), saved.to_bits());
    assert_eq!(unsafe { bytes(w.sim.ptr(), 0xD50) }, simulator_before);
}

#[test]
fn accident_curve_keeps_a_positive_st0_value_that_rounds_to_zero_when_stored() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    w.add(1, 1, 1, 0, 0.5, false, 4);
    w.volumes[1].cells[1][1] = 4.0;
    STATE.with(|s| s.borrow_mut().wide_probability = Some(1e-300));
    assert_eq!(w.run().0.eax as u8, 1);
    let actual = calls();
    assert_eq!(actual.iter().filter(|c| matches!(c, Call::Curve(..))).count(), 2);
    assert!(!actual.iter().any(|c| matches!(c, Call::Accident(..))));
}
