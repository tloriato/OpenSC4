// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Immediate occupied-capacity updates.

use super::*;
use crate::x87::F80;

layout! {
    /// Building profile fields used by occupied-capacity accounting.
    struct Profile {
        0x00 => purpose / set_purpose: u32,
        0x0C => capacities / set_capacities: *mut u8,
        0x10 => capacities_end / set_capacities_end: *mut u8,
    }
}

unsafe fn find(header: *mut u8, key: u32) -> *mut u8 {
    unsafe {
        let mut candidate = header;
        let mut node = TreeNode::from_ptr(header).parent();
        while !node.is_null() {
            let view = TreeNode::from_ptr(node);
            if view.key() < key {
                node = view.right();
            } else {
                candidate = node;
                node = view.left();
            }
        }
        if candidate != header && key < TreeNode::from_ptr(candidate).key() {
            header
        } else {
            candidate
        }
    }
}

port! {
    /// Sum the developer's unsigned 16-bit capacities across profile maps and add the signed total to its supply.
    /// Only the low occupied byte controls the sign. Preserve the argument words used for the running total.
    #[port(address = 0x006A_2C80, owner = "demand", name = "cSC4DemandSimulator::UpdateCapacity", naked)]
    pub unsafe extern "C" fn fc4p_006a2c80() {
        core::arch::naked_asm!(
            "push edx", "push eax", "mov edx, esp", "call {body}", "add esp, 8", "ret 12",
            body = sym update_capacity,
        )
    }
}

unsafe extern "fastcall" fn update_capacity(this: *mut u8, entry: *mut Entry) -> u32 {
    unsafe {
        let sim = DemandSim::from_ptr(this);
        let entry = &mut *entry;
        let building = entry.arg(0);
        let service = sim.building_development();
        let profile = vcall!(service, 0x9C => fn(u32) -> *mut u8, building);
        let profile = Profile::from_ptr(profile);
        let purpose = profile.purpose();
        if purpose == 0 {
            return 0;
        }
        let developer = vcall!(sim.building_development(), 0x78 => fn(u32, u32) -> u32, purpose, entry.arg(1));
        let mut map = profile.capacities();
        let end = profile.capacities_end();
        entry.put(0, 0);
        while map != end {
            let header = read_unaligned(map.cast::<*mut u8>());
            let node = find(header, developer);
            if node != header {
                let capacity = u32::from(read_unaligned(node.add(0x14).cast::<u16>()));
                entry.put(0, entry.arg(0).wrapping_add(capacity));
            }
            map = map.add(12);
        }
        let node = find(sim.nodes(), developer);
        if node == sim.nodes() {
            return developer;
        }
        let occupied = entry.arg(2) as u8;
        let object = TreeNode::from_ptr(node).value() as *mut u8;
        let total = if occupied == 0 {
            entry.arg(0).wrapping_neg()
        } else {
            entry.arg(0)
        };
        entry.put(2, total);
        let value = F80::from_i32(total as i32).to_f32();
        let result = virtual_call(
            object,
            0x40,
            (developer & !0xFF) | u32::from(occupied),
            read_unaligned(object.cast()),
            &[value.to_bits()],
        );
        result.eax
    }
}

