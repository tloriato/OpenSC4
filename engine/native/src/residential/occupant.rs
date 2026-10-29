// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Per-building ordinance lists and service coverage.

use crate::port::Port;
use crate::x87::F80;
use core::mem::MaybeUninit;

layout! {
    /// Links shared by the circular effect and curve lists.
    struct Link {
        0x00 => next / set_next: *mut u8,
        0x04 => previous / set_previous: *mut u8,
    }
}

layout! {
    /// Service-specific ordinance heads, capacity chunks, and covered-population counters.
    pub struct OccupantSim {
        0xAC => school_covered / set_school_covered: u32,
        0xB0 => hospital_covered / set_hospital_covered: u32,
        0x194 => hospital_effects / set_hospital_effects: *mut u8,
        0x198 => school_effects / set_school_effects: *mut u8,
        0x2FC => school_chunks / set_school_chunks: *mut u32,
        0x308 => hospital_chunks / set_hospital_chunks: *mut u32,
    }
}

layout! {
    /// The storage range owned by a temporary point vector.
    struct Points {
        0x00 => begin / set_begin: *mut f32,
        0x04 => end / set_end: *mut f32,
        0x08 => capacity / set_capacity: *mut f32,
    }
}

game_fn! { fn clear_effects(this:*mut *mut u8)->u32 = "thiscall" 0x006F_CB40; }
game_fn! { fn clear_curves(this:*mut *mut u8)->u32 = "thiscall" 0x006F_CBD0; }
game_fn! { fn affects_occupant(this:*mut u8,occupant:*mut u8,effect:*mut u8,kind:u32)->u32 = "thiscall" 0x006F_94F0; }
game_fn! { fn copy_effect(this:*mut u8,source:*mut u8)->u32 = "thiscall" 0x006F_B9F0; }
game_fn! { fn copy_curve(this:*mut u8,source:*mut u8)->u32 = "thiscall" 0x005F_4430; }
game_fn! { fn allocate(size:u32)->*mut u8 = "C" 0x0090_CF54; }
game_fn! { fn free(storage:*mut u8)->u64 = "C" 0x0090_CF63; }
game_fn! { fn efficiency(this:*mut u8,occupant:*mut u8,staff:*mut f32,vehicle:*mut f32,school:u32,ignore_strikes:u32)->u32 = "thiscall" 0x006F_8710; }
game_fn! { fn select_by_address(this:*mut u8,occupant:*mut u8,kind:u32,source:*mut *mut u8,out:*mut *mut u8)->u32 = "thiscall" 0x006F_D070; }
game_fn! {
    #[allow(clippy::too_many_arguments)]
    fn mark_coverage(this:*mut u8,grids:*mut *mut *mut u8,index:u32,occupant:*mut u8,radius_property:u32,multiplier:f32,capacity_property:u32,chunk:u32,served:*mut u32)->u32 = "thiscall" 0x006F_9BF0;
}
game_fn! { fn read_uint(holder:*mut u8,id:u32,out:*mut u32)->u64 = "C" 0x005F_D480; }
game_fn! { fn read_float(holder:*mut u8,id:u32,out:*mut f32)->u32 = "C" 0x005F_D4F0; }
game_fn! { fn curve_new(this:*mut u8)->u32 = "thiscall" 0x007D_0410; }
game_fn! { fn property_curves(this:*mut u8,holder:*mut u8,first:*mut u8,second:*mut u8,third:*mut u8,school:u32) = "thiscall" 0x006F_C400; }
game_fn! { fn modifiers(this:*mut u8,occupant:*mut u8,list:*mut *mut u8,radius:*mut f32,capacity:*mut u32,effect:*mut u32)->u32 = "thiscall" 0x006F_9340; }
game_fn! { fn curve_points(this:*mut u8,points:*mut u8)->u32 = "thiscall" 0x005F_4760; }

game_data! { static one:f32 = 0x00A8_1228; }
game_data! { static default_efficiency:u32 = 0x00B4_AF90; }
game_data! { static hundred:f32 = 0x00A8_0AA0; }
game_data! { static percent:f32 = 0x00A8_23A4; }

