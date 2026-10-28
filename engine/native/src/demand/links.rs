// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Demand-link propagation until no transfer exceeds the threshold.

use super::queries::{add_float, divide_float, multiply_float, subtract_from_float};
use super::*;
use crate::x87::F80;

game_fn! { fn node_value(map: *mut u8, key: *const u32) -> *mut *mut u8 = "thiscall" 0x0049_7A00; }
game_data! { static threshold: f32 = 0x00A8_23A4; }
game_data! { static zero: f32 = 0x00A8_1054; }
game_data! { static one: f32 = 0x00A8_1228; }

layout! {
    /// The link map is a multimap keyed by source demand. Its record starts at offset 0x14.
    struct Link {
        0x1C => target / set_target: u32,
        0x20 => kind / set_kind: u32,
        0x24 => factor / set_factor: f32,
    }
}

unsafe fn bounds(header: *mut u8, key: u32) -> (*mut u8, *mut u8) {
    unsafe {
        let root = TreeNode::from_ptr(header).parent();
        let (mut lower, mut upper) = (header, header);
        let mut current = root;
        while !current.is_null() {
            let node = TreeNode::from_ptr(current);
            if key < node.key() {
                upper = current;
                current = node.left();
            } else {
                current = node.right();
            }
        }
        current = root;
        while !current.is_null() {
            let node = TreeNode::from_ptr(current);
            if node.key() < key {
                current = node.right();
            } else {
                lower = current;
                current = node.left();
            }
        }
        (lower, upper)
    }
}

fn significant(value: F80, threshold: f32) -> bool {
    let comparison = value.abs().compare(F80::from_f32(threshold));
    !comparison.less() && !comparison.equal() && !comparison.unordered()
}

port! {
    /// Visit sources and their links in tree order. Apply significant deltas before committing each source;
    /// sum links correct their source after its commit. Caps never cause an extra pass. There is no pass limit.
    /// Return the demand header's upper bytes with AL one, or the untouched header for an empty map.
    #[port(address = 0x006A_4E70, owner = "demand", name = "cSC4DemandSimulator::DoDemandIndexLinks")]
    pub unsafe extern "thiscall" fn fc4p_006a4e70(this: *mut u8) -> u32 {
        unsafe {
            let sim = DemandSim::from_ptr(this);
            loop {
                let header = sim.nodes();
                let mut source_entry = TreeNode::from_ptr(header).left();
                if source_entry == header { return header as u32; }
                let mut stable = true;
                loop {
                    let source_node = TreeNode::from_ptr(source_entry);
                    let source = source_node.value() as *mut u8;
                    let link_header = read_unaligned(this.add(0x5C).cast::<*mut u8>());
                    let (mut link, end) = bounds(link_header, source_node.key());
                    let (mut supply_sum, mut demand_sum) = (0.0f32, 0.0f32);
                    let mut summed = false;
                    while link != end {
                        let record = Link::from_ptr(link);
                        let target = read_unaligned(node_value(this.add(0x50), link.add(0x1C).cast()));
                        let kind = record.kind();
                        match kind {
                            0x47BB_3F30..=0x47BB_3F32 => {
                                let factor = record.factor();
                                let input_slot = if kind == 0x47BB_3F32 { 0x2C } else { 0x28 };
                                let value = multiply_float(vcall_st0!(source, input_slot), factor);
                                let stored = value.to_f32();
                                if significant(value, read_unaligned(threshold())) {
                                    let slot = if kind == 0x47BB_3F31 { 0x40 } else { 0x44 };
                                    vcall!(target, slot => fn(f32) -> u32, stored);
                                    stable = false;
                                }
                            }
                            0x47BB_3F41 => {
                                let mut supply = vcall_st0!(source, 0x20).to_f32();
                                let mut demand = vcall_st0!(source, 0x24);
                                if F80::from_f32(supply).compare(F80::from_f32(read_unaligned(threshold()))).less() {
                                    supply = f32::from_bits(0x3C23_D70A);
                                }
                                if demand.compare(F80::from_f32(read_unaligned(zero()))).less() {
                                    demand = F80::from_f32(read_unaligned(zero()));
                                }
                                let cap = if demand.compare(F80::from_f32(supply)).less() {
                                    divide_float(demand, supply).to_f32()
                                } else { read_unaligned(one()) };
                                vcall!(target, 0x6C => fn(*const f32) -> u32, &cap);
                            }
                            0x47BB_3F50 => {
                                summed = true;
                                supply_sum = add_float(vcall_st0!(target, 0x20), supply_sum).to_f32();
                                demand_sum = add_float(vcall_st0!(target, 0x24), demand_sum).to_f32();
                            }
                            _ => {}
                        }
                        link = successor(link);
                    }
                    vcall!(source, 0x78 => fn() -> u32);
                    if summed {
                        let ds = subtract_from_float(vcall_st0!(source, 0x20), supply_sum).to_f32();
                        let dd = subtract_from_float(vcall_st0!(source, 0x24), demand_sum).to_f32();
                        if significant(F80::from_f32(ds), read_unaligned(threshold())) {
                            vcall!(source, 0x40 => fn(f32) -> u32, ds);
                            stable = false;
                        }
                        if significant(F80::from_f32(dd), read_unaligned(threshold())) {
                            vcall!(source, 0x44 => fn(f32) -> u32, dd);
                            stable = false;
                        }
                    }
                    source_entry = successor(source_entry);
                    if source_entry == sim.nodes() { break; }
                }
                if stable { return (source_entry as u32 & !0xFF) | 1; }
            }
        }
    }
}

