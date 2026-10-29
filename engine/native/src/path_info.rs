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

//! `cSC4PathInfo`, converted one function at a time. Its game functions and globals are declared in this file.

use crate::abi::{call_raw, vslot, Conv, RawCall};
use crate::port::Port;
use core::ptr::{read_unaligned, write_unaligned};

layout! {
    /// Path information, with its primary interface at offset zero.
    pub struct PathInfo {
        0x00 => vtable / set_vtable: u32,
        0x04 => resource_vtable / set_resource_vtable: u32,
        0x20 => buckets / set_buckets: *mut *mut u8,
        0x24 => buckets_end / set_buckets_end: *mut *mut u8,
        0x28 => buckets_capacity / set_buckets_capacity: *mut *mut u8,
        0x2C => path_count / set_path_count: u32,
        0x30 => flags / set_flags: u8,
    }
}

layout! {
    /// A path's vertices, stored length and intersection flag.
    pub struct Path {
        0x00 => begin / set_begin: *mut f32,
        0x04 => end / set_end: *mut f32,
        0x08 => capacity / set_capacity: *mut f32,
        0x0C => length / set_length: f32,
        0x10 => intersection / set_intersection: u8,
    }
}

layout! {
    /// One chained hash-map entry. The path starts eight bytes after the entry.
    pub struct PathNode {
        0x00 => next / set_next: *mut u8,
        0x04 => key / set_key: u32,
    }
}

/// The number of buckets uses the signed pointer difference, shifted before the unsigned division.
fn bucket_count(info: PathInfo) -> u32 {
    ((info.buckets_end() as u32).wrapping_sub(info.buckets() as u32) as i32 >> 2) as u32
}

// SAFETY: callers supply the game's initialized map, whose bucket count is nonzero.
unsafe fn find(info: PathInfo, key: u32) -> *mut u8 {
    unsafe {
        let mut node = read_unaligned(info.buckets().add((key % bucket_count(info)) as usize));
        while !node.is_null() {
            let entry = PathNode::from_ptr(node);
            if entry.key() == key {
                break;
            }
            node = entry.next();
        }
        node
    }
}

fn path_key(kind: u32, entry: u32, exit: u32, lane: u32) -> u32 {
    (((kind.wrapping_shl(8) | (lane & 0xFF)).wrapping_shl(8) | (entry & 0xFF)).wrapping_shl(8)) | (exit & 0xFF)
}

port! {
    /// Returns either the primary or resource interface, retaining it through the primary AddRef slot.
    /// AL reports success; the other bytes come from AddRef, or from the output pointer on failure.
    #[port(address = 0x0053_CFA0, owner = "pathinfo", name = "cSC4PathInfo::QueryInterface")]
    pub unsafe extern "thiscall" fn fc4p_0053cfa0(this: *mut u8, iid: u32, out: *mut *mut u8) -> u64 {
        unsafe {
            write_unaligned(out, core::ptr::null_mut());
            let interface = match iid {
                0x456B_8F1D if !this.is_null() => this.wrapping_add(4),
                0xA966_7793 => this,
                _ => core::ptr::null_mut(),
            };
            write_unaligned(out, interface);
            if read_unaligned(out).is_null() {
                let edx = if iid == 0x456B_8F1D { interface as u32 } else { iid };
                return (u64::from(edx) << 32) | u64::from(out as u32 & !0xFF);
            }
            let mut call = RawCall::new(Conv::Thiscall, vslot(this, 4), this as u32, &[]);
            call.eax = read_unaligned(this.cast());
            call.edx = if iid == 0x456B_8F1D { interface as u32 } else { iid };
            call_raw(&mut call);
            (u64::from(call.edx) << 32) | u64::from((call.eax & !0xFF) | 1)
        }
    }
}

port! {
    /// Packs the type, lane and edge bytes, then calls the primary GetPath slot.
    #[port(address = 0x0053_CFF0, owner = "pathinfo", name = "cSC4PathInfo::GetPath (edge bytes)")]
    pub unsafe extern "thiscall" fn fc4p_0053cff0(
        this: *mut u8, kind: u32, entry: u32, exit: u32, lane: u32, length: *mut u32
    ) -> u64 {
        unsafe {
            let key = path_key(kind, entry, exit, lane);
            let args = [key, length as u32];
            let mut call = RawCall::new(Conv::Thiscall, vslot(this, 0x0C), this as u32, &args);
            call.eax = read_unaligned(this.cast());
            call.edx = key;
            call_raw(&mut call);
            (u64::from(call.edx) << 32) | u64::from(call.eax)
        }
    }
}