// Stored operands stay in memory, including signaling NaNs, until the arithmetic consumes them.
macro_rules! stored_operation {
    ($name:ident,$instruction:literal) => {
        fn $name(value:F80,operand:f32)->F80 {
            let mut out=F80::ZERO;
            unsafe {
                core::arch::asm!("fld tbyte ptr [{value}]",$instruction,"fstp tbyte ptr [{out}]",
                    value=in(reg)&value,operand=in(reg)&operand,out=in(reg)&mut out,out("st(0)") _);
            }
            out
        }
    };
}
stored_operation!(multiply, "fmul dword ptr [{operand}]");
stored_operation!(subtract, "fsub dword ptr [{operand}]");
stored_operation!(subtract_from, "fsubr dword ptr [{operand}]");

port! {
    /// Clear both destination lists, then copy each effect's two curves into paired, owning nodes.
    /// The occupant and kind arguments are unused. Source links are read after each pair is appended.
    #[port(address=0x006F_CD70,owner="residential",name="CopyResidentialEffectCurves",naked)]
    pub unsafe extern "C" fn fc4p_006fcd70() {
        core::arch::naked_asm!("mov edx,esp","jmp {body}",body=sym copy_effect_curves);
    }
}

/// Pass the original argument area to Rust: its third word becomes the source sentinel.
unsafe extern "fastcall" fn copy_effect_curves(
    _this: *mut u8,
    stack: *mut u32,
    _occupant: *mut u8,
    _kind: u32,
    source: *mut *mut u8,
    first: *mut *mut u8,
    second: *mut *mut u8,
) -> u32 {
    unsafe {
        let sentinel = *source;
        let mut node = Link::from_ptr(sentinel).next();
        stack.add(3).write(sentinel as u32);
        clear_curves(first);
        let mut eax = clear_curves(second);
        while node != sentinel {
            let head = *first;
            let storage = allocate(0x18);
            let curve = storage.wrapping_add(8);
            if !curve.is_null() {
                copy_curve(curve, node.wrapping_add(0x0C));
            }
            let tail = Link::from_ptr(head).previous();
            Link::from_ptr(storage).set_next(head);
            Link::from_ptr(storage).set_previous(tail);
            Link::from_ptr(tail).set_next(storage);
            Link::from_ptr(head).set_previous(storage);

            let head = *second;
            let storage = allocate(0x18);
            let curve = storage.wrapping_add(8);
            if !curve.is_null() {
                copy_curve(curve, node.wrapping_add(0x24));
            }
            let tail = Link::from_ptr(head).previous();
            Link::from_ptr(storage).set_previous(tail);
            Link::from_ptr(storage).set_next(head);
            Link::from_ptr(tail).set_next(storage);
            eax = sentinel as u32;
            Link::from_ptr(head).set_previous(storage);
            node = Link::from_ptr(node).next();
        }
        eax
    }
}

port! {
    /// Replace the output with copies of the source effects accepted by the occupant predicate.
    /// Preserve source order and capture the output sentinel before each allocation and copy.
    #[port(address=0x006F_D070,owner="residential",name="cSC4ResidentialSimulator::GetOrdinanceEffectListForOccupant",naked)]
    pub unsafe extern "C" fn fc4p_006fd070() {
        core::arch::naked_asm!("mov edx,esp","jmp {body}",body=sym select_effects);
    }
}

/// The source argument is also scratch space for its sentinel.
unsafe extern "fastcall" fn select_effects(
    this: *mut u8,
    stack: *mut u32,
    occupant: *mut u8,
    kind: u32,
    source: *mut *mut u8,
    out: *mut *mut u8,
) -> u32 {
    unsafe {
        clear_effects(out);
        let sentinel = *source;
        let mut node = Link::from_ptr(sentinel).next();
        stack.add(3).write(sentinel as u32);
        let mut eax = source as u32;
        while node != sentinel {
            let effect = node.wrapping_add(8);
            eax = affects_occupant(this, occupant, effect, kind);
            if eax as u8 != 0 {
                let head = *out;
                let storage = allocate(0x3C);
                let value = storage.wrapping_add(8);
                if !value.is_null() {
                    copy_effect(value, effect);
                }
                let tail = Link::from_ptr(head).previous();
                eax = tail as u32;
                Link::from_ptr(storage).set_next(head);
                Link::from_ptr(storage).set_previous(tail);
                Link::from_ptr(tail).set_next(storage);
                Link::from_ptr(head).set_previous(storage);
            }
            node = Link::from_ptr(node).next();
        }
        eax
    }
}

