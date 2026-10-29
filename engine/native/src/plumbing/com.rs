// SPDX-License-Identifier: GPL-3.0-or-later
//! Interface identity and shared reference counting.

use super::PlumbingSim;
use crate::abi::{call_raw, vslot, Conv, RawCall};
use crate::port::Port;

port! {
    /// Returns the class identifier without accessing the interface or changing EDX.
    #[port(address = 0x006D_A3B0, owner = "plumbing", name = "cSC4PlumbingSimulator::GetGZCLSID", naked)]
    pub unsafe extern "C" fn fc4p_006da3b0() {
        core::arch::naked_asm!("mov eax, 0x0990c075", "ret");
    }
}

port! {
    /// Writes a supported interface before AddRef; a rejected IID leaves the destination untouched.
    /// AL is the answer, with upper EAX bytes from AddRef or the IID comparison arithmetic.
    #[port(address = 0x006D_A3C0, owner = "plumbing", name = "cSC4PlumbingSimulator::QueryInterface")]
    pub unsafe extern "fastcall" fn fc4p_006da3c0(this: *mut u8, entry_edx: u32, iid: u32, out: *mut *mut u8) -> u64 {
        let offset = match iid {
            1 | 0x4778_2CCC => 0,
            0x4522_94AA => 4,
            0xE4FD_A3D4 => 8,
            _ => {
                let eax = if iid > 0x4778_2CCC { iid } else { iid.wrapping_sub(0x4522_94AA) };
                return (u64::from(entry_edx) << 32) | u64::from(eax & !0xFF);
            }
        };
        // SAFETY: the caller provides the destination and a live primary interface on success.
        unsafe {
            out.write_unaligned(if this.is_null() { this } else { this.wrapping_add(offset) });
            let mut call = RawCall::new(Conv::Thiscall, vslot(this, 4), this as u32, &[]);
            call.eax = this.cast::<u32>().read_unaligned();
            call.edx = out as u32;
            call_raw(&mut call);
            (u64::from(call.edx) << 32) | u64::from((call.eax & !0xFF) | 1)
        }
    }
}

port! {
    /// Decrements counts above one; otherwise calls Shutdown then deleting destruction with flag one.
    /// The count is signed and the vtable is read again after Shutdown.
    #[port(address = 0x006D_A610, owner = "plumbing", name = "cSC4PlumbingSimulator::Release")]
    pub unsafe extern "fastcall" fn fc4p_006da610(this: *mut u8, entry_edx: u32) -> u64 {
        // SAFETY: the game supplies its live simulator and virtual methods.
        unsafe {
            let sim = PlumbingSim::from_ptr(this);
            let count = sim.reference_count();
            if count > 1 {
                let next = count.wrapping_sub(1);
                sim.set_reference_count(next);
                return (u64::from(entry_edx) << 32) | u64::from(next as u32);
            }
            let mut shutdown = RawCall::new(Conv::Thiscall, vslot(this, 0x10), this as u32, &[]);
            shutdown.eax = this.cast::<u32>().read_unaligned();
            shutdown.edx = entry_edx;
            call_raw(&mut shutdown);
            let args = [1];
            let mut delete = RawCall::new(Conv::Thiscall, vslot(this, 0x74), this as u32, &args);
            delete.eax = shutdown.eax;
            delete.edx = this.cast::<u32>().read_unaligned();
            call_raw(&mut delete);
            u64::from(delete.edx) << 32
        }
    }
}

