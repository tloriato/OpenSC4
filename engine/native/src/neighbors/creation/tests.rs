// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! The core's creation, marker placement and orientation cases at the game's layout.

use super::*;
use crate::neighbors::pair;
use crate::testing::{fake, fake_data, Fake, GameFpu, Object};
use std::cell::RefCell;

#[derive(Default)]
struct State {
    calls: Vec<(u32, u32, Vec<u32>)>,
    connection: u32,
    node: u32,
    message: u32,
    other: u32,
    marker: u32,
    occupant: u32,
    properties: u32,
    duplicate: u32,
    marker_ok: bool,
    position: [u32; 3],
}

thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }

fn record(this: u32, slot: u32, args: &[u32]) {
    STATE.with(|s| s.borrow_mut().calls.push((this, slot, args.to_vec())));
}

unsafe extern "thiscall" fn exists(this: *mut u8, kind: u32, x: u32, z: u32) -> u64 {
    record(this as u32, 0x28, &[kind, x, z]);
    pair(STATE.with(|s| s.borrow().duplicate), 0x2828_2828)
}
unsafe extern "C" fn allocate_connection(size: u32) -> u64 {
    record(0, 0x005E_55E0, &[size]);
    pair(STATE.with(|s| s.borrow().connection), 0)
}
unsafe extern "thiscall" fn connection_new(this: *mut u8) -> u64 {
    record(this as u32, 0x006D_4680, &[]);
    pair(this as u32, 0)
}
unsafe extern "fastcall" fn node_new(this: *mut u8, edx: u32, count: u32, hint: u32) -> u64 {
    record(this as u32, 0x0041_1B00, &[edx, count, hint]);
    pair(STATE.with(|s| s.borrow().node), 0)
}
unsafe extern "C" fn cell(x: u32, z: u32, rx: *mut u32, rz: *mut u32, city: u32) -> u64 {
    record(0, 0x006C_E820, &[x, z, rx as u32, rz as u32, city]);
    unsafe {
        rx.write_unaligned(17);
        rz.write_unaligned(23);
    }
    pair(0, 0)
}
unsafe extern "C" fn allocate_message(size: u32) -> u64 {
    record(0, 0x0091_33DA, &[size]);
    pair(STATE.with(|s| s.borrow().message), 0)
}
unsafe extern "thiscall" fn message_new(this: *mut u8) -> u64 {
    record(this as u32, 0x0091_34D6, &[]);
    pair(this as u32, 0)
}
unsafe extern "thiscall" fn addref(this: *mut u8) -> u64 {
    record(this as u32, 4, &[]);
    pair(0x1234_0001, 0x0404_0404)
}
unsafe extern "thiscall" fn release(this: *mut u8) -> u64 {
    record(this as u32, 8, &[]);
    pair(0x5678_0000, 0x0808_0808)
}
unsafe extern "thiscall" fn other(this: *mut u8) -> u64 {
    record(this as u32, 0x24, &[]);
    pair(STATE.with(|s| s.borrow().other), 0)
}
unsafe extern "thiscall" fn symmetric(this: *mut u8, connection: u32, add: u32) -> u64 {
    record(this as u32, 0x58, &[connection, add]);
    pair(0, 0)
}
unsafe extern "thiscall" fn width(this: *mut u8) -> u64 {
    record(this as u32, 0x174, &[]);
    pair(64, 0)
}
unsafe extern "thiscall" fn center(this: *mut u8, x: i32, z: i32, px: *mut u32, pz: *mut u32) -> u64 {
    record(this as u32, 0x184, &[x as u32, z as u32]);
    unsafe {
        px.write_unaligned((x.wrapping_mul(16).wrapping_add(8) as f32).to_bits());
        pz.write_unaligned((z.wrapping_mul(16).wrapping_add(8) as f32).to_bits());
    }
    pair(0, 0)
}
unsafe extern "thiscall" fn altitude(this: *mut u8, x: u32, z: u32) -> f32 {
    record(this as u32, 0x34, &[x, z]);
    300.0
}
unsafe extern "thiscall" fn create_marker(this: *mut u8, kind: u32, output: *mut u32) -> u64 {
    record(this as u32, 0x3C, &[kind, output as u32]);
    let (ok, marker) = STATE.with(|s| (s.borrow().marker_ok, s.borrow().marker));
    assert_eq!(unsafe { output.read_unaligned() }, 0);
    if ok {
        unsafe {
            output.write_unaligned(marker);
        }
    }
    pair(u32::from(ok), 0)
}
unsafe extern "thiscall" fn occupant(this: *mut u8) -> u64 {
    record(this as u32, 0x0C, &[]);
    pair(STATE.with(|s| s.borrow().occupant), 0)
}
unsafe extern "thiscall" fn position(this: *mut u8, position: *const [u32; 3]) -> u64 {
    let position = unsafe { position.read_unaligned() };
    record(this as u32, 0x24, &position);
    STATE.with(|s| s.borrow_mut().position = position);
    pair(0, 0)
}
unsafe extern "thiscall" fn orientation(this: *mut u8, value: u32) -> u64 {
    record(this as u32, 0x28, &[value]);
    pair(0, 0)
}
unsafe extern "thiscall" fn properties(this: *mut u8) -> u64 {
    record(this as u32, 0x18, &[]);
    pair(STATE.with(|s| s.borrow().properties), 0)
}
unsafe extern "thiscall" fn property(this: *mut u8, id: u32, on: u32, flag: u32) -> u64 {
    record(this as u32, 0x34, &[id, on, flag]);
    pair(0, 0)
}
unsafe extern "thiscall" fn add_marker(this: *mut u8, occupant: u32) -> u64 {
    record(this as u32, 0x4C, &[occupant]);
    pair(0, 0)
}
macro_rules! message_word {
    ($name:ident, $slot:literal) => {
        unsafe extern "thiscall" fn $name(this: *mut u8, value: u32) -> u64 {
            record(this as u32, $slot, &[value]);
            pair(0, 0)
        }
    };
}
message_word!(message_type, 0x14);
message_word!(message_kind, 0x2C);
message_word!(message_x, 0x40);
message_word!(message_z, 0x54);
unsafe extern "thiscall" fn post(this: *mut u8, message: u32, flags: u32) -> u64 {
    record(this as u32, 0x10, &[message, flags]);
    pair(0xABCD_0001, 0xDEAD_BEEF)
}

