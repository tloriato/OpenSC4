// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! Native counterparts of the core's bd11_00_messages cases, with the callback order and raw words checked.

use super::*;
use crate::testing::{fake, Fake, GameFpu, Object};
use std::cell::RefCell;
use std::collections::VecDeque;

const HIGH: u32 = 0x89AB_CDEF;
const RESULT: u32 = 0xA1B2_C3D4;
const SAVED: [u32; 4] = [0x1122_3344, 0x5566_7788, 0x99AA_BBCC, 0xDDEE_FF00];

#[derive(Debug, PartialEq)]
enum Event {
    V(usize, u32, Vec<u32>),
    G(u32, usize, Vec<u32>),
    Terrain(usize, [i32; 4], u32),
    Place(usize, u32, [u32; 3], u32),
    Append(usize, u32),
    Find(u32, u32, u32),
    Erase(usize, u32),
}

struct Step {
    event: Event,
    result: u64,
    float: f64,
}

fn step(event: Event, eax: u32) -> Step {
    Step {
        event,
        result: (u64::from(HIGH) << 32) | u64::from(eax),
        float: 0.0,
    }
}
fn v(object: *mut u8, slot: u32, args: &[u32], eax: u32) -> Step {
    step(Event::V(object as usize, slot, args.to_vec()), eax)
}
fn g(address: u32, object: *mut u8, args: &[u32]) -> Step {
    step(Event::G(address, object as usize, args.to_vec()), RESULT)
}
thread_local! {
    static SCRIPT: RefCell<VecDeque<Step>> = const { RefCell::new(VecDeque::new()) };
    static SCRATCH: RefCell<Vec<usize>> = const { RefCell::new(Vec::new()) };
}
fn script(steps: Vec<Step>) {
    SCRIPT.with(|s| {
        assert!(s.borrow().is_empty());
        *s.borrow_mut() = steps.into();
    });
}
fn take(event: Event) -> Step {
    SCRIPT.with(|s| {
        let expected = s.borrow_mut().pop_front().expect("unexpected callback");
        assert_eq!(event, expected.event);
        expected
    })
}
fn finished() {
    SCRIPT.with(|s| assert!(s.borrow().is_empty(), "missing callbacks"));
}

