// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, fake_data, GameFpu, Object};
use std::cell::RefCell;

unsafe fn fc4p_004afb80(this: *mut u8, network: *mut u8, intersection: *mut u8, elapsed: f32) {
    let f: unsafe extern "thiscall" fn(*mut u8, *mut u8, *mut u8, f32) =
        unsafe { core::mem::transmute(super::fc4p_004afb80 as *const ()) };
    unsafe {
        f(this, network, intersection, elapsed);
    }
}

unsafe fn fc4p_004afc20(this: *mut u8, network: *mut u8, node: *mut u8, unused: u32) {
    let f: unsafe extern "thiscall" fn(*mut u8, *mut u8, *mut u8, u32) =
        unsafe { core::mem::transmute(super::fc4p_004afc20 as *const ()) };
    unsafe {
        f(this, network, node, unused);
    }
}

#[derive(Debug, PartialEq)]
enum Call {
    Interface(usize),
    Neighbor(usize, usize, u32, u32, u32),
    Edge(usize, usize, usize, usize, usize, u32),
    Props(usize, usize, usize, u32),
}

#[derive(Default)]
struct State {
    calls: Vec<Call>,
    interface: usize,
    neighbors: [usize; 4],
    clear_dirty: bool,
    map: usize,
}

thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }

fn reset() {
    STATE.with(|s| *s.borrow_mut() = State::default());
}
fn calls() -> Vec<Call> {
    STATE.with(|s| std::mem::take(&mut s.borrow_mut().calls))
}

extern "thiscall" fn props(this: *mut u8, network: *mut u8, intersection: *mut u8, initial: u32) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Props(
            this as usize,
            network as usize,
            intersection as usize,
            initial,
        ));
        assert_ne!(unsafe { Intersection::from_ptr(intersection) }.dirty(), 0);
        if initial == 1 {
            assert_eq!(unsafe { Intersection::from_ptr(intersection) }.active(), 0);
            assert_eq!(
                unsafe { TrafficNetworkMap::from_ptr(s.map as *mut u8) }.intersections_changed(),
                0
            );
        }
        if s.clear_dirty {
            unsafe { Intersection::from_ptr(intersection) }.set_dirty(0);
        }
    });
}

extern "thiscall" fn interface(this: *mut u8) -> *mut u8 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Interface(this as usize));
        s.interface as *mut u8
    })
}

extern "thiscall" fn neighbor(this: *mut u8, network: *mut u8, direction: u32, a: u32, b: u32) -> *mut u8 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls
            .push(Call::Neighbor(this as usize, network as usize, direction, a, b));
        s.neighbors[direction as u8 as usize] as *mut u8
    })
}

extern "thiscall" fn configure(
    this: *mut u8,
    network: *mut u8,
    neighbor: *mut u8,
    intersection: *mut u8,
    edge: *mut u8,
    direction: u32,
) {
    STATE.with(|s| {
        s.borrow_mut().calls.push(Call::Edge(
            this as usize,
            network as usize,
            neighbor as usize,
            intersection as usize,
            edge as usize,
            direction,
        ))
    });
    unsafe { IntersectionEdge::from_ptr(edge) }.set_kind(u32::from(direction as u8) + 10);
}

fn edge(ptr: *mut u8, direction: usize, kind: u32, green: u8, warning: u8, remaining: f32) {
    let edge = unsafe { IntersectionEdge::from_ptr(ptr.wrapping_add(direction * 20)) };
    edge.set_kind(kind);
    edge.set_green(green);
    edge.set_warning(warning);
    edge.set_green_duration(9.0f32.to_bits());
    edge.set_red_duration(11.0f32.to_bits());
    edge.set_remaining(remaining);
}

