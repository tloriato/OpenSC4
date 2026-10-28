// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Assigning candidate records and their contained lot sets.

use super::*;

game_fn! {
    #[allow(dead_code)]
    fn dispose_lot_nodes(tree: *mut u8, node: u32) -> u32 = "thiscall" 0x0070_4990;
}
game_fn! {
    #[allow(dead_code)]
    fn copy_lot_nodes(tree: *mut u8, source: u32, parent: u32) -> u32 = "thiscall" 0x0070_5380;
}
game_fn! {
    #[allow(dead_code)]
    fn assign_lot_set(destination: *mut u8, source: *const u8) -> u32 = "thiscall" 0x0070_5950;
}
game_fn! {
    #[allow(dead_code)]
    fn assign_integer_vector(destination: *mut u8, source: *const u8) -> u32 = "thiscall" 0x0050_DB90;
}

register_port! {
    /// Assign a lot set: keep self-assignment unchanged, dispose the old nonempty tree, copy a nonempty
    /// source through its original helper, and find the copied tree's first and last nodes.
    0x0070_5950, fc4p_00705950, assign_set, 1, 4, "AssignCandidateLotSet", |r| {
        let this = r.ecx;
        let source = r.args[0];
        if this == source { r.eax = this; return; }
        r.eax = word(this, 4);
        if r.eax != 0 {
            r.eax = word(this, 0);
            r.ecx = word(r.eax, 4);
            let root = r.ecx;
            r.ecx = this;
            invoke(r, Conv::Thiscall, crate::abi::target(0x0070_4990), &[root]);
            r.eax = word(this, 0);
            store(r.eax, 8, r.eax);
            r.edx = word(this, 0);
            store(r.edx, 4, 0);
            r.eax = word(this, 0);
            store(r.eax, 0xC, r.eax);
            store(this, 4, 0);
        }
        store(this, 4, 0);
        r.eax = word(source, 0);
        r.eax = word(r.eax, 4);
        if r.eax == 0 {
            r.ecx = word(this, 0);
            store(r.ecx, 4, 0);
            r.eax = word(this, 0);
            store(r.eax, 8, r.eax);
            r.eax = word(this, 0);
            store(r.eax, 0xC, r.eax);
            r.eax = this;
            return;
        }
        let header = word(this, 0);
        r.ecx = this;
        invoke(r, Conv::Thiscall, crate::abi::target(0x0070_5380), &[r.eax, header]);
        store(header, 4, r.eax);
        r.edx = word(this, 0);
        r.eax = word(r.edx, 4);
        r.ecx = word(r.eax, 8);
        while r.ecx != 0 {
            r.eax = r.ecx;
            r.ecx = word(r.eax, 8);
        }
        store(r.edx, 8, r.eax);
        r.edx = word(this, 0);
        r.ecx = word(r.edx, 4);
        r.eax = word(r.ecx, 0xC);
        while r.eax != 0 {
            r.ecx = r.eax;
            r.eax = word(r.ecx, 0xC);
        }
        store(r.edx, 0xC, r.ecx);
        r.edx = word(source, 4);
        store(this, 4, r.edx);
        r.eax = this;
    }
}

register_port! {
    /// Assign a candidate, retaining padding. Assign the lot set and four network vectors through
    /// their original helpers, and copy scalar and rectangle words without interpreting float bits.
    0x0070_7240, fc4p_00707240, assign_candidate, 1, 4, "AssignGrowthCandidate", |r| {
        let this = r.ecx;
        let source = r.args[0];
        r.eax = word(source, 0); store(this, 0, r.eax);
        r.ecx = word(source, 4); store(this, 4, r.ecx);
        r.edx = word(source, 8); store(this, 8, r.edx);
        r.eax = word(source, 0xC); store(this, 0xC, r.eax);
        r.ecx = word(source, 0x10); store(this, 0x10, r.ecx);
        r.edx = (r.edx & !0xFF) | u32::from(*(source.wrapping_add(0x14) as *const u8));
        r.ecx = source.wrapping_add(0x18);
        *(this.wrapping_add(0x14) as *mut u8) = r.edx as u8;
        r.eax = (r.eax & !0xFF) | u32::from(*(source.wrapping_add(0x15) as *const u8));
        let source_set = r.ecx;
        r.ecx = this.wrapping_add(0x18);
        *(this.wrapping_add(0x15) as *mut u8) = r.eax as u8;
        invoke(r, Conv::Thiscall, crate::abi::target(0x0070_5950), &[source_set]);
        r.edx = word(source, 0x24); store(this, 0x24, r.edx);
        r.eax = word(source, 0x28); store(this, 0x28, r.eax);
        r.ecx = word(source, 0x2C); store(this, 0x2C, r.ecx);
        r.edx = word(source, 0x30); store(this, 0x30, r.edx);
        r.eax = word(source, 0x34); store(this, 0x34, r.eax);
        r.ecx = source.wrapping_add(0x38);
        r.eax = word(r.ecx, 0);
        r.edx = this.wrapping_add(0x38);
        store(r.edx, 0, r.eax);
        r.eax = word(r.ecx, 4); store(r.edx, 4, r.eax);
        r.eax = word(r.ecx, 8); store(r.edx, 8, r.eax);
        r.ecx = word(r.ecx, 0xC); store(r.edx, 0xC, r.ecx);
        for i in 0..9 {
            let value = word(source, 0x48 + i * 4);
            store(this, 0x48 + i * 4, value);
        }
        r.ecx = 0;
        r.edx = (r.edx & !0xFF) | u32::from(*(source.wrapping_add(0x6C) as *const u8));
        r.eax = source.wrapping_add(0x70);
        r.ecx = this.wrapping_add(0x70);
        *(this.wrapping_add(0x6C) as *mut u8) = r.edx as u8;
        invoke(r, Conv::Thiscall, crate::abi::target(0x0050_DB90), &[r.eax]);
        r.ecx = source.wrapping_add(0x7C);
        let source_road = r.ecx;
        r.ecx = this.wrapping_add(0x7C);
        invoke(r, Conv::Thiscall, crate::abi::target(0x0050_DB90), &[source_road]);
        r.edx = source.wrapping_add(0x88);
        r.ecx = this.wrapping_add(0x88);
        invoke(r, Conv::Thiscall, crate::abi::target(0x0050_DB90), &[r.edx]);
        r.eax = source.wrapping_add(0x94);
        r.ecx = this.wrapping_add(0x94);
        invoke(r, Conv::Thiscall, crate::abi::target(0x0050_DB90), &[r.eax]);
        r.ecx = (r.ecx & !0xFF) | u32::from(*(source.wrapping_add(0xA0) as *const u8));
        *(this.wrapping_add(0xA0) as *mut u8) = r.ecx as u8;
        r.eax = this;
    }
}

pub const PORTS: &[Port] = &[fc4p_00705950::PORT, fc4p_00707240::PORT];

#[cfg(test)]
mod tests;
