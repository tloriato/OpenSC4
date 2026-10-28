// SPDX-License-Identifier: GPL-3.0-or-later
// FreeCity4 - Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, fake_data, Fake, GameFpu, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq)]
struct Call(&'static str, usize, Vec<u32>);
struct State {
    props: Vec<(u32, u32)>,
    calls: Vec<Call>,
    wear: f32,
    usage_factor: f64,
    funding_factor: f64,
    usage: u32,
    funding: u32,
    sim: usize,
    lot: usize,
    header: usize,
    clear_header: bool,
}
impl Default for State {
    fn default() -> Self {
        Self {
            props: vec![],
            calls: vec![],
            wear: 0.0,
            usage_factor: 2.0,
            funding_factor: 3.0,
            usage: 0,
            funding: 0,
            sim: 0,
            lot: 0,
            header: 0,
            clear_header: false,
        }
    }
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn log(name: &'static str, p: *mut u8, args: &[u32]) {
    STATE.with(|s| s.borrow_mut().calls.push(Call(name, p as usize, args.to_vec())));
}
fn property(id: u32, out: *mut u32) -> u32 {
    STATE.with(|s| {
        let s = s.borrow();
        if let Some(&(_, value)) = s.props.iter().find(|&&(p, _)| p == id) {
            unsafe {
                out.write_unaligned(value);
            }
            1
        } else {
            0
        }
    })
}
extern "C" fn get_float_fake(p: *mut u8, id: u32, out: *mut f32) -> u32 {
    log("float", p, &[id]);
    property(id, out.cast())
}
extern "C" fn get_int_fake(p: *mut u8, id: u32, out: *mut i32) -> u32 {
    log("int", p, &[id]);
    property(id, out.cast())
}
extern "C" fn get_uint_fake(p: *mut u8, id: u32, out: *mut u32) -> u32 {
    log("uint", p, &[id]);
    property(id, out)
}
extern "C" fn set_float_fake(p: *mut u8, id: u32, value: f32) -> u32 {
    log("set_float", p, &[id, value.to_bits()]);
    STATE.with(|s| s.borrow_mut().wear = value);
    0xCAFE_BABE
}
extern "thiscall" fn funding(p: *mut u8) -> *mut f32 {
    log("funding", p, &[]);
    unsafe { p.add(4).cast() }
}
extern "thiscall" fn holder(p: *mut u8) -> *mut u8 {
    log("holder", p, &[]);
    unsafe { p.add(4).cast::<*mut u8>().read() }
}
extern "thiscall" fn efficiency(p: *mut u8, out: *mut u32, plant: *mut u8) -> *mut f32 {
    log("efficiency", p, &[plant as u32]);
    unsafe {
        out.write(p.add(0x40).cast::<u32>().read());
        if p.add(0x44).cast::<u32>().read() != 0 {
            p.add(0x44).cast()
        } else {
            out.cast()
        }
    }
}
extern "thiscall" fn curve_fake(p: *mut u8, input: f32) -> f64 {
    log("curve", p, &[input.to_bits()]);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if p as usize == s.sim + 0x3A8 {
            s.usage = input.to_bits();
            s.usage_factor
        } else {
            assert_eq!(p as usize, s.sim + 0x398);
            s.funding = input.to_bits();
            s.funding_factor
        }
    })
}
extern "thiscall" fn position(p: *mut u8, out: *mut f32) -> u32 {
    log("position", p, &[]);
    unsafe {
        out.write(32.0);
        out.add(1).write(48.0);
        out.add(2).write(64.0);
    }
    0
}
extern "thiscall" fn position_to_cell(p: *mut u8, x: f32, z: f32, ox: *mut i32, oz: *mut i32) -> u32 {
    log("cell", p, &[x.to_bits(), z.to_bits()]);
    unsafe {
        ox.write(2);
        oz.write(4);
    }
    0
}
extern "thiscall" fn lot(p: *mut u8, x: i32, z: i32, zero: u32) -> *mut u8 {
    log("lot", p, &[x as u32, z as u32, zero]);
    STATE.with(|s| s.borrow().lot as *mut u8)
}
extern "thiscall" fn addref(p: *mut u8) -> u32 {
    log("addref", p, &[]);
    0xABCD_1234
}
extern "thiscall" fn release(p: *mut u8) -> u32 {
    log("release", p, &[]);
    0xBCDE_2345
}
extern "thiscall" fn condition(p: *mut u8, value: u32) -> u32 {
    log("condition", p, &[value]);
    0xCDEF_3456
}
extern "C" fn alloc(size: u32) -> *mut u8 {
    log("allocate", core::ptr::null_mut(), &[size]);
    STATE.with(|s| s.borrow().header as *mut u8)
}
extern "thiscall" fn push(p: *mut u32, value: *mut *mut u8) -> u32 {
    unsafe {
        log("push", p.cast(), &[(*value) as u32]);
        let header = p.read() as *mut u32;
        assert_eq!(header.read(), header as u32);
        assert_eq!(header.add(1).read(), header as u32);
    }
    0
}
extern "thiscall" fn demolish(
    p: *mut u8,
    one: u32,
    list: *mut u32,
    zero: u32,
    flags: u32,
    a: u32,
    b: u32,
    c: u32,
    d: u32,
) -> u32 {
    log("demolish", p, &[one, unsafe { list.read() }, zero, flags, a, b, c, d]);
    0
}
extern "thiscall" fn radiation(p: *mut u8, position: *mut f32, strength: f32, radius: f32) -> u32 {
    unsafe {
        log(
            "radiation",
            p,
            &[
                position.read().to_bits(),
                position.add(1).read().to_bits(),
                position.add(2).read().to_bits(),
                strength.to_bits(),
                radius.to_bits(),
            ],
        );
    }
    0
}
extern "C" fn post_fake(id: u32, a: u32, b: u32, c: u32, d: u32, plant: *mut u8) -> u32 {
    log("post", plant, &[id, a, b, c, d]);
    0
}
extern "thiscall" fn clear(p: *mut u32) -> u32 {
    log("clear", p.cast(), &[unsafe { p.read() }]);
    if STATE.with(|s| s.borrow().clear_header) {
        unsafe {
            p.write(0);
        }
    }
    0
}
extern "C" fn free_fake(p: *mut u8) -> u32 {
    log("free", p, &[]);
    0xDEF0_4567
}

