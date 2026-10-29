// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Core message cases, plus argument storage and partial integer results.

use super::*;
use crate::testing::{fake, fake_data, Fake, Object};
use std::cell::RefCell;

#[derive(Default)]
struct State {
    calls: Vec<(u32, u32, Vec<u32>)>,
    kind: u32,
    data: [u32; 2],
    width: i32,
    height: i32,
    rect: [i32; 4],
    occupant: u32,
    occupant_kind: u32,
    pole: u32,
    pole_flag: u32,
    allocator: u32,
    framework: u32,
    header: u32,
    current: u32,
    nodes: Vec<u32>,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn record(this: u32, slot: u32, args: &[u32]) {
    STATE.with(|s| s.borrow_mut().calls.push((this, slot, args.to_vec())));
}
fn answer() -> u64 {
    pair(0xAABB_CC00, 0xDDDD_EEEE)
}
unsafe extern "fastcall" fn message_type(this: *mut u8, edx: u32) -> u64 {
    record(this as u32, 0x10, &[edx]);
    pair(STATE.with(|s| s.borrow().kind), 0x1111_2222)
}
unsafe extern "thiscall" fn data1(this: *mut u8) -> u64 {
    record(this as u32, 0x24, &[]);
    pair(STATE.with(|s| s.borrow().data[0]), 0x3333_4444)
}
unsafe extern "thiscall" fn data2(this: *mut u8) -> u64 {
    record(this as u32, 0x38, &[]);
    pair(STATE.with(|s| s.borrow().data[1]), 0x5555_6666)
}
unsafe extern "thiscall" fn occupant(this: *mut u8) -> u64 {
    record(this as u32, 0x28, &[]);
    pair(STATE.with(|s| s.borrow().occupant), 0)
}
unsafe extern "thiscall" fn occupant_kind(this: *mut u8) -> u64 {
    record(this as u32, 0x1C, &[]);
    pair(STATE.with(|s| s.borrow().occupant_kind), 0x7777_8888)
}
unsafe extern "thiscall" fn pole_flag(this: *mut u8) -> u64 {
    record(this as u32, 0x34, &[]);
    pair(STATE.with(|s| s.borrow().pole_flag), 0x9999_AAAA)
}
unsafe extern "thiscall" fn query(this: *mut u8, iid: u32, output: *mut u32) -> u64 {
    record(this as u32, 0, &[iid, output as u32]);
    unsafe {
        output.write_unaligned(STATE.with(|s| s.borrow().pole));
    }
    answer()
}
unsafe extern "thiscall" fn reference_output(this: *mut u32) -> u64 {
    assert_eq!(unsafe { this.read_unaligned() }, 0);
    record(this as u32, 0x004F_20A0, &[]);
    pair(this as u32, 0)
}
unsafe extern "thiscall" fn remove_occupant(this: *mut u8, occupant: u32) -> u64 {
    record(this as u32, 0x20, &[occupant]);
    answer()
}
unsafe extern "thiscall" fn release(this: *mut u8) -> u64 {
    record(this as u32, 8, &[]);
    answer()
}
unsafe extern "thiscall" fn width(this: *mut u8) -> u64 {
    record(this as u32, 0x174, &[]);
    pair(STATE.with(|s| s.borrow().width) as u32, 0)
}
unsafe extern "thiscall" fn height(this: *mut u8) -> u64 {
    record(this as u32, 0x178, &[]);
    pair(STATE.with(|s| s.borrow().height) as u32, 0)
}
unsafe extern "thiscall" fn terrain_changed(this: *mut u8, rect: *const [i32; 4]) -> u64 {
    let rect = unsafe { rect.read_unaligned() };
    record(this as u32, 0x006D_0BE0, &rect.map(|n| n as u32));
    STATE.with(|s| s.borrow_mut().rect = rect);
    answer()
}
unsafe extern "thiscall" fn push_deals(this: *mut u8, deals: u32, stats: u32) -> u64 {
    record(this as u32, 0x006D_0A40, &[deals, stats]);
    answer()
}
#[unsafe(naked)]
unsafe extern "C" fn simulate_deals() {
    core::arch::naked_asm!("push edx", "mov edx, eax", "call {}", "ret", sym simulate_body);
}
unsafe extern "fastcall" fn simulate_body(this: *mut u8, eax: u32, edx: u32) -> u64 {
    record(
        this as u32,
        0x006D_0E50,
        &[
            eax,
            edx,
            unsafe { *this.add(0x3C) }.into(),
            unsafe { *this.add(0x3D) }.into(),
        ],
    );
    answer()
}
unsafe extern "thiscall" fn clear_deals(this: *mut u8) -> u64 {
    record(this as u32, 0x0042_1660, &[]);
    answer()
}
unsafe extern "C" fn framework() -> u64 {
    record(0, 0x0087_93EC, &[]);
    pair(STATE.with(|s| s.borrow().framework), 0)
}
unsafe extern "thiscall" fn get_allocator(this: *mut u8, service: u32, iid: u32, output: *mut u32) -> u64 {
    record(this as u32, 0x14, &[service, iid]);
    unsafe {
        output.write_unaligned(STATE.with(|s| s.borrow().allocator));
    }
    answer()
}
unsafe extern "thiscall" fn release_reference(this: *mut u32) -> u64 {
    record(0, 0x0041_1870, &[unsafe { this.read_unaligned() }]);
    unsafe {
        this.write_unaligned(0);
    }
    answer()
}
unsafe extern "thiscall" fn allocate(this: *mut u8, size: u32) -> u64 {
    record(this as u32, 0x0C, &[size]);
    pair(STATE.with(|s| s.borrow().header), 0)
}
unsafe extern "thiscall" fn free(this: *mut u8, node: *mut u8) -> u64 {
    record(this as u32, 0x10, &[node as u32]);
    unsafe {
        let prev = pointer(node, 4);
        let next = pointer(node, 0);
        assert_eq!(pointer(prev, 0), next);
        assert_eq!(pointer(next, 4), prev);
    }
    answer()
}
unsafe extern "thiscall" fn enumerate(this: *mut u8, output: *mut u32, flag: u32) -> u64 {
    record(this as u32, 0x50, &[output as u32, flag]);
    unsafe {
        let header = output.read_unaligned() as *mut u8;
        assert_eq!(pointer(header, 0), header);
        assert_eq!(pointer(header, 4), header);
        assert_eq!(word(header, 8), 0xCCDD_EEFF);
        let nodes = STATE.with(|s| s.borrow().nodes.clone());
        let mut previous = header;
        for node in nodes {
            let node = node as *mut u8;
            node.cast::<*mut u8>().write_unaligned(header);
            node.add(4).cast::<*mut u8>().write_unaligned(previous);
            previous.cast::<*mut u8>().write_unaligned(node);
            header.add(4).cast::<*mut u8>().write_unaligned(node);
            previous = node;
        }
    }
    answer()
}
unsafe extern "thiscall" fn current(this: *mut u8) -> u64 {
    record(this as u32, 0x90, &[]);
    pair(STATE.with(|s| s.borrow().current), 0)
}
unsafe extern "thiscall" fn set_deals(this: *mut u8, list: u32) -> u64 {
    record(this as u32, 0x114, &[list]);
    answer()
}
unsafe extern "thiscall" fn update(this: *mut u8) -> u64 {
    record(this as u32, 0x11C, &[]);
    answer()
}
unsafe extern "thiscall" fn destroy(this: *mut u32) -> u64 {
    record(this as u32, 0x0041_1FE0, &[unsafe { this.read_unaligned() }]);
    let header = unsafe { this.read_unaligned() as *mut u8 };
    assert_eq!(unsafe { pointer(header, 0) }, header);
    assert_eq!(unsafe { pointer(header, 4) }, header);
    answer()
}

struct Fixture {
    _tables: Vec<Vec<usize>>,
    objects: Vec<Object>,
    _guards: Vec<Fake>,
    p: Vec<*mut u8>,
}
impl Fixture {
    fn new() -> Self {
        let mut objects: Vec<_> = (0..13).map(|_| Object::new(0x80)).collect();
        let p: Vec<_> = objects.iter_mut().map(Object::ptr).collect();
        let mut tables = vec![vec![0usize; 0x188 / 4]; 13];
        tables[0][0x20 / 4] = remove_occupant as *const () as usize;
        tables[0][0x50 / 4] = enumerate as *const () as usize;
        tables[1][0x10 / 4] = message_type as *const () as usize;
        tables[1][0x24 / 4] = data1 as *const () as usize;
        tables[1][0x38 / 4] = data2 as *const () as usize;
        tables[1][0x28 / 4] = occupant as *const () as usize;
        tables[2][0x174 / 4] = width as *const () as usize;
        tables[2][0x178 / 4] = height as *const () as usize;
        tables[3][0] = query as *const () as usize;
        tables[3][0x1C / 4] = occupant_kind as *const () as usize;
        tables[4][0x34 / 4] = pole_flag as *const () as usize;
        tables[4][2] = release as *const () as usize;
        tables[5][0x0C / 4] = allocate as *const () as usize;
        tables[5][0x10 / 4] = free as *const () as usize;
        tables[6][0x14 / 4] = get_allocator as *const () as usize;
        tables[7][0x90 / 4] = current as *const () as usize;
        for table in &mut tables[8..10] {
            table[0x114 / 4] = set_deals as *const () as usize;
            table[0x11C / 4] = update as *const () as usize;
            table[2] = release as *const () as usize;
        }
        unsafe {
            for (object, table) in p.iter().zip(&tables) {
                object.cast::<*const usize>().write(table.as_ptr());
            }
            NeighborsSim::from_ptr(p[0]).set_city(p[2]);
        }
        let guards = [
            (0x006D_0BE0, terrain_changed as *const () as usize),
            (0x006D_0A40, push_deals as *const () as usize),
            (0x006D_0E50, simulate_deals as *const () as usize),
            (0x004F_20A0, reference_output as *const () as usize),
            (0x0042_1660, clear_deals as *const () as usize),
            (0x0087_93EC, framework as *const () as usize),
            (0x0041_1870, release_reference as *const () as usize),
            (0x0041_1FE0, destroy as *const () as usize),
        ]
        .into_iter()
        .map(|(a, f)| fake(a, f))
        .collect();
        STATE.with(|s| {
            *s.borrow_mut() = State {
                occupant: p[3] as u32,
                pole: p[4] as u32,
                occupant_kind: 0x2890_D4DE,
                allocator: p[5] as u32,
                framework: p[6] as u32,
                current: p[8] as u32,
                header: p[10] as u32,
                width: 64,
                height: 64,
                ..State::default()
            }
        });
        Self {
            _tables: tables,
            objects,
            _guards: guards,
            p,
        }
    }
    fn dispatch(&self, kind: u32) -> (u64, u32) {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.kind = kind;
            s.calls.clear();
        });
        let mut argument = self.p[1] as u32;
        let result = unsafe { dispatch(self.p[0].add(4), &mut argument, 0xDEAD_BEEF) };
        (result, argument)
    }
    fn calls(&self) -> Vec<(u32, u32, Vec<u32>)> {
        STATE.with(|s| s.borrow().calls.clone())
    }
}

