// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Core commodity figures, history, shared-curve behavior and x87 boundary cases.

use super::*;
use crate::testing::{fake, fake_data, Fake, GameFpu, Object};
use std::cell::RefCell;

type Event = (usize, u32, Vec<u32>);

#[derive(Clone)]
struct Figures {
    power: [i32; 2],
    water: [i32; 2],
    garbage64: [i64; 3],
    garbage32: [u32; 2],
    landfill: [u32; 2],
    funds: i64,
    income: i64,
    expenses: [i64; 3],
    energy_expenses: Option<i64>,
    water_department: bool,
    sanitation_department: bool,
    curve_property: bool,
    imports: [f64; 3],
    exports: [f64; 3],
    deals: Vec<(u32, i64)>,
    interval_years: f32,
    term_property: bool,
}

impl Default for Figures {
    fn default() -> Self {
        Self {
            power: [1000, 800],
            water: [2000, 1500],
            garbage64: [10, 20, 400],
            garbage32: [30, 5],
            landfill: [100, 1300],
            funds: 12345,
            income: 678,
            expenses: [99, 0, 1],
            energy_expenses: Some(2),
            water_department: false,
            sanitation_department: true,
            curve_property: true,
            imports: [50.0, 0.0, 0.0],
            exports: [0.0, 0.0, 7.9],
            deals: vec![
                (0x4773_FFE0, 51),
                (0x4773_FFE2, 13),
                (0x4773_FFE4, 17),
                (0x4773_FFE1, i64::MAX),
            ],
            interval_years: 1.0,
            term_property: true,
        }
    }
}

#[derive(Default)]
struct State {
    calls: Vec<Event>,
    pointers: Vec<u32>,
    figures: Figures,
    date_table: u32,
    dates: Vec<u32>,
    stats: Vec<(u32, u32, f64)>,
    history: Vec<(u32, u32)>,
    curve_input: u32,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }

