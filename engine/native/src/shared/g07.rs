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

/// This module's replacements.
pub const PORTS: &[Port] = &[
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
}
