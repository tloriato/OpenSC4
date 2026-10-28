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

//! `cSC4LandValueSimulator`: the per-tick land value pass (006be7b0 DoIdle).

use super::*;
use crate::abi::{call_raw, Conv, RawCall};
use core::mem::MaybeUninit;
use core::ptr::{read_unaligned, write_unaligned};

/// Reads a word from a live game object.
unsafe fn word(p: *mut u8, offset: usize) -> i32 {
    // SAFETY: the caller supplies the original object's readable field.
    unsafe { read_unaligned(p.wrapping_add(offset).cast()) }
}

/// The nested vector row is twelve bytes; its first word points to the cells.
unsafe fn cell<T>(grid: *mut u8, x: i32, z: i32) -> *mut T {
    // SAFETY: coordinates and the nested vectors are supplied by the game's scan.
    unsafe {
        let rows = read_unaligned(grid.cast::<*mut u8>());
        let row = read_unaligned(rows.wrapping_add(x.wrapping_mul(12) as usize).cast::<*mut T>());
        row.wrapping_offset(z as isize)
    }
}

fn histogram_len(lv: LandValueSim) -> i32 {
    (lv.histogram_end() as u32).wrapping_sub(lv.histogram_begin() as u32) as i32 >> 2
}

unsafe fn clear_histogram(lv: LandValueSim) -> u32 {
    let len = histogram_len(lv);
    for i in 0..len {
        // SAFETY: these bins are the simulator's allocated histogram.
        unsafe { write_unaligned(lv.histogram_begin().wrapping_offset(i as isize), 0) };
    }
    len as u32
}

/// The timer's destructor preserves EAX, including leftovers from the state's final operation.
unsafe fn finish_timer(timer: *mut u8, eax: u32) -> u32 {
    let mut call = RawCall::new(Conv::Thiscall, crate::abi::target(0x00A6_D837), timer as u32, &[]);
    call.eax = eax;
    // SAFETY: the profiling scope has no arguments and preserves the incoming registers.
    unsafe { call_raw(&mut call) };
    call.eax
}

/// Advances the pacing counter before checking the deadline or the stopped byte.
unsafe fn budget_stopped(budget: *mut u8) -> bool {
    // SAFETY: idle_budget_new initialized all fields used here.
    unsafe {
        let counter = (word(budget, 8) as u32).wrapping_add(word(budget, 12) as u32);
        write_unaligned(budget.add(8).cast::<u32>(), counter);
        if counter & 1023 == 0 && game::idle_budget_expired(budget) as u8 != 0 {
            *budget.add(16) = 1;
            return true;
        }
        *budget.add(16) != 0
    }
}

unsafe fn tract_altitude(lv: LandValueSim, x: i32, z: i32) -> f32 {
    let (mut px, mut pz) = (MaybeUninit::<f32>::uninit(), MaybeUninit::<f32>::uninit());
    // SAFETY: the grid fills both centre coordinates; terrain returns its height in ST0.
    unsafe {
        vcall!(lv.intrinsic_grid(), 0x54 => fn(i32, i32, *mut f32, *mut f32),
            x, z, px.as_mut_ptr(), pz.as_mut_ptr());
        vcall_st0!(lv.terrain(), 0x34, px.assume_init(), pz.assume_init()).to_f32()
    }
}

unsafe fn statistics_done(lv: LandValueSim) {
    // SAFETY: all fields, iterators, histogram bins and game calls belong to the live simulator.
    unsafe {
        let count = lv.count();
        lv.set_mean_altitude(0.0);
        if count > 0 {
            lv.set_mean_altitude((F80::from_i32(lv.sum()) / F80::from_i32(count)).to_f32());
        } else if lv.ignore_zoning() == 0 {
            game::scan_reset(lv.tract_iterator(), 0, 0);
            lv.set_sum(0);
            lv.set_count(0);
            lv.set_ignore_zoning(1);
            clear_histogram(lv);
            lv.set_pipeline_state(3);
            return;
        }
        let (mut best, mut peak) = (0i32, 0i32);
        let len = histogram_len(lv);
        let mut bin = lv.histogram_begin();
        for i in 0..len {
            let n = read_unaligned(bin);
            if n > peak {
                peak = n;
                best = i;
            }
            bin = bin.wrapping_add(1);
        }
        let shift = lv.histogram_shift();
        let mode = 1i32
            .wrapping_shl(shift.wrapping_sub(1))
            .wrapping_add(best.wrapping_shl(shift));
        let wide = F80::from_i32(mode);
        lv.set_mode_altitude(wide.to_f32());
        if wide.compare(F80::from_f32(lv.min_altitude())).less() {
            lv.set_mode_altitude(lv.min_altitude());
        }
        let cmp = F80::from_f32(lv.mode_altitude()).compare(F80::from_f32(lv.max_altitude()));
        if !cmp.less() && !cmp.equal() && !cmp.unordered() {
            lv.set_mode_altitude(lv.max_altitude());
        }
        game::land_value_setup_altitude(lv.as_ptr());
        // This reset is performed directly, without a call to scan_reset.
        let scan = lv.slope_iterator();
        write_unaligned(scan.add(4).cast::<i32>(), 0);
        write_unaligned(scan.add(8).cast::<i32>(), 0);
        *scan = 0;
        if word(scan, 16) > 0 || word(scan, 20) > 0 || word(scan, 24) < 0 || word(scan, 28) < 0 {
            *scan = 2;
        }
        lv.set_pipeline_state(5);
    }
}

