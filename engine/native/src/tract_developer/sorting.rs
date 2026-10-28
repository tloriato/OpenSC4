// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Pointer sorting: median selection, partition and the final insertion pass.

use super::*;

game_fn! {
    #[allow(dead_code)]
    fn move_words(destination: *mut u8, source: *const u8, bytes: u32) -> u32 = "C" 0x009E_EB70;
}
game_fn! {
    #[allow(dead_code)]
    fn insert_unguarded(last: *mut u32, value: u32, compare: usize) -> u32 = "C" 0x0070_4810;
}
game_fn! {
    #[allow(dead_code)]
    fn insertion_sort(first: *mut u32, last: *mut u32, compare: usize) -> u32 = "C" 0x0070_62D0;
}
game_fn! {
    #[allow(dead_code)]
    fn insertion_tail(first: *mut u32, last: *mut u32, unused: u32, compare: usize) -> u32 = "C" 0x0070_4A90;
}

register_port! {
    /// Returns the address of the median of three referenced values. Reads each value again before
    /// every comparison, including when a comparator has changed the referenced storage.
    0x0070_4790, fc4p_00704790, median, 4, 0, "MedianCandidate", |r| {
        let [a, b, c, compare] = r.args;
        r.ecx = word(a, 0);
        r.eax = word(b, 0);
        invoke(r, Conv::Cdecl, compare as usize, &[r.ecx, r.eax]);
        if r.eax as u8 != 0 {
            r.edx = word(c, 0);
            r.eax = word(b, 0);
            invoke(r, Conv::Cdecl, compare as usize, &[r.eax, r.edx]);
            if r.eax as u8 != 0 { r.eax = b; return; }
            r.ecx = word(c, 0);
            r.edx = word(a, 0);
            invoke(r, Conv::Cdecl, compare as usize, &[r.edx, r.ecx]);
            r.eax = if r.eax as u8 != 0 { c } else { a };
        } else {
            r.eax = word(c, 0);
            r.ecx = word(a, 0);
            invoke(r, Conv::Cdecl, compare as usize, &[r.ecx, r.eax]);
            if r.eax as u8 != 0 { r.eax = a; return; }
            r.edx = word(c, 0);
            r.eax = word(b, 0);
            invoke(r, Conv::Cdecl, compare as usize, &[r.eax, r.edx]);
            r.eax = if r.eax as u8 != 0 { c } else { b };
        }
    }
}

register_port! {
    /// Insert a value by shifting predecessors until the comparator rejects a predecessor.
    /// A preceding sentinel is required; this helper has no beginning-of-range guard.
    0x0070_4810, fc4p_00704810, linear_insert, 3, 0, "UnguardedLinearInsertCandidate", |r| {
        let [mut last, value, compare] = r.args;
        let mut previous = last.wrapping_sub(4);
        r.eax = word(previous, 0);
        invoke(r, Conv::Cdecl, compare as usize, &[value, r.eax]);
        while r.eax as u8 != 0 {
            r.ecx = word(previous, 0);
            store(last, 0, r.ecx);
            r.edx = word(previous, u32::MAX - 3);
            last = previous;
            previous = previous.wrapping_sub(4);
            invoke(r, Conv::Cdecl, compare as usize, &[value, r.edx]);
        }
        store(last, 0, value);
    }
}

register_port! {
    /// Apply unguarded insertion to each value in the half-open range. The third argument is unused.
    0x0070_4A90, fc4p_00704a90, unguarded_sort, 4, 0, "UnguardedInsertionSortCandidates", |r| {
        let [mut first, last, _, compare] = r.args;
        r.eax = last;
        while first != r.eax {
            r.eax = word(first, u32::MAX - 3);
            let value = word(first, 0);
            let mut previous = first.wrapping_sub(4);
            let mut destination = first;
            invoke(r, Conv::Cdecl, compare as usize, &[value, r.eax]);
            while r.eax as u8 != 0 {
                r.ecx = word(previous, 0);
                store(destination, 0, r.ecx);
                r.edx = word(previous, u32::MAX - 3);
                destination = previous;
                previous = previous.wrapping_sub(4);
                invoke(r, Conv::Cdecl, compare as usize, &[value, r.edx]);
            }
            r.eax = last;
            first = first.wrapping_add(4);
            store(destination, 0, value);
        }
    }
}

