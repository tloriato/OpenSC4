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

//! Replacements ("ports") and the table the loader reads.
//!
//! Each port replaces one function of the exe. [`port!`] declares it: the function is exported as
//! `fc4p_<address>` (8 lowercase hex digits of the original entry; the macro checks the name against the
//! address at compile time), and a [`Port`] entry with its owner (the class it belongs to, the unit the ini
//! switches on) and name is defined next to it, as `<export>::PORT`. Each module lists its entries in a
//! `PORTS` slice, and the crate root joins those into the exported table [`PortTable`] (`fc4native_ports`).
//!
//! Three forms:
//! - plain: `#[port(address = .., owner = .., name = ..)] pub unsafe extern "thiscall" fn fc4p_..(..) {..}`;
//! - `entry_eax` (thiscall only): the original leaves part of the caller's EAX in its result, so the export is a
//!   two-instruction stub (`mov edx, eax; jmp body`) and the body is a fastcall function that receives `this` in
//!   ECX and the entry EAX in EDX, written in the signature as `[name: u32]` after `this`; the stack arguments
//!   and the callee pop are unchanged;
//! - `naked`: the body is a `naked_asm!` block, for helpers with their own register convention.

/// One replacement, as the loader reads it (`#[repr(C)]`, 16 bytes).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Port {
    /// The original function's entry address in the exe.
    pub address: u32,
    /// Our function (the `fc4p_<address>` export).
    pub entry: *const (),
    /// The owner, NUL-terminated (e.g. `landvalue`).
    pub owner: *const u8,
    /// The original's name, NUL-terminated (e.g. `cSC4LandValueSimulator::TractIsZoned`).
    pub name: *const u8,
}

// SAFETY: the pointers point at code and string literals, never written.
unsafe impl Sync for Port {}

impl Port {
    /// `owner` and `name` end with NUL (the macro appends it).
    pub const fn new(address: u32, entry: *const (), owner: &'static str, name: &'static str) -> Port {
        Port {
            address,
            entry,
            owner: owner.as_ptr(),
            name: name.as_ptr(),
        }
    }
    const EMPTY: Port = Port {
        address: 0,
        entry: core::ptr::null(),
        owner: core::ptr::null(),
        name: core::ptr::null(),
    };
}

/// The exported table (`fc4native_ports`), `#[repr(C)]`: magic `FC4P`, layout version, entry count, entry size,
/// entries.
#[repr(C)]
#[derive(Debug)]
pub struct PortTable {
    pub magic: u32,
    pub version: u32,
    pub count: u32,
    pub entry_size: u32,
    pub entries: *const Port,
}

// SAFETY: see `Port`.
unsafe impl Sync for PortTable {}

/// `FC4P` in memory order.
pub const TABLE_MAGIC: u32 = u32::from_le_bytes(*b"FC4P");
pub const TABLE_VERSION: u32 = 1;

impl PortTable {
    pub const fn new(entries: &'static [Port]) -> PortTable {
        PortTable {
            magic: TABLE_MAGIC,
            version: TABLE_VERSION,
            count: entries.len() as u32,
            entry_size: core::mem::size_of::<Port>() as u32,
            entries: entries.as_ptr(),
        }
    }
}

/// The modules' `PORTS` lists, joined (`N` = their total length).
pub const fn concat<const N: usize>(lists: &[&[Port]]) -> [Port; N] {
    let mut out = [Port::EMPTY; N];
    let mut k = 0;
    let mut i = 0;
    while i < lists.len() {
        let mut j = 0;
        while j < lists[i].len() {
            out[k] = lists[i][j];
            k += 1;
            j += 1;
        }
        i += 1;
    }
    assert!(k == N, "port table length");
    out
}

/// No address is replaced twice.
pub const fn addresses_unique(ports: &[Port]) -> bool {
    let mut i = 0;
    while i < ports.len() {
        let mut j = i + 1;
        while j < ports.len() {
            if ports[i].address == ports[j].address {
                return false;
            }
            j += 1;
        }
        i += 1;
    }
    true
}

/// `name` is `fc4p_` and the 8 lowercase hex digits of `address`.
pub const fn export_name_matches(name: &str, address: u32) -> bool {
    let b = name.as_bytes();
    if b.len() != 13 || b[0] != b'f' || b[1] != b'c' || b[2] != b'4' || b[3] != b'p' || b[4] != b'_' {
        return false;
    }
    let mut i = 0;
    while i < 8 {
        let nibble = (address >> (28 - 4 * i)) & 0xF;
        if b[5 + i] != b"0123456789abcdef"[nibble as usize] {
            return false;
        }
        i += 1;
    }
    true
}

