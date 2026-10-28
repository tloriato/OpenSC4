// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Candidate sites formed from existing lots, splits and aggregations.

use super::*;
use crate::x87::F80;

game_fn! { #[allow(dead_code)] fn stage_fits(stage: u32, zone: u32) -> u32 = "C" 0x0069_5340; }
game_fn! { #[allow(dead_code)] fn take_candidate() -> u32 = "C" 0x0070_7D90; }
game_fn! { #[allow(dead_code)] fn return_candidate(candidate: u32) -> u32 = "C" 0x0070_7320; }
game_fn! { #[allow(dead_code)] fn networks(this: u32, candidate: u32) -> u32 = "thiscall" 0x0070_5040; }
game_fn! { #[allow(dead_code)] fn legal(this: u32, candidate: u32) -> u32 = "thiscall" 0x0070_8700; }
game_fn! { #[allow(dead_code)] fn compatible(this: u32, configuration: u32, candidate: u32, roads: u32) -> u32 = "thiscall" 0x0070_6500; }
game_fn! { #[allow(dead_code)] fn append(this: u32, value: u32) -> u32 = "thiscall" 0x007E_BD30; }
game_fn! { #[allow(dead_code)] fn grow_vector(this: u32, end: u32, value: u32, allocator: u32, count: u32, fill: u32) -> u32 = "thiscall" 0x0051_CA60; }

mod eligibility;
pub mod legality;

register_port! {
    /// Scan lots and configurations in pointer order. Retain the first compatible configuration
    /// per lot, count each rejection, and set the attempt flag for nonoccupied lots outside zone failure.
    /// Stored NaN slopes fail either slope comparison.
    0x0070_8110, fc4p_00708110, existing, 0, 0, "cSC4TractDeveloper::ListCandidateLots_Existing", |r| {
        let this = r.ecx;
        r.eax = word(this, 0xCC); let mut lot_node = word(r.eax, 8); r.ecx = 0;
        let mut best = 0i32;
        if lot_node == r.eax { r.eax = r.ecx; return; }
        loop {
            let lot = word(lot_node, 0x10); let mut key = lot;
            let suitability = match eligibility::cached(r, this, lot) {
                Some(value) => value,
                None => eligibility::check(r, this, lot, &mut key as *mut u32 as u32, true),
            };
            r.edx = word(lot, 0); r.ecx = lot;
            invoke(r, Conv::Thiscall, word(r.edx, 0x84) as usize, &[]);
            if r.eax != 3 && suitability != 6 { *(this.wrapping_add(0x108) as *mut u8) = 1; }
            let mut location = [0u32; 2];
            r.eax = word(lot, 0); r.ecx = location.as_mut_ptr().add(1) as u32;
            let z = r.ecx; r.edx = location.as_mut_ptr() as u32; let x = r.edx; r.ecx = lot;
            invoke(r, Conv::Thiscall, word(r.eax, 0x54) as usize, &[x, z]);
            if suitability != 27 { best = best.max(suitability as i32); }
            else {
                let mut size = [0u8; 2];
                r.eax = word(lot, 0); r.ecx = size.as_mut_ptr().add(1) as u32;
                let depth = r.ecx; r.edx = size.as_mut_ptr() as u32; let width = r.edx; r.ecx = lot;
                invoke(r, Conv::Thiscall, word(r.eax, 0x58) as usize, &[width, depth]);
                r.eax = word(lot, 0); r.ecx = lot;
                let slope = float_call(r, word(r.eax, 0x48) as usize).to_f32();
                r.eax = word(this, 0xD8); r.ecx = word(r.eax, 8);
                let mut node = r.ecx;
                while node != word(this, 0xD8) {
                    let configuration = word(node, 0x10);
                    let mut dimensions = [0u32; 2];
                    r.edx = word(configuration, 0); r.eax = dimensions.as_mut_ptr().add(1) as u32;
                    let depth = r.eax; r.ecx = dimensions.as_mut_ptr() as u32; let width = r.ecx; r.ecx = configuration;
                    invoke(r, Conv::Thiscall, word(r.edx, 0x1C) as usize, &[width, depth]);
                    r.edx = u32::from(size[0]);
                    let mismatch = if dimensions[0] != r.edx { true } else { r.eax = u32::from(size[1]); dimensions[1] != r.eax };
                    if mismatch {
                        r.edx = word(this, 0x170); r.eax = word(r.edx, 0x24);
                        store(r.eax, 0x14, word(r.eax, 0x14).wrapping_add(1)); best = best.max(5);
                    } else {
                        r.edx = word(configuration, 0); r.ecx = configuration;
                        invoke(r, Conv::Thiscall, word(r.edx, 0x2C) as usize, &[]);
                        let stage = r.eax & 0xFF;
                        r.edx = location[1]; r.ecx = word(this, 0x3C); r.edx = location[0]; r.eax = word(r.ecx, 0);
                        invoke(r, Conv::Thiscall, word(r.eax, 0x30) as usize, &[r.edx, location[1]]);
                        invoke(r, Conv::Cdecl, crate::abi::target(0x0069_5340), &[stage, r.eax]);
                        if r.eax as u8 == 0 {
                            r.ecx = word(this, 0x170); r.eax = word(r.ecx, 0x24);
                            store(r.eax, 0x18, word(r.eax, 0x18).wrapping_add(1)); best = best.max(6);
                        } else {
                            r.eax = word(configuration, 0); r.ecx = configuration;
                            let minimum = float_call(r, word(r.eax, 0x24) as usize).to_f32();
                            r.edx = word(configuration, 0); r.ecx = configuration;
                            let maximum = float_call(r, word(r.edx, 0x20) as usize);
                            let low = first_slope_status(maximum, &minimum, &slope);
                            r.eax = (r.eax & 0xFFFF_0000) | u32::from(low);
                            let low_failure = low & 0x4100 == 0 || low & 0x4100 == 0x4100;
                            let high_failure = if low_failure { false } else {
                                let high = maximum_slope_status(maximum, &slope);
                                r.eax = (r.eax & 0xFFFF_0000) | u32::from(high);
                                high & 0x4100 == 0 || high & 0x4100 == 0x4100
                            };
                            if low_failure || high_failure {
                                r.eax = word(this, 0x170); r.eax = word(r.eax, 0x24);
                                store(r.eax, 0x28, word(r.eax, 0x28).wrapping_add(1)); best = best.max(10);
                            } else {
                                invoke(r, Conv::Cdecl, crate::abi::target(0x0070_7D90), &[]);
                                let candidate = r.eax; store(candidate, 0, 2); store(candidate, 4, lot);
                                r.ecx = candidate.wrapping_add(0xC); let z = r.ecx;
                                r.edx = candidate.wrapping_add(8); let x = r.edx; r.eax = word(lot, 0); r.ecx = lot;
                                invoke(r, Conv::Thiscall, word(r.eax, 0x54) as usize, &[x, z]);
                                r.eax = (r.eax & !0xFF) | u32::from(size[0]); *(candidate.wrapping_add(0x14) as *mut u8) = size[0];
                                r.ecx = (r.ecx & !0xFF) | u32::from(size[1]); *(candidate.wrapping_add(0x15) as *mut u8) = size[1];
                                r.edx = word(lot, 0); r.ecx = lot;
                                invoke(r, Conv::Thiscall, word(r.edx, 0x5C) as usize, &[]);
                                store(candidate, 0x10, r.eax); r.ecx = this;
                                invoke(r, Conv::Thiscall, crate::abi::target(0x0070_5040), &[candidate]);
                                r.ecx = this;
                                invoke(r, Conv::Thiscall, crate::abi::target(0x0070_6500), &[configuration, candidate, 0]);
                                best = best.max(r.eax as i32);
                                if r.eax == 27 {
                                    r.eax = word(this, 0xE8); r.edx = word(this, 0xEC); r.ecx = this.wrapping_add(0xE4);
                                    if r.eax != r.edx {
                                        if r.eax != 0 { store(r.eax, 0, candidate); }
                                        store(r.ecx, 4, word(r.ecx, 4).wrapping_add(4));
                                    } else {
                                        let mut value = candidate;
                                        r.edx = size.as_mut_ptr().add(1) as u32; let allocator = r.edx;
                                        r.edx = &mut value as *mut u32 as u32;
                                        invoke(r, Conv::Thiscall, crate::abi::target(0x0051_CA60), &[r.eax, r.edx, allocator, 1, 1]);
                                    }
                                    r.eax = word(this, 0x170); r.eax = word(r.eax, 0x24);
                                    store(r.eax, 0x6C, word(r.eax, 0x6C).wrapping_add(1)); break;
                                }
                                invoke(r, Conv::Cdecl, crate::abi::target(0x0070_7320), &[candidate]);
                            }
                        }
                    }
                    node = successor(r, node); r.ecx = r.eax;
                }
            }
            lot_node = successor(r, lot_node);
            if lot_node == word(this, 0xCC) { break; }
        }
        r.eax = best as u32;
    }
}

/// Unsigned division uses the CPU operation so a zero-width configuration raises the same fault.
fn divide_width(width: u32, divisor: u32) -> (u32, u32) {
    let quotient: u32;
    let remainder: u32;
    // SAFETY: the x86 DIV has no memory operand and only changes the stated registers and flags.
    unsafe {
        core::arch::asm!("xor edx, edx", "div ecx", inout("eax") width => quotient,
        in("ecx") divisor, out("edx") remainder, options(nostack));
    }
    (quotient, remainder)
}

register_port! {
    /// Split suitable lots along their width into equal pieces at the same depth. The piece count
    /// is stored as a byte. Compatibility failure returns the candidate without changing the result.
    /// Slope NaNs pass; successful candidates set result twenty-seven even after a larger prior code.
    0x0070_8C00, fc4p_00708c00, subdivision, 0, 0, "cSC4TractDeveloper::ListCandidateLots_Subdivision", |r| {
        let this = r.ecx; r.ecx = word(this, 0xCC); let mut lot_node = word(r.ecx, 8); r.eax = 0;
        let mut best = 0i32;
        if lot_node == r.ecx { return; }
        loop {
            let lot = word(lot_node, 0x10); let mut size = [0u8; 2];
            r.eax = word(lot, 0); r.ecx = size.as_mut_ptr().add(1) as u32;
            let depth = r.ecx; r.edx = size.as_mut_ptr() as u32; let width = r.edx; r.ecx = lot;
            invoke(r, Conv::Thiscall, word(r.eax, 0x58) as usize, &[width, depth]);
            let mut location = [0u32; 2]; r.eax = word(lot, 0); r.ecx = location.as_mut_ptr().add(1) as u32;
            let z = r.ecx; r.edx = location.as_mut_ptr() as u32; let x = r.edx; r.ecx = lot;
            invoke(r, Conv::Thiscall, word(r.eax, 0x54) as usize, &[x, z]);
            r.eax = word(lot, 0); r.ecx = lot;
            let slope = float_call(r, word(r.eax, 0x48) as usize).to_f32();
            let mut key = lot;
            let suitability = match eligibility::cached(r, this, lot) {
                Some(value) => value,
                None => eligibility::check(r, this, lot, &mut key as *mut u32 as u32, false),
            };
            if suitability != 27 { best = best.max(suitability as i32); }
            else {
                r.eax = word(this, 0xD8); r.ecx = word(r.eax, 8); let mut node = r.ecx;
                while node != word(this, 0xD8) {
                    let configuration = word(node, 0x10); let previous_best = best;
                    let mut dimensions = [0u32; 2]; r.edx = word(configuration, 0);
                    r.eax = dimensions.as_mut_ptr().add(1) as u32; let depth = r.eax;
                    r.ecx = dimensions.as_mut_ptr() as u32; let width = r.ecx; r.ecx = configuration;
                    invoke(r, Conv::Thiscall, word(r.edx, 0x1C) as usize, &[width, depth]);
                    r.eax = u32::from(size[0]); r.edx = dimensions[0];
                    let mut bad_size = r.eax < r.edx;
                    if !bad_size { r.ecx = u32::from(size[1]); bad_size = r.ecx < dimensions[1] || (r.eax == r.edx && r.ecx == dimensions[1]); }
                    let mut bad_split = false;
                    if !bad_size {
                        (r.eax, r.edx) = divide_width(r.eax, dimensions[0]);
                        bad_split = r.edx != 0 || r.ecx != dimensions[1];
                    }
                    if bad_size || bad_split {
                        if bad_size { r.eax = word(this, 0x170); r.eax = word(r.eax, 0x3C); }
                        else { r.edx = word(this, 0x170); r.eax = word(r.edx, 0x3C); }
                        r.ecx = word(r.eax, 0xC).wrapping_add(1); store(r.eax, 0xC, r.ecx);
                        if previous_best < 3 { best = 3; }
                    } else {
                        r.edx = word(configuration, 0); r.ecx = configuration;
                        invoke(r, Conv::Thiscall, word(r.edx, 0x2C) as usize, &[]);
                        let stage = r.eax & 0xFF; r.edx = location[1]; r.ecx = word(this, 0x3C);
                        r.edx = location[0]; r.eax = word(r.ecx, 0);
                        invoke(r, Conv::Thiscall, word(r.eax, 0x30) as usize, &[r.edx, location[1]]);
                        invoke(r, Conv::Cdecl, crate::abi::target(0x0069_5340), &[stage, r.eax]);
                        if r.eax as u8 == 0 {
                            r.eax = word(this, 0x170); r.eax = word(r.eax, 0x3C);
                            store(r.eax, 0x18, word(r.eax, 0x18).wrapping_add(1)); best = best.max(6);
                        } else {
                            r.edx = word(configuration, 0); r.ecx = configuration;
                            let minimum = float_call(r, word(r.edx, 0x24) as usize).to_f32();
                            r.eax = word(configuration, 0); r.ecx = configuration;
                            let maximum = float_call(r, word(r.eax, 0x20) as usize);
                            let low = first_slope_status(maximum, &slope, &minimum);
                            r.eax = (r.eax & 0xFFFF_0000) | u32::from(low);
                            let low_failure = low & 0x0500 == 0x0100;
                            let high_failure = if low_failure { false } else {
                                let high = maximum_slope_status(maximum, &slope);
                                r.eax = (r.eax & 0xFFFF_0000) | u32::from(high); high & 0x4100 == 0
                            };
                            if low_failure || high_failure {
                                r.ecx = word(this, 0x170); r.eax = word(r.ecx, 0x3C);
                                store(r.eax, 0x28, word(r.eax, 0x28).wrapping_add(1)); best = best.max(10);
                            } else {
                                (r.eax, r.edx) = divide_width(u32::from(size[0]), dimensions[0]); let pieces = r.eax;
                                invoke(r, Conv::Cdecl, crate::abi::target(0x0070_7D90), &[]);
                                let candidate = r.eax; store(candidate, 0, 4); store(candidate, 4, lot);
                                r.ecx = (r.ecx & !0xFF) | (dimensions[0] & 0xFF); *(candidate.wrapping_add(0x14) as *mut u8) = r.ecx as u8;
                                r.edx = (r.edx & !0xFF) | (dimensions[1] & 0xFF); *(candidate.wrapping_add(0x15) as *mut u8) = r.edx as u8;
                                r.eax = word(lot, 0); r.ecx = lot;
                                invoke(r, Conv::Thiscall, word(r.eax, 0x5C) as usize, &[]);
                                store(candidate, 0x10, r.eax); r.ecx = word(candidate, 4); r.edx = word(r.ecx, 0);
                                r.eax = candidate.wrapping_add(8);
                                invoke(r, Conv::Thiscall, word(r.edx, 0x54) as usize, &[r.eax, candidate.wrapping_add(0xC)]);
                                r.ecx = (r.ecx & !0xFF) | (pieces & 0xFF); *(candidate.wrapping_add(0x6C) as *mut u8) = r.ecx as u8;
                                r.ecx = this;
                                invoke(r, Conv::Thiscall, crate::abi::target(0x0070_5040), &[candidate]);
                                r.ecx = this;
                                invoke(r, Conv::Thiscall, crate::abi::target(0x0070_6500), &[configuration, candidate, 0]);
                                if r.eax == 27 {
                                    let mut value = candidate; r.edx = &mut value as *mut u32 as u32; r.ecx = this.wrapping_add(0xE4);
                                    invoke(r, Conv::Thiscall, crate::abi::target(0x007E_BD30), &[r.edx]);
                                    r.eax = word(this, 0x170); r.eax = word(r.eax, 0x3C);
                                    store(r.eax, 0x6C, word(r.eax, 0x6C).wrapping_add(1)); best = 27;
                                } else { invoke(r, Conv::Cdecl, crate::abi::target(0x0070_7320), &[candidate]); }
                            }
                        }
                    }
                    node = successor(r, node); r.ecx = r.eax;
                }
            }
            lot_node = successor(r, lot_node);
            if lot_node == word(this, 0xCC) { break; }
        }
        r.eax = best as u32;
    }
}

/// The inlined tree traversal retains EAX and ECX exactly, including the header special case.
unsafe fn successor<const N: usize>(r: &mut Entry<N>, node: u32) -> u32 {
    // SAFETY: node belongs to a live ordered tree with the game's header links.
    unsafe {
        r.eax = node;
        r.ecx = word(r.eax, 0xC);
        if r.ecx != 0 {
            r.eax = r.ecx;
            r.ecx = word(r.eax, 8);
            while r.ecx != 0 {
                r.eax = r.ecx;
                r.ecx = word(r.eax, 8);
            }
        } else {
            r.ecx = word(r.eax, 4);
            while r.eax == word(r.ecx, 0xC) {
                r.eax = r.ecx;
                r.ecx = word(r.ecx, 4);
            }
            if word(r.eax, 0xC) != r.ecx {
                r.eax = r.ecx;
            }
        }
        r.eax
    }
}

/// Call a virtual method returning ST0 and retain its integer register outputs too.
unsafe fn float_call<const N: usize>(r: &mut Entry<N>, target: usize) -> F80 {
    let mut result = F80::ZERO;
    // SAFETY: these virtual methods return one x87 value. No floating operation intervenes before
    // the exact ten-byte store, which removes only that result from the x87 stack.
    unsafe {
        invoke(r, Conv::Thiscall, target, &[]);
        core::arch::asm!("fstp tbyte ptr [{p}]", p = in(reg) &mut result, out("st(0)") _);
    }
    result
}

/// Compare a stored operand against another while the maximum slope occupies the next register.
/// Return the status word before removing that maximum; its TOP bits are visible in AX.
fn first_slope_status(maximum: F80, lhs: &f32, rhs: &f32) -> u16 {
    let status: u16;
    // SAFETY: both stored operands are live. Exactly two registers are loaded and popped.
    unsafe {
        core::arch::asm!(
            "fld tbyte ptr [{maximum}]", "fld dword ptr [{lhs}]", "fcomp dword ptr [{rhs}]",
            "fnstsw ax", "fstp st(0)", maximum = in(reg) &maximum,
            lhs = in(reg) lhs, rhs = in(reg) rhs, out("ax") status, out("st(0)") _, out("st(1)") _,
        );
    }
    status
}

fn maximum_slope_status(maximum: F80, slope: &f32) -> u16 {
    let status: u16;
    // SAFETY: the maximum retains ST0 precision, the slope is reloaded from its stored float.
    unsafe {
        core::arch::asm!(
            "fld tbyte ptr [{maximum}]", "fld dword ptr [{slope}]", "fcomp st(1)",
            "fnstsw ax", "fstp st(0)", maximum = in(reg) &maximum,
            slope = in(reg) slope, out("ax") status, out("st(0)") _, out("st(1)") _,
        );
    }
    status
}

register_port! {
    /// Try both anchors for every strictly larger configuration that contains the lot dimensions.
    /// Reject ordered out-of-range slopes, retaining NaNs. A failed trial writes result five only
    /// when the configuration width is below five, even after an earlier successful trial.
    0x0070_97E0, fc4p_007097e0, aggregation, 0, 0, "cSC4TractDeveloper::ListCandidateLots_Aggregation", |r| {
        let this = r.ecx;
        r.eax = word(this, 0xCC); r.ecx = word(r.eax, 8); r.edx = 0;
        let mut lot_node = r.ecx;
        let mut best = 0i32;
        if lot_node == r.eax { r.eax = r.edx; return; }
        loop {
            let lot = word(lot_node, 0x10);
            let mut size = [0u8; 2];
            r.eax = word(lot, 0); r.ecx = size.as_mut_ptr().add(1) as u32;
            let depth = r.ecx; r.edx = size.as_mut_ptr() as u32; let width = r.edx; r.ecx = lot;
            invoke(r, Conv::Thiscall, word(r.eax, 0x58) as usize, &[width, depth]);
            let mut location = [0u32; 2];
            r.eax = word(lot, 0); r.ecx = location.as_mut_ptr().add(1) as u32;
            let z = r.ecx; r.edx = location.as_mut_ptr() as u32; let x = r.edx; r.ecx = lot;
            invoke(r, Conv::Thiscall, word(r.eax, 0x54) as usize, &[x, z]);
            r.eax = word(lot, 0); r.ecx = lot;
            let slope = float_call(r, word(r.eax, 0x48) as usize).to_f32();
            r.eax = word(this, 0xD8); r.ecx = word(r.eax, 8);
            let mut configuration_node = r.ecx;
            while configuration_node != word(this, 0xD8) {
                r.ecx = configuration_node;
                let configuration = word(r.ecx, 0x10);
                let mut dimensions = [0u32; 2];
                r.edx = word(configuration, 0); r.eax = dimensions.as_mut_ptr().add(1) as u32;
                let depth = r.eax; r.ecx = dimensions.as_mut_ptr() as u32; let width = r.ecx; r.ecx = configuration;
                invoke(r, Conv::Thiscall, word(r.edx, 0x1C) as usize, &[width, depth]);
                r.eax = u32::from(size[0]); r.edx = dimensions[0];
                let mut reject_size = r.eax > r.edx;
                if !reject_size { r.ecx = u32::from(size[1]); reject_size = r.ecx > dimensions[1] || (r.eax == r.edx && r.ecx == dimensions[1]); }
                if reject_size {
                    r.eax = word(this, 0x170); r.eax = word(r.eax, 0x30);
                    store(r.eax, 0x14, word(r.eax, 0x14).wrapping_add(1)); best = best.max(5);
                } else {
                    r.edx = word(configuration, 0); r.ecx = configuration;
                    invoke(r, Conv::Thiscall, word(r.edx, 0x2C) as usize, &[]);
                    let stage = r.eax & 0xFF;
                    r.edx = location[1]; r.ecx = word(this, 0x3C); r.edx = location[0]; r.eax = word(r.ecx, 0);
                    invoke(r, Conv::Thiscall, word(r.eax, 0x30) as usize, &[r.edx, location[1]]);
                    invoke(r, Conv::Cdecl, crate::abi::target(0x0069_5340), &[stage, r.eax]);
                    if r.eax as u8 == 0 {
                        r.eax = word(this, 0x170); r.eax = word(r.eax, 0x30);
                        r.edx = word(r.eax, 0x18); r.ecx = best as u32; r.edx = r.edx.wrapping_add(1);
                        store(r.eax, 0x18, r.edx); r.eax = 5; best = best.max(5);
                    } else {
                        r.edx = word(configuration, 0); r.ecx = configuration;
                        let minimum = float_call(r, word(r.edx, 0x24) as usize).to_f32();
                        r.eax = word(configuration, 0); r.ecx = configuration;
                        let maximum = float_call(r, word(r.eax, 0x20) as usize);
                        let low = first_slope_status(maximum, &slope, &minimum);
                        r.eax = (r.eax & 0xFFFF_0000) | u32::from(low);
                        let low_failure = low & 0x0500 == 0x0100;
                        let high_failure = if low_failure { false } else {
                            let high = maximum_slope_status(maximum, &slope);
                            r.eax = (r.eax & 0xFFFF_0000) | u32::from(high);
                            high & 0x4100 == 0
                        };
                        if low_failure || high_failure {
                            r.edx = word(this, 0x170); r.eax = word(r.edx, 0x30);
                            store(r.eax, 0x28, word(r.eax, 0x28).wrapping_add(1)); best = best.max(5);
                        } else {
                            for trial in 0..2 {
                                invoke(r, Conv::Cdecl, crate::abi::target(0x0070_7D90), &[]);
                                let candidate = r.eax; store(candidate, 0, 3);
                                r.ecx = (r.ecx & !0xFF) | (dimensions[0] & 0xFF);
                                *(candidate.wrapping_add(0x14) as *mut u8) = r.ecx as u8;
                                r.edx = (r.edx & !0xFF) | (dimensions[1] & 0xFF);
                                *(candidate.wrapping_add(0x15) as *mut u8) = r.edx as u8;
                                r.eax = word(lot, 0); r.ecx = lot;
                                invoke(r, Conv::Thiscall, word(r.eax, 0x5C) as usize, &[]);
                                store(candidate, 0x10, r.eax);
                                if trial == 0 {
                                    r.edx = word(lot, 0); r.eax = candidate.wrapping_add(0xC);
                                    let z = r.eax; r.ecx = candidate.wrapping_add(8); let x = r.ecx; r.ecx = lot;
                                    invoke(r, Conv::Thiscall, word(r.edx, 0x54) as usize, &[x, z]);
                                } else {
                                    r.ecx = lot; r.edx = word(r.ecx, 0);
                                    invoke(r, Conv::Thiscall, word(r.edx, 0x54) as usize, &[candidate.wrapping_add(8), candidate.wrapping_add(0xC)]);
                                    r.ecx = u32::from(size[0]); r.eax = dimensions[0].wrapping_sub(r.ecx);
                                    if r.eax as i32 > 0 {
                                        r.ecx = word(candidate, 0x10);
                                        match r.ecx {
                                            0 => store(candidate, 8, word(candidate, 8).wrapping_add(r.eax)),
                                            1 => { r.ecx = word(candidate, 0xC).wrapping_add(r.eax); store(candidate, 0xC, r.ecx); }
                                            2 => store(candidate, 8, word(candidate, 8).wrapping_sub(r.eax)),
                                            3 => { r.ecx = word(candidate, 0xC).wrapping_sub(r.eax); store(candidate, 0xC, r.ecx); }
                                            _ => {}
                                        }
                                    }
                                    store(candidate, 0x3C, u32::MAX);
                                }
                                r.ecx = this;
                                invoke(r, Conv::Thiscall, crate::abi::target(0x0070_5040), &[candidate]);
                                r.ecx = this;
                                invoke(r, Conv::Thiscall, crate::abi::target(0x0070_8700), &[candidate]);
                                let mut success = false;
                                if r.eax as u8 != 0 {
                                    if trial == 1 { r.edx = configuration; }
                                    r.ecx = this;
                                    invoke(r, Conv::Thiscall, crate::abi::target(0x0070_6500), &[configuration, candidate, 0]);
                                    success = r.eax == 27;
                                }
                                if success {
                                    let mut value = candidate;
                                    if trial == 0 { r.edx = &mut value as *mut u32 as u32; }
                                    else { r.eax = &mut value as *mut u32 as u32; }
                                    let p = &mut value as *mut u32 as u32; r.ecx = this.wrapping_add(0xE4);
                                    invoke(r, Conv::Thiscall, crate::abi::target(0x007E_BD30), &[p]);
                                    if trial == 0 { r.eax = word(this, 0x170); r.eax = word(r.eax, 0x30); }
                                    else { r.ecx = word(this, 0x170); r.eax = word(r.ecx, 0x30); }
                                    store(r.eax, 0x6C, word(r.eax, 0x6C).wrapping_add(1)); best = 27;
                                } else {
                                    invoke(r, Conv::Cdecl, crate::abi::target(0x0070_7320), &[candidate]);
                                    if trial == 0 { r.ecx = dimensions[0]; r.eax = 5; }
                                    else { r.eax = dimensions[0]; }
                                    if (dimensions[0] as i32) < 5 { best = 5; }
                                }
                            }
                        }
                    }
                }
                configuration_node = successor(r, configuration_node);
            }
            lot_node = successor(r, lot_node);
            if lot_node == word(this, 0xCC) { break; }
        }
        r.eax = best as u32;
    }
}

pub const PORTS: &[Port] = &[
    fc4p_00708110::PORT,
    legality::fc4p_00708700::PORT,
    fc4p_00708c00::PORT,
    fc4p_007097e0::PORT,
];

#[cfg(test)]
mod tests;
