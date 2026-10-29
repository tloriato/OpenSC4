// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the Free
// Software Foundation, either version 3 of the License, or (at your option)
// any later version. This program is distributed WITHOUT ANY WARRANTY; see
// the GNU General Public License for more details. You should have received a
// copy of the license along with this program (see LICENSE at the repository
// root); if not, see <https://www.gnu.org/licenses/>.

//! Production and pole-cost cases use raw objects and ordered property/virtual fakes.
use super::*;
use crate::testing::{call_thiscall_with_eax, fake, fake_data, GameFpu, Object};
use std::cell::{Cell, RefCell};
#[derive(Debug, PartialEq)]
struct Call(u32, usize, u32);
thread_local! {
    static CALLS: RefCell<Vec<Call>> = const {RefCell::new(Vec::new())};
    static CAPACITY: Cell<*mut u32> = const {Cell::new(core::ptr::null_mut())};
}
fn log(slot: u32, this: *mut u8, arg: u32) {
    CALLS.with(|c| c.borrow_mut().push(Call(slot, this as usize, arg)));
}
fn calls() -> Vec<Call> {
    CALLS.with(|c| std::mem::take(&mut *c.borrow_mut()))
}
#[repr(C)]
struct Node {
    vt: *const usize,
    child: *mut u8,
    value: u64,
    success: u32,
    result: u32,
}
extern "thiscall" fn holder(p: *mut Node) -> *mut u8 {
    log(0x18, p.cast(), 0);
    unsafe { (*p).child }
}
extern "thiscall" fn occupant(p: *mut Node) -> *mut u8 {
    log(0x0C, p.cast(), 0);
    unsafe { (*p).child }
}
extern "C" fn uint(p: *mut Node, id: u32, out: *mut u32) -> u32 {
    log(0x005F_D480, p.cast(), id);
    CAPACITY.with(|c| c.set(out));
    unsafe {
        out.write_unaligned((*p).value as u32);
        (*p).success
    }
}
extern "C" fn int64(p: *mut Node, id: u32, out: *mut u64) -> u32 {
    log(0x005F_D4B0, p.cast(), id);
    unsafe {
        out.write_unaligned((*p).value);
        (*p).result
    }
}
extern "thiscall" fn efficiency(this: *mut u8, out: *mut f32, plant: *mut u8) -> *mut f32 {
    log(0xB4, this, plant as u32);
    unsafe {
        let replacement = this.add(0x48).cast::<u32>().read_unaligned();
        if replacement != 0 {
            CAPACITY.with(|c| c.get().write_unaligned(replacement));
        }
        let value = this.add(0x40).cast::<f32>().read_unaligned();
        out.write_unaligned(value);
        let alternate = this.add(0x44).cast::<*mut f32>().read_unaligned();
        if alternate.is_null() {
            out
        } else {
            alternate
        }
    }
}
#[test]
fn production_handles_null_missing_and_unsigned_capacity_with_original_call_order() {
    let _fpu = GameFpu::new();
    let _read = fake(0x005F_D480, uint as *const () as usize);
    let mut object = Object::new(0x400);
    let mut vt = [0usize; 0xB8 / 4];
    vt[0xB4 / 4] = efficiency as *const () as usize;
    unsafe {
        object.ptr().cast::<*const usize>().write(vt.as_ptr());
    }
    let mut pvt = [0usize; 7];
    pvt[6] = holder as *const () as usize;
    let mut props = Node {
        vt: core::ptr::null(),
        child: core::ptr::null_mut(),
        value: 0,
        success: 1,
        result: 0,
    };
    let props_ptr = (&mut props as *mut Node).cast();
    let mut plant = Node {
        vt: pvt.as_ptr(),
        child: props_ptr,
        value: 0,
        success: 0,
        result: 0,
    };
    let p = (&mut plant as *mut Node).cast();
    assert_eq!(unsafe { fc4p_006eb1c0(object.ptr(), core::ptr::null_mut()) }, 0);
    assert!(calls().is_empty());
    for (capacity, factor, success, expected) in [
        (100u32, 0.5f32, 1, 50u32),
        (100, 0.0, 1, 0),
        (100, -0.25, 1, (-25i32) as u32),
        (u32::MAX, 1.0, 1, 0),
        (0x8000_0000, 1.0, 1, 0x8000_0000),
        (100, f32::NAN, 1, 0),
        (100, f32::INFINITY, 1, 0),
        (42, 0.5, 0, 42),
        (43, 0.5, 0x100, 43),
    ] {
        unsafe {
            (*props_ptr.cast::<Node>()).value = u64::from(capacity);
            (*props_ptr.cast::<Node>()).success = success;
        }
        unsafe {
            object.ptr().add(0x40).cast::<f32>().write_unaligned(factor);
        }
        assert_eq!(unsafe { fc4p_006eb1c0(object.ptr(), p) }, expected);
        let mut expected_calls = vec![
            Call(0x18, p as usize, 0),
            Call(0x005F_D480, props_ptr as usize, 0x2781_2852),
        ];
        if success as u8 != 0 {
            expected_calls.push(Call(0xB4, object.ptr() as usize, p as u32));
        }
        assert_eq!(calls(), expected_calls);
    }
}
#[test]
fn production_reloads_capacity_and_uses_the_pointer_returned_by_efficiency() {
    let _fpu = GameFpu::new();
    let _read = fake(0x005F_D480, uint as *const () as usize);
    let mut object = Object::new(0x400);
    let mut vt = [0usize; 0xB8 / 4];
    vt[0xB4 / 4] = efficiency as *const () as usize;
    let mut pvt = [0usize; 7];
    pvt[6] = holder as *const () as usize;
    let mut props = Node {
        vt: core::ptr::null(),
        child: core::ptr::null_mut(),
        value: 100,
        success: 1,
        result: 0,
    };
    let mut plant = Node {
        vt: pvt.as_ptr(),
        child: (&mut props as *mut Node).cast(),
        value: 0,
        success: 0,
        result: 0,
    };
    let mut alternate = 0.25f32;
    unsafe {
        object.ptr().cast::<*const usize>().write(vt.as_ptr());
        object.ptr().add(0x40).cast::<f32>().write_unaligned(0.5);
        object
            .ptr()
            .add(0x44)
            .cast::<*mut f32>()
            .write_unaligned(&mut alternate);
        object.ptr().add(0x48).cast::<u32>().write_unaligned(80);
    }
    assert_eq!(
        unsafe { fc4p_006eb1c0(object.ptr(), (&mut plant as *mut Node).cast()) },
        20
    );
    assert_eq!(calls().len(), 3);
}
#[test]
fn infrastructure_costs_sum_low_words_wrap_clear_high_word_and_keep_last_result_bytes() {
    let _read = fake(0x005F_D4B0, int64 as *const () as usize);
    let mut object = Object::new(0x400);
    let sim = unsafe { PowerSim::from_ptr(object.ptr()) };
    let mut vt = [0usize; 7];
    vt[3] = occupant as *const () as usize;
    vt[6] = holder as *const () as usize;
    let mut h1 = Node {
        vt: vt.as_ptr(),
        child: core::ptr::null_mut(),
        value: 0xAAAA_AAAA_FFFF_FFFF,
        success: 0,
        result: 0x1122_3344,
    };
    let mut h2 = Node {
        vt: vt.as_ptr(),
        child: core::ptr::null_mut(),
        value: 0xBBBB_BBBB_0000_0003,
        success: 0,
        result: 0xAABB_CCDD,
    };
    let (ph1, ph2) = ((&mut h1 as *mut Node).cast(), (&mut h2 as *mut Node).cast());
    let mut o1 = Node {
        vt: vt.as_ptr(),
        child: ph1,
        value: 0,
        success: 0,
        result: 0,
    };
    let mut o2 = Node {
        vt: vt.as_ptr(),
        child: ph2,
        value: 0,
        success: 0,
        result: 0,
    };
    let (po1, po2) = ((&mut o1 as *mut Node).cast(), (&mut o2 as *mut Node).cast());
    let mut p1 = Node {
        vt: vt.as_ptr(),
        child: po1,
        value: 0,
        success: 0,
        result: 0,
    };
    let mut p2 = Node {
        vt: vt.as_ptr(),
        child: po2,
        value: 0,
        success: 0,
        result: 0,
    };
    let (pp1, pp2) = ((&mut p1 as *mut Node).cast(), (&mut p2 as *mut Node).cast());
    let mut poles = [pp1, pp2];
    sim.set_poles_begin(poles.as_mut_ptr());
    sim.set_poles_end(unsafe { poles.as_mut_ptr().add(2) });
    for (old, result) in [(0u64, 0xAABB_CC01u32), (2, 0), (0x1234_5600_0000_0002, 0x1234_5601)] {
        sim.set_infrastructure_cost(old);
        let before = unsafe { std::slice::from_raw_parts(object.ptr(), 0x400) }.to_vec();
        let (eax, _) =
            unsafe { call_thiscall_with_eax(fc4p_006eb490 as *const () as usize, object.ptr(), &[], 0x9988_7766) };
        assert_eq!(eax, result);
        assert_eq!(sim.infrastructure_cost(), 2);
        let mut expected = before;
        expected[0x78..0x80].copy_from_slice(&2u64.to_le_bytes());
        assert_eq!(unsafe { std::slice::from_raw_parts(object.ptr(), 0x400) }, expected);
        assert_eq!(
            calls(),
            [
                Call(0x0C, pp1 as usize, 0),
                Call(0x18, po1 as usize, 0),
                Call(0x005F_D4B0, ph1 as usize, 0x0913_2DE8),
                Call(0x0C, pp2 as usize, 0),
                Call(0x18, po2 as usize, 0),
                Call(0x005F_D4B0, ph2 as usize, 0x0913_2DE8)
            ]
        );
    }
    sim.set_poles_end(poles.as_mut_ptr());
    sim.set_infrastructure_cost(3);
    assert_eq!(
        unsafe { call_thiscall_with_eax(fc4p_006eb490 as *const () as usize, object.ptr(), &[], 0x9988_7766) }.0,
        0x9988_7701
    );
    assert_eq!(sim.infrastructure_cost(), 0);
    assert!(calls().is_empty());
    assert_eq!(
        unsafe { call_thiscall_with_eax(fc4p_006eb490 as *const () as usize, object.ptr(), &[], 0x9988_7766) }.0,
        0
    );
}

