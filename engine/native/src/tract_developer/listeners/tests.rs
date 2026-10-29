// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, Fake, GameFpu, Object};
use crate::tract_developer::testing::{run, snapshot, PreserveMachine};
use std::cell::RefCell;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Call {
    Lock(u32),
    Retain(u32),
    Copy(u32, u32),
    Defer(u32, u32, u32),
    Find(u32, u32),
    Allocate(u32),
    Construct(u32, u8),
    Insert(u32, u32, u32),
    Append(u32, u32),
    Assign(u32),
    Flush(u32),
    Release(u32),
}

#[derive(Default)]
struct State {
    calls: Vec<Call>,
    lock: u32,
    found: u32,
    list: u32,
    node: u32,
    append_reference: u32,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }

extern "thiscall" fn lock(this: u32, object: u32) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call::Lock(object));
        unsafe { store(this, 4, s.lock) };
    });
    0x1111_2222_3333_4444
}
extern "thiscall" fn retain(this: u32, object: u32) -> u64 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Retain(object)));
    unsafe { store(this, 0, object) };
    0x2222_3333_4444_5555
}
extern "thiscall" fn copy(this: u32, source: u32) -> u64 {
    let (object, key) = unsafe { (word(source, 0), word(source, 4)) };
    STATE.with(|s| s.borrow_mut().calls.push(Call::Copy(object, key)));
    unsafe {
        store(this, 0, object);
        store(this, 4, key);
    }
    0x3333_4444_5555_6666
}
extern "thiscall" fn defer(this: u32, value: u32) -> u64 {
    let (flag, object, key) = unsafe { (*(value as *const u8), word(value, 4), word(value, 8)) };
    assert_eq!(flag, 1);
    STATE.with(|s| s.borrow_mut().calls.push(Call::Defer(this, object, key)));
    0x4444_5555_6666_7777
}
extern "thiscall" fn find(this: u32, key: u32) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        assert_eq!(unsafe { word(this.wrapping_sub(0x6C), 0x64) }, 1);
        s.calls.push(Call::Find(this, unsafe { word(key, 0) }));
        0x5555_6666_0000_0000 | u64::from(s.found)
    })
}
extern "C" fn allocate(bytes: u32) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call::Allocate(bytes));
        0x6666_7777_0000_0000 | u64::from(s.list)
    })
}
extern "thiscall" fn construct(this: u32, allocator: u32) -> u64 {
    STATE.with(|s| {
        s.borrow_mut()
            .calls
            .push(Call::Construct(this, unsafe { *(allocator as *const u8) }))
    });
    // The caller keeps the allocated list pointer, independently of this result.
    0x7777_8888_1234_5678
}
extern "thiscall" fn insert(this: u32, output: u32, pair: u32) -> u64 {
    STATE.with(|s| {
        s.borrow_mut()
            .calls
            .push(Call::Insert(this, unsafe { word(pair, 0) }, unsafe { word(pair, 4) }))
    });
    unsafe {
        store(output, 0, 0);
    }
    0x8888_9999_2345_6789
}
extern "thiscall" fn append(this: u32, value: u32) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        let input = unsafe { word(value, 0) };
        s.calls.push(Call::Append(this, input));
        assert_eq!(input, 0);
        unsafe {
            store(word(this, 0), 4, s.node);
            store(value, 0, s.append_reference);
        }
    });
    0x9999_AAAA_3456_789A
}
extern "thiscall" fn assign(this: u32, object: u32) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        assert_eq!(this, s.node + 8);
        s.calls.push(Call::Assign(object));
    });
    unsafe {
        store(this, 0, object);
    }
    0xAAAA_BBBB_4567_89AB
}
extern "thiscall" fn flush(this: u32) -> u64 {
    assert_eq!(unsafe { word(this, 0x64) }, 0);
    STATE.with(|s| s.borrow_mut().calls.push(Call::Flush(this)));
    0xBBBB_CCCC_5678_9ABC
}
extern "thiscall" fn release(this: u32) -> u64 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Release(this)));
    0xDEAD_BEEF_CAFE_BABE
}

struct Fixture {
    registry: Object,
    _objects: Vec<Object>,
    _table: Box<[usize; 3]>,
    _fakes: Vec<Fake>,
    lock: u32,
    object: u32,
    list: u32,
    node: u32,
    header: u32,
}

impl Fixture {
    fn new() -> Self {
        STATE.with(|s| *s.borrow_mut() = State::default());
        let table = Box::new([0, 0, release as *const () as usize]);
        let mut objects: Vec<_> = (0..6).map(|_| Object::new(16)).collect();
        let ptrs: Vec<_> = objects.iter_mut().map(|o| o.ptr() as u32).collect();
        let (lock, object, list, node, header, found) = (ptrs[0], ptrs[1], ptrs[2], ptrs[3], ptrs[4], ptrs[5]);
        unsafe {
            for p in [lock, object] {
                store(p, 0, table.as_ptr() as u32);
            }
            store(list, 0, header);
            store(header, 0, header);
            store(header, 4, header);
            store(found, 8, list);
        }
        STATE.with(|s| {
            let s = &mut *s.borrow_mut();
            s.lock = lock;
            s.list = list;
            s.node = node;
            s.found = found;
        });
        let fakes = [
            (0x0048_C9F0, lock_fn()),
            (0x004F_20C0, retain as *const () as usize),
            (0x0092_D1E3, copy as *const () as usize),
            (0x0092_E11A, defer as *const () as usize),
            (0x0058_EFE0, find as *const () as usize),
            (0x005E_55E0, allocate as *const () as usize),
            (0x004F_5A00, construct as *const () as usize),
            (0x0092_FDD8, insert as *const () as usize),
            (0x007E_98C0, append as *const () as usize),
            (0x0054_47B0, assign as *const () as usize),
            (0x0092_DF35, flush as *const () as usize),
        ]
        .into_iter()
        .map(|(a, f)| fake(a, f))
        .collect();
        Self {
            registry: Object::new(0x80),
            _objects: objects,
            _table: table,
            _fakes: fakes,
            lock,
            object,
            list,
            node,
            header,
        }
    }

