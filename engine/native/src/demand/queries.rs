// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Neutral tax and jobs queries, retaining the original x87 evaluation order.

use super::*;
use crate::x87::F80;

game_fn! { fn node_value(map: *mut u8, key: *const u32) -> u64 = "thiscall" 0x0049_7A00; }
game_fn! {
    #[allow(dead_code)]
    fn curve_output(curve: *mut u8, input: f32) -> St0 = "thiscall" 0x005F_4250;
}

// Stored operands remain memory operands so a signaling NaN is handled by the arithmetic itself.
macro_rules! stored_op {
    ($name:ident, $instruction:literal) => {
        fn $name(value: F80, operand: f32) -> F80 {
            let mut output = F80::ZERO;
            unsafe {
                core::arch::asm!(
                    "fld tbyte ptr [{value}]", $instruction, "fstp tbyte ptr [{output}]",
                    value = in(reg) &value, operand = in(reg) &operand, output = in(reg) &mut output,
                    out("st(0)") _,
                );
            }
            output
        }
    };
}
stored_op!(add_float, "fadd dword ptr [{operand}]");
stored_op!(multiply_float, "fmul dword ptr [{operand}]");

unsafe fn supply(object: *mut u8, eax: u32, edx: u32) -> RawCall {
    unsafe {
        let mut call = RawCall::new(Conv::Thiscall, vslot(object, 0x20), object as u32, &[]);
        call.eax = eax;
        call.edx = edx;
        call.take_st0 = 1;
        call_raw(&mut call);
        call
    }
}

port! {
    /// Evaluate the tax curve at (industrial + stored commercial) + stored residential supply.
    /// The curve result is returned unrounded in ST0, together with its EAX and EDX results.
    #[port(address = 0x006A_3DD0, owner = "demand", name = "cSC4DemandSimulator::GetNeutralTaxRate", naked)]
    pub unsafe extern "C" fn fc4p_006a3dd0() {
        core::arch::naked_asm!(
            "push edx", "push eax", "sub esp, 12", "lea edx, [esp + 12]", "call {body}",
            "fld tbyte ptr [esp]", "add esp, 20", "ret",
            body = sym neutral_tax,
        )
    }
}

unsafe extern "fastcall" fn neutral_tax(this: *mut u8, entry: *mut Entry) -> u64 {
    unsafe {
        let sim = DemandSim::from_ptr(this);
        let map = this.add(0x50);
        let mut key = 0x1001;
        let value = node_value(map, &key);
        let object = read_unaligned(value as u32 as *const *mut u8);
        let residential = supply(object, value as u32, read_unaligned(object.cast())).st0.to_f32();
        key = 0x3001;
        let value = node_value(map, &key);
        let object = read_unaligned(value as u32 as *const *mut u8);
        let commercial = supply(object, value as u32, read_unaligned(object.cast())).st0.to_f32();
        key = 0x4001;
        let value = node_value(map, &key);
        let object = read_unaligned(value as u32 as *const *mut u8);
        let industrial = supply(object, value as u32, read_unaligned(object.cast()));
        let curve = sim.neutral_tax_curve();
        let population = add_float(add_float(industrial.st0, commercial), residential).to_f32();
        let args = [population.to_bits()];
        let mut call = RawCall::new(Conv::Thiscall, crate::abi::target(0x005F_4250), curve as u32, &args);
        call.eax = population.to_bits();
        call.edx = industrial.edx;
        call.take_st0 = 1;
        call_raw(&mut call);
        write_unaligned(entry.cast::<u8>().sub(12).cast::<F80>(), call.st0);
        registers(call.eax, call.edx)
    }
}

port! {
    /// Return zero for resident censuses; otherwise truncate supply times ((jobs[3] + jobs[2]) + jobs[1]).
    /// EDX retains its entry value for resident keys and the high conversion word for all other keys.
    #[port(address = 0x006A_3E60, owner = "demand", name = "cSC4DemandSimulator::GetJobsBySensus")]
    pub unsafe extern "fastcall" fn fc4p_006a3e60(this: *mut u8, entry_edx: u32, census_type: u32) -> u64 {
        unsafe {
            if matches!(census_type, 0x1011 | 0x1021 | 0x1031) { return registers(0, entry_edx); }
            let mut jobs = [0u32; 4];
            virtual_call(this, 0x20, read_unaligned(this.cast()), entry_edx, &[jobs.as_mut_ptr() as u32, census_type, 0]);
            let total = add_float(add_float(F80::from_f32(f32::from_bits(jobs[3])), f32::from_bits(jobs[2])), f32::from_bits(jobs[1])).to_f32();
            let node = node_value(this.add(0x50), &census_type);
            let object = read_unaligned(node as u32 as *const *mut u8);
            let value = supply(object, read_unaligned(object.cast()), (node >> 32) as u32).st0;
            multiply_float(value, total).ftol() as u64
        }
    }
}

