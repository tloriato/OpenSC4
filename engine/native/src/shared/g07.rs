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

//! Shared helpers, group 07: small functions that several classes use. Each function is independent; its game functions and globals are declared in this file.

use crate::port::Port;
use core::ptr::{read_unaligned, write_unaligned};

// The getter entries leave all registers except the specified result bits alone.
macro_rules! word_getter {
    ($export:ident, $address:literal, $name:literal, $offset:literal) => {
        port! {
            #[port(address = $address, owner = "shared", name = $name, naked)]
            pub unsafe extern "C" fn $export() {
                core::arch::naked_asm!("mov eax, dword ptr [ecx + {offset}]", "ret", offset = const $offset);
            }
        }
    };
}
macro_rules! address_getter {
    ($export:ident, $address:literal, $name:literal, $offset:literal) => {
        port! {
            #[port(address = $address, owner = "shared", name = $name, naked)]
            pub unsafe extern "C" fn $export() {
                core::arch::naked_asm!("lea eax, [ecx + {offset}]", "ret", offset = const $offset);
            }
        }
    };
}
macro_rules! constant_getter {
    ($export:ident, $address:literal, $name:literal, $value:literal) => {
        port! {
            #[port(address = $address, owner = "shared", name = $name, naked)]
            pub unsafe extern "C" fn $export() {
                core::arch::naked_asm!("mov eax, {value}", "ret", value = const $value as u32);
            }
        }
    };
}

word_getter!(fc4p_006e0130, 0x006e0130, "advice_event_id", 0xa8);
word_getter!(fc4p_007624d0, 0x007624d0, "popup_tracker_value", 0xec);
word_getter!(fc4p_0076b620, 0x0076b620, "graph_hit_test_data", 0x148);
word_getter!(fc4p_00798370, 0x00798370, "city_prop_manager", 0x130);
word_getter!(fc4p_007c1910, 0x007c1910, "render_lighting_manager", 0x118);
word_getter!(fc4p_007c9fb0, 0x007c9fb0, "render_view_utilities", 0x114);
word_getter!(fc4p_007c9fd0, 0x007c9fd0, "render_camera_control", 0x8c);
word_getter!(fc4p_007e56f0, 0x007e56f0, "city_demolition_utility", 0x120);
word_getter!(fc4p_00809240, 0x00809240, "texture_binding_resource_group", 0x70);
word_getter!(fc4p_00809250, 0x00809250, "buffer_cache_last_access", 0x64);
word_getter!(fc4p_0081f910, 0x0081f910, "socket_notification_target", 0x50);
address_getter!(fc4p_0076b630, 0x0076b630, "line_graph_window_interface", -216);
address_getter!(fc4p_00800640, 0x00800640, "advice_title", 0x30);
constant_getter!(fc4p_006d5050, 0x006d5050, "class_id_aa3fe6bc", 0xaa3fe6bc);
constant_getter!(fc4p_00700e80, 0x00700e80, "class_id_ea230a28", 0xea230a28);

port! {
    /// Returns the coverage float directly in ST0, without a second rounding.
    #[port(address = 0x006e0200, owner = "shared", name = "fire_average_coverage", naked)]
    pub unsafe extern "C" fn fc4p_006e0200() {
        core::arch::naked_asm!("fld dword ptr [ecx + 0x98]", "ret");
    }
}
port! {
    /// Returns only AL; the upper three bytes of EAX retain their entry value.
    #[port(address = 0x00741740, owner = "shared", name = "page_flipping_enabled", naked)]
    pub unsafe extern "C" fn fc4p_00741740() {
        core::arch::naked_asm!("mov al, byte ptr [ecx + 0xdd]", "ret");
    }
}
port! {
    /// Consumes two arguments and leaves all registers and memory alone.
    #[port(address = 0x006f7dc0, owner = "shared", name = "check_time_of_day", naked)]
    pub unsafe extern "C" fn fc4p_006f7dc0() {
        core::arch::naked_asm!("ret 8");
    }
}

// Each interface entry adjusts ECX and transfers control with the original stack and registers.
// The test build resolves the same declared game address to a fake.
macro_rules! interface_thunk {
    ($export:ident, $address:literal, $name:literal, $operation:literal, $adjust:literal,
     $callee:ident, $target:literal, ($($arg:ident: $ty:ty),*)) => {
        game_fn! {
            #[allow(dead_code)]
            fn $callee(this: *mut u8 $(, $arg: $ty)*) -> u64 = "thiscall" $target;
        }
        #[cfg(test)]
        mod $callee {
            pub unsafe extern "thiscall" fn bridge(this: *mut u8 $(, $arg: $ty)*) -> u64 {
                // SAFETY: the thunk forwards the original convention and argument words.
                unsafe { super::$callee(this $(, $arg)*) }
            }
        }
        port! {
            #[port(address = $address, owner = "shared", name = $name, naked)]
            pub unsafe extern "C" fn $export() {
                #[cfg(not(test))]
                core::arch::naked_asm!(
                    concat!($operation, " ecx, {adjust}"), "push {target}", "ret",
                    adjust = const $adjust, target = const $target,
                );
                #[cfg(test)]
                core::arch::naked_asm!(
                    concat!($operation, " ecx, {adjust}"), "jmp {target}",
                    adjust = const $adjust, target = sym $callee::bridge,
                );
            }
        }
    };
}
interface_thunk!(
    fc4p_006bdde0,
    0x006bdde0,
    "land_value_serializable_release",
    "sub",
    8,
    target_006bdde0,
    0x006bd8a0,
    ()
);
interface_thunk!(
    fc4p_006c2270,
    0x006c2270,
    "texture_interface_add_ref",
    "sub",
    4,
    target_006c2270,
    0x004664a0,
    ()
);
interface_thunk!(
    fc4p_006c2290,
    0x006c2290,
    "texture_secondary_add_ref",
    "sub",
    8,
    target_006c2290,
    0x004664a0,
    ()
);
interface_thunk!(
    fc4p_006ce6f0,
    0x006ce6f0,
    "audio_interface_add_ref",
    "sub",
    4,
    target_006ce6f0,
    0x004f0240,
    ()
);
interface_thunk!(
    fc4p_006ce720,
    0x006ce720,
    "audio_secondary_add_ref",
    "sub",
    8,
    target_006ce720,
    0x004f0240,
    ()
);
interface_thunk!(fc4p_006dfc40, 0x006dfc40, "dispatch_interface_query", "sub", 8, target_006dfc40, 0x006ac550, (a0: u32, a1: u32));
interface_thunk!(
    fc4p_006dfc50,
    0x006dfc50,
    "dispatch_interface_final_release",
    "sub",
    4,
    target_006dfc50,
    0x006dfaf0,
    ()
);
interface_thunk!(
    fc4p_006dfc60,
    0x006dfc60,
    "dispatch_secondary_final_release",
    "sub",
    8,
    target_006dfc60,
    0x006dfaf0,
    ()
);
interface_thunk!(
    fc4p_006e17a0,
    0x006e17a0,
    "fire_interface_add_ref",
    "sub",
    8,
    target_006e17a0,
    0x0082be80,
    ()
);
interface_thunk!(
    fc4p_006e17f0,
    0x006e17f0,
    "fire_secondary_add_ref",
    "sub",
    16,
    target_006e17f0,
    0x0082be80,
    ()
);
interface_thunk!(
    fc4p_006e1800,
    0x006e1800,
    "fire_interface_release",
    "sub",
    8,
    target_006e1800,
    0x006ad1b0,
    ()
);
interface_thunk!(
    fc4p_006e1820,
    0x006e1820,
    "fire_secondary_release",
    "sub",
    16,
    target_006e1820,
    0x006ad1b0,
    ()
);
interface_thunk!(
    fc4p_00762970,
    0x00762970,
    "catalog_interface_add_ref",
    "sub",
    4,
    target_00762970,
    0x00762470,
    ()
);
interface_thunk!(
    fc4p_00762980,
    0x00762980,
    "catalog_interface_release",
    "sub",
    4,
    target_00762980,
    0x00762480,
    ()
);
interface_thunk!(
    fc4p_00762ec0,
    0x00762ec0,
    "popup_interface_release",
    "sub",
    4,
    target_00762ec0,
    0x00762bd0,
    ()
);
interface_thunk!(
    fc4p_007637a0,
    0x007637a0,
    "lua_interface_add_ref",
    "sub",
    8,
    target_007637a0,
    0x0057eb00,
    ()
);
interface_thunk!(
    fc4p_007637b0,
    0x007637b0,
    "lua_interface_release",
    "sub",
    8,
    target_007637b0,
    0x0057eb70,
    ()
);
interface_thunk!(
    fc4p_007637d0,
    0x007637d0,
    "lua_secondary_add_ref",
    "sub",
    12,
    target_007637d0,
    0x0057eb00,
    ()
);
interface_thunk!(
    fc4p_007637e0,
    0x007637e0,
    "lua_secondary_release",
    "sub",
    12,
    target_007637e0,
    0x0057eb70,
    ()
);
interface_thunk!(
    fc4p_007b0e40,
    0x007b0e40,
    "advice_window_interface_add_ref",
    "sub",
    220,
    target_007b0e40,
    0x007be160,
    ()
);
interface_thunk!(
    fc4p_007b0e50,
    0x007b0e50,
    "advice_window_interface_release",
    "sub",
    220,
    target_007b0e50,
    0x007be170,
    ()
);
interface_thunk!(
    fc4p_007be550,
    0x007be550,
    "advice_window_secondary_add_ref",
    "sub",
    216,
    target_007be550,
    0x007be160,
    ()
);
interface_thunk!(
    fc4p_007be560,
    0x007be560,
    "advice_window_secondary_release",
    "sub",
    216,
    target_007be560,
    0x007be170,
    ()
);
interface_thunk!(
    fc4p_007be720,
    0x007be720,
    "bar_graph_interface_release",
    "sub",
    216,
    target_007be720,
    0x008fb72e,
    ()
);
interface_thunk!(
    fc4p_007bf5c0,
    0x007bf5c0,
    "catalog_window_add_ref",
    "sub",
    220,
    target_007bf5c0,
    0x00762470,
    ()
);
interface_thunk!(
    fc4p_007bf5d0,
    0x007bf5d0,
    "catalog_window_release",
    "sub",
    220,
    target_007bf5d0,
    0x00762480,
    ()
);
interface_thunk!(
    fc4p_00800c30,
    0x00800c30,
    "draw_system_add_ref",
    "add",
    4,
    target_00800c30,
    0x009297a0,
    ()
);
interface_thunk!(
    fc4p_00800c40,
    0x00800c40,
    "draw_system_release",
    "add",
    4,
    target_00800c40,
    0x0091dd89,
    ()
);
interface_thunk!(
    fc4p_00808600,
    0x00808600,
    "resource_interface_add_ref",
    "add",
    4,
    target_00808600,
    0x009753b7,
    ()
);
interface_thunk!(
    fc4p_00808610,
    0x00808610,
    "resource_interface_release",
    "add",
    4,
    target_00808610,
    0x009753c5,
    ()
);
interface_thunk!(fc4p_008089a0, 0x008089a0, "resource_interface_query", "sub", 4, target_008089a0, 0x008085c0, (a0: u32, a1: u32));
interface_thunk!(fc4p_008089b0, 0x008089b0, "delete_resource_interface", "sub", 4, target_008089b0, 0x00808a00, (a0: u32));
interface_thunk!(fc4p_0082ae90, 0x0082ae90, "delete_draw_context_interface", "sub", 4, target_0082ae90, 0x0082b7d0, (a0: u32));
interface_thunk!(
    fc4p_00875aba,
    0x00875aba,
    "player_interface_add_ref",
    "sub",
    4,
    target_00875aba,
    0x00945942,
    ()
);

layout! {
    /// Four consecutive rectangle words, also used as a plain record by copy helpers.
    struct RectangleWords {
        0x00 => left / set_left: u32,
        0x04 => top / set_top: u32,
        0x08 => right / set_right: u32,
        0x0c => bottom / set_bottom: u32,
    }
}
layout! {
    /// Draw-context fields used by lock, format, and stride helpers.
    struct DrawContextFields {
        0x10 => refresh_target / set_refresh_target: *mut u8,
        0x28 => locks / set_locks: u32,
        0x2c => format / set_format: u32,
        0x44 => stride / set_stride: u32,
        0x49 => dirty / set_dirty: u8,
    }
}
layout! {
    /// A holder whose callbacks may replace its surface pointer.
    struct SurfaceHolder {
        0x04 => surface / set_surface: *mut u8,
        0x08 => mode / set_mode: u32,
    }
}
layout! {
    /// The two original income words read by the ordinance getter.
    struct OrdinanceIncome {
        0x80 => low / set_low: u32,
        0x84 => high / set_high: u32,
    }
}
layout! {
    /// The last simulated income, stored low word before high word.
    struct SimulatedIncome {
        0x98 => low / set_low: u32,
        0x9c => high / set_high: u32,
    }
}
layout! {
    /// Sentinel-list links and a node's payload word.
    struct ListNode {
        0x00 => next / set_next: *mut u8,
        0x08 => payload / set_payload: u32,
    }
}
layout! {
    /// A budget record's table index and owning table object.
    struct BudgetRecord {
        0x04 => index / set_index: u32,
        0x08 => owner / set_owner: *mut u8,
    }
}

/// Reads a word at the helper's original object offset.
unsafe fn word(this: *mut u8, offset: usize) -> u32 {
    // SAFETY: each caller supplies a live object with the requested field.
    unsafe { read_unaligned(this.wrapping_add(offset).cast()) }
}
/// Writes a word at the helper's original object offset.
unsafe fn put_word(this: *mut u8, offset: usize, value: u32) {
    // SAFETY: each caller supplies a writable field.
    unsafe { write_unaligned(this.wrapping_add(offset).cast(), value) };
}

/// Entry registers and the original argument words, saved by the small ABI entry.
#[repr(C)]
struct Entry {
    edx: u32,
    eax: u32,
    ecx: u32,
    return_address: u32,
}
impl Entry {
    /// Reads an original stack argument without changing its width or signedness.
    unsafe fn arg(&self, index: usize) -> u32 {
        // SAFETY: the entry's caller supplied this argument word after its return address.
        unsafe { read_unaligned((self as *const Self).cast::<u32>().add(4 + index)) }
    }
    fn result(&self, eax: u32) -> u64 {
        (u64::from(self.edx) << 32) | u64::from(eax)
    }
}

// Save volatile entry registers before calling Rust, and restore the original stack convention.
// The Rust body returns EDX:EAX and can also supply ECX through the saved word.
macro_rules! register_port {
    ($export:ident, $address:literal, $name:literal, $pop:literal,
     $body:ident($this:ident, $entry:ident) $code:block) => {
        port! {
            #[port(address = $address, owner = "shared", name = $name, naked)]
            pub unsafe extern "C" fn $export() {
                core::arch::naked_asm!(
                    "push ecx", "push eax", "push edx", "mov edx, esp", "call {body}",
                    "add esp, 8", "pop ecx", "ret {pop}", body = sym $body, pop = const $pop,
                );
            }
        }
        unsafe extern "fastcall" fn $body($this: *mut u8, saved: *mut Entry) -> u64 {
            // SAFETY: the entry saved this frame and the caller's original arguments follow it.
            let $entry = unsafe { &mut *saved };
            $code
        }
    };
}

game_fn! { fn free_object(this: *mut u8) -> u64 = "C" 0x005e5620; }
game_fn! { fn destroy_zone(this: *mut u8) -> u64 = "thiscall" 0x006bbbc0; }
game_fn! { fn destroy_filter(this: *mut u8) -> u64 = "thiscall" 0x005e95c0; }
game_fn! { fn copy_station_list(this: *mut u8, output: *mut u8, source: *mut u8) -> u64 = "thiscall" 0x006dfc70; }
game_fn! { fn copy_budget(this: *mut u8, output: *mut u8) = "thiscall" 0x006e9900; }