#[test]
fn dates_status_reports_ready_and_unknown_messages_keep_partial_results() {
    let f = Fixture::new();
    for kind in [
        0,
        0x26D3_1EC3,
        0x6695_6810,
        0x6695_6811,
        0x6695_6813,
        0x6695_6815,
        0x6695_6817,
        0x6695_6819,
        0x6695_681B,
        u32::MAX,
    ] {
        let eax = if kind > 0x6695_6810 && kind < 0x6695_681A {
            kind.wrapping_sub(0x6695_6816)
        } else {
            kind
        };
        assert_eq!(f.dispatch(kind), (pair((eax & !0xFF) | 1, 0x1111_2222), f.p[1] as u32));
        assert_eq!(f.calls(), vec![(f.p[1] as u32, 0x10, vec![0xDEAD_BEEF])]);
    }
    assert_eq!(f.dispatch(0x6AC2_84F3).0, pair(0x6AC2_8401, 0x1111_2222));
    assert_eq!(unsafe { *f.p[0].add(0x0C) }, 1);
    for kind in [0x6695_6814, 0x6695_681A] {
        unsafe {
            *f.p[0].add(0x3C) = 0x80;
            *f.p[0].add(0x3D) = 0xFF;
        }
        assert_eq!(f.dispatch(kind).0, answer() | 1);
        assert_eq!(f.calls()[1], (f.p[0] as u32, 0x006D_0A40, vec![0x80, 0xFF]));
    }
    assert_eq!(f.dispatch(0x6695_6816).0, answer() | 1);
    assert_eq!(
        f.calls()[1],
        (f.p[0] as u32, 0x006D_0E50, vec![0, 0x1111_2222, 0x80, 0xFF])
    );
    for status in [0, 8, 1, u32::MAX] {
        unsafe {
            *f.p[0].add(0x3C) = 0;
            *f.p[0].add(0x3D) = 0;
        }
        STATE.with(|s| s.borrow_mut().data[0] = status);
        assert_eq!(f.dispatch(0xEA41_8D8F).0, answer() | 1);
        let flags = if status == 0 || status == 8 { [0, 1] } else { [1, 0] };
        assert_eq!(
            f.calls(),
            vec![
                (f.p[1] as u32, 0x10, vec![0xDEAD_BEEF]),
                (f.p[1] as u32, 0x24, vec![]),
                (
                    f.p[0] as u32,
                    0x006D_0E50,
                    vec![status, 0x3333_4444, flags[0], flags[1]]
                ),
            ]
        );
    }
}

