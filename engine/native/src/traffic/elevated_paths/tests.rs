// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, fake_data, Fake, Object};
use crate::traffic::test_support::{bytes, float_environment, object, table};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Keys(u32, u32),
    Reserve(u32),
    Path(u32, u32),
    Insert(u32, u32, u32),
    Framework,
    Service(u32, u32),
    Holder(u32),
    Allocate(u32),
    Occupants([u32; 2], bool, u32, u32),
    Query(u32, u32),
    Flags(u32, u32),
    Paths(u32),
    Near(u32, u32, [u32; 3], u32, i32, u32),
    ReleaseNetwork(u32),
    ReleaseOccupant(u32),
    Clear(u32),
    FreeHeader(u32),
    Free(u32),
}
#[derive(Default)]
struct State {
    calls: Vec<Call>,
    keys: [u32; 3],
    count: u32,
    vertices: Vec<(u32, usize)>,
    point_memory: usize,
    reserve_count: u32,
    header: usize,
    nodes: Vec<usize>,
    listed: Vec<usize>,
    networks: Vec<usize>,
    path_info: usize,
    pool: usize,
    app: usize,
    pool_global: usize,
    query: [u32; 3],
    flags: [u32; 3],
    null_paths: bool,
    near: Vec<(u32, u32, u32, u32)>,
    reacquire: bool,
    scratch: usize,
}
thread_local! { static STATE:RefCell<State>=RefCell::new(State::default()); }
fn log(c: Call) {
    STATE.with(|s| s.borrow_mut().calls.push(c));
}
fn calls() -> Vec<Call> {
    STATE.with(|s| std::mem::take(&mut s.borrow_mut().calls))
}
unsafe extern "thiscall" fn keys(_: *mut u8, kind: u32, entry: u32, out: *mut u32) -> u32 {
    log(Call::Keys(kind, entry));
    STATE.with(|s| {
        let s = s.borrow();
        unsafe {
            for (i, &v) in s.keys.iter().enumerate() {
                out.add(i).write(v);
            }
        }
        s.count
    })
}
unsafe extern "thiscall" fn reserve(this: *mut u8, count: u32) {
    log(Call::Reserve(count));
    STATE.with(|s| {
        let s = s.borrow();
        unsafe {
            store(this, 0, s.point_memory as u32);
            store(this, 4, s.point_memory as u32);
            store(this, 8, (s.point_memory as u32).wrapping_add(s.reserve_count * 12));
        }
    });
}
extern "thiscall" fn path(_: *mut u8, key: u32, zero: u32) -> *mut u8 {
    log(Call::Path(key, zero));
    STATE.with(|s| {
        s.borrow()
            .vertices
            .iter()
            .find(|&&(k, _)| k == key)
            .map_or(core::ptr::null_mut(), |&(_, p)| p as *mut u8)
    })
}
unsafe extern "thiscall" fn insert(this: *mut u8, end: u32, point: *const u8, scratch: *mut u32, a: u32, b: u32) {
    log(Call::Insert(unsafe { scratch.read() }, a, b));
    STATE.with(|s| s.borrow_mut().scratch = scratch as usize);
    unsafe {
        core::ptr::copy_nonoverlapping(point, end as *mut u8, 12);
        store(this, 4, end + 12);
        store(this, 8, end + 24);
    }
}
extern "C" fn framework_fake() -> *mut u8 {
    log(Call::Framework);
    STATE.with(|s| s.borrow().app as *mut u8)
}
unsafe extern "thiscall" fn service(_: *mut u8, class: u32, iid: u32, out: *mut u32) {
    log(Call::Service(class, iid));
    STATE.with(|s| unsafe { out.write(s.borrow().pool as u32) });
}
unsafe extern "thiscall" fn holder(this: *mut u32) {
    log(Call::Holder(unsafe { this.read() }));
    unsafe {
        this.write(0);
    }
}
extern "thiscall" fn allocate(_: *mut u8, size: u32) -> *mut u8 {
    log(Call::Allocate(size));
    STATE.with(|s| s.borrow().header as *mut u8)
}
extern "thiscall" fn free_header(_: *mut u8, head: u32) {
    log(Call::FreeHeader(head));
}
unsafe extern "thiscall" fn get_occupants(_: *mut u8, out: *mut u32, a: *mut u32, b: *mut u32, kind: u32, limit: u32) {
    log(Call::Occupants(
        unsafe { [a.read(), a.add(1).read()] },
        a == b,
        kind,
        limit,
    ));
    STATE.with(|s| {
        let s = s.borrow();
        let head = unsafe { out.read() } as *mut u8;
        assert_eq!(unsafe { [word(head, 0), word(head, 4)] }, [head as u32; 2]);
        unsafe {
            store(head, 0, s.nodes.first().copied().unwrap_or(head as usize) as u32);
            for (i, &node) in s.nodes.iter().enumerate() {
                store(
                    node as *mut u8,
                    0,
                    s.nodes.get(i + 1).copied().unwrap_or(head as usize) as u32,
                );
                store(node as *mut u8, 8, s.listed[i] as u32);
            }
        }
    });
}
unsafe extern "thiscall" fn query(this: *mut u8, iid: u32, out: *mut *mut u8) -> u32 {
    let id = unsafe { word(this, 4) };
    log(Call::Query(id, iid));
    STATE.with(|s| {
        let s = s.borrow();
        if s.query[id as usize] as u8 != 0 {
            unsafe {
                out.write(s.networks[id as usize] as *mut u8);
            }
        }
        s.query[id as usize]
    })
}
unsafe extern "thiscall" fn flags(this: *mut u8, mask: u32) -> u32 {
    let id = unsafe { word(this, 4) };
    log(Call::Flags(id, mask));
    STATE.with(|s| s.borrow().flags[id as usize])
}
unsafe extern "thiscall" fn get_paths(this: *mut u8) -> *mut u8 {
    let id = unsafe { word(this, 4) };
    log(Call::Paths(id));
    STATE.with(|s| {
        let s = s.borrow();
        if s.null_paths {
            core::ptr::null_mut()
        } else {
            s.path_info as *mut u8
        }
    })
}
unsafe extern "thiscall" fn near(
    _: *mut u8,
    out: *mut u32,
    kind: u32,
    point: *mut u8,
    any: u32,
    opposite: i32,
    radius: u32,
) {
    let words = unsafe { [word(point, 0), word(point, 4), word(point, 8)] };
    let id = STATE.with(|s| {
        s.borrow()
            .calls
            .iter()
            .rev()
            .find_map(|c| if let Call::Paths(id) = c { Some(*id) } else { None })
            .unwrap()
    });
    log(Call::Near(id, kind, words, any, opposite, radius));
    STATE.with(|s| {
        let s = s.borrow();
        let (_, _, begin, len) = s
            .near
            .iter()
            .copied()
            .find(|&(n, p, _, _)| n == id && p == words[0])
            .unwrap_or((id, words[0], 0, 0));
        unsafe {
            out.write(begin);
            out.add(1).write(begin + len * 4);
            out.add(2).write(begin + len * 4);
        }
    });
}
unsafe extern "thiscall" fn release_network(this: *mut u8) {
    log(Call::ReleaseNetwork(unsafe { word(this, 4) }));
}
unsafe extern "thiscall" fn release_occupant(this: *mut u8) {
    log(Call::ReleaseOccupant(unsafe { word(this, 4) }));
}
unsafe extern "thiscall" fn clear(this: *mut u32) {
    log(Call::Clear(unsafe { this.read() }));
    STATE.with(|s| {
        let s = s.borrow();
        if s.reacquire {
            unsafe {
                (s.pool_global as *mut u32).write(0);
            }
        }
    });
}
extern "C" fn free(p: *mut u8) {
    log(Call::Free(p as u32));
}

