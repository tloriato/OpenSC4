// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! One state of the daily residential pass, including its adaptive time slices.

use crate::port::Port;
use crate::x87::F80;
use core::mem::MaybeUninit;
use core::ptr::null_mut;

layout! {
    /// Fields used by the daily pass, relative to the primary interface.
    struct DailySim {
        0x20 => lots / set_lots: *mut u8,
        0x38 => proximity / set_proximity: [*mut u8; 3],
        0x58 => school_coverage / set_school_coverage: *mut *mut u8,
        0x64 => hospital_coverage / set_hospital_coverage: *mut *mut u8,
        0x70 => school_effect / set_school_effect: *mut u8,
        0x74 => hospital_effect / set_hospital_effect: *mut u8,
        0x94 => state / set_state: u32,
        0xA4 => population / set_population: i32,
        0xAC => school_population / set_school_population: i32,
        0xB0 => hospital_population / set_hospital_population: i32,
        0x19C => new_schools / set_new_schools: *mut u8,
        0x1A0 => new_hospitals / set_new_hospitals: *mut u8,
        0x1A4 => new_other / set_new_other: *mut u8,
        0x1C0 => other / set_other: *mut u8,
        0x2FC => school_curves / set_school_curves: *mut *mut u8,
        0x308 => hospital_curves / set_hospital_curves: *mut *mut u8,
        0x314 => school_sum / set_school_sum: u32,
        0x318 => hospital_sum / set_hospital_sum: u32,
        0x3C0 => tuning_read / set_tuning_read: u8,
        0x3C4 => temporary / set_temporary: *mut u8,
        0x3C8 => school_auto / set_school_auto: u8,
        0x3C9 => hospital_auto / set_hospital_auto: u8,
    }
}
layout! {
    /// Circular building lists have two links and an occupant pointer.
    struct Node {
        0 => next / set_next: *mut u8,
        4 => previous / set_previous: *mut u8,
        8 => occupant / set_occupant: *mut u8,
    }
}
layout! {
    /// A vector of pointers to building-list sentinels.
    struct Lists {
        0 => begin / set_begin: *mut *mut u8,
        4 => end / set_end: *mut *mut u8,
    }
}
layout! {
    /// The budget constructor initializes these pacing fields.
    struct Budget {
        8 => counter / set_counter: u32,
        12 => step / set_step: u32,
        16 => stopped / set_stopped: u8,
    }
}

