// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Native-layout pricing cases, including the core's price and maximum-amount examples.

use super::*;
use crate::testing::{fake, fake_data, Fake, GameFpu, Object};
use std::cell::RefCell;

const RESULT: u64 = 0x7654_3210_FEDC_BA98;
const PROPERTY_RESULT: u64 = 0x1234_5678_ABCD_EF01;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Current,
    Exemplar,
    Holder(u32),
    Property(u32, u64),
    Framework,
    Service(u32, u32),
    ReleaseLocal,
    Allocate(u32),
    Find(u32, u32),
    Amount,
    Stat(bool, u32, u32),
    Curve(u32, u32),
    Destroy,
}

struct State {
    calls: Vec<Call>,
    current: usize,
    other: usize,
    exemplar: usize,
    holder: usize,
    allocator: usize,
    sim: usize,
    scale: Option<f32>,
    base: Option<f32>,
    minimum: Option<f64>,
    maximum: Option<f64>,
    existing: Option<i32>,
    figures: [[F80; 5]; 2],
    curve: Vec<F80>,
    curve_index: usize,
    garbage: bool,
    sentinel: [u32; 3],
    nodes: [[u32; 3]; 2],
    deal: usize,
    arguments: usize,
    replace_amount: Option<f64>,
    final_amount: Option<f64>,
    destructor_registers: (u32, u32),
    allocator_global: usize,
}

impl Default for State {
    fn default() -> Self {
        Self {
            calls: vec![],
            current: 0,
            other: 0,
            exemplar: 0,
            holder: 0,
            allocator: 0,
            sim: 0,
            scale: Some(2.5),
            base: Some(1000.0),
            minimum: Some(f64::from(0.1f32)),
            maximum: Some(f64::from(0.5f32)),
            existing: None,
            figures: [
                [500.0, 0.0, 300.0, 0.0, 0.0].map(F80::from_f64),
                [1000.0, 0.0, 2000.0, 0.0, 2.0].map(F80::from_f64),
            ],
            curve: vec![],
            curve_index: 0,
            garbage: false,
            sentinel: [0xCDCD_CDCD; 3],
            nodes: [[0; 3]; 2],
            deal: 0,
            arguments: 0,
            replace_amount: None,
            final_amount: None,
            destructor_registers: (0, 0),
            allocator_global: 0,
        }
    }
}

thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }

extern "thiscall" fn current(_: *mut u8) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Current);
        pair(s.current as u32, 0x1122_3344)
    })
}
extern "thiscall" fn exemplar(_: *mut u8) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Exemplar);
        pair(s.exemplar as u32, 0x2233_4455)
    })
}
extern "thiscall" fn holder_18(_: *mut u8) -> u64 {
    holder(0x18)
}
extern "thiscall" fn holder_1c(_: *mut u8) -> u64 {
    holder(0x1C)
}
fn holder(slot: u32) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Holder(slot));
        pair(s.holder as u32, 0x3344_5566)
    })
}
extern "C" fn float_property(holder: *mut u8, id: u32, out: *mut f32) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(holder as usize, s.holder);
        s.calls
            .push(Call::Property(id, u64::from(unsafe { out.read().to_bits() })));
        if let Some(value) = if id == 0xEA4A_D2FF { s.scale } else { s.base } {
            unsafe { out.write(value) };
        }
    });
    PROPERTY_RESULT
}
extern "C" fn double_property(holder: *mut u8, id: u32, out: *mut f64) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(holder as usize, s.holder);
        s.calls.push(Call::Property(id, unsafe { out.read().to_bits() }));
        if let Some(value) = if id == 0xCA46_9346 { s.maximum } else { s.minimum } {
            unsafe { out.write(value) };
        }
    });
    PROPERTY_RESULT
}
extern "C" fn get_framework() -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Framework);
        pair(s.exemplar as u32, 0x4455_6677)
    })
}
extern "thiscall" fn service(_: *mut u8, cls: u32, iid: u32, out: *mut *mut u8) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert!(unsafe { out.read().is_null() });
        s.calls.push(Call::Service(cls, iid));
        unsafe { out.write(s.allocator as *mut u8) };
    });
    pair(1, 0x5566_7788)
}
extern "thiscall" fn release_local(local: *mut *mut u8) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(unsafe { local.read() } as usize, s.allocator);
        assert_eq!(
            unsafe { (s.allocator_global as *const usize).read() },
            s.allocator
        );
        s.calls.push(Call::ReleaseLocal);
    });
    pair(0x7788_99AA, 0x6677_8899)
}
extern "thiscall" fn allocate(this: *mut u8, bytes: u32) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.allocator);
        s.calls.push(Call::Allocate(bytes));
        pair(s.sentinel.as_mut_ptr() as u32, 0x7788_99AA)
    })
}
extern "thiscall" fn find(this: *mut u8, kind: u32, other: *mut u8, list: *mut *mut u8) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.sim);
        s.calls.push(Call::Find(kind, other as u32));
        let header = s.sentinel.as_mut_ptr() as u32;
        assert_eq!(unsafe { list.read() } as u32, header);
        assert_eq!(s.sentinel, [header, header, 0xCDCD_CDCD]);
        if s.existing.is_some() {
            let n0 = s.nodes[0].as_mut_ptr() as u32;
            let n1 = s.nodes[1].as_mut_ptr() as u32;
            s.nodes[0] = [n1, header, s.deal as u32];
            // A second entry's object is deliberately invalid: only the first is consulted here.
            s.nodes[1] = [header, n0, 0xDEAD_BEEF];
            s.sentinel[0] = n0;
            s.sentinel[1] = n1;
        }
    });
    pair(0x8899_AABB, 0x8899_AABB)
}
extern "thiscall" fn amount(_: *mut u8) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Amount);
        pair(s.existing.unwrap() as u32, 0x99AA_BBCC)
    })
}

extern "fastcall" fn statistic(this: *mut u8, out: *mut F80, slot: u32, commodity: u32) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let other = this as usize != s.current;
        assert!(this as usize == s.current || this as usize == s.other);
        s.calls.push(Call::Stat(other, slot, commodity));
        let index = match slot {
            0xD8 => 0,
            0xF0 => 1,
            0xE8 => 2,
            0xE0 => 3,
            0xF8 => 4,
            _ => unreachable!(),
        };
        unsafe { out.write_unaligned(s.figures[usize::from(other)][index]) };
        if let Some(value) = s.replace_amount.take() {
            unsafe {
                (s.arguments as *mut u8)
                    .add(4)
                    .cast::<f64>()
                    .write_unaligned(value)
            };
        }
    });
    pair(0xAABB_CCDD, 0xBBCC_DDEE)
}
macro_rules! stat_fake {
    ($name:ident, $slot:literal) => {
        #[unsafe(naked)]
        extern "thiscall" fn $name() {
            core::arch::naked_asm!(
                "sub esp, 12", "mov edx, esp", "push dword ptr [esp + 16]", "push {slot}", "call {body}",
                "fld tbyte ptr [esp]", "add esp, 12", "ret 4",
                slot = const $slot, body = sym statistic,
            );
        }
    };
}
stat_fake!(demanded, 0xD8);
stat_fake!(exported, 0xF0);
stat_fake!(produced, 0xE8);
stat_fake!(imported, 0xE0);
stat_fake!(unit_cost, 0xF8);

fn response(input: f32, garbage: bool) -> F80 {
    let points: &[(f32, f32)] = if garbage {
        &[(0.0, 1e-5), (0.0075, 0.0075), (1.0, 0.075)]
    } else {
        &[(0.0, 1.0), (0.1, 0.3), (0.3, 0.1), (1.0, 0.0)]
    };
    for pair in points.windows(2) {
        let ((x0, y0), (x1, y1)) = (pair[0], pair[1]);
        if input <= x0 {
            return F80::from_f32(y0);
        }
        if input < x1 {
            let fraction = subtract_double(F80::from_f32(input), f64::from(x0))
                / subtract_double(F80::from_f32(x1), f64::from(x0));
            return add_float(fraction * subtract_double(F80::from_f32(y1), f64::from(y0)), y0);
        }
    }
    F80::from_f32(points.last().unwrap().1)
}

