// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Build the two owning lists of active ordinance effects.

use crate::port::Port;
use core::mem::MaybeUninit;
use core::ptr::null_mut;

layout! {
    /// Circular links in the temporary kind lists and owning effect lists.
    struct Link {
        0x00 => next / set_next: *mut u8,
        0x04 => previous / set_previous: *mut u8,
    }
}
layout! {
    /// An ordinance's factors, two curves and kind list.
    struct Effect {
        0x00 => radius / set_radius: f32,
        0x14 => capacity / set_capacity: f32,
        0x18 => strength / set_strength: f32,
        0x2C => kinds / set_kinds: *mut u8,
        0x30 => decay / set_decay: f32,
    }
}
layout! {
    /// A point vector's three storage pointers.
    struct Points {
        0x00 => begin / set_begin: *mut u32,
        0x04 => end / set_end: *mut u32,
        0x08 => capacity / set_capacity: *mut u32,
    }
}
layout! {
    /// The first word is the curve's owned allocation.
    struct Curve {
        0x00 => storage / set_storage: *mut u8,
    }
}

game_fn! { fn array_allocate(size:u32)->*mut u32 = "C" 0x005E_55E0; }
game_fn! { fn allocate(size:u32)->*mut u8 = "C" 0x0090_CF54; }
game_fn! { fn free(storage:*mut u8)->u32 = "C" 0x0090_CF63; }
game_fn! { fn curve_new(this:*mut u8)->u32 = "thiscall" 0x007D_0410; }
game_fn! { fn curve_points(this:*mut u8,points:*mut u8)->u32 = "thiscall" 0x005F_4480; }
game_fn! { fn curve_property(this:*mut u8,property:*mut u8)->u32 = "thiscall" 0x005F_4620; }
game_fn! { fn read_float(holder:*mut u8,id:u32,out:*mut f32)->u32 = "C" 0x005F_D4F0; }
game_fn! { fn copy_effect(this:*mut u8,source:*mut u8)->u32 = "thiscall" 0x006F_B9F0; }
game_fn! { fn insert_points(this:*mut u8,position:*mut u32,point:*const u32,tag:*const u8,count:u32,stride:u32)->u32 = "thiscall" 0x007C_61A0; }

game_data! { static ordinance_manager:*mut u8 = 0x00B4_3D90; }
game_data! { static percent:f32 = 0x00A8_23A4; }

/// The stored percent is consumed directly by the multiply, without an intervening float load.
fn scaled(value: f32, percent: f32) -> f32 {
    let mut out = 0.0f32;
    unsafe {
        core::arch::asm!("fld dword ptr [{value}]", "fmul dword ptr [{percent}]", "fstp dword ptr [{out}]",
            value=in(reg)&value, percent=in(reg)&percent, out=in(reg)&mut out, out("st(0)") _);
    }
    out
}

unsafe fn clear_kinds(head: *mut u8) {
    unsafe {
        let mut node = Link::from_ptr(head).next();
        while node != head {
            let previous = node;
            node = Link::from_ptr(node).next();
            free(previous);
        }
        Link::from_ptr(head).set_next(head);
        Link::from_ptr(head).set_previous(head);
    }
}

unsafe fn construct_effect(effect: Effect) {
    unsafe {
        curve_new(effect.as_ptr().add(4));
        curve_new(effect.as_ptr().add(0x1C));
        effect.set_kinds(null_mut());
        let head = allocate(12);
        Link::from_ptr(head).set_next(head);
        Link::from_ptr(head).set_previous(head);
        effect.set_kinds(head);
    }
}

unsafe fn float_property(ordinance: *mut u8, id: u32, out: *mut f32) -> bool {
    unsafe {
        let holder = vcall!(ordinance,0x40 => fn()->*mut u8);
        read_float(holder, id, out) as u8 != 0
    }
}

unsafe fn curve_property_for(ordinance: *mut u8, id: u32, curve: *mut u8) -> bool {
    unsafe {
        let holder = vcall!(ordinance,0x40 => fn()->*mut u8);
        let property = vcall!(holder,0x24 => fn(u32)->*mut u8,id);
        if property.is_null() {
            false
        } else {
            curve_property(curve, property);
            true
        }
    }
}