#[test]
fn tick_changes_only_timers_and_transition_bytes_and_calls_props_once() {
    reset();
    let _fpu = GameFpu::new();
    let _props = fake(0x004A_E850, props as *const () as usize);
    let mut zero = 0.0f32;
    let mut threshold = 0.75f32;
    let _zero = fake_data(0x00A8_1054, (&mut zero as *mut f32).cast());
    let _threshold = fake_data(0x00A8_FDD4, (&mut threshold as *mut f32).cast());
    let mut mem = Object::new(0x70);
    let p = mem.ptr();
    unsafe {
        core::ptr::write_bytes(p, 0xA5, 0x70);
    }
    edge(p, 0, 2, 0, 7, 0.25);
    edge(p, 1, 2, 4, 3, 0.0);
    edge(p, 2, 2, 1, 0, 1.0);
    edge(p, 3, 3, 8, 9, 123.0);
    unsafe { Intersection::from_ptr(p) }.set_dirty(0);
    let mut expected = unsafe { core::slice::from_raw_parts(p, 0x70) }.to_vec();
    expected[4] = 1;
    expected[5] = 0;
    expected[16..20].copy_from_slice(&9.0f32.to_bits().to_le_bytes());
    expected[24] = 0;
    expected[25] = 0;
    expected[36..40].copy_from_slice(&11.0f32.to_bits().to_le_bytes());
    expected[45] = 1;
    expected[56..60].copy_from_slice(&0.5f32.to_bits().to_le_bytes());
    expected[0x64] = 1;
    let map = 0x1234usize as *mut u8;
    let network = 0x5678usize as *mut u8;
    let (_, _, arguments) = unsafe {
        crate::traffic_network::stack_arguments(
            super::fc4p_004afb80 as *const () as usize,
            map,
            [network as u32, p as u32, 0.5f32.to_bits()],
            false,
        )
    };
    assert_eq!(arguments, [network as u32, 0.5f32.to_bits(), 0.5f32.to_bits()]);
    assert_eq!(unsafe { core::slice::from_raw_parts(p, 0x70) }, expected);
    assert_eq!(calls(), [Call::Props(map as usize, network as usize, p as usize, 0)]);
}

#[test]
fn tick_zero_warning_threshold_nan_infinity_and_existing_warning() {
    reset();
    let _fpu = GameFpu::new();
    let _props = fake(0x004A_E850, props as *const () as usize);
    let mut zero = 0.0f32;
    let mut threshold = 0.75f32;
    let _zero = fake_data(0x00A8_1054, (&mut zero as *mut f32).cast());
    let _threshold = fake_data(0x00A8_FDD4, (&mut threshold as *mut f32).cast());
    let mut mem = Object::new(0x70);
    let p = mem.ptr();
    for (remaining, elapsed, warning, expected_timer, expected_warning, dirty) in [
        (0.5, 0.5, 0, 0.0, 1, true),
        (1.25, 0.5, 0, 0.75, 0, false),
        (0.75, 0.5, 7, 0.25, 7, false),
        (f32::INFINITY, 0.5, 0, f32::INFINITY, 0, false),
        (f32::NEG_INFINITY, 0.5, 0, 9.0, 0, true),
        (f32::NAN, 0.5, 0, f32::NAN, 0, false),
        (f32::INFINITY, f32::INFINITY, 0, f32::NAN, 0, false),
        (-0.0, 0.0, 0, -0.0, 1, true),
    ] {
        edge(p, 0, 2, 0, warning, remaining);
        unsafe { Intersection::from_ptr(p) }.set_dirty(0);
        unsafe {
            fc4p_004afb80(core::ptr::null_mut(), core::ptr::null_mut(), p, elapsed);
        }
        let e = unsafe { IntersectionEdge::from_ptr(p) };
        if expected_timer.is_nan() {
            assert!(e.remaining().is_nan());
        } else {
            assert_eq!(e.remaining().to_bits(), expected_timer.to_bits());
        }
        assert_eq!(e.warning(), expected_warning);
        assert_eq!(unsafe { Intersection::from_ptr(p) }.dirty(), u8::from(dirty));
        assert_eq!(calls().len(), usize::from(dirty));
    }
}

#[test]
fn tick_retains_dirty_and_skips_non_stoplight_edges() {
    reset();
    let _props = fake(0x004A_E850, props as *const () as usize);
    let mut mem = Object::new(0x70);
    let p = mem.ptr();
    for (i, kind) in [0, 1, 3, u32::MAX].into_iter().enumerate() {
        edge(p, i, kind, 0xA5, 0x5A, -5.0);
    }
    let before = unsafe { core::slice::from_raw_parts(p, 0x70) }.to_vec();
    unsafe {
        fc4p_004afb80(core::ptr::null_mut(), core::ptr::null_mut(), p, f32::NAN);
    }
    assert_eq!(unsafe { core::slice::from_raw_parts(p, 0x70) }, before);
    assert!(calls().is_empty());
    unsafe { Intersection::from_ptr(p) }.set_dirty(9);
    STATE.with(|s| s.borrow_mut().clear_dirty = true);
    unsafe {
        fc4p_004afb80(core::ptr::null_mut(), core::ptr::null_mut(), p, 0.0);
    }
    assert_eq!(calls().len(), 1);
    assert_eq!(unsafe { Intersection::from_ptr(p) }.dirty(), 0);
}

