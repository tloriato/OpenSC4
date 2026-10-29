// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors
//! Money-flow, rate and monthly-total cases from the core budget tests and b15 conformance vectors,
//! on the game's object layout.

use super::*;
use crate::testing::{fake, fake_data, GameFpu, Object};
use crate::x87::F80;
use std::cell::RefCell;
use std::collections::VecDeque;

#[derive(Debug, PartialEq, Eq)]
struct Call(&'static str, usize, Vec<u32>);

#[derive(Default)]
struct State {
    calls: Vec<Call>,
    message: *mut u8,
    sim: *mut u8,
    department: *mut u8,
    node: *mut u8,
    next_table: u32,
    next_server: *mut u8,
    difficulties: VecDeque<u32>,
}
thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}
fn reset() {
    STATE.with(|s| *s.borrow_mut() = State::default());
}
fn log(name: &'static str, this: *mut u8, args: &[u32]) {
    STATE.with(|s| s.borrow_mut().calls.push(Call(name, this as usize, args.to_vec())));
}
fn calls() -> Vec<Call> {
    STATE.with(|s| std::mem::take(&mut s.borrow_mut().calls))
}
fn event(name: &'static str, this: *mut u8, args: &[u32]) -> Call {
    Call(name, this as usize, args.to_vec())
}
fn table(slots: &[(usize, usize)]) -> Box<[usize; 64]> {
    let mut t = Box::new([0; 64]);
    for &(offset, address) in slots {
        t[offset / 4] = address;
    }
    t
}
unsafe fn put<T>(p: *mut u8, offset: usize, value: T) {
    // SAFETY: every test allocates storage covering this field.
    unsafe { write_unaligned(p.add(offset).cast::<T>(), value) };
}
unsafe fn bytes(p: *mut u8, size: usize) -> Vec<u8> {
    // SAFETY: the test's object covers size bytes.
    unsafe { core::slice::from_raw_parts(p, size) }.to_vec()
}
unsafe fn invoke(address: u32, this: *mut u8, args: &[u32], eax: u32, edx: u32) -> u64 {
    let mut call = RawCall::new(Conv::Thiscall, crate::testing::resolve(address), this as u32, args);
    call.eax = eax;
    call.edx = edx;
    // SAFETY: every test supplies the original method's argument words.
    unsafe { call_raw(&mut call) };
    pair(call.eax, call.edx)
}
fn words(n: i64) -> [u32; 2] {
    [n as u32, (n as u64 >> 32) as u32]
}

extern "thiscall" fn add_ref(this: *mut u8) -> u64 {
    log("add_ref", this, &[]);
    STATE.with(|s| {
        let sim = s.borrow().sim;
        if !sim.is_null() {
            // SAFETY: the test uses +0x40 for its output word.
            assert!(!unsafe { field::<*mut u8>(sim, 0x40) }.is_null());
        }
    });
    pair(0xAABB_CCDD, 0x1234_5678)
}
extern "thiscall" fn shutdown(this: *mut u8) -> u64 {
    log("shutdown", this, &[]);
    STATE.with(|s| {
        // SAFETY: this is the test simulator; its second table lives for the test.
        unsafe { put(this, 0, s.borrow().next_table) };
    });
    pair(0x1020_3040, 0x5060_7080)
}
extern "thiscall" fn delete(this: *mut u8, flag: u32) -> u64 {
    log("delete", this, &[flag]);
    pair(0x89AB_CDEF, 0xFEDC_BA98)
}

#[test]
fn interfaces_write_before_addref_and_keep_upper_result_bytes() {
    reset();
    let vt = table(&[(4, add_ref as *const () as usize)]);
    let mut object = Object::new(0x860);
    let p = object.ptr();
    // SAFETY: the test owns the complete simulator and its output word.
    unsafe {
        put(p, 0, vt.as_ptr());
        STATE.with(|s| s.borrow_mut().sim = p);
        for (iid, offset) in [(1, 0), (0x6772_BD23, 0), (0x4522_94AA, 4), (0xE4FD_A3D4, 8)] {
            put(p, 0x40, 0xDEAD_BEEFu32);
            assert_eq!(
                invoke(0x0068_FC80, p, &[iid, p.add(0x40) as u32], 0x1122_3344, 0x8765_4321),
                pair(0xAABB_CC01, 0x1234_5678)
            );
            assert_eq!(field::<*mut u8>(p, 0x40), p.add(offset));
            assert_eq!(calls(), [event("add_ref", p, &[])]);
        }
        for iid in [
            0u32,
            2,
            0x4522_94A9,
            0x4522_94AB,
            0x6772_BD22,
            0x6772_BD24,
            0xE4FD_A3D3,
            u32::MAX,
        ] {
            put(p, 0x40, 0xDEAD_BEEFu32);
            let before = bytes(p, 0x860);
            let expected = if iid > 0x6772_BD23 {
                iid
            } else {
                iid.wrapping_sub(0x4522_94AA)
            } & !0xFF;
            assert_eq!(
                invoke(0x0068_FC80, p, &[iid, p.add(0x40) as u32], 0, 0x8765_4321),
                pair(expected, 0x8765_4321)
            );
            assert_eq!(bytes(p, 0x860), before);
            assert!(calls().is_empty());
        }
    }
}