struct World {
    sim: Object,
    path: Object,
    keys: Object,
    points: Object,
    header: Object,
    intersections: Object,
    vertices: Vec<Object>,
    info: Vec<Object>,
    listed: Vec<Object>,
    networks: Vec<Object>,
    nodes: Vec<Object>,
    _pool: Object,
    _manager: Object,
    _near: Object,
    _app: Object,
    _pool_global: Box<*mut u8>,
    _manager_global: Box<*mut u8>,
    _dx: Box<[i32; 4]>,
    _dz: Box<[i32; 4]>,
    _tables: Vec<Vec<usize>>,
    _fakes: Vec<Fake>,
    results: Vec<Object>,
}
impl World {
    fn new() -> Self {
        let tables = vec![
            table(&[(0x38, keys as *const () as usize), (0xC, path as *const () as usize)]),
            table(&[
                (0xC, allocate as *const () as usize),
                (0x10, free_header as *const () as usize),
            ]),
            table(&[(0xA4, get_occupants as *const () as usize)]),
            table(&[(0x1C, near as *const () as usize)]),
            table(&[
                (0, query as *const () as usize),
                (8, release_occupant as *const () as usize),
            ]),
            table(&[
                (0x24, flags as *const () as usize),
                (0xC4, get_paths as *const () as usize),
                (8, release_network as *const () as usize),
            ]),
            table(&[(0x14, service as *const () as usize)]),
        ];
        let mut pool = object(0x20, &tables[1]);
        let mut manager = object(0x20, &tables[2]);
        let near_obj = object(0x20, &tables[3]);
        let app = object(0x20, &tables[6]);
        let mut pool_global = Box::new(pool.ptr());
        let mut manager_global = Box::new(manager.ptr());
        let mut dx = Box::new([-1, 0, 1, 0]);
        let mut dz = Box::new([0, -1, 0, 1]);
        let fakes = vec![
            fake(0x0053_DC50, reserve as *const () as usize),
            fake(0x0050_C3B0, insert as *const () as usize),
            fake(0x0087_93EC, framework_fake as *const () as usize),
            fake(0x0041_1870, holder as *const () as usize),
            fake(0x0041_3E60, clear as *const () as usize),
            fake(0x0090_CF63, free as *const () as usize),
            fake_data(0x00B2_07E4, (&mut *pool_global as *mut *mut u8).cast()),
            fake_data(0x00B4_3D0C, (&mut *manager_global as *mut *mut u8).cast()),
            fake_data(0x00AB_325C, dx.as_mut_ptr().cast()),
            fake_data(0x00AB_326C, dz.as_mut_ptr().cast()),
        ];
        let mut w = Self {
            sim: Object::new(0xD50),
            path: object(0x20, &tables[0]),
            keys: Object::new(32),
            points: Object::new(64),
            header: Object::new(16),
            intersections: Object::new(46 * 65536),
            vertices: vec![],
            info: vec![],
            listed: vec![],
            networks: vec![],
            nodes: vec![],
            _pool: pool,
            _manager: manager,
            _near: near_obj,
            _app: app,
            _pool_global: pool_global,
            _manager_global: manager_global,
            _dx: dx,
            _dz: dz,
            _tables: tables,
            _fakes: fakes,
            results: vec![],
        };
        unsafe {
            core::ptr::write_bytes(w.intersections.ptr(), 0xA5, 46 * 65536);
            ElevatedSim::from_ptr(w.sim.ptr()).set_intersections(w.intersections.ptr());
        }
        STATE.with(|s| {
            *s.borrow_mut() = State {
                keys: [w.keys.ptr() as u32, w.keys.ptr() as u32, 0],
                point_memory: w.points.ptr() as usize,
                reserve_count: 4,
                header: w.header.ptr() as usize,
                path_info: w._near.ptr() as usize,
                pool: w._pool.ptr() as usize,
                app: w._app.ptr() as usize,
                pool_global: (&mut *w._pool_global as *mut *mut u8) as usize,
                query: [1; 3],
                flags: [1; 3],
                ..State::default()
            }
        });
        w
    }
    fn set_keys(&mut self, keys: &[u32]) {
        unsafe {
            for (i, &k) in keys.iter().enumerate() {
                self.keys.ptr().cast::<u32>().add(i).write(k);
            }
        }
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.count = keys.len() as u32;
            s.keys[1] = s.keys[0] + keys.len() as u32 * 4;
        });
    }
    fn vertex(&mut self, key: u32, words: [u32; 3]) {
        let mut v = Object::new(12);
        let mut info = Object::new(4);
        unsafe {
            for (i, &word) in words.iter().enumerate() {
                v.ptr().cast::<u32>().add(i).write(word);
            }
            store(info.ptr(), 0, v.ptr() as u32);
        }
        STATE.with(|s| s.borrow_mut().vertices.push((key, info.ptr() as usize)));
        self.vertices.push(v);
        self.info.push(info);
    }
    fn occupant(&mut self) {
        let id = self.listed.len() as u32;
        let mut occ = object(16, &self._tables[4]);
        let mut net = object(16, &self._tables[5]);
        let mut node = Object::new(12);
        unsafe {
            store(occ.ptr(), 4, id);
            store(net.ptr(), 4, id);
        }
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.listed.push(occ.ptr() as usize);
            s.networks.push(net.ptr() as usize);
            s.nodes.push(node.ptr() as usize);
        });
        self.listed.push(occ);
        self.networks.push(net);
        self.nodes.push(node);
    }
    fn near_result(&mut self, id: u32, point: u32, keys: &[u32]) {
        let mut memory = Object::new(keys.len() * 4 + 16);
        unsafe {
            for (i, &k) in keys.iter().enumerate() {
                memory.ptr().cast::<u32>().add(i).write(k);
            }
        }
        STATE.with(|s| {
            s.borrow_mut()
                .near
                .push((id, point, memory.ptr() as u32, keys.len() as u32))
        });
        self.results.push(memory);
    }
    fn run(&mut self, x: u32, z: u32, entry: u32, exit: u32) -> [u32; 5] {
        let mut frame = [
            fc4p_00715940 as *const () as u32,
            self.sim.ptr() as u32,
            x,
            z,
            self.path.ptr() as u32,
            entry,
            exit,
            0,
            0,
            0,
            0,
            0,
        ];
        unsafe {
            core::arch::asm!("push [edi + 24]","push [edi + 20]","push [edi + 16]","push [edi + 12]","push [edi + 8]","mov ecx,[edi + 4]","call dword ptr [edi]",
            "mov eax,[esp - 20]","mov [edi + 28],eax","mov eax,[esp - 16]","mov [edi + 32],eax","mov eax,[esp - 12]","mov [edi + 36],eax",
            "mov eax,[esp - 8]","mov [edi + 40],eax","mov eax,[esp - 4]","mov [edi + 44],eax",in("edi")frame.as_mut_ptr(),clobber_abi("C"));
        }
        frame[7..12].try_into().unwrap()
    }
    fn direction(&mut self, x: u32, z: u32, entry: u32) -> *mut u16 {
        self.intersections
            .ptr()
            .wrapping_add(((x << 8 | z) * 46 + 0x24 + entry * 2) as usize)
            .cast()
    }
}

