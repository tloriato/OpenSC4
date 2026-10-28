// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Simulation messages and immediate occupancy transitions.

use super::*;

game_fn! { #[allow(dead_code)] fn begin(this:*mut u8) -> u64 = "thiscall" 0x006A_27E0; }
game_fn! { #[allow(dead_code)] fn propagate(this:*mut u8) -> u64 = "thiscall" 0x006A_4E70; }
game_fn! { #[allow(dead_code)] fn idle(this:*mut u8,budget:u32) -> u64 = "thiscall" 0x006A_7020; }
game_fn! { #[allow(dead_code)] fn daily(this:*mut u8) -> u64 = "thiscall" 0x006A_7600; }
game_fn! { #[allow(dead_code)] fn monthly(this:*mut u8) -> u64 = "thiscall" 0x006A_9450; }
game_fn! { #[allow(dead_code)] fn tune(this:*mut u8) -> u64 = "thiscall" 0x006A_8790; }
game_fn! { #[allow(dead_code)] fn reset(this:*mut u8,force:u32) -> u64 = "thiscall" 0x006A_5CB0; }
game_fn! { #[allow(dead_code)] fn occupied(state:u32) -> u64 = "C" 0x0069_5260; }
game_fn! { #[allow(dead_code)] fn property(holder:*mut u8,id:u32,out:*mut u8) -> u64 = "C" 0x005F_D390; }
game_fn! { #[allow(dead_code)] fn effect(this:*mut u8,kind:u32,add:u32,percentage:u32) -> u64 = "thiscall" 0x006A_5920; }
game_fn! { #[allow(dead_code)] fn capacity(this:*mut u8,kind:u32,wealth:u32,add:u32) -> u64 = "thiscall" 0x006A_2C80; }
game_data! { static full_percentage:u32 = 0x00B0_D278; }

#[repr(C)]
struct MessageEntry {
    eax: u32,
    edx: u32,
    scratch: u32,
    return_address: u32,
    args: [u32; 1],
}

unsafe fn invoke(address: u32, conv: Conv, this: *mut u8, eax: u32, edx: u32, args: &[u32]) -> (u32, u32) {
    let mut call = RawCall::new(conv, crate::abi::target(address), this as u32, args);
    call.eax = eax;
    call.edx = edx;
    unsafe { call_raw(&mut call) };
    (call.eax, call.edx)
}

port! {
    /// Dispatch the secondary interface's message, preserving its argument scratch writes and the final
    /// register values above AL. The occupancy flag's unused upper bytes retain the entry stack scratch.
    #[port(address = 0x006A_9470, owner = "demand", name = "cSC4DemandSimulator::DoMessage", naked)]
    pub unsafe extern "C" fn fc4p_006a9470() {
        core::arch::naked_asm!(
            "push dword ptr [esp - 8]", "push edx", "push eax", "mov edx, esp", "call {body}",
            "add esp, 12", "ret 4", body = sym dispatch,
        )
    }
}

unsafe extern "fastcall" fn dispatch(this: *mut u8, entry: *mut MessageEntry) -> u64 {
    unsafe {
        let e = &mut *entry;
        let message = e.args[0] as *mut u8;
        let sim = this.wrapping_sub(4);
        let first = virtual_call(message, 0x10, read_unaligned(message.cast()), e.edx, &[]);
        let (mut eax, mut edx) = (first.eax, first.edx);
        match eax {
            0x6695_6810 => {
                (eax, edx) = invoke(0x006A_27E0, Conv::Thiscall, sim, eax, edx, &[]);
            }
            0x26C6_3343 => {
                (eax, edx) = invoke(0x006A_4E70, Conv::Thiscall, sim, eax, edx, &[]);
                (eax, edx) = invoke(0x006A_4E70, Conv::Thiscall, sim, eax, edx, &[]);
            }
            0x6695_6814 => {
                (eax, edx) = invoke(0x006A_7020, Conv::Thiscall, sim, eax, edx, &[5]);
                (eax, edx) = invoke(0x006A_7600, Conv::Thiscall, sim, eax, edx, &[]);
            }
            0x6695_6816 => {
                (eax, edx) = invoke(0x006A_9450, Conv::Thiscall, sim, eax, edx, &[]);
            }
            0x6695_681A => {
                let data = virtual_call(message, 0x4C, eax, read_unaligned(message.cast()), &[]);
                (eax, edx) = invoke(0x006A_7020, Conv::Thiscall, sim, data.eax, data.edx, &[data.eax]);
            }
            0xCA1B_151C => {
                (eax, edx) = invoke(0x006A_8790, Conv::Thiscall, sim, eax, edx, &[]);
            }
            0x8A0D_15DA | 0xEA0D_163E => {
                let a = virtual_call(message, 0x24, eax, read_unaligned(message.cast()), &[]);
                let b = virtual_call(message, 0x38, read_unaligned(message.cast()), a.edx, &[]);
                let c = virtual_call(message, 0x4C, b.eax, read_unaligned(message.cast()), &[]);
                (eax, edx) = invoke(0x006A_5CB0, Conv::Thiscall, sim, c.eax, c.edx, &[0]);
            }
            0xE8BD_A2E7 => {
                let lot_call = virtual_call(message, 0x80, read_unaligned(message.cast()), edx, &[]);
                let lot = lot_call.eax as *mut u8;
                let old = virtual_call(message, 0x24, lot_call.eax, read_unaligned(message.cast()), &[]);
                e.args[0] = old.eax;
                let new = virtual_call(message, 0x38, read_unaligned(message.cast()), old.edx, &[]);
                let kind = virtual_call(message, 0x4C, new.eax, read_unaligned(message.cast()), &[]);
                let wealth = virtual_call(message, 0x60, read_unaligned(message.cast()), kind.edx, &[]);
                let (new_state, wealth) = (new.eax, wealth);
                edx = wealth.edx;
                let was_occupied = (2..4).contains(&(old.eax as u8));
                let is_occupied = (2..4).contains(&(new_state as u8));
                eax = u32::from(was_occupied);
                if was_occupied != is_occupied {
                    let mut kind = kind.eax;
                    if kind == 0 {
                        let c = virtual_call(lot, 0xB8, eax, read_unaligned(lot.cast()), &[1]);
                        (eax, edx) = (c.eax, c.edx);
                        kind = eax;
                    }
                    if kind != 0 {
                        e.args[0] &= !0xFF;
                        let config = virtual_call(lot, 0xC0, read_unaligned(lot.cast()), edx, &[]);
                        (eax, edx) = (config.eax, config.edx);
                        if eax != 0 {
                            let config = eax as *mut u8;
                            let exemplar = virtual_call(config, 0x0C, eax, read_unaligned(config.cast()), &[]);
                            let exemplar_pointer = exemplar.eax as *mut u8;
                            let holder = virtual_call(
                                exemplar_pointer,
                                0x18,
                                exemplar.eax,
                                read_unaligned(exemplar_pointer.cast()),
                                &[],
                            );
                            (eax, edx) = invoke(
                                0x005F_D390,
                                Conv::Cdecl,
                                exemplar_pointer,
                                holder.eax,
                                holder.edx,
                                &[holder.eax, 0x2781_2844, e.args.as_mut_ptr() as u32],
                            );
                        }
                        eax = (eax & !0xFF) | (e.args[0] & 0xFF);
                        if eax as u8 == 0 {
                            let (flag, flag_edx) =
                                invoke(0x0069_5260, Conv::Cdecl, core::ptr::null_mut(), eax, edx, &[new_state]);
                            let add = (e.scratch & !0xFF) | (flag & 0xFF);
                            let percentage = read_unaligned(full_percentage());
                            (eax, edx) = invoke(
                                0x006A_5920,
                                Conv::Thiscall,
                                sim,
                                percentage,
                                flag_edx,
                                &[kind, add, percentage],
                            );
                            (eax, edx) = invoke(0x006A_2C80, Conv::Thiscall, sim, eax, edx, &[kind, wealth.eax, add]);
                        }
                    }
                }
            }
            _ => {}
        }
        registers((eax & !0xFF) | 1, edx)
    }
}

pub(super) const PORTS: &[Port] = &[fc4p_006a9470::PORT];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demand::test_support::Probe;
    use crate::testing::{fake, fake_data, Object};
    use std::cell::RefCell;

    #[derive(Debug, PartialEq)]
    enum Call {
        Virtual(u32, u32, Vec<u32>),
        Game(u32, u32, Vec<u32>),
        Property(u32, u32, u32),
    }
    #[derive(Default)]
    struct State {
        calls: Vec<Call>,
        id: u32,
        old: u32,
        new: u32,
        kind: u32,
        wealth: u32,
        lot: u32,
        config: u32,
        exemplar: u32,
        holder: u32,
        fallback_kind: u32,
        property: u8,
    }
    thread_local! {static STATE:RefCell<State>=RefCell::new(State::default());}
    fn getter(p: *mut u8, slot: u32, value: impl FnOnce(&State) -> u32) -> u64 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.calls.push(Call::Virtual(p as u32, slot, vec![]));
            registers(value(&s), 0x7654_0000 | slot)
        })
    }
    extern "thiscall" fn id(p: *mut u8) -> u64 {
        getter(p, 0x10, |s| s.id)
    }
    extern "thiscall" fn old(p: *mut u8) -> u64 {
        getter(p, 0x24, |s| s.old)
    }
    extern "thiscall" fn new(p: *mut u8) -> u64 {
        getter(p, 0x38, |s| s.new)
    }
    extern "thiscall" fn kind(p: *mut u8) -> u64 {
        getter(p, 0x4C, |s| s.kind)
    }
    extern "thiscall" fn wealth(p: *mut u8) -> u64 {
        getter(p, 0x60, |s| s.wealth)
    }
    extern "thiscall" fn lot(p: *mut u8) -> u64 {
        getter(p, 0x80, |s| s.lot)
    }
    extern "thiscall" fn config(p: *mut u8) -> u64 {
        getter(p, 0xC0, |s| s.config)
    }
    extern "thiscall" fn exemplar(p: *mut u8) -> u64 {
        getter(p, 0x0C, |s| s.exemplar)
    }
    extern "thiscall" fn holder(p: *mut u8) -> u64 {
        getter(p, 0x18, |s| s.holder)
    }
    extern "thiscall" fn fallback(p: *mut u8, arg: u32) -> u64 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.calls.push(Call::Virtual(p as u32, 0xB8, vec![arg]));
            registers(s.fallback_kind, 0x7654_00B8)
        })
    }
    fn game(address: u32, this: *mut u8, args: &[u32]) -> u64 {
        STATE.with(|s| {
            s.borrow_mut()
                .calls
                .push(Call::Game(address, this as u32, args.to_vec()))
        });
        registers(0xABCD_EF24, 0x9988_7766)
    }
    extern "thiscall" fn beginning(p: *mut u8) -> u64 {
        game(0x006A_27E0, p, &[])
    }
    extern "thiscall" fn propagation(p: *mut u8) -> u64 {
        game(0x006A_4E70, p, &[])
    }
    extern "thiscall" fn idling(p: *mut u8, a: u32) -> u64 {
        game(0x006A_7020, p, &[a])
    }
    extern "thiscall" fn dailies(p: *mut u8) -> u64 {
        game(0x006A_7600, p, &[])
    }
    extern "thiscall" fn monthlies(p: *mut u8) -> u64 {
        game(0x006A_9450, p, &[])
    }
    extern "thiscall" fn tuning(p: *mut u8) -> u64 {
        game(0x006A_8790, p, &[])
    }
    extern "thiscall" fn resetting(p: *mut u8, a: u32) -> u64 {
        game(0x006A_5CB0, p, &[a])
    }
    extern "thiscall" fn effects(p: *mut u8, a: u32, b: u32, c: u32) -> u64 {
        game(0x006A_5920, p, &[a, b, c])
    }
    extern "thiscall" fn capacities(p: *mut u8, a: u32, b: u32, c: u32) -> u64 {
        game(0x006A_2C80, p, &[a, b, c])
    }
    extern "C" fn is_occupied(a: u32) -> u64 {
        game(0x0069_5260, core::ptr::null_mut(), &[a]);
        registers(0xDEAD_BE00 | u32::from((2..4).contains(&(a as u8))), 0xAABB_CCDD)
    }
    extern "C" fn read_property(p: u32, id: u32, out: *mut u32) -> u64 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.calls.push(Call::Property(p, id, unsafe { *out }));
            unsafe { *out.cast::<u8>() = s.property };
        });
        registers(0xA1B2_C355, 0x8877_6655)
    }
    #[unsafe(naked)]
    unsafe extern "C" fn seeded() {
        core::arch::naked_asm!("mov dword ptr [esp - 8], 0xa1b2c3d4","jmp {body}",body=sym fc4p_006a9470)
    }
    struct World {
        _table: Box<[usize; 49]>,
        message: Box<[u32; 1]>,
        lot: Box<[u32; 1]>,
        config: Box<[u32; 1]>,
        exemplar: Box<[u32; 1]>,
        sim: Object,
    }
    impl World {
        fn new() -> Self {
            let mut table = Box::new([0usize; 49]);
            for (slot, f) in [
                (0x10, id as *const ()),
                (0x24, old as *const ()),
                (0x38, new as *const ()),
                (0x4C, kind as *const ()),
                (0x60, wealth as *const ()),
                (0x80, lot as *const ()),
                (0xC0, config as *const ()),
                (0x0C, exemplar as *const ()),
                (0x18, holder as *const ()),
                (0xB8, fallback as *const ()),
            ] {
                table[slot / 4] = f as usize;
            }
            let word = table.as_ptr() as u32;
            Self {
                _table: table,
                message: Box::new([word]),
                lot: Box::new([word]),
                config: Box::new([word]),
                exemplar: Box::new([word]),
                sim: Object::new(0x100),
            }
        }
        fn prepare(&mut self, id: u32) {
            STATE.with(|s| {
                *s.borrow_mut() = State {
                    id,
                    old: 0,
                    new: 2,
                    kind: 0x12345678,
                    wealth: 0x89ABCDEF,
                    lot: self.lot.as_mut_ptr() as u32,
                    config: self.config.as_mut_ptr() as u32,
                    exemplar: self.exemplar.as_mut_ptr() as u32,
                    holder: 0x99887766,
                    ..State::default()
                }
            });
        }
        fn run(&mut self) -> Probe {
            let before = unsafe { core::slice::from_raw_parts(self.sim.ptr(), 0x100) }.to_vec();
            let mut p = Probe::new(
                seeded as *const (),
                unsafe { self.sim.ptr().add(4) },
                [self.message.as_mut_ptr() as u32, 0xAABBCCDD, 0xEEFF0011],
            );
            unsafe { p.checked_run(4) };
            assert_eq!(&p.after[1..], &p.args[1..]);
            assert_eq!(unsafe { core::slice::from_raw_parts(self.sim.ptr(), 0x100) }, before);
            p
        }
    }
    fn calls() -> Vec<Call> {
        STATE.with(|s| std::mem::take(&mut s.borrow_mut().calls))
    }
    fn fakes() -> Vec<crate::testing::Fake> {
        [
            (0x006A_27E0, beginning as *const ()),
            (0x006A_4E70, propagation as *const ()),
            (0x006A_7020, idling as *const ()),
            (0x006A_7600, dailies as *const ()),
            (0x006A_9450, monthlies as *const ()),
            (0x006A_8790, tuning as *const ()),
            (0x006A_5CB0, resetting as *const ()),
            (0x006A_5920, effects as *const ()),
            (0x006A_2C80, capacities as *const ()),
            (0x0069_5260, is_occupied as *const ()),
            (0x005F_D390, read_property as *const ()),
        ]
        .into_iter()
        .map(|(a, f)| fake(a, f as usize))
        .collect()
    }

    #[test]
    fn dispatch_adjusts_this_and_orders_begin_presave_daily_monthly_idle_and_tuning_calls() {
        let _f = fakes();
        let mut w = World::new();
        for (id, targets) in [
            (0x6695_6810, vec![(0x006A_27E0, vec![])]),
            (0x26C6_3343, vec![(0x006A_4E70, vec![]), (0x006A_4E70, vec![])]),
            (0x6695_6814, vec![(0x006A_7020, vec![5]), (0x006A_7600, vec![])]),
            (0x6695_6816, vec![(0x006A_9450, vec![])]),
            (0x6695_681A, vec![(0x006A_7020, vec![0x12345678])]),
            (0xCA1B_151C, vec![(0x006A_8790, vec![])]),
        ] {
            w.prepare(id);
            let p = w.run();
            let message = w.message.as_mut_ptr() as u32;
            let mut expected = vec![Call::Virtual(message, 0x10, vec![])];
            if id == 0x6695_681A {
                expected.push(Call::Virtual(message, 0x4C, vec![]));
            }
            expected.extend(
                targets
                    .into_iter()
                    .map(|(a, args)| Call::Game(a, w.sim.ptr() as u32, args)),
            );
            assert_eq!(calls(), expected);
            assert_eq!((p.eax, p.edx), (0xABCD_EF01, 0x9988_7766));
            assert_eq!(p.after, p.args);
        }
        for id in [
            0,
            1,
            u32::MAX,
            0x26C63342,
            0x66956813,
            0x66956815,
            0x66956819,
            0x6695681B,
            0xE8BDA2E8,
        ] {
            w.prepare(id);
            let p = w.run();
            assert_eq!(
                calls(),
                vec![Call::Virtual(w.message.as_mut_ptr() as u32, 0x10, vec![])]
            );
            assert_eq!((p.eax, p.edx), ((id & !0xFF) | 1, 0x7654_0010));
            assert_eq!(p.after, p.args);
        }
    }

    #[test]
    fn regional_messages_read_all_three_payloads_then_reset_without_force() {
        let _f = fakes();
        let mut w = World::new();
        for id in [0x8A0D_15DA, 0xEA0D_163E] {
            w.prepare(id);
            let p = w.run();
            let m = w.message.as_mut_ptr() as u32;
            assert_eq!(
                calls(),
                vec![
                    Call::Virtual(m, 0x10, vec![]),
                    Call::Virtual(m, 0x24, vec![]),
                    Call::Virtual(m, 0x38, vec![]),
                    Call::Virtual(m, 0x4C, vec![]),
                    Call::Game(0x006A_5CB0, w.sim.ptr() as u32, vec![0])
                ]
            );
            assert_eq!((p.eax, p.edx), (0xABCD_EF01, 0x9988_7766));
            assert_eq!(p.after, p.args);
        }
    }

    #[test]
    fn occupancy_transition_uses_only_state_bytes_and_preserves_the_unused_flag_bytes() {
        let _f = fakes();
        let mut percentage = 0x3F80_0000u32;
        let _data = fake_data(0x00B0_D278, (&mut percentage as *mut u32).cast());
        let mut w = World::new();
        for old in [0, 1, 2, 3, 4, 0xFFFF_FF02, 0xABCDEF03, 0xFFFFFFFE] {
            for new in [0, 1, 2, 3, 4, 0xFFFF_FF02, 0xABCDEF03, 0xFFFFFFFE] {
                w.prepare(0xE8BD_A2E7);
                STATE.with(|s| {
                    let mut s = s.borrow_mut();
                    s.old = old;
                    s.new = new;
                });
                let p = w.run();
                let m = w.message.as_mut_ptr() as u32;
                let lot = w.lot.as_mut_ptr() as u32;
                let mut expected = vec![
                    Call::Virtual(m, 0x10, vec![]),
                    Call::Virtual(m, 0x80, vec![]),
                    Call::Virtual(m, 0x24, vec![]),
                    Call::Virtual(m, 0x38, vec![]),
                    Call::Virtual(m, 0x4C, vec![]),
                    Call::Virtual(m, 0x60, vec![]),
                ];
                let old_flag = (2..4).contains(&(old as u8));
                let new_flag = (2..4).contains(&(new as u8));
                if old_flag != new_flag {
                    let flag = 0xA1B2_C300 | u32::from(new_flag);
                    expected.extend([
                        Call::Virtual(lot, 0xC0, vec![]),
                        Call::Virtual(w.config.as_mut_ptr() as u32, 0x0C, vec![]),
                        Call::Virtual(w.exemplar.as_mut_ptr() as u32, 0x18, vec![]),
                        Call::Property(0x99887766, 0x27812844, old & !0xFF),
                        Call::Game(0x0069_5260, 0, vec![new]),
                        Call::Game(0x006A_5920, w.sim.ptr() as u32, vec![0x12345678, flag, percentage]),
                        Call::Game(0x006A_2C80, w.sim.ptr() as u32, vec![0x12345678, 0x89ABCDEF, flag]),
                    ]);
                    assert_eq!(p.after[0], old & !0xFF);
                    assert_eq!((p.eax, p.edx), (0xABCD_EF01, 0x9988_7766));
                } else {
                    assert_eq!(p.after[0], old);
                    assert_eq!((p.eax, p.edx), (1, 0x7654_0060));
                }
                assert_eq!(calls(), expected);
            }
        }
    }

    #[test]
    fn occupancy_handles_missing_kinds_absent_configs_and_the_property_veto() {
        let _f = fakes();
        let mut percentage = 0x3F80_0000u32;
        let _data = fake_data(0x00B0_D278, (&mut percentage as *mut u32).cast());
        let mut w = World::new();
        for (fallback_kind, config_present, veto) in [
            (0, false, 0),
            (77, false, 0),
            (77, true, 1),
            (77, true, 255),
            (77, true, 0),
        ] {
            w.prepare(0xE8BD_A2E7);
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                s.old = 0xAABBCC01;
                s.kind = 0;
                s.fallback_kind = fallback_kind;
                if !config_present {
                    s.config = 0;
                }
                s.property = veto;
            });
            let p = w.run();
            let c = calls();
            let lot = w.lot.as_mut_ptr() as u32;
            assert_eq!(c[6], Call::Virtual(lot, 0xB8, vec![1]));
            if fallback_kind == 0 {
                assert_eq!(c.len(), 7);
                assert_eq!(p.after[0], 0xAABBCC01);
                assert_eq!((p.eax, p.edx), (1, 0x765400B8));
            } else if veto != 0 {
                assert_eq!(c.len(), 11);
                assert_eq!(p.after[0], 0xAABBCC00 | u32::from(veto));
                assert_eq!((p.eax, p.edx), (0xA1B2C301, 0x88776655));
            } else {
                assert_eq!(p.after[0], 0xAABBCC00);
                assert_eq!((p.eax, p.edx), (0xABCDEF01, 0x99887766));
                assert_eq!(
                    &c[c.len() - 3..],
                    &[
                        Call::Game(0x00695260, 0, vec![2]),
                        Call::Game(0x006A5920, w.sim.ptr() as u32, vec![77, 0xA1B2C301, percentage]),
                        Call::Game(0x006A2C80, w.sim.ptr() as u32, vec![77, 0x89ABCDEF, 0xA1B2C301])
                    ]
                );
            }
        }
    }
}
