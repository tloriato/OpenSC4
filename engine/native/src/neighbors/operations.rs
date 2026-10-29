// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Connection deletion by coordinates and copying the complete deal list.

use super::{virtual_call, word};
use crate::port::Port;

game_fn! {
    fn delete_connection(this: *mut u8, edx: u32, connection: *mut u8, remove: u32) -> u64 = "fastcall" 0x006D_17B0;
}
game_fn! {
    fn assign_deals(destination: *mut u8, edx: u32, source: *mut u8) -> u64 = "fastcall" 0x004A_9150;
}

port! {
    /// Looks up a connection by type and cell, then deletes a match with list removal enabled.
    /// Both integer result registers retain the final call's values, including a failed lookup.
    #[port(address = 0x006D_4640, owner = "neighbors", name = "cSC4NeighborsSimulator::DeleteConnection")]
    pub unsafe extern "thiscall" fn fc4p_006d4640(this: *mut u8, kind: u32, x: i32, z: i32) -> u64 {
        // SAFETY: the simulator's connection lookup and deletion use the game's live objects.
        unsafe {
            let result = virtual_call(this, 0x2C, &[kind, x as u32, z as u32], word(this, 0), x as u32);
            if result as u32 == 0 {
                result
            } else {
                delete_connection(this, (result >> 32) as u32, result as u32 as *mut u8, 1)
            }
        }
    }
}

port! {
    /// Assigns the simulator's deal list to the caller's list through the game's list assignment.
    /// The destination may be the simulator's own list. Both result registers come from assignment.
    #[port(address = 0x006D_4670, owner = "neighbors", name = "cSC4NeighborsSimulator::FindDealsAll")]
    pub unsafe extern "fastcall" fn fc4p_006d4670(this: *mut u8, entry_edx: u32, destination: *mut u8) -> u64 {
        // SAFETY: both pointers are live list objects; assignment handles identity and references.
        unsafe { assign_deals(destination, entry_edx, this.add(0x20)) }
    }
}

pub const PORTS: &[Port] = &[fc4p_006d4640::PORT, fc4p_006d4670::PORT];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::neighbors::pair;
    use crate::testing::{fake, Object};
    use std::cell::RefCell;

    #[derive(Debug, PartialEq)]
    enum Event {
        Lookup(u32, u32, u32, i32, i32),
        Delete(u32, u32, u32, u32),
        Assign(u32, u32, u32),
    }

    thread_local! {
        static EVENTS: RefCell<Vec<Event>> = const { RefCell::new(Vec::new()) };
        static FOUND: RefCell<u32> = const { RefCell::new(0) };
    }

    unsafe extern "fastcall" fn lookup(this: *mut u8, edx: u32, kind: u32, x: i32, z: i32) -> u64 {
        EVENTS.with(|events| events.borrow_mut().push(Event::Lookup(this as u32, edx, kind, x, z)));
        pair(FOUND.with(|value| *value.borrow()), 0xACED_1234)
    }

    unsafe extern "fastcall" fn delete(this: *mut u8, edx: u32, connection: *mut u8, remove: u32) -> u64 {
        EVENTS.with(|events| {
            events
                .borrow_mut()
                .push(Event::Delete(this as u32, edx, connection as u32, remove))
        });
        pair(0x8765_4321, 0x1234_5678)
    }

    unsafe extern "fastcall" fn assign(this: *mut u8, edx: u32, source: *mut u8) -> u64 {
        EVENTS.with(|events| events.borrow_mut().push(Event::Assign(this as u32, edx, source as u32)));
        pair(0xDEAD_1234, 0xC0DE_5678)
    }

    #[test]
    fn delete_by_cell_calls_lookup_then_removal_only_for_a_match() {
        let _fake = fake(0x006D_17B0, delete as *const () as usize);
        let mut table = [0usize; 12];
        table[11] = lookup as *const () as usize;
        let mut object = Object::new(0x40);
        let this = object.ptr();
        // SAFETY: the object contains its lookup vtable; the game callee is replaced above.
        unsafe {
            this.cast::<*const usize>().write(table.as_ptr());
            for (found, kind, x, z) in [(0, 0, -1, 64), (0x1234, 12, i32::MIN, i32::MAX)] {
                EVENTS.with(|events| events.borrow_mut().clear());
                FOUND.with(|value| *value.borrow_mut() = found);
                let result = fc4p_006d4640(this, kind, x, z);
                let mut expected = vec![Event::Lookup(this as u32, x as u32, kind, x, z)];
                if found == 0 {
                    assert_eq!(result, pair(0, 0xACED_1234));
                } else {
                    expected.push(Event::Delete(this as u32, 0xACED_1234, found, 1));
                    assert_eq!(result, pair(0x8765_4321, 0x1234_5678));
                }
                EVENTS.with(|events| assert_eq!(*events.borrow(), expected));
                assert_eq!(word(this, 0), table.as_ptr() as u32);
                assert!(core::slice::from_raw_parts(this.add(4), 0x3C)
                    .iter()
                    .all(|&byte| byte == 0));
            }
        }
    }

    #[test]
    fn find_all_assigns_to_the_output_including_self_assignment() {
        let _fake = fake(0x004A_9150, assign as *const () as usize);
        let mut object = Object::new(0x40);
        let mut destination = Object::new(4);
        let this = object.ptr();
        // SAFETY: fake assignment records its arguments without dereferencing list contents.
        unsafe {
            for output in [destination.ptr(), this.add(0x20)] {
                EVENTS.with(|events| events.borrow_mut().clear());
                assert_eq!(fc4p_006d4670(this, 0xAABB_CCDD, output), pair(0xDEAD_1234, 0xC0DE_5678));
                EVENTS.with(|events| {
                    assert_eq!(
                        *events.borrow(),
                        vec![Event::Assign(output as u32, 0xAABB_CCDD, this.add(0x20) as u32)]
                    )
                });
                assert!(core::slice::from_raw_parts(this, 0x40).iter().all(|&byte| byte == 0));
            }
        }
    }
}
