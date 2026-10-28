// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Monthly department strike transitions.

use super::collections::Node;
use super::service::Department;
use crate::port::Port;

layout! {
    /// The city interface queried only when the last building strike ends.
    struct MonthlySim {
        0x18 => city / set_city: *mut u8,
    }
}
layout! {
    /// The mutable array of circular building lists.
    struct Lists {
        0x00 => begin / set_begin: *mut *mut u8,
        0x04 => end / set_end: *mut *mut u8,
    }
}
game_fn! { fn update(this:*mut u8,building:*mut u8,department:*mut u8,staff:u32,grade:u32)->u32 = "thiscall" 0x006F_A280; }
game_fn! { fn begin_strike(this:*mut u8,department:*mut u8,building:*mut u8)->u32 = "thiscall" 0x006F_84C0; }
game_fn! { fn end_strike(this:*mut u8,department:*mut u8,duration:u32)->u32 = "thiscall" 0x006F_8570; }
game_fn! { fn read_uint(holder:*mut u8,id:u32,out:*mut u32)->u32 = "C" 0x005F_D480; }

fn list_count(lists: Lists) -> u32 {
    ((lists.end() as u32).wrapping_sub(lists.begin() as u32) as i32 >> 2) as u32
}
port! {
    /// Visit eligible buildings in list order. Once any transition occurs, every later striking
    /// building requests BeginStrike. End a department strike only after a change leaves none striking.
    #[port(address=0x006F_AFF0,owner="residential",name="cSC4ResidentialSimulator::DoMonthlyStrikeCheck")]
    pub unsafe extern "thiscall" fn fc4p_006faff0(this:*mut u8,department:*mut u8,lists:*mut u8,staff:u32,grade:u32,group:u32) {
        unsafe {
            let lists=Lists::from_ptr(lists);
            let mut changed=false;
            let mut striking=0u32;
            let mut index=0u32;
            if list_count(lists)==0 {return;}
            loop {
                let head=lists.begin().wrapping_add(index as usize).read_unaligned();
                let mut node=Node::from_ptr(head).next();
                while node!=head {
                    let building=Node::from_ptr(node).value();
                    if vcall!(building,0x58 => fn(u32)->u32,group) as u8!=0 {
                        if update(this,building,department,staff,grade) as u8!=0 {changed=true;}
                        let mut day=0u32;
                        let holder=vcall!(building,0x18 => fn()->*mut u8);
                        if read_uint(holder,0xEAA6_9F83,&mut day) as u8!=0 && day!=0 {
                            striking=striking.wrapping_add(1);
                            if changed {begin_strike(this,department,building);}
                        }
                    }
                    node=Node::from_ptr(node).next();
                }
                index=index.wrapping_add(1);
                if index>=list_count(lists) {break;}
            }
            let department_view=Department::from_ptr(department);
            if changed && department_view.strike()!=0 && striking==0 {
                let clock=vcall!(MonthlySim::from_ptr(this).city(),0xC8 => fn()->*mut u8);
                let clock=vcall!(clock,0x1C => fn()->*mut u8);
                let today=vcall!(clock,0x68 => fn()->u32);
                end_strike(this,department,today.wrapping_sub(department_view.strike_day()));
            }
        }
    }
}
pub const PORTS: &[Port] = &[fc4p_006faff0::PORT];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{fake, Object};
    use std::cell::RefCell;

    #[derive(Debug, PartialEq, Eq)]
    enum Call {
        Group(u32),
        Update(u32, u32, u32),
        Holder(u32),
        Day(u32),
        Begin(u32),
        City,
        Clock,
        Today,
        End(u32),
    }
    thread_local! {static CALLS:RefCell<Vec<Call>>=const{RefCell::new(Vec::new())};}
    thread_local! {static GROW_LISTS:RefCell<usize>=const{RefCell::new(0)};}
    fn record(c: Call) {
        CALLS.with(|v| v.borrow_mut().push(c));
    }
    fn take() -> Vec<Call> {
        CALLS.with(|v| core::mem::take(&mut *v.borrow_mut()))
    }
    #[repr(C)]
    struct Building {
        table: *const usize,
        id: u32,
        eligible: u32,
        changed: u32,
        day: u32,
        present: u32,
    }
    extern "thiscall" fn group(this: *mut u8, g: u32) -> u32 {
        assert_eq!(g, 0x1510);
        let b = unsafe { &*this.cast::<Building>() };
        record(Call::Group(b.id));
        b.eligible
    }
    extern "thiscall" fn holder(this: *mut u8) -> *mut u8 {
        record(Call::Holder(unsafe { &*this.cast::<Building>() }.id));
        this
    }
    extern "thiscall" fn update(_this: *mut u8, b: *mut u8, _d: *mut u8, staff: u32, grade: u32) -> u32 {
        let b = unsafe { &*b.cast::<Building>() };
        record(Call::Update(b.id, staff, grade));
        GROW_LISTS.with(|g| {
            let p = *g.borrow();
            if p != 0 {
                unsafe {
                    let lists = Lists::from_ptr(p as *mut u8);
                    lists.set_end(lists.begin().wrapping_add(2));
                }
                *g.borrow_mut() = 0;
            }
        });
        b.changed
    }
    extern "C" fn read(b: *mut u8, id: u32, out: *mut u32) -> u32 {
        assert_eq!(id, 0xEAA6_9F83);
        let b = unsafe { &*b.cast::<Building>() };
        record(Call::Day(b.id));
        assert_eq!(unsafe { *out }, 0);
        unsafe { *out = b.day };
        b.present
    }
    extern "thiscall" fn begin(_this: *mut u8, _d: *mut u8, b: *mut u8) -> u32 {
        record(Call::Begin(unsafe { &*b.cast::<Building>() }.id));
        0
    }
    extern "thiscall" fn end(_this: *mut u8, _d: *mut u8, duration: u32) -> u32 {
        record(Call::End(duration));
        0
    }
    extern "thiscall" fn city(this: *mut u8) -> *mut u8 {
        record(Call::City);
        this
    }
    extern "thiscall" fn clock(this: *mut u8) -> *mut u8 {
        record(Call::Clock);
        this
    }
    extern "thiscall" fn today(_this: *mut u8) -> u32 {
        record(Call::Today);
        5
    }

    fn run(values: &[(u32, u32, u32, u32)], active: u8) -> Vec<Call> {
        run_with_growth(values, active, false)
    }
    fn run_with_growth(values: &[(u32, u32, u32, u32)], active: u8, grow: bool) -> Vec<Call> {
        let guards = [
            fake(0x006F_A280, update as *const () as usize),
            fake(0x005F_D480, read as *const () as usize),
            fake(0x006F_84C0, begin as *const () as usize),
            fake(0x006F_8570, end as *const () as usize),
        ];
        let mut table = [0usize; 51];
        table[22] = group as *const () as usize;
        table[6] = holder as *const () as usize;
        table[50] = city as *const () as usize;
        table[7] = clock as *const () as usize;
        table[26] = today as *const () as usize;
        let mut city_object = [table.as_ptr() as usize];
        let mut sim = Object::new(0x40);
        let mut department = Object::new(0x60);
        unsafe {
            MonthlySim::from_ptr(sim.ptr()).set_city(city_object.as_mut_ptr().cast());
            let d = Department::from_ptr(department.ptr());
            d.set_strike(active);
            d.set_strike_day(u32::MAX - 4)
        };
        let mut buildings: Vec<_> = values
            .iter()
            .enumerate()
            .map(|(i, &(eligible, changed, day, present))| Building {
                table: table.as_ptr(),
                id: i as u32,
                eligible,
                changed,
                day,
                present,
            })
            .collect();
        let mut head = [0usize; 3];
        let hp = head.as_mut_ptr().cast::<u8>();
        let mut nodes = vec![[0usize; 3]; buildings.len()];
        let mut previous = hp;
        for (b, n) in buildings.iter_mut().zip(nodes.iter_mut()) {
            unsafe {
                let p = n.as_mut_ptr().cast::<u8>();
                Node::from_ptr(previous).set_next(p);
                let n = Node::from_ptr(p);
                n.set_previous(previous);
                n.set_value((b as *mut Building).cast());
                previous = p;
            }
        }
        unsafe {
            Node::from_ptr(previous).set_next(hp);
            Node::from_ptr(hp).set_previous(previous)
        };
        let mut array = [hp, hp];
        let mut lists = [
            array.as_mut_ptr(),
            array.as_mut_ptr().wrapping_add(usize::from(!values.is_empty())),
        ];
        if grow {
            GROW_LISTS.with(|g| *g.borrow_mut() = lists.as_mut_ptr() as usize);
        }
        unsafe { fc4p_006faff0(sim.ptr(), department.ptr(), lists.as_mut_ptr().cast(), 11, 22, 0x1510) };
        drop(guards);
        take()
    }

    #[test]
    fn changed_flag_stays_set_for_later_striking_buildings() {
        assert_eq!(
            run(&[(1, 0, 100, 1), (0, 1, 100, 1), (1, 1, 100, 1), (1, 0, 100, 1)], 1),
            vec![
                Call::Group(0),
                Call::Update(0, 11, 22),
                Call::Holder(0),
                Call::Day(0),
                Call::Group(1),
                Call::Group(2),
                Call::Update(2, 11, 22),
                Call::Holder(2),
                Call::Day(2),
                Call::Begin(2),
                Call::Group(3),
                Call::Update(3, 11, 22),
                Call::Holder(3),
                Call::Day(3),
                Call::Begin(3)
            ]
        );
    }
    #[test]
    fn last_strike_ends_with_wrapping_day_difference() {
        assert_eq!(
            run(&[(1, 1, 0, 1), (1, 0, 50, 0)], 1),
            vec![
                Call::Group(0),
                Call::Update(0, 11, 22),
                Call::Holder(0),
                Call::Day(0),
                Call::Group(1),
                Call::Update(1, 11, 22),
                Call::Holder(1),
                Call::Day(1),
                Call::City,
                Call::Clock,
                Call::Today,
                Call::End(10)
            ]
        );
    }
    #[test]
    fn no_transition_or_inactive_department_skips_clock() {
        for (changed, active) in [(0, 1), (1, 0)] {
            assert_eq!(
                run(&[(1, changed, 0, 1)], active),
                vec![Call::Group(0), Call::Update(0, 11, 22), Call::Holder(0), Call::Day(0)]
            );
        }
        assert!(run(&[], 1).is_empty());
    }

    #[test]
    fn transitions_reload_the_array_end_before_visiting_the_next_list() {
        assert_eq!(
            run_with_growth(&[(1, 1, 50, 1)], 1, true),
            vec![
                Call::Group(0),
                Call::Update(0, 11, 22),
                Call::Holder(0),
                Call::Day(0),
                Call::Begin(0),
                Call::Group(0),
                Call::Update(0, 11, 22),
                Call::Holder(0),
                Call::Day(0),
                Call::Begin(0)
            ]
        );
    }
}