#[test]
fn terrain_messages_unpack_clamp_and_keep_inverted_rectangles() {
    let f = Fixture::new();
    for (data, size, expected) in [
        ([(1 << 16) | 63, (6 << 16) | 9], [64, 64], [0, 5, 63, 9]),
        ([0x0000_FFFF, 0xFFFF_FFFF], [32, 16], [0, 65534, 31, 15]),
        ([0, 0], [0, -5], [0, 0, -1, -6]),
        ([0, 0], [i32::MIN, i32::MIN], [0, 0, 0, 0]),
    ] {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.data = data;
            s.width = size[0];
            s.height = size[1];
        });
        assert_eq!(f.dispatch(0x278E_6003).0, answer() | 1);
        assert_eq!(STATE.with(|s| s.borrow().rect), expected);
        assert_eq!(
            f.calls(),
            vec![
                (f.p[1] as u32, 0x10, vec![0xDEAD_BEEF]),
                (f.p[1] as u32, 0x24, vec![]),
                (f.p[1] as u32, 0x38, vec![]),
                (f.p[2] as u32, 0x174, vec![]),
                (f.p[2] as u32, 0x178, vec![]),
                (f.p[0] as u32, 0x006D_0BE0, expected.map(|n| n as u32).to_vec()),
            ]
        );
    }
}

