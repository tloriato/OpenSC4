// SPDX-License-Identifier: GPL-3.0-or-later
// FreeCity4 - Copyright (C) 2026 FreeCity4 contributors

//! Plant wear, distress, demolition, and radiation.

use super::*;

layout! {
    /// Ageing counters and the demolition service.
    struct AgeSim {
        0x20 => department / set_department: *mut u8,
        0x34 => demolition / set_demolition: *mut u8,
        0x308 => overworked / set_overworked: u32,
        0x30C => aged / set_aged: u32,
        0x310 => distressed / set_distressed: u32,
    }
}
game_fn! { fn get_float(holder: *mut u8, id: u32, out: *mut f32) -> u32 = "C" 0x005F_D4F0; }
game_fn! { fn get_int(holder: *mut u8, id: u32, out: *mut i32) -> u32 = "C" 0x005F_D450; }
game_fn! { fn get_uint(holder: *mut u8, id: u32, out: *mut u32) -> u32 = "C" 0x005F_D480; }
game_fn! { fn set_float(holder: *mut u8, id: u32, value: f32) -> u32 = "C" 0x005F_E4F0; }
game_fn! { fn curve(curve: *mut u8, input: f32) -> St0 = "thiscall" 0x005F_4250; }
game_fn! { fn allocate(size: u32) -> *mut u8 = "C" 0x0090_CF54; }
game_fn! { fn free(node: *mut u8) -> u32 = "C" 0x0090_CF63; }
game_fn! { fn push_front(list: *mut u32, value: *mut *mut u8) -> u32 = "thiscall" 0x0046_7A10; }
game_fn! { fn clear_list(list: *mut u32) -> u32 = "thiscall" 0x0047_3EC0; }
game_fn! { fn post(id: u32, a: u32, b: u32, c: u32, d: u32, occupant: *mut u8) -> u32 = "C" 0x005F_98C0; }
game_data! { static global_city: *mut u8 = 0x00B4_3CEC; }
game_data! { static lots: *mut u8 = 0x00B4_3D08; }
game_data! { static pollution: *mut u8 = 0x00B4_3D9C; }
game_data! { static one: f32 = 0x00B0_D424; }
game_data! { static aged_wear: f64 = 0x00AB_2880; }
game_data! { static zero: f32 = 0x00A8_1054; }

/// Merge the x87 status into AX, retaining the last result's high word.
fn status(eax: u32, comparison: crate::x87::Compared) -> u32 {
    (eax & 0xFFFF_0000) | u32::from(comparison.status_word())
}

port! {
    /// Advance wear from usage and funding curves, count plant conditions, and apply distress or
    /// demolition at the stored thresholds. The caller's argument word is reused for efficiency,
    /// funding, and the temporary lot-list header.
    #[port(address = 0x006E_C8F0, owner = "power", name = "cSC4PowerSimulator::AgePowerPlant", naked)]
    pub unsafe extern "C" fn fc4p_006ec8f0() {
        core::arch::naked_asm!(
            "lea edx, [esp + 4]", "call {body}", "ret 4", body = sym age,
        )
    }
}

