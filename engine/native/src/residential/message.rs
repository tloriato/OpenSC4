// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Daily, monthly and occupant message dispatch from the secondary interface.

use crate::abi::{call_raw, target, vslot, Conv, RawCall};
use crate::port::Port;

layout! {
    /// Offsets relative to the message-target interface, four bytes past the primary interface.
    struct MessageSim {
        0x90 => state / set_state: u32,
        0x9C => dirty / set_dirty: u8,
    }
}

game_fn! {
    #[allow(dead_code)]
    fn daily(this: *mut u8, milliseconds: u32) -> u64 = "thiscall" 0x006F_F520;
}
game_fn! {
    #[allow(dead_code)]
    fn building_changed(this: *mut u8, building: *mut u8, inserted: u32) -> u64 = "thiscall" 0x006F_B120;
}

unsafe fn invoke(function: usize, this: *mut u8, eax: u32, edx: u32, args: &[u32]) -> (u32, u32) {
    let mut call = RawCall::new(Conv::Thiscall, function, this as u32, args);
    call.eax = eax;
    call.edx = edx;
    unsafe { call_raw(&mut call) };
    (call.eax, call.edx)
}

fn registers(eax: u32, edx: u32) -> u64 {
    (u64::from(edx) << 32) | u64::from(eax)
}

port! {
    /// Forward daily work with a five-millisecond budget. A month restarts a finished cycle or marks
    /// the current one dirty. Building notifications query, dispatch, then release the building.
    /// Preserve the last callee's EAX upper bytes and EDX; only the handled flag in AL is replaced.
    /// EDX is an incoming scratch register, so fastcall supplies it without changing the stack arguments.
    #[port(address = 0x0070_0350, owner = "residential", name = "cSC4ResidentialSimulator::DoMessage")]
    pub unsafe extern "fastcall" fn fc4p_00700350(this: *mut u8, entry_edx: u32, message: *mut u8) -> u64 {
        unsafe {
            let table = message.cast::<u32>().read_unaligned();
            let (kind, mut edx) = invoke(vslot(message, 0x10), message, table, entry_edx, &[]);
            let sim = MessageSim::from_ptr(this);
            let mut eax = kind;
            match kind {
                0x6695_6814 => {
                    let (eax, edx) = invoke(target(0x006F_F520), this.wrapping_sub(4), eax, edx, &[5]);
                    return registers(eax, edx);
                }
                0x6695_6816 => {
                    if sim.state() == 10 {
                        sim.set_dirty(0);
                        sim.set_state(0);
                        eax = 0;
                    } else {
                        sim.set_dirty(1);
                    }
                }
                0x99EF_1142 | 0x99EF_1143 => {
                    let table = message.cast::<u32>().read_unaligned();
                    (eax, edx) = invoke(vslot(message, 0x4C), message, eax, table, &[]);
                    if eax == 0x2781_28A0 {
                        let table = message.cast::<u32>().read_unaligned();
                        (eax, _) = invoke(vslot(message, 0x28), message, table, edx, &[]);
                        let occupant = eax as *mut u8;
                        let mut building = core::ptr::null_mut::<u8>();
                        let table = occupant.cast::<u32>().read_unaligned();
                        (eax, edx) = invoke(vslot(occupant, 0), occupant, eax, table,
                            &[0x87DD_A3A9, &mut building as *mut *mut u8 as u32]);
                        if eax as u8 != 0 {
                            edx = (edx & !0xFF) | u32::from(kind == 0x99EF_1142);
                            (eax, _) = invoke(target(0x006F_B120), this.wrapping_sub(4), building as u32,
                                edx, &[building as u32, edx]);
                            let table = building.cast::<u32>().read_unaligned();
                            (eax, edx) = invoke(vslot(building, 8), building, eax, table, &[]);
                        }
                    }
                }
                _ => {}
            }
            registers((eax & !0xFF) | 1, edx)
        }
    }
}

