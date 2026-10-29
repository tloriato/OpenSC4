// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Node, distance and reversible-route expectations carried from the core pathfinder tests.

use super::*;
use crate::abi::{call_raw, Conv, RawCall};
use crate::testing::{call_thiscall_with_eax, fake, fake_data, Fake, GameFpu, Object};
use std::cell::RefCell;

#[test]
fn city_bounds_use_unsigned_comparisons_including_unusual_dimensions() {
    let mut object = Object::new(0x3160);
    let pf = unsafe { Pathfinder::from_ptr(object.ptr()) };
    for (width, height) in [(8, 8), (0, 0), (0, 8), (8, 0), (u32::MAX, 0x8000_0000)] {
        pf.set_count_x(width);
        pf.set_count_z(height);
        for x in [0, 7, 8, 0x7FFF_FFFF, 0x8000_0000, u32::MAX] {
            for z in [0, 7, 8, 0x7FFF_FFFF, 0x8000_0000, u32::MAX] {
                assert_eq!(
                    unsafe { fc4p_006d5cf0(object.ptr(), x, z) },
                    u32::from(x < width && z < height)
                );
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Maximum(usize, usize, u32),
    Used(usize, usize, u32),
    Fill(usize, u32, u32),
    Update(usize, usize, u32),
}

#[derive(Default)]
struct State {
    calls: Vec<Call>,
    maximum: u32,
    used: u32,
    swap_traffic: Option<(*mut u8, *mut u8)>,
    replace_density: Option<(*mut u8, u32)>,
    replace_end: Option<(*mut u8, *mut u8)>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

unsafe extern "thiscall" fn maximum(this: *mut u8, holder: *mut u8, kind: u32) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Maximum(this as usize, holder as usize, kind));
        if let Some((pf, next)) = s.swap_traffic.take() {
            unsafe { Pathfinder::from_ptr(pf) }.set_traffic(next);
        }
        0x1234_ABCD_0000_0000 | u64::from(s.maximum)
    })
}

unsafe extern "thiscall" fn used(this: *mut u8, holder: *mut u8, kind: u32) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Used(this as usize, holder as usize, kind));
        if let Some((pf, density)) = s.replace_density.take() {
            unsafe { Pathfinder::from_ptr(pf) }.set_density(density);
        }
        0x5678_DCBA_0000_0000 | u64::from(s.used)
    })
}

unsafe extern "C" fn fill(dest: *mut u8, value: u32, count: u32) -> u32 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Fill(dest as usize, value, count)));
    for i in 0..count {
        unsafe { write_unaligned(dest.add(i as usize * 4).cast::<u32>(), value) };
    }
    value
}

unsafe extern "thiscall" fn update(this: *mut u8, node: *mut u8) -> u32 {
    let score = unsafe { Node::from_ptr(node) }.score().to_bits();
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Update(this as usize, node as usize, score));
        if let Some((pf, end)) = s.replace_end.take() {
            unsafe { Pathfinder::from_ptr(pf) }.set_next_node(end);
        }
    });
    0xDEAD_BEEF
}

fn take_calls() -> Vec<Call> {
    STATE.with(|s| core::mem::take(&mut s.borrow_mut().calls))
}

