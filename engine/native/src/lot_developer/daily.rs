// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! The resumable daily lot walk and its adaptive time budget.

use super::*;

layout! {
    /// Daily work state and the pending and current cycle latches.
    pub struct DailyState {
        0x5D => roads_dirty / set_roads_dirty: u8,
        0x60 => no_zot_until / set_no_zot_until: u32,
        0xAC => state / set_state: u32,
        0xB0 => iterator / set_iterator: *mut u8,
        0xB4 => visited / set_visited: i32,
        0xB8 => roads_changed / set_roads_changed: u8,
        0xB9 => units_changed / set_units_changed: u8,
        0xBA => can_check_commute / set_can_check_commute: u8,
    }
}
layout! {
    /// The lot manager's iterator retains a pointer to the list's sentinel word.
    pub struct LotIterator {
        0x00 => current / set_current: *mut u8,
        0x04 => list / set_list: *mut *mut u8,
    }
}

game_fn! { fn start_timer(this: *mut u32, milliseconds: u32) = "thiscall" 0x0068_BF50; }
game_fn! { fn timer_expired(this: *mut u32) -> u32 = "thiscall" 0x0068_BF80; }
game_fn! { fn finish_timer(this: *mut u32) -> u32 = "thiscall" 0x0068_B960; }
game_fn! { fn update_lot(this: *mut u8, lot: *mut u8) = "thiscall" 0x006C_D3A0; }
game_data! { static date_service: *mut u8 = 0x00B4_3D5C; }
game_data! { static budget_step: u32 = 0x00B4_9B08; }
game_data! { static budget_feedback: u32 = 0x00B4_9B0C; }
game_data! { static budget_initialized: u32 = 0x00B4_9B10; }
game_data! { static rotation_pending: u8 = 0x00B4_9AE9; }

port! {
    /// Starts a cycle when needed, then visits count/28+1 lots or yields at a time check. The iterator is
    /// advanced before each update; work and service pointers are read again after callbacks. Only AL is
    /// replaced in the final timer result. States other than zero, one and three return immediately.
    #[port(address = 0x006C_D770, owner = "lotdeveloper", name = "cSC4LotDeveloper::DoDailySimulation")]
    pub unsafe extern "thiscall" fn fc4p_006cd770(this: *mut u8) -> u32 {
        unsafe {
            let state = DailyState::from_ptr(this);
            let developer = LotDeveloper::from_ptr(this);
            let mode = state.state();
            if mode != 1 {
                if mode != 0 && mode != 3 { return (mode.wrapping_sub(3) & !255) | 1; }
                let iterator = state.iterator();
                if iterator.is_null() {
                    state.set_iterator(vcall!(developer.lot_manager(), 0x3C => fn() -> *mut u8));
                } else {
                    let iterator = LotIterator::from_ptr(iterator);
                    if !iterator.list().is_null() {
                        let head = iterator.list().read_unaligned();
                        iterator.set_current(head.cast::<*mut u8>().read_unaligned());
                    }
                }
                let count = vcall!(developer.lot_manager(), 0x40 => fn() -> i32);
                if count <= 0 { return (count as u32 & !255) | 1; }
                let roads = state.roads_dirty();
                let units = developer.business_units_dirty();
                state.set_visited(0);
                state.set_roads_changed(roads);
                state.set_units_changed(units);
                developer.set_business_units_dirty(0);
                state.set_roads_dirty(0);
                let day = vcall!(date_service().read(), 0x20 => fn() -> u32);
                state.set_can_check_commute(u8::from(day > state.no_zot_until()));
                state.set_state(1);
            }
            let flags = budget_initialized().read();
            if flags & 1 == 0 {
                budget_initialized().write(flags | 1);
                budget_step().write(1024);
                budget_feedback().write(0);
            }
            let mut timer = [0u32; 6];
            start_timer(timer.as_mut_ptr(), 5);
            let step = budget_step().read();
            timer[2] = 0;
            timer[3] = step;
            timer[4] = 0;
            timer[5] = budget_step() as u32;
            let count = vcall!(developer.lot_manager(), 0x40 => fn() -> i32);
            if count <= 0 {
                state.set_state(3);
                return (finish_timer(timer.as_mut_ptr()) & !255) | 1;
            }
            let quota = count / 28 + 1;
            let mut accumulated = 0u32;
            if state.visited() < quota {
                loop {
                    accumulated = accumulated.wrapping_add(step);
                    timer[2] = accumulated;
                    if accumulated & 1023 == 0 && timer_expired(timer.as_mut_ptr()) as u8 != 0 {
                        // Only the low byte is written; the upper bytes belong to the timer's local word.
                        timer[4] = (timer[4] & !255) | 1;
                        break;
                    }
                    let iterator = LotIterator::from_ptr(state.iterator());
                    if iterator.list().is_null() || iterator.current() == iterator.list().read_unaligned() {
                        state.set_state(3);
                        rotation_pending().write(0);
                        break;
                    }
                    let node = iterator.current();
                    iterator.set_current(node.cast::<*mut u8>().read_unaligned());
                    let lot = node.add(8).cast::<*mut u8>().read_unaligned();
                    update_lot(this, lot);
                    let visited = state.visited().wrapping_add(1);
                    state.set_visited(visited);
                    if visited >= quota { break; }
                }
            }
            let complete = state.visited() >= quota;
            if complete { state.set_visited(0); }
            (finish_timer(timer.as_mut_ptr()) & !255) | u32::from(complete)
        }
    }
}

pub const PORTS: &[Port] = &[fc4p_006cd770::PORT];