register_port! {
    /// Partition around a copied pivot, scanning inward and exchanging values until the pointers meet.
    /// Like the original algorithm, the scans rely on a comparator and pivot that provide sentinels.
    0x0070_54E0, fc4p_007054e0, partition, 4, 0, "PartitionCandidates", |r| {
        let [mut first, mut last, pivot, compare] = r.args;
        loop {
            r.eax = word(first, 0);
            invoke(r, Conv::Cdecl, compare as usize, &[r.eax, pivot]);
            while r.eax as u8 != 0 {
                r.ecx = word(first, 4);
                first = first.wrapping_add(4);
                invoke(r, Conv::Cdecl, compare as usize, &[r.ecx, pivot]);
            }
            r.edx = word(last, u32::MAX - 3);
            last = last.wrapping_sub(4);
            invoke(r, Conv::Cdecl, compare as usize, &[pivot, r.edx]);
            while r.eax as u8 != 0 {
                r.eax = word(last, u32::MAX - 3);
                last = last.wrapping_sub(4);
                invoke(r, Conv::Cdecl, compare as usize, &[pivot, r.eax]);
            }
            if first >= last { r.eax = first; return; }
            r.eax = word(first, 0);
            r.ecx = word(last, 0);
            store(first, 0, r.ecx);
            store(last, 0, r.eax);
            first = first.wrapping_add(4);
        }
    }
}

register_port! {
    /// Guard the first element, move the prefix through the game's overlap-safe copy when a new
    /// minimum is found, and otherwise call the original unguarded insertion helper.
    0x0070_62D0, fc4p_007062d0, guarded_sort, 3, 0, "InsertionSortCandidates", |r| {
        let [first, last, compare] = r.args;
        r.eax = last;
        if first == last { return; }
        let mut current = first.wrapping_add(4);
        if current == last { return; }
        let mut bytes = current.wrapping_sub(first);
        loop {
            r.eax = word(first, 0);
            let value = word(current, 0);
            invoke(r, Conv::Cdecl, compare as usize, &[value, r.eax]);
            if r.eax as u8 != 0 {
                if bytes as i32 > 0 {
                    r.ecx = current.wrapping_sub(bytes).wrapping_add(4);
                    invoke(r, Conv::Cdecl, crate::abi::target(0x009E_EB70), &[r.ecx, first, bytes]);
                }
                store(first, 0, value);
            } else {
                r.edx = compare;
                invoke(r, Conv::Cdecl, crate::abi::target(0x0070_4810), &[current, value, r.edx]);
            }
            r.eax = last;
            current = current.wrapping_add(4);
            bytes = bytes.wrapping_add(4);
            if current == r.eax { return; }
        }
    }
}

register_port! {
    /// Finish sorting with guarded insertion on at most sixteen values and unguarded insertion on
    /// the tail. The byte span is rounded down to a multiple of four and compared as a signed word.
    0x0070_6EA0, fc4p_00706ea0, final_sort, 3, 0, "FinalInsertionSortCandidates", |r| {
        let [first, last, compare] = r.args;
        r.eax = first;
        r.ecx = last.wrapping_sub(first) & !3;
        if r.ecx as i32 > 0x40 {
            let split = first.wrapping_add(0x40);
            invoke(r, Conv::Cdecl, crate::abi::target(0x0070_62D0), &[first, split, compare]);
            invoke(r, Conv::Cdecl, crate::abi::target(0x0070_4A90), &[split, last, 0, compare]);
        } else {
            r.edx = compare;
            invoke(r, Conv::Cdecl, crate::abi::target(0x0070_62D0), &[first, last, r.edx]);
        }
    }
}

pub const PORTS: &[Port] = &[
    fc4p_00704790::PORT,
    fc4p_00704810::PORT,
    fc4p_00704a90::PORT,
    fc4p_007054e0::PORT,
    fc4p_007062d0::PORT,
    fc4p_00706ea0::PORT,
];

#[cfg(test)]
mod tests;
