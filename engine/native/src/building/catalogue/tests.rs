// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

use super::*;
use crate::testing::{fake, fake_data, Fake, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Event {
    Begin(usize),
    Assign(usize, usize),
    Insert(usize, u32, u32, u32),
    Reserve(usize, u32),
    Set(usize, u32),
    Erase(usize, u32),
    Tried(usize, usize),
    Find(usize, [u32; 2]),
}
#[derive(Default)]
struct State {
    events: Vec<Event>,
    first: usize,
    map: usize,
    tree: usize,
    found: u32,
    appended: Vec<u32>,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn log(e: Event) {
    STATE.with(|s| s.borrow_mut().events.push(e));
}
fn events() -> Vec<Event> {
    STATE.with(|s| core::mem::take(&mut s.borrow_mut().events))
}
extern "thiscall" fn begin(this: *mut u8, out: *mut *mut u8) -> u64 {
    log(Event::Begin(this as usize));
    STATE.with(|s| {
        let s = s.borrow();
        unsafe {
            out.write(s.first as *mut u8);
            out.add(1).write(s.map as *mut u8);
        }
    });
    0x1122_3344_AABB_CCDD
}
extern "thiscall" fn assign(this: *mut u8, source: *mut u8) -> u64 {
    log(Event::Assign(this as usize, source as usize));
    unsafe {
        let src = TypeVector::from_ptr(source);
        let dst = TypeVector::from_ptr(this);
        let count = length(src.begin(), src.end());
        for i in 0..count {
            write(
                dst.begin().wrapping_add(i as usize),
                read(src.begin().wrapping_add(i as usize)),
            );
        }
        dst.set_end(dst.begin().wrapping_offset(count as isize));
    }
    0x1234_5678_8765_4321
}
extern "thiscall" fn insert(this: *mut u8, position: *mut u8, _cookie: *mut u8, key: *mut u32, a: u32, b: u32) -> u64 {
    let key = unsafe { read(key) };
    log(Event::Insert(this as usize, position as u32, a, b));
    STATE.with(|s| s.borrow_mut().appended.push(key));
    0x9876_5432_FEDC_BA98
}
extern "thiscall" fn reserve(this: *mut u8, count: u32) -> u64 {
    log(Event::Reserve(this as usize, count));
    0
}
extern "thiscall" fn set(this: *mut u8, _out: *mut u8, key: *mut u32) -> u64 {
    let key = unsafe { read(key) };
    log(Event::Set(this as usize, key));
    unsafe {
        let p = this.add(0x10).cast::<u32>();
        write(p, read(p).wrapping_add(1));
    }
    0xAABB_CCDD_1234_5678
}
extern "thiscall" fn erase(this: *mut u8, key: *mut u32) -> u64 {
    log(Event::Erase(this as usize, unsafe { read(key) }));
    0xCAFE_BABE_1234_5678
}
extern "thiscall" fn tried(this: *mut u8, key: *mut u32) -> *mut u8 {
    log(Event::Tried(this as usize, key as usize));
    STATE.with(|s| s.borrow().tree as *mut u8)
}
extern "thiscall" fn find(this: *mut u8, key: *mut u32) -> u64 {
    log(Event::Find(this as usize, unsafe { [read(key), read(key.add(1))] }));
    STATE.with(|s| 0x8877_6655_0000_0000 | u64::from(s.borrow().found))
}

fn data_map(address: u32, begin: &mut *mut *mut u8, end: &mut *mut *mut u8) -> [Fake; 2] {
    [
        fake_data(address, (begin as *mut *mut *mut u8).cast()),
        fake_data(address + 4, (end as *mut *mut *mut u8).cast()),
    ]
}

#[test]
fn building_array_clamps_count_keeps_total_and_traverses_chains_and_buckets() {
    let _begin = fake(0x004B_0390, begin as *const () as usize);
    let mut map = [0u32; 4];
    let mp = map.as_mut_ptr().cast::<u8>();
    let _map = fake_data(0x00B4_8548, mp);
    let mut total = 3u32;
    let _count = fake_data(0x00B4_8558, (&mut total as *mut u32).cast());
    let mut c = [0u32, 7];
    let cp = c.as_mut_ptr().cast::<u8>();
    let mut b = [0u32, 5];
    let bp = b.as_mut_ptr().cast::<u8>();
    let mut a = [bp as u32, 1];
    let ap = a.as_mut_ptr().cast::<u8>();
    let mut buckets = [null_mut(), ap, null_mut(), cp];
    unsafe {
        let m = HashMap::from_ptr(mp);
        m.set_buckets(buckets.as_mut_ptr());
        m.set_buckets_end(buckets.as_mut_ptr().wrapping_add(4));
    }
    STATE.with(|s| {
        *s.borrow_mut() = State {
            first: ap as usize,
            map: mp as usize,
            ..State::default()
        }
    });
    for (wanted, expected_count) in [(0, 0), (1, 1), (2, 2), (3, 3), (u32::MAX, 3)] {
        let mut out = [0xDEAD_BEEFu32; 5];
        let mut count = wanted;
        let result = unsafe { fc4p_00697b80(null_mut(), 0xABC0_1234, out.as_mut_ptr(), &mut count) };
        assert_eq!(count, expected_count);
        assert_eq!(result as u32, 3);
        assert_eq!(&out[..count as usize], &[1, 5, 7][..count as usize]);
        assert!(out[count as usize..].iter().all(|&v| v == 0xDEAD_BEEF));
        assert_eq!(
            (result >> 32) as u32,
            match count {
                0 => 0xABC0_1234,
                1 => 1,
                2 => 3,
                _ => 4,
            }
        );
        assert_eq!(
            events(),
            if count == 0 {
                vec![]
            } else {
                vec![Event::Begin(mp as usize)]
            }
        );
    }
    let mut count = u32::MAX;
    assert_eq!(
        unsafe { fc4p_00697b80(null_mut(), 0x1234_5678, null_mut(), &mut count) },
        0x1234_5678_0000_0003
    );
    assert_eq!(count, 3);
    assert!(events().is_empty());
    STATE.with(|s| s.borrow_mut().first = 0);
    assert_eq!(
        unsafe { fc4p_00697b80(null_mut(), 0, buckets.as_mut_ptr().cast(), &mut count) },
        0x1122_3344_0000_0003
    );
    events();
}

#[test]
fn family_vector_enumeration_resets_length_and_uses_growth_only_at_capacity() {
    let _begin = fake(0x004B_0390, begin as *const () as usize);
    let _insert = fake(0x0051_CA60, insert as *const () as usize);
    let mut map = [0u32; 4];
    let mp = map.as_mut_ptr().cast::<u8>();
    let _map = fake_data(0x00B4_8598, mp);
    let mut b = [0u32, 5];
    let bp = b.as_mut_ptr().cast::<u8>();
    let mut a = [bp as u32, 1];
    let ap = a.as_mut_ptr().cast::<u8>();
    let mut buckets = [null_mut(), ap];
    unsafe {
        let m = HashMap::from_ptr(mp);
        m.set_buckets(buckets.as_mut_ptr());
        m.set_buckets_end(buckets.as_mut_ptr().wrapping_add(2));
    }
    STATE.with(|s| {
        *s.borrow_mut() = State {
            first: ap as usize,
            map: mp as usize,
            ..State::default()
        }
    });
    let mut storage = [0xDEAD_BEEFu32; 4];
    let p = storage.as_mut_ptr();
    let mut vector = [p, p.wrapping_add(3), p.wrapping_add(4)];
    assert_eq!(
        unsafe { fc4p_00697d10(vector.as_mut_ptr().cast()) },
        0x0000_0002_0000_0002
    );
    assert_eq!(storage, [1, 5, 0xDEAD_BEEF, 0xDEAD_BEEF]);
    assert_eq!(vector[1], p.wrapping_add(2));
    events();
    let mut vector = [p, p.wrapping_add(3), p];
    unsafe { fc4p_00697d10(vector.as_mut_ptr().cast()) };
    assert_eq!(STATE.with(|s| core::mem::take(&mut s.borrow_mut().appended)), [1, 5]);
    assert_eq!(events().len(), 3);
    STATE.with(|s| s.borrow_mut().first = 0);
    assert_eq!(
        unsafe { fc4p_00697d10(vector.as_mut_ptr().cast()) },
        0x1122_3344_AABB_CCDD
    );
    assert_eq!(vector[1], p);
    events();
}

#[test]
fn family_expansion_vector_and_set_keep_distinct_clear_append_and_return_rules() {
    let _assign = fake(0x0046_2C10, assign as *const () as usize);
    let _insert = fake(0x0051_CA60, insert as *const () as usize);
    let _reserve = fake(0x004F_0940, reserve as *const () as usize);
    let _set = fake(0x0062_4DD0, set as *const () as usize);
    let mut members = [0x100u32, 0x200];
    let m = members.as_mut_ptr();
    let mut family = [0u32, 0x80, m as u32, m.wrapping_add(2) as u32];
    let fp = family.as_mut_ptr().cast::<u8>();
    let mut family_buckets = [fp];
    let mut fb = family_buckets.as_mut_ptr();
    let mut fe = fb.wrapping_add(1);
    let _families = data_map(0x00B4_859C, &mut fb, &mut fe);
    let mut building = [0u32, 0x100];
    let bp = building.as_mut_ptr().cast::<u8>();
    let mut building_buckets = [bp];
    let mut bb = building_buckets.as_mut_ptr();
    let mut be = bb.wrapping_add(1);
    let _buildings = data_map(0x00B4_854C, &mut bb, &mut be);
    let mut storage = [0xDEAD_BEEFu32; 4];
    let p = storage.as_mut_ptr();
    let mut vector = [p, p.wrapping_add(3), p.wrapping_add(4)];
    assert_eq!(
        unsafe { fc4p_00697ef0(vector.as_mut_ptr().cast(), 0x80) },
        0x1234_5678_0000_0002
    );
    assert_eq!(storage, [0x100, 0x200, 0xDEAD_BEEF, 0xDEAD_BEEF]);
    events();
    assert_eq!(
        unsafe { fc4p_00697ef0(vector.as_mut_ptr().cast(), 0x100) },
        u64::from(bp as u32) << 32
    );
    assert_eq!(vector[1], p.wrapping_add(1));
    assert!(events().is_empty());
    let before = vector;
    assert_eq!(unsafe { fc4p_00697ef0(vector.as_mut_ptr().cast(), 0x101) }, 0);
    assert_eq!(vector, before);
    vector[2] = p;
    assert_eq!(
        unsafe { fc4p_00697ef0(vector.as_mut_ptr().cast(), 0x100) },
        0x9876_5432_0000_0000
    );
    assert_eq!(STATE.with(|s| core::mem::take(&mut s.borrow_mut().appended)), [0x100]);
    events();
    let mut set = Object::new(20);
    let sp = set.ptr();
    unsafe { write(sp.add(16).cast::<u32>(), 3) };
    assert_eq!(unsafe { fc4p_00699d20(sp, 0x80) }, 0xAABB_CCDD_0000_0002);
    assert_eq!(
        events(),
        [
            Event::Reserve(sp as usize, 4),
            Event::Set(sp as usize, 0x100),
            Event::Reserve(sp as usize, 5),
            Event::Set(sp as usize, 0x200)
        ]
    );
    assert_eq!(unsafe { fc4p_00699d20(sp, 0x100) }, 0xAABB_CCDD_0000_0000);
    assert_eq!(
        events(),
        [Event::Reserve(sp as usize, 6), Event::Set(sp as usize, 0x100)]
    );
    assert_eq!(unsafe { fc4p_00699d20(sp, 0x101) }, 0);
    assert!(events().is_empty());
    unsafe { HashNode::from_ptr(fp) }.set_members_end(m);
    assert_eq!(unsafe { fc4p_00699d20(sp, 0x80) }, 0);
    assert!(events().is_empty());
    unsafe { HashNode::from_ptr(fp) }.set_members_end(m.wrapping_sub(1));
    assert_eq!(unsafe { fc4p_00699d20(sp, 0x80) }, u64::from(u32::MAX));
    assert!(events().is_empty());
}

#[test]
fn name_callbacks_pop_one_word_and_forward_both_return_registers() {
    let _a = fake(0x0048_62F0, erase as *const () as usize);
    let _b = fake(0x0069_B720, erase as *const () as usize);
    let mut names = 0u8;
    let mut families = 0u8;
    let _names = fake_data(0x00B4_855C, &mut names);
    let _families = fake_data(0x00B4_8584, &mut families);
    for (entry, map) in [
        (fc4p_0069ab00 as *const (), &mut names as *mut u8),
        (fc4p_0069ca60 as *const (), &mut families as *mut u8),
    ] {
        let args = [0x89AB_CDEF];
        let mut call = RawCall::new(Conv::Stdcall, entry as usize, 0xDEAD_BEEF, &args);
        call.edx = 0xA5A5_5A5A;
        unsafe { call_raw(&mut call) };
        assert_eq!((call.eax, call.edx), (0x1234_5678, 0xCAFE_BABE));
        assert_eq!(events(), [Event::Erase(map as usize, 0x89AB_CDEF)]);
    }
}

#[test]
fn candidate_predicate_uses_active_developer_preserves_coordinates_and_edx() {
    let _a = fake(0x0069_AB40, tried as *const () as usize);
    let _b = fake(0x0069_5F70, find as *const () as usize);
    let mut global = 0u8;
    let mut active = 0x1010u32;
    let mut tree = [0x1234_5678u32, 0];
    let _g = fake_data(0x00B4_85E0, &mut global);
    let _active = fake_data(0x00B4_8544, (&mut active as *mut u32).cast());
    STATE.with(|s| {
        s.borrow_mut().tree = tree.as_mut_ptr() as usize;
    });
    for (found, expected) in [(0x1234_5678, 1), (0, 0), (0x1234_5600, 0)] {
        STATE.with(|s| s.borrow_mut().found = found);
        let mut candidate = [0x8000_0000u32, 67, 0xDEAD_BEEF];
        assert_eq!(
            unsafe { fc4p_0069b070(candidate.as_mut_ptr()) },
            0x8877_6655_0000_0000 | expected
        );
        assert_eq!(candidate, [0x8000_0000, 67, 0xDEAD_BEEF]);
        assert_eq!(
            events(),
            [
                Event::Tried(&mut global as *mut u8 as usize, &mut active as *mut u32 as usize),
                Event::Find(tree.as_ptr() as usize, [0x8000_0000, 67])
            ]
        );
    }
}