/// Declares one port; see the module docs.
macro_rules! port {
    // Plain.
    (
        $(#[doc = $doc:literal])*
        #[port(address = $address:literal, owner = $owner:literal, name = $name:literal)]
        $vis:vis unsafe extern $abi:tt fn $export:ident($($arg:ident: $ty:ty),* $(,)?) $(-> $ret:ty)? $body:block
    ) => {
        $(#[doc = $doc])*
        #[doc = ""]
        #[doc = "# Safety"]
        #[doc = ""]
        #[doc = "Called by the game in place of the original, with the original's arguments and convention."]
        #[no_mangle]
        $vis unsafe extern $abi fn $export($($arg: $ty),*) $(-> $ret)? $body

        #[doc = concat!("The port table entry of [`", stringify!($export), "`].")]
        pub mod $export {
            /// The entry for the loader's table.
            pub const PORT: $crate::port::Port = $crate::port::Port::new(
                $address, super::$export as *const (), concat!($owner, "\0"), concat!($name, "\0"));
            const _: () = assert!(
                $crate::port::export_name_matches(stringify!($export), $address),
                "the export must be named fc4p_ and the 8 lowercase hex digits of the address"
            );
        }
    };
    // `entry_eax`: the export passes the caller's EAX to the body as a hidden second argument.
    (
        $(#[doc = $doc:literal])*
        #[port(address = $address:literal, owner = $owner:literal, name = $name:literal, entry_eax)]
        $vis:vis unsafe extern "thiscall" fn $export:ident(
            $this:ident: $this_ty:ty, [$eax:ident: u32] $(, $arg:ident: $ty:ty)* $(,)?
        ) $(-> $ret:ty)? $body:block
    ) => {
        $(#[doc = $doc])*
        #[doc = ""]
        #[doc = "Thiscall export: copies EAX to EDX and jumps to the body (fastcall: `this` in ECX, entry EAX in"]
        #[doc = "EDX, the stack arguments as the caller pushed them)."]
        #[doc = ""]
        #[doc = "# Safety"]
        #[doc = ""]
        #[doc = "Called by the game in place of the original, with the original's arguments and convention."]
        #[unsafe(naked)]
        #[no_mangle]
        $vis unsafe extern "C" fn $export() {
            core::arch::naked_asm!("mov edx, eax", "jmp {body}", body = sym $export::body)
        }

        #[doc = concat!("The body and port table entry of [`", stringify!($export), "`].")]
        pub mod $export {
            #[allow(unused_imports)]
            use super::*;

            /// The entry for the loader's table.
            pub const PORT: $crate::port::Port = $crate::port::Port::new(
                $address, super::$export as *const (), concat!($owner, "\0"), concat!($name, "\0"));
            const _: () = assert!(
                $crate::port::export_name_matches(stringify!($export), $address),
                "the export must be named fc4p_ and the 8 lowercase hex digits of the address"
            );

            /// The body (see the export).
            ///
            /// # Safety
            ///
            /// Reached only through the export, with the original's arguments.
            pub unsafe extern "fastcall" fn body($this: $this_ty, $eax: u32 $(, $arg: $ty)*) $(-> $ret)? $body
        }
    };
    // `naked`: the body is a `naked_asm!` block.
    (
        $(#[doc = $doc:literal])*
        #[port(address = $address:literal, owner = $owner:literal, name = $name:literal, naked)]
        $vis:vis unsafe extern "C" fn $export:ident() $body:block
    ) => {
        $(#[doc = $doc])*
        #[doc = ""]
        #[doc = "# Safety"]
        #[doc = ""]
        #[doc = "Called by the game in place of the original, with the original's register convention."]
        #[unsafe(naked)]
        #[no_mangle]
        $vis unsafe extern "C" fn $export() $body

        #[doc = concat!("The port table entry of [`", stringify!($export), "`].")]
        pub mod $export {
            /// The entry for the loader's table.
            pub const PORT: $crate::port::Port = $crate::port::Port::new(
                $address, super::$export as *const (), concat!($owner, "\0"), concat!($name, "\0"));
            const _: () = assert!(
                $crate::port::export_name_matches(stringify!($export), $address),
                "the export must be named fc4p_ and the 8 lowercase hex digits of the address"
            );
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_names() {
        assert!(export_name_matches("fc4p_006bd540", 0x006B_D540));
        assert!(!export_name_matches("fc4p_006BD540", 0x006B_D540));
        assert!(!export_name_matches("fc4p_006bd541", 0x006B_D540));
        assert!(!export_name_matches("fc4p_06bd540", 0x006B_D540));
    }

    #[test]
    fn table() {
        let t = &crate::fc4native_ports;
        assert_eq!(t.magic, TABLE_MAGIC);
        assert_eq!(t.count as usize, crate::PORTS.len());
        assert_eq!(t.entry_size, 16);
        let mut seen = std::collections::HashSet::new();
        for p in crate::PORTS.iter() {
            assert!(seen.insert(p.address), "address {:08x} twice", p.address);
            // SAFETY: NUL-terminated literals.
            let owner = unsafe { std::ffi::CStr::from_ptr(p.owner.cast()) }.to_str().unwrap();
            let name = unsafe { std::ffi::CStr::from_ptr(p.name.cast()) }.to_str().unwrap();
            assert!(!owner.is_empty() && owner.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()));
            assert!(!name.is_empty());
            assert!(!p.entry.is_null());
        }
    }
}
