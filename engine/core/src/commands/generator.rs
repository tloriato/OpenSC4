// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! `cGZStringCommandGenerator` (command-system.md §3).

use std::rc::Rc;

use super::params::{CommandParameterSet, SetRef};
use super::split::{line_of, parse_delimited_text};
use super::{RefCount, ValueConvert};
use crate::variant::Variant;

/// The IIDs `QueryInterface` answers (slot 0): `cIGZCommandGenerator`, `0x0B90251E` and 1.
pub const GENERATOR_IIDS: [u32; 3] = [0x0B90_251F, 0x0B90_251E, 1];
/// The largest capacity `ConvertStringToCommand` tries; later words are dropped (§3.4).
pub const MAX_COMMAND_WORDS: usize = 256;

/// The command server slots these classes call (§5). The server itself is not specified,
/// so this is a typed callback; methods take `&self` because a command handler may call
/// back into the server while it runs (§4.4).
pub trait CommandServer {
    /// Slot 12 `GetCommandID(name)`: the lowest registered ID whose name equals `name` in
    /// any case, 0 when none.
    fn get_command_id(&self, name: &[u8]) -> u32;
    /// Slot 11 `GetCommandInfo(id, name, null)`: the registered name (its case kept), or
    /// `None` for an unregistered ID.
    fn get_command_name(&self, id: u32) -> Option<Vec<u8>>;
    /// Slot 20 `ExecuteCommand(id, in, out)` (the ID overload, reaching §4.4).
    fn execute_command_id(&self, id: u32, input: Option<&SetRef>, output: Option<&SetRef>) -> i32;
}

/// `ConvertStringToArgumentString` (§3.5): the word, quoted when it contains a space or a
/// tab. Inner quotes are not escaped and the empty string is not quoted *[quirk]*.
pub fn quote_argument(src: &[u8]) -> Vec<u8> {
    if src.iter().any(|&b| b == b' ' || b == b'\t') {
        let mut out = Vec::with_capacity(src.len() + 2);
        out.push(b'"');
        out.extend_from_slice(src);
        out.push(b'"');
        out
    } else {
        src.to_vec()
    }
}

/// The string command generator (§3.1).
pub struct StringCommandGenerator {
    refs: RefCount,
    server: Option<Rc<dyn CommandServer>>,
    /// The words of the last conversion, reused by every conversion (§3.1).
    words: Vec<Vec<u8>>,
    /// Created by the constructor, released by `Shutdown`, never used *[quirk]*.
    unused_set: Option<SetRef>,
    conv: Rc<dyn ValueConvert>,
}

impl StringCommandGenerator {
    /// The constructor (§3.2): count 0, no server, empty `words`, a new set held at `+0x18`.
    pub fn new(conv: Rc<dyn ValueConvert>) -> Self {
        Self {
            refs: RefCount::default(),
            server: None,
            words: Vec::new(),
            unused_set: Some(CommandParameterSet::new_owned()),
            conv,
        }
    }

    pub fn ref_count(&self) -> u32 {
        self.refs.count()
    }
    pub fn is_destroyed(&self) -> bool {
        self.refs.is_destroyed()
    }
    pub fn has_server(&self) -> bool {
        self.server.is_some()
    }
    /// The never-used set (§3.1), for tests.
    pub fn unused_set(&self) -> Option<&SetRef> {
        self.unused_set.as_ref()
    }
    /// The words of the last conversion.
    pub fn words(&self) -> &[Vec<u8>] {
        &self.words
    }

    /// Slot 0.
    pub fn query_interface(&mut self, iid: u32) -> bool {
        if GENERATOR_IIDS.contains(&iid) {
            self.add_ref();
            true
        } else {
            false
        }
    }
    /// Slot 1.
    pub fn add_ref(&mut self) -> u32 {
        self.refs.add_ref()
    }
    /// Slot 2: as §1.2; on destruction the destructor releases `+0x18`, the words and the
    /// server (no `Shutdown`).
    pub fn release(&mut self) -> u32 {
        let n = self.refs.release();
        if self.refs.is_destroyed() {
            self.drop_unused_set();
            self.words.clear();
            self.server = None;
        }
        n
    }

    fn drop_unused_set(&mut self) {
        if let Some(s) = self.unused_set.take() {
            s.borrow_mut().release();
        }
    }

