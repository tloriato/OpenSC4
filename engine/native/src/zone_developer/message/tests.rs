// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, Fake, Object};
use std::cell::RefCell;

const HIGH: u64 = 0x7654_3210_0000_0000;
const ANSWER: u64 = HIGH | 0xAABB_CCDDu64;

#[derive(Debug, PartialEq)]
struct Event(u32, u32, Vec<u32>);

#[derive(Default)]
struct State {
    events: Vec<Event>,
    kind: u32,
    commands: Vec<u32>,
    class: u32,
    event: u32,
    query: bool,
    excluded: bool,
    recreate: bool,
    renderer: u32,
    registry: u32,
    tools: [u32; 3],
    occupant: u32,
    network: u32,
    setting: u32,
    argument: u32,
    developer: u32,
    release_result: u64,
}

thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }

fn event(this: *mut u8, slot: u32, args: &[u32]) {
    let id = unsafe { read_unaligned(this.add(4).cast::<u32>()) };
    STATE.with(|s| s.borrow_mut().events.push(Event(id, slot, args.to_vec())));
}

fn game_event(address: u32, args: &[u32]) {
    STATE.with(|s| s.borrow_mut().events.push(Event(0, address, args.to_vec())));
}

extern "thiscall" fn message_type(this: *mut u8) -> u64 {
    event(this, 0x10, &[]);
    HIGH | u64::from(STATE.with(|s| s.borrow().kind))
}

extern "thiscall" fn command(this: *mut u8) -> u64 {
    event(this, 0x24, &[]);
    HIGH | u64::from(STATE.with(|s| s.borrow_mut().commands.remove(0)))
}

extern "thiscall" fn class(this: *mut u8) -> u64 {
    event(this, 0x4C, &[]);
    HIGH | u64::from(STATE.with(|s| s.borrow().class))
}

extern "thiscall" fn occupant(this: *mut u8) -> u64 {
    event(this, 0x28, &[]);
    HIGH | u64::from(STATE.with(|s| s.borrow().occupant))
}

extern "thiscall" fn event_type(this: *mut u8) -> u64 {
    event(this, 0x60, &[]);
    HIGH | u64::from(STATE.with(|s| s.borrow().event))
}

extern "thiscall" fn setting(this: *mut u8) -> u64 {
    event(this, 0x3C, &[]);
    HIGH | u64::from(STATE.with(|s| s.borrow().setting))
}

extern "thiscall" fn query(this: *mut u8, iid: u32, out: *mut *mut u8) -> u64 {
    event(this, 0, &[iid]);
    assert!(unsafe { out.read() }.is_null());
    let (network, success) = STATE.with(|s| {
        let s = s.borrow();
        (s.network, s.query)
    });
    unsafe { out.write(network as *mut u8) };
    HIGH | 0xAABB_CC00 | u64::from(success)
}

extern "thiscall" fn excluded(this: *mut u8, flags: u32) -> u64 {
    event(this, 0x24, &[flags]);
    HIGH | 0xAABB_CC00 | u64::from(STATE.with(|s| s.borrow().excluded))
}

extern "thiscall" fn release(this: *mut u8) -> u64 {
    event(this, 8, &[]);
    STATE.with(|s| s.borrow().release_result)
}

extern "thiscall" fn position(this: *mut u8, out: *mut [u32; 3]) -> u64 {
    event(this, 0x20, &[]);
    unsafe { out.write([1.25f32.to_bits(), (-5.0f32).to_bits(), 2.5f32.to_bits()]) };
    ANSWER
}

extern "thiscall" fn cell_position(this: *mut u8, x: u32, z: u32, ox: *mut i32, oz: *mut i32) -> u64 {
    event(this, 0x17C, &[x, z]);
    assert_eq!(unsafe { ox.read() } as u32, STATE.with(|s| s.borrow().argument));
    unsafe {
        ox.write(1);
        oz.write(2);
    }
    ANSWER
}

extern "thiscall" fn bounds(this: *mut u8, out: *mut [i32; 4]) -> u64 {
    event(this, 0x38, &[]);
    unsafe { out.write([-2, 5, 4, 8]) };
    ANSWER
}

