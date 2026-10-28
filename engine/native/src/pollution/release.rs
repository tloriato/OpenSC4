// SPDX-License-Identifier: GPL-3.0-or-later
//! Release through the simulator's secondary interfaces.

use super::*;

game_fn! { fn release(this: *mut u8, edx: u32) -> u64 = "fastcall" 0x006E_5F20; }

port! {
    /// Releases the primary object through its message-target interface.
    #[port(address = 0x006E_7100, owner = "pollution", name = "message target Release adjustment")]
    pub unsafe extern "fastcall" fn fc4p_006e7100(this: *mut u8, edx: u32) -> u64 {
        // SAFETY: this interface is four bytes into the primary object.
        unsafe { release(this.wrapping_sub(4), edx) }
    }
}
port! {
    /// Releases the primary object through its prop-requester interface.
    #[port(address = 0x006E_7110, owner = "pollution", name = "prop requester Release adjustment")]
    pub unsafe extern "fastcall" fn fc4p_006e7110(this: *mut u8, edx: u32) -> u64 {
        // SAFETY: this interface is eight bytes into the primary object.
        unsafe { release(this.wrapping_sub(8), edx) }
    }
}
port! {
    /// Releases the primary object through its serializable interface.
    #[port(address = 0x006E_7120, owner = "pollution", name = "serializable Release adjustment")]
    pub unsafe extern "fastcall" fn fc4p_006e7120(this: *mut u8, edx: u32) -> u64 {
        // SAFETY: this interface is twelve bytes into the primary object.
        unsafe { release(this.wrapping_sub(12), edx) }
    }
}

/// This file's replacements.
pub const PORTS: &[Port] = &[fc4p_006e7100::PORT, fc4p_006e7110::PORT, fc4p_006e7120::PORT];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{fake, Object};
    use std::cell::Cell;
    thread_local! { static EXPECTED: Cell<u32> = const { Cell::new(0) }; }
    extern "fastcall" fn release_fake(this: *mut u8, edx: u32) -> u64 {
        EXPECTED.with(|e| assert_eq!(this as u32, e.get()));
        registers(0xABCD_0123, edx)
    }
    #[test]
    fn adjusts_each_interface_and_preserves_both_result_words() {
        let _fake = fake(0x006E_5F20, release_fake as *const () as usize);
        let mut object = Object::new(0x240);
        let base = object.ptr();
        EXPECTED.with(|e| e.set(base as u32));
        // SAFETY: each secondary interface is within the test object.
        unsafe {
            for (entry, offset) in [
                (fc4p_006e7100 as unsafe extern "fastcall" fn(*mut u8, u32) -> u64, 4),
                (fc4p_006e7110, 8),
                (fc4p_006e7120, 12),
            ] {
                assert_eq!(entry(base.add(offset), 0xDEAD_BEEF), 0xDEAD_BEEF_ABCD_0123);
            }
            EXPECTED.with(|e| e.set(0xFFFF_FFFC));
            assert_eq!(fc4p_006e7100(core::ptr::null_mut(), 9), registers(0xABCD_0123, 9));
        }
    }
}
