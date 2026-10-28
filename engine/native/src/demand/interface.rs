// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Class identity, interface lookup and demand-map access.

use super::*;

game_fn! {
    fn find_node(map: *mut u8, output: *mut u32, key: *const u32) -> u64 = "thiscall" 0x0049_7520;
}

port! {
    /// Return the class identifier, retaining every other register.
    #[port(address = 0x006A_2300, owner = "demand", name = "cSC4DemandSimulator::GetGZCLSID", naked)]
    pub unsafe extern "C" fn fc4p_006a2300() {
        core::arch::naked_asm!("mov eax, 0x2990bff2", "ret")
    }
}

port! {
    /// Resolve the primary, message-target or serializable interface and retain it through primary AddRef.
    /// Failure leaves the output untouched. AL is the answer; upper EAX bytes retain the lookup or AddRef result.
    #[port(address = 0x006A_2310, owner = "demand", name = "cSC4DemandSimulator::QueryInterface")]
    pub unsafe extern "thiscall" fn fc4p_006a2310(this: *mut u8, iid: u32, output: *mut *mut u8) -> u32 {
        unsafe {
            let offset = match iid {
                1 | 0x4796_7E16 => 0,
                0x4522_94AA => 4,
                0xE4FD_A3D4 => 8,
                _ => {
                    let eax = if iid <= 0x4796_7E16 { iid.wrapping_sub(0x4522_94AA) } else { iid };
                    return eax & !0xFF;
                }
            };
            write_unaligned(output, if this.is_null() { this } else { this.add(offset) });
            let table = read_unaligned(this.cast::<u32>());
            let call = virtual_call(this, 4, table, output as u32, &[]);
            (call.eax & !0xFF) | 1
        }
    }
}

port! {
    /// Notify every demand in ascending key order, returning the map header left by iteration.
    #[port(address = 0x006A_27E0, owner = "demand", name = "cSC4DemandSimulator::SimulatorBegin")]
    pub unsafe extern "thiscall" fn fc4p_006a27e0(this: *mut u8) -> u32 {
        unsafe {
            let sim = DemandSim::from_ptr(this);
            let header = sim.nodes();
            let mut node = TreeNode::from_ptr(header).left();
            if node == header { return header as u32; }
            loop {
                let object = TreeNode::from_ptr(node).value() as *mut u8;
                vcall!(object, 0x14 => fn() -> u32);
                node = successor(node);
                if node == sim.nodes() { return node as u32; }
            }
        }
    }
}

port! {
    /// Look up the wrapping sum of demand ID and index. The entry retains access to the two scratch argument words.
    #[port(address = 0x006A_2960, owner = "demand", name = "cSC4DemandSimulator::GetDemand", naked)]
    pub unsafe extern "C" fn fc4p_006a2960() {
        core::arch::naked_asm!(
            "push edx", "push eax", "mov edx, esp", "call {body}", "add esp, 8", "ret 8",
            body = sym get_demand,
        )
    }
}

unsafe extern "fastcall" fn get_demand(this: *mut u8, entry: *mut Entry) -> u64 {
    unsafe {
        let sim = DemandSim::from_ptr(this);
        let entry = &mut *entry;
        if sim.initialized() == 0 {
            vcall!(this, 0x0C => fn() -> u32);
        }
        entry.put(1, entry.arg(0).wrapping_add(entry.arg(1)));
        let words = entry.args.as_mut_ptr();
        let result = find_node(this.add(0x50), words, words.add(1));
        let node = entry.arg(0) as *mut u8;
        let value = if node == sim.nodes() {
            0
        } else {
            TreeNode::from_ptr(node).value()
        };
        registers(value, (result >> 32) as u32)
    }
}