fn index(this: *mut u8) -> usize {
    STATE.with(|s| {
        let s = s.borrow();
        if let Some(i) = s.pointers.iter().position(|&p| p == this as u32) {
            i
        } else if this as u32 == s.pointers[0] + 0x2C {
            40
        } else if let Some(i) = s.dates.iter().position(|&p| p == this as u32 || p + 8 == this as u32) {
            41 + i
        } else {
            43
        }
    })
}
fn record(this: *mut u8, slot: u32, args: &[u32]) {
    let i = index(this);
    STATE.with(|s| s.borrow_mut().calls.push((i, slot, args.to_vec())));
}
fn answer() -> u64 {
    pair(0xA1B2_C3D4, 0xE5F6_0718)
}
unsafe fn dispatch(this: *mut u8, slot: u32, args: &[u32]) -> u64 {
    record(this, slot, args);
    let i = index(this);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let p = &s.pointers;
        let f = &s.figures;
        let eax = match (i, slot) {
            (0, 0x94) => p[5],
            (1, 0xD0) => p[4],
            (1, 0x80) => p[7],
            (3, 0x90) => p[2],
            (4, 0x18) => return f.funds as u64,
            (4, 0x40) => return f.income as u64,
            (5, 0x18) => p[6],
            (6, 0x24) => {
                if f.curve_property {
                    p[30]
                } else {
                    0
                }
            }
            (8, 0x1C) => 123456,
            (9, 0x30) => f.power[0] as u32,
            (9, 0x2C) => f.power[1] as u32,
            (10, 0x38) => f.water[0] as u32,
            (10, 0x5C) => f.water[1] as u32,
            (11, 0x98) => return f.garbage64[0] as u64,
            (11, 0x88) => return f.garbage64[1] as u64,
            (11, 0x8C) => return f.garbage64[2] as u64,
            (11, 0x80) => f.garbage32[0],
            (11, 0x7C) => f.garbage32[1],
            (13, 0x70) => match args[0] {
                0x8910_BC8A => p[14],
                0xAA40_14B4 => {
                    if f.water_department {
                        p[15]
                    } else {
                        0
                    }
                }
                0x6A42_A8F5 => {
                    if f.sanitation_department {
                        p[16]
                    } else {
                        0
                    }
                }
                _ => unreachable!(),
            },
            (14..=16, 0x3C) => return f.expenses[i - 14] as u64,
            (14, 0x50) => {
                if f.energy_expenses.is_some() {
                    p[17]
                } else {
                    0
                }
            }
            (17, 0x28) => return f.energy_expenses.unwrap() as u64,
            (18, 0x0C) => p[20],
            (22..=25, 0x58) => return f.deals[i - 22].1 as u64,
            (22..=25, 0x20) => f.deals[i - 22].0,
            (2, 0xEC | 0xF4 | 0xDC | 0xE4 | 0xFC) => {
                let value = f64::from_bits(u64::from(args[1]) | (u64::from(args[2]) << 32));
                s.stats.push((slot, args[0], value));
                return answer();
            }
            (7, 0x30) => {
                s.history.push((args[0], args[1]));
                return answer();
            }
            (2, 0xCC | 0xD4 | 0x114 | 0x11C) | (0, 0x50) | (41..=42, 0xA0) => return answer(),
            _ => panic!("unexpected object {i}, slot {slot:x}"),
        };
        pair(eax, 0x1234_5678)
    })
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
slot0!(s20, 0x20);
slot0!(s28, 0x28);
slot0!(s2c, 0x2C);
slot0!(s30, 0x30);
slot0!(s38, 0x38);
slot0!(s3c, 0x3C);
slot0!(s40, 0x40);
slot0!(s58, 0x58);
slot0!(s5c, 0x5C);
slot0!(s7c, 0x7C);
slot0!(s80, 0x80);
slot0!(s88, 0x88);
slot0!(s8c, 0x8C);
slot0!(s90, 0x90);
slot0!(s94, 0x94);
slot0!(s98, 0x98);
slot0!(sd0, 0xD0);
slot0!(s11c, 0x11C);
slot0!(s1c, 0x1C);
slot1!(s24, 0x24);
slot1!(s50, 0x50);
slot1!(s70, 0x70);
slot1!(sa0, 0xA0);
slot1!(s114, 0x114);
slot1!(s0c, 0x0C);

