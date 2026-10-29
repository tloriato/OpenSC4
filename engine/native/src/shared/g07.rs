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

layout! {
    /// A landfill zone's owned records and retained service.
    struct LandfillZoneFields {
        0x00 => table / set_table: u32,
        0x1c => first / set_first: u32,
        0x20 => end / set_end: u32,
        0x44 => service / set_service: *mut u8,
    }
}
layout! {
    /// Three-word vector header, also used for vectors of pairs and zone records.
    struct SharedVectorFields {
        0x00 => first / set_first: u32,
        0x04 => end / set_end: u32,
        0x08 => capacity_end / set_capacity_end: u32,
    }
}
layout! {
    /// A zone record; bytes +0x0e and +0x0f are padding and are not copied by construction.
    struct ZoneRecordFields {
        0x00 => table / set_table: u32,
        0x04 => first / set_first: u32,
        0x08 => second / set_second: u32,
        0x0c => first_flag / set_first_flag: u8,
        0x0d => second_flag / set_second_flag: u8,
        0x10 => third / set_third: u32,
        0x14 => fourth / set_fourth: u32,
        0x18 => fifth / set_fifth: u32,
        0x1c => sixth / set_sixth: u32,
    }
}
layout! {
    /// Fire and police budget lookup fields at the shared simulator offsets.
    struct FundingSimulatorFields {
        0x74 => department / set_department: *mut u8,
        0xb0 => purpose / set_purpose: u32,
    }
}
layout! {
    /// The budget item keeps a 64-bit cost and a funding float copied by its stored bits.
    struct FundingItemFields {
        0x10 => cost / set_cost: u64,
        0x1c => funding_bits / set_funding_bits: u32,
    }
}
layout! {
    /// Object grid dimensions and the array of three-word row headers.
    struct ObjectGridFields {
        0x24 => width / set_width: i32,
        0x28 => height / set_height: i32,
        0x50 => rows / set_rows: *mut u8,
    }
}
layout! {
    /// Header for the tree of 56-byte nodes.
    struct SharedTreeFields {
        0x00 => sentinel / set_sentinel: *mut u8,
        0x04 => count / set_count: u32,
    }
}
layout! {
    /// Only the tag, parent and two self links of a new sentinel are initialized.
    struct TreeSentinelFields {
        0x00 => tag / set_tag: u8,
        0x04 => parent / set_parent: *mut u8,
        0x08 => left / set_left: *mut u8,
        0x0c => right / set_right: *mut u8,
    }
}
layout! {
    /// The surface adapter owns two interfaces and a separate mode flag.
    struct SurfaceAdapterFields {
        0x00 => primary_table / set_primary_table: u32,
        0x04 => lock_table / set_lock_table: u32,
        0x0c => drawing_table / set_drawing_table: u32,
        0x10 => context / set_context: *mut u8,
        0x14 => surface / set_surface: *mut u8,
        0x28 => locks / set_locks: u32,
        0x30 => height / set_height: u32,
        0x40 => scale / set_scale: u32,
        0x48 => mode / set_mode: u8,
    }
}
layout! {
    /// A filter list embedded at +0xe0; the signed count is stored even when nonpositive.
    struct FilterListFields {
        0x120 => count / set_count: i32,
    }
}

game_fn! { fn allocate_message(bytes: u32) -> u64 = "C" 0x009133da; }
game_fn! { fn construct_message(this: *mut u8) -> u64 = "thiscall" 0x009134d6; }
game_data! { static shared_message_server: *mut u8 = 0x00b43ccc; }
register_port! { fc4p_006cf5c0, 0x006cf5c0, "send_neighbor_change_message", 0,
    send_neighbor_change_message(this, entry) {
        // SAFETY: ECX is an optional query interface; two caller-popped words select post/send and code.
        // Message construction and dispatch retain exactly the references returned by the game.
        unsafe {
            let mut retained = 0u32;
            let mut result = entry.result(entry.eax);
            let proceed = if this.is_null() { true } else {
                result = vcall!(this, 0 => fn(u32, *mut u32) -> u64, 1, &mut retained);
                result as u8 != 0
            };
            if proceed {
                let allocation = allocate_message(0x2c);
                let constructed = if allocation as u32 == 0 { allocation } else { construct_message(allocation as u32 as *mut u8) };
                let message = constructed as u32 as *mut u8;
                let held = if message.is_null() { constructed } else { virtual_result(message, 4, message as u32, word(message, 0), &[]) };
                virtual_result(message, 0x14, word(message, 0), (held >> 32) as u32, &[0xea418d8f]);
                let code = entry.arg(1);
                virtual_result(message, 0x2c, code, word(message, 0), &[code]);
                result = virtual_result(message, 0x84, retained, word(message, 0), &[retained]);
                let server = *shared_message_server();
                if entry.arg(0) as u8 != 0 {
                    result = virtual_result(server, 0x10, (result as u32 & !255) | (entry.arg(0) & 255), word(server, 0), &[message as u32, 0]);
                } else {
                    result = virtual_result(server, 0x0c, word(server, 0), (result >> 32) as u32, &[message as u32]);
                }
                result = virtual_result(message, 8, result as u32, word(message, 0), &[]);
            }
            if retained != 0 { result = virtual_result(retained as *mut u8, 8, word(retained as *mut u8, 0), (result >> 32) as u32, &[]); }
            entry.ecx = retained;
            result
        }
    }
}

game_fn! { fn update_filter_list(this: *mut u8) -> u64 = "thiscall" 0x007a93e0; }
port! {
    /// Copy the positive number of filter IDs in order, store the signed count, and refresh the filter.
    #[port(address = 0x007a97d0, owner = "shared", name = "set_filter_ids")]
    pub unsafe extern "thiscall" fn fc4p_007a97d0(this: *mut u8, source: *mut u8, count: i32) -> u64 {
        // SAFETY: the caller supplies enough source and object storage for a positive count.
        unsafe {
            let mut index = 0i32;
            while index < count {
                let offset = (index as usize).wrapping_mul(4);
                put_word(this, 0xe0usize.wrapping_add(offset), word(source, offset));
                index += 1;
            }
            FilterListFields::from_ptr(this).set_count(count);
            update_filter_list(this)
        }
    }
}

register_port! { fc4p_007cec30, 0x007cec30, "allocate_and_copy_word_pairs", 12,
    allocate_and_copy_word_pairs(_this, entry) {
        // SAFETY: the source range consists of eight-byte pairs; a null output skips its first pair.
        unsafe {
            let count = entry.arg(0);
            let allocation = if count == 0 { 0 } else { allocate_shared(count.wrapping_shl(3)) as u32 };
            let mut source = entry.arg(1);
            let end = entry.arg(2);
            let mut output = allocation;
            let edx = if source == end { source } else { source.wrapping_sub(allocation) };
            while source != end {
                if output != 0 {
                    put_word(output as *mut u8, 0, word(source as *mut u8, 0));
                    put_word(output as *mut u8, 4, word(source as *mut u8, 4));
                }
                output = output.wrapping_add(8);
                source = source.wrapping_add(8);
            }
            entry.ecx = output;
            (u64::from(edx) << 32) | u64::from(allocation)
        }
    }
}

register_port! { fc4p_007ced60, 0x007ced60, "construct_word_pair_vector_copy", 4,
    construct_word_pair_vector_copy(this, entry) {
        // SAFETY: both headers are live. The source range is read again after destination setup,
        // so a source header equal to the destination produces an empty copied range.
        unsafe {
            let source = entry.arg(0) as *mut u8;
            let count = (word(source, 4).wrapping_sub(word(source, 0)) as i32 >> 3) as u32;
            let vector = SharedVectorFields::from_ptr(this);
            vector.set_first(0); vector.set_end(0); vector.set_capacity_end(0);
            let allocation = if count == 0 { 0 } else { allocate_shared(count.wrapping_shl(3)) as u32 };
            vector.set_first(allocation); vector.set_end(allocation);
            vector.set_capacity_end(allocation.wrapping_add(count.wrapping_shl(3)));
            let end = word(source, 4);
            let mut current = word(source, 0);
            let mut output = allocation;
            while current != end {
                if output != 0 {
                    put_word(output as *mut u8, 0, word(current as *mut u8, 0));
                    put_word(output as *mut u8, 4, word(current as *mut u8, 4));
                }
                current = current.wrapping_add(8);
                output = output.wrapping_add(8);
            }
            vector.set_end(output);
            entry.ecx = current;
            (u64::from(end) << 32) | u64::from(this as u32)
        }
    }
}

game_fn! { fn construct_lock_interface(this: *mut u8) -> u64 = "thiscall" 0x0090d957; }
game_fn! { fn destroy_lock_interface(this: *mut u8) -> u64 = "thiscall" 0x0090d964; }
port! {
    /// Install the three adapter tables and initialize its counters and mode after base construction.
    #[port(address = 0x0082b780, owner = "shared", name = "construct_surface_adapter")]
    pub unsafe extern "thiscall" fn fc4p_0082b780(this: *mut u8) -> u64 {
        // SAFETY: the caller provides the adapter and its embedded interface at +4.
        unsafe {
            let adapter = SurfaceAdapterFields::from_ptr(this);
            adapter.set_primary_table(0x00ac1338);
            let result = construct_lock_interface(this.add(4));
            adapter.set_drawing_table(0x00ac14e8);
            adapter.set_primary_table(0x00ac1550);
            adapter.set_lock_table(0x00ac1538);
            adapter.set_drawing_table(0x00ac1510);
            adapter.set_context(core::ptr::null_mut()); adapter.set_surface(core::ptr::null_mut());
            adapter.set_locks(0); adapter.set_mode(0); adapter.set_height(0); adapter.set_scale(1);
            (result & 0xffff_ffff_0000_0000) | u64::from(this as u32)
        }
    }
}
port! {
    /// Release the surface, then context, destroy the embedded interface, and optionally free the adapter.
    #[port(address = 0x0082b7d0, owner = "shared", name = "delete_surface_adapter")]
    pub unsafe extern "thiscall" fn fc4p_0082b7d0(this: *mut u8, flags: u32) -> u64 {
        // SAFETY: the adapter owns both references; only bit zero controls freeing the object itself.
        unsafe {
            let adapter = SurfaceAdapterFields::from_ptr(this);
            adapter.set_primary_table(0x00ac1550); adapter.set_lock_table(0x00ac1538); adapter.set_drawing_table(0x00ac1510);
            if !adapter.surface().is_null() { vcall!(adapter.surface(), 8 => fn() -> u64); }
            if !adapter.context().is_null() { vcall!(adapter.context(), 8 => fn() -> u64); }
            let mut result = destroy_lock_interface(this.add(4));
            if flags & 1 != 0 { result = free_object(this); }
            (result & 0xffff_ffff_0000_0000) | u64::from(this as u32)
        }
    }
}

layout! {
    /// Two words in each element of a pair vector.
    struct SharedWordPairFields {
        0x00 => first / set_first: u32,
        0x04 => second / set_second: u32,
    }
}
layout! {
    /// Eight cleared state words, a 16-bit capacity, and the count before the embedded deque.
    struct QueuedStateFields {
        0x00 => parameters / set_parameters: [u32; 8],
        0x20 => capacity / set_capacity: u16,
        0x24 => count / set_count: u32,
    }
}

