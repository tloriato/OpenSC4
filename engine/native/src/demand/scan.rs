// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! One-cell population and occupied-job census.

use super::queries::{add_float, multiply_float};
use super::*;
use crate::x87::F80;

game_fn! {
    #[allow(dead_code)]
    fn census_value(map: *mut u8, key: *const u32) -> *mut f32 = "thiscall" 0x006A_3170;
}
game_data! { static percent: f32 = 0x00A8_23A4; }
game_data! { static zero: f32 = 0x00A8_1054; }
game_data! { static one: f32 = 0x00A8_1228; }
game_data! { static education_scale: f32 = 0x00AB_0E84; }

/// This caller uses the ECX left by its final callee as well as the usual return registers.
#[repr(C)]
struct Called {
    raw: RawCall,
    ecx: u32,
}
const _: () = assert!(core::mem::offset_of!(Called, ecx) == 44);

unsafe fn invoke(target: usize, this: *mut u8, args: &[u32], floating: bool) -> Called {
    let mut call = Called {
        raw: RawCall::new(Conv::Thiscall, target, this as u32, args),
        ecx: 0,
    };
    call.raw.take_st0 = u32::from(floating);
    unsafe {
        core::arch::asm!(
            "mov ecx, dword ptr [edi + 20]", "mov eax, dword ptr [edi + 16]",
            "2:", "test ecx, ecx", "jz 3f", "dec ecx", "push dword ptr [eax + ecx*4]", "jmp 2b",
            "3:", "mov eax, dword ptr [edi + 4]", "mov ecx, dword ptr [edi + 8]", "mov edx, dword ptr [edi + 12]",
            "call dword ptr [edi]", "mov dword ptr [edi + 4], eax", "mov dword ptr [edi + 12], edx",
            "mov dword ptr [edi + 44], ecx", "cmp dword ptr [edi + 28], 0", "je 4f",
            "fstp tbyte ptr [edi + 32]", "4:",
            in("edi") &mut call, clobber_abi("C"),
        );
    }
    call
}

unsafe fn method(this: *mut u8, slot: usize, args: &[u32], floating: bool) -> Called {
    unsafe { invoke(vslot(this, slot), this, args, floating) }
}

unsafe fn finish(entry: &mut Entry, call: Called) -> u64 {
    entry.eax = call.ecx;
    registers(call.raw.eax, call.raw.edx)
}

unsafe fn pointer(this: *mut u8, offset: usize) -> *mut u8 {
    unsafe { read_unaligned(this.add(offset).cast()) }
}

port! {
    /// Census one lot. Residential lots add workforce and resident counts; other supported purposes add occupied
    /// capacity under developer+1. Wealth is unrestricted, capacities use AX, and education conversion uses AL.
    #[port(address = 0x006A_5A00, owner = "demand", name = "cSC4DemandSimulator::PerformCensus", naked)]
    pub unsafe extern "C" fn fc4p_006a5a00() {
        core::arch::naked_asm!(
            "push edx", "push eax", "mov edx, esp", "call {body}", "mov ecx, dword ptr [esp]", "add esp, 8", "ret 8",
            body = sym perform,
        )
    }
}

