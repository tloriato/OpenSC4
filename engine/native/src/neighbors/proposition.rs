// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Dated deal propositions and their two independently retained list references.

use super::{pair, pointer, virtual_call, word};
use crate::abi::{call_raw, Conv, RawCall};
use crate::port::Port;
use crate::x87::F80;

game_fn! { fn deal_node(list: *mut u8, edx: u32, count: u32, hint: u32) -> u64 = "fastcall" 0x0042_0FD0; }
game_fn! { fn property_float(holder: *mut u8, id: u32, out: *mut u32) -> u64 = "C" 0x005F_D4F0; }
game_return_fn! { fn proposition_construct_random(this: *mut u8, seed: i32) -> u64 = 0x0091_3A5C, fc4r_006d0628; }
game_fn! { fn random_range(this: *mut u8, low_lo: u32, low_hi: u32, high_lo: u32, high_hi: u32) -> St0 = "thiscall" 0x0091_3761; }
game_fn! { fn date_from_time(this: *mut u8, edx: u32, time: u32) -> u64 = "fastcall" 0x0091_2DFA; }
game_fn! { fn day_number(this: *mut u8, edx: u32) -> u64 = "fastcall" 0x0091_297E; }
game_fn! { fn date_from_day(this: *mut u8, edx: u32, day: u32) -> u64 = "fastcall" 0x0091_2E7B; }
game_fn! { #[allow(dead_code)] fn destroy_date(this: *mut u8) = "thiscall" 0x0091_28BF; }
game_data! { static random: [u8; 24] = 0x00B4_9B18; }
game_data! { static random_initialized: u32 = 0x00B4_9B30; }
game_data! { static time_service: *mut u8 = 0x00B4_3D5C; }
game_data! { static term_epsilon: f64 = 0x00AA_2270; }
game_data! { static half: f32 = 0x00A8_4D2C; }
game_data! { static integer_bias: f32 = 0x00AB_1CAC; }

unsafe fn append(list: *mut u8, deal: *mut u8, edx: u32) -> u64 {
    unsafe {
        let header = pointer(list, 0);
        let mut result = deal_node(list, edx, 1, 0);
        let node = result as u32 as *mut u8;
        let destination = node.wrapping_add(8);
        if !destination.is_null() {
            destination.cast::<*mut u8>().write_unaligned(deal);
            if !deal.is_null() {
                result = virtual_call(deal, 4, &[], destination as u32, word(deal, 0));
            }
        }
        let previous = pointer(header, 4);
        node.cast::<*mut u8>().write_unaligned(header);
        node.add(4).cast::<*mut u8>().write_unaligned(previous);
        previous.cast::<*mut u8>().write_unaligned(node);
        header.add(4).cast::<*mut u8>().write_unaligned(node);
        pair(previous as u32, (result >> 32) as u32)
    }
}

unsafe fn destroy(date: *mut u8, result: u64) -> u64 {
    let mut call = RawCall::new(Conv::Thiscall, crate::abi::target(0x0091_28BF), date as u32, &[]);
    call.eax = result as u32;
    call.edx = (result >> 32) as u32;
    unsafe {
        call_raw(&mut call);
    }
    pair(call.eax, call.edx)
}

/// Rounds with the game's single-precision bias, then corrects an exact next-integer comparison.
/// The candidate is written over the incoming type word, just as the caller's scratch storage is used.
unsafe fn term_years(years: f32, scratch: *mut u32) -> i32 {
    unsafe {
        let biased =
            ((F80::from_f32(years) - F80::from_f32(half().read())) + F80::from_f32(integer_bias().read())).to_f32();
        let lower = biased.to_bits().wrapping_add(0xB4C0_0000) as i32;
        let candidate = lower.wrapping_add(1);
        scratch.write_unaligned(candidate as u32);
        let status: u16;
        core::arch::asm!(
            "fild dword ptr [{candidate}]", "fld dword ptr [{years}]", "fucompp", "fnstsw ax",
            candidate = in(reg) &candidate, years = in(reg) &years,
            out("ax") status, out("st(0)") _, out("st(1)") _,
        );
        if status & 0x4400 == 0x4000 {
            candidate
        } else {
            lower
        }
    }
}

port! {
    /// Proposes a deal, optionally makes it audible and inserts it into the simulator, sets its amount,
    /// draws its term, and sets current/start/expiration dates before inserting it into the output list.
    /// The entry passes the original words because the type, city, and add words become scratch storage.
    #[port(address = 0x006D_0520, owner = "neighbors", name = "cSC4NeighborsSimulator::create_deal_proposition", naked)]
    pub unsafe extern "C" fn fc4p_006d0520() {
        core::arch::naked_asm!("lea edx, [esp + 4]", "call {}", "ret 20", sym create_proposition);
    }
}

unsafe extern "fastcall" fn create_proposition(this: *mut u8, args: *mut u32) -> u64 {
    // SAFETY: the entry supplies the simulator, five original argument words, and live game services.
    unsafe {
        let mut deal: *mut u8 = core::ptr::null_mut();
        let mut result = virtual_call(
            this,
            0x60,
            &[
                args.read_unaligned(),
                args.add(2).read_unaligned(),
                &mut deal as *mut *mut u8 as u32,
            ],
            word(this, 0),
            args.add(2).read_unaligned(),
        );
        if result as u8 == 0 {
            if !deal.is_null() {
                result = virtual_call(deal, 8, &[], result as u32, word(deal, 0));
            }
            return result & !0xFF;
        }
        let add = args.add(4).read_unaligned() as u8;
        result = (result & !0xFF) | u64::from(add);
        if add != 0 {
            result = virtual_call(deal, 0x94, &[0x80, 0], result as u32, word(deal, 0));
            append(this.add(0x20), deal, (result >> 32) as u32);
        }
        result = virtual_call(
            deal,
            0x2C,
            &[args.add(1).read_unaligned()],
            word(deal, 0),
            args.add(1).read_unaligned(),
        );
        args.add(4).write_unaligned(1.0f32.to_bits());
        args.add(2).write_unaligned(5.0f32.to_bits());
        result = virtual_call(this, 0x94, &[], word(this, 0), (result >> 32) as u32);
        let tuning = result as u32 as *mut u8;
        result = virtual_call(tuning, 0x18, &[], tuning as u32, word(tuning, 0));
        let holder = result as u32 as *mut u8;
        property_float(holder, 0xAA46_934B, args.add(4));
        property_float(holder, 0xEA46_934F, args.add(2));
        let low = f32::from_bits(args.add(4).read_unaligned());
        let high = f32::from_bits(args.add(2).read_unaligned());
        let cmp = F80::from_f32(low).compare(F80::from_f32(high));
        if !cmp.less() && !cmp.equal() && !cmp.unordered() {
            args.add(2).write_unaligned(low.to_bits());
        }
        if random_initialized().read() & 1 == 0 {
            random_initialized().write(random_initialized().read() | 1);
            proposition_construct_random(random().cast(), -1);
        }
        let high = (F80::from_f32(f32::from_bits(args.add(2).read_unaligned())) + F80::from_f64(term_epsilon().read()))
            .to_f64();
        let low = F80::from_f32(f32::from_bits(args.add(4).read_unaligned())).to_f64();
        let low = low.to_bits();
        let high = high.to_bits();
        let years = random_range(
            random().cast(),
            low as u32,
            (low >> 32) as u32,
            high as u32,
            (high >> 32) as u32,
        )
        .to_f32();
        args.add(4).write_unaligned(years.to_bits());
        let time = time_service().read();
        result = virtual_call(time, 0x1C, &[], word(time, 0), 0);
        let mut dates = core::mem::MaybeUninit::<[u32; 8]>::uninit();
        let today = dates.as_mut_ptr().cast::<u8>();
        let expiration = today.add(16);
        result = date_from_time(today, (result >> 32) as u32, result as u32);
        let years = term_years(f32::from_bits(args.add(4).read_unaligned()), args);
        result = day_number(today.add(8), (result >> 32) as u32);
        let expires = (result as u32).wrapping_add((years as u32).wrapping_mul(365));
        date_from_day(expiration, (result >> 32) as u32, expires);
        virtual_call(deal, 0x74, &[today.add(8) as u32], today.add(8) as u32, word(deal, 0));
        virtual_call(deal, 0x64, &[today.add(8) as u32], today.add(8) as u32, word(deal, 0));
        result = virtual_call(
            deal,
            0x6C,
            &[expiration.add(8) as u32],
            expiration.add(8) as u32,
            word(deal, 0),
        );
        result = append(args.add(3).read_unaligned() as *mut u8, deal, (result >> 32) as u32);
        result = destroy(expiration, result);
        result = destroy(today, result);
        if !deal.is_null() {
            result = virtual_call(deal, 8, &[], word(deal, 0), (result >> 32) as u32);
        }
        (result & !0xFF) | 1
    }
}

pub const PORTS: &[Port] = &[fc4p_006d0520::PORT];

#[cfg(test)]
mod tests;
