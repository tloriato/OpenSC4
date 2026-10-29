// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Deal life cycles, ordered garbage capacity, callbacks and preserved machine state.

use super::*;
use crate::testing::{call_thiscall_with_eax, fake, fake_data, Fake, GameFpu, Object};
use std::cell::RefCell;

layout! {
    /// Fields of the game's deal used by the test interfaces.
    struct Deal {
        0x20 => kind / set_kind: u32,
        0x34 => amount / set_amount: i32,
        0x50 => cost / set_cost: i64,
        0x58 => start / set_start: u32,
        0x60 => expiration / set_expiration: u32,
        0x68 => current / set_current: u32,
        0x70 => suspension / set_suspension: u32,
        0x78 => flags / set_flags: u32,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Event {
    Slot(usize, u32, Vec<u32>),
    Clear,
    Property(u32),
    NewDate(u32, u32, u32),
    Destroy(u32),
    AddDays(u32),
    Compare(u32, u32),
    Report(usize, u32),
    Allocate,
    Append(usize),
    Erase(usize),
    Free(u32),
    Publish(u32, u32),
    Post(Vec<u32>),
}

struct State {
    pointers: Vec<u32>,
    deals: Vec<u32>,
    events: Vec<Event>,
    // Returned allocation addresses must survive later growth of this collection.
    #[allow(clippy::vec_box)]
    allocations: Vec<Box<[u32; 3]>>,
    date_table: u32,
    day: u32,
    term: Option<f32>,
    grace: Option<f32>,
    probation: Option<f32>,
    landfill: [u32; 2],
    figures: [f64; 3],
    budgets: [f64; 2],
    failures: u32,
    activate: bool,
    reenter: bool,
    silent_checks: u32,
    clock_swap: Option<(u32, u32)>,
    null_date: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            pointers: Vec::new(),
            deals: Vec::new(),
            events: Vec::new(),
            allocations: Vec::new(),
            date_table: 0,
            day: 1000,
            term: Some(1.0),
            grace: Some(90.0),
            probation: Some(90.0),
            landfill: [0, 0],
            figures: [0.0, 50.0, 0.0],
            budgets: [1.0e9; 2],
            failures: 0,
            activate: true,
            reenter: false,
            silent_checks: 0,
            clock_swap: None,
            null_date: false,
        }
    }
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }

fn record(event: Event) {
    STATE.with(|s| s.borrow_mut().events.push(event));
}
fn object_id(this: *mut u8) -> usize {
    STATE.with(|s| {
        let s = s.borrow();
        if let Some(i) = s.pointers.iter().position(|&p| p == this as u32) {
            i
        } else {
            10 + s.deals.iter().position(|&p| p == this as u32).expect("known object")
        }
    })
}
fn answer(eax: u32) -> u64 {
    pair(eax, 0xDCBA_9876)
}
fn events() -> Vec<Event> {
    STATE.with(|s| s.borrow().events.clone())
}

unsafe fn dispatch(this: *mut u8, slot: u32, args: &[u32]) -> u64 {
    let id = object_id(this);
    record(Event::Slot(id, slot, args.to_vec()));
    if id >= 10 {
        // SAFETY: test deals are 0x80-byte objects with the original field offsets.
        let deal = unsafe { Deal::from_ptr(this) };
        let eax = match slot {
            0x1C => STATE.with(|s| s.borrow().pointers[6]),
            0x20 => deal.kind(),
            0x28 => deal.amount() as u32,
            0x2C => {
                deal.set_amount(args[0] as i32);
                1
            }
            0x58 => return deal.cost() as u64,
            0x5C => 0x1234_5601,
            0x60 | 0x68 | 0x78 => {
                let day = match slot {
                    0x60 => deal.start(),
                    0x68 => deal.expiration(),
                    _ => deal.suspension(),
                };
                unsafe { (args[0] as *mut u32).sub(1).write_unaligned(day) };
                1
            }
            0x64 | 0x6C | 0x74 => {
                let day = if args[0] == 0 {
                    0
                } else {
                    unsafe { (args[0] as *const u32).sub(1).read_unaligned() }
                };
                match slot {
                    0x64 => deal.set_start(day),
                    0x6C => deal.set_expiration(day),
                    _ => deal.set_current(day),
                }
                1
            }
            0x7C => {
                deal.set_suspension(unsafe { (args[0] as *const u32).sub(1).read_unaligned() });
                1
            }
            0x84 => {
                deal.set_flags(deal.flags() | STATE.with(|s| s.borrow().failures));
                0x1234_5601
            }
            0x88 => {
                deal.set_flags(deal.flags() | 4);
                0x1234_5601
            }
            0x8C => {
                if STATE.with(|s| s.borrow().activate) {
                    deal.set_flags((deal.flags() | 2) & !5);
                    0x1234_5601
                } else {
                    0x1234_5600
                }
            }
            0x94 => {
                let mut mask = args[0];
                if args[1] != 0 {
                    if deal.flags() & 0x100 != 0 {
                        mask &= !0x38;
                    }
                    deal.set_flags(deal.flags() | mask);
                } else {
                    deal.set_flags(deal.flags() & !mask);
                }
                0x1234_5601
            }
            0x98 | 0x9C => {
                if args[0] == 0x80 && deal.flags() & 0x80 != 0 {
                    STATE.with(|s| {
                        let mut s = s.borrow_mut();
                        s.silent_checks += 1;
                        if s.silent_checks == 2 {
                            deal.set_flags(deal.flags() & !0x80);
                        }
                        assert!(s.silent_checks <= 2, "silent cursor must be rechecked");
                    });
                }
                0x1234_5600 | u32::from(deal.flags() & args[0] != 0)
            }
            _ => panic!("unexpected deal slot {slot:x}"),
        };
        return answer(eax);
    }
    let eax = match (id, slot) {
        (0, 0x88) => {
            assert_eq!(args, &[STATE.with(|s| s.borrow().pointers[7])]);
            assert_eq!(unsafe { NeighborsSim::from_ptr(this) }.busy(), 1);
            0xAABB_CC55
        }
        (0, 0x94) => STATE.with(|s| s.borrow().pointers[1]),
        (1, 0x18) => STATE.with(|s| s.borrow().pointers[2]),
        (3, 0x28) => {
            let values = STATE.with(|s| s.borrow().landfill);
            unsafe {
                (args[0] as *mut u32).write_unaligned(values[0]);
                (args[1] as *mut u32).write_unaligned(values[1]);
            }
            0x1234_5601
        }
        (4, 0x90) => STATE.with(|s| s.borrow().pointers[5]),
        (8, 0x1C) => {
            if let Some((object, table)) = STATE.with(|s| s.borrow_mut().clock_swap.take()) {
                unsafe { (object as *mut u32).write_unaligned(table) };
            }
            STATE.with(|s| {
                let s = s.borrow();
                unsafe { ((s.pointers[9] + 4) as *mut u32).write_unaligned(s.day) };
                s.pointers[9] + 8
            })
        }
        _ => panic!("unexpected object {id} slot {slot:x}"),
    };
    answer(eax)
}