fn call_two_words(entry: *const (), this: *mut u8, a: u32, b: u32) -> (u32, u32, u32, u32) {
    let mut f = [entry as u32, this as u32, a, b, 0, 0, 0, 0];
    unsafe {
        core::arch::asm!(
            "push dword ptr [edi + 12]","push dword ptr [edi + 8]","mov ecx, [edi + 4]","mov edx, 0x12345678","call dword ptr [edi]",
            "mov [edi + 16], eax","mov [edi + 20], edx","mov ecx, [esp - 4]","mov [edi + 24], ecx","mov ecx, [esp - 8]","mov [edi + 28], ecx",
            in("edi") f.as_mut_ptr(),clobber_abi("C"),
        );
    }
    (f[4], f[5], f[6], f[7])
}
fn call_two(entry: *const (), this: *mut u8, a: u32, b: u32) -> (u32, u32, u32) {
    let r = call_two_words(entry, this, a, b);
    (r.0, r.1, r.2)
}
#[repr(C)]
struct Budget {
    vt: *const usize,
    funding: f32,
    success: u32,
}
extern "thiscall" fn budget_query(this: *mut Budget, holder: *mut u8, id: u32, record: *mut u32) -> u32 {
    log(0xCC, this.cast(), id);
    log(0xCCD0, holder, 0);
    unsafe {
        assert_eq!(record.add(7).read(), 0);
        record.add(7).write((*this).funding.to_bits());
        (*this).success
    }
}
extern "thiscall" fn plant_condition(this: *mut u8, out: *mut f32, plant: *mut u8) -> *mut f32 {
    log(0xB8, this, plant as u32);
    unsafe {
        out.write_unaligned(0.5);
        let alternative = this.add(0x44).cast::<*mut f32>().read_unaligned();
        if alternative.is_null() {
            out
        } else {
            alternative
        }
    }
}
extern "thiscall" fn curve(this: *mut u8, x: f32) -> f64 {
    log(0x005F_4250, this, x.to_bits());
    60.000_001
}
#[test]
fn efficiency_uses_budget_curve_then_condition_and_the_actual_argument_scratch_word() {
    let _fpu = GameFpu::new();
    let _curve = fake(0x005F_4250, curve as *const () as usize);
    let mut zero = 0u32;
    let _zero = fake_data(0x00B4_A5B0, (&mut zero as *mut u32).cast());
    let mut bvt = [0usize; 0xD0 / 4];
    bvt[0xCC / 4] = budget_query as *const () as usize;
    let mut budget = Budget {
        vt: bvt.as_ptr(),
        funding: 0.9,
        success: 1,
    };
    let mut bp = (&mut budget as *mut Budget).cast::<u8>();
    let _budget = fake_data(0x00B4_3D64, (&mut bp as *mut *mut u8).cast());
    let mut object = Object::new(0x400);
    let mut vt = [0usize; 0xBC / 4];
    vt[0xB8 / 4] = plant_condition as *const () as usize;
    unsafe {
        object.ptr().cast::<*const usize>().write(vt.as_ptr());
    }
    let mut pvt = [0usize; 7];
    pvt[6] = holder as *const () as usize;
    let h = 0x1234usize as *mut u8;
    let mut plant = Node {
        vt: pvt.as_ptr(),
        child: h,
        value: 0,
        success: 0,
        result: 0,
    };
    let pp = (&mut plant as *mut Node).cast::<u8>();
    let mut out = 0u32;
    let op = &mut out as *mut u32;
    let result = call_two(fc4p_006eb5f0 as *const (), object.ptr(), op as u32, pp as u32);
    let expected = multiply_float(F80::from_f64(60.000_001), 0.01).to_f32();
    let expected = multiply_float(F80::from_f32(expected), 0.5).to_f32().to_bits();
    assert_eq!(out, expected);
    assert_eq!(result, (op as u32, expected, 0.5f32.to_bits()));
    assert_eq!(
        calls(),
        [
            Call(0x18, pp as usize, 0),
            Call(0xCC, bp as usize, 0xCA58_E540),
            Call(0xCCD0, h as usize, 0),
            Call(
                0x005F_4250,
                unsafe { object.ptr().add(0x378) } as usize,
                90.0f32.to_bits()
            ),
            Call(0xB8, object.ptr() as usize, pp as u32)
        ]
    );
    // A nonzero high byte of the budget result still means failure when its low byte is zero.
    unsafe {
        (*bp.cast::<Budget>()).success = 0x100;
    }
    let mut alternate = 0.25f32;
    unsafe {
        object
            .ptr()
            .add(0x44)
            .cast::<*mut f32>()
            .write_unaligned(&mut alternate);
    }
    let result = call_two(fc4p_006eb5f0 as *const (), object.ptr(), op as u32, pp as u32);
    assert_eq!(out, 0);
    assert_eq!(result, (op as u32, 0, 0.5f32.to_bits()));
    assert_eq!(
        calls().iter().map(|c| c.0).collect::<Vec<_>>(),
        [0x18, 0xCC, 0xCCD0, 0xB8]
    );
}
#[test]
fn null_efficiency_copies_raw_default_bits_and_keeps_edx_without_calls() {
    let mut zero = 0x7F80_1234u32;
    let _zero = fake_data(0x00B4_A5B0, (&mut zero as *mut u32).cast());
    let mut out = 0u32;
    let op = &mut out as *mut u32;
    let r = call_two(fc4p_006eb5f0 as *const (), core::ptr::null_mut(), op as u32, 0);
    assert_eq!((r, out), ((op as u32, 0x1234_5678, 0), zero));
    assert!(calls().is_empty());
}
extern "C" fn fixed_production(this: *mut u8, plant: *mut Node) -> u32 {
    // This is a thiscall fake; the wrapper below supplies the appropriate convention.
    log(0x006E_B1C0, this, plant as u32);
    unsafe { (*plant).value as u32 }
}
extern "thiscall" fn produce(this: *mut u8, plant: *mut Node) -> u32 {
    fixed_production(this, plant)
}
extern "C" fn plant_type(holder: *mut Node, id: u32, out: *mut i32) -> u32 {
    log(0x005F_D450, holder.cast(), id);
    unsafe {
        assert_eq!(out.read(), 0);
        out.write((*holder).value as i32);
    }
    0
}
extern "C" fn used(holder: *mut Node, id: u32, value: i32) -> u32 {
    log(0x005F_E330, holder.cast(), id);
    log(0xE330, holder.cast(), value as u32);
    0xABCD_EF01
}
#[test]
fn visited_plants_record_signed_usage_in_ring_and_property_and_skip_other_states() {
    let _fpu = GameFpu::new();
    let _production = fake(0x006E_B1C0, produce as *const () as usize);
    let _kind = fake(0x005F_D450, plant_type as *const () as usize);
    let _used = fake(0x005F_E330, used as *const () as usize);
    let mut object = Object::new(0x400);
    let sim = unsafe { PowerSim::from_ptr(object.ptr()) };
    sim.set_production(100);
    sim.set_ring_index(2);
    let mut pvt = [0usize; 7];
    pvt[6] = holder as *const () as usize;
    let mut props = Node {
        vt: core::ptr::null(),
        child: core::ptr::null_mut(),
        value: 3,
        success: 0,
        result: 0,
    };
    let hp = (&mut props as *mut Node).cast();
    let mut plant = Node {
        vt: pvt.as_ptr(),
        child: hp,
        value: 101,
        success: 0,
        result: 0,
    };
    let pp = (&mut plant as *mut Node).cast();
    let mut header = Object::new(16);
    let mut one = Object::new(16);
    let mut skipped = Object::new(16);
    let mut two = Object::new(16);
    let h = unsafe { PlantNode::from_ptr(header.ptr()) };
    let n1 = unsafe { PlantNode::from_ptr(one.ptr()) };
    let ns = unsafe { PlantNode::from_ptr(skipped.ptr()) };
    let n2 = unsafe { PlantNode::from_ptr(two.ptr()) };
    h.set_next(n1.as_ptr());
    n1.set_next(ns.as_ptr());
    ns.set_next(n2.as_ptr());
    n2.set_next(h.as_ptr());
    for n in [n1, n2] {
        n.set_visited(1);
        n.set_occupant(pp);
    }
    ns.set_visited(2);
    ns.set_occupant(core::ptr::null_mut());
    sim.set_plant_header(h.as_ptr());
    let target = unsafe { object.ptr().add(0x90 + 23 * 4).cast::<i32>() };
    unsafe {
        target.write(i32::MAX - 10);
    }
    let before = unsafe { std::slice::from_raw_parts(object.ptr(), 0x400) }.to_vec();
    let (eax, _, slot, ratio) = call_two_words(fc4p_006ebc10 as *const (), object.ptr(), 1, 2);
    assert_eq!((eax, slot, ratio), (h.as_ptr() as u32, 2, 0.5f32.to_bits())); // The denominator argument is untouched.
    assert_eq!((n1.visited(), ns.visited(), n2.visited()), (2, 2, 2));
    assert_eq!(unsafe { target.read() }, (i32::MAX - 10).wrapping_add(100));
    let mut expected = before;
    expected[0x90 + 23 * 4..0x94 + 23 * 4].copy_from_slice(&(i32::MAX - 10).wrapping_add(100).to_le_bytes());
    assert_eq!(unsafe { std::slice::from_raw_parts(object.ptr(), 0x400) }, expected);
    let mut expected_calls = Vec::new();
    for _ in 0..2 {
        expected_calls.extend([
            Call(0x18, pp as usize, 0),
            Call(0x006E_B1C0, object.ptr() as usize, pp as u32),
            Call(0x005F_D450, hp as usize, 0x2781_2853),
            Call(0x005F_E330, hp as usize, 0x08D9_763D),
            Call(0xE330, hp as usize, 50),
        ]);
    }
    assert_eq!(calls(), expected_calls);
}

