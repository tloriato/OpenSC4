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

//! `cSC4LandValueSimulator`: setup, tuning, shutdown and messages (006be3b0 Init, 006be110 ReadSimulatorTunableValues, 006bdf70 Shutdown, 006bf0d0 DoMessage).

use super::*;
use core::ptr::{null_mut, read_unaligned, write_unaligned};

game_fn! {
    fn histogram_resize(this: *mut u8, count: u32, value: *const u32) -> u32 = "thiscall" 0x006B_DF10;
}

const CHANGES: [u32; 5] = [0x99EF_1142, 0x99EF_1143, 0x678E_6E3A, 0x278E_6003, 0x0769_9455];

// All accesses below are to the live game objects supplied to the enclosing port.
unsafe fn pointer(this: *mut u8, offset: usize) -> *mut u8 {
    unsafe { read_unaligned(this.add(offset).cast()) }
}

unsafe fn store_pointer(this: *mut u8, offset: usize, value: *mut u8) {
    unsafe { write_unaligned(this.add(offset).cast(), value) }
}

unsafe fn delete_curve(curve: *mut u8) -> u32 {
    unsafe {
        let data = pointer(curve, 0);
        if !data.is_null() {
            game::free_array(data);
        }
        game::free(curve)
    }
}

port! {
    /// Unregister the agent and notifications, release services and grids, and delete iterators and curves.
    /// Retain the developer map, dirty byte, pipeline state, histogram and statistics.
    /// AL is one; the other EAX bytes retain the last pointer or call result.
    #[port(address = 0x006B_DF70, owner = "landvalue", name = "cSC4LandValueSimulator::Shutdown", entry_eax)]
    pub unsafe extern "thiscall" fn fc4p_006bdf70(this: *mut u8, [entry_eax: u32]) -> u32 {
        unsafe {
            let lv = LandValueSim::from_ptr(this);
            if lv.initialized() == 0 {
                return (entry_eax & !0xFF) | 1;
            }
            lv.set_initialized(0);
            let sim = vcall!(*game::city(), 0xC8 => fn() -> *mut u8);
            if !sim.is_null() {
                vcall!(sim, 0x50 => fn(*mut u8) -> u32, this.add(4));
            }
            if !lv.message_server().is_null() {
                for id in CHANGES {
                    vcall!(lv.message_server(), 0x18 => fn(*mut u8, u32) -> u32, this.add(4), id);
                }
                vcall!(lv.message_server(), 8 => fn() -> u32);
                lv.set_message_server(null_mut());
            }
            for offset in [0x1C, 0x20, 0x18] {
                let p = pointer(this, offset);
                if !p.is_null() {
                    vcall!(p, 8 => fn() -> u32);
                    store_pointer(this, offset, null_mut());
                }
            }
            for offset in [0x24, 0x28, 0x2C, 0x30] {
                let p = pointer(this, offset);
                if !p.is_null() {
                    vcall!(p, 0x10 => fn() -> u32);
                    vcall!(pointer(this, offset), 8 => fn() -> u32);
                    store_pointer(this, offset, null_mut());
                }
            }
            let mut eax = 0;
            for offset in [0x3C, 0x40] {
                let p = pointer(this, offset);
                eax = p as u32;
                if !p.is_null() {
                    eax = game::free(p);
                    store_pointer(this, offset, null_mut());
                }
            }
            let curve = lv.altitude_curve();
            if !curve.is_null() {
                eax = delete_curve(curve);
                lv.set_altitude_curve(null_mut());
            }
            let curve = lv.desirability_curve();
            if !curve.is_null() {
                eax = delete_curve(curve);
                lv.set_desirability_curve(null_mut());
            }
            (eax & !0xFF) | 1
        }
    }
}

unsafe fn property_variant(holder: *mut u8, id: u32) -> *mut u8 {
    unsafe {
        let property = vcall!(holder, 0x24 => fn(u32) -> *mut u8, id);
        vcall!(property, 0x14 => fn() -> *mut u8)
    }
}

unsafe fn tune_curve(this: *mut u8, offset: usize, property: *mut u8) {
    unsafe {
        let curve = pointer(this, offset);
        if property.is_null() {
            if !curve.is_null() {
                delete_curve(curve);
                store_pointer(this, offset, null_mut());
            }
        } else {
            if curve.is_null() {
                let allocation = game::allocate(0x10);
                let curve = if allocation.is_null() {
                    null_mut()
                } else {
                    game::response_curve_new(allocation)
                };
                store_pointer(this, offset, curve);
            }
            game::response_curve_set(pointer(this, offset), property);
        }
    }
}

port! {
    /// Load byte tuning, replace or delete response curves, and update developer weights without clearing the map.
    /// Missing exemplars leave the simulator untouched. Required array properties are read without checks.
    #[port(address = 0x006B_E110, owner = "landvalue", name = "cSC4LandValueSimulator::ReadSimulatorTunableValues")]
    pub unsafe extern "thiscall" fn fc4p_006be110(this: *mut u8) -> u32 {
        unsafe {
            let key = [0x6534_284Au32, 0xE7E2_C2DB, 0xE7E2_C8D8];
            let mut resource = null_mut();
            let result = vcall!(*game::resource_manager(), 0x0C => fn(*const u32, u32, *mut *mut u8, u32, u32) -> u32,
                key.as_ptr(), 0xA521_60F5, &mut resource, 0, 0);
            if result as u8 == 0 { return result; }
            let holder = vcall!(resource, 0x1C => fn() -> *mut u8);
            game::property_u8(holder, 0x47E2_C200, this.add(0x7C));
            for (id, offset) in [(0x47E2_C300, 0x7D), (0x47E2_C301, 0x7F), (0x47E2_C310, 0x81)] {
                let variant = property_variant(holder, id);
                let bytes = vcall!(variant, 0x17C => fn() -> *const u8);
                *this.add(offset) = *bytes;
                *this.add(offset + 1) = *bytes.add(1);
            }
            let altitude = vcall!(holder, 0x24 => fn(u32) -> *mut u8, 0x47E2_C320);
            tune_curve(this, 0x84, altitude);
            let ids_variant = property_variant(holder, 0x47E2_C330);
            let ids = vcall!(ids_variant, 0x19C => fn() -> *const u32);
            let weights_variant = property_variant(holder, 0x47E2_C331);
            let weights = vcall!(weights_variant, 0x1BC => fn() -> *const u32);
            let mut i = 0u32;
            let mut count = vcall!(ids_variant, 0x10 => fn() -> u32);
            while i < count {
                let developer = vcall!(*game::growth_developer(), 0x80 => fn(u32) -> *mut u8,
                    read_unaligned(ids.wrapping_add(i as usize)));
                let output = game::developer_weight(this.add(0x8C), &developer);
                write_unaligned(output, read_unaligned(weights.wrapping_add(i as usize)));
                i = i.wrapping_add(1);
                count = vcall!(ids_variant, 0x10 => fn() -> u32);
            }
            let desirability = vcall!(holder, 0x24 => fn(u32) -> *mut u8, 0x47E2_C332);
            tune_curve(this, 0x88, desirability);
            vcall!(resource, 8 => fn() -> u32)
        }
    }
}