macro_rules! slot0 {
    ($name:ident, $slot:literal) => {
        unsafe extern "thiscall" fn $name(this: *mut u8) -> u64 {
            unsafe { dispatch(this, $slot, &[]) }
        }
    };
}
macro_rules! slot1 {
    ($name:ident, $slot:literal) => {
        unsafe extern "thiscall" fn $name(this: *mut u8, a: u32) -> u64 {
            unsafe { dispatch(this, $slot, &[a]) }
        }
    };
}
slot0!(s18, 0x18);
slot0!(s1c, 0x1C);
slot0!(s20, 0x20);
slot0!(s28, 0x28);
slot0!(s58, 0x58);
slot0!(s5c, 0x5C);
slot0!(s84, 0x84);
slot0!(s88, 0x88);
slot0!(s8c, 0x8C);
slot0!(s90, 0x90);
slot0!(s94, 0x94);
slot1!(s2c, 0x2C);
slot1!(s60, 0x60);
slot1!(s64, 0x64);
slot1!(s68, 0x68);
slot1!(s6c, 0x6C);
slot1!(s74, 0x74);
slot1!(s78, 0x78);
slot1!(s7c, 0x7C);
slot1!(s98, 0x98);
slot1!(s9c, 0x9C);
slot1!(stats, 0x88);
unsafe extern "thiscall" fn flags(this: *mut u8, a: u32, b: u32) -> u64 {
    unsafe { dispatch(this, 0x94, &[a, b]) }
}
unsafe extern "thiscall" fn land(this: *mut u8, a: u32, b: u32) -> u64 {
    unsafe { dispatch(this, 0x28, &[a, b]) }
}

