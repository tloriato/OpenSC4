// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, fake_data, Fake, GameFpu, Object};
use std::cell::RefCell;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Call {
    Resource,
    New,
    Insert([u32; 2]),
    Points(usize, Vec<[u32; 2]>),
    ResourceCurve(u32),
    Property(u32),
    PropertyCurve(usize, usize),
    CellHq(u32, u32),
    GlobalHq,
    CellLe(u32, u32),
    GlobalLe,
    Release,
    Eval(u32),
    Free(usize),
}
#[derive(Default)]
struct State {
    calls: Vec<Call>,
    allocations: Vec<Object>,
    available: bool,
    spare: bool,
    shipped: bool,
    hq: f64,
    le: u32,
    output: Option<f64>,
    properties: [usize; 3],
    lookup: usize,
    curve_data: usize,
    vector_data: usize,
}
thread_local! {static STATE:RefCell<State>=RefCell::new(State::default());}
fn record(call: Call) {
    STATE.with(|s| s.borrow_mut().calls.push(call));
}
fn allocate(size: usize) -> *mut u8 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.allocations.push(Object::new(size));
        s.allocations.last_mut().unwrap().ptr()
    })
}
fn take() -> Vec<Call> {
    STATE.with(|s| core::mem::take(&mut s.borrow_mut().calls))
}
extern "thiscall" fn resource(this: *mut u8, key: *const u32, iid: u32, out: *mut *mut u8, a: u32, b: u32) -> u64 {
    assert_eq!(unsafe { core::slice::from_raw_parts(key, 3) }, KEY);
    assert_eq!((iid, a, b), (0xA521_60F5, 0, 0));
    record(Call::Resource);
    if STATE.with(|s| s.borrow().available) {
        unsafe { *out = this };
        registers(0xABCD_EF01, 0x1122_3344)
    } else {
        registers(0xABCD_EF00, 0x1122_3344)
    }
}
extern "thiscall" fn new_curve(this: *mut u8) -> u32 {
    record(Call::New);
    unsafe { core::ptr::write_bytes(this, 0, 16) };
    this as u32
}
extern "thiscall" fn insert(
    this: *mut u8,
    position: *mut u32,
    point: *const u32,
    _tag: *const u8,
    count: u32,
    stride: u32,
) -> u32 {
    assert_eq!((count, stride), (1, 1));
    let point = unsafe { [point.read_unaligned(), point.add(1).read_unaligned()] };
    record(Call::Insert(point));
    unsafe {
        let points = Points::from_ptr(this);
        assert_eq!(position, points.end());
        let existing = ((points.end() as usize).wrapping_sub(points.begin() as usize)) / 4;
        let spare = STATE.with(|s| s.borrow().spare);
        let words = if spare { 4 } else { existing + 2 };
        let data = allocate(words * 4).cast::<u32>();
        if existing > 0 {
            core::ptr::copy_nonoverlapping(points.begin(), data, existing);
        }
        data.add(existing).write(point[0]);
        data.add(existing + 1).write(point[1]);
        points.set_begin(data);
        points.set_end(data.add(existing + 2));
        points.set_capacity(data.add(words));
        STATE.with(|s| s.borrow_mut().vector_data = data as usize);
    }
    0
}
extern "thiscall" fn set(this: *mut u8, vector: *mut u8) -> u32 {
    unsafe {
        let points = Points::from_ptr(vector);
        let len = ((points.end() as usize) - (points.begin() as usize)) / 8;
        let data = core::slice::from_raw_parts(points.begin().cast::<[u32; 2]>(), len).to_vec();
        record(Call::Points(this as usize, data));
        let bytes = len * 8;
        let copied = allocate(bytes);
        core::ptr::copy_nonoverlapping(points.begin().cast::<u8>(), copied, bytes);
        this.cast::<*mut u8>().write(copied);
        this.add(4).cast::<*mut u8>().write(copied.add(bytes));
        STATE.with(|s| s.borrow_mut().curve_data = copied as usize);
    }
    1
}
extern "thiscall" fn set_from_resource(_this: *mut u8, key: *const u32, id: u32) -> u32 {
    assert_eq!(unsafe { core::slice::from_raw_parts(key, 3) }, KEY);
    record(Call::ResourceCurve(id));
    1
}
extern "thiscall" fn property(_this: *mut u8, id: u32) -> *mut u8 {
    record(Call::Property(id));
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let p = s.properties[s.lookup];
        s.lookup += 1;
        p as *mut u8
    })
}
extern "thiscall" fn from_property(this: *mut u8, p: *mut u8) -> u32 {
    record(Call::PropertyCurve(this as usize, p as usize));
    0
}
extern "thiscall" fn release(_this: *mut u8) -> u32 {
    record(Call::Release);
    0
}
extern "C" fn free_storage(p: *mut u8) -> u64 {
    record(Call::Free(p as usize));
    registers(0x7654_3200, 0x89AB_CDEF)
}
extern "thiscall" fn cell_hq(_this: *mut u8, x: u32, z: u32) -> f64 {
    record(Call::CellHq(x, z));
    STATE.with(|s| s.borrow().hq)
}
extern "thiscall" fn global_hq(_this: *mut u8) -> f64 {
    record(Call::GlobalHq);
    STATE.with(|s| s.borrow().hq)
}
extern "thiscall" fn cell_le(_this: *mut u8, x: u32, z: u32) -> u32 {
    record(Call::CellLe(x, z));
    STATE.with(|s| s.borrow().le)
}
extern "thiscall" fn global_le(_this: *mut u8) -> u32 {
    record(Call::GlobalLe);
    STATE.with(|s| s.borrow().le)
}
extern "thiscall" fn eval(this: *mut u8, x: f32) -> f64 {
    record(Call::Eval(x.to_bits()));
    if let Some(out) = STATE.with(|s| s.borrow().output) {
        return out;
    }
    if STATE.with(|s| s.borrow().shipped) {
        let le_curve = STATE.with(|s| s.borrow().calls.contains(&Call::ResourceCurve(0x6941_A67F)));
        if le_curve {
            return f64::from(50.0 + x * 0.25);
        }
        let points = [
            (0.0, 0.0),
            (25.0, 30.0),
            (50.0, 40.0),
            (75.0, 50.0),
            (100.0, 50.0),
            (200.0, 0.0),
        ];
        for pair in points.windows(2) {
            if x <= pair[1].0 {
                return f64::from(pair[0].1 + (pair[1].1 - pair[0].1) * (x - pair[0].0) / (pair[1].0 - pair[0].0));
            }
        }
        return 0.0;
    }
    unsafe {
        let begin = this.cast::<*const [u32; 2]>().read();
        let end = this.add(4).cast::<*const [u32; 2]>().read();
        let points = core::slice::from_raw_parts(begin, (end as usize - begin as usize) / 8);
        let a = points[0];
        let b = points[1];
        let (ax, ay, bx, by) = (
            f32::from_bits(a[0]),
            f32::from_bits(a[1]),
            f32::from_bits(b[0]),
            f32::from_bits(b[1]),
        );
        f64::from(ay + (by - ay) * (x - ax) / (bx - ax))
    }
}

