// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Bridge edge, padding, and elevation fixtures moved from the core's network_bridge tests.

use super::*;
use crate::network_tool::bridge_helpers::tests::invoke;
use crate::testing::{fake, fake_data, GameFpu, Object};
use std::cell::RefCell;

#[derive(Clone, Debug, PartialEq)]
enum Call {
    Lookup(u32, u32),
    Edge(u32, i32, u32),
    Wet(u32, u32, u32),
    Surface(u32, u32, u32),
}
#[derive(Default)]
struct State {
    calls: Vec<Call>,
    infos: Vec<(u32, u32)>,
    edges: Vec<(i32, u32)>,
    surfaces: Vec<(u32, u32, f32, u32)>,
    lookup_write: Option<(u32, usize, u32)>,
    water_write: Option<(u32, usize, u32)>,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn reset() {
    STATE.with(|s| *s.borrow_mut() = State::default());
}
fn calls() -> Vec<Call> {
    STATE.with(|s| std::mem::take(&mut s.borrow_mut().calls))
}
unsafe extern "thiscall" fn lookup(this: *mut u8, key: u32) -> *mut u8 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Lookup(this as u32, key));
        if let Some((p, o, v)) = s.lookup_write.take() {
            unsafe { write(p as *mut u8, o, v) };
        }
        s.infos.iter().find(|&&(k, _)| k == key).map_or(0, |&(_, p)| p) as *mut u8
    })
}
extern "thiscall" fn edge(this: *mut u8, index: i32, ty: u32) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Edge(this as u32, index, ty));
        s.edges
            .iter()
            .find(|&&(i, _)| i == index)
            .map_or(0xAABB_CC01, |&(_, v)| v)
    })
}
unsafe extern "thiscall" fn wet(this: *mut u8, x: u32, z: u32) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Wet(this as u32, x, z));
        if let Some((p, o, v)) = s.water_write.take() {
            unsafe { write(p as *mut u8, o, v) };
        }
        s.surfaces
            .iter()
            .find(|&&(cx, cz, _, _)| cx == x && cz == z)
            .map_or(0, |&(_, _, _, w)| w)
    })
}
extern "thiscall" fn surface(this: *mut u8, x: u32, z: u32) -> f32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Surface(this as u32, x, z));
        s.surfaces
            .iter()
            .find(|&&(cx, cz, _, _)| cx == x && cz == z)
            .map_or(0.0, |&(_, _, h, _)| h)
    })
}

struct World {
    tool: Object,
    steps: Vec<[u32; 3]>,
    cells: Vec<[u32; 2]>,
    _info: Object,
    parameters: Object,
    _terrain: Object,
    _vtable: Box<[usize; 35]>,
    _guards: Vec<crate::testing::Fake>,
    _masks: Box<[u32; 13]>,
}
impl World {
    fn new(n: usize, width: usize) -> Self {
        reset();
        let mut tool = Object::new(0x380);
        let mut steps: Vec<_> = (0..n)
            .map(|i| [(i * width) as u32, ((i + 1) * width - 1) as u32, 0])
            .collect();
        let mut cells: Vec<_> = (0..n * width).map(|i| [i as u32, 32]).collect();
        let mut info = Object::new(0x50);
        let mut parameters = Object::new(0x178 * 2);
        let mut terrain = Object::new(16);
        let mut vtable = Box::new([0; 35]);
        vtable[0x88 / 4] = wet as *const () as usize;
        vtable[0x48 / 4] = surface as *const () as usize;
        let mut masks = Box::new(std::array::from_fn(|i| 1u32 << i));
        let guards = vec![
            fake(0x0063_3220, lookup as *const () as usize),
            fake_data(0x00AA_88B8, masks.as_mut_ptr().cast()),
        ];
        unsafe {
            let t = NetworkTool::from_ptr(tool.ptr());
            t.set_steps(steps.as_mut_ptr().cast());
            t.set_cells(cells.as_mut_ptr().cast());
            t.set_valid_steps(n as i32);
            t.set_straight_steps(n as i32);
            t.set_type_info(info.ptr());
            t.set_terrain(terrain.ptr());
            write(terrain.ptr(), 0, vtable.as_ptr());
            write(info.ptr(), 0x30, parameters.ptr());
            write(tool.ptr(), 0x204, -1f32);
            write(parameters.ptr(), 0x5C, 20f32);
            write(parameters.ptr(), 0x78, 5f32);
        }
        Self {
            tool,
            steps,
            cells,
            _info: info,
            parameters,
            _terrain: terrain,
            _vtable: vtable,
            _guards: guards,
            _masks: masks,
        }
    }
    fn p(&mut self) -> *mut u8 {
        self.tool.ptr()
    }
    fn check(&mut self, index: i32, ty: u32) -> u32 {
        invoke(
            fc4p_0062eb80 as *const () as usize,
            self.p(),
            &mut [index as u32, ty],
            false,
            false,
        )
        .eax
    }
    fn pad(&mut self, minimum: i32, start: &mut i32, end: &mut i32, ty: u32) -> (u32, u32) {
        let mut args = [minimum as u32, start as *mut i32 as u32, end as *mut i32 as u32, ty];
        let eax = invoke(fc4p_0062ecd0 as *const () as usize, self.p(), &mut args, false, false).eax;
        (eax, args[2])
    }
}

