// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::lot_developer::daily::*;

slot!(daily_count);
slot!(daily_iterator);
slot!(daily_date);

unsafe extern "thiscall" fn daily_start(this: *mut u32, milliseconds: u32) {
    run(core::ptr::null_mut(), "daily_start", &[this as u32, milliseconds]);
}
unsafe extern "thiscall" fn daily_expired(this: *mut u32) -> u32 {
    run(core::ptr::null_mut(), "daily_expired", &[this as u32]) as u32
}
unsafe extern "thiscall" fn daily_finish(this: *mut u32) -> u32 {
    run(core::ptr::null_mut(), "daily_finish", &[this as u32]) as u32
}
unsafe extern "thiscall" fn daily_update(this: *mut u8, lot: u32) {
    let iterator = unsafe { LotIterator::from_ptr(DailyState::from_ptr(this).iterator()) };
    run(this, "daily_update", &[lot, iterator.current() as u32]);
}

struct World {
    developer: Object,
    manager: Mock,
    date: Mock,
    iterator: [u32; 2],
    head: Box<[u32; 3]>,
    nodes: Vec<[u32; 3]>,
    list: Box<u32>,
    step: Box<u32>,
    feedback: Box<u32>,
    initialized: Box<u32>,
    pending: Box<u8>,
    date_pointer: Box<*mut u8>,
    _fakes: Vec<crate::testing::Fake>,
}

impl World {
    fn new(lots: usize, mode: u32, step: u32) -> Self {
        let mut world = Self {
            developer: Object::new(0x108),
            manager: mock!(0x3C => daily_iterator, 0x40 => daily_count),
            date: mock!(0x20 => daily_date),
            iterator: [0; 2],
            head: Box::new([0; 3]),
            nodes: vec![[0; 3]; lots],
            list: Box::new(0),
            step: Box::new(step),
            feedback: Box::new(37),
            initialized: Box::new(0x81),
            pending: Box::new(0xFF),
            date_pointer: Box::new(core::ptr::null_mut()),
            _fakes: [
                (0x0068_BF50, daily_start as *const () as usize),
                (0x0068_BF80, daily_expired as *const () as usize),
                (0x0068_B960, daily_finish as *const () as usize),
                (0x006C_D3A0, daily_update as *const () as usize),
            ]
            .into_iter()
            .map(|(a, f)| fake(a, f))
            .collect(),
        };
        let head = world.head.as_mut_ptr() as u32;
        *world.list = head;
        world.head[0] = if lots == 0 {
            head
        } else {
            world.nodes.as_mut_ptr() as u32
        };
        for i in 0..lots {
            world.nodes[i][0] = if i + 1 == lots {
                head
            } else {
                (unsafe { world.nodes.as_mut_ptr().add(i + 1) }) as u32
            };
            world.nodes[i][2] = 0x1000 + i as u32;
        }
        world.iterator = [world.head[0], &mut *world.list as *mut u32 as u32];
        *world.date_pointer = world.date.ptr();
        world._fakes.extend([
            fake_data(0x00B4_9B08, (&mut *world.step as *mut u32).cast()),
            fake_data(0x00B4_9B0C, (&mut *world.feedback as *mut u32).cast()),
            fake_data(0x00B4_9B10, (&mut *world.initialized as *mut u32).cast()),
            fake_data(0x00B4_9AE9, &mut *world.pending),
            fake_data(0x00B4_3D5C, (&mut *world.date_pointer as *mut *mut u8).cast()),
        ]);
        unsafe {
            LotDeveloper::from_ptr(world.developer.ptr()).set_lot_manager(world.manager.ptr());
            DailyState::from_ptr(world.developer.ptr()).set_state(mode);
        }
        world
    }

    fn state(&mut self) -> DailyState {
        unsafe { DailyState::from_ptr(self.developer.ptr()) }
    }
    fn invoke(&mut self) -> u32 {
        // The iterator array can move with the fixture, so publish its address immediately before entry.
        self.state().set_iterator(self.iterator.as_mut_ptr().cast());
        unsafe { fc4p_006cd770(self.developer.ptr()) }
    }
    fn start(&mut self, count: i32) {
        expect(core::ptr::null_mut(), "daily_start", vec![out(&[0x11, 0x22]), w(5)], 0);
        expect(self.manager.ptr(), "daily_count", vec![], count as u32 as u64);
    }
    fn timer(&mut self, accumulated: u32, expired: u32) -> Arg {
        Arg::In(vec![
            0x11,
            0x22,
            accumulated,
            *self.step,
            expired,
            &mut *self.step as *mut u32 as u32,
        ])
    }
    fn check(&mut self, accumulated: u32, answer: u32) {
        let timer = self.timer(accumulated, 0);
        expect(core::ptr::null_mut(), "daily_expired", vec![timer], u64::from(answer));
    }
    fn finish(&mut self, accumulated: u32, expired: u32) {
        let timer = self.timer(accumulated, expired);
        expect(core::ptr::null_mut(), "daily_finish", vec![timer], RETURN);
    }
    fn update(&mut self, index: usize) {
        expect(
            self.developer.ptr(),
            "daily_update",
            vec![w(0x1000 + index as u32), w(self.nodes[index][0])],
            0,
        );
    }
}

#[test]
fn daily_quota_and_consecutive_days_match_the_core_cases() {
    for (count, quota) in [(100, 4), (27, 1), (28, 2), (56, 3)] {
        let mut world = World::new(count as usize, 1, 1024);
        for day in 0..2 {
            world.start(count);
            for i in 0..quota {
                world.check((i + 1) * 1024, 0x100);
                world.update((day * quota + i) as usize);
            }
            world.finish(quota * 1024, 0);
            assert_eq!(world.invoke(), (RETURN as u32 & !255) | 1);
            assert_eq!(world.state().state(), 1);
            assert_eq!(world.state().visited(), 0);
            finished();
        }
    }
}

