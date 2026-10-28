// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Tax-history and national-economy modifiers for the twelve total demand nodes.

use super::queries::{add_float, divide_float, multiply_float};
use super::*;
use crate::x87::F80;
use core::mem::MaybeUninit;

game_data! { static application: *mut u8 = 0x00B4_3C94; }
game_data! { static history: *mut u8 = 0x00B4_3DC0; }
game_data! { static simulator: *mut u8 = 0x00B4_3D5C; }
game_data! { static tenth: f32 = 0x00A8_C950; }
game_data! { static one: f32 = 0x00A8_1228; }
game_fn! { fn date_new(date: *mut u8, day: u32) = "thiscall" 0x0091_2E7B; }
game_fn! { fn date_drop(date: *mut u8) -> u64 = "thiscall" 0x0091_28BF; }
game_fn! { fn curve_output(curve: *mut u8, input: f32) -> St0 = "thiscall" 0x005F_4250; }
game_fn! { fn map_value(map: *mut u8, key: *const u32) -> *mut *mut u8 = "thiscall" 0x0049_7A00; }

const TYPES: [u32; 12] = [
    0x1010, 0x1020, 0x1030, 0x3110, 0x3120, 0x3130, 0x3320, 0x3330, 0x4100, 0x4200, 0x4300, 0x4400,
];

fn subtract_float(value: F80, operand: f32) -> F80 {
    let mut output = F80::ZERO;
    // SAFETY: the operands and destination are live, and the one pushed register is popped.
    unsafe {
        core::arch::asm!(
            "fld tbyte ptr [{value}]", "fsub dword ptr [{operand}]", "fstp tbyte ptr [{output}]",
            value = in(reg) &value, operand = in(reg) &operand, output = in(reg) &mut output, out("st(0)") _,
        );
    }
    output
}

port! {
    /// Read the tax history for each total demand type, then apply its curve, ordinance factor and
    /// national economy modifier in that order. Clamp neutral tax below 0.1, keeping unordered values.
    /// Multiply the scaled maximum tax by a shared reciprocal, storing it after the first division.
    /// Commercial offices and industry receive their respective simulator bonuses.
    #[port(address = 0x006A_6070, owner = "demand", name = "cSC4DemandSimulator::UpdateTaxAndEconomyModifiers")]
    pub unsafe extern "thiscall" fn fc4p_006a6070(this: *mut u8) -> u64 {
        // SAFETY: the simulator holds live service, curve and node objects with the original interfaces.
        unsafe {
            let sim = DemandSim::from_ptr(this);
            let nation = vcall!(read_unaligned(application()), 0x88 => fn() -> *mut u8);
            let history = read_unaligned(history());
            let neutral_raw = vcall_st0!(this, 0x28);
            let mut neutral = neutral_raw.to_f32();
            if neutral_raw.compare(F80::from_f32(read_unaligned(tenth()))).less() {
                neutral = f32::from_bits(0x3DCC_CCCD);
            }
            let today = vcall!(read_unaligned(simulator()), 0x1C => fn() -> *mut u8);
            let day = vcall!(today, 0x68 => fn() -> u32).wrapping_sub(sim.tax_penalty_days());
            let mut date = MaybeUninit::<[u32; 4]>::uninit();
            let date = date.as_mut_ptr().cast::<u8>();
            date_new(date, day);
            let mut average = MaybeUninit::<f32>::uninit();
            let mut maximum = MaybeUninit::<f32>::uninit();
            let mut minimum = MaybeUninit::<f32>::uninit();
            let mut reciprocal = 0.0f32;
            for (i, kind) in TYPES.into_iter().enumerate() {
                // Outputs are reused even when a later history query reports no data.
                vcall!(history, 0x1C => fn(u32, *mut u8, *mut u8, *mut f32, *mut f32, *mut f32),
                    0xAABA_AA9Fu32 + i as u32, date.add(8), today, average.as_mut_ptr(), maximum.as_mut_ptr(), minimum.as_mut_ptr());
                let scaled = multiply_float(F80::from_f32(maximum.assume_init()), read_unaligned(tenth()));
                let recip = if i == 0 {
                    let value = divide_float(F80::from_f32(read_unaligned(one())), neutral);
                    reciprocal = value.to_f32();
                    value
                } else {
                    F80::from_f32(reciprocal)
                };
                let ratio = (recip * scaled).to_f32();
                let tax = curve_output(sim.tax_curve(), ratio).to_f32();
                let key = kind + 0x20000;
                let object = read_unaligned(map_value(this.add(0x50), &key));
                let factor = vcall_st0!(sim.ordinances(), 0x64, 0x2A65_0000u32 + kind, 0u32);
                let modifier = add_float(subtract_float(factor, read_unaligned(one())), tax).to_f32();
                vcall!(object, 0x64 => fn(u32), modifier.to_bits());
                // The first six types retain the economy slot before asking the nation for its value.
                let saved_slot = if i < 6 { vslot(object, 0x5C) } else { 0 };
                let economy = vcall_st0!(nation, 0x40, kind);
                let economy = if i >= 8 {
                    add_float(economy, f32::from_bits(sim.industrial_bonus()))
                } else if i >= 6 {
                    add_float(economy, f32::from_bits(sim.commercial_bonus()))
                } else {
                    economy
                }.to_f32();
                let target = if i < 6 { saved_slot } else { vslot(object, 0x5C) };
                let set = core::mem::transmute::<usize, unsafe extern "thiscall" fn(*mut u8, u32)>(target);
                set(object, economy.to_bits());
            }
            date_drop(date)
        }
    }
}