unsafe extern "fastcall" fn perform(this: *mut u8, entry: *mut Entry) -> u64 {
    unsafe {
        let entry = &mut *entry;
        let (x, z) = (entry.arg(0), entry.arg(1));
        let call = method(pointer(this, 0x40), 0x18, &[x, z, 1], false);
        let lot = call.raw.eax as *mut u8;
        if lot.is_null() {
            return finish(entry, call);
        }
        let call = method(lot, 0x98, &[], false);
        if call.raw.eax as u8 == 0 {
            return finish(entry, call);
        }
        let call = method(lot, 0xB8, &[1], false);
        if call.raw.eax == 0 {
            return finish(entry, call);
        }
        let call = method(pointer(this, 0x3C), 0x9C, &[call.raw.eax], false);
        let profile = call.raw.eax as *mut u8;
        entry.put(1, profile as u32);
        let purpose = read_unaligned(profile.cast::<u32>());
        if !matches!(purpose, 1 | 2 | 3 | 5 | 6 | 7 | 8) {
            return finish(entry, call);
        }
        let residential_branch = purpose == 1;
        let call = method(lot, 0xA4, &[], false);
        let wealth = call.raw.eax;
        let purpose = read_unaligned((entry.arg(1) as *const u8).cast::<u32>());
        let call = method(pointer(this, 0x3C), 0x78, &[purpose, wealth], false);
        let developer = call.raw.eax;
        entry.put(0, developer);
        let call = method(pointer(this, 0x3C), 0x80, &[developer], false);
        let call = method(call.raw.eax as *mut u8, 0x34, &[], false);
        let call = method(call.raw.eax as *mut u8, 0x1C, &[x, z], false);
        let desirability = i32::from(call.raw.eax as i16);
        entry.put(1, desirability as u32);
        let occupancy = multiply_float(F80::from_i32(desirability), read_unaligned(percent()));
        entry.put(1, occupancy.to_f32().to_bits());
        if occupancy.compare(F80::from_f32(read_unaligned(zero()))).less() {
            entry.put(1, 0);
        } else {
            let comparison = F80::from_f32(f32::from_bits(entry.arg(1))).compare(F80::from_f32(read_unaligned(one())));
            if !comparison.less() && !comparison.equal() && !comparison.unordered() {
                entry.put(1, 1.0f32.to_bits());
            }
        }
        let call = method(lot, 0xC8, &[entry.arg(0), 0], false);
        entry.put(0, u32::from(call.raw.eax as u16));
        let occupied = multiply_float(F80::from_i32(entry.arg(0) as i32), f32::from_bits(entry.arg(1)));
        if residential_branch {
            let mut workforce_record = [0u32; 4];
            let residential = pointer(this, 0x4C);
            entry.put(1, occupied.to_f32().to_bits());
            let call = method(residential, 0x78, &[workforce_record.as_mut_ptr() as u32, x, z], false);
            let workers = multiply_float(
                F80::from_f32(f32::from_bits(entry.arg(1))),
                read_unaligned(call.raw.eax as *const f32),
            )
            .to_f32();
            let call = method(pointer(this, 0x4C), 0x8C, &[x, z], true);
            let education = u32::from(call.raw.st0.ftol() as u8);
            entry.put(0, education);
            let level = add_float(
                multiply_float(F80::from_i32(education as i32), read_unaligned(education_scale())),
                read_unaligned(one()),
            )
            .ftol() as i32;
            let level = level.clamp(1, 4) as u32;
            let key = wealth.wrapping_add(0x210).wrapping_shl(4).wrapping_add(level);
            entry.put(0, key);
            let map = this.add(0x1C);
            let call = invoke(
                crate::abi::target(0x006A_3170),
                map,
                &[entry.args.as_ptr() as u32],
                false,
            );
            let value = call.raw.eax as *mut f32;
            write_unaligned(value, add_float(F80::from_f32(workers), read_unaligned(value)).to_f32());
            entry.put(0, wealth.wrapping_shl(4).wrapping_add(0x1001));
            let call = invoke(
                crate::abi::target(0x006A_3170),
                map,
                &[entry.args.as_ptr() as u32],
                false,
            );
            let value = call.raw.eax as *mut f32;
            write_unaligned(
                value,
                add_float(F80::from_f32(f32::from_bits(entry.arg(1))), read_unaligned(value)).to_f32(),
            );
            finish(entry, call)
        } else {
            entry.put(1, developer.wrapping_add(1));
            entry.put(0, occupied.to_f32().to_bits());
            let call = invoke(
                crate::abi::target(0x006A_3170),
                this.add(0x1C),
                &[entry.args.as_ptr().add(1) as u32],
                false,
            );
            let value = call.raw.eax as *mut f32;
            write_unaligned(
                value,
                add_float(F80::from_f32(f32::from_bits(entry.arg(0))), read_unaligned(value)).to_f32(),
            );
            finish(entry, call)
        }
    }
}