pub(super) const PORTS: &[Port] = &[fc4p_006a2c80::PORT];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demand::{interface::tests::Map, test_support::Probe};
    use crate::testing::{GameFpu, Object};
    use std::cell::RefCell;

    thread_local! {
        static CALLS: RefCell<Vec<(u32, Vec<u32>)>> = const { RefCell::new(Vec::new()) };
    }

    #[repr(C)]
    struct Service {
        table: *const usize,
        profile: *mut u8,
        developer: u32,
    }
    extern "thiscall" fn profile(this: *mut Service, building: u32) -> *mut u8 {
        CALLS.with(|c| c.borrow_mut().push((0x9C, vec![building])));
        unsafe { (*this).profile }
    }
    extern "thiscall" fn developer(this: *mut Service, purpose: u32, wealth: u32) -> u32 {
        CALLS.with(|c| c.borrow_mut().push((0x78, vec![purpose, wealth])));
        unsafe { (*this).developer }
    }
    extern "thiscall" fn supply(this: *mut u8, value: u32) -> u32 {
        CALLS.with(|c| c.borrow_mut().push((0x40, vec![this as u32, value])));
        0xABCD_1234
    }

    #[test]
    fn occupancy_adds_and_removes_the_sum_of_matching_unsigned_capacities() {
        let _fpu = GameFpu::new();
        let mut object_table = [0; 17];
        object_table[0x40 / 4] = supply as *const () as usize;
        let mut object = [object_table.as_ptr() as u32];
        let object = object.as_mut_ptr().cast::<u8>();
        for key in [0x1010u32, 0xFFFF_FFF0] {
            let mut a = Map::new(&[(key.wrapping_sub(1), 7), (key, 0xBEEF_FFFF), (key.wrapping_add(1), 3)]);
            let mut b = Map::new(&[(key, 0xCAFE_0001)]);
            let mut absent = Map::new(&[(key.wrapping_add(1), 2)]);
            let mut empty = Map::new(&[]);
            let mut maps = [
                a.header() as u32,
                3,
                0,
                b.header() as u32,
                1,
                0,
                absent.header() as u32,
                1,
                0,
                empty.header() as u32,
                0,
                0,
            ];
            let mut profile_mem = Object::new(0x18);
            let view = unsafe { Profile::from_ptr(profile_mem.ptr()) };
            view.set_purpose(1);
            view.set_capacities(maps.as_mut_ptr().cast());
            view.set_capacities_end(unsafe { maps.as_mut_ptr().add(maps.len()).cast() });
            let mut table = [0; 40];
            table[0x9C / 4] = profile as *const () as usize;
            table[0x78 / 4] = developer as *const () as usize;
            let mut service = Service {
                table: table.as_ptr(),
                profile: view.as_ptr(),
                developer: key,
            };
            let mut memory = Object::new(0xB0);
            let sim = unsafe { DemandSim::from_ptr(memory.ptr()) };
            sim.set_building_development((&mut service as *mut Service).cast());
            let mut nodes = Map::new(&[(key, object as u32)]);
            sim.set_nodes(nodes.header());
            for (occupied, value) in [(1, 65536.0f32), (0xFF, 65536.0), (0x100, -65536.0)] {
                let mut call = Probe::new(fc4p_006a2c80 as *const (), sim.as_ptr(), [5, u32::MAX, occupied]);
                unsafe {
                    call.run();
                }
                assert_eq!(call.eax, 0xABCD_1234);
                assert_eq!(
                    call.after,
                    [
                        65536,
                        u32::MAX,
                        if occupied as u8 == 0 { (-65536i32) as u32 } else { 65536 }
                    ]
                );
                assert_eq!(
                    CALLS.with(|c| c.take()),
                    [
                        (0x9C, vec![5]),
                        (0x78, vec![1, u32::MAX]),
                        (0x40, vec![object as u32, value.to_bits()])
                    ]
                );
            }
        }
    }

    #[test]
    fn zero_purpose_and_missing_demand_skip_supply_updates() {
        let mut profile_mem = Object::new(0x18);
        let view = unsafe { Profile::from_ptr(profile_mem.ptr()) };
        let mut table = [0; 40];
        table[0x9C / 4] = profile as *const () as usize;
        table[0x78 / 4] = developer as *const () as usize;
        let mut service = Service {
            table: table.as_ptr(),
            profile: view.as_ptr(),
            developer: 0x1010,
        };
        let mut memory = Object::new(0xB0);
        let sim = unsafe { DemandSim::from_ptr(memory.ptr()) };
        sim.set_building_development((&mut service as *mut Service).cast());
        let mut call = Probe::new(fc4p_006a2c80 as *const (), sim.as_ptr(), [5, 1, 1]);
        unsafe {
            call.run();
        }
        assert_eq!(call.eax, 0);
        assert_eq!(call.after, [5, 1, 1]);
        assert_eq!(CALLS.with(|c| c.take()), [(0x9C, vec![5])]);
        let mut nodes = Map::new(&[(0x1011, 0)]);
        sim.set_nodes(nodes.header());
        view.set_purpose(1);
        unsafe {
            call.run();
        }
        assert_eq!(call.eax, 0x1010);
        assert_eq!(call.after, [0, 1, 1]);
        assert_eq!(CALLS.with(|c| c.take()), [(0x9C, vec![5]), (0x78, vec![1, 1])]);
    }
}