unsafe extern "thiscall" fn enumerate(this: *mut u8, output: *mut u32, all: u32) -> u64 {
    record(this, 0x50, &[all]);
    let header = unsafe { output.read_unaligned() as *mut u8 };
    assert_eq!(unsafe { pointer(header, 0) }, header);
    assert_eq!(unsafe { pointer(header, 4) }, header);
    answer()
}
unsafe extern "thiscall" fn landfill(this: *mut u8, used: *mut u32, capacity: *mut u32) -> u64 {
    record(this, 0x28, &[]);
    let values = STATE.with(|s| s.borrow().figures.landfill);
    unsafe {
        used.write_unaligned(values[0]);
        capacity.write_unaligned(values[1]);
    }
    answer()
}
unsafe extern "thiscall" fn imported(this: *mut u8, commodity: u32) -> f64 {
    record(this, 0xE0, &[commodity]);
    STATE.with(|s| s.borrow().figures.imports[commodity as usize - 1])
}
unsafe extern "thiscall" fn exported(this: *mut u8, commodity: u32) -> f64 {
    record(this, 0xD8, &[commodity]);
    STATE.with(|s| s.borrow().figures.exports[commodity as usize - 1])
}
macro_rules! setstat {
    ($name:ident, $slot:literal) => {
        unsafe extern "thiscall" fn $name(this: *mut u8, commodity: u32, value: f64) -> u64 {
            let bits = double_words(value);
            unsafe { dispatch(this, $slot, &[commodity, bits[0], bits[1]]) }
        }
    };
}
setstat!(sec, 0xEC);
setstat!(sf4, 0xF4);
setstat!(sdc, 0xDC);
setstat!(se4, 0xE4);
setstat!(sfc, 0xFC);
unsafe extern "thiscall" fn history_set(this: *mut u8, id: u32, value: u32) -> u64 {
    unsafe { dispatch(this, 0x30, &[id, value]) }
}
unsafe extern "thiscall" fn funds_set(this: *mut u8, value: f64) -> u64 {
    unsafe { dispatch(this, 0xCC, &double_words(value)) }
}
unsafe extern "thiscall" fn income_set(this: *mut u8, value: f64) -> u64 {
    unsafe { dispatch(this, 0xD4, &double_words(value)) }
}
unsafe extern "C" fn property(this: *mut u8, id: u32, out: *mut f32) -> u64 {
    record(this, 0x005F_D4F0, &[id]);
    if STATE.with(|s| s.borrow().figures.term_property) {
        unsafe {
            out.write_unaligned(STATE.with(|s| s.borrow().figures.interval_years));
        }
    }
    answer()
}
unsafe extern "thiscall" fn date_new(this: *mut u8, time: u32) -> u64 {
    STATE.with(|s| s.borrow_mut().dates.push(this as u32));
    record(this, 0x0091_2DFA, &[time]);
    unsafe {
        this.add(8)
            .cast::<u32>()
            .write_unaligned(STATE.with(|s| s.borrow().date_table));
    }
    answer()
}
unsafe extern "thiscall" fn date_copy(this: *mut u8, source: *mut u8) -> u64 {
    STATE.with(|s| s.borrow_mut().dates.push(this as u32));
    record(this, 0x0091_2E24, &[index(source) as u32]);
    unsafe {
        this.add(8)
            .cast::<u32>()
            .write_unaligned(STATE.with(|s| s.borrow().date_table));
    }
    answer()
}
#[unsafe(naked)]
unsafe extern "C" fn date_destroy() {
    core::arch::naked_asm!("push edx", "mov edx, eax", "call {}", "ret", sym destroy_body);
}
unsafe extern "fastcall" fn destroy_body(this: *mut u8, eax: u32, edx: u32) -> u64 {
    record(this, 0x0091_28BF, &[]);
    pair(eax, edx)
}
unsafe extern "thiscall" fn destroy_list(this: *mut u32) -> u64 {
    let header = unsafe { this.read_unaligned() as *mut u8 };
    record(header, 0x0041_1FE0, &[]);
    answer()
}
unsafe extern "thiscall" fn set_curve(this: *mut u8, property: u32) -> u64 {
    record(this, 0x005F_4620, &[index(property as *mut u8) as u32]);
    answer()
}
unsafe extern "thiscall" fn curve(this: *mut u8, value: f32) -> f64 {
    record(this, 0x005F_4250, &[value.to_bits()]);
    STATE.with(|s| s.borrow_mut().curve_input = value.to_bits());
    2.25
}

