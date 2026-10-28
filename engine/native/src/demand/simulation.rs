// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Daily demand history and the monthly simulation boundary.

use super::*;

game_fn! { fn propagate(this: *mut u8) -> u64 = "thiscall" 0x006A_4E70; }
game_fn! { fn node_value(map: *mut u8, key: *const u32) -> *mut *mut u8 = "thiscall" 0x0049_7A00; }
game_fn! { fn connections(this: *mut u8) -> u64 = "thiscall" 0x006A_3ED0; }
game_fn! { fn export(this: *mut u8) -> u64 = "thiscall" 0x006A_8FF0; }
game_data! { static history: *mut u8 = 0x00B4_3DC0; }

const DAILY: [(u32, u32); 12] = [
    (0x21010, 0x2A67_A399),
    (0x21020, 0xAA67_A3A0),
    (0x21030, 0x8A67_A3A4),
    (0x23110, 0x0A67_A3A9),
    (0x23120, 0xAA67_A3AD),
    (0x23130, 0xCA67_A3B2),
    (0x23320, 0xAA67_A3B7),
    (0x23330, 0x2A67_A3BC),
    (0x24100, 0xEA67_A3BF),
    (0x24200, 0x0A67_A3C4),
    (0x24300, 0xEA67_A3C8),
    (0x24400, 0x8A67_A3CC),
];

port! {
    /// Propagate demand, then record the twelve active city-total demands using integer truncation.
    #[port(address = 0x006A_7600, owner = "demand", name = "cSC4DemandSimulator::DoDailySimulation", naked)]
    pub unsafe extern "C" fn fc4p_006a7600() {
        core::arch::naked_asm!(
            "push edx", "push eax", "mov edx, esp", "call {body}",
            "mov ecx, dword ptr [esp]", "add esp, 8", "ret",
            body = sym daily,
        )
    }
}

unsafe extern "fastcall" fn daily(this: *mut u8, entry: *mut Entry) -> u64 {
    // SAFETY: the simulator owns its node map; each node and history service supplies the original slots.
    unsafe {
        propagate(this);
        let history = *history();
        let mut result = 0;
        for (key, series) in DAILY {
            let node = *node_value(this.add(0x50), &key);
            // The history slot is selected before calling the node, which may change the service's table.
            let slot = vslot(history, 0x30);
            let value = vcall_st0!(node, 0x30).ftol() as i32;
            let record = core::mem::transmute::<usize, unsafe extern "thiscall" fn(*mut u8, u32, i32) -> u64>(slot);
            result = record(history, series, value);
        }
        (*entry).eax = 0x24400;
        result
    }
}

port! {
    /// Update connection utilization and exported demand, then restart a finished census.
    #[port(address = 0x006A_9450, owner = "demand", name = "cSC4DemandSimulator::DoMonthlySimulation")]
    pub unsafe extern "thiscall" fn fc4p_006a9450(this: *mut u8) -> u64 {
        // SAFETY: all calls and fields belong to the supplied simulator.
        unsafe {
            connections(this);
            let result = export(this);
            let sim = DemandSim::from_ptr(this);
            if sim.census_state() == 3 {
                sim.set_census_state(0);
            }
            result
        }
    }
}