unsafe extern "thiscall" fn exported(this: *mut u8, commodity: u32) -> f64 {
    record(Event::Slot(object_id(this), 0xD8, vec![commodity]));
    STATE.with(|s| s.borrow().figures[0])
}
unsafe extern "thiscall" fn produced(this: *mut u8, commodity: u32) -> f64 {
    record(Event::Slot(object_id(this), 0xE8, vec![commodity]));
    STATE.with(|s| s.borrow().figures[1])
}
unsafe extern "thiscall" fn imported(this: *mut u8, commodity: u32) -> f64 {
    record(Event::Slot(object_id(this), 0xE0, vec![commodity]));
    STATE.with(|s| s.borrow().figures[2])
}
unsafe extern "thiscall" fn budget(this: *mut u8) -> f64 {
    let id = object_id(this);
    record(Event::Slot(id, 0xC8, vec![]));
    STATE.with(|s| s.borrow().budgets[usize::from(id == 6)])
}
unsafe extern "C" fn property(holder: *mut u8, id: u32, out: *mut f32) -> u64 {
    assert_eq!(object_id(holder), 2);
    record(Event::Property(id));
    let value = STATE.with(|s| {
        let s = s.borrow();
        match id {
            0xEA46_934F => s.term,
            0x6B58_8FAD => s.grace,
            0x8B58_C59E => s.probation,
            _ => panic!("property"),
        }
    });
    if let Some(value) = value {
        unsafe { out.write_unaligned(value) };
    }
    answer(u32::from(value.is_some()))
}
unsafe extern "C" fn allocate_node(size: u32) -> u64 {
    assert_eq!(size, 12);
    record(Event::Allocate);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let mut node = Box::new([0xCCCC_CCCC; 3]);
        let pointer = node.as_mut_ptr() as u32;
        s.allocations.push(node);
        answer(pointer)
    })
}
unsafe extern "C" fn free_node(node: *mut u8) -> u64 {
    record(Event::Free(node as u32));
    answer(1)
}
unsafe extern "thiscall" fn clear(this: *mut u8) -> u64 {
    record(Event::Clear);
    let header = unsafe { pointer(this, 0) };
    unsafe {
        let mut node = pointer(header, 0);
        while node != header {
            let next = pointer(node, 0);
            free_node(node);
            node = next;
        }
        Node::from_ptr(header).set_next(header);
        Node::from_ptr(header).set_previous(header);
    }
    answer(header as u32)
}
unsafe extern "thiscall" fn append(this: *mut u8, cursor: *mut *mut u8) -> u64 {
    let selected = unsafe { cursor.read_unaligned() };
    record(Event::Append(object_id(unsafe { pointer(selected, 8) })));
    let node = unsafe { allocate_node(12) as u32 as *mut u8 };
    unsafe {
        let header = Node::from_ptr(pointer(this, 0));
        let previous = header.previous();
        let node = Node::from_ptr(node);
        node.set_value(selected);
        node.set_next(header.as_ptr());
        node.set_previous(previous);
        Node::from_ptr(previous).set_next(node.as_ptr());
        header.set_previous(node.as_ptr());
    }
    answer(node as u32)
}
unsafe extern "thiscall" fn erase(this: *mut u8, out: *mut u32, node: *mut u8) -> u64 {
    assert_eq!(this as u32, STATE.with(|s| s.borrow().pointers[0]) + 0x20);
    record(Event::Erase(object_id(unsafe { pointer(node, 8) })));
    unsafe {
        let node = Node::from_ptr(node);
        Node::from_ptr(node.previous()).set_next(node.next());
        Node::from_ptr(node.next()).set_previous(node.previous());
        out.write_unaligned(node.next() as u32);
    }
    answer(out as u32)
}
unsafe extern "fastcall" fn publish(this: *mut u8, _edx: u32, deals: u32, stats: u32) -> u64 {
    record(Event::Publish(deals, stats));
    let sim = unsafe { NeighborsSim::from_ptr(this) };
    assert_eq!(sim.busy(), 1);
    sim.set_deals_dirty(0);
    sim.set_stats_dirty(0);
    pair(0xBEEF_ABCD, 0x8765_4321)
}
unsafe extern "C" fn post(a: u32, b: u32, c: u32, d: u32, e: u32, f: u32) -> u64 {
    record(Event::Post(vec![a, b, c, d, e, f]));
    answer(0x9988_7766)
}

#[unsafe(naked)]
unsafe extern "C" fn report_fake() {
    core::arch::naked_asm!(
        "push dword ptr [esp + 8]", "push dword ptr [esp + 8]", "push ecx",
        "call {}", "add esp, 12", "ret", sym report_impl,
    );
}
unsafe extern "C" fn report_impl(this: *mut u8, send: u32, status: u32) -> u64 {
    assert_eq!(send, 1);
    record(Event::Report(object_id(this), status));
    let reenter = STATE.with(|s| {
        let mut s = s.borrow_mut();
        let yes = s.reenter;
        s.reenter = false;
        yes
    });
    if reenter {
        let sim = STATE.with(|s| s.borrow().pointers[0]) as *mut u8;
        unsafe {
            call_thiscall_with_eax(fc4p_006d0e50 as *const () as usize, sim, &[], 0x1234_5678);
        }
    }
    answer(0x1234_5601)
}