port! {
    /// Clear one coverage grid and visit its building list in order. Multiply the ordinance radius
    /// factors first, then the vehicle efficiency, and pass the stored result to the coverage marker.
    /// The third caller argument becomes the temporary effect-list head, which is cleared and freed.
    #[port(address=0x006F_D0F0,owner="residential",name="cSC4ResidentialSimulator::UpdateCoverageMap",naked)]
    pub unsafe extern "C" fn fc4p_006fd0f0() {
        core::arch::naked_asm!("mov edx,esp","jmp {body}",body=sym update_coverage);
    }
}

unsafe extern "fastcall" fn update_coverage(
    this: *mut u8,
    stack: *mut u32,
    grids: *mut *mut *mut u8,
    index: u32,
    buildings: *mut *mut u8,
    _unused: u32,
    school: u32,
) -> u32 {
    unsafe {
        let sim = OccupantSim::from_ptr(this);
        let grid = *(*grids).wrapping_add(index as usize);
        vcall!(grid,0x6C => fn(u32)->u32,0);
        let sentinel = *buildings;
        let mut node = Link::from_ptr(sentinel).next();
        let mut vehicle = 0.0f32;
        let (chunks, kind, radius_property, capacity_property, effects, served) = if school as u8 != 0 {
            (
                sim.school_chunks(),
                0xA92D_8E40,
                0x691B_42B3,
                0x8922_041B,
                this.add(0x198),
                this.add(0xAC),
            )
        } else {
            (
                sim.hospital_chunks(),
                0x292D_8F9B,
                0x091B_42A2,
                0x6922_0415,
                this.add(0x194),
                this.add(0xB0),
            )
        };
        let chunk = chunks.wrapping_add(index as usize).read_unaligned();
        let storage = allocate(0x3C);
        Link::from_ptr(storage).set_next(storage);
        Link::from_ptr(storage).set_previous(storage);
        stack.add(3).write(storage as u32);
        let list = stack.add(3).cast::<*mut u8>();
        let mut multiplier = MaybeUninit::<f32>::uninit();
        while node != sentinel {
            let occupant = node.add(8).cast::<*mut u8>().read_unaligned();
            efficiency(this, occupant, multiplier.as_mut_ptr(), &mut vehicle, school, 0);
            clear_effects(list);
            select_by_address(this, occupant, kind, effects.cast(), list);
            let mut factor = F80::from_f32(*one());
            let mut effect = Link::from_ptr(storage).next();
            while effect != storage {
                factor = multiply(factor, effect.add(8).cast::<f32>().read_unaligned());
                effect = Link::from_ptr(effect).next();
            }
            let value = (F80::from_f32(vehicle) * factor).to_f32();
            multiplier.write(value);
            mark_coverage(
                this,
                grids,
                index,
                occupant,
                radius_property,
                value,
                capacity_property,
                chunk,
                served.cast(),
            );
            node = Link::from_ptr(node).next();
        }
        clear_effects(list);
        free(storage) as u32
    }
}

port! {
    /// Query the radius and distance at the caller's level. Hospital capacity wins the radius and
    /// ordinance selection; a school capacity keeps the school funding and curve flag set.
    /// Empty point vectors leave distance untouched. The first point is overwritten while scanning.
    /// The interpolation's previous-x term has no percent scale. AL is success; EDX and upper EAX
    /// retain the last property-read or free result. The occupant argument becomes the effect head.
    /// Only the low byte of the local school flag is initialized; the entry preserves its other bytes.
    #[port(address=0x006F_D260,owner="residential",name="cSC4ResidentialSimulator::GetOccupantCoverage",naked)]
    pub unsafe extern "C" fn fc4p_006fd260() {
        core::arch::naked_asm!("mov edx,esp","push dword ptr [esp-0x64]","call {body}","ret 16",body=sym occupant_coverage);
    }
}