macro_rules! getter {
    ($name:ident, $slot:literal) => {
        extern "thiscall" fn $name(this: *mut u8) -> u64 {
            take(Event::V(this as usize, $slot, vec![])).result
        }
    };
}
getter!(get_kind, 0x10);
getter!(get_data1, 0x24);
getter!(get_data2, 0x38);
getter!(get_data3, 0x4C);
getter!(get_data4, 0x60);
getter!(get_void1, 0x28);
getter!(get_unknown, 0x80);
getter!(get_holder, 0x18);
getter!(add_ref, 0x04);
getter!(release, 0x08);
extern "thiscall" fn get_param(this: *mut u8, index: u32) -> u64 {
    take(Event::V(this as usize, 0x14, vec![index])).result
}
extern "thiscall" fn has_property(this: *mut u8, property: u32) -> u64 {
    take(Event::V(this as usize, 0x0C, vec![property])).result
}
extern "thiscall" fn in_group(this: *mut u8, group: u32) -> u64 {
    take(Event::V(this as usize, 0x58, vec![group])).result
}
extern "thiscall" fn building_type(this: *mut u8, flag: u32) -> u64 {
    take(Event::V(this as usize, 0xB8, vec![flag])).result
}
extern "thiscall" fn get_float(this: *mut u8) -> f64 {
    take(Event::V(this as usize, 0x90, vec![])).float
}
extern "thiscall" fn monthly_fake(this: *mut u8) -> u64 {
    take(Event::G(0x0069_6130, this as usize, vec![])).result
}
extern "thiscall" fn idle_fake(this: *mut u8, budget: u32) -> u64 {
    take(Event::G(0x0069_9EA0, this as usize, vec![budget])).result
}
extern "thiscall" fn daily_fake(this: *mut u8, day: u32) -> u64 {
    take(Event::G(0x0069_B6D0, this as usize, vec![day])).result
}
extern "thiscall" fn terrain_fake(this: *mut u8, rect: *const [i32; 4], kind: u32) -> u64 {
    SCRATCH.with(|s| s.borrow_mut().push(rect as usize));
    // SAFETY: the dispatcher supplies a complete four-word rectangle.
    take(Event::Terrain(this as usize, unsafe { read(rect) }, kind)).result
}
extern "thiscall" fn place_fake(this: *mut u8, building: u32, position: *const [u32; 3], mode: u32) -> u64 {
    SCRATCH.with(|s| s.borrow_mut().push(position as usize));
    // SAFETY: the command supplies three consecutive float words.
    take(Event::Place(this as usize, building, unsafe { read(position) }, mode)).result
}
extern "C" fn classify_fake(state: u32) -> u64 {
    let result = take(Event::G(0x0069_5260, 0, vec![state])).result;
    assert_eq!(result as u32, u32::from(matches!(state as u8, 2 | 3)));
    result
}
extern "thiscall" fn changed_fake(this: *mut u8, lot: u32, wealth: u32, built: u32) -> u64 {
    take(Event::G(0x0069_CA80, this as usize, vec![lot, wealth, built])).result
}
extern "thiscall" fn append_fake(this: *mut u8, occupant: *const u32) -> u64 {
    // SAFETY: the original argument word holds the occupant pointer throughout this call.
    take(Event::Append(this as usize, unsafe { read(occupant) })).result
}
extern "C" fn find_fake(out: *mut u32, mut first: u32, end: u32, occupant: *const u32) -> u64 {
    // SAFETY: test nodes are three-word list nodes, and the dispatcher supplies the occupant word.
    unsafe {
        let wanted = read(occupant);
        let result = take(Event::Find(first, end, wanted)).result;
        while first != end && read((first as *const u32).add(2)) != wanted {
            first = read(first as *const u32);
        }
        write(out, first);
        result
    }
}
extern "thiscall" fn erase_fake(this: *mut u8, out: *mut u32, node: u32) -> u64 {
    let result = take(Event::Erase(this as usize, node)).result;
    // SAFETY: the test's node links are writable and out is the caller's argument word.
    unsafe {
        let node = node as *mut u32;
        let next = read(node);
        let prev = read(node.add(1));
        write(prev as *mut u32, next);
        write((next as *mut u32).add(1), prev);
        write(out, next);
    }
    result
}

#[repr(C)]
#[derive(Default)]
struct Frame {
    argument: u32,
    eax: u32,
    edx: u32,
    saved: [u32; 4],
    esp_before: u32,
    esp_after: u32,
    argument_slot: u32,
}

// Preserve the actual host registers, seed all four callee-saved registers, and observe the original
// argument slot before anything reuses its stack space. An incorrect pop count also breaks this frame.
#[unsafe(naked)]
unsafe extern "fastcall" fn call_message(this: *mut u8, edx: u32, frame: *mut Frame) {
    core::arch::naked_asm!(
        "push ebp", "push ebx", "push esi", "push edi",
        "mov eax, [esp + 20]", "mov [eax + 28], esp",
        "mov [eax + 36], esp", "sub dword ptr [eax + 36], 4",
        "push dword ptr [eax]",
        "mov ebx, 0x11223344", "mov esi, 0x55667788", "mov edi, 0x99aabbcc", "mov ebp, 0xddeeff00",
        "call {entry}",
        "mov ecx, [esp - 4]", "push ecx", "push eax", "mov eax, [esp + 28]",
        "lea ecx, [esp + 8]", "mov [eax + 32], ecx", "mov ecx, [esp]", "mov [eax + 4], ecx", "mov [eax + 8], edx",
        "mov [eax + 12], ebx", "mov [eax + 16], esi", "mov [eax + 20], edi", "mov [eax + 24], ebp",
        "mov ecx, [esp + 4]", "mov [eax], ecx", "add esp, 8",
        "pop edi", "pop esi", "pop ebx", "pop ebp", "ret 4",
        entry = sym fc4p_0069cce0,
    );
}