pub(super) const PORTS: &[Port] = &[fc4p_006a7600::PORT, fc4p_006a9450::PORT];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demand::test_support::Probe;
    use crate::testing::{fake, fake_data, GameFpu, Object};
    use std::cell::RefCell;

    #[derive(Debug, PartialEq)]
    enum Call {
        Propagate(u32),
        Lookup(u32, u32),
        Active(u32),
        Record(u32, i32),
        Connections(u32),
        Export(u32),
    }
    #[derive(Default)]
    struct State {
        calls: Vec<Call>,
        node: u32,
        value: f32,
        key: u32,
        state_after_export: u32,
    }
    thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
    extern "thiscall" fn propagation(p: *mut u8) -> u64 {
        STATE.with(|s| s.borrow_mut().calls.push(Call::Propagate(p as u32)));
        u64::MAX
    }
    extern "thiscall" fn lookup(p: *mut u8, key: *const u32) -> *mut u32 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.key = unsafe { *key };
            let k = s.key;
            s.calls.push(Call::Lookup(p as u32, k));
            &mut s.node as *mut u32
        })
    }
    extern "thiscall" fn active(_p: *mut u8) -> f32 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            let k = s.key;
            s.calls.push(Call::Active(k));
            s.value
        })
    }
    extern "thiscall" fn record(_p: *mut u8, series: u32, value: i32) -> u64 {
        STATE.with(|s| s.borrow_mut().calls.push(Call::Record(series, value)));
        0x1234_5678_90AB_CDEF
    }
    extern "thiscall" fn connection(p: *mut u8) -> u64 {
        STATE.with(|s| s.borrow_mut().calls.push(Call::Connections(p as u32)));
        unsafe { DemandSim::from_ptr(p).set_census_state(99) };
        u64::MAX
    }
    extern "thiscall" fn exporting(p: *mut u8) -> u64 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.calls.push(Call::Export(p as u32));
            unsafe { DemandSim::from_ptr(p).set_census_state(s.state_after_export) };
        });
        0x1234_5678_90AB_CDEF
    }

    #[test]
    fn daily_propagates_then_records_every_series_and_preserves_conversion_edges() {
        let _fpu = GameFpu::new();
        let _a = fake(0x006A_4E70, propagation as *const () as usize);
        let _b = fake(0x0049_7A00, lookup as *const () as usize);
        let mut node_table = [0usize; 13];
        node_table[12] = active as *const () as usize;
        let mut node = [node_table.as_ptr() as usize];
        let mut history_table = [0usize; 13];
        history_table[12] = record as *const () as usize;
        let mut history_object = [history_table.as_ptr() as usize];
        let mut history_pointer = history_object.as_mut_ptr().cast::<u8>();
        let _c = fake_data(0x00B4_3DC0, (&mut history_pointer as *mut *mut u8).cast());
        for (value, integer) in [
            (1234.7, 1234),
            (-56.9, -56),
            (2147483648.0, i32::MIN),
            (3.0e9, -1294967296),
            (-3.0e9, 1294967296),
            (1.0e19, 0),
            (f32::INFINITY, 0),
            (f32::NEG_INFINITY, 0),
            (f32::NAN, 0),
        ] {
            STATE.with(|s| {
                *s.borrow_mut() = State {
                    node: node.as_mut_ptr() as u32,
                    value,
                    ..State::default()
                }
            });
            let mut sim = Object::new(0x100);
            let before = unsafe { core::slice::from_raw_parts(sim.ptr(), 0x100) }.to_vec();
            let mut probe = Probe::new(fc4p_006a7600 as *const (), sim.ptr(), [0; 3]);
            unsafe { probe.checked_run(0) };
            assert_eq!((probe.eax, probe.edx, probe.ecx), (0x90AB_CDEF, 0x1234_5678, 0x24400));
            let mut expected = vec![Call::Propagate(sim.ptr() as u32)];
            for (key, series) in DAILY {
                expected.extend([
                    Call::Lookup(unsafe { sim.ptr().add(0x50) } as u32, key),
                    Call::Active(key),
                    Call::Record(series, integer),
                ]);
            }
            assert_eq!(STATE.with(|s| std::mem::take(&mut s.borrow_mut().calls)), expected);
            assert_eq!(unsafe { core::slice::from_raw_parts(sim.ptr(), 0x100) }, before);
        }
    }

    #[test]
    fn monthly_uses_the_state_after_export_and_only_restarts_finished_censuses() {
        let _a = fake(0x006A_3ED0, connection as *const () as usize);
        let _b = fake(0x006A_8FF0, exporting as *const () as usize);
        for state in [0, 1, 2, 3, 4, u32::MAX] {
            STATE.with(|s| {
                *s.borrow_mut() = State {
                    state_after_export: state,
                    ..State::default()
                }
            });
            let mut sim = Object::new(0x100);
            unsafe { DemandSim::from_ptr(sim.ptr()).set_census_state(3) };
            let mut probe = Probe::new(fc4p_006a9450 as *const (), sim.ptr(), [0; 3]);
            unsafe { probe.checked_run(0) };
            assert_eq!((probe.eax, probe.edx), (0x90AB_CDEF, 0x1234_5678));
            assert_eq!(
                unsafe { DemandSim::from_ptr(sim.ptr()).census_state() },
                if state == 3 { 0 } else { state }
            );
            assert_eq!(
                STATE.with(|s| std::mem::take(&mut s.borrow_mut().calls)),
                vec![Call::Connections(sim.ptr() as u32), Call::Export(sim.ptr() as u32)]
            );
        }
    }
}