unsafe fn init_date(this: *mut u8, kind: u32, day: u32) -> u64 {
    record(Event::NewDate(this as u32, kind, day));
    let table = STATE.with(|s| s.borrow().date_table);
    unsafe {
        this.cast::<u32>().write_unaligned(table);
        this.add(4).cast::<u32>().write_unaligned(day);
        this.add(8).cast::<u32>().write_unaligned(table);
        this.add(12).cast::<u32>().write_unaligned(0);
    }
    answer(this as u32)
}
unsafe extern "thiscall" fn date_new(this: *mut u8) -> u64 {
    unsafe { init_date(this, 0, 0) }
}
unsafe extern "thiscall" fn date_time(this: *mut u8, date: *mut u8) -> u64 {
    unsafe { init_date(this, 1, word(date.sub(8), 4)) }
}
unsafe extern "thiscall" fn date_day(this: *mut u8, day: u32) -> u64 {
    let result = unsafe { init_date(this, 2, day) };
    if STATE.with(|s| s.borrow().null_date) {
        answer(0)
    } else {
        result
    }
}
unsafe extern "thiscall" fn day_number(this: *mut u8) -> u64 {
    answer(unsafe { this.sub(4).cast::<u32>().read_unaligned() })
}
unsafe extern "thiscall" fn add_days(this: *mut u8, days: u32) -> u64 {
    record(Event::AddDays(days));
    unsafe {
        let day = this.sub(4).cast::<u32>();
        day.write_unaligned(day.read_unaligned().wrapping_add(days));
    }
    answer(1)
}
unsafe extern "C" fn compare(today: *mut u8, date: *mut u8) -> u64 {
    let a = unsafe { word(today, 4) };
    let b = unsafe { word(date, 4) };
    record(Event::Compare(a, b));
    answer(u32::from(a >= b))
}
#[unsafe(naked)]
unsafe extern "C" fn destroy() {
    core::arch::naked_asm!(
        "push eax", "push edx", "push ecx", "call {}", "add esp, 4", "pop edx", "pop eax", "ret", sym destroy_impl,
    );
}
extern "C" fn destroy_impl(this: *mut u8) {
    record(Event::Destroy(this as u32));
}

struct World {
    objects: Vec<Object>,
    deals: Vec<Object>,
    nodes: Vec<[u32; 3]>,
    header: Box<[u32; 3]>,
    pending: Box<[u32; 3]>,
    _tables: Vec<Box<[usize]>>,
    _globals: Vec<u32>,
    _fakes: Vec<Fake>,
    _fpu: GameFpu,
}

