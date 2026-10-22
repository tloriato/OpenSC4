// SPDX-License-Identifier: GPL-3.0-or-later
//! Pure Rust cGZLuaScriptServer owner: docs/specs/lua-host.md §6.3 only.
//! No VM execution, command generator, parser or message delivery is implemented here.
//! Successful paths require valid foreign interfaces and available storage. Explicit
//! failures mark native undefined paths; they promise neither recovery nor rollback.
mod arguments;
mod breakpoints;
mod foreign;
pub use foreign::{Failure, Foreign, Handle, Interface, Primitive, RecordToken, StackOp, Value};
use std::collections::BTreeMap;

// LH1: these three roles are distinct; the base constructor takes service ID
// and signed priority, not a class/service pair.
pub const CLASS_ID: u32 = 0xcbd101a9;
pub const SERVICE_ID: u32 = 0x4bd10133;
pub const SERVICE_PRIORITY: i32 = -999999;
pub const SCRIPT_SERVER_IID: u32 = 0x4bd10129;
pub const CREATED: u32 = 0x6ba990d8;
pub const TERMINATED: u32 = 0xcba990d3;
pub const PAUSE_CHANGED: u32 = 0xebaaf9be;
pub const BREAKPOINT_CHANGED: u32 = 0x8bd13ee2;
pub const GLOBALS: i32 = -10001;
pub const REGISTRY: i32 = -10000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Breakpoint {
    pub id: i32,
    pub name: Vec<u8>,
    pub line: i32,
    pub enabled: u8,
    pub condition: Vec<u8>,
}