extern "fastcall" fn curve(this: *mut u8, out: *mut F80, input: u32) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.sim + 0x2C);
        s.calls.push(Call::Curve(this as u32, input));
        let value = if s.curve.is_empty() {
            response(f32::from_bits(input), s.garbage)
        } else {
            s.curve[s.curve_index]
        };
        s.curve_index += 1;
        unsafe { out.write_unaligned(value) };
    });
    pair(0xCCDD_EEFF, 0xDDEE_FF00)
}
#[unsafe(naked)]
extern "thiscall" fn curve_entry() {
    core::arch::naked_asm!(
        "sub esp, 12", "mov edx, esp", "push dword ptr [esp + 16]", "call {body}",
        "fld tbyte ptr [esp]", "add esp, 12", "ret 4", body = sym curve,
    );
}
extern "fastcall" fn destroy(this: *mut *mut u8, edx: u32, eax: u32) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(unsafe { this.read() } as u32, s.sentinel.as_ptr() as u32);
        s.calls.push(Call::Destroy);
        s.destructor_registers = (eax, edx);
        if let Some(value) = s.final_amount {
            unsafe {
                (s.arguments as *mut u8)
                    .add(4)
                    .cast::<f64>()
                    .write_unaligned(value)
            };
        }
    });
    RESULT
}
#[unsafe(naked)]
extern "thiscall" fn destroy_entry() {
    core::arch::naked_asm!("push eax", "call {body}", "ret", body = sym destroy);
}