#[test]
fn capacity_checks_maximum_then_used_and_writes_only_on_success() {
    let mut object = Object::new(0x3160);
    let pf = unsafe { Pathfinder::from_ptr(object.ptr()) };
    let vtable = [0usize; 0x50 / 4 + 1];
    let mut vtable = Box::new(vtable);
    vtable[0x50 / 4] = maximum as *const () as usize;
    let mut traffic = [vtable.as_ptr() as usize];
    let traffic = traffic.as_mut_ptr().cast::<u8>();
    let holder = 0x1234_5678usize as *mut u8;
    pf.set_traffic(traffic);
    let _fake = fake(0x0070_DFF0, used as *const () as usize);
    // Cases: maximum zero, already full, unsigned over-capacity, partial acceptance, all accepted,
    // zero requested density, and full-width unsigned capacities.
    for (max, used, density, arrived, eax) in [
        (0, 0x1234, 10, None, 0),
        (0x1234, 0x1234, 10, None, 0x1200),
        (1, 0x8000_0123, 10, None, 0x8000_0100),
        (100, 95, 0x1234, Some(5), 0x1201),
        (100, 90, 10, Some(10), 1),
        (100, 0, 0, Some(0), 1),
        (u32::MAX, 1, u32::MAX, Some(u32::MAX - 1), 0xFFFF_FF01),
    ] {
        STATE.with(|s| {
            *s.borrow_mut() = State {
                maximum: max,
                used,
                ..State::default()
            }
        });
        pf.set_density(density);
        pf.set_arrived(0xAABB_CCDD);
        pf.set_reached_type(0x1122_3344);
        let before = unsafe { core::slice::from_raw_parts(object.ptr(), 0x3160).to_vec() };
        let expected_edx = if max == 0 {
            0x1234_ABCD_0000_0000
        } else {
            0x5678_DCBA_0000_0000
        };
        assert_eq!(
            unsafe { fc4p_006d5d80(object.ptr(), holder, 0xABCD_0103) },
            expected_edx | eax
        );
        assert_eq!(pf.arrived(), arrived.unwrap_or(0xAABB_CCDD));
        assert_eq!(pf.reached_type(), if arrived.is_some() { 3 } else { 0x1122_3344 });
        let mut expected = vec![Call::Maximum(traffic as usize, holder as usize, 3)];
        if max != 0 {
            expected.push(Call::Used(traffic as usize, holder as usize, 3));
        }
        assert_eq!(take_calls(), expected);
        let mut after = before;
        if let Some(n) = arrived {
            after[0x6C..0x70].copy_from_slice(&n.to_le_bytes());
            after[0x8C..0x90].copy_from_slice(&3u32.to_le_bytes());
        }
        assert_eq!(unsafe { core::slice::from_raw_parts(object.ptr(), 0x3160) }, after);
    }
}

#[test]
fn capacity_rereads_owner_and_density_after_calls() {
    let mut object = Object::new(0x3160);
    let pf = unsafe { Pathfinder::from_ptr(object.ptr()) };
    let mut vt = [0usize; 21];
    vt[20] = maximum as *const () as usize;
    let mut first = [vt.as_ptr() as usize];
    let mut second = [vt.as_ptr() as usize];
    let first = first.as_mut_ptr().cast::<u8>();
    let second = second.as_mut_ptr().cast::<u8>();
    pf.set_traffic(first);
    pf.set_density(10);
    STATE.with(|s| {
        *s.borrow_mut() = State {
            maximum: 9,
            used: 2,
            swap_traffic: Some((object.ptr(), second)),
            replace_density: Some((object.ptr(), 3)),
            ..State::default()
        }
    });
    let _fake = fake(0x0070_DFF0, used as *const () as usize);
    assert_eq!(
        unsafe { fc4p_006d5d80(object.ptr(), core::ptr::null_mut(), 0x100) },
        0x5678_DCBA_0000_0001
    );
    assert_eq!(pf.arrived(), 3);
    assert_eq!(
        take_calls(),
        [Call::Maximum(first as usize, 0, 0), Call::Used(second as usize, 0, 0)]
    );
}

struct World {
    object: Object,
    dx: Vec<i32>,
    dz: Vec<i32>,
    zero: Box<f32>,
    _data: Fake,
    _fpu: GameFpu,
}