#[test]
fn occupant_removal_queries_only_poles_and_deletes_only_when_flag_is_clear() {
    let f = Fixture::new();
    assert_eq!(f.dispatch(0xAA7A_014D).0, answer() | 1);
    assert_eq!(
        f.calls(),
        vec![
            (f.p[1] as u32, 0x10, vec![0xDEAD_BEEF]),
            (f.p[1] as u32, 0x28, vec![]),
            (f.p[0] as u32, 0x20, vec![f.p[3] as u32])
        ]
    );
    STATE.with(|s| s.borrow_mut().occupant_kind = 0xC772_BF98);
    assert_eq!(f.dispatch(0x99EF_1143), (pair(0xC772_BF01, 0x7777_8888), f.p[1] as u32));
    assert_eq!(f.calls().len(), 3);
    for flag in [0, 1, 0xFFFF_FF00, 0xFFFF_FFFF] {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.occupant_kind = 0x2890_D4DE;
            s.pole_flag = flag;
        });
        assert_eq!(f.dispatch(0x99EF_1143), (answer() | 1, f.p[4] as u32));
        let calls = f.calls();
        let argument = calls[3].0;
        let mut expected = vec![
            (f.p[1] as u32, 0x10, vec![0xDEAD_BEEF]),
            (f.p[1] as u32, 0x28, vec![]),
            (f.p[3] as u32, 0x1C, vec![]),
            (argument, 0x004F_20A0, vec![]),
            (f.p[3] as u32, 0, vec![0xA8C8_6142, argument]),
            (f.p[4] as u32, 0x34, vec![]),
        ];
        if flag as u8 == 0 {
            expected.push((f.p[0] as u32, 0x20, vec![f.p[3] as u32]));
        }
        expected.push((f.p[4] as u32, 8, vec![]));
        assert_eq!(calls, expected);
    }
}

