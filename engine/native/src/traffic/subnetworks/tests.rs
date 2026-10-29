// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, fake_data, Fake, Object};
use crate::traffic::test_support::{bytes, float_environment, invoke, table};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    New(u32),
    Lots,
    Find(u32, i32, i32),
    Tunnel(u32),
    Push(u32),
    Free,
    Drop,
    Clear,
    Fill(i32, i32, u32, u32),
}

#[derive(Default)]
struct State {
    calls: Vec<Call>,
    // Block addresses must survive growth of the owner's vector.
    #[allow(clippy::vec_box)]
    blocks: Vec<Box<[u32; 32]>>,
    map: Vec<u32>,
    near_end: bool,
    tunnels: Vec<(u32, u32)>,
    label_second: bool,
    shrink: bool,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }

unsafe extern "thiscall" fn new_deque(this: *mut u8, size: u32) {
    unsafe {
        assert_eq!(bytes(this, 40), [0; 40]);
    }
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::New(size));
        let mut block = Box::new([0xDEAD_BEEF; 32]);
        let first = block.as_mut_ptr();
        let cur = first.wrapping_add(if s.near_end { 31 } else { 0 });
        s.map = vec![0; 64];
        s.map[0] = first as u32;
        let node = s.map.as_mut_ptr();
        s.blocks.push(block);
        unsafe {
            for (i, value) in [
                cur as u32,
                first as u32,
                first.wrapping_add(32) as u32,
                node as u32,
                cur as u32,
                first as u32,
                first.wrapping_add(32) as u32,
                node as u32,
                node as u32,
                64,
            ]
            .into_iter()
            .enumerate()
            {
                store(this, i * 4, value);
            }
        }
    });
}
unsafe extern "thiscall" fn drop_deque(_this: *mut u8) {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Drop));
}
unsafe extern "thiscall" fn push(this: *mut u8, value: *const u32) {
    unsafe {
        let deque = WorkDeque::from_ptr(this);
        assert_eq!(deque.end(), deque.last().wrapping_sub(1));
        let value = value.read();
        deque.end().write(value);
        let mut block = Box::new([0xDEAD_BEEF; 32]);
        let first = block.as_mut_ptr();
        let node = deque.node().add(1);
        node.write(first);
        deque.set_node(node);
        deque.set_first(first);
        deque.set_last(first.add(32));
        deque.set_end(first);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.calls.push(Call::Push(value));
            s.blocks.push(block);
        });
    }
}
unsafe extern "C" fn free_block(memory: *mut u8) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert!(s.blocks.iter().any(|b| b.as_ptr().cast::<u8>() == memory));
        s.calls.push(Call::Free);
    });
}
unsafe extern "thiscall" fn lots(this: *mut u8) -> *mut u8 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Lots));
    this.wrapping_add(4)
}
unsafe extern "thiscall" fn find(this: *mut u8, kind: u32, x: i32, z: i32) -> *mut u8 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Find(kind, x, z)));
    unsafe { first_info(SubnetworkSim::from_ptr(this), x, z, kind) }
}
unsafe extern "thiscall" fn tunnel(_this: *mut u8, result: *mut u32, key: *const u32) {
    unsafe {
        let key = key.read();
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.calls.push(Call::Tunnel(key));
            result
                .add(1)
                .write(s.tunnels.iter().find(|&&(k, _)| k == key).map_or(0, |&(_, p)| p));
        });
    }
}
unsafe extern "thiscall" fn clear(_this: *mut u8) {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Clear));
}
unsafe extern "thiscall" fn fill(this: *mut u8, x: i32, z: i32, kind: u32, entry: u32) {
    unsafe {
        let sim = SubnetworkSim::from_ptr(this);
        let info = first_info(sim, x, z, kind);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.calls.push(Call::Fill(x, z, kind, entry));
            if s.label_second && entry & 1 == 0 {
                label(info, entry + 1).write_unaligned(7);
            }
            if s.shrink {
                sim.set_max_z(1);
                sim.set_max_x(1);
            }
        });
    }
}

