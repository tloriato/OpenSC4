// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! The local population summary written into the caller's map.

use super::*;

game_fn! { fn clear_subtree(map: *mut u8, root: *mut u8) -> u64 = "thiscall" 0x004A_79A0; }
game_fn! { fn map_value(map: *mut u8, key: *const u32) -> u64 = "thiscall" 0x0049_7A00; }

layout! {
    /// The caller owns the header and the count; the header survives clearing.
    struct SummaryMap {
        0x00 => header / set_header: *mut u8,
        0x04 => count / set_count: u32,
    }
}

port! {
    /// Clear the output map, then visit demand nodes in key order. Region use 1 contributes truncated
    /// supply; region use 2 contributes truncated demand. The argument word becomes the last visited key.
    /// Return the source map's header in EAX and retain the last conversion or region getter's EDX.
    #[port(address = 0x006A_5F80, owner = "demand", name = "cSC4DemandSimulator::GetLocalPopulationSummary", naked)]
    pub unsafe extern "C" fn fc4p_006a5f80() {
        core::arch::naked_asm!(
            "push edx", "push eax", "mov edx, esp", "call {body}",
            "mov ecx, dword ptr [esp]", "add esp, 8", "ret 4",
            body = sym summary,
        )
    }
}

unsafe extern "fastcall" fn summary(this: *mut u8, entry: *mut Entry) -> u64 {
    // SAFETY: the game supplies its simulator and the caller-owned map.
    unsafe {
        let entry = &mut *entry;
        entry.eax = this as u32;
        let sim = DemandSim::from_ptr(this);
        let output = SummaryMap::from_ptr(entry.arg(0) as *mut u8);
        let mut edx = entry.edx;
        if output.count() != 0 {
            clear_subtree(output.as_ptr(), TreeNode::from_ptr(output.header()).parent());
            TreeNode::from_ptr(output.header()).set_left(output.header());
            edx = output.header() as u32;
            TreeNode::from_ptr(edx as *mut u8).set_parent(core::ptr::null_mut());
            TreeNode::from_ptr(output.header()).set_right(output.header());
            output.set_count(0);
        }
        let mut node = TreeNode::from_ptr(sim.nodes()).left();
        while node != sim.nodes() {
            let tree = TreeNode::from_ptr(node);
            let object = tree.value() as *mut u8;
            entry.put(0, tree.key());
            let kind = virtual_call(object, 0x70, tree.key(), read_unaligned(object.cast()), &[]);
            edx = kind.edx;
            if kind.eax == 1 || kind.eax == 2 {
                let target = map_value(output.as_ptr(), entry.args.as_ptr()) as u32 as *mut u32;
                let mut value = RawCall::new(
                    Conv::Thiscall,
                    vslot(object, if kind.eax == 1 { 0x20 } else { 0x24 }),
                    object as u32,
                    &[],
                );
                value.eax = target as u32;
                value.edx = read_unaligned(object.cast());
                value.take_st0 = 1;
                call_raw(&mut value);
                let integer = value.st0.ftol() as u64;
                write_unaligned(target, integer as u32);
                edx = (integer >> 32) as u32;
            }
            node = successor(node);
        }
        registers(sim.nodes() as u32, edx)
    }
}

