// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Core vectors t13, REH12, REH22, AQ2/AQ3 and VC29, with game-layout grids and ordered virtual calls.

use super::*;
use crate::testing::{fake, GameFpu, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Population,
    X(u32),
    Z(u32),
    Size(u32),
    Get(u32, u32, u32),
    Tract(usize, usize, usize, u32, u32),
}
thread_local! {static CALLS:RefCell<Vec<Call>>=const{RefCell::new(Vec::new())};}
fn log(c: Call) {
    CALLS.with(|v| v.borrow_mut().push(c));
}
fn take() -> Vec<Call> {
    CALLS.with(|v| core::mem::take(&mut *v.borrow_mut()))
}

#[repr(C)]
struct Grid {
    vtable: *const usize,
    id: u32,
    size: i32,
    nx: u32,
    nz: u32,
    wide: F80,
    populations: Vec<u32>,
    values: Vec<f64>,
    change: Option<(*mut u8, usize, *mut u8)>,
    sizes: Vec<i32>,
}
unsafe extern "thiscall" fn size(p: *mut Grid) -> u64 {
    unsafe {
        log(Call::Size((*p).id));
        let n = if (*p).sizes.is_empty() {
            (*p).size
        } else {
            (*p).sizes.remove(0)
        };
        change(p);
        (0x1122_3344u64 << 32) | u64::from(n as u32)
    }
}
unsafe extern "thiscall" fn nx(p: *mut Grid) -> u32 {
    log(Call::X(unsafe { (*p).id }));
    unsafe { (*p).nx }
}
unsafe extern "thiscall" fn nz(p: *mut Grid) -> u32 {
    log(Call::Z(unsafe { (*p).id }));
    unsafe { (*p).nz }
}
unsafe fn change(p: *mut Grid) {
    unsafe {
        if let Some((owner, offset, next)) = (*p).change.take() {
            write_unaligned(owner.add(offset).cast::<*mut u8>(), next);
        }
    }
}
unsafe extern "thiscall" fn pop(p: *mut Grid, x: u32, z: u32) -> u64 {
    unsafe {
        log(Call::Get((*p).id, x, z));
        let grid = &*p;
        let v = grid
            .populations
            .get((x as usize).wrapping_mul((*p).nz as usize).wrapping_add(z as usize))
            .copied()
            .unwrap_or(0);
        change(p);
        (0xDDCC_BBAAu64 << 32) | u64::from(v)
    }
}
unsafe extern "thiscall" fn value(p: *mut Grid, x: u32, z: u32) -> f64 {
    unsafe {
        log(Call::Get((*p).id, x, z));
        let grid = &*p;
        let v = grid
            .values
            .get((x as usize).wrapping_mul((*p).nz as usize).wrapping_add(z as usize))
            .copied()
            .unwrap_or(0.0);
        change(p);
        v
    }
}
#[unsafe(naked)]
unsafe extern "thiscall" fn wide_value(_p: *mut Grid, _x: u32, _z: u32) {
    core::arch::naked_asm!(
        "fld tbyte ptr [ecx + 20]",
        "mov eax, 0x98765432",
        "mov edx, 0x12345678",
        "ret 8"
    )
}
unsafe extern "thiscall" fn total(this: *mut u8) -> u64 {
    log(Call::Population);
    (0x5566_7788u64 << 32) | u64::from(unsafe { ResidentialSim::from_ptr(this) }.population() as u32)
}