game_fn! { fn allocate(size: u32) -> *mut u8 = "C" 0x0090_CF54; }
game_fn! { fn free(storage: *mut u8) -> u32 = "C" 0x0090_CF63; }
game_fn! { fn strike_check(this: *mut u8, department: *mut u8, lists: *mut u8, staff: u32, grade: u32, group: u32) = "thiscall" 0x006F_AFF0; }
game_fn! { fn clear_effects(this: *mut u8) = "thiscall" 0x006F_CB40; }
game_fn! { fn ordinance_effects(this: *mut u8, hospital: *mut u8, school: *mut u8) = "thiscall" 0x006F_E0F0; }
game_fn! { fn allocate_node(this: *mut u8, count: u32, hint: u32) -> *mut u8 = "thiscall" 0x0041_3DD0; }
game_fn! { fn clear_list(this: *mut u8) = "thiscall" 0x0041_3E60; }
game_fn! { fn framework() -> *mut u8 = "C" 0x0087_93EC; }
game_fn! { fn release_reference(this: *mut *mut u8) = "thiscall" 0x0041_1870; }
game_fn! { fn allocate_grid(size: u32) -> *mut u8 = "C" 0x005E_55E0; }
game_fn! { fn grid_new(this: *mut u8, id: u32) -> *mut u8 = "thiscall" 0x0045_DDB0; }
game_fn! { fn start_proximity(destination: *mut u8, source: *mut u8, key: u32) = "C" 0x005E_4FA0; }
game_fn! { fn start_high_proximity(destination: *mut u8, source: *mut u8, key: u32) = "C" 0x005E_5130; }
game_fn! { fn proximity_step(data: *mut u8, x: u32, z: u32) -> u32 = "C" 0x005E_4E80; }
game_fn! { fn copy_proximity(destination: *mut u8, source: *mut u8, callback: u32, context: u32) = "C" 0x006F_8970; }
game_fn! { fn budget_new(this: *mut u8, milliseconds: u32, pacing: *mut [u32; 2]) = "thiscall" 0x0068_CC20; }
game_fn! { fn budget_expired(this: *mut u8) -> u32 = "thiscall" 0x0068_BF80; }
game_fn! { fn budget_end(this: *mut u8) = "thiscall" 0x0068_B960; }
game_fn! { fn reset_population(this: *mut u8) = "thiscall" 0x006F_6EE0; }
game_fn! { fn read_population_tuning(this: *mut u8, tuning: *mut u8) = "thiscall" 0x006F_C0C0; }
game_fn! { fn scan_next(this: *mut u8, x: *mut u32, z: *mut u32) = "thiscall" 0x0060_C8D0; }
game_fn! { fn population_change(this: *mut u8, tuning: *mut u8, x: u32, z: u32) = "thiscall" 0x006F_A450; }
game_fn! { fn scan_reset(this: *mut u8, x: u32, z: u32) = "thiscall" 0x0060_C860; }
game_fn! { fn post_population(this: *mut u8) = "thiscall" 0x006F_8070; }
game_fn! { fn update_coverage(this: *mut u8, maps: *mut u8, index: u32, list: *mut *mut u8, effects: *mut u8, school: u32) = "thiscall" 0x006F_D0F0; }
game_fn! { fn read_bool(holder: *mut u8, id: u32, out: *mut u8) = "C" 0x005F_D390; }
game_fn! {
    #[allow(clippy::too_many_arguments)]
    fn occupant_grade(this: *mut u8, occupant: *mut u8, coverage: *mut u8, curve: *mut u8, effect: *mut u8, grids: *mut u8, maximum: f32, school: u32, sum: *mut u8) = "thiscall" 0x006F_D970;
}
game_fn! { fn history(this: *mut u8, series: u32, value: u32) = "thiscall" 0x006F_7D00; }

game_data! { static allocator: *mut u8 = 0x00B2_07E4; }
game_data! { static school_groups: [u32; 14] = 0x00AB_2B60; }
game_data! { static hospital_groups: [u32; 6] = 0x00AB_2B98; }
game_data! { static pacing_flags: u32 = 0x00B4_AFB8; }
game_data! { static low_pacing: [u32; 2] = 0x00B4_AFB0; }
game_data! { static medium_pacing: [u32; 2] = 0x00B4_AFA8; }
game_data! { static high_pacing: [u32; 2] = 0x00B4_AFA0; }
game_data! { static population_pacing: [u32; 2] = 0x00B4_AF98; }
game_data! { static per_mille: f32 = 0x00A8_FAF0; }
game_data! { static rounding_bias: f32 = 0x00AB_2BB0; }

fn count(lists: Lists) -> u32 {
    ((lists.end() as u32).wrapping_sub(lists.begin() as u32) as i32 >> 2) as u32
}

unsafe fn append(node: *mut u8, head: *mut u8, occupant: *mut u8) {
    unsafe {
        if !node.wrapping_add(8).is_null() {
            Node::from_ptr(node).set_occupant(occupant);
        }
        let tail = Node::from_ptr(head).previous();
        let n = Node::from_ptr(node);
        n.set_next(head);
        n.set_previous(tail);
        Node::from_ptr(tail).set_next(node);
        Node::from_ptr(head).set_previous(node);
    }
}

unsafe fn classify(this: *mut u8, head: *mut u8, lists: Lists, groups: *const u32) {
    unsafe {
        let mut node = Node::from_ptr(head).next();
        while node != head {
            let mut matched = false;
            let mut i = 0u32;
            while i < count(lists) {
                if !matched {
                    let occupant = Node::from_ptr(node).occupant();
                    let group = groups.wrapping_add(i as usize).read_unaligned();
                    if vcall!(occupant, 0x58 => fn(u32) -> u32, group) as u8 != 0 {
                        let slot = lists.begin().wrapping_add(i as usize);
                        let destination = slot.read_unaligned();
                        let allocation = allocate_node(slot.cast(), 1, 0);
                        append(allocation, destination, occupant);
                        matched = true;
                    }
                }
                i = i.wrapping_add(1);
            }
            node = Node::from_ptr(node).next();
        }
        clear_list(this);
    }
}