struct Fixture {
    _tables: Vec<Vec<usize>>,
    _objects: Vec<Object>,
    p: Vec<*mut u8>,
    globals: Vec<(u32, u64)>,
    _guards: Vec<Fake>,
}
impl Fixture {
    fn new(figures: Figures) -> Self {
        let mut objects: Vec<_> = (0..31).map(|_| Object::new(0x80)).collect();
        let p: Vec<_> = objects.iter_mut().map(Object::ptr).collect();
        let mut tables = vec![vec![0usize; 0x120 / 4]; 32];
        let zero_slots: &[(usize, usize)] = &[
            (0x18, s18 as *const () as usize),
            (0x20, s20 as *const () as usize),
            (0x28, s28 as *const () as usize),
            (0x2C, s2c as *const () as usize),
            (0x30, s30 as *const () as usize),
            (0x38, s38 as *const () as usize),
            (0x3C, s3c as *const () as usize),
            (0x40, s40 as *const () as usize),
            (0x58, s58 as *const () as usize),
            (0x5C, s5c as *const () as usize),
            (0x7C, s7c as *const () as usize),
            (0x80, s80 as *const () as usize),
            (0x88, s88 as *const () as usize),
            (0x8C, s8c as *const () as usize),
            (0x90, s90 as *const () as usize),
            (0x94, s94 as *const () as usize),
            (0x98, s98 as *const () as usize),
            (0xD0, sd0 as *const () as usize),
            (0x11C, s11c as *const () as usize),
            (0x1C, s1c as *const () as usize),
        ];
        for table in &mut tables {
            for &(slot, f) in zero_slots {
                table[slot / 4] = f;
            }
            for (slot, f) in [
                (0x24, s24 as *const () as usize),
                (0x50, s50 as *const () as usize),
                (0x70, s70 as *const () as usize),
                (0xA0, sa0 as *const () as usize),
                (0x114, s114 as *const () as usize),
                (0x0C, s0c as *const () as usize),
            ] {
                table[slot / 4] = f;
            }
        }
        tables[0][0x50 / 4] = enumerate as *const () as usize;
        tables[2][0xCC / 4] = funds_set as *const () as usize;
        tables[2][0xD4 / 4] = income_set as *const () as usize;
        tables[2][0xE0 / 4] = imported as *const () as usize;
        tables[2][0xD8 / 4] = exported as *const () as usize;
        for (slot, f) in [
            (0xEC, sec as *const () as usize),
            (0xF4, sf4 as *const () as usize),
            (0xDC, sdc as *const () as usize),
            (0xE4, se4 as *const () as usize),
            (0xFC, sfc as *const () as usize),
        ] {
            tables[2][slot / 4] = f;
        }
        tables[7][0x30 / 4] = history_set as *const () as usize;
        tables[12][0x28 / 4] = landfill as *const () as usize;
        unsafe {
            for (object, table) in p.iter().zip(&tables) {
                object.cast::<*const usize>().write_unaligned(table.as_ptr());
            }
            NeighborsSim::from_ptr(p[0]).set_deals(p[21]);
            NeighborsSim::from_ptr(p[0]).set_deals_dirty(1);
            NeighborsSim::from_ptr(p[0]).set_stats_dirty(1);
            let mut previous = p[21];
            p[21].cast::<*mut u8>().write_unaligned(p[21]);
            for n in 0..figures.deals.len() {
                let node = p[26 + n];
                previous.cast::<*mut u8>().write_unaligned(node);
                node.cast::<*mut u8>().write_unaligned(p[21]);
                node.add(8).cast::<*mut u8>().write_unaligned(p[22 + n]);
                previous = node;
            }
        }
        let globals = vec![
            (0x00B4_3CEC, p[1] as u64),
            (0x00B4_3C94, p[3] as u64),
            (0x00B4_3D5C, p[8] as u64),
            (0x00B4_3DA0, p[9] as u64),
            (0x00B4_3D94, p[10] as u64),
            (0x00B4_3D9C, p[11] as u64),
            (0x00B4_3D28, p[12] as u64),
            (0x00B4_3D64, p[13] as u64),
            (0x00B2_07E0, p[18] as u64),
            (0x00AA_209C, 12.0f32.to_bits() as u64),
            (0x00A8_0AA8, 4_294_967_296.0f32.to_bits() as u64),
            (0x00AB_1F80, 1e-5f64.to_bits()),
            (0x00A8_0AB0, 1.0f64.to_bits()),
        ];
        let guards = [
            (0x005F_D4F0, property as *const () as usize),
            (0x0091_2DFA, date_new as *const () as usize),
            (0x0091_2E24, date_copy as *const () as usize),
            (0x0091_28BF, date_destroy as *const () as usize),
            (0x0041_1FE0, destroy_list as *const () as usize),
            (0x005F_4620, set_curve as *const () as usize),
            (0x005F_4250, curve as *const () as usize),
        ]
        .into_iter()
        .map(|(a, f)| fake(a, f))
        .collect();
        STATE.with(|s| {
            *s.borrow_mut() = State {
                pointers: p.iter().map(|&p| p as u32).collect(),
                figures,
                date_table: tables[31].as_ptr() as u32,
                ..State::default()
            }
        });
        Self {
            _tables: tables,
            _objects: objects,
            p,
            globals,
            _guards: guards,
        }
    }
    fn run(&mut self) -> u64 {
        let _data: Vec<_> = self
            .globals
            .iter_mut()
            .map(|(a, v)| fake_data(*a, (v as *mut u64).cast()))
            .collect();
        unsafe { update(self.p[0], 0x1234_5678, self.p[1], 0xCCDD_EEFF) }
    }
    fn run_export(&mut self, function: usize) -> u64 {
        let _data: Vec<_> = self
            .globals
            .iter_mut()
            .map(|(a, v)| fake_data(*a, (v as *mut u64).cast()))
            .collect();
        let args = [self.p[1] as u32];
        let mut call = RawCall::new(Conv::Thiscall, function, self.p[0] as u32, &args);
        call.edx = 0x1234_5678;
        unsafe {
            call_raw(&mut call);
        }
        pair(call.eax, call.edx)
    }
    fn stat(&self, slot: u32, commodity: u32) -> f64 {
        STATE.with(|s| {
            s.borrow()
                .stats
                .iter()
                .find(|&&(sl, c, _)| sl == slot && c == commodity)
                .unwrap()
                .2
        })
    }
    fn calls(&self) -> Vec<Event> {
        STATE.with(|s| s.borrow().calls.clone())
    }
}