/// Append at most one effect per service. All property reads occur, including after a match is found.
unsafe fn append_effect(
    ordinance: *mut u8,
    effect: Effect,
    points: Points,
    destination: *mut *mut u8,
    ids: [u32; 6],
    value: *mut f32,
    other_value: *mut f32,
) {
    unsafe {
        effect.set_radius(1.0);
        curve_points(effect.as_ptr().add(4), points.as_ptr());
        effect.set_capacity(1.0);
        effect.set_decay(1.0);
        effect.set_strength(1.0);
        curve_points(effect.as_ptr().add(0x1C), points.as_ptr());
        clear_kinds(effect.kinds());

        let mut present = float_property(ordinance, ids[0], value);
        if present {
            effect.set_radius(scaled(value.read(), *percent()));
        }
        present |= curve_property_for(ordinance, ids[1], effect.as_ptr().add(4));
        if float_property(ordinance, ids[2], other_value) {
            effect.set_capacity(scaled(other_value.read(), *percent()));
            present = true;
        }
        if float_property(ordinance, ids[3], other_value) {
            effect.set_decay(scaled(other_value.read(), *percent()));
            present = true;
        }
        if float_property(ordinance, ids[4], other_value) {
            effect.set_strength(scaled(other_value.read(), *percent()));
            present = true;
        }
        present |= curve_property_for(ordinance, ids[5], effect.as_ptr().add(0x1C));
        if present {
            let head = destination.read_unaligned();
            let storage = allocate(0x3C);
            let payload = storage.wrapping_add(8);
            if !payload.is_null() {
                copy_effect(payload, effect.as_ptr());
            }
            let tail = Link::from_ptr(head).previous();
            Link::from_ptr(storage).set_next(head);
            Link::from_ptr(storage).set_previous(tail);
            Link::from_ptr(tail).set_next(storage);
            Link::from_ptr(head).set_previous(storage);
        }
    }
}

unsafe fn destroy_effect(effect: Effect) -> u32 {
    unsafe {
        clear_kinds(effect.kinds());
        let head = effect.kinds();
        if !head.is_null() {
            free(head);
        }
        let second = Curve::from_ptr(effect.as_ptr().add(0x1C)).storage();
        if !second.is_null() {
            free(second);
        }
        let first = Curve::from_ptr(effect.as_ptr().add(4)).storage();
        if first.is_null() {
            0
        } else {
            free(first)
        }
    }
}

port! {
    /// Append school then health records for each active ordinance in the manager's order.
    /// Each scalar is a stored single multiplied by the game's percent constant on x87.
    /// Temporary kind lists stay empty. Existing destination nodes are retained.
    /// Arrays of at least 64 IDs use the game's array allocator; that allocation is not released here.
    #[port(address=0x006F_E0F0,owner="residential",name="cSC4ResidentialSimulator::GetOrdinanceEffects")]
    pub unsafe extern "thiscall" fn fc4p_006fe0f0(_this:*mut u8,health:*mut *mut u8,school:*mut *mut u8)->u32 {
        unsafe {
            let manager = *ordinance_manager();
            let mut count = vcall!(manager,0x38 => fn()->u32);
            let mut inline = [MaybeUninit::<u32>::uninit(); 64];
            let ids = if count < 64 {
                inline.as_mut_ptr().cast::<u32>()
            } else {
                array_allocate(count.wrapping_mul(4))
            };
            let mut health_storage = [MaybeUninit::<u32>::uninit(); 13];
            let health_effect = Effect::from_ptr(health_storage.as_mut_ptr().cast());
            construct_effect(health_effect);
            let mut school_storage = [MaybeUninit::<u32>::uninit(); 13];
            let school_effect = Effect::from_ptr(school_storage.as_mut_ptr().cast());
            construct_effect(school_effect);

            let mut point_storage = [0u32; 3];
            let points = Points::from_ptr(point_storage.as_mut_ptr().cast());
            let mut point = [0, 100f32.to_bits()];
            let tag = MaybeUninit::<u8>::uninit();
            insert_points(points.as_ptr(), null_mut(), point.as_ptr(), tag.as_ptr(), 1, 1);
            point = [100f32.to_bits(); 2];
            let end = points.end();
            if end == points.capacity() {
                insert_points(points.as_ptr(), end, point.as_ptr(), tag.as_ptr(), 1, 1);
            } else {
                if !end.is_null() {
                    end.write_unaligned(point[0]);
                    end.wrapping_add(1).write_unaligned(point[1]);
                }
                points.set_end(points.end().wrapping_add(2));
            }
            vcall!(manager,0x3C => fn(*mut u32,*mut u32)->u32,ids,&mut count);
            let mut value = MaybeUninit::<f32>::uninit();
            let mut other_value = MaybeUninit::<f32>::uninit();
            let mut index = 0u32;
            while index < count {
                let ordinance = vcall!(manager,0x50 => fn(u32)->*mut u8,ids.wrapping_add(index as usize).read_unaligned());
                if !ordinance.is_null() && !vcall!(ordinance,0x40 => fn()->*mut u8).is_null() {
                    append_effect(ordinance, school_effect, points, school,
                        [0xA91B_3AF4,0xA91B_3AFA,0xA92D_9D7A,0x692E_F65A,0x892D_9D02,0xC91B_3B02],
                        value.as_mut_ptr(),other_value.as_mut_ptr());
                    append_effect(ordinance, health_effect, points, health,
                        [0x491B_3AD5,0x891B_3AE6,0xE91B_3AEE,0xC92D_9C7A,0x092D_909B,0xE92D_9DB4],
                        value.as_mut_ptr(),other_value.as_mut_ptr());
                }
                index = index.wrapping_add(1);
            }
            let storage = points.begin();
            if !storage.is_null() {
                free(storage.cast());
            }
            destroy_effect(school_effect);
            destroy_effect(health_effect)
        }
    }
}

pub const PORTS: &[Port] = &[fc4p_006fe0f0::PORT];

#[cfg(test)]
mod tests;
