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

//! Test support (`cfg(test)` only): where game functions and globals resolve in tests, the game's x87 control
//! word, zeroed objects at the original layout, and calls with a chosen entry EAX.
//!
//! Fakes are per thread (tests run in parallel) and removed when the returned guard drops.

use crate::abi::{call_raw, Conv, RawCall};
use crate::x87::{control_word, set_control_word, GAME_CONTROL_WORD};
use std::cell::RefCell;

thread_local! {
    static FAKES: RefCell<Vec<(u32, usize)>> = const { RefCell::new(Vec::new()) };
    static DATA: RefCell<Vec<(u32, usize)>> = const { RefCell::new(Vec::new()) };
}

/// Removes its fake when dropped.
#[must_use = "the fake is removed when this guard drops"]
pub struct Fake {
    address: u32,
    data: bool,
}

impl Drop for Fake {
    fn drop(&mut self) {
        let table = if self.data { &DATA } else { &FAKES };
        table.with(|t| t.borrow_mut().retain(|&(a, _)| a != self.address));
    }
}

/// Calls to the game function at `address` go to `function` (when the crate has no port of it).
pub fn fake(address: u32, function: usize) -> Fake {
    FAKES.with(|t| {
        let mut t = t.borrow_mut();
        t.retain(|&(a, _)| a != address);
        t.push((address, function));
    });
    Fake { address, data: false }
}

/// The game global at `address` lives at `location`.
pub fn fake_data(address: u32, location: *mut u8) -> Fake {
    DATA.with(|t| {
        let mut t = t.borrow_mut();
        t.retain(|&(a, _)| a != address);
        t.push((address, location as usize));
    });
    Fake { address, data: true }
}

/// Our port of the game function at `address`, else the fake, else a panic.
pub fn resolve(address: u32) -> usize {
    if let Some(p) = crate::PORTS.iter().find(|p| p.address == address) {
        return p.entry as usize;
    }
    FAKES
        .with(|t| t.borrow().iter().find(|&&(a, _)| a == address).map(|&(_, f)| f))
        .unwrap_or_else(|| panic!("game function 0x{address:08X} has no port and no fake in this test"))
}

/// The stand-in of the game global at `address`, else a panic.
pub fn resolve_data(address: u32) -> usize {
    DATA.with(|t| t.borrow().iter().find(|&&(a, _)| a == address).map(|&(_, f)| f))
        .unwrap_or_else(|| panic!("game global 0x{address:08X} has no stand-in in this test"))
}

/// Runs the x87 at the game's control word (0x007F) until dropped, then restores the previous one.
pub struct GameFpu(u16);

impl GameFpu {
    #[allow(clippy::new_without_default)]
    pub fn new() -> GameFpu {
        let saved = control_word();
        set_control_word(GAME_CONTROL_WORD);
        GameFpu(saved)
    }
}

impl Drop for GameFpu {
    fn drop(&mut self) {
        set_control_word(self.0);
    }
}

/// Zeroed memory for one game object, 16-byte aligned.
pub struct Object(Vec<u128>);

impl Object {
    pub fn new(size: usize) -> Object {
        Object(vec![0; size.div_ceil(16)])
    }
    pub fn ptr(&mut self) -> *mut u8 {
        self.0.as_mut_ptr().cast()
    }
}

/// Calls the thiscall function at `function` with `this`, the stack words `args` and EAX = `eax` at entry;
/// returns (EAX, EDX) at exit.
///
/// # Safety
///
/// `function` is a thiscall function taking `args.len()` stack words.
pub unsafe fn call_thiscall_with_eax(function: usize, this: *mut u8, args: &[u32], eax: u32) -> (u32, u32) {
    let mut c = RawCall::new(Conv::Thiscall, function, this as usize as u32, args);
    c.eax = eax;
    // SAFETY: the caller's contract.
    unsafe { call_raw(&mut c) };
    (c.eax, c.edx)
}