#[test]
fn city_reset_drains_backwards_releases_nodes_and_clears_dirty_after_pushes() {
    let mut f = Fixture::new();
    let mut cache = f.p[5];
    let mut manager = f.p[7];
    let _cache = fake_data(0x00B2_07E0, (&mut cache as *mut *mut u8).cast());
    let _manager = fake_data(0x00B4_3C94, (&mut manager as *mut *mut u8).cast());
    for (kind, cold, with_nodes) in [(0x26D3_1EC4, true, true), (0x26D3_1EC5, false, false)] {
        if cold {
            cache = core::ptr::null_mut();
        }
        unsafe {
            *f.p[0].add(0x3C) = 1;
            *f.p[0].add(0x3D) = 1;
            f.p[10].add(8).cast::<u32>().write_unaligned(0xCCDD_EEFF);
            f.p[11].add(8).cast::<*mut u8>().write_unaligned(f.p[9]);
            f.p[12].add(8).cast::<u32>().write_unaligned(0);
        }
        STATE.with(|s| {
            s.borrow_mut().nodes = if with_nodes {
                vec![f.p[11] as u32, f.p[12] as u32]
            } else {
                vec![]
            }
        });
        let (result, arg) = f.dispatch(kind);
        assert_eq!(result, answer() | 1);
        assert_eq!(arg, f.p[10] as u32);
        assert_eq!(cache, f.p[5]);
        assert_eq!(unsafe { [*f.p[0].add(0x3C), *f.p[0].add(0x3D)] }, [0, 0]);
        let calls = f.calls();
        let mut expected = vec![
            (f.p[1] as u32, 0x10, vec![0xDEAD_BEEF]),
            (unsafe { f.p[0].add(0x20) } as u32, 0x0042_1660, vec![]),
        ];
        if cold {
            expected.extend([
                (0, 0x0087_93EC, vec![]),
                (f.p[6] as u32, 0x14, vec![0x3AE4_BEA3, 0x3AE4_BEAB]),
                (0, 0x0041_1870, vec![f.p[5] as u32]),
            ]);
        }
        let argument = calls[expected.len() + 1].2[0];
        expected.extend([
            (f.p[5] as u32, 0x0C, vec![12]),
            (f.p[0] as u32, 0x50, vec![argument, 0]),
            (f.p[7] as u32, 0x90, vec![]),
        ]);
        if with_nodes {
            expected.extend([
                (f.p[5] as u32, 0x10, vec![f.p[12] as u32]),
                (f.p[9] as u32, 0x114, vec![unsafe { f.p[0].add(0x20) } as u32]),
                (f.p[9] as u32, 0x11C, vec![]),
                (f.p[9] as u32, 8, vec![]),
                (f.p[5] as u32, 0x10, vec![f.p[11] as u32]),
            ]);
        }
        expected.extend([
            (f.p[8] as u32, 0x114, vec![unsafe { f.p[0].add(0x20) } as u32]),
            (f.p[8] as u32, 0x11C, vec![]),
            (argument, 0x0041_1FE0, vec![f.p[10] as u32]),
        ]);
        assert_eq!(calls, expected);
    }
    f.objects.clear();
}

#[test]
fn exported_message_interface_preserves_the_calling_convention() {
    let f = Fixture::new();
    STATE.with(|s| s.borrow_mut().kind = 0x6AC2_84F3);
    let args = [f.p[1] as u32];
    let mut call = RawCall::new(
        Conv::Thiscall,
        fc4p_006d42b0 as *const () as usize,
        unsafe { f.p[0].add(4) } as u32,
        &args,
    );
    call.edx = 0xDEAD_BEEF;
    unsafe {
        call_raw(&mut call);
    }
    assert_eq!((call.eax, call.edx), (0x6AC2_8401, 0x1111_2222));
}

#[test]
fn commodity_publication_uses_the_same_reverse_drain_without_clearing_the_deals() {
    let f = Fixture::new();
    let mut cache = f.p[5];
    let mut manager = f.p[7];
    let _cache = fake_data(0x00B2_07E0, (&mut cache as *mut *mut u8).cast());
    let _manager = fake_data(0x00B4_3C94, (&mut manager as *mut *mut u8).cast());
    unsafe {
        *f.p[0].add(0x3C) = 1;
        *f.p[0].add(0x3D) = 1;
        f.p[10].add(8).cast::<u32>().write_unaligned(0xCCDD_EEFF);
        f.p[11].add(8).cast::<*mut u8>().write_unaligned(f.p[9]);
        f.p[12].add(8).cast::<u32>().write_unaligned(0);
    }
    STATE.with(|s| s.borrow_mut().nodes = vec![f.p[11] as u32, f.p[12] as u32]);
    let mut argument = u32::MAX;
    assert_eq!(
        unsafe { publish_cities(f.p[0], &mut argument, answer(), true) },
        answer()
    );
    assert_eq!(argument, f.p[10] as u32);
    let list = unsafe { f.p[0].add(0x20) } as u32;
    assert_eq!(
        f.calls(),
        vec![
            (f.p[5] as u32, 0x0C, vec![12]),
            (f.p[0] as u32, 0x50, vec![&mut argument as *mut u32 as u32, 0]),
            (f.p[7] as u32, 0x90, vec![]),
            (f.p[5] as u32, 0x10, vec![f.p[12] as u32]),
            (f.p[9] as u32, 0x114, vec![list]),
            (f.p[9] as u32, 0x11C, vec![]),
            (f.p[9] as u32, 8, vec![]),
            (f.p[5] as u32, 0x10, vec![f.p[11] as u32]),
            (f.p[8] as u32, 0x114, vec![list]),
            (f.p[8] as u32, 0x11C, vec![]),
            (&mut argument as *mut u32 as u32, 0x0041_1FE0, vec![f.p[10] as u32]),
        ]
    );
    assert_eq!(unsafe { [*f.p[0].add(0x3C), *f.p[0].add(0x3D)] }, [0, 0]);
}
