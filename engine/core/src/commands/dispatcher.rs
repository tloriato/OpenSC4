// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! `cGZMessage2CommandDispatcher` and `cGZMessage2CommandDispatcherMessage`
//! (command-system.md §4).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::generator::{CommandServer, StringCommandGenerator};
use super::params::{CommandParameterSet, SetRef, STATUS_PARAMETER_ID};
use super::{RefCount, ValueConvert};

/// The IIDs the dispatcher's `QueryInterface` answers (slot 0).
pub const DISPATCHER_IIDS: [u32; 3] = [0x0B90_3CC3, 0x0B90_3CC4, 1];
/// The IIDs that give the command message's primary pointer (§4.5).
pub const COMMAND_MESSAGE_IIDS: [u32; 2] = [0x4B99_446A, 0x0B90_3CC5];
/// The standard-message IIDs §4.5 names for the `+0x04` pointer. [choice] Only these three are
/// listed by the spec; foundations.md §5.1 owns the full list.
pub const STANDARD_MESSAGE_IIDS: [u32; 3] = [0x6529_7976, 0xA522_94B4, 1];

/// Which face of the command message a `QueryInterface` returned (§4.5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageFace {
    /// The primary pointer (`+0x00`).
    Command,
    /// The standard message part (`+0x04`).
    Standard,
}

/// Message server 2's `MessageSend` (slot 3), as the dispatcher calls it (§4.4): synchronous
/// delivery to every target subscribed to the message type, then to the wildcard targets.
/// The server is not part of this owner; this is a typed callback.
pub trait CommandMessageServer {
    fn message_send(&self, msg: &Rc<CommandMessage>) -> bool;
}

/// The reused command message (§4.5): a standard message (type, data1, data2) plus counted
/// references to the last command's sets. data1/data2 and the counted references are the
/// same sets, so one handle each.
#[derive(Debug, Default)]
pub struct CommandMessage {
    refs: Cell<RefCount>,
    msg_type: Cell<u32>,
    data1: RefCell<Option<SetRef>>,
    data2: RefCell<Option<SetRef>>,
}

impl CommandMessage {
    /// The constructor: count 0, type 0, both references null.
    pub fn new() -> Self {
        Self::default()
    }
    pub fn ref_count(&self) -> u32 {
        self.refs.get().count()
    }
    /// `AddRef` on either pointer (one shared count, §4.5).
    pub fn add_ref(&self) -> u32 {
        let mut r = self.refs.get();
        let n = r.add_ref();
        self.refs.set(r);
        n
    }
    /// `Release` on either pointer; at count ≤ 1 the destructor releases both sets.
    pub fn release(&self) -> u32 {
        let mut r = self.refs.get();
        let n = r.release();
        self.refs.set(r);
        if r.is_destroyed() {
            self.set_sets(None, None);
        }
        n
    }
    /// `GetType()`: the command ID.
    pub fn get_type(&self) -> u32 {
        self.msg_type.get()
    }
    /// `GetData1()`: the input set.
    pub fn get_data1(&self) -> Option<SetRef> {
        self.data1.borrow().clone()
    }
    /// `GetData2()`: the output set.
    pub fn get_data2(&self) -> Option<SetRef> {
        self.data2.borrow().clone()
    }

    /// QueryInterface (`0x009297B0`): the command IIDs give the primary pointer, the standard
    /// IIDs the `+0x04` pointer, each with one more reference; else `None`.
    pub fn query_interface(&self, iid: u32) -> Option<MessageFace> {
        let face = if COMMAND_MESSAGE_IIDS.contains(&iid) {
            MessageFace::Command
        } else if STANDARD_MESSAGE_IIDS.contains(&iid) {
            MessageFace::Standard
        } else {
            return None;
        };
        self.add_ref();
        Some(face)
    }

    /// The standard part's `Create` (slot 3): a plain standard message of the same type.
    pub fn create(&self) -> StandardMessage {
        StandardMessage {
            msg_type: self.get_type(),
        }
    }

    /// Takes counted references to the new sets and releases the old ones.
    fn set_sets(&self, input: Option<SetRef>, output: Option<SetRef>) {
        for s in input.iter().chain(output.iter()) {
            s.borrow_mut().add_ref();
        }
        let old1 = self.data1.replace(input);
        let old2 = self.data2.replace(output);
        // The destructor releases +0x34 then +0x30.
        for s in old2.iter().chain(old1.iter()) {
            s.borrow_mut().release();
        }
    }
}

/// A plain standard message, as `Create` on the command message makes it (§4.5); it does not
/// answer the command IIDs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StandardMessage {
    pub msg_type: u32,
}

impl StandardMessage {
    pub fn query_interface(&self, iid: u32) -> Option<MessageFace> {
        STANDARD_MESSAGE_IIDS.contains(&iid).then_some(MessageFace::Standard)
    }
}