struct World {
    sim: Object,
    objects: Vec<Object>,
    table: Box<[usize; 64]>,
    _fakes: Vec<Fake>,
}
impl World {
    fn new() -> Self {
        let mut table = Box::new([0usize; 64]);
        for (slot, function) in [
            (0x04, add_ref as *const ()),
            (0x08, release as *const ()),
            (0x0C, has_property as *const ()),
            (0x10, get_kind as *const ()),
            (0x14, get_param as *const ()),
            (0x18, get_holder as *const ()),
            (0x1C, place_fake as *const ()),
            (0x24, get_data1 as *const ()),
            (0x28, get_void1 as *const ()),
            (0x38, get_data2 as *const ()),
            (0x4C, get_data3 as *const ()),
            (0x58, in_group as *const ()),
            (0x60, get_data4 as *const ()),
            (0x80, get_unknown as *const ()),
            (0x90, get_float as *const ()),
            (0xB8, building_type as *const ()),
        ] {
            table[slot / 4] = function as usize;
        }
        let mut sim = Object::new(0x100);
        // SAFETY: this is the primary vtable word in a complete simulator buffer.
        unsafe {
            write(sim.ptr().cast(), table.as_ptr());
        }
        let fakes = [
            (0x0069_6130, monthly_fake as *const ()),
            (0x0069_9EA0, idle_fake as *const ()),
            (0x0069_B6D0, daily_fake as *const ()),
            (0x0069_8610, terrain_fake as *const ()),
            (0x0069_CA80, changed_fake as *const ()),
            (0x0069_5260, classify_fake as *const ()),
            (0x0046_7B50, append_fake as *const ()),
            (0x004D_C710, find_fake as *const ()),
            (0x0041_3F70, erase_fake as *const ()),
        ]
        .into_iter()
        .map(|(address, function)| fake(address, function as usize))
        .collect();
        Self {
            sim,
            objects: Vec::new(),
            table,
            _fakes: fakes,
        }
    }
    fn object(&mut self) -> *mut u8 {
        let mut o = Object::new(0x10);
        let p = o.ptr();
        // SAFETY: the object's first word is its vtable.
        unsafe {
            write(p.cast(), self.table.as_ptr());
        }
        self.objects.push(o);
        p
    }
    fn primary(&mut self) -> *mut u8 {
        self.sim.ptr()
    }
    fn target(&mut self) -> *mut u8 {
        self.primary().wrapping_add(8)
    }
    fn view(&mut self) -> MessageTarget {
        // SAFETY: the simulator covers all message-target fields.
        unsafe { MessageTarget::from_ptr(self.target()) }
    }
    fn run(&mut self, message: *mut u8, steps: Vec<Step>) -> Frame {
        script(steps);
        let mut f = Frame {
            argument: message as u32,
            ..Frame::default()
        };
        // SAFETY: the frame, simulator and callback script supply the full original contract.
        unsafe {
            call_message(self.target(), 0xDEAD_BEEF, &mut f);
        }
        assert_eq!(f.saved, SAVED);
        assert_eq!(f.esp_before, f.esp_after);
        assert_eq!(f.eax as u8, 1);
        finished();
        f
    }
}

#[test]
fn unknown_messages_keep_arithmetic_return_bytes_and_do_not_write_objects() {
    let mut w = World::new();
    let m = w.object();
    for kind in [
        0,
        0x0B99_AE80,
        0x230E_27AB,
        0x278E_6002,
        0x278E_6004,
        0x6695_680F,
        0x6695_6811,
        0x6695_6813,
        0x6695_6815,
        0x6695_6817,
        0x6695_681B,
        0x99EF_1141,
        0x99EF_1144,
        0xE8BD_A2E6,
        0xE8BD_A2E8,
        u32::MAX,
    ] {
        // SAFETY: the complete simulator buffer is readable.
        let before = unsafe { read(w.primary().cast::<[u8; 0x100]>()) };
        let f = w.run(m, vec![v(m, 0x10, &[], kind)]);
        let eax = if kind > 0x278E_6003 && kind < 0x6695_6816 {
            kind.wrapping_sub(0x6695_6814)
        } else {
            kind
        };
        assert_eq!((f.eax, f.edx, f.argument), ((eax & !0xFF) | 1, HIGH, m as u32));
        // SAFETY: same complete simulator buffer.
        assert_eq!(unsafe { read(w.primary().cast::<[u8; 0x100]>()) }, before);
    }
}