#[test]
fn release_uses_signed_reference_counts_and_rereads_the_table() {
    reset();
    let first = table(&[(0x10, shutdown as *const () as usize)]);
    let second = table(&[(0xEC, delete as *const () as usize)]);
    let mut object = Object::new(0x860);
    let p = object.ptr();
    // SAFETY: the test's simulator and lifetime slots.
    unsafe {
        let sim = BudgetSim::from_ptr(p);
        STATE.with(|s| s.borrow_mut().next_table = second.as_ptr() as u32);
        for n in [i32::MIN, -1, 0, 1, 2, i32::MAX] {
            sim.set_vtable(first.as_ptr() as u32);
            sim.set_references(n);
            let result = invoke(0x0069_00C0, p, &[], 0, 0x1234_5678);
            if n > 1 {
                assert_eq!(result, pair((n - 1) as u32, 0x1234_5678));
                assert_eq!(sim.references(), n - 1);
                assert!(calls().is_empty());
            } else {
                assert_eq!(result, pair(0, 0xFEDC_BA98));
                assert_eq!(sim.references(), n);
                assert_eq!(calls(), [event("shutdown", p, &[]), event("delete", p, &[1])]);
            }
        }
    }
}

extern "C" fn allocate(size: u32) -> *mut u8 {
    log("allocate", core::ptr::null_mut(), &[size]);
    STATE.with(|s| s.borrow().message)
}
extern "thiscall" fn construct(this: *mut u8) -> *mut u8 {
    log("construct", this, &[]);
    this
}
extern "thiscall" fn set_type(this: *mut u8, id: u32) -> u32 {
    log("type", this, &[id]);
    7
}
extern "thiscall" fn set_pointer(this: *mut u8, sim: *mut u8) -> u32 {
    log("pointer", this, &[sim as u32]);
    8
}
extern "thiscall" fn set_unknown(this: *mut u8, sim: *mut u8) -> u32 {
    log("unknown", this, &[sim as u32]);
    STATE.with(|s| {
        let next = s.borrow().next_server;
        if !next.is_null() {
            // SAFETY: the test's live simulator.
            unsafe { BudgetSim::from_ptr(sim) }.set_messages(next);
        }
    });
    9
}
extern "thiscall" fn post(this: *mut u8, message: *mut u8, target: u32) -> u32 {
    log("post", this, &[message as u32, target]);
    10
}
extern "thiscall" fn message_release(this: *mut u8) -> u64 {
    log("message_release", this, &[]);
    pair(0x4567_89AB, 0xCDEF_0123)
}

#[test]
fn funds_message_payload_call_order_and_server_reread() {
    reset();
    let mut object = Object::new(0x860);
    let p = object.ptr();
    // SAFETY: the test simulator has no server, so no further objects are needed.
    unsafe {
        assert_eq!(
            invoke(0x0068_FE10, p, &[], 0xAABB_CCDD, 0x1122_3344),
            pair(0, 0x1122_3344)
        );
    }
    assert!(calls().is_empty());
    let message_vt = table(&[
        (4, add_ref as *const () as usize),
        (8, message_release as *const () as usize),
        (0x14, set_type as *const () as usize),
        (0x30, set_pointer as *const () as usize),
        (0x84, set_unknown as *const () as usize),
    ]);
    let server_vt = table(&[(0x10, post as *const () as usize)]);
    let mut message = Object::new(0x2C);
    let m = message.ptr();
    let mut server = [server_vt.as_ptr() as u32];
    let s = server.as_mut_ptr().cast();
    let mut first_server = [0u32];
    let _a = fake(0x0091_33DA, allocate as *const () as usize);
    let _c = fake(0x0091_34D6, construct as *const () as usize);
    STATE.with(|st| {
        let mut st = st.borrow_mut();
        st.message = m;
        st.next_server = s;
    });
    // SAFETY: all interfaces and allocator results are backed by the test's storage.
    unsafe {
        put(m, 0, message_vt.as_ptr());
        BudgetSim::from_ptr(p).set_messages(first_server.as_mut_ptr().cast());
        assert_eq!(invoke(0x0068_FE10, p, &[], 0, 0), pair(0x4567_89AB, 0xCDEF_0123));
    }
    assert_eq!(
        calls(),
        [
            event("allocate", core::ptr::null_mut(), &[0x2C]),
            event("construct", m, &[]),
            event("add_ref", m, &[]),
            event("type", m, &[0x0772_FAD4]),
            event("pointer", m, &[p as u32]),
            event("unknown", m, &[p as u32]),
            event("post", s, &[m as u32, 0]),
            event("message_release", m, &[])
        ]
    );
}

extern "fastcall" fn post_balance(this: *mut u8, edx: u32) -> u64 {
    // SAFETY: the test's simulator.
    let sim = unsafe { BudgetSim::from_ptr(this) };
    assert_eq!(sim.funds(), sim.shadow_funds());
    log(
        "funds",
        this,
        &[edx, sim.funds() as u32, (sim.funds() as u64 >> 32) as u32],
    );
    pair(0x1234_56AB, 0x789A_BCDE)
}
extern "thiscall" fn deposit(this: *mut u8, amount: i64) -> u64 {
    log("deposit", this, &words(amount));
    pair(0xABCD_EF80, 0x8765_4321)
}

