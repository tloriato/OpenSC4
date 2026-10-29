// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Corner-refresh cases from the core's NT30 fixtures, with the original cell and tool storage.

use super::*;
use crate::testing::{fake, Fake, GameFpu, Object};
use std::cell::RefCell;
use std::collections::BTreeMap;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Begin(usize),
    Find(u32),
    Height(u32),
    Occupant(u32, u32, u32),
    Query(u32, u32, u32),
    Interface(u32),
    Piece,
    Orientation,
    Release,
    DirectHeight(usize, u32, usize),
    Grow(usize, usize, [u32; 3], u32, u32),
    Allocate(u32),
    Free(usize),
}
#[derive(Default)]
struct State {
    calls: Vec<Call>,
    first: usize,
    table: usize,
    tool: usize,
    cells: BTreeMap<u32, usize>,
    heights: BTreeMap<u32, f32>,
    direct: BTreeMap<u32, f32>,
    occupant: usize,
    allocation: usize,
    grow_buffer: usize,
    query_result: u32,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn record(call: Call) {
    STATE.with(|s| s.borrow_mut().calls.push(call));
}
fn calls() -> Vec<Call> {
    STATE.with(|s| core::mem::take(&mut s.borrow_mut().calls))
}

extern "thiscall" fn begin(this: *mut u8, iterator: *mut u32) -> u64 {
    record(Call::Begin(this as usize));
    STATE.with(|s| {
        let s = s.borrow();
        unsafe {
            iterator.write(s.first as u32);
            iterator.add(1).write(s.table as u32);
        }
    });
    registers(iterator as u32, 0xAABB_CCDD)
}
extern "thiscall" fn find(_this: *mut u8, key: u32) -> u64 {
    record(Call::Find(key));
    let p = STATE.with(|s| s.borrow().cells.get(&key).copied().unwrap_or(0));
    registers(p as u32, 0x9988_7766)
}
extern "thiscall" fn height(_this: *mut u8, vertex: u32) -> f32 {
    record(Call::Height(vertex));
    STATE.with(|s| s.borrow().heights.get(&vertex).copied().unwrap_or(100.0))
}
extern "thiscall" fn occupant(_this: *mut u8, x: u32, z: u32, filter: u32, out: *mut *mut u8) {
    record(Call::Occupant(x, z, filter));
    unsafe {
        *out = STATE.with(|s| s.borrow().occupant as *mut u8);
    }
}
extern "thiscall" fn query(_this: *mut u8, out: *mut *mut u8, x: u32, z: u32, filter: u32) -> u32 {
    record(Call::Query(x, z, filter));
    STATE.with(|s| {
        let s = s.borrow();
        unsafe {
            *out = s.occupant as *mut u8;
        }
        s.query_result
    })
}
extern "thiscall" fn interface(this: *mut u8, iid: u32, out: *mut *mut u8) {
    record(Call::Interface(iid));
    unsafe {
        *out = this;
    }
}
extern "thiscall" fn piece(_this: *mut u8) -> u32 {
    record(Call::Piece);
    0x1234_5678
}
extern "thiscall" fn orientation(_this: *mut u8) -> u32 {
    record(Call::Orientation);
    0xABCD_EF03
}
extern "thiscall" fn release(_this: *mut u8) -> u64 {
    record(Call::Release);
    registers(77, 0x1020_3040)
}
extern "thiscall" fn direct_height(this: *mut u8, vertex: u32, out: *mut u8) -> u64 {
    record(Call::DirectHeight(this as usize, vertex, out as usize));
    STATE.with(|s| {
        if let Some(&h) = s.borrow().direct.get(&vertex) {
            unsafe {
                write(out, 0, h);
            }
        }
    });
    registers(1, 0x5566_7788)
}
extern "thiscall" fn grow(this: *mut u8, position: *mut u8, r: *mut u32, scratch: *mut u8, count: u32, copy: u32) {
    let r = unsafe { [*r, *r.add(1), *r.add(2)] };
    record(Call::Grow(this as usize, position as usize, r, count, copy));
    assert_eq!(unsafe { *scratch }, 1);
    STATE.with(|s| {
        let s = s.borrow();
        let tool = unsafe { NetworkTool::from_ptr(s.tool as *mut u8) };
        let p = s.grow_buffer as *mut u8;
        unsafe {
            store_record(p, &r);
        }
        tool.set_solved_begin(p);
        tool.set_solved_end(p.wrapping_add(12));
    });
}
extern "C" fn alloc(size: u32) -> *mut u8 {
    record(Call::Allocate(size));
    STATE.with(|s| s.borrow().allocation as *mut u8)
}
extern "C" fn delete(p: *mut u8) -> u64 {
    record(Call::Free(p as usize));
    registers(1, 0xBBCC_DDEE)
}

#[unsafe(naked)]
unsafe extern "C" fn position_seeded() {
    core::arch::naked_asm!("mov dword ptr [esp - 8], 0xdecafbad", "jmp {entry}", entry = sym fc4p_00629cf0);
}
#[unsafe(naked)]
unsafe extern "C" fn terrain_seeded() {
    core::arch::naked_asm!("mov dword ptr [esp - 8], 0xdecafbad", "jmp {entry}", entry = sym fc4p_00629eb0);
}

struct World {
    tool: Object,
    cells: Vec<Object>,
    buckets: Box<[u32; 8]>,
    // Bucket links must remain valid when the owning vector grows.
    #[allow(clippy::vec_box)]
    nodes: Vec<Box<[u32; 2]>>,
    solved: [u32; 48],
    manager: Box<[usize; 1]>,
    occupant: Box<[usize; 1]>,
    _tool_vt: Box<[usize; 80]>,
    _manager_vt: Box<[usize; 40]>,
    _occupant_vt: Box<[usize; 30]>,
    _fakes: Vec<Fake>,
}
impl World {
    fn new() -> Self {
        STATE.with(|s| {
            *s.borrow_mut() = State {
                query_result: 1,
                ..State::default()
            }
        });
        let mut tool_vt = Box::new([0usize; 80]);
        tool_vt[0x124 / 4] = height as *const () as usize;
        let mut manager_vt = Box::new([0usize; 40]);
        manager_vt[0x80 / 4] = query as *const () as usize;
        let mut occupant_vt = Box::new([0usize; 30]);
        occupant_vt[0] = interface as *const () as usize;
        occupant_vt[2] = release as *const () as usize;
        occupant_vt[0x5C / 4] = piece as *const () as usize;
        occupant_vt[0x68 / 4] = orientation as *const () as usize;
        let manager = Box::new([manager_vt.as_ptr() as usize]);
        let occupant_object = Box::new([occupant_vt.as_ptr() as usize]);
        let mut w = Self {
            tool: Object::new(0x400),
            cells: Vec::new(),
            buckets: Box::new([0; 8]),
            nodes: Vec::new(),
            solved: [0xCDCD_CDCD; 48],
            manager,
            occupant: occupant_object,
            _tool_vt: tool_vt,
            _manager_vt: manager_vt,
            _occupant_vt: occupant_vt,
            _fakes: [
                (0x004B_0390, begin as *const ()),
                (0x0063_3220, find as *const ()),
                (0x0062_4B70, occupant as *const ()),
                (0x0065_EB00, direct_height as *const ()),
                (0x0062_6240, grow as *const ()),
                (0x0090_CF54, alloc as *const ()),
                (0x0090_CF63, delete as *const ()),
            ]
            .into_iter()
            .map(|(a, f)| fake(a, f as usize))
            .collect(),
        };
        let tool = unsafe { NetworkTool::from_ptr(w.tool.ptr()) };
        tool.set_vtable(w._tool_vt.as_ptr() as u32);
        tool.set_width(128);
        tool.set_height(128);
        tool.set_vertex_stride(129);
        tool.set_occupant_manager(w.manager.as_mut_ptr().cast());
        unsafe {
            write(tool.as_ptr(), 0x364, 0x55AAu32);
        }
        w
    }
    fn prepare(&mut self, used: usize, capacity: usize) {
        let p = self.tool.ptr();
        unsafe {
            let t = NetworkTool::from_ptr(p);
            t.set_solved_begin(self.solved.as_mut_ptr().cast());
            t.set_solved_end(self.solved.as_mut_ptr().add(used * 3).cast());
            RefreshTool::from_ptr(p).set_solved_capacity(self.solved.as_mut_ptr().add(capacity * 3).cast());
            write(p, 0xA8, self.buckets.as_mut_ptr());
            write(p, 0xAC, self.buckets.as_mut_ptr().add(8));
        }
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.tool = p as usize;
            s.table = p as usize + 0xA4;
            s.occupant = self.occupant.as_mut_ptr() as usize;
        });
    }
    fn cell(&mut self, x: u32, z: u32, placed: bool, solved: i32, corners: [f32; 4]) -> *mut u8 {
        let mut cell = Object::new(0xC0);
        let p = cell.ptr();
        unsafe {
            let c = RefreshCell::from_ptr(p);
            c.set_x(x);
            c.set_z(z);
            c.set_placed(u8::from(placed));
            c.set_solved_index(solved);
            let vertices = [z * 129 + x, (z + 1) * 129 + x, (z + 1) * 129 + x + 1, z * 129 + x + 1];
            for i in 0..4 {
                write(p, 0x58 + i * 4, corners[i]);
                write(p, 0xA8 + i * 4, vertices[i]);
            }
        }
        STATE.with(|s| s.borrow_mut().cells.insert((z << 16) + x, p as usize));
        self.cells.push(cell);
        p
    }
    fn smoothing(&mut self, keys: &[u32]) {
        let mut first = 0;
        for &key in keys.iter().rev() {
            let mut node = Box::new([self.buckets[key as usize % 8], key]);
            let p = node.as_mut_ptr() as u32;
            self.buckets[key as usize % 8] = p;
            first = p;
            self.nodes.push(node);
        }
        STATE.with(|s| s.borrow_mut().first = first as usize);
    }
    fn position(&mut self) -> (u32, u32) {
        let mut c = RawCall::new(
            Conv::Thiscall,
            position_seeded as *const () as usize,
            self.tool.ptr() as u32,
            &[],
        );
        c.edx = 0x1122_3344;
        unsafe {
            call_raw(&mut c);
        }
        (c.eax, c.edx)
    }
    fn terrain(&mut self, vertices: &[u32]) -> (u32, u32) {
        let v = [
            vertices.as_ptr() as u32,
            vertices.as_ptr().wrapping_add(vertices.len()) as u32,
        ];
        let args = [v.as_ptr() as u32];
        let mut c = RawCall::new(
            Conv::Thiscall,
            terrain_seeded as *const () as usize,
            self.tool.ptr() as u32,
            &args,
        );
        c.edx = 0x1122_3344;
        unsafe {
            call_raw(&mut c);
        }
        (c.eax, c.edx)
    }
}