#[test]
fn cheats_set_power_and_plumbing_and_toggle_every_raw_coffee_byte() {
    let mut w = World::new();
    let m = w.object();
    let view = w.view();
    for (id, offset) in [
        (0x4918_65E1, Some(0xDD)),
        (0x08DA_D89A, Some(0xDE)),
        (0x1234_5678, None),
    ] {
        for _ in 0..2 {
            // SAFETY: copy every byte to ensure only the selected flag is written.
            let mut expected = unsafe { read(w.primary().cast::<[u8; 0x100]>()) };
            if let Some(offset) = offset {
                expected[offset] = 1;
            }
            let f = w.run(m, vec![v(m, 0x10, &[], 0x230E_27AC), v(m, 0x24, &[], id)]);
            assert_eq!((f.eax, f.edx), ((id & !0xFF) | 1, HIGH));
            // SAFETY: the same complete buffer.
            assert_eq!(unsafe { read(w.primary().cast::<[u8; 0x100]>()) }, expected);
        }
    }
    for flag in 0..=u8::MAX {
        view.set_coffee_bar(flag);
        let f = w.run(m, vec![v(m, 0x10, &[], 0x230E_27AC), v(m, 0x24, &[], 0x6AB6_A2D7)]);
        assert_eq!(view.coffee_bar(), u8::from(flag == 0));
        assert_eq!((f.eax, f.edx), (0x6AB6_A201, HIGH));
    }
}

#[test]
fn simulation_messages_call_monthly_idle_and_daily_in_order_with_original_words() {
    let mut w = World::new();
    let p = w.primary();
    let m = w.object();
    for kind in [0x6695_6810, 0x6695_6816] {
        let f = w.run(m, vec![v(m, 0x10, &[], kind), g(0x0069_6130, p, &[])]);
        assert_eq!((f.eax, f.edx), (RESULT & !0xFF | 1, HIGH));
    }
    for word in [0, 5, 0x8000_0000, u32::MAX] {
        let f = w.run(
            m,
            vec![
                v(m, 0x10, &[], 0x6695_6814),
                v(m, 0x4C, &[], word),
                g(0x0069_9EA0, p, &[5]),
                g(0x0069_B6D0, p, &[word]),
            ],
        );
        assert_eq!((f.eax, f.edx), (RESULT & !0xFF | 1, HIGH));
        w.run(
            m,
            vec![
                v(m, 0x10, &[], 0x6695_681A),
                v(m, 0x4C, &[], word),
                g(0x0069_9EA0, p, &[word]),
            ],
        );
    }
}

#[test]
fn terrain_unpacks_and_clamps_signed_dimensions_without_reordering_inverted_rectangles() {
    let mut w = World::new();
    let p = w.primary();
    let m = w.object();
    for (x, z, counts, rect) in [
        (0x0003_0005, 0x0000_0040, [64, 64], [2, 0, 5, 63]),
        (0, 0x0001_FFFF, [64, 64], [0, 0, 0, 63]),
        (0x000B_0003, 0x0005_0004, [64, 64], [10, 4, 3, 4]),
        (u32::MAX, u32::MAX, [64, 64], [65534, 65534, 63, 63]),
        (0, 0, [0, -1], [0, 0, -1, -2]),
        (0x0001_0001, 0x0001_0001, [i32::MIN, i32::MAX], [0, 0, i32::MAX, 1]),
    ] {
        w.view().set_cells_x(counts[0]);
        w.view().set_cells_z(counts[1]);
        let f = w.run(
            m,
            vec![
                v(m, 0x10, &[], 0x278E_6003),
                v(m, 0x24, &[], x),
                v(m, 0x38, &[], z),
                v(m, 0x60, &[], 0xFFFF_FF07),
                step(Event::Terrain(p as usize, rect, 0xFFFF_FF07), RESULT),
            ],
        );
        assert_eq!((f.eax, f.edx, f.argument), (RESULT & !0xFF | 1, HIGH, m as u32));
        SCRATCH.with(|s| assert_eq!(s.borrow_mut().pop(), Some(f.argument_slot as usize - 20)));
    }
}

