// SPDX-License-Identifier: GPL-3.0-or-later
//! Building and network occupant notifications.

use crate::abi::{call_raw, vslot, Conv, RawCall};
use crate::port::Port;

game_fn! { fn building_changed(this:*mut u8, occupant:*mut u8, inserted:u32) -> u64 = "thiscall" 0x006D_BBC0; }
game_fn! { fn network_changed(this:*mut u8, network:*mut u8, inserted:u32) -> u64 = "thiscall" 0x006D_B9A0; }

/// The network query writes directly into the caller's occupant argument word.
unsafe fn changed(this: *mut u8, entry_edx: u32, slot: *mut *mut u8, inserted: u32) -> u64 {
    // SAFETY: the entry provides the original caller-owned argument slot and live occupant.
    unsafe {
        let occupant = slot.read_unaligned();
        let mut c = RawCall::new(Conv::Thiscall, vslot(occupant, 0x1C), occupant as u32, &[]);
        c.eax = occupant.cast::<u32>().read_unaligned();
        c.edx = entry_edx;
        call_raw(&mut c);
        if c.eax == 0x2781_28A0 {
            return building_changed(this, occupant, inserted);
        }
        if c.eax != 0xC772_BF98 {
            return (u64::from(c.edx) << 32) | u64::from(c.eax);
        }
        let table = occupant.cast::<u32>().read_unaligned();
        slot.write_unaligned(core::ptr::null_mut());
        let args = [0x49C0_5B9F, slot as u32];
        c = RawCall::new(
            Conv::Thiscall,
            (table as *const usize).read_unaligned(),
            occupant as u32,
            &args,
        );
        c.eax = slot as u32;
        c.edx = table;
        call_raw(&mut c);
        if c.eax as u8 != 0 {
            let network = slot.read_unaligned();
            let args = [0x8000_0000];
            let mut flags = RawCall::new(Conv::Thiscall, vslot(network, 0x20), network as u32, &args);
            flags.eax = c.eax;
            flags.edx = network.cast::<u32>().read_unaligned();
            call_raw(&mut flags);
            c = flags;
            if c.eax as u8 == 0 {
                let result = network_changed(this, slot.read_unaligned(), inserted);
                c.eax = result as u32;
                c.edx = (result >> 32) as u32;
            }
        }
        let network = slot.read_unaligned();
        if !network.is_null() {
            let mut release = RawCall::new(Conv::Thiscall, vslot(network, 8), network as u32, &[]);
            release.eax = c.eax;
            release.edx = network.cast::<u32>().read_unaligned();
            call_raw(&mut release);
            c = release;
        }
        (u64::from(c.edx) << 32) | u64::from(c.eax)
    }
}

unsafe extern "fastcall" fn inserted(this: *mut u8, edx: u32, slot: *mut *mut u8) -> u64 {
    // SAFETY: the entry forwards its original argument word.
    unsafe { changed(this, edx, slot, 1) }
}
unsafe extern "fastcall" fn removed(this: *mut u8, edx: u32, slot: *mut *mut u8) -> u64 {
    // SAFETY: the entry forwards its original argument word.
    unsafe { changed(this, edx, slot, 0) }
}

port! {
    /// Dispatches buildings directly. Networks are queried, skipped when flag 0x80000000 is set,
    /// otherwise dispatched, then released even when QueryInterface reported failure.
    #[port(address = 0x006D_C870, owner = "plumbing", name = "cSC4PlumbingSimulator::DoOccupantInserted", naked)]
    pub unsafe extern "C" fn fc4p_006dc870() {
        core::arch::naked_asm!("lea eax, [esp + 4]", "push eax", "call {}", "ret 4", sym inserted);
    }
}
port! {
    /// Performs the same dispatch and release sequence for removal, with inserted equal to zero.
    #[port(address = 0x006D_C8F0, owner = "plumbing", name = "cSC4PlumbingSimulator::DoOccupantRemoved", naked)]
    pub unsafe extern "C" fn fc4p_006dc8f0() {
        core::arch::naked_asm!("lea eax, [esp + 4]", "push eax", "call {}", "ret 4", sym removed);
    }
}

