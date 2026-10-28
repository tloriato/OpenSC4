// SPDX-License-Identifier: GPL-3.0-or-later
// FreeCity4 - Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, fake_data, Fake, GameFpu, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq)]
struct Call(&'static str, usize, Vec<u32>);
thread_local! {
    static CALLS: RefCell<Vec<Call>> = const { RefCell::new(Vec::new()) };
}
fn log(name: &'static str, this: *mut u8, args: &[u32]) {
    CALLS.with(|c| c.borrow_mut().push(Call(name, this as usize, args.to_vec())));
}
fn calls() -> Vec<Call> {
    CALLS.with(|c| std::mem::take(&mut *c.borrow_mut()))
}
unsafe fn word(p: *mut u8, offset: usize) -> u32 {
    unsafe { p.add(offset).cast::<u32>().read_unaligned() }
}
unsafe fn put(p: *mut u8, offset: usize, value: u32) {
    unsafe { p.add(offset).cast::<u32>().write_unaligned(value) }
}
extern "thiscall" fn kind(p: *mut u8) -> u32 {
    log("kind", p, &[]);
    unsafe { word(p, 4) }
}
extern "thiscall" fn holder(p: *mut u8) -> *mut u8 {
    log("holder", p, &[]);
    unsafe { word(p, 8) as *mut u8 }
}
extern "thiscall" fn query(p: *mut u8, iid: u32, out: *mut u32) -> u32 {
    unsafe {
        log("query", p, &[iid, out.read()]);
        if word(p, 0x18) != 0 {
            out.write(word(p, 0x10));
        }
        word(p, 0x14)
    }
}
extern "thiscall" fn predicate(p: *mut u8) -> u32 {
    log("predicate", p, &[]);
    unsafe { word(p, 4) }
}
extern "thiscall" fn flag(p: *mut u8, mask: u32) -> u32 {
    log("flag", p, &[mask]);
    unsafe {
        if mask == 0x8000_0000 {
            word(p, 4)
        } else {
            word(p, 8)
        }
    }
}
extern "thiscall" fn cell(p: *mut u8, x: *mut u32, z: *mut u32) -> u32 {
    log("cell", p, &[]);
    unsafe {
        x.write(word(p, 0x10));
        z.write(word(p, 0x14));
    }
    0x1234_5678
}
extern "thiscall" fn release(p: *mut u8) -> u32 {
    log("release", p, &[]);
    unsafe { word(p, 0xC) }
}
extern "thiscall" fn bbox(p: *mut u8, low: *mut f32, high: *mut f32) -> u32 {
    log("bbox", p, &[]);
    unsafe {
        for i in 0..3 {
            low.add(i).write(f32::from_bits(word(p, 0x20 + i * 4)));
            high.add(i).write(f32::from_bits(word(p, 0x2C + i * 4)));
        }
    }
    0
}
extern "thiscall" fn footprint(p: *mut u8, out: *mut Rect) -> u32 {
    log("footprint", p, &[]);
    unsafe {
        out.write(Rect {
            x0: 2,
            z0: 3,
            x1: 4,
            z1: 5,
        });
    }
    0
}
#[unsafe(naked)]
unsafe extern "thiscall" fn width() {
    core::arch::naked_asm!("push ecx", "call {log}", "add esp, 4", "fld dword ptr [ecx + 4]", "ret", log = sym width_log)
}
// The logger returns the city pointer in ECX for the ST0 fake above.
#[unsafe(naked)]
unsafe extern "C" fn width_log() {
    core::arch::naked_asm!("push dword ptr [esp + 4]", "call {body}", "add esp, 4", "mov ecx, [esp + 4]", "ret", body = sym log_width)
}
extern "C" fn log_width(p: *mut u8) {
    log("width", p, &[]);
}
#[unsafe(naked)]
unsafe extern "thiscall" fn depth() {
    core::arch::naked_asm!("push ecx", "call {log}", "add esp, 4", "fld dword ptr [ecx + 8]", "ret", log = sym depth_log)
}
#[unsafe(naked)]
unsafe extern "C" fn depth_log() {
    core::arch::naked_asm!("push dword ptr [esp + 4]", "call {body}", "add esp, 4", "mov ecx, [esp + 4]", "ret", body = sym log_depth)
}
extern "C" fn log_depth(p: *mut u8) {
    log("depth", p, &[]);
}
extern "thiscall" fn expand_fake(p: *mut u8, rect: *mut Rect) -> u32 {
    unsafe {
        let r = &mut *rect;
        log("expand", p, &[r.x0, r.z0, r.x1, r.z1]);
        r.x0 = r.x0.wrapping_sub(1);
        r.z0 = r.z0.wrapping_sub(1);
        r.x1 = r.x1.wrapping_add(1);
        r.z1 = r.z1.wrapping_add(1);
    }
    0x1111_2222
}
extern "thiscall" fn invalidate_fake(p: *mut u8, rect: *mut Rect) -> u32 {
    unsafe {
        let r = &*rect;
        log("invalidate", p, &[r.x0, r.z0, r.x1, r.z1]);
    }
    0x3333_4444
}
extern "C" fn uint(p: *mut u8, id: u32, out: *mut u32) -> u32 {
    log("uint", p, &[id]);
    unsafe {
        out.write(word(p, if id == 0x2781_2854 { 4 } else { 8 }));
    }
    0x100
}
extern "C" fn int(p: *mut u8, id: u32, out: *mut i32) -> u32 {
    log("int", p, &[id]);
    unsafe {
        out.write(word(p, 0xC) as i32);
    }
    1
}
extern "C" fn int64(p: *mut u8, id: u32, out: *mut u64) -> u32 {
    log("int64", p, &[id]);
    unsafe {
        out.write(p.add(0x10).cast::<u64>().read_unaligned());
    }
    1
}
extern "C" fn move_fake(dst: *mut u8, src: *mut u8, count: u32) -> u32 {
    log("move", dst, &[src as u32, count]);
    unsafe {
        core::ptr::copy(src, dst, count as usize);
    }
    dst as u32
}
extern "C" fn free_fake(p: *mut u8) -> u32 {
    log("free", p, &[]);
    0x9876_5432
}
extern "thiscall" fn budget_query(p: *mut u8, holder: *mut u8, purpose: u32, out: *mut u32) -> u32 {
    unsafe {
        log("budget_query", p, &[holder as u32, purpose, out.add(7).read()]);
        for i in 0..8 {
            out.add(i).write(word(p, 0x10 + i * 4));
        }
        word(p, 4)
    }
}
extern "thiscall" fn department(p: *mut u8, id: u32) -> *mut u8 {
    log("department", p, &[id]);
    unsafe { word(p, 8) as *mut u8 }
}
extern "thiscall" fn item(p: *mut u8, id: u32) -> *mut u8 {
    log("item", p, &[id]);
    unsafe { word(p, 8) as *mut u8 }
}
extern "thiscall" fn remove_funding(p: *mut u8, holder: *mut u8, purpose: u32) -> u32 {
    log("remove_funding", p, &[holder as u32, purpose]);
    1
}
extern "thiscall" fn count(p: *mut u8) -> u64 {
    log("count", p, &[]);
    unsafe { p.add(0x10).cast::<u64>().read_unaligned() }
}
extern "thiscall" fn set_count(p: *mut u8, n: u64) -> u32 {
    log("set_count", p, &[n as u32, (n >> 32) as u32]);
    unsafe {
        p.add(0x10).cast::<u64>().write_unaligned(n);
    }
    0x7788_9900
}

// Return both EAX and the callee-popped argument word, before another call can reuse its storage.
#[unsafe(naked)]
unsafe extern "fastcall" fn invoke(this: *mut u8, eax: u32, occupant: *mut u8) -> u64 {
    core::arch::naked_asm!(
        "mov eax, edx", "push dword ptr [esp + 4]", "call {entry}",
        "mov edx, [esp - 4]", "ret 4", entry = sym fc4p_006ebf60,
    )
}

struct Fixture {
    sim: Object,
    occupant: Object,
    interface: Object,
    holder: Object,
    city: Object,
    budget: Object,
    department: Object,
    item: Object,
    header: Object,
    rows: Vec<[*mut i16; 3]>,
    cells: Vec<Vec<i16>>,
    _tables: Vec<Vec<usize>>,
    _fakes: Vec<Fake>,
    _half: Box<f32>,
    _budget: Box<*mut u8>,
}
impl Fixture {
    fn new(kind_value: u32) -> Self {
        calls();
        let mut f = Self {
            sim: Object::new(0x400),
            occupant: Object::new(0x50),
            interface: Object::new(0x30),
            holder: Object::new(0x30),
            city: Object::new(0x10),
            budget: Object::new(0x40),
            department: Object::new(0x20),
            item: Object::new(0x20),
            header: Object::new(16),
            rows: vec![],
            cells: (0..8).map(|_| vec![0i16; 8]).collect(),
            _tables: vec![],
            _fakes: vec![],
            _half: Box::new(0.5),
            _budget: Box::new(core::ptr::null_mut()),
        };
        f.rows = f
            .cells
            .iter_mut()
            .map(|r| [r.as_mut_ptr(), core::ptr::null_mut(), core::ptr::null_mut()])
            .collect();
        let (s, o, i, h, c, b, d, it) = (
            f.sim.ptr(),
            f.occupant.ptr(),
            f.interface.ptr(),
            f.holder.ptr(),
            f.city.ptr(),
            f.budget.ptr(),
            f.department.ptr(),
            f.item.ptr(),
        );
        f.table(
            o,
            &[
                (0, query as *const () as usize),
                (4, release as *const () as usize),
                (8, release as *const () as usize),
                (0x18, holder as *const () as usize),
                (0x1C, kind as *const () as usize),
                (0x28, bbox as *const () as usize),
                (0x2C, footprint as *const () as usize),
            ],
        );
        f.table(
            i,
            &[
                (8, release as *const () as usize),
                (0x14, cell as *const () as usize),
                (0x18, predicate as *const () as usize),
                (
                    0x20,
                    if kind_value == 0xC772_BF98 {
                        flag as *const () as usize
                    } else {
                        cell as *const () as usize
                    },
                ),
                (0x34, predicate as *const () as usize),
                (0xEC, cell as *const () as usize),
            ],
        );
        f.table(
            c,
            &[
                (0x16C, width as *const () as usize),
                (0x170, depth as *const () as usize),
            ],
        );
        f.table(
            b,
            &[
                (0xCC, budget_query as *const () as usize),
                (0x70, department as *const () as usize),
            ],
        );
        f.table(
            d,
            &[
                (0x50, item as *const () as usize),
                (0x5C, remove_funding as *const () as usize),
            ],
        );
        f.table(
            it,
            &[
                (0x20, count as *const () as usize),
                (0x24, set_count as *const () as usize),
            ],
        );
        unsafe {
            put(s, 0xC, 1);
            put(s, 0x18, c as u32);
            put(s, 0x84, f.rows.as_mut_ptr() as u32);
            put(s, 0x298, f.header.ptr() as u32);
            let header = PlantLink::from_ptr(f.header.ptr());
            header.set_next(header.as_ptr());
            header.set_previous(header.as_ptr());
            put(o, 4, kind_value);
            put(o, 8, h as u32);
            put(o, 0xC, 0xABCD_1234);
            put(o, 0x10, i as u32);
            put(o, 0x14, 0xFF01);
            put(o, 0x18, 1);
            for (off, value) in [
                (0x20, 32f32),
                (0x24, 0.0),
                (0x28, 16.0),
                (0x2C, 64.0),
                (0x30, 0.0),
                (0x34, 48.0),
            ] {
                put(o, off, value.to_bits());
            }
            put(i, 8, 1);
            put(i, 0xC, 0xF1E2_D3C4);
            put(i, 0x10, 3);
            put(i, 0x14, 4);
            put(c, 4, 16f32.to_bits());
            put(c, 8, 16f32.to_bits());
            put(b, 8, d as u32);
            put(b, 0x10, 0x123);
            put(b, 0x14, 0x456);
            put(d, 8, it as u32);
        }
        *f._budget = b;
        f._fakes = vec![
            fake(0x006E_AE20, expand_fake as *const () as usize),
            fake(0x006E_B230, invalidate_fake as *const () as usize),
            fake(0x005F_D480, uint as *const () as usize),
            fake(0x005F_D450, int as *const () as usize),
            fake(0x005F_D4B0, int64 as *const () as usize),
            fake(0x009E_EB70, move_fake as *const () as usize),
            fake(0x0090_CF63, free_fake as *const () as usize),
            fake_data(0x00A8_4D2C, (&mut *f._half as *mut f32).cast()),
            fake_data(0x00B4_3D64, (&mut *f._budget as *mut *mut u8).cast()),
        ];
        f
    }
    fn table(&mut self, p: *mut u8, slots: &[(usize, usize)]) {
        let mut table = vec![0; slots.iter().map(|s| s.0 / 4 + 1).max().unwrap()];
        for &(offset, function) in slots {
            table[offset / 4] = function;
        }
        unsafe {
            p.cast::<*const usize>().write(table.as_ptr());
        }
        self._tables.push(table);
    }
    fn run(&mut self) -> (u32, u32) {
        let r = unsafe { invoke(self.sim.ptr(), 0xFEDC_BA98, self.occupant.ptr()) };
        (r as u32, (r >> 32) as u32)
    }
    fn names(&self) -> Vec<&'static str> {
        calls().into_iter().map(|c| c.0).collect()
    }
}