impl World {
    fn new(flags: &[u32]) -> Self {
        STATE.with(|s| *s.borrow_mut() = State::default());
        let fpu = GameFpu::new();
        let mut table = vec![0usize; 0x120 / 4].into_boxed_slice();
        for (slot, f) in [
            (0x18, s18 as *const () as usize),
            (0x1C, s1c as *const () as usize),
            (0x20, s20 as *const () as usize),
            (0x28, s28 as *const () as usize),
            (0x2C, s2c as *const () as usize),
            (0x58, s58 as *const () as usize),
            (0x5C, s5c as *const () as usize),
            (0x60, s60 as *const () as usize),
            (0x64, s64 as *const () as usize),
            (0x68, s68 as *const () as usize),
            (0x6C, s6c as *const () as usize),
            (0x74, s74 as *const () as usize),
            (0x78, s78 as *const () as usize),
            (0x7C, s7c as *const () as usize),
            (0x84, s84 as *const () as usize),
            (0x88, s88 as *const () as usize),
            (0x8C, s8c as *const () as usize),
            (0x90, s90 as *const () as usize),
            (0x94, self::flags as *const () as usize),
            (0x98, s98 as *const () as usize),
            (0x9C, s9c as *const () as usize),
            (0xC8, budget as *const () as usize),
            (0xD8, exported as *const () as usize),
            (0xE8, produced as *const () as usize),
            (0xE0, imported as *const () as usize),
        ] {
            table[slot / 4] = f;
        }
        table[0x94 / 4] = self::flags as *const () as usize;
        let mut sim_table = table.clone();
        sim_table[0x88 / 4] = stats as *const () as usize;
        sim_table[0x94 / 4] = s94 as *const () as usize;
        let mut landfill_table = table.clone();
        landfill_table[0x28 / 4] = land as *const () as usize;
        let mut clock_table = table.clone();
        clock_table[0x1C / 4] = s1c as *const () as usize;
        let mut date_table = table.clone();
        date_table[0x9C / 4] = add_days as *const () as usize;
        let tables = vec![table, sim_table, landfill_table, clock_table, date_table];
        let mut objects: Vec<Object> = (0..10).map(|_| Object::new(0x40)).collect();
        let pointers: Vec<u32> = objects
            .iter_mut()
            .enumerate()
            .map(|(i, o)| {
                let t = match i {
                    0 => 1,
                    3 => 2,
                    8 => 3,
                    _ => 0,
                };
                let p = o.ptr();
                unsafe { p.cast::<u32>().write_unaligned(tables[t].as_ptr() as u32) };
                p as u32
            })
            .collect();
        let mut deals: Vec<Object> = flags.iter().map(|_| Object::new(0x80)).collect();
        let deal_pointers: Vec<u32> = deals
            .iter_mut()
            .enumerate()
            .map(|(i, o)| {
                let p = o.ptr();
                unsafe { p.cast::<u32>().write_unaligned(tables[0].as_ptr() as u32) };
                let d = unsafe { Deal::from_ptr(p) };
                d.set_kind(0x4773_FFE1);
                d.set_flags(flags[i]);
                d.set_amount(60);
                d.set_cost(1);
                d.set_start(500);
                d.set_expiration(1500);
                d.set_suspension(1000);
                p as u32
            })
            .collect();
        let mut header = Box::new([0u32; 3]);
        let h = header.as_mut_ptr() as u32;
        let mut nodes: Vec<[u32; 3]> = deal_pointers.iter().map(|&d| [0, 0, d]).collect();
        let node_ptrs: Vec<u32> = nodes.iter_mut().map(|n| n.as_mut_ptr() as u32).collect();
        for (i, node) in nodes.iter_mut().enumerate() {
            node[0] = node_ptrs.get(i + 1).copied().unwrap_or(h);
            node[1] = if i == 0 { h } else { node_ptrs[i - 1] };
        }
        header[0] = node_ptrs.first().copied().unwrap_or(h);
        header[1] = node_ptrs.last().copied().unwrap_or(h);
        let mut pending = Box::new([0u32; 3]);
        let p = pending.as_mut_ptr() as u32;
        pending[0] = p;
        pending[1] = p;
        let sim = unsafe { NeighborsSim::from_ptr(objects[0].ptr()) };
        sim.set_initialized(1);
        sim.set_deals(h as *mut u8);
        sim.set_pending_removals(p as *mut u8);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.pointers = pointers.clone();
            s.deals = deal_pointers;
            s.date_table = tables[4].as_ptr() as u32;
        });
        let mut globals: Vec<u32> = vec![
            pointers[7],
            pointers[4],
            pointers[8],
            pointers[3],
            12.0f32.to_bits(),
            0.000107f32.to_bits(),
            1.0f32.to_bits(),
            4294967296.0f32.to_bits(),
        ];
        let mut fakes: Vec<Fake> = [
            0x00B4_3CEC,
            0x00B4_3C94,
            0x00B4_3D5C,
            0x00B4_3D28,
            0x00AA_209C,
            0x00AB_1F7C,
            0x00A8_1228,
            0x00A8_0AA8,
        ]
        .into_iter()
        .zip(globals.iter_mut())
        .map(|(a, v)| fake_data(a, (v as *mut u32).cast()))
        .collect();
        for (address, function) in [
            (0x005F_98C0, post as *const () as usize),
            (0x0050_A2A0, clear as *const () as usize),
            (0x005F_D4F0, property as *const () as usize),
            (0x0090_CF54, allocate_node as *const () as usize),
            (0x0090_CF63, free_node as *const () as usize),
            (0x006C_F5C0, report_fake as *const () as usize),
            (0x0042_7640, append as *const () as usize),
            (0x0042_16E0, erase as *const () as usize),
            (0x006D_0A40, publish as *const () as usize),
            (0x0091_3177, date_new as *const () as usize),
            (0x0091_2DFA, date_time as *const () as usize),
            (0x0091_2E7B, date_day as *const () as usize),
            (0x0091_28BF, destroy as *const () as usize),
            (0x0091_297E, day_number as *const () as usize),
            (0x006C_E6A0, compare as *const () as usize),
        ] {
            fakes.push(fake(address, function));
        }
        Self {
            objects,
            deals,
            nodes,
            header,
            pending,
            _tables: tables,
            _globals: globals,
            _fakes: fakes,
            _fpu: fpu,
        }
    }
    fn sim(&mut self) -> NeighborsSim {
        unsafe { NeighborsSim::from_ptr(self.objects[0].ptr()) }
    }
    fn deal(&mut self, index: usize) -> Deal {
        unsafe { Deal::from_ptr(self.deals[index].ptr()) }
    }
    fn run(&mut self, day: u32) -> (u32, u32) {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.day = day;
            s.events.clear();
        });
        unsafe {
            call_thiscall_with_eax(
                fc4p_006d0e50 as *const () as usize,
                self.objects[0].ptr(),
                &[],
                0x1234_5678,
            )
        }
    }
    fn left(&self) -> Vec<usize> {
        let h = self.header.as_ptr() as *mut u8;
        let mut node = unsafe { pointer(h, 0) };
        let mut left = Vec::new();
        while node != h {
            left.push(object_id(unsafe { pointer(node, 8) }));
            node = unsafe { pointer(node, 0) };
        }
        left
    }
}

fn reports() -> Vec<(usize, u32)> {
    events()
        .iter()
        .filter_map(|e| {
            if let Event::Report(id, status) = e {
                Some((*id, *status))
            } else {
                None
            }
        })
        .collect()
}
fn slot(id: usize, offset: u32, args: &[u32]) -> Event {
    Event::Slot(id, offset, args.to_vec())
}