struct Fixture {
    _fpu: GameFpu,
    memory: Object,
    table: Box<[usize; 23]>,
    _population_table: Box<[usize; 23]>,
    _sim_table: Box<[usize; 17]>,
    grids: Vec<Grid>,
}
impl Fixture {
    fn new(nx_: u32, nz_: u32) -> Self {
        let mut table = Box::new([0usize; 23]);
        table[10] = size as *const () as usize;
        table[12] = nx as *const () as usize;
        table[13] = nz as *const () as usize;
        table[22] = value as *const () as usize;
        let mut pt = table.clone();
        pt[22] = pop as *const () as usize;
        let mut simt = Box::new([0usize; 17]);
        simt[16] = total as *const () as usize;
        let mut memory = Object::new(0x350);
        unsafe {
            memory.ptr().cast::<*const usize>().write(simt.as_ptr());
        }
        let grids = (0..9)
            .map(|i| Grid {
                vtable: if i < 3 { pt.as_ptr() } else { table.as_ptr() },
                id: i,
                size: 4,
                nx: nx_,
                nz: nz_,
                wide: F80::ZERO,
                populations: vec![0; (nx_ * nz_) as usize],
                values: vec![0.0; (nx_ * nz_) as usize],
                change: None,
                sizes: Vec::new(),
            })
            .collect();
        let mut f = Self {
            _fpu: GameFpu::new(),
            memory,
            table,
            _population_table: pt,
            _sim_table: simt,
            grids,
        };
        let p = f.p();
        let sim = unsafe { ResidentialSim::from_ptr(p) };
        sim.set_population_low(f.gp(0));
        sim.set_population_medium(f.gp(1));
        sim.set_population_high(f.gp(2));
        sim.set_eq_low(f.gp(3));
        sim.set_eq_medium(f.gp(4));
        sim.set_eq_high(f.gp(5));
        sim.set_hq_low(f.gp(6));
        sim.set_hq_medium(f.gp(7));
        sim.set_hq_high(f.gp(8));
        f
    }
    fn p(&mut self) -> *mut u8 {
        self.memory.ptr()
    }
    fn gp(&mut self, i: usize) -> *mut u8 {
        (&mut self.grids[i] as *mut Grid).cast()
    }
    fn set(&mut self, x: u32, z: u32, populations: [u32; 3], q: [f64; 3]) {
        let index = (x * self.grids[0].nz + z) as usize;
        for w in 0..3 {
            self.grids[w].populations[index] = populations[w];
            self.grids[w + 3].values[index] = q[w];
            self.grids[w + 6].values[index] = q[w];
        }
    }
    fn query(&mut self, export: usize, args: &[u32]) -> RawCall {
        let mut c = RawCall::new(Conv::Thiscall, export, self.p() as u32, args);
        c.take_st0 = 1;
        unsafe {
            call_raw(&mut c);
        }
        c
    }
    fn tract(&mut self, school: bool, x: u32, z: u32) -> (RawCall, [u32; 4]) {
        let p = self.p();
        let mut words = [
            p.wrapping_add(if school { 0x7C } else { 0x88 }) as u32,
            p.wrapping_add(0x48) as u32,
            x,
            z,
        ];
        let c = self.query(invoke_tract as *const () as usize, &[words.as_mut_ptr() as u32]);
        (c, words)
    }
    fn extrema(&mut self, school: bool) -> (u64, [i32; 4], [f32; 2]) {
        let mut coords = [-999; 4];
        let mut values = [999.0f32; 2];
        let args = [
            &mut coords[0] as *mut i32 as u32,
            &mut coords[1] as *mut i32 as u32,
            &mut coords[2] as *mut i32 as u32,
            &mut coords[3] as *mut i32 as u32,
            &mut values[0] as *mut f32 as u32,
            &mut values[1] as *mut f32 as u32,
        ];
        let export = if school {
            fc4p_006f7380 as *const () as usize
        } else {
            fc4p_006f7550 as *const () as usize
        };
        let result = unsafe { invoke_extrema(self.p(), export, args.as_ptr()) };
        (result, coords, values)
    }
}