pub(super) const PORTS: &[Port] = &[fc4p_006a3dd0::PORT, fc4p_006a3e60::PORT];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{fake, GameFpu, Object};
    use std::cell::RefCell;

    #[derive(Debug, PartialEq)]
    enum Call {
        Node(u32, u32),
        Jobs(u32, u32, u32),
        Supply(u32),
        Curve(u32, u32),
    }
    #[repr(C)]
    struct Node {
        table: *const usize,
        value: F80,
    }
    struct State {
        nodes: [u32; 3],
        map: u32,
        jobs: [u32; 3],
        calls: Vec<Call>,
        curve: F80,
    }
    impl Default for State {
        fn default() -> Self {
            Self {
                nodes: [0; 3],
                map: 0,
                jobs: [0; 3],
                calls: Vec::new(),
                curve: F80::ZERO,
            }
        }
    }
    thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }

    extern "thiscall" fn node(map: *mut u8, key: *const u32) -> u64 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            let key = unsafe { *key };
            assert_eq!(map as u32, s.map);
            s.calls.push(Call::Node(map as u32, key));
            let i = match key {
                0x1001 => 0,
                0x3001 => 1,
                0x4001 => 2,
                _ => 0,
            };
            registers(&mut s.nodes[i] as *mut u32 as u32, 0xAABB_CCDD)
        })
    }
    extern "C" fn log_supply(object: u32) {
        STATE.with(|s| s.borrow_mut().calls.push(Call::Supply(object)));
    }
    #[unsafe(naked)]
    unsafe extern "thiscall" fn query_supply(_this: *mut Node) -> f32 {
        core::arch::naked_asm!(
            "push ecx", "call {log}", "pop ecx", "fld tbyte ptr [ecx + 4]",
            "mov eax, 0x12345678", "mov edx, 0x87654321", "ret", log = sym log_supply,
        )
    }
    extern "thiscall" fn jobs(this: *mut u8, output: *mut u32, id: u32, index: u32) -> u32 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.calls.push(Call::Jobs(this as u32, id, index));
            unsafe {
                core::ptr::copy_nonoverlapping(s.jobs.as_ptr(), output.add(1), 3);
            }
        });
        0
    }
    extern "fastcall" fn curve_result(this: *mut u8, output: *mut F80, input: u32) {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.calls.push(Call::Curve(this as u32, input));
            unsafe {
                *output = s.curve;
            }
        });
    }
    #[unsafe(naked)]
    unsafe extern "thiscall" fn curve(_this: *mut u8, _input: f32) -> f32 {
        core::arch::naked_asm!(
            "sub esp, 12", "mov edx, esp", "push dword ptr [esp + 16]", "call {body}",
            "fld tbyte ptr [esp]", "add esp, 12", "mov eax, 0xFFEEDDCC", "mov edx, 0x99887766", "ret 4",
            body = sym curve_result,
        )
    }
    struct World {
        memory: Object,
        nodes: [Node; 3],
        _table: Box<[usize; 9]>,
        _fakes: Vec<crate::testing::Fake>,
    }
    impl World {
        fn new() -> Self {
            let mut memory = Object::new(0xB4);
            let mut table = Box::new([0; 9]);
            table[0x20 / 4] = query_supply as *const () as usize;
            let nodes = core::array::from_fn(|_| Node {
                table: table.as_ptr(),
                value: F80::ZERO,
            });
            let _fakes = vec![
                fake(0x0049_7A00, node as *const () as usize),
                fake(0x005F_4250, curve as *const () as usize),
            ];
            STATE.with(|s| {
                *s.borrow_mut() = State {
                    map: unsafe { memory.ptr().add(0x50) } as u32,
                    ..State::default()
                }
            });
            Self {
                memory,
                nodes,
                _table: table,
                _fakes,
            }
        }
        fn prepare(&mut self) {
            STATE.with(|s| s.borrow_mut().nodes = self.nodes.each_mut().map(|n| n as *mut Node as u32));
        }
    }

    #[test]
    fn neutral_tax_orders_supplies_and_retains_curve_precision_and_registers() {
        let _fpu = GameFpu::new();
        let mut w = World::new();
        w.prepare();
        let sim = unsafe { DemandSim::from_ptr(w.memory.ptr()) };
        sim.set_neutral_tax_curve(0x1234usize as *mut u8);
        // Keep the third supply beyond float32 precision until its first addition.
        w.nodes[0].value = F80::from_f32(16_777_216.0);
        w.nodes[1].value = F80::from_f32(-16_777_216.0);
        w.nodes[2].value = F80::from_f64(1.000_000_000_931_322_6);
        let response = F80::from_f64(8.973_094_000_931_323);
        STATE.with(|s| s.borrow_mut().curve = response);
        let mut call = RawCall::new(
            Conv::Thiscall,
            fc4p_006a3dd0 as *const () as usize,
            sim.as_ptr() as u32,
            &[],
        );
        call.take_st0 = 1;
        unsafe {
            call_raw(&mut call);
        }
        assert_eq!((call.eax, call.edx), (0xFFEE_DDCC, 0x9988_7766));
        assert_eq!(call.st0, response);
        let expected = 1.0f32.to_bits();
        assert_eq!(
            STATE.with(|s| std::mem::take(&mut s.borrow_mut().calls)),
            [
                Call::Node(sim.as_ptr() as u32 + 0x50, 0x1001),
                Call::Supply(&w.nodes[0] as *const Node as u32),
                Call::Node(sim.as_ptr() as u32 + 0x50, 0x3001),
                Call::Supply(&w.nodes[1] as *const Node as u32),
                Call::Node(sim.as_ptr() as u32 + 0x50, 0x4001),
                Call::Supply(&w.nodes[2] as *const Node as u32),
                Call::Curve(0x1234, expected)
            ]
        );
    }

    #[test]
    fn neutral_tax_uses_the_core_population_rounding_case() {
        let _fpu = GameFpu::new();
        let mut w = World::new();
        w.prepare();
        for (node, bits) in w.nodes.iter_mut().zip([0x46D4_8E00, 0x4520_97AE, 0x4625_5C67]) {
            node.value = F80::from_f32(f32::from_bits(bits));
        }
        let response = F80::from_f32(f32::from_bits(0x410F_91CB));
        STATE.with(|s| s.borrow_mut().curve = response);
        let sim = unsafe { DemandSim::from_ptr(w.memory.ptr()) };
        sim.set_neutral_tax_curve(0x1234usize as *mut u8);
        let mut call = RawCall::new(
            Conv::Thiscall,
            fc4p_006a3dd0 as *const () as usize,
            sim.as_ptr() as u32,
            &[],
        );
        call.take_st0 = 1;
        unsafe {
            call_raw(&mut call);
        }
        assert_eq!(call.st0.to_f32().to_bits(), 0x410F_91CB);
        STATE.with(|s| assert_eq!(s.borrow().calls.last(), Some(&Call::Curve(0x1234, 0x471D_A794))));
    }

    #[test]
    fn jobs_sum_last_output_first_and_convert_negative_large_and_nonfinite_products() {
        let _fpu = GameFpu::new();
        let mut w = World::new();
        w.prepare();
        let table = [0, 0, 0, 0, 0, 0, 0, 0, jobs as *const () as usize];
        let sim = unsafe { DemandSim::from_ptr(w.memory.ptr()) };
        sim.set_vtable(table.as_ptr() as u32);
        STATE.with(|s| s.borrow_mut().jobs = [0x3C23_D70A, 0x3CF5_C28F, 0x3F75_C28F]);
        for (supply, expected) in [
            (1000.0, 999i64),
            (-3.7 / 0.999_999_94, -3),
            (3.0e9 / 0.999_999_94, 3_000_000_000),
            (f32::NAN, i64::MIN),
            (f32::INFINITY, i64::MIN),
            (f32::NEG_INFINITY, i64::MIN),
        ] {
            w.nodes[0].value = F80::from_f32(supply);
            assert_eq!(
                unsafe { fc4p_006a3e60(sim.as_ptr(), 0x1122_3344, 0x3111) },
                expected as u64
            );
            assert_eq!(
                STATE.with(|s| std::mem::take(&mut s.borrow_mut().calls)),
                [
                    Call::Jobs(sim.as_ptr() as u32, 0x3111, 0),
                    Call::Node(sim.as_ptr() as u32 + 0x50, 0x3111),
                    Call::Supply(&w.nodes[0] as *const Node as u32)
                ]
            );
        }
        for id in [0x1011, 0x1021, 0x1031] {
            assert_eq!(
                unsafe { fc4p_006a3e60(sim.as_ptr(), 0x1122_3344, id) },
                0x1122_3344_0000_0000
            );
        }
        assert!(STATE.with(|s| s.borrow().calls.is_empty()));
    }
}