#[test]
fn initialize_intersection_checks_interface_then_configures_all_edges_in_order() {
    reset();
    let _configure = fake(0x004A_E9E0, configure as *const () as usize);
    let _props = fake(0x004A_E850, props as *const () as usize);
    let mut map_vt = [0usize; 0x40 / 4];
    map_vt[0x3C / 4] = neighbor as *const () as usize;
    let mut network_vt = [0usize; 0xC8 / 4];
    network_vt[0xC4 / 4] = interface as *const () as usize;
    let mut map = Object::new(0x80);
    let mut network = Object::new(4);
    let mut node = Object::new(0x78);
    let mut tile0 = Object::new(0x58);
    let mut tile2 = Object::new(0x58);
    unsafe {
        TrafficNetworkMap::from_ptr(map.ptr()).set_vtable(map_vt.as_ptr());
        network.ptr().cast::<*const usize>().write(network_vt.as_ptr());
        TileInfo::from_ptr(tile0.ptr()).set_network(0x1234usize as *mut u8);
        TileInfo::from_ptr(tile2.ptr()).set_network(0x5678usize as *mut u8);
        core::ptr::write_bytes(node.ptr(), 0xA5, 0x78);
    }
    let before = unsafe { core::slice::from_raw_parts(node.ptr(), 0x78) }.to_vec();
    unsafe {
        fc4p_004afc20(map.ptr(), network.ptr(), node.ptr(), 0xFFFF_FFFF);
    }
    assert_eq!(calls(), [Call::Interface(network.ptr() as usize)]);
    assert_eq!(unsafe { core::slice::from_raw_parts(node.ptr(), 0x78) }, before);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.interface = 0xDEAD;
        s.neighbors = [tile0.ptr() as usize, 0, tile2.ptr() as usize, 0];
        s.map = map.ptr() as usize;
        s.clear_dirty = true;
    });
    let (_, _, arguments) = unsafe {
        crate::traffic_network::stack_arguments(
            super::fc4p_004afc20 as *const () as usize,
            map.ptr(),
            [network.ptr() as u32, node.ptr() as u32, 0xCAFE_BABE],
            false,
        )
    };
    assert_eq!(
        arguments,
        [
            (network.ptr() as u32 & !0xFF) | 4,
            node.ptr().wrapping_add(8 + 80) as u32,
            0xCAFE_BABE
        ]
    );
    let p = node.ptr().wrapping_add(8);
    let mut expected_calls = vec![Call::Interface(network.ptr() as usize)];
    for (i, neighbor) in [0x1234, 0, 0x5678, 0].into_iter().enumerate() {
        expected_calls.push(Call::Neighbor(
            map.ptr() as usize,
            network.ptr() as usize,
            (network.ptr() as u32 & !0xFF) | i as u32,
            0,
            0,
        ));
        expected_calls.push(Call::Edge(
            map.ptr() as usize,
            network.ptr() as usize,
            neighbor,
            p as usize,
            p.wrapping_add(i * 20) as usize,
            (network.ptr() as u32 & !0xFF) | i as u32,
        ));
    }
    expected_calls.push(Call::Props(map.ptr() as usize, network.ptr() as usize, p as usize, 1));
    assert_eq!(calls(), expected_calls);
    let mut expected = before;
    for i in 0..4 {
        expected[8 + i * 20..12 + i * 20].copy_from_slice(&(i as u32 + 10).to_le_bytes());
    }
    expected[8 + 0x50] = 0;
    expected[8 + 0x64] = 0;
    assert_eq!(unsafe { core::slice::from_raw_parts(node.ptr(), 0x78) }, expected);
    assert_eq!(
        unsafe { TrafficNetworkMap::from_ptr(map.ptr()) }.intersections_changed(),
        1
    );
}