#[test]
fn core_figures_all_deal_costs_history_and_shared_garbage_curve() {
    let _fpu = GameFpu::new();
    let mut f = Fixture::new(Figures::default());
    assert_eq!(f.run(), answer());
    assert_eq!(f.stat(0xEC, 1), 1000.0);
    assert_eq!(f.stat(0xF4, 1), 800.0);
    assert_eq!(f.stat(0xEC, 2), 2000.0);
    assert_eq!(f.stat(0xF4, 2), 1500.0);
    assert_eq!(f.stat(0xEC, 3), 165.0);
    assert_eq!(f.stat(0xF4, 3), 400.0);
    assert_eq!(f.stat(0xFC, 1), f64::from(150.0f32 / 1051.0f32));
    assert_eq!(f.stat(0xFC, 2), 1e-5);
    assert_eq!(f.stat(0xFC, 3), 2.25);
    assert_eq!(STATE.with(|s| s.borrow().curve_input), (20.0f32 / 166.0f32).to_bits());
    assert_eq!(
        STATE.with(|s| s.borrow().history.clone()),
        vec![
            (0xAA11_F837, 50),
            (0xAA11_F83A, 0),
            (0x0A14_EBB5, 0),
            (0xCA14_EBC6, 0),
            (0xEA11_017B, 7),
            (0xEA11_017E, 0),
            (0xEB54_F32E, 400),
            (0x4B54_F333, 165)
        ]
    );
    let calls = f.calls();
    let slots: Vec<_> = calls.iter().map(|&(i, slot, _)| (i, slot)).collect();
    let mut expected = vec![
        (3, 0x90),
        (1, 0xD0),
        (4, 0x18),
        (2, 0xCC),
        (1, 0xD0),
        (4, 0x40),
        (2, 0xD4),
        (0, 0x94),
        (5, 0x18),
        (1, 0x80),
        (6, 0x005F_D4F0),
        (8, 0x1C),
        (41, 0x0091_2DFA),
        (42, 0x0091_2E24),
        (42, 0xA0),
    ];
    for deal in 22..26 {
        expected.extend([(deal, 0x58), (deal, 0x20)]);
    }
    expected.extend([
        (18, 0x0C),
        (0, 0x50),
        (3, 0x90),
        (2, 0x114),
        (2, 0x11C),
        (20, 0x0041_1FE0),
        (9, 0x30),
        (9, 0x2C),
        (2, 0xE0),
        (2, 0xD8),
        (13, 0x70),
        (14, 0x3C),
    ]);
    for slot in [0xEC, 0xF4, 0xDC, 0xE4, 0xFC] {
        expected.push((2, slot));
    }
    expected.extend([
        (7, 0x30),
        (7, 0x30),
        (14, 0x50),
        (17, 0x28),
        (10, 0x38),
        (10, 0x5C),
        (2, 0xE0),
        (2, 0xD8),
        (13, 0x70),
    ]);
    for slot in [0xEC, 0xF4, 0xDC, 0xE4, 0xFC] {
        expected.push((2, slot));
    }
    expected.extend([
        (7, 0x30),
        (7, 0x30),
        (12, 0x28),
        (11, 0x98),
        (11, 0x88),
        (11, 0x80),
        (11, 0x7C),
        (11, 0x8C),
        (2, 0xE0),
        (2, 0xD8),
        (13, 0x70),
        (16, 0x3C),
        (6, 0x24),
        (40, 0x005F_4620),
        (40, 0x005F_4250),
    ]);
    for slot in [0xEC, 0xF4, 0xDC, 0xE4, 0xFC] {
        expected.push((2, slot));
    }
    expected.extend([
        (7, 0x30),
        (7, 0x30),
        (7, 0x30),
        (7, 0x30),
        (42, 0x0091_28BF),
        (41, 0x0091_28BF),
    ]);
    assert_eq!(slots, expected);
    assert_eq!(calls[3].2, double_words(12345.0));
    assert_eq!(calls[6].2, double_words(678.0));
    assert_eq!(calls[10].2, [0xEA46_934F]);
    assert_eq!(calls[14].2, [0x5B]);
    assert_eq!(unsafe { [*f.p[0].add(0x3C), *f.p[0].add(0x3D)] }, [0, 0]);
}