struct Fixture {
    _tables: Vec<Vec<usize>>,
    _objects: Vec<Object>,
    globals: [u32; 4],
    guards: Vec<Fake>,
    sim: *mut u8,
    connection: *mut u8,
    header: *mut u8,
    previous: *mut u8,
    node: *mut u8,
    city: *mut u8,
    props: *mut u8,
    terrain: *mut u8,
    message: *mut u8,
    server: *mut u8,
    marker: *mut u8,
    occupant: *mut u8,
    properties: *mut u8,
    other: *mut u8,
}

impl Fixture {
    fn new() -> Self {
        let mut objects: Vec<_> = (0..15).map(|_| Object::new(0x80)).collect();
        let p: Vec<_> = objects.iter_mut().map(Object::ptr).collect();
        let mut tables = vec![vec![0usize; 0x188 / 4]; 15];
        for table in &mut tables {
            table[1] = addref as *const () as usize;
            table[2] = release as *const () as usize;
        }
        tables[0][0x28 / 4] = exists as *const () as usize;
        tables[1][0x24 / 4] = other as *const () as usize;
        tables[5][0x174 / 4] = width as *const () as usize;
        tables[5][0x184 / 4] = center as *const () as usize;
        tables[6][0x3C / 4] = create_marker as *const () as usize;
        tables[6][0x4C / 4] = add_marker as *const () as usize;
        tables[7][0x34 / 4] = altitude as *const () as usize;
        tables[8][0x14 / 4] = message_type as *const () as usize;
        tables[8][0x2C / 4] = message_kind as *const () as usize;
        tables[8][0x40 / 4] = message_x as *const () as usize;
        tables[8][0x54 / 4] = message_z as *const () as usize;
        tables[9][0x10 / 4] = post as *const () as usize;
        tables[10][0x0C / 4] = occupant as *const () as usize;
        tables[10][0x28 / 4] = orientation as *const () as usize;
        tables[11][0x24 / 4] = position as *const () as usize;
        tables[11][0x18 / 4] = properties as *const () as usize;
        tables[12][0x34 / 4] = property as *const () as usize;
        tables[13][0x58 / 4] = symmetric as *const () as usize;
        unsafe {
            for (object, table) in p.iter().zip(&tables) {
                object.cast::<*const usize>().write(table.as_ptr());
            }
            NeighborsSim::from_ptr(p[0]).set_city(p[5]);
            NeighborsSim::from_ptr(p[0]).set_connections(p[2]);
        }
        let mut fixture = Self {
            _tables: tables,
            _objects: objects,
            globals: [p[6] as u32, p[7] as u32, p[9] as u32, 15.5f32.to_bits()],
            guards: vec![],
            sim: p[0],
            connection: p[1],
            header: p[2],
            previous: p[3],
            node: p[4],
            city: p[5],
            props: p[6],
            terrain: p[7],
            message: p[8],
            server: p[9],
            marker: p[10],
            occupant: p[11],
            properties: p[12],
            other: p[13],
        };
        for (address, function) in [
            (0x005E_55E0, allocate_connection as *const () as usize),
            (0x006D_4680, connection_new as *const () as usize),
            (0x0041_1B00, node_new as *const () as usize),
            (0x006C_E820, cell as *const () as usize),
            (0x0091_33DA, allocate_message as *const () as usize),
            (0x0091_34D6, message_new as *const () as usize),
        ] {
            fixture.guards.push(fake(address, function));
        }
        fixture
    }

