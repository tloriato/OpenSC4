// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, fake_data, Fake, GameFpu, Object};
use crate::traffic::test_support::{bytes, float_environment, invoke, object, table};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Grow(usize, usize, [u32; 3], u32, u32, u32),
    Insert(usize, u32, u32, usize),
    Construct(usize, usize),
    Erase(usize, u32),
    Free(usize),
    Date(Vec<u8>),
    Neighbors(usize, Vec<u8>),
    Response(usize, u32),
    Damage(usize, usize, usize, u32, u32, u32),
}

#[derive(Default)]
struct State {
    calls: Vec<Call>,
    sim: usize,
    response: f64,
    change_count: Option<u32>,
    curve_rate: Option<f32>,
    mutate: Option<(usize, usize, usize, u32)>,
}

thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }

fn log(c: Call) {
    STATE.with(|s| s.borrow_mut().calls.push(c));
}
fn take() -> Vec<Call> {
    STATE.with(|s| core::mem::take(&mut s.borrow_mut().calls))
}
fn reset() {
    STATE.with(|s| *s.borrow_mut() = State::default());
}

unsafe extern "thiscall" fn grow(
    this: *mut u8,
    position: u32,
    value: *const u32,
    scratch: *mut u32,
    count: u32,
    mode: u32,
) -> u64 {
    unsafe {
        let values = [value.read(), value.add(1).read(), value.add(2).read()];
        log(Call::Grow(
            this as usize,
            position as usize,
            values,
            scratch.read(),
            count,
            mode,
        ));
        scratch.write(0x9876_5432);
        store(this, 4, 0x1234_5678);
    }
    0xAABB_CCDD_5566_7788
}

extern "thiscall" fn insert(this: *mut u8, position: u32, count: u32, value: *const u32) -> u64 {
    log(Call::Insert(this as usize, position, count, value as usize));
    0xABCD_0123_9876_5432
}

unsafe extern "thiscall" fn construct(this: *mut u8, source: *mut u8) -> u64 {
    log(Call::Construct(this as usize, source as usize));
    unsafe {
        store(this, 0, word(source, 0));
        store(this, 4, word(source, 4));
    }
    STATE.with(|s| {
        if let Some((when, p, off, v)) = s.borrow().mutate {
            if when == source as usize {
                unsafe {
                    store(p as *mut u8, off, v);
                }
            }
        }
    });
    0x9988_7766_5544_3322
}

extern "thiscall" fn erase(this: *mut u8, root: u32) -> u64 {
    log(Call::Erase(this as usize, root));
    STATE.with(|s| {
        if let Some((when, p, off, v)) = s.borrow().mutate {
            if when == root as usize {
                unsafe {
                    store(p as *mut u8, off, v);
                }
            }
        }
    });
    0x1122_3344_5566_7788
}

extern "C" fn free(p: *mut u8) -> u64 {
    log(Call::Free(p as usize));
    0xCAFE_BABE_DEAD_BEEF
}

extern "thiscall" fn response(this: *mut u8, input: f32) -> f64 {
    log(Call::Response(this as usize, input.to_bits()));
    STATE.with(|s| {
        let s = s.borrow();
        if let Some(rate) = s.curve_rate {
            unsafe {
                store(s.sim as *mut u8, 0x7B4, rate.to_bits());
            }
        }
        s.response
    })
}

extern "thiscall" fn damage(this: *mut u8, intact: *mut u8, damaged: *mut u8, delta: u32, kind: u32) -> u64 {
    let offset = if kind == 1 { 0x7B0 } else { 0x7E4 };
    log(Call::Damage(
        this as usize,
        intact as usize,
        damaged as usize,
        delta,
        kind,
        unsafe { word(this, offset) },
    ));
    STATE.with(|s| {
        if let Some(count) = s.borrow().change_count {
            unsafe {
                store(this, offset, count);
            }
        }
    });
    0xDEAD_BEEF_1234_5678
}

extern "thiscall" fn date(_this: *mut u8) -> u64 {
    let p = STATE.with(|s| s.borrow().sim as *mut u8);
    log(Call::Date(unsafe { bytes(p, 0xD40) }));
    0x9988_7766_0123_4567
}

#[unsafe(naked)]
unsafe extern "thiscall" fn neighbors(_this: *mut u8) -> u64 {
    core::arch::naked_asm!("call {body}", "mov ecx, 0x10FFEEDD", "ret", body = sym neighbors_body);
}