#[test]
fn bridge_edge_fixture_accepts_straight_road_rejects_crossing_and_other_network() {
    let mut w = World::new(3, 1);
    let mut info = Object::new(0xB8);
    let p = info.ptr();
    STATE.with(|s| s.borrow_mut().infos.push((0x0020_0001, p as u32)));
    unsafe {
        write(p, 8, 1u32);
        write(p, 0x18, 0x0002_0002u32);
    }
    assert_eq!(w.check(1, 0), 1);
    unsafe {
        write(p, 0x18, 0x0000_0200u32);
    }
    assert_eq!(w.check(1, 0), p as u32 & !0xFF);
    unsafe {
        write(p, 0x18, 0x0002_0002u32);
        write(p, 8, 3u32);
    }
    assert_eq!(w.check(1, 0), p as u32 & !0xFF);
    STATE.with(|s| s.borrow_mut().infos.clear());
    assert_eq!(w.check(1, 0), 1);
    assert_eq!(calls(), vec![Call::Lookup(w.p() as u32, 0x0020_0001); 4]);
}

#[test]
fn bridge_edges_cover_all_pen_bits_and_wide_step_positions() {
    for pen in 0..8u8 {
        for width in [1, 3] {
            let mut w = World::new(1, width);
            unsafe {
                NetworkTool::from_ptr(w.p()).set_pen(pen);
            }
            let mut infos: Vec<_> = (0..width).map(|_| Object::new(0xB8)).collect();
            for (i, info) in infos.iter_mut().enumerate() {
                let allowed = if pen & 2 == 0 {
                    if width == 1 {
                        0x0002_0002
                    } else {
                        [0x0402_0002, 0x0402_0402, 0x0002_0402][i]
                    }
                } else if width == 1 {
                    0x0200_0200
                } else {
                    [0x0204_0200, 0x0204_0204, 0x0200_0204][i]
                };
                unsafe {
                    write(info.ptr(), 8, 0x8000_1000u32);
                    write(info.ptr(), 0x18 + 12 * 4, allowed);
                }
                STATE.with(|s| s.borrow_mut().infos.push((0x0020_0000 + i as u32, info.ptr() as u32)));
            }
            assert_eq!(w.check(0, 12), 1);
            assert_eq!(calls().len(), width);
            unsafe {
                write(infos[width - 1].ptr(), 0x18 + 12 * 4, u32::MAX);
            }
            assert_eq!(w.check(0, 12), infos[width - 1].ptr() as u32 & !0xFF);
        }
    }
}

