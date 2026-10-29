// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Zot removal, demolition startup and the two lighting queue passes.

use super::*;

layout! {
    /// Lighting counters and the flora service, measured from the primary interface.
    pub struct UpdateState {
        0x38 => flora / set_flora: *mut u8,
        0x9C => lighting_mode / set_lighting_mode: u32,
        0xA0 => lighting_batch / set_lighting_batch: u32,
        0xA4 => lighting_elapsed / set_lighting_elapsed: u32,
    }
}

game_fn! { #[allow(dead_code)] fn remove_prop(this: *mut u8, lot: *mut u8, kind: u32) -> u64 = "thiscall" 0x006C_9780; }
game_fn! { #[allow(dead_code)] fn remove_connections(this: *mut u8, lot: *mut u8) -> u64 = "thiscall" 0x006C_9FB0; }
game_fn! { #[allow(dead_code)] fn clear_lot(this: *mut u8, lot: *mut u8, state: u32, immediate: u32, reason: u32) -> u64 = "thiscall" 0x006C_3C50; }
game_fn! { #[allow(dead_code)] fn light_building(this: *mut u8, building: *mut u8, enabled: u32) -> u64 = "thiscall" 0x006C_8E10; }

unsafe fn direct(address: u32, this: *mut u8, args: &[u32], eax: u32, edx: u32) -> RawCall {
    let mut call = RawCall::new(Conv::Thiscall, crate::abi::target(address), this as u32, args);
    call.eax = eax;
    call.edx = edx;
    unsafe {
        call_raw(&mut call);
    }
    call
}

port! {
    /// Removes the current zot prop when its property is nonzero, then clears the existing property.
    #[port(address = 0x006C_B940, owner = "lotdeveloper", name = "cSC4LotDeveloper::RemoveBuildingZotProp")]
    pub unsafe extern "fastcall" fn fc4p_006cb940(this: *mut u8, entry_edx: u32, lot: *mut u8) -> u64 {
        unsafe {
            let mut call = virtual_call(lot, 0x0C, &[], table(lot), entry_edx, false);
            let holder = call.eax as *mut u8;
            call = virtual_call(holder, 0x24, &[0xC999_C45E], call.eax, table(holder), false);
            let property = call.eax as *mut u8;
            if !property.is_null() {
                call = virtual_call(property, 0x14, &[], call.eax, table(property), false);
                let variant = call.eax as *mut u8;
                call = virtual_call(variant, 0x124, &[], table(variant), call.edx, false);
                call.eax = (call.eax as *const u32).read_unaligned();
                if call.eax != 0 {
                    call = direct(0x006C_9780, this, &[lot as u32, call.eax], call.eax, call.edx);
                }
                call = virtual_call(variant, 0x68, &[0], call.eax, table(variant), false);
            }
            result(call)
        }
    }
}

/// Advances only the cursor fields written by a forward step, retaining the containing queue's state.
unsafe fn advance_cursor(this: *mut u8) -> (u32, u32) {
    unsafe {
        let cursor = this.add(0x8C).cast::<u32>();
        let current = cursor.read_unaligned().wrapping_add(4);
        let last = cursor.add(2).read_unaligned();
        cursor.write_unaligned(current);
        if current != last {
            return (current, last);
        }
        let node = cursor.add(3).read_unaligned().wrapping_add(4);
        cursor.add(3).write_unaligned(node);
        let first = (node as *const u32).read_unaligned();
        cursor.add(1).write_unaligned(first);
        cursor.add(2).write_unaligned(first.wrapping_add(128));
        let first = cursor.add(1).read_unaligned();
        cursor.write_unaligned(first);
        (first, last)
    }
}

port! {
    /// Supplies the caller's tick word so the enabled byte can be written back in place.
    #[port(address = 0x006C_BD60, owner = "lotdeveloper", name = "AdvanceLightingBatch", naked)]
    pub unsafe extern "C" fn fc4p_006cbd60() {
        core::arch::naked_asm!("lea eax, [esp + 4]", "push eax", "call {}", "ret 4", sym lighting_batch);
    }
}

unsafe extern "fastcall" fn lighting_batch(this: *mut u8, entry_edx: u32, tick: *mut u32) -> u64 {
    unsafe {
        let state = UpdateState::from_ptr(this);
        let elapsed = state.lighting_elapsed().wrapping_add(tick.read_unaligned());
        state.set_lighting_elapsed(elapsed);
        if elapsed <= 200 {
            return (u64::from(entry_edx) << 32) | u64::from(elapsed);
        }
        let enabled = u32::from(state.lighting_mode() == 2);
        let mut remaining = state.lighting_batch();
        state.set_lighting_elapsed(0);
        let flags = (tick.read_unaligned() & !255) | enabled;
        tick.cast::<u8>().write(enabled as u8);
        let mut returned = (u64::from(entry_edx) << 32) | u64::from(elapsed);
        while remaining != 0 {
            let end = LotDeveloper::from_ptr(this).night_end();
            let current = this.add(0x8C).cast::<*const u32>().read_unaligned();
            remaining = remaining.wrapping_sub(1);
            if current == end.current {
                state.set_lighting_mode(0);
                return (u64::from(end.node as u32) << 32) | u64::from(current as u32);
            }
            let building = current.read_unaligned();
            let (eax, edx) = advance_cursor(this);
            returned = result(direct(0x006C_8E10, this, &[building, flags], eax, edx));
        }
        returned
    }
}

port! {
    /// Retains the final ECX word and supplies the caller's mode word to the full lighting pass.
    #[port(address = 0x006C_BE50, owner = "lotdeveloper", name = "FinishLightingQueue", naked)]
    pub unsafe extern "C" fn fc4p_006cbe50() {
        core::arch::naked_asm!(
            "sub esp, 4", "lea eax, [esp + 8]", "push esp", "push eax", "call {}", "pop ecx", "ret 4",
            sym finish_lighting,
        );
    }
}

unsafe extern "fastcall" fn finish_lighting(
    this: *mut u8,
    _entry_edx: u32,
    mode: *mut u32,
    output_ecx: *mut u32,
) -> u64 {
    unsafe {
        let developer = LotDeveloper::from_ptr(this);
        let flags = (mode.read_unaligned() & !255) | u32::from(mode.read_unaligned() == 2);
        mode.cast::<u8>().write(flags as u8);
        developer.set_night_cursor(developer.night_begin());
        loop {
            let end = developer.night_end();
            let current = this.add(0x8C).cast::<*const u32>().read_unaligned();
            if current == end.current {
                break;
            }
            direct(
                0x006C_8E10,
                this,
                &[current.read_unaligned(), flags],
                flags,
                current.read_unaligned(),
            );
            advance_cursor(this);
        }
        let begin = developer.night_begin();
        let cursor = this.add(0x8C).cast::<u32>();
        cursor.write_unaligned(begin.current as u32);
        cursor.add(1).write_unaligned(begin.first as u32);
        UpdateState::from_ptr(this).set_lighting_mode(0);
        cursor.add(2).write_unaligned(begin.last as u32);
        cursor.add(3).write_unaligned(begin.node as u32);
        output_ecx.write_unaligned(begin.last as u32);
        (u64::from(begin.node as u32) << 32) | u64::from(begin.first as u32)
    }
}

port! {
    /// Clears flora on foundation lots, removes props and the zot value, removes network nodes, then
    /// clears the lot into demolition state five. Service fields are read after preceding callbacks.
    #[port(address = 0x006C_C1D0, owner = "lotdeveloper", name = "cSC4LotDeveloper::StartDemolishLot")]
    pub unsafe extern "fastcall" fn fc4p_006cc1d0(this: *mut u8, entry_edx: u32, lot: *mut u8) -> u64 {
        unsafe {
            let developer = LotDeveloper::from_ptr(this);
            let mut call = virtual_call(lot, 0x30, &[], table(lot), entry_edx, false);
            if call.eax as u8 & 1 != 0 {
                let mut rect = [0u32; 4];
                virtual_call(lot, 0x38, &[rect.as_mut_ptr() as u32], rect.as_mut_ptr() as u32, table(lot), false);
                let flora = UpdateState::from_ptr(this).flora();
                call = virtual_call(flora, 0x50, &[rect.as_ptr() as u32, 0], rect.as_ptr() as u32, table(flora), false);
            }
            let props = developer.prop_manager();
            call = virtual_call(props, 0x70, &[lot as u32], call.eax, table(props), false);
            call = virtual_call(lot, 0x0C, &[], table(lot), call.edx, false);
            let holder = call.eax as *mut u8;
            call = virtual_call(holder, 0x24, &[0xC999_C45E], call.eax, table(holder), false);
            let property = call.eax as *mut u8;
            if !property.is_null() {
                call = virtual_call(property, 0x14, &[], call.eax, table(property), false);
                let variant = call.eax as *mut u8;
                call = virtual_call(variant, 0x68, &[0], call.eax, table(variant), false);
            }
            call = direct(0x006C_9FB0, this, &[lot as u32], call.eax, call.edx);
            result(direct(0x006C_3C50, this, &[lot as u32, 5, 1, 0], call.eax, call.edx))
        }
    }
}

pub const PORTS: &[Port] = &[
    fc4p_006cb940::PORT,
    fc4p_006cbd60::PORT,
    fc4p_006cbe50::PORT,
    fc4p_006cc1d0::PORT,
];