struct Fixture {
    sim: Object,
    resource: Object,
    _table: Box<[usize; 37]>,
    _guards: Vec<Fake>,
    _fpu: GameFpu,
}
impl Fixture {
    fn new() -> Self {
        STATE.with(|s| {
            *s.borrow_mut() = State {
                available: true,
                ..State::default()
            }
        });
        let mut table = Box::new([0usize; 37]);
        table[2] = release as *const () as usize;
        table[3] = resource as *const () as usize;
        table[9] = property as *const () as usize;
        table[0x90 / 4] = cell_hq as *const () as usize;
        table[0x84 / 4] = global_hq as *const () as usize;
        table[0x74 / 4] = cell_le as *const () as usize;
        table[0x88 / 4] = global_le as *const () as usize;
        let mut sim = Object::new(0x40);
        let mut resource = Object::new(0x20);
        unsafe {
            sim.ptr().cast::<*const usize>().write(table.as_ptr());
            resource.ptr().cast::<*const usize>().write(table.as_ptr());
        }
        let mut guards = vec![
            fake(0x007D_0410, new_curve as *const () as usize),
            fake(0x007C_61A0, insert as *const () as usize),
            fake(0x005F_4480, set as *const () as usize),
            fake(0x005F_4700, set_from_resource as *const () as usize),
            fake(0x005F_4620, from_property as *const () as usize),
            fake(0x005F_4250, eval as *const () as usize),
            fake(0x0090_CF63, free_storage as *const () as usize),
        ];
        // The game global stand-ins share the fixture's separately owned resource object.
        unsafe {
            let rp = resource.ptr();
            rp.add(4).cast::<*mut u8>().write(rp);
        }
        guards.push(fake_data(0x00B4_3CA8, unsafe { resource.ptr().add(4) }));
        unsafe {
            resource.ptr().add(8).cast::<u32>().write(0);
            resource.ptr().add(12).cast::<u32>().write(0.01f32.to_bits());
        }
        guards.push(fake_data(0x00B4_AF90, unsafe { resource.ptr().add(8) }));
        guards.push(fake_data(0x00A8_23A4, unsafe { resource.ptr().add(12) }));
        Self {
            sim,
            resource,
            _table: table,
            _guards: guards,
            _fpu: GameFpu::new(),
        }
    }
    fn life(&mut self, global: bool) -> u64 {
        unsafe {
            if global {
                fc4p_006fc9e0(self.sim.ptr())
            } else {
                fc4p_006fc570(self.sim.ptr(), 0xFFFF_FFFD, 123)
            }
        }
    }
    fn workforce(&mut self, global: bool, out: &mut u32) -> u64 {
        unsafe {
            if global {
                fc4p_006fc860(self.sim.ptr(), out)
            } else {
                fc4p_006fc6e0(self.sim.ptr(), out, 0xFFFF_FFFD, 123)
            }
        }
    }
}