// Owned object storage and tables remain at fixed addresses throughout each call.
struct World {
    sim: Object,
    current: Object,
    other: Object,
    _objects: Vec<Object>,
    _tables: [Box<[usize; 64]>; 5],
    _data: Box<(*mut u8, *mut u8, f64, f64, f32, f64, f64)>,
    _fakes: Vec<Fake>,
}
impl World {
    fn new() -> Self {
        let _fpu = GameFpu::new();
        STATE.with(|s| *s.borrow_mut() = State::default());
        let mut sim = Object::new(0x40);
        let mut current = Object::new(0x10);
        let mut other = Object::new(0x10);
        let mut manager = Object::new(4);
        let mut tuning = Object::new(4);
        let mut allocator = Object::new(4);
        let mut deal = Object::new(4);
        let mut tables: [Box<[usize; 64]>; 5] = core::array::from_fn(|_| Box::new([0; 64]));
        tables[0][0x74 / 4] = find as *const () as usize;
        tables[0][0x94 / 4] = exemplar as *const () as usize;
        tables[1][0x14 / 4] = service as *const () as usize;
        tables[1][0x18 / 4] = holder_18 as *const () as usize;
        tables[1][0x1C / 4] = holder_1c as *const () as usize;
        tables[2][0x90 / 4] = self::current as *const () as usize;
        tables[3][0x0C / 4] = allocate as *const () as usize;
        tables[3][0x28 / 4] = amount as *const () as usize;
        tables[4][0xD8 / 4] = demanded as *const () as usize;
        tables[4][0xF0 / 4] = exported as *const () as usize;
        tables[4][0xE8 / 4] = produced as *const () as usize;
        tables[4][0xE0 / 4] = imported as *const () as usize;
        tables[4][0xF8 / 4] = unit_cost as *const () as usize;
        unsafe {
            sim.ptr().cast::<usize>().write(tables[0].as_ptr() as usize);
            tuning.ptr().cast::<usize>().write(tables[1].as_ptr() as usize);
            manager.ptr().cast::<usize>().write(tables[2].as_ptr() as usize);
            allocator.ptr().cast::<usize>().write(tables[3].as_ptr() as usize);
            deal.ptr().cast::<usize>().write(tables[3].as_ptr() as usize);
            current.ptr().cast::<usize>().write(tables[4].as_ptr() as usize);
            other.ptr().cast::<usize>().write(tables[4].as_ptr() as usize);
            NeighborsSim::from_ptr(sim.ptr()).set_exemplar(tuning.ptr());
        }
        let mut data = Box::new((manager.ptr(), allocator.ptr(), 0.0, 1.0, 1.0, 0.5, 1e-8));
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.current = current.ptr() as usize;
            s.other = other.ptr() as usize;
            s.sim = sim.ptr() as usize;
            s.exemplar = tuning.ptr() as usize;
            s.holder = 0x1020_3040;
            s.allocator = allocator.ptr() as usize;
            s.deal = deal.ptr() as usize;
            s.allocator_global = &mut data.1 as *mut *mut u8 as usize;
        });
        let fakes = vec![
            fake(0x005F_D4F0, float_property as *const () as usize),
            fake(0x005F_D530, double_property as *const () as usize),
            fake(0x0087_93EC, get_framework as *const () as usize),
            fake(0x0041_1870, release_local as *const () as usize),
            fake(0x0042_1760, destroy_entry as *const () as usize),
            fake(0x005F_4250, curve_entry as *const () as usize),
            fake_data(0x00B4_3C94, (&mut data.0 as *mut *mut u8).cast()),
            fake_data(0x00B2_0D58, (&mut data.1 as *mut *mut u8).cast()),
            fake_data(0x00A8_0990, (&mut data.2 as *mut f64).cast()),
            fake_data(0x00A8_0AB0, (&mut data.3 as *mut f64).cast()),
            fake_data(0x00A8_1228, (&mut data.4 as *mut f32).cast()),
            fake_data(0x00A9_2D28, (&mut data.5 as *mut f64).cast()),
            fake_data(0x00AB_1F88, (&mut data.6 as *mut f64).cast()),
        ];
        Self {
            sim,
            current,
            other,
            _objects: vec![manager, tuning, allocator, deal],
            _tables: tables,
            _data: data,
            _fakes: fakes,
        }
    }

    fn call(&mut self, cost: bool, kind: u32, amount: f64, null: bool) -> (F80, u64, Vec<Call>) {
        let bits = amount.to_bits();
        let other = if null { 0 } else { self.other.ptr() as u32 };
        let mut args = if cost {
            vec![kind, bits as u32, (bits >> 32) as u32, other]
        } else {
            vec![kind, other]
        };
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.calls.clear();
            s.curve_index = 0;
        });
        let before = unsafe { std::slice::from_raw_parts(self.sim.ptr(), 0x40).to_vec() };
        let city_before = unsafe { std::slice::from_raw_parts(self.current.ptr(), 0x10).to_vec() };
        let other_before = unsafe { std::slice::from_raw_parts(self.other.ptr(), 0x10).to_vec() };
        let entry = if cost {
            fc4p_006d3250 as *const () as usize
        } else {
            fc4p_006d3620 as *const () as usize
        };
        let (value, registers) = unsafe { checked_float(entry, self.sim.ptr(), &mut args) };
        assert_eq!(
            unsafe { std::slice::from_raw_parts(self.sim.ptr(), 0x40) },
            before
        );
        assert_eq!(
            unsafe { std::slice::from_raw_parts(self.current.ptr(), 0x10) },
            city_before
        );
        assert_eq!(
            unsafe { std::slice::from_raw_parts(self.other.ptr(), 0x10) },
            other_before
        );
        let calls = STATE.with(|s| core::mem::take(&mut s.borrow_mut().calls));
        (value, registers, calls)
    }
}