#[test]
fn lane_continuity_worked_example_and_full_call_order() {
    let mut w = World::new();
    let point = [168f32.to_bits(), 270f32.to_bits(), 160f32.to_bits()];
    w.set_keys(&[0x0100_0200]);
    w.vertex(0x0100_0200, point);
    w.occupant();
    w.near_result(0, point[0], &[0x0100_0000]);
    unsafe {
        w.direction(10, 10, 2).write_unaligned(0xFFFF);
    }
    let mut expected = unsafe { bytes(w.intersections.ptr(), 46 * 65536) };
    let offset = ((10 * 256 + 10) * 46 + 0x24 + 4) as usize;
    expected[offset..offset + 2].copy_from_slice(&0xFFF1u16.to_le_bytes());
    let env = float_environment();
    let args = w.run(10, 10, 2, 0);
    assert_eq!(args, [10, 10, w.header.ptr() as u32, 2, 0]);
    assert_eq!(unsafe { bytes(w.intersections.ptr(), 46 * 65536) }, expected);
    assert_eq!(float_environment(), env);
    assert_eq!(
        calls(),
        [
            Call::Keys(1, 2),
            Call::Reserve(1),
            Call::Path(0x0100_0200, 0),
            Call::Allocate(12),
            Call::Occupants([11, 10], true, 0xC772_BF98, u32::MAX),
            Call::Query(0, 0xA821_EF94),
            Call::Flags(0, 0x1F4F),
            Call::Paths(0),
            Call::Near(0, 1, point, 255, 0, 0x4080_0000),
            Call::Free(w.results[0].ptr() as u32),
            Call::ReleaseNetwork(0),
            Call::ReleaseOccupant(0),
            Call::Clear(w.header.ptr() as u32),
            Call::FreeHeader(w.header.ptr() as u32),
            Call::Free(w.points.ptr() as u32),
            Call::Free(w.keys.ptr() as u32)
        ]
    );
}

