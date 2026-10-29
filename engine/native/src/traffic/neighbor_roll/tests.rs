// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, fake_data, Fake, Object};
use crate::traffic::test_support::{float_environment, invoke, object, table};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Fill(u32, u32, u32),
    Framework,
    Query(u32, u32),
    ReleaseTemporary(usize),
    Allocate(u32),
    Connections,
    Holder(usize),
    Get(usize, u32),
    Set(usize, u32, Vec<u32>),
    Property(usize, u32),
    Variant,
    Kind,
    Values(bool),
    Capacity(usize, u32, u32),
    Clear,
    Deallocate(usize),
    Free(usize),
}
#[derive(Default)]
struct State {
    calls: Vec<Call>,
    fills: usize,
    vectors: [usize; 2],
    allocator: usize,
    sentinel: usize,
    first: usize,
    framework: usize,
    property: usize,
    variant: usize,
    values: usize,
    narrow: bool,
    arrivals: Vec<(usize, u32, [u32; 9])>,
    reset_cache: usize,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn log(c: Call) {
    STATE.with(|s| s.borrow_mut().calls.push(c));
}
fn calls() -> Vec<Call> {
    STATE.with(|s| std::mem::take(&mut s.borrow_mut().calls))
}
unsafe extern "thiscall" fn fill(this: *mut u32, position: u32, count: u32, zero: *const u32) -> u32 {
    log(Call::Fill(position, count, unsafe { zero.read() }));
    let p = STATE.with(|s| {
        let mut s = s.borrow_mut();
        let p = s.vectors[s.fills];
        s.fills += 1;
        p as u32
    });
    unsafe {
        this.write(p);
        this.add(1).write(p + 36);
        this.add(2).write(p + 36);
    }
    0
}
extern "C" fn framework() -> *mut u8 {
    log(Call::Framework);
    STATE.with(|s| s.borrow().framework as *mut u8)
}
unsafe extern "thiscall" fn query(_: *mut u8, cls: u32, iid: u32, out: *mut *mut u8) -> u32 {
    log(Call::Query(cls, iid));
    unsafe {
        out.write(STATE.with(|s| s.borrow().allocator as *mut u8));
    }
    1
}
unsafe extern "thiscall" fn release_temporary(this: *mut *mut u8) -> u32 {
    log(Call::ReleaseTemporary(unsafe { this.read() } as usize));
    unsafe {
        this.write(core::ptr::null_mut());
    }
    0
}
extern "thiscall" fn allocate(_: *mut u8, size: u32) -> *mut u8 {
    log(Call::Allocate(size));
    STATE.with(|s| s.borrow().sentinel as *mut u8)
}
unsafe extern "thiscall" fn connections(_: *mut u8, list: *mut u32) -> u32 {
    log(Call::Connections);
    STATE.with(|s| {
        let s = s.borrow();
        let sentinel = unsafe { list.read() } as *mut u8;
        assert_eq!(unsafe { word(sentinel, 0) }, sentinel as u32);
        assert_eq!(unsafe { word(sentinel, 4) }, sentinel as u32);
        unsafe {
            store(sentinel, 0, if s.first == 0 { sentinel as u32 } else { s.first as u32 });
        }
    });
    0
}
extern "thiscall" fn holder(this: *mut u8) -> *mut u8 {
    log(Call::Holder(this as usize));
    unsafe { word(this, 4) as *mut u8 }
}
unsafe extern "C" fn get(holder: *mut u8, id: u32, out: *mut u32) -> u32 {
    log(Call::Get(holder as usize, id));
    STATE.with(|s| {
        let s = s.borrow();
        if let Some((_, _, values)) = s.arrivals.iter().find(|&&(h, k, _)| h == holder as usize && k == id) {
            let dest = unsafe { out.read() } as *mut u32;
            unsafe {
                core::ptr::copy_nonoverlapping(values.as_ptr(), dest, 9);
            }
            0xABCD_EF01
        } else {
            0xABCD_EF00
        }
    })
}
unsafe extern "C" fn set(holder: *mut u8, id: u32, vector: *const u32) -> u32 {
    let p = unsafe { vector.read() } as *const u32;
    let len = (unsafe { vector.add(1).read() }.wrapping_sub(p as u32) / 4) as usize;
    log(Call::Set(holder as usize, id, unsafe {
        std::slice::from_raw_parts(p, len).to_vec()
    }));
    0
}
extern "thiscall" fn property(this: *mut u8, id: u32) -> *mut u8 {
    log(Call::Property(this as usize, id));
    STATE.with(|s| s.borrow().property as *mut u8)
}
extern "thiscall" fn variant(_: *mut u8) -> *mut u8 {
    log(Call::Variant);
    STATE.with(|s| s.borrow().variant as *mut u8)
}
extern "thiscall" fn kind(_: *mut u8) -> u32 {
    log(Call::Kind);
    STATE.with(|s| if s.borrow().narrow { 0x8004 } else { 0x8006 })
}
extern "thiscall" fn values16(_: *mut u8) -> *mut u8 {
    log(Call::Values(true));
    STATE.with(|s| s.borrow().values as *mut u8)
}
extern "thiscall" fn values32(_: *mut u8) -> *mut u8 {
    log(Call::Values(false));
    STATE.with(|s| s.borrow().values as *mut u8)
}
extern "fastcall" fn capacity(holder: *mut u8, index: u32, value: u32) {
    log(Call::Capacity(holder as usize, index, value));
    STATE.with(|s| {
        let s = s.borrow();
        unsafe {
            if s.narrow {
                (s.values as *mut u16).add(index as usize).write(value as u16);
            } else {
                (s.values as *mut u32).add(index as usize).write(value);
            }
        }
    });
}
#[unsafe(naked)]
unsafe extern "C" fn capacity_entry() {
    core::arch::naked_asm!("mov edx, [esp + 4]", "push ebx", "mov ecx, edi", "call {}",
        "mov eax, 0x12345678", "mov ecx, 0x23456789", "mov edx, 0x3456789a", "ret", sym capacity);
}
unsafe extern "thiscall" fn clear(_: *mut u32) -> u32 {
    log(Call::Clear);
    STATE.with(|s| {
        let s = s.borrow();
        if s.reset_cache != 0 {
            unsafe {
                (s.reset_cache as *mut u32).write(0);
            }
        }
    });
    0
}
extern "thiscall" fn deallocate(_: *mut u8, p: u32) -> u32 {
    log(Call::Deallocate(p as usize));
    0
}
extern "C" fn free_body(p: u32) {
    log(Call::Free(p as usize));
}
#[unsafe(naked)]
unsafe extern "C" fn free_entry() {
    core::arch::naked_asm!("push [esp + 4]", "call {}", "add esp, 4",
        "mov eax, 0xaabbccdd", "mov ecx, 0x55667788", "mov edx, 0x99887766", "ret", sym free_body);
}

struct World {
    manager: Object,
    allocator: Object,
    framework: Object,
    sentinel: Object,
    property: Object,
    variant: Object,
    connections: Vec<Object>,
    nodes: Vec<Object>,
    holders: Vec<Object>,
    vectors: [Box<[u32; 9]>; 2],
    values: Box<[u32; 12]>,
    cache: Box<*mut u8>,
    _manager: Box<*mut u8>,
    _tables: Vec<Vec<usize>>,
    _fakes: Vec<Fake>,
}
impl World {
    fn new(count: usize, narrow: bool, present: bool) -> Self {
        let tables = vec![
            table(&[(0x40, connections as *const () as usize)]),
            table(&[
                (0x0C, allocate as *const () as usize),
                (0x10, deallocate as *const () as usize),
            ]),
            table(&[(0x14, query as *const () as usize)]),
            table(&[(0x0C, holder as *const () as usize)]),
            table(&[(0x24, property as *const () as usize)]),
            table(&[(0x14, variant as *const () as usize)]),
            table(&[
                (0x0C, kind as *const () as usize),
                (0x18C, values16 as *const () as usize),
                (0x19C, values32 as *const () as usize),
            ]),
        ];
        let mut w = Self {
            manager: object(4, &tables[0]),
            allocator: object(4, &tables[1]),
            framework: object(4, &tables[2]),
            sentinel: Object::new(12),
            property: object(4, &tables[5]),
            variant: object(4, &tables[6]),
            connections: (0..count).map(|_| object(8, &tables[3])).collect(),
            nodes: (0..count).map(|_| Object::new(12)).collect(),
            holders: (0..count).map(|_| object(4, &tables[4])).collect(),
            vectors: [Box::new([0; 9]), Box::new([0; 9])],
            values: Box::new([0; 12]),
            cache: Box::new(core::ptr::null_mut()),
            _manager: Box::new(core::ptr::null_mut()),
            _tables: tables,
            _fakes: vec![
                fake(0x0042_3320, fill as *const () as usize),
                fake(0x0087_93EC, framework as *const () as usize),
                fake(0x0041_1870, release_temporary as *const () as usize),
                fake(0x005F_F510, get as *const () as usize),
                fake(0x005F_E6B0, set as *const () as usize),
                fake(0x0070_DE70, capacity_entry as *const () as usize),
                fake(0x0041_1D70, clear as *const () as usize),
                fake(0x0090_CF63, free_entry as *const () as usize),
            ],
        };
        *w.cache = w.allocator.ptr();
        *w._manager = w.manager.ptr();
        w._fakes
            .push(fake_data(0x00B2_07DC, (&mut *w.cache as *mut *mut u8).cast()));
        w._fakes
            .push(fake_data(0x00B4_3D8C, (&mut *w._manager as *mut *mut u8).cast()));
        for i in 0..count {
            unsafe {
                store(w.connections[i].ptr(), 4, w.holders[i].ptr() as u32);
                store(w.nodes[i].ptr(), 8, w.connections[i].ptr() as u32);
                let next = if i + 1 == count {
                    w.sentinel.ptr()
                } else {
                    w.nodes[i + 1].ptr()
                };
                store(w.nodes[i].ptr(), 0, next as u32);
            }
        }
        if narrow {
            unsafe {
                core::ptr::copy_nonoverlapping(
                    [7u16, 7, 7, 7, 1, 2, 3, 4, 9, 9, 9, 9].as_ptr(),
                    w.values.as_mut_ptr().cast(),
                    12,
                );
            }
        } else {
            *w.values = [7, 7, 7, 7, 70_000, 80_000, 90_000, 100_000, 9, 9, 9, 9];
        }
        STATE.with(|s| {
            *s.borrow_mut() = State {
                vectors: [w.vectors[0].as_mut_ptr() as usize, w.vectors[1].as_mut_ptr() as usize],
                allocator: w.allocator.ptr() as usize,
                sentinel: w.sentinel.ptr() as usize,
                framework: w.framework.ptr() as usize,
                first: if count == 0 { 0 } else { w.nodes[0].ptr() as usize },
                property: if present { w.property.ptr() as usize } else { 0 },
                variant: w.variant.ptr() as usize,
                values: w.values.as_mut_ptr() as usize,
                narrow,
                ..State::default()
            }
        });
        w
    }
    fn run(&mut self) -> Registers {
        unsafe {
            invoke(
                fc4p_00714be0 as *const () as usize,
                0x1234 as *mut u8,
                &[0x8765_4321],
                0xABCD_1234,
                0xDEAD_BEEF,
                false,
            )
        }
    }
    fn setup_calls(&mut self) -> Vec<Call> {
        vec![
            Call::Fill(0, 9, 0),
            Call::Fill(0, 9, 0),
            Call::Allocate(12),
            Call::Connections,
        ]
    }
    fn cleanup_calls(&mut self) -> Vec<Call> {
        vec![
            Call::Clear,
            Call::Deallocate(self.sentinel.ptr() as usize),
            Call::Free(self.vectors[1].as_ptr() as usize),
            Call::Free(self.vectors[0].as_ptr() as usize),
        ]
    }
}

#[test]
fn neighbor_roll_moves_present_arrivals_clears_arrays_and_rolls_narrow_capacity() {
    let mut w = World::new(1, true, true);
    let h = w.holders[0].ptr() as usize;
    let arrival = [0, 5, 0, 0, 0, 0, 0, 0, 0];
    STATE.with(|s| s.borrow_mut().arrivals.push((h, 0xAA9E_A81B, arrival)));
    let env = float_environment();
    assert_eq!(
        w.run(),
        Registers {
            eax: 0xAABB_CC01,
            ecx: 0x5566_7788,
            edx: 0x9988_7766
        }
    );
    assert_eq!(float_environment(), env);
    let actual = unsafe { std::slice::from_raw_parts(w.values.as_ptr().cast::<u16>(), 12) };
    assert_eq!(actual, [7, 7, 7, 7, 0, 0, 0, 0, 1, 2, 3, 4]);
    let mut expected = w.setup_calls();
    expected.push(Call::Holder(w.connections[0].ptr() as usize));
    for id in 0xAA9E_A81A..0xAA9E_A81D {
        expected.push(Call::Get(h, id));
        if id == 0xAA9E_A81B {
            expected.push(Call::Set(h, id + 3, arrival.to_vec()));
        }
        expected.push(Call::Set(h, id, vec![0; 9]));
    }
    for index in 4..8 {
        expected.extend([
            Call::Property(h, 0x8A0B_4100),
            Call::Variant,
            Call::Kind,
            Call::Values(true),
            Call::Capacity(h, index + 4, index - 3),
            Call::Capacity(h, index, 0),
        ]);
    }
    expected.extend(w.cleanup_calls());
    assert_eq!(calls(), expected);
}

#[test]
fn neighbor_roll_wide_and_missing_capacity_follow_connection_list_order() {
    for present in [true, false] {
        let mut w = World::new(2, false, present);
        w.run();
        let actual = calls();
        let mut expected = w.setup_calls();
        for i in 0..2 {
            let h = w.holders[i].ptr() as usize;
            expected.push(Call::Holder(w.connections[i].ptr() as usize));
            for id in 0xAA9E_A81A..0xAA9E_A81D {
                expected.extend([Call::Get(h, id), Call::Set(h, id, vec![0; 9])]);
            }
            for index in 4..8 {
                expected.push(Call::Property(h, 0x8A0B_4100));
                if present {
                    expected.extend([Call::Variant, Call::Kind, Call::Values(false)]);
                }
                let value = if present && i == 0 { 30_000 + index * 10_000 } else { 0 };
                expected.extend([Call::Capacity(h, index + 4, value), Call::Capacity(h, index, 0)]);
            }
        }
        expected.extend(w.cleanup_calls());
        assert_eq!(actual, expected);
    }
}

#[test]
fn empty_neighbor_list_acquires_and_reacquires_the_cached_allocator() {
    let mut w = World::new(0, true, false);
    *w.cache = core::ptr::null_mut();
    STATE.with(|s| s.borrow_mut().reset_cache = (&mut *w.cache as *mut *mut u8) as usize);
    assert_eq!(w.run().eax, 0xAABB_CC01);
    assert_eq!(*w.cache, w.allocator.ptr());
    let acquire = [
        Call::Framework,
        Call::Query(0x3AE4_BEA3, 0x3AE4_BEAB),
        Call::ReleaseTemporary(w.allocator.ptr() as usize),
    ];
    let mut expected = vec![Call::Fill(0, 9, 0), Call::Fill(0, 9, 0)];
    expected.extend(acquire);
    expected.extend([Call::Allocate(12), Call::Connections, Call::Clear]);
    expected.extend([
        Call::Framework,
        Call::Query(0x3AE4_BEA3, 0x3AE4_BEAB),
        Call::ReleaseTemporary(w.allocator.ptr() as usize),
    ]);
    expected.extend(w.cleanup_calls().into_iter().skip(1));
    assert_eq!(calls(), expected);
}