#[test]
fn life_expectancy_shipped_curve_and_truncation() {
    let mut f = Fixture::new();
    STATE.with(|s| s.borrow_mut().shipped = true);
    for global in [false, true] {
        for (hq, le) in [(100.0, 75), (22.0, 55), (30.0, 57), (0.0, 50), (44.0, 61)] {
            STATE.with(|s| s.borrow_mut().hq = hq);
            assert_eq!(f.life(global), registers(0x7654_3200 | le, 0x89AB_CDEF));
            let calls = take();
            let query = if global {
                Call::GlobalHq
            } else {
                Call::CellHq(0xFFFF_FFFD, 123)
            };
            assert_eq!(calls[0..3], [Call::Resource, Call::New, Call::Insert([0, 0])]);
            assert_eq!(calls[3], Call::Insert([200f32.to_bits(), 200f32.to_bits()]));
            assert!(matches!(&calls[4],Call::Points(_,p) if *p==vec![[0,0],[200f32.to_bits(),200f32.to_bits()]]));
            assert_eq!(calls[5], Call::ResourceCurve(0x6941_A67F));
            assert_eq!(
                &calls[6..8],
                if global {
                    vec![Call::Release, query]
                } else {
                    vec![query, Call::Release]
                }
            );
            assert_eq!(calls[8], Call::Eval((hq as f32).to_bits()));
            let (vector, curve) = STATE.with(|s| (s.borrow().vector_data, s.borrow().curve_data));
            assert_eq!(&calls[9..], [Call::Free(vector), Call::Free(curve)]);
        }
    }
}

#[test]
fn workforce_uses_only_life_byte_and_stores_one_percent_multiply() {
    let mut f = Fixture::new();
    STATE.with(|s| s.borrow_mut().shipped = true);
    for global in [false, true] {
        for (le, expected) in [(75, 0.5f32), (61, 0.444), (0, 0.0)] {
            STATE.with(|s| s.borrow_mut().le = 0xFEDC_BA00 | le);
            let mut out = 0xDEAD_BEEF;
            let result = f.workforce(global, &mut out);
            assert_eq!(result, registers(&mut out as *mut u32 as u32, 0x89AB_CDEF));
            assert_eq!(out, expected.to_bits());
            let calls = take();
            assert_eq!(calls[5], Call::ResourceCurve(0x4941_A92B));
            assert_eq!(
                calls[6],
                if global {
                    Call::GlobalLe
                } else {
                    Call::CellLe(0xFFFF_FFFD, 123)
                }
            );
            assert_eq!(calls[7], Call::Release);
            assert_eq!(calls[8], Call::Eval((le as f32).to_bits()));
        }
    }
}

#[test]
fn missing_resource_returns_full_residual_words_and_static_zero_bits() {
    let mut f = Fixture::new();
    STATE.with(|s| s.borrow_mut().available = false);
    unsafe { f.resource.ptr().add(8).cast::<u32>().write(0x8000_0000) };
    for global in [false, true] {
        assert_eq!(f.life(global), registers(0xABCD_EF00, 0x1122_3344));
        assert_eq!(take(), [Call::Resource]);
        let mut out = 0xDEAD_BEEF;
        let result = f.workforce(global, &mut out);
        assert_eq!(out, 0x8000_0000);
        assert_eq!(
            result,
            registers(
                &mut out as *mut u32 as u32,
                if global { 0x8000_0000 } else { 0x1122_3344 }
            )
        );
        assert_eq!(take(), [Call::Resource]);
    }
}

#[test]
fn code_defaults_and_spare_vector_capacity_are_preserved() {
    let mut f = Fixture::new();
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.spare = true;
        s.hq = 100.0;
        s.le = 100
    });
    for global in [false, true] {
        assert_eq!(f.life(global) as u8, 100);
        let calls = take();
        assert_eq!(calls.iter().filter(|c| matches!(c, Call::Insert(_))).count(), 1);
        let mut out = 0;
        f.workforce(global, &mut out);
        assert_eq!(out, 0.5f32.to_bits());
        let calls = take();
        assert_eq!(calls.iter().filter(|c| matches!(c, Call::Insert(_))).count(), 1);
    }
}