port! {
    /// Performs one state of the land value pass, retaining partial scans when the deadline expires.
    /// The five scan states each share their own adaptive pacing counters across simulator instances.
    #[port(address = 0x006B_E7B0, owner = "landvalue", name = "cSC4LandValueSimulator::DoIdle")]
    pub unsafe extern "thiscall" fn fc4p_006be7b0(this: *mut u8, milliseconds: u32) -> u32 {
        // SAFETY: this is the game's simulator, with its live service pointers and allocated grids.
        unsafe {
            let lv = LandValueSim::from_ptr(this);
            let mut timer = MaybeUninit::<[u32; 6]>::uninit();
            let timer = timer.as_mut_ptr().cast::<u8>();
            game::idle_timer_new(timer, 4);
            game::idle_timer_start(timer);
            let state = lv.pipeline_state();
            let eax = match state {
                0 => {
                    let eax = game::scan_reset(lv.tract_iterator(), 0, 0);
                    lv.set_pipeline_state(1);
                    eax
                }
                2 => {
                    let scratch = vcall!(lv.scratch_grid(), 0x60 => fn() -> *mut u8);
                    let near = F80::from_i32(i32::from(lv.water_near())).to_f32();
                    let grid = lv.intrinsic_grid();
                    let radius = F80::from_i32(i32::from(lv.water_radius())).to_f32();
                    let count_z = vcall!(grid, 0x34 => fn() -> i32);
                    let count_x = vcall!(grid, 0x30 => fn() -> i32);
                    game::land_value_water_effect(scratch, count_x, count_z, scratch, 0, radius, near, 0.0);
                    lv.set_min_altitude(vcall_st0!(lv.terrain(), 0x60).to_f32());
                    lv.set_max_altitude(vcall_st0!(lv.terrain(), 0x5C).to_f32());
                    game::scan_reset(lv.tract_iterator(), 0, 0);
                    lv.set_sum(0);
                    lv.set_count(0);
                    lv.set_ignore_zoning(0);
                    let eax = clear_histogram(lv);
                    lv.set_pipeline_state(3);
                    eax
                }
                6 => {
                    let destination = lv.scratch_grid();
                    let source = vcall!(lv.intrinsic_grid(), 0x60 => fn() -> *mut u8);
                    let destination = vcall!(destination, 0x60 => fn() -> *mut u8);
                    let eax = game::grid_bytes_assign(destination, source);
                    lv.set_developer_cursor(read_unaligned(lv.developer_header().add(8).cast()));
                    lv.set_pipeline_state(7);
                    eax
                }
                7 => {
                    let node = lv.developer_cursor();
                    if node != lv.developer_header() {
                        let developer = read_unaligned(node.add(16).cast::<*mut u8>());
                        let mut context = [word(node, 20) as u32, lv.desirability_curve() as u32];
                        let source = vcall!(developer, 0x34 => fn() -> *mut u8);
                        game::land_value_process_developer(lv.scratch_grid(), source, 0x006B_D640,
                            context.as_mut_ptr().cast());
                        game::developer_cursor_next(this.add(0x70).cast())
                    } else {
                        let destination = lv.total_grid();
                        let source = vcall!(lv.scratch_grid(), 0x60 => fn() -> *mut u8);
                        let destination = vcall!(destination, 0x60 => fn() -> *mut u8);
                        game::grid_bytes_assign(destination, source);
                        let eax = game::scan_reset(lv.tract_iterator(), 0, 0);
                        lv.set_sum(0);
                        lv.set_count(0);
                        lv.set_pipeline_state(8);
                        eax
                    }
                }
                1 | 3 | 4 | 5 | 8 => {
                    let (bit, pacing) = match state {
                        1 => (1, game::idle_base_pacing()),
                        3 => (2, game::idle_statistics_pacing()),
                        5 => (4, game::idle_slope_pacing()),
                        4 => (8, game::idle_altitude_pacing()),
                        _ => (16, game::idle_average_pacing()),
                    };
                    if *game::idle_pacing_flags() & bit == 0 {
                        *game::idle_pacing_flags() |= bit;
                        *pacing = [1024, 0];
                    }
                    let mut budget = MaybeUninit::<[u32; 6]>::uninit();
                    let budget = budget.as_mut_ptr().cast::<u8>();
                    game::idle_budget_new(budget, milliseconds, pacing.cast());
                    let mut first = core::ptr::null_mut();
                    let mut second = core::ptr::null_mut();
                    let mut shift = 0;
                    let mut lot_developer = core::ptr::null_mut();
                    match state {
                        1 => first = vcall!(lv.scratch_grid(), 0x60 => fn() -> *mut u8),
                        4 => {
                            first = vcall!(lv.scratch_grid(), 0x60 => fn() -> *mut u8);
                            second = vcall!(lv.intrinsic_grid(), 0x60 => fn() -> *mut u8);
                        }
                        5 => {
                            first = vcall!(lv.slope_grid(), 0x60 => fn() -> *mut u8);
                            shift = vcall!(lv.slope_grid(), 0x2C => fn() -> u32);
                            lot_developer = vcall!(*game::idle_city(), 0x90 => fn() -> *mut u8);
                        }
                        8 => first = vcall!(lv.total_grid(), 0x60 => fn() -> *mut u8),
                        _ => {}
                    }
                    let (mut x, mut z) = (0i32, 0i32);
                    while !budget_stopped(budget) {
                        let next = if state == 4 && lv.altitude_curve().is_null() {
                            false
                        } else {
                            let scan = if state == 5 { lv.slope_iterator() } else { lv.tract_iterator() };
                            game::scan_next(scan, &mut x, &mut z) as u8 != 0
                        };
                        if !next {
                            match state {
                                1 => lv.set_pipeline_state(2),
                                3 => statistics_done(lv),
                                4 | 5 => {
                                    game::scan_reset(lv.tract_iterator(), 0, 0);
                                    lv.set_pipeline_state(if state == 5 { 4 } else { 6 });
                                }
                                8 => {
                                    let count = lv.count();
                                    let average = if count > 0 { lv.sum().wrapping_div(count) as u8 } else { 0 };
                                    lv.set_average(average);
                                    vcall!(*game::idle_history(), 0x30 => fn(u32, u32), 0x6A66_3AC8, u32::from(average));
                                    lv.set_pipeline_state(9);
                                    game::land_value_end_cycle(this);
                                }
                                _ => {}
                            }
                            break;
                        }
                        match state {
                            1 => {
                                let land = game::land_value_tract_has_land(this, x, z) as u8 != 0;
                                *cell::<u8>(first, x, z) = if land { lv.base_value() } else { 0 };
                            }
                            3 => {
                                let land = game::land_value_tract_has_land(this, x, z) as u8 != 0;
                                let altitude = tract_altitude(lv, x, z);
                                if land {
                                    if F80::from_f32(altitude).compare(F80::from_f32(lv.min_altitude())).less() {
                                        lv.set_min_altitude(altitude);
                                    }
                                    let cmp = F80::from_f32(altitude).compare(F80::from_f32(lv.max_altitude()));
                                    if !cmp.less() && !cmp.equal() && !cmp.unordered() {
                                        lv.set_max_altitude(altitude);
                                    }
                                }
                                let zoned = lv.ignore_zoning() != 0 || game::land_value_tract_is_zoned(this, x, z) as u8 != 0;
                                if zoned && land {
                                    let sum = (F80::from_i32(lv.sum()) + F80::from_f32(altitude)).ftol() as i32;
                                    lv.set_sum(sum);
                                    lv.set_count(lv.count().wrapping_add(1));
                                    let mut bin = (F80::from_f32(altitude).ftol() as i32).wrapping_shr(lv.histogram_shift());
                                    if bin < 0 { bin = 0; }
                                    let len = histogram_len(lv);
                                    if bin as u32 >= len as u32 { bin = len.wrapping_sub(1); }
                                    let p = lv.histogram_begin().wrapping_offset(bin as isize);
                                    write_unaligned(p, read_unaligned(p).wrapping_add(1));
                                }
                            }
                            4 => {
                                let altitude = tract_altitude(lv, x, z);
                                let effect = game::land_value_altitude_effect(this, altitude) as u8;
                                let value = i32::from(*cell::<u8>(first, x, z)) + i32::from(effect);
                                *cell::<u8>(second, x, z) = value.clamp(0, 255) as u8;
                            }
                            5 => {
                                let rect = [x.wrapping_shl(shift), z.wrapping_shl(shift),
                                    x.wrapping_add(1).wrapping_shl(shift).wrapping_sub(1),
                                    z.wrapping_add(1).wrapping_shl(shift).wrapping_sub(1)];
                                let destination = cell::<f32>(first, x, z);
                                let slope = vcall_st0!(lot_developer, 0x74, rect.as_ptr(), 1u32);
                                write_unaligned(destination, slope.to_f32());
                            }
                            8 if (lv.ignore_zoning() != 0 || game::land_value_tract_is_zoned(this, x, z) as u8 != 0)
                                && game::land_value_tract_has_land(this, x, z) as u8 != 0 => {
                                lv.set_sum(lv.sum().wrapping_add(i32::from(*cell::<u8>(first, x, z))));
                                lv.set_count(lv.count().wrapping_add(1));
                            }
                            _ => {}
                        }
                    }
                    game::idle_budget_finish(budget)
                }
                _ => state,
            };
            finish_timer(timer, eax)
        }
    }
}