/// Copy the four original argument words back after the callee has popped them, preserving its results.
#[unsafe(naked)]
unsafe extern "thiscall" fn invoke_tract(_this: *mut u8, _words: *mut u32) {
    core::arch::naked_asm!(
        "push esi", "mov esi, dword ptr [esp + 8]", "push dword ptr [esi + 12]", "push dword ptr [esi + 8]",
        "push dword ptr [esi + 4]", "push dword ptr [esi]", "call {port}",
        "mov ecx, dword ptr [esp - 16]", "mov dword ptr [esi], ecx",
        "mov ecx, dword ptr [esp - 12]", "mov dword ptr [esi + 4], ecx",
        "mov ecx, dword ptr [esp - 8]", "mov dword ptr [esi + 8], ecx",
        "mov ecx, dword ptr [esp - 4]", "mov dword ptr [esi + 12], ecx", "pop esi", "ret 4",
        port=sym fc4p_006f6fc0,
    )
}

#[unsafe(naked)]
unsafe extern "thiscall" fn invoke_extrema(_this: *mut u8, _export: usize, _args: *const u32) -> u64 {
    core::arch::naked_asm!(
        "push esi",
        "push edi",
        "mov edi, dword ptr [esp + 12]",
        "mov esi, dword ptr [esp + 16]",
        "push dword ptr [esi + 20]",
        "push dword ptr [esi + 16]",
        "push dword ptr [esi + 12]",
        "push dword ptr [esi + 8]",
        "push dword ptr [esi + 4]",
        "push dword ptr [esi]",
        "mov dword ptr [esp - 20], 17",
        "mov dword ptr [esp - 16], 19",
        "mov dword ptr [esp - 12], 23",
        "mov dword ptr [esp - 8], 29",
        "call edi",
        "pop edi",
        "pop esi",
        "ret 8",
    )
}

fn gets(ids: &[u32], x: u32, z: u32) -> Vec<Call> {
    ids.iter().map(|&id| Call::Get(id, x, z)).collect()
}

#[test]
fn tract_quotient_wraps_population_sum_and_preserves_argument_writes() {
    let mut f = Fixture::new(1, 1);
    f.set(0, 0, [40000, 30000, 0], [10.0; 3]);
    let p1 = f.gp(1) as u32;
    let q1 = f.gp(4) as u32;
    let (c, args) = f.tract(true, 0, 0);
    assert_eq!(c.eax, 4464);
    assert_eq!(
        c.st0.to_f32().to_bits(),
        (F80::from_f32(700000.0) / F80::from_i32(4464)).to_f32().to_bits()
    );
    assert_eq!(args, [q1, 4464, p1, 300000.0f32.to_bits()]);
    assert_eq!(take(), gets(&[0, 1, 2, 2, 5, 1, 4, 0, 3], 0, 0));
    for school in [true, false] {
        let mut f = Fixture::new(1, 1);
        f.set(0, 0, [3, 5, 2], [20.0, 80.0, 100.0]);
        let (c, _) = f.tract(school, 0, 0);
        assert_eq!(c.st0.to_f32(), 66.0);
        take();
    }
}

#[test]
fn zero_and_wrapped_zero_skip_all_float_grids() {
    for pops in [[0, 0, 0], [32768, 32768, 0], [0xAABB_0001, 0xCCDD_FFFF, 0xEEFF_0000]] {
        let mut f = Fixture::new(1, 1);
        f.set(0, 0, pops, [f64::NAN; 3]);
        let p = f.p();
        let p1 = f.gp(1) as u32;
        let (c, args) = f.tract(true, 0, 0);
        assert_eq!(c.st0, F80::ZERO);
        assert_eq!((c.eax, c.edx), (pops[2], 0xDDCC_BBAA));
        assert_eq!(args, [p.wrapping_add(0x7C) as u32, p1, 0, 0]);
        assert_eq!(take(), gets(&[0, 1, 2], 0, 0));
    }
}

#[test]
fn tract_snapshots_first_population_pointers_then_rereads_for_weighting() {
    let mut f = Fixture::new(1, 1);
    f.set(0, 0, [1, 2, 3], [10.0, 20.0, 30.0]);
    // The initial denominator uses the saved R$$ pointer; the weighted pass fetches its replacement.
    let p = f.p();
    let replacement = f.gp(2);
    f.grids[0].change = Some((p, 0x4C, replacement));
    let (c, _) = f.tract(true, 0, 0);
    assert_eq!(c.st0.to_f32(), (F80::from_i32(160) / F80::from_i32(6)).to_f32());
    assert_eq!(take(), gets(&[0, 1, 2, 2, 5, 2, 4, 0, 3], 0, 0));
}