struct Fixture {
    sim_object: Object,
    sim: SubnetworkSim,
    _table: Vec<usize>,
    _lots: Object,
    _lot_rows: Vec<[u32; 3]>,
    lot_cells: Vec<Vec<u32>>,
    _rows: Vec<[u32; 3]>,
    cells: Vec<Vec<u32>>,
    records: Vec<Object>,
    connections: Vec<u16>,
    _travel: Box<[u32; 13]>,
    _dx: Box<[i32; 4]>,
    _dz: Box<[i32; 4]>,
    _opposite: Box<[u32; 4]>,
    _guards: Vec<Fake>,
}
impl Fixture {
    fn new() -> Self {
        STATE.with(|s| *s.borrow_mut() = State::default());
        let mut sim_object = Object::new(0xD00);
        let sim = unsafe { SubnetworkSim::from_ptr(sim_object.ptr()) };
        sim.set_min_x(0);
        sim.set_min_z(0);
        sim.set_max_x(3);
        sim.set_max_z(3);
        sim.set_next_label(1);
        let mut cells: Vec<Vec<u32>> = (0..4).map(|_| vec![0; 4]).collect();
        let mut rows: Vec<[u32; 3]> = cells.iter_mut().map(|r| [r.as_mut_ptr() as u32, 0, 0]).collect();
        sim.set_rows(rows.as_mut_ptr().cast());
        let mut lot_cells: Vec<Vec<u32>> = (0..4).map(|_| vec![0; 4]).collect();
        let mut lot_rows: Vec<[u32; 3]> = lot_cells.iter_mut().map(|r| [r.as_mut_ptr() as u32, 0, 0]).collect();
        let table = table(&[(0x60, lots as *const () as usize)]);
        let mut lot_object = Object::new(8);
        unsafe {
            store(lot_object.ptr(), 0, table.as_ptr() as u32);
            store(lot_object.ptr(), 4, lot_rows.as_mut_ptr() as u32);
        }
        sim.set_lots(lot_object.ptr());
        let mut connections = vec![0u16; 4 * 256 * 23];
        sim.set_intersections(connections.as_mut_ptr().cast());
        let mut travel = Box::new([0; 13]);
        travel[1] = 1;
        let mut dx = Box::new([-1, 0, 1, 0]);
        let mut dz = Box::new([0, -1, 0, 1]);
        let mut opposite = Box::new([2, 3, 0, 1]);
        let guards = vec![
            fake(0x0074_5980, new_deque as *const () as usize),
            fake(0x0071_3E90, drop_deque as *const () as usize),
            fake(0x0062_66E0, push as *const () as usize),
            fake(0x0090_CF63, free_block as *const () as usize),
            fake(0x0070_FB30, find as *const () as usize),
            fake(0x0060_D170, tunnel as *const () as usize),
            fake_data(0x00AB_3244, travel.as_mut_ptr().cast()),
            fake_data(0x00AB_325C, dx.as_mut_ptr().cast()),
            fake_data(0x00AB_326C, dz.as_mut_ptr().cast()),
            fake_data(0x00AB_3394, opposite.as_mut_ptr().cast()),
        ];
        Self {
            sim_object,
            sim,
            _table: table,
            _lots: lot_object,
            _lot_rows: lot_rows,
            lot_cells,
            _rows: rows,
            cells,
            records: vec![],
            connections,
            _travel: travel,
            _dx: dx,
            _dz: dz,
            _opposite: opposite,
            _guards: guards,
        }
    }
    fn add(&mut self, x: usize, z: usize, kind: u8) -> *mut u8 {
        let mut object = Object::new(24);
        let p = object.ptr();
        unsafe {
            p.add(6).write(kind);
            store(p, 20, self.cells[x][z]);
        }
        self.cells[x][z] = p as u32;
        self.records.push(object);
        p
    }
    fn bits(&mut self, x: usize, z: usize, value: u16) {
        self.connections[(x * 256 + z) * 23 + 1] = value;
    }
    fn portal(&mut self, x: usize, z: usize, to_x: u8, to_z: u8) {
        self.connections[(x * 256 + z) * 23 + 22] |= 2;
        let mut portal = Object::new(12);
        unsafe {
            portal.ptr().add(8).write(to_x);
            portal.ptr().add(9).write(to_z);
            portal.ptr().add(10).cast::<u16>().write_unaligned(2);
        }
        STATE.with(|s| {
            s.borrow_mut()
                .tunnels
                .push((((x as u32) << 8) | z as u32, portal.ptr() as u32))
        });
        self.records.push(portal);
    }
    fn run(&mut self, x: i32, z: i32, e: u32) {
        unsafe {
            fc4p_00717ec0(self.sim.as_ptr(), x, z, 1, e);
        }
    }
    fn labels(p: *mut u8) -> [u16; 4] {
        unsafe { Info::from_ptr(p).labels() }
    }
    fn calls(&self) -> Vec<Call> {
        STATE.with(|s| std::mem::take(&mut s.borrow_mut().calls))
    }
    fn update(&mut self) {
        let environment = float_environment();
        let this = self.sim.as_ptr();
        let result = unsafe {
            invoke(
                fc4p_007183b0 as *const () as usize,
                this,
                &[],
                0xABCD_EF01,
                0x1234_5678,
                false,
            )
        };
        assert_eq!(result.ecx, this as u32);
        assert_eq!(environment, float_environment());
    }
}