extern "thiscall" fn facing(this: *mut u8) -> u64 {
    event(this, 0x5C, &[]);
    HIGH | 3
}

extern "thiscall" fn lot_zone(this: *mut u8) -> u64 {
    event(this, 0x7C, &[]);
    HIGH | 4
}

extern "thiscall" fn delete_lot(this: *mut u8, lot: *mut u8) -> u64 {
    assert_eq!(this as u32 + 4, STATE.with(|s| s.borrow().developer));
    assert_eq!(unsafe { MessageDeveloper::from_ptr(this.add(4)) }.current_zone(), 4);
    game_event(0x60, &[lot as u32]);
    ANSWER
}

extern "thiscall" fn recreate(this: *mut u8, zone: u32) -> u64 {
    event(this, 0x34, &[zone]);
    HIGH | 0xAABB_CC00 | u64::from(STATE.with(|s| s.borrow().recreate))
}

extern "thiscall" fn create(this: *mut u8, x: i32, z: i32, w: i32, h: i32, f: u32) -> u64 {
    assert_eq!(this as u32 + 8, STATE.with(|s| s.borrow().developer));
    assert_eq!(unsafe { MessageDeveloper::from_ptr(this.add(8)) }.current_zone(), 4);
    game_event(0x0072_D440, &[x as u32, z as u32, w as u32, h as u32, f]);
    ANSWER
}

extern "thiscall" fn extend(this: *mut u8, occupant: *mut u8) -> u64 {
    assert_eq!(this as u32 + 8, STATE.with(|s| s.borrow().developer));
    assert_eq!(unsafe { MessageDeveloper::from_ptr(this.add(8)) }.current_zone(), 0);
    game_event(0x0073_1F60, &[occupant as u32]);
    HIGH | 0xBBDD_EEAA
}

extern "thiscall" fn rail(this: *mut u8, occupant: *mut u8) -> u64 {
    assert_eq!(this as u32 + 8, STATE.with(|s| s.borrow().developer));
    game_event(0x0073_2450, &[occupant as u32]);
    HIGH | 0xBBCC_DDAA
}

extern "thiscall" fn plop_lots(this: *mut u8) -> u64 {
    game_event(0x0072_C8E0, &[this as u32]);
    ANSWER
}

extern "thiscall" fn plop_buildings(this: *mut u8) -> u64 {
    game_event(0x0072_BE50, &[this as u32]);
    ANSWER
}

extern "C" fn get_renderer() -> u64 {
    game_event(0x0060_2160, &[]);
    HIGH | u64::from(STATE.with(|s| s.borrow().renderer))
}

extern "C" fn get_registry() -> u64 {
    game_event(0x0060_2680, &[]);
    HIGH | u64::from(STATE.with(|s| s.borrow().registry))
}

extern "thiscall" fn get_tool(this: *mut u8, kind: u32, acquire: u32) -> u64 {
    event(this, 0x14, &[kind, acquire]);
    let index = match kind {
        3 => 0,
        11 => 1,
        0 => 2,
        _ => unreachable!(),
    };
    HIGH | u64::from(STATE.with(|s| s.borrow().tools[index]))
}

extern "thiscall" fn add_ref(this: *mut u8) -> u64 {
    event(this, 4, &[]);
    ANSWER
}
extern "thiscall" fn init_tool(this: *mut u8) -> u64 {
    event(this, 0x0C, &[]);
    ANSWER
}

extern "thiscall" fn origin(this: *mut u8, x: *mut i32, z: *mut i32) -> u64 {
    event(this, 0x54, &[]);
    unsafe {
        x.write(this.add(8).cast::<i32>().read());
        z.write(this.add(12).cast::<i32>().read());
    }
    ANSWER
}

extern "thiscall" fn lot_initialize(this: *mut u8) -> u64 {
    event(this, 0x20, &[]);
    ANSWER
}

struct Thing {
    _table: Box<[usize; 100]>,
    words: Box<[u32; 4]>,
}