#[test]
fn pc24_denominator_rounding_water_department_and_missing_garbage_department() {
    let _fpu = GameFpu::new();
    let figures = Figures {
        power: [16_777_216, 1],
        water: [3, 2],
        expenses: [1, 1, 0],
        imports: [0.0; 3],
        exports: [0.0; 3],
        deals: vec![],
        water_department: true,
        sanitation_department: false,
        energy_expenses: None,
        ..Figures::default()
    };
    let mut f = Fixture::new(figures);
    assert_eq!(f.run(), answer());
    assert_eq!(f.stat(0xFC, 1), f64::from(f32::from_bits(0x3380_0000)));
    assert_eq!(f.stat(0xFC, 2), 0.25);
    assert_eq!(f.stat(0xFC, 3), 1e-5);
    assert!(f
        .calls()
        .iter()
        .all(|&(_, slot, _)| slot != 0x005F_4620 && slot != 0x005F_4250));
}

#[test]
fn unsigned_garbage_counts_wrapping_landfill_and_missing_property_keep_existing_curve() {
    let _fpu = GameFpu::new();
    let figures = Figures {
        garbage64: [0, 0, -123],
        garbage32: [0x8000_0001, u32::MAX],
        landfill: [u32::MAX, u32::MAX],
        curve_property: false,
        deals: vec![],
        ..Figures::default()
    };
    let mut f = Fixture::new(figures);
    assert_eq!(f.run(), answer());
    assert_eq!(f.stat(0xEC, 3), 6_442_450_944.0);
    assert_eq!(f.stat(0xF4, 3), -123.0);
    assert_eq!(
        STATE.with(|s| s.borrow().history.last().copied()),
        Some((0x4B54_F333, 0x8000_0000))
    );
    assert!(f.calls().iter().any(|&(_, slot, _)| slot == 0x005F_4250));
    assert!(f.calls().iter().all(|&(_, slot, _)| slot != 0x005F_4620));

    let figures = Figures {
        garbage64: [0; 3],
        garbage32: [0; 2],
        landfill: [u32::MAX, 11],
        deals: vec![],
        ..Figures::default()
    };
    let mut f = Fixture::new(figures);
    assert_eq!(f.run(), answer());
    assert_eq!(f.stat(0xEC, 3), 1.0);
}