unsafe extern "fastcall" fn occupant_coverage(this: *mut u8, stack: *mut u32, flag_word: u32) -> u64 {
    unsafe {
        let occupant = stack.add(1).read() as *mut u8;
        let holder = vcall!(occupant,0x18 => fn()->*mut u8);
        let mut capacity = MaybeUninit::<u32>::uninit();
        let school = read_uint(holder, 0x8922_041B, capacity.as_mut_ptr()) as u8 != 0;
        let hospital = read_uint(holder, 0x6922_0415, capacity.as_mut_ptr());
        if hospital as u8 == 0 && !school {
            return hospital & !0xFF;
        }
        let school_word = (flag_word & !0xFF) | u32::from(school);
        let (kind, radius_property, effects) = if hospital as u8 != 0 {
            (0x292D_8F9B, 0x091B_42A2, this.add(0x194))
        } else {
            (0xA92D_8E40, 0x691B_42B3, this.add(0x198))
        };
        let mut base_radius = 0.0f32;
        let mut vehicle = f32::from_bits(*default_efficiency());
        let mut radius_modifier = 0.0f32;
        let (mut capacity_modifier, mut effect_modifier) = (0u32, 0u32);
        let storage = allocate(0x3C);
        Link::from_ptr(storage).set_next(storage);
        Link::from_ptr(storage).set_previous(storage);
        stack.add(1).write(storage as u32);
        let list = stack.add(1).cast::<*mut u8>();
        let mut first = MaybeUninit::<[u32; 4]>::uninit();
        let mut second = MaybeUninit::<[u32; 4]>::uninit();
        let mut third = MaybeUninit::<[u32; 4]>::uninit();
        let (first, second, third) = (
            first.as_mut_ptr().cast::<u8>(),
            second.as_mut_ptr().cast::<u8>(),
            third.as_mut_ptr().cast::<u8>(),
        );
        curve_new(first);
        curve_new(second);
        curve_new(third);
        read_float(holder, radius_property, &mut base_radius);
        let mut staff = f32::from_bits(radius_property);
        efficiency(this, occupant, &mut staff, &mut vehicle, school_word, 0);
        property_curves(this, holder, first, second, third, school_word);
        clear_effects(list);
        select_by_address(this, occupant, kind, effects.cast(), list);
        modifiers(
            this,
            occupant,
            list,
            &mut radius_modifier,
            &mut capacity_modifier,
            &mut effect_modifier,
        );
        let product = multiply(multiply(F80::from_f32(vehicle), radius_modifier), base_radius);
        let mut vector = [0u32; 3];
        let points = Points::from_ptr(vector.as_mut_ptr().cast());
        let radius = stack.add(4).read() as *mut f32;
        radius.write_unaligned(product.to_f32());
        curve_points(third, points.as_ptr());
        let begin = points.begin();
        let count = ((points.end() as u32).wrapping_sub(begin as u32) as i32 >> 3) as u32;
        if count != 0 {
            let distance = stack.add(3).read() as *mut f32;
            distance.cast::<u32>().write_unaligned(0);
            let mut index = 1u32;
            while index < count {
                let current = begin.wrapping_add(index.wrapping_mul(2) as usize);
                let level = stack.add(2).read() as *mut f32;
                let scaled = multiply(F80::from_f32(level.read_unaligned()), *hundred());
                if scaled.compare(F80::from_f32(current.add(1).read_unaligned())).less() {
                    begin
                        .cast::<u32>()
                        .write_unaligned(current.cast::<u32>().read_unaligned());
                    begin
                        .add(1)
                        .cast::<u32>()
                        .write_unaligned(current.add(1).cast::<u32>().read_unaligned());
                    index = index.wrapping_add(1);
                } else {
                    let level = stack.add(2).read() as *mut f32;
                    let scaled = multiply(F80::from_f32(level.read_unaligned()), *hundred());
                    let radius = stack.add(4).read() as *mut f32;
                    let distance = stack.add(3).read() as *mut f32;
                    let numerator = subtract_from(scaled, begin.add(1).read_unaligned());
                    let denominator = subtract(
                        F80::from_f32(begin.add(1).read_unaligned()),
                        current.add(1).read_unaligned(),
                    );
                    let fraction = numerator / denominator;
                    let delta = subtract(F80::from_f32(current.read_unaligned()), begin.read_unaligned());
                    let delta = multiply(delta, *percent());
                    let term = multiply(fraction * delta, radius.read_unaligned());
                    let previous = multiply(F80::from_f32(begin.read_unaligned()), radius.read_unaligned());
                    distance.write_unaligned((term + previous).to_f32());
                    break;
                }
            }
        }
        if !begin.is_null() {
            free(begin.cast());
        }
        for curve in [third, second, first] {
            let storage = curve.cast::<*mut u8>().read_unaligned();
            if !storage.is_null() {
                free(storage);
            }
        }
        clear_effects(list);
        (free(storage) & !0xFF) | 1
    }
}

pub const PORTS: &[Port] = &[
    fc4p_006fcd70::PORT,
    fc4p_006fd070::PORT,
    fc4p_006fd0f0::PORT,
    fc4p_006fd260::PORT,
];

#[cfg(test)]
mod tests;