#[test]
fn removal_is_ignored_until_initialized_and_preserves_upper_eax() {
    let mut f = Fixture::new(0x2781_28A0);
    unsafe {
        put(f.sim.ptr(), 0xC, 0);
    }
    assert_eq!(f.run(), (0xFEDC_BA00, f.occupant.ptr() as u32));
    assert!(calls().is_empty());
    unsafe {
        put(f.sim.ptr(), 0xC, 1);
        put(f.occupant.ptr(), 4, 0x7654_3210);
    }
    assert_eq!(f.run(), (0x7654_3210, f.occupant.ptr() as u32));
    assert_eq!(f.names(), ["kind"]);
}

#[test]
fn queries_use_the_argument_word_and_failure_keeps_the_full_result() {
    for kind in [0x2890_D4DE, 0x0990_B2EA, 0xC772_BF98] {
        let mut f = Fixture::new(kind);
        unsafe {
            put(f.occupant.ptr(), 0x14, 0xABCD_EF00);
            put(f.occupant.ptr(), 0x18, 0);
        }
        let arg = if kind == 0xC772_BF98 {
            0
        } else {
            f.occupant.ptr() as u32
        };
        assert_eq!(f.run(), (0xABCD_EF00, arg));
        let c = calls();
        assert_eq!(c.len(), 2);
        assert_eq!(c[1].2[1], arg);
    }
}