#[test]
fn set_funds_posts_only_on_change_and_does_not_repair_an_unchanged_shadow() {
    reset();
    let _post = fake(0x0068_FE10, post_balance as *const () as usize);
    let mut object = Object::new(0x860);
    let p = object.ptr();
    // SAFETY: the test's complete simulator and fake notification method.
    unsafe {
        let sim = BudgetSim::from_ptr(p);
        for amount in [100, -50, 0x1234_5678_8765_4321, i64::MIN, i64::MAX] {
            sim.set_funds(amount.wrapping_add(1));
            sim.set_shadow_funds(999);
            let before = bytes(p, 0x860);
            assert_eq!(
                invoke(0x0069_0140, p, &words(amount), 0, 0),
                pair(0x1234_56AB, 0x789A_BCDE)
            );
            assert_eq!((sim.funds(), sim.shadow_funds()), (amount, amount));
            let mut expected = before;
            expected[0x28..0x30].copy_from_slice(&amount.to_le_bytes());
            expected[0x30..0x38].copy_from_slice(&amount.to_le_bytes());
            assert_eq!(bytes(p, 0x860), expected);
            assert_eq!(
                calls(),
                [event(
                    "funds",
                    p,
                    &[
                        (amount as u64 >> 32) as u32,
                        amount as u32,
                        (amount as u64 >> 32) as u32
                    ]
                )]
            );
            sim.set_shadow_funds(999);
            let before = bytes(p, 0x860);
            assert_eq!(invoke(0x0069_0140, p, &words(amount), 0, 0), amount as u64);
            assert_eq!(bytes(p, 0x860), before);
            assert!(calls().is_empty());
        }
    }
}

#[test]
fn withdrawal_zero_insufficient_exact_balance_and_negative_delegation() {
    reset();
    let _post = fake(0x0068_FE10, post_balance as *const () as usize);
    let vt = table(&[(0x1C, deposit as *const () as usize)]);
    let mut object = Object::new(0x860);
    let p = object.ptr();
    // SAFETY: the test's complete simulator and money-flow slots.
    unsafe {
        let sim = BudgetSim::from_ptr(p);
        sim.set_vtable(vt.as_ptr() as u32);
        for (funds, amount) in [(100, 25), (100, 100), (i64::MAX, 0x1_0000_0001), (0x1_0000_0000, 1)] {
            sim.set_funds(funds);
            sim.set_shadow_funds(999);
            assert_eq!(
                invoke(0x0069_0200, p, &words(amount), 0, 0),
                pair(0x1234_5601, 0x789A_BCDE)
            );
            assert_eq!((sim.funds(), sim.shadow_funds()), (funds - amount, funds - amount));
            assert_eq!(calls().len(), 1);
        }
        for (funds, amount) in [(100, 101), (-1, 1), (0, 1), (0, 0x1234_5678_0000_0000)] {
            sim.set_funds(funds);
            let before = bytes(p, 0x860);
            assert_eq!(
                invoke(0x0069_0200, p, &words(amount), 0, 0),
                pair((amount as u64 >> 32) as u32 & !0xFF, (funds as u64 >> 32) as u32)
            );
            assert_eq!(bytes(p, 0x860), before);
            assert!(calls().is_empty());
        }
        let before = bytes(p, 0x860);
        assert_eq!(invoke(0x0069_0200, p, &words(0), 0, 0xDEAD_BEEF), pair(1, 0xDEAD_BEEF));
        assert_eq!(bytes(p, 0x860), before);
        for amount in [-1, -0x1234_5678_0000_0000, i64::MIN] {
            assert_eq!(
                invoke(0x0069_0200, p, &words(amount), 0, 0),
                pair(0xABCD_EF80, 0x8765_4321)
            );
            assert_eq!(calls(), [event("deposit", p, &words(amount.wrapping_neg()))]);
            assert_eq!(bytes(p, 0x860), before);
        }
    }
}

extern "thiscall" fn difficulty(this: *mut u8) -> u32 {
    log("difficulty", this, &[]);
    STATE.with(|s| s.borrow_mut().difficulties.pop_front().unwrap())
}

#[test]
fn starting_funds_follow_difficulty_and_always_post() {
    reset();
    let _post = fake(0x0068_FE10, post_balance as *const () as usize);
    let city_vt = table(&[(0x60, difficulty as *const () as usize)]);
    let mut city = [city_vt.as_ptr() as u32];
    let c = city.as_mut_ptr().cast();
    let mut amounts = [500_000i64, 200_000, 100_000];
    let mut object = Object::new(0x860);
    let p = object.ptr();
    // SAFETY: the test city and tuning arrays remain live.
    unsafe {
        let sim = BudgetSim::from_ptr(p);
        sim.set_city(c);
        sim.set_starting_funds(amounts.as_mut_ptr());
        for (i, &amount) in amounts.iter().enumerate() {
            sim.set_funds(amount);
            sim.set_shadow_funds(999);
            STATE.with(|s| s.borrow_mut().difficulties.push_back(i as u32));
            invoke(0x0069_0450, p, &[], 0, 0);
            assert_eq!((sim.funds(), sim.shadow_funds()), (amount, amount));
            assert_eq!(
                calls(),
                [
                    event("difficulty", c, &[]),
                    event("funds", p, &[amount as u32, amount as u32, 0])
                ]
            );
        }
    }
}