#[test]
fn life_output_wraps_uses_ftol_and_rounds_hq_before_evaluation() {
    let mut f = Fixture::new();
    STATE.with(|s| s.borrow_mut().hq = 100.000_003);
    for global in [false, true] {
        for (output, byte) in [(-10.75, 246), (300.75, 44), (3e9, 0), (1e19, 0), (f64::NAN, 0)] {
            STATE.with(|s| s.borrow_mut().output = Some(output));
            assert_eq!(f.life(global) as u8, byte);
            assert!(take().contains(&Call::Eval(100f32.to_bits())));
        }
    }
}

#[test]
fn workforce_keeps_extended_exponent_until_multiply() {
    let mut f = Fixture::new();
    STATE.with(|s| s.borrow_mut().output = Some(1e40));
    let mut out = 0;
    f.workforce(false, &mut out);
    let expected = multiply_percent(F80::from_f64(1e40), 0.01).to_f32().to_bits();
    assert_eq!(out, expected);
    assert!(f32::from_bits(out).is_finite());
    take();
}

#[test]
fn property_holder_selects_ids_and_falls_back_per_curve() {
    let mut f = Fixture::new();
    let mut outputs = [Object::new(16), Object::new(16), Object::new(16)];
    for (school, ids) in [
        (1, [0xC929_9662, 0xC92D_7EC0, 0xE921_A936]),
        (0, [0xA92A_E3BD, 0xA92D_9FCD, 0x2921_A9CC]),
        (0x100, [0xA92A_E3BD, 0xA92D_9FCD, 0x2921_A9CC]),
    ] {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.properties = [0, 0x1234, 0];
            s.lookup = 0
        });
        let [a, b, c] = outputs.each_mut().map(|o| o.ptr());
        unsafe { fc4p_006fc400(f.sim.ptr(), f.resource.ptr(), a, b, c, school) };
        let calls = take();
        let defaults = vec![[0, 100f32.to_bits()], [100f32.to_bits(), 100f32.to_bits()]];
        let vector = STATE.with(|s| s.borrow().vector_data);
        assert_eq!(
            calls,
            vec![
                Call::Insert(defaults[0]),
                Call::Insert(defaults[1]),
                Call::Property(ids[0]),
                Call::Points(a as usize, defaults.clone()),
                Call::Property(ids[1]),
                Call::PropertyCurve(b as usize, 0x1234),
                Call::Property(ids[2]),
                Call::Points(c as usize, defaults),
                Call::Free(vector)
            ]
        );
    }
}

#[unsafe(naked)]
unsafe extern "thiscall" fn resource_only_al(
    _this: *mut u8,
    _key: *const u32,
    _iid: u32,
    _out: *mut *mut u8,
    _a: u32,
    _b: u32,
) -> u64 {
    core::arch::naked_asm!("xor al,al", "ret 20");
}

#[test]
fn lookup_supplies_the_residual_registers_for_an_al_only_failure() {
    let mut f = Fixture::new();
    f._table[3] = resource_only_al as *const () as usize;
    let key = KEY;
    let mut resource = null_mut();
    let expected = registers((f._table.as_ptr() as u32) & !0xFF, key.as_ptr() as u32);
    assert_eq!(unsafe { lookup(key.as_ptr(), &mut resource) }, expected);
    assert_eq!(resource, null_mut());
    assert!(take().is_empty());
}

fn environment() -> (u16, u32, u16) {
    let mut cw = 0u16;
    let mut mxcsr = 0u32;
    let mut status = 0u16;
    unsafe {
        core::arch::asm!("fnstcw word ptr [{cw}]","stmxcsr dword ptr [{mxcsr}]","fnstsw word ptr [{status}]",
        cw=in(reg)&mut cw,mxcsr=in(reg)&mut mxcsr,status=in(reg)&mut status,options(nostack));
    }
    (cw, mxcsr, status & 0x3800)
}

#[test]
fn queries_preserve_the_float_environment_and_existing_x87_value_without_simulator_writes() {
    let mut f = Fixture::new();
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.hq = 100.0;
        s.le = 75
    });
    let before = unsafe { core::slice::from_raw_parts(f.sim.ptr(), 0x40).to_vec() };
    let sentinel = 1.234_567_890_123_456_7f64;
    unsafe {
        core::arch::asm!("fld qword ptr [{p}]",p=in(reg)&sentinel,out("st(0)") _);
    }
    let initial = environment();
    for global in [false, true] {
        f.life(global);
        assert_eq!(environment(), initial);
        let mut out = 0;
        f.workforce(global, &mut out);
        assert_eq!(environment(), initial);
    }
    let mut restored = 0f64;
    unsafe {
        core::arch::asm!("fstp qword ptr [{p}]",p=in(reg)&mut restored,out("st(0)") _);
    }
    assert_eq!(restored.to_bits(), sentinel.to_bits());
    assert_eq!(unsafe { core::slice::from_raw_parts(f.sim.ptr(), 0x40) }, before);
    take();
}