unsafe fn setup_proximity(sim: DailySim, wealth: usize) {
    unsafe {
        if sim.temporary().is_null() {
            let allocation = allocate_grid(0x44);
            let grid = if allocation.is_null() {
                null_mut()
            } else {
                grid_new(allocation, 0x49B9_E600)
            };
            sim.set_temporary(grid);
            vcall!(grid, 0x0C => fn());
            vcall!(sim.temporary(), 4 => fn());
        }
        // Retain the destination table across the source size query, then fetch its resize slot.
        let table = sim.temporary().cast::<*const usize>().read_unaligned();
        let size = vcall!(sim.proximity()[wealth], 0x28 => fn() -> u32);
        let resize = table.add(0x24 / 4).read();
        let resize = core::mem::transmute::<usize, unsafe extern "thiscall" fn(*mut u8, u32)>(resize);
        resize(sim.temporary(), size);
        let lots = sim.lots();
        let key = vcall!(lots, 0xA8 => fn(u32, u32) -> u32, 1, wealth as u32 + 1);
        let source = vcall!(lots, 0xA4 => fn() -> *mut u8);
        if wealth == 2 {
            start_high_proximity(sim.temporary(), source, key);
        } else {
            start_proximity(sim.temporary(), source, key);
        }
        sim.set_state(wealth as u32 + 1);
    }
}

unsafe fn start_budget(storage: *mut u8, milliseconds: u32, state: u32) -> Budget {
    unsafe {
        let (bit, pacing) = match state {
            1 => (1, low_pacing()),
            2 => (2, medium_pacing()),
            3 => (4, high_pacing()),
            _ => (8, population_pacing()),
        };
        if *pacing_flags() & bit == 0 {
            *pacing_flags() |= bit;
            (*pacing)[0] = 1024;
            (*pacing)[1] = 0;
        }
        budget_new(storage, milliseconds, pacing);
        Budget::from_ptr(storage)
    }
}

unsafe fn tick(budget: Budget, counter: &mut u32, step: u32) -> bool {
    unsafe {
        *counter = counter.wrapping_add(step);
        budget.set_counter(*counter);
        if *counter & 1023 == 0 && budget_expired(budget.as_ptr()) as u8 != 0 {
            budget.set_stopped(1);
            return true;
        }
        budget.stopped() != 0
    }
}

unsafe fn run_proximity(sim: DailySim, budget: Budget, state: u32) -> bool {
    unsafe {
        let mut counter = budget.counter();
        let step = budget.step();
        loop {
            let grid = sim.temporary();
            let z = vcall!(grid, 0x34 => fn() -> u32);
            let x = vcall!(grid, 0x30 => fn() -> u32);
            let data = vcall!(grid, 0x60 => fn() -> *mut u8);
            if proximity_step(data, x, z) == 0 {
                copy_proximity(sim.proximity()[state as usize - 1], sim.temporary(), 0x006F_6F80, 0);
                if state == 3 {
                    vcall!(sim.temporary(), 0x10 => fn());
                    vcall!(sim.temporary(), 8 => fn());
                    sim.set_temporary(null_mut());
                    sim.set_state(11);
                } else {
                    sim.set_state(state + 13);
                }
                return true;
            }
            let step = if state == 1 { budget.step() } else { step };
            if tick(budget, &mut counter, step) {
                return false;
            }
        }
    }
}

unsafe fn run_population(sim: DailySim, budget: Budget, milliseconds: u32) -> bool {
    unsafe {
        let this = sim.as_ptr();
        if sim.tuning_read() == 0 {
            read_population_tuning(this, this.add(0x350));
        }
        let mut counter = budget.counter();
        let step = budget.step();
        let mut x = MaybeUninit::<u32>::uninit();
        let mut z = milliseconds;
        loop {
            scan_next(this.add(0x328), x.as_mut_ptr(), &mut z);
            if *this.add(0x328) == 2 {
                break;
            }
            population_change(this, this.add(0x350), x.assume_init(), z);
            if *this.add(0x328) == 2 {
                break;
            }
            if tick(budget, &mut counter, step) {
                if *this.add(0x328) != 2 {
                    return false;
                }
                break;
            }
        }
        sim.set_state(12);
        true
    }
}