/// Initialize just the fields written by the grid's construction in this function. Padding stays untouched.
unsafe fn construct_grid(p: *mut u8, slope: bool) {
    unsafe {
        core::ptr::write_volatile(p.add(4).cast::<u32>(), 0x00A8_0784);
        store_pointer(
            p,
            0,
            if slope {
                game::grid_f32_vtable()
            } else {
                game::grid_u8_vtable()
            },
        );
        store_pointer(
            p,
            4,
            if slope {
                game::grid_f32_serial_vtable()
            } else {
                game::grid_u8_serial_vtable()
            },
        );
        *p.add(8) = 0;
        for offset in [0x0C, 0x18, 0x38, 0x3C, 0x40] {
            write_unaligned(p.add(offset).cast::<u32>(), 0);
        }
        write_unaligned(p.add(0x10).cast::<u32>(), if slope { 0x49B9_E60A } else { 0x49B9_E602 });
    }
}

unsafe fn iterator_bounds(this: *mut u8, grid_offset: usize, iterator_offset: usize) {
    unsafe {
        if pointer(this, iterator_offset).is_null() {
            let allocation = game::allocate(0x20);
            let iterator = if allocation.is_null() {
                null_mut()
            } else {
                game::scanning_iterator_new(allocation)
            };
            store_pointer(this, iterator_offset, iterator);
        }
        let z = vcall!(pointer(this, grid_offset), 0x34 => fn() -> i32).wrapping_sub(1);
        let x = vcall!(pointer(this, grid_offset), 0x30 => fn() -> i32).wrapping_sub(1);
        let iterator = pointer(this, iterator_offset);
        write_unaligned(iterator.add(0x10).cast::<i32>(), 0);
        write_unaligned(iterator.add(0x14).cast::<i32>(), 0);
        write_unaligned(iterator.add(0x18).cast::<i32>(), x);
        write_unaligned(iterator.add(0x1C).cast::<i32>(), z);
        if x < 0 {
            write_unaligned(iterator.add(0x18).cast::<i32>(), 0);
        }
        if z < 0 {
            write_unaligned(iterator.add(0x1C).cast::<i32>(), 0);
        }
    }
}

port! {
    /// Retain services, read tuning, create missing grids and iterators, size the histogram, and register messages.
    /// Restored grids skip construction and setup. AL is one; EAX's upper bytes retain the last call result.
    #[port(address = 0x006B_E3B0, owner = "landvalue", name = "cSC4LandValueSimulator::Init", naked)]
    pub unsafe extern "C" fn fc4p_006be3b0() {
        core::arch::naked_asm!(
            "mov edx, eax", "mov eax, esp",
            "push ebx", "push ebp", "push esi", "push edi", "sub esp, 0x18",
            "mov esi, ecx", "push eax", "call {body}", "jmp edx", body = sym initialize,
        )
    }
}

// Return the value in EAX and the next frame-marshaling continuation in EDX.
unsafe extern "fastcall" fn initialize(this: *mut u8, entry_eax: u32, stack: *mut u8) -> u64 {
    unsafe {
        let lv = LandValueSim::from_ptr(this);
        if lv.initialized() != 0 {
            return (u64::from(initialize_return as *const () as u32) << 32) | u64::from((entry_eax & !0xFF) | 1);
        }
        if *game::land_value_started() == 0 {
            *game::land_value_started() = 1;
        }
        if lv.city().is_null() {
            lv.set_city(*game::city());
            vcall!(lv.city(), 4 => fn() -> u32);
        }
        if lv.zone_manager().is_null() {
            lv.set_zone_manager(*game::zone_manager());
            vcall!(lv.zone_manager(), 4 => fn() -> u32);
        }
        if lv.terrain().is_null() {
            lv.set_terrain(*game::terrain());
            vcall!(lv.terrain(), 4 => fn() -> u32);
        }
        game::land_value_tuning(this);
        for (offset, id) in [
            (0x24, 0x89D5_BB39),
            (0x28, 0x89D5_BB51),
            (0x2C, 0x89D5_BB60),
            (0x30, 0x49D5_BB6F),
        ] {
            if pointer(this, offset).is_null() {
                let grid = game::allocate(0x44);
                if !grid.is_null() {
                    construct_grid(grid, offset == 0x30);
                }
                store_pointer(this, offset, grid);
                vcall!(grid, 4 => fn() -> u32);
                vcall!(pointer(this, offset), 0x0C => fn() -> u32);
                let size = if offset == 0x30 { 8 } else { u32::from(lv.tract_size()) };
                vcall!(pointer(this, offset), 0x24 => fn(u32) -> u32, size);
                vcall!(pointer(this, offset), 0x18 => fn(u32) -> u32, id);
            }
        }
        lv.set_state(0);
        iterator_bounds(this, 0x24, 0x3C);
        iterator_bounds(this, 0x30, 0x40);
        lv.set_histogram_shift(5);
        let height = vcall_st0!(lv.terrain(), 0x60);
        let bins = (height.ftol() as i32).wrapping_shr(lv.histogram_shift());
        // The histogram fill word belongs at entry ESP - 0x18, within the reserved frame.
        let zero = stack.sub(0x18).cast::<u32>();
        zero.write(0);
        histogram_resize(this.add(0x50), bins as u32, zero);
        let mut eax = vcall!(*game::history(), 0x34 => fn(u32, u32) -> u32, 0x6A66_3AC8, 1);
        let sim = *game::simulator();
        if !sim.is_null() {
            let mut name = [0u32; 6];
            game::string_from_cstr(name.as_mut_ptr().cast(), game::land_value_agent_name());
            for level in [0, 10, 4, 6] {
                vcall!(sim, 0x4C => fn(*mut u8, u32, *mut u8, u32) -> u32,
                        this.add(4), level, name.as_mut_ptr().cast(), 2);
            }
            eax = name[1];
            core::ptr::write_volatile(name.as_mut_ptr(), 0x00A8_0810);
            if eax != 0 {
                eax = game::free_array(eax as *mut u8);
            }
        }
        let continuation = if lv.message_server().is_null() {
            lv.set_message_server(*game::message_server());
            initialize_subscribe as *const () as u32
        } else {
            initialize_finish as *const () as u32
        };
        (u64::from(continuation) << 32) | u64::from(eax)
    }
}

// Subscription may allocate. Keep each call at entry ESP - 0x28 so retained frame
// addresses agree with the other callers of the message server.
#[unsafe(naked)]
unsafe extern "C" fn initialize_subscribe() {
    core::arch::naked_asm!(
        "mov ecx, [esi + 0x14]", "mov eax, [ecx]", "call dword ptr [eax + 4]",
        "lea edi, [esi + 4]",
        "push {change0}", "push edi", "mov ecx, [esi + 0x14]", "mov eax, [ecx]", "call dword ptr [eax + 0x14]",
        "push {change1}", "push edi", "mov ecx, [esi + 0x14]", "mov eax, [ecx]", "call dword ptr [eax + 0x14]",
        "push {change2}", "push edi", "mov ecx, [esi + 0x14]", "mov eax, [ecx]", "call dword ptr [eax + 0x14]",
        "push {change3}", "push edi", "mov ecx, [esi + 0x14]", "mov eax, [ecx]", "call dword ptr [eax + 0x14]",
        "push {change4}", "push edi", "mov ecx, [esi + 0x14]", "mov eax, [ecx]", "call dword ptr [eax + 0x14]",
        "jmp {finish}", finish = sym initialize_finish,
        change0 = const CHANGES[0], change1 = const CHANGES[1], change2 = const CHANGES[2],
        change3 = const CHANGES[3], change4 = const CHANGES[4],
    )
}