unsafe fn corners(p: *mut u8) -> [u32; 4] {
    core::array::from_fn(|i| unsafe { read(p, 0x58 + 4 * i) })
}

#[test]
fn smoothing_refreshes_already_solved_unplaced_and_equal_cells_in_hash_order() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    w.prepare(1, 16);
    let a = w.cell(1, 0, false, -1, [110.0; 4]);
    let b = w.cell(9, 0, true, 0, [110.0; 4]);
    let c = w.cell(4, 0, true, -1, [100.0; 4]);
    w.smoothing(&[1, 9, 4]);
    assert_eq!(w.position(), (0, 8));
    for p in [a, b, c] {
        assert_eq!(unsafe { corners(p) }, [100.0f32.to_bits(); 4]);
    }
    let mut expected = vec![Call::Begin(w.tool.ptr() as usize + 0xA4)];
    for (key, vertices) in [(1, [1, 130, 131, 2]), (9, [9, 138, 139, 10]), (4, [4, 133, 134, 5])] {
        expected.push(Call::Find(key));
        expected.extend(vertices.into_iter().map(Call::Height));
    }
    assert_eq!(calls(), expected);
    assert_eq!(unsafe { RefreshCell::from_ptr(a) }.solved_index(), -1);
    assert_eq!(unsafe { RefreshCell::from_ptr(b) }.solved_index(), 0);
    assert_eq!(unsafe { RefreshCell::from_ptr(c) }.solved_index(), -1);
}

