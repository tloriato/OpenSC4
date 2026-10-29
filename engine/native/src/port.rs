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

/// Entries of the exe's functions that end fewer than 5 bytes before the next function begins, in address order.
/// The loader's 5-byte jump would overwrite the start of the next function, so their ports stay out of the table
/// the loader reads; their exports remain for `prove`.
pub const SHORT: [u32; 112] = [
    0x0087_524B, 0x0087_5259, 0x0087_5277, 0x0087_527B, 0x0087_527F, 0x0087_5283, 0x0087_5287, 0x0087_528B,
    0x0087_528F, 0x0087_5784, 0x0087_5788, 0x0087_578C, 0x0087_5790, 0x0087_7815, 0x0087_8C04, 0x0087_93F9,
    0x0087_B357, 0x0088_FEF8, 0x0089_1D96, 0x0089_6662, 0x0089_6666, 0x008D_2B70, 0x008D_3668, 0x008F_697D,
    0x0090_06D6, 0x0091_297E, 0x0091_C264, 0x0091_C570, 0x0092_0C3F, 0x0092_53D7, 0x0092_B351, 0x0093_2B83,
    0x0093_34E3, 0x0093_9792, 0x0093_998F, 0x0093_E47C, 0x0093_E47F, 0x0093_F933, 0x0094_0DA8, 0x0094_595C,
    0x0094_5E3B, 0x0094_5E3F, 0x0094_5E43, 0x0094_8B85, 0x0094_9ADE, 0x0094_9B47, 0x0094_9B73, 0x0094_A5EA,
    0x0096_831B, 0x0096_8329, 0x0096_8337, 0x0096_BA27, 0x0096_BC99, 0x0096_BC9D, 0x0096_C061, 0x0096_C470,
    0x0096_C474, 0x0096_C478, 0x0096_D8B9, 0x0096_E960, 0x0097_9419, 0x0097_B493, 0x0097_B4A1, 0x0097_CE34,
    0x0097_E89E, 0x0097_F070, 0x0099_35E3, 0x0099_35F8, 0x0099_3660, 0x0099_3673, 0x0099_5D93, 0x0099_5E67,
    0x0099_9165, 0x0099_B7EE, 0x0099_B951, 0x0099_BCE8, 0x0099_BE4C, 0x0099_BE50, 0x0099_BE66, 0x0099_BEF9,
    0x0099_BEFD, 0x009C_32AC, 0x009C_48CE, 0x009C_5C96, 0x009C_5E01, 0x009C_879F, 0x009C_87BF, 0x009C_88DE,
    0x009C_8A3B, 0x009C_DC23, 0x009C_DC27, 0x009C_EC4A, 0x009C_EDE1, 0x009D_4199, 0x009D_419D, 0x009D_41A1,
    0x009D_41A5, 0x009D_41A9, 0x009D_41AD, 0x009D_449C, 0x009D_4714, 0x009D_7E63, 0x009D_8539, 0x009D_8564,
    0x009D_858F, 0x009D_AA18, 0x009D_B021, 0x009D_B37A, 0x009D_B663, 0x009D_B800, 0x00A6_D837, 0x00A7_F35F,
];

/// A jump at `address` stays inside its function (`address` is not in [`SHORT`]).
pub const fn jump_fits(address: u32) -> bool {
    let (mut lo, mut hi) = (0, SHORT.len());
    while lo < hi {
        let mid = (lo + hi) / 2;
        if SHORT[mid] == address {
            return false;
        }
        if SHORT[mid] < address {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    true
}

/// How many of `ports` the loader can install.
pub const fn count_installable(ports: &[Port]) -> usize {
    let mut n = 0;
    let mut i = 0;
    while i < ports.len() {
        if jump_fits(ports[i].address) {
            n += 1;
        }
        i += 1;
    }
    n
}

/// The ports the loader can install, in order (`N` = [`count_installable`]).
pub const fn installable<const N: usize>(ports: &[Port]) -> [Port; N] {
    let mut out = [Port::EMPTY; N];
    let (mut i, mut k) = (0, 0);
    while i < ports.len() {
        if jump_fits(ports[i].address) {
            out[k] = ports[i];
            k += 1;
        }
        i += 1;
    }
    assert!(k == N, "installable port count");
    out
}

/// No address is replaced twice.
#[cfg(test)]
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
    fn short_functions_sorted_and_left_out() {
        assert!(SHORT.windows(2).all(|w| w[0] < w[1]));
        assert!(!jump_fits(0x0096_D8B9) && jump_fits(0x0096_D8B0));
        assert!(crate::PORTS.iter().all(|p| jump_fits(p.address)));
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
