// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Back-to-back residential and commercial lots, street spacing and temporary vertex locks.

use super::{Port, ZoneDeveloper};
use core::ptr::{null_mut, read_unaligned, write_unaligned};

layout! {
    /// Parcel dimensions, road conditions and the forced-middle-street option.
    struct ParcelSettings {
        0x84 => width / set_width: i32,
        0x88 => depth / set_depth: i32,
        0x8C => minimum / set_minimum: i32,
        0x90 => maximum / set_maximum: i32,
        0x94 => block / set_block: i32,
        0x98 => middle_street / set_middle_street: u8,
    }
}

game_fn! { fn allocate(size: u32) -> *mut u8 = "C" 0x0090_CF54; }
game_fn! { fn free(memory: *mut u8) = "C" 0x0090_CF63; }
game_fn! {
    fn remnants(this: *mut u8, x: i32, z: i32, width: i32, height: i32, facing: i32)
        = "thiscall" 0x0072_D440;
}
game_fn! {
    fn street(this: *mut u8, x1: i32, z1: i32, x2: i32, z2: i32, per_cell: u32)
        = "thiscall" 0x0072_A2A0;
}

/// Remaining depth uses unsigned thresholds and signed divisions, with wrapping intermediate sums.
fn depth_rule(rem: i32, front: bool, far_road: bool, minimum: i32, maximum: i32, depth: i32, ps: i32) -> i32 {
    let r = rem as u32;
    if far_road && front {
        if r <= maximum as u32 || r < minimum.wrapping_mul(2) as u32 {
            return rem;
        }
        if r < maximum.wrapping_mul(2).wrapping_sub(1) as u32 || r < minimum.wrapping_mul(3).wrapping_add(ps) as u32 {
            return rem.wrapping_add(1) / 2;
        }
        if r < depth.wrapping_mul(3).wrapping_add(ps) as u32 {
            return rem.wrapping_add(1) / 3;
        }
        return depth;
    }
    let threshold = minimum.wrapping_mul(2).wrapping_add(ps) as u32;
    if !far_road && !front {
        if r < threshold {
            return if rem > 1 { rem.wrapping_sub(1) } else { depth };
        }
        if r >= depth.wrapping_mul(2).wrapping_add(ps) as u32 {
            return depth;
        }
        if rem.wrapping_sub(1) as u32 > maximum as u32 {
            return rem / 2;
        }
        return if rem > 1 { rem.wrapping_sub(1) } else { depth };
    }
    if r < threshold {
        return rem;
    }
    if r >= depth.wrapping_mul(2).wrapping_add(ps) as u32 {
        return depth;
    }
    if r <= maximum as u32 {
        return rem;
    }
    rem / 2
}

unsafe fn clear_nodes(header: *mut u8) {
    unsafe {
        let mut node = read_unaligned(header.cast::<*mut u8>());
        while node != header {
            let next = read_unaligned(node.cast::<*mut u8>());
            free(node);
            node = next;
        }
        write_unaligned(header.cast::<*mut u8>(), header);
        write_unaligned(header.add(4).cast::<*mut u8>(), header);
    }
}

port! {
    /// Walk oriented rows and depths, adapting lot depths near the boundaries. Failed attempts use
    /// remnants; real attempts are levelled after their back street. Created lots remain locked
    /// through the whole walk, then are unlocked and released in creation order. All list storage
    /// comes from the game's allocator. Invalid row orientations return before allocating.
    #[port(address = 0x0072_EDD0, owner = "zonedeveloper", name = "cSC4ZoneDeveloper::Parcellize", naked)]
    pub unsafe extern "C" fn fc4p_0072edd0() {
        core::arch::naked_asm!("mov edx, [esp - 0x20]", "push dword ptr [esp - 0x10]",
            "call {body}", "ret", body = sym parcellize);
    }
}

