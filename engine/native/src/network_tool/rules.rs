// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Ordered occupant sets and cached rule matching.

use super::*;

layout! {
    /// Ordered tree node; the header has the same link offsets.
    struct Node {
        0x04 => parent / set_parent: *mut u8,
        0x08 => left / set_left: *mut u8,
        0x0C => right / set_right: *mut u8,
        0x10 => key / set_key: u32,
    }
}
game_fn! {
    #[allow(dead_code)]
    fn insert_cache(this: *mut u8, position: *mut u8, record: *mut u8, scratch: *mut u32, count: u32, stride: u32)
        = "thiscall" 0x0062_1A40;
}
game_fn! { fn cached_value(this: *mut u8, key: u32, value: *mut u32) -> u32 = "thiscall" 0x0062_1D30; }
game_fn! { fn match_rule(this: *mut u8, context: *mut u8, output: *mut *mut u8) -> u32 = "thiscall" 0x0062_2040; }

/// Tree successor, retaining the header's rightmost-node convention.
unsafe fn next(mut p: *mut u8) -> *mut u8 {
    unsafe {
        let node = Node::from_ptr(p);
        let mut link = node.right();
        if !link.is_null() {
            p = link;
            loop {
                link = Node::from_ptr(p).left();
                if link.is_null() {
                    return p;
                }
                p = link;
            }
        }
        link = node.parent();
        while p == Node::from_ptr(link).right() {
            p = link;
            link = Node::from_ptr(link).parent();
        }
        if Node::from_ptr(p).right() != link {
            p = link;
        }
        p
    }
}
port! {
    /// Compare two pointer-ordered occupant sets. Reset occupants present only in the first set to mode
    /// zero and occupants present only in the second to mode five, interleaving calls in pointer order.
    #[port(address = 0x0061_F5D0, owner = "networktool", name = "UpdateOccupantSetModes")]
    pub unsafe extern "C" fn fc4p_0061f5d0(first: *mut *mut u8, second: *mut *mut u8) {
        unsafe {
            let a = first.read_unaligned();
            let b = second.read_unaligned();
            let mut x = Node::from_ptr(a).left();
            let mut y = Node::from_ptr(b).left();
            while x != a && y != b {
                let xp = Node::from_ptr(x).key();
                let yp = Node::from_ptr(y).key();
                if xp < yp {
                    vcall!(xp as *mut u8, 0x44 => fn(u32, u32), 0, 0);
                    x = next(x);
                } else if yp < xp {
                    vcall!(yp as *mut u8, 0x44 => fn(u32, u32), 5, 0);
                    y = next(y);
                } else {
                    x = next(x);
                    y = next(y);
                }
            }
            while x != a {
                vcall!(Node::from_ptr(x).key() as *mut u8, 0x44 => fn(u32, u32), 0, 0);
                x = next(x);
            }
            while y != b {
                vcall!(Node::from_ptr(y).key() as *mut u8, 0x44 => fn(u32, u32), 5, 0);
                y = next(y);
            }
        }
    }
}
// The original record and caller output word are also the growth helper's inputs. Calls leave
// the Rust preparation frame before entering the game, retaining the caller's stack depths.
port! {
    /// Look up the key byte, query a missing value, and append its byte/value/mask record.
    #[port(address = 0x0062_1D30, owner = "networktool", name = "GetCachedRuleValue", naked)]
    pub unsafe extern "C" fn fc4p_00621d30() {
        core::arch::naked_asm!(
            "sub esp, 12", "push ebx", "push esi", "push edi",
            "mov esi, ecx", "mov edi, esp", "mov edx, edi", "call {find}",
            "test edx, edx", "jz 3f",
            "mov ebx, eax", "lea eax, [edi + 16]", "push eax",
            "mov edx, [edi + 28]", "push edx", "mov ecx, esi", "mov eax, ebx", "call dword ptr [eax + 4]",
            "mov ecx, esi", "mov edx, edi", "call {append}",
            "test edx, edx", "jz 2f",
            "mov ebx, eax", "call {target}", "push 1", "push 1", "lea edx, [edi + 32]", "push edx",
            "lea ecx, [edi + 12]", "push ecx", "push ebx", "lea ecx, [esi + 8]", "call eax",
            "2:", "mov ecx, edi", "call {finish}",
            "3:", "pop edi", "pop esi", "pop ebx", "add esp, 12", "ret 8",
            find = sym find_cached_value, append = sym append_cached_value,
            target = sym insert_cache_target, finish = sym finish_cached_value,
        );
    }
}
extern "C" fn insert_cache_target() -> usize {
    crate::abi::target(0x0062_1A40)
}
unsafe extern "fastcall" fn find_cached_value(this: *mut u8, frame: *mut u32) -> u64 {
    unsafe {
        let key = frame.add(7).read();
        let mut p: *mut u8 = read(this, 8);
        let end: *mut u8 = read(this, 12);
        while p != end {
            if read::<u8>(p, 0) == key as u8 {
                let value = read(p, 4);
                (frame.add(8).read() as *mut u32).write_unaligned(value);
                return registers(value, 0);
            }
            p = p.wrapping_add(12);
        }
        frame.add(3).cast::<u8>().write(key as u8);
        frame.add(4).write(0);
        frame.add(5).write(0);
        registers(read(this, 0), 1)
    }
}
unsafe extern "fastcall" fn append_cached_value(this: *mut u8, frame: *mut u32) -> u64 {
    unsafe {
        let position: *mut u8 = read(this, 12);
        if position == read::<*mut u8>(this, 16) {
            return registers(position as u32, 1);
        }
        if !position.is_null() {
            write(position, 0, frame.add(3).cast::<u8>().read());
            write(position, 4, frame.add(4).read());
            write(position, 8, frame.add(5).read());
        }
        let position: *mut u8 = read(this, 12);
        write(this, 12, position.wrapping_add(12));
        0
    }
}
unsafe extern "thiscall" fn finish_cached_value(frame: *mut u32) -> u64 {
    unsafe {
        let value = frame.add(4).read();
        let output = frame.add(8).read() as *mut u32;
        output.write_unaligned(value);
        registers(value, output as u32)
    }
}
port! {
    /// Test masked value constraints first, then context predicates, stopping on the first failure.
    /// Only success publishes the rule's result-vector address.
    #[port(address = 0x0062_2040, owner = "networktool", name = "MatchNetworkRule")]
    pub unsafe extern "thiscall" fn fc4p_00622040(this: *mut u8, context: *mut u8, output: *mut *mut u8) -> u8 {
        unsafe {
            let mut p: *mut u8 = read(this, 4);
            let end: *mut u8 = read(this, 8);
            let mut value = 0;
            while p != end {
                let key = u32::from(read::<u8>(p, 0));
                cached_value(context, key, &mut value);
                let expected: u32 = read(p, 4);
                let mask: u32 = read(p, 8);
                if (expected ^ value) & mask != 0 { return 0; }
                p = p.wrapping_add(12);
            }
            let mut p: *mut u8 = read(this, 0x10);
            let end: *mut u8 = read(this, 0x14);
            while p != end {
                let key = (context as u32 & !0xFF) | u32::from(read::<u8>(p, 4));
                if vcall!(context, 0 => fn(u32) -> u32, key) as u8 == 0 { return 0; }
                p = p.wrapping_add(8);
            }
            output.write_unaligned(this.add(0x10));
            1
        }
    }
}
port! {
    /// Search the unsigned equal-key range and test its rules in tree order. Clear the output before
    /// searching and stop at the first matching rule.
    #[port(address = 0x0062_20D0, owner = "networktool", name = "FindMatchingNetworkRule")]
    pub unsafe extern "thiscall" fn fc4p_006220d0(this: *mut u8, context: *mut u8, output: *mut *mut u8) -> u8 {
        unsafe {
            output.write_unaligned(core::ptr::null_mut());
            let mut low: *mut u8 = read(this, 4);
            let mut p = Node::from_ptr(low).parent();
            if !p.is_null() {
                let key: u32 = read(context, 4);
                while !p.is_null() {
                    let n = Node::from_ptr(p);
                    if n.key() >= key { low = p; p = n.left(); } else { p = n.right(); }
                }
            }
            let mut high: *mut u8 = read(this, 4);
            p = Node::from_ptr(high).parent();
            if !p.is_null() {
                let key: u32 = read(context, 4);
                while !p.is_null() {
                    let n = Node::from_ptr(p);
                    if key < n.key() { high = p; p = n.left(); } else { p = n.right(); }
                }
            }
            while low != high {
                if match_rule(low.add(0x14), context, output) as u8 != 0 { return 1; }
                low = next(low);
            }
            0
        }
    }
}

pub const PORTS: &[Port] = {
    let _ = fc4p_00621d30::PORT;
    &[
        fc4p_0061f5d0::PORT,
        fc4p_00622040::PORT,
        fc4p_006220d0::PORT,
    ]
};

#[cfg(test)]
mod tests;