pub const PORTS: &[Port] = &[fc4p_00700350::PORT];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{call_thiscall_with_eax, fake, Object};
    use std::cell::RefCell;

    #[derive(Debug, PartialEq, Eq)]
    enum Call {
        Type(usize),
        Data(usize),
        Occupant(usize),
        Query(usize, u32, u32),
        Changed(usize, usize, u32),
        Release(usize),
        Daily(usize, u32),
    }

    thread_local! {
        static CALLS: RefCell<Vec<Call>> = const { RefCell::new(Vec::new()) };
    }

    fn record(call: Call) {
        CALLS.with(|calls| calls.borrow_mut().push(call));
    }

    fn take() -> Vec<Call> {
        CALLS.with(|calls| std::mem::take(&mut *calls.borrow_mut()))
    }

    #[repr(C)]
    struct Message {
        table: *const usize,
        kind: u32,
        data: u32,
        occupant: *mut u8,
    }

    #[repr(C)]
    struct Occupant {
        table: *const usize,
        query: u32,
        building: *mut u8,
    }

    unsafe extern "thiscall" fn kind(this: *mut Message) -> u64 {
        record(Call::Type(this as usize));
        registers(unsafe { (*this).kind }, 0x1234_5678)
    }

    unsafe extern "thiscall" fn data(this: *mut Message) -> u64 {
        record(Call::Data(this as usize));
        registers(unsafe { (*this).data }, 0x2345_6789)
    }

    unsafe extern "thiscall" fn occupant(this: *mut Message) -> u64 {
        record(Call::Occupant(this as usize));
        registers(unsafe { (*this).occupant as u32 }, 0x3456_789A)
    }

    unsafe extern "thiscall" fn query(this: *mut Occupant, iid: u32, out: *mut *mut u8) -> u64 {
        record(Call::Query(this as usize, iid, unsafe { out.read() } as u32));
        unsafe { out.write((*this).building) };
        registers(unsafe { (*this).query }, 0x4567_89AB)
    }

    extern "thiscall" fn changed(this: *mut u8, building: *mut u8, inserted: u32) -> u64 {
        record(Call::Changed(this as usize, building as usize, inserted));
        registers(0xCDEF_0123, 0xDEAD_BEEF)
    }

    extern "thiscall" fn release(this: *mut u8) -> u64 {
        record(Call::Release(this as usize));
        registers(0x89AB_CDEF, 0x5678_9ABC)
    }

    extern "thiscall" fn daily(this: *mut u8, budget: u32) -> u64 {
        record(Call::Daily(this as usize, budget));
        registers(0xABCD_EF00, 0x6789_ABCD)
    }

    struct World {
        memory: Object,
        message: Box<Message>,
        occupant: Box<Occupant>,
        building: Object,
        _message_table: Box<[usize; 20]>,
        _occupant_table: Box<[usize; 3]>,
        _building_table: Box<[usize; 3]>,
    }

    impl World {
        fn new(kind_value: u32) -> Self {
            take();
            let mut message_table = Box::new([0; 20]);
            message_table[4] = kind as *const () as usize;
            message_table[10] = occupant as *const () as usize;
            message_table[19] = data as *const () as usize;
            let occupant_table = Box::new([query as *const () as usize, 0, 0]);
            let building_table = Box::new([0, 0, release as *const () as usize]);
            let mut building = Object::new(4);
            unsafe { building.ptr().cast::<*const usize>().write(building_table.as_ptr()) };
            let mut occupant = Box::new(Occupant {
                table: occupant_table.as_ptr(),
                query: 1,
                building: building.ptr(),
            });
            let message = Box::new(Message {
                table: message_table.as_ptr(),
                kind: kind_value,
                data: 0x2781_28A0,
                occupant: (&mut *occupant as *mut Occupant).cast(),
            });
            Self {
                memory: Object::new(0x3D0),
                message,
                occupant,
                building,
                _message_table: message_table,
                _occupant_table: occupant_table,
                _building_table: building_table,
            }
        }

        fn primary(&mut self) -> *mut u8 {
            self.memory.ptr()
        }
        fn message_ptr(&mut self) -> *mut u8 {
            (&mut *self.message as *mut Message).cast()
        }
        fn interface(&mut self) -> MessageSim {
            unsafe { MessageSim::from_ptr(self.primary().add(4)) }
        }

        fn run(&mut self) -> (u32, u32) {
            let this = self.interface().as_ptr();
            let message = self.message_ptr() as u32;
            unsafe { call_thiscall_with_eax(fc4p_00700350 as *const () as usize, this, &[message], 0xDEAD_BEEF) }
        }
    }

    #[test]
    fn month_restarts_only_a_finished_cycle() {
        let mut world = World::new(0x6695_6816);
        let message = world.message_ptr() as usize;
        for state in [0, 1, 9, 10, 11, u32::MAX] {
            let sim = world.interface();
            sim.set_state(state);
            sim.set_dirty(0x55);
            let before = unsafe { std::slice::from_raw_parts(world.primary(), 0x3D0).to_vec() };
            assert_eq!(world.run(), (if state == 10 { 1 } else { 0x6695_6801 }, 0x1234_5678));
            assert_eq!(
                (sim.state(), sim.dirty()),
                (if state == 10 { 0 } else { state }, u8::from(state != 10))
            );
            let mut expected = before;
            expected[0xA0] = u8::from(state != 10);
            if state == 10 {
                expected[0x94..0x98].fill(0);
            }
            assert_eq!(unsafe { std::slice::from_raw_parts(world.primary(), 0x3D0) }, expected);
            assert_eq!(take(), [Call::Type(message)]);
        }
    }

    #[test]
    fn daily_passes_primary_interface_and_preserves_full_result() {
        let _fake = fake(0x006F_F520, daily as *const () as usize);
        let mut world = World::new(0x6695_6814);
        assert_eq!(world.run(), (0xABCD_EF00, 0x6789_ABCD));
        assert_eq!(
            take(),
            [
                Call::Type(world.message_ptr() as usize),
                Call::Daily(world.primary() as usize, 5)
            ]
        );
    }

    #[test]
    fn ignored_types_keep_type_upper_bytes_and_do_no_work() {
        for kind in [
            0,
            0x6695_6810,
            0x6695_6813,
            0x6695_6815,
            0x6695_681A,
            0x99EF_1141,
            0x99EF_1144,
            u32::MAX,
        ] {
            let mut world = World::new(kind);
            assert_eq!(world.run(), ((kind & !0xFF) | 1, 0x1234_5678));
            assert_eq!(take(), [Call::Type(world.message_ptr() as usize)]);
        }
    }

    #[test]
    fn occupant_messages_query_dispatch_and_release_in_order() {
        let _fake = fake(0x006F_B120, changed as *const () as usize);
        for kind in [0x99EF_1142, 0x99EF_1143] {
            let mut world = World::new(kind);
            world.occupant.query = 0xABCD_EF80;
            assert_eq!(world.run(), (0x89AB_CD01, 0x5678_9ABC));
            let message = world.message_ptr() as usize;
            assert_eq!(
                take(),
                [
                    Call::Type(message),
                    Call::Data(message),
                    Call::Occupant(message),
                    Call::Query((&*world.occupant as *const Occupant) as usize, 0x87DD_A3A9, 0),
                    Call::Changed(
                        world.primary() as usize,
                        world.building.ptr() as usize,
                        0x4567_8900 | u32::from(kind == 0x99EF_1142)
                    ),
                    Call::Release(world.building.ptr() as usize)
                ]
            );
        }
    }

    #[test]
    fn other_occupant_types_and_failed_queries_stop_at_their_test() {
        let mut world = World::new(0x99EF_1142);
        world.message.data = 0x1234_5600;
        assert_eq!(world.run(), (0x1234_5601, 0x2345_6789));
        let message = world.message_ptr() as usize;
        assert_eq!(take(), [Call::Type(message), Call::Data(message)]);
        world.message.data = 0x2781_28A0;
        world.occupant.query = 0xABCD_EF00;
        assert_eq!(world.run(), (0xABCD_EF01, 0x4567_89AB));
        assert_eq!(
            take(),
            [
                Call::Type(message),
                Call::Data(message),
                Call::Occupant(message),
                Call::Query((&*world.occupant as *const Occupant) as usize, 0x87DD_A3A9, 0)
            ]
        );
    }
}