#[test]
fn place_command_reads_three_parameters_in_order_and_stores_rounded_x_y_minus_one() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    let p = w.primary();
    let m = w.object();
    let params = w.object();
    let a = w.object();
    let b = w.object();
    let c = w.object();
    for (x, y) in [
        (40.0, 72.0),
        (1.0 + 2f64.powi(-24), -0.0),
        (f64::INFINITY, f64::NEG_INFINITY),
    ] {
        let mut sx = v(b, 0x90, &[], 0);
        sx.float = x;
        let mut sy = v(c, 0x90, &[], 0);
        sy.float = y;
        let f = w.run(
            m,
            vec![
                v(m, 0x10, &[], 0x0B99_AE81),
                v(m, 0x28, &[], params as u32),
                v(params, 0x14, &[0], a as u32),
                v(a, 0x60, &[], 0x100),
                v(params, 0x14, &[1], b as u32),
                sx,
                v(params, 0x14, &[2], c as u32),
                sy,
                step(
                    Event::Place(
                        p as usize,
                        0x100,
                        [
                            F80::from_f64(x).to_f32().to_bits(),
                            F80::from_f64(y).to_f32().to_bits(),
                            (-1.0f32).to_bits(),
                        ],
                        2,
                    ),
                    RESULT,
                ),
            ],
        );
        assert_eq!((f.eax, f.edx, f.argument), (RESULT & !0xFF | 1, HIGH, m as u32));
        SCRATCH.with(|s| assert_eq!(s.borrow_mut().pop(), Some(f.argument_slot as usize - 20)));
    }
}

fn occupant_prefix(m: *mut u8, occupant: *mut u8, holder: *mut u8, kind: u32, preview: u32) -> Vec<Step> {
    vec![
        v(m, 0x10, &[], kind),
        v(m, 0x28, &[], occupant as u32),
        v(occupant, 0x18, &[], holder as u32),
        v(holder, 0x0C, &[0x0292_44DB], preview),
    ]
}

#[test]
fn occupant_notifications_skip_previews_and_non_special_groups_using_only_predicate_low_bytes() {
    let mut w = World::new();
    let m = w.object();
    let occupant = w.object();
    let holder = w.object();
    for kind in [0x99EF_1142, 0x99EF_1143] {
        for preview in [0xABCD_EF01, 0xABCD_EFFF] {
            let f = w.run(m, occupant_prefix(m, occupant, holder, kind, preview));
            assert_eq!(
                (f.eax, f.edx, f.argument),
                ((preview & !0xFF) | 1, HIGH, occupant as u32)
            );
        }
        let mut steps = occupant_prefix(m, occupant, holder, kind, 0x1234_5600);
        steps.extend([
            v(occupant, 0x58, &[0x150A], 0x4455_6600),
            v(occupant, 0x58, &[0x150B], 0x7788_9900),
        ]);
        let f = w.run(m, steps);
        assert_eq!((f.eax, f.edx, f.argument), (0x7788_9901, HIGH, occupant as u32));
    }
}