register_port! { fc4p_006ba280, 0x006ba280, "delete_zone_record", 4,
    delete_zone_record(this, entry) {
        // SAFETY: the game supplies its live record and allocator ownership.
        unsafe {
            let flags = entry.arg(0);
            put_word(this, 0, 0x00ab15dc);
            if flags & 1 != 0 { return (free_object(this) & 0xffff_ffff_0000_0000) | u64::from(this as u32); }
        }
        entry.result(this as u32)
    }
}
register_port! { fc4p_006bbd40, 0x006bbd40, "delete_landfill_zone", 4,
    delete_landfill_zone(this, entry) {
        // SAFETY: the destructor and deallocator receive the original object in order.
        unsafe {
            let mut result = destroy_zone(this);
            if entry.arg(0) & 1 != 0 { result = free_object(this); }
            (result & 0xffff_ffff_0000_0000) | u64::from(this as u32)
        }
    }
}
register_port! { fc4p_006c4a00, 0x006c4a00, "delete_default_occupant_filter", 4,
    delete_default_occupant_filter(this, entry) {
        // SAFETY: the destructor and deallocator receive the original object in order.
        unsafe {
            let mut result = destroy_filter(this);
            if entry.arg(0) & 1 != 0 { result = free_object(this); }
            (result & 0xffff_ffff_0000_0000) | u64::from(this as u32)
        }
    }
}
port! {
    #[port(address = 0x006c2040, owner = "shared", name = "persist_database_segment_interface")]
    pub unsafe extern "thiscall" fn fc4p_006c2040(this: *mut u8) -> *mut u8 {
        if this.is_null() { this } else { this.wrapping_add(8) }
    }
}
register_port! { fc4p_006c5fe0, 0x006c5fe0, "copy_four_words", 4,
    copy_four_words(this, entry) {
        // SAFETY: transfers read each source word only after writing the preceding output word.
        unsafe {
            let output = entry.arg(0) as *mut u8;
            for offset in [0, 4, 8] { put_word(output, offset, word(this, offset)); }
            let fourth = word(this, 12);
            put_word(output, 12, fourth);
            entry.ecx = fourth;
            (u64::from(output as u32) << 32) | u64::from(output as u32)
        }
    }
}
register_port! { fc4p_006ce230, 0x006ce230, "set_field_3c", 4,
    set_field_3c(this, entry) {
        // SAFETY: the caller supplies the field and one stack argument.
        unsafe { let value = entry.arg(0); put_word(this, 0x3c, value); entry.result(value) }
    }
}
port! {
    #[port(address = 0x006e0180, owner = "shared", name = "fire_stations")]
    pub unsafe extern "thiscall" fn fc4p_006e0180(this: *mut u8, output: *mut u8) -> u64 {
        // SAFETY: the simulator holds the embedded list and copy interface.
        unsafe { copy_station_list(this.add(8), output, this.add(0x88)) }
    }
}
port! {
    #[port(address = 0x00700f40, owner = "shared", name = "copy_budget_state")]
    pub unsafe extern "thiscall" fn fc4p_00700f40(this: *mut u8, output: *mut u8) -> *mut u8 {
        // SAFETY: the copy helper operates on the embedded interface.
        unsafe { copy_budget(this.add(4), output); }
        output
    }
}
register_port! { fc4p_0076aaa0, 0x0076aaa0, "shutdown_preferences_window", 0,
    shutdown_preferences_window(this, entry) {
        // SAFETY: the object contains the callback word.
        unsafe { put_word(this, 8, 0); }
        entry.result((entry.eax & !255) | 1)
    }
}
register_port! { fc4p_0076bb00, 0x0076bb00, "clear_vector_words", 0,
    clear_vector_words(this, entry) {
        // SAFETY: all three words belong to the caller's vector header.
        unsafe { put_word(this, 0, 0); put_word(this, 4, 0); put_word(this, 8, 0); }
        entry.result(0)
    }
}
register_port! { fc4p_007bfa00, 0x007bfa00, "set_render_flag", 4,
    set_render_flag(this, entry) {
        // SAFETY: only the flag byte is writable; the argument occupies a complete stack word.
        unsafe {
            let value = entry.arg(0) & 255;
            *this.add(0x102) = value as u8;
            entry.result((entry.eax & !255) | value)
        }
    }
}
register_port! { fc4p_007c2740, 0x007c2740, "enable_transparent_background", 4,
    enable_transparent_background(this, entry) {
        // SAFETY: only the flag byte is writable; the argument occupies a complete stack word.
        unsafe {
            let value = entry.arg(0) & 255;
            *this.add(0x62) = value as u8;
            entry.result((entry.eax & !255) | value)
        }
    }
}
register_port! { fc4p_007c1870, 0x007c1870, "rectangle_is_empty", 0,
    rectangle_is_empty(this, entry) {
        // SAFETY: the rectangle stores left, top, right, bottom as signed words.
        unsafe {
            let rect = RectangleWords::from_ptr(this);
            if rect.right() as i32 <= rect.left() as i32 { return entry.result(1); }
            let bottom = rect.bottom();
            (u64::from(bottom) << 32) | u64::from(bottom as i32 <= rect.top() as i32)
        }
    }
}
register_port! { fc4p_007faf40, 0x007faf40, "virtual_base_interface", 0,
    virtual_base_interface(this, entry) {
        if this.is_null() { return entry.result(0); }
        // SAFETY: the object points at its virtual-base displacement table.
        let displacement = unsafe { word(word(this, 4) as *mut u8, 4) };
        (u64::from(displacement) << 32) | u64::from((this as u32).wrapping_add(displacement).wrapping_add(4))
    }
}
port! {
    #[port(address = 0x0080ae60, owner = "shared", name = "construct_texture_binding")]
    pub unsafe extern "thiscall" fn fc4p_0080ae60(this: *mut u8) -> *mut u8 {
        // SAFETY: the first word is the object's vtable.
        unsafe { put_word(this, 0, 0x00abd92c); }
        this
    }
}
register_port! { fc4p_0080ae70, 0x0080ae70, "reset_texture_binding_vtable", 0,
    reset_texture_binding_vtable(this, entry) {
        // SAFETY: the first word is the object's vtable.
        unsafe { put_word(this, 0, 0x00abd92c); }
        entry.result(entry.eax)
    }
}
port! {
    #[port(address = 0x0080c350, owner = "shared", name = "gambling_monthly_income")]
    pub unsafe extern "thiscall" fn fc4p_0080c350(this: *mut u8) -> u64 {
        // SAFETY: the two adjacent words store the complete income value.
        unsafe {
            let income = OrdinanceIncome::from_ptr(this);
            let low = income.low();
            (u64::from(income.high()) << 32) | u64::from(low)
        }
    }
}
port! {
    #[port(address = 0x0082ace0, owner = "shared", name = "has_active_locks")]
    pub unsafe extern "thiscall" fn fc4p_0082ace0(this: *mut u8) -> u64 {
        // SAFETY: the word field stores the lock count.
        let count = unsafe { word(this, 0x28) };
        (u64::from(count) << 32) | u64::from(count != 0)
    }
}
register_port! { fc4p_0082ad00, 0x0082ad00, "copy_context_rectangle", 4,
    copy_context_rectangle(this, entry) {
        // SAFETY: the rectangle and output cover four words; transfers are sequential.
        unsafe {
            let output = entry.arg(0) as *mut u8;
            let source = this.add(0x18);
            put_word(output, 0, word(source, 0));
            put_word(output, 4, word(source, 4));
            let third = word(source, 8);
            put_word(output, 8, third);
            let fourth = word(source, 12);
            put_word(output, 12, fourth);
            entry.ecx = fourth;
            (u64::from(third) << 32) | u64::from(output as u32)
        }
    }
}
register_port! { fc4p_0082ad20, 0x0082ad20, "context_stride_bits", 0,
    context_stride_bits(this, entry) {
        // SAFETY: the format width is stored at this offset.
        unsafe { entry.result(DrawContextFields::from_ptr(this).stride().wrapping_shl(3)) }
    }
}
port! {
    #[port(address = 0x0082be80, owner = "shared", name = "draw_context_add_ref")]
    pub unsafe extern "thiscall" fn fc4p_0082be80(this: *mut u8) -> u32 {
        // SAFETY: the reference counter belongs to this live object.
        unsafe {
            let count = word(this, 0xc4).wrapping_add(1);
            put_word(this, 0xc4, count);
            count
        }
    }
}

/// Calls a virtual slot with the volatile register inputs left by its caller.
unsafe fn virtual_result(this: *mut u8, offset: usize, eax: u32, edx: u32, args: &[u32]) -> u64 {
    // SAFETY: callers supply the original object's slot and complete argument words.
    unsafe {
        let mut c = crate::abi::RawCall::new(
            crate::abi::Conv::Thiscall,
            crate::abi::vslot(this, offset),
            this as u32,
            args,
        );
        c.eax = eax;
        c.edx = edx;
        crate::abi::call_raw(&mut c);
        (u64::from(c.edx) << 32) | u64::from(c.eax)
    }
}

register_port! { fc4p_006d5140, 0x006d5140, "release_signed_reference", 0,
    release_signed_reference(this, entry) {
        // SAFETY: the counter and both lifecycle slots belong to the live object.
        unsafe {
            let count = word(this, 0x10);
            if count as i32 > 1 {
                put_word(this, 0x10, count.wrapping_sub(1));
                return entry.result(count.wrapping_sub(1));
            }
            let first = virtual_result(this, 0x10, word(this, 0), entry.edx, &[]);
            virtual_result(this, 0x14, first as u32, word(this, 0), &[1]) & 0xffff_ffff_0000_0000
        }
    }
}
register_port! { fc4p_00762bd0, 0x00762bd0, "popup_tracker_release", 0,
    popup_tracker_release(this, entry) {
        // SAFETY: the counter and deleting slot belong to the live object.
        unsafe {
            let count = word(this, 8);
            if count > 1 {
                put_word(this, 8, count.wrapping_sub(1));
                return entry.result(count.wrapping_sub(1));
            }
            virtual_result(this, 0x10, word(this, 0), entry.edx, &[1]) & 0xffff_ffff_0000_0000
        }
    }
}
register_port! { fc4p_00891c72, 0x00891c72, "release_unsigned_reference", 0,
    release_unsigned_reference(this, entry) {
        // SAFETY: the counter and deleting slot belong to the live object.
        unsafe {
            let count = word(this, 4);
            if count > 1 {
                put_word(this, 4, count.wrapping_sub(1));
                return entry.result(count.wrapping_sub(1));
            }
            virtual_result(this, 0x38, word(this, 0), entry.edx, &[1]) & 0xffff_ffff_0000_0000
        }
    }
}