/// The message-based command dispatcher (§4.1).
pub struct Message2CommandDispatcher {
    refs: RefCount,
    server: Option<Rc<dyn CommandMessageServer>>,
    message: Option<Rc<CommandMessage>>,
    default_in: Option<SetRef>,
    default_out: Option<SetRef>,
    /// Created and initialised by `Init`, never used *[quirk]*.
    generator: Option<StringCommandGenerator>,
    conv: Rc<dyn ValueConvert>,
}

impl Message2CommandDispatcher {
    /// The constructor zeroes everything (§4.1).
    pub fn new(conv: Rc<dyn ValueConvert>) -> Self {
        Self {
            refs: RefCount::default(),
            server: None,
            message: None,
            default_in: None,
            default_out: None,
            generator: None,
            conv,
        }
    }

    pub fn ref_count(&self) -> u32 {
        self.refs.count()
    }
    pub fn is_destroyed(&self) -> bool {
        self.refs.is_destroyed()
    }
    pub fn message(&self) -> Option<&Rc<CommandMessage>> {
        self.message.as_ref()
    }
    pub fn default_input(&self) -> Option<&SetRef> {
        self.default_in.as_ref()
    }
    pub fn default_output(&self) -> Option<&SetRef> {
        self.default_out.as_ref()
    }
    pub fn generator(&self) -> Option<&StringCommandGenerator> {
        self.generator.as_ref()
    }

    /// Slot 0.
    pub fn query_interface(&mut self, iid: u32) -> bool {
        if DISPATCHER_IIDS.contains(&iid) {
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
    /// Slot 2: as §1.2; the destructor releases `+0x18`, `+0x14`, `+0x10`, `+0x0C`, `+0x08`
    /// in that order, without `Shutdown`.
    pub fn release(&mut self) -> u32 {
        let n = self.refs.release();
        if self.refs.is_destroyed() {
            if let Some(mut g) = self.generator.take() {
                g.release();
            }
            self.release_members();
        }
        n
    }

    fn release_members(&mut self) {
        if let Some(s) = self.default_out.take() {
            s.borrow_mut().release();
        }
        if let Some(s) = self.default_in.take() {
            s.borrow_mut().release();
        }
        if let Some(m) = self.message.take() {
            m.release();
        }
        self.server = None;
    }

    /// Slot 3 (§4.3): creates each member that is still null. `message_server` is the framework
    /// service `0x04FA845B` (`None` when absent), `command_server` the service the generator's
    /// `Init` takes. Always true.
    pub fn init(
        &mut self,
        message_server: Option<Rc<dyn CommandMessageServer>>,
        command_server: Option<Rc<dyn CommandServer>>,
    ) -> bool {
        if self.server.is_none() {
            self.server = message_server;
        }
        if self.message.is_none() {
            let m = Rc::new(CommandMessage::new());
            m.add_ref();
            self.message = Some(m);
        }
        if self.default_in.is_none() {
            self.default_in = Some(CommandParameterSet::new_owned());
        }
        if self.default_out.is_none() {
            self.default_out = Some(CommandParameterSet::new_owned());
        }
        if self.generator.is_none() {
            let mut g = StringCommandGenerator::new(self.conv.clone());
            g.add_ref();
            g.init(command_server);
            self.generator = Some(g);
        }
        true
    }

    /// Slot 4 (§4.3): releases the message server, the message and both default sets, then
    /// shuts down and releases the generator. True.
    pub fn shutdown(&mut self) -> bool {
        self.release_members();
        if let Some(mut g) = self.generator.take() {
            g.shutdown();
            g.release();
        }
        true
    }

    /// Slot 5 `ExecuteCommand(id, in, out)` (§4.4).
    pub fn execute_command(&mut self, id: u32, input: Option<&SetRef>, output: Option<&SetRef>) -> i32 {
        let Some(server) = self.server.clone() else { return 2 };
        let (Some(message), Some(din), Some(dout)) =
            (self.message.clone(), self.default_in.clone(), self.default_out.clone())
        else {
            // Unreachable after Init, which creates every member.
            return 2;
        };
        let input = match input {
            Some(s) => s.clone(),
            None => {
                din.borrow_mut().set_parameter_count(0);
                din
            }
        };
        let output = match output {
            Some(s) => s.clone(),
            None => {
                dout.borrow_mut().set_parameter_count(0);
                dout
            }
        };
        message.msg_type.set(id);
        message.set_sets(Some(input), Some(output.clone()));
        // The result of MessageSend is ignored. No borrow is held during delivery: handlers
        // write the output set and may execute nested commands (GC3) *[quirk]*.
        server.message_send(&message);
        let status = output
            .borrow()
            .get_parameter(STATUS_PARAMETER_ID)
            .map(|v| self.conv.get_val_sint32(v).unwrap_or(0));
        match status {
            Some(s) => s,
            None => {
                output.borrow_mut().set_status_parameter_value(0);
                0
            }
        }
    }
}