/// Exercises the exports with live stack arguments, an existing x87 value, and register sentinels.
unsafe fn checked_float(entry: usize, this: *mut u8, args: &mut [u32]) -> (F80, u64) {
    let _fpu = GameFpu::new();
    let sentinel = 123.25f64;
    let mut restored = 0.0f64;
    let (mut cw, mut mxcsr) = (0u16, 0u32);
    let (mut after_cw, mut after_mxcsr) = (0u16, 0u32);
    let (before_flags, after_flags): (u32, u32);
    #[repr(C)]
    struct Checked {
        call: RawCall,
        preserved: u32,
        state: usize,
    }
    let mut c = Checked {
        call: RawCall::new(Conv::Thiscall, entry, this as u32, args),
        preserved: 0,
        state: 0,
    };
    c.call.eax = 0x0123_4567;
    c.call.edx = 0x89AB_CDEF;
    // The state field receives the address of the pushed argument array immediately before entry.
    let arguments = STATE.with(|s| &mut s.borrow_mut().arguments as *mut usize as usize);
    c.state = arguments;
    unsafe {
        core::arch::asm!("fld qword ptr [{p}]", "fnstcw [{cw}]", "stmxcsr [{mx}]",
            p=in(reg)&sentinel, cw=in(reg)&mut cw, mx=in(reg)&mut mxcsr, out("st(0)") _);
        core::arch::asm!("pushfd", "pop {flags:e}", flags=out(reg)before_flags);
        let before_top = crate::x87::status_word() & 0x3800;
        core::arch::asm!(
            "push esi", "push ebx", "push ebp", "mov esi, 0x12345678", "mov ebx, 0x23456789", "mov ebp, 0x3456789a",
            "mov ecx, [edi + 20]", "mov eax, [edi + 16]",
            "2:", "test ecx, ecx", "jz 3f", "dec ecx", "push dword ptr [eax + ecx*4]", "jmp 2b",
            "3:", "mov eax, [edi + {state}]", "mov [eax], esp",
            "mov eax, [edi + 4]", "mov ecx, [edi + 8]", "mov edx, [edi + 12]", "call dword ptr [edi]",
            "mov [edi + 4], eax", "mov [edi + 12], edx", "fstp tbyte ptr [edi + 32]",
            "xor eax, eax", "cmp esi, 0x12345678", "sete al", "xor edx, edx", "cmp ebx, 0x23456789", "sete dl", "and eax, edx",
            "xor edx, edx", "cmp ebp, 0x3456789a", "sete dl", "and eax, edx", "mov [edi + {preserved}], eax",
            "pop ebp", "pop ebx", "pop esi",
            preserved = const core::mem::offset_of!(Checked, preserved), state = const core::mem::offset_of!(Checked, state),
            in("edi") &mut c, clobber_abi("C"),
        );
        assert_eq!(crate::x87::status_word() & 0x3800, before_top);
        core::arch::asm!("pushfd", "pop {flags:e}", flags=out(reg)after_flags);
        core::arch::asm!("fstp qword ptr [{p}]", "fnstcw [{cw}]", "stmxcsr [{mx}]",
            p=in(reg)&mut restored, cw=in(reg)&mut after_cw, mx=in(reg)&mut after_mxcsr, out("st(0)") _);
    }
    assert_eq!((after_cw, after_mxcsr), (cw, mxcsr));
    assert_eq!(after_flags & 0x400, before_flags & 0x400);
    assert_eq!(restored.to_bits(), sentinel.to_bits());
    assert_eq!(c.preserved, 1);
    (c.call.st0, pair(c.call.eax, c.call.edx))
}

fn expected_calls(
    cost: bool,
    selling: bool,
    commodity: u32,
    kind: u32,
    other: u32,
    sim: u32,
    existing: bool,
) -> Vec<Call> {
    let mut calls = vec![Call::Current];
    if !cost {
        calls.push(Call::Exemplar);
    }
    calls.push(Call::Holder(if cost { 0x1C } else { 0x18 }));
    calls.push(Call::Property(0xEA4A_D2FF, u64::from(1.0f32.to_bits())));
    calls.push(Call::Property(
        [0xEA46_932C, 0x6A46_9337, 0x8A46_933D][commodity as usize - 1],
        0,
    ));
    calls.push(Call::Allocate(12));
    calls.push(Call::Find(kind, other));
    if existing {
        calls.push(Call::Amount);
    }
    let (buyer, seller) = (selling, !selling);
    let slots = if cost {
        vec![
            (buyer, 0xF0),
            (buyer, 0xD8),
            (seller, 0xF0),
            (seller, 0xD8),
            (buyer, 0xE8),
            (buyer, 0xE0),
            (seller, 0xE8),
            (seller, 0xE0),
        ]
    } else {
        vec![
            (buyer, 0xF0),
            (buyer, 0xD8),
            (buyer, 0xE8),
            (buyer, 0xE0),
            (seller, 0xE8),
            (seller, 0xE0),
            (seller, 0xF0),
            (seller, 0xD8),
        ]
    };
    calls.extend(
        slots
            .into_iter()
            .map(|(city, slot)| Call::Stat(city, slot, commodity)),
    );
    if cost {
        calls.push(Call::Property(0xCA46_9346, 1.0f64.to_bits()));
        calls.push(Call::Property(0x6A46_9342, 1.0f64.to_bits()));
        calls.push(Call::Stat(seller, 0xF8, commodity));
        // Core example: need 500, buyer supply 300, seller supply 2000, seller need 1000.
        let x = if selling {
            [10.01f32, -1.0f32]
        } else {
            [-1.99f32, 0.45f32]
        };
        calls.extend(x.map(|input| Call::Curve(sim + 0x2C, input.to_bits())));
    }
    calls.push(Call::Destroy);
    calls
}