pub const PORTS: &[Port] = &[fc4p_006dc870::PORT, fc4p_006dc8f0::PORT];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{call_thiscall_with_eax, fake, Object};
    use std::cell::RefCell;

    #[derive(Default)]
    struct State {
        calls: Vec<(u32, usize, u32)>,
        kind: u32,
        qi: u32,
        flag: u32,
        network: usize,
        slot: usize,
        final_slot: Option<usize>,
    }
    thread_local! { static STATE:RefCell<State> = RefCell::new(State::default()); }
    fn note(kind: u32, this: usize, arg: u32) {
        STATE.with(|s| s.borrow_mut().calls.push((kind, this, arg)));
    }
    extern "thiscall" fn kind(this: *mut u8) -> u32 {
        note(1, this as usize, 0);
        STATE.with(|s| s.borrow().kind)
    }
    extern "thiscall" fn query(this: *mut u8, iid: u32, out: *mut *mut u8) -> u32 {
        note(2, this as usize, iid);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.slot = out as usize;
            assert!(unsafe { out.read_unaligned() }.is_null());
            unsafe { out.write_unaligned(s.network as *mut u8) };
            s.qi
        })
    }
    extern "thiscall" fn flag(this: *mut u8, mask: u32) -> u32 {
        note(3, this as usize, mask);
        STATE.with(|s| s.borrow().flag)
    }
    extern "thiscall" fn release(this: *mut u8) -> u64 {
        note(4, this as usize, 0);
        0x7654_3210_AABB_CCFE
    }
    extern "thiscall" fn building(this: *mut u8, occupant: *mut u8, inserted: u32) -> u64 {
        note(5, this as usize, inserted);
        note(7, occupant as usize, 0);
        0x1234_5678_ABCD_EF12
    }
    extern "thiscall" fn network(this: *mut u8, occupant: *mut u8, inserted: u32) -> u64 {
        note(6, this as usize, inserted);
        note(7, occupant as usize, 0);
        STATE.with(|s| {
            let s = s.borrow();
            if let Some(value) = s.final_slot {
                unsafe { (s.slot as *mut usize).write_unaligned(value) };
            }
        });
        0x2345_6789_BCDE_F123
    }
    #[unsafe(naked)]
    unsafe extern "fastcall" fn call_and_read_argument(_this: *mut u8, _function: usize, _occupant: *mut u8) -> u64 {
        core::arch::naked_asm!("push [esp + 4]", "call edx", "mov edx, [esp - 4]", "ret 4");
    }
    #[test]
    fn notifications_reread_the_caller_slot_after_the_network_callback() {
        let _network = fake(0x006D_B9A0, network as *const () as usize);
        let mut ovt = [0usize; 8];
        ovt[0] = query as *const () as usize;
        ovt[7] = kind as *const () as usize;
        let mut nvt = [0usize; 9];
        nvt[2] = release as *const () as usize;
        nvt[8] = flag as *const () as usize;
        let mut occupant = [ovt.as_ptr() as usize];
        let mut net = [nvt.as_ptr() as usize];
        let mut next = [nvt.as_ptr() as usize];
        let o = occupant.as_mut_ptr().cast::<u8>();
        let n = net.as_mut_ptr().cast::<u8>();
        let mut sim = Object::new(0x1C8);
        let p = sim.ptr();
        for (function, inserted) in [
            (fc4p_006dc870 as *const () as usize, 1),
            (fc4p_006dc8f0 as *const () as usize, 0),
        ] {
            for replacement in [0, next.as_mut_ptr() as usize] {
                STATE.with(|s| {
                    *s.borrow_mut() = State {
                        kind: 0xC772_BF98,
                        qi: 1,
                        network: n as usize,
                        final_slot: Some(replacement),
                        ..State::default()
                    }
                });
                let before = unsafe { std::slice::from_raw_parts(p, 0x1C8) }.to_vec();
                let result = unsafe { call_and_read_argument(p, function, o) };
                assert_eq!((result >> 32) as usize, replacement);
                assert_eq!(result as u32, if replacement == 0 { 0xBCDE_F123 } else { 0xAABB_CCFE });
                let mut expected = vec![
                    (1, o as usize, 0),
                    (2, o as usize, 0x49C0_5B9F),
                    (3, n as usize, 0x8000_0000),
                    (6, p as usize, inserted),
                    (7, n as usize, 0),
                ];
                if replacement != 0 {
                    expected.push((4, replacement, 0));
                }
                STATE.with(|s| assert_eq!(s.borrow().calls, expected));
                assert_eq!(unsafe { std::slice::from_raw_parts(p, 0x1C8) }, before);
            }
        }
    }

    #[test]
    fn notifications_dispatch_buildings_and_networks_and_release_in_order() {
        // Occupant dispatch cases underlying the core's consumers_and_producers and pipe_flags_burst_and_cost.
        let _building = fake(0x006D_BBC0, building as *const () as usize);
        let _network = fake(0x006D_B9A0, network as *const () as usize);
        let mut ovt = [0usize; 8];
        ovt[0] = query as *const () as usize;
        ovt[7] = kind as *const () as usize;
        let mut nvt = [0usize; 9];
        nvt[2] = release as *const () as usize;
        nvt[8] = flag as *const () as usize;
        let mut occupant = [ovt.as_ptr() as usize];
        let mut net = [nvt.as_ptr() as usize];
        let o = occupant.as_mut_ptr().cast::<u8>();
        let n = net.as_mut_ptr().cast::<u8>();
        let mut simulator = Object::new(0x1C8);
        let p = simulator.ptr();
        for (function, inserted) in [
            (fc4p_006dc870 as *const () as usize, 1),
            (fc4p_006dc8f0 as *const () as usize, 0),
        ] {
            for kind in [0u32, 0x2781_28A0, 0xC772_BF98] {
                for qi in [0u32, 0xAABB_CC00, 0xAABB_CC01] {
                    for flag in [0u32, 0xAABB_CC00, 0xAABB_CC80] {
                        STATE.with(|s| {
                            *s.borrow_mut() = State {
                                kind,
                                qi,
                                flag,
                                network: n as usize,
                                ..State::default()
                            }
                        });
                        let (eax, edx) = unsafe { call_thiscall_with_eax(function, p, &[o as u32], 0) };
                        let mut expected = vec![(1, o as usize, 0)];
                        match kind {
                            0x2781_28A0 => {
                                expected.extend([(5, p as usize, inserted), (7, o as usize, 0)]);
                                assert_eq!((eax, edx), (0xABCD_EF12, 0x1234_5678));
                            }
                            0xC772_BF98 => {
                                expected.push((2, o as usize, 0x49C0_5B9F));
                                if qi as u8 != 0 {
                                    expected.push((3, n as usize, 0x8000_0000));
                                    if flag as u8 == 0 {
                                        expected.extend([(6, p as usize, inserted), (7, n as usize, 0)]);
                                    }
                                }
                                expected.push((4, n as usize, 0));
                                assert_eq!((eax, edx), (0xAABB_CCFE, 0x7654_3210));
                            }
                            _ => assert_eq!(eax, kind),
                        }
                        STATE.with(|s| assert_eq!(s.borrow().calls, expected));
                    }
                }
                STATE.with(|s| {
                    *s.borrow_mut() = State {
                        kind,
                        qi: 0,
                        flag: 0,
                        network: 0,
                        ..State::default()
                    }
                });
                let result = unsafe { call_and_read_argument(p, function, o) };
                assert_eq!((result >> 32) as u32, if kind == 0xC772_BF98 { 0 } else { o as u32 });
                if kind == 0xC772_BF98 {
                    STATE.with(|s| assert_eq!(s.borrow().calls, [(1, o as usize, 0), (2, o as usize, 0x49C0_5B9F)]));
                }
            }
        }
    }
}