impl World {
    fn new() -> Self {
        let mut zero = Box::new(0.0);
        let data = fake_data(0x00A8_1054, (&mut *zero as *mut f32).cast());
        let mut w = Self {
            object: Object::new(0x3160),
            dx: vec![0x1234; 22],
            dz: vec![0x5678; 22],
            zero,
            _data: data,
            _fpu: GameFpu::new(),
        };
        let pf = w.pf();
        pf.set_count_x(16);
        pf.set_count_z(8);
        pf.set_distances_x(unsafe { w.dx.as_mut_ptr().add(1) });
        pf.set_distances_z(unsafe { w.dz.as_mut_ptr().add(1) });
        pf.set_end_x0(10);
        pf.set_end_x1(10);
        pf.set_end_z0(5);
        pf.set_end_z1(5);
        pf.set_search_x0(2);
        pf.set_search_z0(0);
        pf.set_search_x1(10);
        pf.set_search_z1(8);
        pf.set_weight(1.0);
        unsafe { setup_distance_tables(w.object.ptr()) };
        w
    }
    fn pf(&mut self) -> Pathfinder {
        unsafe { Pathfinder::from_ptr(self.object.ptr()) }
    }
    fn distance(&mut self, x: i32, z: i32) -> RawCall {
        let args = [x as u32, z as u32];
        let mut call = RawCall::new(
            Conv::Thiscall,
            fc4p_006d6160 as *const () as usize,
            self.object.ptr() as u32,
            &args,
        );
        call.take_st0 = 1;
        unsafe { call_raw(&mut call) };
        call
    }
}

#[unsafe(naked)]
unsafe extern "thiscall" fn distance_argument_after(this: *mut u8, x: i32, z: i32) -> u64 {
    core::arch::naked_asm!(
        "push dword ptr [esp + 8]", "push dword ptr [esp + 8]",
        "call {distance}", "fstp st(0)", "mov edx, [esp - 8]", "ret 8",
        distance = sym fc4p_006d6160,
    )
}

#[unsafe(naked)]
unsafe extern "thiscall" fn setup_preserved_ecx(this: *mut u8) -> u32 {
    core::arch::naked_asm!("call {setup}", "mov eax, ecx", "ret", setup = sym fc4p_006d6040)
}

#[test]
fn distance_preserves_setup_this_and_overwrites_only_the_outside_x_argument() {
    let mut w = World::new();
    assert_eq!(unsafe { setup_preserved_ecx(w.object.ptr()) }, w.object.ptr() as u32);
    // The argument copy lives eight bytes below ESP after the callee pops both words.
    assert_eq!(
        unsafe { distance_argument_after(w.object.ptr(), -1, 5) },
        (11u64 << 32) | 11
    );
    assert_eq!(
        unsafe { distance_argument_after(w.object.ptr(), 4, 5) },
        (4u64 << 32) | 4
    );
}