port! {
    /// Type zero reads the shared intersection bit. Other types return their stored byte unchanged.
    /// The upper three result bytes retain the type argument.
    #[port(address = 0x0053_D0A0, owner = "pathinfo", name = "cSC4PathInfo::IsIntersection")]
    pub unsafe extern "fastcall" fn fc4p_0053d0a0(this: *mut u8, entry_edx: u32, kind: u32) -> u64 {
        let value = unsafe {
            if kind == 0 { (PathInfo::from_ptr(this).flags() >> 1) & 1 }
            else { read_unaligned(this.wrapping_add(kind as usize).wrapping_add(0x31)) }
        };
        (u64::from(entry_edx) << 32) | u64::from((kind & !0xFF) | u32::from(value))
    }
}

port! {
    /// Looks up an exact key. A miss preserves the optional length output.
    #[port(address = 0x0053_DD30, owner = "pathinfo", name = "cSC4PathInfo::GetPath (key)")]
    pub unsafe extern "thiscall" fn fc4p_0053dd30(this: *mut u8, key: u32, length: *mut u32) -> u64 {
        unsafe {
            let node = find(PathInfo::from_ptr(this), key);
            if node.is_null() { return 0; }
            let path = node.add(8);
            let edx = if !length.is_null() {
                let bits = read_unaligned(path.add(0x0C).cast::<u32>());
                write_unaligned(length, bits);
                bits
            } else { node as u32 };
            (u64::from(edx) << 32) | u64::from(path as u32)
        }
    }
}

port! {
    /// Finds an unused key by increasing the signed lane value, stopping after a used lane at or above 26.
    #[port(address = 0x0053_DDE0, owner = "pathinfo", name = "cSC4PathInfo::GetUniquePathKey")]
    pub unsafe extern "thiscall" fn fc4p_0053dde0(
        this: *mut u8, kind: u32, entry: u32, exit: u32, lane: i32
    ) -> u32 {
        unsafe {
            let info = PathInfo::from_ptr(this);
            let mut lane = lane;
            let mut key = path_key(kind, entry, exit, lane as u32);
            while !find(info, key).is_null() {
                if lane >= 26 { break; }
                lane = lane.wrapping_add(1);
                key = path_key(kind, entry, exit, lane as u32);
            }
            key
        }
    }
}

port! {
    /// Returns the elevation flag as a full-width zero or one.
    #[port(address = 0x0054_03A0, owner = "pathinfo", name = "cSC4PathInfo::UseElevation")]
    pub unsafe extern "fastcall" fn fc4p_005403a0(this: *mut u8, entry_edx: u32) -> u64 {
        (u64::from(entry_edx) << 32) | u64::from(unsafe { PathInfo::from_ptr(this) }.flags() & 1)
    }
}

port! {
    /// Sets bit zero from the low argument byte, preserving the other flags and returning the resulting byte.
    #[port(address = 0x0054_03B0, owner = "pathinfo", name = "cSC4PathInfo::SetUseElevationFlag")]
    pub unsafe extern "fastcall" fn fc4p_005403b0(this: *mut u8, entry_edx: u32, enabled: u32) -> u64 {
        let info = unsafe { PathInfo::from_ptr(this) };
        let flags = if enabled as u8 != 0 { info.flags() | 1 } else { info.flags() & !1 };
        info.set_flags(flags);
        (u64::from(entry_edx) << 32) | u64::from(flags)
    }
}

#[cfg(test)]
mod tests;

pub mod geometry;
pub mod life;
pub mod scan;

/// This module's replacements.
const DIRECT: &[Port] = &[
    fc4p_0053cfa0::PORT,
    fc4p_0053cff0::PORT,
    fc4p_0053d0a0::PORT,
    fc4p_0053dd30::PORT,
    fc4p_0053dde0::PORT,
    fc4p_005403a0::PORT,
    fc4p_005403b0::PORT,
];
const ALL: [Port; DIRECT.len() + geometry::PORTS.len() + life::PORTS.len() + scan::PORTS.len()] =
    crate::port::concat(&[DIRECT, geometry::PORTS, life::PORTS, scan::PORTS]);
pub const PORTS: &[Port] = &ALL;