#[test]
fn tract_result_keeps_wide_exponent_and_fractional_register_precision() {
    let mut f = Fixture::new(1, 1);
    f.set(0, 0, [1, 0, 0], [0.0; 3]);
    f.table[22] = wide_value as *const () as usize;
    // 2^200 cannot be represented as float32, but the R$ product is never stored as float32.
    f.grids[3].wide = F80::from_f64(2f64.powi(200));
    let (c, _) = f.tract(true, 0, 0);
    assert_eq!(c.st0, F80::from_f64(2f64.powi(200)));
    assert_eq!(c.edx, 0x1234_5678);
    take();
    // A float64 callee's register value is used before the game's 24-bit arithmetic rounds it.
    f.table[22] = value as *const () as usize;
    f.grids[3].values[0] = 1.0 + 2f64.powi(-24);
    let (c, _) = f.tract(true, 0, 0);
    assert_eq!(c.st0.to_f32(), 1.0);
    take();
}

#[test]
fn global_query_rounds_running_sum_and_uses_signed_published_population() {
    for export in [fc4p_006f7720 as *const () as usize, fc4p_006f7870 as *const () as usize] {
        let school = export == fc4p_006f7720 as *const () as usize;
        let base = if school { 3 } else { 6 };
        let mut f = Fixture::new(2, 1);
        f.set(0, 0, [1, 0, 0], [10.0, 0.0, 0.0]);
        f.set(1, 0, [1, 0, 0], [80.0, 0.0, 0.0]);
        let sim = unsafe { ResidentialSim::from_ptr(f.p()) };
        sim.set_population(2);
        let c = f.query(export, &[]);
        assert_eq!(c.st0.to_f32(), 45.0);
        assert_eq!((c.eax, c.edx), (2, 0x5566_7788));
        let mut expected = vec![Call::Population, Call::X(base), Call::Z(base)];
        for x in 0..2 {
            expected.extend(gets(&[0, base, 1, base + 1, 2, base + 2], x, 0));
        }
        expected.push(Call::Population);
        assert_eq!(take(), expected);
        sim.set_population(-2);
        assert_eq!(f.query(export, &[]).st0.to_f32(), -45.0);
        take();
        sim.set_population(0);
        assert_eq!(f.query(export, &[]).st0, F80::ZERO);
        assert_eq!(take(), [Call::Population]);
    }
}

#[test]
fn global_float_store_vector_and_empty_grid_division() {
    let mut f = Fixture::new(1, 1);
    f.set(0, 0, [7, 0, 0], [f64::from(f32::from_bits(0x3DCC_CCCE)), 0.0, 0.0]);
    unsafe { ResidentialSim::from_ptr(f.p()) }.set_population(7);
    let c = f.query(fc4p_006f7720 as *const () as usize, &[]);
    assert_eq!(c.st0.to_f64(), 0.100_000_008_940_696_72);
    take();
    f.grids[3].nx = 0;
    assert_eq!(f.query(fc4p_006f7720 as *const () as usize, &[]).st0, F80::ZERO);
    assert_eq!(take(), [Call::Population, Call::X(3), Call::Z(3), Call::Population]);
}

unsafe extern "thiscall" fn fake_tract(this: *mut u8, q: *mut *mut u8, p: *mut *mut u8, x: u32, z: u32) -> f64 {
    log(Call::Tract(this as usize, q as usize, p as usize, x, z));
    3.5
}