#[test]
fn flora_and_network_filters_invalidate_the_original_rectangles_then_release() {
    for kind in [0x0990_B2EA, 0xC772_BF98] {
        for (first, second, active) in [(0u32, 1u32, true), (1, 1, false), (0x100, 0x100, kind == 0x0990_B2EA)] {
            let mut f = Fixture::new(kind);
            unsafe {
                put(f.interface.ptr(), 4, first);
                put(f.interface.ptr(), 8, second);
            }
            assert_eq!(f.run(), (0xF1E2_D3C4, f.interface.ptr() as u32));
            let c = calls();
            let invalidation: Vec<_> = c.iter().filter(|c| c.0 == "invalidate").collect();
            assert_eq!(invalidation.len(), usize::from(active));
            if active {
                assert_eq!(
                    invalidation[0].2,
                    if kind == 0x0990_B2EA {
                        vec![2, 3, 4, 5]
                    } else {
                        vec![3, 4, 3, 4]
                    }
                );
            }
            assert_eq!(c.last().unwrap().0, "release");
            unsafe {
                assert_eq!(word(f.sim.ptr(), 0x338) as u8, u8::from(active));
            }
            if kind == 0xC772_BF98 {
                let flags: Vec<_> = c.iter().filter(|c| c.0 == "flag").map(|c| c.2[0]).collect();
                assert_eq!(
                    flags,
                    if first as u8 == 0 {
                        vec![0x8000_0000, 0x40000]
                    } else {
                        vec![0x8000_0000]
                    }
                );
                assert!(!c.iter().any(|c| c.0 == "expand"));
            }
        }
    }
}

