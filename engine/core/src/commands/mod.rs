// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The command system: `cGZCommandParameterSet` (§1), `RZParseDelimitedText`
//! (§2), `cGZStringCommandGenerator` (§3) and `cGZMessage2CommandDispatcher`
//! with its message (§4).
//!
//! Written only from `docs/specs/command-system.md`; section references (`§n`)
//! point at that spec. Open points are tagged `[choice]` and carry a question
//! id (GCn) in `docs/specs/questions/command-system.md`.
//!
//! The command server (`cGZCommandServer`) and message server 2 are **not**
//! implemented here: the spec lists only the slots these classes call (§5),
//! so they are typed callback traits ([`CommandServer`],
//! [`CommandMessageServer`]). The converting variant getters
//! (`GetValString`, `GetValSint32`, data-formats.md §5.1) are a callback trait
//! too ([`ValueConvert`]); `fc4_formats::command_values` provides the exact
//! one.
//!
//! Ownership: parameter sets are shared, mutable objects (handlers write the
//! output set while the dispatcher holds it), so they are
//! `Rc<RefCell<CommandParameterSet>>` ([`SetRef`]). Every reference count the
//! original keeps is kept as a number ([`RefCount`]) so the glue can map it
//! onto AddRef/Release; dropping the Rust handles frees the memory.

mod dispatcher;
mod generator;
mod params;
mod split;
#[cfg(test)]
mod tests;

pub use dispatcher::{
    CommandMessage, CommandMessageServer, Message2CommandDispatcher, MessageFace, StandardMessage,
    COMMAND_MESSAGE_IIDS, DISPATCHER_IIDS, STANDARD_MESSAGE_IIDS,
};
pub use generator::{quote_argument, CommandServer, StringCommandGenerator, GENERATOR_IIDS, MAX_COMMAND_WORDS};
pub use params::{
    make_lower, CommandParameterSet, SetRef, ANY_PARAMETER_ID, NO_PARAMETER_ID, PARAMETER_SET_IIDS, STATUS_PARAMETER_ID,
};
pub use split::{line_of, parse_delimited_text};

use crate::variant::Variant;

/// The converting getters of a variant that the command classes call
/// (data-formats.md §5.1).
pub trait ValueConvert {
    /// `GetValString`: writes the value's text into `out` (replacing it) and
    /// returns true only for a string; other values write their text and
    /// return false. A value without text (an object, by data-formats'
    /// *[choice]*) returns false and leaves `out` unchanged.
    fn get_val_string(&self, v: &Variant, out: &mut Vec<u8>) -> bool;
    /// `GetValSint32`: the converted value, `None` when the getter fails.
    fn get_val_sint32(&self, v: &Variant) -> Option<i32>;
}

/// A GZCOM reference count as these classes keep it (§1.2): starts at 0;
/// `AddRef` increments and returns it; `Release` above 1 decrements and
/// returns the new count, otherwise destroys the object and returns 0 (the
/// count is not decremented first).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RefCount {
    count: u32,
    destroyed: bool,
}

impl RefCount {
    pub fn count(&self) -> u32 {
        self.count
    }
    pub fn is_destroyed(&self) -> bool {
        self.destroyed
    }
    pub fn add_ref(&mut self) -> u32 {
        self.count = self.count.wrapping_add(1);
        self.count
    }
    /// Returns the new count; 0 means the object was destroyed.
    pub fn release(&mut self) -> u32 {
        if self.count > 1 {
            self.count -= 1;
            self.count
        } else {
            self.destroyed = true;
            0
        }
    }
}