impl Thing {
    fn new(id: u32, slots: &[(usize, usize)]) -> Self {
        let mut table = Box::new([0; 100]);
        for &(slot, function) in slots {
            table[slot / 4] = function;
        }
        let words = Box::new([table.as_ptr() as u32, id, 0, 0]);
        Self { _table: table, words }
    }
    fn ptr(&mut self) -> *mut u8 {
        self.words.as_mut_ptr().cast()
    }
}

macro_rules! slots { ($($slot:expr => $function:ident),* $(,)?) => { &[$(($slot, $function as *const () as usize)),*] }; }

struct Fixture {
    object: Object,
    message: Thing,
    lot: Thing,
    _objects: Vec<Thing>,
    _secondary: Box<[usize; 25]>,
    rows: [[*mut u8; 4]; 3],
    row_headers: [u32; 9],
    _fakes: Vec<Fake>,
}

impl Fixture {
    fn new(kind: u32) -> Self {
        let mut message = Thing::new(
            1,
            slots![0x10 => message_type, 0x24 => command, 0x4C => class,
            0x28 => occupant, 0x60 => event_type, 0x3C => setting],
        );
        let mut renderer = Thing::new(2, slots![4 => add_ref]);
        let mut registry = Thing::new(3, slots![0x14 => get_tool]);
        let mut tools: Vec<_> = (4..=6)
            .map(|id| Thing::new(id, slots![4 => add_ref, 0x0C => init_tool]))
            .collect();
        let mut occupant = Thing::new(7, slots![0 => query, 0x20 => position]);
        let mut network = Thing::new(8, slots![0x24 => excluded, 8 => release]);
        let mut city = Thing::new(9, slots![0x17C => cell_position]);
        let mut manager = Thing::new(10, slots![0x34 => recreate]);
        let mut lot = Thing::new(
            11,
            slots![0x38 => bounds, 0x5C => facing, 0x7C => lot_zone, 0x54 => origin, 0x20 => lot_initialize],
        );
        let mut secondary = Box::new([0usize; 25]);
        secondary[24] = delete_lot as *const () as usize;
        let mut object = Object::new(0x110);
        unsafe { core::ptr::write_bytes(object.ptr(), 0xA5, 0x110) };
        let dev = unsafe { MessageDeveloper::from_ptr(object.ptr().add(8)) };
        unsafe { write_unaligned(object.ptr().add(4).cast::<u32>(), secondary.as_ptr() as u32) };
        dev.set_renderer(renderer.ptr());
        dev.set_street_tool(tools[0].ptr());
        dev.set_dirt_tool(tools[1].ptr());
        dev.set_road_tool(tools[2].ptr());
        dev.set_city(city.ptr());
        dev.set_zone_manager(manager.ptr());
        dev.set_enabled(0);
        dev.set_cells_x(3);
        dev.set_cells_z(4);
        dev.set_current_zone(0);
        STATE.with(|s| {
            *s.borrow_mut() = State {
                kind,
                renderer: renderer.ptr() as u32,
                registry: registry.ptr() as u32,
                tools: [tools[0].ptr() as u32, tools[1].ptr() as u32, tools[2].ptr() as u32],
                occupant: occupant.ptr() as u32,
                network: network.ptr() as u32,
                class: 0xC772_BF98,
                release_result: ANSWER,
                developer: dev.as_ptr() as u32,
                ..State::default()
            }
        });
        let mut objects = vec![renderer, registry, occupant, network, city, manager];
        objects.append(&mut tools);
        let fakes = [
            (0x0060_2160, get_renderer as *const () as usize),
            (0x0060_2680, get_registry as *const () as usize),
            (0x0072_C8E0, plop_lots as *const () as usize),
            (0x0072_BE50, plop_buildings as *const () as usize),
            (0x0073_1F60, extend as *const () as usize),
            (0x0073_2450, rail as *const () as usize),
            (0x0072_D440, create as *const () as usize),
        ]
        .into_iter()
        .map(|(a, f)| fake(a, f))
        .collect();
        // Keep both objects' storage stable when the fixture moves.
        let _ = (message.ptr(), lot.ptr());
        Self {
            object,
            message,
            lot,
            _objects: objects,
            _secondary: secondary,
            rows: [[null_mut(); 4]; 3],
            row_headers: [0xA5A5_A5A5; 9],
            _fakes: fakes,
        }
    }