#[test]
fn getters_keep_full_64_bit_values_and_signed_tax_indexing() {
    reset();
    let _fpu = GameFpu::new();
    let mut zero = 0.0f32;
    let _zero = fake_data(0x00A8_1054, (&mut zero as *mut f32).cast());
    // Prefix storage makes tax index -1 a valid record for this test.
    let mut object = Object::new(0x960);
    let p = object.ptr().wrapping_add(0x100);
    // SAFETY: all selected positive and negative tax records are backed by the test allocation.
    unsafe {
        let sim = BudgetSim::from_ptr(p);
        sim.set_funds(i64::MIN + 0x1234_5678);
        sim.set_tax_income(i64::MAX - 0x8765_4321);
        assert_eq!(invoke(0x0068_FD10, p, &[], 0, 0), sim.funds() as u64);
        assert_eq!(invoke(0x0068_FD30, p, &[], 0, 0), sim.tax_income() as u64);
        assert_eq!(invoke(0x0068_FD40, p, &[0xABCD_EF01], 0xFFFF_FFFF, 0xFFFF_FFFF), 0);
        for index in [-1i32, 0, 11, 12, i32::MAX] {
            if index < 12 {
                sim.tax_group(index).set_assessed(-0x1234_5678_1234);
                sim.tax_group(index).set_rate(7.25);
            }
            let before = bytes(p.sub(0x100), 0x960);
            assert_eq!(
                invoke(0x0068_FDE0, p, &[index as u32], 0, 0),
                if index < 12 { -0x1234_5678_1234i64 as u64 } else { 0 }
            );
            let args = [index as u32];
            let mut call = RawCall::new(
                Conv::Thiscall,
                access::fc4p_0068fd50 as *const () as usize,
                p as u32,
                &args,
            );
            call.edx = 0x1122_3344;
            call.take_st0 = 1;
            call_raw(&mut call);
            assert_eq!(
                call.eax,
                if index < 12 {
                    index.wrapping_shl(7) as u32
                } else {
                    index as u32
                }
            );
            assert_eq!(call.edx, 0x1122_3344);
            assert_eq!(call.st0, F80::from_f32(if index < 12 { 7.25 } else { 0.0 }));
            assert_eq!(bytes(p.sub(0x100), 0x960), before);
        }
    }
    assert!(calls().is_empty());
}

extern "fastcall" fn multiply(ecx: u32, edx: u32, left: i64, right: i64) -> i64 {
    log(
        "multiply",
        ecx as *mut u8,
        &[edx, words(left)[0], words(left)[1], words(right)[0], words(right)[1]],
    );
    left.wrapping_mul(right)
}
#[test]
fn borrowing_limit_calls_the_game_integer_helper() {
    reset();
    let _m = fake(0x009F_0D10, multiply as *const () as usize);
    let mut object = Object::new(0x860);
    let p = object.ptr();
    // SAFETY: the test's simulator and multiplication helper.
    unsafe {
        let sim = BudgetSim::from_ptr(p);
        for (multiplier, base) in [
            (10, 200_000),
            (12, 21_000),
            (0, 123),
            (-3, 0x1234_5678_8765),
            (i32::MIN, i64::MAX),
            (9, -7),
        ] {
            sim.set_borrowing_multiplier(multiplier);
            sim.set_borrowing_base(base);
            assert_eq!(
                invoke(0x0068_FDA0, p, &[], 0, 0) as i64,
                i64::from(multiplier).wrapping_mul(base)
            );
            assert_eq!(
                calls(),
                [event(
                    "multiply",
                    base as u32 as *mut u8,
                    &[
                        (multiplier >> 31) as u32,
                        multiplier as u32,
                        (multiplier >> 31) as u32,
                        base as u32,
                        (base as u64 >> 32) as u32
                    ]
                )]
            );
        }
    }
}

extern "thiscall" fn history(this: *mut u8, id: u32, value: u32) -> u64 {
    log("history", this, &[id, value]);
    pair(0xAABB_CCDD, 0x1234_5678)
}
#[test]
fn daily_rates_use_biased_rounding_for_all_twelve_history_ids() {
    reset();
    let _fpu = GameFpu::new();
    let vt = table(&[(0x30, history as *const () as usize)]);
    let mut service = [vt.as_ptr() as u32];
    let h = service.as_mut_ptr().cast::<u8>();
    let mut global = h;
    let mut scale = 10.0f32;
    let mut bias = 12_582_912.0f32;
    let _h = fake_data(0x00B4_3DC0, (&mut global as *mut *mut u8).cast());
    let _s = fake_data(0x00A9_4D50, (&mut scale as *mut f32).cast());
    let _b = fake_data(0x00AB_0888, (&mut bias as *mut f32).cast());
    let mut object = Object::new(0x860);
    let p = object.ptr();
    let cases = [
        (9.25, 92u32),
        (9.35, 94),
        (2.5, 25),
        (30.0, 300),
        (9.15, 92),
        (9.05, 90),
        (9.0, 90),
        (9.0, 90),
        (9.0, 90),
        (9.0, 90),
        (0.25, 2),
        (0.35, 4),
    ];
    // SAFETY: the test's simulator and history interface.
    unsafe {
        let sim = BudgetSim::from_ptr(p);
        for (i, &(rate, _)) in cases.iter().enumerate() {
            sim.tax_group(i as i32).set_rate(rate);
        }
        let before = bytes(p, 0x860);
        assert_eq!(invoke(0x0069_00F0, p, &[], 0, 0), pair(0xAABB_CCDD, 0x1234_5678));
        assert_eq!(bytes(p, 0x860), before);
    }
    assert_eq!(
        calls(),
        cases
            .iter()
            .enumerate()
            .map(|(i, &(_, value))| event("history", h, &[0xAABA_AA9F + i as u32, value]))
            .collect::<Vec<_>>()
    );
}