/// This file's replacements.
pub const PORTS: &[crate::port::Port] = &[fc4p_006be7b0::PORT];

#[cfg(test)]
mod tests {
    //! The core pipeline and terrain-change cases, exercised with the original object and vector layouts.
    use super::*;
    use crate::testing::{fake, fake_data, Fake, GameFpu, Object};
    use std::cell::RefCell;

    #[derive(Clone, Debug, PartialEq)]
    enum Call {
        Timer(u32),
        Start,
        Budget(u32),
        Expired,
        Finish,
        Reset(usize, i32, i32),
        Next(usize),
        Land(i32, i32),
        Grid(u32),
        Shift,
        Centre(i32, i32),
        Altitude(f32, f32),
        Zone(i32, i32),
        Max,
        Min,
        Water(i32, i32, u32, f32, f32, f32),
        Setup,
        LotDeveloper,
        Slope([i32; 4], u32),
        Copy(usize, usize),
        Developer,
        Process(usize, u32, usize),
        Advance,
        History(u32, u32),
        End(u32),
        Curve(f32),
    }

    struct Inputs {
        calls: Vec<Call>,
        land: [bool; 2],
        zoned: [bool; 2],
        altitudes: [f32; 2],
        slope: f32,
        stride: u32,
        checks_left: i32,
        stopped: u8,
        developer: usize,
    }
    impl Default for Inputs {
        fn default() -> Self {
            Self {
                calls: vec![],
                land: [true; 2],
                zoned: [true; 2],
                altitudes: [228.071_06, 300.0],
                slope: 8.655_487,
                stride: 1024,
                checks_left: i32::MAX,
                stopped: 0,
                developer: 0,
            }
        }
    }
    thread_local! { static INPUTS: RefCell<Inputs> = RefCell::new(Inputs::default()); }
    fn record(c: Call) {
        INPUTS.with(|s| s.borrow_mut().calls.push(c));
    }
    fn calls() -> Vec<Call> {
        INPUTS.with(|s| core::mem::take(&mut s.borrow_mut().calls))
    }
    fn edit(f: impl FnOnce(&mut Inputs)) {
        INPUTS.with(|s| f(&mut s.borrow_mut()));
    }

    extern "thiscall" fn timer_new(_p: *mut u8, kind: u32) {
        record(Call::Timer(kind));
    }
    extern "thiscall" fn timer_start(_p: *mut u8) {
        record(Call::Start);
    }
    #[unsafe(naked)]
    unsafe extern "thiscall" fn timer_finish(_p: *mut u8) {
        core::arch::naked_asm!("ret");
    }
    extern "thiscall" fn budget_new(p: *mut u8, ms: u32, shared: *mut u32) -> u32 {
        record(Call::Budget(ms));
        // SAFETY: the port provides a six-word budget and the game's shared pair.
        unsafe {
            write_unaligned(p.add(8).cast::<u32>(), 0);
            let (stride, stopped) = INPUTS.with(|s| {
                let s = s.borrow();
                (s.stride, s.stopped)
            });
            write_unaligned(p.add(12).cast::<u32>(), stride);
            *p.add(16) = stopped;
            write_unaligned(p.add(20).cast::<*mut u32>(), shared);
        }
        p as u32
    }
    extern "thiscall" fn expired(_p: *mut u8) -> u32 {
        record(Call::Expired);
        INPUTS.with(|s| {
            let mut s = s.borrow_mut();
            s.checks_left -= 1;
            u32::from(s.checks_left < 0)
        })
    }
    extern "thiscall" fn budget_finish(p: *mut u8) -> u32 {
        record(Call::Finish);
        // SAFETY: budget_new initialized this counter.
        unsafe { word(p, 8) as u32 }
    }
    extern "thiscall" fn reset(p: *mut u8, x: i32, z: i32) -> u32 {
        record(Call::Reset(p as usize, x, z));
        // SAFETY: a 32-byte scanning iterator.
        unsafe {
            write_unaligned(p.add(4).cast::<i32>(), x);
            write_unaligned(p.add(8).cast::<i32>(), z);
            *p = if x < word(p, 16) || z < word(p, 20) || x > word(p, 24) || z > word(p, 28) {
                2
            } else {
                0
            };
        }
        x as u32
    }
    extern "thiscall" fn next(p: *mut u8, x: *mut i32, z: *mut i32) -> u32 {
        record(Call::Next(p as usize));
        // SAFETY: the iterator and both output pointers are live stack or fixture memory.
        unsafe {
            if *p == 2 {
                return 0;
            }
            if *p == 1 {
                let mut nx = word(p, 4).wrapping_add(word(p, 12));
                let mut nz = word(p, 8);
                if nx > word(p, 24) {
                    nx = word(p, 16);
                    nz = nz.wrapping_add(word(p, 12));
                }
                write_unaligned(p.add(4).cast::<i32>(), nx);
                write_unaligned(p.add(8).cast::<i32>(), nz);
                if nz > word(p, 28) {
                    *p = 2;
                    return 0;
                }
            }
            *p = 1;
            *x = word(p, 4);
            *z = word(p, 8);
        }
        1
    }
    extern "thiscall" fn land(_p: *mut u8, x: i32, z: i32) -> u32 {
        record(Call::Land(x, z));
        INPUTS.with(|s| u32::from(s.borrow().land[x as usize]))
    }