#[unsafe(naked)]
unsafe extern "fastcall" fn invoke(this: *mut u8, plant: *mut u8) -> u64 {
    core::arch::naked_asm!("push edx", "call {entry}", "mov edx, [esp - 4]", "ret", entry = sym fc4p_006ec8f0)
}

struct Fixture {
    sim: Object,
    plant: Object,
    holder: Object,
    department: Object,
    lot: Object,
    header: Object,
    city: Object,
    manager: Object,
    demolition: Object,
    pollution: Object,
    tables: Vec<Vec<usize>>,
    globals: Box<[*mut u8; 3]>,
    constants: Box<(f32, f64, f32)>,
    fakes: Vec<Fake>,
    _fpu: GameFpu,
}
impl Fixture {
    fn new() -> Self {
        STATE.with(|s| *s.borrow_mut() = State::default());
        let mut f = Self {
            sim: Object::new(0x400),
            plant: Object::new(16),
            holder: Object::new(16),
            department: Object::new(16),
            lot: Object::new(16),
            header: Object::new(16),
            city: Object::new(16),
            manager: Object::new(16),
            demolition: Object::new(16),
            pollution: Object::new(16),
            tables: vec![],
            globals: Box::new([core::ptr::null_mut(); 3]),
            constants: Box::new((1.0, 0.8, 0.0)),
            fakes: vec![],
            _fpu: GameFpu::new(),
        };
        let (s, p, h, d, l, c, m, demo, pollution) = (
            f.sim.ptr(),
            f.plant.ptr(),
            f.holder.ptr(),
            f.department.ptr(),
            f.lot.ptr(),
            f.city.ptr(),
            f.manager.ptr(),
            f.demolition.ptr(),
            f.pollution.ptr(),
        );
        f.table(s, &[(0xB4, efficiency as *const () as usize)]);
        f.table(
            p,
            &[
                (0x18, holder as *const () as usize),
                (0x20, position as *const () as usize),
            ],
        );
        f.table(d, &[(0x28, funding as *const () as usize)]);
        f.table(
            l,
            &[
                (4, addref as *const () as usize),
                (8, release as *const () as usize),
                (0x90, condition as *const () as usize),
            ],
        );
        f.table(c, &[(0x17C, position_to_cell as *const () as usize)]);
        f.table(m, &[(0x18, lot as *const () as usize)]);
        f.table(demo, &[(0x1C, demolish as *const () as usize)]);
        f.table(pollution, &[(0x108, radiation as *const () as usize)]);
        unsafe {
            s.add(0x20).cast::<*mut u8>().write(d);
            s.add(0x34).cast::<*mut u8>().write(demo);
            s.add(0x40).cast::<f32>().write(1.0);
            p.add(4).cast::<*mut u8>().write(h);
            d.add(4).cast::<f32>().write(0.75);
        }
        STATE.with(|state| {
            let mut state = state.borrow_mut();
            state.sim = s as usize;
            state.lot = l as usize;
            state.header = f.header.ptr() as usize;
        });
        for (index, (address, p)) in [(0x00B4_3CEC, c), (0x00B4_3D08, m), (0x00B4_3D9C, pollution)]
            .into_iter()
            .enumerate()
        {
            f.globals[index] = p;
            f.fakes
                .push(fake_data(address, (&mut f.globals[index] as *mut *mut u8).cast()));
        }
        f.fakes.extend([
            fake(0x005F_D4F0, get_float_fake as *const () as usize),
            fake(0x005F_D450, get_int_fake as *const () as usize),
            fake(0x005F_D480, get_uint_fake as *const () as usize),
            fake(0x005F_E4F0, set_float_fake as *const () as usize),
            fake(0x005F_4250, curve_fake as *const () as usize),
            fake(0x0090_CF54, alloc as *const () as usize),
            fake(0x0090_CF63, free_fake as *const () as usize),
            fake(0x0046_7A10, push as *const () as usize),
            fake(0x0047_3EC0, clear as *const () as usize),
            fake(0x005F_98C0, post_fake as *const () as usize),
            fake_data(0x00B0_D424, (&mut f.constants.0 as *mut f32).cast()),
            fake_data(0x00AB_2880, (&mut f.constants.1 as *mut f64).cast()),
            fake_data(0x00A8_1054, (&mut f.constants.2 as *mut f32).cast()),
        ]);
        f
    }
    fn table(&mut self, p: *mut u8, entries: &[(usize, usize)]) {
        let mut table = vec![0; entries.iter().map(|e| e.0 / 4 + 1).max().unwrap()];
        for &(slot, target) in entries {
            table[slot / 4] = target;
        }
        unsafe {
            p.cast::<*const usize>().write(table.as_ptr());
        }
        self.tables.push(table);
    }
    fn prop(&self, id: u32, value: u32) {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.props.retain(|&(i, _)| i != id);
            s.props.push((id, value));
        });
    }
    fn float(&self, id: u32, value: f32) {
        self.prop(id, value.to_bits());
    }
    fn run(&mut self) -> (u32, u32) {
        let result = unsafe { invoke(self.sim.ptr(), self.plant.ptr()) };
        (result as u32, (result >> 32) as u32)
    }
    fn counters(&mut self) -> (u32, u32, u32) {
        let s = unsafe { AgeSim::from_ptr(self.sim.ptr()) };
        (s.overworked(), s.aged(), s.distressed())
    }
    fn calls(&self) -> Vec<Call> {
        STATE.with(|s| std::mem::take(&mut s.borrow_mut().calls))
    }
}