game_fn! { #[allow(dead_code)] fn read_vertex_record(source: u32, index: u32, output: *mut u8) -> u64 = "C" 0x0080aef0; }
game_fn! { #[allow(dead_code)] fn convert_vertex_record(this: *mut u8, input: *mut u8, output: *mut u8) -> u64 = "thiscall" 0x0080b090; }
game_fn! { #[allow(dead_code)] fn acquire_reference_lock(this: *mut u8, lock: *mut u8) -> u64 = "thiscall" 0x0048c9f0; }
game_fn! { #[allow(dead_code)] fn clear_cursor_handle(this: *mut u8, value: u32) -> u64 = "thiscall" 0x00891d48; }
game_fn! { #[allow(dead_code)] fn destroy_cursor_string(this: *mut u8) -> u64 = "thiscall" 0x00414250; }
game_data! { static set_cursor_function: usize = 0x00a80578; }
game_data! { static minimum_picture_zoom: f32 = 0x00a8821c; }
game_data! { static maximum_picture_zoom: f32 = 0x00a9422c; }

/// Preserves the volatile inputs of calls whose return can retain part of those inputs.
unsafe fn address_result(
    address: u32,
    conv: crate::abi::Conv,
    this: u32,
    eax: u32,
    edx: u32,
    args: &[u32],
) -> u64 {
    let mut call = crate::abi::RawCall::new(conv, crate::abi::target(address), this, args);
    call.eax = eax;
    call.edx = edx;
    // SAFETY: callers declare the address's signature and supply its original convention and arguments.
    unsafe {
        crate::abi::call_raw(&mut call);
    }
    (u64::from(call.edx) << 32) | u64::from(call.eax)
}

register_port! { fc4p_00793ff0, 0x00793ff0, "replace_window_surface", 4,
    replace_window_surface(this, entry) {
        // SAFETY: the holder and its retained surfaces expose the original interface slots.
        unsafe {
            let incoming = entry.arg(0) as *mut u8;
            let previous = word(this, 4) as *mut u8;
            let mut result = entry.result(entry.eax);
            if incoming != previous {
                if !incoming.is_null() {
                    result = virtual_result(incoming, 4, word(incoming, 0), (result >> 32) as u32, &[]);
                }
                put_word(this, 4, incoming as u32);
                if !previous.is_null() {
                    result = virtual_result(previous, 8, result as u32, word(previous, 0), &[]);
                }
            }
            let surface = word(this, 4) as *mut u8;
            if !surface.is_null() {
                let table = word(surface, 0);
                result = virtual_result(surface, 0x78, table, (result >> 32) as u32, &[255, 0, 255]);
                // The second call uses the table saved before the color conversion.
                let args = [result as u32];
                let mut call = crate::abi::RawCall::new(crate::abi::Conv::Thiscall,
                    word(table as *mut u8, 0x5c) as usize, surface as u32, &args);
                call.eax = result as u32;
                call.edx = (result >> 32) as u32;
                crate::abi::call_raw(&mut call);
                result = (u64::from(call.edx) << 32) | u64::from(call.eax);
            }
            let window = this.wrapping_sub(0xe0);
            virtual_result(window, 0x170, result as u32, word(window, 0), &[])
        }
    }
}

register_port! { fc4p_007a9810, 0x007a9810, "refresh_matching_filter", 4,
    refresh_matching_filter(this, entry) {
        // SAFETY: the message, filter ID array, and window base belong to the live owner.
        unsafe {
            let message = entry.arg(0) as *mut u8;
            let mut result = virtual_result(message, 0x10, word(message, 0), entry.edx, &[]);
            if result as u32 == 0x426840a0 {
                result = virtual_result(message, 0x24, result as u32, word(message, 0), &[]);
                let count = word(this, 0x44);
                result = (u64::from(count) << 32) | u64::from(result as u32);
                let mut index = 0i32;
                while index < count as i32 {
                    if result as u32 == word(this, 4 + index as usize * 4) {
                        result = address_result(0x007a93e0, crate::abi::Conv::Thiscall, this.wrapping_sub(0xdc) as u32, result as u32, count, &[]);
                        if result as u8 != 0 {
                            let window = this.wrapping_sub(0xd8);
                            result = virtual_result(window, 0x170, word(window, 0), (result >> 32) as u32, &[]);
                        }
                        break;
                    }
                    index = index.wrapping_add(1);
                }
            }
            result & !255
        }
    }
}

register_port! { fc4p_007bfbe0, 0x007bfbe0, "set_picture_zoom_limits", 8,
    set_picture_zoom_limits(this, entry) {
        // SAFETY: the picture view's two zoom limits and the game's comparison constants are live.
        unsafe {
            use crate::x87::F80;
            let first = entry.arg(0);
            let first_comparison = F80::from_f32(f32::from_bits(first)).compare(F80::from_f32(*minimum_picture_zoom()));
            put_word(this, 8, first);
            if first_comparison.less() {
                put_word(this, 8, 0x3e800000);
            }
            let second = entry.arg(1);
            let comparison = F80::from_f32(f32::from_bits(second)).compare(F80::from_f32(*maximum_picture_zoom()));
            put_word(this, 12, second);
            if !comparison.less() && !comparison.equal() && !comparison.unordered() {
                put_word(this, 12, 0x43800000);
            }
            (u64::from(second) << 32) | u64::from((first & 0xffff0000) | u32::from(comparison.status_word() & 0xff00) | 1)
        }
    }
}

port! {
    /// Returns atan2(-first, -second) in ST0 and leaves integer registers alone.
    #[port(address = 0x007f82f0, owner = "shared", name = "opposite_heading", naked)]
    pub unsafe extern "C" fn fc4p_007f82f0() {
        core::arch::naked_asm!("fld dword ptr [esp + 4]", "fchs", "fld dword ptr [esp + 8]", "fchs", "fpatan", "ret");
    }
}

register_port! { fc4p_00801810, 0x00801810, "destroy_virtual_base_reference", 0,
    destroy_virtual_base_reference(this, _entry) {
        // SAFETY: the virtual-base table supplies the displacement into this allocation.
        unsafe {
            let table = word(this, 4) as *mut u8;
            put_word(this, 0, 0x00abd4b4);
            let displacement = word(table, 4);
            put_word(this.wrapping_add(displacement as usize), 4, 0x00abd468);
            let second_table = word(this, 4);
            let displacement = word(second_table as *mut u8, 4);
            let value = displacement.wrapping_sub(0x2c);
            put_word(this.wrapping_add(displacement as usize), 0, value);
            let held = word(this, 8) as *mut u8;
            if held.is_null() {
                (u64::from(second_table) << 32) | u64::from(displacement)
            } else {
                let result = virtual_result(held, 8, displacement, word(held, 0), &[]);
                put_word(this, 8, 0);
                result
            }
        }
    }
}

register_port! { fc4p_0080b420, 0x0080b420, "next_vertex_record", 4,
    next_vertex_record(this, entry) {
        // SAFETY: the cursor describes an accessible source and the output holds one record.
        unsafe {
            let index = word(this, 0x28);
            if index as i32 >= word(this, 0x20) as i32 { return entry.result(index & !255); }
            let output = entry.arg(0) as *mut u8;
            if word(this, 0x1c) != 0 {
                let mut record = core::mem::MaybeUninit::<[u32; 3]>::uninit();
                let temporary = record.as_mut_ptr() as u32;
                let source = word(this, 4);
                let result = address_result(0x0080aef0, crate::abi::Conv::Cdecl, temporary, index, source, &[source, index, temporary]);
                let result = address_result(0x0080b090, crate::abi::Conv::Thiscall, this as u32, output as u32, (result >> 32) as u32, &[temporary, output as u32]);
                put_word(this, 0x28, word(this, 0x28).wrapping_add(1));
                (result & !255) | 1
            } else {
                let source = word(this, 4);
                let result = address_result(0x0080aef0, crate::abi::Conv::Cdecl, 0, source, output as u32, &[source, index, output as u32]);
                let next = word(this, 0x28).wrapping_add(1);
                put_word(this, 0x28, next);
                (result & 0xffffffff00000000) | u64::from((next & !255) | 1)
            }
        }
    }
}

register_port! { fc4p_0082aea0, 0x0082aea0, "query_draw_context_interface", 8,
    query_draw_context_interface(this, entry) {
        // SAFETY: the owner and any fallback interface expose QueryInterface and AddRef.
        unsafe {
            let id = entry.arg(0);
            let output = entry.arg(1) as *mut u8;
            if matches!(id, 1 | 0x20732180 | 0xab300b2b) {
                put_word(output, 0, if id == 0xab300b2b && !this.is_null() { this as u32 + 12 } else { this as u32 });
                let result = virtual_result(this, 4, word(this, 0), if id == 0x20732180 { entry.edx } else { output as u32 }, &[]);
                (result & !255) | 1
            } else {
                let fallback = word(this, 0x14) as *mut u8;
                if fallback.is_null() { return entry.result(id & !255); }
                virtual_result(fallback, 0, id, word(fallback, 0), &[id, output as u32])
            }
        }
    }
}

register_port! { fc4p_0082af70, 0x0082af70, "replace_surface_and_cache_geometry", 8,
    replace_surface_and_cache_geometry(this, entry) {
        // SAFETY: both retained interfaces and their geometry records belong to this holder.
        unsafe {
            let mut result = entry.result(entry.eax);
            for (offset, incoming) in [(4usize, entry.arg(0)), (8, entry.arg(1))] {
                let old = word(this, offset);
                if incoming != old {
                    if incoming != 0 { result = virtual_result(incoming as *mut u8, 4, word(incoming as *mut u8, 0), (result >> 32) as u32, &[]); }
                    put_word(this, offset, incoming);
                    if old != 0 { result = virtual_result(old as *mut u8, 8, result as u32, word(old as *mut u8, 0), &[]); }
                }
            }
            let surface = entry.arg(1) as *mut u8;
            result = virtual_result(surface, 0x30, word(surface, 0), (result >> 32) as u32, &[]);
            let geometry = result as u32 as *mut u8;
            for offset in [0usize, 4, 8, 12] { put_word(this, 12 + offset, word(geometry, offset)); }
            let mut format = core::mem::MaybeUninit::<[u32; 2]>::uninit();
            result = virtual_result(surface, 0x3c, format.as_mut_ptr() as u32, word(surface, 0), &[format.as_mut_ptr() as u32]);
            put_word(this, 0x20, word(result as u32 as *mut u8, 0));
            result = virtual_result(surface, 0x44, result as u32, word(surface, 0), &[]);
            put_word(this, 0x38, result as u32);
            put_word(this, 0x2c, word(this, 0x14).wrapping_sub(word(this, 0x0c)));
            put_word(this, 0x30, word(this, 0x18).wrapping_sub(word(this, 0x10)));
            let target = word(this, 4) as *mut u8;
            virtual_result(target, 0x44, word(this, 0x14).wrapping_sub(word(this, 0x0c)), word(target, 0), &[0])
        }
    }
}

register_port! { fc4p_0082b030, 0x0082b030, "refresh_surface_rectangle", 0,
    refresh_surface_rectangle(this, entry) {
        // SAFETY: surface rectangle reads and target callbacks use their original slots.
        unsafe {
            let surface = word(this, 8) as *mut u8;
            let result = virtual_result(surface, 0x30, word(surface, 0), entry.edx, &[]);
            let rectangle = result as u32 as *mut u8;
            let mut last = 0;
            for offset in [0usize, 4, 8, 12] { last = word(rectangle, offset); put_word(this, 12 + offset, last); }
            let target = word(this, 4) as *mut u8;
            virtual_result(target, 0x44, last, word(target, 0), &[0]);
            let target = word(this, 4) as *mut u8;
            let result = virtual_result(target, 0x44, word(target, 0), word(this, 8), &[word(this, 8)]);
            let target = word(this, 4) as *mut u8;
            virtual_result(target, 0xa8, word(target, 0), (result >> 32) as u32, &[])
        }
    }
}

register_port! { fc4p_0082b080, 0x0082b080, "set_surface_color_from_interface", 4,
    set_surface_color_from_interface(this, entry) {
        // SAFETY: QueryInterface initializes the optional retained color interface.
        unsafe {
            let source = entry.arg(0) as *mut u8;
            let mut color = 0u32;
            let mut result = virtual_result(source, 0, word(source, 0), &mut color as *mut u32 as u32, &[0x86d72b57, &mut color as *mut u32 as u32]);
            if result as u8 != 0 {
                let mut channels = core::mem::MaybeUninit::<[u8; 4]>::uninit();
                let p = channels.as_mut_ptr().cast::<u8>();
                result = virtual_result(color as *mut u8, 0x10, word(color as *mut u8, 0), p as u32, &[p as u32, p.add(1) as u32, p.add(2) as u32, p.add(3) as u32]);
                let pixel = if result as u8 != 0 {
                    let bytes = channels.assume_init();
                    u32::from_be_bytes([bytes[3], bytes[0], bytes[1], bytes[2]])
                } else { u32::MAX };
                let target = word(this, 4) as *mut u8;
                result = virtual_result(target, 0x54, word(target, 0), if result as u8 != 0 { pixel } else { (result >> 32) as u32 }, &[pixel]);
            } else {
                let target = word(this, 4) as *mut u8;
                result = virtual_result(target, 0x54, result as u32, word(target, 0), &[u32::MAX]);
            }
            if color != 0 { result = virtual_result(color as *mut u8, 8, word(color as *mut u8, 0), (result >> 32) as u32, &[]); }
            result
        }
    }
}

register_port! { fc4p_0082b120, 0x0082b120, "lock_draw_context", 4,
    lock_draw_context(this, entry) {
        // SAFETY: all calls use the holder's current target, which callbacks may replace.
        unsafe {
            if word(this, 0x28) == 0 {
                let target = word(this, 0x10) as *mut u8;
                let result = virtual_result(target, 0x40, word(target, 0), entry.edx, &[]);
                let mut dirty = 1u8;
                if result as u8 != 0 {
                    let target = word(this, 0x10) as *mut u8;
                    if virtual_result(target, 0xa4, result as u32, word(target, 0), &[]) as u8 == 0 { dirty = 0; }
                }
                *this.add(0x49) = dirty;
                let target = word(this, 0x10) as *mut u8;
                let result = virtual_result(target, 0x44, word(target, 0), word(this, 0x14), &[word(this, 0x14)]);
                let target = word(this, 0x10) as *mut u8;
                let result = virtual_result(target, 0xa4, word(target, 0), (result >> 32) as u32, &[]);
                if result as u8 != 0 {
                    let target = word(this, 0x10) as *mut u8;
                    virtual_result(target, 0xac, result as u32, word(target, 0), &[]);
                }
            }
            let flags = entry.arg(0) >> 15;
            let flags = ((flags & !255) | u32::from(!(flags as u8))) & 0xffffff01;
            let target = word(this, 0x10) as *mut u8;
            let result = virtual_result(target, 0xb8, word(target, 0), this.add(0x30) as u32, &[this.add(0x30) as u32, flags]);
            put_word(this, 0x28, word(this, 0x28).wrapping_add(1));
            (result & !255) | 1
        }
    }
}

register_port! { fc4p_0082b280, 0x0082b280, "fill_context_surface", 12,
    fill_context_surface(this, entry) {
        // SAFETY: the target implements begin, clip, fill, completion, and end slots.
        unsafe {
            let target = word(this, 0x10) as *mut u8;
            let mut result = virtual_result(target, 0x0c, word(target, 0), entry.edx, &[]);
            let clip = entry.arg(2);
            if clip != 0 {
                let target = word(this, 0x10) as *mut u8;
                result = virtual_result(target, 0x14, clip, word(target, 0), &[clip]);
            }
            let target = word(this, 0x10) as *mut u8;
            let table = word(target, 0) as *mut u8;
            let pixel = address_result(0x0082abf0, crate::abi::Conv::Thiscall, this as u32, entry.arg(0), (result >> 32) as u32, &[entry.arg(0)]);
            let args = [0, pixel as u32];
            let mut call = crate::abi::RawCall::new(crate::abi::Conv::Thiscall, word(table, 0x50) as usize, target as u32, &args);
            call.eax = pixel as u32;
            call.edx = (pixel >> 32) as u32;
            crate::abi::call_raw(&mut call);
            let target = word(this, 0x10) as *mut u8;
            let result = virtual_result(target, 0x8c, entry.arg(1), word(target, 0), &[entry.arg(1)]);
            let target = word(this, 0x10) as *mut u8;
            let result = virtual_result(target, 0x24, result as u32, word(target, 0), &[]);
            let target = word(this, 0x10) as *mut u8;
            (virtual_result(target, 0xa0, word(target, 0), (result >> 32) as u32, &[]) & !255) | 1
        }
    }
}

register_port! { fc4p_0082b2f0, 0x0082b2f0, "fill_rectangle_coordinates", 24,
    fill_rectangle_coordinates(this, entry) {
        // SAFETY: the slot consumes a four-word temporary rectangle before returning.
        unsafe {
            let mut rectangle = [entry.arg(1), entry.arg(2), entry.arg(3), entry.arg(4)];
            virtual_result(this, 0x50, word(this, 0), entry.arg(0), &[entry.arg(0), rectangle.as_mut_ptr() as u32, entry.arg(5)])
        }
    }
}

register_port! { fc4p_0082b690, 0x0082b690, "copy_surface_to_point", 16,
    copy_surface_to_point(this, entry) {
        // SAFETY: the point and source rectangle contain two and four words respectively.
        unsafe {
            let target = word(this, 0x10) as *mut u8;
            let mut result = virtual_result(target, 0x0c, word(target, 0), entry.edx, &[]);
            if entry.arg(3) != 0 {
                let target = word(this, 0x10) as *mut u8;
                result = virtual_result(target, 0x14, entry.arg(3), word(target, 0), &[entry.arg(3)]);
            }
            let interface = this.add(12);
            virtual_result(interface, 0x24, word(interface, 0), (result >> 32) as u32, &[entry.arg(0)]);
            let source = entry.arg(2) as *mut u8;
            let left = word(source, 0);
            let top = word(source, 4);
            let right = word(source, 8);
            let bottom = word(source, 12);
            let point = entry.arg(1) as *mut u8;
            let x = word(point, 0);
            let y = word(point, 4);
            let mut destination = [x, y, right.wrapping_add(x.wrapping_sub(left)), bottom.wrapping_add(y.wrapping_sub(top))];
            let target = word(this, 0x10) as *mut u8;
            let result = virtual_result(target, 0x98, destination.as_mut_ptr() as u32, word(target, 0), &[entry.arg(0), destination.as_mut_ptr() as u32, source as u32]);
            let target = word(this, 0x10) as *mut u8;
            let result = virtual_result(target, 0x24, result as u32, word(target, 0), &[]);
            let target = word(this, 0x10) as *mut u8;
            (virtual_result(target, 0xa0, word(target, 0), (result >> 32) as u32, &[]) & !255) | 1
        }
    }
}

register_port! { fc4p_0087931b, 0x0087931b, "query_scheduler_interface", 8,
    query_scheduler_interface(this, entry) {
        // SAFETY: the scheduler exposes the primary and its four-byte-adjusted interface.
        unsafe {
            let id = entry.arg(0);
            if !matches!(id, 1 | 101 | 0x23ea70a1) { return entry.result(id.wrapping_sub(0x23ea70a1) & !255); }
            let output = entry.arg(1) as *mut u8;
            put_word(output, 0, if id == 0x23ea70a1 && !this.is_null() { (this as u32).wrapping_add(4) } else { this as u32 });
            (virtual_result(this, 4, word(this, 0), if id == 0x23ea70a1 { output as u32 } else { entry.edx }, &[]) & !255) | 1
        }
    }
}

register_port! { fc4p_00879355, 0x00879355, "increment_locked_reference", 0,
    increment_locked_reference(this, entry) {
        // SAFETY: the lock helper initializes its two-word scope; its second word is the held lock.
        unsafe {
            let mut scope = core::mem::MaybeUninit::<[u32; 2]>::uninit();
            let locked = address_result(0x0048c9f0, crate::abi::Conv::Thiscall, scope.as_mut_ptr() as u32, this.add(0xe0) as u32, entry.edx, &[this.add(0xe0) as u32]);
            let held = word(scope.as_mut_ptr().cast(), 4) as *mut u8;
            let value = word(this, 0x80).wrapping_add(1);
            put_word(this, 0x80, value);
            let result = virtual_result(held, 8, word(held, 0), (locked >> 32) as u32, &[]);
            (result & 0xffffffff00000000) | u64::from(value)
        }
    }
}

register_port! { fc4p_00879383, 0x00879383, "decrement_locked_reference", 0,
    decrement_locked_reference(this, entry) {
        // SAFETY: the lock scope is released even when the unsigned reference count is at most one.
        unsafe {
            let mut scope = core::mem::MaybeUninit::<[u32; 2]>::uninit();
            let locked = address_result(0x0048c9f0, crate::abi::Conv::Thiscall, scope.as_mut_ptr() as u32, this.add(0xe0) as u32, entry.edx, &[this.add(0xe0) as u32]);
            let old = word(this, 0x80);
            let value = if old > 1 { let value = old.wrapping_sub(1); put_word(this, 0x80, value); value } else { 0 };
            let held = word(scope.as_mut_ptr().cast(), 4) as *mut u8;
            let result = virtual_result(held, 8, word(held, 0), (locked >> 32) as u32, &[]);
            (result & 0xffffffff00000000) | u64::from(value)
        }
    }
}

register_port! { fc4p_00891cae, 0x00891cae, "apply_cursor_if_enabled", 4,
    apply_cursor_if_enabled(this, entry) {
        // SAFETY: the cursor owner, optional provider, and imported function slot are live game data.
        unsafe {
            if entry.arg(0) as u8 == 0 { return entry.result(entry.eax); }
            let mut result = entry.result(entry.eax);
            let mut coordinates = None;
            let has_handle = word(this, 0x2c) != 0;
            if has_handle {
                let provider = word(this, 12) as *mut u8;
                if *this.add(8) == 0 && !provider.is_null() {
                    result = virtual_result(provider, 0x80, word(provider, 0), entry.edx, &[]);
                    if result as u8 == 0 { coordinates = Some((0u32, 0u32)); }
                }
                if coordinates.is_none() {
                    let args = [word(this, 0x2c)];
                    let mut call = crate::abi::RawCall::new(crate::abi::Conv::Stdcall, *set_cursor_function(), this as u32, &args);
                    call.eax = result as u32;
                    call.edx = (result >> 32) as u32;
                    crate::abi::call_raw(&mut call);
                    return (u64::from(call.edx) << 32) | u64::from(call.eax);
                }
            } else if word(this, 12) != 0 {
                coordinates = Some((read_unaligned(this.add(0x28).cast::<i16>()) as i32 as u32, read_unaligned(this.add(0x2a).cast::<i16>()) as i32 as u32));
            }
            if let Some((x, y)) = coordinates {
                let provider = word(this, 12) as *mut u8;
                result = virtual_result(provider, 0x7c, word(provider, 0), if has_handle { (result >> 32) as u32 } else { x }, &[word(this, 0x24), x, y]);
            }
            result
        }
    }
}

register_port! { fc4p_008922f0, 0x008922f0, "destroy_cursor_owner", 0,
    destroy_cursor_owner(this, entry) {
        // SAFETY: reset ownership before releasing the retained cursor and embedded string.
        unsafe {
            put_word(this, 0, 0x00ac52d4);
            let mut result = entry.result(entry.eax);
            if word(this, 0x2c) != 0 { result = address_result(0x00891d48, crate::abi::Conv::Thiscall, this as u32, result as u32, (result >> 32) as u32, &[0]); }
            let held = word(this, 0x24) as *mut u8;
            if !held.is_null() {
                result = virtual_result(held, 8, word(held, 0), (result >> 32) as u32, &[]);
                put_word(this, 0x24, 0);
            }
            address_result(0x00414250, crate::abi::Conv::Thiscall, this.add(0x10) as u32, result as u32, (result >> 32) as u32, &[])
        }
    }
}

game_fn! { #[allow(dead_code)] fn base_port_usage(this: *mut u8, id: u32, usage: *mut u32, capacity: *mut u32) -> u64 = "thiscall" 0x006e9770; }
game_fn! { #[allow(dead_code)] fn find_port_record(this: *mut u8, id: u32, output: *mut u32) -> u64 = "thiscall" 0x006e9590; }
game_data! { static funding_zero: f32 = 0x00a81054; }
game_data! { static unsigned_word_bias: f32 = 0x00a80aa8; }

register_port! { fc4p_00700fe0, 0x00700fe0, "funded_port_usage", 12,
    funded_port_usage(this, entry) {
        // SAFETY: the base query initializes the two caller-owned output words on success.
        unsafe {
            use crate::x87::F80;
            let usage = entry.arg(1) as *mut u8;
            let capacity = entry.arg(2) as *mut u8;
            let mut result = address_result(0x006e9770, crate::abi::Conv::Thiscall,
                this.add(4) as u32, entry.arg(0), entry.edx,
                &[entry.arg(0), usage as u32, capacity as u32]);
            let accepted = result as u8;
            if accepted == 0 { return result; }
            let department = word(this, 0x34) as *mut u8;
            result = virtual_result(department, 0x50, result as u32, word(department, 0), &[1]);
            let item = result as u32 as *mut u8;
            if !item.is_null() {
                result = virtual_result(item, 0x2c, word(item, 0), (result >> 32) as u32, &[]);
                let current = F80::from_i64(result as i64).to_f32();
                result = virtual_result(item, 0x28, result as u32, word(item, 0), &[]);
                let full = F80::from_i64(result as i64);
                let compared = full.compare(F80::from_f32(*funding_zero()));
                if !compared.less() && !compared.equal() && !compared.unordered() {
                    let amount = word(capacity, 0);
                    let mut amount_float = F80::from_i32(amount as i32);
                    if (amount as i32) < 0 { amount_float = amount_float + F80::from_f32(*unsigned_word_bias()); }
                    result = ((amount_float / full) * F80::from_f32(current)).ftol() as u64;
                    put_word(capacity, 0, result as u32);
                }
            }
            let limit = word(capacity, 0);
            if word(usage, 0) > limit { put_word(usage, 0, limit); }
            (result & 0xffffffff00000000) | u64::from((limit & !255) | u32::from(accepted))
        }
    }
}

register_port! { fc4p_00701830, 0x00701830, "funded_port_maintenance_cost", 4,
    funded_port_maintenance_cost(this, entry) {
        // SAFETY: a successful lookup identifies the owner's 32-byte stage record.
        unsafe {
            use crate::x87::F80;
            let mut record = 0u32;
            let result = address_result(0x006e9590, crate::abi::Conv::Thiscall,
                this.add(4) as u32, &mut record as *mut u32 as u32, entry.edx,
                &[entry.arg(0), &mut record as *mut u32 as u32]);
            let mut cost = 0u64;
            if result as u8 != 0 {
                let index = word(record as *mut u8, 4).wrapping_shl(5);
                let owner = word(record as *mut u8, 8) as *mut u8;
                let table = word(owner, 0x14).wrapping_add(index) as *mut u8;
                cost = (u64::from(word(table, 12)) << 32) | u64::from(word(table, 8));
            }
            let department = word(this, 0x34) as *mut u8;
            let result = virtual_result(department, 0x50, result as u32, word(department, 0), &[1]);
            let item = result as u32 as *mut u8;
            if !item.is_null() {
                let result = virtual_result(item, 0x2c, word(item, 0), (result >> 32) as u32, &[]);
                let current = F80::from_i64(result as i64).to_f32();
                let result = virtual_result(item, 0x28, result as u32, word(item, 0), &[]);
                let full = F80::from_i64(result as i64);
                let compared = full.compare(F80::from_f32(*funding_zero()));
                if !compared.less() && !compared.equal() && !compared.unordered() {
                    return ((F80::from_i64(cost as i64) / full) * F80::from_f32(current)).ftol() as u64;
                }
            }
            cost
        }
    }
}

register_port! { fc4p_00745980, 0x00745980, "initialize_word_deque", 4,
    initialize_word_deque(this, entry) {
        // SAFETY: each allocated block holds 32 words; the map is centered around its live blocks.
        unsafe {
            let size = entry.arg(0);
            let blocks = (size >> 5).wrapping_add(1);
            let map_size = blocks.wrapping_add(2).max(8);
            put_word(this, 0x24, map_size);
            let allocation = allocate_shared(map_size.wrapping_shl(2));
            let map = allocation as u32;
            let first = map.wrapping_add(word(this, 0x24).wrapping_sub(blocks).wrapping_shr(1).wrapping_shl(2));
            let limit = first.wrapping_add(blocks.wrapping_shl(2));
            put_word(this, 0x20, map);
            let mut cursor = first;
            while cursor < limit {
                let block = allocate_shared(128) as u32;
                put_word(cursor as *mut u8, 0, block);
                cursor = cursor.wrapping_add(4);
            }
            put_word(this, 0x0c, first);
            let first_block = word(first as *mut u8, 0);
            put_word(this, 4, first_block);
            put_word(this, 8, first_block.wrapping_add(128));
            let last = limit.wrapping_sub(4);
            put_word(this, 0x1c, last);
            let last_block = word(last as *mut u8, 0);
            put_word(this, 0x14, last_block);
            put_word(this, 0x18, last_block.wrapping_add(128));
            put_word(this, 0, word(this, 4));
            let remainder = entry.arg(0) & 31;
            let finish = word(this, 0x14).wrapping_add(remainder * 4);
            put_word(this, 0x10, finish);
            entry.ecx = word(this, 0x14);
            (u64::from(finish) << 32) | u64::from(remainder)
        }
    }
}

game_fn! { #[allow(dead_code)] fn move_byte_range(output: u32, source: u32, count: u32) -> u64 = "C" 0x009eeb70; }
game_fn! { #[allow(dead_code)] fn copy_byte_range(first: u32, last: u32, output: u32) -> u64 = "C" 0x006d6880; }
game_fn! { #[allow(dead_code)] fn copy_seven_word_range(first: u32, last: u32, output: u32, stop: *mut u8) -> u64 = "C" 0x008042b0; }
game_fn! { #[allow(dead_code)] fn append_listener(this: *mut u8, value: *const u32) -> u64 = "thiscall" 0x007ebd30; }
game_fn! { #[allow(dead_code)] fn erase_listener_range(this: *mut u8, first: u32, last: u32) -> u64 = "thiscall" 0x0040e490; }
game_fn! { #[allow(dead_code)] fn flush_listener_changes(this: *mut u8) -> u64 = "thiscall" 0x00473ec0; }

/// A game call with all three volatile registers available to its caller.
unsafe fn shared_direction_call(address: u32, conv: crate::abi::Conv, eax: u32, ecx: u32, edx: u32, args: &[u32], backwards: bool) -> crate::abi::RawCall {
    let mut call = crate::abi::RawCall::new(conv, crate::abi::target(address), ecx, args);
    call.eax = eax;
    call.edx = edx;
    call.take_st0 = u32::from(backwards);
    // SAFETY: the address is declared above and callers supply its convention and arguments.
    unsafe {
        core::arch::asm!(
            "mov ecx, [edi + 20]", "mov eax, [edi + 16]",
            "2:", "test ecx, ecx", "jz 3f", "dec ecx", "push dword ptr [eax + ecx*4]", "jmp 2b",
            "3:", "mov eax, [edi + 4]", "mov ecx, [edi + 8]", "mov edx, [edi + 12]",
            "cmp dword ptr [edi + 28], 0", "je 5f", "std", "5:",
            "call dword ptr [edi]", "pushfd", "pop dword ptr [edi + 28]", "cld",
            "mov [edi + 4], eax", "mov [edi + 8], ecx", "mov [edi + 12], edx",
            "cmp dword ptr [edi + 24], 0", "je 4f", "mov eax, [edi + 20]", "lea esp, [esp + eax*4]",
            "4:", in("edi") &mut call, clobber_abi("C"),
        );
    }
    call
}

// REP word transfers follow the incoming direction flag. Rust itself runs with it clear.
#[repr(C)]
struct DirectionEntry {
    edx: u32,
    eax: u32,
    ecx: u32,
    flags: u32,
    return_address: u32,
}
impl DirectionEntry {
    unsafe fn arg(&self, index: usize) -> u32 {
        // SAFETY: the wrapper saved the five words before the original arguments.
        unsafe { (self as *const Self).cast::<u32>().add(5 + index).read_unaligned() }
    }
}
macro_rules! direction_register_port {
    ($export:ident, $address:literal, $name:literal, $pop:literal,
     $body:ident($this:ident, $entry:ident) $code:block) => {
        port! {
            #[port(address = $address, owner = "shared", name = $name, naked)]
            pub unsafe extern "C" fn $export() {
                core::arch::naked_asm!(
                    "pushfd", "cld", "push ecx", "push eax", "push edx", "mov edx, esp", "call {body}",
                    "add esp, 8", "pop ecx", "popfd", "ret {pop}", body = sym $body, pop = const $pop,
                );
            }
        }
        unsafe extern "fastcall" fn $body($this: *mut u8, saved: *mut DirectionEntry) -> u64 {
            // SAFETY: the wrapper saves these registers and flags before the caller's argument words.
            let $entry = unsafe { &mut *saved };
            $code
        }
    };
}

direction_register_port! { fc4p_006d6a00, 0x006d6a00, "insert_byte_vector_range", 16,
    insert_byte_vector_range(this, entry) {
        // SAFETY: all three vector pointers and the supplied source range are live game storage.
        unsafe {
            use crate::abi::Conv::Cdecl;
            let position = entry.arg(0);
            let source = entry.arg(1);
            let last = entry.arg(2);
            let count = last.wrapping_sub(source);
            if source == last { return (u64::from(source) << 32) | u64::from(entry.eax); }
            let mut backwards = entry.flags & 0x400 != 0;
            let end = word(this, 4);
            let spare = word(this, 8).wrapping_sub(end);
            if spare >= count {
                let tail = end.wrapping_sub(position);
                let call = if tail > count {
                    let start = end.wrapping_sub(count);
                    let mut src = start;
                    let mut dst = end;
                    let mut remaining = count as i32;
                    while remaining > 0 {
                        *(dst as *mut u8) = *(src as *const u8);
                        src = src.wrapping_add(1); dst = dst.wrapping_add(1); remaining -= 1;
                    }
                    put_word(this, 4, word(this, 4).wrapping_add(count));
                    let moving = start.wrapping_sub(position);
                    let mut edx = if count as i32 > 0 { dst } else { count };
                    let mut ecx = count;
                    if moving as i32 > 0 {
                        let moved = shared_direction_call(0x009eeb70, Cdecl, end.wrapping_sub(moving), ecx, edx,
                            &[end.wrapping_sub(moving), position, moving], backwards);
                        edx = moved.edx; ecx = moved.ecx;
                        backwards = moved.take_st0 & 0x400 != 0;
                    }
                    shared_direction_call(0x006d6880, Cdecl, source, ecx, edx, &[source, last, position], backwards)
                } else {
                    let split = source.wrapping_add(tail);
                    let mut src = split;
                    let mut dst = end;
                    let mut remaining = last.wrapping_sub(split) as i32;
                    while remaining > 0 {
                        *(dst as *mut u8) = *(src as *const u8);
                        src = src.wrapping_add(1); dst = dst.wrapping_add(1); remaining -= 1;
                    }
                    let finish = word(this, 4).wrapping_add(count.wrapping_sub(tail));
                    put_word(this, 4, finish);
                    let mut dst = finish;
                    let mut src = position;
                    let mut remaining = end.wrapping_sub(position) as i32;
                    while remaining > 0 {
                        *(dst as *mut u8) = *(src as *const u8);
                        src = src.wrapping_add(1); dst = dst.wrapping_add(1); remaining -= 1;
                    }
                    put_word(this, 4, word(this, 4).wrapping_add(tail));
                    shared_direction_call(0x006d6880, Cdecl, 0, split, source, &[source, split, position], backwards)
                };
                entry.ecx = call.ecx;
                entry.flags = (entry.flags & !0x400) | (call.take_st0 & 0x400);
                return (u64::from(call.edx) << 32) | u64::from(call.eax);
            }
            let size = end.wrapping_sub(word(this, 0));
            let capacity = size.wrapping_add(size.max(count));
            let mut allocation = 0;
            if capacity != 0 {
                let choice = if size >= count { &size } else { &count };
                let call = shared_direction_call(0x0090cf54, Cdecl, size, choice as *const u32 as u32, source, &[capacity], backwards);
                allocation = call.eax; backwards = call.take_st0 & 0x400 != 0;
            }
            let mut src = word(this, 0);
            let mut dst = allocation;
            let mut remaining = position.wrapping_sub(src) as i32;
            let mut ecx = remaining as u32;
            while remaining > 0 {
                ecx = (ecx & !255) | u32::from(*(src as *const u8));
                *(dst as *mut u8) = ecx as u8;
                src = src.wrapping_add(1); dst = dst.wrapping_add(1); remaining -= 1;
            }
            let mut edx = src;
            if count as i32 > 0 {
                ecx = source.wrapping_sub(dst);
                let mut remaining = count;
                while remaining != 0 {
                    edx = (edx & !255) | u32::from(*(ecx.wrapping_add(dst) as *const u8));
                    *(dst as *mut u8) = edx as u8;
                    dst = dst.wrapping_add(1); remaining -= 1;
                }
            }
            ecx = word(this, 4).wrapping_sub(position);
            if ecx as i32 > 0 {
                edx = position.wrapping_sub(dst);
                while ecx != 0 {
                    *(dst as *mut u8) = *(edx.wrapping_add(dst) as *const u8);
                    dst = dst.wrapping_add(1); ecx -= 1;
                }
            }
            let old = word(this, 0);
            if old != 0 {
                let call = shared_direction_call(0x0090cf63, Cdecl, old, ecx, edx, &[old], backwards);
                ecx = call.ecx; edx = call.edx; backwards = call.take_st0 & 0x400 != 0;
            }
            put_word(this, 0, allocation); put_word(this, 4, dst);
            let end = allocation.wrapping_add(capacity);
            put_word(this, 8, end);
            entry.ecx = ecx;
            entry.flags = (entry.flags & !0x400) | (u32::from(backwards) << 10);
            (u64::from(edx) << 32) | u64::from(end)
        }
    }
}

direction_register_port! { fc4p_00748f20, 0x00748f20, "grow_seven_word_vector", 20,
    grow_seven_word_vector(this, entry) {
        // SAFETY: the caller supplies complete 28-byte records and a vector header.
        unsafe {
            use crate::abi::Conv::Cdecl;
            let position = entry.arg(0);
            let source = entry.arg(1);
            let count = entry.arg(3);
            let size = (word(this, 4).wrapping_sub(word(this, 0)) as i32 / 28) as u32;
            let capacity = size.wrapping_add(size.max(count));
            let mut backwards = entry.flags & 0x400 != 0;
            let mut ecx = if size < count {
                (entry as *mut DirectionEntry).cast::<u32>().add(8) as u32
            } else { &size as *const u32 as u32 };
            let mut allocation = 0;
            if capacity != 0 {
                let call = shared_direction_call(0x0090cf54, Cdecl, capacity.wrapping_mul(28), ecx, 0,
                    &[capacity.wrapping_mul(28)], backwards);
                allocation = call.eax; ecx = call.ecx; backwards = call.take_st0 & 0x400 != 0;
            }
            let step = if backwards { 0xfffffffcu32 } else { 4 };
            let mut output = allocation;
            let mut src = word(this, 0);
            let mut edx = src;
            while src != position {
                if output != 0 {
                    for i in 0u32..7 {
                        let offset = i.wrapping_mul(step);
                        put_word(output.wrapping_add(offset) as *mut u8, 0, word(src.wrapping_add(offset) as *mut u8, 0));
                    }
                }
                src = src.wrapping_add(28); output = output.wrapping_add(28); edx = src; ecx = position;
            }
            for _ in 0..count {
                if output != 0 {
                    for i in 0u32..7 {
                        let offset = i.wrapping_mul(step);
                        put_word(output.wrapping_add(offset) as *mut u8, 0, word(source.wrapping_add(offset) as *mut u8, 0));
                    }
                    ecx = 0;
                }
                output = output.wrapping_add(28);
                if count != 1 { edx = 0; }
            }
            // The original stop byte is an argument word and is also passed to the range helper.
            let stop = (entry as *mut DirectionEntry).cast::<u32>().add(9).cast::<u8>();
            if *stop == 0 {
                let call = shared_direction_call(0x008042b0, Cdecl, position, stop as u32, word(this, 4),
                    &[position, word(this, 4), output, stop as u32], backwards);
                output = call.eax; edx = call.edx; ecx = call.ecx; backwards = call.take_st0 & 0x400 != 0;
            }
            let old = word(this, 0);
            if old != 0 {
                let call = shared_direction_call(0x0090cf63, Cdecl, old, ecx, edx, &[old], backwards);
                edx = call.edx; ecx = call.ecx; backwards = call.take_st0 & 0x400 != 0;
            }
            put_word(this, 4, output);
            put_word(this, 8, allocation.wrapping_add(capacity.wrapping_mul(28)));
            put_word(this, 0, allocation);
            entry.ecx = ecx;
            entry.flags = (entry.flags & !0x400) | (u32::from(backwards) << 10);
            (u64::from(edx) << 32) | u64::from(allocation)
        }
    }
}

layout! {
    /// Scheduler listener lists, nesting counts, and iteration flags.
    struct SharedScheduler {
        0xa8 => cycle_list / set_cycle_list: *mut u8,
        0xb0 => frame_list / set_frame_list: *mut u8,
        0xbc => cycle_period / set_cycle_period: u32,
        0xc0 => frame_number / set_frame_number: u32,
        0xc4 => cycle_number / set_cycle_number: u32,
        0xd2 => copying_cycles / set_copying_cycles: u8,
        0xd3 => copying_frames / set_copying_frames: u8,
        0xd8 => cycle_depth / set_cycle_depth: u32,
        0xdc => frame_depth / set_frame_depth: u32,
    }
}

register_port! { fc4p_0087a74b, 0x0087a74b, "dispatch_frame_listeners", 4,
    dispatch_frame_listeners(this, entry) {
        // SAFETY: the scheduler owns its circular list and the listener interfaces in it.
        unsafe {
            let scheduler = SharedScheduler::from_ptr(this);
            let lock = this.add(0xe0);
            let mut result = virtual_result(lock, 4, word(lock, 0), entry.edx, &[]);
            let header = scheduler.frame_list();
            scheduler.set_frame_depth(scheduler.frame_depth().wrapping_add(1));
            let mut snapshot = [0u32; 3];
            scheduler.set_copying_frames(1);
            let mut node = word(header, 0) as *mut u8;
            while node != header {
                let value = word(node, 8);
                result = address_result(0x007ebd30, crate::abi::Conv::Thiscall, snapshot.as_mut_ptr() as u32,
                    &value as *const u32 as u32, (result >> 32) as u32, &[&value as *const u32 as u32]);
                node = word(node, 0) as *mut u8;
            }
            scheduler.set_copying_frames(0);
            result = virtual_result(lock, 8, word(lock, 0), (result >> 32) as u32, &[]);
            let mut cursor = snapshot[0];
            let end = snapshot[1];
            while cursor != end {
                let listener = word(cursor as *mut u8, 0) as *mut u8;
                result = virtual_result(listener, 0x28, word(listener, 0), (result >> 32) as u32, &[entry.arg(0)]);
                cursor = cursor.wrapping_add(4);
            }
            result = address_result(0x0040e490, crate::abi::Conv::Thiscall, snapshot.as_mut_ptr() as u32,
                result as u32, (result >> 32) as u32, &[snapshot[0], end]);
            if scheduler.frame_depth() == 1 {
                result = address_result(0x00473ec0, crate::abi::Conv::Thiscall, this.add(0xb4) as u32,
                    result as u32, (result >> 32) as u32, &[]);
            }
            scheduler.set_frame_depth(scheduler.frame_depth().wrapping_sub(1));
            if snapshot[0] != 0 {
                result = address_result(0x0090cf63, crate::abi::Conv::Cdecl, snapshot.as_mut_ptr() as u32,
                    result as u32, (result >> 32) as u32, &[snapshot[0]]);
            }
            result
        }
    }
}

register_port! { fc4p_0087a812, 0x0087a812, "dispatch_scheduler_cycle", 0,
    dispatch_scheduler_cycle(this, _entry) {
        // SAFETY: the scheduler exposes its primary interface, embedded lock, and live listener list.
        unsafe {
            let scheduler = SharedScheduler::from_ptr(this);
            let frame = scheduler.frame_number();
            scheduler.set_frame_number(frame.wrapping_add(1));
            let result = virtual_result(this, 0x3c, frame, word(this, 0), &[frame]);
            let lock = this.add(0xe0);
            virtual_result(lock, 4, word(lock, 0), (result >> 32) as u32, &[]);
            let frame = scheduler.frame_number();
            let period = scheduler.cycle_period();
            // The CPU raises division-by-zero for a zero period.
            let (quotient, remainder): (u32, u32);
            core::arch::asm!("div {period:e}", period = in(reg) period,
                inlateout("eax") frame => quotient, inlateout("edx") 0u32 => remainder,
                options(nomem, nostack));
            scheduler.set_cycle_depth(scheduler.cycle_depth().wrapping_add(1));
            let mut result;
            if remainder == 0 {
                let header = scheduler.cycle_list();
                scheduler.set_cycle_number(scheduler.cycle_number().wrapping_add(1));
                let cycle = scheduler.cycle_number();
                let mut snapshot = [0u32; 3];
                result = 0;
                scheduler.set_copying_cycles(1);
                let mut node = word(header, 0) as *mut u8;
                while node != header {
                    let value = word(node, 8);
                    result = address_result(0x007ebd30, crate::abi::Conv::Thiscall, snapshot.as_mut_ptr() as u32,
                        &value as *const u32 as u32, (result >> 32) as u32, &[&value as *const u32 as u32]);
                    node = word(node, 0) as *mut u8;
                }
                scheduler.set_copying_cycles(0);
                result = virtual_result(lock, 8, word(lock, 0), (result >> 32) as u32, &[]);
                let mut cursor = snapshot[0];
                let end = snapshot[1];
                while cursor != end {
                    let listener = word(cursor as *mut u8, 0) as *mut u8;
                    result = virtual_result(listener, 0x2c, word(listener, 0), (result >> 32) as u32, &[cycle]);
                    cursor = cursor.wrapping_add(4);
                }
                result = address_result(0x0040e490, crate::abi::Conv::Thiscall, snapshot.as_mut_ptr() as u32,
                    result as u32, (result >> 32) as u32, &[snapshot[0], end]);
                if snapshot[0] != 0 {
                    result = address_result(0x0090cf63, crate::abi::Conv::Cdecl, snapshot.as_mut_ptr() as u32,
                        result as u32, (result >> 32) as u32, &[snapshot[0]]);
                }
            } else {
                let _ = quotient;
                result = virtual_result(lock, 8, word(lock, 0), remainder, &[]);
            }
            if scheduler.cycle_depth() == 1 {
                result = address_result(0x00473ec0, crate::abi::Conv::Thiscall, this.add(0xac) as u32,
                    result as u32, (result >> 32) as u32, &[]);
            }
            scheduler.set_cycle_depth(scheduler.cycle_depth().wrapping_sub(1));
            result
        }
    }
}

game_data! { static basis_one: f32 = 0x00b0ec64; }
game_data! { static model_initial_word: u32 = 0x00b4e65c; }
game_data! { static model_virtual_bases: [u32; 2] = 0x00abd284; }
game_data! { static model_node_pool: *mut u8 = 0x00b0e340; }
game_data! { static model_pool_service: u32 = 0x00b0e33c; }
game_fn! { #[allow(dead_code)] fn construct_model_base(this: *mut u8, complete: u32) -> u64 = "thiscall" 0x00801cf0; }
game_fn! { #[allow(dead_code)] fn grow_model_node_pool(this: *mut u8) -> u64 = "thiscall" 0x0090cf9a; }
game_fn! { #[allow(dead_code)] fn allocate_model_words(bytes: u32) -> u64 = "C" 0x005e55e0; }
game_fn! { #[allow(dead_code, clippy::too_many_arguments)] fn pack_model_render_state(this: *mut u8, a: u32, b: u32, c: u32, d: u32, e: u32, f: u32, g: u32, h: u32, i: u32, j: u32, k: u32, l: u32, m: u32, n: u32) -> u64 = "thiscall" 0x007d2b50; }

unsafe fn model_virtual_offset(this: *mut u8) -> u32 {
    // SAFETY: the model has a valid virtual-base table. Tests substitute the fixed game table's storage.
    unsafe {
        let table = word(this, 4);
        let table = if table == 0x00abd284 { model_virtual_bases().cast::<u8>() } else { table as *mut u8 };
        word(table, 4)
    }
}

direction_register_port! { fc4p_007fdcb0, 0x007fdcb0, "construct_render_model_instance", 4,
    construct_render_model_instance(this, entry) {
        // SAFETY: the caller supplies a complete model or an initialized virtual-base table for its subobject.
        unsafe {
            use crate::abi::Conv::{Cdecl, Thiscall};
            let mut backwards = entry.flags & 0x400 != 0;
            if entry.arg(0) != 0 {
                put_word(this, 4, 0x00abd284); put_word(this, 12, 0x00abd27c);
                put_word(this, 0x170, 0x00aa1f38);
            }
            let table = word(this, 4);
            put_word(this, 0, 0x00abd168);
            let offset = model_virtual_offset(this);
            put_word(this.wrapping_add(offset as usize), 4, 0x00abd118);
            let call = shared_direction_call(0x00801cf0, Thiscall, table, this.add(8) as u32, entry.edx, &[0], backwards);
            backwards = call.take_st0 & 0x400 != 0;
            put_word(this, 0, 0x00abd220); put_word(this, 8, 0x00abd21c);
            let offset = model_virtual_offset(this);
            put_word(this.wrapping_add(offset as usize), 4, 0x00abd1d0);
            let offset = model_virtual_offset(this);
            put_word(this.wrapping_add(offset as usize), 0, offset.wrapping_sub(0x16c));
            put_word(this, 0x34, 0); *this.add(0x38) = 0; *this.add(0x39) = 0;
            for offset in (0x3c..=0x68).step_by(4) { put_word(this, offset, *model_initial_word()); }
            for (offset, value) in [
                (0x48,0), (0x54,0), (0x3c,0x3f800000), (0x40,0), (0x4c,0x3f800000),
                (0x58,0), (0x44,0), (0x50,0), (0x5c,0x3f800000), (0x6c,0x3f800000),
                (0x60,0), (0x64,0), (0x68,0),
            ] { put_word(this, offset, value); }
            *this.add(0x39) = 0; *this.add(0x38) = 0;
            for (offset, value) in [(0x70,0), (0x74,0), (0x78,0), (0x7c,0x3f800000), (0x80,0x3f800000), (0x84,0x3f800000)] { put_word(this, offset, value); }
            *this.add(0x88) = 255; *this.add(0x89) = 255;
            for offset in [0x8c,0x90,0x94,0x98] { put_word(this, offset, 0); }
            for (offset, value) in [(0x9c,1), (0x9d,1), (0x9e,255), (0x9f,0)] { *this.add(offset) = value; }
            put_word(this, 0xa0, 0x3a83126f); put_word(this, 0xa4, 0);
            for (offset, value) in [(0xa8,3), (0xa9,1), (0xaa,0)] { *this.add(offset) = value; }
            put_word(this, 0xac, 0); put_word(this, 0xc4, 0);
            for offset in (0x13c..=0x168).step_by(4) { put_word(this, offset, *model_initial_word()); }
            let mut edx = *model_initial_word();
            put_word(this, 0x1c, 1);
            for i in 0..5 {
                let mut node = word(*model_node_pool(), 8) as *mut u8;
                if node.is_null() {
                    let call = shared_direction_call(0x0090cf9a, Thiscall, 0, model_pool_service() as u32, edx, &[], backwards);
                    node = call.eax as *mut u8;
                    backwards = call.take_st0 & 0x400 != 0;
                }
                let next = word(node, 0);
                let pool = *model_node_pool();
                put_word(pool, 8, next);
                let pool = *model_node_pool();
                put_word(pool, 12, word(pool, 12).wrapping_add(1));
                put_word(node, 0, 0); put_word(node, 4, 0);
                put_word(this, 0xc8 + i * 4, node as u32);
                edx = pool as u32;
            }
            let call = shared_direction_call(0x005e55e0, Cdecl, 0, *model_node_pool() as u32, edx, &[20], backwards);
            backwards = call.take_st0 & 0x400 != 0;
            let allocation = call.eax as *mut u8;
            put_word(this, 0x94, allocation as u32);
            for offset in (0..20).step_by(4) { put_word(allocation, offset, 0); }
            let step = if backwards { 0xfffffffcu32 } else { 4 };
            let mut output = this.add(0xdc) as u32;
            for _ in 0..24 { put_word(output as *mut u8, 0, 0); output = output.wrapping_add(step); }
            for i in 0..4 {
                let p = this.add(0xea + i * 24);
                *p.add(1) = 255; *p = 255; *p.sub(1) = 255; *p.sub(2) = 255;
            }
            let args = [1,1,1,1,1,1,0,3,4,5,4,0x3f000000,1,u32::MAX];
            let call = shared_direction_call(0x007d2b50, Thiscall, this.add(0x14a) as u32,
                this.add(0x8c) as u32, call.edx, &args, backwards);
            entry.ecx = call.ecx;
            entry.flags = (entry.flags & !0x400) | (call.take_st0 & 0x400);
            (u64::from(call.edx) << 32) | u64::from(this as u32)
        }
    }
}

game_fn! {
    #[allow(dead_code)]
    fn reference_release_exception_handler(record: u32, frame: u32, context: u32, dispatcher: u32) -> u32 = "C" 0x00a6f090;
}

port! {
    /// Release a held interface under the game's exception frame without clearing the holder.
    #[port(address = 0x007ce1a0, owner = "shared", name = "release_held_interface", naked)]
    pub unsafe extern "C" fn fc4p_007ce1a0() {
        // The game's handler uses EBP and the saved stack at -0x10. Keep that ABI frame so its
        // existing cleanup continuation can restore the caller if Release raises an exception.
        core::arch::naked_asm!(
            "push ebp", "mov ebp, esp", "push -1", "push 0x00a6f090",
            "mov eax, fs:[0]", "push eax", "mov fs:[0], esp", "push ecx",
            "push ebx", "push esi", "push edi", "mov [ebp - 0x10], esp",
            "push edx", "lea edx, [ebp - 0xc]", "call {body}", "add esp, 4",
            "mov dword ptr [ebp - 4], -1", "mov ecx, [ebp - 0xc]", "mov fs:[0], ecx",
            "pop edi", "pop esi", "pop ebx", "mov esp, ebp", "pop ebp", "ret",
            body = sym release_held_interface,
        );
    }
}

unsafe extern "fastcall" fn release_held_interface(this: *mut u8, frame: *mut u32) -> u64 {
    // SAFETY: the ABI entry builds the game's registration frame and supplies a live holder.
    unsafe {
        let entry_edx = frame.sub(5).read();
        if word(this, 0) == 0 { return (u64::from(entry_edx) << 32) | u64::from(frame.read()); }
        frame.add(2).write(0);
        let held = word(this, 0) as *mut u8;
        virtual_result(held, 8, word(held, 0), entry_edx, &[])
    }
}

/// Loads one float field without imposing Rust alignment requirements.
unsafe fn shared_float(pointer: *mut u8, offset: usize) -> crate::x87::F80 {
    // SAFETY: callers provide the live field at its original byte offset.
    crate::x87::F80::from_f32(f32::from_bits(unsafe { word(pointer, offset) }))
}

register_port! { fc4p_007f8170, 0x007f8170, "transform_axis_aligned_bounds", 0,
    transform_axis_aligned_bounds(_this, entry) {
        // SAFETY: the transform has fourteen floats; the six bounds may overlap it.
        unsafe {
            use crate::x87::F80;
            let matrix = entry.arg(0) as *mut u8;
            let bounds = entry.arg(1) as *mut u8;
            let scale = shared_float(matrix, 0x34);
            let mut scaled = [0f32; 5];
            for (i, value) in scaled.iter_mut().enumerate() {
                *value = (scale * shared_float(bounds, i * 4)).to_f32();
            }
            let last = scale * shared_float(bounds, 20);
            let mut eax = 0;
            for axis in 0..3 {
                let out = bounds.add(axis * 4);
                let translation = word(matrix, 0x28 + axis * 4);
                put_word(out, 0, translation);
                let translation = word(matrix, 0x28 + axis * 4);
                put_word(out, 12, translation);
                eax = translation;
                for component in 0..3 {
                    let coefficient = shared_float(matrix, 4 + axis * 12 + component * 4);
                    let low = (F80::from_f32(scaled[component]) * coefficient).to_f32();
                    let high = if component == 2 { last } else { F80::from_f32(scaled[component + 3]) };
                    let high = coefficient * high;
                    let low = F80::from_f32(low);
                    let comparison = low.compare(high);
                    // At the comparison, the retained final scaled bound and the upper product occupy
                    // two x87 registers. FNSTSW records their stack depth along with the comparison.
                    let status = comparison.status_word();
                    eax = (eax & 0xffff0000) | u32::from((status & !0x3800) | (status.wrapping_sub(0x1000) & 0x3800));
                    if comparison.less() {
                        put_word(out, 0, (low + shared_float(out, 0)).to_f32().to_bits());
                        put_word(out, 12, (high + shared_float(out, 12)).to_f32().to_bits());
                    } else {
                        put_word(out, 0, (high + shared_float(out, 0)).to_f32().to_bits());
                        put_word(out, 12, (low + shared_float(out, 12)).to_f32().to_bits());
                    }
                }
            }
            entry.ecx = bounds.wrapping_add(12) as u32;
            (u64::from(matrix.wrapping_add(0x30) as u32) << 32) | u64::from(eax)
        }
    }
}

register_port! { fc4p_0080b530, 0x0080b530, "build_basis_from_up_vector", 0,
    build_basis_from_up_vector(_this, entry) {
        // SAFETY: the caller supplies a three-float vector and nine-float output, which may overlap.
        unsafe {
            use crate::x87::F80;
            let source = entry.arg(0) as *mut u8;
            let output = entry.arg(1) as *mut u8;
            for i in 0..3 { put_word(output, 4 + i * 12, word(source, i * 4)); }
            let abs_x = shared_float(source, 0).abs();
            let abs_z = shared_float(source, 8).abs();
            let comparison = abs_z.compare(abs_x);
            let (eax, ecx);
            if comparison.less() {
                let y = shared_float(source, 4);
                let nx = (-shared_float(source, 0)).to_f32();
                let length = (y * y + F80::from_f32(nx) * F80::from_f32(nx)).sqrt();
                let inverse = F80::from_f32(*basis_one()) / length;
                let rounded_inverse = inverse.to_f32();
                let x = inverse * y;
                let y = F80::from_f32(rounded_inverse) * F80::from_f32(nx);
                let z = (F80::from_f32(rounded_inverse) * F80::from_f32(*funding_zero())).to_f32();
                let c0 = y * shared_float(source, 8) - F80::from_f32(z) * shared_float(source, 4);
                let c1 = F80::from_f32(z) * shared_float(source, 0) - x * shared_float(source, 8);
                let c2 = x * shared_float(source, 4) - y * shared_float(source, 0);
                ecx = c2.to_f32().to_bits();
                put_word(output, 32, ecx); put_word(output, 8, c0.to_f32().to_bits());
                put_word(output, 20, c1.to_f32().to_bits());
                eax = z.to_bits(); put_word(output, 24, eax);
                put_word(output, 0, x.to_f32().to_bits()); put_word(output, 12, y.to_f32().to_bits());
            } else {
                let z = shared_float(source, 8);
                let y = -shared_float(source, 4);
                let nz = (-z).to_f32();
                let y = -y;
                let rounded_y = y.to_f32();
                let length = (y * F80::from_f32(rounded_y) + F80::from_f32(nz) * F80::from_f32(nz)).sqrt();
                let inverse = F80::from_f32(*basis_one()) / length;
                let rounded_inverse = inverse.to_f32();
                let x = inverse * F80::from_f32(*funding_zero());
                let y = F80::from_f32(nz) * F80::from_f32(rounded_inverse);
                let z = F80::from_f32(rounded_y) * F80::from_f32(rounded_inverse);
                let rounded_z = z.to_f32();
                let c0 = z * shared_float(source, 4) - y * shared_float(source, 8);
                let c1 = x * shared_float(source, 8) - F80::from_f32(rounded_z) * shared_float(source, 0);
                let c2 = y * shared_float(source, 0) - x * shared_float(source, 4);
                ecx = c2.to_f32().to_bits();
                put_word(output, 24, ecx); put_word(output, 0, c0.to_f32().to_bits());
                put_word(output, 12, c1.to_f32().to_bits());
                eax = rounded_z.to_bits(); put_word(output, 32, eax);
                put_word(output, 8, x.to_f32().to_bits()); put_word(output, 20, y.to_f32().to_bits());
            }
            entry.ecx = ecx;
            (u64::from(output as u32) << 32) | u64::from(eax)
        }
    }
}

register_port! { fc4p_0080b830, 0x0080b830, "build_basis_from_forward_vector", 0,
    build_basis_from_forward_vector(_this, entry) {
        // SAFETY: both vector and matrix ranges are live, including caller-selected overlap.
        unsafe {
            use crate::x87::F80;
            let source = entry.arg(0) as *mut u8;
            let output = entry.arg(1) as *mut u8;
            for i in 0..3 { put_word(output, 8 + i * 12, word(source, i * 4)); }
            let nz = -shared_float(source, 8);
            let x = shared_float(source, 0);
            let length = -(x * x + nz * nz).sqrt();
            entry.ecx = output as u32;
            if !length.compare(F80::from_f32(*funding_zero())).less() {
                for (offset, value) in [(0, 0), (12, 0), (24, 0x3f800000), (4, 0x3f800000), (16, 0), (28, 0)] {
                    put_word(output, offset, value);
                }
                return 0x3f80000000000000;
            }
            let inverse = F80::from_f32(*basis_one()) / length;
            let x0 = (nz * inverse).to_f32(); put_word(output, 0, x0.to_bits());
            let x1 = (F80::from_f32(*funding_zero()) * inverse).to_f32(); put_word(output, 12, x1.to_bits());
            let x2 = (x * inverse).to_f32(); put_word(output, 24, x2.to_bits());
            let c0 = F80::from_f32(x2) * shared_float(source, 4) - F80::from_f32(x1) * shared_float(source, 8);
            let c1 = F80::from_f32(x0) * shared_float(source, 8) - F80::from_f32(x2) * shared_float(source, 0);
            let c2 = F80::from_f32(x1) * shared_float(source, 0) - F80::from_f32(x0) * shared_float(source, 4);
            let edx = c2.to_f32().to_bits();
            put_word(output, 28, edx); put_word(output, 4, c0.to_f32().to_bits());
            put_word(output, 16, c1.to_f32().to_bits());
            (u64::from(edx) << 32) | u64::from(x2.to_bits())
        }
    }
}

#[cfg(test)]
mod range_and_scheduler_tests {
    use super::*;
    use crate::abi::{call_raw, Conv, RawCall};
    use crate::testing::{fake, fake_data, GameFpu, Object};
    use std::cell::{Cell, RefCell};

    const HIGH: u64 = 0x9876543200000000;
    const LOW: u32 = 0x12345678;
    thread_local! {
        static EVENTS: RefCell<Vec<(u32, Vec<u32>)>> = const { RefCell::new(Vec::new()) };
        static ALLOCATION: Cell<u32> = const { Cell::new(0) };
        static OWNER: Cell<u32> = const { Cell::new(0) };
        static STORAGE: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
        static CHANGES: Cell<bool> = const { Cell::new(false) };
    }
    fn event(this: u32, args: &[u32]) {
        EVENTS.with(|v| v.borrow_mut().push((this, args.to_vec())));
    }
    fn events() -> Vec<(u32, Vec<u32>)> {
        EVENTS.with(|v| core::mem::take(&mut *v.borrow_mut()))
    }
    unsafe fn invoke(address: usize, conv: Conv, this: *mut u8, args: &[u32]) -> (u32, u32) {
        let mut call = RawCall::new(conv, address, this as u32, args);
        call.eax = 0xdeadbeef; call.edx = 0x23456789;
        // SAFETY: each test supplies the entry's exact convention and live argument storage.
        unsafe { call_raw(&mut call) };
        (call.eax, call.edx)
    }
    extern "C" fn allocate(bytes: u32) -> u64 {
        event(0x0090cf54, &[bytes]);
        HIGH | u64::from(ALLOCATION.with(Cell::get))
    }
    extern "C" fn free(pointer: u32) -> u64 {
        let owner = OWNER.with(Cell::get) as *mut u8;
        // SAFETY: the owner is live throughout the tested call.
        unsafe { event(0x0090cf63, &[pointer, word(owner, 0), word(owner, 4), word(owner, 8)]); }
        HIGH | u64::from(LOW)
    }
    extern "C" fn move_bytes(output: u32, source: u32, count: u32) -> u64 {
        event(0x009eeb70, &[output, source, count]);
        // SAFETY: both regions are in the test's vector, and may overlap.
        unsafe { core::ptr::copy(source as *const u8, output as *mut u8, count as usize); }
        HIGH | u64::from(output)
    }
    extern "C" fn copy_bytes(source: u32, end: u32, output: u32) -> u64 {
        event(0x006d6880, &[source, end, output]);
        let mut source = source;
        let mut output = output;
        // SAFETY: tests provide readable and writable byte ranges, including intentional overlap.
        unsafe {
            while (end.wrapping_sub(source) as i32) > 0 {
                *(output as *mut u8) = *(source as *const u8);
                source = source.wrapping_add(1); output = output.wrapping_add(1);
            }
        }
        HIGH | u64::from(output)
    }
    extern "C" fn copy_records(source: u32, end: u32, output: u32, stop: *mut u8) -> u64 {
        // SAFETY: the final argument points at the original caller's stop byte.
        unsafe { event(0x008042b0, &[source, end, output, u32::from(*stop)]); }
        let mut source = source;
        let mut output = output;
        // SAFETY: every complete record is within the live test allocation.
        unsafe {
            while source != end {
                if output != 0 {
                    for i in 0..7 { put_word(output as *mut u8, i * 4, word(source as *mut u8, i * 4)); }
                }
                source = source.wrapping_add(28); output = output.wrapping_add(28);
            }
        }
        HIGH | u64::from(output)
    }
    #[test]
    fn byte_vector_insertion_covers_spare_capacity_growth_aliases_and_empty_ranges() {
        let _a = fake(0x0090cf54, allocate as *const () as usize);
        let _f = fake(0x0090cf63, free as *const () as usize);
        let _m = fake(0x009eeb70, move_bytes as *const () as usize);
        let _c = fake(0x006d6880, copy_bytes as *const () as usize);
        let mut owner = Object::new(32);
        let p = owner.ptr(); OWNER.with(|v| v.set(p as u32));
        for (size, position, count, capacity, alias) in [
            (6usize, 1usize, 2usize, 16usize, false),
            (6, 4, 4, 16, false), (6, 4, 2, 16, false), (6, 6, 3, 16, false),
            (6, 0, 3, 6, false), (6, 3, 8, 6, false), (0, 0, 3, 0, false),
            (6, 3, 0, 6, false), (6, 1, 2, 16, true), (6, 4, 4, 16, true),
            (6, 2, 3, 6, true),
        ] {
            let mut old = [0xa5u8; 64];
            for (i, b) in old.iter_mut().take(size).enumerate() { *b = i as u8 + 1; }
            let mut new = [0x5au8; 64];
            let external = [21u8, 22, 23, 24, 25, 26, 27, 28];
            let base = if size == 0 { 0 } else { old.as_mut_ptr() as u32 };
            let pos = base.wrapping_add(position as u32);
            let src = if alias { old.as_ptr() as u32 } else { external.as_ptr() as u32 };
            ALLOCATION.with(|v| v.set(new.as_mut_ptr() as u32));
            // SAFETY: all tested ranges fit the arrays above and the header covers three pointer words.
            unsafe {
                put_word(p, 0, base); put_word(p, 4, base + size as u32); put_word(p, 8, base + capacity as u32);
                let before_header = core::slice::from_raw_parts(p, 32).to_vec();
                let mut expected_old = old;
                let mut expected_new = new;
                let read_source = |old: &[u8; 64], i: usize| if alias { old[i] } else { external[i] };
                if count != 0 {
                    if capacity - size >= count {
                        let tail = size - position;
                        if tail > count {
                            for i in 0..count { expected_old[size + i] = expected_old[size - count + i]; }
                            expected_old.copy_within(position..size - count, position + count);
                            for i in 0..count { expected_old[position + i] = read_source(&expected_old, i); }
                        } else {
                            for i in tail..count { expected_old[size + i - tail] = read_source(&expected_old, i); }
                            for i in 0..tail { expected_old[size + count - tail + i] = expected_old[position + i]; }
                            for i in 0..tail { expected_old[position + i] = read_source(&expected_old, i); }
                        }
                    } else {
                        expected_new[..position].copy_from_slice(&old[..position]);
                        for i in 0..count { expected_new[position + i] = read_source(&old, i); }
                        expected_new[position + count..size + count].copy_from_slice(&old[position..size]);
                    }
                }
                let result = invoke(fc4p_006d6a00 as *const () as usize, Conv::Thiscall, p,
                    &[pos, src, src + count as u32, 0xfedcba98]);
                assert_eq!(old, expected_old); assert_eq!(new, expected_new);
                let calls = events();
                if count == 0 {
                    assert_eq!(result, (0xdeadbeef, src)); assert!(calls.is_empty());
                    assert_eq!(core::slice::from_raw_parts(p, 32), before_header);
                } else if capacity - size >= count {
                    assert_eq!((word(p, 0), word(p, 4), word(p, 8)), (base, base + (size + count) as u32, base + capacity as u32));
                    let tail = size - position;
                    let copied = if tail > count { count } else { tail };
                    assert_eq!(result, (pos + copied as u32, (HIGH >> 32) as u32));
                    let mut want = Vec::new();
                    if tail > count { want.push((0x009eeb70, vec![pos + count as u32, pos, (tail - count) as u32])); }
                    want.push((0x006d6880, vec![src, src + copied as u32, pos]));
                    assert_eq!(calls, want);
                } else {
                    let allocation = new.as_ptr() as u32;
                    let cap = size + size.max(count);
                    assert_eq!((word(p, 0), word(p, 4), word(p, 8)), (allocation, allocation + (size + count) as u32, allocation + cap as u32));
                    assert_eq!(result.0, allocation + cap as u32);
                    let mut want = vec![(0x0090cf54, vec![cap as u32])];
                    if base != 0 { want.push((0x0090cf63, vec![base, base, base + size as u32, base + capacity as u32])); }
                    assert_eq!(calls, want);
                }
            }
        }
    }
    #[test]
    fn seven_word_vector_growth_repeats_records_optionally_copies_tail_and_keeps_padding() {
        let _a = fake(0x0090cf54, allocate as *const () as usize);
        let _f = fake(0x0090cf63, free as *const () as usize);
        let _c = fake(0x008042b0, copy_records as *const () as usize);
        let mut owner = Object::new(32);
        let p = owner.ptr(); OWNER.with(|v| v.set(p as u32));
        for count in [0usize, 1, 2, 5] {
            for position in [0usize, 1, 3] {
                for stop in [0u32, 1, 0x12340000, 0x123400ff] {
                    let old: [u32; 21] = core::array::from_fn(|i| i as u32 + 100);
                    let source = [10u32, 20, 30, 40, 50, 60, 70];
                    let mut output = [0xa5a5a5a5u32; 70];
                    let base = old.as_ptr() as u32;
                    let allocation = output.as_mut_ptr() as u32;
                    ALLOCATION.with(|v| v.set(allocation));
                    // SAFETY: capacity never exceeds ten records, and the vector has three records.
                    unsafe {
                        put_word(p, 0, base); put_word(p, 4, base + 84); put_word(p, 8, base + 84);
                        let before = core::slice::from_raw_parts(p, 32).to_vec();
                        let result = invoke(fc4p_00748f20 as *const () as usize, Conv::Thiscall, p,
                            &[base + position as u32 * 28, source.as_ptr() as u32, 0x87654321, count as u32, stop]);
                        let tail = if stop as u8 == 0 { 3 - position } else { 0 };
                        let finish = position + count + tail;
                        assert_eq!(result, (allocation, (HIGH >> 32) as u32));
                        let mut expected = [0xa5a5a5a5u32; 70];
                        expected[..position * 7].copy_from_slice(&old[..position * 7]);
                        for i in 0..count { expected[(position + i) * 7..(position + i + 1) * 7].copy_from_slice(&source); }
                        if tail != 0 { expected[(position + count) * 7..finish * 7].copy_from_slice(&old[position * 7..]); }
                        assert_eq!(output, expected);
                        let mut expected_header = before;
                        for (offset, value) in [(0, allocation), (4, allocation + finish as u32 * 28), (8, allocation + (3 + 3usize.max(count)) as u32 * 28)] {
                            expected_header[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
                        }
                        assert_eq!(core::slice::from_raw_parts(p, 32), expected_header);
                        let mut want = vec![(0x0090cf54, vec![(3 + 3usize.max(count)) as u32 * 28])];
                        if stop as u8 == 0 { want.push((0x008042b0, vec![base + position as u32 * 28, base + 84, allocation + (position + count) as u32 * 28, 0])); }
                        want.push((0x0090cf63, vec![base, base, base + 84, base + 84]));
                        assert_eq!(events(), want);
                    }
                }
            }
        }
    }

    extern "thiscall" fn append(this: *mut u8, value: *const u32) -> u64 {
        let owner = OWNER.with(Cell::get) as *mut u8;
        // SAFETY: both caller-owned words and the scheduler remain live during dispatch.
        unsafe {
            event(0x007ebd30, &[*value, u32::from(*owner.add(0xd2)), u32::from(*owner.add(0xd3))]);
            STORAGE.with(|v| {
                let mut v = v.borrow_mut();
                v.push(*value);
                put_word(this, 0, v.as_ptr() as u32); put_word(this, 4, v.as_ptr().add(v.len()) as u32);
                put_word(this, 8, v.as_ptr().add(v.capacity()) as u32);
            });
        }
        HIGH | u64::from(LOW)
    }
    extern "thiscall" fn erase(this: *mut u8, first: u32, end: u32) -> u64 {
        // SAFETY: the snapshot's three-word header is live.
        unsafe { assert_eq!((word(this, 0), word(this, 4)), (first, end)); put_word(this, 4, first); }
        event(0x0040e490, &[end.wrapping_sub(first) / 4]);
        HIGH | u64::from(LOW)
    }
    extern "thiscall" fn flush(this: *mut u8) -> u64 {
        event(this as u32, &[0x00473ec0]); HIGH | u64::from(LOW)
    }
    extern "thiscall" fn acquire(this: *mut u8) -> u64 {
        event(this as u32, &[4]); HIGH | u64::from(LOW)
    }
    extern "thiscall" fn release(this: *mut u8) -> u64 {
        let owner = OWNER.with(Cell::get) as *mut u8;
        // SAFETY: these flag bytes belong to the live scheduler.
        unsafe { event(this as u32, &[8, u32::from(*owner.add(0xd2)), u32::from(*owner.add(0xd3))]); }
        HIGH | u64::from(LOW)
    }
    extern "thiscall" fn frame(this: *mut u8, value: u32) -> u64 {
        event(this as u32, &[0x28, value]); HIGH | u64::from(LOW)
    }
    extern "thiscall" fn cycle(this: *mut u8, value: u32) -> u64 {
        event(this as u32, &[0x2c, value]);
        if CHANGES.with(Cell::get) {
            // SAFETY: callback mutation of the live scheduler must not change the current notification.
            unsafe { put_word(OWNER.with(Cell::get) as *mut u8, 0xc4, 0xabcdef01); }
        }
        HIGH | u64::from(LOW)
    }
    extern "thiscall" fn scheduler_frame(this: *mut u8, value: u32) -> u64 {
        event(this as u32, &[0x3c, value]);
        if CHANGES.with(Cell::get) {
            // SAFETY: the dispatcher must reload these fields after this callback.
            unsafe { put_word(this, 0xc0, 12); put_word(this, 0xbc, 3); }
        }
        HIGH | u64::from(LOW)
    }
    #[test]
    fn scheduler_snapshots_lists_reloads_counters_and_flushes_only_outermost_dispatches() {
        let _a = fake(0x007ebd30, append as *const () as usize);
        let _e = fake(0x0040e490, erase as *const () as usize);
        let _q = fake(0x00473ec0, flush as *const () as usize);
        let _f = fake(0x0090cf63, free as *const () as usize);
        let mut owner = Object::new(0x100);
        let p = owner.ptr(); OWNER.with(|v| v.set(p as u32));
        let mut vt = [0usize; 16];
        vt[1] = acquire as *const () as usize; vt[2] = release as *const () as usize;
        vt[0x28 / 4] = frame as *const () as usize; vt[0x2c / 4] = cycle as *const () as usize;
        vt[0x3c / 4] = scheduler_frame as *const () as usize;
        let mut listeners = [[vt.as_ptr() as u32]; 2];
        let mut nodes = [[0u32; 3]; 3];
        let header = nodes[0].as_mut_ptr() as u32;
        for (i, listener) in listeners.iter_mut().enumerate() {
            nodes[i + 1][2] = listener.as_mut_ptr() as u32;
        }
        for is_cycle in [false, true] {
            for nonempty in [false, true] {
                for nested in [0u32, 1, u32::MAX] {
                    for initial_frame in [0u32, 1, u32::MAX] {
                        for changes in [false, true] {
                            CHANGES.with(|v| v.set(changes));
                            STORAGE.with(|v| *v.borrow_mut() = Vec::with_capacity(4));
                            nodes[0][0] = if nonempty { nodes[1].as_ptr() as u32 } else { header };
                            nodes[1][0] = nodes[2].as_ptr() as u32; nodes[2][0] = header;
                            // SAFETY: every object has its live vtable, and both circular lists have a sentinel.
                            unsafe {
                                core::ptr::write_bytes(p, 0xa5, 0x100);
                                put_word(p, 0, vt.as_ptr() as u32); put_word(p, 0xe0, vt.as_ptr() as u32);
                                put_word(p, 0xa8, header); put_word(p, 0xb0, header);
                                put_word(p, 0xbc, 2); put_word(p, 0xc0, initial_frame); put_word(p, 0xc4, u32::MAX);
                                put_word(p, 0xd8, nested); put_word(p, 0xdc, nested);
                                *p.add(0xd2) = 0; *p.add(0xd3) = 0;
                                let before = core::slice::from_raw_parts(p, 0x100).to_vec();
                                let result = invoke(if is_cycle { fc4p_0087a812 as *const () as usize } else { fc4p_0087a74b as *const () as usize },
                                    Conv::Thiscall, p, if is_cycle { &[] } else { &[0x98765432] });
                                assert_eq!(result, (LOW, (HIGH >> 32) as u32));
                                let triggered = !is_cycle || changes || initial_frame.wrapping_add(1) % 2 == 0;
                                let mut want = Vec::new();
                                if is_cycle { want.push((p as u32, vec![0x3c, initial_frame])); }
                                want.push((p as u32 + 0xe0, vec![4]));
                                if triggered && nonempty {
                                    for listener in &listeners { want.push((0x007ebd30, vec![listener.as_ptr() as u32, u32::from(is_cycle), u32::from(!is_cycle)])); }
                                }
                                want.push((p as u32 + 0xe0, vec![8, 0, 0]));
                                if triggered {
                                    if nonempty {
                                        for listener in &listeners { want.push((listener.as_ptr() as u32, vec![if is_cycle { 0x2c } else { 0x28 }, if is_cycle { 0 } else { 0x98765432 }])); }
                                    }
                                    want.push((0x0040e490, vec![if nonempty { 2 } else { 0 }]));
                                }
                                if !is_cycle && nested == 0 { want.push((p as u32 + 0xb4, vec![0x00473ec0])); }
                                if triggered && nonempty {
                                    let buffer = STORAGE.with(|v| v.borrow().as_ptr() as u32);
                                    want.push((0x0090cf63, vec![buffer, word(p, 0), word(p, 4), word(p, 8)]));
                                }
                                if is_cycle && nested == 0 { want.push((p as u32 + 0xac, vec![0x00473ec0])); }
                                assert_eq!(events(), want);
                                let mut expected = before;
                                if is_cycle {
                                    expected[0xc0..0xc4].copy_from_slice(&(if changes { 12 } else { initial_frame.wrapping_add(1) }).to_le_bytes());
                                    if changes { expected[0xbc..0xc0].copy_from_slice(&3u32.to_le_bytes()); }
                                    if triggered { expected[0xc4..0xc8].copy_from_slice(&(if changes && nonempty { 0xabcdef01u32 } else { 0 }).to_le_bytes()); }
                                }
                                assert_eq!(core::slice::from_raw_parts(p, 0x100), expected);
                            }
                        }
                    }
                }
            }
        }
    }

    fn exception_chain() -> u32 {
        let result: u32;
        // SAFETY: the thread's exception-chain head is a readable Windows TIB field.
        unsafe { core::arch::asm!("mov {value:e}, fs:[0]", value = out(reg) result, options(nostack, readonly)); }
        result
    }
    extern "thiscall" fn release_interface(this: *mut u8) -> u64 {
        let mut chain = exception_chain() as *const u32;
        // SAFETY: the port installs a live registration frame for the duration of Release.
        unsafe {
            while chain as u32 != u32::MAX && chain.add(1).read() != 0x00a6f090 { chain = chain.read() as *const u32; }
            if chain as u32 == u32::MAX { event(this as u32, &[]); }
            else { event(this as u32, &[chain.add(1).read(), chain.add(2).read()]); }
        }
        HIGH | u64::from(LOW)
    }
    #[test]
    fn held_interface_release_keeps_holder_links_exception_frame_and_restores_chain() {
        let mut vt = [0usize; 3]; vt[2] = release_interface as *const () as usize;
        let mut held = [vt.as_ptr() as u32];
        let mut holder = [0u32, 0xa5a5a5a5];
        let previous = exception_chain();
        // SAFETY: both the empty holder and the held object's slot 8 have their original layouts.
        unsafe {
            assert_eq!(invoke(fc4p_007ce1a0 as *const () as usize, Conv::Thiscall, holder.as_mut_ptr().cast(), &[]), (previous, 0x23456789));
            assert!(events().is_empty()); assert_eq!(exception_chain(), previous);
            holder[0] = held.as_mut_ptr() as u32;
            let before = holder;
            assert_eq!(invoke(fc4p_007ce1a0 as *const () as usize, Conv::Thiscall, holder.as_mut_ptr().cast(), &[]), (LOW, (HIGH >> 32) as u32));
            assert_eq!(events(), [(held.as_ptr() as u32, vec![0x00a6f090, 0])]);
            assert_eq!(holder, before); assert_eq!(exception_chain(), previous);
        }
    }

    #[test]
    fn bounds_transform_scales_translates_swaps_negative_axes_and_preserves_input_order() {
        let _fpu = GameFpu::new();
        for (matrix, bounds, expected) in [
            // Model-instance core vectors: asymmetric negative scale, reversed empty endpoints,
            // and a full row matrix whose kind byte does not restrict this helper's arithmetic.
            ([f32::from_bits(7),0.,-1.,0., 1.,0.,0., 0.,0.,-1., 10.,20.,30.,-2.], [1.,2.,3.,4.,6.,8.], [14.,12.,36.,22.,18.,46.]),
            ([0.,1.,0.,0., 0.,1.,0., 0.,0.,1., 0.,0.,0.,1.], [1.,1.,1.,0.,0.,0.], [0.,0.,0.,1.,1.,1.]),
            ([0.,1.,2.,0., 0.,1.,3., 4.,0.,1., 10.,20.,30.,-2.], [1.,2.,3.,4.,5.,6.], [-18.,-26.,-14.,0.,-2.,16.]),
            ([0.,1.,0.,0., 0.,1.,0., 0.,0.,1., 10.,20.,30.,2.], [-1.,-2.,-3.,4.,5.,6.], [8.,16.,24.,18.,30.,42.]),
            ([0.,-1.,0.,0., 0.,-2.,0., 0.,0.,-3., 10.,20.,30.,1.], [-1.,-2.,-3.,4.,5.,6.], [6.,10.,12.,11.,24.,39.]),
            ([0.,1.,2.,3., 4.,5.,6., 7.,8.,9., 0.,0.,0.,1.], [-1.,-2.,-3.,4.,5.,6.], [-14.,-32.,-50.,32.,77.,122.]),
            ([0.,1.,0.,0., 0.,1.,0., 0.,0.,1., 10.,20.,30.,0.], [-1.,-2.,-3.,4.,5.,6.], [10.,20.,30.,10.,20.,30.]),
        ] {
            let mut output = [0xa5a5a5a5u32; 8];
            for (dst, value) in output[1..7].iter_mut().zip(bounds) { *dst = f32::to_bits(value); }
            let input = output;
            let source = matrix;
            let args = [matrix.as_ptr() as u32, output.as_mut_ptr().wrapping_add(1) as u32];
            // SAFETY: the matrix and six-float bounds are complete live arrays; the helper is cdecl.
            let result = unsafe { invoke(fc4p_007f8170 as *const () as usize, Conv::Cdecl, core::ptr::null_mut(), &args) };
            assert_eq!(output[1..7], expected.map(f32::to_bits));
            assert_eq!(output[0], input[0]); assert_eq!(output[7], input[7]); assert_eq!(matrix, source);
            assert_eq!(result.1, matrix.as_ptr().wrapping_add(12) as u32);
            assert_eq!(result.0 & 0xffff0000, f32::to_bits(matrix[12]) & 0xffff0000);
            assert_eq!(result.0 & 0x3800, 0x3000);
        }
    }

    #[test]
    fn vector_bases_keep_axis_orientation_signed_zeros_and_degenerate_fallback() {
        let _fpu = GameFpu::new();
        let mut one = 1f32; let mut zero = 0f32;
        let _one = fake_data(0x00b0ec64, (&mut one as *mut f32).cast());
        let _zero = fake_data(0x00a81054, (&mut zero as *mut f32).cast());
        for (forward, source, expected) in [
            (false, [1f32,0.,0.], [0.,1.,-0., -1.,0.,0., 0.,0.,1.]),
            (false, [0.,1.,0.], [1.,0.,0., 0.,1.,-0., -0.,0.,1.]),
            (false, [0.,0.,1.], [1.,0.,0., 0.,0.,-1., -0.,1.,0.]),
            (true, [0.,0.,1.], [1.,0.,0., -0.,1.,0., -0.,-0.,1.]),
            (true, [1.,0.,0.], [0.,0.,1., -0.,1.,0., -1.,-0.,0.]),
            (true, [0.,1.,0.], [0.,1.,0., 0.,0.,1., 1.,0.,0.]),
            (true, [0.,0.,0.], [0.,1.,0., 0.,0.,0., 1.,0.,0.]),
        ] {
            let mut output = [0xa5a5a5a5u32; 11];
            let function = if forward { fc4p_0080b830 as *const () as usize } else { fc4p_0080b530 as *const () as usize };
            // SAFETY: source and nine-word output have their full original extents.
            let result = unsafe { invoke(function, Conv::Cdecl, core::ptr::null_mut(), &[source.as_ptr() as u32, output.as_mut_ptr().wrapping_add(1) as u32]) };
            assert_eq!(output[1..10], expected.map(f32::to_bits), "forward={forward} source={source:?}");
            assert_eq!((output[0], output[10]), (0xa5a5a5a5, 0xa5a5a5a5));
            if forward {
                if source[0] == 0. && source[2] == 0. { assert_eq!(result, (0, 0x3f800000)); }
                else { assert_eq!(result, (output[7], output[8])); }
            } else { assert_eq!(result.1, output.as_ptr().wrapping_add(1) as u32); }
        }
    }

    #[test]
    fn vector_bases_preserve_nan_payloads_and_tiny_extended_exponents() {
        let _fpu = GameFpu::new();
        let mut one = 1f32; let mut zero = 0f32;
        let _one = fake_data(0x00b0ec64, (&mut one as *mut f32).cast());
        let _zero = fake_data(0x00a81054, (&mut zero as *mut f32).cast());
        for source in [[0u32, 0, 0], [0x7fc12345, 1f32.to_bits(), 0], [0, 1f32.to_bits(), 0x7f812345], [0x7f800000, 0, 0]] {
            for forward in [false, true] {
                let mut output = [0xa5a5a5a5u32; 9];
                let function = if forward { fc4p_0080b830 as *const () as usize } else { fc4p_0080b530 as *const () as usize };
                // SAFETY: the arrays contain valid float bit patterns and are large enough for the helpers.
                unsafe { invoke(function, Conv::Cdecl, core::ptr::null_mut(), &[source.as_ptr() as u32, output.as_mut_ptr() as u32]); }
                let column = if forward { 2 } else { 1 };
                assert_eq!([output[column], output[column + 3], output[column + 6]], source);
                if forward && source[0] & 0x7fffffff != 0x7f800000 {
                    assert_eq!([output[0], output[3], output[6], output[1], output[4], output[7]], [0,0,0x3f800000,0x3f800000,0,0]);
                } else {
                    assert!(output.iter().enumerate().any(|(i, value)| i % 3 != column && f32::from_bits(*value).is_nan()));
                }
            }
        }
        let source = [f32::MIN_POSITIVE, 0., 0.];
        let mut output = [0u32; 9];
        // SAFETY: complete arrays, and the scaled square must retain the x87 exponent range.
        unsafe { invoke(fc4p_0080b530 as *const () as usize, Conv::Cdecl, core::ptr::null_mut(), &[source.as_ptr() as u32, output.as_mut_ptr() as u32]); }
        assert_eq!(output, [0f32, f32::MIN_POSITIVE, -0., -1.,0.,0., 0.,0.,f32::MIN_POSITIVE].map(f32::to_bits));
    }

    thread_local! {
        static MODEL_NODES: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
        static MODEL_IMAGE: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    }
    extern "thiscall" fn model_base(this: *mut u8, complete: u32) -> u64 {
        // SAFETY: this points eight bytes into the caller-owned model instance.
        unsafe {
            let owner = this.sub(8);
            let offset = model_virtual_offset(owner);
            event(this as u32, &[complete, word(owner, 0), word(owner.wrapping_add(offset as usize), 4)]);
            put_word(this, 0x18, 0x11223344);
        }
        HIGH | u64::from(LOW)
    }
    extern "thiscall" fn grow_nodes(this: *mut u8) -> u64 {
        let owner = OWNER.with(Cell::get) as *mut u8;
        // SAFETY: the constructor has initialized these fields before it acquires pool nodes.
        unsafe { event(this as u32, &[0x0090cf9a, word(owner, 0x1c), word(owner, 0x94), word(owner, 0x13c)]); }
        HIGH | u64::from(MODEL_NODES.with(|v| v.borrow_mut().remove(0)))
    }
    extern "C" fn allocate_five_words(bytes: u32) -> u64 {
        event(0x005e55e0, &[bytes]); HIGH | u64::from(ALLOCATION.with(Cell::get))
    }
    #[allow(clippy::too_many_arguments)]
    extern "thiscall" fn pack_state(this: *mut u8, a: u32, b: u32, c: u32, d: u32, e: u32, f: u32, g: u32, h: u32, i: u32, j: u32, k: u32, l: u32, m: u32, n: u32) -> u64 {
        event(this as u32, &[0x007d2b50,a,b,c,d,e,f,g,h,i,j,k,l,m,n]);
        let owner = OWNER.with(Cell::get) as *mut u8;
        // SAFETY: the constructor's complete object and packed state remain live.
        unsafe {
            MODEL_IMAGE.with(|v| *v.borrow_mut() = core::slice::from_raw_parts(owner, 0x1a0).to_vec());
            put_word(this, 0, LOW);
        }
        HIGH | u64::from(LOW)
    }
    #[test]
    fn model_constructor_initializes_complete_or_embedded_objects_acquires_nodes_and_packs_state() {
        let _base = fake(0x00801cf0, model_base as *const () as usize);
        let _grow = fake(0x0090cf9a, grow_nodes as *const () as usize);
        let _allocate = fake(0x005e55e0, allocate_five_words as *const () as usize);
        let _pack = fake(0x007d2b50, pack_state as *const () as usize);
        let mut default_word = 0x7f812345u32;
        let mut bases = [0u32, 0x16c];
        let mut service = 0u32;
        let _default = fake_data(0x00b4e65c, (&mut default_word as *mut u32).cast());
        let _bases = fake_data(0x00abd284, bases.as_mut_ptr().cast());
        let _service = fake_data(0x00b0e33c, (&mut service as *mut u32).cast());
        let mut owner = Object::new(0x1a0);
        let p = owner.ptr(); OWNER.with(|v| v.set(p as u32));
        let mut pool = [0u32; 4];
        let mut pool_pointer = pool.as_mut_ptr().cast::<u8>();
        let _pool = fake_data(0x00b0e340, (&mut pool_pointer as *mut *mut u8).cast());
        let mut alternate_bases = [0u32, 0x178];
        for complete in [0u32, 1, 0x100] {
            for existing in [0usize, 2, 5] {
                let mut nodes = [[0xa5a5a5a5u32; 3]; 5];
                for i in 0..existing { nodes[i][0] = if i + 1 == existing { 0 } else { nodes[i + 1].as_ptr() as u32 }; }
                for node in nodes.iter_mut().skip(existing) { node[0] = 0; }
                pool[2] = if existing == 0 { 0 } else { nodes[0].as_mut_ptr() as u32 };
                pool[3] = u32::MAX - 2;
                MODEL_NODES.with(|v| *v.borrow_mut() = nodes.iter_mut().skip(existing).map(|n| n.as_mut_ptr() as u32).collect());
                let mut heap = [0xa5a5a5a5u32; 6];
                ALLOCATION.with(|v| v.set(heap.as_mut_ptr() as u32));
                // SAFETY: the model, pool, virtual-base tables, nodes and heap cover every field written.
                unsafe {
                    core::ptr::write_bytes(p, 0xa5, 0x1a0);
                    put_word(p, 4, alternate_bases.as_mut_ptr() as u32);
                    let mut expected = core::slice::from_raw_parts(p, 0x1a0).to_vec();
                    let result = invoke(fc4p_007fdcb0 as *const () as usize, Conv::Thiscall, p, &[complete]);
                    assert_eq!(result, (p as u32, (HIGH >> 32) as u32));
                    let mut set = |offset: usize, value: u32| expected[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
                    let offset = if complete != 0 { 0x16c } else { 0x178 };
                    if complete != 0 { set(4, 0x00abd284); set(12, 0x00abd27c); }
                    set(0, 0x00abd220); set(8, 0x00abd21c);
                    set(offset, (offset as u32).wrapping_sub(0x16c)); set(offset + 4, 0x00abd1d0);
                    set(0x20, 0x11223344); set(0x1c, 1); set(0x34, 0);
                    for offset in (0x3c..=0x68).step_by(4) { set(offset, if [0x3c,0x4c,0x5c].contains(&offset) { 0x3f800000 } else { 0 }); }
                    set(0x6c, 0x3f800000);
                    for offset in [0x70,0x74,0x78,0x8c,0x90,0x98,0xa4,0xac,0xc4] { set(offset, 0); }
                    set(0x94, heap.as_ptr() as u32);
                    for offset in [0x7c,0x80,0x84] { set(offset, 0x3f800000); }
                    set(0xa0, 0x3a83126f);
                    for (i, node) in nodes.iter().enumerate() { set(0xc8 + i * 4, node.as_ptr() as u32); }
                    for offset in (0xdc..0x13c).step_by(4) { set(offset, 0); }
                    for offset in (0x13c..=0x168).step_by(4) { set(offset, default_word); }
                    for (offset, value) in [(0x38,0),(0x39,0),(0x88,255),(0x89,255),(0x9c,1),(0x9d,1),(0x9e,255),(0x9f,0),(0xa8,3),(0xa9,1),(0xaa,0)] { expected[offset] = value; }
                    for i in 0..4 { expected[0xe8 + i * 24..0xec + i * 24].fill(255); }
                    assert_eq!(MODEL_IMAGE.with(|v| v.borrow().clone()), expected);
                    expected[0x8c..0x90].copy_from_slice(&LOW.to_le_bytes());
                    assert_eq!(core::slice::from_raw_parts(p, 0x1a0), expected);
                    assert_eq!(heap, [0,0,0,0,0,0xa5a5a5a5]);
                    assert!(nodes.iter().all(|n| *n == [0,0,0xa5a5a5a5]));
                    assert_eq!((pool[2],pool[3]), (0,2));
                    assert!(MODEL_NODES.with(|v| v.borrow().is_empty()));
                    let mut want = vec![(p as u32 + 8, vec![0,0x00abd168,0x00abd118])];
                    for _ in existing..5 { want.push((&mut service as *mut u32 as u32, vec![0x0090cf9a,1,0,default_word])); }
                    want.push((0x005e55e0, vec![20]));
                    want.push((p as u32 + 0x8c, vec![0x007d2b50,1,1,1,1,1,1,0,3,4,5,4,0x3f000000,1,u32::MAX]));
                    assert_eq!(events(), want);
                }
            }
        }
    }

    #[test]
    fn basis_outputs_can_overlap_the_input_vector() {
        let _fpu = GameFpu::new();
        let mut one = 1f32; let mut zero = 0f32;
        let _one = fake_data(0x00b0ec64, (&mut one as *mut f32).cast());
        let _zero = fake_data(0x00a81054, (&mut zero as *mut f32).cast());
        for (forward, expected) in [
            (false, [0x3f3504f3,0x3f800000,0x80000000,0xbf3504f3,0x3f800000,0,0,0,0x3fb504f3]),
            (true, [0x3f3504f3,0,0x3f800000,0x80000000,0x3f9a8279,0,0xbf3504f3,0x80000000,0x3f800000]),
        ] {
            let mut memory = [0xa5a5a5a5u32; 11];
            memory[1..4].copy_from_slice(&[0x3f800000,0,0]);
            let pointer = memory.as_mut_ptr().wrapping_add(1) as u32;
            // SAFETY: the shared vector and output matrix are inside the same writable allocation.
            unsafe { invoke(if forward { fc4p_0080b830 as *const () as usize } else { fc4p_0080b530 as *const () as usize },
                Conv::Cdecl, core::ptr::null_mut(), &[pointer,pointer]); }
            assert_eq!(memory[1..10], expected);
            assert_eq!((memory[0],memory[10]), (0xa5a5a5a5,0xa5a5a5a5));
        }
    }

    #[test]
    fn math_helpers_preserve_existing_x87_values_control_words_mxcsr_and_saved_registers() {
        let _fpu = GameFpu::new();
        let mut one = 1f32; let mut zero = 0f32;
        let _one = fake_data(0x00b0ec64, (&mut one as *mut f32).cast());
        let _zero = fake_data(0x00a81054, (&mut zero as *mut f32).cast());
        let vector = [3f32,4.,5.];
        let matrix = [0f32,1.,0.,0.,0.,1.,0.,0.,0.,1.,10.,20.,30.,2.];
        for (function, source) in [(fc4p_007f8170 as *const () as usize, matrix.as_ptr()),
            (fc4p_0080b530 as *const () as usize, vector.as_ptr()), (fc4p_0080b830 as *const () as usize, vector.as_ptr())] {
            let mut output = [-1f32,-2.,-3.,4.,5.,6.,0.,0.,0.];
            let mut saved = [0u32; 22];
            saved[0] = function as u32; saved[1] = source as u32; saved[2] = output.as_mut_ptr() as u32;
            let sentinel = 123.25f64;
            // SAFETY: both arguments are complete live arrays. The block preserves its own saved
            // registers and leaves one sentinel below all of the helper's temporary x87 values.
            unsafe {
                core::arch::asm!(
                    "push ebx", "push ebp", "push esi", "mov ebx, 0x12345678", "mov ebp, 0x23456789", "mov esi, 0x3456789a",
                    "mov [edi + 12], esp", "fld qword ptr [edx]",
                    "fnstcw word ptr [edi + 52]", "stmxcsr [edi + 60]", "pushfd", "pop dword ptr [edi + 68]",
                    "fnstsw ax", "movzx eax, ax", "mov [edi + 80], eax",
                    "push dword ptr [edi + 8]", "push dword ptr [edi + 4]", "call dword ptr [edi]", "add esp, 8",
                    "mov [edi + 16], esp", "mov [edi + 20], ebx", "mov [edi + 24], ebp", "mov [edi + 28], esi", "mov [edi + 32], edi",
                    "mov [edi + 44], eax", "mov [edi + 48], edx",
                    "fnstcw word ptr [edi + 56]", "stmxcsr [edi + 64]", "pushfd", "pop dword ptr [edi + 72]",
                    "fnstsw ax", "movzx eax, ax", "mov [edi + 84], eax", "fstp qword ptr [edi + 36]",
                    "pop esi", "pop ebp", "pop ebx",
                    in("edi") saved.as_mut_ptr(), in("edx") &sentinel, clobber_abi("C"), out("st(0)") _,
                );
            }
            assert_eq!(saved[3], saved[4]);
            assert_eq!(&saved[5..8], &[0x12345678,0x23456789,0x3456789a]);
            assert_eq!(saved[8], saved.as_ptr() as u32);
            assert_eq!(u64::from(saved[9]) | (u64::from(saved[10]) << 32), sentinel.to_bits());
            assert_eq!(saved[13], saved[14]); assert_eq!(saved[15], saved[16]);
            assert_eq!(saved[17] & 0x400, saved[18] & 0x400);
            assert_eq!(saved[20] & 0x3800, saved[21] & 0x3800);
            if function == fc4p_007f8170 as *const () as usize { assert_eq!(saved[11] & 0x3800, 0x2800); }
        }
    }

    #[unsafe(naked)]
    unsafe extern "C" fn direction_preserving_allocate() {
        core::arch::naked_asm!("pushfd", "cld", "push dword ptr [esp + 8]", "call {body}", "add esp, 4", "popfd", "ret", body = sym allocate);
    }
    #[unsafe(naked)]
    unsafe extern "C" fn direction_clearing_allocate() {
        core::arch::naked_asm!("cld", "jmp {body}", body = sym allocate);
    }
    #[unsafe(naked)]
    unsafe extern "C" fn direction_preserving_free() {
        core::arch::naked_asm!("pushfd", "cld", "push dword ptr [esp + 8]", "call {body}", "add esp, 4", "popfd", "ret", body = sym free);
    }
    #[test]
    fn seven_word_transfers_follow_the_direction_returned_by_the_allocator() {
        let _free = fake(0x0090cf63, direction_preserving_free as *const () as usize);
        let mut owner = Object::new(32);
        let p = owner.ptr(); OWNER.with(|v| v.set(p as u32));
        let old: [u32; 32] = core::array::from_fn(|i| 100 + i as u32);
        let source: [u32; 16] = core::array::from_fn(|i| 200 + i as u32);
        for backwards in [false, true] {
            let _allocate = fake(0x0090cf54, if backwards { direction_preserving_allocate as *const () as usize } else { direction_clearing_allocate as *const () as usize });
            let mut output = [0xa5a5a5a5u32; 48];
            let base = old.as_ptr().wrapping_add(8) as u32;
            let allocation = output.as_mut_ptr().wrapping_add(8) as u32;
            ALLOCATION.with(|v| v.set(allocation));
            let arguments = [base + 28, source.as_ptr().wrapping_add(8) as u32, 0x87654321, 1, 1];
            let (result, flags): (u32,u32);
            // SAFETY: both arrays have six readable words before their record pointers, covering
            // backward transfers. The assembly clears DF before returning to Rust.
            unsafe {
                put_word(p, 0, base); put_word(p, 4, base + 56); put_word(p, 8, base + 56);
                core::arch::asm!(
                    "std", "push dword ptr [edi + 16]", "push dword ptr [edi + 12]", "push dword ptr [edi + 8]",
                    "push dword ptr [edi + 4]", "push dword ptr [edi]", "call {entry}", "pushfd", "pop edx", "cld",
                    entry = sym fc4p_00748f20, in("edi") arguments.as_ptr(), in("ecx") p,
                    lateout("eax") result, lateout("edx") flags, clobber_abi("C"),
                );
                assert_eq!(result, allocation); assert_eq!(flags & 0x400, if backwards { 0x400 } else { 0 });
                assert_eq!((word(p,0),word(p,4),word(p,8)), (allocation,allocation+56,allocation+112));
            }
            let mut expected = [0xa5a5a5a5u32; 48];
            if backwards { expected[2..9].copy_from_slice(&old[2..9]); expected[9..16].copy_from_slice(&source[2..9]); }
            else { expected[8..15].copy_from_slice(&old[8..15]); expected[15..22].copy_from_slice(&source[8..15]); }
            assert_eq!(output,expected);
            assert_eq!(events(), [(0x0090cf54, vec![112]), (0x0090cf63, vec![base,base,base+56,base+56])]);
        }
    }
}

/// This module's replacements.
pub const PORTS: &[Port] = &[
    fc4p_006d6a00::PORT,
    fc4p_00748f20::PORT,
    fc4p_007ce1a0::PORT,
    fc4p_007f8170::PORT,
    fc4p_007fdcb0::PORT,
    fc4p_0080b530::PORT,
    fc4p_0080b830::PORT,
    fc4p_0087a74b::PORT,
    fc4p_0087a812::PORT,
    fc4p_00700fe0::PORT,
    fc4p_00701830::PORT,
    fc4p_00745980::PORT,
    fc4p_00793ff0::PORT,
    fc4p_007a9810::PORT,
    fc4p_007bfbe0::PORT,
    fc4p_007f82f0::PORT,
    fc4p_00801810::PORT,
    fc4p_0080b420::PORT,
    fc4p_0082aea0::PORT,
    fc4p_0082af70::PORT,
    fc4p_0082b030::PORT,
    fc4p_0082b080::PORT,
    fc4p_0082b120::PORT,
    fc4p_0082b280::PORT,
    fc4p_0082b2f0::PORT,
    fc4p_0082b690::PORT,
    fc4p_0087931b::PORT,
    fc4p_00879355::PORT,
    fc4p_00879383::PORT,
    fc4p_00891cae::PORT,
    fc4p_008922f0::PORT,
    fc4p_006ba280::PORT,
    fc4p_006bbbc0::PORT,
    fc4p_006bbc10::PORT,
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
    fc4p_006cf5c0::PORT,
    fc4p_006d5050::PORT,
    fc4p_006d5140::PORT,
    fc4p_006d66f0::PORT,
    fc4p_006dfc40::PORT,
    fc4p_006dfc50::PORT,
    fc4p_006dfc60::PORT,
    fc4p_006e0130::PORT,
    fc4p_006e0180::PORT,
    fc4p_006e0200::PORT,
    fc4p_006e08d0::PORT,
    fc4p_006e17a0::PORT,
    fc4p_006e17f0::PORT,
    fc4p_006e1800::PORT,
    fc4p_006e1820::PORT,
    fc4p_006e9210::PORT,
    fc4p_006f1570::PORT,
    fc4p_006f7dc0::PORT,
    fc4p_00700e80::PORT,
    fc4p_00700f40::PORT,
    fc4p_00700fa0::PORT,
    fc4p_00701800::PORT,
    fc4p_00725810::PORT,
    fc4p_007266f0::PORT,
    fc4p_0072a1c0::PORT,
    fc4p_0072b410::PORT,
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
    fc4p_00793fe0::PORT,
    fc4p_00798370::PORT,
    fc4p_007a65d0::PORT,
    fc4p_007a97d0::PORT,
    fc4p_007b0e40::PORT,
    fc4p_007b0e50::PORT,
    fc4p_007b62f0::PORT,
    fc4p_007b6310::PORT,
    fc4p_007be550::PORT,
    fc4p_007be560::PORT,
    fc4p_007be720::PORT,
    fc4p_007bf5c0::PORT,
    fc4p_007bf5d0::PORT,
    fc4p_007bfa00::PORT,
    fc4p_007c07d0::PORT,
    fc4p_007c1870::PORT,
    fc4p_007c1910::PORT,
    fc4p_007c2740::PORT,
    fc4p_007c9fb0::PORT,
    fc4p_007c9fd0::PORT,
    fc4p_007cec30::PORT,
    fc4p_007ced60::PORT,
    fc4p_007d2730::PORT,
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
    fc4p_0081d200::PORT,
    fc4p_0081f910::PORT,
    fc4p_00825c90::PORT,
    fc4p_0082abe0::PORT,
    fc4p_0082abf0::PORT,
    fc4p_0082ace0::PORT,
    fc4p_0082ad00::PORT,
    fc4p_0082ad20::PORT,
    fc4p_0082ad30::PORT,
    fc4p_0082ae10::PORT,
    fc4p_0082ae90::PORT,
    fc4p_0082af20::PORT,
    fc4p_0082af50::PORT,
    fc4p_0082b1a0::PORT,
    fc4p_0082b1c0::PORT,
    fc4p_0082b1d0::PORT,
    fc4p_0082b1e0::PORT,
    fc4p_0082b200::PORT,
    fc4p_0082b4b0::PORT,
    fc4p_0082b540::PORT,
    fc4p_0082b550::PORT,
    fc4p_0082b750::PORT,
    fc4p_0082b760::PORT,
    fc4p_0082b780::PORT,
    fc4p_0082b7d0::PORT,
    fc4p_0082be80::PORT,
    fc4p_00875aba::PORT,
    fc4p_00891c72::PORT,
    fc4p_008929e0::PORT,
    fc4p_00892a0a::PORT,
    fc4p_00897c8d::PORT,
    fc4p_00897fed::PORT,
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
            put_word(output, 16, ITEM_COST.with(|v| v.get()));
            put_word(output, 20, 1);
            put_word(output, 28, ITEM_FUNDING.with(|v| v.get()));
        }
        (RESULT & 0xffff_ffff_0000_0000) | u64::from(ANSWER.with(|v| v.get()))
    }
    #[test]
    fn station_funding_uses_saved_table_reloaded_service_optional_outputs_and_low_byte() {
        ITEM_COST.with(|v| v.set(0x87654321));
        ITEM_FUNDING.with(|v| v.set(0x7fc12345));
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

    thread_local! {
        static RETAINED: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
        static ITEM_COST: std::cell::Cell<u32> = const { std::cell::Cell::new(0x87654321) };
        static ITEM_FUNDING: std::cell::Cell<u32> = const { std::cell::Cell::new(0x7fc12345) };
    }
    extern "thiscall" fn query_message_source(this: *mut u8, id: u32, output: *mut u32) -> u64 {
        // SAFETY: the query output is initialized to zero in the caller's frame.
        unsafe {
            record(this, &[0, id, *output]);
            *output = RETAINED.with(|v| v.get());
        }
        (RESULT & 0xffff_ffff_0000_0000) | u64::from(ANSWER.with(|v| v.get()))
    }
    extern "thiscall" fn new_message(this: *mut u8) -> u64 {
        record(this, &[0xc70]);
        (RESULT & 0xffff_ffff_0000_0000) | u64::from(this as u32)
    }
    extern "thiscall" fn message_add_ref(this: *mut u8) -> u64 {
        record(this, &[4]);
        RESULT
    }
    extern "thiscall" fn message_release(this: *mut u8) -> u64 {
        record(this, &[8]);
        RESULT
    }
    extern "thiscall" fn message_type(this: *mut u8, value: u32) -> u64 {
        record(this, &[0x14, value]);
        RESULT
    }
    extern "thiscall" fn message_code(this: *mut u8, value: u32) -> u64 {
        record(this, &[0x2c, value]);
        RESULT
    }
    extern "thiscall" fn message_payload(this: *mut u8, value: u32) -> u64 {
        record(this, &[0x84, value]);
        RESULT
    }
    extern "thiscall" fn message_send(this: *mut u8, message: u32) -> u64 {
        record(this, &[0x0c, message]);
        RESULT
    }
    extern "thiscall" fn message_post(this: *mut u8, message: u32, priority: u32) -> u64 {
        record(this, &[0x10, message, priority]);
        RESULT
    }
    #[test]
    fn neighbor_message_queries_optional_source_dispatches_and_releases_in_order() {
        use crate::testing::fake_data;
        let _allocate = fake(0x009133da, allocate_words as *const () as usize);
        let _new = fake(0x009134d6, new_message as *const () as usize);
        let mut source = Object::new(4);
        let source_pointer = source.ptr();
        let mut held = Object::new(4);
        let held_pointer = held.ptr();
        let mut message = Object::new(44);
        let message_pointer = message.ptr();
        let mut server = Object::new(4);
        let server_pointer = server.ptr();
        let mut global_server = server_pointer;
        let _data = fake_data(0x00b43ccc, (&mut global_server as *mut *mut u8).cast());
        let mut table = [0usize; 34];
        table[0] = query_message_source as *const () as usize;
        table[1] = message_add_ref as *const () as usize;
        table[2] = message_release as *const () as usize;
        table[3] = message_send as *const () as usize;
        table[4] = message_post as *const () as usize;
        table[5] = message_type as *const () as usize;
        table[0x2c / 4] = message_code as *const () as usize;
        table[0x84 / 4] = message_payload as *const () as usize;
        // SAFETY: all four test interfaces have the same live callback table.
        unsafe {
            for p in [source_pointer, held_pointer, message_pointer, server_pointer] {
                put_word(p, 0, table.as_ptr() as u32);
            }
        }
        for source in [0u32, source_pointer as u32] {
            for answer in [0u32, 0x12345600, 0x12345601] {
                for retained in [0u32, held_pointer as u32] {
                    for post in [0u32, 1, 0x12345600, 0xff] {
                        ANSWER.with(|v| v.set(answer));
                        RETAINED.with(|v| v.set(retained));
                        let proceed = source == 0 || answer as u8 != 0;
                        let held = if source == 0 { 0 } else { retained };
                        ALLOCATIONS
                            .with(|a| *a.borrow_mut() = if proceed { vec![message_pointer as u32] } else { vec![] });
                        let before = memory(message_pointer, 44);
                        assert_eq!(
                            invoke_cdecl(
                                fc4p_006cf5c0 as *const () as usize,
                                source as *mut u8,
                                &[post, 0x87654321],
                                EAX
                            ),
                            (
                                if proceed || held != 0 { RESULT as u32 } else { answer },
                                (RESULT >> 32) as u32
                            )
                        );
                        let mut expected = vec![];
                        if source != 0 {
                            expected.push((source, vec![0, 1, 0]));
                        }
                        if proceed {
                            expected.extend([
                                (0, vec![0xa110, 44]),
                                (message_pointer as u32, vec![0xc70]),
                                (message_pointer as u32, vec![4]),
                                (message_pointer as u32, vec![0x14, 0xea418d8f]),
                                (message_pointer as u32, vec![0x2c, 0x87654321]),
                                (message_pointer as u32, vec![0x84, held]),
                                (
                                    server_pointer as u32,
                                    if post as u8 == 0 {
                                        vec![0x0c, message_pointer as u32]
                                    } else {
                                        vec![0x10, message_pointer as u32, 0]
                                    },
                                ),
                                (message_pointer as u32, vec![8]),
                            ]);
                        }
                        if held != 0 {
                            expected.push((held, vec![8]));
                        }
                        assert_eq!(calls(), expected);
                        assert_eq!(memory(message_pointer, 44), before);
                        assert!(ALLOCATIONS.with(|a| a.borrow().is_empty()));
                    }
                }
            }
        }
    }
    extern "thiscall" fn refresh_ids(this: *mut u8) -> u64 {
        // SAFETY: the copied list count and its first ID belong to the owner's live fields.
        unsafe {
            record(this, &[word(this, 0x120), word(this, 0xe0)]);
        }
        RESULT
    }
    #[test]
    fn filter_ids_copy_sequentially_and_keep_nonpositive_counts() {
        let _refresh = fake(0x007a93e0, refresh_ids as *const () as usize);
        let mut owner = Object::new(0x150);
        let p = owner.ptr();
        for count in [i32::MIN, -1, 0, 1, 16, 17] {
            for source_offset in [0x40usize, 0xdc, 0xe0, 0xe4] {
                fill(p, 0x150);
                // SAFETY: all tested input ranges and outputs fit this allocation.
                unsafe {
                    for i in 0..18usize {
                        put_word(p, source_offset + i * 4, 0x12340000 + i as u32);
                    }
                }
                let mut expected = memory(p, 0x150);
                for i in 0..count.max(0) as usize {
                    let value = u32::from_le_bytes(
                        expected[source_offset + i * 4..source_offset + i * 4 + 4]
                            .try_into()
                            .unwrap(),
                    );
                    expected_word(&mut expected, 0xe0 + i * 4, value);
                }
                expected_word(&mut expected, 0x120, count as u32);
                let first = u32::from_le_bytes(expected[0xe0..0xe4].try_into().unwrap());
                assert_eq!(
                    invoke(
                        fc4p_007a97d0 as *const () as usize,
                        p,
                        &[p.wrapping_add(source_offset) as u32, count as u32]
                    ),
                    (RESULT as u32, (RESULT >> 32) as u32)
                );
                assert_eq!(calls(), vec![(p as u32, vec![count as u32, first])]);
                assert_eq!(memory(p, 0x150), expected);
            }
        }
    }
    #[test]
    fn pair_allocation_copies_each_word_preserves_unused_capacity_and_returns_base() {
        let _allocate = fake(0x0090cf54, allocate_words as *const () as usize);
        let mut source = Object::new(48);
        let r = source.ptr();
        let mut output = Object::new(64);
        let out = output.ptr();
        // SAFETY: six pairs of distinct source words fit.
        unsafe {
            for i in 0..12usize {
                put_word(r, i * 4, 0x12340000 + i as u32);
            }
        }
        for (capacity, count) in [(0u32, 0u32), (0, 1), (1, 0), (1, 1), (6, 3), (6, 6)] {
            fill(out, 64);
            let allocation = if capacity == 0 { 0 } else { out as u32 };
            ALLOCATIONS.with(|a| *a.borrow_mut() = if capacity == 0 { vec![] } else { vec![allocation] });
            assert_eq!(
                invoke(
                    fc4p_007cec30 as *const () as usize,
                    core::ptr::null_mut(),
                    &[capacity, r as u32, r as u32 + count * 8]
                ),
                (
                    allocation,
                    if count == 0 {
                        r as u32
                    } else {
                        (r as u32).wrapping_sub(allocation)
                    }
                )
            );
            let mut expected = vec![0xa5; 64];
            if allocation != 0 {
                expected[..count as usize * 8].copy_from_slice(&memory(r, count as usize * 8));
            }
            assert_eq!(memory(out, 64), expected);
            assert_eq!(
                calls(),
                if capacity == 0 {
                    vec![]
                } else {
                    vec![(0, vec![0xa110, capacity * 8])]
                }
            );
        }
    }
    #[test]
    fn pair_vector_copy_reloads_source_after_header_initialization_including_self_copy() {
        let _allocate = fake(0x0090cf54, allocate_words as *const () as usize);
        let mut source_header = Object::new(16);
        let r = source_header.ptr();
        let mut destination_header = Object::new(16);
        let p = destination_header.ptr();
        let mut source = Object::new(24);
        let values = source.ptr();
        let mut output = Object::new(24);
        let out = output.ptr();
        // SAFETY: three source pairs contain distinct words.
        unsafe {
            for i in 0..6usize {
                put_word(values, i * 4, 0x76540000 + i as u32);
            }
        }
        for count in [0u32, 1, 3] {
            for self_copy in [false, true] {
                for null_allocation in [false, true] {
                    if null_allocation && count > 1 {
                        continue;
                    }
                    fill(p, 16);
                    fill(r, 16);
                    fill(out, 24);
                    let source = if self_copy { p } else { r };
                    // SAFETY: both possible source headers describe the live source pairs.
                    unsafe {
                        put_word(source, 0, values as u32);
                        put_word(source, 4, values as u32 + count * 8);
                        put_word(source, 8, values as u32 + count * 8);
                    }
                    let allocation = if count == 0 || null_allocation { 0 } else { out as u32 };
                    ALLOCATIONS.with(|a| *a.borrow_mut() = if count == 0 { vec![] } else { vec![allocation] });
                    let result = invoke(fc4p_007ced60 as *const () as usize, p, &[source as u32]);
                    let copied = if self_copy { 0 } else { count };
                    let end = allocation + copied * 8;
                    assert_eq!(
                        result,
                        (
                            p as u32,
                            if self_copy {
                                allocation
                            } else {
                                values as u32 + count * 8
                            }
                        )
                    );
                    let mut expected = vec![0xa5; 16];
                    expected_word(&mut expected, 0, allocation);
                    expected_word(&mut expected, 4, end);
                    expected_word(&mut expected, 8, allocation + count * 8);
                    assert_eq!(memory(p, 16), expected);
                    let mut expected_output = vec![0xa5; 24];
                    if allocation != 0 {
                        expected_output[..copied as usize * 8].copy_from_slice(&memory(values, copied as usize * 8));
                    }
                    assert_eq!(memory(out, 24), expected_output);
                    assert_eq!(
                        calls(),
                        if count == 0 {
                            vec![]
                        } else {
                            vec![(0, vec![0xa110, count * 8])]
                        }
                    );
                }
            }
        }
    }
    extern "thiscall" fn construct_adapter_base(this: *mut u8) -> u64 {
        let owner = this.wrapping_sub(4);
        // SAFETY: observe the primary table at base construction and change an otherwise untouched field.
        unsafe {
            record(this, &[0xc0, word(owner, 0)]);
            put_word(owner, 0x1c, 0x76543210);
        }
        RESULT
    }
    extern "thiscall" fn destroy_adapter_base(this: *mut u8) -> u64 {
        record(this, &[0xd0]);
        RESULT
    }
    extern "thiscall" fn adapter_release(this: *mut u8) -> u64 {
        // SAFETY: the adapter has already installed its final primary table before either release.
        unsafe {
            record(this, &[8, word(HOLDER.with(|v| v.get()) as *mut u8, 0)]);
        }
        RESULT
    }
    #[test]
    fn surface_adapter_constructor_initializes_only_named_fields_after_base() {
        let _base = fake(0x0090d957, construct_adapter_base as *const () as usize);
        let mut owner = Object::new(0x50);
        let p = owner.ptr();
        fill(p, 0x50);
        let mut expected = memory(p, 0x50);
        for (offset, value) in [
            (0, 0x00ac1550),
            (4, 0x00ac1538),
            (12, 0x00ac1510),
            (16, 0),
            (20, 0),
            (0x1c, 0x76543210),
            (0x28, 0),
            (0x30, 0),
            (0x40, 1),
        ] {
            expected_word(&mut expected, offset, value);
        }
        expected[0x48] = 0;
        assert_eq!(
            invoke(fc4p_0082b780 as *const () as usize, p, &[]),
            (p as u32, (RESULT >> 32) as u32)
        );
        assert_eq!(calls(), vec![(p.wrapping_add(4) as u32, vec![0xc0, 0x00ac1338])]);
        assert_eq!(memory(p, 0x50), expected);
    }
    #[test]
    fn surface_adapter_delete_releases_surface_context_then_base_and_optional_allocation() {
        let _base = fake(0x0090d964, destroy_adapter_base as *const () as usize);
        let _free = fake(0x005e5620, fake_free as *const () as usize);
        let mut owner = Object::new(0x50);
        let p = owner.ptr();
        let mut surface = Object::new(4);
        let a = surface.ptr();
        let mut context = Object::new(4);
        let b = context.ptr();
        let table = [0usize, 0, adapter_release as *const () as usize];
        // SAFETY: both retained interfaces have a live release slot.
        unsafe {
            put_word(a, 0, table.as_ptr() as u32);
            put_word(b, 0, table.as_ptr() as u32);
        }
        HOLDER.with(|v| v.set(p as u32));
        for flags in [0u32, 1, 2, 0x100, 0x101, u32::MAX] {
            for surface in [0u32, a as u32] {
                for context in [0u32, b as u32] {
                    fill(p, 0x50);
                    // SAFETY: set optional references without touching adjacent fields.
                    unsafe {
                        put_word(p, 0x14, surface);
                        put_word(p, 0x10, context);
                    }
                    let mut expected = memory(p, 0x50);
                    for (offset, value) in [(0, 0x00ac1550), (4, 0x00ac1538), (12, 0x00ac1510)] {
                        expected_word(&mut expected, offset, value);
                    }
                    assert_eq!(
                        invoke(fc4p_0082b7d0 as *const () as usize, p, &[flags]),
                        (
                            p as u32,
                            if flags & 1 != 0 {
                                0xfedcba98
                            } else {
                                (RESULT >> 32) as u32
                            }
                        )
                    );
                    let mut expected_calls = vec![];
                    for pointer in [surface, context] {
                        if pointer != 0 {
                            expected_calls.push((pointer, vec![8, 0x00ac1550]));
                        }
                    }
                    expected_calls.push((p.wrapping_add(4) as u32, vec![0xd0]));
                    if flags & 1 != 0 {
                        expected_calls.push((p as u32, vec![0xf0, 0x00ac1550]));
                    }
                    assert_eq!(calls(), expected_calls);
                    assert_eq!(memory(p, 0x50), expected);
                }
            }
        }
    }
    #[test]
    fn station_funding_keeps_low_cost_word_and_float_bits_from_core_budget_example() {
        ITEM_COST.with(|v| v.set(5));
        ITEM_FUNDING.with(|v| v.set(0.6f32.to_bits()));
        let mut simulator = Object::new(0xb4);
        let p = simulator.ptr();
        let mut station = Object::new(4);
        let s = station.ptr();
        let mut department = Object::new(4);
        let q = department.ptr();
        let mut table = [0usize; 52];
        table[6] = station_id as *const () as usize;
        table[0xcc / 4] = funding_item as *const () as usize;
        // SAFETY: the station and department use separate slots in the same live table.
        unsafe {
            put_word(s, 0, table.as_ptr() as u32);
            put_word(q, 0, table.as_ptr() as u32);
            put_word(p, 0x74, q as u32);
            put_word(p, 0xb0, 0xea567bc3);
        }
        HOLDER.with(|v| v.set(p as u32));
        NEXT_SERVICE.with(|v| v.set(q as u32));
        ANSWER.with(|v| v.set(1));
        let mut output = [0u32; 2];
        let out = output.as_mut_ptr();
        invoke(
            fc4p_006e08d0 as *const () as usize,
            p,
            &[s as u32, out as u32, out.wrapping_add(1) as u32],
        );
        assert_eq!(output, [5, 0.6f32.to_bits()]);
        assert_eq!(
            calls(),
            vec![(s as u32, vec![0xea567bc3]), (q as u32, vec![0x55667788, 0])]
        );
    }
}


#[cfg(test)]
mod assigned_tests {
    use super::*;
    use crate::abi::{call_raw, Conv, RawCall};
    use crate::testing::{fake, fake_data, GameFpu, Object};
    use std::cell::{Cell, RefCell};

    const EAX: u32 = 0xa17b39e2;
    const EDX: u32 = 0xd63458a9;
    const HIGH: u64 = 0x9abcdef000000000;
    const VALUE: u32 = 0x12345678;
    thread_local! {
        static EVENTS: RefCell<Vec<(u32, Vec<u32>)>> = const { RefCell::new(Vec::new()) };
        static ANSWERS: RefCell<[u32; 128]> = const { RefCell::new([VALUE; 128]) };
        static OWNER: Cell<u32> = const { Cell::new(0) };
        static HELD: Cell<u32> = const { Cell::new(0) };
        static RECT: Cell<u32> = const { Cell::new(0) };
        static QUERY_RESULT: Cell<u32> = const { Cell::new(1) };
    }
    fn event(this: *mut u8, args: &[u32]) {
        EVENTS.with(|v| v.borrow_mut().push((this as u32, args.to_vec())));
    }
    fn events() -> Vec<(u32, Vec<u32>)> {
        EVENTS.with(|v| core::mem::take(&mut *v.borrow_mut()))
    }
    fn answer(slot: usize) -> u64 {
        HIGH | u64::from(ANSWERS.with(|v| v.borrow()[slot / 4]))
    }
    fn set_answer(slot: usize, value: u32) {
        ANSWERS.with(|v| v.borrow_mut()[slot / 4] = value);
    }
    fn invoke(f: usize, this: *mut u8, args: &[u32]) -> (u32, u32) {
        let mut call = RawCall::new(Conv::Thiscall, f, this as u32, args);
        call.eax = EAX;
        call.edx = EDX;
        // SAFETY: each test supplies this entry's complete argument list and live storage.
        unsafe {
            call_raw(&mut call);
        }
        (call.eax, call.edx)
    }
    fn snapshot(p: *mut u8, n: usize) -> Vec<u8> {
        // SAFETY: each test requests only the extent of its own live allocation.
        unsafe { core::slice::from_raw_parts(p, n).to_vec() }
    }
    fn paint(p: *mut u8, n: usize) {
        // SAFETY: the live allocation covers the requested bytes.
        unsafe {
            core::ptr::write_bytes(p, 0xa5, n);
        }
    }
    fn expected_word(bytes: &mut [u8], offset: usize, word: u32) {
        bytes[offset..offset + 4].copy_from_slice(&word.to_le_bytes());
    }
    macro_rules! slot {
        ($name:ident, $offset:literal $(, $arg:ident)*) => {
            extern "thiscall" fn $name(this: *mut u8 $(, $arg: u32)*) -> u64 {
                event(this, &[$offset $(, $arg)*]);
                answer($offset)
            }
        };
    }
    slot!(s00, 0);
    slot!(s04, 4);
    slot!(s08, 8);
    slot!(s0c, 12);
    slot!(s10, 0x10);
    slot!(s14, 0x14, a);
    slot!(s24, 0x24);
    slot!(s40, 0x40);
    slot!(s44, 0x44, a);
    slot!(s50, 0x50, a, b);
    slot!(s54, 0x54, a);
    slot!(s5c, 0x5c, a);
    slot!(s78, 0x78, a, b, c);
    slot!(s7c, 0x7c, a, b, c);
    slot!(s80, 0x80);
    slot!(s8c, 0x8c, a);
    slot!(sa0, 0xa0);
    slot!(sa4, 0xa4);
    slot!(sa8, 0xa8);
    slot!(sac, 0xac);
    slot!(s170, 0x170);
    fn table() -> [usize; 128] {
        let mut t = [0usize; 128];
        for (offset, function) in [
            (0, s00 as *const ()),
            (4, s04 as *const ()),
            (8, s08 as *const ()),
            (12, s0c as *const ()),
            (0x10, s10 as *const ()),
            (0x14, s14 as *const ()),
            (0x24, s24 as *const ()),
            (0x40, s40 as *const ()),
            (0x44, s44 as *const ()),
            (0x50, s50 as *const ()),
            (0x54, s54 as *const ()),
            (0x5c, s5c as *const ()),
            (0x78, s78 as *const ()),
            (0x7c, s7c as *const ()),
            (0x80, s80 as *const ()),
            (0x8c, s8c as *const ()),
            (0xa0, sa0 as *const ()),
            (0xa4, sa4 as *const ()),
            (0xa8, sa8 as *const ()),
            (0xac, sac as *const ()),
            (0x170, s170 as *const ()),
        ] {
            t[offset / 4] = function as usize;
        }
        t
    }

    #[test]
    fn window_surface_acquires_before_assignment_releases_old_and_always_invalidates() {
        let mut owner = Object::new(0x120);
        let base = owner.ptr();
        let holder = base.wrapping_add(0xe0);
        let mut a = Object::new(4);
        let a = a.ptr();
        let mut b = Object::new(4);
        let b = b.ptr();
        let table = table();
        // SAFETY: all three interfaces use live tables and the holder fits its owner.
        unsafe {
            put_word(base, 0, table.as_ptr() as u32);
            put_word(a, 0, table.as_ptr() as u32);
            put_word(b, 0, table.as_ptr() as u32);
            for old in [0u32, a as u32, b as u32] {
                for new in [0u32, a as u32, b as u32] {
                    paint(holder, 0x20);
                    put_word(holder, 4, old);
                    let mut expected = snapshot(base, 0x120);
                    expected_word(&mut expected, 0xe4, new);
                    assert_eq!(
                        invoke(fc4p_00793ff0 as *const () as usize, holder, &[new]),
                        (VALUE, (HIGH >> 32) as u32)
                    );
                    let mut expected_calls = vec![];
                    if old != new {
                        if new != 0 {
                            expected_calls.push((new, vec![4]));
                        }
                        if old != 0 {
                            expected_calls.push((old, vec![8]));
                        }
                    }
                    if new != 0 {
                        expected_calls
                            .extend([(new, vec![0x78, 255, 0, 255]), (new, vec![0x5c, VALUE])]);
                    }
                    expected_calls.push((base as u32, vec![0x170]));
                    assert_eq!(events(), expected_calls);
                    assert_eq!(snapshot(base, 0x120), expected);
                }
            }
        }
    }
    extern "thiscall" fn filter_refresh(this: *mut u8) -> u64 {
        event(this, &[0xf1]);
        answer(0x40)
    }
    #[test]
    fn filter_notification_checks_full_type_signed_count_ids_and_low_byte_of_refresh() {
        let _refresh = fake(0x007a93e0, filter_refresh as *const () as usize);
        let mut owner = Object::new(0x140);
        let base = owner.ptr();
        let p = base.wrapping_add(0xdc);
        let mut message = Object::new(4);
        let message = message.ptr();
        let table = table();
        // SAFETY: each array and interface lies within its backing allocation.
        unsafe {
            put_word(base, 4, table.as_ptr() as u32);
            put_word(message, 0, table.as_ptr() as u32);
            for kind in [0, 0x426840a0, 0x426840a1] {
                for count in [i32::MIN, -1, 0, 1, 3] {
                    for id in [0x11223344, 0x11223345] {
                        for accepted in [0u32, 0x12345600, 0x12345601] {
                            set_answer(0x10, kind);
                            set_answer(0x24, id);
                            set_answer(0x40, accepted);
                            for i in 0..3 {
                                put_word(p, 4 + i * 4, 0x11223344 + i as u32);
                            }
                            put_word(p, 0x44, count as u32);
                            let before = snapshot(base, 0x140);
                            let matched = count > 0 && (id == 0x11223344 || count > 1);
                            let refreshed = kind == 0x426840a0 && matched;
                            let invalidated = refreshed && accepted as u8 != 0;
                            let low = if invalidated {
                                VALUE
                            } else if refreshed {
                                accepted
                            } else if kind == 0x426840a0 {
                                id
                            } else {
                                kind
                            };
                            let high = if kind == 0x426840a0 && !refreshed {
                                count as u32
                            } else {
                                (HIGH >> 32) as u32
                            };
                            assert_eq!(
                                invoke(fc4p_007a9810 as *const () as usize, p, &[message as u32]),
                                (low & !255, high)
                            );
                            let mut expected = vec![(message as u32, vec![0x10])];
                            if kind == 0x426840a0 {
                                expected.push((message as u32, vec![0x24]));
                            }
                            if refreshed {
                                expected.push((base as u32, vec![0xf1]));
                            }
                            if invalidated {
                                expected.push((base as u32 + 4, vec![0x170]));
                            }
                            assert_eq!(events(), expected);
                            assert_eq!(snapshot(base, 0x140), before);
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn picture_zoom_limits_keep_nan_payload_signed_zero_and_status_return_bits() {
        use crate::x87::F80;
        let _fpu = GameFpu::new();
        let mut low = 0.25f32;
        let mut high = 256f32;
        let _low = fake_data(0x00a8821c, (&mut low as *mut f32).cast());
        let _high = fake_data(0x00a9422c, (&mut high as *mut f32).cast());
        let mut owner = Object::new(32);
        let p = owner.ptr();
        let values = [
            0u32, 0x80000000, 0x3e7fffff, 0x3e800000, 0x3e800001, 0x437fffff, 0x43800000,
            0x43800001, 0x7f800000, 0xff800000, 0x7fc12345, 0x7f812345,
        ];
        for first in values {
            for second in values {
                paint(p, 32);
                let comparison = F80::from_f32(f32::from_bits(second)).compare(F80::from_f32(high));
                let expected_first = if f32::from_bits(first) < low {
                    low.to_bits()
                } else {
                    first
                };
                let expected_second = if f32::from_bits(second) > high {
                    high.to_bits()
                } else {
                    second
                };
                let mut expected = snapshot(p, 32);
                expected_word(&mut expected, 8, expected_first);
                expected_word(&mut expected, 12, expected_second);
                assert_eq!(
                    invoke(fc4p_007bfbe0 as *const () as usize, p, &[first, second]),
                    (
                        (first & 0xffff0000) | u32::from(comparison.status_word() & 0xff00) | 1,
                        second
                    )
                );
                assert_eq!(snapshot(p, 32), expected);
            }
        }
    }
    #[test]
    fn opposite_heading_returns_extended_precision_and_preserves_integer_registers() {
        use crate::x87::F80;
        let _fpu = GameFpu::new();
        for a in [
            0f32,
            -0.,
            1.,
            -1.,
            f32::INFINITY,
            f32::from_bits(0x7fc12345),
        ] {
            for b in [0f32, -0., 2., -2., f32::NEG_INFINITY] {
                let expected = (-F80::from_f32(a)).atan2(-F80::from_f32(b));
                let args = [a.to_bits(), b.to_bits()];
                let mut call =
                    RawCall::new(Conv::Cdecl, fc4p_007f82f0 as *const () as usize, 0, &args);
                call.eax = EAX;
                call.edx = EDX;
                call.take_st0 = 1;
                // SAFETY: this cdecl helper accepts two float words and returns ST0.
                unsafe {
                    call_raw(&mut call);
                }
                assert_eq!((call.eax, call.edx), (EAX, EDX));
                assert_eq!(call.st0.to_bytes(), expected.to_bytes());
            }
        }
    }
    #[test]
    fn virtual_base_destruction_updates_displaced_tables_and_only_clears_released_reference() {
        let mut owner = Object::new(160);
        let p = owner.ptr();
        let mut held = Object::new(4);
        let held = held.ptr();
        let table = table();
        let mut virtual_table = [0u32; 2];
        // SAFETY: the virtual-base slots and optional held reference are backed by live storage.
        unsafe {
            put_word(held, 0, table.as_ptr() as u32);
            for offset in [0x30u32, 0x70] {
                virtual_table[1] = offset;
                for retained in [0u32, held as u32] {
                    paint(p, 160);
                    put_word(p, 4, virtual_table.as_ptr() as u32);
                    put_word(p, 8, retained);
                    let mut expected = snapshot(p, 160);
                    expected_word(&mut expected, 0, 0x00abd4b4);
                    expected_word(&mut expected, offset as usize, offset - 0x2c);
                    expected_word(&mut expected, offset as usize + 4, 0x00abd468);
                    let result = invoke(fc4p_00801810 as *const () as usize, p, &[]);
                    if retained == 0 {
                        assert_eq!(result, (offset, virtual_table.as_ptr() as u32));
                        assert!(events().is_empty());
                    } else {
                        expected_word(&mut expected, 8, 0);
                        assert_eq!(result, (VALUE, (HIGH >> 32) as u32));
                        assert_eq!(events(), vec![(held as u32, vec![8])]);
                    }
                    assert_eq!(snapshot(p, 160), expected);
                }
            }
        }
    }
    extern "C" fn vertex_read(source: u32, index: u32, output: *mut u8) -> u64 {
        event(output, &[0x80ae, source, index]);
        // SAFETY: every successful cursor read provides a three-word destination.
        unsafe {
            for i in 0..3 {
                put_word(output, i * 4, 0x11223344 + i as u32);
            }
        }
        HIGH | u64::from(VALUE)
    }
    extern "thiscall" fn vertex_convert(this: *mut u8, input: *mut u8, output: *mut u8) -> u64 {
        // SAFETY: the read callback initialized the complete temporary record.
        unsafe {
            event(
                this,
                &[
                    0x80b0,
                    word(input, 0),
                    word(input, 4),
                    word(input, 8),
                    output as u32,
                ],
            );
            put_word(output, 0, word(input, 8));
            // Exercise the cursor reload after conversion.
            put_word(this, 0x28, 0x12ff);
        }
        HIGH | u64::from(VALUE)
    }
    #[test]
    fn vertex_cursor_signed_end_raw_and_converted_paths_reload_index_after_calls() {
        let _read = fake(0x0080aef0, vertex_read as *const () as usize);
        let _convert = fake(0x0080b090, vertex_convert as *const () as usize);
        let mut owner = Object::new(64);
        let p = owner.ptr();
        let mut output = [0xa5a5a5a5u32; 3];
        // SAFETY: every cursor field and the output record fit their allocations.
        unsafe {
            for (index, limit) in [
                (0i32, 0i32),
                (-1, 0),
                (1, 2),
                (256, 2),
                (i32::MIN, i32::MAX),
            ] {
                for converted in [false, true] {
                    paint(p, 64);
                    output.fill(0xa5a5a5a5);
                    put_word(p, 4, 0x87654321);
                    put_word(p, 0x1c, u32::from(converted));
                    put_word(p, 0x20, limit as u32);
                    put_word(p, 0x28, index as u32);
                    let mut expected = snapshot(p, 64);
                    let result = invoke(
                        fc4p_0080b420 as *const () as usize,
                        p,
                        &[output.as_mut_ptr() as u32],
                    );
                    if index >= limit {
                        assert_eq!(result, (index as u32 & !255, EDX));
                        assert_eq!(output, [0xa5a5a5a5; 3]);
                        assert!(events().is_empty());
                    } else {
                        let next = if converted {
                            0x1300
                        } else {
                            (index as u32).wrapping_add(1)
                        };
                        expected_word(&mut expected, 0x28, next);
                        assert_eq!(
                            result,
                            (
                                if converted {
                                    (VALUE & !255) | 1
                                } else {
                                    (next & !255) | 1
                                },
                                (HIGH >> 32) as u32
                            )
                        );
                        let calls = events();
                        assert_eq!(calls.len(), if converted { 2 } else { 1 });
                        assert_eq!(calls[0].1, [0x80ae, 0x87654321, index as u32]);
                        if converted {
                            assert_eq!(
                                calls[1],
                                (
                                    p as u32,
                                    vec![
                                        0x80b0,
                                        0x11223344,
                                        0x11223345,
                                        0x11223346,
                                        output.as_mut_ptr() as u32
                                    ]
                                )
                            );
                            assert_eq!(output, [0x11223346, 0xa5a5a5a5, 0xa5a5a5a5]);
                        } else {
                            assert_eq!(calls[0].0, output.as_mut_ptr() as u32);
                            assert_eq!(output, [0x11223344, 0x11223345, 0x11223346]);
                        }
                    }
                    assert_eq!(snapshot(p, 64), expected);
                }
            }
        }
    }

    extern "thiscall" fn query(this: *mut u8, id: u32, output: *mut u32) -> u64 {
        // SAFETY: every query output is initialized and points at one writable word.
        unsafe {
            event(this, &[0, id, *output]);
            *output = HELD.with(|v| v.get());
        }
        HIGH | u64::from(QUERY_RESULT.with(|v| v.get()))
    }
    #[test]
    fn draw_context_query_adjusts_known_interfaces_delegates_and_keeps_rejected_output() {
        let mut owner = Object::new(64);
        let p = owner.ptr();
        let mut fallback = Object::new(4);
        let fallback = fallback.ptr();
        let primary = table();
        let mut secondary = table();
        secondary[0] = query as *const () as usize;
        // SAFETY: each query has a writable output and valid primary/fallback slots.
        unsafe {
            put_word(p, 0, primary.as_ptr() as u32);
            put_word(fallback, 0, secondary.as_ptr() as u32);
            for id in [0u32, 1, 101, 0x20732180, 0xab300b2b, u32::MAX] {
                for delegate in [0u32, fallback as u32] {
                    put_word(p, 0x14, delegate);
                    let mut output = 0x87654321;
                    HELD.with(|v| v.set(0x76543210));
                    QUERY_RESULT.with(|v| v.set(0x12345601));
                    let before = snapshot(p, 64);
                    let result = invoke(
                        fc4p_0082aea0 as *const () as usize,
                        p,
                        &[id, &mut output as *mut u32 as u32],
                    );
                    if matches!(id, 1 | 0x20732180 | 0xab300b2b) {
                        assert_eq!(result, ((VALUE & !255) | 1, (HIGH >> 32) as u32));
                        assert_eq!(
                            output,
                            if id == 0xab300b2b {
                                p as u32 + 12
                            } else {
                                p as u32
                            }
                        );
                        assert_eq!(events(), vec![(p as u32, vec![4])]);
                    } else if delegate != 0 {
                        assert_eq!(result, (0x12345601, (HIGH >> 32) as u32));
                        assert_eq!(output, 0x76543210);
                        assert_eq!(events(), vec![(delegate, vec![0, id, 0x87654321])]);
                    } else {
                        assert_eq!(result, (id & !255, EDX));
                        assert_eq!(output, 0x87654321);
                        assert!(events().is_empty());
                    }
                    assert_eq!(snapshot(p, 64), before);
                }
            }
        }
    }
    #[test]
    fn scheduler_query_recognizes_only_three_ids_and_keeps_failure_arithmetic_in_upper_bytes() {
        let mut owner = Object::new(16);
        let p = owner.ptr();
        let table = table();
        // SAFETY: the primary interface and output word remain live for every query.
        unsafe {
            put_word(p, 0, table.as_ptr() as u32);
            for id in [
                0u32,
                1,
                100,
                101,
                102,
                0x23ea70a0,
                0x23ea70a1,
                0x23ea70a2,
                u32::MAX,
            ] {
                let mut output = 0x87654321u32;
                let before = snapshot(p, 16);
                let result = invoke(
                    fc4p_0087931b as *const () as usize,
                    p,
                    &[id, &mut output as *mut u32 as u32],
                );
                if matches!(id, 1 | 101 | 0x23ea70a1) {
                    assert_eq!(result, ((VALUE & !255) | 1, (HIGH >> 32) as u32));
                    assert_eq!(output, p as u32 + if id == 0x23ea70a1 { 4 } else { 0 });
                    assert_eq!(events(), vec![(p as u32, vec![4])]);
                } else {
                    assert_eq!(result, (id.wrapping_sub(0x23ea70a1) & !255, EDX));
                    assert_eq!(output, 0x87654321);
                    assert!(events().is_empty());
                }
                assert_eq!(snapshot(p, 16), before);
            }
        }
    }
    extern "thiscall" fn acquire(this: *mut u8, lock: *mut u8) -> u64 {
        event(lock, &[0x48c9]);
        // SAFETY: the original scope contains two words, with the acquired lock in its second.
        unsafe {
            put_word(this, 0, 0x12345678);
            put_word(this, 4, HELD.with(|v| v.get()));
        }
        HIGH | u64::from(VALUE)
    }
    extern "thiscall" fn reference_unlocked(this: *mut u8) -> u64 {
        // SAFETY: OWNER identifies the live reference-count allocation used by this test.
        unsafe {
            event(this, &[8, word(OWNER.with(|v| v.get()) as *mut u8, 0x80)]);
        }
        HIGH | u64::from(VALUE)
    }
    #[test]
    fn locked_reference_counts_wrap_on_increment_and_never_decrement_zero_or_one() {
        let _acquire = fake(0x0048c9f0, acquire as *const () as usize);
        let mut owner = Object::new(0x100);
        let p = owner.ptr();
        let mut lock = Object::new(4);
        let lock = lock.ptr();
        let mut table = table();
        table[2] = reference_unlocked as *const () as usize;
        HELD.with(|v| v.set(lock as u32));
        OWNER.with(|v| v.set(p as u32));
        // SAFETY: the counter and lock table are fully allocated.
        unsafe {
            put_word(lock, 0, table.as_ptr() as u32);
            for old in [0u32, 1, 2, 0x7fffffff, 0x80000000, u32::MAX] {
                for increment in [false, true] {
                    paint(p, 0x100);
                    put_word(p, 0x80, old);
                    let value = if increment {
                        old.wrapping_add(1)
                    } else {
                        old.saturating_sub(1)
                    };
                    let stored = if increment || old > 1 { value } else { old };
                    let mut expected = snapshot(p, 0x100);
                    expected_word(&mut expected, 0x80, stored);
                    let function = if increment {
                        fc4p_00879355 as *const ()
                    } else {
                        fc4p_00879383 as *const ()
                    };
                    assert_eq!(
                        invoke(function as usize, p, &[]),
                        (value, (HIGH >> 32) as u32)
                    );
                    assert_eq!(
                        events(),
                        vec![
                            (p as u32 + 0xe0, vec![0x48c9]),
                            (lock as u32, vec![8, stored])
                        ]
                    );
                    assert_eq!(snapshot(p, 0x100), expected);
                }
            }
        }
    }
    extern "thiscall" fn rectangle(this: *mut u8) -> u64 {
        event(this, &[0x30]);
        HIGH | u64::from(RECT.with(|v| v.get()))
    }
    extern "thiscall" fn surface_format(this: *mut u8, output: *mut u32) -> u64 {
        event(this, &[0x3c]);
        // SAFETY: the format output has two words; only its first is required here.
        unsafe {
            *output = 0x89abcdef;
        }
        HIGH | u64::from(output as u32)
    }
    slot!(surface_stride, 0x44);
    #[test]
    fn surface_replacement_caches_geometry_format_stride_and_balances_both_references() {
        let mut owner = Object::new(80);
        let p = owner.ptr();
        let mut old_target = Object::new(4);
        let old_target = old_target.ptr();
        let mut target = Object::new(4);
        let target = target.ptr();
        let mut old_surface = Object::new(4);
        let old_surface = old_surface.ptr();
        let mut surface = Object::new(4);
        let surface = surface.ptr();
        let mut bounds = [0x80000000u32, 10, 30, 0xfffffff0];
        let target_table = table();
        let mut surface_table = table();
        surface_table[0x30 / 4] = rectangle as *const () as usize;
        surface_table[0x3c / 4] = surface_format as *const () as usize;
        surface_table[0x44 / 4] = surface_stride as *const () as usize;
        RECT.with(|v| v.set(bounds.as_mut_ptr() as u32));
        // SAFETY: both reference pairs and geometry records are live throughout each replacement.
        unsafe {
            for t in [old_target, target] {
                put_word(t, 0, target_table.as_ptr() as u32);
            }
            for s in [old_surface, surface] {
                put_word(s, 0, surface_table.as_ptr() as u32);
            }
            for previous_target in [0u32, old_target as u32, target as u32] {
                for previous_surface in [0u32, old_surface as u32, surface as u32] {
                    paint(p, 80);
                    put_word(p, 4, previous_target);
                    put_word(p, 8, previous_surface);
                    let mut expected = snapshot(p, 80);
                    for (offset, value) in [
                        (4, target as u32),
                        (8, surface as u32),
                        (0xc, bounds[0]),
                        (0x10, bounds[1]),
                        (0x14, bounds[2]),
                        (0x18, bounds[3]),
                        (0x20, 0x89abcdef),
                        (0x38, VALUE),
                        (0x2c, bounds[2].wrapping_sub(bounds[0])),
                        (0x30, bounds[3].wrapping_sub(bounds[1])),
                    ] {
                        expected_word(&mut expected, offset, value);
                    }
                    assert_eq!(
                        invoke(
                            fc4p_0082af70 as *const () as usize,
                            p,
                            &[target as u32, surface as u32]
                        ),
                        (VALUE, (HIGH >> 32) as u32)
                    );
                    let mut expected_calls = vec![];
                    for (old, new) in [
                        (previous_target, target as u32),
                        (previous_surface, surface as u32),
                    ] {
                        if old != new {
                            expected_calls.push((new, vec![4]));
                            if old != 0 {
                                expected_calls.push((old, vec![8]));
                            }
                        }
                    }
                    expected_calls.extend([
                        (surface as u32, vec![0x30]),
                        (surface as u32, vec![0x3c]),
                        (surface as u32, vec![0x44]),
                        (target as u32, vec![0x44, 0]),
                    ]);
                    assert_eq!(events(), expected_calls);
                    assert_eq!(snapshot(p, 80), expected);
                }
            }
        }
    }
    #[test]
    fn rectangle_refresh_resets_target_then_rebinds_current_surface_and_flushes() {
        let mut owner = Object::new(64);
        let p = owner.ptr();
        let mut target = Object::new(4);
        let target = target.ptr();
        let mut surface = Object::new(4);
        let surface = surface.ptr();
        let target_table = table();
        let mut surface_table = table();
        surface_table[0x30 / 4] = rectangle as *const () as usize;
        let mut bounds = [1u32, 2, 3, 4];
        RECT.with(|v| v.set(bounds.as_mut_ptr() as u32));
        // SAFETY: the geometry and both interface tables remain allocated during refresh.
        unsafe {
            paint(p, 64);
            put_word(target, 0, target_table.as_ptr() as u32);
            put_word(surface, 0, surface_table.as_ptr() as u32);
            put_word(p, 4, target as u32);
            put_word(p, 8, surface as u32);
            let mut expected = snapshot(p, 64);
            for (i, value) in bounds.into_iter().enumerate() {
                expected_word(&mut expected, 12 + i * 4, value);
            }
            assert_eq!(
                invoke(fc4p_0082b030 as *const () as usize, p, &[]),
                (VALUE, (HIGH >> 32) as u32)
            );
            assert_eq!(
                events(),
                vec![
                    (surface as u32, vec![0x30]),
                    (target as u32, vec![0x44, 0]),
                    (target as u32, vec![0x44, surface as u32]),
                    (target as u32, vec![0xa8])
                ]
            );
            assert_eq!(snapshot(p, 64), expected);
        }
    }
    extern "thiscall" fn color_channels(
        this: *mut u8,
        red: *mut u8,
        green: *mut u8,
        blue: *mut u8,
        alpha: *mut u8,
    ) -> u64 {
        event(this, &[0x10]);
        // SAFETY: the color caller supplies four distinct writable bytes.
        unsafe {
            *red = 0x12;
            *green = 0x34;
            *blue = 0x56;
            *alpha = 0x78;
        }
        answer(0x10)
    }
    #[test]
    fn optional_color_interface_packs_argb_defaults_on_failure_and_releases_even_failed_queries() {
        let mut owner = Object::new(32);
        let p = owner.ptr();
        let mut source = Object::new(4);
        let source = source.ptr();
        let mut color = Object::new(4);
        let color = color.ptr();
        let mut target = Object::new(4);
        let target = target.ptr();
        let mut source_table = table();
        source_table[0] = query as *const () as usize;
        let mut color_table = table();
        color_table[0x10 / 4] = color_channels as *const () as usize;
        let target_table = table();
        // SAFETY: the holder, source, and optional color interfaces have the tested slots.
        unsafe {
            put_word(p, 4, target as u32);
            put_word(source, 0, source_table.as_ptr() as u32);
            put_word(color, 0, color_table.as_ptr() as u32);
            put_word(target, 0, target_table.as_ptr() as u32);
            for accepted in [0u32, 0x12345600, 0x12345601] {
                for color_ok in [0u32, 0x87654300, 0x87654301] {
                    for retained in [0u32, color as u32] {
                        if accepted as u8 != 0 && retained == 0 {
                            continue;
                        }
                        QUERY_RESULT.with(|v| v.set(accepted));
                        HELD.with(|v| v.set(retained));
                        set_answer(0x10, color_ok);
                        let before = snapshot(p, 32);
                        assert_eq!(
                            invoke(fc4p_0082b080 as *const () as usize, p, &[source as u32]),
                            (VALUE, (HIGH >> 32) as u32)
                        );
                        let mut expected = vec![(source as u32, vec![0, 0x86d72b57, 0])];
                        if accepted as u8 != 0 {
                            expected.push((color as u32, vec![0x10]));
                        }
                        expected.push((
                            target as u32,
                            vec![
                                0x54,
                                if accepted as u8 != 0 && color_ok as u8 != 0 {
                                    0x78123456
                                } else {
                                    u32::MAX
                                },
                            ],
                        ));
                        if retained != 0 {
                            expected.push((color as u32, vec![8]));
                        }
                        assert_eq!(events(), expected);
                        assert_eq!(snapshot(p, 32), before);
                    }
                }
            }
        }
    }
    extern "thiscall" fn lock_record(this: *mut u8, output: *mut u8, flags: u32) -> u64 {
        event(this, &[0xb8, flags]);
        // SAFETY: this record is the caller's writable buffer description.
        unsafe {
            put_word(output, 0, 0x55667788);
        }
        answer(0xb8)
    }
    #[test]
    fn draw_context_lock_initializes_only_at_zero_checks_low_bytes_and_keeps_all_flag_bits() {
        let mut owner = Object::new(80);
        let p = owner.ptr();
        let mut target = Object::new(4);
        let target = target.ptr();
        let mut table = table();
        table[0xb8 / 4] = lock_record as *const () as usize;
        // SAFETY: the context fields and target callback table have their original sizes.
        unsafe {
            put_word(target, 0, table.as_ptr() as u32);
            for count in [0u32, 1, u32::MAX] {
                for ready in [0u32, 0x12345600, 0x12345601] {
                    for active in [0u32, 0xabcdef00, 0xabcdef01] {
                        for flags in [0u32, 0x7fff, 0x8000, 0x10000, 0x80000000, u32::MAX] {
                            paint(p, 80);
                            put_word(p, 0x10, target as u32);
                            put_word(p, 0x14, 0x76543210);
                            put_word(p, 0x28, count);
                            set_answer(0x40, ready);
                            set_answer(0xa4, active);
                            let mut expected = snapshot(p, 80);
                            if count == 0 {
                                expected[0x49] = u8::from(ready as u8 == 0 || active as u8 != 0);
                            }
                            expected_word(&mut expected, 0x28, count.wrapping_add(1));
                            expected_word(&mut expected, 0x30, 0x55667788);
                            assert_eq!(
                                invoke(fc4p_0082b120 as *const () as usize, p, &[flags]),
                                ((VALUE & !255) | 1, (HIGH >> 32) as u32)
                            );
                            let mut expected_calls = vec![];
                            if count == 0 {
                                expected_calls.push((target as u32, vec![0x40]));
                                if ready as u8 != 0 {
                                    expected_calls.push((target as u32, vec![0xa4]));
                                }
                                expected_calls.extend([
                                    (target as u32, vec![0x44, 0x76543210]),
                                    (target as u32, vec![0xa4]),
                                ]);
                                if active as u8 != 0 {
                                    expected_calls.push((target as u32, vec![0xac]));
                                }
                            }
                            expected_calls.push((
                                target as u32,
                                vec![
                                    0xb8,
                                    ((flags >> 15) & 0xffffff00) | u32::from(flags & 0x8000 == 0),
                                ],
                            ));
                            assert_eq!(events(), expected_calls);
                            assert_eq!(snapshot(p, 80), expected);
                        }
                    }
                }
            }
        }
    }
    extern "thiscall" fn expand(this: *mut u8, pixel: u32) -> u64 {
        event(this, &[0xabf0, pixel]);
        HIGH | 0x13579bdf
    }
    #[test]
    fn fill_surface_converts_color_inside_begin_optional_clip_and_end_sequence() {
        let _expand = fake(0x0082abf0, expand as *const () as usize);
        let mut owner = Object::new(80);
        let p = owner.ptr();
        let mut target = Object::new(4);
        let target = target.ptr();
        let table = table();
        // SAFETY: the target supports the complete fill protocol and the owner retains it.
        unsafe {
            paint(p, 80);
            put_word(p, 0x10, target as u32);
            put_word(target, 0, table.as_ptr() as u32);
            for clip in [0u32, 0x87654321] {
                let before = snapshot(p, 80);
                assert_eq!(
                    invoke(
                        fc4p_0082b280 as *const () as usize,
                        p,
                        &[0x11223344, 0x55667788, clip]
                    ),
                    ((VALUE & !255) | 1, (HIGH >> 32) as u32)
                );
                let mut expected = vec![(target as u32, vec![0xc])];
                if clip != 0 {
                    expected.push((target as u32, vec![0x14, clip]));
                }
                expected.extend([
                    (p as u32, vec![0xabf0, 0x11223344]),
                    (target as u32, vec![0x50, 0, 0x13579bdf]),
                    (target as u32, vec![0x8c, 0x55667788]),
                    (target as u32, vec![0x24]),
                    (target as u32, vec![0xa0]),
                ]);
                assert_eq!(events(), expected);
                assert_eq!(snapshot(p, 80), before);
            }
        }
    }
    extern "thiscall" fn fill_coordinates(
        this: *mut u8,
        first: u32,
        rectangle: *mut u8,
        last: u32,
    ) -> u64 {
        // SAFETY: the caller's four-word temporary is valid until the slot returns.
        unsafe {
            event(
                this,
                &[
                    0x50,
                    first,
                    word(rectangle, 0),
                    word(rectangle, 4),
                    word(rectangle, 8),
                    word(rectangle, 12),
                    last,
                ],
            );
        }
        HIGH | u64::from(VALUE)
    }
    #[test]
    fn coordinate_fill_forwards_six_words_without_normalizing_rectangle() {
        let mut owner = Object::new(32);
        let p = owner.ptr();
        let mut table = table();
        table[0x50 / 4] = fill_coordinates as *const () as usize;
        // SAFETY: the primary table has this three-argument fill slot.
        unsafe {
            put_word(p, 0, table.as_ptr() as u32);
            let before = snapshot(p, 32);
            let args = [0x12345678, u32::MAX, 0x80000000, 0, 0x7fffffff, 0x87654321];
            assert_eq!(
                invoke(fc4p_0082b2f0 as *const () as usize, p, &args),
                (VALUE, (HIGH >> 32) as u32)
            );
            assert_eq!(
                events(),
                vec![(
                    p as u32,
                    vec![0x50, args[0], args[1], args[2], args[3], args[4], args[5]]
                )]
            );
            assert_eq!(snapshot(p, 32), before);
        }
    }
    extern "thiscall" fn bind_source(this: *mut u8, source: u32) -> u64 {
        event(this, &[0x124, source]);
        HIGH | u64::from(VALUE)
    }
    extern "thiscall" fn copy_rectangles(
        this: *mut u8,
        source: u32,
        dest: *mut u8,
        rect: *mut u8,
    ) -> u64 {
        // SAFETY: both four-word rectangles are readable during this slot call.
        unsafe {
            event(
                this,
                &[
                    0x98,
                    source,
                    word(dest, 0),
                    word(dest, 4),
                    word(dest, 8),
                    word(dest, 12),
                    rect as u32,
                ],
            );
        }
        HIGH | u64::from(VALUE)
    }
    #[test]
    fn surface_copy_translates_source_rectangle_with_wrapping_arithmetic_and_optional_clip() {
        let mut owner = Object::new(80);
        let p = owner.ptr();
        let mut target = Object::new(4);
        let target = target.ptr();
        let mut target_table = table();
        target_table[0x98 / 4] = copy_rectangles as *const () as usize;
        let mut interface_table = table();
        interface_table[0x24 / 4] = bind_source as *const () as usize;
        // SAFETY: the embedded interface and target occupy their original fields.
        unsafe {
            paint(p, 80);
            put_word(p, 0xc, interface_table.as_ptr() as u32);
            put_word(p, 0x10, target as u32);
            put_word(target, 0, target_table.as_ptr() as u32);
            for clip in [0u32, 0x89abcdef] {
                for (point, source) in [
                    ([100u32, 200], [10u32, 20, 40, 60]),
                    ([u32::MAX, 0x80000000], [0x7fffffffu32, 1, 0x80000001, 0]),
                ] {
                    let mut point = point;
                    let mut source = source;
                    let before = snapshot(p, 80);
                    assert_eq!(
                        invoke(
                            fc4p_0082b690 as *const () as usize,
                            p,
                            &[
                                0x12345678,
                                point.as_mut_ptr() as u32,
                                source.as_mut_ptr() as u32,
                                clip
                            ]
                        ),
                        ((VALUE & !255) | 1, (HIGH >> 32) as u32)
                    );
                    let mut expected = vec![(target as u32, vec![0xc])];
                    if clip != 0 {
                        expected.push((target as u32, vec![0x14, clip]));
                    }
                    expected.extend([
                        (p as u32 + 12, vec![0x124, 0x12345678]),
                        (
                            target as u32,
                            vec![
                                0x98,
                                0x12345678,
                                point[0],
                                point[1],
                                source[2].wrapping_add(point[0].wrapping_sub(source[0])),
                                source[3].wrapping_add(point[1].wrapping_sub(source[1])),
                                source.as_mut_ptr() as u32,
                            ],
                        ),
                        (target as u32, vec![0x24]),
                        (target as u32, vec![0xa0]),
                    ]);
                    assert_eq!(events(), expected);
                    assert_eq!(snapshot(p, 80), before);
                }
            }
        }
    }
    extern "stdcall" fn set_cursor(handle: u32) -> u64 {
        event(core::ptr::null_mut(), &[0x5c0, handle]);
        HIGH | u64::from(VALUE)
    }
    #[test]
    fn enabled_cursor_chooses_native_handle_or_provider_and_sign_extends_coordinates() {
        let mut cursor_function = set_cursor as *const () as usize;
        let _cursor = fake_data(0x00a80578, (&mut cursor_function as *mut usize).cast());
        let mut owner = Object::new(64);
        let p = owner.ptr();
        let mut provider = Object::new(4);
        let provider = provider.ptr();
        let table = table();
        // SAFETY: the optional provider and cursor object's fields remain live during each call.
        unsafe {
            put_word(provider, 0, table.as_ptr() as u32);
            for enabled in [0u32, 0x12345600, 1, 0x12345601] {
                for handle in [0u32, 0x76543210] {
                    for present in [false, true] {
                        for force in [false, true] {
                            for ready in [0u32, 0x12345600, 0x12345601] {
                                paint(p, 64);
                                *p.add(8) = u8::from(force);
                                put_word(p, 12, if present { provider as u32 } else { 0 });
                                put_word(p, 0x24, 0x87654321);
                                put_word(p, 0x28, 0x8001ffff);
                                put_word(p, 0x2c, handle);
                                set_answer(0x80, ready);
                                let before = snapshot(p, 64);
                                let queried =
                                    enabled as u8 != 0 && handle != 0 && !force && present;
                                let native = enabled as u8 != 0
                                    && handle != 0
                                    && (!queried || ready as u8 != 0);
                                let emulated = enabled as u8 != 0
                                    && ((handle == 0 && present) || (queried && ready as u8 == 0));
                                assert_eq!(
                                    invoke(fc4p_00891cae as *const () as usize, p, &[enabled]),
                                    if native || emulated {
                                        (VALUE, (HIGH >> 32) as u32)
                                    } else {
                                        (EAX, EDX)
                                    }
                                );
                                let mut expected = vec![];
                                if queried {
                                    expected.push((provider as u32, vec![0x80]));
                                }
                                if native {
                                    expected.push((0, vec![0x5c0, handle]));
                                }
                                if emulated {
                                    expected.push((
                                        provider as u32,
                                        vec![
                                            0x7c,
                                            0x87654321,
                                            if handle == 0 { u32::MAX } else { 0 },
                                            if handle == 0 { 0xffff8001 } else { 0 },
                                        ],
                                    ));
                                }
                                assert_eq!(events(), expected);
                                assert_eq!(snapshot(p, 64), before);
                            }
                        }
                    }
                }
            }
        }
    }
    extern "thiscall" fn clear_handle(this: *mut u8, value: u32) -> u64 {
        // SAFETY: the owner has already installed its final vtable.
        unsafe {
            event(this, &[0x1d48, value, word(this, 0)]);
        }
        HIGH | u64::from(VALUE)
    }
    extern "thiscall" fn destroy_string(this: *mut u8) -> u64 {
        // SAFETY: observe the reset retained field through its containing allocation.
        unsafe {
            event(this, &[0x4250, word(this.wrapping_sub(0x10), 0x24)]);
        }
        HIGH | u64::from(VALUE)
    }
    #[test]
    fn cursor_destruction_resets_vtable_handle_and_retained_reference_before_embedded_string() {
        let _clear = fake(0x00891d48, clear_handle as *const () as usize);
        let _destroy = fake(0x00414250, destroy_string as *const () as usize);
        let mut owner = Object::new(64);
        let p = owner.ptr();
        let mut held = Object::new(4);
        let held = held.ptr();
        let table = table();
        // SAFETY: the owner, optional interface, and embedded string fields are allocated.
        unsafe {
            put_word(held, 0, table.as_ptr() as u32);
            for handle in [0u32, 0x12345678] {
                for retained in [0u32, held as u32] {
                    paint(p, 64);
                    put_word(p, 0x24, retained);
                    put_word(p, 0x2c, handle);
                    let mut expected = snapshot(p, 64);
                    expected_word(&mut expected, 0, 0x00ac52d4);
                    expected_word(&mut expected, 0x24, 0);
                    assert_eq!(
                        invoke(fc4p_008922f0 as *const () as usize, p, &[]),
                        (VALUE, (HIGH >> 32) as u32)
                    );
                    let mut expected_calls = vec![];
                    if handle != 0 {
                        expected_calls.push((p as u32, vec![0x1d48, 0, 0x00ac52d4]));
                    }
                    if retained != 0 {
                        expected_calls.push((held as u32, vec![8]));
                    }
                    expected_calls.push((p as u32 + 0x10, vec![0x4250, 0]));
                    assert_eq!(events(), expected_calls);
                    assert_eq!(snapshot(p, 64), expected);
                }
            }
        }
    }

    #[unsafe(naked)]
    unsafe extern "C" fn preserve_registers() {
        core::arch::naked_asm!("ret");
    }
    #[unsafe(naked)]
    unsafe extern "C" fn clear_low_byte() {
        core::arch::naked_asm!("xor al, al", "ret");
    }
    #[test]
    fn partial_callee_results_retain_original_register_inputs() {
        let mut owner = Object::new(0x140);
        let p = owner.ptr();
        let mut message = Object::new(4);
        let m = message.ptr();
        let _refresh = fake(0x007a93e0, clear_low_byte as *const () as usize);
        let _string = fake(0x00414250, preserve_registers as *const () as usize);
        let mut table = table();
        table[1] = preserve_registers as *const () as usize;
        // SAFETY: all tables and outputs are live; the naked fakes intentionally preserve register bits.
        unsafe {
            put_word(p, 0, table.as_ptr() as u32);
            put_word(m, 0, table.as_ptr() as u32);
            set_answer(0x10, 0x426840a0);
            set_answer(0x24, 0x87654321);
            put_word(p, 0xe0, 0x87654321);
            put_word(p, 0x120, 1);
            assert_eq!(
                invoke(
                    fc4p_007a9810 as *const () as usize,
                    p.add(0xdc),
                    &[m as u32]
                ),
                (0x87654300, 1)
            );
            assert_eq!(
                events(),
                vec![(m as u32, vec![0x10]), (m as u32, vec![0x24])]
            );
            let mut output = 0u32;
            for id in [1u32, 0x20732180, 0xab300b2b] {
                assert_eq!(
                    invoke(
                        fc4p_0082aea0 as *const () as usize,
                        p,
                        &[id, &mut output as *mut u32 as u32]
                    ),
                    (
                        (table.as_ptr() as u32 & !255) | 1,
                        if id == 0x20732180 {
                            EDX
                        } else {
                            &mut output as *mut u32 as u32
                        }
                    )
                );
            }
            put_word(p, 0x24, 0);
            put_word(p, 0x2c, 0);
            assert_eq!(
                invoke(fc4p_008922f0 as *const () as usize, p, &[]),
                (EAX, EDX)
            );
            assert!(events().is_empty());
        }
    }

    thread_local! {
        static FUNDING: Cell<(i64, i64)> = const { Cell::new((700, 350)) };
        static USAGE: Cell<(u32, u32)> = const { Cell::new((8000, 10000)) };
        static RECORD: Cell<u32> = const { Cell::new(0) };
        static ALLOCATIONS: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
    }
    extern "thiscall" fn port_usage(
        this: *mut u8,
        id: u32,
        used: *mut u32,
        capacity: *mut u32,
    ) -> u64 {
        // SAFETY: the caller supplies two initialized, writable output words.
        unsafe {
            event(this, &[0x9770, id, *used, *capacity]);
            let (a, b) = USAGE.with(|v| v.get());
            *used = a;
            *capacity = b;
        }
        HIGH | u64::from(QUERY_RESULT.with(|v| v.get()))
    }
    extern "thiscall" fn port_find(this: *mut u8, id: u32, output: *mut u32) -> u64 {
        // SAFETY: the optional stage handle is initialized to zero by the caller.
        unsafe {
            event(this, &[0x9590, id, *output]);
            *output = RECORD.with(|v| v.get());
        }
        HIGH | u64::from(QUERY_RESULT.with(|v| v.get()))
    }
    extern "thiscall" fn funding_item(this: *mut u8, category: u32) -> u64 {
        event(this, &[0x50, category]);
        HIGH | u64::from(HELD.with(|v| v.get()))
    }
    extern "thiscall" fn funding_current(this: *mut u8) -> u64 {
        event(this, &[0x2c]);
        FUNDING.with(|v| v.get().1 as u64)
    }
    extern "thiscall" fn funding_full(this: *mut u8) -> u64 {
        event(this, &[0x28]);
        FUNDING.with(|v| v.get().0 as u64)
    }
    fn funding_table() -> [usize; 128] {
        let mut t = table();
        t[0x50 / 4] = funding_item as *const () as usize;
        t[0x2c / 4] = funding_current as *const () as usize;
        t[0x28 / 4] = funding_full as *const () as usize;
        t
    }
    #[test]
    fn port_usage_keeps_core_half_and_above_full_funding_queries_and_unsigned_limits() {
        use crate::x87::F80;
        let _fpu = GameFpu::new();
        let _base = fake(0x006e9770, port_usage as *const () as usize);
        let mut zero = 0f32;
        let mut bias = 4294967296f32;
        let _zero = fake_data(0x00a81054, (&mut zero as *mut f32).cast());
        let _bias = fake_data(0x00a80aa8, (&mut bias as *mut f32).cast());
        let mut owner = Object::new(80);
        let p = owner.ptr();
        let mut department = Object::new(4);
        let department = department.ptr();
        let mut item = Object::new(4);
        let item = item.ptr();
        let table = funding_table();
        // SAFETY: the developer's funding provider and both outputs are live during the query.
        unsafe {
            put_word(p, 0x34, department as u32);
            put_word(department, 0, table.as_ptr() as u32);
            put_word(item, 0, table.as_ptr() as u32);
            for (used, cap, full, current, literal) in [
                (8000u32, 10000u32, 700i64, 350i64, Some((5000u32, 5000u32))),
                (8000, 10000, 700, 840, Some((8000, 12000))),
                (4000, 10000, 700, 350, Some((4000, 5000))),
                (6000, 10000, 700, 350, Some((5000, 5000))),
                (u32::MAX, 0x80000000, 1, 1, None),
                (u32::MAX, u32::MAX, 1, 1, None),
                (u32::MAX, u32::MAX, 1, i64::MAX, None),
                (1, 31, i64::MAX, 1, None),
                (100, 200, 0, 1, None),
                (100, 200, -1, i64::MIN, None),
                (100, 200, 700, -350, None),
            ] {
                USAGE.with(|v| v.set((used, cap)));
                FUNDING.with(|v| v.set((full, current)));
                for accepted in [0u32, 0x12345600, 0x12345601] {
                    for present in [false, true] {
                        QUERY_RESULT.with(|v| v.set(accepted));
                        HELD.with(|v| v.set(if present { item as u32 } else { 0 }));
                        let mut outputs = [0x11223344u32, 0x55667788];
                        let before = snapshot(p, 80);
                        let result = invoke(
                            fc4p_00700fe0 as *const () as usize,
                            p,
                            &[
                                0x87654321,
                                outputs.as_mut_ptr() as u32,
                                outputs.as_mut_ptr().add(1) as u32,
                            ],
                        );
                        let scaled = accepted as u8 != 0 && present && full > 0;
                        let converted = if scaled {
                            let mut amount = F80::from_i32(cap as i32);
                            if (cap as i32) < 0 {
                                amount = amount + F80::from_f32(bias);
                            }
                            ((amount / F80::from_i64(full))
                                * F80::from_f32(F80::from_i64(current).to_f32()))
                            .ftol() as u64
                        } else {
                            u64::from(cap)
                        };
                        let limit = converted as u32;
                        let expected = if accepted as u8 != 0 {
                            [used.min(limit), limit]
                        } else {
                            [used, cap]
                        };
                        assert_eq!(outputs, expected);
                        if let Some(literal) = literal {
                            if scaled {
                                assert_eq!((outputs[0], outputs[1]), literal);
                            }
                        }
                        let high = if accepted as u8 == 0 || !present {
                            (HIGH >> 32) as u32
                        } else if scaled {
                            (converted >> 32) as u32
                        } else {
                            (full as u64 >> 32) as u32
                        };
                        assert_eq!(
                            result,
                            (
                                if accepted as u8 == 0 {
                                    accepted
                                } else {
                                    (limit & !255) | u32::from(accepted as u8)
                                },
                                high
                            )
                        );
                        let mut expected_calls = vec![(
                            p as u32 + 4,
                            vec![0x9770, 0x87654321, 0x11223344, 0x55667788],
                        )];
                        if accepted as u8 != 0 {
                            expected_calls.push((department as u32, vec![0x50, 1]));
                            if present {
                                expected_calls
                                    .extend([(item as u32, vec![0x2c]), (item as u32, vec![0x28])]);
                            }
                        }
                        assert_eq!(events(), expected_calls);
                        assert_eq!(snapshot(p, 80), before);
                    }
                }
            }
        }
    }
    #[test]
    fn maintenance_cost_reads_signed_stage_pair_scales_in_order_and_queries_funding_on_a_miss() {
        use crate::x87::F80;
        let _fpu = GameFpu::new();
        let _find = fake(0x006e9590, port_find as *const () as usize);
        let mut zero = 0f32;
        let _zero = fake_data(0x00a81054, (&mut zero as *mut f32).cast());
        let mut owner = Object::new(80);
        let p = owner.ptr();
        let mut department = Object::new(4);
        let department = department.ptr();
        let mut item = Object::new(4);
        let item = item.ptr();
        let mut record = Object::new(12);
        let record = record.ptr();
        let mut stage_owner = Object::new(24);
        let stage_owner = stage_owner.ptr();
        let mut stages = [0xa5a5a5a5u32; 24];
        let table = funding_table();
        RECORD.with(|v| v.set(record as u32));
        // SAFETY: index one points at the second 32-byte stage record in the owned table.
        unsafe {
            put_word(p, 0x34, department as u32);
            put_word(department, 0, table.as_ptr() as u32);
            put_word(item, 0, table.as_ptr() as u32);
            put_word(record, 4, 1);
            put_word(record, 8, stage_owner as u32);
            put_word(stage_owner, 0x14, stages.as_mut_ptr() as u32);
            for cost in [0i64, 700, -700, 0x123456789abcdef, i64::MIN, i64::MAX] {
                stages[10] = cost as u32;
                stages[11] = (cost as u64 >> 32) as u32;
                for (full, current) in [
                    (700i64, 350i64),
                    (700, 840),
                    (0, 100),
                    (-1, 1),
                    (1, i64::MAX),
                ] {
                    FUNDING.with(|v| v.set((full, current)));
                    for accepted in [0u32, 0x12345600, 0x12345601] {
                        for present in [false, true] {
                            QUERY_RESULT.with(|v| v.set(accepted));
                            HELD.with(|v| v.set(if present { item as u32 } else { 0 }));
                            let stored = if accepted as u8 != 0 { cost } else { 0 };
                            let expected = if present && full > 0 {
                                ((F80::from_i64(stored) / F80::from_i64(full))
                                    * F80::from_f32(F80::from_i64(current).to_f32()))
                                .ftol() as u64
                            } else {
                                stored as u64
                            };
                            let before = snapshot(p, 80);
                            assert_eq!(
                                invoke(fc4p_00701830 as *const () as usize, p, &[0x87654321]),
                                (expected as u32, (expected >> 32) as u32)
                            );
                            if cost == 700 && full == 700 && accepted as u8 != 0 && present {
                                assert_eq!(expected, current as u64);
                            }
                            let mut expected_calls = vec![
                                (p as u32 + 4, vec![0x9590, 0x87654321, 0]),
                                (department as u32, vec![0x50, 1]),
                            ];
                            if present {
                                expected_calls
                                    .extend([(item as u32, vec![0x2c]), (item as u32, vec![0x28])]);
                            }
                            assert_eq!(events(), expected_calls);
                            assert_eq!(snapshot(p, 80), before);
                        }
                    }
                }
            }
        }
    }
    extern "C" fn deque_allocate(bytes: u32) -> u64 {
        // SAFETY: the allocation callback observes the already-initialized map-size field.
        unsafe {
            event(
                core::ptr::null_mut(),
                &[
                    0xa110,
                    bytes,
                    word(OWNER.with(|v| v.get()) as *mut u8, 0x24),
                ],
            );
        }
        HIGH | u64::from(ALLOCATIONS.with(|v| v.borrow_mut().remove(0)))
    }
    #[test]
    fn deque_initialization_centers_map_allocates_exact_blocks_and_keeps_unused_bytes() {
        let _allocate = fake(0x0090cf54, deque_allocate as *const () as usize);
        let mut owner = Object::new(64);
        let p = owner.ptr();
        let mut map = [0xa5a5a5a5u32; 16];
        let mut blocks = [[0x76543210u32; 32]; 8];
        OWNER.with(|v| v.set(p as u32));
        // SAFETY: tested sizes need at most eight blocks and a sixteen-word map.
        unsafe {
            for size in [0u32, 1, 31, 32, 33, 159, 160, 255] {
                for null_blocks in [false, true] {
                    paint(p, 64);
                    map.fill(0xa5a5a5a5);
                    let block_count = (size >> 5) + 1;
                    let map_count = (block_count + 2).max(8);
                    let center = (map_count - block_count) / 2;
                    let addresses: Vec<u32> = blocks
                        .iter_mut()
                        .take(block_count as usize)
                        .map(|b| {
                            if null_blocks {
                                0
                            } else {
                                b.as_mut_ptr() as u32
                            }
                        })
                        .collect();
                    let mut allocations = vec![map.as_mut_ptr() as u32];
                    allocations.extend_from_slice(&addresses);
                    ALLOCATIONS.with(|v| *v.borrow_mut() = allocations);
                    let first_node = map.as_mut_ptr().add(center as usize) as u32;
                    let last_node = first_node + (block_count - 1) * 4;
                    let first = addresses[0];
                    let last = addresses[block_count as usize - 1];
                    let finish = last + (size & 31) * 4;
                    let mut expected = snapshot(p, 64);
                    for (offset, value) in [
                        (0, first),
                        (4, first),
                        (8, first + 128),
                        (12, first_node),
                        (16, finish),
                        (20, last),
                        (24, last + 128),
                        (28, last_node),
                        (32, map.as_mut_ptr() as u32),
                        (36, map_count),
                    ] {
                        expected_word(&mut expected, offset, value);
                    }
                    assert_eq!(
                        invoke(fc4p_00745980 as *const () as usize, p, &[size]),
                        (size & 31, finish)
                    );
                    let mut expected_map = [0xa5a5a5a5u32; 16];
                    expected_map[center as usize..(center + block_count) as usize]
                        .copy_from_slice(&addresses);
                    assert_eq!(map, expected_map);
                    assert_eq!(snapshot(p, 64), expected);
                    assert!(blocks.iter().flatten().all(|&v| v == 0x76543210));
                    let mut expected_calls = vec![(0, vec![0xa110, map_count * 4, map_count])];
                    expected_calls
                        .extend((0..block_count).map(|_| (0, vec![0xa110, 128, map_count])));
                    assert_eq!(events(), expected_calls);
                    assert!(ALLOCATIONS.with(|v| v.borrow().is_empty()));
                }
            }
        }
    }

    extern "thiscall" fn observe_surface_acquire(this: *mut u8) -> u64 {
        // SAFETY: OWNER points at the holder that is replacing its retained surface.
        unsafe {
            event(this, &[4, word(OWNER.with(|v| v.get()) as *mut u8, 4)]);
        }
        HIGH | u64::from(VALUE)
    }
    extern "thiscall" fn replace_surface_on_release(this: *mut u8) -> u64 {
        // SAFETY: changing the live holder in this callback tests the caller's subsequent reload.
        unsafe {
            let owner = OWNER.with(|v| v.get()) as *mut u8;
            event(this, &[8, word(owner, 4)]);
            put_word(owner, 4, HELD.with(|v| v.get()));
        }
        HIGH | u64::from(VALUE)
    }
    #[test]
    fn surface_callbacks_observe_acquire_assignment_release_order_and_replace_current_surface() {
        let mut owner = Object::new(0x100);
        let base = owner.ptr();
        let p = base.wrapping_add(0xe0);
        let mut old = Object::new(4);
        let old = old.ptr();
        let mut new = Object::new(4);
        let new = new.ptr();
        let mut replacement = Object::new(4);
        let replacement = replacement.ptr();
        let mut table = table();
        table[1] = observe_surface_acquire as *const () as usize;
        table[2] = replace_surface_on_release as *const () as usize;
        OWNER.with(|v| v.set(p as u32));
        HELD.with(|v| v.set(replacement as u32));
        // SAFETY: the window and all three surfaces expose live callback tables.
        unsafe {
            put_word(base, 0, table.as_ptr() as u32);
            for s in [old, new, replacement] {
                put_word(s, 0, table.as_ptr() as u32);
            }
            put_word(p, 4, old as u32);
            let mut expected = snapshot(base, 0x100);
            expected_word(&mut expected, 0xe4, replacement as u32);
            assert_eq!(
                invoke(fc4p_00793ff0 as *const () as usize, p, &[new as u32]),
                (VALUE, (HIGH >> 32) as u32)
            );
            assert_eq!(
                events(),
                vec![
                    (new as u32, vec![4, old as u32]),
                    (old as u32, vec![8, new as u32]),
                    (replacement as u32, vec![0x78, 255, 0, 255]),
                    (replacement as u32, vec![0x5c, VALUE]),
                    (base as u32, vec![0x170])
                ]
            );
            assert_eq!(snapshot(base, 0x100), expected);
        }
    }
}