pub(super) const PORTS: &[Port] = &[fc4p_006a6070::PORT];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{fake, fake_data, Fake, GameFpu, Object};
    use std::cell::RefCell;

    #[derive(Debug, PartialEq)]
    enum Call {
        NationService,
        Neutral,
        ClockService,
        Day,
        DateNew(u32),
        History(u32),
        Curve(u32),
        Node(u32),
        Factor(u32, u32),
        Tax(u32, u32),
        Economy(u32),
        SetEconomy(u32, u32, bool),
        DateDrop,
    }
    struct State {
        calls: Vec<Call>,
        neutral: F80,
        curve: F80,
        factor: F80,
        economy: F80,
        maxima: [Option<f32>; 12],
        outputs: Option<[usize; 3]>,
        date: usize,
        nation: u32,
        today: u32,
        nodes: [u32; 12],
        index: usize,
        alternate_table: u32,
        replace_table: bool,
    }
    impl Default for State {
        fn default() -> Self {
            Self {
                calls: vec![],
                neutral: F80::ZERO,
                curve: F80::ZERO,
                factor: F80::ZERO,
                economy: F80::ZERO,
                maxima: [Some(70.0); 12],
                outputs: None,
                date: 0,
                nation: 0,
                today: 0,
                nodes: [0; 12],
                index: 0,
                alternate_table: 0,
                replace_table: false,
            }
        }
    }
    thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }

    extern "thiscall" fn nation_service(_p: *mut u8) -> u32 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.calls.push(Call::NationService);
            s.nation
        })
    }
    extern "thiscall" fn clock_service(_p: *mut u8) -> u32 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.calls.push(Call::ClockService);
            s.today
        })
    }
    extern "thiscall" fn day(_p: *mut u8) -> u32 {
        STATE.with(|s| s.borrow_mut().calls.push(Call::Day));
        10
    }
    extern "thiscall" fn date_new_fake(p: *mut u8, day: u32) {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.calls.push(Call::DateNew(day));
            s.date = p as usize;
        });
        unsafe {
            p.cast::<[u32; 4]>().write([0x1234_5678, 0xAABB_CCDD, day, 0x9988_7766]);
        }
    }
    extern "thiscall" fn date_drop_fake(p: *mut u8) -> u64 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            assert_eq!(p as usize, s.date);
            s.calls.push(Call::DateDrop);
        });
        0x1234_5678_9988_7766
    }
    extern "thiscall" fn history_fake(
        _p: *mut u8,
        id: u32,
        begin: *mut u8,
        end: *mut u8,
        average: *mut f32,
        maximum: *mut f32,
        minimum: *mut f32,
    ) -> u32 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.index = id.wrapping_sub(0xAABA_AA9F) as usize;
            s.calls.push(Call::History(id));
            assert_eq!(begin as usize, s.date + 8);
            assert_eq!(end as u32, s.today);
            assert_eq!(unsafe { *begin.cast::<u32>() }, 10u32.wrapping_sub(60));
            let outputs = [average as usize, maximum as usize, minimum as usize];
            if let Some(prior) = s.outputs {
                assert_eq!(prior, outputs);
            } else {
                s.outputs = Some(outputs);
            }
            unsafe {
                *average = 123.0;
                *minimum = -456.0;
                if let Some(value) = s.maxima[s.index] {
                    *maximum = value;
                }
            }
        });
        // The caller consumes the outputs even when the query reports failure.
        0
    }
    extern "thiscall" fn neutral_record(_p: *mut u8) -> *const F80 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.calls.push(Call::Neutral);
            &s.neutral as *const F80
        })
    }
    extern "thiscall" fn curve_record(p: *mut u8, input: u32) -> *const F80 {
        assert_eq!(p as u32, 0x1234);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.calls.push(Call::Curve(input));
            &s.curve as *const F80
        })
    }
    extern "thiscall" fn factor_record(_p: *mut u8, id: u32, flags: u32) -> *const F80 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.calls.push(Call::Factor(id, flags));
            &s.factor as *const F80
        })
    }
    extern "thiscall" fn economy_record(_p: *mut u8, kind: u32) -> *const F80 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.calls.push(Call::Economy(kind));
            if s.replace_table {
                unsafe {
                    *(s.nodes[s.index] as *mut u32) = s.alternate_table;
                }
            }
            &s.economy as *const F80
        })
    }
    #[unsafe(naked)]
    unsafe extern "thiscall" fn neutral_fake(_p: *mut u8) {
        core::arch::naked_asm!("call {body}", "fld tbyte ptr [eax]", "ret", body = sym neutral_record)
    }
    #[unsafe(naked)]
    unsafe extern "thiscall" fn curve_fake(_p: *mut u8, _input: u32) {
        core::arch::naked_asm!("push dword ptr [esp + 4]", "call {body}", "fld tbyte ptr [eax]", "ret 4", body = sym curve_record)
    }
    #[unsafe(naked)]
    unsafe extern "thiscall" fn factor_fake(_p: *mut u8, _id: u32, _flags: u32) {
        core::arch::naked_asm!("push dword ptr [esp + 8]", "push dword ptr [esp + 8]", "call {body}", "fld tbyte ptr [eax]", "ret 8", body = sym factor_record)
    }
    #[unsafe(naked)]
    unsafe extern "thiscall" fn economy_fake(_p: *mut u8, _kind: u32) {
        core::arch::naked_asm!("push dword ptr [esp + 4]", "call {body}", "fld tbyte ptr [eax]", "ret 4", body = sym economy_record)
    }
    extern "thiscall" fn node(_p: *mut u8, key: *const u32) -> *mut u32 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.calls.push(Call::Node(unsafe { *key }));
            let i = s.index;
            &mut s.nodes[i] as *mut u32
        })
    }
    extern "thiscall" fn tax(p: *mut u8, bits: u32) {
        let key = unsafe { *p.add(4).cast::<u32>() };
        STATE.with(|s| s.borrow_mut().calls.push(Call::Tax(key, bits)));
    }
    extern "thiscall" fn set_economy(p: *mut u8, bits: u32) {
        set(p, bits, false);
    }
    extern "thiscall" fn set_alternate(p: *mut u8, bits: u32) {
        set(p, bits, true);
    }
    fn set(p: *mut u8, bits: u32, alternate: bool) {
        let key = unsafe { *p.add(4).cast::<u32>() };
        STATE.with(|s| s.borrow_mut().calls.push(Call::SetEconomy(key, bits, alternate)));
    }
    struct Fixture {
        sim: Object,
        _tables: Vec<[usize; 40]>,
        _services: Vec<[u32; 1]>,
        _nodes: Vec<[u32; 2]>,
        _globals: Box<[u32; 3]>,
        _constants: Box<[f32; 2]>,
        _guards: Vec<Fake>,
    }
    impl Fixture {
        fn new(neutral: f32) -> Self {
            let table = |entries: &[(usize, usize)]| {
                let mut t = [0; 40];
                for &(slot, f) in entries {
                    t[slot / 4] = f;
                }
                t
            };
            let mut tables = vec![
                table(&[(0x88, nation_service as *const () as usize)]),
                table(&[(0x1C, history_fake as *const () as usize)]),
                table(&[(0x1C, clock_service as *const () as usize)]),
                table(&[(0x68, day as *const () as usize)]),
                table(&[(0x40, economy_fake as *const () as usize)]),
                table(&[(0x64, factor_fake as *const () as usize)]),
                table(&[(0x28, neutral_fake as *const () as usize)]),
                table(&[
                    (0x64, tax as *const () as usize),
                    (0x5C, set_economy as *const () as usize),
                ]),
                table(&[
                    (0x64, tax as *const () as usize),
                    (0x5C, set_alternate as *const () as usize),
                ]),
            ];
            let mut services: Vec<_> = (0..6).map(|i| [tables[i].as_mut_ptr() as u32]).collect();
            let pointers: Vec<_> = services.iter_mut().map(|s| s.as_mut_ptr() as u32).collect();
            let mut nodes: Vec<_> = TYPES
                .map(|kind| [tables[7].as_mut_ptr() as u32, kind + 0x20000])
                .to_vec();
            let mut sim = Object::new(0xBC);
            let view = unsafe { DemandSim::from_ptr(sim.ptr()) };
            view.set_vtable(tables[6].as_mut_ptr() as u32);
            view.set_ordinances(pointers[5] as *mut u8);
            view.set_tax_curve(0x1234usize as *mut u8);
            view.set_tax_penalty_days(60);
            view.set_commercial_bonus(0.5f32.to_bits());
            view.set_industrial_bonus(0.25f32.to_bits());
            let mut globals = Box::new([pointers[0], pointers[1], pointers[2]]);
            let mut constants = Box::new([0.1f32, 1.0f32]);
            let guards = vec![
                fake_data(0x00B4_3C94, globals.as_mut_ptr().cast()),
                fake_data(0x00B4_3DC0, unsafe { globals.as_mut_ptr().add(1).cast() }),
                fake_data(0x00B4_3D5C, unsafe { globals.as_mut_ptr().add(2).cast() }),
                fake_data(0x00A8_C950, constants.as_mut_ptr().cast()),
                fake_data(0x00A8_1228, unsafe { constants.as_mut_ptr().add(1).cast() }),
                fake(0x0091_2E7B, date_new_fake as *const () as usize),
                fake(0x0091_28BF, date_drop_fake as *const () as usize),
                fake(0x0049_7A00, node as *const () as usize),
                fake(0x005F_4250, curve_fake as *const () as usize),
            ];
            STATE.with(|s| {
                *s.borrow_mut() = State {
                    neutral: F80::from_f32(neutral),
                    curve: F80::from_f32(0.25),
                    factor: F80::from_f32(1.5),
                    economy: F80::from_f32(0.06),
                    nation: pointers[4],
                    today: pointers[3],
                    nodes: core::array::from_fn(|i| nodes[i].as_mut_ptr() as u32),
                    alternate_table: tables[8].as_mut_ptr() as u32,
                    ..State::default()
                }
            });
            Self {
                sim,
                _tables: tables,
                _services: services,
                _nodes: nodes,
                _globals: globals,
                _constants: constants,
                _guards: guards,
            }
        }
        fn run(&mut self) {
            let before = unsafe { core::slice::from_raw_parts(self.sim.ptr(), 0xBC).to_vec() };
            assert_eq!(unsafe { fc4p_006a6070(self.sim.ptr()) }, 0x1234_5678_9988_7766);
            assert_eq!(unsafe { core::slice::from_raw_parts(self.sim.ptr(), 0xBC) }, before);
        }
    }

    #[test]
    fn tax_and_economy_calls_follow_all_twelve_types_and_cache_the_first_six_economy_slots() {
        let _fpu = GameFpu::new();
        let mut f = Fixture::new(10.0);
        STATE.with(|s| s.borrow_mut().replace_table = true);
        f.run();
        let mut expected = vec![
            Call::NationService,
            Call::Neutral,
            Call::ClockService,
            Call::Day,
            Call::DateNew(10u32.wrapping_sub(60)),
        ];
        for (i, kind) in TYPES.into_iter().enumerate() {
            let economy = if i >= 8 {
                0.06f32 + 0.25
            } else if i >= 6 {
                0.06f32 + 0.5
            } else {
                0.06f32
            };
            expected.extend([
                Call::History(0xAABA_AA9F + i as u32),
                Call::Curve(0.7f32.to_bits()),
                Call::Node(kind + 0x20000),
                Call::Factor(0x2A65_0000 + kind, 0),
                Call::Tax(kind + 0x20000, 0.75f32.to_bits()),
                Call::Economy(kind),
                Call::SetEconomy(kind + 0x20000, economy.to_bits(), i >= 6),
            ]);
        }
        expected.push(Call::DateDrop);
        STATE.with(|s| assert_eq!(s.borrow().calls, expected));
    }

    #[test]
    fn tax_ratio_uses_the_core_reciprocal_multiplication_worked_cases() {
        let _fpu = GameFpu::new();
        for (neutral_bits, maximum, ratio_bits) in [
            (0x410F_F5FD, 70.0, 0x3F47_2A4A),
            (0x410F_F5FD, 90.0, 0x3F80_08E7),
            (0x410F_F67A, 70.0, 0x3F47_299E),
        ] {
            let mut f = Fixture::new(f32::from_bits(neutral_bits));
            STATE.with(|s| s.borrow_mut().maxima = [Some(maximum); 12]);
            f.run();
            STATE.with(|s| {
                assert_eq!(
                    s.borrow()
                        .calls
                        .iter()
                        .filter_map(|c| if let Call::Curve(bits) = c { Some(*bits) } else { None })
                        .collect::<Vec<_>>(),
                    [ratio_bits; 12]
                )
            });
        }
    }

    #[test]
    fn neutral_clamp_keeps_nan_and_history_failures_reuse_the_preceding_maximum() {
        let _fpu = GameFpu::new();
        for (neutral, ratio) in [
            (0.0, 70.0),
            (-3.0, 70.0),
            (0.1, 70.0),
            (f32::INFINITY, 0.0),
            (f32::NAN, f32::NAN),
        ] {
            let mut f = Fixture::new(neutral);
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                s.maxima = [None; 12];
                s.maxima[0] = Some(70.0);
            });
            f.run();
            STATE.with(|s| {
                for call in &s.borrow().calls {
                    if let Call::Curve(bits) = call {
                        if ratio.is_nan() {
                            assert!(f32::from_bits(*bits).is_nan());
                        } else {
                            assert_eq!(*bits, ratio.to_bits());
                        }
                    }
                }
            });
        }
    }
}