#[test]
fn connection_refresh_appends_existing_piece_and_orientation_with_stack_padding() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    w.prepare(0, 16);
    let p = w.cell(4, 4, true, -1, [110.0; 4]);
    unsafe { RefreshCell::from_ptr(p) }.set_connection(1);
    w.smoothing(&[0x0004_0004]);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        for v in [520, 649, 650, 521] {
            s.heights.insert(v + 100_000, 101.0);
        }
    });
    assert_eq!(w.position(), (0, 8));
    assert_eq!(unsafe { corners(p) }, [101.0f32.to_bits(); 4]);
    assert_eq!(&w.solved[..3], &[0x1234_5678, 0xDECA_FB03, 0x0004_0004]);
    assert_eq!(unsafe { RefreshCell::from_ptr(p) }.solved_index(), 0);
    assert_eq!(
        calls(),
        [
            Call::Begin(w.tool.ptr() as usize + 0xA4),
            Call::Find(0x0004_0004),
            Call::Height(100_520),
            Call::Height(100_649),
            Call::Height(100_650),
            Call::Height(100_521),
            Call::Occupant(4, 4, 0x55AA),
            Call::Piece,
            Call::Orientation,
            Call::Release
        ]
    );
}

#[test]
fn smoothing_full_vector_calls_growth_helper_with_complete_record() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    w.prepare(0, 0);
    let p = w.cell(4, 4, true, -1, [110.0; 4]);
    w.smoothing(&[0x0004_0004]);
    let mut buffer = [0u32; 3];
    STATE.with(|s| s.borrow_mut().grow_buffer = buffer.as_mut_ptr() as usize);
    w.position();
    assert_eq!(buffer, [0x1234_5678, 0xDECA_FB03, 0x0004_0004]);
    assert_eq!(unsafe { RefreshCell::from_ptr(p) }.solved_index(), 0);
    let log = calls();
    assert_eq!(
        log[9],
        Call::Grow(
            w.tool.ptr() as usize + 0x98,
            w.solved.as_mut_ptr() as usize,
            buffer,
            1,
            1
        )
    );
    assert_eq!(log[10], Call::Release);
}