#[test]
fn cell_queries_use_two_size_calls_on_the_same_grid_and_call_the_game_address() {
    let _fake = fake(0x006F_6FC0, fake_tract as *const () as usize);
    for (export, base, offset) in [
        (fc4p_006f79c0 as *const () as usize, 3, 0x7C),
        (fc4p_006f7a10 as *const () as usize, 6, 0x88),
    ] {
        let mut f = Fixture::new(1, 1);
        f.grids[base].sizes = vec![2, 4];
        let p = f.p();
        let replacement = f.gp(8);
        f.grids[base].change = Some((p, offset, replacement));
        let c = f.query(export, &[5, 7]);
        assert_eq!(c.st0.to_f32(), 3.5);
        assert_eq!(
            take(),
            [
                Call::Size(base as u32),
                Call::Size(base as u32),
                Call::Tract(
                    p as usize,
                    p.wrapping_add(offset) as usize,
                    p.wrapping_add(0x48) as usize,
                    1,
                    3
                )
            ]
        );
        f.grids[base].sizes = vec![1, -1];
        unsafe {
            write_unaligned(p.add(offset).cast::<*mut u8>(), f.gp(base));
        }
        f.query(export, &[u32::MAX, u32::MAX]);
        let events = take();
        assert_eq!(
            events.last(),
            Some(&Call::Tract(
                p as usize,
                p.wrapping_add(offset) as usize,
                p.wrapping_add(0x48) as usize,
                1,
                u32::MAX
            ))
        );
    }
}

#[test]
fn cell_query_integration_uses_the_r1_eq_or_hq_grid_size() {
    let mut f = Fixture::new(4, 4);
    f.set(1, 1, [1, 0, 0], [50.0, 60.0, 70.0]);
    assert_eq!(f.query(fc4p_006f79c0 as *const () as usize, &[5, 6]).st0.to_f32(), 50.0);
    take();
    f.grids[6].size = 2;
    f.set(2, 3, [1, 0, 0], [30.0, 40.0, 50.0]);
    assert_eq!(f.query(fc4p_006f7a10 as *const () as usize, &[5, 7]).st0.to_f32(), 30.0);
    take();
}

#[test]
fn extrema_empty_city_zeroes_all_outputs_and_keeps_pointer_residue() {
    for school in [true, false] {
        let mut f = Fixture::new(1, 1);
        let (result, coords, values) = f.extrema(school);
        assert_eq!(result as u8, 0);
        assert_eq!(coords, [0; 4]);
        assert_eq!(values, [0.0; 2]);
        assert_eq!(take(), [Call::Population]);
    }
}

#[test]
fn extrema_core_coordinate_vector_and_strict_ties() {
    for school in [true, false] {
        let mut f = Fixture::new(8, 8);
        unsafe { ResidentialSim::from_ptr(f.p()) }.set_population(3);
        for (x, z, q) in [(2, 3, 10.0), (5, 1, 80.0), (7, 7, 80.0)] {
            f.set(x, z, [1, 0, 0], [q, 0.0, 0.0]);
        }
        let (result, coords, values) = f.extrema(school);
        assert_eq!(result as u8, 1);
        assert_eq!(coords, [10, 14, 22, 6]);
        assert_eq!(values, [10.0, 80.0]);
        let calls = take();
        let id = if school { 3 } else { 6 };
        assert_eq!(&calls[..3], &[Call::Population, Call::X(id), Call::Z(id)]);
        assert_eq!(
            &calls[calls.len() - 8..],
            &[
                Call::Size(id),
                Call::Size(id),
                Call::Size(id),
                Call::Size(id),
                Call::Size(id),
                Call::Size(id),
                Call::Size(id),
                Call::Size(id)
            ]
        );
        // Every empty tract short-circuits through all three population grids; an occupied R$ skips the others.
        assert_eq!(&calls[3..6], gets(&[0, 1, 2], 0, 0));
    }
}

#[test]
fn extrema_no_updates_keep_existing_coordinate_words() {
    for school in [true, false] {
        let mut f = Fixture::new(0, 0);
        unsafe { ResidentialSim::from_ptr(f.p()) }.set_population(1);
        let (_, coords, values) = f.extrema(school);
        assert_eq!(coords, [70, 78, 94, 118]);
        assert_eq!(values, [200.0, -200.0]);
        take();
        let mut f = Fixture::new(1, 1);
        unsafe { ResidentialSim::from_ptr(f.p()) }.set_population(1);
        f.set(0, 0, [1, 0, 0], [f64::NAN, 0.0, 0.0]);
        let (_, coords, values) = f.extrema(school);
        assert_eq!(coords, [70, 78, 94, 118]);
        assert_eq!(values, [200.0, -200.0]);
        take();
    }
}