register_port! { fc4p_006d66f0, 0x006d66f0, "copy_ten_byte_records", 0,
    copy_ten_byte_records(_this, entry) {
        // SAFETY: this cdecl helper receives begin, end, and output as stack words.
        unsafe {
            let mut source = entry.arg(0);
            let end = entry.arg(1);
            let mut output = entry.arg(2);
            while source != end {
                if output != 0 {
                    put_word(output as *mut u8, 0, word(source as *mut u8, 0));
                    put_word(output as *mut u8, 4, word(source as *mut u8, 4));
                    let tail = read_unaligned((source.wrapping_add(8)) as *const u16);
                    write_unaligned(output.wrapping_add(8) as *mut u16, tail);
                }
                source = source.wrapping_add(10);
                output = output.wrapping_add(10);
            }
            entry.ecx = source;
            (u64::from(end) << 32) | u64::from(output)
        }
    }
}
register_port! { fc4p_007266f0, 0x007266f0, "copy_sixteen_byte_records", 0,
    copy_sixteen_byte_records(_this, entry) {
        // SAFETY: this cdecl helper receives begin, end, and output as stack words.
        unsafe {
            let mut source = entry.arg(0);
            let end = entry.arg(1);
            let mut output = entry.arg(2);
            while source != end {
                if output != 0 {
                    for offset in [0, 4, 8, 12] { put_word(output as *mut u8, offset, word(source as *mut u8, offset)); }
                }
                source = source.wrapping_add(16);
                output = output.wrapping_add(16);
            }
            entry.ecx = source;
            (u64::from(end) << 32) | u64::from(output)
        }
    }
}
register_port! { fc4p_00725810, 0x00725810, "copy_sixteen_byte_records_backward", 0,
    copy_sixteen_byte_records_backward(_this, entry) {
        // SAFETY: this cdecl helper receives begin, end, and output-end as stack words.
        unsafe {
            let begin = entry.arg(0);
            let mut source = entry.arg(1);
            let mut output = entry.arg(2);
            let count = source.wrapping_sub(begin) as i32 >> 4;
            entry.ecx = source;
            if count <= 0 { return entry.result(output); }
            for _ in 0..count {
                source = source.wrapping_sub(16);
                output = output.wrapping_sub(16);
                for offset in [0, 4, 8, 12] { put_word(output as *mut u8, offset, word(source as *mut u8, offset)); }
            }
            entry.ecx = source;
            u64::from(output)
        }
    }
}
register_port! { fc4p_006e9210, 0x006e9210, "sum_budget_list", 0,
    sum_budget_list(this, entry) {
        // SAFETY: the sentinel list owns its records and each record's indexed budget storage.
        unsafe {
            let sentinel = word(this, 0x44);
            let mut node = word(sentinel as *mut u8, 0);
            let mut sum = 0u64;
            while node != sentinel {
                let record = BudgetRecord::from_ptr(ListNode::from_ptr(node as *mut u8).payload() as *mut u8);
                let index = record.index().wrapping_shl(5);
                let owner = record.owner();
                let data = word(owner, 0x14).wrapping_add(index) as *mut u8;
                node = ListNode::from_ptr(node as *mut u8).next() as u32;
                let value = (u64::from(word(data, 12)) << 32) | u64::from(word(data, 8));
                sum = sum.wrapping_add(value);
            }
            entry.ecx = node;
            sum
        }
    }
}
register_port! { fc4p_00701800, 0x00701800, "sum_budget_categories", 0,
    sum_budget_categories(this, entry) {
        // SAFETY: list nodes contain category IDs consumed by the embedded budget interface.
        unsafe {
            let sentinel = word(this, 0x48);
            let mut node = word(sentinel as *mut u8, 0);
            let mut sum = 0u32;
            let mut edx = entry.edx;
            while node != sentinel {
                let id = ListNode::from_ptr(node as *mut u8).payload();
                let interface = this.add(4);
                let result = virtual_result(interface, 0x30, id, word(interface, 0), &[id]);
                node = ListNode::from_ptr(node as *mut u8).next() as u32;
                sum = sum.wrapping_add(result as u32);
                edx = (result >> 32) as u32;
            }
            (u64::from(edx) << 32) | u64::from(sum)
        }
    }
}
register_port! { fc4p_0080c0f0, 0x0080c0f0, "simulate_landfill_gas_recovery", 0,
    simulate_landfill_gas_recovery(this, entry) {
        // SAFETY: the simulator's income slot returns both words of the stored amount.
        unsafe {
            let result = virtual_result(this, 0x14, word(this, 0), entry.edx, &[]);
            let income = SimulatedIncome::from_ptr(this);
            income.set_low(result as u32);
            income.set_high((result >> 32) as u32);
            (result & !255) | 1
        }
    }
}
register_port! { fc4p_0082abe0, 0x0082abe0, "query_context_format_flag", 0,
    query_context_format_flag(this, entry) {
        // SAFETY: this is the interface twelve bytes after the primary interface.
        unsafe { let primary = this.sub(12); virtual_result(primary, 0x1c, word(primary, 0), entry.edx, &[0x8010]) }
    }
}
register_port! { fc4p_0082af50, 0x0082af50, "enable_context_format_flag", 0,
    enable_context_format_flag(this, entry) {
        // SAFETY: this is the interface twelve bytes after the primary interface.
        unsafe {
            let primary = this.sub(12);
            let result = virtual_result(primary, 0x18, word(primary, 0), entry.edx, &[0x8010]);
            (result & 0xffff_ffff_0000_0000) | u64::from(word(this, 4))
        }
    }
}
register_port! { fc4p_0082b1c0, 0x0082b1c0, "context_rectangle_width", 0,
    context_rectangle_width(this, entry) {
        // SAFETY: the rectangle slot returns a live four-word rectangle.
        unsafe {
            let result = virtual_result(this, 0x30, word(this, 0), entry.edx, &[]);
            let rect = result as u32 as *mut u8;
            entry.ecx = rect as u32;
            (result & 0xffff_ffff_0000_0000) | u64::from(RectangleWords::from_ptr(rect).right().wrapping_sub(RectangleWords::from_ptr(rect).left()))
        }
    }
}
register_port! { fc4p_0082b1d0, 0x0082b1d0, "context_rectangle_height", 0,
    context_rectangle_height(this, entry) {
        // SAFETY: the rectangle slot returns a live four-word rectangle.
        unsafe {
            let result = virtual_result(this, 0x30, word(this, 0), entry.edx, &[]);
            let rect = result as u32 as *mut u8;
            entry.ecx = rect as u32;
            (result & 0xffff_ffff_0000_0000) | u64::from(RectangleWords::from_ptr(rect).bottom().wrapping_sub(RectangleWords::from_ptr(rect).top()))
        }
    }
}
game_fn! { #[allow(dead_code)] fn pixel_format_bits(this: *mut u8, format: u32) -> u64 = "thiscall" 0x00449500; }
register_port! { fc4p_0082b1e0, 0x0082b1e0, "copy_context_pixel_format", 4,
    copy_context_pixel_format(this, entry) {
        // SAFETY: output owns two words; the format helper receives the same output as its this pointer.
        unsafe {
            let format = DrawContextFields::from_ptr(this).format();
            let output = entry.arg(0) as *mut u8;
            put_word(output, 0, format);
            let args = [format];
            let mut c = crate::abi::RawCall::new(crate::abi::Conv::Thiscall, crate::abi::target(0x00449500), output as u32, &args);
            c.eax = format;
            c.edx = entry.edx;
            crate::abi::call_raw(&mut c);
            put_word(output, 4, c.eax);
            (u64::from(c.edx) << 32) | u64::from(output as u32)
        }
    }
}
register_port! { fc4p_0082b1a0, 0x0082b1a0, "release_context_lock", 4,
    release_context_lock(this, entry) {
        // SAFETY: the counter, dirty flag, and refresh interface belong to this context.
        unsafe {
            let context = DrawContextFields::from_ptr(this);
            let count = context.locks().wrapping_sub(1);
            context.set_locks(count);
            let result = if count == 0 {
                let flag = context.dirty();
                if flag != 0 {
                    let target = context.refresh_target();
                    virtual_result(target, 0xa8, word(target, 0), entry.edx, &[])
                } else { entry.result(entry.eax & !255) }
            } else { entry.result(entry.eax) };
            (result & !255) | 1
        }
    }
}
register_port! { fc4p_0082af20, 0x0082af20, "ensure_surface_ready", 0,
    ensure_surface_ready(this, entry) {
        // SAFETY: each callback may replace the surface, so reload its pointer before every call.
        unsafe {
            let surface = SurfaceHolder::from_ptr(this).surface();
            let mut result = virtual_result(surface, 0x40, word(surface, 0), entry.edx, &[]);
            if result as u8 == 0 {
                let surface = SurfaceHolder::from_ptr(this).surface();
                let mode = SurfaceHolder::from_ptr(this).mode();
                result = virtual_result(surface, 0x44, mode, word(surface, 0), &[mode]);
                let surface = SurfaceHolder::from_ptr(this).surface();
                result = virtual_result(surface, 0xa8, result as u32, word(surface, 0), &[]);
            }
            (result & 0xffff_ffff_0000_0000) | u64::from(SurfaceHolder::from_ptr(this).surface() as u32)
        }
    }
}

game_fn! { fn allocate_shared(bytes: u32) -> u64 = "C" 0x0090cf54; }
game_fn! { fn free_shared(pointer: *mut u8) -> u64 = "C" 0x0090cf63; }
game_fn! { fn copy_zone_records(first: *mut u8, last: *mut u8, output: *mut u8, stop: *mut u8) -> u64 = "C" 0x006bb5c0; }
game_fn! { fn repeat_zone_record(output: *mut u8, count: u32, source: *mut u8, stop: *mut u8) -> u64 = "C" 0x006bb120; }
game_fn! { fn budget_balance(this: *mut u8) -> u64 = "thiscall" 0x006e9210; }

register_port! { fc4p_006bbbc0, 0x006bbbc0, "destroy_landfill_zone", 0,
    destroy_landfill_zone(this, entry) {
        // SAFETY: the zone owns a service reference and a contiguous array of 32-byte records.
        unsafe {
            put_word(this, 0, 0x00ab1610);
            let service = word(this, 0x44) as *mut u8;
            let mut result = entry.result(entry.eax);
            if !service.is_null() {
                result = virtual_result(service, 8, word(service, 0), entry.edx, &[]);
            }
            let mut record = word(this, 0x1c);
            let end = word(this, 0x20);
            while record != end {
                let p = record as *mut u8;
                result = virtual_result(p, 0, result as u32, word(p, 0), &[0]);
                record = record.wrapping_add(0x20);
            }
            let allocation = word(this, 0x1c) as *mut u8;
            if !allocation.is_null() { result = free_shared(allocation); }
            result
        }
    }
}

register_port! { fc4p_006bbc10, 0x006bbc10, "grow_zone_record_vector", 20,
    grow_zone_record_vector(this, entry) {
        // SAFETY: the vector's ranges and caller-owned record are live. The last two argument words
        // are also the allocation word and stop byte passed to the range helpers.
        unsafe {
            let position = entry.arg(0) as *mut u8;
            let source = entry.arg(1) as *mut u8;
            let count = entry.arg(3);
            let size = (word(this, 4).wrapping_sub(word(this, 0)) as i32 >> 5) as u32;
            let capacity = size.wrapping_add(size.max(count));
            let allocation = if capacity == 0 { 0 } else { allocate_shared(capacity.wrapping_shl(5)) as u32 };
            let arguments = (entry as *mut Entry).cast::<u32>().add(4);
            arguments.add(3).write_unaligned(allocation);
            let stop = arguments.add(4).cast::<u8>();
            let mut result = copy_zone_records(word(this, 0) as *mut u8, position, allocation as *mut u8, stop);
            let mut output = result as u32;
            if count == 1 {
                if output != 0 {
                    let target = output as *mut u8;
                    put_word(target, 0, 0x00ab15dc);
                    for offset in [4, 8] { put_word(target, offset, word(source, offset)); }
                    *target.add(12) = *source.add(12);
                    *target.add(13) = *source.add(13);
                    for offset in [16, 20, 24, 28] { put_word(target, offset, word(source, offset)); }
                    result = (u64::from(word(source, 28)) << 32) | u64::from(source as u32);
                }
                output = output.wrapping_add(32);
            } else {
                result = repeat_zone_record(output as *mut u8, count, source, stop);
                output = result as u32;
            }
            result = (result & !255) | u64::from(*stop);
            if *stop == 0 {
                result = copy_zone_records(position, word(this, 4) as *mut u8, output as *mut u8, stop);
                output = result as u32;
            }
            let mut old = word(this, 0);
            let end = word(this, 4);
            while old != end {
                let p = old as *mut u8;
                result = virtual_result(p, 0, result as u32, word(p, 0), &[0]);
                old = old.wrapping_add(32);
            }
            let old = word(this, 0) as *mut u8;
            if !old.is_null() { result = free_shared(old); }
            put_word(this, 4, output);
            put_word(this, 0, allocation);
            let end = allocation.wrapping_add(capacity.wrapping_shl(5));
            put_word(this, 8, end);
            (result & 0xffff_ffff_0000_0000) | u64::from(end)
        }
    }
}

register_port! { fc4p_006e08d0, 0x006e08d0, "station_funding", 12,
    station_funding(this, entry) {
        // SAFETY: the budget fills a 32-byte item. Its table is captured before the station callback,
        // but its object pointer is read again afterwards.
        unsafe {
            let department = word(this, 0x74) as *mut u8;
            let table = word(department, 0) as *mut u8;
            let purpose = word(this, 0xb0);
            let station = entry.arg(0) as *mut u8;
            let mut item = core::mem::MaybeUninit::<[u32; 8]>::uninit();
            let item_ptr = item.as_mut_ptr().cast::<u8>();
            put_word(item_ptr, 28, 0);
            let id = virtual_result(station, 0x18, word(station, 0), purpose, &[purpose]);
            let args = [id as u32, item_ptr as u32];
            let mut call = crate::abi::RawCall::new(crate::abi::Conv::Thiscall,
                word(table, 0xcc) as usize, word(this, 0x74), &args);
            call.eax = id as u32;
            call.edx = (id >> 32) as u32;
            crate::abi::call_raw(&mut call);
            if call.eax as u8 == 0 { return (u64::from(call.edx) << 32) | u64::from(call.eax); }
            let funding = entry.arg(2) as *mut u8;
            if !funding.is_null() { put_word(funding, 0, word(item_ptr, 28)); }
            let cost = entry.arg(1) as *mut u8;
            let mut edx = call.edx;
            if !cost.is_null() { edx = word(item_ptr, 16); put_word(cost, 0, edx); }
            (u64::from(edx) << 32) | u64::from((cost as u32 & !255) | 1)
        }
    }
}

register_port! { fc4p_006f1570, 0x006f1570, "insert_word_list_range", 16,
    insert_word_list_range(_this, entry) {
        // SAFETY: each source node holds next, previous, value; new nodes use the game allocator.
        unsafe {
            let position = entry.arg(0) as *mut u8;
            let mut source = entry.arg(1);
            let end = entry.arg(2);
            let mut result = entry.result(entry.eax);
            while source != end {
                let allocation = allocate_shared(12);
                let node = allocation as u32 as *mut u8;
                let value_address = (node as u32).wrapping_add(8);
                let mut edx = (allocation >> 32) as u32;
                if value_address != 0 {
                    edx = word(source as *mut u8, 8);
                    put_word(value_address as *mut u8, 0, edx);
                }
                let previous = word(position, 4) as *mut u8;
                put_word(node, 0, position as u32);
                put_word(node, 4, previous as u32);
                put_word(previous, 0, node as u32);
                put_word(position, 4, node as u32);
                entry.ecx = previous as u32;
                result = (u64::from(edx) << 32) | u64::from(node as u32);
                source = word(source as *mut u8, 0);
            }
            result
        }
    }
}

register_port! { fc4p_00700fa0, 0x00700fa0, "publish_budget_balance", 0,
    publish_budget_balance(this, entry) {
        // SAFETY: the budget's optional sink is retained around the balance update.
        unsafe {
            let service = word(this, 0x30) as *mut u8;
            let mut result = virtual_result(service, 0x50, word(service, 0), entry.edx, &[1]);
            let sink = result as u32 as *mut u8;
            if !sink.is_null() {
                virtual_result(sink, 4, sink as u32, word(sink, 0), &[]);
                let table = word(sink, 0) as *mut u8;
                let balance = budget_balance(this);
                let args = [balance as u32, (balance >> 32) as u32];
                let mut call = crate::abi::RawCall::new(crate::abi::Conv::Thiscall,
                    word(table, 0x30) as usize, sink as u32, &args);
                call.eax = balance as u32;
                call.edx = (balance >> 32) as u32;
                crate::abi::call_raw(&mut call);
                result = virtual_result(sink, 8, word(sink, 0), call.edx, &[]);
            }
            result
        }
    }
}

register_port! { fc4p_0072a1c0, 0x0072a1c0, "snap_point_and_apply", 4,
    snap_point_and_apply(this, entry) {
        // SAFETY: the input supplies x and z; the service may update both stack locals.
        unsafe {
            let point = entry.arg(0) as *mut u8;
            let mut x = word(point, 0);
            let mut z = word(point, 8);
            let arguments = (entry as *mut Entry).cast::<u32>().add(4);
            arguments.write_unaligned(z);
            let service = word(this, 0x20) as *mut u8;
            let mut result = virtual_result(service, 0x188, x, word(service, 0), &[x, z]);
            if result as u8 == 0 { return result & 0xffff_ffff_0000_0000; }
            let service = word(this, 0x20) as *mut u8;
            let z_pointer = arguments;
            vcall!(service, 0x17c => fn(u32, u32, *mut u32, *mut u32) -> u64,
                word(point, 0), word(point, 8), &mut x, z_pointer);
            z = z_pointer.read_unaligned();
            result = virtual_result(this, 0x18, z, word(this, 0), &[x, z, 0]);
            result
        }
    }
}

register_port! { fc4p_0072b410, 0x0072b410, "grid_object_at_origin", 12,
    grid_object_at_origin(this, entry) {
        // SAFETY: valid signed coordinates select one row header and one object slot.
        unsafe {
            let x = entry.arg(0) as i32;
            if x < 0 || x >= word(this, 0x24) as i32 { return entry.result(0); }
            let z = entry.arg(1) as i32;
            if z < 0 || z >= word(this, 0x28) as i32 { return entry.result(0); }
            let rows = word(this, 0x50) as *mut u8;
            let row = word(rows, (x as usize).wrapping_mul(12));
            let object = word(row as *mut u8, (z as usize).wrapping_mul(4));
            if entry.arg(2) as u8 == 0 { return (u64::from(row) << 32) | u64::from(object); }
            if object == 0 { return u64::from(row) << 32; }
            let args = (entry as *mut Entry).cast::<u32>().add(4);
            let result = vcall!(object as *mut u8, 0x54 => fn(*mut u32, *mut u32) -> u64,
                args.add(2), args);
            let matches = args.add(2).read_unaligned() == x as u32 && args.read_unaligned() == z as u32;
            (result & 0xffff_ffff_0000_0000) | u64::from(if matches { object } else { 0 })
        }
    }
}

// These entries transfer the original stack directly to another interface.
interface_thunk!(
    fc4p_00793fe0,
    0x00793fe0,
    "release_button_secondary_interface",
    "sub",
    224,
    target_00793fe0,
    0x00793ec0,
    ()
);

macro_rules! surface_forward {
    ($export:ident, $address:literal, $name:literal, $slot:literal) => {
        port! {
            #[port(address = $address, owner = "shared", name = $name, naked)]
            pub unsafe extern "C" fn $export() {
                core::arch::naked_asm!("mov ecx, [ecx + 0x14]", "mov eax, [ecx]",
                    "jmp dword ptr [eax + {slot}]", slot = const $slot);
            }
        }
    };
}
macro_rules! surface_flag_forward {
    ($export:ident, $address:literal, $name:literal, $slot:literal, $flag:literal) => {
        port! {
            #[port(address = $address, owner = "shared", name = $name, naked)]
            pub unsafe extern "C" fn $export() {
                core::arch::naked_asm!("mov byte ptr [ecx + 0x48], {flag}",
                    "mov ecx, [ecx + 0x14]", "mov eax, [ecx]",
                    "jmp dword ptr [eax + {slot}]", slot = const $slot, flag = const $flag);
            }
        }
    };
}
surface_flag_forward!(fc4p_0082b4b0, 0x0082b4b0, "enable_surface_mode", 0x5c, 1);
surface_forward!(fc4p_0082b540, 0x0082b540, "query_surface_mode", 0x68);
surface_flag_forward!(fc4p_0082b550, 0x0082b550, "disable_surface_mode", 0x6c, 0);
surface_forward!(fc4p_0082b750, 0x0082b750, "forward_surface_operation_94", 0x94);
surface_forward!(fc4p_0082b760, 0x0082b760, "forward_surface_operation_98", 0x98);

register_port! { fc4p_007a65d0, 0x007a65d0, "edge_direction_components", 0,
    edge_direction_components(_this, entry) {
        // SAFETY: the cdecl caller supplies two writable output words, which may alias.
        unsafe {
            let direction = entry.arg(0);
            let distance = entry.arg(1);
            let x = entry.arg(2) as *mut u8;
            let z = entry.arg(3) as *mut u8;
            let (a, b, eax, edx, ecx) = match direction {
                1 => (distance, u32::MAX, x as u32, z as u32, distance),
                2 => (u32::MAX, distance.wrapping_neg(), x as u32, z as u32, distance.wrapping_neg()),
                3 => (distance.wrapping_neg(), 1, distance.wrapping_neg(), z as u32, x as u32),
                _ => (1, distance, x as u32, distance, z as u32),
            };
            put_word(x, 0, a);
            put_word(z, 0, b);
            entry.ecx = ecx;
            (u64::from(edx) << 32) | u64::from(eax)
        }
    }
}

macro_rules! child_value {
    ($export:ident, $address:literal, $name:literal, $slot:literal, $body:ident) => {
        register_port! { $export, $address, $name, 0,
            $body(this, entry) {
                // SAFETY: this interface has its optional child pointer 32 bytes before it.
                unsafe {
                    let child = word(this.wrapping_sub(32), 0) as *mut u8;
                    if child.is_null() { return entry.result(0); }
                    virtual_result(child, $slot, word(child, 0), entry.edx, &[])
                }
            }
        }
    };
}
child_value!(fc4p_007b62f0, 0x007b62f0, "child_value_24", 0x24, child_value_24);
child_value!(fc4p_007b6310, 0x007b6310, "child_value_28", 0x28, child_value_28);

game_fn! { fn set_window_rectangle(this: *mut u8, a: u32, b: u32, c: u32, d: u32) -> u64 = "thiscall" 0x009bc0b8; }
game_fn! { fn update_window_rectangle(this: *mut u8) -> u64 = "thiscall" 0x007c0390; }
port! {
    /// Set the four rectangle words, then refresh the window's dependent geometry.
    #[port(address = 0x007c07d0, owner = "shared", name = "set_and_update_window_rectangle")]
    pub unsafe extern "thiscall" fn fc4p_007c07d0(this: *mut u8, a: u32, b: u32, c: u32, d: u32) -> u64 {
        // SAFETY: forward the original window and complete arguments in order.
        unsafe { set_window_rectangle(this, a, b, c, d); update_window_rectangle(this) }
    }
}

port! {
    /// Transfer the original direction flag to the Rust body, and restore it before returning.
    #[port(address = 0x007d2730, owner = "shared", name = "copy_sixteen_embedded_words", naked)]
    pub unsafe extern "C" fn fc4p_007d2730() {
        core::arch::naked_asm!(
            "pushfd", "cld", "push ecx", "push eax", "push edx", "mov edx, esp",
            "call {body}", "add esp, 8", "pop ecx", "popfd", "ret 4",
            body = sym copy_sixteen_embedded_words,
        );
    }
}
unsafe extern "fastcall" fn copy_sixteen_embedded_words(this: *mut u8, saved: *mut Entry) -> u64 {
    // SAFETY: the entry saved the three registers followed by flags, return address and output pointer.
    // Rust runs with DF clear; the saved direction controls the word transfers.
    unsafe {
        let entry = &mut *saved;
        let step = if entry.return_address & 0x400 == 0 {
            4u32
        } else {
            0xffff_fffc
        };
        let mut source = (this as u32).wrapping_add(0x1ac);
        let mut output = entry.arg(1);
        for _ in 0..16 {
            put_word(output as *mut u8, 0, word(source as *mut u8, 0));
            source = source.wrapping_add(step);
            output = output.wrapping_add(step);
        }
        entry.ecx = 0;
        entry.result(entry.eax)
    }
}

register_port! { fc4p_0081d200, 0x0081d200, "construct_tree_sentinel", 0,
    construct_tree_sentinel(this, _entry) {
        // SAFETY: the game allocator supplies the 56-byte sentinel; only its tag and links are initialized.
        unsafe {
            put_word(this, 0, 0);
            let result = allocate_shared(0x38);
            put_word(this, 0, result as u32);
            put_word(this, 4, 0);
            *(result as u32 as *mut u8) = 0;
            let sentinel = word(this, 0) as *mut u8;
            put_word(sentinel, 4, 0);
            let sentinel = word(this, 0) as *mut u8;
            put_word(sentinel, 8, sentinel as u32);
            let sentinel = word(this, 0) as *mut u8;
            put_word(sentinel, 12, sentinel as u32);
            (result & 0xffff_ffff_0000_0000) | u64::from(this as u32)
        }
    }
}

register_port! { fc4p_00825c90, 0x00825c90, "pixel_format_color_mask", 0,
    pixel_format_color_mask(_this, entry) {
        let result = match entry.eax {
            4 | 6 => 0x7fff,
            5 => 0xffff,
            7 => 0xfff,
            8 | 9 => 0xffffff,
            _ => u32::MAX,
        };
        entry.result(result)
    }
}

register_port! { fc4p_0082abf0, 0x0082abf0, "expand_pixel_color", 4,
    expand_pixel_color(this, entry) {
        // SAFETY: the context's format word selects the packed channel arithmetic.
        unsafe {
            let format = DrawContextFields::from_ptr(this).format();
            let pixel = entry.arg(0);
            let (eax, ecx, edx) = match format {
                4 | 6 => {
                    let channels = (pixel & 0x3e0).wrapping_add((pixel & 0x7c00) << 3);
                    let expanded = ((pixel & 31).wrapping_add(channels << 3)) << 3;
                    let extra = (expanded >> 5) & 0x70707;
                    (expanded.wrapping_add(extra).wrapping_add(0xff000000), channels, extra)
                }
                5 => {
                    let expanded = (((pixel & 0xf800) << 5).wrapping_add(pixel & 31)) << 3;
                    let green = ((pixel & 0x7e0).wrapping_add(0x07f80000)) << 5;
                    let channels = ((pixel >> 1) & 0x300).wrapping_add(expanded).wrapping_add(green);
                    (((expanded >> 5) & 0x70007).wrapping_add(channels), channels, green)
                }
                7 => {
                    let channels = (((((pixel & 0xf00) + 0xf0000) << 4) + (pixel & 0xf0)) << 4) + (pixel & 15);
                    (channels.wrapping_mul(17), channels, pixel & 0xf0)
                }
                8 | 9 => (pixel | 0xff000000, format, entry.edx),
                _ => (0, format, entry.edx),
            };
            entry.ecx = ecx;
            (u64::from(edx) << 32) | u64::from(eax)
        }
    }
}

register_port! { fc4p_0082ad30, 0x0082ad30, "pack_pixel_color", 12,
    pack_pixel_color(this, entry) {
        // SAFETY: only the low byte of each input word participates in packing.
        unsafe {
            let format = DrawContextFields::from_ptr(this).format();
            let red = entry.arg(0) & 255;
            let green = entry.arg(1) & 255;
            let blue = entry.arg(2) & 255;
            let (eax, ecx, edx) = match format {
                4 | 6 => ((blue >> 3) + ((((red & 0xf8) + 0x1e0) << 5) + (green & 0xf8)) * 4, green & 0xf8, blue >> 3),
                5 => ((blue >> 3) + ((((red & 0xf8) + 0xf0) << 5) + (green & 0xfc)) * 8, green & 0xfc, blue >> 3),
                7 => ((((red & 0xf0) + 0xf00) << 4) + (green & 0xf0) + (blue >> 4), green & 0xf0, blue >> 4),
                8 | 9 => ((((red + 0xff00) << 8) + green) << 8 | blue, green, blue),
                _ => (0, format, entry.edx),
            };
            entry.ecx = ecx;
            (u64::from(edx) << 32) | u64::from(eax)
        }
    }
}

game_fn! { fn expand_color(this: *mut u8, pixel: u32) -> u64 = "thiscall" 0x0082abf0; }
register_port! { fc4p_0082ae10, 0x0082ae10, "unpack_pixel_channels", 16,
    unpack_pixel_channels(this, entry) {
        // SAFETY: the three byte outputs may alias; red, green, blue are stored in that order.
        unsafe {
            let result = expand_color(this, entry.arg(0));
            let pixel = result as u32;
            let red = entry.arg(1) as *mut u8;
            let green = entry.arg(2) as *mut u8;
            let blue = entry.arg(3) as *mut u8;
            *red = (pixel >> 16) as u8;
            *green = (pixel >> 8) as u8;
            *blue = pixel as u8;
            entry.ecx = blue as u32;
            (u64::from(green as u32) << 32) | u64::from((pixel & !255) | 1)
        }
    }
}

register_port! { fc4p_0082b200, 0x0082b200, "describe_context_pixel_format", 4,
    describe_context_pixel_format(this, entry) {
        // SAFETY: the output owns two words, and may overlap the format field.
        unsafe {
            let format = DrawContextFields::from_ptr(this).format();
            let output = entry.arg(0) as *mut u8;
            let bits = match format { 2 => 8, 3..=7 => 16, 8 | 9 => 32, _ => 0 };
            put_word(output, 0, format);
            put_word(output, 4, bits);
            entry.ecx = format;
            (u64::from(output as u32) << 32) | u64::from(bits)
        }
    }
}

register_port! { fc4p_008929e0, 0x008929e0, "set_x87_precision", 0,
    set_x87_precision(_this, entry) {
        // SAFETY: the cdecl argument selects precision; all other control bits are retained.
        let mode = unsafe { entry.arg(0) };
        let precision = match mode { 1 => 0x200, 2 => 0x300, _ => 0 };
        crate::x87::set_control_word((crate::x87::control_word() & !0x300) | precision);
        entry.result(entry.eax)
    }
}
register_port! { fc4p_00892a0a, 0x00892a0a, "get_x87_precision", 0,
    get_x87_precision(_this, entry) {
        let mode = match crate::x87::control_word() & 0x300 { 0 => 0, 0x200 => 1, _ => 2 };
        entry.result(mode)
    }
}

game_fn! { fn allocate_word_list_node(value: *mut u8) -> u64 = "stdcall" 0x0094b33a; }
register_port! { fc4p_00897c8d, 0x00897c8d, "insert_word_list_node", 12,
    insert_word_list_node(_this, entry) {
        // SAFETY: the iterator output and position may alias; links are reread after allocation.
        unsafe {
            let result = allocate_word_list_node(entry.arg(2) as *mut u8);
            let node = result as u32 as *mut u8;
            let position = entry.arg(1) as *mut u8;
            let previous = word(position, 4) as *mut u8;
            put_word(node, 0, position as u32);
            put_word(node, 4, previous as u32);
            put_word(previous, 0, node as u32);
            put_word(position, 4, node as u32);
            let output = entry.arg(0) as *mut u8;
            put_word(output, 0, node as u32);
            entry.ecx = output as u32;
            (u64::from(previous as u32) << 32) | u64::from(output as u32)
        }
    }
}

game_fn! { fn construct_embedded_deque(this: *mut u8) -> u64 = "thiscall" 0x0092e6c9; }
port! {
    /// Clear nine words and set the 16-bit capacity before constructing the embedded deque.
    #[port(address = 0x00897fed, owner = "shared", name = "construct_queued_state")]
    pub unsafe extern "thiscall" fn fc4p_00897fed(this: *mut u8) -> u64 {
        // SAFETY: the caller provides the state object and its embedded deque at +0x28.
        unsafe {
            for offset in (0..32).step_by(4) { put_word(this, offset, 0); }
            write_unaligned(this.add(32).cast::<u16>(), 10);
            put_word(this, 36, 0);
            let result = construct_embedded_deque(this.add(40));
            (result & 0xffff_ffff_0000_0000) | u64::from(this as u32)
        }
    }
}

/// This module's replacements.
pub const PORTS: &[Port] = &[
    fc4p_00793fe0::PORT,
    fc4p_007a65d0::PORT,
    fc4p_007b62f0::PORT,
    fc4p_007b6310::PORT,
    fc4p_007c07d0::PORT,
    fc4p_007d2730::PORT,
    fc4p_0081d200::PORT,
    fc4p_00825c90::PORT,
    fc4p_0082abf0::PORT,
    fc4p_0082ad30::PORT,
    fc4p_0082ae10::PORT,
    fc4p_0082b200::PORT,
    fc4p_0082b4b0::PORT,
    fc4p_0082b540::PORT,
    fc4p_0082b550::PORT,
    fc4p_0082b750::PORT,
    fc4p_0082b760::PORT,
    fc4p_008929e0::PORT,
    fc4p_00892a0a::PORT,
    fc4p_00897c8d::PORT,
    fc4p_00897fed::PORT,
    fc4p_006bbbc0::PORT,
    fc4p_006bbc10::PORT,
    fc4p_006e08d0::PORT,
    fc4p_006f1570::PORT,
    fc4p_00700fa0::PORT,
    fc4p_0072a1c0::PORT,
    fc4p_0072b410::PORT,
    fc4p_006ba280::PORT,
    fc4p_006bbd40::PORT,
    fc4p_006bdde0::PORT,
    fc4p_006c2040::PORT,
    fc4p_006c2270::PORT,
    fc4p_006c2290::PORT,
    fc4p_006c4a00::PORT,
    fc4p_006c5fe0::PORT,
    fc4p_006ce230::PORT,
    fc4p_006ce6f0::PORT,
    fc4p_006ce720::PORT,
    fc4p_006d5050::PORT,
    fc4p_006d5140::PORT,
    fc4p_006d66f0::PORT,
    fc4p_006dfc40::PORT,
    fc4p_006dfc50::PORT,
    fc4p_006dfc60::PORT,
    fc4p_006e0130::PORT,
    fc4p_006e0180::PORT,
    fc4p_006e0200::PORT,
    fc4p_006e17a0::PORT,
    fc4p_006e17f0::PORT,
    fc4p_006e1800::PORT,
    fc4p_006e1820::PORT,
    fc4p_006e9210::PORT,
    fc4p_006f7dc0::PORT,
    fc4p_00700e80::PORT,
    fc4p_00700f40::PORT,
    fc4p_00701800::PORT,
    fc4p_00725810::PORT,
    fc4p_007266f0::PORT,
    fc4p_00741740::PORT,
    fc4p_007624d0::PORT,
    fc4p_00762970::PORT,
    fc4p_00762980::PORT,
    fc4p_00762bd0::PORT,
    fc4p_00762ec0::PORT,
    fc4p_007637a0::PORT,
    fc4p_007637b0::PORT,
    fc4p_007637d0::PORT,
    fc4p_007637e0::PORT,
    fc4p_0076aaa0::PORT,
    fc4p_0076b620::PORT,
    fc4p_0076b630::PORT,
    fc4p_0076bb00::PORT,
    fc4p_00798370::PORT,
    fc4p_007b0e40::PORT,
    fc4p_007b0e50::PORT,
    fc4p_007be550::PORT,
    fc4p_007be560::PORT,
    fc4p_007be720::PORT,
    fc4p_007bf5c0::PORT,
    fc4p_007bf5d0::PORT,
    fc4p_007bfa00::PORT,
    fc4p_007c1870::PORT,
    fc4p_007c1910::PORT,
    fc4p_007c2740::PORT,
    fc4p_007c9fb0::PORT,
    fc4p_007c9fd0::PORT,
    fc4p_007e56f0::PORT,
    fc4p_007faf40::PORT,
    fc4p_00800640::PORT,
    fc4p_00800c30::PORT,
    fc4p_00800c40::PORT,
    fc4p_00808600::PORT,
    fc4p_00808610::PORT,
    fc4p_008089a0::PORT,
    fc4p_008089b0::PORT,
    fc4p_00809240::PORT,
    fc4p_00809250::PORT,
    fc4p_0080ae60::PORT,
    fc4p_0080ae70::PORT,
    fc4p_0080c0f0::PORT,
    fc4p_0080c350::PORT,
    fc4p_0081f910::PORT,
    fc4p_0082abe0::PORT,
    fc4p_0082ace0::PORT,
    fc4p_0082ad00::PORT,
    fc4p_0082ad20::PORT,
    fc4p_0082ae90::PORT,
    fc4p_0082af20::PORT,
    fc4p_0082af50::PORT,
    fc4p_0082b1a0::PORT,
    fc4p_0082b1c0::PORT,
    fc4p_0082b1d0::PORT,
    fc4p_0082b1e0::PORT,
    fc4p_0082be80::PORT,
    fc4p_00875aba::PORT,
    fc4p_00891c72::PORT,
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::abi::{call_raw, Conv, RawCall};
    use crate::testing::{fake, GameFpu, Object};
    use crate::x87::F80;
    use std::cell::RefCell;
    const EAX: u32 = 0xa17b_39e2;
    const EDX: u32 = 0xd634_58a9;
    const RESULT: u64 = 0x9abc_def0_1234_5678;
    // SAFETY: every test passes the entry's original argument count and live memory.
    fn invoke(function: usize, this: *mut u8, args: &[u32]) -> (u32, u32) {
        let mut c = RawCall::new(Conv::Thiscall, function, this as u32, args);
        c.eax = EAX;
        c.edx = EDX;
        unsafe {
            call_raw(&mut c);
        }
        (c.eax, c.edx)
    }
    fn memory(p: *mut u8, len: usize) -> Vec<u8> {
        // SAFETY: tests request only their allocated object's extent.
        unsafe { std::slice::from_raw_parts(p, len).to_vec() }
    }
    fn fill(p: *mut u8, len: usize) {
        // SAFETY: tests request only their allocated object's extent.
        unsafe {
            core::ptr::write_bytes(p, 0xa5, len);
        }
    }
    fn expected_word(memory: &mut [u8], offset: usize, value: u32) {
        memory[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    thread_local! { static CALLS: RefCell<Vec<(u32, Vec<u32>)>> = const { RefCell::new(Vec::new()) }; }
    fn record(this: *mut u8, args: &[u32]) {
        CALLS.with(|c| c.borrow_mut().push((this as u32, args.to_vec())));
    }
    fn calls() -> Vec<(u32, Vec<u32>)> {
        CALLS.with(|c| core::mem::take(&mut *c.borrow_mut()))
    }
    extern "thiscall" fn fake_zero(this: *mut u8) -> u64 {
        record(this, &[]);
        RESULT
    }
    extern "thiscall" fn fake_one(this: *mut u8, a: u32) -> u64 {
        record(this, &[a]);
        RESULT
    }
    extern "thiscall" fn fake_two(this: *mut u8, a: u32, b: u32) -> u64 {
        record(this, &[a, b]);
        RESULT
    }
    extern "C" fn fake_free(this: *mut u8) -> u64 {
        // SAFETY: the deletion tests pass a live first word.
        record(this, &[0xf0, unsafe { word(this, 0) }]);
        0xfedc_ba98_7654_3210
    }
    macro_rules! getter_test {
        ($name:ident, $export:ident, $offset:literal) => {
            #[test]
            fn $name() {
                let mut object = Object::new(0x180);
                let p = object.ptr();
                fill(p, 0x180);
                for value in [0u32, 1, 0x8000_0000, u32::MAX, 0x1234_5678] {
                    // SAFETY: the test allocation covers the word field.
                    unsafe {
                        put_word(p, $offset, value);
                    }
                    let before = memory(p, 0x180);
                    assert_eq!(invoke($export as *const () as usize, p, &[]), (value, EDX));
                    assert_eq!(memory(p, 0x180), before);
                }
            }
        };
    }
    getter_test!(advice_event_id, fc4p_006e0130, 0xa8);
    getter_test!(popup_tracker_value, fc4p_007624d0, 0xec);
    getter_test!(graph_hit_test_data, fc4p_0076b620, 0x148);
    getter_test!(city_prop_manager, fc4p_00798370, 0x130);
    getter_test!(render_lighting_manager, fc4p_007c1910, 0x118);
    getter_test!(render_view_utilities, fc4p_007c9fb0, 0x114);
    getter_test!(render_camera_control, fc4p_007c9fd0, 0x8c);
    getter_test!(city_demolition_utility, fc4p_007e56f0, 0x120);
    getter_test!(texture_binding_resource_group, fc4p_00809240, 0x70);
    getter_test!(buffer_cache_last_access, fc4p_00809250, 0x64);
    getter_test!(socket_notification_target, fc4p_0081f910, 0x50);

    #[test]
    fn address_and_constant_getters() {
        for input in [0u32, 1, 0x1000, u32::MAX] {
            assert_eq!(
                invoke(fc4p_0076b630 as *const () as usize, input as *mut u8, &[]),
                (input.wrapping_add(-216i32 as u32), EDX)
            );
        }
        for input in [0u32, 1, 0x1000, u32::MAX] {
            assert_eq!(
                invoke(fc4p_00800640 as *const () as usize, input as *mut u8, &[]),
                (input.wrapping_add(48i32 as u32), EDX)
            );
        }
        assert_eq!(
            invoke(fc4p_006d5050 as *const () as usize, core::ptr::null_mut(), &[]),
            (0xaa3fe6bc, EDX)
        );
        assert_eq!(
            invoke(fc4p_00700e80 as *const () as usize, core::ptr::null_mut(), &[]),
            (0xea230a28, EDX)
        );
    }
    macro_rules! thunk_test {
        ($name:ident, $export:ident, $adjust:literal, $target:literal, $fake:ident, [$($arg:literal),*]) => {
            #[test]
            fn $name() {
                let _fake = fake($target, $fake as *const () as usize);
                let mut object = Object::new(0x300);
                let p = object.ptr();
                fill(p, 0x300);
                let before = memory(p, 0x300);
                let this = p.wrapping_add(0x100);
                assert_eq!(invoke($export as *const () as usize, this, &[$($arg),*]), (RESULT as u32, (RESULT >> 32) as u32));
                assert_eq!(calls(), vec![((this as u32).wrapping_add($adjust as i32 as u32), vec![$($arg),*])]);
                assert_eq!(memory(p, 0x300), before);
            }
        };
    }
    thunk_test!(adjust_this_006bdde0, fc4p_006bdde0, -8, 0x006bd8a0, fake_zero, []);
    thunk_test!(adjust_this_006c2270, fc4p_006c2270, -4, 0x004664a0, fake_zero, []);
    thunk_test!(adjust_this_006c2290, fc4p_006c2290, -8, 0x004664a0, fake_zero, []);
    thunk_test!(adjust_this_006ce6f0, fc4p_006ce6f0, -4, 0x004f0240, fake_zero, []);
    thunk_test!(adjust_this_006ce720, fc4p_006ce720, -8, 0x004f0240, fake_zero, []);
    thunk_test!(
        adjust_this_006dfc40,
        fc4p_006dfc40,
        -8,
        0x006ac550,
        fake_two,
        [0x1357_2468, 0xfedc_ba98]
    );
    thunk_test!(adjust_this_006dfc50, fc4p_006dfc50, -4, 0x006dfaf0, fake_zero, []);
    thunk_test!(adjust_this_006dfc60, fc4p_006dfc60, -8, 0x006dfaf0, fake_zero, []);
    thunk_test!(adjust_this_006e17a0, fc4p_006e17a0, -8, 0x0082be80, fake_zero, []);
    thunk_test!(adjust_this_006e17f0, fc4p_006e17f0, -16, 0x0082be80, fake_zero, []);
    thunk_test!(adjust_this_006e1800, fc4p_006e1800, -8, 0x006ad1b0, fake_zero, []);
    thunk_test!(adjust_this_006e1820, fc4p_006e1820, -16, 0x006ad1b0, fake_zero, []);
    thunk_test!(adjust_this_00762970, fc4p_00762970, -4, 0x00762470, fake_zero, []);
    thunk_test!(adjust_this_00762980, fc4p_00762980, -4, 0x00762480, fake_zero, []);
    thunk_test!(adjust_this_00762ec0, fc4p_00762ec0, -4, 0x00762bd0, fake_zero, []);
    thunk_test!(adjust_this_007637a0, fc4p_007637a0, -8, 0x0057eb00, fake_zero, []);
    thunk_test!(adjust_this_007637b0, fc4p_007637b0, -8, 0x0057eb70, fake_zero, []);
    thunk_test!(adjust_this_007637d0, fc4p_007637d0, -12, 0x0057eb00, fake_zero, []);
    thunk_test!(adjust_this_007637e0, fc4p_007637e0, -12, 0x0057eb70, fake_zero, []);
    thunk_test!(adjust_this_007b0e40, fc4p_007b0e40, -220, 0x007be160, fake_zero, []);
    thunk_test!(adjust_this_007b0e50, fc4p_007b0e50, -220, 0x007be170, fake_zero, []);
    thunk_test!(adjust_this_007be550, fc4p_007be550, -216, 0x007be160, fake_zero, []);
    thunk_test!(adjust_this_007be560, fc4p_007be560, -216, 0x007be170, fake_zero, []);
    thunk_test!(adjust_this_007be720, fc4p_007be720, -216, 0x008fb72e, fake_zero, []);
    thunk_test!(adjust_this_007bf5c0, fc4p_007bf5c0, -220, 0x00762470, fake_zero, []);
    thunk_test!(adjust_this_007bf5d0, fc4p_007bf5d0, -220, 0x00762480, fake_zero, []);
    thunk_test!(adjust_this_00800c30, fc4p_00800c30, 4, 0x009297a0, fake_zero, []);
    thunk_test!(adjust_this_00800c40, fc4p_00800c40, 4, 0x0091dd89, fake_zero, []);
    thunk_test!(adjust_this_00808600, fc4p_00808600, 4, 0x009753b7, fake_zero, []);
    thunk_test!(adjust_this_00808610, fc4p_00808610, 4, 0x009753c5, fake_zero, []);
    thunk_test!(
        adjust_this_008089a0,
        fc4p_008089a0,
        -4,
        0x008085c0,
        fake_two,
        [0x1357_2468, 0xfedc_ba98]
    );
    thunk_test!(
        adjust_this_008089b0,
        fc4p_008089b0,
        -4,
        0x00808a00,
        fake_one,
        [0x1357_2468]
    );
    thunk_test!(
        adjust_this_0082ae90,
        fc4p_0082ae90,
        -4,
        0x0082b7d0,
        fake_one,
        [0x1357_2468]
    );
    thunk_test!(adjust_this_00875aba, fc4p_00875aba, -4, 0x00945942, fake_zero, []);

    #[test]
    fn float_byte_and_noop_getters() {
        let _fpu = GameFpu::new();
        let mut object = Object::new(0x100);
        let p = object.ptr();
        for bits in [0u32, 0x8000_0000, 0x3f80_0000, 0x7f80_0000, 0x7fc1_2345, 0x7f81_2345, 1] {
            // SAFETY: the field is present and the port returns one ST0 value.
            unsafe {
                put_word(p, 0x98, bits);
                let mut c = RawCall::new(Conv::Thiscall, fc4p_006e0200 as *const () as usize, p as u32, &[]);
                c.eax = EAX;
                c.edx = EDX;
                c.take_st0 = 1;
                let cw = crate::x87::control_word();
                call_raw(&mut c);
                assert_eq!(c.st0, F80::from_f32(f32::from_bits(bits)));
                assert_eq!((c.eax, c.edx), (EAX, EDX));
                assert_eq!(crate::x87::control_word(), cw);
                assert_eq!(word(p, 0x98), bits);
            }
        }
        for flag in [0u8, 1, 0x80, 0xff] {
            // SAFETY: the byte lies inside the allocation.
            unsafe {
                *p.add(0xdd) = flag;
            }
            assert_eq!(
                invoke(fc4p_00741740 as *const () as usize, p, &[]),
                ((EAX & !255) | u32::from(flag), EDX)
            );
        }
        assert_eq!(
            invoke(fc4p_006f7dc0 as *const () as usize, p, &[0x1234, u32::MAX]),
            (EAX, EDX)
        );
        for input in [0u32, 1, 0x1000, 0xffff_fffc] {
            assert_eq!(
                invoke(fc4p_006c2040 as *const () as usize, input as *mut u8, &[]).0,
                if input == 0 { 0 } else { input.wrapping_add(8) }
            );
        }
    }
    #[test]
    fn deletion_flags_calls_returns_and_memory() {
        let _free = fake(0x005e5620, fake_free as *const () as usize);
        let _zone = fake(0x006bbbc0, fake_zero as *const () as usize);
        let _filter = fake(0x005e95c0, fake_zero as *const () as usize);
        for (f, destroy) in [
            (fc4p_006ba280 as *const () as usize, false),
            (fc4p_006bbd40 as *const () as usize, true),
            (fc4p_006c4a00 as *const () as usize, true),
        ] {
            for flags in [0u32, 1, 2, 3, 0x100, 0x8000_0001, u32::MAX] {
                let mut object = Object::new(0x50);
                let p = object.ptr();
                fill(p, 0x50);
                let mut expected = memory(p, 0x50);
                if !destroy {
                    expected_word(&mut expected, 0, 0x00ab15dc);
                }
                let hi = if flags & 1 != 0 {
                    0xfedc_ba98
                } else if destroy {
                    (RESULT >> 32) as u32
                } else {
                    EDX
                };
                assert_eq!(invoke(f, p, &[flags]), (p as u32, hi));
                let mut expected_calls = Vec::new();
                if destroy {
                    expected_calls.push((p as u32, vec![]));
                }
                if flags & 1 != 0 {
                    expected_calls.push((p as u32, vec![0xf0, if destroy { 0xa5a5_a5a5 } else { 0x00ab15dc }]));
                }
                assert_eq!(calls(), expected_calls);
                assert_eq!(memory(p, 0x50), expected);
            }
        }
    }
    #[test]
    fn copy_words_overlap_in_order() {
        for (f, source, edx_is_output) in [
            (fc4p_006c5fe0 as *const () as usize, 0, true),
            (fc4p_0082ad00 as *const () as usize, 0x18, false),
        ] {
            for delta in [0usize, 4, 8, 12, 16, 32] {
                let mut object = Object::new(0x100);
                let p = object.ptr();
                // SAFETY: initialize all words of the test allocation.
                unsafe {
                    for i in (0..0x100).step_by(4) {
                        put_word(p, i, 0x1234_0000 | i as u32);
                    }
                }
                let out_offset = source + delta;
                let output = p.wrapping_add(out_offset);
                let mut expected = memory(p, 0x100);
                let mut third = 0;
                for i in [0, 4, 8, 12] {
                    let value = u32::from_le_bytes(expected[source + i..source + i + 4].try_into().unwrap());
                    if i == 8 {
                        third = value;
                    }
                    expected_word(&mut expected, out_offset + i, value);
                }
                assert_eq!(
                    invoke(f, p, &[output as u32]),
                    (output as u32, if edx_is_output { output as u32 } else { third })
                );
                assert_eq!(memory(p, 0x100), expected);
            }
        }
    }
    #[test]
    fn setters_touch_only_their_fields() {
        let mut object = Object::new(0x140);
        let p = object.ptr();
        fill(p, 0x140);
        for value in [0u32, 1, 0x80, 0x8000_0000, u32::MAX] {
            let mut expected = memory(p, 0x140);
            expected_word(&mut expected, 0x3c, value);
            assert_eq!(invoke(fc4p_006ce230 as *const () as usize, p, &[value]), (value, EDX));
            assert_eq!(memory(p, 0x140), expected);
            for (f, offset) in [
                (fc4p_007bfa00 as *const () as usize, 0x102),
                (fc4p_007c2740 as *const () as usize, 0x62),
            ] {
                let mut expected = memory(p, 0x140);
                expected[offset] = value as u8;
                assert_eq!(invoke(f, p, &[value]), ((EAX & !255) | (value & 255), EDX));
                assert_eq!(memory(p, 0x140), expected);
            }
        }
        for (f, offsets, eax) in [
            (fc4p_0076aaa0 as *const () as usize, &[8][..], (EAX & !255) | 1),
            (fc4p_0076bb00 as *const () as usize, &[0, 4, 8][..], 0),
        ] {
            fill(p, 0x140);
            let mut expected = memory(p, 0x140);
            for &offset in offsets {
                expected_word(&mut expected, offset, 0);
            }
            assert_eq!(invoke(f, p, &[]), (eax, EDX));
            assert_eq!(memory(p, 0x140), expected);
        }
        for (f, eax, check_edx) in [
            (fc4p_0080ae60 as *const () as usize, p as u32, false),
            (fc4p_0080ae70 as *const () as usize, EAX, true),
        ] {
            fill(p, 0x140);
            let mut expected = memory(p, 0x140);
            expected_word(&mut expected, 0, 0x00abd92c);
            let result = invoke(f, p, &[]);
            assert_eq!(result.0, eax);
            if check_edx {
                assert_eq!(result.1, EDX);
            }
            assert_eq!(memory(p, 0x140), expected);
        }
    }
    #[test]
    fn copy_wrappers_use_original_addresses_and_interfaces() {
        let _list = fake(0x006dfc70, fake_two as *const () as usize);
        let _budget = fake(0x006e9900, fake_one as *const () as usize);
        let mut object = Object::new(0xc0);
        let p = object.ptr();
        assert_eq!(
            invoke(fc4p_006e0180 as *const () as usize, p, &[0x1234]),
            (RESULT as u32, (RESULT >> 32) as u32)
        );
        assert_eq!(calls(), vec![(p as u32 + 8, vec![0x1234, p as u32 + 0x88])]);
        assert_eq!(invoke(fc4p_00700f40 as *const () as usize, p, &[0x5678]).0, 0x5678);
        assert_eq!(calls(), vec![(p as u32 + 4, vec![0x5678])]);
    }
    #[test]
    fn rectangle_signed_bounds_and_register_paths() {
        let mut object = Object::new(16);
        let p = object.ptr();
        for r in [
            [0i32, 0, 1, 1],
            [0, 0, 0, 1],
            [0, 0, 1, 0],
            [2, 3, 1, 4],
            [-2, -3, -1, -2],
            [i32::MIN, i32::MAX, i32::MAX, i32::MIN],
        ] {
            // SAFETY: the rectangle is exactly four words.
            unsafe {
                for (i, v) in r.into_iter().enumerate() {
                    put_word(p, i * 4, v as u32);
                }
            }
            let before = memory(p, 16);
            assert_eq!(
                invoke(fc4p_007c1870 as *const () as usize, p, &[]),
                (
                    u32::from(r[2] <= r[0] || r[3] <= r[1]),
                    if r[2] <= r[0] { EDX } else { r[3] as u32 }
                )
            );
            assert_eq!(memory(p, 16), before);
        }
    }
    #[test]
    fn virtual_base_null_and_wrapping_displacements() {
        assert_eq!(
            invoke(fc4p_007faf40 as *const () as usize, core::ptr::null_mut(), &[]),
            (0, EDX)
        );
        let mut object = Object::new(16);
        let p = object.ptr();
        let mut table = [0u32; 2];
        // SAFETY: object and its displacement table remain live throughout the test.
        unsafe {
            put_word(p, 4, table.as_mut_ptr() as u32);
        }
        for displacement in [0u32, 8, 0xffff_fff8, 0x7fff_ffff] {
            table[1] = displacement;
            assert_eq!(
                invoke(fc4p_007faf40 as *const () as usize, p, &[]),
                ((p as u32).wrapping_add(displacement).wrapping_add(4), displacement)
            );
        }
    }
    #[test]
    fn income_lock_count_stride_and_reference_boundaries() {
        let mut object = Object::new(0xd0);
        let p = object.ptr();
        fill(p, 0xd0);
        for value in [0u64, 1, 0x8000_0000_0000_0000, u64::MAX, 0x1234_abcd_9876_fedc] {
            // SAFETY: both words of the income field are present.
            unsafe {
                put_word(p, 0x80, value as u32);
                put_word(p, 0x84, (value >> 32) as u32);
            }
            let before = memory(p, 0xd0);
            assert_eq!(
                invoke(fc4p_0080c350 as *const () as usize, p, &[]),
                (value as u32, (value >> 32) as u32)
            );
            assert_eq!(memory(p, 0xd0), before);
        }
        for value in [0u32, 1, 0x7fff_ffff, 0x8000_0000, u32::MAX] {
            // SAFETY: every requested field is in the test allocation.
            unsafe {
                put_word(p, 0x28, value);
                put_word(p, 0x44, value);
                put_word(p, 0xc4, value);
            }
            assert_eq!(
                invoke(fc4p_0082ace0 as *const () as usize, p, &[]),
                (u32::from(value != 0), value)
            );
            assert_eq!(
                invoke(fc4p_0082ad20 as *const () as usize, p, &[]),
                (value.wrapping_shl(3), EDX)
            );
            let mut expected = memory(p, 0xd0);
            expected_word(&mut expected, 0xc4, value.wrapping_add(1));
            assert_eq!(
                invoke(fc4p_0082be80 as *const () as usize, p, &[]).0,
                value.wrapping_add(1)
            );
            assert_eq!(memory(p, 0xd0), expected);
        }
    }
    fn invoke_c(function: usize, args: &[u32]) -> (u32, u32) {
        let mut c = RawCall::new(Conv::Cdecl, function, 0, args);
        c.eax = EAX;
        c.edx = EDX;
        // SAFETY: the record-copy helpers take exactly three cdecl words.
        unsafe {
            call_raw(&mut c);
        }
        (c.eax, c.edx)
    }
    #[test]
    fn packed_record_copies_empty_null_and_overlap() {
        for (f, size) in [
            (fc4p_006d66f0 as *const () as usize, 10usize),
            (fc4p_007266f0 as *const () as usize, 16usize),
        ] {
            for count in [0usize, 1, 3] {
                for delta in [0usize, 4, 40] {
                    let mut object = Object::new(0x100);
                    let p = object.ptr();
                    // SAFETY: initialize the complete byte allocation.
                    unsafe {
                        for i in 0..0x100 {
                            *p.add(i) = i as u8;
                        }
                    }
                    let begin = p.wrapping_add(16);
                    let end = begin.wrapping_add(count * size);
                    let output = begin.wrapping_add(delta);
                    let mut expected = memory(p, 0x100);
                    for item in 0..count {
                        for offset in (0..size).step_by(4) {
                            let width = if offset + 4 > size { 2 } else { 4 };
                            let source = 16 + item * size + offset;
                            let target = 16 + delta + item * size + offset;
                            for i in 0..width {
                                let v = expected[source + i];
                                expected[target + i] = v;
                            }
                        }
                    }
                    assert_eq!(
                        invoke_c(f, &[begin as u32, end as u32, output as u32]),
                        ((output as u32).wrapping_add((count * size) as u32), end as u32)
                    );
                    assert_eq!(memory(p, 0x100), expected);
                }
            }
            assert_eq!(
                invoke_c(f, &[0x1000, 0x1000 + size as u32, 0]),
                (size as u32, 0x1000 + size as u32)
            );
            assert_eq!(invoke_c(f, &[0, 0, 0]), (0, 0));
        }
    }
    #[test]
    fn backward_record_copy_signed_count_partial_and_overlap() {
        let f = fc4p_00725810 as *const () as usize;
        for len in [0usize, 1, 15, 16, 17, 32, 33] {
            for delta in [0usize, 4, 16] {
                let mut object = Object::new(0x100);
                let p = object.ptr();
                // SAFETY: initialize the complete byte allocation.
                unsafe {
                    for i in 0..0x100 {
                        *p.add(i) = i as u8;
                    }
                }
                let count = len / 16;
                let mut source = 32 + len;
                let mut output = source + delta;
                let mut expected = memory(p, 0x100);
                for _ in 0..count {
                    source -= 16;
                    output -= 16;
                    for offset in [0usize, 4, 8, 12] {
                        let bytes: [u8; 4] = expected[source + offset..source + offset + 4].try_into().unwrap();
                        expected[output + offset..output + offset + 4].copy_from_slice(&bytes);
                    }
                }
                assert_eq!(
                    invoke_c(
                        f,
                        &[
                            p.wrapping_add(32) as u32,
                            p.wrapping_add(32 + len) as u32,
                            p.wrapping_add(32 + len + delta) as u32
                        ]
                    ),
                    (p.wrapping_add(output) as u32, if count == 0 { EDX } else { 0 })
                );
                assert_eq!(memory(p, 0x100), expected);
            }
        }
        assert_eq!(invoke_c(f, &[0x1020, 0x1000, 0x2000]), (0x2000, EDX));
    }
    thread_local! { static NEXT_TABLE: std::cell::Cell<u32> = const { std::cell::Cell::new(0) }; }
    extern "thiscall" fn cleanup_swap(this: *mut u8) -> u64 {
        record(this, &[0xc1]);
        // SAFETY: the test supplies a live first word and replacement vtable.
        unsafe {
            put_word(this, 0, NEXT_TABLE.with(|v| v.get()));
        }
        RESULT
    }
    #[test]
    fn release_methods_signed_unsigned_counts_and_fresh_vtables() {
        for (f, offset, signed, slot) in [
            (fc4p_006d5140 as *const () as usize, 0x10, true, 0x14),
            (fc4p_00762bd0 as *const () as usize, 8, false, 0x10),
            (fc4p_00891c72 as *const () as usize, 4, false, 0x38),
        ] {
            for count in [0u32, 1, 2, 0x7fff_ffff, 0x8000_0000, u32::MAX] {
                let mut table = [0usize; 16];
                let mut next = [0usize; 16];
                table[slot / 4] = fake_one as *const () as usize;
                next[slot / 4] = fake_one as *const () as usize;
                if signed {
                    table[0x10 / 4] = cleanup_swap as *const () as usize;
                }
                NEXT_TABLE.with(|v| v.set(next.as_ptr() as u32));
                let mut object = Object::new(0x40);
                let p = object.ptr();
                fill(p, 0x40);
                // SAFETY: the object's vtable and count fields are present.
                unsafe {
                    put_word(p, 0, table.as_ptr() as u32);
                    put_word(p, offset, count);
                }
                let decrement = if signed { count as i32 > 1 } else { count > 1 };
                let mut expected = memory(p, 0x40);
                if decrement {
                    expected_word(&mut expected, offset, count - 1);
                } else if signed {
                    expected_word(&mut expected, 0, next.as_ptr() as u32);
                }
                assert_eq!(
                    invoke(f, p, &[]),
                    if decrement {
                        (count - 1, EDX)
                    } else {
                        (0, (RESULT >> 32) as u32)
                    }
                );
                let expected_calls = if decrement {
                    vec![]
                } else if signed {
                    vec![(p as u32, vec![0xc1]), (p as u32, vec![1])]
                } else {
                    vec![(p as u32, vec![1])]
                };
                assert_eq!(calls(), expected_calls);
                assert_eq!(memory(p, 0x40), expected);
            }
        }
    }
    #[test]
    fn budget_list_sum_empty_carry_and_wrap() {
        let mut object = Object::new(0x50);
        let p = object.ptr();
        let mut sentinel = [0u32; 3];
        let h = sentinel.as_mut_ptr() as u32;
        // SAFETY: the object points to its live sentinel.
        unsafe {
            put_word(p, 0x44, h);
        }
        sentinel[0] = h;
        assert_eq!(invoke(fc4p_006e9210 as *const () as usize, p, &[]), (0, 0));
        let mut nodes = [[0u32; 3]; 3];
        let mut records = [[0u32; 3]; 3];
        let mut owner = [0u32; 6];
        let mut data = [0u32; 24];
        owner[5] = data.as_mut_ptr() as u32;
        for i in 0..3 {
            nodes[i][0] = if i == 2 { h } else { nodes[i + 1].as_ptr() as u32 };
            nodes[i][2] = records[i].as_ptr() as u32;
            records[i][1] = i as u32;
            records[i][2] = owner.as_ptr() as u32;
        }
        // SAFETY: link the live sentinel to the first node.
        unsafe {
            put_word(sentinel.as_mut_ptr().cast(), 0, nodes[0].as_ptr() as u32);
        }
        for values in [
            [0u64, 0, 0],
            [u32::MAX as u64, 1, 0],
            [u64::MAX, 1, 5],
            [0x8000_0000_0000_0000, 0x8000_0000_0000_0000, 0x1234_5678_90ab_cdef],
        ] {
            let mut sum = 0u64;
            for (i, value) in values.into_iter().enumerate() {
                data[i * 8 + 2] = value as u32;
                data[i * 8 + 3] = (value >> 32) as u32;
                sum = sum.wrapping_add(value);
            }
            let before = memory(p, 0x50);
            assert_eq!(
                invoke(fc4p_006e9210 as *const () as usize, p, &[]),
                (sum as u32, (sum >> 32) as u32)
            );
            assert_eq!(memory(p, 0x50), before);
        }
    }
    extern "thiscall" fn category_value(this: *mut u8, id: u32) -> u64 {
        record(this, &[id]);
        (u64::from(id ^ 0xabcdef00) << 32) | u64::from(id)
    }
    #[test]
    fn category_sum_wraps_and_retains_last_edx() {
        let mut object = Object::new(0x60);
        let p = object.ptr();
        let mut table = [0usize; 16];
        table[0x30 / 4] = category_value as *const () as usize;
        let mut sentinel = [0u32; 3];
        let h = sentinel.as_mut_ptr() as u32;
        sentinel[0] = h;
        // SAFETY: the embedded interface and sentinel pointer occupy their original offsets.
        unsafe {
            put_word(p, 4, table.as_ptr() as u32);
            put_word(p, 0x48, h);
        }
        assert_eq!(invoke(fc4p_00701800 as *const () as usize, p, &[]), (0, EDX));
        assert!(calls().is_empty());
        let mut nodes = [[0u32; 3]; 3];
        let values = [u32::MAX, 4, 0x8000_0000];
        let mut sum = 0u32;
        for i in 0..3 {
            nodes[i][0] = if i == 2 { h } else { nodes[i + 1].as_ptr() as u32 };
            nodes[i][2] = values[i];
            sum = sum.wrapping_add(values[i]);
        }
        // SAFETY: link the live sentinel to the first node.
        unsafe {
            put_word(sentinel.as_mut_ptr().cast(), 0, nodes[0].as_ptr() as u32);
        }
        assert_eq!(
            invoke(fc4p_00701800 as *const () as usize, p, &[]),
            (sum, values[2] ^ 0xabcdef00)
        );
        assert_eq!(calls(), values.map(|id| (p as u32 + 4, vec![id])).to_vec());
    }
    #[test]
    fn landfill_gas_income_writes_both_words_and_retains_return_bytes() {
        let mut object = Object::new(0xb0);
        let p = object.ptr();
        fill(p, 0xb0);
        let mut table = [0usize; 8];
        table[0x14 / 4] = fake_zero as *const () as usize;
        // SAFETY: the first word points to the income callback's vtable.
        unsafe {
            put_word(p, 0, table.as_ptr() as u32);
        }
        let mut expected = memory(p, 0xb0);
        expected_word(&mut expected, 0x98, RESULT as u32);
        expected_word(&mut expected, 0x9c, (RESULT >> 32) as u32);
        assert_eq!(
            invoke(fc4p_0080c0f0 as *const () as usize, p, &[]),
            ((RESULT as u32 & !255) | 1, (RESULT >> 32) as u32)
        );
        assert_eq!(calls(), vec![(p as u32, vec![])]);
        assert_eq!(memory(p, 0xb0), expected);
    }
    extern "thiscall" fn set_format_flag(this: *mut u8, flag: u32) -> u64 {
        record(this, &[flag]);
        // SAFETY: tests pass a complete primary object; this callback replaces its field.
        unsafe {
            put_word(this, 16, 0x2468_1357);
        }
        RESULT
    }
    #[test]
    fn context_flag_calls_use_primary_interface_and_reload_return_field() {
        let mut object = Object::new(0x60);
        let p = object.ptr();
        let mut table = [0usize; 16];
        table[0x1c / 4] = fake_one as *const () as usize;
        table[0x18 / 4] = set_format_flag as *const () as usize;
        // SAFETY: primary interface and its secondary field are present.
        unsafe {
            put_word(p, 0, table.as_ptr() as u32);
            put_word(p, 16, 0x1111);
        }
        let secondary = p.wrapping_add(12);
        assert_eq!(
            invoke(fc4p_0082abe0 as *const () as usize, secondary, &[]),
            (RESULT as u32, (RESULT >> 32) as u32)
        );
        assert_eq!(calls(), vec![(p as u32, vec![0x8010])]);
        assert_eq!(
            invoke(fc4p_0082af50 as *const () as usize, secondary, &[]),
            (0x2468_1357, (RESULT >> 32) as u32)
        );
        assert_eq!(calls(), vec![(p as u32, vec![0x8010])]);
    }
    thread_local! { static RECTANGLE: std::cell::Cell<u32> = const { std::cell::Cell::new(0) }; }
    extern "thiscall" fn rectangle_pointer(this: *mut u8) -> u64 {
        record(this, &[]);
        (RESULT & 0xffff_ffff_0000_0000) | u64::from(RECTANGLE.with(|v| v.get()))
    }
    #[test]
    fn context_dimensions_use_wrapping_differences() {
        let mut object = Object::new(16);
        let p = object.ptr();
        let mut table = [0usize; 16];
        table[0x30 / 4] = rectangle_pointer as *const () as usize;
        let mut rect = [0u32; 4];
        // SAFETY: the object points to its live vtable.
        unsafe {
            put_word(p, 0, table.as_ptr() as u32);
        }
        RECTANGLE.with(|v| v.set(rect.as_mut_ptr() as u32));
        for values in [
            [0u32, 0, 10, 20],
            [10, 20, 0, 0],
            [0x8000_0000, u32::MAX, 0x7fff_ffff, 1],
        ] {
            rect = values;
            assert_eq!(
                invoke(fc4p_0082b1c0 as *const () as usize, p, &[]),
                (rect[2].wrapping_sub(rect[0]), (RESULT >> 32) as u32)
            );
            assert_eq!(
                invoke(fc4p_0082b1d0 as *const () as usize, p, &[]),
                (rect[3].wrapping_sub(rect[1]), (RESULT >> 32) as u32)
            );
            assert_eq!(calls(), vec![(p as u32, vec![]), (p as u32, vec![])]);
        }
    }
    extern "thiscall" fn pixel_bits(this: *mut u8, format: u32) -> u64 {
        // SAFETY: the output's first word has already been set before the call.
        record(this, &[format, unsafe { word(this, 0) }]);
        RESULT
    }
    #[test]
    fn pixel_format_output_is_written_before_call_including_alias() {
        let _fake = fake(0x00449500, pixel_bits as *const () as usize);
        let mut object = Object::new(0x60);
        let p = object.ptr();
        for offset in [0x2cusize, 0x40] {
            for format in [0u32, 1, 4, u32::MAX] {
                fill(p, 0x60);
                // SAFETY: the original format word and output span both fit.
                unsafe {
                    put_word(p, 0x2c, format);
                }
                let output = p.wrapping_add(offset);
                let mut expected = memory(p, 0x60);
                expected_word(&mut expected, offset, format);
                expected_word(&mut expected, offset + 4, RESULT as u32);
                assert_eq!(
                    invoke(fc4p_0082b1e0 as *const () as usize, p, &[output as u32]),
                    (output as u32, (RESULT >> 32) as u32)
                );
                assert_eq!(calls(), vec![(output as u32, vec![format, format])]);
                assert_eq!(memory(p, 0x60), expected);
            }
        }
    }
    #[test]
    fn release_lock_wraps_count_refreshes_only_last_dirty_lock_and_pops_argument() {
        let mut object = Object::new(0x60);
        let p = object.ptr();
        let mut service = Object::new(16);
        let q = service.ptr();
        let mut table = [0usize; 44];
        table[0xa8 / 4] = fake_zero as *const () as usize;
        // SAFETY: the service begins with a live callback vtable.
        unsafe {
            put_word(q, 0, table.as_ptr() as u32);
        }
        for count in [0u32, 1, 2, u32::MAX] {
            for flag in [0u8, 1, 0xff] {
                fill(p, 0x60);
                // SAFETY: context fields are within the allocation.
                unsafe {
                    put_word(p, 0x28, count);
                    put_word(p, 0x10, q as u32);
                    *p.add(0x49) = flag;
                }
                let mut expected = memory(p, 0x60);
                expected_word(&mut expected, 0x28, count.wrapping_sub(1));
                let refresh = count == 1 && flag != 0;
                assert_eq!(
                    invoke(fc4p_0082b1a0 as *const () as usize, p, &[0x1234]),
                    if refresh {
                        ((RESULT as u32 & !255) | 1, (RESULT >> 32) as u32)
                    } else {
                        ((EAX & !255) | 1, EDX)
                    }
                );
                assert_eq!(calls(), if refresh { vec![(q as u32, vec![])] } else { vec![] });
                assert_eq!(memory(p, 0x60), expected);
            }
        }
    }
    thread_local! {
        static SURFACE_OWNER:std::cell::Cell<u32>=const {std::cell::Cell::new(0)};
        static SECOND_SURFACE:std::cell::Cell<u32>=const {std::cell::Cell::new(0)};
        static THIRD_SURFACE:std::cell::Cell<u32>=const {std::cell::Cell::new(0)};
        static READY:std::cell::Cell<u32>=const {std::cell::Cell::new(0)};
    }
    extern "thiscall" fn surface_ready(this: *mut u8) -> u64 {
        record(this, &[0x40]);
        let ready = READY.with(|v| v.get());
        if ready as u8 == 0 {
            // SAFETY: this test callback replaces the live holder's surface pointer.
            unsafe {
                put_word(
                    SURFACE_OWNER.with(|v| v.get()) as *mut u8,
                    4,
                    SECOND_SURFACE.with(|v| v.get()),
                );
            }
        }
        (RESULT & 0xffff_ffff_0000_0000) | u64::from(ready)
    }
    extern "thiscall" fn surface_mode(this: *mut u8, mode: u32) -> u64 {
        record(this, &[0x44, mode]);
        // SAFETY: this callback replaces the same live holder's surface pointer again.
        unsafe {
            put_word(
                SURFACE_OWNER.with(|v| v.get()) as *mut u8,
                4,
                THIRD_SURFACE.with(|v| v.get()),
            );
        }
        RESULT
    }
    extern "thiscall" fn surface_refresh(this: *mut u8) -> u64 {
        record(this, &[0xa8]);
        RESULT
    }
    #[test]
    fn surface_readiness_uses_low_byte_and_reloads_after_each_callback() {
        let mut object = Object::new(16);
        let p = object.ptr();
        let mut surfaces = [Object::new(16), Object::new(16), Object::new(16)];
        let pointers = surfaces.each_mut().map(|o| o.ptr());
        let mut table = [0usize; 44];
        table[0x40 / 4] = surface_ready as *const () as usize;
        table[0x44 / 4] = surface_mode as *const () as usize;
        table[0xa8 / 4] = surface_refresh as *const () as usize;
        // SAFETY: every surface begins with a live vtable and the mode field belongs to its holder.
        unsafe {
            for q in pointers {
                put_word(q, 0, table.as_ptr() as u32);
            }
            put_word(p, 8, 0x7654_3210);
        }
        SURFACE_OWNER.with(|v| v.set(p as u32));
        SECOND_SURFACE.with(|v| v.set(pointers[1] as u32));
        THIRD_SURFACE.with(|v| v.set(pointers[2] as u32));
        for ready in [0u32, 1, 0x1234_5600, 0x1234_5680] {
            READY.with(|v| v.set(ready));
            // SAFETY: restore the first surface for each readiness case.
            unsafe {
                put_word(p, 4, pointers[0] as u32);
            }
            let final_surface = if ready as u8 == 0 { pointers[2] } else { pointers[0] };
            assert_eq!(
                invoke(fc4p_0082af20 as *const () as usize, p, &[]),
                (final_surface as u32, (RESULT >> 32) as u32)
            );
            let mut expected = vec![(pointers[0] as u32, vec![0x40])];
            if ready as u8 == 0 {
                expected.extend([
                    (pointers[1] as u32, vec![0x44, 0x7654_3210]),
                    (pointers[2] as u32, vec![0xa8]),
                ]);
            }
            assert_eq!(calls(), expected);
        }
    }
    #[test]
    fn rectangle_and_packed_copy_return_the_original_ecx() {
        let mut object = Object::new(32);
        let p = object.ptr();
        // SAFETY: both calls read complete live objects; the copy's stack arguments are caller-popped.
        unsafe {
            put_word(p, 0, 0);
            put_word(p, 4, 0);
            put_word(p, 8, 4);
            put_word(p, 12, 8);
            let mut ecx = p as u32;
            core::arch::asm!("call {f}",f=sym fc4p_007c1870,inlateout("ecx") ecx,
                inlateout("eax") EAX=>_,inlateout("edx") EDX=>_,clobber_abi("C"));
            assert_eq!(ecx, p as u32);
            let args = [p as u32, p as u32 + 10, p as u32 + 16];
            core::arch::asm!("push dword ptr [edi+8]","push dword ptr [edi+4]","push dword ptr [edi]",
                "call {f}","add esp,12", f=sym fc4p_006d66f0,in("edi") args.as_ptr(),
                lateout("ecx") ecx,clobber_abi("C"));
            assert_eq!(ecx, p as u32 + 10);
        }
    }
    #[test]
    fn integer_entries_preserve_floating_environment_and_existing_x87_value() {
        #[repr(C, align(16))]
        struct State([u8; 512]);
        let mut object = Object::new(0xd0);
        let p = object.ptr();
        for f in [
            fc4p_0076bb00 as *const () as usize,
            fc4p_0076aaa0 as *const () as usize,
            fc4p_007c1870 as *const () as usize,
            fc4p_0082ad20 as *const () as usize,
        ] {
            let mut before = State([0; 512]);
            let mut after = State([0; 512]);
            // SAFETY: one free x87 slot; aligned state buffers; the pushed value is removed before assertions.
            unsafe {
                core::arch::asm!("fld1","fxsave [{p}]",p=in(reg) &mut before,out("st(0)") _,options(nostack));
                invoke(f, p, &[]);
                core::arch::asm!("fxsave [{p}]","fstp st(0)",p=in(reg) &mut after,out("st(0)") _,options(nostack));
            }
            assert_eq!(before.0[..160], after.0[..160]);
        }
    }

    thread_local! {
        static ALLOCATIONS: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
        static HOLDER: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
        static NEXT_SERVICE: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
        static ANSWER: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
    }
    extern "C" fn allocate_words(bytes: u32) -> u64 {
        record(core::ptr::null_mut(), &[0xa110, bytes]);
        let pointer = ALLOCATIONS.with(|a| a.borrow_mut().remove(0));
        (RESULT & 0xffff_ffff_0000_0000) | u64::from(pointer)
    }
    extern "C" fn release_words(pointer: *mut u8) -> u64 {
        record(pointer, &[0xfee]);
        RESULT
    }
    extern "thiscall" fn destroy_record(this: *mut u8, flags: u32) -> u64 {
        record(this, &[flags]);
        RESULT
    }
    #[test]
    fn landfill_destruction_releases_before_reading_range_and_keeps_fields() {
        let _free = fake(0x0090cf63, release_words as *const () as usize);
        let mut zone = Object::new(0x50);
        let p = zone.ptr();
        let mut records = Object::new(64);
        let r = records.ptr();
        let mut service = Object::new(4);
        let q = service.ptr();
        let table = [destroy_record as *const () as usize, 0, fake_zero as *const () as usize];
        // SAFETY: every object has a live table and both records fit their allocation.
        unsafe {
            put_word(q, 0, table.as_ptr() as u32);
            put_word(r, 0, table.as_ptr() as u32);
            put_word(r, 32, table.as_ptr() as u32);
        }
        for present in [false, true] {
            for count in [0u32, 1, 2] {
                fill(p, 0x50);
                let begin = if count == 0 { 0 } else { r as u32 };
                // SAFETY: set the zone's owned service and record range.
                unsafe {
                    put_word(p, 0x44, if present { q as u32 } else { 0 });
                    put_word(p, 0x1c, begin);
                    put_word(p, 0x20, begin + count * 32);
                }
                let mut expected = memory(p, 0x50);
                expected_word(&mut expected, 0, 0x00ab1610);
                let result = invoke(fc4p_006bbbc0 as *const () as usize, p, &[]);
                assert_eq!(
                    result,
                    if present || count != 0 {
                        (RESULT as u32, (RESULT >> 32) as u32)
                    } else {
                        (EAX, EDX)
                    }
                );
                let mut expected_calls = vec![];
                if present {
                    expected_calls.push((q as u32, vec![]));
                }
                for index in 0..count {
                    expected_calls.push((begin + index * 32, vec![0]));
                }
                if count != 0 {
                    expected_calls.push((begin, vec![0xfee]));
                }
                assert_eq!(calls(), expected_calls);
                assert_eq!(memory(p, 0x50), expected);
            }
        }
    }
    extern "C" fn copy_records(first: *mut u8, last: *mut u8, output: *mut u8, stop: *mut u8) -> u64 {
        // SAFETY: the test supplies whole records and the caller's live stop byte.
        unsafe {
            record(first, &[0xc0, last as u32, output as u32, u32::from(*stop)]);
            let mut source = first;
            let mut destination = output;
            while source != last {
                if !destination.is_null() {
                    for offset in [0, 4, 8, 16, 20, 24, 28] {
                        put_word(destination, offset, word(source, offset));
                    }
                    *destination.add(12) = *source.add(12);
                    *destination.add(13) = *source.add(13);
                }
                source = source.wrapping_add(32);
                destination = destination.wrapping_add(32);
            }
            (RESULT & 0xffff_ffff_0000_0000) | u64::from(destination as u32)
        }
    }
    extern "C" fn repeat_records(output: *mut u8, count: u32, source: *mut u8, stop: *mut u8) -> u64 {
        // SAFETY: the output capacity covers count records, and stop remains in the caller's frame.
        unsafe {
            record(output, &[0xc1, count, source as u32, u32::from(*stop)]);
            for index in 0..count {
                let destination = output.wrapping_add(index as usize * 32);
                if !destination.is_null() {
                    core::ptr::copy_nonoverlapping(source, destination, 32);
                }
            }
            (RESULT & 0xffff_ffff_0000_0000) | u64::from((output as u32).wrapping_add(count * 32))
        }
    }
    #[test]
    fn zone_vector_growth_copies_prefix_insert_suffix_then_destroys_old_records() {
        let _allocate = fake(0x0090cf54, allocate_words as *const () as usize);
        let _free = fake(0x0090cf63, release_words as *const () as usize);
        let _copy = fake(0x006bb5c0, copy_records as *const () as usize);
        let _repeat = fake(0x006bb120, repeat_records as *const () as usize);
        let mut header = Object::new(16);
        let p = header.ptr();
        let mut old = Object::new(64);
        let r = old.ptr();
        let mut output = Object::new(256);
        let out = output.ptr();
        let mut value = Object::new(32);
        let v = value.ptr();
        let table = [destroy_record as *const () as usize];
        fill(r, 64);
        fill(v, 32);
        // SAFETY: install each record's destructor table and a value with distinct fields.
        unsafe {
            for index in [0, 32] {
                put_word(r, index, table.as_ptr() as u32);
            }
            for offset in [4, 8, 16, 20, 24, 28] {
                put_word(v, offset, 0x7000 + offset as u32);
            }
            *v.add(12) = 0x12;
            *v.add(13) = 0x34;
        }
        for size in [0u32, 2] {
            for count in [0u32, 1, 3] {
                for skip_suffix in [0u32, 0xabcdef01] {
                    let begin = if size == 0 { 0 } else { r as u32 };
                    let position = if size == 0 { 0 } else { begin + 32 };
                    let capacity = size + size.max(count);
                    let allocation = if capacity == 0 { 0 } else { out as u32 };
                    ALLOCATIONS.with(|a| *a.borrow_mut() = if capacity == 0 { vec![] } else { vec![allocation] });
                    fill(p, 16);
                    fill(out, 256);
                    // SAFETY: restore the vector's three words for this independent case.
                    unsafe {
                        put_word(p, 0, begin);
                        put_word(p, 4, begin + size * 32);
                        put_word(p, 8, begin + size * 32);
                    }
                    let result = invoke(
                        fc4p_006bbc10 as *const () as usize,
                        p,
                        &[position, v as u32, 0xdeadbeef, count, skip_suffix],
                    );
                    let prefix = u32::from(size != 0);
                    let length = prefix + count + if skip_suffix == 0 { size - prefix } else { 0 };
                    // SAFETY: the vector has exactly three written words; the padding word stays intact.
                    unsafe {
                        assert_eq!(
                            [word(p, 0), word(p, 4), word(p, 8), word(p, 12)],
                            [
                                allocation,
                                allocation + length * 32,
                                allocation + capacity * 32,
                                0xa5a5a5a5
                            ]
                        );
                    }
                    let edx = if size == 0 && count == 1 && skip_suffix != 0 {
                        0x701c
                    } else {
                        (RESULT >> 32) as u32
                    };
                    assert_eq!(result, (allocation + capacity * 32, edx));
                    let mut expected = vec![];
                    if capacity != 0 {
                        expected.push((0, vec![0xa110, capacity * 32]));
                    }
                    expected.push((begin, vec![0xc0, position, allocation, skip_suffix & 255]));
                    let inserted = allocation + prefix * 32;
                    if count != 1 {
                        expected.push((inserted, vec![0xc1, count, v as u32, skip_suffix & 255]));
                    }
                    if skip_suffix == 0 {
                        expected.push((position, vec![0xc0, begin + size * 32, inserted + count * 32, 0]));
                    }
                    for index in 0..size {
                        expected.push((begin + index * 32, vec![0]));
                    }
                    if size != 0 {
                        expected.push((begin, vec![0xfee]));
                    }
                    assert_eq!(calls(), expected);
                    if count == 1 {
                        let mut record = memory(v, 32);
                        expected_word(&mut record, 0, 0x00ab15dc);
                        record[14..16].fill(0xa5);
                        assert_eq!(memory(inserted as *mut u8, 32), record);
                    }
                    assert!(ALLOCATIONS.with(|a| a.borrow().is_empty()));
                }
            }
        }
    }
    extern "thiscall" fn station_id(this: *mut u8, purpose: u32) -> u64 {
        record(this, &[purpose]);
        // SAFETY: swap the simulator's department so the next call must reload its object pointer.
        unsafe {
            put_word(
                HOLDER.with(|v| v.get()) as *mut u8,
                0x74,
                NEXT_SERVICE.with(|v| v.get()),
            );
        }
        0x11223344_55667788
    }
    extern "thiscall" fn funding_item(this: *mut u8, id: u32, output: *mut u8) -> u64 {
        // SAFETY: the caller initialized only the item's final word; the budget writes the rest.
        unsafe {
            record(this, &[id, word(output, 28)]);
            put_word(output, 16, 0x87654321);
            put_word(output, 28, 0x7fc12345);
        }
        (RESULT & 0xffff_ffff_0000_0000) | u64::from(ANSWER.with(|v| v.get()))
    }
    #[test]
    fn station_funding_uses_saved_table_reloaded_service_optional_outputs_and_low_byte() {
        let mut simulator = Object::new(0xb4);
        let p = simulator.ptr();
        let mut station = Object::new(4);
        let s = station.ptr();
        let mut service = Object::new(4);
        let q = service.ptr();
        let mut replacement = Object::new(4);
        let q2 = replacement.ptr();
        let mut table = [0usize; 52];
        table[0xcc / 4] = funding_item as *const () as usize;
        let mut station_table = [0usize; 7];
        station_table[6] = station_id as *const () as usize;
        // SAFETY: the replacement deliberately has no usable table: the original captured table must be used.
        unsafe {
            put_word(s, 0, station_table.as_ptr() as u32);
            put_word(q, 0, table.as_ptr() as u32);
            put_word(q2, 0, 0);
            put_word(p, 0xb0, 0x12);
        }
        HOLDER.with(|v| v.set(p as u32));
        NEXT_SERVICE.with(|v| v.set(q2 as u32));
        let mut output = Object::new(16);
        let out = output.ptr();
        for answer in [0u32, 0xdeadbe00, 1, 0x12345680] {
            for cost in [0u32, out as u32] {
                for funding in [0u32, out as u32, out as u32 + 4] {
                    ANSWER.with(|v| v.set(answer));
                    fill(out, 16);
                    // SAFETY: restore the department before the station callback swaps it.
                    unsafe {
                        put_word(p, 0x74, q as u32);
                    }
                    let result = invoke(fc4p_006e08d0 as *const () as usize, p, &[s as u32, cost, funding]);
                    let mut expected = vec![0xa5; 16];
                    if answer as u8 != 0 {
                        if funding != 0 {
                            expected_word(&mut expected, (funding - out as u32) as usize, 0x7fc12345);
                        }
                        if cost != 0 {
                            expected_word(&mut expected, 0, 0x87654321);
                        }
                        assert_eq!(
                            result,
                            (
                                (cost & !255) | 1,
                                if cost == 0 { (RESULT >> 32) as u32 } else { 0x87654321 }
                            )
                        );
                    } else {
                        assert_eq!(result, (answer, (RESULT >> 32) as u32));
                    }
                    assert_eq!(memory(out, 16), expected);
                    assert_eq!(calls(), vec![(s as u32, vec![0x12]), (q2 as u32, vec![0x55667788, 0])]);
                }
            }
        }
    }
    #[test]
    fn list_range_insertion_preserves_order_and_empty_registers() {
        let _allocate = fake(0x0090cf54, allocate_words as *const () as usize);
        let mut list = Object::new(12);
        let p = list.ptr();
        let mut input = Object::new(36);
        let r = input.ptr();
        let mut output = Object::new(36);
        let out = output.ptr();
        for count in [0u32, 1, 3] {
            fill(out, 36);
            // SAFETY: circular sentinels and the specified source node chain are live.
            unsafe {
                put_word(p, 0, p as u32);
                put_word(p, 4, p as u32);
                for index in 0..3u32 {
                    put_word(r, index as usize * 12, r as u32 + (index + 1) * 12);
                    put_word(r, index as usize * 12 + 8, 0x10203040 + index);
                }
            }
            ALLOCATIONS.with(|a| *a.borrow_mut() = (0..count).map(|i| out as u32 + i * 12).collect());
            let result = invoke(
                fc4p_006f1570 as *const () as usize,
                core::ptr::null_mut(),
                &[p as u32, r as u32, r as u32 + count * 12, 0xdeadbeef],
            );
            if count == 0 {
                assert_eq!(result, (EAX, EDX));
            } else {
                assert_eq!(result, (out as u32 + (count - 1) * 12, 0x10203040 + count - 1));
                // SAFETY: follow only the count newly linked nodes and check both directions.
                unsafe {
                    let mut node = word(p, 0);
                    let mut previous = p as u32;
                    for index in 0..count {
                        assert_eq!(node, out as u32 + index * 12);
                        assert_eq!(word(node as *mut u8, 4), previous);
                        assert_eq!(word(node as *mut u8, 8), 0x10203040 + index);
                        previous = node;
                        node = word(node as *mut u8, 0);
                    }
                    assert_eq!(node, p as u32);
                    assert_eq!(word(p, 4), previous);
                }
            }
            assert_eq!(calls(), vec![(0, vec![0xa110, 12]); count as usize]);
        }
    }
    extern "thiscall" fn balance(this: *mut u8) -> u64 {
        record(this, &[0xba1]);
        RESULT
    }
    extern "thiscall" fn budget_sink(this: *mut u8, create: u32) -> u64 {
        record(this, &[create]);
        (RESULT & 0xffff_ffff_0000_0000) | u64::from(NEXT_SERVICE.with(|v| v.get()))
    }
    #[test]
    fn budget_publish_retains_sets_full_signed_balance_then_releases_optional_sink() {
        let _balance = fake(0x006e9210, balance as *const () as usize);
        let mut budget = Object::new(0x34);
        let p = budget.ptr();
        let mut service = Object::new(4);
        let q = service.ptr();
        let mut sink = Object::new(4);
        let s = sink.ptr();
        let mut table = [0usize; 21];
        table[0x50 / 4] = budget_sink as *const () as usize;
        table[1] = fake_zero as *const () as usize;
        table[2] = fake_zero as *const () as usize;
        table[0x30 / 4] = fake_two as *const () as usize;
        // SAFETY: the service and optional sink use live callbacks.
        unsafe {
            put_word(p, 0x30, q as u32);
            put_word(q, 0, table.as_ptr() as u32);
            put_word(s, 0, table.as_ptr() as u32);
        }
        for present in [0u32, s as u32] {
            NEXT_SERVICE.with(|v| v.set(present));
            let before = memory(p, 0x34);
            let result = invoke(fc4p_00700fa0 as *const () as usize, p, &[]);
            assert_eq!(
                result,
                (if present == 0 { 0 } else { RESULT as u32 }, (RESULT >> 32) as u32)
            );
            assert_eq!(memory(p, 0x34), before);
            let mut expected = vec![(q as u32, vec![1])];
            if present != 0 {
                expected.extend([
                    (s as u32, vec![]),
                    (p as u32, vec![0xba1]),
                    (s as u32, vec![RESULT as u32, (RESULT >> 32) as u32]),
                    (s as u32, vec![]),
                ]);
            }
            assert_eq!(calls(), expected);
        }
    }
    extern "thiscall" fn point_available(this: *mut u8, x: u32, z: u32) -> u64 {
        record(this, &[0x188, x, z]);
        (RESULT & 0xffff_ffff_0000_0000) | u64::from(ANSWER.with(|v| v.get()))
    }
    extern "thiscall" fn snap_point(this: *mut u8, x: u32, z: u32, ox: *mut u32, oz: *mut u32) -> u64 {
        // SAFETY: the caller initialized both local words before the callback.
        unsafe {
            record(this, &[0x17c, x, z, *ox, *oz]);
            *ox = x + 10;
            *oz = z + 20;
        }
        RESULT
    }
    extern "thiscall" fn apply_point(this: *mut u8, x: u32, z: u32, flags: u32) -> u64 {
        record(this, &[0x18, x, z, flags]);
        RESULT
    }
    #[test]
    fn point_snap_checks_low_byte_and_passes_adjusted_coordinates() {
        let mut owner = Object::new(0x24);
        let p = owner.ptr();
        let mut service = Object::new(4);
        let q = service.ptr();
        let mut point = [4u32, 0x12345678, 8];
        let mut table = [0usize; 99];
        table[0x188 / 4] = point_available as *const () as usize;
        table[0x17c / 4] = snap_point as *const () as usize;
        table[0x18 / 4] = apply_point as *const () as usize;
        // SAFETY: both owner and service begin with a live table.
        unsafe {
            put_word(p, 0, table.as_ptr() as u32);
            put_word(p, 0x20, q as u32);
            put_word(q, 0, table.as_ptr() as u32);
        }
        for answer in [0u32, 0xdeadbe00, 0x12345601, 0xff] {
            ANSWER.with(|v| v.set(answer));
            assert_eq!(
                invoke(fc4p_0072a1c0 as *const () as usize, p, &[point.as_mut_ptr() as u32]),
                (if answer as u8 == 0 { 0 } else { RESULT as u32 }, (RESULT >> 32) as u32)
            );
            let mut expected = vec![(q as u32, vec![0x188, 4, 8])];
            if answer as u8 != 0 {
                expected.extend([(q as u32, vec![0x17c, 4, 8, 4, 8]), (p as u32, vec![0x18, 14, 28, 0])]);
            }
            assert_eq!(calls(), expected);
            assert_eq!(point, [4, 0x12345678, 8]);
        }
    }
    extern "thiscall" fn object_origin(this: *mut u8, x: *mut u32, z: *mut u32) -> u64 {
        record(this, &[0x54]);
        // SAFETY: the two outputs are distinct original argument words.
        unsafe {
            *x = ANSWER.with(|v| v.get());
            *z = NEXT_SERVICE.with(|v| v.get());
        }
        RESULT
    }
    #[test]
    fn grid_lookup_checks_signed_bounds_and_optional_origin_match() {
        let mut owner = Object::new(0x54);
        let p = owner.ptr();
        let mut object = Object::new(4);
        let q = object.ptr();
        let mut cells = [0u32; 4];
        let mut rows = [0u32; 6];
        rows[0] = cells.as_mut_ptr() as u32;
        rows[3] = cells.as_mut_ptr().wrapping_add(2) as u32;
        let mut table = [0usize; 22];
        table[0x54 / 4] = object_origin as *const () as usize;
        // SAFETY: the owner describes a 2 by 2 grid of object pointers.
        unsafe {
            put_word(p, 0x24, 2);
            put_word(p, 0x28, 2);
            put_word(p, 0x50, rows.as_ptr() as u32);
            put_word(q, 0, table.as_ptr() as u32);
        }
        for x in [-1i32, 0, 1, 2, i32::MIN, i32::MAX] {
            for z in [-1i32, 0, 1, 2] {
                for origin in [0u32, 1, 0xdeadbe00, 0xff] {
                    for present in [0u32, q as u32] {
                        cells.fill(present);
                        ANSWER.with(|v| v.set(x as u32));
                        NEXT_SERVICE.with(|v| v.set(z as u32));
                        let result = invoke(fc4p_0072b410 as *const () as usize, p, &[x as u32, z as u32, origin]);
                        let valid = (0..2).contains(&x) && (0..2).contains(&z);
                        let checked = valid && present != 0 && origin as u8 != 0;
                        assert_eq!(
                            result,
                            (
                                if valid { present } else { 0 },
                                if checked {
                                    (RESULT >> 32) as u32
                                } else if valid {
                                    rows[x as usize * 3]
                                } else {
                                    EDX
                                }
                            )
                        );
                        assert_eq!(calls(), if checked { vec![(q as u32, vec![0x54])] } else { vec![] });
                    }
                }
            }
        }
        cells.fill(q as u32);
        for (ox, oz) in [(0u32, 1u32), (1, 0), (2, 2)] {
            ANSWER.with(|v| v.set(ox));
            NEXT_SERVICE.with(|v| v.set(oz));
            assert_eq!(
                invoke(fc4p_0072b410 as *const () as usize, p, &[0, 0, 1]),
                (0, (RESULT >> 32) as u32)
            );
            assert_eq!(calls(), vec![(q as u32, vec![0x54])]);
        }
    }

    fn invoke_cdecl(function: usize, this: *mut u8, args: &[u32], eax: u32) -> (u32, u32) {
        let mut call = RawCall::new(Conv::Cdecl, function, this as u32, args);
        call.eax = eax;
        call.edx = EDX;
        // SAFETY: every use describes a cdecl entry and its complete argument list.
        unsafe {
            call_raw(&mut call);
        }
        (call.eax, call.edx)
    }
    thunk_test!(button_secondary_release, fc4p_00793fe0, -224, 0x00793ec0, fake_zero, []);
    #[test]
    fn edge_direction_components_wrap_negation_and_write_aliases_in_order() {
        let mut output = [0u32; 4];
        let p = output.as_mut_ptr().cast::<u8>();
        for direction in [0u32, 1, 2, 3, 4, u32::MAX] {
            for distance in [0u32, 1, 0x80000000, u32::MAX] {
                for alias in [false, true] {
                    output.fill(0xa5a5a5a5);
                    let z = p.wrapping_add(if alias { 0 } else { 4 });
                    let (xv, zv, eax, edx) = match direction {
                        1 => (distance, u32::MAX, p as u32, z as u32),
                        2 => (u32::MAX, 0u32.wrapping_sub(distance), p as u32, z as u32),
                        3 => (0u32.wrapping_sub(distance), 1, 0u32.wrapping_sub(distance), z as u32),
                        _ => (1, distance, p as u32, distance),
                    };
                    assert_eq!(
                        invoke_cdecl(
                            fc4p_007a65d0 as *const () as usize,
                            core::ptr::null_mut(),
                            &[direction, distance, p as u32, z as u32],
                            EAX
                        ),
                        (eax, edx)
                    );
                    assert_eq!(
                        output,
                        if alias {
                            [zv, 0xa5a5a5a5, 0xa5a5a5a5, 0xa5a5a5a5]
                        } else {
                            [xv, zv, 0xa5a5a5a5, 0xa5a5a5a5]
                        }
                    );
                }
            }
        }
    }
    #[test]
    fn optional_child_values_forward_only_when_present() {
        let mut owner = Object::new(48);
        let p = owner.ptr();
        let mut child = Object::new(4);
        let q = child.ptr();
        let mut table = [0usize; 11];
        table[9] = fake_zero as *const () as usize;
        table[10] = fake_zero as *const () as usize;
        // SAFETY: install the live child table; the interface is 32 bytes into the owner.
        unsafe {
            put_word(q, 0, table.as_ptr() as u32);
        }
        for function in [fc4p_007b62f0 as *const () as usize, fc4p_007b6310 as *const () as usize] {
            for pointer in [0u32, q as u32] {
                // SAFETY: set only the optional child field.
                unsafe {
                    put_word(p, 0, pointer);
                }
                let before = memory(p, 48);
                assert_eq!(
                    invoke(function, p.wrapping_add(32), &[]),
                    if pointer == 0 {
                        (0, EDX)
                    } else {
                        (RESULT as u32, (RESULT >> 32) as u32)
                    }
                );
                assert_eq!(calls(), if pointer == 0 { vec![] } else { vec![(q as u32, vec![])] });
                assert_eq!(memory(p, 48), before);
            }
        }
    }
    extern "thiscall" fn rectangle_set(this: *mut u8, a: u32, b: u32, c: u32, d: u32) -> u64 {
        record(this, &[a, b, c, d]);
        RESULT
    }
    #[test]
    fn window_rectangle_set_precedes_geometry_update() {
        let _set = fake(0x009bc0b8, rectangle_set as *const () as usize);
        let _update = fake(0x007c0390, fake_zero as *const () as usize);
        let mut owner = Object::new(4);
        let p = owner.ptr();
        let args = [0u32, u32::MAX, 0x80000000, 0x12345678];
        assert_eq!(
            invoke(fc4p_007c07d0 as *const () as usize, p, &args),
            (RESULT as u32, (RESULT >> 32) as u32)
        );
        assert_eq!(calls(), vec![(p as u32, args.to_vec()), (p as u32, vec![])]);
    }
    #[test]
    fn embedded_word_copy_handles_identity_and_overlapping_outputs_sequentially() {
        let mut owner = Object::new(0x240);
        let p = owner.ptr();
        for displacement in [-8i32, -4, 0, 4, 8, 64] {
            fill(p, 0x240);
            // SAFETY: the source covers sixteen distinct initialized words.
            unsafe {
                for i in 0..16usize {
                    put_word(p, 0x1ac + i * 4, 0xabc000 + i as u32);
                }
            }
            let mut expected = memory(p, 0x240);
            let target = (0x1aci32 + displacement) as usize;
            for i in 0..16usize {
                let value = u32::from_le_bytes(expected[0x1ac + i * 4..0x1b0 + i * 4].try_into().unwrap());
                expected_word(&mut expected, target + i * 4, value);
            }
            assert_eq!(
                invoke(fc4p_007d2730 as *const () as usize, p, &[p.wrapping_add(target) as u32]),
                (EAX, EDX)
            );
            assert_eq!(memory(p, 0x240), expected);
        }
    }
    extern "C" fn allocate_sentinel(bytes: u32) -> u64 {
        // SAFETY: observe the header before allocation; its first word is already cleared.
        unsafe {
            record(
                HOLDER.with(|v| v.get()) as *mut u8,
                &[bytes, word(HOLDER.with(|v| v.get()) as *mut u8, 0)],
            );
        }
        (RESULT & 0xffff_ffff_0000_0000) | u64::from(NEXT_SERVICE.with(|v| v.get()))
    }
    #[test]
    fn tree_constructor_initializes_only_tag_parent_links_and_header() {
        let _allocate = fake(0x0090cf54, allocate_sentinel as *const () as usize);
        let mut owner = Object::new(16);
        let p = owner.ptr();
        let mut sentinel = Object::new(56);
        let q = sentinel.ptr();
        fill(p, 16);
        fill(q, 56);
        HOLDER.with(|v| v.set(p as u32));
        NEXT_SERVICE.with(|v| v.set(q as u32));
        assert_eq!(
            invoke(fc4p_0081d200 as *const () as usize, p, &[]),
            (p as u32, (RESULT >> 32) as u32)
        );
        let mut expected_owner = vec![0xa5; 16];
        expected_word(&mut expected_owner, 0, q as u32);
        expected_word(&mut expected_owner, 4, 0);
        let mut expected_sentinel = vec![0xa5; 56];
        expected_sentinel[0] = 0;
        expected_word(&mut expected_sentinel, 4, 0);
        expected_word(&mut expected_sentinel, 8, q as u32);
        expected_word(&mut expected_sentinel, 12, q as u32);
        assert_eq!(memory(p, 16), expected_owner);
        assert_eq!(memory(q, 56), expected_sentinel);
        assert_eq!(calls(), vec![(p as u32, vec![56, 0])]);
    }
    #[test]
    fn format_color_masks_take_the_format_in_eax() {
        for format in (0u32..32).chain([0x80000000, u32::MAX]) {
            let mask = match format {
                4 | 6 => 0x7fff,
                5 => 0xffff,
                7 => 0xfff,
                8 | 9 => 0xffffff,
                _ => u32::MAX,
            };
            assert_eq!(
                invoke_cdecl(fc4p_00825c90 as *const () as usize, core::ptr::null_mut(), &[], format),
                (mask, EDX)
            );
        }
    }
    #[test]
    fn pixel_expansion_exhausts_sixteen_bit_colors_and_checks_full_words() {
        let mut context = Object::new(0x30);
        let p = context.ptr();
        for format in (0u32..12).chain([u32::MAX]) {
            // SAFETY: the format word fits in the context.
            unsafe {
                put_word(p, 0x2c, format);
            }
            let before = memory(p, 0x30);
            for pixel in (0u32..=0xffff).chain([0x80000000, 0xdeadbeef, u32::MAX]) {
                let scale5 = |v: u32| (v << 3) | (v >> 2);
                let scale6 = |v: u32| (v << 2) | (v >> 4);
                let (expected, edx) = match format {
                    4 | 6 => {
                        let r = scale5((pixel >> 10) & 31);
                        let g = scale5((pixel >> 5) & 31);
                        let b = scale5(pixel & 31);
                        (
                            0xff000000 | r << 16 | g << 8 | b,
                            (((pixel >> 10) & 31) >> 2) << 16 | (((pixel >> 5) & 31) >> 2) << 8 | ((pixel & 31) >> 2),
                        )
                    }
                    5 => (
                        0xff000000
                            | scale5((pixel >> 11) & 31) << 16
                            | scale6((pixel >> 5) & 63) << 8
                            | scale5(pixel & 31),
                        0xff000000u32.wrapping_add((pixel & 0x7e0) << 5),
                    ),
                    7 => (
                        0xff000000
                            | (((pixel >> 8) & 15) * 17) << 16
                            | (((pixel >> 4) & 15) * 17) << 8
                            | ((pixel & 15) * 17),
                        pixel & 0xf0,
                    ),
                    8 | 9 => (pixel | 0xff000000, EDX),
                    _ => (0, EDX),
                };
                assert_eq!(
                    invoke(fc4p_0082abf0 as *const () as usize, p, &[pixel]),
                    (expected, edx),
                    "format {format}, pixel {pixel:08x}"
                );
            }
            assert_eq!(memory(p, 0x30), before);
        }
    }
    #[test]
    fn pixel_packing_keeps_format_additions_and_uses_only_argument_bytes() {
        let mut context = Object::new(0x30);
        let p = context.ptr();
        for format in (0u32..12).chain([u32::MAX]) {
            // SAFETY: the format word fits in the context.
            unsafe {
                put_word(p, 0x2c, format);
            }
            let before = memory(p, 0x30);
            for red in [0u32, 7, 8, 15, 16, 127, 128, 248, 255] {
                for green in [0u32, 3, 4, 7, 8, 15, 16, 252, 255] {
                    for blue in [0u32, 7, 8, 15, 16, 128, 255] {
                        let (expected, edx) = match format {
                            4 | 6 => (
                                0xf000 + ((red >> 3) << 10) + ((green >> 3) << 5) + (blue >> 3),
                                blue >> 3,
                            ),
                            5 => (
                                0xf000 + ((red >> 3) << 11) + ((green >> 2) << 5) + (blue >> 3),
                                blue >> 3,
                            ),
                            7 => (
                                0xf000 | ((red >> 4) << 8) | ((green >> 4) << 4) | (blue >> 4),
                                blue >> 4,
                            ),
                            8 | 9 => (0xff000000 | red << 16 | green << 8 | blue, blue),
                            _ => (0, EDX),
                        };
                        assert_eq!(
                            invoke(
                                fc4p_0082ad30 as *const () as usize,
                                p,
                                &[red | 0xdeadbe00, green | 0x12345600, blue | 0x87654300]
                            ),
                            (expected, edx)
                        );
                    }
                }
            }
            assert_eq!(memory(p, 0x30), before);
        }
    }
    extern "thiscall" fn expand_test_color(this: *mut u8, color: u32) -> u64 {
        record(this, &[color]);
        RESULT
    }
    #[test]
    fn channel_unpack_calls_expand_address_and_writes_three_bytes_in_order() {
        let _expand = fake(0x0082abf0, expand_test_color as *const () as usize);
        let mut context = Object::new(4);
        let p = context.ptr();
        let mut output = [0xa5u8; 12];
        let out = output.as_mut_ptr();
        for offsets in [[1usize, 4, 8], [1, 1, 1], [1, 4, 1], [4, 1, 1]] {
            output.fill(0xa5);
            let pointers = offsets.map(|offset| out.wrapping_add(offset) as u32);
            let mut expected = [0xa5; 12];
            for (offset, value) in offsets.into_iter().zip([0x34, 0x56, 0x78]) {
                expected[offset] = value;
            }
            assert_eq!(
                invoke(
                    fc4p_0082ae10 as *const () as usize,
                    p,
                    &[0xabcdef01, pointers[0], pointers[1], pointers[2]]
                ),
                ((RESULT as u32 & !255) | 1, pointers[1])
            );
            assert_eq!(output, expected);
            assert_eq!(calls(), vec![(p as u32, vec![0xabcdef01])]);
        }
    }
    #[test]
    fn context_format_description_writes_raw_format_and_depth_with_aliasing() {
        let mut context = Object::new(0x40);
        let p = context.ptr();
        for format in (0u32..12).chain([0x80000000, u32::MAX]) {
            for offset in [0x18usize, 0x2c] {
                fill(p, 0x40);
                // SAFETY: both possible outputs and the context format fit.
                unsafe {
                    put_word(p, 0x2c, format);
                }
                let out = p.wrapping_add(offset);
                let bits = match format {
                    2 => 8,
                    3..=7 => 16,
                    8 | 9 => 32,
                    _ => 0,
                };
                let mut expected = memory(p, 0x40);
                expected_word(&mut expected, offset, format);
                expected_word(&mut expected, offset + 4, bits);
                assert_eq!(
                    invoke(fc4p_0082b200 as *const () as usize, p, &[out as u32]),
                    (bits, out as u32)
                );
                assert_eq!(memory(p, 0x40), expected);
            }
        }
    }
    extern "thiscall" fn surface_mode_forward(this: *mut u8, a: u32, b: u32) -> u64 {
        // SAFETY: the holder's mode byte must already be updated before transfer to the surface.
        unsafe {
            record(
                this,
                &[a, b, u32::from(*(HOLDER.with(|v| v.get()) as *mut u8).add(0x48))],
            );
        }
        RESULT
    }
    #[test]
    fn surface_entries_forward_original_stack_and_update_only_the_mode_byte() {
        let mut holder = Object::new(0x50);
        let p = holder.ptr();
        let mut surface = Object::new(4);
        let q = surface.ptr();
        HOLDER.with(|v| v.set(p as u32));
        let mut table = [0usize; 39];
        for slot in [0x5c, 0x68, 0x6c, 0x94, 0x98] {
            table[slot / 4] = surface_mode_forward as *const () as usize;
        }
        // SAFETY: every forwarding slot accepts the exact two words used by the test.
        unsafe {
            put_word(q, 0, table.as_ptr() as u32);
        }
        for (function, flag) in [
            (fc4p_0082b4b0 as *const () as usize, 1),
            (fc4p_0082b540 as *const () as usize, 0xa5),
            (fc4p_0082b550 as *const () as usize, 0),
            (fc4p_0082b750 as *const () as usize, 0xa5),
            (fc4p_0082b760 as *const () as usize, 0xa5),
        ] {
            fill(p, 0x50);
            // SAFETY: restore the holder's surface pointer without altering its mode byte.
            unsafe {
                put_word(p, 0x14, q as u32);
            }
            let mut expected = memory(p, 0x50);
            expected[0x48] = flag;
            assert_eq!(
                invoke(function, p, &[0x12345678, 0xdeadbeef]),
                (RESULT as u32, (RESULT >> 32) as u32)
            );
            assert_eq!(calls(), vec![(q as u32, vec![0x12345678, 0xdeadbeef, u32::from(flag)])]);
            assert_eq!(memory(p, 0x50), expected);
        }
    }
    #[test]
    fn x87_precision_controls_preserve_other_control_bits_stack_status_and_mxcsr() {
        let _fpu = GameFpu::new();
        #[repr(C, align(16))]
        struct State([u8; 512]);
        for control in [0x007fu16, 0x027f, 0x037f, 0x017f, 0x0c7f, 0x087f] {
            for mode in [0u32, 1, 2, 3, u32::MAX] {
                crate::x87::set_control_word(control);
                let mut before = State([0; 512]);
                let mut after = State([0; 512]);
                // SAFETY: one free x87 slot and two aligned save buffers.
                unsafe {
                    core::arch::asm!("fldpi", "fxsave [{state}]", state = in(reg) &mut before, out("st(0)") _);
                }
                let result = invoke_cdecl(fc4p_008929e0 as *const () as usize, core::ptr::null_mut(), &[mode], EAX);
                // SAFETY: record the environment and pop exactly the test value pushed above.
                unsafe {
                    core::arch::asm!("fxsave [{state}]", "fstp st(0)", state = in(reg) &mut after, out("st(0)") _);
                }
                let expected = (control & !0x300)
                    | match mode {
                        1 => 0x200,
                        2 => 0x300,
                        _ => 0,
                    };
                assert_eq!(result, (EAX, EDX));
                assert_eq!(crate::x87::control_word(), expected);
                before.0[..2].copy_from_slice(&expected.to_le_bytes());
                assert_eq!(before.0[..160], after.0[..160]);
                assert_eq!(
                    invoke_cdecl(fc4p_00892a0a as *const () as usize, core::ptr::null_mut(), &[], EAX),
                    (
                        match expected & 0x300 {
                            0 => 0,
                            0x200 => 1,
                            _ => 2,
                        },
                        EDX
                    )
                );
            }
            crate::x87::set_control_word(control);
            assert_eq!(
                invoke_cdecl(fc4p_00892a0a as *const () as usize, core::ptr::null_mut(), &[], EAX),
                (
                    match control & 0x300 {
                        0 => 0,
                        0x200 => 1,
                        _ => 2,
                    },
                    EDX
                )
            );
        }
    }
    extern "stdcall" fn allocate_single_node(value: *mut u8) -> u64 {
        // SAFETY: the payload pointer is valid for one word, and the fake supplies a complete node.
        unsafe {
            record(value, &[word(value, 0)]);
            let node = NEXT_SERVICE.with(|v| v.get()) as *mut u8;
            put_word(node, 8, word(value, 0));
            (RESULT & 0xffff_ffff_0000_0000) | u64::from(node as u32)
        }
    }
    #[test]
    fn single_node_insertion_links_then_writes_iterator_even_when_it_aliases_position() {
        let _allocate = fake(0x0094b33a, allocate_single_node as *const () as usize);
        let mut position = Object::new(12);
        let p = position.ptr();
        let mut node = Object::new(12);
        let q = node.ptr();
        let mut payload = 0x98765432u32;
        let mut iterator = 0u32;
        NEXT_SERVICE.with(|v| v.set(q as u32));
        for alias in [false, true] {
            fill(q, 12);
            // SAFETY: restore an empty circular list and a separate iterator output when requested.
            unsafe {
                put_word(p, 0, p as u32);
                put_word(p, 4, p as u32);
            }
            let output = if alias { p } else { (&mut iterator as *mut u32).cast() };
            assert_eq!(
                invoke(
                    fc4p_00897c8d as *const () as usize,
                    core::ptr::null_mut(),
                    &[output as u32, p as u32, &mut payload as *mut u32 as u32]
                ),
                (output as u32, p as u32)
            );
            assert_eq!(calls(), vec![(&mut payload as *mut u32 as u32, vec![payload])]);
            // SAFETY: inspect only the two live link headers and payload word.
            unsafe {
                assert_eq!([word(q, 0), word(q, 4), word(q, 8)], [p as u32, p as u32, payload]);
                assert_eq!([word(p, 0), word(p, 4)], [q as u32, q as u32]);
                assert_eq!(word(output, 0), q as u32);
            }
        }
    }
    extern "thiscall" fn construct_test_deque(this: *mut u8) -> u64 {
        let base = this.wrapping_sub(40);
        record(this, &memory(base, 40).into_iter().map(u32::from).collect::<Vec<_>>());
        // SAFETY: the fake writes only the embedded deque's first word.
        unsafe {
            put_word(this, 0, 0xfedcba98);
        }
        RESULT
    }
    #[test]
    fn queued_state_constructor_leaves_padding_and_initializes_before_embedded_constructor() {
        let _construct = fake(0x0092e6c9, construct_test_deque as *const () as usize);
        let mut owner = Object::new(64);
        let p = owner.ptr();
        fill(p, 64);
        let mut expected = vec![0xa5; 64];
        expected[..32].fill(0);
        expected[32..34].copy_from_slice(&10u16.to_le_bytes());
        expected[36..40].fill(0);
        let at_call = expected[..40].iter().copied().map(u32::from).collect();
        expected_word(&mut expected, 40, 0xfedcba98);
        assert_eq!(
            invoke(fc4p_00897fed as *const () as usize, p, &[]),
            (p as u32, (RESULT >> 32) as u32)
        );
        assert_eq!(calls(), vec![(p.wrapping_add(40) as u32, at_call)]);
        assert_eq!(memory(p, 64), expected);
    }
    #[test]
    fn embedded_word_copy_obeys_and_preserves_backward_direction() {
        let mut owner = Object::new(0x240);
        let p = owner.ptr();
        fill(p, 0x240);
        // SAFETY: the backwards source spans sixteen live words ending at +0x1ac.
        unsafe {
            for i in 0..16usize {
                put_word(p, 0x1ac - i * 4, 0xabc000 + i as u32);
            }
        }
        let mut expected = memory(p, 0x240);
        for i in 0..16usize {
            let value = u32::from_le_bytes(expected[0x1ac - i * 4..0x1b0 - i * 4].try_into().unwrap());
            expected_word(&mut expected, 0x1a8 - i * 4, value);
        }
        let mut registers = [EAX, EDX, 0, p.wrapping_add(0x1a8) as u32, p as u32];
        // SAFETY: the output spans sixteen words backwards. DF is cleared immediately after saving it,
        // before control returns to any Rust code; the port uses a direction-aware ABI entry.
        unsafe {
            core::arch::asm!(
                "std", "push dword ptr [edi + 12]", "mov ecx, [edi + 16]",
                "mov eax, [edi]", "mov edx, [edi + 4]", "call {function}",
                "pushfd", "pop ecx", "cld", "mov [edi], eax", "mov [edi + 4], edx",
                "mov [edi + 8], ecx", function = sym fc4p_007d2730,
                in("edi") registers.as_mut_ptr(), clobber_abi("C"),
            );
        }
        assert_eq!(registers[..2], [EAX, EDX]);
        assert_ne!(registers[2] & 0x400, 0);
        assert_eq!(memory(p, 0x240), expected);
    }
}