#[test]
fn unmatched_exits_and_null_paths_stop_before_occupant_lookup() {
    let mut w = World::new();
    w.set_keys(&[0x0100_0201, 0x0100_0300]);
    let before = unsafe { bytes(w.intersections.ptr(), 64) };
    let path = w.path.ptr() as u32;
    assert_eq!(w.run(0, 0, 2, 0), [0, 0, path, 2, 0]);
    assert_eq!(unsafe { bytes(w.intersections.ptr(), 64) }, before);
    assert_eq!(
        calls(),
        [
            Call::Keys(1, 2),
            Call::Reserve(2),
            Call::Path(0x0100_0300, 0),
            Call::Free(w.points.ptr() as u32),
            Call::Free(w.keys.ptr() as u32)
        ]
    );
    STATE.with(|s| s.borrow_mut().keys = [0; 3]);
    w.set_keys(&[]);
    STATE.with(|s| s.borrow_mut().point_memory = 0);
    w.run(0, 0, 0, 0);
    assert_eq!(calls(), [Call::Keys(1, 0), Call::Reserve(0)]);
}

#[test]
fn last_qualifying_occupant_replaces_the_nibble_and_unmasked_entries_spill() {
    let mut w = World::new();
    w.set_keys(&[0x0100_0201, 0x0200_0201]);
    w.vertex(0x0100_0201, [1, 2, 3]);
    w.vertex(0x0200_0201, [4, 5, 6]);
    w.occupant();
    w.occupant();
    w.near_result(0, 1, &[0x0100_0000]);
    w.near_result(0, 4, &[0x0100_0200]);
    w.near_result(1, 1, &[0x0100_2100]);
    w.near_result(1, 4, &[0x0100_0400]);
    unsafe {
        w.direction(0, 0, 2).write_unaligned(0xFFFF);
    }
    w.run(0, 0, 2, 1);
    assert_eq!(unsafe { w.direction(0, 0, 2).read_unaligned() }, 0xFF2F);
    let c = calls();
    assert_eq!(c.iter().filter(|c| matches!(c, Call::Near(..))).count(), 4);
    assert!(c
        .windows(2)
        .any(|c| c == [Call::ReleaseNetwork(0), Call::ReleaseOccupant(0)]));
}