#[test]
fn edge_test_uses_global_masks_wrapping_keys_and_live_step_bounds() {
    let mut w = World::new(1, 3);
    let mut info = Object::new(0xB8);
    w.cells[0] = [u32::MAX, 0x1234_FFFF];
    let key = 0x1234_FFFFu32.wrapping_shl(16).wrapping_add(u32::MAX);
    w._masks[3] = 0x0010;
    unsafe {
        write(info.ptr(), 8, 0x0010u32);
        write(info.ptr(), 0x18 + 12, 0x0002_0002u32);
    }
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.infos.push((key, info.ptr() as u32));
        s.lookup_write = Some((w.steps.as_mut_ptr() as u32, 4, 0));
    });
    assert_eq!(w.check(0, 3), 1);
    assert_eq!(calls(), [Call::Lookup(w.p() as u32, key)]);
    // An empty unsigned range performs no lookups and retains the upper end-index bytes in EAX.
    w.steps[0] = [0x1240, 0x1234, 0];
    assert_eq!(w.check(0, 3), 0x1201);
    assert!(calls().is_empty());
}

#[test]
fn minimum_span_fixture_alternates_back_then_forward_and_uses_the_game_entry() {
    let _edge = fake(0x0062_EB80, edge as *const () as usize);
    let mut w = World::new(64, 1);
    let (mut start, mut end) = (20, 31);
    let (eax, overwritten) = w.pad(22, &mut start, &mut end, 4);
    assert_eq!((eax, start, end), (1, 15, 36));
    assert_eq!(overwritten as u8, 1);
    let p = w.p() as u32;
    assert_eq!(
        calls(),
        [19, 32, 18, 33, 17, 34, 16, 35, 15, 36].map(|i| Call::Edge(p, i, 4))
    );
}

#[test]
fn span_padding_undoes_blocked_attempts_and_keeps_extending_the_other_side() {
    let _edge = fake(0x0062_EB80, edge as *const () as usize);
    let mut w = World::new(20, 1);
    w.steps[2][2] = 1; // Back to 3 also checks the preceding step's kind.
    let (mut start, mut end) = (4, 6);
    let (eax, overwritten) = w.pad(6, &mut start, &mut end, 7);
    assert_eq!((eax, start, end, overwritten as u8), (1, 4, 9, 0));
    assert_eq!(calls(), [7, 8, 9].map(|i| Call::Edge(w.p() as u32, i, 7)));
    STATE.with(|s| s.borrow_mut().edges = vec![(7, 0x1234_5600)]);
    w.steps[2][2] = 0;
    let (mut start, mut end) = (4, 6);
    let (eax, _) = w.pad(5, &mut start, &mut end, 7);
    assert_eq!((eax, start, end), (1, 2, 6));
    assert_eq!(calls(), [3, 7, 2].map(|i| Call::Edge(w.p() as u32, i, 7)));
}

#[test]
fn span_padding_failure_return_bytes_limits_and_short_circuit() {
    let _edge = fake(0x0062_EB80, edge as *const () as usize);
    let mut w = World::new(8, 1);
    let (mut start, mut end) = (2, 6);
    let end_pointer = &mut end as *mut i32 as u32;
    assert_eq!(w.pad(3, &mut start, &mut end, 0), (1, end_pointer));
    assert!(calls().is_empty());
    // A long enough span keeps the original length's upper bytes and does not check the limits.
    let (mut start, mut end) = (-256, 512);
    assert_eq!(w.pad(2, &mut start, &mut end, 0).0, 0x301);
    let (mut start, mut end) = (2, 6);
    assert_eq!(w.pad(9, &mut start, &mut end, 0).0, 0);
    assert_eq!((start, end), (2, 7));
    STATE.with(|s| s.borrow_mut().edges = vec![(7, 0x1234_5600)]);
    let (mut start, mut end) = (2, 6);
    assert_eq!(w.pad(9, &mut start, &mut end, 0).0, 0x1234_5600);
    assert_eq!((start, end), (2, 6));
    STATE.with(|s| s.borrow_mut().edges.clear());
    // Reaching the straight-prefix limit fails even when enough cells were admitted.
    unsafe {
        NetworkTool::from_ptr(w.p()).set_straight_steps(7);
    }
    let (mut start, mut end) = (2, 6);
    assert_eq!(w.pad(6, &mut start, &mut end, 0).0, 0);
    assert_eq!((start, end), (2, 7));
}