#[test]
fn tick_preserves_x87_stack_control_word_mxcsr_and_direction_flag() {
    reset();
    let _fpu = GameFpu::new();
    let mut zero = 0f32;
    let mut threshold = 0.75f32;
    let _zero = fake_data(0x00A8_1054, (&mut zero as *mut f32).cast());
    let _threshold = fake_data(0x00A8_FDD4, (&mut threshold as *mut f32).cast());
    let mut mem = Object::new(0x70);
    edge(mem.ptr(), 0, 2, 0, 0, 10.0);
    let sentinel = 123.25f64;
    let mut restored = 0f64;
    let mut before = [0u32; 3];
    let mut after = [0u32; 3];
    unsafe {
        core::arch::asm!("fld qword ptr [{p}]", p = in(reg) &sentinel, out("st(0)") _);
        core::arch::asm!("fnstcw word ptr [{p}]", "stmxcsr dword ptr [{p} + 4]", "pushfd", "pop dword ptr [{p} + 8]", p = in(reg) before.as_mut_ptr());
        fc4p_004afb80(core::ptr::null_mut(), core::ptr::null_mut(), mem.ptr(), 0.5);
        core::arch::asm!("fnstcw word ptr [{p}]", "stmxcsr dword ptr [{p} + 4]", "pushfd", "pop dword ptr [{p} + 8]", p = in(reg) after.as_mut_ptr());
        core::arch::asm!("fstp qword ptr [{p}]", p = in(reg) &mut restored, out("st(0)") _);
    }
    assert_eq!(before[0..2], after[0..2]);
    assert_eq!(before[2] & 0x400, after[2] & 0x400);
    assert_eq!(restored, sentinel);
}

mod prop_updates {
    use super::*;
    use crate::testing::Fake;

    type Logged = (usize, usize, Vec<u32>);
    thread_local! { static LOG: RefCell<Vec<Logged>> = const { RefCell::new(Vec::new()) }; }
    fn log(this: *mut Actor, slot: usize, args: &[u32]) {
        LOG.with(|l| l.borrow_mut().push((this as usize, slot, args.to_vec())));
    }
    fn take() -> Vec<Logged> {
        LOG.with(|l| std::mem::take(&mut *l.borrow_mut()))
    }

    #[repr(C)]
    struct Actor {
        vtable: *const usize,
        occupant: *mut Actor,
        descriptor: *mut Actor,
        flag2: u32,
        flag3: u32,
        direct: u32,
        valid: u32,
        orientation: u32,
        state: u32,
        replace: *mut Actor,
        end_slot: *mut *mut *mut u8,
        next_end: *mut *mut u8,
        mutate_edge: *mut u8,
    }

    fn actor(vt: &[usize]) -> Box<Actor> {
        Box::new(Actor {
            vtable: vt.as_ptr(),
            occupant: core::ptr::null_mut(),
            descriptor: core::ptr::null_mut(),
            flag2: 1,
            flag3: 0,
            direct: 1,
            valid: 1,
            orientation: 0,
            state: 0,
            replace: core::ptr::null_mut(),
            end_slot: core::ptr::null_mut(),
            next_end: core::ptr::null_mut(),
            mutate_edge: core::ptr::null_mut(),
        })
    }

    extern "thiscall" fn city(this: *mut Actor) {
        log(this, 0x88, &[]);
        unsafe {
            if !(*this).mutate_edge.is_null() {
                IntersectionEdge::from_ptr((*this).mutate_edge).set_green(1);
            }
        }
    }
    extern "thiscall" fn occupant(this: *mut Actor) -> *mut Actor {
        log(this, 0x0C, &[]);
        unsafe {
            let old = (*this).occupant;
            if !(*this).replace.is_null() {
                (*this).occupant = (*this).replace;
            }
            old
        }
    }
    extern "thiscall" fn descriptor(this: *mut Actor) -> *mut Actor {
        log(this, 0x18, &[]);
        unsafe { (*this).descriptor }
    }
    extern "thiscall" fn flags(this: *mut Actor, flag: u32) -> u32 {
        log(this, 0x58, &[flag]);
        unsafe {
            if flag == 0x5002 {
                (*this).flag2
            } else {
                (*this).flag3
            }
        }
    }
    extern "thiscall" fn valid(this: *mut Actor, id: u32) -> u32 {
        log(this, 0x0C, &[id]);
        unsafe { (*this).valid }
    }
    extern "thiscall" fn direct(this: *mut Actor, flag: u32) -> u32 {
        log(this, 0x80, &[flag]);
        unsafe { (*this).direct }
    }
    extern "thiscall" fn orientation(this: *mut Actor) -> u32 {
        log(this, 0x24, &[]);
        unsafe { (*this).orientation }
    }
    extern "thiscall" fn state(this: *mut Actor) -> u32 {
        log(this, 0x30, &[]);
        unsafe { (*this).state }
    }
    extern "thiscall" fn set_state(this: *mut Actor, value: u32) {
        log(this, 0x2C, &[value]);
        unsafe {
            (*this).state = value;
            if !(*this).end_slot.is_null() {
                (*this).end_slot.write((*this).next_end);
            }
        }
    }