    #[repr(C)]
    struct Row {
        begin: *mut u8,
        end: *mut u8,
        capacity: *mut u8,
    }
    #[repr(C)]
    struct Vector {
        begin: *mut Row,
        end: *mut Row,
        capacity: *mut Row,
    }
    #[repr(C)]
    struct Grid {
        vt: *const usize,
        id: u32,
        data: Box<Vector>,
        rows: Box<[Row; 2]>,
        values: Box<[u32; 2]>,
    }
    impl Grid {
        fn new(vt: *const usize, id: u32) -> Box<Self> {
            let mut values = Box::new([0u32; 2]);
            let mut rows = Box::new(core::array::from_fn(|i| {
                let p = values.as_mut_ptr().wrapping_add(i).cast::<u8>();
                Row {
                    begin: p,
                    end: p.wrapping_add(4),
                    capacity: p.wrapping_add(4),
                }
            }));
            let p = rows.as_mut_ptr();
            Box::new(Self {
                vt,
                id,
                data: Box::new(Vector {
                    begin: p,
                    end: p.wrapping_add(2),
                    capacity: p.wrapping_add(2),
                }),
                rows,
                values,
            })
        }
        fn ptr(&mut self) -> *mut u8 {
            (self as *mut Self).cast()
        }
        fn vector(&mut self) -> *mut u8 {
            (&mut *self.data as *mut Vector).cast()
        }
        fn bytes(&self) -> [u8; 2] {
            [self.values[0] as u8, self.values[1] as u8]
        }
    }
    extern "thiscall" fn grid_data(g: *mut Grid) -> *mut u8 {
        // SAFETY: the fixture's Grid object.
        unsafe {
            record(Call::Grid((*g).id));
            (*g).vector()
        }
    }
    extern "thiscall" fn grid_shift(g: *mut Grid) -> u32 {
        record(Call::Shift);
        // SAFETY: the fixture's Grid object.
        if unsafe { (*g).id } == 3 {
            3
        } else {
            0
        }
    }
    extern "thiscall" fn grid_size(_g: *mut Grid) -> i32 {
        1
    }
    extern "thiscall" fn count_x(_g: *mut Grid) -> i32 {
        2
    }
    extern "thiscall" fn count_z(_g: *mut Grid) -> i32 {
        1
    }
    extern "thiscall" fn centre(_g: *mut Grid, x: i32, z: i32, px: *mut f32, pz: *mut f32) {
        record(Call::Centre(x, z));
        // SAFETY: the two outputs provided by the port.
        unsafe {
            *px = x as f32;
            *pz = z as f32;
        }
    }
    extern "thiscall" fn altitude(_p: *mut u8, x: f32, z: f32) -> f32 {
        record(Call::Altitude(x, z));
        INPUTS.with(|s| s.borrow().altitudes[x as usize])
    }
    extern "thiscall" fn max_height(_p: *mut u8) -> f32 {
        record(Call::Max);
        2500.0
    }
    extern "thiscall" fn min_height(_p: *mut u8) -> f32 {
        record(Call::Min);
        20.0
    }
    extern "thiscall" fn zone(_p: *mut u8, x: i32, z: i32) -> u32 {
        record(Call::Zone(x, z));
        INPUTS.with(|s| u32::from(s.borrow().zoned[x as usize]))
    }
    extern "C" fn water(a: *mut u8, nx: i32, nz: i32, b: *mut u8, source: u32, radius: f32, near: f32, far: f32) {
        assert_eq!(a, b);
        record(Call::Water(nx, nz, source, radius, near, far));
    }
    extern "thiscall" fn setup(p: *mut u8) {
        record(Call::Setup);
        // SAFETY: the simulator supplied to the port.
        let lv = unsafe { LandValueSim::from_ptr(p) };
        lv.set_pivot(lv.mode_altitude());
        lv.set_low_range(32.0);
        lv.set_high_range(32.0);
        lv.set_top_index(100);
    }
    extern "thiscall" fn curve(_p: *mut u8, x: f32) -> f32 {
        record(Call::Curve(x));
        50.0
    }
    extern "thiscall" fn lot(_p: *mut u8) -> *mut u8 {
        record(Call::LotDeveloper);
        INPUTS.with(|s| s.borrow().developer as *mut u8)
    }
    extern "thiscall" fn slope_sample(_p: *mut u8, rect: *const [i32; 4], water: u32) -> f32 {
        // SAFETY: the port's four coordinate rectangle.
        record(Call::Slope(unsafe { *rect }, water));
        INPUTS.with(|s| s.borrow().slope)
    }
    extern "thiscall" fn copy(dst: *mut u8, src: *mut u8) -> u32 {
        record(Call::Copy(dst as usize, src as usize));
        // SAFETY: fixture vectors with two single-cell rows.
        unsafe {
            for x in 0..2 {
                *cell::<u8>(dst, x, 0) = *cell::<u8>(src, x, 0);
            }
        }
        dst as u32
    }
    extern "thiscall" fn dev_map(_p: *mut u8) -> *mut u8 {
        record(Call::Developer);
        0x1234usize as *mut u8
    }
    extern "C" fn process(dst: *mut u8, src: *mut u8, callback: usize, context: *mut u32) {
        assert_eq!(src as usize, 0x1234);
        assert_eq!(callback, 0x006B_D640);
        // SAFETY: the two-word callback context.
        unsafe {
            record(Call::Process(dst as usize, *context, *context.add(1) as usize));
        }
    }
    extern "thiscall" fn advance(cursor: *mut *mut u8) -> u32 {
        record(Call::Advance);
        // SAFETY: the fixture node uses its first word to store the next node.
        unsafe {
            *cursor = read_unaligned((*cursor).cast());
        }
        0x1234_5600
    }
    extern "thiscall" fn history(_p: *mut u8, id: u32, v: u32) {
        record(Call::History(id, v));
    }
    extern "thiscall" fn end(p: *mut u8) {
        // SAFETY: the port passes its simulator.
        record(Call::End(unsafe { LandValueSim::from_ptr(p) }.pipeline_state()));
    }