#[test]
fn deck_elevation_fixture_keeps_last_cell_and_excludes_the_far_step() {
    let _fpu = GameFpu::new();
    let mut w = World::new(4, 2);
    let mut record = Object::new(0xA8);
    let p = record.ptr();
    unsafe {
        let b = Bridge::from_ptr(p);
        b.set_start(0);
        b.set_end(3);
        b.set_index(0);
        write(p, 0x14, 250f32);
    }
    STATE.with(|s| {
        s.borrow_mut().surfaces = vec![
            (0, 32, 900.0, 1),
            (1, 32, 250.0, 0),
            (2, 32, 800.0, 0),
            (3, 32, 250.0, 0x100),
            (4, 32, 700.0, 1),
            (5, 32, 250.0, 1),
            (6, 32, 5000.0, 1),
            (7, 32, 5000.0, 1),
        ]
    });
    invoke(
        fc4p_0062ea90 as *const () as usize,
        w.p(),
        &mut [p as u32],
        false,
        false,
    );
    assert_eq!(unsafe { Bridge::from_ptr(p).deck() }, 270.0);
    assert_eq!(unsafe { read::<f32>(p, 0x14) }, 250.0);
    let terrain = unsafe { NetworkTool::from_ptr(w.p()).terrain() } as u32;
    let expected: Vec<_> = (0..6)
        .flat_map(|x| [Call::Wet(terrain, x, 32), Call::Surface(terrain, x, 32)])
        .collect();
    assert_eq!(calls(), expected);
    unsafe {
        write(w.p(), 0x204, 400f32);
    }
    invoke(
        fc4p_0062ea90 as *const () as usize,
        w.p(),
        &mut [p as u32],
        false,
        false,
    );
    assert_eq!(unsafe { Bridge::from_ptr(p).deck() }, 400.0);
}

#[test]
fn deck_elevation_preserves_nan_override_and_reads_coordinates_after_water_query() {
    let _fpu = GameFpu::new();
    let mut w = World::new(2, 1);
    let mut record = Object::new(0xA8);
    let p = record.ptr();
    unsafe {
        Bridge::from_ptr(p).set_end(1);
        write(w.p(), 0x204, 0x7F81_2345u32);
    }
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.surfaces = vec![(99, 32, 250.0, 0)];
        s.water_write = Some((w.cells.as_mut_ptr() as u32, 0, 99));
    });
    invoke(
        fc4p_0062ea90 as *const () as usize,
        w.p(),
        &mut [p as u32],
        false,
        false,
    );
    assert_eq!(unsafe { read::<u32>(p, 0x10) }, 0x7F81_2345);
    let terrain = unsafe { NetworkTool::from_ptr(w.p()).terrain() } as u32;
    assert_eq!(calls(), [Call::Wet(terrain, 0, 32), Call::Surface(terrain, 99, 32)]);
    // An empty span still copies the override bit for bit.
    unsafe {
        Bridge::from_ptr(p).set_end(0);
    }
    invoke(
        fc4p_0062ea90 as *const () as usize,
        w.p(),
        &mut [p as u32],
        false,
        false,
    );
    assert!(calls().is_empty());
    assert_eq!(unsafe { read::<u32>(p, 0x10) }, 0x7F81_2345);
}

#[test]
fn empty_first_step_uses_the_argument_bits_and_type_index_selects_clearances() {
    let _fpu = GameFpu::new();
    let mut w = World::new(2, 1);
    let mut record = Object::new(0xA8);
    let p = record.ptr();
    w.steps[0] = [1, 0, 0];
    unsafe {
        Bridge::from_ptr(p).set_end(1);
    }
    invoke(
        fc4p_0062ea90 as *const () as usize,
        w.p(),
        &mut [p as u32],
        false,
        false,
    );
    assert_eq!(unsafe { read::<u32>(p, 0x10) }, p as u32);
    assert!(calls().is_empty());
    w.steps[0] = [0, 0, 0];
    unsafe {
        Bridge::from_ptr(p).set_index(1);
        write(w.parameters.ptr(), 0x178 + 0x78, 12f32);
    }
    STATE.with(|s| s.borrow_mut().surfaces = vec![(0, 32, 250.0, 0)]);
    invoke(
        fc4p_0062ea90 as *const () as usize,
        w.p(),
        &mut [p as u32],
        false,
        false,
    );
    assert_eq!(unsafe { Bridge::from_ptr(p).deck() }, 262.0);
}