#[test]
fn pole_removal_releases_shifts_and_subtracts_cost_even_without_a_list_match() {
    for index in [Some(0), Some(2), None] {
        let mut f = Fixture::new(0x2890_D4DE);
        let p = f.interface.ptr();
        let mut poles = [core::ptr::null_mut(); 3];
        if let Some(index) = index {
            poles[index] = p;
        }
        unsafe {
            let power = PowerSim::from_ptr(f.sim.ptr());
            power.set_poles_begin(poles.as_mut_ptr());
            power.set_poles_end(poles.as_mut_ptr().add(3));
            power.set_pole_count(3);
            power.set_broken_count(1);
            power.set_infrastructure_cost(0);
            f.holder.ptr().add(0x10).cast::<u64>().write_unaligned(0x1_0000_0001);
            assert_eq!(f.run(), (0xF1E2_D3C4, p as u32));
            assert_eq!(power.infrastructure_cost(), 0u64.wrapping_sub(0x1_0000_0001));
            assert_eq!(power.pole_count(), if index.is_some() { 2 } else { 3 });
            assert_eq!(power.broken_count(), 1);
            assert_eq!(
                power.poles_end(),
                poles.as_mut_ptr().add(if index.is_some() { 2 } else { 3 })
            );
        }
        let mut expected = vec!["kind", "query", "predicate", "cell", "expand", "invalidate"];
        if let Some(index) = index {
            expected.push("release");
            if index != 2 {
                expected.push("move");
            }
        }
        expected.extend(["holder", "int64", "release"]);
        assert_eq!(f.names(), expected);
        if index == Some(0) {
            assert!(poles[..2].iter().all(|p| p.is_null()));
        }
    }
    let mut f = Fixture::new(0x2890_D4DE);
    unsafe {
        put(f.interface.ptr(), 4, 1);
    }
    f.run();
    assert_eq!(f.names(), ["kind", "query", "predicate", "release"]);
}