#[test]
fn core_price_and_shared_garbage_curve() {
    let mut w = World::new();
    let (result, registers, calls) = w.call(true, 0x4773_FFE0, 100.0, false);
    assert_eq!(registers, RESULT);
    STATE.with(|s| assert_eq!(s.borrow().destructor_registers.1, 0xDDEE_FF00));
    let price = result.to_f64();
    assert!((price / 100.0 - 2.631_428_6).abs() < 1e-6, "{price}");
    assert_eq!(result.ftol(), 263);
    assert_eq!(
        calls,
        expected_calls(
            true,
            false,
            1,
            0x4773_FFE0,
            w.other.ptr() as u32,
            w.sim.ptr() as u32,
            false
        )
    );
    STATE.with(|s| s.borrow_mut().garbage = true);
    let (result, _, _) = w.call(true, 0x4773_FFE0, 100.0, false);
    let price = result.to_f64();
    assert!((price / 100.0 - 2.215_041_8).abs() < 1e-6, "{price}");
    assert_eq!(result.ftol(), 221);
}

#[test]
fn core_maximum_amount_and_first_matching_deal() {
    let mut w = World::new();
    for (existing, demand, want) in [
        (None, 1000.0, 950.0),
        (Some(100), 1000.0, 1050.0),
        (Some(0), 1900.0, 100.0),
        (Some(-100), 1000.0, 850.0),
    ] {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.existing = existing;
            s.figures[1][0] = F80::from_f64(demand);
        });
        let (value, registers, calls) = w.call(false, 0x4773_FFE0, 0.0, false);
        assert_eq!(registers, RESULT);
        STATE.with(|s| assert_eq!(s.borrow().destructor_registers.1, 0xBBCC_DDEE));
        assert_eq!(value.to_f64(), want);
        assert_eq!(
            calls,
            expected_calls(
                false,
                false,
                1,
                0x4773_FFE0,
                w.other.ptr() as u32,
                w.sim.ptr() as u32,
                existing.is_some()
            )
        );
    }
}

#[test]
fn all_six_types_choose_their_commodity_and_parties() {
    let mut w = World::new();
    for offset in 0..6 {
        let kind = 0x4773_FFE0 + offset;
        let (value, _, calls) = w.call(false, kind, 0.0, false);
        assert_eq!(value.to_f64(), if offset & 1 == 0 { 950.0 } else { 0.0 });
        assert_eq!(
            calls,
            expected_calls(
                false,
                offset & 1 != 0,
                offset / 2 + 1,
                kind,
                w.other.ptr() as u32,
                w.sim.ptr() as u32,
                false
            )
        );
        let (_, _, calls) = w.call(true, kind, 100.0, false);
        assert_eq!(
            calls,
            expected_calls(
                true,
                offset & 1 != 0,
                offset / 2 + 1,
                kind,
                w.other.ptr() as u32,
                w.sim.ptr() as u32,
                false
            )
        );
    }
}