// Each adjustment forwards to the original entry, which may be independently replaced.
game_fn! { #[allow(dead_code)] fn query(this: *mut u8, iid: u32, out: *mut *mut u8) -> u32 = "thiscall" 0x006D_A3C0; }
game_fn! { #[allow(dead_code)] fn release(this: *mut u8) -> u32 = "thiscall" 0x006D_A610; }

port! {
    /// Forwards the serializable interface's QueryInterface to the primary interface.
    #[port(address = 0x006D_C6E0, owner = "plumbing", name = "cSC4PlumbingSimulator::SerializableQueryInterface", naked)]
    pub unsafe extern "C" fn fc4p_006dc6e0() {
        #[cfg(not(test))]
        core::arch::naked_asm!("sub ecx, 8", "jmp dword ptr [2f]", "2: .long 0x006da3c0");
        #[cfg(test)]
        core::arch::naked_asm!("sub ecx, 8", "jmp {}", sym fc4p_006da3c0);
    }
}
port! {
    /// Forwards the message target's Release to the primary interface.
    #[port(address = 0x006D_C6F0, owner = "plumbing", name = "cSC4PlumbingSimulator::MessageTargetRelease", naked)]
    pub unsafe extern "C" fn fc4p_006dc6f0() {
        #[cfg(not(test))]
        core::arch::naked_asm!("sub ecx, 4", "jmp dword ptr [2f]", "2: .long 0x006da610");
        #[cfg(test)]
        core::arch::naked_asm!("sub ecx, 4", "jmp {}", sym fc4p_006da610);
    }
}
port! {
    /// Forwards the serializable interface's Release to the primary interface.
    #[port(address = 0x006D_C700, owner = "plumbing", name = "cSC4PlumbingSimulator::SerializableRelease", naked)]
    pub unsafe extern "C" fn fc4p_006dc700() {
        #[cfg(not(test))]
        core::arch::naked_asm!("sub ecx, 8", "jmp dword ptr [2f]", "2: .long 0x006da610");
        #[cfg(test)]
        core::arch::naked_asm!("sub ecx, 8", "jmp {}", sym fc4p_006da610);
    }
}

pub const PORTS: &[Port] = &[
    fc4p_006da3b0::PORT,
    fc4p_006da3c0::PORT,
    fc4p_006da610::PORT,
    fc4p_006dc6e0::PORT,
    fc4p_006dc6f0::PORT,
    fc4p_006dc700::PORT,
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{call_thiscall_with_eax, Object};
    use std::cell::RefCell;

    thread_local! { static CALLS: RefCell<Vec<(usize, u32)>> = const { RefCell::new(Vec::new()) }; }
    thread_local! { static NEXT: RefCell<usize> = const { RefCell::new(0) }; }

    extern "fastcall" fn retain(this: *mut u8, out: *mut *mut u8) -> u64 {
        // SAFETY: the destination was written before this callback.
        CALLS.with(|c| c.borrow_mut().push((this as usize, unsafe { out.read() } as u32)));
        0x7654_3210_AABB_CCFE
    }
    extern "thiscall" fn shutdown(this: *mut u8) -> u32 {
        CALLS.with(|c| c.borrow_mut().push((this as usize, 10)));
        NEXT.with(|n| unsafe { this.cast::<usize>().write(*n.borrow()) });
        0xABCD_EF12
    }
    extern "fastcall" fn delete(this: *mut u8, edx: usize, flag: u32) -> u64 {
        assert_eq!(edx, unsafe { this.cast::<usize>().read() });
        CALLS.with(|c| c.borrow_mut().push((this as usize, flag)));
        0x1234_5678_8765_4321
    }
    fn take() -> Vec<(usize, u32)> {
        CALLS.with(|c| core::mem::take(&mut *c.borrow_mut()))
    }

    #[test]
    fn identity_and_interfaces_preserve_full_results() {
        let mut object = Object::new(0x1C8);
        let mut vt = [0usize; 30];
        vt[1] = retain as *const () as usize;
        unsafe { object.ptr().cast::<usize>().write(vt.as_ptr() as usize) };
        let mut c = RawCall::new(Conv::Thiscall, fc4p_006da3b0 as *const () as usize, 0, &[]);
        c.edx = 0x8765_4321;
        unsafe { call_raw(&mut c) };
        assert_eq!((c.eax, c.edx), (0x0990_C075, 0x8765_4321));
        for (iid, offset) in [(1, 0), (0x4778_2CCC, 0), (0x4522_94AA, 4), (0xE4FD_A3D4, 8)] {
            let mut out = core::ptr::null_mut();
            assert_eq!(
                unsafe { fc4p_006da3c0(object.ptr(), 0, iid, &mut out) },
                0x7654_3210_AABB_CC01
            );
            assert_eq!(out, object.ptr().wrapping_add(offset));
            assert_eq!(take(), [(object.ptr() as usize, out as u32)]);
            let args = [iid, &mut out as *mut *mut u8 as u32];
            let result =
                unsafe { call_thiscall_with_eax(fc4p_006dc6e0 as *const () as usize, object.ptr().add(8), &args, 0) };
            assert_eq!(result, (0xAABB_CC01, 0x7654_3210));
            assert_eq!(take(), [(object.ptr() as usize, out as u32)]);
        }
        for iid in [
            0u32,
            2,
            0x4522_94A9,
            0x4522_94AB,
            0x4778_2CCB,
            0x4778_2CCD,
            0xE4FD_A3D3,
            u32::MAX,
        ] {
            let mut out = 0x1234usize as *mut u8;
            let eax = if iid > 0x4778_2CCC {
                iid
            } else {
                iid.wrapping_sub(0x4522_94AA)
            } & !0xFF;
            assert_eq!(
                unsafe { fc4p_006da3c0(object.ptr(), 0x1122_3344, iid, &mut out) },
                0x1122_3344_0000_0000 | u64::from(eax)
            );
            assert_eq!(out as usize, 0x1234);
            assert!(take().is_empty());
        }
    }

    #[test]
    fn release_and_secondary_interfaces_share_the_signed_count() {
        let mut object = Object::new(0x1C8);
        let sim = unsafe { PlumbingSim::from_ptr(object.ptr()) };
        let mut first = [0usize; 30];
        first[4] = shutdown as *const () as usize;
        let mut second = [0usize; 30];
        second[29] = delete as *const () as usize;
        NEXT.with(|n| *n.borrow_mut() = second.as_ptr() as usize);
        for count in [2, 256, i32::MAX] {
            sim.set_reference_count(count);
            let before = unsafe { std::slice::from_raw_parts(object.ptr(), 0x1C8) }.to_vec();
            assert_eq!(
                unsafe { fc4p_006da610(object.ptr(), 0x1122_3344) },
                0x1122_3344_0000_0000 | (count - 1) as u64
            );
            let mut expected = before;
            expected[0x10..0x14].copy_from_slice(&(count - 1).to_le_bytes());
            assert_eq!(unsafe { std::slice::from_raw_parts(object.ptr(), 0x1C8) }, expected);
            assert!(take().is_empty());
        }
        for count in [1, 0, -1, i32::MIN] {
            unsafe { object.ptr().cast::<usize>().write(first.as_ptr() as usize) };
            sim.set_reference_count(count);
            assert_eq!(unsafe { fc4p_006da610(object.ptr(), 0) }, 0x1234_5678_0000_0000);
            assert_eq!(sim.reference_count(), count);
            assert_eq!(take(), [(object.ptr() as usize, 10), (object.ptr() as usize, 1)]);
        }
        for (function, offset) in [
            (fc4p_006dc6f0 as *const () as usize, 4),
            (fc4p_006dc700 as *const () as usize, 8),
        ] {
            sim.set_reference_count(2);
            assert_eq!(
                unsafe { call_thiscall_with_eax(function, object.ptr().add(offset), &[], 0) }.0,
                1
            );
            assert_eq!(sim.reference_count(), 1);
        }
    }
}