    #[allow(clippy::too_many_arguments)]
    fn run(&mut self, kind: u32, x: i32, z: i32, flip: u32, duplicate: u32, marker_ok: bool, other: bool) {
        let data_guards: Vec<_> = [0x00B4_3D10, 0x00B4_3CF4, 0x00B4_3CCC, 0x00AB_1CA8]
            .into_iter()
            .zip(self.globals.iter_mut())
            .map(|(address, value)| fake_data(address, (value as *mut u32).cast()))
            .collect();
        STATE.with(|s| {
            *s.borrow_mut() = State {
                connection: self.connection as u32,
                node: self.node as u32,
                message: self.message as u32,
                other: if other { self.other as u32 } else { 0 },
                marker: self.marker as u32,
                occupant: self.occupant as u32,
                properties: self.properties as u32,
                duplicate,
                marker_ok,
                ..State::default()
            }
        });
        let mut args = [kind, x as u32, z as u32, flip];
        let original = args;
        unsafe {
            self.header.add(4).cast::<*mut u8>().write_unaligned(self.previous);
            let result = create(self.sim, args.as_mut_ptr());
            let mut expected = vec![(self.sim as u32, 0x28, vec![kind, x as u32, z as u32])];
            if duplicate as u8 != 0 {
                assert_eq!(args, original);
                assert_eq!(result, pair(duplicate, 0x2828_2828));
            } else {
                expected.extend([
                    (0, 0x005E_55E0, vec![0x44]),
                    (self.connection as u32, 0x006D_4680, vec![]),
                    (self.connection as u32, 4, vec![]),
                    (self.sim.add(0x1C) as u32, 0x0041_1B00, vec![self.header as u32, 1, 0]),
                    (self.connection as u32, 4, vec![]),
                    (self.connection as u32, 8, vec![]),
                    (
                        0,
                        0x006C_E820,
                        vec![
                            x as u32,
                            z as u32,
                            args.as_mut_ptr().add(1) as u32,
                            args.as_mut_ptr() as u32,
                            0,
                        ],
                    ),
                    (self.connection as u32, 0x24, vec![]),
                ]);
                if other {
                    expected.push((self.other as u32, 0x58, vec![self.connection as u32, 1]));
                }
                assert_eq!(pointer(self.node, 0), self.header);
                assert_eq!(pointer(self.node, 4), self.previous);
                assert_eq!(pointer(self.node, 8), self.connection);
                assert_eq!(pointer(self.previous, 0), self.node);
                assert_eq!(pointer(self.header, 4), self.node);
                let c = Connection::from_ptr(self.connection);
                assert_eq!(
                    (c.kind(), c.x(), c.z(), c.region_x(), c.region_z()),
                    (kind, x as u32, z as u32, 17, 23)
                );
                if kind == 9 {
                    assert_eq!(args, [23, 17, z as u32, flip]);
                } else {
                    let flip_on = flip as u8 != 0;
                    let (ix, iz, orientation) = if x < 0 {
                        (x + 1, z, if flip_on { 1 } else { 3 })
                    } else if z < 0 {
                        (x, z + 1, if flip_on { 2 } else { 0 })
                    } else if x >= 64 {
                        (x - 1, z, if flip_on { 3 } else { 1 })
                    } else {
                        (x, z - 1, if flip_on { 0 } else { 2 })
                    };
                    if x >= 0 && z >= 0 {
                        expected.push((self.city as u32, 0x174, vec![]));
                    }
                    let px = (ix * 16 + 8) as f32;
                    let pz = (iz * 16 + 8) as f32;
                    expected.extend([
                        (self.city as u32, 0x184, vec![ix as u32, iz as u32]),
                        (self.terrain as u32, 0x34, vec![px.to_bits(), pz.to_bits()]),
                        (
                            self.props as u32,
                            0x3C,
                            vec![0x29F1_0000, args.as_mut_ptr().add(1) as u32],
                        ),
                    ]);
                    if marker_ok {
                        let height = if [3, 7, 11].contains(&kind) { 315.5f32 } else { 300.0 };
                        expected.extend([
                            (self.marker as u32, 0x0C, vec![]),
                            (
                                self.occupant as u32,
                                0x24,
                                vec![px.to_bits(), height.to_bits(), pz.to_bits()],
                            ),
                            (self.marker as u32, 0x28, vec![orientation]),
                            (self.marker as u32, 0x0C, vec![]),
                            (self.occupant as u32, 0x18, vec![]),
                            (self.properties as u32, 0x34, vec![0xCAA4_6595, 1, 0]),
                        ]);
                        if kind == 5 || kind == 6 {
                            expected.push((self.properties as u32, 0x34, vec![0xCAA4_5670, 1, 0]));
                            expected.push((
                                self.properties as u32,
                                0x34,
                                vec![if kind == 5 { 0xCAA4_5672 } else { 0xCAA4_5671 }, 1, 0],
                            ));
                        }
                        expected.extend([
                            (self.marker as u32, 0x0C, vec![]),
                            (self.props as u32, 0x4C, vec![self.occupant as u32]),
                            (self.marker as u32, 8, vec![]),
                        ]);
                    }
                    assert_eq!(
                        args,
                        [
                            self.props as u32,
                            if marker_ok { self.marker as u32 } else { 0 },
                            z as u32,
                            flip
                        ]
                    );
                }
                expected.extend([
                    (0, 0x0091_33DA, vec![0x2C]),
                    (self.message as u32, 0x0091_34D6, vec![]),
                    (self.message as u32, 4, vec![]),
                    (self.message as u32, 0x14, vec![0x8A0D_15DA]),
                    (self.message as u32, 0x2C, vec![kind]),
                    (self.message as u32, 0x40, vec![x as u32]),
                    (self.message as u32, 0x54, vec![z as u32]),
                    (self.server as u32, 0x10, vec![self.message as u32, 0]),
                    (self.message as u32, 8, vec![]),
                ]);
                assert_eq!(result, pair(0x5678_0000, 0x0808_0808));
            }
            STATE.with(|s| assert_eq!(s.borrow().calls, expected));
        }
        drop(data_guards);
    }
}