#[test]
fn query_and_flag_low_bytes_and_null_path_info_skip_restriction_but_release() {
    let mut w = World::new();
    w.set_keys(&[0]);
    w.vertex(0, [1, 2, 3]);
    for _ in 0..3 {
        w.occupant();
    }
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.query = [0xABCD_0000, 0xABCD_0001, 1];
        s.flags = [1, 0x100, 1];
        s.null_paths = true;
    });
    let before = unsafe { w.direction(0, 0, 0).read_unaligned() };
    w.run(0, 0, 0, 0);
    assert_eq!(unsafe { w.direction(0, 0, 0).read_unaligned() }, before);
    let c = calls();
    assert!(!c.iter().any(|c| matches!(c, Call::Near(..))));
    assert!(!c.contains(&Call::ReleaseNetwork(0)));
    assert!(c.contains(&Call::ReleaseNetwork(1)) && c.contains(&Call::ReleaseNetwork(2)));
    assert_eq!(c.iter().filter(|c| matches!(c, Call::ReleaseOccupant(_))).count(), 3);
}

#[test]
fn empty_near_results_clear_the_nibble_and_free_allocated_empty_vectors() {
    let mut w = World::new();
    w.set_keys(&[0]);
    w.vertex(0, [1, 2, 3]);
    w.occupant();
    w.near_result(0, 1, &[]);
    unsafe {
        w.direction(0, 0, 0).write_unaligned(0xFFFF);
    }
    w.run(0, 0, 0, 0);
    assert_eq!(unsafe { w.direction(0, 0, 0).read_unaligned() }, 0xFFF0);
    assert!(calls().contains(&Call::Free(w.results[0].ptr() as u32)));
}