#[test]
fn daily_history_keeps_the_float_bits_for_negative_and_nonfinite_rates() {
    reset();
    let _fpu = GameFpu::new();
    let vt = table(&[(0x30, history as *const () as usize)]);
    let mut service = [vt.as_ptr() as u32];
    let h = service.as_mut_ptr().cast::<u8>();
    let mut global = h;
    let mut scale = 10.0f32;
    let mut bias = 12_582_912.0f32;
    let _h = fake_data(0x00B4_3DC0, (&mut global as *mut *mut u8).cast());
    let _s = fake_data(0x00A9_4D50, (&mut scale as *mut f32).cast());
    let _b = fake_data(0x00AB_0888, (&mut bias as *mut f32).cast());
    let mut object = Object::new(0x860);
    let p = object.ptr();
    let cases = [
        (-0.35, (-4i32) as u32),
        (-0.25, (-2i32) as u32),
        (0.0, 0),
        (-0.0, 0),
        (1.05, 10),
        (1.15, 12),
        (f32::MAX, 0x3440_0000),
        (f32::INFINITY, 0x3440_0000),
        (f32::NEG_INFINITY, 0xB440_0000),
        (f32::from_bits(0x7FC0_1234), 0x3480_1234),
        (f32::from_bits(0x7F80_1234), 0x3480_1234),
        (f32::from_bits(1), 0),
    ];
    // SAFETY: the test's simulator and history interface.
    unsafe {
        for (i, &(rate, _)) in cases.iter().enumerate() {
            BudgetSim::from_ptr(p).tax_group(i as i32).set_rate(rate);
        }
        invoke(0x0069_00F0, p, &[], 0, 0);
    }
    assert_eq!(
        calls(),
        cases
            .iter()
            .enumerate()
            .map(|(i, &(_, value))| event("history", h, &[0xAABA_AA9F + i as u32, value]))
            .collect::<Vec<_>>()
    );
}

fn float_environment() -> (u16, u16, u32, u32) {
    let mut cw = 0u16;
    let mut sw = 0u16;
    let mut mxcsr = 0u32;
    let flags: u32;
    // SAFETY: the outputs cover their words; reading the environment does not change it.
    unsafe {
        core::arch::asm!("fnstcw [{cw}]", "fnstsw [{sw}]", "stmxcsr [{mxcsr}]", "pushfd", "pop {flags}",
            cw = in(reg) &mut cw, sw = in(reg) &mut sw, mxcsr = in(reg) &mut mxcsr,
            flags = out(reg) flags);
    }
    (cw, sw & 0x3800, mxcsr, flags & 0x400)
}

#[test]
fn tax_methods_preserve_the_control_word_mxcsr_direction_and_existing_x87_stack() {
    reset();
    let _fpu = GameFpu::new();
    let vt = table(&[
        (0x30, history as *const () as usize),
        (0x60, difficulty as *const () as usize),
    ]);
    let mut service = [vt.as_ptr() as u32];
    let h = service.as_mut_ptr().cast::<u8>();
    let mut global = h;
    let mut scale = 10.0f32;
    let mut bias = 12_582_912.0f32;
    let mut inverse = f32::from_bits(0x3A5A_740E);
    let mut multipliers = [1.0f32; 3];
    let _h = fake_data(0x00B4_3DC0, (&mut global as *mut *mut u8).cast());
    let _s = fake_data(0x00A9_4D50, (&mut scale as *mut f32).cast());
    let _b = fake_data(0x00AB_0888, (&mut bias as *mut f32).cast());
    let _i = fake_data(0x00AB_08B8, (&mut inverse as *mut f32).cast());
    let mut object = Object::new(0x860);
    let p = object.ptr();
    // SAFETY: the test's live city, tax groups and tuning; the sentinel is pushed once and popped once.
    unsafe {
        let sim = BudgetSim::from_ptr(p);
        sim.set_city(h);
        sim.set_tax_multipliers(multipliers.as_mut_ptr());
        sim.tax_group(0).set_rate(9.0);
        sim.tax_group(0).set_assessed(13_000);
        STATE.with(|s| s.borrow_mut().difficulties.push_back(2));
        let sentinel = 123.25f64;
        let mut restored = 0.0f64;
        core::arch::asm!("fld qword ptr [{p}]", p = in(reg) &sentinel, out("st(0)") _);
        let before = float_environment();
        invoke(0x0069_00F0, p, &[], 0, 0);
        let after_daily = float_environment();
        assert_eq!(invoke(0x0069_04C0, p, &[0], 0, 0), 97);
        let after_income = float_environment();
        let args = [0];
        let mut rate = RawCall::new(
            Conv::Thiscall,
            access::fc4p_0068fd50 as *const () as usize,
            p as u32,
            &args,
        );
        rate.take_st0 = 1;
        call_raw(&mut rate);
        let after_rate = float_environment();
        core::arch::asm!("fstp qword ptr [{p}]", p = in(reg) &mut restored, out("st(0)") _);
        assert_eq!([after_daily, after_income, after_rate], [before; 3]);
        assert_eq!(restored.to_bits(), sentinel.to_bits());
        assert_eq!(rate.st0, F80::from_f32(9.0));
    }
    calls();
}