    fn dev(&mut self) -> MessageDeveloper {
        unsafe { MessageDeveloper::from_ptr(self.object.ptr().add(8)) }
    }

    fn run(&mut self) -> (u64, u32) {
        for (i, row) in self.rows.iter_mut().enumerate() {
            self.row_headers[3 * i] = row.as_mut_ptr() as u32;
        }
        self.dev().set_lot_rows(self.row_headers.as_mut_ptr().cast());
        let dev = self.dev();
        let mut expected = unsafe { core::slice::from_raw_parts(dev.as_ptr().sub(8), 0x110) }.to_vec();
        STATE.with(|s| {
            let s = s.borrow();
            let mut word = |offset: usize, value: u32| {
                expected[8 + offset..12 + offset].copy_from_slice(&value.to_le_bytes());
            };
            match s.kind {
                0x26D3_1EC1 => {
                    for (offset, old, new) in [
                        (0x30, dev.renderer(), s.renderer),
                        (0x40, dev.street_tool(), s.tools[0]),
                        (0x3C, dev.dirt_tool(), s.tools[1]),
                        (0x44, dev.road_tool(), s.tools[2]),
                    ] {
                        if old.is_null() {
                            word(offset, new);
                        }
                    }
                }
                0xA6B7_9602 => word(0xCC, unsafe { (s.setting as *const u32).read() }),
                0x99EF_1142 if s.class == 0xC772_BF98 && !(s.query && s.excluded) && !self.rows[1][2].is_null() => {
                    word(0x98, 0);
                }
                _ => {}
            }
        });
        let mut context = [self.dev().as_ptr() as u32, self.message.ptr() as u32, 0, 0, 0];
        STATE.with(|s| s.borrow_mut().argument = context[1]);
        // Keep the caller-owned argument word available after the callee pops it.
        unsafe {
            core::arch::asm!(
                "push dword ptr [edi + 4]", "mov ecx, [edi]", "mov edx, 0x10203040", "call {function}",
                "mov [edi + 8], eax", "mov [edi + 12], edx", "mov eax, [esp - 4]", "mov [edi + 16], eax",
                function = sym fc4p_007328d0,
                in("edi") context.as_mut_ptr(),
                clobber_abi("C"),
            );
        }
        assert_eq!(
            unsafe { core::slice::from_raw_parts(dev.as_ptr().sub(8), 0x110) },
            expected
        );
        ((u64::from(context[3]) << 32) | u64::from(context[2]), context[4])
    }

    fn events(&self) -> Vec<Event> {
        STATE.with(|s| std::mem::take(&mut s.borrow_mut().events))
    }
}

#[test]
fn unknown_messages_and_setting_messages_preserve_upper_return_bytes() {
    for kind in [0u32, 0x230E_27AB, u32::MAX] {
        let mut f = Fixture::new(kind);
        let message = f.message.ptr() as u32;
        assert_eq!(f.run(), (HIGH | u64::from((kind & !0xFF) | 1), message));
        assert_eq!(f.events(), [Event(1, 0x10, vec![])]);
    }
    let mut f = Fixture::new(0xA6B7_9602);
    let mut setting = 0x89AB_CDEFu32;
    STATE.with(|s| s.borrow_mut().setting = &mut setting as *mut u32 as u32);
    assert_eq!(f.run().0, HIGH | 0x89AB_CD01);
    assert_eq!(f.dev().zoning_setting(), setting);
    assert_eq!(f.events(), [Event(1, 0x10, vec![]), Event(1, 0x3C, vec![])]);
}