pub(super) const PORTS: &[Port] = &[fc4p_006a4e70::PORT];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demand::interface::tests::Map;
    use crate::testing::{fake, fake_data, Fake, GameFpu, Object};
    use std::cell::RefCell;

    #[derive(Debug, PartialEq)]
    enum Call {
        Target(u32),
        Query(u32, u32),
        Add(u32, u32, u32),
        Cap(u32, u32),
        End(u32),
    }
    thread_local! { static CALLS: RefCell<Vec<Call>> = const { RefCell::new(Vec::new()) }; }
    #[repr(C)]
    struct Demand {
        table: *const usize,
        id: u32,
        supply: f32,
        demand: f32,
        new_supply: f32,
        new_demand: f32,
        cap: f32,
    }
    fn log(c: Call) {
        CALLS.with(|s| s.borrow_mut().push(c));
    }
    extern "thiscall" fn supply(this: *mut Demand) -> f32 {
        unsafe {
            log(Call::Query((*this).id, 0x20));
            (*this).supply
        }
    }
    extern "thiscall" fn demand(this: *mut Demand) -> f32 {
        unsafe {
            log(Call::Query((*this).id, 0x24));
            (*this).demand
        }
    }
    extern "thiscall" fn new_supply(this: *mut Demand) -> f32 {
        unsafe {
            log(Call::Query((*this).id, 0x28));
            (*this).new_supply
        }
    }
    extern "thiscall" fn new_demand(this: *mut Demand) -> f32 {
        unsafe {
            log(Call::Query((*this).id, 0x2C));
            (*this).new_demand
        }
    }
    extern "thiscall" fn add_supply(this: *mut Demand, value: f32) -> u32 {
        unsafe {
            log(Call::Add((*this).id, 0x40, value.to_bits()));
            (*this).new_supply = add_float(F80::from_f32((*this).new_supply), value).to_f32();
        }
        1
    }
    extern "thiscall" fn add_demand(this: *mut Demand, value: f32) -> u32 {
        unsafe {
            log(Call::Add((*this).id, 0x44, value.to_bits()));
            (*this).new_demand = add_float(F80::from_f32((*this).new_demand), value).to_f32();
        }
        1
    }
    extern "thiscall" fn cap(this: *mut Demand, value: *const f32) -> u32 {
        unsafe {
            log(Call::Cap((*this).id, (*value).to_bits()));
            (*this).cap = *value;
        }
        1
    }
    extern "thiscall" fn end(this: *mut Demand) -> u32 {
        unsafe {
            let d = &mut *this;
            log(Call::End(d.id));
            d.supply = add_float(F80::from_f32(d.supply), d.new_supply).to_f32();
            d.demand = add_float(F80::from_f32(d.demand), d.new_demand).to_f32();
            d.new_supply = 0.0;
            d.new_demand = 0.0;
        }
        0
    }
    extern "thiscall" fn value(map: *mut u8, key: *const u32) -> *mut *mut u8 {
        unsafe {
            let key = *key;
            log(Call::Target(key));
            let header = read_unaligned(map.cast::<*mut u8>());
            let mut node = TreeNode::from_ptr(header).left();
            while node != header && TreeNode::from_ptr(node).key() != key {
                node = successor(node);
            }
            assert_ne!(node, header);
            node.add(0x14).cast()
        }
    }

    struct World {
        memory: Object,
        nodes: Vec<Demand>,
        _map: Map,
        links: Vec<[u32; 10]>,
        _table: Box<[usize; 31]>,
        _threshold: Box<f32>,
        _zero: Box<f32>,
        _one: Box<f32>,
        _guards: Vec<Fake>,
    }
    impl World {
        fn new(ids: &[u32], records: &[(u32, u32, u32, f32)]) -> Self {
            let mut table = Box::new([0; 31]);
            for (slot, f) in [
                (0x20, supply as *const ()),
                (0x24, demand as *const ()),
                (0x28, new_supply as *const ()),
                (0x2C, new_demand as *const ()),
                (0x40, add_supply as *const ()),
                (0x44, add_demand as *const ()),
                (0x6C, cap as *const ()),
                (0x78, end as *const ()),
            ] {
                table[slot / 4] = f as usize;
            }
            let mut nodes: Vec<_> = ids
                .iter()
                .map(|&id| Demand {
                    table: table.as_ptr(),
                    id,
                    supply: 0.0,
                    demand: 0.0,
                    new_supply: 0.0,
                    new_demand: 0.0,
                    cap: 0.0,
                })
                .collect();
            let values: Vec<_> = nodes.iter_mut().map(|n| (n.id, n as *mut Demand as u32)).collect();
            let mut map = Map::new(&values);
            // An in-order right chain retains the order of equal source keys.
            let mut links = vec![[0u32; 10]; records.len() + 1];
            let pointers: Vec<_> = links.iter_mut().map(|n| n.as_mut_ptr() as u32).collect();
            links[0][1] = pointers.get(1).copied().unwrap_or(0);
            links[0][2] = pointers.get(1).copied().unwrap_or(pointers[0]);
            links[0][3] = *pointers.last().unwrap();
            for (i, &(source, target, kind, factor)) in records.iter().enumerate() {
                let n = &mut links[i + 1];
                n[1] = pointers[i];
                n[3] = pointers.get(i + 2).copied().unwrap_or(0);
                n[4] = source;
                n[7] = target;
                n[8] = kind;
                n[9] = factor.to_bits();
            }
            let mut memory = Object::new(0xB4);
            let sim = unsafe { DemandSim::from_ptr(memory.ptr()) };
            sim.set_nodes(map.header());
            unsafe {
                write_unaligned(sim.as_ptr().add(0x5C).cast::<u32>(), pointers[0]);
            }
            let mut threshold = Box::new(0.01f32);
            let mut zero = Box::new(0.0f32);
            let mut one = Box::new(1.0f32);
            let guards = vec![
                fake(0x0049_7A00, value as *const () as usize),
                fake_data(0x00A8_23A4, (&mut *threshold as *mut f32).cast()),
                fake_data(0x00A8_1054, (&mut *zero as *mut f32).cast()),
                fake_data(0x00A8_1228, (&mut *one as *mut f32).cast()),
            ];
            Self {
                memory,
                nodes,
                _map: map,
                links,
                _table: table,
                _threshold: threshold,
                _zero: zero,
                _one: one,
                _guards: guards,
            }
        }
        fn run(&mut self) -> u32 {
            unsafe { fc4p_006a4e70(self.memory.ptr()) }
        }
    }

    #[test]
    fn core_links_transfer_drop_and_sum_case_preserves_call_order() {
        let _fpu = GameFpu::new();
        let mut w = World::new(
            &[0x100, 0x200, 0x300, 0x400, 0x500],
            &[
                (0x100, 0x300, 0x47BB_3F31, 0.02),
                (0x100, 0x200, 0x47BB_3F30, 0.01),
                (0x100, 0x500, 0x47BB_3F41, 1.0),
                (0x400, 0x300, 0x47BB_3F50, 1.0),
            ],
        );
        w.nodes[0].new_supply = 1.0;
        w.nodes[0].new_demand = 4.0;
        let result = w.run();
        assert_eq!(result, (w._map.header() as u32 & !0xFF) | 1);
        assert_eq!(w.nodes[1].demand.to_bits(), 0);
        assert_eq!(w.nodes[2].supply.to_bits(), 0.02f32.to_bits());
        assert_eq!(w.nodes[3].supply.to_bits(), 0.02f32.to_bits());
        assert_eq!(w.nodes[4].cap.to_bits(), 1.0f32.to_bits());
        let calls = CALLS.with(|s| s.take());
        let first = vec![
            Call::Target(0x300),
            Call::Query(0x100, 0x28),
            Call::Add(0x300, 0x40, 0.02f32.to_bits()),
            Call::Target(0x200),
            Call::Query(0x100, 0x28),
            Call::Target(0x500),
            Call::Query(0x100, 0x20),
            Call::Query(0x100, 0x24),
            Call::Cap(0x500, 0),
            Call::End(0x100),
            Call::End(0x200),
            Call::End(0x300),
            Call::Target(0x300),
            Call::Query(0x300, 0x20),
            Call::Query(0x300, 0x24),
            Call::End(0x400),
            Call::Query(0x400, 0x20),
            Call::Query(0x400, 0x24),
            Call::Add(0x400, 0x40, 0.02f32.to_bits()),
            Call::End(0x500),
        ];
        assert_eq!(&calls[..first.len()], &first);
        assert_eq!(calls.iter().filter(|c| matches!(c, Call::End(_))).count(), 10);
    }

    #[test]
    fn caps_clamp_negative_inputs_and_nan_to_one_without_triggering_another_pass() {
        let _fpu = GameFpu::new();
        for (s, d, expected) in [
            (-1.0, -2.0, 0.0f32),
            (2.0, 1.0, 0.5),
            (0.0, 0.005, 0.5),
            (f32::NAN, 1.0, 1.0),
            (1.0, f32::NAN, 1.0),
        ] {
            let mut w = World::new(&[1, 2], &[(1, 2, 0x47BB_3F41, 1.0)]);
            w.nodes[0].supply = s;
            w.nodes[0].demand = d;
            w.run();
            assert_eq!(w.nodes[1].cap.to_bits(), expected.to_bits());
            let calls = CALLS.with(|s| s.take());
            assert_eq!(calls.iter().filter(|c| matches!(c, Call::End(_))).count(), 2);
        }
    }

    #[test]
    fn contributes_unknown_links_and_empty_maps_keep_their_distinct_behaviour() {
        let _fpu = GameFpu::new();
        let mut w = World::new(&[1, 2], &[(1, 2, 0x47BB_3F32, 0.5), (1, 2, 0xDEAD_BEEF, 1.0)]);
        w.nodes[0].new_demand = 2.0;
        w.run();
        assert_eq!(w.nodes[1].demand, 1.0);
        let calls = CALLS.with(|s| s.take());
        assert_eq!(
            &calls[..5],
            &[
                Call::Target(2),
                Call::Query(1, 0x2C),
                Call::Add(2, 0x44, 1.0f32.to_bits()),
                Call::Target(2),
                Call::End(1)
            ]
        );
        assert_eq!(w.links.len(), 3);
        let mut w = World::new(&[], &[]);
        assert_eq!(w.run(), w._map.header() as u32);
        assert!(CALLS.with(|s| s.take()).is_empty());
    }

    #[test]
    fn unordered_and_threshold_sized_transfers_are_dropped() {
        let _fpu = GameFpu::new();
        for value in [0.01f32, -0.01, f32::from_bits(0x7FC1_2345)] {
            let mut w = World::new(&[1, 2], &[(1, 2, 0x47BB_3F30, 1.0)]);
            w.nodes[0].new_supply = value;
            w.run();
            assert_eq!(w.nodes[1].demand, 0.0);
            assert_eq!(
                CALLS.with(|s| s.take()),
                [Call::Target(2), Call::Query(1, 0x28), Call::End(1), Call::End(2)]
            );
        }
    }
}