pub(super) const PORTS: &[Port] = &[fc4p_006a5a00::PORT];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demand::test_support::Probe;
    use crate::testing::{fake, fake_data, Fake, GameFpu, Object};
    use std::{cell::RefCell, collections::BTreeMap};

    #[derive(Debug, PartialEq)]
    enum Call {
        Slot(u32, u32, Vec<u32>),
        Census(u32, u32),
    }
    struct State {
        objects: [u32; 6],
        profile: u32,
        occupied: u32,
        building: u32,
        wealth: u32,
        desirability: u32,
        capacity: u32,
        workforce: f32,
        education: f32,
        values: BTreeMap<u32, f32>,
        calls: Vec<Call>,
    }
    impl Default for State {
        fn default() -> Self {
            Self {
                objects: [0; 6],
                profile: 0,
                occupied: 1,
                building: 5,
                wealth: 1,
                desirability: 50,
                capacity: 0xABCD_0064,
                workforce: 0.4,
                education: 120.0,
                values: BTreeMap::new(),
                calls: Vec::new(),
            }
        }
    }
    thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
    fn log(this: *mut u8, slot: u32, args: Vec<u32>) {
        let tag = unsafe { read_unaligned(this.add(4).cast::<u32>()) };
        STATE.with(|s| s.borrow_mut().calls.push(Call::Slot(tag, slot, args)));
    }
    extern "thiscall" fn lot(this: *mut u8, x: u32, z: u32, mode: u32) -> u32 {
        log(this, 0x18, vec![x, z, mode]);
        STATE.with(|s| s.borrow().objects[1])
    }
    extern "thiscall" fn occupied(this: *mut u8) -> u32 {
        log(this, 0x98, vec![]);
        STATE.with(|s| s.borrow().occupied)
    }
    extern "thiscall" fn building(this: *mut u8, mode: u32) -> u32 {
        log(this, 0xB8, vec![mode]);
        STATE.with(|s| s.borrow().building)
    }
    extern "thiscall" fn profile(this: *mut u8, id: u32) -> u32 {
        log(this, 0x9C, vec![id]);
        STATE.with(|s| s.borrow().profile)
    }
    extern "thiscall" fn wealth(this: *mut u8) -> u32 {
        log(this, 0xA4, vec![]);
        STATE.with(|s| s.borrow().wealth)
    }
    extern "thiscall" fn developer(this: *mut u8, purpose: u32, wealth: u32) -> u32 {
        log(this, 0x78, vec![purpose, wealth]);
        0x3110
    }
    extern "thiscall" fn growth(this: *mut u8, developer: u32) -> u32 {
        log(this, 0x80, vec![developer]);
        STATE.with(|s| s.borrow().objects[3])
    }
    extern "thiscall" fn grid(this: *mut u8) -> u32 {
        log(this, 0x34, vec![]);
        STATE.with(|s| s.borrow().objects[4])
    }
    extern "thiscall" fn desirability(this: *mut u8, x: u32, z: u32) -> u32 {
        log(this, 0x1C, vec![x, z]);
        STATE.with(|s| s.borrow().desirability)
    }
    extern "thiscall" fn capacity(this: *mut u8, developer: u32, mode: u32) -> u32 {
        log(this, 0xC8, vec![developer, mode]);
        STATE.with(|s| s.borrow().capacity)
    }
    extern "thiscall" fn workforce(this: *mut u8, _record: *mut u32, x: u32, z: u32) -> *const f32 {
        log(this, 0x78, vec![x, z]);
        STATE.with(|s| &s.borrow().workforce as *const f32)
    }
    extern "thiscall" fn education(this: *mut u8, x: u32, z: u32) -> f32 {
        log(this, 0x8C, vec![x, z]);
        STATE.with(|s| s.borrow().education)
    }
    extern "thiscall" fn value_body(map: *mut u8, key: *const u32) -> u64 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            let key = unsafe { *key };
            s.calls.push(Call::Census(map as u32, key));
            registers(s.values.entry(key).or_insert(0.0) as *mut f32 as u32, 0xAABB_CCDD)
        })
    }
    #[unsafe(naked)]
    unsafe extern "thiscall" fn value(_map: *mut u8, _key: *const u32) -> u64 {
        core::arch::naked_asm!("push dword ptr [esp + 4]", "call {body}","mov ecx, 0xCAFEBABE","ret 4",body=sym value_body)
    }
    #[unsafe(naked)]
    unsafe extern "thiscall" fn absent(_this: *mut u8, _x: u32, _z: u32, _mode: u32) -> u32 {
        core::arch::naked_asm!("xor eax,eax", "mov ecx,0x12345678", "ret 12")
    }
    struct World {
        memory: Object,
        objects: Vec<[u32; 2]>,
        tables: Vec<[usize; 52]>,
        profile: Box<u32>,
        _constants: Box<[f32; 4]>,
        _guards: Vec<Fake>,
    }
    impl World {
        fn new(purpose: u32) -> Self {
            STATE.with(|s| *s.borrow_mut() = State::default());
            let mut tables = vec![[0; 52]; 6];
            for (tag, slot, f) in [
                (0, 0x18, lot as *const ()),
                (1, 0x98, occupied as *const ()),
                (1, 0xB8, building as *const ()),
                (1, 0xA4, wealth as *const ()),
                (1, 0xC8, capacity as *const ()),
                (2, 0x9C, profile as *const ()),
                (2, 0x78, developer as *const ()),
                (2, 0x80, growth as *const ()),
                (3, 0x34, grid as *const ()),
                (4, 0x1C, desirability as *const ()),
                (5, 0x78, workforce as *const ()),
                (5, 0x8C, education as *const ()),
            ] {
                tables[tag][slot / 4] = f as usize;
            }
            let mut objects: Vec<_> = tables
                .iter()
                .enumerate()
                .map(|(i, t)| [t.as_ptr() as u32, i as u32])
                .collect();
            let mut profile = Box::new(purpose);
            let mut memory = Object::new(0xB4);
            unsafe {
                write_unaligned(memory.ptr().add(0x40).cast::<u32>(), objects[0].as_mut_ptr() as u32);
                write_unaligned(memory.ptr().add(0x3C).cast::<u32>(), objects[2].as_mut_ptr() as u32);
                write_unaligned(memory.ptr().add(0x4C).cast::<u32>(), objects[5].as_mut_ptr() as u32);
            }
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                s.profile = &mut *profile as *mut u32 as u32;
                s.objects = core::array::from_fn(|i| objects[i].as_mut_ptr() as u32);
            });
            let mut constants = Box::new([0.01, 0.0, 1.0, f32::from_bits(0x3CA3_8353)]);
            let mut guards = vec![fake(0x006A_3170, value as *const () as usize)];
            for (i, address) in [0x00A8_23A4, 0x00A8_1054, 0x00A8_1228, 0x00AB_0E84]
                .into_iter()
                .enumerate()
            {
                guards.push(fake_data(address, (&mut constants[i] as *mut f32).cast()));
            }
            Self {
                memory,
                objects,
                tables,
                profile,
                _constants: constants,
                _guards: guards,
            }
        }
        fn run(&mut self) -> Probe {
            let mut call = Probe::new(fc4p_006a5a00 as *const (), self.memory.ptr(), [7, 9, 0xDEAD_BEEF]);
            unsafe {
                call.run();
            }
            call
        }
        fn base_calls(&self) -> Vec<Call> {
            vec![
                Call::Slot(0, 0x18, vec![7, 9, 1]),
                Call::Slot(1, 0x98, vec![]),
                Call::Slot(1, 0xB8, vec![1]),
                Call::Slot(2, 0x9C, vec![5]),
                Call::Slot(1, 0xA4, vec![]),
                Call::Slot(2, 0x78, vec![*self.profile, 1]),
                Call::Slot(2, 0x80, vec![0x3110]),
                Call::Slot(3, 0x34, vec![]),
                Call::Slot(4, 0x1C, vec![7, 9]),
                Call::Slot(1, 0xC8, vec![0x3110, 0]),
            ]
        }
    }

    #[test]
    fn core_residential_census_reads_purpose_only_and_adds_workforce_before_residents() {
        let _fpu = GameFpu::new();
        let mut w = World::new(1);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.values.insert(0x2113, 10.0);
            s.values.insert(0x1011, 100.0);
        });
        let call = w.run();
        let map = w.memory.ptr() as u32 + 0x1C;
        let mut expected = w.base_calls();
        expected.extend([
            Call::Slot(5, 0x78, vec![7, 9]),
            Call::Slot(5, 0x8C, vec![7, 9]),
            Call::Census(map, 0x2113),
            Call::Census(map, 0x1011),
        ]);
        STATE.with(|s| {
            let s = s.borrow();
            assert_eq!(s.calls, expected);
            assert_eq!(s.values[&0x2113], 30.0);
            assert_eq!(s.values[&0x1011], 150.0);
        });
        assert_eq!(call.ecx, 0xCAFE_BABE);
        assert_eq!(call.after, [0x1011, 50.0f32.to_bits(), 0xDEAD_BEEF]);
        assert_eq!(w.objects.len(), 6);
    }

    #[test]
    fn business_census_clamps_signed_desirability_and_uses_unsigned_capacity_low_word() {
        let _fpu = GameFpu::new();
        for (d, expected) in [(0xBEEF_FFFF, 0.0f32), (125, 100.0), (33, 33.0)] {
            let mut w = World::new(2);
            STATE.with(|s| s.borrow_mut().desirability = d);
            let call = w.run();
            let mut calls = w.base_calls();
            calls.push(Call::Census(w.memory.ptr() as u32 + 0x1C, 0x3111));
            STATE.with(|s| {
                let s = s.borrow();
                assert_eq!(s.calls, calls);
                assert_eq!(s.values[&0x3111], expected);
            });
            assert_eq!(call.ecx, 0xCAFE_BABE);
            assert_eq!(call.after, [expected.to_bits(), 0x3111, 0xDEAD_BEEF]);
        }
    }

    #[test]
    fn absent_lots_and_unsupported_purposes_stop_without_census_calls() {
        let mut w = World::new(1);
        w.tables[0][0x18 / 4] = absent as *const () as usize;
        let call = w.run();
        assert_eq!(call.ecx, 0x1234_5678);
        assert_eq!(call.after, [7, 9, 0xDEAD_BEEF]);
        assert!(STATE.with(|s| s.borrow().calls.is_empty()));
        for purpose in [0, 4, 9, u32::MAX] {
            let mut w = World::new(purpose);
            let call = w.run();
            assert_eq!(call.after, [7, &mut *w.profile as *mut u32 as u32, 0xDEAD_BEEF]);
            assert_eq!(STATE.with(|s| s.borrow().calls.len()), 4);
            assert!(STATE.with(|s| s.borrow().values.is_empty()));
        }
        for (valid, building, expected) in [(0x100, 5, 2), (1, 0, 3)] {
            let mut w = World::new(1);
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                s.occupied = valid;
                s.building = building;
            });
            w.run();
            assert_eq!(STATE.with(|s| s.borrow().calls.len()), expected);
        }
    }

    #[test]
    fn education_wraps_through_al_and_wealth_keys_use_wrapping_arithmetic() {
        let _fpu = GameFpu::new();
        for (education, wealth, level) in [(-1.0, 1u32, 4u32), (256.0, u32::MAX, 1), (f32::NAN, 1, 1)] {
            let mut w = World::new(1);
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                s.education = education;
                s.wealth = wealth;
            });
            let call = w.run();
            let worker = wealth.wrapping_add(0x210).wrapping_shl(4).wrapping_add(level);
            let resident = wealth.wrapping_shl(4).wrapping_add(0x1001);
            STATE.with(|s| {
                let s = s.borrow();
                assert!(s.values.contains_key(&worker));
                assert!(s.values.contains_key(&resident));
            });
            assert_eq!(call.after[0], resident);
        }
    }
}