#[test]
fn allocator_is_acquired_lazily_and_again_after_list_cleanup() {
    let mut w = World::new();
    w.set_keys(&[0]);
    w.vertex(0, [1, 2, 3]);
    *w._pool_global = core::ptr::null_mut();
    STATE.with(|s| s.borrow_mut().reacquire = true);
    let args = w.run(0, 0, 3, 0);
    assert_eq!(args, [0, 0, w.header.ptr() as u32, 0, 0]);
    let c = calls();
    assert_eq!(c.iter().filter(|c| matches!(c, Call::Framework)).count(), 2);
    assert_eq!(c.iter().filter(|c| matches!(c, Call::Holder(_))).count(), 2);
    assert!(c.contains(&Call::Service(0x3AE4_BEA3, 0x3AE4_BEAB)));
}

#[test]
fn point_growth_uses_game_helper_with_the_original_direction_argument() {
    let mut w = World::new();
    w.set_keys(&[0]);
    w.vertex(0, [0x7F80_0001, 0x8000_0000, 0x7FC0_1234]);
    STATE.with(|s| s.borrow_mut().reserve_count = 0);
    w.run(0, 0, 3, 0);
    let c = calls();
    assert!(c.contains(&Call::Insert(3, 1, 1)));
    assert!(c.contains(&Call::Occupants([0, 1], true, 0xC772_BF98, u32::MAX)));
    assert_eq!(
        unsafe { bytes(w.points.ptr(), 12) },
        [0x7F80_0001u32, 0x8000_0000, 0x7FC0_1234]
            .iter()
            .flat_map(|x| x.to_le_bytes())
            .collect::<Vec<_>>()
    );
}

#[test]
fn lane_continuity_last_flagged_occupant_decides() {
    let mut w = World::new();
    w.set_keys(&[0x0100_0002]);
    w.vertex(0x0100_0002, [50, 0, 0]);
    w.occupant();
    w.occupant();
    w.near_result(0, 50, &[0x0100_0002, 0x0100_0302]);
    w.near_result(1, 50, &[0x0100_0102]);
    for entry in 0..4 {
        unsafe {
            w.direction(1, 1, entry).write_unaligned(0xFFFF);
        }
    }
    w.run(1, 1, 0, 2);
    assert_eq!(
        (0..4)
            .map(|entry| unsafe { w.direction(1, 1, entry).read_unaligned() })
            .collect::<Vec<_>>(),
        [0xF2FF, 0xFFFF, 0xFFFF, 0xFFFF]
    );
    assert!(calls().contains(&Call::Occupants([0, 1], true, 0xC772_BF98, u32::MAX)));
}

#[test]
fn lane_continuity_without_flagged_neighbour() {
    let mut w = World::new();
    w.set_keys(&[0x0100_0002]);
    w.vertex(0x0100_0002, [50, 0, 0]);
    w.occupant();
    STATE.with(|s| s.borrow_mut().flags[0] = 0);
    for entry in 0..4 {
        unsafe {
            w.direction(1, 1, entry).write_unaligned(0xFFFF);
        }
    }
    w.run(1, 1, 0, 2);
    assert!((0..4).all(|entry| unsafe { w.direction(1, 1, entry).read_unaligned() } == 0xFFFF));
    assert!(!calls().iter().any(|call| matches!(call, Call::Near(..))));
}