#[test]
fn visited_ratio_uses_unsigned_inputs_stores_to_argument_and_handles_inactive_or_empty_list() {
    let _fpu = GameFpu::new();
    let mut object = Object::new(0x400);
    let sim = unsafe { PowerSim::from_ptr(object.ptr()) };
    let mut header = Object::new(16);
    let h = unsafe { PlantNode::from_ptr(header.ptr()) };
    h.set_next(h.as_ptr());
    sim.set_plant_header(h.as_ptr());
    for (active, numerator, denominator, ratio) in [
        (1, 1, 2, 0.5f32.to_bits()),
        (0, 1, 0, 0),
        (-1, u32::MAX, 0, 0),
        (1, u32::MAX, 0x8000_0000, 2.0f32.to_bits()),
        (1, 1, 0, f32::INFINITY.to_bits()),
    ] {
        sim.set_production(active);
        let before = unsafe { std::slice::from_raw_parts(object.ptr(), 0x400) }.to_vec();
        let r = call_two_words(fc4p_006ebc10 as *const (), object.ptr(), numerator, denominator);
        assert_eq!((r.0, r.2, r.3), (h.as_ptr() as u32, denominator, ratio));
        assert_eq!(unsafe { std::slice::from_raw_parts(object.ptr(), 0x400) }, before);
        assert!(calls().is_empty());
    }
}
#[test]
fn layout_offsets_match_primary_power_object_and_plant_nodes() {
    assert_eq!(
        PowerSim::FIELDS,
        &[
            ("primary_vtable", 0, 4),
            ("references", 0x10, 4),
            ("city", 0x18, 4),
            ("budget", 0x1C, 4),
            ("department", 0x20, 4),
            ("zones", 0x2C, 4),
            ("ring_index", 0x38, 4),
            ("production", 0x3C, 4),
            ("consumption", 0x40, 4),
            ("monthly_demand", 0x60, 4),
            ("deficit", 0x64, 4),
            ("infrastructure_cost", 0x78, 8),
            ("powered_grid", 0x80, 4),
            ("usage_ring", 0x90, 480),
            ("annual_usage", 0x270, 40),
            ("plant_header", 0x298, 4),
            ("poles_begin", 0x29C, 4),
            ("poles_end", 0x2A0, 4),
            ("ordinance_factor", 0x2AC, 4),
            ("radius", 0x2B0, 4),
            ("pole_count", 0x2B8, 4),
            ("broken_count", 0x2BC, 4),
            ("broken_fraction", 0x2C0, 4),
            ("wire", 0x2C8, 4),
            ("powered", 0x2D4, 4),
            ("plant_count", 0x2DC, 4),
            ("plant_counts", 0x2E0, 40),
            ("overworked_count", 0x308, 4),
            ("aged_count", 0x30C, 4),
            ("distressed_count", 0x310, 4),
            ("recompute", 0x338, 1),
            ("age_plants", 0x339, 1),
            ("funding_curve", 0x378, 16),
        ]
    );
    assert_eq!(
        PlantNode::FIELDS,
        &[("next", 0, 4), ("visited", 8, 4), ("occupant", 12, 4)]
    );
}
fn environment() -> (u16, u16, u32, u32) {
    let (mut cw, mut mxcsr) = (0u16, 0u32);
    let (sw, flags): (u16, u32);
    unsafe {
        core::arch::asm!("fnstcw word ptr [{cw}]","fnstsw ax","stmxcsr dword ptr [{mxcsr}]","pushfd","pop {flags:e}",cw=in(reg)&mut cw,mxcsr=in(reg)&mut mxcsr,flags=lateout(reg)flags,out("ax")sw);
    }
    (cw, sw & 0x3800, mxcsr, flags & 0x400)
}
#[test]
fn plant_math_keeps_an_existing_x87_value_control_word_mxcsr_and_direction_flag() {
    let _fpu = GameFpu::new();
    let _curve = fake(0x005F_4250, curve as *const () as usize);
    let _read = fake(0x005F_D480, uint as *const () as usize);
    let mut zero = 0u32;
    let _zero = fake_data(0x00B4_A5B0, (&mut zero as *mut u32).cast());
    let mut bvt = [0usize; 0xD0 / 4];
    bvt[0xCC / 4] = budget_query as *const () as usize;
    let mut budget = Budget {
        vt: bvt.as_ptr(),
        funding: 1.0,
        success: 1,
    };
    let mut bp = (&mut budget as *mut Budget).cast::<u8>();
    let _budget = fake_data(0x00B4_3D64, (&mut bp as *mut *mut u8).cast());
    let mut object = Object::new(0x400);
    let mut vt = [0usize; 0xBC / 4];
    vt[0xB8 / 4] = plant_condition as *const () as usize;
    vt[0xB4 / 4] = efficiency as *const () as usize;
    unsafe {
        object.ptr().cast::<*const usize>().write(vt.as_ptr());
        object.ptr().add(0x40).cast::<f32>().write(0.5);
    }
    let mut pvt = [0usize; 7];
    pvt[6] = holder as *const () as usize;
    let mut props = Node {
        vt: core::ptr::null(),
        child: core::ptr::null_mut(),
        value: 100,
        success: 1,
        result: 0,
    };
    let mut plant = Node {
        vt: pvt.as_ptr(),
        child: (&mut props as *mut Node).cast(),
        value: 0,
        success: 0,
        result: 0,
    };
    let pp = (&mut plant as *mut Node).cast::<u8>();
    let sentinel = 123.25f64;
    let mut restored = 0.0f64;
    unsafe {
        core::arch::asm!("fld qword ptr [{p}]",p=in(reg)&sentinel,out("st(0)")_);
    }
    let before = environment();
    let produced = unsafe { fc4p_006eb1c0(object.ptr(), pp) };
    let after_produced = environment();
    let mut out = 0u32;
    let result = call_two(
        fc4p_006eb5f0 as *const (),
        object.ptr(),
        &mut out as *mut u32 as u32,
        pp as u32,
    );
    let after_efficiency = environment();
    unsafe {
        core::arch::asm!("fstp qword ptr [{p}]",p=in(reg)&mut restored,out("st(0)")_);
    }
    assert_eq!(produced, 50);
    assert_eq!(result.1, out);
    assert_eq!(after_produced, before);
    assert_eq!(after_efficiency, before);
    assert_eq!(restored, sentinel);
    calls();
}