extern "thiscall" fn neighbors_body(this: *mut u8) -> u64 {
    log(Call::Neighbors(this as usize, unsafe { bytes(this, 0xD40) }));
    0xABC0_FED0_1234_56FF
}

fn functions() -> Vec<Fake> {
    [
        (0x0062_6240, grow as *const () as usize),
        (0x007D_A570, insert as *const () as usize),
        (0x0071_2280, construct as *const () as usize),
        (0x0042_6A60, erase as *const () as usize),
        (0x0090_CF63, free as *const () as usize),
        (0x005F_4250, response as *const () as usize),
        (0x0071_5FB0, damage as *const () as usize),
        (0x0071_4E60, neighbors as *const () as usize),
    ]
    .into_iter()
    .map(|(a, f)| fake(a, f))
    .collect()
}

struct Constants {
    _zero64: Box<f64>,
    _floats: Box<[f32; 5]>,
    _guards: Vec<Fake>,
}
impl Constants {
    fn new() -> Self {
        let mut z = Box::new(0.0f64);
        let mut f = Box::new([100.0, 4_294_967_296.0, 0.0, 1.0, 0.0]);
        let mut guards = vec![fake_data(0x00A8_0990, (&mut *z as *mut f64).cast())];
        for (i, a) in [0x00A8_0AA0, 0x00A8_0AA8, 0x00A8_1054, 0x00A8_1228]
            .into_iter()
            .enumerate()
        {
            guards.push(fake_data(a, (&mut f[i] as *mut f32).cast()));
        }
        Self {
            _zero64: z,
            _floats: f,
            _guards: guards,
        }
    }
}

unsafe fn run(f: unsafe extern "C" fn(), this: *mut u8, args: &[u32], cdecl: bool) -> Registers {
    unsafe { invoke(f as *const () as usize, this, args, 0x1234_5678, 0x89AB_CDEF, cdecl) }
}

#[test]
fn accident_append_copies_all_bits_and_only_advances_the_end() {
    reset();
    let mut sim_mem = object(0xD40, &[]);
    let mut entries = object(36, &[]);
    unsafe {
        let p = sim_mem.ptr();
        let s = RecalcSim::from_ptr(p);
        s.set_accidents_end(entries.ptr());
        s.set_accidents_limit(entries.ptr().add(36));
        let mut want = bytes(p, 0xD40);
        want[0x134..0x138].copy_from_slice(&(entries.ptr().add(12) as u32).to_le_bytes());
        let r = run(fc4p_00716950, p, &[u32::MAX, 0x8000_0000, 0x7FA1_2345], false);
        assert_eq!(r.eax, entries.ptr().add(12) as u32);
        assert_eq!(r.edx, 0x7FA1_2345);
        assert_eq!(r.ecx, p.add(0x130) as u32);
        assert_eq!(bytes(p, 0xD40), want);
        assert_eq!(
            (word(entries.ptr(), 0), word(entries.ptr(), 4), word(entries.ptr(), 8)),
            (u32::MAX, 0x8000_0000, 0x7FA1_2345)
        );
        assert_eq!(bytes(entries.ptr().add(12), 24), vec![0xA5; 24]);
    }
    assert!(take().is_empty());
}

#[test]
fn accident_null_end_skips_the_copy_and_full_storage_calls_growth() {
    reset();
    let _f = functions();
    let mut mem = Object::new(0xD40);
    unsafe {
        let p = mem.ptr();
        let s = RecalcSim::from_ptr(p);
        s.set_accidents_limit(100usize as *mut u8);
        let r = run(fc4p_00716950, p, &[7, 8, 9], false);
        assert_eq!((r.eax, r.edx), (12, 7));
        s.set_accidents_limit(s.accidents_end());
        let r = run(fc4p_00716950, p, &[7, 8, 9], false);
        assert_eq!((r.eax, r.edx), (0x5566_7788, 0xAABB_CCDD));
        assert_eq!(s.accidents_end() as u32, 0x1234_5678);
        assert_eq!(take(), [Call::Grow(p.add(0x130) as usize, 12, [7, 8, 9], 7, 1, 1)]);
    }
}