#[test]
fn plop_commands_call_the_second_getter_only_after_the_first_misses() {
    for (commands, address, expected) in [
        (vec![0x8929_E68E], Some(0x0072_C8E0), ANSWER & !0xFF | 1),
        (vec![0, 0x4AB5_69C9], Some(0x0072_BE50), ANSWER & !0xFF | 1),
        (vec![0x4AB5_69C9, 0x1234_5678], None, HIGH | 0x1234_5601),
    ] {
        let mut f = Fixture::new(0x230E_27AC);
        let count = commands.len();
        STATE.with(|s| s.borrow_mut().commands = commands);
        assert_eq!(f.run().0, expected);
        let mut events = vec![Event(1, 0x10, vec![])];
        events.extend((0..count).map(|_| Event(1, 0x24, vec![])));
        if let Some(address) = address {
            events.push(Event(0, address, vec![f.dev().as_ptr().wrapping_sub(8) as u32]));
        }
        assert_eq!(f.events(), events);
    }
}

#[test]
fn initialization_acquires_only_missing_services_in_order() {
    let mut f = Fixture::new(0x26D3_1EC1);
    let dev = f.dev();
    dev.set_renderer(null_mut());
    dev.set_street_tool(null_mut());
    dev.set_dirt_tool(null_mut());
    dev.set_road_tool(null_mut());
    let message = f.message.ptr() as u32;
    assert_eq!(f.run(), (ANSWER & !0xFF | 1, message));
    assert_eq!(
        f.events(),
        [
            Event(1, 0x10, vec![]),
            Event(0, 0x0060_2160, vec![]),
            Event(2, 4, vec![]),
            Event(0, 0x0060_2680, vec![]),
            Event(3, 0x14, vec![3, 1]),
            Event(4, 4, vec![]),
            Event(4, 0x0C, vec![]),
            Event(0, 0x0060_2680, vec![]),
            Event(3, 0x14, vec![11, 1]),
            Event(5, 4, vec![]),
            Event(5, 0x0C, vec![]),
            Event(0, 0x0060_2680, vec![]),
            Event(3, 0x14, vec![0, 1]),
            Event(6, 4, vec![]),
            Event(6, 0x0C, vec![]),
        ]
    );
    let renderer = f.dev().renderer() as u32;
    assert_eq!(f.run(), (HIGH | u64::from((renderer & !0xFF) | 1), message));
    assert_eq!(f.events(), [Event(1, 0x10, vec![])]);
    f.dev().set_dirt_tool(null_mut());
    assert_eq!(f.run().0, ANSWER & !0xFF | 1);
    assert_eq!(
        f.events(),
        [
            Event(1, 0x10, vec![]),
            Event(0, 0x0060_2680, vec![]),
            Event(3, 0x14, vec![11, 1]),
            Event(5, 4, vec![]),
            Event(5, 0x0C, vec![])
        ]
    );
}

#[test]
fn initialization_scans_x_then_z_and_initializes_only_lot_origins() {
    let mut f = Fixture::new(0x26D3_1EC1);
    f.dev().set_enabled(0x80);
    f.lot.words[2] = 1;
    f.lot.words[3] = 2;
    f.rows[0][2] = f.lot.ptr();
    f.rows[1][2] = f.lot.ptr();
    f.rows[1][3] = f.lot.ptr();
    assert_eq!(f.run().1, 3);
    assert_eq!(
        f.events(),
        [
            Event(1, 0x10, vec![]),
            Event(11, 0x54, vec![]),
            Event(11, 0x54, vec![]),
            Event(11, 0x20, vec![]),
            Event(11, 0x54, vec![])
        ]
    );
    for (x, z, counter) in [(0, 4, 0), (-1, 4, 0), (3, 0, 3), (3, -1, 3)] {
        f.dev().set_cells_x(x);
        f.dev().set_cells_z(z);
        assert_eq!(f.run().1, counter);
        assert_eq!(f.events(), [Event(1, 0x10, vec![])]);
    }
}