thread_local! {static ALLOCATED:Cell<bool>=const {Cell::new(false)};}
extern "thiscall" fn condition_property(this: *mut Node, id: u32) -> u64 {
    log(0x24, this.cast(), id);
    (0xEEEE_FFFFu64 << 32) | u64::from(unsafe { (*this).child as u32 })
}
extern "thiscall" fn curve_ctor(this: *mut u32) -> u32 {
    log(0x007D_0410, core::ptr::null_mut(), 0);
    for i in 0..4 {
        unsafe {
            this.add(i).write(0);
        }
    }
    this as u32
}
extern "thiscall" fn curve_set_fake(this: *mut u32, property: *mut u8) -> u32 {
    log(0x005F_4620, property, 0);
    unsafe {
        for i in 0..4 {
            assert_eq!(this.add(i).read(), 0);
        }
        this.write(if ALLOCATED.with(|a| a.get()) { 0x8765 } else { 0 });
        this.add(2).cast::<f64>().write_unaligned(60.000_001);
    }
    0x1234_5678
}
extern "C" fn wear_property(this: *mut u8, id: u32, out: *mut u32) -> u64 {
    log(0x005F_D4F0, this, id);
    unsafe {
        assert_eq!(out.read(), 0);
        out.write(0.25f32.to_bits());
    }
    0x1234_5678_0000_0000
}
extern "thiscall" fn condition_curve_body(this: *mut u32, x: f32) -> f64 {
    log(0x005F_4250, core::ptr::null_mut(), x.to_bits());
    unsafe { this.add(2).cast::<f64>().read_unaligned() }
}
#[unsafe(naked)]
unsafe extern "thiscall" fn condition_curve(this: *mut u8, x: f32) -> f64 {
    core::arch::naked_asm!("push dword ptr [esp + 4]","call {body}","mov edx, 0xAABBCCDD","ret 4",body=sym condition_curve_body);
}
extern "C" fn curve_free(storage: *mut u8) -> u32 {
    log(0x0090_CF63, storage, 0);
    0xFFFF_FFFF
}
#[test]
fn condition_handles_null_and_missing_curve_without_touching_the_plant_argument() {
    let mut zero = 0x7F80_1234u32;
    let _zero = fake_data(0x00B4_A5B0, (&mut zero as *mut u32).cast());
    let mut out = [0xAAAA_AAAAu32, 0, 0xBBBB_BBBB];
    let op = &mut out[1] as *mut u32;
    let result = call_two_words(fc4p_006ec820 as *const (), core::ptr::null_mut(), op as u32, 0);
    assert_eq!(result, (op as u32, 0x1234_5678, 0, op as u32));
    assert_eq!(out, [0xAAAA_AAAA, zero, 0xBBBB_BBBB]);
    assert!(calls().is_empty());
    let mut vt = [0usize; 10];
    vt[6] = holder as *const () as usize;
    vt[9] = condition_property as *const () as usize;
    let mut props = Node {
        vt: vt.as_ptr(),
        child: core::ptr::null_mut(),
        value: 0,
        success: 0,
        result: 0,
    };
    let hp = (&mut props as *mut Node).cast();
    let mut plant = Node {
        vt: vt.as_ptr(),
        child: hp,
        value: 0,
        success: 0,
        result: 0,
    };
    let pp = (&mut plant as *mut Node).cast::<u8>();
    let result = call_two_words(fc4p_006ec820 as *const (), core::ptr::null_mut(), op as u32, pp as u32);
    assert_eq!(result, (op as u32, 0xEEEE_FFFF, pp as u32, op as u32));
    assert_eq!(out[1], zero);
    assert_eq!(
        calls(),
        [Call(0x18, pp as usize, 0), Call(0x24, hp as usize, 0x8A1E_07EE)]
    );
}
#[test]
fn condition_constructs_sets_evaluates_and_frees_curve_in_order_with_original_return_registers() {
    let _fpu = GameFpu::new();
    let mut zero = 0u32;
    let _zero = fake_data(0x00B4_A5B0, (&mut zero as *mut u32).cast());
    let _new = fake(0x007D_0410, curve_ctor as *const () as usize);
    let _set = fake(0x005F_4620, curve_set_fake as *const () as usize);
    let _wear = fake(0x005F_D4F0, wear_property as *const () as usize);
    let _curve = fake(0x005F_4250, condition_curve as *const () as usize);
    let _free = fake(0x0090_CF63, curve_free as *const () as usize);
    let mut vt = [0usize; 10];
    vt[6] = holder as *const () as usize;
    vt[9] = condition_property as *const () as usize;
    let property = 0x5678usize as *mut u8;
    let mut props = Node {
        vt: vt.as_ptr(),
        child: property,
        value: 0,
        success: 0,
        result: 0,
    };
    let hp = (&mut props as *mut Node).cast();
    let mut plant = Node {
        vt: vt.as_ptr(),
        child: hp,
        value: 0,
        success: 0,
        result: 0,
    };
    let pp = (&mut plant as *mut Node).cast::<u8>();
    for allocated in [false, true] {
        ALLOCATED.with(|a| a.set(allocated));
        let mut out = 0u32;
        let op = &mut out as *mut u32;
        let before = environment();
        let result = call_two_words(fc4p_006ec820 as *const (), core::ptr::null_mut(), op as u32, pp as u32);
        assert_eq!(environment(), before);
        let expected = F80::from_f64(60.000_001).to_f32().to_bits();
        assert_eq!(out, expected);
        assert_eq!(
            result,
            (
                op as u32,
                if allocated { expected } else { 0xAABB_CCDD },
                0.25f32.to_bits(),
                op as u32
            )
        );
        let mut expected_calls = vec![
            Call(0x18, pp as usize, 0),
            Call(0x24, hp as usize, 0x8A1E_07EE),
            Call(0x007D_0410, 0, 0),
            Call(0x005F_4620, property as usize, 0),
            Call(0x005F_D4F0, hp as usize, 0xC910_B4CF),
            Call(0x005F_4250, 0, 0.25f32.to_bits()),
        ];
        if allocated {
            expected_calls.push(Call(0x0090_CF63, 0x8765, 0));
        }
        assert_eq!(calls(), expected_calls);
    }
}