#[test]
fn creates_markers_at_inner_centers_with_each_orientation_and_property_set() {
    let _fpu = GameFpu::new();
    let mut f = Fixture::new();
    for kind in [1, 3, 5, 6, 7, 11] {
        for (x, z) in [(-1, 5), (5, -1), (64, 5), (5, 64), (5, 5), (-1, -1)] {
            for flip in [0, 1, 0x100, 0xFFFF_FFFF] {
                f.run(kind, x, z, flip, 0, true, true);
            }
        }
    }
}

#[test]
fn duplicate_ferry_missing_city_and_marker_failure_paths() {
    let _fpu = GameFpu::new();
    let mut f = Fixture::new();
    f.run(1, -1, 5, 0, 0xABCD_1201, true, true);
    f.run(9, -1, 5, 0, 0, true, false);
    f.run(1, 64, 5, 0, 0, false, false);
    // Existence is tested only in AL; nonzero upper bytes do not reject creation.
    f.run(1, 5, 64, 0, 0xABCD_1200, true, false);
}

#[test]
fn export_pops_four_words_and_preserves_the_callers_registers() {
    let mut f = Fixture::new();
    STATE.with(|s| s.borrow_mut().duplicate = 0x1234_5601);
    let args = [1, u32::MAX, 5, 0];
    let mut call = crate::abi::RawCall::new(
        crate::abi::Conv::Thiscall,
        fc4p_006d4860 as *const () as usize,
        f.sim as u32,
        &args,
    );
    unsafe {
        crate::abi::call_raw(&mut call);
    }
    assert_eq!((call.eax, call.edx), (0x1234_5601, 0x2828_2828));
    f.guards.clear();
}