unsafe extern "fastcall" fn age(this: *mut u8, arg: *mut u32) -> u32 {
    unsafe {
        let sim = AgeSim::from_ptr(this);
        let mut wear = 0.0f32;
        let mut rate = 0.0f32;
        let mut capacity = 0i32;
        let mut used = 0i32;
        let mut first_threshold = 99.0f32;
        let mut second_threshold = 99.0f32;
        let mut flags = 0u32;
        let funding = vcall!(sim.department(), 0x28 => fn() -> *mut f32);
        let occupant = arg.read() as *mut u8;
        let holder = vcall!(occupant, 0x18 => fn() -> *mut u8);
        get_float(holder, 0xC910_B4CF, &mut wear);
        get_float(holder, 0xEA1C_F220, &mut rate);
        get_int(holder, 0x2781_2852, &mut capacity);
        get_int(holder, 0x08D9_763D, &mut used);
        get_float(holder, 0xEA1C_F221, &mut first_threshold);
        get_float(holder, 0xEA1C_F222, &mut second_threshold);
        get_uint(holder, 0x6A43_150F, &mut flags);
        let efficiency = vcall!(this, 0xB4 => fn(*mut u32, *mut u8) -> *mut f32, arg, occupant);
        let effective = multiply_float(F80::from_i32(capacity), efficiency.read_unaligned()).ftol() as i32;
        let usage = if effective > 0 && used > 0 {
            (F80::from_i32(used) / F80::from_i32(effective)).to_f32()
        } else {
            0.0
        };
        // Save the funding value before either curve call and reuse the original argument word.
        arg.write(funding.cast::<u32>().read_unaligned());
        let usage_factor = curve(this.add(0x3A8), usage).to_f32();
        let funding_factor = curve(this.add(0x398), f32::from_bits(arg.read()));
        wear = add_float(multiply_float(multiply_float(funding_factor, usage_factor), rate), wear).to_f32();
        let eax = set_float(holder, 0xC910_B4CF, wear);
        // The first counter increments when the first threshold exceeds one.
        let compared = F80::from_f32(first_threshold).compare(F80::from_f32(*one()));
        if compared.status_word() & 0x4100 == 0 {
            sim.set_overworked(sim.overworked().wrapping_add(1));
        }
        let compared = F80::from_f32(wear).compare(F80::from_f64(*aged_wear()));
        if compared.status_word() & 0x100 == 0 {
            sim.set_aged(sim.aged().wrapping_add(1));
        }
        let compared = F80::from_f32(wear).compare(F80::from_f32(second_threshold));
        let mut eax = status(eax, compared);
        if compared.status_word() & 0x100 != 0 {
            let compared = F80::from_f32(wear).compare(F80::from_f32(first_threshold));
            eax = status(eax, compared);
            if compared.status_word() & 0x100 != 0 {
                return eax;
            }
        }
        let manager = *lots();
        let mut position = core::mem::MaybeUninit::<[f32; 3]>::uninit();
        let p = position.as_mut_ptr().cast::<f32>();
        vcall!(occupant, 0x20 => fn(*mut f32) -> u32, p);
        let mut x = core::mem::MaybeUninit::<i32>::uninit();
        let mut z = core::mem::MaybeUninit::<i32>::uninit();
        vcall!(*global_city(), 0x17C => fn(f32, f32, *mut i32, *mut i32) -> u32,
            p.read(), p.add(2).read(), x.as_mut_ptr(), z.as_mut_ptr());
        let lot = vcall!(manager, 0x18 => fn(i32, i32, u32) -> *mut u8, x.assume_init(), z.assume_init(), 0);
        if lot.is_null() {
            return 0;
        }
        let compared = F80::from_f32(wear).compare(F80::from_f32(second_threshold));
        eax = status(lot as u32, compared);
        if compared.status_word() & 0x100 == 0 && flags > 0 {
            let header = allocate(12);
            header.cast::<*mut u8>().write(header);
            header.add(4).cast::<*mut u8>().write(header);
            arg.write(header as u32);
            let mut lot_ref = lot;
            vcall!(lot, 4 => fn() -> u32);
            push_front(arg, &mut lot_ref);
            vcall!(lot, 8 => fn() -> u32);
            vcall!(sim.demolition(), 0x1C => fn(u32, *mut u32, u32, u32, u32, u32, u32, u32) -> u32,
                1, arg, 0, flags | 0x300, 0, 0, 0, 0);
            let mut radius = 0.0f32;
            let mut strength = 0.0f32;
            get_float(holder, 0xAA5C_04C8, &mut radius);
            get_float(holder, 0xAA5C_04C5, &mut strength);
            let compared = F80::from_f32(radius).compare(F80::from_f32(*zero()));
            if compared.status_word() & 0x4100 == 0 {
                vcall!(*pollution(), 0x108 => fn(*mut f32, f32, f32) -> u32, p, strength, radius);
                let mut kind = core::mem::MaybeUninit::<u32>::uninit();
                if get_uint(holder, 0x2781_2853, kind.as_mut_ptr()) as u8 != 0 && kind.assume_init() == 5 {
                    post(0xAA55_2F77, 0, 0, 0, 0, occupant);
                }
            }
            clear_list(arg);
            let header = arg.read() as *mut u8;
            if header.is_null() {
                0
            } else {
                free(header)
            }
        } else {
            let compared = F80::from_f32(wear).compare(F80::from_f32(first_threshold));
            eax = status(eax, compared);
            if compared.status_word() & 0x100 == 0 {
                sim.set_distressed(sim.distressed().wrapping_add(1));
                vcall!(lot, 0x90 => fn(u32) -> u32, 2)
            } else {
                eax
            }
        }
    }
}

pub const PORTS: &[Port] = &[fc4p_006ec8f0::PORT];

#[cfg(test)]
mod tests;