pub(super) const PORTS: &[Port] = &[fc4p_006a5f80::PORT];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demand::{interface::tests::Map, test_support::Probe};
    use crate::testing::{fake, GameFpu, Object};
    use std::cell::RefCell;

    #[derive(Debug, PartialEq)]
    enum Call {
        Clear(u32, u32),
        Kind(u32),
        Value(u32, u32),
        Supply(u32),
        Demand(u32),
    }
    #[derive(Default)]
    // Map value addresses must survive later insertions.
    #[allow(clippy::vec_box)]
    struct State {
        calls: Vec<Call>,
        values: Vec<Box<u32>>,
    }
    thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
    #[repr(C)]
    struct Node {
        table: *const usize,
        kind: u32,
        value: f32,
        id: u32,
    }
    extern "thiscall" fn clear(map: *mut u8, root: *mut u8) -> u64 {
        STATE.with(|s| s.borrow_mut().calls.push(Call::Clear(map as u32, root as u32)));
        0xFFFF_FFFF_FFFF_FFFF
    }
    extern "thiscall" fn value(map: *mut u8, key: *const u32) -> *mut u32 {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.calls.push(Call::Value(map as u32, unsafe { *key }));
            s.values.push(Box::new(0xDEAD_BEEF));
            s.values.last_mut().unwrap().as_mut() as *mut u32
        })
    }
    extern "thiscall" fn kind(p: *mut u8) -> u64 {
        let n = unsafe { &*p.cast::<Node>() };
        STATE.with(|s| s.borrow_mut().calls.push(Call::Kind(n.id)));
        registers(n.kind, 0x9988_7766)
    }
    extern "thiscall" fn supply(p: *mut u8) -> f32 {
        let n = unsafe { &*p.cast::<Node>() };
        STATE.with(|s| s.borrow_mut().calls.push(Call::Supply(n.id)));
        n.value
    }
    extern "thiscall" fn demand(p: *mut u8) -> f32 {
        let n = unsafe { &*p.cast::<Node>() };
        STATE.with(|s| s.borrow_mut().calls.push(Call::Demand(n.id)));
        n.value
    }

    #[test]
    fn summary_clears_the_owned_map_selects_region_use_and_truncates_with_wrapping() {
        let _fpu = GameFpu::new();
        let _a = fake(0x004A_79A0, clear as *const () as usize);
        let _b = fake(0x0049_7A00, value as *const () as usize);
        let mut table = [0; 29];
        table[0x20 / 4] = supply as *const () as usize;
        table[0x24 / 4] = demand as *const () as usize;
        table[0x70 / 4] = kind as *const () as usize;
        let cases = [
            (1234.7, 1234),
            (-56.9, -56),
            (2_147_483_648.0, i32::MIN),
            (3.0e9, -1_294_967_296),
            (-3.0e9, 1_294_967_296),
            (1.0e19, 0),
            (f32::INFINITY, 0),
            (f32::NEG_INFINITY, 0),
            (f32::NAN, 0),
        ];
        for (float, integer) in cases {
            for last_kind in [0, 1, 2, 3, 0x101] {
                STATE.with(|s| *s.borrow_mut() = State::default());
                let mut nodes: Vec<_> = [1, 2, last_kind]
                    .into_iter()
                    .enumerate()
                    .map(|(id, kind)| {
                        Box::new(Node {
                            table: table.as_ptr(),
                            kind,
                            value: float,
                            id: id as u32,
                        })
                    })
                    .collect();
                let values: Vec<_> = nodes
                    .iter_mut()
                    .enumerate()
                    .map(|(i, n)| (10 + i as u32, n.as_mut() as *mut Node as u32))
                    .collect();
                let mut source = Map::new(&values);
                let mut memory = Object::new(0xB4);
                let sim = unsafe { DemandSim::from_ptr(memory.ptr()) };
                sim.set_nodes(source.header());
                let mut header = [0xA5A5_A5A5u32; 6];
                header[1] = 0x1234;
                let mut output = [header.as_mut_ptr() as u32, 7, 0xCAFEBABE];
                let output_ptr = output.as_mut_ptr() as u32;
                let before = unsafe { core::slice::from_raw_parts(sim.as_ptr(), 0xB4).to_vec() };
                let mut call = Probe::new(fc4p_006a5f80 as *const (), sim.as_ptr(), [output_ptr, 77, 88]);
                unsafe {
                    call.run();
                }
                assert_eq!(call.after, [12, 77, 88]);
                assert_eq!(call.eax, source.header() as u32);
                assert_eq!(
                    call.edx,
                    if last_kind == 1 || last_kind == 2 {
                        (crate::x87::F80::from_f32(float).ftol() as u64 >> 32) as u32
                    } else {
                        0x9988_7766
                    }
                );
                assert_eq!(call.ecx, sim.as_ptr() as u32);
                assert_eq!(output, [header.as_mut_ptr() as u32, 0, 0xCAFEBABE]);
                assert_eq!(header, [0xA5A5_A5A5, 0, output[0], output[0], 0xA5A5_A5A5, 0xA5A5_A5A5]);
                assert_eq!(unsafe { core::slice::from_raw_parts(sim.as_ptr(), 0xB4) }, before);
                let mut expected = vec![Call::Clear(output_ptr, 0x1234)];
                for (i, k) in [1, 2, last_kind].into_iter().enumerate() {
                    expected.push(Call::Kind(i as u32));
                    if k == 1 || k == 2 {
                        expected.push(Call::Value(output_ptr, 10 + i as u32));
                        expected.push(if k == 1 {
                            Call::Supply(i as u32)
                        } else {
                            Call::Demand(i as u32)
                        });
                    }
                }
                STATE.with(|s| {
                    let s = s.borrow();
                    assert_eq!(s.calls, expected);
                    assert!(s.values.iter().all(|v| **v as i32 == integer));
                });
            }
        }
    }

    #[test]
    fn empty_sources_preserve_the_argument_and_edx_without_clearing_an_empty_output() {
        let mut source = Map::new(&[]);
        let mut memory = Object::new(0xB4);
        let sim = unsafe { DemandSim::from_ptr(memory.ptr()) };
        sim.set_nodes(source.header());
        let mut output = [0x1234u32, 0];
        let args = [output.as_mut_ptr() as u32, 77, 88];
        let mut call = Probe::new(fc4p_006a5f80 as *const (), sim.as_ptr(), args);
        unsafe {
            call.run();
        }
        assert_eq!(call.after, args);
        assert_eq!((call.eax, call.edx), (source.header() as u32, 0x5566_7788));
        assert_eq!(call.ecx, sim.as_ptr() as u32);
        assert_eq!(output, [0x1234, 0]);
    }
}