unsafe fn coverage(sim: DailySim, school: bool) {
    unsafe {
        let this = sim.as_ptr();
        let (list_offset, maps, effects, next) = if school {
            sim.set_school_population(0);
            (0x1A8, 0x58, 0x198, 6)
        } else {
            sim.set_hospital_population(0);
            (0x1B4, 0x64, 0x194, 8)
        };
        let lists = Lists::from_ptr(this.add(list_offset));
        let mut i = 0u32;
        while i < count(lists) {
            update_coverage(
                this,
                this.add(maps),
                i,
                lists.begin().wrapping_add(i as usize),
                this.add(effects),
                u32::from(school),
            );
            i = i.wrapping_add(1);
        }
        sim.set_state(next);
    }
}

unsafe fn grade(sim: DailySim, school: bool, milliseconds: u32) {
    unsafe {
        let this = sim.as_ptr();
        let lists = Lists::from_ptr(this.add(if school { 0x1A8 } else { 0x1B4 }));
        let effect = if school {
            sim.school_effect()
        } else {
            sim.hospital_effect()
        };
        vcall!(effect, 0x6C => fn(u32), 0);
        let mut scratch = milliseconds;
        let mut i = 0u32;
        while i < count(lists) {
            let head = lists.begin().wrapping_add(i as usize).read_unaligned();
            let mut node = Node::from_ptr(head).next();
            while node != head {
                let auto = if school { sim.school_auto() } else { sim.hospital_auto() };
                let occupant = Node::from_ptr(node).occupant();
                scratch = (scratch & !255) | u32::from(auto);
                if auto == 0 {
                    let holder = vcall!(occupant, 0x18 => fn() -> *mut u8);
                    read_bool(holder, 0x6C05_FC4D, (&mut scratch as *mut u32).cast());
                }
                if scratch as u8 != 0 {
                    let mut staff = 0u32;
                    let mut vehicle = 0u32;
                    vcall!(this, 0x50 => fn(*mut u8, *mut u32, *mut u32, u32), occupant, &mut staff, &mut vehicle, u32::from(school));
                    if vcall!(this, 0xE8 => fn(*mut u8, *mut u32) -> u32, occupant, &mut staff) as u8 != 0 {
                        vcall!(this, 0x4C => fn(*mut u8, *mut u32, *mut u32, u32), occupant, &mut staff, &mut vehicle, u32::from(school));
                    }
                }
                let (maps, curves, effect, grids, sum) = if school {
                    (
                        sim.school_coverage(),
                        sim.school_curves(),
                        sim.school_effect(),
                        0x7C,
                        0x314,
                    )
                } else {
                    (
                        sim.hospital_coverage(),
                        sim.hospital_curves(),
                        sim.hospital_effect(),
                        0x88,
                        0x318,
                    )
                };
                occupant_grade(
                    this,
                    occupant,
                    maps.wrapping_add(i as usize).read_unaligned(),
                    curves.wrapping_add(i as usize).read_unaligned(),
                    effect,
                    this.add(grids),
                    200.0,
                    u32::from(school),
                    this.add(sum),
                );
                node = Node::from_ptr(node).next();
            }
            i = i.wrapping_add(1);
        }
        sim.set_state(if school { 7 } else { 9 });
    }
}

unsafe fn record_history(sim: DailySim) {
    unsafe {
        let this = sim.as_ptr();
        let eq = vcall_st0!(this, 0x80).ftol() as u32;
        history(this, 0xAA1A_2864, eq);
        let hq = vcall_st0!(this, 0x84).ftol() as u32;
        history(this, 0xAA1A_2866, hq);
        history(this, 0xAA1A_2CCA, sim.population() as u32);
        let population = sim.population();
        if population > 0 {
            let denominator = F80::from_i32(population);
            let hospital = (F80::from_i32(sim.hospital_population()) / denominator).to_f32();
            let school = ((F80::from_i32(sim.school_population()) / denominator) * F80::from_f32(*per_mille())
                + F80::from_f32(*rounding_bias()))
            .to_f32();
            history(this, 0x4A8A_3FE5, school.to_bits().wrapping_add(0xB4C0_0000));
            let hospital =
                (F80::from_f32(hospital) * F80::from_f32(*per_mille()) + F80::from_f32(*rounding_bias())).to_f32();
            history(this, 0x4A8A_3FE2, hospital.to_bits().wrapping_add(0xB4C0_0000));
        } else {
            history(this, 0x4A8A_3FE2, 0);
            history(this, 0x4A8A_3FE5, 0);
        }
        sim.set_state(10);
    }
}

