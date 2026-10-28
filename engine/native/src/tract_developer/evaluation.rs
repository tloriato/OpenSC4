// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Existing site capacity and candidate scoring.

use super::*;
use crate::x87::F80;

game_fn! { #[allow(dead_code)] fn advance_node(iterator: u32) -> u32 = "thiscall" 0x005D_17C0; }
game_fn! { #[allow(dead_code)] fn existing_capacity(this: u32, candidate: u32) -> u32 = "thiscall" 0x0070_55B0; }
game_fn! { #[allow(dead_code)] fn random_uniform(this: u32) -> St0 = "thiscall" 0x0091_372E; }
game_data! { static unsigned_bias: f32 = 0x00A8_0AA8; }
game_data! { static float_zero: f32 = 0x00A8_1054; }
game_data! { static scoring_random: u8 = 0x00B4_AFD8; }

/// Account for one lot. The aggregation path has distinct register arithmetic around the same calls.
unsafe fn account<const N: usize>(r: &mut Entry<N>, this: u32, candidate: u32, lot: u32, aggregate: bool) {
    // SAFETY: the enclosing port supplies live lot, developer and candidate objects.
    unsafe {
        r.ecx = word(this, 0x80);
        r.eax = word(r.ecx, 0);
        let lot_table = word(lot, 0);
        invoke(r, Conv::Thiscall, word(r.eax, 0x18) as usize, &[1]);
        let developer_type = r.eax;
        r.ecx = lot;
        invoke(r, Conv::Thiscall, word(lot_table, 0xC8) as usize, &[developer_type]);
        r.edx = word(lot, 0);
        r.ecx = lot;
        let capacity = r.eax & 0xFFFF;
        invoke(r, Conv::Thiscall, word(r.edx, 0x84) as usize, &[]);
        let offset = if r.eax == 4 { 0x28 } else { 0x24 };
        if aggregate {
            if r.eax == 4 {
                r.ecx = word(candidate, offset);
                r.eax = capacity;
                r.ecx = r.ecx.wrapping_add(r.eax);
                store(candidate, offset, r.ecx);
            } else {
                r.eax = word(candidate, offset);
                r.ecx = capacity;
                r.eax = r.eax.wrapping_add(r.ecx);
                store(candidate, offset, r.eax);
            }
            r.edx = word(lot, 0);
            r.ecx = lot;
            invoke(r, Conv::Thiscall, word(r.edx, 0x24) as usize, &[]);
        } else {
            store(candidate, offset, word(candidate, offset).wrapping_add(capacity));
            r.eax = word(lot, 0);
            r.ecx = lot;
            invoke(r, Conv::Thiscall, word(r.eax, 0x24) as usize, &[]);
        }
        if r.eax != 0 {
            r.edx = word(r.eax, 0);
            r.ecx = r.eax;
            invoke(r, Conv::Thiscall, word(r.edx, 0x2C) as usize, &[]);
            r.eax &= 0xFF;
        } else {
            r.eax = 0;
        }
        if (word(candidate, 0x2C) as i32) < r.eax as i32 {
            store(candidate, 0x2C, r.eax);
        }
        r.eax = word(lot, 0);
        r.ecx = lot;
        invoke(r, Conv::Thiscall, word(r.eax, 0xB8) as usize, &[0]);
        if r.eax != 0 {
            r.ecx = word(this, 0x1C);
            r.edx = word(r.ecx, 0);
            let building = r.eax;
            invoke(r, Conv::Thiscall, word(r.edx, 0x9C) as usize, &[building]);
            r.eax = word(r.eax, 4);
            if (word(candidate, 0x30) as i32) < r.eax as i32 {
                store(candidate, 0x30, r.eax);
            }
        }
    }
}

register_port! {
    /// Keep nonnegative cached sums. Otherwise reset capacity, stage and wealth, then account for
    /// the lot or every lot in an aggregation, using the low capacity word and low stage byte.
    0x0070_55B0, fc4p_007055b0, find_capacity, 1, 4, "cSC4TractDeveloper::FindExistingCapacity", |r| {
        let this = r.ecx;
        let candidate = r.args[0];
        let c = Candidate::from_ptr(candidate as *mut u8);
        r.ecx = c.abandoned() as u32;
        r.eax = 0;
        if c.abandoned() >= 0 && c.occupied() >= 0 { return; }
        c.set_stage(0); c.set_abandoned(0); c.set_occupied(0); c.set_wealth(0);
        r.eax = (c.kind() as u32).wrapping_sub(1);
        match c.kind() {
            1 | 2 | 4 => { account(r, this, candidate, c.lot() as u32, false); }
            3 => {
                r.ecx = c.lot_header();
                r.eax = word(r.ecx, 8);
                let mut iterator = r.eax;
                if r.eax == r.ecx { return; }
                loop {
                    let lot = word(r.eax, 0x10);
                    account(r, this, candidate, lot, true);
                    r.ecx = &mut iterator as *mut u32 as u32;
                    invoke(r, Conv::Thiscall, crate::abi::target(0x005D_17C0), &[]);
                    r.eax = iterator;
                    if r.eax == c.lot_header() { break; }
                }
            }
            _ => {}
        }
    }
}

unsafe fn draw<const N: usize>(r: &mut Entry<N>) -> F80 {
    let mut call = RawCall::new(Conv::Thiscall, crate::abi::target(0x0091_372E), r.ecx, &[]);
    call.eax = r.eax;
    call.edx = r.edx;
    // SAFETY: the random method takes no stack words and returns exactly one x87 value.
    unsafe {
        core::arch::asm!(
            "mov eax, [edi + 4]", "mov ecx, [edi + 8]", "mov edx, [edi + 12]",
            "call dword ptr [edi]",
            "mov [edi + 4], eax", "mov [edi + 8], ecx", "mov [edi + 12], edx",
            "fstp tbyte ptr [edi + 32]",
            in("edi") &mut call, clobber_abi("C"), out("st(0)") _,
        );
    }
    r.eax = call.eax;
    r.ecx = call.ecx;
    r.edx = call.edx;
    call.st0
}

unsafe fn float(base: u32, offset: u32) -> F80 {
    // SAFETY: the caller identifies a readable single precision field.
    unsafe { F80::from_f32(read_unaligned(base.wrapping_add(offset) as *const f32)) }
}

unsafe fn save_score(candidate: u32, value: F80) {
    // SAFETY: the candidate has a writable score at +0x34.
    unsafe {
        store(candidate, 0x34, value.to_f32().to_bits());
    }
}

register_port! {
    /// Score candidates in vector order. Capacity penalties and lot costs are stored before the
    /// network term. Network side sums remain in the x87 across the intermediate score stores.
    /// Commercial sites skip every network term; industrial sites draw once per candidate.
    0x0070_5750, fc4p_00705750, score_candidates, 0, 0, "cSC4TractDeveloper::ScoreCandidateLots", |r| {
        let this = r.ecx;
        let td = TractDeveloper::from_ptr(this as *mut u8);
        r.ecx = td.developer();
        r.eax = word(r.ecx, 0);
        invoke(r, Conv::Thiscall, word(r.eax, 0x14) as usize, &[]);
        let zone = r.eax;
        r.eax = r.eax.wrapping_sub(1);
        if r.eax != 0 { r.eax = r.eax.wrapping_sub(1); if r.eax != 0 { r.eax = r.eax.wrapping_sub(1); } }
        let (road, avenue, rail, random, lot_costs) = match zone {
            1 => (1.0, 2.0, 10.0, 0.0, true),
            2 => (-2.0, -5.0, 0.0, 0.0, true),
            3 => (-1.0, -1.0, -5.0, 20.0, false),
            _ => (0.0, 0.0, 0.0, 0.0, true),
        };
        let mut position = td.candidates_begin();
        if position == td.candidates_end() { return; }
        loop {
            let candidate = word(position, 0);
            store(candidate, 0x34, 0);
            if word(candidate, 0) != 1 {
                r.ecx = this;
                invoke(r, Conv::Thiscall, crate::abi::target(0x0070_55B0), &[candidate]);
                let abandoned = F80::from_i32(word(candidate, 0x28) as i32) * float(this, 0x54);
                let occupied = F80::from_i32(word(candidate, 0x24) as i32) * float(this, 0x50);
                save_score(candidate, (abandoned + occupied) + float(candidate, 0x34));
            }
            if lot_costs {
                r.eax = word(candidate, 0);
                if r.eax == 3 {
                    r.ecx = word(candidate, 0x1C).wrapping_sub(1);
                    let mut count = F80::from_i32(r.ecx as i32);
                    if (r.ecx as i32) < 0 { count = count + F80::from_f32(read_unaligned(unsigned_bias())); }
                    save_score(candidate, count * float(this, 0x58) + float(candidate, 0x34));
                }
                if r.eax == 4 { save_score(candidate, float(this, 0x5C) + float(candidate, 0x34)); }
            }
            let comparison = F80::from_f32(rail).compare(F80::from_f32(read_unaligned(float_zero())));
            r.eax = (r.eax & 0xFFFF_0000) | u32::from(comparison.status_word());
            if !comparison.equal() {
                r.ecx = word(candidate, 0x88);
                r.eax = word(candidate, 0x94);
                r.edx = word(candidate, 0x7C);
                let term = |base: u32, side: u32, weight: f32| F80::from_i32(word(base, side * 4) as i32) * F80::from_f32(weight);
                let mut score = ((term(r.ecx, 0, avenue) + term(r.eax, 0, rail)) + term(r.edx, 0, road)) + float(candidate, 0x34);
                save_score(candidate, score);
                for side in 1..4 {
                    let side_score = (term(r.edx, side, road) + term(r.ecx, side, avenue)) + term(r.eax, side, rail);
                    score = if side == 3 { side_score + score } else { score + side_score };
                    save_score(candidate, score);
                }
            }
            let comparison = F80::from_f32(random).compare(F80::from_f32(read_unaligned(float_zero())));
            r.eax = (r.eax & 0xFFFF_0000) | u32::from(comparison.status_word());
            if !comparison.equal() {
                r.ecx = scoring_random() as u32;
                let uniform = draw(r);
                save_score(candidate, uniform * F80::from_f32(random) + float(candidate, 0x34));
            }
            r.eax = td.candidates_end();
            position = position.wrapping_add(4);
            if position == r.eax { break; }
        }
    }
}

pub const PORTS: &[Port] = &[fc4p_007055b0::PORT, fc4p_00705750::PORT];

#[cfg(test)]
mod tests;