#[test]
fn only_the_current_city_object_passes_the_gate_and_other_calls_keep_edx() {
    assert_eq!(
        unsafe { update(core::ptr::null_mut(), 0xDEAD_BEEF, core::ptr::null_mut(), 0) },
        pair(0, 0xDEAD_BEEF)
    );
    let mut current = 0x1234u32;
    let _data = fake_data(0x00B4_3CEC, (&mut current as *mut u32).cast());
    for city in [1u32, 0x5678, u32::MAX] {
        assert_eq!(
            unsafe { update(core::ptr::null_mut(), 0xDEAD_BEEF, city as *mut u8, 0) },
            pair(city, 0xDEAD_BEEF)
        );
    }
    let args = [0u32];
    let mut call = RawCall::new(Conv::Thiscall, fc4p_006d2a20 as *const () as usize, 0, &args);
    call.edx = 0xDEAD_BEEF;
    unsafe {
        call_raw(&mut call);
    }
    assert_eq!((call.eax, call.edx), (0, 0xDEAD_BEEF));
}

#[unsafe(naked)]
unsafe extern "C" fn export_with_term_scratch() {
    core::arch::naked_asm!(
        "lea eax, [esp - 4]", "and eax, -8", "mov dword ptr [eax - 0x74], 0x40000000",
        "jmp {}", sym fc4p_006d2a20,
    );
}

#[test]
fn missing_term_property_uses_the_original_stack_word_and_export_pops_one_argument() {
    let _fpu = GameFpu::new();
    let figures = Figures {
        term_property: false,
        ..Figures::default()
    };
    let mut f = Fixture::new(figures);
    let mut expected = unsafe { core::slice::from_raw_parts(f.p[0], 0x80) }.to_vec();
    expected[0x3C] = 0;
    expected[0x3D] = 0;
    assert_eq!(f.run_export(export_with_term_scratch as *const () as usize), answer());
    assert_eq!(f.stat(0xEC, 3), 115.0);
    assert_eq!(unsafe { core::slice::from_raw_parts(f.p[0], 0x80) }, expected);
}

#[test]
fn each_cost_is_stored_as_float_before_accumulation_and_zero_denominators_are_unclamped() {
    let _fpu = GameFpu::new();
    let figures = Figures {
        power: [0, 0],
        expenses: [0, 0, 0],
        imports: [0.0; 3],
        deals: vec![(0x4773_FFE0, 16_777_216), (0x4773_FFE0, 1), (0x4773_FFE0, -16_777_216)],
        sanitation_department: false,
        ..Figures::default()
    };
    let mut f = Fixture::new(figures);
    assert_eq!(f.run(), answer());
    assert_eq!(f.stat(0xFC, 1).to_bits(), 0.0f64.to_bits());

    let figures = Figures {
        power: [-1, i32::MIN],
        expenses: [1, 0, 0],
        imports: [0.0; 3],
        exports: [f64::NAN, 0.0, 0.0],
        deals: vec![],
        interval_years: 0.0,
        sanitation_department: false,
        ..Figures::default()
    };
    let mut f = Fixture::new(figures);
    assert_eq!(f.run(), answer());
    assert_eq!(f.stat(0xFC, 1), f64::INFINITY);
    assert_eq!(f.stat(0xEC, 3), f64::INFINITY);
    assert_eq!(STATE.with(|s| s.borrow().history[1].1), 0);
    assert_eq!(STATE.with(|s| s.borrow().history.last().unwrap().1), 0);
}