#[test]
fn accident_append_reloads_the_end_after_an_aliased_record_write() {
    reset();
    let mut mem = Object::new(0xD40);
    unsafe {
        let p = mem.ptr();
        let s = RecalcSim::from_ptr(p);
        s.set_accidents_end(p.add(0x134));
        s.set_accidents_limit(p.add(0x200));
        run(fc4p_00716950, p, &[0x100, 0x200, 0x300], false);
        assert_eq!(s.accidents_end() as u32, 0x10C);
        assert_eq!(word(p, 0x138), 0x200);
        assert_eq!(word(p, 0x13C), 0x300);
    }
}

#[test]
fn word_resize_shrinks_with_signed_pointer_difference_and_grows_even_when_equal() {
    reset();
    let _f = functions();
    let mut vector = object(12, &[]);
    unsafe {
        let p = vector.ptr();
        for (begin, end, desired, expected) in [(0x1000, 0x1020, 3, 0x100C), (0x1000, 0x0FFC, 0, 0x1000)] {
            store(p, 0, begin);
            store(p, 4, end);
            let r = run(fc4p_007169c0, p, &[desired, 0x1234], false);
            assert_eq!((r.eax, r.edx), (0, end));
            assert_eq!(word(p, 4), expected);
            assert!(take().is_empty());
        }
        for desired in [8, 10] {
            store(p, 0, 0x1000);
            store(p, 4, 0x1020);
            let before = bytes(p, 12);
            let r = run(fc4p_007169c0, p, &[desired, 0x1234], false);
            assert_eq!((r.eax, r.edx), (0x9876_5432, 0xABCD_0123));
            assert_eq!(take(), [Call::Insert(p as usize, 0x1020, desired - 8, 0x1234)]);
            assert_eq!(bytes(p, 12), before);
        }
    }
}

#[test]
fn range_construction_calls_each_tree_then_copies_the_tail_and_returns_the_end() {
    reset();
    let _f = functions();
    let mut src = Object::new(56);
    let mut dest = object(56, &[]);
    unsafe {
        let (a, b) = (src.ptr(), dest.ptr());
        for i in 0..14 {
            store(a, i * 4, 0xAABB_0000 + i as u32);
        }
        STATE.with(|s| s.borrow_mut().mutate = Some((a as usize, a as usize, 20, 0x9876_5432)));
        let before = bytes(b, 56);
        let r = run(
            fc4p_00716a10,
            0x1234usize as *mut u8,
            &[a as u32, a.add(56) as u32, b as u32],
            true,
        );
        assert_eq!(r.eax, b.add(56) as u32);
        assert_eq!(r.edx, word(a, 48));
        assert_eq!(r.ecx, word(a, 52));
        assert_eq!(
            take(),
            [
                Call::Construct(b as usize, a as usize),
                Call::Construct(b.add(28) as usize, a.add(28) as usize)
            ]
        );
        for off in [0, 28] {
            assert_eq!(bytes(b.add(off), 8), bytes(a.add(off), 8));
            assert_eq!(&bytes(b, 56)[off + 8..off + 12], &before[off + 8..off + 12]);
            assert_eq!(bytes(b.add(off + 12), 16), bytes(a.add(off + 12), 16));
        }
    }
}

#[test]
fn empty_and_null_construction_preserve_edx_and_skip_calls() {
    reset();
    unsafe {
        let r = run(fc4p_00716a10, 0x1234usize as *mut u8, &[0x1000, 0x1000, 0x2000], true);
        assert_eq!(
            r,
            Registers {
                eax: 0x2000,
                ecx: 0x1234,
                edx: 0x89AB_CDEF
            }
        );
        let r = run(fc4p_00716a10, 0x1234usize as *mut u8, &[0x1000, 0x101C, 0], true);
        assert_eq!(
            r,
            Registers {
                eax: 28,
                ecx: 0x1234,
                edx: 0x89AB_CDEF
            }
        );
    }
    assert!(take().is_empty());
}