    struct Service {
        _vt: Box<[usize; 40]>,
        object: Box<[usize; 1]>,
    }
    impl Service {
        fn new(slots: &[(usize, usize)]) -> Self {
            let mut vt = Box::new([0usize; 40]);
            for &(offset, f) in slots {
                vt[offset / 4] = f;
            }
            let object = Box::new([vt.as_ptr() as usize]);
            Self { _vt: vt, object }
        }
        fn ptr(&mut self) -> *mut u8 {
            self.object.as_mut_ptr().cast()
        }
    }
    struct World {
        lv: Object,
        scan: Object,
        slope_scan: Object,
        intrinsic: Box<Grid>,
        scratch: Box<Grid>,
        total: Box<Grid>,
        slope: Box<Grid>,
        histogram: Box<[i32; 128]>,
        header: Box<[usize; 6]>,
        node: Box<[usize; 6]>,
        _vtable: Box<[usize; 40]>,
        _services: Vec<Service>,
        _globals: Box<[*mut u8; 2]>,
        _flags: Box<u32>,
        pacing: Box<[[u32; 2]; 5]>,
        _guards: Vec<Fake>,
        _fpu: GameFpu,
    }
    impl World {
        fn new() -> Self {
            INPUTS.with(|s| *s.borrow_mut() = Inputs::default());
            let mut vt = Box::new([0usize; 40]);
            for (offset, f) in [
                (0x28, grid_size as *const () as usize),
                (0x2c, grid_shift as *const () as usize),
                (0x30, count_x as *const () as usize),
                (0x34, count_z as *const () as usize),
                (0x54, centre as *const () as usize),
                (0x60, grid_data as *const () as usize),
            ] {
                vt[offset / 4] = f;
            }
            let intrinsic = Grid::new(vt.as_ptr(), 0);
            let scratch = Grid::new(vt.as_ptr(), 1);
            let total = Grid::new(vt.as_ptr(), 2);
            let slope = Grid::new(vt.as_ptr(), 3);
            let mut services = vec![
                Service::new(&[
                    (0x34, altitude as *const () as usize),
                    (0x60, max_height as *const () as usize),
                    (0x5c, min_height as *const () as usize),
                ]),
                Service::new(&[(0x30, zone as *const () as usize)]),
                Service::new(&[(0x90, lot as *const () as usize)]),
                Service::new(&[(0x74, slope_sample as *const () as usize)]),
                Service::new(&[(0x30, history as *const () as usize)]),
                Service::new(&[(0x34, dev_map as *const () as usize)]),
            ];
            let terrain_ptr = services[0].ptr();
            let zones_ptr = services[1].ptr();
            let mut globals = Box::new([services[2].ptr(), services[4].ptr()]);
            edit(|s| s.developer = services[3].ptr() as usize);
            let mut flags = Box::new(0u32);
            let mut pacing = Box::new([[0u32; 2]; 5]);
            let mut guards = vec![];
            for (a, f) in [
                (0x88fedf, timer_new as *const () as usize),
                (0x8905c4, timer_start as *const () as usize),
                (0xa6d837, timer_finish as *const () as usize),
                (0x68cc20, budget_new as *const () as usize),
                (0x68bf80, expired as *const () as usize),
                (0x68b960, budget_finish as *const () as usize),
                (0x60c860, reset as *const () as usize),
                (0x60c8d0, next as *const () as usize),
                (0x6bd5c0, land as *const () as usize),
                (0x5e5390, water as *const () as usize),
                (0x6bd750, setup as *const () as usize),
                (0x5f4250, curve as *const () as usize),
                (0x5117d0, copy as *const () as usize),
                (0x6bda30, process as *const () as usize),
                (0x5d17c0, advance as *const () as usize),
                (0x6bd6b0, end as *const () as usize),
            ] {
                guards.push(fake(a, f));
            }
            guards.push(fake_data(0xb4910c, (&mut *flags as *mut u32).cast()));
            for (i, a) in [0xb49104, 0xb490fc, 0xb490ec, 0xb490f4, 0xb490e4]
                .into_iter()
                .enumerate()
            {
                guards.push(fake_data(a, pacing[i].as_mut_ptr().cast()));
            }
            // These globals contain interface pointers, rather than the objects themselves.
            guards.push(fake_data(0xb43d68, globals.as_mut_ptr().cast()));
            guards.push(fake_data(0xb43dc0, globals.as_mut_ptr().wrapping_add(1).cast()));
            let mut w = Self {
                lv: Object::new(0x100),
                scan: Object::new(0x20),
                slope_scan: Object::new(0x20),
                intrinsic,
                scratch,
                total,
                slope,
                histogram: Box::new([0; 128]),
                header: Box::new([0; 6]),
                node: Box::new([0; 6]),
                _vtable: vt,
                _services: services,
                _globals: globals,
                _flags: flags,
                pacing,
                _guards: guards,
                _fpu: GameFpu::new(),
            };
            let lv = w.sim();
            lv.set_intrinsic_grid(w.intrinsic.ptr());
            lv.set_scratch_grid(w.scratch.ptr());
            lv.set_total_grid(w.total.ptr());
            lv.set_slope_grid(w.slope.ptr());
            lv.set_terrain(terrain_ptr);
            lv.set_zone_manager(zones_ptr);
            lv.set_tract_iterator(w.scan.ptr());
            lv.set_slope_iterator(w.slope_scan.ptr());
            lv.set_histogram_begin(w.histogram.as_mut_ptr());
            lv.set_histogram_end(w.histogram.as_mut_ptr().wrapping_add(128));
            lv.set_histogram_shift(5);
            lv.set_base_value(1);
            lv.set_water_near(50);
            lv.set_water_radius(10);
            lv.set_altitude_curve(terrain_ptr);
            lv.set_desirability_curve(terrain_ptr);
            lv.set_developer_header(w.header.as_mut_ptr().cast());
            w.header[2] = w.header.as_mut_ptr() as usize;
            // SAFETY: both scanning iterators occupy 32 bytes.
            unsafe {
                for p in [w.scan.ptr(), w.slope_scan.ptr()] {
                    write_unaligned(p.add(12).cast::<i32>(), 1);
                    write_unaligned(p.add(24).cast::<i32>(), 1);
                }
            }
            w
        }
        fn sim(&mut self) -> LandValueSim {
            // SAFETY: the fixture owns 0x100 bytes covering the simulator layout.
            unsafe { LandValueSim::from_ptr(self.lv.ptr()) }
        }
        fn step(&mut self) -> u32 {
            // SAFETY: all fixture pointers and callees stay live throughout this call.
            unsafe { fc4p_006be7b0(self.lv.ptr(), 100) }
        }
    }