#[test]
fn ageing_uses_defaults_curve_order_stored_factors_and_the_original_argument_word() {
    let mut f = Fixture::new();
    f.prop(0x2781_2852, 100);
    f.prop(0x08D9_763D, 50);
    f.float(0xEA1C_F220, 1.0);
    assert_eq!(f.run().1, 0.75f32.to_bits());
    STATE.with(|s| {
        let s = s.borrow();
        assert_eq!(s.usage, 0.5f32.to_bits());
        assert_eq!(s.funding, 0.75f32.to_bits());
        assert_eq!(s.wear, 6.0);
    });
    assert_eq!(f.counters(), (1, 1, 0));
    let calls = f.calls();
    assert_eq!(
        calls[2..9].iter().map(|c| c.2[0]).collect::<Vec<_>>(),
        [
            0xC910_B4CF,
            0xEA1C_F220,
            0x2781_2852,
            0x08D9_763D,
            0xEA1C_F221,
            0xEA1C_F222,
            0x6A43_150F,
        ]
    );
    assert_eq!(
        calls.iter().map(|c| c.0).collect::<Vec<_>>(),
        [
            "funding",
            "holder",
            "float",
            "float",
            "int",
            "int",
            "float",
            "float",
            "uint",
            "efficiency",
            "curve",
            "curve",
            "set_float"
        ]
    );
    assert_eq!(
        calls[10],
        Call("curve", f.sim.ptr() as usize + 0x3A8, vec![0.5f32.to_bits()])
    );
    assert_eq!(
        calls[11],
        Call("curve", f.sim.ptr() as usize + 0x398, vec![0.75f32.to_bits()])
    );
    assert_eq!(calls[12].2, [0xC910_B4CF, 6.0f32.to_bits()]);
}