#[test]
fn early_returns_still_query_current_city_and_cost_scale() {
    let mut w = World::new();
    let (value, registers, calls) = w.call(false, 0x4773_FFE0, 0.0, true);
    assert_eq!(value, F80::ZERO);
    assert_eq!(registers, pair(0, 0x1122_3344));
    assert_eq!(calls, [Call::Current]);
    for amount in [-1.0, -0.0, 0.0, 1e-9, f64::NEG_INFINITY] {
        let (value, registers, calls) = w.call(true, 0x4773_FFE0, amount, false);
        assert_eq!(value, F80::ZERO);
        assert_eq!(registers >> 32, PROPERTY_RESULT >> 32);
        assert_eq!(
            registers as u32 & 0xFFFF_0000,
            PROPERTY_RESULT as u32 & 0xFFFF_0000
        );
        assert_eq!(registers as u32 & 0x4500, 0x0100);
        assert_eq!(
            calls,
            [
                Call::Current,
                Call::Holder(0x1C),
                Call::Property(0xEA4A_D2FF, u64::from(1.0f32.to_bits()))
            ]
        );
    }
    for amount in [1e-8, f64::INFINITY, f64::NAN] {
        let (_, registers, calls) = w.call(true, 0x4773_FFE0, amount, false);
        assert_eq!(registers, RESULT);
        assert_eq!(calls.last(), Some(&Call::Destroy));
    }
}

#[test]
fn allocator_cache_is_filled_before_releasing_the_temporary() {
    let mut w = World::new();
    for cost in [false, true] {
        w._data.1 = core::ptr::null_mut();
        let (_, _, calls) = w.call(cost, 0x4773_FFE0, 100.0, false);
        let index = calls.iter().position(|c| *c == Call::Framework).unwrap();
        assert_eq!(
            &calls[index..index + 4],
            &[
                Call::Framework,
                Call::Service(0x3AE4_BEA3, 0x3AE4_BEAB),
                Call::ReleaseLocal,
                Call::Allocate(12)
            ]
        );
        assert_ne!(w._data.1, core::ptr::null_mut());
        let (_, _, calls) = w.call(cost, 0x4773_FFE0, 100.0, false);
        assert!(!calls.contains(&Call::Framework));
    }
}

#[test]
fn defaults_and_unknown_types_retain_the_original_fallbacks() {
    let mut w = World::new();
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.scale = None;
        s.base = None;
        s.minimum = None;
        s.maximum = None;
        s.figures[0][4] = F80::from_f64(3.0);
    });
    for cost in [false, true] {
        let (value, _, calls) = w.call(cost, 0x1234, 100.0, false);
        assert_eq!(value.to_f64(), if cost { 600.0 } else { 0.0 });
        assert!(!calls
            .iter()
            .any(|c| matches!(c, Call::Property(0xEA46_932C | 0x6A46_9337 | 0x8A46_933D, _))));
        for call in calls {
            if let Call::Stat(_, _, commodity) = call {
                assert_eq!(commodity, if cost { w.sim.ptr() as u32 } else { 1 });
            }
        }
    }
}

#[test]
fn maximum_uses_buyer_floor_seller_surplus_and_zero_clamp() {
    let mut w = World::new();
    for (buyer_demand, seller_supply, seller_demand, want) in [
        (0.0, 2000.0, 1000.0, 702.5),
        (500.0, 900.0, 1000.0, 0.0),
        (500.0, 2000.0, 1050.0, 950.0),
    ] {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.figures[0][0] = F80::from_f64(buyer_demand);
            s.figures[1][0] = F80::from_f64(seller_demand);
            s.figures[1][2] = F80::from_f64(seller_supply);
        });
        assert_eq!(w.call(false, 0x4773_FFE0, 0.0, false).0.to_f64(), want);
    }
}

#[test]
fn price_clamps_and_returns_the_unrounded_final_product() {
    let mut w = World::new();
    for (curve, want) in [(-10.0, 220.0), (10.0, 300.0)] {
        STATE.with(|s| s.borrow_mut().curve = vec![F80::from_f64(curve); 2]);
        assert_eq!(w.call(true, 0x4773_FFE0, 100.0, false).0.to_f64(), want);
    }
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.minimum = Some(0.0);
        s.maximum = Some(0.0);
        s.figures[1][4] = F80::from_f64(1.1e30);
    });
    let result = w.call(true, 0x4773_FFE0, 1.1e30, false).0;
    assert!(result.to_f64().is_finite());
    assert!(result.to_f32().is_infinite());
}