#[test]
fn special_insertions_keep_duplicates_and_add_ref_after_append() {
    let mut w = World::new();
    let list = w.target().wrapping_add(0x44);
    let m = w.object();
    let a = w.object();
    let b = w.object();
    let holder = w.object();
    for (occupant, first_group) in [(a, true), (b, false), (a, true)] {
        let mut steps = occupant_prefix(m, occupant, holder, 0x99EF_1142, 0);
        steps.push(v(occupant, 0x58, &[0x150A], u32::from(first_group)));
        if !first_group {
            steps.push(v(occupant, 0x58, &[0x150B], 0xABCD_EF80));
        }
        steps.extend([
            step(Event::Append(list as usize, occupant as u32), RESULT),
            v(occupant, 4, &[], 0x5566_7788),
        ]);
        let f = w.run(m, steps);
        assert_eq!((f.eax, f.edx, f.argument), (0x5566_7701, HIGH, occupant as u32));
    }
}

#[test]
fn removal_erases_only_the_first_match_releases_its_value_and_writes_the_successor_to_the_argument() {
    let mut w = World::new();
    let list = w.target().wrapping_add(0x44);
    let m = w.object();
    let a = w.object();
    let b = w.object();
    let holder = w.object();
    let mut sentinel = [0u32; 3];
    let mut one = [0u32; 3];
    let mut two = [0u32; 3];
    let mut three = [0u32; 3];
    let head = sentinel.as_mut_ptr() as u32;
    let n1 = one.as_mut_ptr() as u32;
    let n2 = two.as_mut_ptr() as u32;
    let n3 = three.as_mut_ptr() as u32;
    sentinel = [n1, n3, 0];
    one = [n2, head, b as u32];
    two = [n3, n1, a as u32];
    three = [head, n2, a as u32];
    w.view().set_special(head as *mut u8);
    let mut steps = occupant_prefix(m, a, holder, 0x99EF_1143, 0);
    steps.extend([
        v(a, 0x58, &[0x150A], 1),
        step(Event::Find(n1, head, a as u32), RESULT),
        step(Event::Erase(list as usize, n2), RESULT),
        v(a, 8, &[], RESULT),
    ]);
    let f = w.run(m, steps);
    assert_eq!((f.eax, f.edx, f.argument), (RESULT & !0xFF | 1, HIGH, n3));
    assert_eq!(
        (sentinel, one, two, three),
        (
            [n1, n3, 0],
            [n3, head, b as u32],
            [n3, n1, a as u32],
            [head, n1, a as u32]
        )
    );
    let absent = w.object();
    let mut steps = occupant_prefix(m, absent, holder, 0x99EF_1143, 0);
    steps.extend([
        v(absent, 0x58, &[0x150A], 0),
        v(absent, 0x58, &[0x150B], 1),
        step(Event::Find(n1, head, absent as u32), RESULT),
    ]);
    let f = w.run(m, steps);
    assert_eq!((f.eax, f.edx, f.argument), (head & !0xFF | 1, HIGH, absent as u32));
}

#[test]
fn state_notifications_guard_null_and_missing_building_before_reading_states() {
    let mut w = World::new();
    let m = w.object();
    let lot = w.object();
    let f = w.run(m, vec![v(m, 0x10, &[], 0xE8BD_A2E7), v(m, 0x80, &[], 0)]);
    assert_eq!((f.eax, f.edx, f.argument), (1, HIGH, m as u32));
    let f = w.run(
        m,
        vec![
            v(m, 0x10, &[], 0xE8BD_A2E7),
            v(m, 0x80, &[], lot as u32),
            v(lot, 0xB8, &[0], 0),
        ],
    );
    assert_eq!((f.eax, f.edx, f.argument), (1, HIGH, m as u32));
}

