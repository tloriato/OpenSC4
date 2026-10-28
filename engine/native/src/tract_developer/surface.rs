// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Identity, reference counting, year messages and saved style selection.

use super::*;

// The calls below use explicit register inputs as well as these stack signatures.
game_fn! {
    #[allow(dead_code)]
    fn write_style_byte(stream: *mut u8, value: u32) -> u32 = "C" 0x005D_9FD0;
}

register_port! {
    /// Returns the class identifier, retaining ECX and EDX.
    0x0070_45A0, fc4p_007045a0, class_id, 0, 0, "cSC4TractDeveloper::GetGZCLSID", |r| {
        r.eax = 0x2990_C142;
    }
}

register_port! {
    /// Answers the four supported interface IDs and AddRefs the primary interface on success.
    /// Unknown IDs leave the output untouched and retain the arithmetic's upper EAX bytes.
    0x0070_45B0, fc4p_007045b0, query_interface, 2, 8, "cSC4TractDeveloper::QueryInterface", |r| {
        let iid = r.args[0];
        r.eax = iid;
        let offset = if iid <= 0xA818_ED1B {
            if iid == 0xA818_ED1B {
                0
            } else {
                r.eax = r.eax.wrapping_sub(1);
                if r.eax == 0 {
                    0
                } else {
                    r.eax = r.eax.wrapping_sub(0x4522_94A9);
                    if r.eax != 0 {
                        r.eax &= !0xFF;
                        return;
                    }
                    4
                }
            }
        } else if iid == 0xE4FD_A3D4 {
            8
        } else {
            r.eax &= !0xFF;
            return;
        };
        r.edx = r.args[1];
        let output = if r.ecx == 0 { 0 } else { r.ecx.wrapping_add(offset) };
        store(r.edx, 0, output);
        r.eax = word(r.ecx, 0);
        invoke(r, Conv::Thiscall, word(r.eax, 4) as usize, &[]);
        r.eax = (r.eax & !0xFF) | 1;
    }
}

register_port! {
    /// Decrements a signed reference count above one. Otherwise shuts down and invokes the deleting
    /// destructor with flag one, using the vtable again after shutdown, then returns zero.
    0x0070_48B0, fc4p_007048b0, release, 0, 0, "cSC4TractDeveloper::Release", |r| {
        let this = r.ecx;
        let td = TractDeveloper::from_ptr(this as *mut u8);
        r.eax = td.references() as u32;
        if td.references() > 1 {
            r.eax = r.eax.wrapping_sub(1);
            td.set_references(r.eax as i32);
        } else {
            r.eax = word(this, 0);
            invoke(r, Conv::Thiscall, word(r.eax, 0x10) as usize, &[]);
            r.edx = word(this, 0);
            r.ecx = this;
            invoke(r, Conv::Thiscall, word(r.edx, 0x3C) as usize, &[1]);
            r.eax = 0;
        }
    }
}

register_port! {
    /// Year messages advance the style timer and rotate at the signed threshold. Month and unrelated
    /// messages leave the style state alone. This is the message-target interface at primary +4.
    0x0070_48E0, fc4p_007048e0, do_message, 1, 4, "cSC4TractDeveloper::DoMessage", |r| {
        let this = r.ecx;
        let td = TractDeveloper::from_ptr(this.wrapping_sub(4) as *mut u8);
        r.ecx = r.args[0];
        r.eax = word(r.ecx, 0);
        invoke(r, Conv::Thiscall, word(r.eax, 0x10) as usize, &[]);
        r.eax = r.eax.wrapping_sub(0x6695_6816);
        if r.eax != 0 {
            r.eax = r.eax.wrapping_sub(1);
            if r.eax == 0 {
                r.edx = (td.years_since_change() as u32).wrapping_add(1);
                r.ecx = td.years_between_changes() as u32;
                r.eax = r.edx;
                td.set_years_since_change(r.edx as i32);
                if r.eax as i32 >= r.ecx as i32 {
                    r.eax = (td.style_index() as u32).wrapping_add(1);
                    r.ecx = 0;
                    td.set_style_index(r.eax as i32);
                    td.set_years_since_change(0);
                    r.edx = (td.styles_end().wrapping_sub(td.styles_begin()) as i32 >> 2) as u32;
                    if r.eax >= r.edx {
                        td.set_style_index(0);
                    }
                }
            }
        }
        r.eax = (r.eax & !0xFF) | 1;
    }
}

register_port! {
    /// Writes version three, style index, elapsed years, the raw single-style byte, active style IDs,
    /// and the rotation period. Only the low byte of the final stream error determines success.
    /// This is the serializable interface at primary +8.
    0x0070_4B60, fc4p_00704b60, write, 1, 4, "cSC4TractDeveloper::Write", |r| {
        let this = r.ecx;
        let td = TractDeveloper::from_ptr(this.wrapping_sub(8) as *mut u8);
        let stream = r.args[0];
        r.eax = word(stream, 0);
        r.ecx = stream;
        invoke(r, Conv::Thiscall, word(r.eax, 0x1C) as usize, &[3]);
        r.eax = td.style_index() as u32;
        r.edx = word(stream, 0);
        r.ecx = stream;
        invoke(r, Conv::Thiscall, word(r.edx, 0x20) as usize, &[r.eax]);
        r.eax = td.years_since_change() as u32;
        r.edx = word(stream, 0);
        r.ecx = stream;
        invoke(r, Conv::Thiscall, word(r.edx, 0x20) as usize, &[r.eax]);
        r.ecx = u32::from(td.single_style());
        invoke(r, Conv::Cdecl, crate::abi::target(0x005D_9FD0), &[stream, r.ecx]);
        let count = (td.styles_end().wrapping_sub(td.styles_begin()) as i32 >> 2) as u32;
        r.edx = word(stream, 0);
        r.ecx = stream;
        invoke(r, Conv::Thiscall, word(r.edx, 0x24) as usize, &[count]);
        let mut i = 0u32;
        while i < count {
            r.ecx = td.styles_begin();
            r.edx = word(r.ecx, i.wrapping_mul(4));
            r.eax = word(stream, 0);
            r.ecx = stream;
            invoke(r, Conv::Thiscall, word(r.eax, 0x24) as usize, &[r.edx]);
            i = i.wrapping_add(1);
        }
        r.ecx = td.years_between_changes() as u32;
        r.eax = word(stream, 0);
        let years = r.ecx;
        r.ecx = stream;
        invoke(r, Conv::Thiscall, word(r.eax, 0x20) as usize, &[years]);
        r.edx = word(stream, 0);
        r.ecx = stream;
        invoke(r, Conv::Thiscall, word(r.edx, 0x48) as usize, &[]);
        r.eax = u32::from(r.eax as u8 == 0);
    }
}

pub const PORTS: &[Port] = &[
    fc4p_007045a0::PORT,
    fc4p_007045b0::PORT,
    fc4p_007048b0::PORT,
    fc4p_007048e0::PORT,
    fc4p_00704b60::PORT,
];

#[cfg(test)]
mod tests;
