// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Identity, reference counting and shared record helpers.

use super::*;
use core::ptr::write_unaligned;

game_fn! { fn destroy_range(begin: *mut u8, end: *mut u8, allocator: *mut u8) -> u32 = "C" 0x0045_9170; }
game_fn! { fn free_vector(storage: *mut u8) -> u32 = "C" 0x0090_CF63; }

port! {
    /// Clears the defined fields of a 64-byte lot configuration record, retaining its padding.
    #[port(address = 0x0045_91E0, owner = "lotdeveloper", name = "InitializeLotConfigurationRecord")]
    pub unsafe extern "thiscall" fn fc4p_004591e0(this: *mut u8) -> *mut u8 {
        unsafe {
            for offset in [0, 4] { write_unaligned(this.add(offset).cast::<u32>(), 0); }
            write_unaligned(this.add(8).cast::<u16>(), 0);
            this.add(10).write(0);
            for offset in [12, 16, 20] { write_unaligned(this.add(offset).cast::<u32>(), 0); }
            this.add(24).write(0);
            for offset in [28, 48, 52, 56, 60, 40, 36, 32] {
                write_unaligned(this.add(offset).cast::<u32>(), 0);
            }
            this
        }
    }
}

port! {
    /// Destroys the vector's elements before rereading and freeing its storage. The vector words stay intact.
    #[port(address = 0x0045_9220, owner = "lotdeveloper", name = "DestroyLotConfigurationVector")]
    pub unsafe extern "thiscall" fn fc4p_00459220(this: *mut u8) -> u32 {
        unsafe {
            let begin = this.add(12).cast::<*mut u8>().read_unaligned();
            let end = this.add(16).cast::<*mut u8>().read_unaligned();
            let mut allocator = ((this as u32) >> 24) as u8;
            let eax = destroy_range(begin, end, &mut allocator);
            let storage = this.add(12).cast::<*mut u8>().read_unaligned();
            if storage.is_null() { eax } else { free_vector(storage) }
        }
    }
}

port! {
    /// Initializes the record in Rust and leaves ECX equal to 255 for its caller.
    #[port(address = 0x005C_A770, owner = "lotdeveloper", name = "InitializeLotCellRecord", naked)]
    pub unsafe extern "C" fn fc4p_005ca770() {
        core::arch::naked_asm!("call {body}", "mov ecx, 255", "ret", body = sym initialize_cell_record)
    }
}

unsafe extern "thiscall" fn initialize_cell_record(this: *mut u8) -> *mut u8 {
    unsafe {
        write_unaligned(this.cast::<u32>(), 0);
        for offset in 4..8 {
            this.add(offset).write(0);
        }
        for offset in [12, 13, 11, 10, 9, 8] {
            this.add(offset).write(255);
        }
        this
    }
}

port! {
    /// Returns the force-empty byte, preserving the other bytes of EAX.
    #[port(address = 0x006A_AF40, owner = "lotdeveloper", name = "cSC4LotDeveloper::GetForceEmptyLotBaseTextures", naked)]
    pub unsafe extern "C" fn fc4p_006aaf40() {
        core::arch::naked_asm!("mov al, byte ptr [ecx + 0x5c]", "ret")
    }
}

port! {
    /// Returns the lot developer's class identifier.
    #[port(address = 0x006C_3B00, owner = "lotdeveloper", name = "cSC4LotDeveloper::GetGZCLSID", naked)]
    pub unsafe extern "C" fn fc4p_006c3b00() {
        core::arch::naked_asm!("mov eax, 0xa990bfe0", "ret")
    }
}

port! {
    /// Receives lot in ECX, x in EDX and z in EAX. Zones 1..6 use a diagonal pattern; 7..9 use a grid.
    #[port(address = 0x006C_3B10, owner = "lotdeveloper", name = "IsEmptyLotSuitableForZot", naked)]
    pub unsafe extern "C" fn fc4p_006c3b10() {
        core::arch::naked_asm!("push eax", "call {body}", "ret", body = sym suitable_for_zot)
    }
}

unsafe extern "fastcall" fn suitable_for_zot(lot: *mut u8, x: i32, z: i32) -> u32 {
    unsafe {
        let zone = vcall!(lot, 0x7C => fn() -> u32).wrapping_sub(1);
        let suitable = match zone {
            0..=5 => x.wrapping_add(z) % 4 == 0,
            6..=8 => x % 4 == 0 && z % 4 == 0,
            _ => false,
        };
        // In-range dispatch loads a one-byte table index without changing EAX.
        (zone & !255) | u32::from(suitable)
    }
}

port! {
    /// Finds one of the three interfaces and AddRefs through the primary table. Unknown identifiers leave
    /// the output untouched and retain the identifier arithmetic in EAX's upper bytes.
    #[port(address = 0x006C_3B80, owner = "lotdeveloper", name = "cSC4LotDeveloper::QueryInterface")]
    pub unsafe extern "fastcall" fn fc4p_006c3b80(this: *mut u8, entry_edx: u32, iid: u32, output: *mut *mut u8) -> u64 {
        unsafe {
            let offset = match iid {
                1 | 0x4818_E5D2 => 0,
                0x4522_94AA => 4,
                0xE4FD_A3D4 => 8,
                _ => {
                    let eax = if iid < 0x4818_E5D2 {
                        iid.wrapping_sub(0x4522_94AA)
                    } else { iid };
                    return (u64::from(entry_edx) << 32) | u64::from(eax & !255);
                }
            };
            output.write_unaligned(if this.is_null() { this } else { this.wrapping_add(offset) });
            let t = table(this);
            let mut call = virtual_call(this, 4, &[], t, output as u32, false);
            call.eax = (call.eax & !255) | 1;
            result(call)
        }
    }
}

port! {
    /// Decrements counts greater than one. At one or below, shuts down and calls the deleting destructor.
    #[port(address = 0x006C_49D0, owner = "lotdeveloper", name = "cSC4LotDeveloper::Release")]
    pub unsafe extern "thiscall" fn fc4p_006c49d0(this: *mut u8) -> u32 {
        unsafe {
            let developer = LotDeveloper::from_ptr(this);
            let count = developer.references();
            if count > 1 {
                let remaining = count.wrapping_sub(1);
                developer.set_references(remaining);
                return remaining as u32;
            }
            vcall!(this, 0x10 => fn() -> u32);
            vcall!(this, 0x104 => fn(u32) -> u32, 1);
            0
        }
    }
}

pub const PORTS: &[Port] = &[
    fc4p_004591e0::PORT,
    fc4p_00459220::PORT,
    fc4p_005ca770::PORT,
    fc4p_006aaf40::PORT,
    fc4p_006c3b00::PORT,
    fc4p_006c3b10::PORT,
    fc4p_006c3b80::PORT,
    fc4p_006c49d0::PORT,
];