    #[test]
    fn pipeline_advances_one_state_per_delivery_and_calls_services_in_order() {
        let mut w = World::new();
        let scan = w.scan.ptr() as usize;
        assert_eq!(w.step(), 0);
        assert_eq!(w.sim().pipeline_state(), 1);
        assert_eq!(calls(), [Call::Timer(4), Call::Start, Call::Reset(scan, 0, 0)]);
        w.step();
        assert_eq!(w.scratch.bytes(), [1, 1]);
        assert_eq!(w.sim().pipeline_state(), 2);
        assert_eq!(
            calls(),
            [
                Call::Timer(4),
                Call::Start,
                Call::Budget(100),
                Call::Grid(1),
                Call::Expired,
                Call::Next(scan),
                Call::Land(0, 0),
                Call::Expired,
                Call::Next(scan),
                Call::Land(1, 0),
                Call::Expired,
                Call::Next(scan),
                Call::Finish
            ]
        );
        w.histogram.fill(99);
        assert_eq!(w.step(), 128);
        assert_eq!(
            calls(),
            [
                Call::Timer(4),
                Call::Start,
                Call::Grid(1),
                Call::Water(2, 1, 0, 10.0, 50.0, 0.0),
                Call::Max,
                Call::Min,
                Call::Reset(scan, 0, 0)
            ]
        );
        assert_eq!((w.sim().min_altitude(), w.sim().max_altitude()), (2500.0, 20.0));
        assert_eq!((w.sim().sum(), w.sim().count(), w.sim().ignore_zoning()), (0, 0, 0));
        assert!(w.histogram.iter().all(|&v| v == 0));
        w.step();
        assert_eq!(w.sim().pipeline_state(), 5);
        assert_eq!(
            (
                w.sim().sum(),
                w.sim().count(),
                w.sim().mean_altitude(),
                w.sim().mode_altitude()
            ),
            (528, 2, 264.0, 240.0)
        );
        let log = calls();
        assert_eq!(
            &log[5..10],
            &[
                Call::Land(0, 0),
                Call::Centre(0, 0),
                Call::Altitude(0.0, 0.0),
                Call::Shift,
                Call::Zone(0, 0)
            ]
        );
        assert!(log.contains(&Call::Setup));
        w.step();
        assert_eq!(w.sim().pipeline_state(), 4);
        assert_eq!(w.slope.values.map(f32::from_bits), [8.655_487; 2]);
        let log = calls();
        assert!(log.contains(&Call::Slope([0, 0, 7, 7], 1)));
        assert!(log.contains(&Call::Slope([8, 0, 15, 7], 1)));
        w.step();
        assert_eq!(w.sim().pipeline_state(), 6);
        assert_eq!(w.intrinsic.bytes(), [51, 51]);
        calls();
        let scratch = w.scratch.vector() as usize;
        let intrinsic = w.intrinsic.vector() as usize;
        assert_eq!(w.step(), scratch as u32);
        assert_eq!(w.sim().pipeline_state(), 7);
        assert_eq!(
            calls(),
            [
                Call::Timer(4),
                Call::Start,
                Call::Grid(0),
                Call::Grid(1),
                Call::Copy(scratch, intrinsic)
            ]
        );
        w.step();
        assert_eq!(w.sim().pipeline_state(), 8);
        assert_eq!(w.total.bytes(), [51, 51]);
        calls();
        w.step();
        assert_eq!((w.sim().pipeline_state(), w.sim().average()), (9, 51));
        let log = calls();
        assert_eq!(
            &log[log.len() - 3..],
            &[Call::History(0x6a663ac8, 51), Call::End(9), Call::Finish]
        );
        w.step();
        assert_eq!(calls(), [Call::Timer(4), Call::Start]);
    }

    #[test]
    fn fallback_retains_first_scan_extrema_and_zeroes_mean_before_returning() {
        let mut w = World::new();
        for _ in 0..3 {
            w.step();
        }
        calls();
        edit(|s| s.zoned = [false; 2]);
        w.sim().set_mean_altitude(999.0);
        w.step();
        let lv = w.sim();
        assert_eq!(
            (
                lv.pipeline_state(),
                lv.ignore_zoning(),
                lv.sum(),
                lv.count(),
                lv.mean_altitude()
            ),
            (3, 1, 0, 0, 0.0)
        );
        assert_eq!((lv.min_altitude(), lv.max_altitude()), (228.071_06, 300.0));
        assert!(w.histogram.iter().all(|&v| v == 0));
        calls();
        edit(|s| s.altitudes[0] = 270.0);
        w.step();
        assert_eq!(
            (
                lv.pipeline_state(),
                lv.sum(),
                lv.count(),
                lv.mean_altitude(),
                lv.mode_altitude()
            ),
            (5, 570, 2, 285.0, 272.0)
        );
        assert_eq!((lv.min_altitude(), lv.max_altitude()), (228.071_06, 300.0));
        assert!(!calls().iter().any(|c| matches!(c, Call::Zone(..))));
    }

    #[test]
    fn partial_statistics_keep_processed_contributions_and_the_original_read_order() {
        let mut w = World::new();
        for _ in 0..3 {
            w.step();
        }
        calls();
        edit(|s| s.checks_left = 1);
        w.step();
        let lv = w.sim();
        assert_eq!((lv.pipeline_state(), lv.sum(), lv.count()), (3, 228, 1));
        let scan = w.scan.ptr() as usize;
        assert_eq!(
            calls(),
            [
                Call::Timer(4),
                Call::Start,
                Call::Budget(100),
                Call::Expired,
                Call::Next(scan),
                Call::Land(0, 0),
                Call::Centre(0, 0),
                Call::Altitude(0.0, 0.0),
                Call::Shift,
                Call::Zone(0, 0),
                Call::Expired,
                Call::Finish
            ]
        );
        edit(|s| {
            s.altitudes[0] = 270.0;
            s.checks_left = i32::MAX;
        });
        w.step();
        assert_eq!((lv.sum(), lv.count(), lv.mean_altitude()), (528, 2, 264.0));
    }