pub(super) const PORTS: &[Port] = &[
    fc4p_006a2300::PORT,
    fc4p_006a2310::PORT,
    fc4p_006a27e0::PORT,
    fc4p_006a2960::PORT,
];

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::testing::{call_thiscall_with_eax, fake, Object};
    use std::cell::RefCell;

    thread_local! {
        static CALLS: RefCell<Vec<(u32, u32)>> = const { RefCell::new(Vec::new()) };
    }

    extern "thiscall" fn retain(this: *mut u8) -> u32 {
        CALLS.with(|c| c.borrow_mut().push((4, this as u32)));
        0xAABB_CCDD
    }

    extern "thiscall" fn initialize(this: *mut u8) -> u32 {
        CALLS.with(|c| c.borrow_mut().push((0x0C, this as u32)));
        unsafe {
            DemandSim::from_ptr(this).set_initialized(1);
        }
        0
    }

    extern "thiscall" fn begin(this: *mut u8) -> u32 {
        CALLS.with(|c| c.borrow_mut().push((0x14, this as u32)));
        0xDEAD_BEEF
    }

    #[test]
    fn identity_keeps_edx_and_interfaces_keep_addref_upper_bytes() {
        let mut call = RawCall::new(Conv::Thiscall, fc4p_006a2300 as *const () as usize, 0, &[]);
        call.edx = 0x1234_5678;
        unsafe {
            call_raw(&mut call);
        }
        assert_eq!((call.eax, call.edx), (0x2990_BFF2, 0x1234_5678));
        let table = [0, retain as *const () as usize];
        let mut object = [table.as_ptr() as u32, 0, 0];
        let this = object.as_mut_ptr().cast();
        for (iid, offset) in [(1, 0), (0x4796_7E16, 0), (0x4522_94AA, 4), (0xE4FD_A3D4, 8)] {
            let mut output = core::ptr::null_mut();
            assert_eq!(unsafe { fc4p_006a2310(this, iid, &mut output) }, 0xAABB_CC01);
            assert_eq!(output, unsafe { this.add(offset) });
        }
        assert_eq!(CALLS.with(|c| c.take()), vec![(4, this as u32); 4]);
        for iid in [0, 2, 0x4522_94AB, 0x4796_7E15, 0x4796_7E17, u32::MAX] {
            let mut output = 0x1234usize as *mut u8;
            let expected = if iid <= 0x4796_7E16 {
                iid.wrapping_sub(0x4522_94AA)
            } else {
                iid
            } & !0xFF;
            assert_eq!(unsafe { fc4p_006a2310(this, iid, &mut output) }, expected);
            assert_eq!(output as usize, 0x1234);
        }
        assert!(CALLS.with(|c| c.take()).is_empty());
    }

    // The map fixture includes a left descent from a right child, upward climbs, and the final header step.
    pub(crate) struct Map {
        pub memory: Vec<[u32; 6]>,
    }

    impl Map {
        pub fn new(values: &[(u32, u32)]) -> Self {
            let mut memory = vec![[0; 6]; values.len() + 1];
            let header = memory[0].as_mut_ptr() as u32;
            if values.is_empty() {
                memory[0][2] = header;
                memory[0][3] = header;
            } else {
                let addresses: Vec<_> = memory.iter_mut().map(|n| n.as_mut_ptr() as u32).collect();
                fn link(
                    memory: &mut [[u32; 6]],
                    addresses: &[u32],
                    values: &[(u32, u32)],
                    lo: usize,
                    hi: usize,
                    parent: u32,
                ) -> u32 {
                    if lo == hi {
                        return 0;
                    }
                    let mid = (lo + hi) / 2;
                    let index = mid + 1;
                    let left = link(memory, addresses, values, lo, mid, addresses[index]);
                    let right = link(memory, addresses, values, mid + 1, hi, addresses[index]);
                    memory[index][1] = parent;
                    memory[index][2] = left;
                    memory[index][3] = right;
                    memory[index][4] = values[mid].0;
                    memory[index][5] = values[mid].1;
                    addresses[index]
                }
                memory[0][1] = link(&mut memory, &addresses, values, 0, values.len(), header);
                memory[0][2] = addresses[1];
                memory[0][3] = addresses[values.len()];
            }
            Self { memory }
        }
        pub fn header(&mut self) -> *mut u8 {
            self.memory[0].as_mut_ptr().cast()
        }
    }

    #[test]
    fn simulation_begin_visits_each_node_in_order_and_empty_map_returns_header() {
        let table = [0, 0, 0, 0, 0, begin as *const () as usize];
        let mut demands: Vec<_> = (0..7).map(|_| Box::new([table.as_ptr() as u32])).collect();
        let values: Vec<_> = demands
            .iter_mut()
            .enumerate()
            .map(|(i, d)| (i as u32, d.as_mut_ptr() as u32))
            .collect();
        for entries in [&values[..], &values[..0]] {
            let mut map = Map::new(entries);
            let mut memory = Object::new(0xB0);
            let sim = unsafe { DemandSim::from_ptr(memory.ptr()) };
            sim.set_nodes(map.header());
            assert_eq!(unsafe { fc4p_006a27e0(sim.as_ptr()) }, sim.nodes() as u32);
            assert_eq!(
                CALLS.with(|c| c.take()),
                entries.iter().map(|&(_, p)| (0x14, p)).collect::<Vec<_>>()
            );
        }
    }

    extern "thiscall" fn find(map: *mut u8, output: *mut u32, key: *const u32) -> u64 {
        unsafe {
            let header = read_unaligned(map.cast::<*mut u8>());
            let key = read_unaligned(key);
            CALLS.with(|c| c.borrow_mut().push((0x0049_7520, key)));
            let mut node = TreeNode::from_ptr(header).left();
            while node != header && TreeNode::from_ptr(node).key() != key {
                node = successor(node);
            }
            write_unaligned(output, node as u32);
            registers(output as u32, 0x7654_3210)
        }
    }

    #[test]
    fn demand_initializes_before_wrapping_key_lookup_and_retains_helper_edx() {
        let _fake = fake(0x0049_7520, find as *const () as usize);
        let mut map = Map::new(&[(3, 0x1122_3344)]);
        let mut memory = Object::new(0xB0);
        let sim = unsafe { DemandSim::from_ptr(memory.ptr()) };
        let table = [0, 0, 0, initialize as *const () as usize];
        sim.set_vtable(table.as_ptr() as u32);
        sim.set_nodes(map.header());
        let run = |args: &[u32]| unsafe {
            call_thiscall_with_eax(fc4p_006a2960 as *const () as usize, sim.as_ptr(), args, 0xAABB_CCDD)
        };
        assert_eq!(run(&[u32::MAX, 4]), (0x1122_3344, 0x7654_3210));
        assert_eq!(run(&[4, 0]), (0, 0x7654_3210));
        assert_eq!(
            CALLS.with(|c| c.take()),
            vec![(0x0C, sim.as_ptr() as u32), (0x0049_7520, 3), (0x0049_7520, 4)]
        );
        let mut probe =
            crate::demand::test_support::Probe::new(fc4p_006a2960 as *const (), sim.as_ptr(), [u32::MAX, 4, 123]);
        unsafe {
            probe.run();
        }
        assert_eq!(probe.after, [map.memory[1].as_mut_ptr() as u32, 3, 123]);
        CALLS.with(|c| c.take());
    }
}