#[test]
fn range_destruction_erases_right_subtrees_before_nodes_and_resets_only_nonempty_trees() {
    reset();
    let _f = functions();
    let mut records = Object::new(84);
    let mut h0 = object(24, &[]);
    let mut h1 = object(24, &[]);
    let mut root = Object::new(24);
    let mut left = Object::new(24);
    let mut replacement = Object::new(24);
    unsafe {
        let p = records.ptr();
        store(p, 0, h0.ptr() as u32);
        store(p, 4, 0);
        store(p, 28, h1.ptr() as u32);
        store(p, 32, 2);
        store(h1.ptr(), 4, root.ptr() as u32);
        store(root.ptr(), 8, left.ptr() as u32);
        store(root.ptr(), 12, 0x2222);
        store(replacement.ptr(), 12, 0x3333);
        STATE.with(|s| s.borrow_mut().mutate = Some((0x2222, root.ptr() as usize, 8, replacement.ptr() as u32)));
        let before = bytes(p, 84);
        let empty_header = bytes(h0.ptr(), 24);
        let mut expected = before.clone();
        expected[32..36].fill(0);
        let r = run(
            fc4p_00716a70,
            0x4444usize as *mut u8,
            &[p as u32, p.add(84) as u32, 0x9876],
            true,
        );
        assert_eq!(r.eax, p.add(84) as u32);
        assert_eq!(r.edx, 0xCAFE_BABE);
        assert_eq!(
            take(),
            [
                Call::Free(h0.ptr() as usize),
                Call::Erase(p.add(28) as usize, 0x2222),
                Call::Free(root.ptr() as usize),
                Call::Erase(p.add(28) as usize, 0x3333),
                Call::Free(replacement.ptr() as usize),
                Call::Free(h1.ptr() as usize)
            ]
        );
        assert_eq!(bytes(p, 84), expected);
        assert_eq!(bytes(h0.ptr(), 24), empty_header);
        assert_eq!(
            (word(h1.ptr(), 4), word(h1.ptr(), 8), word(h1.ptr(), 12)),
            (0, h1.ptr() as u32, h1.ptr() as u32)
        );
    }
}

#[test]
fn destruction_empty_range_and_null_headers_keep_entry_registers() {
    reset();
    let mut records = Object::new(28);
    unsafe {
        let p = records.ptr();
        for end in [p, p.add(28)] {
            let r = run(fc4p_00716a70, 0x4444usize as *mut u8, &[p as u32, end as u32, 0], true);
            assert_eq!(
                r,
                Registers {
                    eax: end as u32,
                    ecx: 0x4444,
                    edx: 0x89AB_CDEF
                }
            );
        }
    }
    assert!(take().is_empty());
}