#[unsafe(naked)]
unsafe extern "C" fn initialize_finish() {
    core::arch::naked_asm!(
        "mov ecx, esi", "mov edx, eax", "call {mark}", "jmp {done}",
        mark = sym mark_initialized, done = sym initialize_return,
    )
}

unsafe extern "fastcall" fn mark_initialized(this: *mut u8, eax: u32) -> u32 {
    // SAFETY: the entry keeps the simulator live until every subscription has completed.
    unsafe { LandValueSim::from_ptr(this) }.set_initialized(1);
    (eax & !0xFF) | 1
}

#[unsafe(naked)]
unsafe extern "C" fn initialize_return() {
    core::arch::naked_asm!("add esp, 0x18", "pop edi", "pop esi", "pop ebp", "pop ebx", "ret",)
}

port! {
    /// Dispatch a message using the message-target interface, four bytes after the simulator base.
    /// Days use budget five; idle forwards data3 verbatim. A dirty finished pass restarts on the next month.
    #[port(address = 0x006B_F0D0, owner = "landvalue", name = "cSC4LandValueSimulator::DoMessage")]
    pub unsafe extern "thiscall" fn fc4p_006bf0d0(this: *mut u8, message: *mut u8) -> u32 {
        unsafe {
            let lv = LandValueSim::from_ptr(this.sub(4));
            let kind = vcall!(message, 0x10 => fn() -> u32);
            let eax = match kind {
                0x6695_6814 => game::land_value_idle(lv.as_ptr(), 5),
                0x6695_681A => {
                    let budget = vcall!(message, 0x4C => fn() -> u32);
                    game::land_value_idle(lv.as_ptr(), budget)
                }
                0x6695_6816 => {
                    if lv.dirty() != 0 && lv.state() == 9 {
                        lv.set_dirty(0);
                        lv.set_state(0);
                    }
                    0
                }
                id if CHANGES.contains(&id) => { lv.set_dirty(1); kind }
                _ => kind,
            };
            (eax & !0xFF) | 1
        }
    }
}

/// This file's replacements.
pub const PORTS: &[Port] = &[
    fc4p_006bdf70::PORT,
    fc4p_006be110::PORT,
    fc4p_006be3b0::PORT,
    fc4p_006bf0d0::PORT,
];

#[cfg(test)]
mod tests {
    // Lifecycle, tuning, and message expectations carried from the core land value simulator and its
    // pass/save/load coverage, exercised here with the game's object layout and individual callees.
    use super::*;
    use crate::testing::{call_thiscall_with_eax, fake, fake_data, Fake, GameFpu, Object};
    use std::cell::RefCell;
    use std::collections::BTreeMap;

    #[derive(Debug, PartialEq)]
    enum Call {
        Virtual(usize, usize, Vec<u32>),
        Allocate(usize, u32),
        Free(usize),
        ArrayFree(usize),
        CurveNew(usize),
        CurveSet(usize, usize),
        ByteProperty(usize, u32, usize),
        IteratorNew(usize),
        Histogram(usize, u32, u32),
        String(usize),
        Map(usize, usize),
        Idle(usize, u32),
    }

    struct State {
        calls: Vec<Call>,
        allocations: Vec<Object>,
        resource: usize,
        holder: usize,
        property: usize,
        variant: usize,
        sim: usize,
        resource_result: u32,
        current_property: u32,
        bytes: [[u8; 2]; 3],
        tract_size: Option<u8>,
        ids: Vec<u32>,
        weights: Vec<u32>,
        counts: Vec<u32>,
        map: BTreeMap<u32, u32>,
        curves_present: [bool; 2],
        null_curve_allocation: bool,
        max_height: f32,
        histogram_fill_address: usize,
        subscription_frames: Vec<usize>,
        message_kind: u32,
        message_budget: u32,
    }

    impl Default for State {
        fn default() -> Self {
            Self {
                calls: Vec::new(),
                allocations: Vec::new(),
                resource: 0,
                holder: 0,
                property: 0,
                variant: 0,
                sim: 0,
                resource_result: 0x1234_5600,
                current_property: 0,
                bytes: [[1, 255], [70, 120], [50, 5]],
                tract_size: None,
                ids: Vec::new(),
                weights: Vec::new(),
                counts: Vec::new(),
                map: BTreeMap::new(),
                curves_present: [false; 2],
                null_curve_allocation: false,
                histogram_fill_address: 0,
                subscription_frames: Vec::new(),
                max_height: 2500.0,
                message_kind: 0,
                message_budget: 0,
            }
        }
    }

    thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }

    fn record(call: Call) {
        STATE.with(|s| s.borrow_mut().calls.push(call));
    }
    fn calls() -> Vec<Call> {
        STATE.with(|s| std::mem::take(&mut s.borrow_mut().calls))
    }
    fn vc(this: *mut u8, slot: usize, args: &[u32]) {
        record(Call::Virtual(this as usize, slot, args.to_vec()));
    }

    extern "thiscall" fn add_ref(this: *mut u8) -> u32 {
        vc(this, 4, &[]);
        0xAABB_CC02
    }
    extern "thiscall" fn release(this: *mut u8) -> u32 {
        vc(this, 8, &[]);
        0xAABB_CC00
    }
    extern "thiscall" fn grid_init(this: *mut u8) -> u32 {
        vc(this, 0x0C, &[]);
        1
    }
    extern "thiscall" fn grid_shutdown(this: *mut u8) -> u32 {
        vc(this, 0x10, &[]);
        1
    }
    extern "thiscall" fn grid_size(this: *mut u8, size: u32) -> u32 {
        vc(this, 0x24, &[size]);
        1
    }
    extern "thiscall" fn grid_id(this: *mut u8, id: u32) -> u32 {
        vc(this, 0x18, &[id]);
        1
    }
    extern "thiscall" fn grid_x(this: *mut u8) -> i32 {
        vc(this, 0x30, &[]);
        unsafe { read_unaligned(this.add(0x1C).cast()) }
    }
    extern "thiscall" fn grid_z(this: *mut u8) -> i32 {
        vc(this, 0x34, &[]);
        unsafe { read_unaligned(this.add(0x20).cast()) }
    }
    extern "thiscall" fn city_sim(this: *mut u8) -> *mut u8 {
        vc(this, 0xC8, &[]);
        STATE.with(|s| s.borrow().sim as *mut u8)
    }
    extern "thiscall" fn remove_agent(this: *mut u8, target: *mut u8) -> u32 {
        vc(this, 0x50, &[target as u32]);
        1
    }
    extern "thiscall" fn add_agent(this: *mut u8, target: *mut u8, level: u32, name: *mut u8, flags: u32) -> u32 {
        assert_eq!(unsafe { read_unaligned(name.add(4).cast::<u32>()) }, 0x1122_3344);
        vc(this, 0x4C, &[target as u32, level, flags]);
        0x8877_6601
    }
    #[unsafe(naked)]
    unsafe extern "thiscall" fn subscribe(_this: *mut u8, _target: *mut u8, _id: u32) -> u32 {
        core::arch::naked_asm!("mov edx, esp", "jmp {body}", body = sym subscribe_at)
    }

    extern "fastcall" fn subscribe_at(this: *mut u8, frame: usize, target: *mut u8, id: u32) -> u32 {
        STATE.with(|s| s.borrow_mut().subscription_frames.push(frame));
        vc(this, 0x14, &[target as u32, id]);
        0x9988_7701
    }
    extern "thiscall" fn unsubscribe(this: *mut u8, target: *mut u8, id: u32) -> u32 {
        vc(this, 0x18, &[target as u32, id]);
        1
    }
    extern "thiscall" fn history_length(this: *mut u8, id: u32, length: u32) -> u32 {
        vc(this, 0x34, &[id, length]);
        0x4433_2201
    }
    extern "thiscall" fn max_height(this: *mut u8) -> f32 {
        vc(this, 0x60, &[]);
        STATE.with(|s| s.borrow().max_height)
    }
    extern "thiscall" fn message_type(this: *mut u8) -> u32 {
        vc(this, 0x10, &[]);
        STATE.with(|s| s.borrow().message_kind)
    }
    extern "thiscall" fn message_data3(this: *mut u8) -> u32 {
        vc(this, 0x4C, &[]);
        STATE.with(|s| s.borrow().message_budget)
    }
    extern "thiscall" fn idle(this: *mut u8, budget: u32) -> u32 {
        record(Call::Idle(this as usize, budget));
        0x1357_9B00
    }
    extern "C" fn allocate(size: u32) -> *mut u8 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            if size == 0x10 && s.null_curve_allocation {
                s.calls.push(Call::Allocate(0, size));
                return null_mut();
            }
            let mut obj = Object::new(size as usize);
            let p = obj.ptr();
            unsafe {
                core::ptr::write_bytes(p, 0xA5, size as usize);
            }
            s.allocations.push(obj);
            s.calls.push(Call::Allocate(p as usize, size));
            p
        })
    }
    extern "C" fn free(p: *mut u8) -> u32 {
        record(Call::Free(p as usize));
        0xDEAD_BE00
    }
    extern "C" fn free_array(p: *mut u8) -> u32 {
        record(Call::ArrayFree(p as usize));
        0xCAFE_BA00
    }
    extern "thiscall" fn curve_new(this: *mut u8) -> *mut u8 {
        record(Call::CurveNew(this as usize));
        unsafe {
            store_pointer(this, 0, null_mut());
        }
        this
    }
    extern "thiscall" fn curve_set(this: *mut u8, property: *mut u8) -> u32 {
        record(Call::CurveSet(this as usize, property as usize));
        1
    }
    extern "C" fn byte_property(holder: *mut u8, id: u32, output: *mut u8) -> u32 {
        record(Call::ByteProperty(holder as usize, id, output as usize));
        STATE.with(|s| {
            if let Some(value) = s.borrow().tract_size {
                unsafe {
                    *output = value;
                }
            }
        });
        1
    }
    extern "thiscall" fn iterator_new(this: *mut u8) -> *mut u8 {
        record(Call::IteratorNew(this as usize));
        unsafe {
            *this = 0;
            for offset in [4, 8, 0x10, 0x14, 0x18, 0x1C] {
                write_unaligned(this.add(offset).cast::<u32>(), 0);
            }
            write_unaligned(this.add(0x0C).cast::<u32>(), 1);
        }
        this
    }
    extern "thiscall" fn histogram(this: *mut u8, count: u32, value: *const u32) -> u32 {
        STATE.with(|s| s.borrow_mut().histogram_fill_address = value as usize);
        record(Call::Histogram(this as usize, count, unsafe { *value }));
        1
    }
    extern "thiscall" fn string_new(this: *mut u8, name: *const u8) -> u32 {
        record(Call::String(name as usize));
        unsafe {
            write_unaligned(this.add(4).cast::<u32>(), 0x1122_3344);
        }
        this as u32
    }
    extern "thiscall" fn resource_lookup(
        this: *mut u8,
        key: *const u32,
        iid: u32,
        output: *mut *mut u8,
        a: u32,
        b: u32,
    ) -> u32 {
        assert_eq!(
            unsafe { core::slice::from_raw_parts(key, 3) },
            &[0x6534_284A, 0xE7E2_C2DB, 0xE7E2_C8D8]
        );
        vc(this, 0x0C, &[iid, a, b]);
        STATE.with(|s| {
            let s = s.borrow();
            unsafe {
                *output = s.resource as *mut u8;
            }
            s.resource_result
        })
    }
    extern "thiscall" fn resource_holder(this: *mut u8) -> *mut u8 {
        vc(this, 0x1C, &[]);
        STATE.with(|s| s.borrow().holder as *mut u8)
    }
    extern "thiscall" fn property_get(this: *mut u8, id: u32) -> *mut u8 {
        vc(this, 0x24, &[id]);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.current_property = id;
            if (id == 0x47E2_C320 && !s.curves_present[0]) || (id == 0x47E2_C332 && !s.curves_present[1]) {
                null_mut()
            } else {
                s.property as *mut u8
            }
        })
    }
    extern "thiscall" fn property_value(this: *mut u8) -> *mut u8 {
        vc(this, 0x14, &[]);
        STATE.with(|s| s.borrow().variant as *mut u8)
    }
    extern "thiscall" fn bytes(this: *mut u8) -> *const u8 {
        vc(this, 0x17C, &[]);
        STATE.with(|s| {
            let s = s.borrow();
            s.bytes[match s.current_property {
                0x47E2_C300 => 0,
                0x47E2_C301 => 1,
                0x47E2_C310 => 2,
                _ => unreachable!(),
            }]
            .as_ptr()
        })
    }
    extern "thiscall" fn ids(this: *mut u8) -> *const u32 {
        vc(this, 0x19C, &[]);
        STATE.with(|s| s.borrow().ids.as_ptr())
    }
    extern "thiscall" fn weights(this: *mut u8) -> *const u32 {
        vc(this, 0x1BC, &[]);
        STATE.with(|s| s.borrow().weights.as_ptr())
    }
    extern "thiscall" fn count(this: *mut u8) -> u32 {
        vc(this, 0x10, &[]);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            if s.counts.is_empty() {
                s.ids.len() as u32
            } else {
                s.counts.remove(0)
            }
        })
    }
    extern "thiscall" fn developer(this: *mut u8, id: u32) -> *mut u8 {
        vc(this, 0x80, &[id]);
        id as *mut u8
    }
    extern "thiscall" fn map(this: *mut u8, key: *const *mut u8) -> *mut u32 {
        let key = unsafe { *key } as u32;
        record(Call::Map(this as usize, key as usize));
        STATE.with(|s| s.borrow_mut().map.entry(key).or_insert(0) as *mut u32)
    }

    struct Virtual {
        object: Object,
        table: Box<[usize; 128]>,
    }
    impl Virtual {
        fn new(entries: &[(usize, usize)]) -> Self {
            let mut result = Self {
                object: Object::new(0xA4),
                table: Box::new([0; 128]),
            };
            result.slot(4, add_ref as *const () as usize);
            result.slot(8, release as *const () as usize);
            for &(offset, target) in entries {
                result.slot(offset, target);
            }
            unsafe {
                store_pointer(result.object.ptr(), 0, result.table.as_mut_ptr().cast());
            }
            result
        }
        fn slot(&mut self, offset: usize, target: usize) {
            self.table[offset / 4] = target;
        }
        fn ptr(&mut self) -> *mut u8 {
            self.object.ptr()
        }
    }

    struct Fixture {
        lv: Object,
        objects: Vec<Virtual>,
        // Global values must remain at stable addresses for the lifetime of fake_data guards.
        #[allow(clippy::vec_box)]
        globals: Vec<Box<usize>>,
        guards: Vec<Fake>,
    }
    impl Fixture {
        fn new() -> Self {
            STATE.with(|s| *s.borrow_mut() = State::default());
            let mut f = Self {
                lv: Object::new(0xA4),
                objects: Vec::new(),
                globals: Vec::new(),
                guards: Vec::new(),
            };
            for (address, target) in [
                (0x005E_55E0, allocate as *const () as usize),
                (0x005E_5620, free as *const () as usize),
                (0x0090_CF63, free_array as *const () as usize),
                (0x007D_0410, curve_new as *const () as usize),
                (0x005F_4620, curve_set as *const () as usize),
                (0x005F_D3C0, byte_property as *const () as usize),
                (0x0060_CE90, iterator_new as *const () as usize),
                (0x006A_3170, map as *const () as usize),
                (0x006B_DF10, histogram as *const () as usize),
                (0x0040_8480, string_new as *const () as usize),
                (0x006B_E7B0, idle as *const () as usize),
            ] {
                f.guards.push(fake(address, target));
            }
            f
        }
        fn view(&mut self) -> LandValueSim {
            unsafe { LandValueSim::from_ptr(self.lv.ptr()) }
        }
        fn object(&mut self, entries: &[(usize, usize)]) -> *mut u8 {
            let mut obj = Virtual::new(entries);
            let p = obj.ptr();
            self.objects.push(obj);
            p
        }
        fn global(&mut self, address: u32, value: usize) {
            let mut value = Box::new(value);
            self.guards.push(fake_data(address, (&mut *value as *mut usize).cast()));
            self.globals.push(value);
        }
        fn tuning(&mut self, found: bool) -> *mut u8 {
            let manager = self.object(&[(0x0C, resource_lookup as *const () as usize)]);
            let resource = self.object(&[(0x1C, resource_holder as *const () as usize)]);
            let holder = self.object(&[(0x24, property_get as *const () as usize)]);
            let property = self.object(&[(0x14, property_value as *const () as usize)]);
            let variant = self.object(&[
                (0x17C, bytes as *const () as usize),
                (0x19C, ids as *const () as usize),
                (0x1BC, weights as *const () as usize),
                (0x10, count as *const () as usize),
            ]);
            self.global(0x00B4_3CA8, manager as usize);
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                s.resource = resource as usize;
                s.holder = holder as usize;
                s.property = property as usize;
                s.variant = variant as usize;
                s.resource_result = if found { 0x1234_5601 } else { 0x1234_5600 };
            });
            manager
        }
        fn grid(&mut self, x: i32, z: i32) -> *mut u8 {
            let grid = self.object(&[
                (0x0C, grid_init as *const () as usize),
                (0x10, grid_shutdown as *const () as usize),
                (0x18, grid_id as *const () as usize),
                (0x24, grid_size as *const () as usize),
                (0x30, grid_x as *const () as usize),
                (0x34, grid_z as *const () as usize),
            ]);
            unsafe {
                write_unaligned(grid.add(0x1C).cast::<i32>(), x);
                write_unaligned(grid.add(0x20).cast::<i32>(), z);
            }
            grid
        }
        fn init_services(&mut self, sim_present: bool) -> (*mut u8, *mut u8, *mut u8, *mut u8, *mut u8) {
            let city = self.object(&[(0xC8, city_sim as *const () as usize)]);
            let zone = self.object(&[]);
            let terrain = self.object(&[(0x60, max_height as *const () as usize)]);
            let history = self.object(&[(0x34, history_length as *const () as usize)]);
            let server = self.object(&[
                (0x14, subscribe as *const () as usize),
                (0x18, unsubscribe as *const () as usize),
            ]);
            let sim = if sim_present {
                self.object(&[
                    (0x4C, add_agent as *const () as usize),
                    (0x50, remove_agent as *const () as usize),
                ])
            } else {
                null_mut()
            };
            for (a, p) in [
                (0x00B4_3CEC, city),
                (0x00B4_3D14, zone),
                (0x00B4_3CF4, terrain),
                (0x00B4_3DC0, history),
                (0x00B4_3CCC, server),
                (0x00B4_3D5C, sim),
            ] {
                self.global(a, p as usize);
            }
            self.global(0x00B4_90E0, 0);
            self.global(0x00A8_9864, 0);
            STATE.with(|s| s.borrow_mut().sim = sim as usize);
            (city, zone, terrain, history, server)
        }
    }

    #[test]
    fn inactive_shutdown_and_repeated_init_only_replace_al() {
        let mut f = Fixture::new();
        for eax in [0, 0xFFFF_FFFF, 0x1234_5678] {
            unsafe {
                assert_eq!(
                    call_thiscall_with_eax(fc4p_006bdf70 as *const () as usize, f.lv.ptr(), &[], eax).0,
                    (eax & !255) | 1
                );
                f.view().set_initialized(0xFF);
                assert_eq!(
                    call_thiscall_with_eax(fc4p_006be3b0 as *const () as usize, f.lv.ptr(), &[], eax).0,
                    (eax & !255) | 1
                );
                f.view().set_initialized(0);
            }
        }
        assert!(calls().is_empty());
    }

    #[test]
    fn missing_exemplar_keeps_all_simulator_bytes() {
        let mut f = Fixture::new();
        let manager = f.tuning(false);
        unsafe {
            core::ptr::write_bytes(f.lv.ptr(), 0xA5, 0xA4);
            assert_eq!(fc4p_006be110(f.lv.ptr()), 0x1234_5600);
            assert!(core::slice::from_raw_parts(f.lv.ptr(), 0xA4).iter().all(|&x| x == 0xA5));
        }
        assert_eq!(
            calls(),
            [Call::Virtual(manager as usize, 0x0C, vec![0xA521_60F5, 0, 0])]
        );
    }

    #[test]
    fn tuning_reads_shipped_bytes_updates_duplicates_and_retains_old_developers() {
        let mut f = Fixture::new();
        f.tuning(true);
        let growth = f.object(&[(0x80, developer as *const () as usize)]);
        f.global(0x00B4_3D68, growth as usize);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.ids = vec![7, 0, 7];
            s.weights = vec![1.0f32.to_bits(), 0x7FC0_1234, (-2.5f32).to_bits()];
            s.map.insert(99, 0xFFFF_FFFF);
            s.tract_size = Some(2);
        });
        unsafe {
            assert_eq!(fc4p_006be110(f.lv.ptr()), 0xAABB_CC00);
        }
        let trace = calls();
        let (holder, property, variant, resource) = STATE.with(|s| {
            let s = s.borrow();
            (s.holder, s.property, s.variant, s.resource)
        });
        assert_eq!(trace[1], Call::Virtual(resource, 0x1C, vec![]));
        assert_eq!(
            trace[2],
            Call::ByteProperty(holder, 0x47E2_C200, f.lv.ptr() as usize + 0x7C)
        );
        let mut expected = Vec::new();
        for id in [0x47E2_C300, 0x47E2_C301, 0x47E2_C310] {
            expected.extend([
                Call::Virtual(holder, 0x24, vec![id]),
                Call::Virtual(property, 0x14, vec![]),
                Call::Virtual(variant, 0x17C, vec![]),
            ]);
        }
        expected.push(Call::Virtual(holder, 0x24, vec![0x47E2_C320]));
        for (id, slot) in [(0x47E2_C330, 0x19C), (0x47E2_C331, 0x1BC)] {
            expected.extend([
                Call::Virtual(holder, 0x24, vec![id]),
                Call::Virtual(property, 0x14, vec![]),
                Call::Virtual(variant, slot, vec![]),
            ]);
        }
        expected.push(Call::Virtual(variant, 0x10, vec![]));
        for id in [7, 0, 7] {
            expected.extend([
                Call::Virtual(growth as usize, 0x80, vec![id]),
                Call::Map(f.lv.ptr() as usize + 0x8C, id as usize),
                Call::Virtual(variant, 0x10, vec![]),
            ]);
        }
        expected.extend([
            Call::Virtual(holder, 0x24, vec![0x47E2_C332]),
            Call::Virtual(resource, 8, vec![]),
        ]);
        assert_eq!(&trace[3..], expected);
        unsafe {
            assert_eq!(
                core::slice::from_raw_parts(f.lv.ptr().add(0x7C), 7),
                &[2, 1, 255, 70, 120, 50, 5]
            );
        }
        STATE.with(|s| {
            assert_eq!(
                s.borrow().map,
                BTreeMap::from([(0, 0x7FC0_1234), (7, (-2.5f32).to_bits()), (99, 0xFFFF_FFFF)])
            )
        });
    }

    #[test]
    fn tuning_rechecks_id_count_and_keeps_missing_tract_size() {
        let mut f = Fixture::new();
        f.tuning(true);
        let growth = f.object(&[(0x80, developer as *const () as usize)]);
        f.global(0x00B4_3D68, growth as usize);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.ids = vec![3, 4];
            s.weights = vec![9, 10];
            s.counts = vec![2, 1];
        });
        f.view().set_tract_size(255);
        unsafe {
            fc4p_006be110(f.lv.ptr());
        }
        assert_eq!(f.view().tract_size(), 255);
        STATE.with(|s| assert_eq!(s.borrow().map, BTreeMap::from([(3, 9)])));
        assert_eq!(calls().iter().filter(|c| matches!(c, Call::Map(..))).count(), 1);
    }

    #[test]
    fn curves_are_created_reused_deleted_and_null_allocation_is_forwarded() {
        for allocation_fails in [false, true] {
            let mut f = Fixture::new();
            f.tuning(true);
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                s.curves_present = [true; 2];
                s.null_curve_allocation = allocation_fails;
            });
            unsafe {
                fc4p_006be110(f.lv.ptr());
            }
            let trace = calls();
            assert_eq!(trace.iter().filter(|c| matches!(c, Call::Allocate(_, 0x10))).count(), 2);
            assert_eq!(
                trace.iter().filter(|c| matches!(c, Call::CurveNew(_))).count(),
                if allocation_fails { 0 } else { 2 }
            );
            assert_eq!(trace.iter().filter(|c| matches!(c, Call::CurveSet(..))).count(), 2);
            if allocation_fails {
                assert!(trace
                    .iter()
                    .filter_map(|c| if let Call::CurveSet(p, _) = c { Some(p) } else { None })
                    .all(|&p| p == 0));
            } else {
                unsafe {
                    fc4p_006be110(f.lv.ptr());
                }
                assert!(!calls()
                    .iter()
                    .any(|c| matches!(c, Call::Allocate(..) | Call::CurveNew(_))));
                let a = f.view().altitude_curve();
                let b = f.view().desirability_curve();
                unsafe {
                    store_pointer(a, 0, 0x1111 as *mut u8);
                }
                STATE.with(|s| s.borrow_mut().curves_present = [false; 2]);
                unsafe {
                    fc4p_006be110(f.lv.ptr());
                }
                let deletions: Vec<_> = calls()
                    .into_iter()
                    .filter(|c| matches!(c, Call::Free(_) | Call::ArrayFree(_)))
                    .collect();
                assert_eq!(
                    deletions,
                    [Call::ArrayFree(0x1111), Call::Free(a as usize), Call::Free(b as usize)]
                );
                assert!(f.view().altitude_curve().is_null());
                assert!(f.view().desirability_curve().is_null());
            }
        }
    }

    #[test]
    fn messages_keep_the_interface_base_dirty_state_and_full_budget() {
        let mut f = Fixture::new();
        let message = f.object(&[
            (0x10, message_type as *const () as usize),
            (0x4C, message_data3 as *const () as usize),
        ]);
        for kind in CHANGES
            .into_iter()
            .chain([0, 0x6695_6810, 0x6695_6811, 0x6695_6815, 0x6695_6817, 0xFFFF_FFFF])
        {
            f.view().set_dirty(0);
            f.view().set_state(7);
            STATE.with(|s| s.borrow_mut().message_kind = kind);
            unsafe {
                assert_eq!(fc4p_006bf0d0(f.lv.ptr().add(4), message), (kind & !255) | 1);
            }
            assert_eq!(f.view().dirty(), u8::from(CHANGES.contains(&kind)));
            assert_eq!(f.view().state(), 7);
            assert_eq!(calls(), [Call::Virtual(message as usize, 0x10, vec![])]);
        }
        for dirty in [0, 1, 255] {
            for state in [u32::MAX, 0, 8, 9, 10] {
                f.view().set_dirty(dirty);
                f.view().set_state(state);
                STATE.with(|s| s.borrow_mut().message_kind = 0x6695_6816);
                unsafe {
                    assert_eq!(fc4p_006bf0d0(f.lv.ptr().add(4), message), 1);
                }
                let restart = dirty != 0 && state == 9;
                assert_eq!(f.view().dirty(), if restart { 0 } else { dirty });
                assert_eq!(f.view().state(), if restart { 0 } else { state });
                assert_eq!(calls(), [Call::Virtual(message as usize, 0x10, vec![])]);
            }
        }
        for (kind, budget) in [
            (0x6695_6814, 5),
            (0x6695_681A, 0),
            (0x6695_681A, 0x8000_0000),
            (0x6695_681A, u32::MAX),
        ] {
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                s.message_kind = kind;
                s.message_budget = budget;
            });
            unsafe {
                assert_eq!(fc4p_006bf0d0(f.lv.ptr().add(4), message), 0x1357_9B01);
            }
            let mut expected = vec![Call::Virtual(message as usize, 0x10, vec![])];
            if kind == 0x6695_681A {
                expected.push(Call::Virtual(message as usize, 0x4C, vec![]));
            }
            expected.push(Call::Idle(f.lv.ptr() as usize, budget));
            assert_eq!(calls(), expected);
        }
    }

    #[test]
    fn shutdown_orders_removal_and_releases_without_clearing_retained_state() {
        for sim_present in [false, true] {
            let mut f = Fixture::new();
            let (city, zone, terrain, _, server) = f.init_services(sim_present);
            let mut expected = vec![Call::Virtual(city as usize, 0xC8, vec![])];
            let sim = STATE.with(|s| s.borrow().sim);
            if sim_present {
                expected.push(Call::Virtual(sim, 0x50, vec![f.lv.ptr() as u32 + 4]));
            }
            for id in CHANGES {
                expected.push(Call::Virtual(server as usize, 0x18, vec![f.lv.ptr() as u32 + 4, id]));
            }
            expected.push(Call::Virtual(server as usize, 8, vec![]));
            for p in [terrain, zone, city] {
                expected.push(Call::Virtual(p as usize, 8, vec![]));
            }
            let lv = f.view();
            lv.set_city(city);
            lv.set_zone_manager(zone);
            lv.set_terrain(terrain);
            lv.set_message_server(server);
            lv.set_initialized(1);
            lv.set_dirty(255);
            lv.set_state(9);
            lv.set_mean_altitude(123.5);
            for offset in [0x24, 0x28, 0x2C, 0x30] {
                let grid = f.grid(3, 4);
                unsafe {
                    store_pointer(f.lv.ptr(), offset, grid);
                }
                expected.extend([
                    Call::Virtual(grid as usize, 0x10, vec![]),
                    Call::Virtual(grid as usize, 8, vec![]),
                ]);
            }
            for offset in [0x3C, 0x40, 0x84, 0x88] {
                let p = allocate(0x20);
                unsafe {
                    store_pointer(f.lv.ptr(), offset, p);
                }
                if offset >= 0x84 {
                    unsafe {
                        store_pointer(p, 0, if offset == 0x84 { 0x1111 as *mut u8 } else { null_mut() });
                    }
                    if offset == 0x84 {
                        expected.push(Call::ArrayFree(0x1111));
                    }
                }
                expected.push(Call::Free(p as usize));
            }
            calls();
            let before = unsafe { core::slice::from_raw_parts(f.lv.ptr(), 0xA4).to_vec() };
            unsafe {
                assert_eq!(
                    call_thiscall_with_eax(fc4p_006bdf70 as *const () as usize, f.lv.ptr(), &[], 0).0,
                    0xDEAD_BE01
                );
            }
            assert_eq!(calls(), expected);
            let mut expected_bytes = before;
            expected_bytes[0x0C] = 0;
            for offset in [0x14, 0x18, 0x1C, 0x20, 0x24, 0x28, 0x2C, 0x30, 0x3C, 0x40, 0x84, 0x88] {
                expected_bytes[offset..offset + 4].fill(0);
            }
            assert_eq!(unsafe { core::slice::from_raw_parts(f.lv.ptr(), 0xA4) }, expected_bytes);
            unsafe {
                call_thiscall_with_eax(fc4p_006bdf70 as *const () as usize, f.lv.ptr(), &[], 0);
            }
            assert!(calls().is_empty());
        }
    }

    #[test]
    fn shutdown_skips_absent_objects_and_returns_one_when_last_curve_is_absent() {
        let mut f = Fixture::new();
        let (city, ..) = f.init_services(false);
        f.view().set_initialized(1);
        unsafe {
            assert_eq!(
                call_thiscall_with_eax(fc4p_006bdf70 as *const () as usize, f.lv.ptr(), &[], u32::MAX).0,
                1
            );
        }
        assert_eq!(calls(), [Call::Virtual(city as usize, 0xC8, vec![])]);
    }

    #[test]
    fn shutdown_retains_the_previous_cleanup_result_when_the_last_curve_is_absent() {
        for offset in [0x3C, 0x40, 0x84, 0x88] {
            let mut f = Fixture::new();
            f.init_services(false);
            f.view().set_initialized(1);
            let allocation = allocate(0x10);
            unsafe {
                store_pointer(allocation, 0, null_mut());
                store_pointer(f.lv.ptr(), offset, allocation);
            }
            calls();
            let result = unsafe { call_thiscall_with_eax(fc4p_006bdf70 as *const () as usize, f.lv.ptr(), &[], 0).0 };
            // Loading the second iterator replaces EAX, while loading either curve does not.
            assert_eq!(result, if offset == 0x3C { 1 } else { 0xDEAD_BE01 });
            assert!(matches!(calls().last(), Some(Call::Free(p)) if *p == allocation as usize));
        }
    }

    #[test]
    fn retained_services_and_iterators_skip_ownership_and_keep_scan_progress() {
        for sim_present in [false, true] {
            let _fpu = GameFpu::new();
            let mut f = Fixture::new();
            f.tuning(false);
            let (city, zone, terrain, _, server) = f.init_services(sim_present);
            let grid = f.grid(4, 8);
            let lv = f.view();
            lv.set_city(city);
            lv.set_zone_manager(zone);
            lv.set_terrain(terrain);
            lv.set_message_server(server);
            for offset in [0x24, 0x28, 0x2C, 0x30] {
                unsafe {
                    store_pointer(f.lv.ptr(), offset, grid);
                }
            }
            for offset in [0x3C, 0x40] {
                let iterator = allocate(0x20);
                unsafe {
                    store_pointer(f.lv.ptr(), offset, iterator);
                }
            }
            calls();
            let result = unsafe { call_thiscall_with_eax(fc4p_006be3b0 as *const () as usize, f.lv.ptr(), &[], 0).0 };
            assert_eq!(result, if sim_present { 0xCAFE_BA01 } else { 0x4433_2201 });
            let trace = calls();
            assert!(!trace.iter().any(|c| matches!(
                c,
                Call::Allocate(..) | Call::IteratorNew(_) | Call::Virtual(_, 4, _) | Call::Virtual(_, 0x14, _)
            )));
            for p in [lv.tract_iterator(), lv.slope_iterator()] {
                assert_eq!(unsafe { core::slice::from_raw_parts(p, 0x10) }, &[0xA5; 0x10]);
                assert_eq!(
                    unsafe { core::slice::from_raw_parts(p.add(0x10).cast::<i32>(), 4) },
                    &[0, 0, 3, 7]
                );
            }
        }
    }

    #[unsafe(naked)]
    unsafe extern "thiscall" fn invoke_init(_this: *mut u8, _entry_stack: *mut usize) -> u32 {
        core::arch::naked_asm!(
            "mov eax, [esp + 4]", "lea edx, [esp - 4]", "mov [eax], edx",
            "call {entry}", "ret 4", entry = sym fc4p_006be3b0,
        )
    }

    #[test]
    fn histogram_fill_uses_a_zero_word_at_entry_stack_minus_24() {
        let _fpu = GameFpu::new();
        let mut f = Fixture::new();
        f.tuning(false);
        f.init_services(false);
        let grid = f.grid(2, 2);
        let lv = f.view();
        lv.set_intrinsic_grid(grid);
        lv.set_total_grid(grid);
        lv.set_scratch_grid(grid);
        lv.set_slope_grid(grid);
        let mut entry_stack = 0;
        // SAFETY: the fixture supplies the retained grids and services; the helper records entry ESP.
        unsafe {
            invoke_init(f.lv.ptr(), &mut entry_stack);
        }
        STATE.with(|s| {
            let s = s.borrow();
            assert_eq!(s.histogram_fill_address, entry_stack - 0x18);
            assert_eq!(s.subscription_frames, [entry_stack - 0x34; 5]);
        });
        assert!(calls().iter().any(|c| matches!(c, Call::Histogram(_, _, 0))));
    }

    #[test]
    fn restored_grids_skip_setup_iterators_clamp_bounds_and_histogram_uses_x87_truncation() {
        for (x, z, height, bins) in [
            (128, 32, 2500.0, 78),
            (0, -1, -33.5, u32::MAX - 1),
            (i32::MIN, 1, f32::NAN, 0),
        ] {
            let _fpu = GameFpu::new();
            let mut f = Fixture::new();
            let manager = f.tuning(false);
            let (city, zone, terrain, history, server) = f.init_services(false);
            let intrinsic = f.grid(x, z);
            let slope = f.grid(x, z);
            let lv = f.view();
            lv.set_intrinsic_grid(intrinsic);
            lv.set_total_grid(intrinsic);
            lv.set_scratch_grid(intrinsic);
            lv.set_slope_grid(slope);
            lv.set_state(9);
            lv.set_dirty(1);
            STATE.with(|s| s.borrow_mut().max_height = height);
            let mut entry_stack = 0;
            unsafe {
                assert_eq!(invoke_init(f.lv.ptr(), &mut entry_stack), 0x9988_7701);
            }
            STATE.with(|s| assert_eq!(s.borrow().histogram_fill_address, entry_stack - 0x18));
            assert_eq!(lv.state(), 0);
            assert_eq!(lv.dirty(), 1);
            assert_eq!(lv.initialized(), 1);
            assert_eq!(lv.histogram_shift(), 5);
            let trace = calls();
            assert_eq!(
                &trace[..4],
                &[
                    Call::Virtual(city as usize, 4, vec![]),
                    Call::Virtual(zone as usize, 4, vec![]),
                    Call::Virtual(terrain as usize, 4, vec![]),
                    Call::Virtual(manager as usize, 0x0C, vec![0xA521_60F5, 0, 0])
                ]
            );
            let mut middle = Vec::new();
            for (p, grid) in [(lv.tract_iterator(), intrinsic), (lv.slope_iterator(), slope)] {
                middle.extend([
                    Call::Allocate(p as usize, 0x20),
                    Call::IteratorNew(p as usize),
                    Call::Virtual(grid as usize, 0x34, vec![]),
                    Call::Virtual(grid as usize, 0x30, vec![]),
                ]);
                let bounds = unsafe { core::slice::from_raw_parts(p.add(0x10).cast::<i32>(), 4) };
                assert_eq!(bounds, &[0, 0, x.wrapping_sub(1).max(0), z.wrapping_sub(1).max(0)]);
                assert_eq!(unsafe { *p }, 0);
                assert_eq!(unsafe { core::slice::from_raw_parts(p.add(1), 3) }, &[0xA5; 3]);
            }
            middle.extend([
                Call::Virtual(terrain as usize, 0x60, vec![]),
                Call::Histogram(f.lv.ptr() as usize + 0x50, bins, 0),
                Call::Virtual(history as usize, 0x34, vec![0x6A66_3AC8, 1]),
                Call::Virtual(server as usize, 4, vec![]),
            ]);
            for id in CHANGES {
                middle.push(Call::Virtual(server as usize, 0x14, vec![f.lv.ptr() as u32 + 4, id]));
            }
            assert_eq!(&trace[4..], middle);
        }
    }

    #[test]
    fn fresh_grids_use_game_vtables_preserve_padding_and_register_levels_in_order() {
        let _fpu = GameFpu::new();
        let mut f = Fixture::new();
        f.tuning(false);
        f.init_services(true);
        let grid = f.grid(0, 0);
        let table = unsafe { pointer(grid, 0) };
        let serial = [0u32; 8];
        for address in [0x00A8_78C0, 0x00A8_7C20] {
            f.guards.push(fake_data(address, table));
        }
        for address in [0x00A8_78A4, 0x00A8_7C04] {
            f.guards.push(fake_data(address, serial.as_ptr().cast_mut().cast()));
        }
        f.view().set_tract_size(2);
        unsafe {
            call_thiscall_with_eax(fc4p_006be3b0 as *const () as usize, f.lv.ptr(), &[], 0);
        }
        let trace = calls();
        let lv = f.view();
        let mut grid_calls = Vec::new();
        for (p, id, size, class) in [
            (lv.intrinsic_grid(), 0x89D5_BB39, 2, 0x49B9_E602u32),
            (lv.total_grid(), 0x89D5_BB51, 2, 0x49B9_E602),
            (lv.scratch_grid(), 0x89D5_BB60, 2, 0x49B9_E602),
            (lv.slope_grid(), 0x49D5_BB6F, 8, 0x49B9_E60A),
        ] {
            grid_calls.extend([
                Call::Allocate(p as usize, 0x44),
                Call::Virtual(p as usize, 4, vec![]),
                Call::Virtual(p as usize, 0x0C, vec![]),
                Call::Virtual(p as usize, 0x24, vec![size]),
                Call::Virtual(p as usize, 0x18, vec![id]),
            ]);
            let mut expected = [0xA5u8; 0x44];
            expected[..4].copy_from_slice(&(table as u32).to_le_bytes());
            expected[4..8].copy_from_slice(&(serial.as_ptr() as u32).to_le_bytes());
            expected[8] = 0;
            for offset in [0x0C, 0x18, 0x38, 0x3C, 0x40] {
                expected[offset..offset + 4].fill(0);
            }
            expected[0x10..0x14].copy_from_slice(&class.to_le_bytes());
            assert_eq!(unsafe { core::slice::from_raw_parts(p, 0x44) }, expected);
        }
        assert_eq!(&trace[4..24], grid_calls);
        let registrations: Vec<_> = trace
            .iter()
            .filter(|c| matches!(c, Call::Virtual(_, 0x4C, _)))
            .collect();
        let sim = STATE.with(|s| s.borrow().sim);
        assert_eq!(
            registrations,
            [0, 10, 4, 6]
                .map(|level| Call::Virtual(sim, 0x4C, vec![f.lv.ptr() as u32 + 4, level, 2]))
                .iter()
                .collect::<Vec<_>>()
        );
        let pos = trace.iter().position(|c| matches!(c, Call::String(_))).unwrap();
        assert_eq!(trace[pos + 5], Call::ArrayFree(0x1122_3344));
    }
}
