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

//! `fc4native.dll`: drop-in replacements for SimCity 4's own functions (`docs/NEXT_STEPS.md` §1-3).
//!
//! Each replacement ("port") keeps the original's calling convention, reads and writes the game's objects at
//! their original offsets, and makes the same calls to other game functions, in the same order, at their
//! original addresses. The DLL has no imports and no C runtime (`/NOENTRY /NODEFAULTLIB`); the FreeCity4
//! plugin loads it and jumps the enabled originals to the `fc4p_<address>` exports listed in the exported
//! table `fc4native_ports` (`engine/plugin/FC4Native.cpp`).
//!
//! `cargo test -p fc4-native --target i686-pc-windows-msvc` builds the same sources with std and runs each
//! port on objects laid out like the game's, with fakes standing in for the game functions it calls.
//!
//! Modules: [`x87`] (the game's float arithmetic and float-to-integer helper), `port` (the table),
//! `abi` (layouts, virtual and game calls), [`game`] (game functions called by address), one module per
//! converted class ([`landvalue`]).

// The DLL is built with `panic = "abort"` (the release profile) and has no std. The workspace's dev profile
// unwinds, which a no-std library cannot: dev builds (`cargo build`/`clippy` of the workspace) link std
// instead and are never loaded into the game. Tests always use std.
#![cfg_attr(not(any(test, panic = "unwind")), no_std)]

#[cfg(target_arch = "x86")]
#[macro_use]
mod port;
// The support API is wider than what the ports so far call.
#[cfg(target_arch = "x86")]
#[macro_use]
#[allow(dead_code)]
mod abi;
#[cfg(all(test, target_arch = "x86"))]
mod testing;

#[cfg(target_arch = "x86")]
pub mod aura;
#[cfg(target_arch = "x86")]
pub mod budget;
#[cfg(target_arch = "x86")]
pub mod building;
#[cfg(target_arch = "x86")]
pub mod construction_occupant;
#[cfg(target_arch = "x86")]
pub mod demand;
#[cfg(target_arch = "x86")]
pub mod game;
#[cfg(target_arch = "x86")]
pub mod growth_developer;
#[cfg(target_arch = "x86")]
pub mod landvalue;
#[cfg(target_arch = "x86")]
pub mod lot_developer;
#[cfg(target_arch = "x86")]
pub mod neighbors;
#[cfg(target_arch = "x86")]
pub mod network_tool;
#[cfg(target_arch = "x86")]
pub mod occupant_manager;
#[cfg(target_arch = "x86")]
pub mod path_info;
#[cfg(target_arch = "x86")]
pub mod pathfinder;
#[cfg(target_arch = "x86")]
pub mod plumbing;
#[cfg(target_arch = "x86")]
pub mod pollution;
#[cfg(target_arch = "x86")]
pub mod power;
#[cfg(target_arch = "x86")]
pub mod prop_developer;
#[cfg(target_arch = "x86")]
pub mod residential;
#[cfg(target_arch = "x86")]
pub mod shared;
#[cfg(target_arch = "x86")]
pub mod terrain;
#[cfg(target_arch = "x86")]
pub mod tract_developer;
#[cfg(target_arch = "x86")]
pub mod traffic;
#[cfg(target_arch = "x86")]
pub mod traffic_network;
#[cfg(target_arch = "x86")]
pub mod weather;
#[cfg(target_arch = "x86")]
pub mod x87;
#[cfg(target_arch = "x86")]
pub mod zone_developer;

#[cfg(target_arch = "x86")]
pub use table::{fc4native_ports, PORTS};

#[cfg(target_arch = "x86")]
mod table {
    use crate::port::{self, Port, PortTable};

    const COUNT: usize = crate::x87::PORTS.len()
        + crate::aura::PORTS.len()
        + crate::budget::PORTS.len()
        + crate::building::PORTS.len()
        + crate::construction_occupant::PORTS.len()
        + crate::demand::PORTS.len()
        + crate::growth_developer::PORTS.len()
        + crate::landvalue::PORTS.len()
        + crate::lot_developer::PORTS.len()
        + crate::neighbors::PORTS.len()
        + crate::network_tool::PORTS.len()
        + crate::occupant_manager::PORTS.len()
        + crate::path_info::PORTS.len()
        + crate::pathfinder::PORTS.len()
        + crate::plumbing::PORTS.len()
        + crate::pollution::PORTS.len()
        + crate::power::PORTS.len()
        + crate::prop_developer::PORTS.len()
        + crate::residential::PORTS.len()
        + crate::shared::PORTS.len()
        + crate::terrain::PORTS.len()
        + crate::tract_developer::PORTS.len()
        + crate::traffic::PORTS.len()
        + crate::traffic_network::PORTS.len()
        + crate::weather::PORTS.len()
        + crate::zone_developer::PORTS.len();
    // Only evaluated at compile time, for the uniqueness check and the static table below.
    #[allow(clippy::large_const_arrays)]
    const ALL: [Port; COUNT] = port::concat(&[
        crate::x87::PORTS,
        crate::aura::PORTS,
        crate::budget::PORTS,
        crate::building::PORTS,
        crate::construction_occupant::PORTS,
        crate::demand::PORTS,
        crate::growth_developer::PORTS,
        crate::landvalue::PORTS,
        crate::lot_developer::PORTS,
        crate::neighbors::PORTS,
        crate::network_tool::PORTS,
        crate::occupant_manager::PORTS,
        crate::path_info::PORTS,
        crate::pathfinder::PORTS,
        crate::plumbing::PORTS,
        crate::pollution::PORTS,
        crate::power::PORTS,
        crate::prop_developer::PORTS,
        crate::residential::PORTS,
        crate::shared::PORTS,
        crate::terrain::PORTS,
        crate::tract_developer::PORTS,
        crate::traffic::PORTS,
        crate::traffic_network::PORTS,
        crate::weather::PORTS,
        crate::zone_developer::PORTS,
    ]);
    // Checked in a test: the pairwise check over the whole table is too long for compile-time evaluation.
    #[test]
    fn addresses_unique() {
        assert!(port::addresses_unique(&PORTS), "an address has two ports");
    }

    const INSTALLABLE: usize = port::count_installable(&ALL);

    /// Every port the loader can install (see [`port::SHORT`] and [`port::HELD`]), in the order of the exported table.
    pub static PORTS: [Port; INSTALLABLE] = port::installable(&ALL);

    /// The table the loader reads (see [`PortTable`]).
    #[no_mangle]
    #[allow(non_upper_case_globals)]
    pub static fc4native_ports: PortTable = PortTable::new(&PORTS);
}

// The linker wants `_fltused` from any object that uses floating point; without the C runtime the DLL defines
// it. Defined in assembly so that it is not exported.
#[cfg(all(target_arch = "x86", not(any(test, panic = "unwind"))))]
core::arch::global_asm!(
    ".pushsection .rdata,\"dr\"",
    ".globl __fltused",
    "__fltused:",
    ".long 0",
    ".popsection"
);

/// The DLL never unwinds into the game: a panic stops at an invalid instruction.
#[cfg(not(any(test, panic = "unwind")))]
#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    // SAFETY: raises an invalid-opcode exception; nothing follows.
    unsafe {
        core::arch::asm!("ud2", options(noreturn))
    }
    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    loop {}
}