#[test]
fn extrema_asks_twice_per_coordinate_and_uses_signed_truncation() {
    let mut f = Fixture::new(1, 1);
    unsafe { ResidentialSim::from_ptr(f.p()) }.set_population(1);
    f.set(0, 0, [1, 0, 0], [10.0, 0.0, 0.0]);
    f.grids[3].sizes = vec![-3, 8, -1, 16, 3, 32, 5, 64];
    let (result, coords, _) = f.extrema(true);
    assert_eq!(coords, [-1, 0, 1, 2]);
    assert_eq!(result, 0x1122_3344_0000_0001);
    take();
}

#[unsafe(naked)]
unsafe extern "thiscall" fn raw_nx(_this: *mut Grid) {
    core::arch::naked_asm!("mov eax, dword ptr [ecx + 12]", "ret")
}
#[unsafe(naked)]
unsafe extern "thiscall" fn raw_nz(_this: *mut Grid) {
    core::arch::naked_asm!("mov eax, dword ptr [ecx + 16]", "ret")
}

#[test]
fn global_query_preserves_edx_from_entry_or_last_grid_when_population_only_writes_eax() {
    for export in [fc4p_006f7720 as *const () as usize, fc4p_006f7870 as *const () as usize] {
        let mut f = Fixture::new(1, 1);
        f._sim_table[16] = super::super::interface::fc4p_006f6f70 as *const () as usize;
        let mut c = RawCall::new(Conv::Thiscall, export, f.p() as u32, &[]);
        c.edx = 0xFEDC_BA98;
        c.take_st0 = 1;
        unsafe {
            call_raw(&mut c);
        }
        assert_eq!((c.eax, c.edx), (0, 0xFEDC_BA98));
        let sim = unsafe { ResidentialSim::from_ptr(f.p()) };
        sim.set_population(1);
        f.set(0, 0, [1, 0, 0], [10.0, 0.0, 0.0]);
        f.table[22] = wide_value as *const () as usize;
        f.grids[3].wide = F80::from_f32(10.0);
        f.grids[6].wide = F80::from_f32(10.0);
        let c = f.query(export, &[]);
        assert_eq!(c.edx, 0x1234_5678);
        assert_eq!(c.st0.to_f32(), 10.0);
        take();
        // An empty scan retains the count getters' EDX, set to the R$ quotient grid's table before X.
        f.table[12] = raw_nx as *const () as usize;
        f.table[13] = raw_nz as *const () as usize;
        f.grids[3].nx = 0;
        f.grids[6].nx = 0;
        let c = f.query(export, &[]);
        assert_eq!(c.edx, f.table.as_ptr() as u32);
        assert_eq!(c.st0, F80::ZERO);
        take();
    }
}

fn environment() -> (u16, u16, u32, u32) {
    let (mut cw, mut mxcsr) = (0u16, 0u32);
    let (sw, flags): (u16, u32);
    unsafe {
        core::arch::asm!("fnstcw word ptr [{cw}]", "fnstsw ax", "stmxcsr dword ptr [{mxcsr}]", "pushfd", "pop {flags:e}",
        cw=in(reg) &mut cw,mxcsr=in(reg) &mut mxcsr,flags=lateout(reg) flags,out("ax") sw);
    }
    (cw, sw & 0x3800, mxcsr, flags & 0x400)
}