#[test]
fn daily_timeout_resumes_existing_work_and_checks_only_on_counter_boundaries() {
    let mut world = World::new(30, 1, 512);
    world.start(30);
    world.update(0);
    world.check(1024, 0xABCD_0001);
    world.finish(1024, 1);
    assert_eq!(world.invoke(), RETURN as u32 & !255);
    assert_eq!(world.state().visited(), 1);
    assert_eq!(world.state().state(), 1);
    assert_eq!(*world.pending, 0xFF);
    finished();
    world.start(30);
    world.update(1);
    world.finish(512, 0);
    assert_eq!(world.invoke(), (RETURN as u32 & !255) | 1);
    assert_eq!(world.state().visited(), 0);
    finished();
}

#[test]
fn daily_end_of_list_clears_rotation_and_the_next_cycle_rewinds_and_takes_latches() {
    let mut world = World::new(1, 1, 1024);
    world.start(28);
    world.check(1024, 0);
    world.update(0);
    world.check(2048, 0);
    world.finish(2048, 0);
    assert_eq!(world.invoke(), RETURN as u32 & !255);
    assert_eq!(world.state().state(), 3);
    assert_eq!(world.state().visited(), 1);
    assert_eq!(*world.pending, 0);
    finished();
    world.state().set_roads_dirty(0x81);
    world.state().set_no_zot_until(100);
    unsafe {
        LotDeveloper::from_ptr(world.developer.ptr()).set_business_units_dirty(0x42);
    }
    expect(world.manager.ptr(), "daily_count", vec![], 28);
    expect(world.date.ptr(), "daily_date", vec![], 101);
    world.start(28);
    world.check(1024, 0);
    world.update(0);
    world.check(2048, 0);
    world.finish(2048, 0);
    assert_eq!(world.invoke(), RETURN as u32 & !255);
    assert_eq!(world.state().roads_changed(), 0x81);
    assert_eq!(world.state().units_changed(), 0x42);
    assert_eq!(world.state().roads_dirty(), 0);
    assert_eq!(
        unsafe { LotDeveloper::from_ptr(world.developer.ptr()) }.business_units_dirty(),
        0
    );
    assert_eq!(world.state().can_check_commute(), 1);
    finished();
}

#[test]
fn daily_initialization_allocates_iterator_initializes_budget_once_and_observes_unsigned_day() {
    for (day, allowed) in [(0, 0), (100, 0), (101, 1), (u32::MAX, 1)] {
        let mut world = World::new(1, 0, 1024);
        *world.initialized = 0x80;
        world.state().set_iterator(core::ptr::null_mut());
        world.state().set_no_zot_until(100);
        expect(
            world.manager.ptr(),
            "daily_iterator",
            vec![],
            world.iterator.as_mut_ptr() as u64,
        );
        expect(world.manager.ptr(), "daily_count", vec![], 1);
        expect(world.date.ptr(), "daily_date", vec![], u64::from(day));
        world.start(1);
        world.check(1024, 0);
        world.update(0);
        world.finish(1024, 0);
        let returned = unsafe { fc4p_006cd770(world.developer.ptr()) };
        assert_eq!(returned, (RETURN as u32 & !255) | 1);
        assert_eq!((*world.initialized, *world.step, *world.feedback), (0x81, 1024, 0));
        assert_eq!(world.state().can_check_commute(), allowed);
        finished();
    }
}

#[test]
fn daily_early_returns_empty_city_and_already_fulfilled_quota() {
    for mode in [2u32, 4, 0x103, u32::MAX] {
        let mut world = World::new(0, mode, 1024);
        assert_eq!(world.invoke(), (mode.wrapping_sub(3) & !255) | 1);
        assert_eq!(world.state().state(), mode);
        finished();
    }
    for count in [0i32, -1, i32::MIN] {
        let mut world = World::new(0, 3, 1024);
        expect(world.manager.ptr(), "daily_count", vec![], count as u32 as u64);
        assert_eq!(world.invoke(), (count as u32 & !255) | 1);
        assert_eq!(world.state().state(), 3);
        finished();
        world.state().set_state(1);
        world.start(count);
        world.finish(0, 0);
        assert_eq!(world.invoke(), (RETURN as u32 & !255) | 1);
        assert_eq!(world.state().state(), 3);
        finished();
    }
    let mut world = World::new(1, 1, 1024);
    world.state().set_visited(2);
    world.start(28);
    world.finish(0, 0);
    assert_eq!(world.invoke(), (RETURN as u32 & !255) | 1);
    assert_eq!(world.state().visited(), 0);
    finished();
}

#[test]
fn daily_null_owner_ends_cycle_and_update_callbacks_can_change_the_work_counter() {
    let mut world = World::new(1, 1, 0);
    world.iterator[1] = 0;
    world.start(28);
    world.check(0, 0);
    world.finish(0, 0);
    assert_eq!(world.invoke(), RETURN as u32 & !255);
    assert_eq!(world.state().state(), 3);
    finished();
    let mut world = World::new(1, 1, 0xFFFF_FE00);
    world.start(28);
    world.update(0);
    edit_last(unsafe { world.developer.ptr().add(0xB4).cast() }, 10);
    world.finish(0xFFFF_FE00, 0);
    assert_eq!(world.invoke(), (RETURN as u32 & !255) | 1);
    assert_eq!(world.state().visited(), 0);
    finished();
}