#[test]
fn start_recalc_resets_only_cycle_fields_before_recalculating_neighbors() {
    reset();
    let _f = functions();
    let _fp = GameFpu::new();
    let _constants = Constants::new();
    let vt = table(&[(0x20, date as *const () as usize)]);
    let mut city_mem = object(4, &vt);
    let mut city_ptr = city_mem.ptr();
    let _city = fake_data(0x00B4_3D5C, (&mut city_ptr as *mut *mut u8).cast());
    let mut mem = object(0xD40, &[]);
    unsafe {
        let p = mem.ptr();
        for bucket in [0, 1, 2, 3, u32::MAX] {
            let s = RecalcSim::from_ptr(p);
            s.set_reroute_bucket(bucket);
            STATE.with(|s| s.borrow_mut().sim = p as usize);
            let before = bytes(p, 0xD40);
            let mut expected = before.clone();
            for (off, size) in [(0x790, 8), (0x798, 8), (0x1C0, 36), (0x6EC, 16)] {
                expected[off..off + size].fill(0);
            }
            expected[0x7A0..0x7A4].copy_from_slice(&0x0123_4567u32.to_le_bytes());
            expected[0x950..0x954].copy_from_slice(&(bucket.wrapping_add(1) & 3).to_le_bytes());
            let env = float_environment();
            let r = run(fc4p_00717c90, p, &[0x9876_5432], false);
            assert_eq!((r.eax, r.edx), (0x1234_5601, 0xABC0_FED0));
            assert_eq!(r.ecx, 0x10FF_EEDD);
            assert_eq!(bytes(p, 0xD40), expected);
            assert_eq!(take(), [Call::Date(before), Call::Neighbors(p as usize, expected)]);
            assert_eq!(float_environment(), env);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn funding_case(
    rail: bool,
    old_rate: f32,
    change: f64,
    cells: u32,
    damaged: u32,
    expected_rate: f32,
    target: u32,
    callback_count: Option<u32>,
) {
    reset();
    let _f = functions();
    let _fp = GameFpu::new();
    let _constants = Constants::new();
    let mut mem = object(0xD40, &[]);
    let mut input = 0.25f32;
    unsafe {
        let p = mem.ptr();
        let s = RecalcSim::from_ptr(p);
        let (count_off, damaged_off, rate_off) = if rail {
            (0x7E0, 0x7E4, 0x7E8)
        } else {
            (0x7AC, 0x7B0, 0x7B4)
        };
        store(p, count_off, cells);
        store(p, damaged_off, damaged);
        store(p, rate_off, old_rate.to_bits());
        s.set_potholes(9);
        STATE.with(|st| {
            let mut st = st.borrow_mut();
            st.sim = p as usize;
            st.response = change;
            st.change_count = callback_count;
        });
        let mut expected = bytes(p, 0xD40);
        expected[rate_off..rate_off + 4].copy_from_slice(&expected_rate.to_bits().to_le_bytes());
        let delta = target.wrapping_sub(damaged);
        let final_count = if target == damaged {
            damaged
        } else if rail {
            callback_count.unwrap_or(target)
        } else {
            callback_count.unwrap_or(damaged).wrapping_add(delta)
        };
        expected[damaged_off..damaged_off + 4].copy_from_slice(&final_count.to_le_bytes());
        if !rail && target != damaged {
            expected[0x318..0x31A].copy_from_slice(&(final_count as u16).to_le_bytes());
        }
        let env = float_environment();
        let r = run(
            if rail { fc4p_00717e00 } else { fc4p_00717d30 },
            p,
            &[&mut input as *mut f32 as u32],
            false,
        );
        assert_eq!(bytes(p, 0xD40), expected);
        let mut calls = vec![Call::Response(p.add(0xD20) as usize, 25.0f32.to_bits())];
        if target != damaged {
            calls.push(Call::Damage(
                p as usize,
                p.add(damaged_off + 8) as usize,
                p.add(damaged_off + 28) as usize,
                delta,
                if rail { 2 } else { 1 },
                if rail { target } else { damaged },
            ));
            assert_eq!(r.edx, 0xDEAD_BEEF);
            assert_eq!(r.eax, if rail { 0x1234_5678 } else { final_count });
        } else {
            assert_eq!(r.eax, target);
        }
        assert_eq!(take(), calls);
        assert_eq!(float_environment(), env);
    }
}

#[test]
fn core_funding_damage_targets_and_published_pothole_count() {
    for rail in [false, true] {
        funding_case(rail, 0.0, -0.01, 10, 0, 0.0, 0, None);
        funding_case(rail, 0.0, 0.02, 50, 0, 0.02, 1, None);
        funding_case(rail, 1.0, 0.02, 70_000, 0, 1.0, 70_000, None);
        funding_case(rail, 0.25, 0.0, 16, 9, 0.25, 4, None);
        funding_case(rail, 0.25, 0.0, 16, 9, 0.25, 4, Some(100));
    }
}

#[test]
fn funding_clamp_nan_unsigned_counts_and_unrounded_curve_results() {
    for rail in [false, true] {
        funding_case(rail, 0.0, f64::INFINITY, 1, 0, 1.0, 1, None);
        funding_case(rail, 0.0, f64::NEG_INFINITY, 1, 0, 0.0, 0, None);
        funding_case(rail, 0.0, f64::NAN, 1, 0, f32::NAN, 0, None);
        funding_case(rail, -0.0, -0.0, 1, 0, -0.0, 0, None);
        funding_case(rail, 0.0, 0.5, 0x8000_0001, 0, 0.5, 0x4000_0000, None);
        funding_case(rail, 0.0, 1.0, u32::MAX, 0, 1.0, 0, None);
        // The curve's subnormal result stays in ST0 until addition, then rounds once at the store.
        funding_case(
            rail,
            f32::from_bits(1),
            f64::from_bits((1023 - 150) << 52),
            1,
            0,
            f32::from_bits(2),
            0,
            None,
        );
        funding_case(rail, -f32::MAX, f64::from(f32::MAX), 5, 0, 0.0, 0, None);
    }
}

#[test]
fn funding_reads_the_previous_rate_after_the_curve_returns() {
    reset();
    let _f = functions();
    let _fp = GameFpu::new();
    let _constants = Constants::new();
    let mut mem = Object::new(0xD40);
    let mut input = 1.0f32;
    unsafe {
        let p = mem.ptr();
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.sim = p as usize;
            s.curve_rate = Some(0.5);
            s.response = 0.25;
        });
        run(fc4p_00717d30, p, &[&mut input as *mut f32 as u32], false);
        assert_eq!(word(p, 0x7B4), 0.75f32.to_bits());
    }
}