    fn execute(&mut self, object: u32, key: u32) -> [u32; 2] {
        let this = self.registry.ptr() as u32;
        let before = snapshot(self.registry.ptr(), 0x80);
        let result = run(0x0092_E795, Conv::Thiscall, this, &[object, key]);
        assert_eq!((result.0, result.2), (0xCAFE_BA01, 0xDEAD_BEEF));
        assert_eq!(snapshot(self.registry.ptr(), 0x80), before);
        let mut entry = Entry {
            eax: 0x1234_5678,
            ecx: this,
            edx: 0x9876_5432,
            return_address: 0,
            args: [object, key],
        };
        // Also expose the caller's argument slots, which the ABI adapter keeps in place.
        STATE.with(|s| s.borrow_mut().calls.clear());
        unsafe {
            register_listener(&mut entry);
        }
        assert_eq!((entry.eax, entry.edx), (0xCAFE_BA01, 0xDEAD_BEEF));
        entry.args
    }
}

fn lock_fn() -> usize {
    lock as *const () as usize
}

#[test]
fn existing_list_appends_assigns_and_releases_a_nonzero_temporary() {
    let _fpu = GameFpu::new();
    let _machine = PreserveMachine::new();
    let mut f = Fixture::new();
    STATE.with(|s| s.borrow_mut().append_reference = f.object);
    let this = f.registry.ptr() as u32;
    assert_eq!(f.execute(f.object, 0xAABB_CCDD), [f.object, f.object]);
    assert_eq!(
        STATE.with(|s| s.borrow().calls.clone()),
        vec![
            Call::Lock(this + 0x20),
            Call::Find(this + 0x6C, 0xAABB_CCDD),
            Call::Append(f.list, 0),
            Call::Release(f.object),
            Call::Assign(f.object),
            Call::Flush(this),
            Call::Release(f.lock),
        ]
    );
}

#[test]
fn duplicate_search_walks_nodes_and_leaves_key_argument_intact() {
    let _fpu = GameFpu::new();
    let _machine = PreserveMachine::new();
    let mut f = Fixture::new();
    unsafe {
        store(f.header, 0, f.node);
        store(f.node, 0, f.header);
        store(f.node, 8, f.object);
    }
    let this = f.registry.ptr() as u32;
    assert_eq!(f.execute(f.object, 73), [f.object, 73]);
    assert_eq!(
        STATE.with(|s| s.borrow().calls.clone()),
        vec![
            Call::Lock(this + 0x20),
            Call::Find(this + 0x6C, 73),
            Call::Flush(this),
            Call::Release(f.lock),
        ]
    );
}

#[test]
fn missing_key_allocates_and_constructs_list_then_inserts_before_appending() {
    let _fpu = GameFpu::new();
    let _machine = PreserveMachine::new();
    let mut f = Fixture::new();
    STATE.with(|s| s.borrow_mut().found = 0);
    let this = f.registry.ptr() as u32;
    assert_eq!(f.execute(f.object, 0xAABB_CCDD), [f.object, 0]);
    assert_eq!(
        STATE.with(|s| s.borrow().calls.clone()),
        vec![
            Call::Lock(this + 0x20),
            Call::Find(this + 0x6C, 0xAABB_CCDD),
            Call::Allocate(4),
            Call::Construct(f.list, 0xAA),
            Call::Insert(this + 0x6C, 0xAABB_CCDD, f.list),
            Call::Append(f.list, 0),
            Call::Assign(f.object),
            Call::Flush(this),
            Call::Release(f.lock),
        ]
    );
}

#[test]
fn nested_updates_defer_and_release_both_copies_before_unlocking() {
    let _fpu = GameFpu::new();
    let _machine = PreserveMachine::new();
    for nesting in [1, 7, u32::MAX] {
        for null in [false, true] {
            let mut f = Fixture::new();
            unsafe {
                store(f.registry.ptr() as u32, 0x64, nesting);
            }
            let this = f.registry.ptr() as u32;
            let object = if null { 0 } else { f.object };
            assert_eq!(f.execute(object, 91), [object, 91]);
            let mut expected = vec![
                Call::Lock(this + 0x20),
                Call::Retain(object),
                Call::Copy(object, 91),
                Call::Defer(this + 0x68, object, 91),
            ];
            if !null {
                expected.extend([Call::Release(object), Call::Release(object)]);
            }
            expected.push(Call::Release(f.lock));
            assert_eq!(STATE.with(|s| s.borrow().calls.clone()), expected);
        }
    }
}