#[test]
fn ageing_effective_capacity_is_signed_truncated_and_usage_requires_both_positive() {
    for (capacity, used, efficiency_value, expected) in [
        (100i32, 50i32, 0.5f32, 1.0f32),
        (3, 2, 0.5, 2.0),
        (0, 10, 1.0, 0.0),
        (-1, 10, 1.0, 0.0),
        (100, -1, 1.0, 0.0),
        (100, 10, f32::NAN, 0.0),
        (i32::MAX, 1, 2.0, 0.0),
    ] {
        let mut f = Fixture::new();
        f.prop(0x2781_2852, capacity as u32);
        f.prop(0x08D9_763D, used as u32);
        unsafe {
            f.sim.ptr().add(0x40).cast::<f32>().write(efficiency_value);
        }
        f.run();
        STATE.with(|s| assert_eq!(s.borrow().usage, expected.to_bits()));
    }
}

#[test]
fn ageing_counters_test_threshold_not_usage_and_compare_wear_to_the_double_constant() {
    for (threshold, wear, expected) in [
        (1.0f32, 0.8f32, (0, 1, 0)),
        (1.0, 0.799_999_95, (0, 0, 0)),
        (0.5, 0.0, (0, 0, 0)),
        (f32::NAN, f32::NAN, (0, 0, 0)),
        (99.0, 0.0, (1, 0, 0)),
    ] {
        let mut f = Fixture::new();
        f.float(0xEA1C_F221, threshold);
        f.float(0xC910_B4CF, wear);
        f.run();
        assert_eq!(f.counters(), expected);
    }
}

#[test]
fn ageing_distress_respects_threshold_order_missing_lots_and_full_return_values() {
    for (first, second, flags, present, expected) in [
        (0.5f32, 1.0f32, 0u32, true, true),
        (1.0, 0.5, 0, true, false),
        (0.5, 0.5, 0, true, true),
        (0.5, 0.5, 0, false, false),
    ] {
        let mut f = Fixture::new();
        f.float(0xC910_B4CF, 0.75);
        f.float(0xEA1C_F221, first);
        f.float(0xEA1C_F222, second);
        f.prop(0x6A43_150F, flags);
        if !present {
            STATE.with(|s| s.borrow_mut().lot = 0);
        }
        let (eax, _) = f.run();
        let calls = f.calls();
        assert_eq!(calls[13].0, "position");
        assert_eq!(calls[14].2, [32f32.to_bits(), 64f32.to_bits()]);
        assert_eq!(calls[15].2, [2, 4, 0]);
        assert_eq!(f.counters().2, u32::from(expected));
        if expected {
            assert_eq!(eax, 0xCDEF_3456);
            assert_eq!(calls.last().unwrap().2, [2]);
        }
        if !present {
            assert_eq!(eax, 0);
        }
    }
}