#[test]
fn default_tax_rates_reread_difficulty_and_copy_raw_words() {
    reset();
    let vt = table(&[(0x60, difficulty as *const () as usize)]);
    let mut city = [vt.as_ptr() as u32];
    let c = city.as_mut_ptr().cast();
    let mut rates = [9.0f32.to_bits(), 7.0f32.to_bits(), 0x7F80_0001];
    let mut object = Object::new(0x860);
    let p = object.ptr();
    let indices: Vec<_> = (0..12).map(|i| i % 3).collect();
    STATE.with(|s| s.borrow_mut().difficulties.extend(indices.iter().copied()));
    // SAFETY: the test city, tuning array, and simulator.
    unsafe {
        let sim = BudgetSim::from_ptr(p);
        sim.set_city(c);
        sim.set_default_rates(rates.as_mut_ptr());
        let mut expected = bytes(p, 0x860);
        assert_eq!(invoke(0x0069_0490, p, &[], 0, 0), pair(2, rates[2]));
        for (i, &d) in indices.iter().enumerate() {
            expected[0x70 + 128 * i..0x74 + 128 * i].copy_from_slice(&rates[d as usize].to_le_bytes());
        }
        assert_eq!(bytes(p, 0x860), expected);
    }
    assert_eq!(
        calls(),
        (0..12).map(|_| event("difficulty", c, &[])).collect::<Vec<_>>()
    );
}

#[test]
fn tax_income_operation_order_truncation_and_64_bit_results() {
    reset();
    let _fpu = GameFpu::new();
    let vt = table(&[(0x60, difficulty as *const () as usize)]);
    let mut city = [vt.as_ptr() as u32];
    let c = city.as_mut_ptr().cast();
    let mut multipliers = [1.2f32, 1.1, 1.0];
    let mut inverse = f32::from_bits(0x3A5A_740E);
    let _inv = fake_data(0x00AB_08B8, (&mut inverse as *mut f32).cast());
    let mut object = Object::new(0x860);
    let p = object.ptr();
    // SAFETY: the test's city, rates, and tuning.
    unsafe {
        let sim = BudgetSim::from_ptr(p);
        sim.set_city(c);
        sim.set_tax_multipliers(multipliers.as_mut_ptr());
        for (index, diff, assessed, rate, expected) in [
            (0, 0, 998_857, 7.0, 6992i64),
            (0, 0, 10_000, 9.0, 90),
            (0, 1, 10_000, 9.0, 82),
            (0, 2, 10_000, 9.0, 75),
            (0, 2, 10_000, 12.0, 100),
            (3, 2, 13_000, 9.0, 97),
            (11, 2, -13_000, 9.0, -97),
            (7, 2, 4_000_000_000_000, 9.0, 30_000_001_024),
            (1, 2, 1, f32::NAN, i64::MIN),
            (2, 2, i64::MAX, f32::MAX, i64::MIN),
        ] {
            sim.tax_group(index).set_assessed(assessed);
            sim.tax_group(index).set_rate(rate);
            STATE.with(|s| s.borrow_mut().difficulties.push_back(diff));
            let before = bytes(p, 0x860);
            assert_eq!(invoke(0x0069_04C0, p, &[index as u32], 0, 0) as i64, expected);
            assert_eq!(bytes(p, 0x860), before);
            assert_eq!(calls(), [event("difficulty", c, &[])]);
        }
    }
}