    #[test]
    fn written_slope_and_intrinsic_cells_survive_edits_between_slices() {
        let mut w = World::new();
        for _ in 0..4 {
            w.step();
        }
        edit(|s| s.checks_left = 1);
        w.step();
        edit(|s| {
            s.slope = 13.074_409;
            s.checks_left = i32::MAX;
        });
        w.step();
        assert_eq!(w.slope.values.map(f32::from_bits), [8.655_487, 13.074_409]);
        edit(|s| s.checks_left = 1);
        w.step();
        assert_eq!(w.sim().pipeline_state(), 4);
        assert_eq!(w.intrinsic.bytes(), [51, 0]);
        edit(|s| {
            s.altitudes = [-100.0; 2];
            s.checks_left = i32::MAX;
        });
        w.step();
        assert_eq!(w.intrinsic.bytes(), [51, 1]);
        assert_eq!(w.sim().mean_altitude(), 264.0);
    }

    #[test]
    fn no_curve_keeps_intrinsic_and_checks_the_budget_before_resetting() {
        let mut w = World::new();
        w.sim().set_pipeline_state(4);
        w.sim().set_altitude_curve(core::ptr::null_mut());
        *w.intrinsic.values = [37, 91];
        edit(|s| s.checks_left = 0);
        w.step();
        assert_eq!(w.sim().pipeline_state(), 4);
        assert_eq!(w.intrinsic.bytes(), [37, 91]);
        assert!(!calls()
            .iter()
            .any(|c| matches!(c, Call::Next(..) | Call::Reset(..) | Call::Altitude(..))));
        edit(|s| s.checks_left = i32::MAX);
        w.step();
        assert_eq!(w.sim().pipeline_state(), 6);
        assert_eq!(w.intrinsic.bytes(), [37, 91]);
        assert!(!calls().iter().any(|c| matches!(c, Call::Next(..) | Call::Altitude(..))));
    }

    #[test]
    fn developer_state_processes_one_node_and_advances_its_stored_cursor() {
        let mut w = World::new();
        let header = w.header.as_mut_ptr().cast::<u8>();
        w.node[0] = header as usize;
        w.node[4] = w._services[5].ptr() as usize;
        w.node[5] = 2.5f32.to_bits() as usize;
        let node = w.node.as_mut_ptr().cast();
        let lv = w.sim();
        lv.set_developer_cursor(node);
        lv.set_pipeline_state(7);
        let curve = lv.desirability_curve() as usize;
        let scratch_grid = lv.scratch_grid() as usize;
        assert_eq!(w.step(), 0x1234_5600);
        assert_eq!((lv.pipeline_state(), lv.developer_cursor()), (7, header));
        assert_eq!(
            calls(),
            [
                Call::Timer(4),
                Call::Start,
                Call::Developer,
                Call::Process(scratch_grid, 2.5f32.to_bits(), curve),
                Call::Advance
            ]
        );
        w.step();
        assert_eq!(lv.pipeline_state(), 8);
        assert_eq!((lv.sum(), lv.count()), (0, 0));
    }

    #[test]
    fn average_short_circuits_land_tests_and_publishes_before_end_notification() {
        let mut w = World::new();
        let lv = w.sim();
        lv.set_pipeline_state(8);
        *w.total.values = [10, 200];
        edit(|s| s.zoned = [false, true]);
        w.step();
        assert_eq!((lv.sum(), lv.count(), lv.average()), (200, 1, 200));
        let log = calls();
        assert!(!log.contains(&Call::Land(0, 0)));
        assert!(log.contains(&Call::Land(1, 0)));
        assert_eq!(
            &log[log.len() - 3..],
            &[Call::History(0x6a663ac8, 200), Call::End(9), Call::Finish]
        );
    }

    #[test]
    fn water_only_statistics_do_not_count_but_still_test_zoning() {
        let mut w = World::new();
        for _ in 0..3 {
            w.step();
        }
        calls();
        edit(|s| s.land = [false; 2]);
        w.step();
        let lv = w.sim();
        assert_eq!((lv.pipeline_state(), lv.ignore_zoning()), (3, 1));
        let log = calls();
        assert!(log.contains(&Call::Zone(0, 0)));
        assert!(log.contains(&Call::Zone(1, 0)));
        w.step();
        assert_eq!(lv.pipeline_state(), 5);
        assert_eq!(
            (
                lv.mean_altitude(),
                lv.mode_altitude(),
                lv.min_altitude(),
                lv.max_altitude()
            ),
            (0.0, 20.0, 2500.0, 20.0)
        );
        lv.set_pipeline_state(8);
        lv.set_count(-1);
        lv.set_sum(i32::MIN);
        edit(|s| s.checks_left = i32::MAX);
        w.step();
        assert_eq!(lv.average(), 0);
    }

    #[test]
    fn base_scan_assigns_zero_to_water_and_configured_base_to_land() {
        let mut w = World::new();
        w.sim().set_pipeline_state(1);
        w.sim().set_base_value(201);
        edit(|s| s.land = [false, true]);
        w.step();
        assert_eq!(w.scratch.bytes(), [0, 201]);
    }

    #[test]
    fn histogram_bins_clamp_ties_keep_the_first_and_integer_sum_uses_x87() {
        let mut w = World::new();
        let lv = w.sim();
        lv.set_pipeline_state(3);
        lv.set_ignore_zoning(1);
        lv.set_sum(16_777_217);
        lv.set_min_altitude(-100.0);
        lv.set_max_altitude(10_000.0);
        edit(|s| {
            s.altitudes = [-1.75, 1e30];
            s.checks_left = 1;
        });
        w.step();
        assert_eq!(lv.sum(), 16_777_215);
        assert_eq!(w.histogram[0], 1);
        edit(|s| s.checks_left = i32::MAX);
        w.step();
        // Out-of-range conversion keeps the low half of the game's integer indefinite.
        assert_eq!(w.histogram[0], 2);
        assert_eq!(lv.sum(), 0);
        assert_eq!(lv.pipeline_state(), 5);
        // Equal positive peaks retain the lowest index; negative counts never beat zero.
        lv.set_pipeline_state(3);
        lv.set_count(2);
        lv.set_sum(200);
        lv.set_min_altitude(-100.0);
        lv.set_max_altitude(1000.0);
        w.histogram.fill(-1);
        w.histogram[2] = 7;
        w.histogram[3] = 7;
        w.step();
        assert_eq!(lv.mode_altitude(), 80.0);
    }