port! {
    /// Advance one daily state. Time slices retain the current state until their scan completes.
    /// AL reports completion; EAX's other bytes come from releasing the temporary eight-byte allocation.
    #[port(address = 0x006F_F520, owner = "residential", name = "cSC4ResidentialSimulator::DoDailySimulation")]
    pub unsafe extern "thiscall" fn fc4p_006ff520(this: *mut u8, milliseconds: u32) -> u32 {
        unsafe {
            let storage = allocate(8);
            *storage = 0;
            let sim = DailySim::from_ptr(this);
            let mut done = true;
            match sim.state() {
                0 => {
                    sim.set_school_sum(0);
                    sim.set_hospital_sum(0);
                    strike_check(this, this.add(0x134), this.add(0x1A8), 0x2A97_6C20, 0x2924_53C5, 0x1A03);
                    strike_check(this, this.add(0xD4), this.add(0x1B4), 0xAA97_6BE7, 0x0924_53CB, 0x1A02);
                    clear_effects(this.add(0x198));
                    clear_effects(this.add(0x194));
                    ordinance_effects(this, this.add(0x194), this.add(0x198));
                    classify(this.add(0x19C), sim.new_schools(), Lists::from_ptr(this.add(0x1A8)), school_groups().cast());
                    classify(this.add(0x1A0), sim.new_hospitals(), Lists::from_ptr(this.add(0x1B4)), hospital_groups().cast());
                    let head = sim.new_other();
                    let mut node = Node::from_ptr(head).next();
                    while node != head {
                        let destination = sim.other();
                        if (*allocator()).is_null() {
                            let mut service = null_mut();
                            let framework = framework();
                            if !framework.is_null() {
                                vcall!(framework, 0x14 => fn(u32, u32, *mut *mut u8), 0x3AE4_BEA3, 0x3AE4_BEAB, &mut service);
                            }
                            *allocator() = service;
                            release_reference(&mut service);
                        }
                        let allocation = vcall!(*allocator(), 0x0C => fn(u32) -> *mut u8, 12);
                        // The occupant is read only after the allocator returns.
                        let occupant = if allocation.wrapping_add(8).is_null() { null_mut() } else { Node::from_ptr(node).occupant() };
                        append(allocation, destination, occupant);
                        node = Node::from_ptr(node).next();
                    }
                    clear_list(this.add(0x1A4));
                    sim.set_state(13);
                }
                state @ 1..=4 => {
                    let mut storage = MaybeUninit::<[u32; 6]>::uninit();
                    let budget = start_budget(storage.as_mut_ptr().cast(), milliseconds, state);
                    done = if state == 4 { run_population(sim, budget, milliseconds) } else { run_proximity(sim, budget, state) };
                    budget_end(budget.as_ptr());
                }
                5 => coverage(sim, true),
                6 => grade(sim, true, milliseconds),
                7 => coverage(sim, false),
                8 => grade(sim, false, milliseconds),
                9 => record_history(sim),
                11 => {
                    reset_population(this);
                    sim.set_state(4);
                    sim.set_tuning_read(0);
                }
                12 => {
                    scan_reset(this.add(0x328), 0, 0);
                    post_population(this);
                    sim.set_state(5);
                }
                state @ 13..=15 => setup_proximity(sim, (state - 13) as usize),
                _ => {}
            }
            (free(storage) & !255) | u32::from(done)
        }
    }
}

pub const PORTS: &[Port] = &[fc4p_006ff520::PORT];

#[cfg(test)]
#[path = "daily_tests.rs"]
mod tests;