#[test]
fn building_removal_wraps_cells_clamps_signed_totals_and_reads_current_requirements() {
    let _fpu = GameFpu::new();
    for (consumed, capacity, consumers, requirement, expected) in [
        (300, 0, 0, 100, (0, 0, -300i16)),
        (300, 0, 0x8000_0001, 0x9000_0000, (0, 0, -300)),
        (40000, 0, 2, 50000, (1, 10000, 25536)),
        (0, 0, 1, 1, (1, 1, 0)),
        (3, 3, 1, 3, (1, 3, 0)),
        (3, 4, 1, 3, (1, 3, 0)),
        (0x8000_0001, 0, 1, 0, (0, 0x7FFF_FFFF, -1)),
    ] {
        let mut f = Fixture::new(0x2781_28A0);
        unsafe {
            put(f.holder.ptr(), 4, consumed);
            put(f.holder.ptr(), 8, capacity);
            put(f.sim.ptr(), 0x68, consumers);
            put(f.sim.ptr(), 0x74, requirement);
        }
        assert_eq!(f.run(), (f.header.ptr() as u32, consumed));
        unsafe {
            assert_eq!(
                (word(f.sim.ptr(), 0x68), word(f.sim.ptr(), 0x74), f.cells[3][2]),
                expected
            );
        }
        assert_eq!(
            f.names(),
            [
                "kind",
                "bbox",
                "width",
                "depth",
                "holder",
                "uint",
                "uint",
                "footprint",
                "expand",
                "invalidate",
                "budget_query"
            ]
        );
        unsafe {
            assert_eq!(word(f.sim.ptr(), 0x338) as u8, 0);
        }
    }
}