#[test]
fn empty_smoothing_set_keeps_begin_helpers_edx() {
    let mut w = World::new();
    w.prepare(0, 16);
    assert_eq!(w.position(), (0, 0xAABB_CCDD));
    assert_eq!(calls(), [Call::Begin(w.tool.ptr() as usize + 0xA4)]);
}

#[test]
fn quiet_nan_and_unrounded_height_compare_before_corner_store() {
    let _fpu = GameFpu::new();
    let mut corner = 16_777_216.0f32;
    assert!(unsafe { refresh_corner(F80::from_i32(16_777_217), (&mut corner as *mut f32).cast()) });
    assert_eq!(corner, 16_777_216.0);
    let height = F80::from_f32(f32::from_bits(0x7FC1_2345));
    unsafe {
        core::arch::asm!("fnclex", options(nostack));
    }
    assert!(unsafe { refresh_corner(height, (&mut corner as *mut f32).cast()) });
    let status: u16;
    unsafe {
        core::arch::asm!("fnstsw ax", out("ax") status, options(nostack));
    }
    assert_eq!(status & 1, 0);
    assert_eq!(corner.to_bits(), 0x7FC1_2345);
    corner = -0.0;
    assert!(!unsafe { refresh_corner(F80::ZERO, (&mut corner as *mut f32).cast()) });
    assert_eq!(corner.to_bits(), 0);
}