impl Default for Breakpoint {
    fn default() -> Self {
        Self {
            id: -1,
            name: Vec::new(),
            line: -1,
            enabled: 1,
            condition: Vec::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Record {
    token: RecordToken,
    thread: Option<Handle>,
    pub source: Vec<u8>,
    pub name: Vec<u8>,
    pub arguments: Vec<u8>,
    pub pause_count: i32,
    pub resume_transitions: u32,
    pub schedulable: bool,
    pub auto_yield: bool,
    pub last_line: i32,
    pub breakpoints: Vec<Breakpoint>,
}

impl Record {
    fn new(token: RecordToken, source: &[u8], name: &[u8]) -> Self {
        Self {
            token,
            thread: None,
            source: source.to_vec(),
            name: name.to_vec(),
            arguments: Vec::new(),
            pause_count: 0,
            resume_transitions: 0,
            schedulable: false,
            auto_yield: false,
            last_line: -1,
            breakpoints: Vec::new(),
        }
    }
    pub fn thread(&self) -> Option<Handle> {
        self.thread
    }
    pub fn token(&self) -> RecordToken {
        self.token
    }
}

#[derive(Default)]
pub struct InfoOutputs<'a> {
    pub thread: Option<&'a mut Option<Handle>>,
    pub source: Option<&'a mut Vec<u8>>,
    pub name: Option<&'a mut Vec<u8>>,
    pub pause_count: Option<&'a mut i32>,
    pub resume_transitions: Option<&'a mut u32>,
    pub schedulable: Option<&'a mut bool>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InterfaceView {
    Primary,
    Base(Handle),
}

#[derive(Default)]
pub struct ScriptServer {
    references: u32,
    init_latch: bool,
    tick_depth: u32,
    attempt_serial: u32,
    last_created: Option<i32>,
    lua: Option<Handle>,
    generator: Option<Handle>,
    records: BTreeMap<i32, Record>,
    current: Option<(i32, RecordToken)>,
    next_token: u64,
    lifecycle_epoch: u64,
    destroying: bool,
    destroyed: bool,
}

type Borrow = Option<(i32, RecordToken)>;
type Result<T> = std::result::Result<T, Failure>;

impl ScriptServer {
    pub fn new() -> Self {
        Self::default()
    }
    /// Factory allocation is distinct from construction. Unavailable factory
    /// storage returns null; record and message storage are preconditions.
    pub fn factory(allocation_available: bool) -> Option<Self> {
        allocation_available.then(Self::new)
    }
    pub fn ref_count(&self) -> u32 {
        self.references
    }
    pub fn init_latched(&self) -> bool {
        self.init_latch
    }
    pub fn tick_depth(&self) -> u32 {
        self.tick_depth
    }
    pub fn attempt_serial(&self) -> u32 {
        self.attempt_serial
    }
    pub fn get_last_created_script_id(&self) -> i32 {
        self.last_created.unwrap_or(-1)
    }
    /// Borrowed, with no AddRef.
    pub fn get_lua(&self) -> Option<Handle> {
        self.lua
    }
    pub fn record(&self, id: i32) -> Option<&Record> {
        self.records.get(&id)
    }
    pub fn is_destroyed(&self) -> bool {
        self.destroyed
    }

    fn call(&mut self, host: &mut impl Foreign, borrow: Borrow, call: Primitive) -> Result<Value> {
        if self.destroyed {
            return Err(Failure::Destroyed);
        }
        let epoch = self.lifecycle_epoch;
        let value = host.invoke(self, call)?;
        if self.destroyed || self.lifecycle_epoch != epoch {
            return Err(Failure::InvalidatingCallback);
        }
        if let Some((id, token)) = borrow {
            if self.records.get(&id).map(|r| r.token) != Some(token) {
                return Err(Failure::InvalidatingCallback);
            }
        }
        Ok(value)
    }
    fn unit(&mut self, host: &mut impl Foreign, borrow: Borrow, call: Primitive) -> Result<()> {
        match self.call(host, borrow, call)? {
            Value::Unit => Ok(()),
            _ => Err(Failure::UnexpectedReply),
        }
    }
    fn boolean(&mut self, host: &mut impl Foreign, borrow: Borrow, call: Primitive) -> Result<bool> {
        match self.call(host, borrow, call)? {
            Value::Bool(v) => Ok(v),
            _ => Err(Failure::UnexpectedReply),
        }
    }
    fn integer(&mut self, host: &mut impl Foreign, borrow: Borrow, call: Primitive) -> Result<i32> {
        match self.call(host, borrow, call)? {
            Value::Int(v) => Ok(v),
            _ => Err(Failure::UnexpectedReply),
        }
    }
    fn handle(&mut self, host: &mut impl Foreign, borrow: Borrow, call: Primitive) -> Result<Option<Handle>> {
        match self.call(host, borrow, call)? {
            Value::Handle(v) => Ok(v),
            _ => Err(Failure::UnexpectedReply),
        }
    }
    fn bytes(&mut self, host: &mut impl Foreign, borrow: Borrow, call: Primitive) -> Result<Vec<u8>> {
        match self.call(host, borrow, call)? {
            Value::Bytes(v) => Ok(v),
            _ => Err(Failure::UnexpectedReply),
        }
    }
    fn release_handle(
        &mut self,
        host: &mut impl Foreign,
        borrow: Borrow,
        interface: Interface,
        handle: Handle,
    ) -> Result<()> {
        self.unit(host, borrow, Primitive::Release { interface, handle })
    }
    fn record_key(&self, id: i32) -> Borrow {
        self.records.get(&id).map(|r| (id, r.token))
    }
    fn thread_at(&self, id: i32) -> Result<Handle> {
        self.records
            .get(&id)
            .and_then(|r| r.thread)
            .ok_or(Failure::MissingThread)
    }

    pub fn query_interface(&mut self, host: &mut impl Foreign, iid: u32) -> Result<Option<InterfaceView>> {
        if self.destroyed {
            return Err(Failure::Destroyed);
        }
        if iid == 1 || iid == SCRIPT_SERVER_IID {
            self.add_ref()?;
            Ok(Some(InterfaceView::Primary))
        } else {
            match self.call(host, None, Primitive::BaseQuery(iid))? {
                Value::Interface(v) => Ok(v.map(InterfaceView::Base)),
                _ => Err(Failure::UnexpectedReply),
            }
        }
    }
    pub fn add_ref(&mut self) -> Result<u32> {
        if self.destroyed {
            return Err(Failure::Destroyed);
        }
        self.references = self.references.wrapping_add(1);
        Ok(self.references)
    }
    pub fn release(&mut self, host: &mut impl Foreign) -> Result<u32> {
        if self.destroyed {
            return Err(Failure::Destroyed);
        }
        if self.destroying {
            return Err(Failure::InvalidatingCallback);
        }
        if self.references > 1 {
            self.references -= 1;
            return Ok(self.references);
        }
        self.shutdown(host)?;
        self.deleting_destructor(host, 1)?;
        Ok(0)
    }
    /// Models the +8 destructor view, including direct plain destruction. The normal
    /// Release dispatch calls Shutdown first; this entry deliberately does not.
    pub fn deleting_destructor(&mut self, host: &mut impl Foreign, flags: u32) -> Result<()> {
        if self.destroyed {
            return Err(Failure::Destroyed);
        }
        if self.destroying {
            return Err(Failure::InvalidatingCallback);
        }
        self.destroying = true;
        self.lifecycle_epoch = self.lifecycle_epoch.wrapping_add(1);
        // §6.3.2: plain destruction frees map storage, not pointed-to records.
        // Normal destruction already killed the records through virtual Shutdown.
        // Do not invent foreign thread releases for a direct, un-shutdown entry.
        self.records.clear();
        if let Some(g) = self.generator.take() {
            self.release_handle(host, None, Interface::Generator, g)?;
        }
        if let Some(l) = self.lua.take() {
            self.release_handle(host, None, Interface::Lua, l)?;
        }
        self.unit(host, None, Primitive::BaseDestroy)?;
        if flags & 1 != 0 {
            self.unit(host, None, Primitive::FreeAllocation)?;
        }
        self.destroyed = true;
        Ok(())
    }
    pub fn init(&mut self, host: &mut impl Foreign) -> Result<bool> {
        if self.destroyed {
            return Err(Failure::Destroyed);
        }
        if !self.init_latch {
            self.init_latch = true;
            self.lifecycle_epoch = self.lifecycle_epoch.wrapping_add(1);
            if self.lua.is_none() {
                self.lua = self.handle(
                    host,
                    None,
                    Primitive::CreateLua {
                        class: 0x4b2811aa,
                        iid: 0x6b2811a6,
                    },
                )?;
                if let Some(l) = self.lua {
                    if !self.boolean(
                        host,
                        None,
                        Primitive::Open {
                            lua: l,
                            libraries: 0x7e,
                        },
                    )? {
                        self.lua = None;
                        self.release_handle(host, None, Interface::Lua, l)?;
                    }
                }
                let Some(lua) = self.lua else {
                    return Ok(false);
                };
                if let Some(old) = self.generator.take() {
                    self.release_handle(host, None, Interface::Generator, old)?;
                }
                self.generator = self.handle(
                    host,
                    None,
                    Primitive::CreateGenerator {
                        class: 0x2bd13fb4,
                        iid: 0x2bd144df,
                    },
                )?;
                if let Some(g) = self.generator {
                    if !self.boolean(
                        host,
                        None,
                        Primitive::InitGenerator {
                            generator: g,
                            lua,
                            global: "sc4app",
                        },
                    )? {
                        self.generator = None;
                        self.release_handle(host, None, Interface::Generator, g)?;
                    }
                }
                if self.generator.is_none() {
                    self.unit(host, None, Primitive::Close(lua))?;
                    self.lua = None;
                    self.release_handle(host, None, Interface::Lua, lua)?;
                    return Ok(false);
                }
            }
            if self.boolean(host, None, Primitive::FrameworkAvailable)? {
                self.unit(host, None, Primitive::RegisterService)?;
            }
        }
        self.setup_global_arguments(host)
    }
    pub fn shutdown(&mut self, host: &mut impl Foreign) -> Result<bool> {
        if self.destroyed {
            return Err(Failure::Destroyed);
        }
        if !self.init_latch {
            return Ok(true);
        }
        self.init_latch = false;
        self.lifecycle_epoch = self.lifecycle_epoch.wrapping_add(1);
        if self.boolean(host, None, Primitive::FrameworkAvailable)? {
            self.unit(host, None, Primitive::UnregisterService)?;
        }
        while let Some(id) = self.records.keys().next().copied() {
            self.kill_script(host, id)?;
        }
        if let Some(g) = self.generator {
            if let Some(target) = self.handle(host, None, Primitive::GeneratorTarget(g))? {
                self.unit(host, None, Primitive::ShutdownGeneratorTarget(target))?;
            }
            self.generator = None;
            self.release_handle(host, None, Interface::Generator, g)?;
        }
        if let Some(l) = self.lua {
            self.unit(host, None, Primitive::Close(l))?;
            self.lua = None;
            self.release_handle(host, None, Interface::Lua, l)?;
        }
        Ok(true)
    }

    fn notify(
        &mut self,
        host: &mut impl Foreign,
        borrow: Borrow,
        kind: u32,
        id: i32,
        payload: Option<i32>,
    ) -> Result<()> {
        let service = self.handle(
            host,
            borrow,
            Primitive::FindMessageService {
                service: 0x04fa845b,
                iid: 0x652294c7,
            },
        )?;
        let Some(service) = service else {
            return Ok(());
        };
        let message = self
            .handle(host, borrow, Primitive::AllocateMessage { bytes: 0x2c })?
            .ok_or(Failure::StorageRequired)?;
        self.unit(
            host,
            borrow,
            Primitive::AddRef {
                interface: Interface::Message,
                handle: message,
            },
        )?;
        self.unit(host, borrow, Primitive::MessageType { message, value: kind })?;
        self.unit(host, borrow, Primitive::MessageId { message, value: id })?;
        if let Some(value) = payload {
            self.unit(host, borrow, Primitive::MessagePayload { message, value })?;
        }
        let _ = self.boolean(
            host,
            borrow,
            Primitive::SendMessage {
                service,
                message,
                flags: 0,
            },
        )?;
        self.release_handle(host, borrow, Interface::Message, message)?;
        self.release_handle(host, borrow, Interface::MessageService, service)
    }
    pub fn enumerate_script_ids(&self, output: &mut Vec<i32>) {
        output.extend(self.records.keys().copied());
    }
    pub fn get_script_info(&mut self, host: &mut impl Foreign, id: i32, outputs: InfoOutputs<'_>) -> Result<bool> {
        let Some(key) = self.record_key(id) else {
            return Ok(false);
        };
        if let Some(out) = outputs.thread {
            let thread = self.records[&id].thread;
            *out = thread;
            if let Some(handle) = thread {
                self.unit(
                    host,
                    Some(key),
                    Primitive::AddRef {
                        interface: Interface::Thread,
                        handle,
                    },
                )?;
            }
        }
        let r = &self.records[&id];
        if let Some(out) = outputs.source {
            out.clone_from(&r.source);
        }
        if let Some(out) = outputs.name {
            out.clone_from(&r.name);
        }
        if let Some(out) = outputs.pause_count {
            *out = r.pause_count;
        }
        if let Some(out) = outputs.resume_transitions {
            *out = r.resume_transitions;
        }
        if let Some(out) = outputs.schedulable {
            *out = r.schedulable;
        }
        Ok(true)
    }
    pub fn get_current_running_script_id(&mut self, host: &mut impl Foreign) -> Result<i32> {
        let Some((id, token)) = self.current else {
            return Ok(-1);
        };
        let Some(r) = self.records.get(&id).filter(|r| r.token == token) else {
            return Err(Failure::InvalidatingCallback);
        };
        let Some(t) = r.thread else {
            return Ok(-1);
        };
        self.integer(host, Some((id, token)), Primitive::ThreadId(t))
    }
    pub fn get_script_auto_yield_enabled(&self, id: i32) -> bool {
        self.records.get(&id).is_some_and(|r| r.auto_yield)
    }
    pub fn set_script_auto_yield_enabled(&mut self, id: i32, enabled: bool) -> bool {
        if let Some(r) = self.records.get_mut(&id) {
            r.auto_yield = enabled;
            true
        } else {
            false
        }
    }
    pub fn pause_script(&mut self, host: &mut impl Foreign, id: i32) -> Result<i32> {
        self.control(host, id, true)
    }
    pub fn resume_script(&mut self, host: &mut impl Foreign, id: i32) -> Result<i32> {
        self.control(host, id, false)
    }
    fn control(&mut self, host: &mut impl Foreign, id: i32, pause: bool) -> Result<i32> {
        if id == -1 {
            let ids: Vec<i32> = self.records.keys().copied().collect();
            // A live VM ID -1 is rejected at creation, so snapshot recursion terminates.
            for id in ids {
                self.control(host, id, pause)?;
            }
            return Ok(1);
        }
        let Some(key) = self.record_key(id) else {
            return Ok(0);
        };
        let r = self.records.get_mut(&id).unwrap();
        r.pause_count = if pause {
            r.pause_count.wrapping_add(1)
        } else {
            r.pause_count.wrapping_sub(1)
        };
        if r.pause_count == i32::from(pause) {
            self.notify(host, Some(key), PAUSE_CHANGED, id, Some(i32::from(pause)))?;
            if !pause {
                let r = self.records.get_mut(&id).unwrap();
                r.resume_transitions = r.resume_transitions.wrapping_add(1);
            }
        }
        Ok(self.records[&id].pause_count)
    }

    pub fn run_script_file(&mut self, host: &mut impl Foreign, filename: &[u8], arguments: &[u8]) -> Result<i32> {
        match self.call(
            host,
            None,
            Primitive::ReadFile {
                path: filename.to_vec(),
                limit: 1_000_000,
            },
        )? {
            Value::File(Some(source)) => self.run_script_string(host, &source, filename, arguments),
            Value::File(None) => Ok(0),
            _ => Err(Failure::UnexpectedReply),
        }
    }
    pub fn run_script_string(
        &mut self,
        host: &mut impl Foreign,
        source: &[u8],
        name: &[u8],
        arguments: &[u8],
    ) -> Result<i32> {
        let lua = self.lua.ok_or(Failure::MissingLua)?;
        self.next_token = self.next_token.checked_add(1).ok_or(Failure::CounterExhausted)?;
        let mut record = Record::new(RecordToken(self.next_token), source, name);
        self.attempt_serial = self.attempt_serial.wrapping_add(1);
        let main = self
            .handle(host, None, Primitive::MainThread(lua))?
            .ok_or(Failure::MissingThread)?;
        if self.integer(
            host,
            None,
            Primitive::LoadBuffer {
                thread: main,
                source: source.to_vec(),
                length: source.len(),
                chunk: name.to_vec(),
            },
        )? != 0
        {
            self.stack(host, None, main, StackOp::Pop(1))?;
            return Ok(-1);
        }
        let component = if self.boolean(host, None, Primitive::FileExists(name.to_vec()))? {
            self.bytes(
                host,
                None,
                Primitive::PathName {
                    path: name.to_vec(),
                    flags: 0,
                },
            )?
        } else {
            name.to_vec()
        };
        let mut display = b"Main - \"".to_vec();
        display.extend(component);
        display.extend(format!("\" - {}", self.attempt_serial as i32).bytes());
        self.stack(host, None, main, StackOp::PushString(display))?;
        self.stack(host, None, main, StackOp::PushValue(-2))?;
        self.stack(host, None, main, StackOp::RawSet(GLOBALS))?;
        let (id, thread) = match self.call(
            host,
            None,
            Primitive::CreateThread {
                lua,
                count: 1,
                iid: 0x2b2810f7,
            },
        )? {
            Value::CreatedThread { id, thread } => (id, thread),
            _ => return Err(Failure::UnexpectedReply),
        };
        record.thread = thread;
        if id == -1 {
            if let Some(t) = record.thread.take() {
                self.release_handle(host, None, Interface::Thread, t)?;
            }
            return Ok(-1);
        }
        let thread = thread.ok_or(Failure::MissingThread)?;
        self.unit(
            host,
            None,
            Primitive::SetUserContext {
                thread,
                record: record.token,
            },
        )?;
        self.last_created = Some(id);
        record.arguments = arguments.to_vec();
        let _ = self.setup_thread_arguments(host, Some(&record))?;
        record.schedulable = true;
        if self.records.contains_key(&id) {
            record.thread = None;
            self.release_handle(host, None, Interface::Thread, thread)?;
            return Err(Failure::DuplicateLiveId(id));
        }
        let key = Some((id, record.token));
        self.records.insert(id, record);
        let state = self
            .handle(host, key, Primitive::RawContext(thread))?
            .ok_or(Failure::MissingThread)?;
        self.unit(
            host,
            key,
            Primitive::InstallLineHook {
                state,
                mask: 4,
                count: 0,
            },
        )?;
        self.notify(host, key, CREATED, id, Some(0))?;
        Ok(id)
    }
    fn erase_record(&mut self, host: &mut impl Foreign, id: i32) -> Result<()> {
        let key = self.record_key(id);
        if let Some(t) = self.records.get_mut(&id).and_then(|r| r.thread.take()) {
            self.release_handle(host, key, Interface::Thread, t)?;
        }
        self.records.remove(&id);
        Ok(())
    }
    pub fn kill_script(&mut self, host: &mut impl Foreign, id: i32) -> Result<bool> {
        let Some(key) = self.record_key(id) else {
            return Ok(false);
        };
        let lua = self.lua.ok_or(Failure::MissingLua)?;
        let thread = self.thread_at(id)?;
        self.unit(host, Some(key), Primitive::DestroyThread { lua, id })?;
        let thread_id = self.integer(host, Some(key), Primitive::ThreadId(thread))?;
        self.notify(host, Some(key), TERMINATED, thread_id, Some(0))?;
        self.erase_record(host, id)?;
        Ok(true)
    }
    pub fn on_tick(&mut self, host: &mut impl Foreign) -> Result<bool> {
        if self.destroyed {
            return Err(Failure::Destroyed);
        }
        self.tick_depth = self.tick_depth.wrapping_add(1);
        if self.tick_depth != 1 {
            self.tick_depth = self.tick_depth.wrapping_sub(1);
            return Ok(true);
        }
        let result = self.tick_pass(host);
        self.tick_depth = self.tick_depth.wrapping_sub(1);
        result.map(|()| true)
    }
    fn tick_pass(&mut self, host: &mut impl Foreign) -> Result<()> {
        use std::ops::Bound::{Excluded, Unbounded};
        let mut previous = None;
        loop {
            let next = match previous {
                None => self.records.keys().next().copied(),
                Some(id) => self.records.range((Excluded(id), Unbounded)).next().map(|(&id, _)| id),
            };
            let Some(id) = next else {
                return Ok(());
            };
            previous = Some(id);
            let r = &self.records[&id];
            if r.pause_count > 0 || !r.schedulable {
                continue;
            }
            let key = Some((id, r.token));
            let thread = r.thread.ok_or(Failure::MissingThread)?;
            if self.integer(host, key, Primitive::Status(thread))? == 1 && !self.should_break(host, id)? {
                let lua = self.lua.ok_or(Failure::MissingLua)?;
                self.current = key;
                let resumed = (|| {
                    let id = self.integer(host, key, Primitive::ThreadId(thread))?;
                    self.boolean(
                        host,
                        key,
                        Primitive::ResumeThread {
                            lua,
                            id,
                            count: 0,
                            arguments: None,
                        },
                    )
                })();
                self.current = None;
                let _ = resumed?;
            }
            let status = self.integer(host, key, Primitive::Status(thread))?;
            if status != 1 {
                let thread_id = self.integer(host, key, Primitive::ThreadId(thread))?;
                self.notify(host, key, TERMINATED, thread_id, Some(0))?;
                if status == 0 {
                    let lua = self.lua.ok_or(Failure::MissingLua)?;
                    let id = self.integer(host, key, Primitive::ThreadId(thread))?;
                    self.unit(host, key, Primitive::DestroyThread { lua, id })?;
                }
                self.erase_record(host, id)?;
            }
        }
    }
}

#[cfg(test)]
mod tests;