#[test]
fn demolition_owns_a_game_allocated_list_and_radiation_gates_the_nuclear_message() {
    for (radius, nuclear, success) in [
        (0.0f32, true, true),
        (2.0, true, true),
        (2.0, false, true),
        (2.0, true, false),
        (f32::NAN, true, true),
    ] {
        let mut f = Fixture::new();
        f.float(0xC910_B4CF, 1.0);
        f.float(0xEA1C_F222, 0.5);
        f.prop(0x6A43_150F, 4);
        f.float(0xAA5C_04C8, radius);
        f.float(0xAA5C_04C5, 7.0);
        if success {
            f.prop(0x2781_2853, if nuclear { 5 } else { 1 });
        }
        let (eax, arg) = f.run();
        assert_eq!(eax, 0xDEF0_4567);
        assert_eq!(arg, f.header.ptr() as u32);
        let calls = f.calls();
        let mut tail = vec![
            "position", "cell", "lot", "allocate", "addref", "push", "release", "demolish", "float", "float",
        ];
        if radius == 2.0 {
            tail.extend(["radiation", "uint"]);
            if nuclear && success {
                tail.push("post");
            }
        }
        tail.extend(["clear", "free"]);
        assert_eq!(calls[13..].iter().map(|c| c.0).collect::<Vec<_>>(), tail);
        assert_eq!(calls[20].2, [1, f.header.ptr() as u32, 0, 0x304, 0, 0, 0, 0]);
        if radius == 2.0 {
            assert_eq!(
                calls[23].2,
                [
                    32f32.to_bits(),
                    48f32.to_bits(),
                    64f32.to_bits(),
                    7f32.to_bits(),
                    2f32.to_bits()
                ]
            );
        }
    }
}

#[test]
fn list_cleanup_can_clear_the_argument_header_and_skip_its_final_free() {
    let mut f = Fixture::new();
    f.float(0xC910_B4CF, 1.0);
    f.float(0xEA1C_F222, 0.5);
    f.prop(0x6A43_150F, 1);
    STATE.with(|s| s.borrow_mut().clear_header = true);
    assert_eq!(f.run(), (0, 0));
    assert_eq!(f.calls().last().unwrap().0, "clear");
}

#[test]
fn curve_output_stores_usage_to_float32_and_keeps_funding_in_the_x87_exponent_range() {
    for (usage_factor, funding_factor, expected) in [
        (
            f64::from_bits((1023u64 - 150) << 52),
            f64::from_bits((1023u64 + 120) << 52),
            0u32,
        ),
        (
            f64::from_bits((1023u64 - 100) << 52),
            f64::from_bits((1023u64 + 150) << 52),
            (127u32 + 50) << 23,
        ),
    ] {
        let mut f = Fixture::new();
        f.float(0xEA1C_F220, 1.0);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.usage_factor = usage_factor;
            s.funding_factor = funding_factor;
        });
        f.run();
        STATE.with(|s| assert_eq!(s.borrow().wear.to_bits(), expected));
    }
}

fn environment() -> (u16, u16, u32, u32) {
    let mut cw = 0u16;
    let mut mxcsr = 0u32;
    let sw: u16;
    let flags: u32;
    unsafe {
        core::arch::asm!(
            "fnstcw [{cw}]", "fnstsw ax", "stmxcsr [{mxcsr}]", "pushfd", "pop {flags:e}",
            cw = in(reg) &mut cw, mxcsr = in(reg) &mut mxcsr, flags = lateout(reg) flags, out("ax") sw,
        );
    }
    (cw, sw & 0x3800, mxcsr, flags & 0x400)
}

#[test]
fn ageing_preserves_the_float_environment_existing_stack_value_and_unrelated_fields() {
    let mut f = Fixture::new();
    let sentinel = 123.25f64;
    let mut restored = 0.0f64;
    let mut expected = unsafe { core::slice::from_raw_parts(f.sim.ptr(), 0x400) }.to_vec();
    expected[0x308..0x30C].copy_from_slice(&1u32.to_le_bytes());
    unsafe {
        core::arch::asm!("fnclex", "fld qword ptr [{p}]", p = in(reg) &sentinel, out("st(0)") _);
    }
    let before = environment();
    let (eax, arg) = f.run();
    let after = environment();
    unsafe {
        core::arch::asm!("fstp qword ptr [{p}]", p = in(reg) &mut restored, out("st(0)") _);
    }
    assert_eq!(after, before);
    assert_eq!(restored.to_bits(), sentinel.to_bits());
    assert_eq!(eax, 0xCAFE_3900);
    assert_eq!(arg, 0.75f32.to_bits());
    assert_eq!(unsafe { core::slice::from_raw_parts(f.sim.ptr(), 0x400) }, expected);
}