    #[test]
    fn nan_altitudes_leave_extrema_unchanged_and_use_the_games_conversion() {
        let mut w = World::new();
        let lv = w.sim();
        lv.set_pipeline_state(3);
        lv.set_ignore_zoning(1);
        lv.set_min_altitude(50.0);
        lv.set_max_altitude(100.0);
        edit(|s| s.altitudes = [f32::NAN; 2]);
        w.step();
        assert_eq!(
            (lv.min_altitude(), lv.max_altitude(), lv.sum(), lv.count()),
            (50.0, 100.0, 0, 2)
        );
        assert_eq!(w.histogram[0], 2);
        assert_eq!(lv.mode_altitude(), 50.0);
    }

    #[test]
    fn pacing_initializes_only_the_selected_group_and_tests_before_consuming_a_tract() {
        let mut w = World::new();
        let lv = w.sim();
        lv.set_pipeline_state(1);
        edit(|s| s.checks_left = 0);
        assert_eq!(w.step(), 1024);
        assert_eq!(*w._flags, 1);
        assert_eq!(w.pacing[0], [1024, 0]);
        assert_eq!(w.pacing[1..], [[0; 2]; 4]);
        assert!(!calls().iter().any(|c| matches!(c, Call::Next(..))));
        // Non-default strides defer the clock test until the count is divisible by 1024.
        w.pacing[0] = [64, 19];
        edit(|s| {
            s.stride = 64;
            s.checks_left = 0;
        });
        w.step();
        assert_eq!(w.pacing[0], [64, 19]);
        assert_eq!(lv.pipeline_state(), 2);
        assert!(!calls().contains(&Call::Expired));
    }

    #[test]
    fn stopped_budget_skips_the_scan_and_state_values_outside_the_table_keep_eax() {
        let mut w = World::new();
        let lv = w.sim();
        lv.set_pipeline_state(3);
        edit(|s| {
            s.stopped = 1;
            s.stride = 1;
        });
        w.step();
        assert_eq!(lv.pipeline_state(), 3);
        assert_eq!(calls(), [Call::Timer(4), Call::Start, Call::Budget(100), Call::Finish]);
        for state in [9, 256, u32::MAX] {
            lv.set_pipeline_state(state);
            assert_eq!(w.step(), state);
            assert_eq!(calls(), [Call::Timer(4), Call::Start]);
        }
    }

    #[test]
    fn altitude_addition_saturates_and_slope_reset_uses_signed_bounds() {
        let mut w = World::new();
        let lv = w.sim();
        lv.set_pipeline_state(4);
        lv.set_pivot(240.0);
        lv.set_min_altitude(0.0);
        lv.set_low_range(32.0);
        lv.set_high_range(32.0);
        lv.set_top_index(100);
        *w.scratch.values = [250, 1];
        w.step();
        assert_eq!(w.intrinsic.bytes(), [255, 51]);
        lv.set_pipeline_state(3);
        lv.set_count(1);
        lv.set_sum(100);
        // Finish an exhausted tract scan and mark out-of-bounds zero exhausted in the slope iterator.
        unsafe {
            *w.scan.ptr() = 2;
            write_unaligned(w.slope_scan.ptr().add(16).cast::<i32>(), 1);
        }
        w.step();
        assert_eq!(lv.pipeline_state(), 5);
        assert_eq!(unsafe { *w.slope_scan.ptr() }, 2);
        let slope_scan = w.slope_scan.ptr() as usize;
        assert!(!calls().iter().any(|c| matches!(c,Call::Reset(p,_,_) if *p==slope_scan)));
    }

    #[test]
    fn histogram_upper_bin_and_empty_span_keep_the_original_write_address() {
        let mut w = World::new();
        let lv = w.sim();
        lv.set_pipeline_state(3);
        lv.set_ignore_zoning(1);
        lv.set_min_altitude(0.0);
        lv.set_max_altitude(10_000.0);
        edit(|s| {
            s.altitudes = [10_000.0; 2];
            s.checks_left = 1;
        });
        w.step();
        assert_eq!(w.histogram[127], 1);

        // Even an empty span selects the preceding word when a tract contributes.
        let mut storage = [41i32, 0];
        let begin = storage.as_mut_ptr().wrapping_add(1);
        lv.set_histogram_begin(begin);
        lv.set_histogram_end(begin);
        edit(|s| s.checks_left = 1);
        w.step();
        assert_eq!(storage, [42, 0]);
        assert_eq!(lv.pipeline_state(), 3);
    }

    #[test]
    fn zero_and_large_histogram_shifts_follow_machine_shift_counts() {
        for (shift, expected_mode) in [(0, i32::MIN), (32, i32::MIN), (33, 1), (u32::MAX, 1 << 30)] {
            let mut w = World::new();
            let lv = w.sim();
            lv.set_pipeline_state(3);
            lv.set_ignore_zoning(1);
            lv.set_histogram_shift(shift);
            lv.set_min_altitude(f32::NEG_INFINITY);
            lv.set_max_altitude(f32::INFINITY);
            edit(|s| s.altitudes = [0.0; 2]);
            w.step();
            assert_eq!(lv.mode_altitude(), F80::from_i32(expected_mode).to_f32());
            assert_eq!(w.histogram[0], 2);
        }
    }

    #[test]
    fn mean_division_loads_the_integer_sum_exactly_and_preserves_control_word() {
        let mut w = World::new();
        let lv = w.sim();
        lv.set_pipeline_state(3);
        lv.set_count(3);
        lv.set_sum(i32::MAX);
        lv.set_min_altitude(0.0);
        lv.set_max_altitude(100.0);
        // SAFETY: this iterator has ended, so the state completes without taking another coordinate.
        unsafe {
            *w.scan.ptr() = 2;
        }
        let cw = crate::x87::control_word();
        w.step();
        assert_eq!(lv.mean_altitude(), 715_827_904.0);
        assert_eq!(crate::x87::control_word(), cw);
    }
}