extern "thiscall" fn department_lookup(this: *mut u8, id: u32) -> u64 {
    log("department", this, &[id]);
    STATE.with(|s| pair(s.borrow().department as u32, 0xAABB_CCDD))
}
extern "thiscall" fn line_lookup(this: *mut u8, id: u32) -> u64 {
    log("line", this, &[id]);
    pair(0x1234_5678, 0x9ABC_DEF0)
}
extern "thiscall" fn map_find(this: *mut u8, out: *mut *mut u8, key: *const u32) -> u64 {
    // SAFETY: the lookup receives the caller's live key and result words.
    unsafe {
        log("find", this, &[*key]);
        STATE.with(|s| out.write(s.borrow().node));
    }
    pair(0x1234_5678, 0x9ABC_DEF0)
}
extern "thiscall" fn department_group(this: *mut u8) -> u32 {
    log("group", this, &[]);
    0x1122_3344
}
extern "thiscall" fn changed(this: *mut u8, group: u32) -> u64 {
    log("changed", this, &[group]);
    pair(0x5566_7788, 0x99AA_BBCC)
}
#[test]
fn item_line_lookup_does_not_retain_borrowed_interfaces() {
    reset();
    let sim_vt = table(&[(0x70, department_lookup as *const () as usize)]);
    let dept_vt = table(&[(0x50, line_lookup as *const () as usize)]);
    let mut object = Object::new(0x860);
    let p = object.ptr();
    let mut department = [dept_vt.as_ptr() as u32];
    let d = department.as_mut_ptr().cast();
    let mut item = [77u32, 99];
    // SAFETY: test simulator, department and item.
    unsafe {
        BudgetSim::from_ptr(p).set_vtable(sim_vt.as_ptr() as u32);
        STATE.with(|s| s.borrow_mut().department = d);
        assert_eq!(
            invoke(0x0068_FF80, p, &[item.as_mut_ptr() as u32], 0, 0),
            pair(0x1234_5678, 0x9ABC_DEF0)
        );
        assert_eq!(calls(), [event("department", p, &[77]), event("line", d, &[99])]);
        STATE.with(|s| s.borrow_mut().department = core::ptr::null_mut());
        assert_eq!(
            invoke(0x0068_FF80, p, &[item.as_mut_ptr() as u32], 0, 0),
            pair(0, 0xAABB_CCDD)
        );
        assert_eq!(calls(), [event("department", p, &[77])]);
    }
}
#[test]
fn department_map_lookup_and_funding_change_null_path() {
    reset();
    let _find = fake(0x0049_7520, map_find as *const () as usize);
    let _changed = fake(0x0069_0E80, changed as *const () as usize);
    let dept_vt = table(&[(0x18, department_group as *const () as usize)]);
    let mut department = [dept_vt.as_ptr() as u32];
    let d = department.as_mut_ptr().cast();
    let mut object = Object::new(0x860);
    let p = object.ptr();
    let mut header = Object::new(0x18);
    let mut node = Object::new(0x18);
    // SAFETY: the test's map and department.
    unsafe {
        BudgetSim::from_ptr(p).set_departments(header.ptr());
        put(node.ptr(), 0x14, d);
        for (n, expected) in [(header.ptr(), 0), (node.ptr(), d as u32)] {
            STATE.with(|s| s.borrow_mut().node = n);
            assert_eq!(
                invoke(0x0069_1000, p, &[0x8765_4321], 0, 0),
                pair(expected, 0x9ABC_DEF0)
            );
            assert_eq!(calls(), [event("find", p.add(0x54), &[0x8765_4321])]);
        }
        assert_eq!(
            invoke(0x0069_1030, p, &[0], 0x1234_5678, 0x90AB_CDEF),
            pair(0x1234_5678, 0x90AB_CDEF)
        );
        assert!(calls().is_empty());
        assert_eq!(
            invoke(0x0069_1030, p, &[d as u32], 0, 0),
            pair(0x5566_7788, 0x99AA_BBCC)
        );
        assert_eq!(calls(), [event("group", d, &[]), event("changed", p, &[0x1122_3344])]);
    }
}

#[unsafe(naked)]
unsafe extern "C" fn missing_group_stack_result() {
    core::arch::naked_asm!("lea eax, [esp - 0x14]", "mov edx, 0x99aabbcc", "ret 4");
}

#[unsafe(naked)]
unsafe extern "thiscall" fn funding_result_offset(_this: *mut u8, _department: *mut u8) -> u64 {
    core::arch::naked_asm!("push [esp + 4]", "call {port}", "sub eax, esp", "ret 4",
        port = sym access::fc4p_00691030);
}

#[test]
fn funding_change_keeps_the_missing_groups_stack_address_result() {
    reset();
    let _changed = fake(0x0069_0E80, missing_group_stack_result as *const () as usize);
    let dept_vt = table(&[(0x18, department_group as *const () as usize)]);
    let mut department = [dept_vt.as_ptr() as u32];
    let d = department.as_mut_ptr().cast();
    let mut object = Object::new(0x860);
    let p = object.ptr();
    // SAFETY: the test's department and group helper, with the original stack-address return convention.
    unsafe {
        assert_eq!(funding_result_offset(p, d), pair((-40i32) as u32, 0x99AA_BBCC));
    }
    assert_eq!(calls(), [event("group", d, &[])]);
}

extern "thiscall" fn ordinance_expense(this: *mut u8) -> i64 {
    log("ordinance_expense", this, &[]);
    -1
}
extern "thiscall" fn ordinance_income(this: *mut u8) -> i64 {
    log("ordinance_income", this, &[]);
    31
}
extern "thiscall" fn neighbor_expense(this: *mut u8) -> i64 {
    log("neighbor_expense", this, &[]);
    7
}
extern "thiscall" fn neighbor_income(this: *mut u8) -> i64 {
    log("neighbor_income", this, &[]);
    17
}
extern "thiscall" fn department_expense(this: *mut u8) -> i64 {
    // SAFETY: test department stores its value at +4.
    unsafe {
        log("department_expense", this, &[]);
        field(this, 4)
    }
}
extern "thiscall" fn department_income(this: *mut u8) -> i64 {
    // SAFETY: test department stores its value at +4.
    unsafe {
        log("department_income", this, &[]);
        field(this, 4)
    }
}
extern "thiscall" fn group_tax(this: *mut u8, index: i32) -> i64 {
    log("tax", this, &[index as u32]);
    i64::from(index) + 1
}