#[test]
fn activation_call_order_and_machine_state() {
    let mut w = World::new(&[1]);
    let before = machine_state();
    assert_eq!(w.run(1000), (0xBEEF_ABCD, 0x8765_4321));
    assert_eq!(machine_state(), before);
    assert_eq!(w.deal(0).flags(), 2);
    assert_eq!(w.sim().busy(), 0);
    let ev = events();
    let land_args = if let Event::Slot(3, 0x28, args) = &ev[5] {
        args.clone()
    } else {
        panic!("landfill order")
    };
    assert_eq!(
        ev,
        vec![
            slot(0, 0x88, &[STATE.with(|s| s.borrow().pointers[7])]),
            Event::Clear,
            slot(0, 0x94, &[]),
            slot(1, 0x18, &[]),
            Event::Property(0xEA46_934F),
            slot(3, 0x28, &land_args),
            slot(4, 0x90, &[]),
            slot(5, 0xD8, &[3]),
            slot(5, 0xE8, &[3]),
            slot(5, 0xE0, &[3]),
            slot(10, 0x9C, &[0x80]),
            slot(10, 0x84, &[]),
            slot(10, 0x20, &[]),
            slot(10, 0x9C, &[0x3C]),
            slot(10, 0x9C, &[1]),
            slot(10, 0x8C, &[]),
            slot(8, 0x1C, &[]),
            slot(10, 0x74, &[STATE.with(|s| s.borrow().pointers[9] + 8)]),
            Event::Report(10, 1),
            slot(0, 0x88, &[STATE.with(|s| s.borrow().pointers[7])]),
            Event::Publish(0, 1),
        ]
    );
    assert_eq!(w.deal(0).current(), 1000);
}

fn machine_state() -> (u16, u16, u32, u32) {
    let (mut cw, mut mxcsr) = (0u16, 0u32);
    let (sw, flags): (u16, u32);
    unsafe {
        core::arch::asm!(
            "fnstcw word ptr [{cw}]", "fnstsw ax", "stmxcsr dword ptr [{mxcsr}]", "pushfd", "pop {flags:e}",
            cw = in(reg) &mut cw, mxcsr = in(reg) &mut mxcsr, flags = lateout(reg) flags, out("ax") sw,
        );
    }
    (cw, sw & 0x3800, mxcsr, flags & 0x400)
}

#[test]
fn early_exits_ui_disable_and_noncanonical_dirty_bytes() {
    let mut w = World::new(&[1]);
    w.sim().set_initialized(0);
    assert_eq!(w.run(1000).0, 0x1234_5600);
    assert!(events().is_empty());
    w.sim().set_initialized(3);
    w.sim().set_busy(0xAB);
    assert_eq!(w.run(1000).0, 0x9988_7766);
    assert_eq!(events(), vec![Event::Post(vec![0xEA41_8D8F, 8, 0, 0, 0, 0])]);
    assert_eq!(w.sim().busy(), 0xAB);
    w.sim().set_busy(0);
    w.sim().set_ui_disabled(0xFE);
    w.sim().set_deals_dirty(0x81);
    w.sim().set_stats_dirty(0xAA);
    w.run(1000);
    assert_eq!(
        events(),
        vec![
            slot(0, 0x88, &[STATE.with(|s| s.borrow().pointers[7])]),
            Event::Publish(0x81, 0xAA)
        ]
    );
    assert_eq!(w.deal(0).flags(), 1);
    assert_eq!(w.sim().busy(), 0);
    w.sim().set_ui_disabled(0);
    w.sim().set_deals_dirty(0x81);
    w.run(1000);
    assert_eq!(events().last(), Some(&Event::Publish(1, 1)));
}

#[test]
fn grace_then_probation_then_removal() {
    let mut w = World::new(&[2]);
    STATE.with(|s| s.borrow_mut().failures = 8);
    w.run(1000);
    assert_eq!(w.deal(0).flags(), 2 | 0x100);
    assert_eq!(w.deal(0).suspension(), 1000);
    STATE.with(|s| s.borrow_mut().failures = 0);
    w.run(1089);
    assert_eq!(w.deal(0).flags(), 2 | 0x100);
    w.run(1090);
    assert_eq!(w.deal(0).flags(), 2 | 0x200);
    assert!(events().contains(&Event::AddDays(90)));
    STATE.with(|s| s.borrow_mut().failures = 8);
    w.run(1091);
    assert!(w.left().is_empty());
    assert_eq!(reports(), vec![(10, 3)]);
    assert_eq!(w.deal(0).amount(), 0);
    assert!(
        events().iter().position(|e| *e == Event::Report(10, 3)).unwrap()
            < events().iter().position(|e| *e == Event::Erase(10)).unwrap()
    );
}

#[test]
fn probation_boundary_and_float_to_day_edges() {
    for (day, expected) in [(1179, 0x202), (1180, 2)] {
        let mut w = World::new(&[0x202]);
        w.run(day);
        assert_eq!(w.deal(0).flags(), expected);
        assert!(events().contains(&Event::AddDays(180)));
    }
    for (grace, probation, expected) in [
        (90.75, 90.75, 181),
        (-1.9, 0.0, u32::MAX),
        (f32::NAN, 0.0, 0),
        (f32::INFINITY, 0.0, 0),
        (4294967296.0, 0.0, 0),
    ] {
        let mut w = World::new(&[0x202]);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.grace = Some(grace);
            s.probation = Some(probation);
        });
        w.run(1000);
        assert!(events().contains(&Event::AddDays(expected)));
    }
    let mut w = World::new(&[0x102]);
    STATE.with(|s| s.borrow_mut().grace = None);
    w.run(1000);
    assert!(events().contains(&Event::AddDays(0)));
    assert_eq!(w.deal(0).flags(), 0x202);
}