#[test]
fn queries_preserve_float_environment_existing_x87_value_and_simulator_bytes() {
    let mut f = Fixture::new(1, 1);
    f.set(0, 0, [3, 5, 2], [20.0, 80.0, 100.0]);
    unsafe { ResidentialSim::from_ptr(f.p()) }.set_population(10);
    let p = f.p();
    let bytes = unsafe { core::slice::from_raw_parts(p, 0x350).to_vec() };
    let sentinel = 123.25f64;
    let mut restored = 0.0f64;
    unsafe {
        core::arch::asm!("fld qword ptr [{p}]",p=in(reg) &sentinel,out("st(0)") _);
    }
    let before = environment();
    for export in [
        fc4p_006f7720 as *const () as usize,
        fc4p_006f7870 as *const () as usize,
        fc4p_006f79c0 as *const () as usize,
        fc4p_006f7a10 as *const () as usize,
    ] {
        let args = if export == fc4p_006f79c0 as *const () as usize || export == fc4p_006f7a10 as *const () as usize {
            &[1, 2][..]
        } else {
            &[][..]
        };
        let c = f.query(export, args);
        assert_eq!(c.st0.to_f32(), 66.0);
        assert_eq!(environment(), before);
        take();
    }
    for school in [true, false] {
        f.tract(school, 0, 0);
        assert_eq!(environment(), before);
        f.extrema(school);
        assert_eq!(environment(), before);
        take();
    }
    unsafe {
        core::arch::asm!("fstp qword ptr [{p}]",p=in(reg) &mut restored,out("st(0)") _);
    }
    assert_eq!(restored.to_bits(), sentinel.to_bits());
    assert_eq!(unsafe { core::slice::from_raw_parts(p, 0x350) }, bytes);
}

#[test]
fn global_divisor_is_reread_after_grid_calls_and_zero_weight_nans_are_evaluated() {
    for export in [fc4p_006f7720 as *const () as usize, fc4p_006f7870 as *const () as usize] {
        let mut f = Fixture::new(1, 1);
        f.set(0, 0, [1, 0, 0], [90.0, 0.0, 0.0]);
        let p = f.p();
        unsafe { ResidentialSim::from_ptr(p) }.set_population(2);
        let last = if export == fc4p_006f7720 as *const () as usize {
            5
        } else {
            8
        };
        f.grids[last].change = Some((p, 0xA4, (-3i32) as u32 as *mut u8));
        let c = f.query(export, &[]);
        assert_eq!(c.st0.to_f32(), -30.0);
        assert_eq!(c.eax, (-3i32) as u32);
        take();
        unsafe { ResidentialSim::from_ptr(p) }.set_population(1);
        f.set(0, 0, [1, 0, 0], [90.0, f64::NAN, 0.0]);
        let c = f.query(export, &[]);
        assert!(c.st0.to_f32().is_nan());
        take();
        let c = f.tract(last == 5, 0, 0).0;
        assert!(c.st0.to_f32().is_nan());
        take();
    }
}

#[test]
fn extrema_bounds_can_leave_only_one_coordinate_pair_unset() {
    for school in [true, false] {
        for (q, want_coords, want_values) in [
            (200.0, [70, 78, 2, 2], [200.0, 200.0]),
            (-200.0, [2, 2, 94, 118], [-200.0, -200.0]),
            (400.0, [70, 78, 2, 2], [200.0, 400.0]),
            (-400.0, [2, 2, 94, 118], [-400.0, -200.0]),
        ] {
            let mut f = Fixture::new(1, 1);
            unsafe { ResidentialSim::from_ptr(f.p()) }.set_population(-1);
            f.set(0, 0, [1, 0, 0], [q, 0.0, 0.0]);
            let (_, coords, values) = f.extrema(school);
            assert_eq!(coords, want_coords);
            assert_eq!(values, want_values);
            take();
        }
        let mut f = Fixture::new(0, 0);
        unsafe { ResidentialSim::from_ptr(f.p()) }.set_population(1);
        let base = if school { 3 } else { 6 };
        f.grids[base].size = 256;
        let (result, coords, _) = f.extrema(school);
        assert_eq!(coords, [4480, 4992, 6016, 7552]);
        assert_eq!(result, 0x1122_3344_0000_1D01);
        take();
    }
}