#[test]
fn seed_order_last_row_and_column_and_record_chain() {
    let mut f = Fixture::new();
    for x in 0..4 {
        for z in 0..4 {
            f.add(x, z, 1);
        }
    }
    f.add(1, 1, 8);
    f.cells[0][1] = 0;
    f.cells[0][2] = 0;
    let _clear = fake(0x0070_FCE0, clear as *const () as usize);
    let _fill = fake(0x0071_7EC0, fill as *const () as usize);
    let before = unsafe { bytes(f.sim_object.ptr(), 0xD00) };
    f.update();
    let mut expected = vec![Call::Clear];
    for x in 0..3 {
        for z in 0..3 {
            if f.cells[x][z] != 0 {
                for e in [0, 1] {
                    expected.push(Call::Fill(x as i32, z as i32, 1, e));
                }
            }
        }
    }
    for x in (0..3).rev() {
        for z in (0..3).rev() {
            if f.cells[x][z] != 0 {
                for e in [2, 3] {
                    expected.push(Call::Fill(x as i32, z as i32, 1, e));
                }
            }
        }
    }
    assert_eq!(f.calls(), expected);
    assert_eq!(unsafe { bytes(f.sim_object.ptr(), 0xD00) }, before);
}

#[test]
fn seeds_recheck_labels_and_dimensions_after_each_fill() {
    let mut f = Fixture::new();
    f.add(0, 0, 1);
    let _clear = fake(0x0070_FCE0, clear as *const () as usize);
    let _fill = fake(0x0071_7EC0, fill as *const () as usize);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.label_second = true;
        s.shrink = true;
    });
    f.update();
    assert_eq!(f.calls(), [Call::Clear, Call::Fill(0, 0, 1, 0), Call::Fill(0, 0, 1, 2)]);
}

#[test]
fn empty_and_negative_seed_dimensions_still_clear() {
    for (x, z) in [(0, 3), (-1, 3), (3, 0), (3, -1)] {
        let mut f = Fixture::new();
        f.sim.set_max_x(x);
        f.sim.set_max_z(z);
        let _clear = fake(0x0070_FCE0, clear as *const () as usize);
        let _fill = fake(0x0071_7EC0, fill as *const () as usize);
        f.update();
        assert_eq!(f.calls(), [Call::Clear]);
    }
}

#[test]
fn three_cell_street_labels_and_empty_entry_match_core_cases() {
    let mut f = Fixture::new();
    let a = f.add(1, 1, 1);
    let b = f.add(2, 1, 1);
    let c = f.add(3, 1, 1);
    f.bits(1, 1, 1 << 10);
    f.bits(2, 1, (1 << 2) | (1 << 8));
    f.bits(3, 1, 1);
    let before = unsafe { bytes(f.sim_object.ptr(), 0xD00) };
    f.run(1, 1, 0);
    assert_eq!(f.sim.next_label(), 1);
    assert_eq!(f.calls(), [Call::New(0), Call::Lots, Call::Drop]);
    assert_eq!(unsafe { bytes(f.sim_object.ptr(), 0xD00) }, before);
    f.run(1, 1, 2);
    assert_eq!(Fixture::labels(a), [0, 0, 1, 0]);
    assert_eq!(Fixture::labels(b), [1, 0, 1, 0]);
    assert_eq!(Fixture::labels(c), [1, 0, 0, 0]);
    assert_eq!(f.sim.next_label(), 2);
    assert_eq!(
        f.calls(),
        [
            Call::New(0),
            Call::Lots,
            Call::Find(1, 2, 1),
            Call::Find(1, 3, 1),
            Call::Find(1, 2, 1),
            Call::Find(1, 1, 1),
            Call::Drop
        ]
    );
}

#[test]
fn rejected_seeds_construct_and_destroy_the_deque_without_querying_lots() {
    for condition in 0..5 {
        let mut f = Fixture::new();
        if condition != 0 {
            let p = f.add(1, 1, if condition == 1 { 2 } else { 1 });
            unsafe {
                if condition == 2 {
                    label(p, 0).write_unaligned(5);
                }
                if condition == 3 {
                    p.add(4).write(3);
                }
            }
            if condition == 4 {
                f.portal(1, 1, 2, 1);
            }
        }
        f.run(1, 1, 0);
        assert_eq!(f.calls(), [Call::New(0), Call::Drop]);
        assert_eq!(f.sim.next_label(), 1);
    }
}