    struct World {
        memory: Object,
        props: Vec<*mut u8>,
        city: Box<Actor>,
        prop: Box<Actor>,
        occupant: Box<Actor>,
        descriptor: Box<Actor>,
        _tables: [Box<[usize; 0x8C / 4]>; 4],
        _city_location: Box<*mut u8>,
        _warning: Box<f32>,
        _globals: [Fake; 2],
    }

    impl World {
        fn new() -> Self {
            take();
            let mut tables = core::array::from_fn(|_| Box::new([0; 0x8C / 4]));
            tables[0][0x88 / 4] = city as *const () as usize;
            tables[1][0x0C / 4] = occupant as *const () as usize;
            tables[1][0x24 / 4] = orientation as *const () as usize;
            tables[1][0x2C / 4] = set_state as *const () as usize;
            tables[1][0x30 / 4] = state as *const () as usize;
            tables[2][0x18 / 4] = descriptor as *const () as usize;
            tables[2][0x58 / 4] = flags as *const () as usize;
            tables[2][0x80 / 4] = direct as *const () as usize;
            tables[3][0x0C / 4] = valid as *const () as usize;
            let mut city = actor(&*tables[0]);
            let mut prop = actor(&*tables[1]);
            let mut occupant = actor(&*tables[2]);
            let mut descriptor = actor(&*tables[3]);
            occupant.descriptor = &mut *descriptor;
            prop.occupant = &mut *occupant;
            let mut city_location = Box::new((&mut *city as *mut Actor).cast::<u8>());
            let mut warning = Box::new(0.75f32);
            let globals = [
                fake_data(0x00B4_3CEC, (&mut *city_location as *mut *mut u8).cast()),
                fake_data(0x00A8_FDD4, (&mut *warning as *mut f32).cast()),
            ];
            let mut memory = Object::new(256 * 20);
            let mut props = vec![(&mut *prop as *mut Actor).cast::<u8>()];
            let intersection = unsafe { Intersection::from_ptr(memory.ptr()) };
            intersection.set_props_begin(props.as_mut_ptr());
            intersection.set_props_end(props.as_mut_ptr().wrapping_add(1));
            Self {
                memory,
                props,
                city,
                prop,
                occupant,
                descriptor,
                _tables: tables,
                _city_location: city_location,
                _warning: warning,
                _globals: globals,
            }
        }

        fn run(&mut self, initial: u32) -> Vec<Logged> {
            unsafe { Intersection::from_ptr(self.memory.ptr()) }.set_dirty(1);
            let (_, _, arguments) = unsafe {
                crate::traffic_network::stack_arguments(
                    fc4p_004ae850 as *const () as usize,
                    0x1234usize as *mut u8,
                    [0x5678, self.memory.ptr() as u32, initial],
                    false,
                )
            };
            assert_eq!(
                arguments,
                [
                    0x5678,
                    unsafe { Intersection::from_ptr(self.memory.ptr()) }.props_end() as u32,
                    initial
                ]
            );
            assert_eq!(unsafe { Intersection::from_ptr(self.memory.ptr()) }.dirty(), 0);
            take()
        }
    }

    #[test]
    fn stoplight_states_orientations_and_low_byte_flags() {
        let _fpu = GameFpu::new();
        let mut w = World::new();
        let prop = &*w.prop as *const Actor as usize;
        let occ = &*w.occupant as *const Actor as usize;
        let desc = &*w.descriptor as *const Actor as usize;
        let city = &*w.city as *const Actor as usize;
        w.occupant.flag2 = 0x1234_5680;
        w.descriptor.valid = 0xABCD_EF01;
        for (orientation, direct, direction) in [
            (0, 1, 0),
            (1, 0, 3),
            (3, 0, 1),
            (0xABCD_0002, 0x100, 0),
            (0xFFFF_FFFF, 0, 1),
            (0xFFFF_FFFD, 0, 255),
        ] {
            w.prop.orientation = orientation;
            w.occupant.direct = direct;
            let p = w.memory.ptr().wrapping_add(direction * 20);
            for (green, remaining, expected) in [(0, 100.0, 2), (7, 0.5, 1), (1, 0.75, 0), (1, f32::NAN, 0)] {
                edge(p, 0, 2, green, 0, remaining);
                assert_eq!(
                    w.run(0),
                    vec![
                        (city, 0x88, vec![]),
                        (prop, 0x0C, vec![]),
                        (occ, 0x18, vec![]),
                        (occ, 0x58, vec![0x5002]),
                        (desc, 0x0C, vec![0x4A70_D491]),
                        (prop, 0x0C, vec![]),
                        (occ, 0x80, vec![0x80]),
                        (prop, 0x24, vec![]),
                        (prop, 0x2C, vec![expected])
                    ]
                );
            }
            unsafe { IntersectionEdge::from_ptr(p) }.set_kind(1);
            assert_eq!(w.run(0).len(), 8);
        }
    }