#[test]
fn unordered_values_are_not_replaced_by_ordered_clamps() {
    let mut w = World::new();
    STATE.with(|s| s.borrow_mut().figures[0][0] = F80::from_f64(f64::NAN));
    assert!(w.call(false, 0x4773_FFE0, 0.0, false).0.to_f64().is_nan());
    STATE.with(|s| s.borrow_mut().curve = vec![F80::from_f64(f64::NAN); 2]);
    assert!(w.call(true, 0x4773_FFE0, 100.0, false).0.to_f64().is_nan());
}

#[test]
fn cost_reloads_amount_after_city_and_destructor_callbacks() {
    let mut w = World::new();
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.replace_amount = Some(0.0);
        s.final_amount = Some(200.0);
        s.curve = vec![F80::ZERO; 2];
    });
    let (value, _, calls) = w.call(true, 0x4773_FFE0, 100.0, false);
    assert_eq!(value.to_f64(), 440.0);
    let inputs: Vec<_> = calls
        .iter()
        .filter_map(|c| {
            if let Call::Curve(_, input) = c {
                Some(f32::from_bits(*input))
            } else {
                None
            }
        })
        .collect();
    assert_eq!(inputs, [-1.99e10f32, 0.5]);
}

#[test]
fn maximum_compares_the_wide_floor_before_loading_its_float_image() {
    let mut w = World::new();
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.base = Some(1e30);
        s.scale = Some(1e-30);
        s.figures[0][0] = F80::from_f64(1e80);
        s.figures[0][2] = F80::ZERO;
        s.figures[1][0] = F80::ZERO;
        s.figures[1][2] = F80::from_f64(1e80);
    });
    // The floor's stored f32 is infinity, but its unrounded value is below the buyer's demand.
    let result = w.call(false, 0x4773_FFE0, 0.0, false).0.to_f64();
    assert!(result.is_finite());
    assert!(result > 9.9e49 && result < 1.01e50, "{result}");
}

#[test]
fn zero_scale_and_unordered_surplus_follow_the_comparison_branches() {
    let mut w = World::new();
    STATE.with(|s| s.borrow_mut().scale = Some(0.0));
    assert!(w.call(false, 0x4773_FFE0, 0.0, false).0.to_f64().is_nan());
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.scale = Some(2.5);
        s.figures[1][2] = F80::from_f64(f64::NAN);
    });
    assert_eq!(w.call(false, 0x4773_FFE0, 0.0, false).0.to_f64(), 950.0);
}

#[test]
fn minimum_then_maximum_clamps_keep_reversed_price_bounds() {
    let mut w = World::new();
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.minimum = Some(2.0);
        s.maximum = Some(-1.0);
        s.curve = vec![F80::from_f32(0.5); 2];
    });
    assert_eq!(w.call(true, 0x4773_FFE0, 100.0, false).0.to_f64(), 0.0);
}

#[test]
fn nonzero_imports_exports_and_existing_amount_feed_both_curve_inputs() {
    let mut w = World::new();
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.base = Some(0.0);
        s.scale = Some(2.0);
        s.existing = Some(4);
        s.figures = [
            [20.0, 5.0, 10.0, 3.0, 0.0].map(F80::from_f64),
            [11.0, 7.0, 40.0, 2.0, 3.0].map(F80::from_f64),
        ];
        s.curve = [0.25, 0.75].map(F80::from_f32).to_vec();
    });
    let (value, _, calls) = w.call(true, 0x4773_FFE0, 2.0, false);
    assert_eq!(value.to_f64(), f64::from(7.8f32));
    let inputs: Vec<_> = calls
        .iter()
        .filter_map(|c| {
            if let Call::Curve(_, input) = c {
                Some(f32::from_bits(*input))
            } else {
                None
            }
        })
        .collect();
    assert_eq!(inputs, [-7.5, 0.619_047_64]);
    assert_eq!(w.call(false, 0x4773_FFE0, 0.0, false).0.to_f64(), 28.0);
}