    /// Slot 3 (§3.2): `framework_server` is the framework's system service `0xEB903A32`
    /// (`cIGZCommandServer`), `None` when the framework has none. A held server is kept.
    pub fn init(&mut self, framework_server: Option<Rc<dyn CommandServer>>) -> bool {
        if self.server.is_none() {
            self.server = framework_server;
        }
        true
    }

    /// Slot 4: releases the server and the `+0x18` set (`Init` does not recreate it).
    pub fn shutdown(&mut self) -> bool {
        self.server = None;
        self.drop_unused_set();
        true
    }

    /// Slots 5 and 6 (`CommandRegistered` / `CommandUnregistered`): ignored, true.
    pub fn command_registered(&mut self, _id: u32, _name: &[u8], _info: &[u8]) -> bool {
        true
    }

    /// Slot 6, see [`StringCommandGenerator::command_registered`].
    pub fn command_unregistered(&mut self, _id: u32, _name: &[u8], _info: &[u8]) -> bool {
        true
    }

    /// Slot 9 `ConvertStringToCommand(text, length, id, params)` (§3.4). `params` receives a
    /// new set holding one reference that the caller owns.
    pub fn convert_string_to_command(
        &mut self,
        text: &[u8],
        length: u32,
        id: &mut u32,
        params: &mut Option<SetRef>,
    ) -> bool {
        let Some(server) = self.server.clone() else {
            return false;
        };
        let line = line_of(text, length);
        let mut cap = 1usize;
        loop {
            let (ok, words) = parse_delimited_text(line, b' ', cap);
            if ok || cap >= MAX_COMMAND_WORDS {
                // More than 256 words: the first 256 are used, the rest dropped *[quirk]*.
                self.words = words;
                break;
            }
            cap *= 2;
        }
        let Some(name) = self.words.first() else { return false };
        *id = server.get_command_id(name);
        if *id == 0 {
            return false;
        }
        let set = CommandParameterSet::new_owned();
        {
            let mut s = set.borrow_mut();
            for (k, w) in self.words.iter().enumerate().skip(1) {
                s.set_parameter((k - 1) as u32, &Variant::string(w));
            }
        }
        *params = Some(set);
        true
    }

    /// Slot 10 `ConvertCommandToString(id, params, out)` (§3.6).
    pub fn convert_command_to_string(&self, id: u32, params: Option<&SetRef>, out: &mut Vec<u8>) -> bool {
        out.clear();
        let Some(server) = self.server.as_ref() else {
            return false;
        };
        let Some(name) = server.get_command_name(id) else {
            return false;
        };
        *out = quote_argument(&name);
        if let Some(p) = params {
            for (_, v) in p.borrow().iter_by_slots() {
                out.push(b' ');
                out.extend_from_slice(&quote_argument(&self.text_of(v)));
            }
        }
        true
    }

    /// Slot 11 `ConvertCommandOutputToString(params, out)` (§3.6): the values separated by one
    /// space, the status parameter included; true. *[choice]* (spec): a null set is empty.
    pub fn convert_command_output_to_string(&self, params: Option<&SetRef>, out: &mut Vec<u8>) -> bool {
        out.clear();
        if let Some(p) = params {
            for (n, (_, v)) in p.borrow().iter_by_slots().enumerate() {
                if n > 0 {
                    out.push(b' ');
                }
                out.extend_from_slice(&quote_argument(&self.text_of(v)));
            }
        }
        true
    }

    /// `GetValString`'s text, its result ignored. A value without text (an object, by
    /// data-formats' *[choice]*) leaves the fresh buffer empty.
    fn text_of(&self, v: &Variant) -> Vec<u8> {
        let mut s = Vec::new();
        self.conv.get_val_string(v, &mut s);
        s
    }

    /// Slot 8 `ExecuteCommand(text, length, out)` (§3.7): 2 without a server or when the
    /// conversion fails, else the server's slot 20 result.
    pub fn execute_command(&mut self, text: &[u8], length: u32, out: Option<&SetRef>) -> i32 {
        let Some(server) = self.server.clone() else { return 2 };
        let mut id = 0u32;
        let mut params = None;
        if !self.convert_string_to_command(text, length, &mut id, &mut params) {
            if let Some(p) = params {
                p.borrow_mut().release();
            }
            return 2;
        }
        let r = server.execute_command_id(id, params.as_ref(), out);
        if let Some(p) = params {
            p.borrow_mut().release();
        }
        r
    }
}