#[test]
fn distance_tables_include_both_outer_cells_and_do_not_touch_following_storage() {
    let mut w = World::new();
    assert_eq!(&w.dx[..18], &[11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0, 1, 2, 3, 4, 5, 6]);
    assert_eq!(&w.dz[..10], &[6, 5, 4, 3, 2, 1, 0, 1, 2, 3]);
    assert_eq!(&w.dx[18..], &[0x1234; 4]);
    assert_eq!(&w.dz[10..], &[0x5678; 12]);
    assert_eq!(
        unsafe { call_thiscall_with_eax(fc4p_006d6040 as *const () as usize, w.object.ptr(), &[], 0).0 },
        9
    );
    let pf = w.pf();
    pf.set_end_x0(-1);
    pf.set_end_x1(16);
    pf.set_end_z0(-2);
    pf.set_end_z1(-1);
    unsafe { setup_distance_tables(w.object.ptr()) };
    assert_eq!(&w.dx[..18], &[0; 18]);
    assert_eq!(&w.dz[..10], &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
    // Inverted bounds retain the first decreasing run, then start the increasing run from that cursor.
    pf.set_end_x0(3);
    pf.set_end_x1(1);
    unsafe { setup_distance_tables(w.object.ptr()) };
    assert_eq!(&w.dx[..7], &[4, 3, 2, 1, 2, 3, 4]);
}

#[test]
fn search_distance_is_zero_inclusive_and_keeps_exact_integer_st0_outside() {
    let mut w = World::new();
    for (x, z) in [(2, 0), (10, 8), (4, 5)] {
        let c = w.distance(x, z);
        assert_eq!(c.eax, x as u32);
        assert_eq!(c.st0, F80::ZERO);
    }
    for (x, z, distance) in [(-1, 5, 11), (1, 5, 9), (11, 5, 1), (10, -1, 6), (10, 9, 4)] {
        // z=9 is within the test's extra backing storage; supply that outside-border table entry.
        if z == 9 {
            w.dz[10] = 4;
        }
        let c = w.distance(x, z);
        assert_eq!(c.eax, distance);
        assert_eq!(c.st0, F80::from_i32(distance as i32));
    }
    w.dx[0] = i32::MAX;
    w.dz[6] = 0;
    assert_eq!(w.distance(-1, 5).st0, F80::from_i32(i32::MAX));
    w.dz[6] = 1;
    let c = w.distance(-1, 5);
    assert_eq!(c.eax, i32::MIN as u32);
    assert_eq!(c.st0, F80::from_i32(i32::MIN));
    // An inverted rectangle has no interior under the signed comparisons.
    w.pf().set_search_x0(11);
    w.dx[11] = 123;
    assert_eq!(w.distance(10, 5).eax, 124);
    assert_eq!(*w.zero, 0.0);
}

#[test]
fn releasing_nodes_retains_pages_and_clears_all_bucket_words() {
    let mut w = World::new();
    let mut pages = [0x1234_5000usize as *mut u8, 0x2345_6000usize as *mut u8];
    let pf = w.pf();
    pf.set_pages(pages.as_mut_ptr());
    let _fake = fake(0x0091_0003, fill as *const () as usize);
    for count in [0, 1, u32::MAX] {
        unsafe { core::ptr::write_bytes(w.object.ptr().add(0x12C), 0xA5, 0xC07 * 4) };
        pf.set_open_count(count);
        pf.set_result(0x1234usize as *mut u8);
        assert_eq!(
            unsafe { call_thiscall_with_eax(fc4p_006d6730 as *const () as usize, w.object.ptr(), &[], 0).0 },
            count
        );
        assert_eq!(pf.result(), core::ptr::null_mut());
        assert_eq!(pf.open_count(), 0);
        assert_eq!(pf.next_node(), pages[0]);
        assert_eq!(pf.page_end(), pages[0].wrapping_add(0x24000));
        assert_eq!(pf.page_index(), 0);
        assert_eq!(pf.pages(), pages.as_mut_ptr());
        assert!(
            unsafe { core::slice::from_raw_parts(w.object.ptr().add(0x12C), 0xC07 * 4) }
                .iter()
                .all(|&b| b == 0)
        );
        assert_eq!(take_calls(), [Call::Fill(w.object.ptr() as usize + 0x12C, 0, 0xC07)]);
    }
}

fn node_at(page: &mut Object, index: usize, x: i16, z: i16, cost: f32, open: u8) -> Node {
    let node = unsafe { Node::from_ptr(page.ptr().add(index * 0x24)) };
    node.set_x(x);
    node.set_z(z);
    node.set_cost(cost);
    node.set_score(-999.0);
    node.set_open(open);
    node
}

#[test]
fn recalculated_exit_node_uses_its_own_cell_and_updates_open_nodes_in_page_order() {
    let mut w = World::new();
    let mut page = Object::new(0x24000);
    let a = node_at(&mut page, 0, -1, 5, 19.0, 1);
    let b = node_at(&mut page, 1, 0, 5, 9.0, 0);
    let c = node_at(&mut page, 2, 2, 5, 4.0, 0x80);
    let mut pages = [page.ptr()];
    let pf = w.pf();
    pf.set_pages(pages.as_mut_ptr());
    pf.set_next_node(page.ptr().wrapping_add(3 * 0x24));
    let _fake = fake(0x006D_6460, update as *const () as usize);
    let before = unsafe { core::slice::from_raw_parts(page.ptr(), 3 * 0x24).to_vec() };
    assert_eq!(unsafe { fc4p_006d6780(w.object.ptr()) }, pf.next_node() as u32);
    assert_eq!((a.score(), b.score(), c.score()), (30.0, 19.0, 4.0));
    assert_eq!(
        take_calls(),
        [
            Call::Update(w.object.ptr() as usize + 0x114, a.as_ptr() as usize, 30.0f32.to_bits()),
            Call::Update(w.object.ptr() as usize + 0x114, c.as_ptr() as usize, 4.0f32.to_bits()),
        ]
    );
    let mut want = before;
    for (i, score) in [30.0f32, 19.0, 4.0].iter().enumerate() {
        want[i * 0x24 + 12..i * 0x24 + 16].copy_from_slice(&score.to_bits().to_le_bytes());
    }
    assert_eq!(unsafe { core::slice::from_raw_parts(page.ptr(), 3 * 0x24) }, want);
    // Inside nodes add zero without multiplying by weight, even if that weight is NaN.
    pf.set_weight(f32::NAN);
    unsafe { fc4p_006d6780(w.object.ptr()) };
    assert!(a.score().is_nan());
    assert!(b.score().is_nan());
    assert_eq!(c.score(), 4.0);
    take_calls();
}

#[test]
fn score_scan_crosses_page_boundaries_and_rereads_the_live_end_after_heap_calls() {
    let mut w = World::new();
    let mut first = Object::new(0x24000);
    let mut second = Object::new(0x24000);
    let mut pages = [first.ptr(), second.ptr()];
    let pf = w.pf();
    pf.set_pages(pages.as_mut_ptr());
    pf.set_next_node(first.ptr());
    let _fake = fake(0x006D_6460, update as *const () as usize);
    assert_eq!(unsafe { fc4p_006d6780(w.object.ptr()) }, first.ptr() as u32);
    assert!(take_calls().is_empty());
    let last = node_at(&mut second, 0, 11, 5, 7.0, 1);
    pf.set_next_node(second.ptr().wrapping_add(0x24));
    unsafe { fc4p_006d6780(w.object.ptr()) };
    assert_eq!(last.score(), 8.0);
    assert_eq!(
        take_calls(),
        [Call::Update(
            w.object.ptr() as usize + 0x114,
            second.ptr() as usize,
            8.0f32.to_bits()
        )]
    );
    let a = node_at(&mut first, 0, 2, 5, 3.0, 1);
    let b = node_at(&mut first, 1, 2, 5, 4.0, 1);
    STATE.with(|s| s.borrow_mut().replace_end = Some((w.object.ptr(), first.ptr().wrapping_add(0x24))));
    unsafe { fc4p_006d6780(w.object.ptr()) };
    assert_eq!(a.score(), 3.0);
    assert_eq!(b.score(), -999.0);
    assert_eq!(take_calls().len(), 1);
}

struct Route {
    world: World,
    nodes: Object,
    traffic: Object,
    cells: Object,
}

impl Route {
    fn new(points: &[(i16, i16, u8, u8)]) -> Self {
        let mut r = Self {
            world: World::new(),
            nodes: Object::new(points.len() * 0x24),
            traffic: Object::new(0x5C),
            cells: Object::new(256 * 256 * 0x2E),
        };
        r.world.pf().set_traffic(r.traffic.ptr());
        unsafe { write_unaligned(r.traffic.ptr().add(0x58).cast::<*mut u8>(), r.cells.ptr()) };
        for (i, &(x, z, t, side)) in points.iter().enumerate() {
            let n = node_at(&mut r.nodes, i, x, z, 0.0, 0);
            n.set_travel(t);
            n.set_side(side);
            n.set_parent(if i + 1 < points.len() {
                unsafe { r.nodes.ptr().add((i + 1) * 0x24) }
            } else {
                core::ptr::null_mut()
            });
        }
        if !points.is_empty() {
            r.world.pf().set_result(r.nodes.ptr());
        }
        r
    }
    fn reversible(&mut self) -> u32 {
        unsafe {
            call_thiscall_with_eax(
                fc4p_006d61e0 as *const () as usize,
                self.world.object.ptr(),
                &[],
                0xAABB_CC33,
            )
            .0
        }
    }
    fn record(&mut self, x: i32, z: i32) -> *mut u8 {
        unsafe { connection_record(self.traffic.ptr(), x, z) }
    }
}

#[test]
fn reversibility_handles_empty_single_equal_and_off_map_segments_without_calls() {
    assert_eq!(Route::new(&[]).reversible(), 0xAABB_CC01);
    assert_eq!(Route::new(&[(1, 1, 1, 0)]).reversible(), 1);
    assert_eq!(Route::new(&[(1, 1, 1, 0), (1, 1, 1, 0), (-1, 1, 1, 0)]).reversible(), 1);
    assert_eq!(Route::new(&[(1, 1, 1, 0), (1, 1, 1, 0), (1, 1, 1, 0)]).reversible(), 1);
}

#[test]
fn reversibility_checks_each_reverse_turn_and_returns_connection_upper_byte_on_failure() {
    // Result-to-root: (4,3), (3,3), (2,3). The current entry side is 0 and the reverse direction is 2.
    let mut r = Route::new(&[(4, 3, 1, 0), (3, 3, 1, 0), (2, 3, 1, 0)]);
    let record = r.record(3, 3);
    unsafe { write_unaligned(record.add(2).cast::<u16>(), 0xAB00) };
    // Bit 8 permits reversal; removing it leaves 0xAA00 in EAX.
    assert_eq!(r.reversible(), 1);
    unsafe { write_unaligned(record.add(2).cast::<u16>(), 0xAA00) };
    assert_eq!(r.reversible(), 0xAA00);
    // Both subway-marked cells bypass the check and reset the direction.
    unsafe {
        *record.add(0x2C) = 2;
        *r.record(2, 3).add(0x2C) = 2;
    }
    assert_eq!(r.reversible(), 1);
    unsafe {
        *r.record(2, 3).add(0x2C) = 0;
    }
    assert_eq!(r.reversible(), 0xAA00);
}

#[test]
fn reversibility_type_changes_reject_only_the_no_switch_nibble() {
    let mut r = Route::new(&[(4, 3, 2, 0), (3, 3, 1, 0), (2, 3, 1, 0)]);
    let record = r.record(3, 3);
    for nibble in 0u16..16 {
        unsafe { write_unaligned(record.add(0x14).cast::<u16>(), nibble << 8) };
        assert_eq!(r.reversible(), if nibble == 15 { record as u32 & !0xFF } else { 1 });
    }
    // A switch to a different root travel type bypasses the ordinary same-type connection test.
    let mut r = Route::new(&[(4, 3, 1, 0), (3, 3, 1, 0), (2, 3, 2, 0)]);
    assert_eq!(r.reversible(), 1);
}

#[test]
fn reversible_turns_cover_all_directions_and_mask_shift_counts_to_five_bits() {
    for (points, direction) in [
        ([(2, 3, 1, 0), (3, 3, 1, 0), (4, 3, 1, 0)], 0u32),
        ([(3, 2, 1, 0), (3, 3, 1, 0), (3, 4, 1, 0)], 1),
        ([(4, 3, 1, 0), (3, 3, 1, 0), (2, 3, 1, 0)], 2),
        ([(3, 4, 1, 0), (3, 3, 1, 0), (3, 2, 1, 0)], 3),
    ] {
        for side in [0, 1, 3, 16, 24, 31, 32, 33, 255] {
            let mut r = Route::new(&points);
            unsafe { Node::from_ptr(r.nodes.ptr().add(0x24)) }.set_side(side);
            let shift = (u32::from(side) + 4 * direction) & 31;
            let word = (1u32 << shift) as u16;
            unsafe { write_unaligned(r.record(3, 3).add(2).cast::<u16>(), word) };
            assert_eq!(r.reversible() & 255, u32::from(shift < 16));
        }
    }
}

#[test]
fn distance_and_score_ports_preserve_existing_x87_values_and_control_state() {
    let mut w = World::new();
    let mut page = Object::new(0x24000);
    let node = node_at(&mut page, 0, -1, 5, 19.0, 0);
    let mut pages = [page.ptr()];
    let pf = w.pf();
    pf.set_pages(pages.as_mut_ptr());
    pf.set_next_node(page.ptr().wrapping_add(0x24));
    let sentinel = 0x4009_21FB_5444_2D18u64;
    let mut restored = 0u64;
    let (mut mxcsr_before, mut mxcsr_after) = (0u32, 0u32);
    let (flags_before, flags_after): (u32, u32);
    let control = crate::x87::control_word();
    unsafe {
        core::arch::asm!(
            "stmxcsr [{mxcsr}]", "pushfd", "pop {flags}", "fld qword ptr [{sentinel}]",
            mxcsr = in(reg) &mut mxcsr_before, sentinel = in(reg) &sentinel,
            flags = out(reg) flags_before, out("st(0)") _,
        );
    }
    let top = crate::x87::status_word() & 0x3800;
    assert_eq!(w.distance(-1, 5).st0, F80::from_i32(11));
    unsafe { fc4p_006d6780(w.object.ptr()) };
    assert_eq!(node.score(), 30.0);
    assert_eq!(crate::x87::status_word() & 0x3800, top);
    unsafe {
        core::arch::asm!(
            "fstp qword ptr [{restored}]", "stmxcsr [{mxcsr}]", "pushfd", "pop {flags}",
            restored = in(reg) &mut restored, mxcsr = in(reg) &mut mxcsr_after,
            flags = out(reg) flags_after, out("st(0)") _,
        );
    }
    assert_eq!(restored, sentinel);
    assert_eq!(crate::x87::control_word(), control);
    assert_eq!(mxcsr_before, mxcsr_after);
    assert_eq!(flags_before & (1 << 10), flags_after & (1 << 10));
}

#[test]
fn reversible_return_registers_keep_entry_edx_or_the_last_turn_operand() {
    let mut r = Route::new(&[]);
    let mut call = RawCall::new(
        Conv::Thiscall,
        fc4p_006d61e0 as *const () as usize,
        r.world.object.ptr() as u32,
        &[],
    );
    call.eax = 0xAABB_CC33;
    call.edx = 0xDDCC_BBAA;
    unsafe { call_raw(&mut call) };
    assert_eq!((call.eax, call.edx), (0xAABB_CC01, 0xDDCC_BBAA));
    let mut r = Route::new(&[(4, 3, 1, 0), (3, 3, 1, 0), (2, 3, 1, 0)]);
    let mut call = RawCall::new(
        Conv::Thiscall,
        fc4p_006d61e0 as *const () as usize,
        r.world.object.ptr() as u32,
        &[],
    );
    unsafe { call_raw(&mut call) };
    assert_eq!((call.eax, call.edx), (0, 1));
    let mut r = Route::new(&[(4, 3, 2, 0), (3, 3, 1, 0), (2, 3, 1, 0)]);
    let record = r.record(3, 3);
    unsafe { write_unaligned(record.add(0x14).cast::<u16>(), 0xFFFF) };
    let mut call = RawCall::new(
        Conv::Thiscall,
        fc4p_006d61e0 as *const () as usize,
        r.world.object.ptr() as u32,
        &[],
    );
    unsafe { call_raw(&mut call) };
    assert_eq!((call.eax, call.edx), (record as u32 & !255, 15));
}