#[test]
fn depth_first_traversal_uses_the_game_deque_at_block_boundaries() {
    let mut f = Fixture::new();
    let a = f.add(1, 1, 1);
    let b = f.add(2, 1, 1);
    let c = f.add(3, 1, 1);
    f.bits(1, 1, 1 << 2);
    f.bits(2, 1, 1 << 2);
    f.bits(3, 1, 1);
    STATE.with(|s| s.borrow_mut().near_end = true);
    let env = float_environment();
    f.run(1, 1, 0);
    assert_eq!(Fixture::labels(a), [1, 0, 0, 0]);
    assert_eq!(Fixture::labels(b), [1, 0, 0, 0]);
    assert_eq!(Fixture::labels(c), [1, 0, 0, 0]);
    assert_eq!(
        f.calls(),
        [
            Call::New(0),
            Call::Lots,
            Call::Find(1, 2, 1),
            Call::Push(u32::from_le_bytes([1, 1, 0, 3])),
            Call::Find(1, 3, 1),
            Call::Find(1, 2, 1),
            Call::Free,
            Call::Drop
        ]
    );
    assert_eq!(f.sim.next_label(), 2);
    assert_eq!(env, float_environment());
}

#[test]
fn off_map_connections_turn_back_only_when_the_reciprocal_bit_exists() {
    for reciprocal in [false, true] {
        let mut f = Fixture::new();
        let a = f.add(0, 1, 1);
        f.bits(0, 1, (1 << 8) | if reciprocal { 1 << 2 } else { 0 });
        f.run(0, 1, 2);
        assert_eq!(Fixture::labels(a), if reciprocal { [1, 0, 1, 0] } else { [0, 0, 1, 0] });
        let expected = if reciprocal {
            vec![Call::New(0), Call::Lots, Call::Find(1, 1, 1), Call::Drop]
        } else {
            vec![Call::New(0), Call::Lots, Call::Drop]
        };
        assert_eq!(f.calls(), expected);
    }
}

#[test]
fn switch_lots_skip_equal_ids_and_stop_at_the_first_different_cell() {
    let mut f = Fixture::new();
    let a = f.add(0, 1, 1);
    let b = f.add(3, 1, 1);
    f.bits(0, 1, 1 << 2);
    f.bits(3, 1, 1);
    f.lot_cells[1][1] = 10;
    f.lot_cells[2][1] = 10;
    f.run(0, 1, 0);
    assert_eq!(Fixture::labels(a), [1, 0, 0, 0]);
    assert_eq!(Fixture::labels(b), [1, 0, 0, 0]);
    assert_eq!(
        f.calls(),
        [
            Call::New(0),
            Call::Lots,
            Call::Find(1, 1, 1),
            Call::Find(1, 3, 1),
            Call::Find(1, 2, 1),
            Call::Find(1, 0, 1),
            Call::Drop
        ]
    );
}

#[test]
fn tunnel_resume_runs_once_and_uses_the_same_entry_at_the_other_portal() {
    let mut f = Fixture::new();
    let a = f.add(0, 1, 1);
    let b = f.add(1, 1, 1);
    let c = f.add(3, 1, 1);
    f.bits(0, 1, 1 << 2);
    f.portal(1, 1, 3, 1);
    f.portal(3, 1, 1, 1);
    f.run(0, 1, 0);
    assert_eq!(Fixture::labels(a), [1, 0, 0, 0]);
    assert_eq!(Fixture::labels(b), [1, 0, 0, 0]);
    assert_eq!(Fixture::labels(c), [1, 0, 0, 0]);
    assert_eq!(
        f.calls(),
        [
            Call::New(0),
            Call::Lots,
            Call::Find(1, 1, 1),
            Call::Tunnel(0x101),
            Call::Find(1, 3, 1),
            Call::Tunnel(0x301),
            Call::Find(1, 1, 1),
            Call::Drop
        ]
    );
    assert_eq!(f.sim.next_label(), 2);
}

#[test]
fn label_counter_wraps_and_connected_flags_use_the_low_byte_at_four() {
    let mut f = Fixture::new();
    let a = f.add(0, 1, 1);
    unsafe {
        a.add(4).write(2);
        a.add(7).write(255);
    }
    f.bits(0, 1, 1 << 8);
    f.sim.set_next_label(u16::MAX);
    f.run(0, 1, 2);
    assert_eq!(Fixture::labels(a), [0, 0, u16::MAX, 0]);
    assert_eq!(f.sim.next_label(), 0);
}

#[test]
fn a_street_on_the_last_column_is_not_a_seed_but_can_be_filled_directly() {
    let mut f = Fixture::new();
    let c = f.add(3, 1, 1);
    f.bits(3, 1, 1);
    f.update();
    assert_eq!(Fixture::labels(c), [0; 4]);
    assert_eq!(f.sim.next_label(), 1);
    assert!(f.calls().is_empty());
    f.run(3, 1, 0);
    assert_eq!(Fixture::labels(c), [1, 0, 0, 0]);
    assert_eq!(f.sim.next_label(), 2);
    assert_eq!(f.calls(), [Call::New(0), Call::Lots, Call::Find(1, 2, 1), Call::Drop]);
}