#[test]
fn incident_missing_network_height_keeps_corner_and_shared_vertex_appends_once() {
    let mut w = World::new();
    w.prepare(1, 16);
    let p = w.cell(3, 4, true, -1, [102.0, 103.0, 104.0, 104.0]);
    // Already solved, unplaced and unavailable neighbours are all skipped by this pass.
    w.cell(4, 3, true, 0, [110.0; 4]);
    w.cell(3, 3, false, -1, [110.0; 4]);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        for v in [519, 648, 649] {
            s.direct.insert(v, 100.0);
        }
    });
    let (_, edx) = w.terrain(&[520, 520]);
    assert_eq!(edx, 0x9988_7766);
    assert_eq!(
        unsafe { corners(p) },
        [
            100.0f32.to_bits(),
            100.0f32.to_bits(),
            100.0f32.to_bits(),
            104.0f32.to_bits()
        ]
    );
    assert_eq!(unsafe { RefreshCell::from_ptr(p) }.solved_index(), 1);
    assert_eq!(&w.solved[3..6], &[0x1234_5678, 0xDECA_FB03, 0x0004_0003]);
    let t = w.tool.ptr() as usize + 0x124;
    assert_eq!(
        calls(),
        [
            Call::Find(0x0003_0003),
            Call::Find(0x0003_0004),
            Call::Find(0x0004_0003),
            Call::Query(3, 4, 0x55AA),
            Call::Interface(0xA821_EF94),
            Call::Release,
            Call::Piece,
            Call::Orientation,
            Call::DirectHeight(t, 519, p as usize + 0x58),
            Call::DirectHeight(t, 648, p as usize + 0x5C),
            Call::DirectHeight(t, 649, p as usize + 0x60),
            Call::DirectHeight(t, 520, p as usize + 0x64),
            Call::Release,
            Call::Find(0x0004_0004),
            Call::Find(0x0003_0003),
            Call::Find(0x0003_0004),
            Call::Find(0x0004_0003),
            Call::Find(0x0004_0004)
        ]
    );
}

#[test]
fn incident_growth_allocates_copies_frees_and_updates_all_vector_pointers() {
    let mut w = World::new();
    w.prepare(1, 1);
    w.solved[..3].copy_from_slice(&[11, 22, 33]);
    let p = w.cell(0, 0, true, -1, [100.0; 4]);
    let mut buffer = [0xCDCD_CDCDu32; 6];
    STATE.with(|s| s.borrow_mut().allocation = buffer.as_mut_ptr() as usize);
    assert_eq!(w.terrain(&[0]).1, 0x1020_3040);
    assert_eq!(buffer, [11, 22, 33, 0x1234_5678, 0xDECA_FB03, 0]);
    let tool = unsafe { NetworkTool::from_ptr(w.tool.ptr()) };
    let refresh = unsafe { RefreshTool::from_ptr(w.tool.ptr()) };
    assert_eq!(tool.solved_begin(), buffer.as_mut_ptr().cast());
    assert_eq!(tool.solved_end(), unsafe { buffer.as_mut_ptr().add(6).cast() });
    assert_eq!(refresh.solved_capacity(), tool.solved_end());
    assert_eq!(unsafe { RefreshCell::from_ptr(p) }.solved_index(), 1);
    let log = calls();
    assert_eq!(
        &log[10..],
        &[
            Call::Allocate(24),
            Call::Free(w.solved.as_mut_ptr() as usize),
            Call::Release
        ]
    );
}

#[test]
fn incident_bounds_include_only_cells_at_city_edges_and_empty_input_preserves_edx() {
    let mut w = World::new();
    w.prepare(0, 16);
    let empty: [u32; 0] = [];
    assert_eq!(w.terrain(&empty), (empty.as_ptr() as u32, 0x1122_3344));
    assert!(calls().is_empty());
    w.terrain(&[0, 128, 128 * 129, 128 * 129 + 128]);
    assert_eq!(
        calls(),
        [
            Call::Find(0),
            Call::Find(127),
            Call::Find(127 << 16),
            Call::Find((127 << 16) + 127)
        ]
    );
}