#[test]
fn occupant_class_filter_and_excluded_network_release_stop_processing() {
    let mut f = Fixture::new(0x99EF_1142);
    STATE.with(|s| s.borrow_mut().class = 0x1234_5678);
    assert_eq!(f.run().0, HIGH | 0x1234_5601);
    assert_eq!(f.events(), [Event(1, 0x10, vec![]), Event(1, 0x4C, vec![])]);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.class = 0xC772_BF98;
        s.query = true;
        s.excluded = true;
    });
    let message = f.message.ptr() as u32;
    assert_eq!(f.run(), (ANSWER & !0xFF | 1, message));
    assert_eq!(
        f.events(),
        [
            Event(1, 0x10, vec![]),
            Event(1, 0x4C, vec![]),
            Event(1, 0x28, vec![]),
            Event(7, 0, vec![0xA821_EF94]),
            Event(8, 0x24, vec![0x8020_0090]),
            Event(8, 8, vec![])
        ]
    );
}

#[test]
fn occupant_events_convert_position_recreate_the_lot_then_dispatch_network_work() {
    for (query, recreate, event_id, helper) in [
        (false, true, 1, 0x0073_1F60),
        (true, false, 2, 0x0073_1F60),
        (true, true, 3, 0x0073_2450),
        (false, false, 0, 0),
        (false, false, 4, 0),
        (false, false, u32::MAX, 0),
    ] {
        let mut f = Fixture::new(0x99EF_1142);
        f.rows[1][2] = f.lot.ptr();
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.query = query;
            s.recreate = recreate;
            s.event = event_id;
        });
        let (result, argument) = f.run();
        assert_eq!(argument, 1);
        assert_eq!(
            result,
            HIGH | match helper {
                0x0073_1F60 => 0xBBDD_EE01,
                0x0073_2450 => 0xBBCC_DD01,
                _ => (event_id & !0xFF) | 1,
            } as u64
        );
        assert_eq!(f.dev().current_zone(), 0);
        let mut events = vec![
            Event(1, 0x10, vec![]),
            Event(1, 0x4C, vec![]),
            Event(1, 0x28, vec![]),
            Event(7, 0, vec![0xA821_EF94]),
        ];
        if query {
            events.extend([Event(8, 0x24, vec![0x8020_0090]), Event(8, 8, vec![])]);
        }
        events.extend([
            Event(7, 0x20, vec![]),
            Event(9, 0x17C, vec![1.25f32.to_bits(), 2.5f32.to_bits()]),
            Event(11, 0x38, vec![]),
            Event(11, 0x5C, vec![]),
            Event(11, 0x7C, vec![]),
            Event(0, 0x60, vec![f.lot.ptr() as u32]),
            Event(10, 0x34, vec![4]),
        ]);
        if recreate {
            events.push(Event(0, 0x0072_D440, vec![(-2i32) as u32, 5, 7, 4, 3]));
        }
        events.push(Event(1, 0x60, vec![]));
        if helper != 0 {
            events.push(Event(0, helper, vec![STATE.with(|s| s.borrow().occupant)]));
        }
        assert_eq!(f.events(), events);
    }
}

#[test]
fn occupant_event_without_a_lot_keeps_current_zone_and_still_dispatches() {
    let mut f = Fixture::new(0x99EF_1142);
    f.dev().set_current_zone(0);
    STATE.with(|s| s.borrow_mut().event = 1);
    assert_eq!(f.run(), (HIGH | 0xBBDD_EE01, 1));
    assert_eq!(
        f.events(),
        [
            Event(1, 0x10, vec![]),
            Event(1, 0x4C, vec![]),
            Event(1, 0x28, vec![]),
            Event(7, 0, vec![0xA821_EF94]),
            Event(7, 0x20, vec![]),
            Event(9, 0x17C, vec![1.25f32.to_bits(), 2.5f32.to_bits()]),
            Event(1, 0x60, vec![]),
            Event(0, 0x0073_1F60, vec![STATE.with(|s| s.borrow().occupant)])
        ]
    );
    f.dev().set_current_zone(0xAABB_CCDD);
    STATE.with(|s| s.borrow_mut().event = 0);
    assert_eq!(f.run(), (HIGH | 1, 1));
    assert_eq!(f.dev().current_zone(), 0xAABB_CCDD);
}