/// The entry adapter keeps the upper bytes of the two scratch words used for street flags.
unsafe extern "fastcall" fn parcellize(this: *mut u8, cross_word: u32, back_word: u32) {
    // SAFETY: the game's developer owns the restriction, dimensions, interfaces and temporary lots.
    unsafe {
        let dev = ZoneDeveloper::from_ptr(this);
        let settings = ParcelSettings::from_ptr(this);
        let q = dev.orientation_q();
        if q > 3 {
            return;
        }
        let d = dev.orientation_d();
        let rect = dev.restriction().cast::<i32>();
        let bounds = [
            read_unaligned(rect),
            read_unaligned(rect.add(1)),
            read_unaligned(rect.add(2)),
            read_unaligned(rect.add(3)),
        ];
        let depth_axis = if q & 1 == 0 { 0 } else { 1 };
        let row_axis = 1 - depth_axis;
        let (a1, a2, b1, b2) = (
            bounds[depth_axis],
            bounds[depth_axis + 2],
            bounds[row_axis],
            bounds[row_axis + 2],
        );
        let lw = settings.width();
        let ld = settings.depth();
        let front = if depth_axis == 0 { 3 } else { 0 };
        let (front, back, start_p, low_p, high_p, step_p, offset_p) = if d == front {
            (front as i32, if depth_axis == 0 { 1 } else { 2 }, a1, a1, a2, ld, ld)
        } else {
            let start = a2.wrapping_sub(ld).wrapping_add(1).max(0);
            (
                if depth_axis == 0 { 1 } else { 2 },
                front as i32,
                start,
                a1.wrapping_sub(ld).wrapping_add(1),
                start,
                ld.wrapping_neg(),
                -1,
            )
        };
        let (start_o, low_o, high_o, step_o, offset_o) = if q == 0 || q == 3 {
            (b1, b1, b2, lw, lw)
        } else {
            let start = b2.wrapping_sub(lw).wrapping_add(1);
            (start, b1.wrapping_sub(lw).wrapping_add(1), start, lw.wrapping_neg(), -1)
        };
        let condition = |side: u32| {
            read_unaligned(
                this.wrapping_add(0xBC)
                    .wrapping_offset(side.wrapping_mul(4) as isize)
                    .cast::<i32>(),
            )
        };
        let first = if condition(d) == 1 { front } else { back };
        let far_road = condition(d.wrapping_sub(2) & 3) == 1;
        let length_row = b2.wrapping_sub(b1).wrapping_add(1);
        let length_depth = a2.wrapping_sub(a1).wrapping_add(1);
        let ps = i32::from(dev.place_streets() != 0);
        let base_width = if depth_axis == 0 { ld } else { lw };
        let base_height = if depth_axis == 0 { lw } else { ld };

        let mut middle = start_o;
        let mut cursor = start_o;
        let mut alternate = false;
        while low_o <= cursor && cursor <= high_o {
            cursor = cursor.wrapping_add(step_o);
            if alternate {
                middle = middle.wrapping_add(step_o);
            }
            alternate = !alternate;
        }
        if middle != start_o {
            middle = middle.wrapping_sub(step_o);
        }

        let header = allocate(12);
        write_unaligned(header.cast::<*mut u8>(), header);
        write_unaligned(header.add(4).cast::<*mut u8>(), header);
        let mut coords = [0i32; 2];
        coords[row_axis] = start_o;
        let mut accumulated = 0i32;
        let mut back_street = false;
        while low_o <= coords[row_axis] && coords[row_axis] <= high_o {
            let mut facing = first;
            coords[depth_axis] = start_p;
            while low_p <= coords[depth_axis] && coords[depth_axis] <= high_p {
                let original_p = coords[depth_axis];
                let mut lot = null_mut();
                let mut delta = 0i32;
                let current_depth = settings.depth();
                let remaining = if step_p > 0 {
                    a2.wrapping_sub(original_p).wrapping_add(1)
                } else {
                    original_p.wrapping_sub(a1).wrapping_add(current_depth)
                };
                let (mut width, mut height) = (base_width, base_height);
                if remaining > 0 {
                    let mut chosen = depth_rule(
                        remaining,
                        facing == front,
                        far_road,
                        settings.minimum(),
                        settings.maximum(),
                        current_depth,
                        ps,
                    );
                    if length_depth as u32 <= settings.maximum() as u32
                        && (length_row as u32) < settings.width().wrapping_mul(2).wrapping_add(ps) as u32
                    {
                        chosen = length_depth;
                    }
                    if chosen != current_depth {
                        match dev.orientation_q() {
                            0 | 2 => {
                                delta = chosen.wrapping_sub(width);
                                width = chosen;
                            }
                            1 | 3 => {
                                delta = chosen.wrapping_sub(height);
                                height = chosen;
                            }
                            _ => {}
                        }
                        if step_p < 0 {
                            coords[depth_axis] = original_p.wrapping_sub(delta);
                        }
                    }
                }
                let (x, z) = (coords[0], coords[1]);
                let mut attempted = [
                    x,
                    z,
                    x.wrapping_add(width).wrapping_sub(1),
                    z.wrapping_add(height).wrapping_sub(1),
                ];
                if vcall!(this.add(4), 0x58 => fn(i32, i32, i32, i32, i32, *mut *mut u8, u32, u32, u32) -> u32,
                        x, z, width, height, facing, &mut lot, 0, 0, 0) as u8
                    == 0
                {
                    remnants(this, x, z, width, height, facing);
                }
                if delta != 0 && step_p > 0 {
                    coords[depth_axis] = coords[depth_axis].wrapping_add(delta);
                }
                if facing == front {
                    facing = back;
                } else {
                    facing = front;
                    if dev.place_streets() != 0 {
                        let saved_p = coords[depth_axis];
                        let saved_o = coords[row_axis];
                        coords[depth_axis] = saved_p.wrapping_add(offset_p);
                        if a1 <= coords[depth_axis] && coords[depth_axis] <= a2 {
                            if saved_o == start_o {
                                coords[row_axis] = b1;
                                let from = coords;
                                coords[row_axis] = b2;
                                street(
                                    this,
                                    from[0],
                                    from[1],
                                    coords[0],
                                    coords[1],
                                    (back_word & !0xFF)
                                        | u32::from(coords[depth_axis] == a1 || coords[depth_axis] == a2),
                                );
                                back_street = true;
                            }
                            coords[depth_axis] = if step_p > 0 {
                                saved_p.wrapping_add(1)
                            } else {
                                saved_p.wrapping_sub(1)
                            };
                        } else {
                            coords[depth_axis] = saved_p;
                        }
                        coords[row_axis] = saved_o;
                    }
                }
                if dev.real_mode() != 0 {
                    attempted[0] = attempted[0].max(0);
                    attempted[1] = attempted[1].max(0);
                    if attempted[2] >= vcall!(dev.city(), 0x174 => fn() -> i32) {
                        attempted[2] = vcall!(dev.city(), 0x174 => fn() -> i32).wrapping_sub(1);
                    }
                    if attempted[3] >= vcall!(dev.city(), 0x178 => fn() -> i32) {
                        attempted[3] = vcall!(dev.city(), 0x178 => fn() -> i32).wrapping_sub(1);
                    }
                    vcall!(dev.lot_configuration(), 0x54 => fn(*mut i32, u32, u32, u32, u32, *mut u8),
                            attempted.as_mut_ptr(), 0, 1, 1, 0x7F7F_FFFF, null_mut());
                }
                if !lot.is_null() {
                    vcall!(lot, 0x114 => fn(u32), 1);
                    let node = allocate(12);
                    let payload = node.wrapping_add(8).cast::<*mut u8>();
                    if !payload.is_null() {
                        write_unaligned(payload, lot);
                    }
                    let previous = read_unaligned(header.add(4).cast::<*mut u8>());
                    write_unaligned(node.cast::<*mut u8>(), header);
                    write_unaligned(node.add(4).cast::<*mut u8>(), previous);
                    write_unaligned(previous.cast::<*mut u8>(), node);
                    write_unaligned(header.add(4).cast::<*mut u8>(), node);
                }
                coords[depth_axis] = coords[depth_axis].wrapping_add(step_p);
            }
            if dev.place_streets() != 0 {
                accumulated = accumulated.wrapping_add(settings.width());
                let width = settings.width();
                let force = settings.middle_street() != 0
                    && back_street
                    && length_row as u32 <= settings.block().wrapping_add(width).wrapping_add(1) as u32
                    && coords[row_axis] == middle;
                if accumulated as u32 >= settings.block() as u32 || force {
                    let saved = coords[row_axis];
                    coords[row_axis] = saved.wrapping_add(offset_o);
                    if force
                        || (b1.wrapping_add(width) as u32 <= coords[row_axis] as u32
                            && coords[row_axis] as u32 <= b2.wrapping_sub(width) as u32)
                    {
                        coords[depth_axis] = a1;
                        let from = coords;
                        coords[depth_axis] = a2;
                        accumulated = 0;
                        street(
                            this,
                            from[0],
                            from[1],
                            coords[0],
                            coords[1],
                            (cross_word & !0xFF) | u32::from(coords[row_axis] == b1 || coords[row_axis] == b2),
                        );
                        coords[row_axis] = if step_o > 0 {
                            saved.wrapping_add(1)
                        } else {
                            saved.wrapping_sub(1)
                        };
                    } else {
                        coords[row_axis] = saved;
                    }
                }
            }
            coords[row_axis] = coords[row_axis].wrapping_add(step_o);
        }
        let mut node = read_unaligned(header.cast::<*mut u8>());
        if node != header {
            while node != header {
                let lot = read_unaligned(node.add(8).cast::<*mut u8>());
                vcall!(lot, 0x114 => fn(u32), 0);
                vcall!(lot, 8 => fn());
                node = read_unaligned(node.cast::<*mut u8>());
            }
            clear_nodes(header);
        }
        clear_nodes(header);
        free(header);
    }
}
pub(super) const PORTS: &[Port] = &[fc4p_0072edd0::PORT];

#[cfg(test)]
mod tests;