#[test]
fn centre_cell_stores_the_half_sum_as_float32_before_dividing() {
    let _fpu = GameFpu::new();
    let mut f = Fixture::new(0x2781_28A0);
    unsafe {
        put(f.occupant.ptr(), 0x20, 0x4240_3333);
        put(f.occupant.ptr(), 0x2C, 0x429F_E666);
        put(f.holder.ptr(), 4, 59);
    }
    f.run();
    assert_eq!(f.cells[4][2], -59);
    assert_eq!(f.cells[3][2], 0);
    calls();
}

#[test]
fn plant_removal_uses_current_type_unlinks_releases_and_updates_budget_in_order() {
    let _fpu = GameFpu::new();
    let mut f = Fixture::new(0x2781_28A0);
    let mut node = Object::new(16);
    unsafe {
        let h = PlantLink::from_ptr(f.header.ptr());
        let n = PlantLink::from_ptr(node.ptr());
        h.set_next(n.as_ptr());
        h.set_previous(n.as_ptr());
        n.set_next(h.as_ptr());
        n.set_previous(h.as_ptr());
        n.set_occupant(f.occupant.ptr());
        put(f.holder.ptr(), 0xC, 3);
        put(f.sim.ptr(), 0x2DC, 1);
        put(f.sim.ptr(), 0x2E4, 1);
        put(f.budget.ptr(), 4, 1);
        f.item.ptr().add(0x10).cast::<u64>().write_unaligned(0);
        assert_eq!(f.run(), (3, 0));
        assert_eq!(h.next(), h.as_ptr());
        assert_eq!(h.previous(), h.as_ptr());
        assert_eq!(word(f.sim.ptr(), 0x2DC), 0);
        assert_eq!(word(f.sim.ptr(), 0x2E4), 1);
        assert_eq!(word(f.sim.ptr(), 0x2EC), u32::MAX);
        assert_eq!(word(f.sim.ptr(), 0x338) as u8, 1);
        assert_eq!(f.item.ptr().add(0x10).cast::<u64>().read_unaligned(), u64::MAX);
    }
    let c = calls();
    assert_eq!(
        c.iter().map(|c| c.0).collect::<Vec<_>>(),
        [
            "kind",
            "bbox",
            "width",
            "depth",
            "holder",
            "uint",
            "uint",
            "footprint",
            "expand",
            "invalidate",
            "budget_query",
            "department",
            "item",
            "remove_funding",
            "count",
            "set_count",
            "int",
            "release",
            "free"
        ]
    );
    assert_eq!(c[11].2, [0x123]);
    assert_eq!(c[12].2, [0x456]);
    assert_eq!(c[13].2, [f.holder.ptr() as u32, 0xCA58_E540]);
    assert_eq!(c[15].2, [u32::MAX, u32::MAX]);
}

#[test]
fn missing_budget_department_item_and_plant_leave_list_and_recompute_untouched() {
    let _fpu = GameFpu::new();
    for stage in 0..3 {
        let mut f = Fixture::new(0x2781_28A0);
        let mut other = Object::new(16);
        unsafe {
            let h = PlantLink::from_ptr(f.header.ptr());
            let n = PlantLink::from_ptr(other.ptr());
            h.set_next(n.as_ptr());
            h.set_previous(n.as_ptr());
            n.set_next(h.as_ptr());
            n.set_previous(h.as_ptr());
            n.set_occupant(core::ptr::null_mut());
            if stage > 0 {
                put(f.budget.ptr(), 4, 1);
            }
            if stage == 1 {
                put(f.budget.ptr(), 8, 0);
            }
            if stage == 2 {
                put(f.department.ptr(), 8, 0);
            }
            assert_eq!(f.run(), (h.as_ptr() as u32, 0));
            assert_eq!(h.next(), n.as_ptr());
            assert_eq!(word(f.sim.ptr(), 0x338) as u8, 0);
        }
        let c = calls();
        assert_eq!(c.last().unwrap().0, ["budget_query", "department", "item"][stage]);
    }
}