#[test]
fn removal_priority_and_reverse_queue_order() {
    for (flags, status) in [(0x23E, 2), (0x23A, 3), (0x232, 4), (0x222, 5)] {
        let mut w = World::new(&[flags]);
        w.run(1000);
        assert_eq!(reports(), vec![(10, status)]);
        assert!(w.left().is_empty());
        assert_eq!(w.pending[0], w.pending.as_ptr() as u32);
        assert_eq!(w.pending[1], w.pending.as_ptr() as u32);
    }
    let mut w = World::new(&[6, 0x20A, 2]);
    w.run(1000);
    assert_eq!(reports(), vec![(10, 2), (11, 3)]);
    assert_eq!(w.left(), vec![12]);
    let erased: Vec<_> = events().into_iter().filter(|e| matches!(e, Event::Erase(_))).collect();
    assert_eq!(erased, vec![Event::Erase(11), Event::Erase(10)]);
    assert_eq!(w.nodes.len(), 3);
}

#[test]
fn garbage_capacity_consumes_exports_in_list_order() {
    for (first, expected) in [(60, vec![11]), (40, vec![])] {
        let mut w = World::new(&[0x202, 0x202]);
        for i in 0..2 {
            w.deal(i).set_kind(0x4773_FFE5);
        }
        w.deal(0).set_amount(first);
        STATE.with(|s| s.borrow_mut().figures[0] = 100.0);
        w.run(1000);
        assert_eq!(w.left(), expected);
    }
    for (export, produced, amount, removed) in [
        (50.0, 50.0, 60, false),
        (f64::NAN, 50.0, 60, false),
        (100.0, f64::NAN, 60, false),
        (100.0, 50.0, -60, true),
    ] {
        let mut w = World::new(&[0x202]);
        w.deal(0).set_kind(0x4773_FFE5);
        w.deal(0).set_amount(amount);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.figures = [export, produced, 0.0];
        });
        w.run(1000);
        assert_eq!(w.left().is_empty(), removed);
    }
    // Unsigned subtraction wraps, and its high bit is corrected before the capacity product.
    let mut w = World::new(&[0x202]);
    w.deal(0).set_kind(0x4773_FFE5);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.landfill = [1, 0];
        s.figures = [100.0, 0.0, 0.0];
    });
    w.run(1000);
    assert_eq!(w.left(), vec![10]);
}

#[test]
fn expiration_then_renewal_and_payer_selection() {
    let mut w = World::new(&[2]);
    w.deal(0).set_start(635);
    w.deal(0).set_expiration(1000);
    w.run(1000);
    assert_eq!(w.deal(0).flags(), 0x42);
    assert_eq!(reports(), vec![(10, 6)]);
    w.run(1003);
    assert_eq!(reports(), vec![(10, 7)]);
    assert_eq!(
        (w.deal(0).start(), w.deal(0).expiration(), w.deal(0).current()),
        (1003, 1368, 1003)
    );
    assert!(events().contains(&slot(6, 0xC8, &[])));
    // The date objects are destroyed in reverse construction order on each scope exit.
    let created: Vec<_> = events()
        .iter()
        .filter_map(|e| {
            if let Event::NewDate(p, _, _) = e {
                Some(*p)
            } else {
                None
            }
        })
        .collect();
    let destroyed: Vec<_> = events()
        .iter()
        .filter_map(|e| if let Event::Destroy(p) = e { Some(*p) } else { None })
        .collect();
    assert_eq!(
        destroyed,
        vec![created[5], created[4], created[3], created[2], created[1], created[0]]
    );
    for kind in 0x4773_FFE0..=0x4773_FFE5 {
        let mut w = World::new(&[0x42]);
        w.deal(0).set_kind(kind);
        w.run(1000);
        assert!(events().contains(&slot(if kind & 1 == 0 { 5 } else { 6 }, 0xC8, &[])));
    }
}

#[test]
fn renewal_budget_boundaries_nan_and_int64_cost_rounding() {
    for (budget, cost, renew) in [
        (1.0, 1, true),
        (0.0, 1, false),
        (f64::NAN, 1, false),
        (f64::INFINITY, i64::MAX, true),
        (-2.0, -1, false),
        (16777216.0, 16777217, false),
        (9007199254740992.0, 9007199254740993, true),
    ] {
        let mut w = World::new(&[0x42]);
        w.deal(0).set_cost(cost);
        STATE.with(|s| s.borrow_mut().budgets[1] = budget);
        w.run(1000);
        assert_eq!(w.deal(0).flags(), if renew { 2 } else { 0x12 });
        assert_eq!(reports(), if renew { vec![(10, 7)] } else { vec![] });
    }
    let mut w = World::new(&[0x42]);
    STATE.with(|s| s.borrow_mut().null_date = true);
    w.run(1000);
    assert!(events().contains(&slot(10, 0x6C, &[0])));
}