#[test]
fn state_classification_is_new_then_old_and_reuses_the_argument_high_bytes_for_changed() {
    let mut w = World::new();
    let p = w.primary();
    let m = w.object();
    let lot = w.object();
    for old in [0, 1, 2, 3, 4, 0xFFFF_FF02, 0x10003, u32::MAX] {
        for new in [0, 1, 2, 3, 4, 0xFFFF_FF02, 0x10003, u32::MAX] {
            let old_built = u32::from(matches!(old as u8, 2 | 3));
            let new_built = u32::from(matches!(new as u8, 2 | 3));
            let argument = (m as u32 & !0xFF) | new_built;
            let mut steps = vec![
                v(m, 0x10, &[], 0xE8BD_A2E7),
                v(m, 0x80, &[], lot as u32),
                v(lot, 0xB8, &[0], 0x8000_0000),
                v(m, 0x24, &[], old),
                v(m, 0x38, &[], new),
                step(Event::G(0x0069_5260, 0, vec![new]), new_built),
                step(Event::G(0x0069_5260, 0, vec![old]), old_built),
            ];
            if new_built != old_built {
                steps.extend([
                    v(m, 0x60, &[], 0x1234_5603),
                    g(0x0069_CA80, p, &[lot as u32, 0x1234_5603, argument]),
                ]);
            }
            let f = w.run(m, steps);
            assert_eq!(
                (f.eax, f.edx, f.argument),
                (
                    if new_built == old_built { 1 } else { RESULT & !0xFF | 1 },
                    HIGH,
                    argument
                )
            );
        }
    }
}

#[test]
fn shared_debug_getter_reads_raw_byte_without_writes_and_retains_the_other_return_bytes() {
    let mut object = [0xA5u8; 0x100];
    let p = object.as_mut_ptr();
    // SAFETY: all flag bytes and the raw-call register contract are supplied by this buffer.
    unsafe {
        let flag = DebugGrowthFlag::from_ptr(p);
        for byte in 0..=u8::MAX {
            flag.set_value(byte);
            let before = object;
            let mut c = RawCall::new(Conv::Thiscall, fc4p_0098ad60 as *const () as usize, p as u32, &[]);
            c.eax = 0x1234_56FF;
            c.edx = 0xABCD_EF01;
            call_raw(&mut c);
            assert_eq!((c.eax, c.edx), (0x1234_5600 | u32::from(byte), 0xABCD_EF01));
            assert_eq!(object, before);
        }
    }
}

#[unsafe(naked)]
unsafe extern "C" fn keep_registers() {
    core::arch::naked_asm!("ret");
}

#[unsafe(naked)]
unsafe extern "C" fn low_byte_false() {
    core::arch::naked_asm!("mov al, 0", "ret 4");
}

#[unsafe(naked)]
unsafe extern "C" fn kind_zero_keeps_edx() {
    core::arch::naked_asm!("xor eax, eax", "ret");
}

#[unsafe(naked)]
unsafe extern "C" fn data3_low_byte() {
    core::arch::naked_asm!("mov al, 0x80", "ret");
}

#[unsafe(naked)]
unsafe extern "C" fn empty_find_keeps_edx() {
    core::arch::naked_asm!("mov eax, [esp + 4]", "mov ecx, [esp + 8]", "mov [eax], ecx", "ret",);
}

#[test]
fn empty_removal_retains_the_original_iterator_scratch_address_in_edx() {
    let mut w = World::new();
    let m = w.object();
    let occupant = w.object();
    let holder = w.object();
    let mut sentinel = [0u32; 3];
    let head = sentinel.as_mut_ptr() as u32;
    sentinel = [head, head, 0];
    w.view().set_special(head as *mut u8);
    let _find = fake(0x004D_C710, empty_find_keeps_edx as *const () as usize);
    let mut steps = occupant_prefix(m, occupant, holder, 0x99EF_1143, 0);
    steps.push(v(occupant, 0x58, &[0x150A], 1));
    let f = w.run(m, steps);
    assert_eq!(
        (f.eax, f.edx, f.argument),
        (head & !0xFF | 1, f.argument_slot - 24, occupant as u32)
    );
    assert_eq!(sentinel, [head, head, 0]);
}