#[test]
fn monthly_totals_preserve_map_order_and_64_bit_wrapping() {
    reset();
    let sim_vt = table(&[(0x48, group_tax as *const () as usize)]);
    let ord_vt = table(&[
        (0x58, ordinance_expense as *const () as usize),
        (0x5C, ordinance_income as *const () as usize),
    ]);
    let neighbor_vt = table(&[
        (0x84, neighbor_expense as *const () as usize),
        (0x80, neighbor_income as *const () as usize),
    ]);
    let dept_vt = table(&[
        (0x3C, department_expense as *const () as usize),
        (0x40, department_income as *const () as usize),
    ]);
    let mut object = Object::new(0x860);
    let p = object.ptr();
    let mut ordinance = [ord_vt.as_ptr() as u32];
    let o = ordinance.as_mut_ptr().cast();
    let mut neighbor = [neighbor_vt.as_ptr() as u32];
    let n = neighbor.as_mut_ptr().cast();
    let mut global = n;
    let _global = fake_data(0x00B4_3D8C, (&mut global as *mut *mut u8).cast());
    let mut header = Object::new(0x18);
    let h = header.ptr();
    let mut nodes: Vec<_> = (0..5).map(|_| Object::new(0x18)).collect();
    let ns: Vec<_> = nodes.iter_mut().map(Object::ptr).collect();
    let mut departments: Vec<_> = (0..5).map(|_| Object::new(0x10)).collect();
    let ds: Vec<_> = departments.iter_mut().map(Object::ptr).collect();
    let values = [i64::MAX, 23, -50, 0x1_0000_0001, i64::MIN];
    let mut bond_header = Object::new(0x40);
    let bh = bond_header.ptr();
    let mut bonds: Vec<_> = (0..3).map(|_| Object::new(0x40)).collect();
    let bs: Vec<_> = bonds.iter_mut().map(Object::ptr).collect();
    let payments = [i64::MAX, 0x1_0000_0001, i64::MAX];
    // SAFETY: all tree, list and interface pointers refer to the allocations above.
    unsafe {
        let sim = BudgetSim::from_ptr(p);
        sim.set_vtable(sim_vt.as_ptr() as u32);
        sim.set_ordinances(o);
        sim.set_departments(h);
        sim.set_bonds(bh);
        put(bh, 0, bh);
        put(h, 8, h);
        assert_eq!(invoke(0x0069_0500, p, &[], 0, 0), 0);
        assert_eq!(invoke(0x0069_0990, p, &[], 0, 0), 6);
        assert_eq!(
            calls(),
            [event("ordinance_expense", o, &[]), event("neighbor_expense", n, &[])]
        );
        assert_eq!(invoke(0x0069_0A30, p, &[], 0, 0), 78 + 31 + 17);
        let mut expected: Vec<_> = (0..12).map(|i| event("tax", p, &[i])).collect();
        expected.extend([event("ordinance_income", o, &[]), event("neighbor_income", n, &[])]);
        assert_eq!(calls(), expected);
        // Ordered tree: root 3, left subtree rooted at 1 with children 0 and 2, right child 4.
        put(h, 4, ns[3]);
        put(h, 8, ns[0]);
        put(h, 0x0C, ns[4]);
        for i in 0..5 {
            put(ns[i], 0x14, ds[i]);
            put(ds[i], 0, dept_vt.as_ptr());
            put(ds[i], 4, values[i]);
        }
        put(ns[0], 4, ns[1]);
        put(ns[1], 4, ns[3]);
        put(ns[2], 4, ns[1]);
        put(ns[3], 4, h);
        put(ns[4], 4, ns[3]);
        put(ns[1], 8, ns[0]);
        put(ns[1], 0x0C, ns[2]);
        put(ns[3], 8, ns[1]);
        put(ns[3], 0x0C, ns[4]);
        put(bh, 0, bs[0]);
        for i in 0..3 {
            put(bs[i], 0, if i < 2 { bs[i + 1] } else { bh });
            put(bs[i], 0x30, payments[i]);
        }
        let bond_sum = payments.into_iter().fold(0i64, i64::wrapping_add);
        let dept_sum = values.into_iter().fold(0i64, i64::wrapping_add);
        let before = bytes(p, 0x860);
        assert_eq!(invoke(0x0069_0500, p, &[], 0, 0) as i64, bond_sum);
        assert!(calls().is_empty());
        assert_eq!(
            invoke(0x0069_0990, p, &[], 0, 0) as i64,
            6i64.wrapping_add(dept_sum).wrapping_add(bond_sum)
        );
        let mut expected = vec![event("ordinance_expense", o, &[]), event("neighbor_expense", n, &[])];
        expected.extend(ds.iter().map(|&d| event("department_expense", d, &[])));
        assert_eq!(calls(), expected);
        assert_eq!(
            invoke(0x0069_0A30, p, &[], 0, 0) as i64,
            (78i64 + 31 + 17).wrapping_add(dept_sum)
        );
        let mut expected: Vec<_> = (0..12).map(|i| event("tax", p, &[i])).collect();
        expected.extend([event("ordinance_income", o, &[]), event("neighbor_income", n, &[])]);
        expected.extend(ds.iter().map(|&d| event("department_income", d, &[])));
        assert_eq!(calls(), expected);
        assert_eq!(bytes(p, 0x860), before);
    }
}