#[test]
fn inactive_failed_activation_nested_retry_and_silent_cursor() {
    let mut w = World::new(&[0]);
    w.run(1000);
    assert_eq!(w.deal(0).flags(), 4);
    w.run(1001);
    assert_eq!(reports(), vec![(10, 2)]);
    assert!(w.left().is_empty());
    let mut w = World::new(&[1]);
    STATE.with(|s| s.borrow_mut().activate = false);
    w.run(1000);
    assert_eq!(w.deal(0).flags(), 1);
    assert!(reports().is_empty());
    let mut w = World::new(&[1]);
    STATE.with(|s| s.borrow_mut().reenter = true);
    w.run(1000);
    let ev = events();
    let i = ev.iter().position(|e| *e == Event::Report(10, 1)).unwrap();
    assert_eq!(ev[i + 1], Event::Post(vec![0xEA41_8D8F, 8, 0, 0, 0, 0]));
    let mut w = World::new(&[0x81]);
    w.run(1000);
    let ev = events();
    let i = ev.iter().position(|e| *e == slot(10, 0x9C, &[0x80])).unwrap();
    assert_eq!(ev[i + 1], slot(10, 0x9C, &[0x80]));
    assert_eq!(w.deal(0).flags(), 2);
}

#[test]
fn date_setter_uses_table_loaded_before_clock_callback() {
    unsafe extern "thiscall" fn wrong(_this: *mut u8, _arg: u32) -> u64 {
        panic!("new table must not be used")
    }
    for initial in [1, 0xA] {
        let mut w = World::new(&[initial]);
        let mut table = w._tables[0].clone();
        table[0x74 / 4] = wrong as *const () as usize;
        table[0x7C / 4] = wrong as *const () as usize;
        let deal = w.deal(0).as_ptr() as u32;
        STATE.with(|s| s.borrow_mut().clock_swap = Some((deal, table.as_ptr() as u32)));
        w.run(1000);
        assert_eq!(w.sim().busy(), 0);
    }
}

#[test]
fn renewal_compares_the_unrounded_st0_budget() {
    #[unsafe(naked)]
    unsafe extern "C" fn extended_budget() {
        core::arch::naked_asm!("fld tbyte ptr [ecx + 0x10]", "ret");
    }
    let mut w = World::new(&[0x42]);
    w._tables[0][0xC8 / 4] = extended_budget as *const () as usize;
    // The largest extended value below one rounds to one when stored as a double.
    let value = F80::from_bytes([0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFE, 0x3F]);
    assert_eq!(value.to_f64(), 1.0);
    unsafe { w.objects[6].ptr().add(0x10).cast::<F80>().write_unaligned(value) };
    w.run(1000);
    assert_eq!(w.deal(0).flags(), 0x12);
    assert!(reports().is_empty());
}

#[test]
fn preserves_existing_x87_value_and_callee_saved_registers() {
    let mut w = World::new(&[0x42]);
    let sentinel = 123.25f64;
    let mut saved = [0u32; 8];
    // SAFETY: the export takes no stack arguments. The block saves and restores EBP, EBX and ESI;
    // EDI carries the output pointer across the call, and the entry preserves it.
    unsafe {
        core::arch::asm!(
            "push ebx", "push ebp", "push esi", "mov ebx, 0x12345678", "mov ebp, 0x23456789", "mov esi, 0x3456789a",
            "mov [edi], esp", "fld qword ptr [edx]", "call {entry}",
            "mov [edi + 4], esp", "mov [edi + 8], ebx", "mov [edi + 12], ebp", "mov [edi + 16], esi", "mov [edi + 20], edi",
            "fstp qword ptr [edi + 24]", "pop esi", "pop ebp", "pop ebx",
            in("edx") &sentinel, entry = sym fc4p_006d0e50,
            in("ecx") w.objects[0].ptr(), in("edi") saved.as_mut_ptr(),
            clobber_abi("C"), out("st(0)") _,
        );
    }
    assert_eq!(saved[0], saved[1]);
    assert_eq!(saved[2], 0x1234_5678);
    assert_eq!(saved[3], 0x2345_6789);
    assert_eq!(saved[4], 0x3456_789A);
    assert_eq!(saved[5], saved.as_ptr() as u32);
    assert_eq!(pair(saved[6], saved[7]), sentinel.to_bits());
}

#[test]
fn short_stats_and_publish_callees_retain_upper_eax() {
    #[unsafe(naked)]
    unsafe extern "C" fn short_stats() {
        core::arch::naked_asm!("mov al, 0x55", "ret 4");
    }
    #[unsafe(naked)]
    unsafe extern "C" fn short_publish() {
        core::arch::naked_asm!("xor al, al", "ret 8");
    }
    for disabled in [0, 0xFE] {
        let mut w = World::new(&[]);
        w._tables[1][0x88 / 4] = short_stats as *const () as usize;
        let _fake = fake(0x006D_0A40, short_publish as *const () as usize);
        w.sim().set_ui_disabled(disabled);
        let source = if disabled == 0 {
            STATE.with(|s| s.borrow().pointers[7])
        } else {
            w._tables[1].as_ptr() as u32
        };
        assert_eq!(w.run(1000), (source & !0xFF, 0));
    }
}