#[test]
fn retained_registers_receive_the_original_table_and_arithmetic_inputs() {
    let mut w = World::new();
    let m = w.object();
    w.table[0x10 / 4] = kind_zero_keeps_edx as *const () as usize;
    let f = w.run(m, vec![]);
    assert_eq!((f.eax, f.edx), (1, 0xDEAD_BEEF));
    w.table[0x10 / 4] = get_kind as *const () as usize;
    w.table[0x4C / 4] = data3_low_byte as *const () as usize;
    let p = w.primary();
    w.run(
        m,
        vec![
            v(m, 0x10, &[], 0x6695_6814),
            g(0x0069_9EA0, p, &[5]),
            g(0x0069_B6D0, p, &[0x80]),
        ],
    );
    w.run(
        m,
        vec![
            v(m, 0x10, &[], 0x6695_681A),
            g(0x0069_9EA0, p, &[w.table.as_ptr() as u32 & !0xFF | 0x80]),
        ],
    );
    let _monthly = fake(0x0069_6130, keep_registers as *const () as usize);
    for kind in [0x6695_6810, 0x6695_6816] {
        let f = w.run(m, vec![v(m, 0x10, &[], kind)]);
        assert_eq!(
            (f.eax, f.edx),
            (if kind == 0x6695_6810 { 1 } else { kind & !0xFF | 1 }, HIGH)
        );
    }
    let occupant = w.object();
    let holder = w.object();
    w.table[0x0C / 4] = low_byte_false as *const () as usize;
    w.table[0x58 / 4] = low_byte_false as *const () as usize;
    let f = w.run(
        m,
        vec![
            v(m, 0x10, &[], 0x99EF_1142),
            v(m, 0x28, &[], occupant as u32),
            v(occupant, 0x18, &[], holder as u32),
        ],
    );
    let table = w.table.as_ptr() as u32;
    assert_eq!((f.eax, f.edx, f.argument), (table & !0xFF | 1, table, occupant as u32));
}

fn environment() -> (u16, u16, u32, u32) {
    let mut mxcsr = 0u32;
    let flags: u32;
    // SAFETY: inspect control state without changing it; the push and pop balance.
    unsafe {
        core::arch::asm!("stmxcsr [{p}]", "pushfd", "pop {flags}",
            p = in(reg) &mut mxcsr, flags = out(reg) flags);
    }
    (
        crate::x87::control_word(),
        crate::x87::status_word() & 0x3800,
        mxcsr,
        flags & 0x400,
    )
}

#[test]
fn command_and_getter_keep_an_existing_x87_value_control_word_mxcsr_and_direction_flag() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    let p = w.primary();
    let m = w.object();
    let params = w.object();
    let a = w.object();
    let b = w.object();
    let c = w.object();
    let mut x = v(b, 0x90, &[], 0);
    x.float = 40.0;
    let mut y = v(c, 0x90, &[], 0);
    y.float = 72.0;
    let steps = vec![
        v(m, 0x10, &[], 0x0B99_AE81),
        v(m, 0x28, &[], params as u32),
        v(params, 0x14, &[0], a as u32),
        v(a, 0x60, &[], 0x100),
        v(params, 0x14, &[1], b as u32),
        x,
        v(params, 0x14, &[2], c as u32),
        y,
        step(
            Event::Place(
                p as usize,
                0x100,
                [40.0f32.to_bits(), 72.0f32.to_bits(), (-1.0f32).to_bits()],
                2,
            ),
            RESULT,
        ),
    ];
    let sentinel = 123.25f64;
    let mut restored = 0.0f64;
    // SAFETY: push a sentinel that both ports must leave in place.
    unsafe {
        core::arch::asm!("fld qword ptr [{p}]", p = in(reg) &sentinel, out("st(0)") _);
    }
    let before = environment();
    w.run(m, steps);
    let after_message = environment();
    let mut call = RawCall::new(Conv::Thiscall, fc4p_0098ad60 as *const () as usize, p as u32, &[]);
    // SAFETY: the simulator covers the getter's flag, and the sentinel is then popped exactly once.
    unsafe {
        call_raw(&mut call);
    }
    let after_getter = environment();
    // SAFETY: the value left by the initial fld is still the top stack item.
    unsafe {
        core::arch::asm!("fstp qword ptr [{p}]", p = in(reg) &mut restored, out("st(0)") _);
    }
    assert_eq!(after_message, before);
    assert_eq!(after_getter, before);
    assert_eq!(restored.to_bits(), sentinel.to_bits());
}