    #[test]
    fn crossing_states_skip_existing_pairs_and_only_read_low_initial_byte() {
        let mut w = World::new();
        w.occupant.flag2 = 0x100;
        w.occupant.flag3 = 0xAABB_CC01;
        let prop = &*w.prop as *const Actor as usize;
        let occ = &*w.occupant as *const Actor as usize;
        let city = &*w.city as *const Actor as usize;
        for train in [false, true] {
            edge(w.memory.ptr(), 2, 3, u8::from(!train), 0, 0.0);
            for initial in [0, 1, 0x100, 0xFF] {
                for old in 0..=4 {
                    w.prop.state = old;
                    let mut expected = vec![
                        (city, 0x88, vec![]),
                        (prop, 0x0C, vec![]),
                        (occ, 0x18, vec![]),
                        (occ, 0x58, vec![0x5002]),
                        (prop, 0x0C, vec![]),
                        (occ, 0x58, vec![0x5003]),
                        (prop, 0x30, vec![]),
                    ];
                    if train && old != 0 && old != 3 {
                        expected.push((prop, 0x2C, vec![if initial as u8 != 0 { 0 } else { 3 }]));
                    } else if !train && old != 1 && old != 2 {
                        expected.push((prop, 0x2C, vec![u32::from(initial as u8 != 0) + 1]));
                    }
                    assert_eq!(w.run(initial), expected);
                }
            }
        }
        w.occupant.flag3 = 0xAABB_CC00;
        assert_eq!(w.run(0).len(), 6);
        w.occupant.flag2 = 1;
        w.descriptor.valid = 0x100;
        assert_eq!(w.run(0).len(), 7);
    }

    #[test]
    fn update_always_calls_city_reloads_occupant_and_end_and_keeps_initial_crossing_snapshot() {
        let mut w = World::new();
        let city = &*w.city as *const Actor as usize;
        let intersection = unsafe { Intersection::from_ptr(w.memory.ptr()) };
        intersection.set_props_end(w.props.as_mut_ptr());
        assert_eq!(w.run(0), [(city, 0x88, vec![])]);
        intersection.set_props_end(w.props.as_mut_ptr().wrapping_add(1));
        let mut other = actor(&*w._tables[2]);
        other.flag2 = 0;
        other.flag3 = 1;
        w.occupant.flag2 = 0;
        w.prop.replace = &mut *other;
        w.prop.state = 2;
        edge(w.memory.ptr(), 0, 3, 0, 0, 0.0);
        w.city.mutate_edge = w.memory.ptr();
        let result = w.run(0);
        assert_eq!(result[2].0, &*w.occupant as *const Actor as usize);
        assert_eq!(result[5], (&*other as *const Actor as usize, 0x58, vec![0x5003]));
        assert_eq!(result.last().unwrap().2, [3]);
        // The setter removes a second prop from the range; the loop must reload the end pointer.
        w.props.push(core::ptr::null_mut());
        let intersection = unsafe { Intersection::from_ptr(w.memory.ptr()) };
        intersection.set_props_begin(w.props.as_mut_ptr());
        intersection.set_props_end(w.props.as_mut_ptr().wrapping_add(2));
        w.prop.end_slot = w.memory.ptr().wrapping_add(0x58).cast();
        w.prop.next_end = w.props.as_mut_ptr().wrapping_add(1);
        w.prop.state = 0;
        w.city.mutate_edge = core::ptr::null_mut();
        edge(w.memory.ptr(), 0, 0, 0, 0, 0.0);
        let result = w.run(0);
        assert_eq!(result.last().unwrap().2, [1]);
    }
}
