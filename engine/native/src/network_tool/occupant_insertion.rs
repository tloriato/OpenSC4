// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Occupant creation, optional prebuilt models, support structures and insertion into the city.

use super::*;
use crate::abi::{call_raw, Conv, RawCall};
use crate::x87::F80;

layout! {
    /// Additional insertion configuration, alongside the primary tool view.
    struct InsertionTool {
        0xFC => style / set_style: u32,
        0x364 => filter / set_filter: u32,
    }
}

game_fn! { fn support_info(this: *mut u8, piece: u32) -> *mut u8 = "thiscall" 0x0062_5860; }
game_fn! { fn support_cell(this: *mut u8, cell: *mut u8) -> *mut u8 = "thiscall" 0x0062_3BB0; }
game_fn! { fn add_supports(this: *mut u8, occupant: *mut u8, cell: *mut u8, support: *mut u8, info: *mut u8) = "thiscall" 0x0064_3FB0; }
game_fn! { #[allow(dead_code)] fn clear_cell(this: *mut u8, x: u32, z: u32, filter: u32) -> u64 = "thiscall" 0x0062_72F0; }
game_fn! { fn recompute_shear(this: *mut u8) = "thiscall" 0x0061_D230; }
game_fn! { fn reverse_edges(this: *mut u8, output: *mut u8) = "thiscall" 0x0062_1210; }
game_fn! { fn adjacent_x(this: *mut u8, x: u32, z: u32, mask: u32) -> u32 = "thiscall" 0x0062_4510; }
game_fn! { fn adjacent_z(this: *mut u8, x: u32, z: u32, mask: u32) -> u32 = "thiscall" 0x0062_4590; }
game_fn! { fn set_model_pose(this: *mut u8, model: u32, angle: u32, x: u32, y: u32, z: u32) = "thiscall" 0x0061_D340; }
game_fn! { fn add_model(this: *mut u8, exemplar: *mut u8, origin: *mut u8, orientation: u32) = "thiscall" 0x0061_D120; }
game_fn! { fn allocate(size: u32) -> u64 = "C" 0x0090_CF54; }
game_data! { static resources: *mut u8 = 0x00B4_3CA8; }
game_data! { static type_bits: u32 = 0x00AA_835C; }
game_data! { static type_groups: u8 = 0x00B4_52D4; }
game_data! { static model_types: u8 = 0x00B4_5318; }
game_data! { static default_offset: u32 = 0x00B4_52AC; }
game_data! { static half: f32 = 0x00A8_4D2C; }
game_data! { static angle_step: f32 = 0x00AA_8708; }
game_data! { static shear_scale: f32 = 0x00AA_870C; }
game_data! { static negative_four: f32 = 0x00AA_8710; }
game_data! { static positive_four: f32 = 0x00A8_E17C; }

// Memory arithmetic keeps the stored operand's role, including signaling NaN selection.
macro_rules! float_memory {
    ($name:ident, $op:literal) => {
        fn $name(value: F80, operand: f32) -> F80 {
            let mut output = F80::ZERO;
            unsafe {
                core::arch::asm!(
                    "fld tbyte ptr [{value}]", $op, "fstp tbyte ptr [{output}]",
                    value = in(reg) &value, operand = in(reg) &operand, output = in(reg) &mut output,
                    out("st(0)") _,
                );
            }
            output
        }
    };
}
float_memory!(add_float, "fadd dword ptr [{operand}]");
float_memory!(subtract_float, "fsub dword ptr [{operand}]");
float_memory!(multiply_float, "fmul dword ptr [{operand}]");

unsafe fn virtual_registers(this: *mut u8, slot: usize, args: &[u32], eax: u32, edx: u32) -> u64 {
    unsafe { call_registers(this, crate::abi::vslot(this, slot), args, eax, edx) }
}

unsafe fn call_registers(this: *mut u8, target: usize, args: &[u32], eax: u32, edx: u32) -> u64 {
    unsafe {
        let mut call = RawCall::new(Conv::Thiscall, target, this as u32, args);
        call.eax = eax;
        call.edx = edx;
        call_raw(&mut call);
        registers(call.eax, call.edx)
    }
}

port! {
    /// Create an occupant, configure any prebuilt model, add supports, clear the cell, and insert it.
    /// The optional output is a linked list retaining one reference. Return the final Release's EDX:EAX.
    /// Scratch storage keeps partially assigned orientation words and the original bounding-box outputs.
    #[port(address = 0x0062_78A0, owner = "networktool", name = "cSC4NetworkTool::InsertNetworkOccupant", naked)]
    pub unsafe extern "C" fn fc4p_006278a0() {
        core::arch::naked_asm!(
            "sub esp, 0xC4", "mov edx, esp", "call {body}", "add esp, 0xC4", "ret 12",
            body = sym insert_occupant,
        );
    }
}

unsafe fn average_height(cell: *mut u8, s: *mut u8) -> u32 {
    unsafe {
        let value = F80::from_f32(read(cell, 0x64));
        let value = multiply_float(add_float(value, read(cell, 0x5C)), *half());
        subtract_float(value, read(s, 0x68)).to_f32().to_bits()
    }
}

/// Return the occupied-edge count and the four-bit edge mask using the packed-byte arithmetic.
fn edge_summary(edges: u32) -> (u32, u32) {
    let bits = (edges & 0x0707_0707).wrapping_add(0x0707_0707) & 0x0808_0808;
    (
        bits.wrapping_mul(0x0101_0101) >> 27,
        bits.wrapping_mul(0x0102_0408) >> 27,
    )
}

fn diagonal_flag(edges: u32, count: u32, mask: u32) -> bool {
    match count {
        1 => matches!(edges & 0x0303_0303, 3 | 0x100 | 0x30000 | 0x01000000),
        2 => {
            if matches!(edges, 0x103 | 0x10300 | 0x01030000 | 0x03000001) {
                mask == 3 || mask == 12
            } else {
                mask == 9 || mask == 6
            }
        }
        4 => edges == 0x0103_0103,
        _ => false,
    }
}

/// Prepare the model's angle and horizontal offset. A false result omits the pose call.
unsafe fn horizontal_pose(
    this: *mut u8,
    cell: *mut u8,
    request: *mut u8,
    s: *mut u8,
    edges: u32,
    count: u32,
    mask: u32,
) -> bool {
    unsafe {
        let extended = edges & 0xF8F8_F8F8;
        let simple_two = !edges.wrapping_mul(2) & edges & 0x0202_0202;
        write(s, 0x17, u8::from(simple_two != 0));
        write(s, 0x54, edges);
        write(s, 0x2C, *default_offset());
        write(s, 0x34, *default_offset());
        if count == 1 && edges & 0x0101_0101 != 0 {
            let a = if edges & 0x00FF_FF00 != 0 {
                *positive_four()
            } else {
                *negative_four()
            };
            let b = if edges & 0xFFFF_0000 != 0 { 4.0f32 } else { -4.0f32 };
            let (x, z, held, other) = if edges & 0x0202_0202 != 0 {
                (a, b, F80::from_f32(b), (-F80::from_f32(a)).to_f32())
            } else {
                (b, (-F80::from_f32(a)).to_f32(), F80::from_f32(a), b)
            };
            write(s, 0x18, x);
            write(s, 0x1C, z);
            write(s, 0x28, other);
            write(s, 0x3C, 0x3F49_0FD0u32);
            let x = multiply_float(F80::from_f32(x), *shear_scale()) + held;
            let z = add_float(multiply_float(F80::from_f32(z), *shear_scale()), other);
            write(s, 0x2C, x.to_f32());
            write(s, 0x34, z.to_f32());
            return true;
        }
        if edges == 0x301 || edges == 0x30100 {
            write(s, 0x3C, 0xBF49_0FD0u32);
            write(s, 0x2C, if edges == 0x301 { -4.0f32 } else { 4.0f32 });
            write(s, 0x34, -4.0f32);
            return true;
        }
        if count == 2 && 1u32.wrapping_shl(mask) & 0x1248 != 0 && (extended != 0 || simple_two != 0) {
            let mut rotated = edges;
            let mut displacement = 0i32;
            if read::<i8>(request, 4) < 0 {
                write(s, 0x18, 0u32);
                reverse_edges(s.add(0x54), s.add(0x18));
                rotated = read(s, 0x18);
            }
            rotated = rotated.rotate_right(u32::from(read::<u8>(request, 4) & 3) * 8);
            while rotated as u8 == 0 || (rotated >> 8) as u8 == 0 {
                rotated = rotated.rotate_right(8);
                displacement = displacement.wrapping_sub(4);
            }
            let first = rotated & 3;
            let second = rotated >> 8 & 3;
            let mut a = displacement.wrapping_add((2u32.wrapping_sub(first)).wrapping_mul(2) as i32) & 15;
            let mut b = displacement.wrapping_sub(second.wrapping_mul(2).wrapping_add(8) as i32) & 15;
            if rotated & 0xF8 != 0 {
                a = if first == 2 {
                    if rotated & 0x10 != 0 {
                        a.wrapping_sub(1)
                    } else {
                        a.wrapping_add(1)
                    }
                } else {
                    a.wrapping_add(first as i32).wrapping_sub(2)
                };
            }
            if rotated & 0xF800 != 0 {
                b = if second == 2 {
                    if rotated & 0x1000 != 0 {
                        b.wrapping_sub(1)
                    } else {
                        b.wrapping_add(1)
                    }
                } else {
                    b.wrapping_add(second as i32).wrapping_sub(2)
                };
            }
            if a > b {
                core::mem::swap(&mut a, &mut b);
            }
            if b.wrapping_sub(a) > 8 {
                a = a.wrapping_add(16);
            }
            let sum = b.wrapping_add(a).wrapping_add(8);
            write(s, 0x18, sum);
            write(s, 0x3C, multiply_float(F80::from_i32(sum), *angle_step()).to_f32());
            let offsets = match mask {
                3 => Some((-4.0f32, -4.0f32)),
                6 => Some((4.0, -4.0)),
                9 => Some((-4.0, 4.0)),
                12 => Some((4.0, 4.0)),
                _ => None,
            };
            if let Some((x, z)) = offsets {
                write(s, 0x2C, x);
                write(s, 0x34, z);
            }
            return true;
        }
        if count < 3 && (count != 2 || extended == 0) && (count != 1 || simple_two == 0) {
            let accepted = match edges {
                0x0200_0200 => {
                    read::<u32>(cell, 4) & 1 != 0
                        || adjacent_z(this, read(cell, 0), read(cell, 4), read(cell, 8)) as u8 != 0
                }
                0x0002_0002 => {
                    read::<u32>(cell, 0) & 1 != 0
                        || adjacent_x(this, read(cell, 0), read(cell, 4), read(cell, 8)) as u8 != 0
                }
                _ => false,
            };
            if !accepted {
                return false;
            }
        }
        write(s, 0x3C, 0u32);
        write(s, 0x2C, 0u32);
        write(s, 0x34, 0u32);
        true
    }
}

/// Store both slope ratios in their original temporary words, including the second ratio's upper
/// orientation bytes. Keep the first ratio unless it is ordered and less than the second.
unsafe fn slopes(s: *mut u8, vector: *mut u8) -> f32 {
    unsafe {
        let status: u16;
        core::arch::asm!(
            "fld dword ptr [{v}]", "fld dword ptr [{v} + 8]",
            "fld dword ptr [{s} + 0x4C]", "fabs", "fld dword ptr [{s} + 0x44]", "fabs",
            "faddp st(1), st(0)", "fld dword ptr [{s} + 0x48]", "fabs",
            "fst dword ptr [{s} + 0x18]", "fdivp st(1), st(0)", "fstp dword ptr [{s} + 0x1C]",
            "fabs", "fxch st(1)", "fabs", "fxch st(1)", "fxch st(1)",
            "faddp st(1), st(0)", "fld dword ptr [{v} + 4]", "fabs",
            "fst dword ptr [{s} + 0x18]", "fdivp st(1), st(0)", "fstp dword ptr [{s} + 0x18]",
            "fld dword ptr [{s} + 0x1C]", "fcomp dword ptr [{s} + 0x18]", "fnstsw ax",
            s = in(reg) s, v = in(reg) vector, out("ax") status,
            out("st(0)") _, out("st(1)") _, out("st(2)") _, out("st(3)") _,
        );
        read(s, if status & 0x0500 == 0x0100 { 0x18 } else { 0x1C })
    }
}

unsafe fn configure_pose(this: *mut u8, cell: *mut u8, request: *mut u8, s: *mut u8) {
    unsafe {
        let occupant: *mut u8 = read(s, 0x10);
        let interface: *mut u8 = read(s, 0x20);
        let flags = vcall!(occupant, 0x30 => fn() -> u32) & 0x1FFF;
        if flags & flags.wrapping_sub(1) != 0 {
            return;
        }
        let mut kind = 0usize;
        while kind < 13 && type_bits().add(kind).read() & flags == 0 {
            kind += 1;
        }
        let edges: u32 = read(cell, 0x18 + kind * 4);
        let model_type = model_types().add(kind * 0x114);
        let rail = flags == 4 || flags == 0x40;
        let (count, mask) = edge_summary(edges);
        write(s, 0x28, mask);
        if diagonal_flag(edges, count, mask) {
            vcall!(occupant, 0x28 => fn(u32), 0x0100_0000);
        }
        recompute_shear(interface);
        let models: *mut u8 = read(model_type, 0xA0);
        let end: u32 = read(model_type, 0xA4);
        if models as u32 == end {
            return;
        }
        let len = (end.wrapping_sub(models as u32) as i32 >> 2) as u32;
        let style = InsertionTool::from_ptr(this).style();
        let selected = read::<u32>(models, if len > style { style.wrapping_mul(4) as usize } else { 0 });
        write(s, 0x40, selected);
        if rail {
            let (angle, x, z) = if matches!(
                edges,
                0x0103_0103 | 0x0301_0301 | 0x0300_0001 | 0x103 | 0x10300 | 0x0103_0000
            ) {
                (0x3F49_0FD0, 0u32, 0u32)
            } else if matches!(edges, 0x0204_0200 | 0x0004_0200 | 0x0204_0000) {
                if edges == 0x0204_0200
                    && read::<u32>(cell, 4) & 1 == 0
                    && adjacent_z(this, read(cell, 0), read(cell, 4), read(cell, 8)) as u8 == 0
                {
                    return;
                }
                (0, 8.0f32.to_bits(), 0)
            } else if matches!(edges, 0x0402_0002 | 0x0400_0002 | 0x0402_0000) {
                if edges == 0x0402_0002
                    && read::<u32>(cell, 0) & 1 == 0
                    && adjacent_x(this, read(cell, 0), read(cell, 4), read(cell, 8)) as u8 == 0
                {
                    return;
                }
                (0, 0, 8.0f32.to_bits())
            } else {
                return;
            };
            let y = average_height(cell, s);
            set_model_pose(interface, selected, angle, x, y, z);
            return;
        }
        if !horizontal_pose(this, cell, request, s, edges, count, mask) {
            return;
        }
        let diagonal = vcall!(interface, 0x20 => fn(u32) -> u32, 0x0100_0000) as u8 != 0;
        let (first, second, out) = if diagonal {
            (0x94, 0xAC, 0x7C)
        } else {
            (0xA0, 0xB8, 0x88)
        };
        let vector = vcall!(interface, 0xE4 => fn(*mut u8, u32, u32) -> *mut u8,
            s.add(out), read::<u32>(s, first), read::<u32>(s, first + 8));
        for offset in [0, 4, 8] {
            write(s, 0x44 + offset, read::<u32>(vector, offset));
        }
        let vector = vcall!(interface, 0xE4 => fn(*mut u8, u32, u32) -> *mut u8,
            s.add(0x88), read::<u32>(s, second), read::<u32>(s, second + 8));
        // Copy the y word before the ratio calculation, as it is also used as a stored operand.
        write(s, 0x80, read::<u32>(vector, 4));
        let slope = slopes(s, vector);
        let z = add_float(F80::from_f32(read(s, 0x34)), read(s, 0x6C)).to_f32();
        let x = add_float(F80::from_f32(read(s, 0x2C)), read(s, 0x64)).to_f32();
        let height = vcall_st0!(occupant, 0xE0, x, z);
        let height = subtract_float(height, read(s, 0x68));
        let correction = multiply_float(F80::from_f32(slope), *shear_scale());
        let y = (height - correction).to_f32().to_bits();
        set_model_pose(interface, selected, read(s, 0x3C), read(s, 0x2C), y, read(s, 0x34));
    }
}

unsafe extern "fastcall" fn insert_occupant(this: *mut u8, s: *mut u8) -> u64 {
    unsafe {
        let tool = NetworkTool::from_ptr(this);
        let cell: *mut u8 = read(s, 0xC8);
        let request: *mut u8 = read(s, 0xCC);
        let piece: u32 = read(request, 0);
        write(s, 0x38, this);
        write(s, 0x50, support_info(this, piece));
        let resource_manager = *resources();
        write(s, 0x58, 0x6534_284Au32);
        write(s, 0x5C, read::<u32>(tool.type_info(), 0xC));
        write(s, 0x60, piece);
        write(s, 0x27, 0u8);
        let mut prebuilt = false;
        for kind in 0..13 {
            if read::<u32>(cell, 8) & type_bits().add(kind).read() != 0 {
                write(s, 0x5C, read::<u32>(type_groups(), kind * 0x114));
                if vcall!(resource_manager, 0xC => fn(*mut u8, u32, *mut u8, u32, u32) -> u32,
                    s.add(0x58), 0xA521_60F5, s.add(0x18), 0, 0) as u8
                    != 0
                {
                    let resource: *mut u8 = read(s, 0x18);
                    vcall!(resource, 8 => fn());
                    prebuilt = true;
                    break;
                }
            }
        }
        write(s, 0x27, u8::from(prebuilt));
        vcall!(this, 0xD8 => fn(*mut u8, *mut u8, *mut u8, u32), cell, s.add(0x10), s.add(0x94),
            if prebuilt { 0x49C1_A034 } else { 0 });
        let occupant: *mut u8 = read(s, 0x10);
        vcall!(occupant, 0x78 => fn(u32, u32), read::<u32>(request, 0), u32::from(read::<u8>(request, 4)));
        if prebuilt {
            vcall!(occupant, 0x2C => fn(u32), 0x0200_0000);
            vcall!(occupant, 0x2C => fn(u32), 0x0400_0000);
            vcall!(occupant, 0x28 => fn(u32), 0x0800_0000);
            write(s, 0x74, 0u32);
            write(s, 0x78, 0u32);
            write(s, 0x20, 0u32);
            write(s, 0x70, 0u32);
            vcall!(occupant, 0 => fn(u32, *mut u8), 0x49C1_A034, s.add(0x20));
            let interface: *mut u8 = read(s, 0x20);
            vcall!(interface.add(4), 0x20 => fn(*mut u8), s.add(0x64));
            configure_pose(this, cell, request, s);
            // The final model uses the selected group, or the active type's group when it is zero.
            let orientation = read::<u8>(request, 4);
            write(s, 0x18, orientation);
            let mut group: u32 = read(s, 0x5C);
            let piece: u32 = read(request, 0);
            write(s, 0x1C, 0u32);
            if vcall!(occupant, 0 => fn(u32, *mut u8) -> u32, 0x49C1_A034, s.add(0x1C)) as u8 != 0 {
                if group == 0 {
                    group = read(tool.type_info(), 0xC);
                }
                write(s, 0x44, 0x6534_284Au32);
                write(s, 0x48, group);
                write(s, 0x4C, piece);
                if vcall!(*resources(), 0xC => fn(*mut u8, u32, *mut u8, u32, u32) -> u32,
                    s.add(0x44), 0xA521_60F5, s.add(0x54), 0, 0) as u8
                    != 0
                {
                    let resource: *mut u8 = read(s, 0x54);
                    if !resource.is_null() {
                        add_model(read(s, 0x1C), resource, s.add(0x70), read(s, 0x18));
                        vcall!(resource, 8 => fn());
                    }
                }
            }
            let interface: *mut u8 = read(s, 0x1C);
            if !interface.is_null() {
                vcall!(interface, 8 => fn());
            }
            let interface: *mut u8 = read(s, 0x20);
            if !interface.is_null() {
                vcall!(interface, 8 => fn());
            }
        }
        let support: *mut u8 = read(s, 0x50);
        if !support.is_null() {
            let info = support_cell(this, cell);
            if !info.is_null() {
                add_supports(this, read(s, 0x10), cell, support, info);
                vcall!(read::<*mut u8>(s, 0x10), 0x28 => fn(u32), 0x4000_0000);
                vcall!(read::<*mut u8>(s, 0x10), 0x28 => fn(u32), 0x0800_0000);
            }
        }
        if !prebuilt {
            let connections = cell
                .wrapping_add(0x18)
                .wrapping_add(tool.network_type().wrapping_mul(4) as usize);
            vcall!(this, 0x140 => fn(*mut u8, *mut u8), read::<*mut u8>(s, 0x10), connections);
        }
        vcall!(read::<*mut u8>(s, 0x10), 0x2C => fn(u32), 0x0040_0000);
        let x = read(cell, 0);
        let filter = InsertionTool::from_ptr(this).filter();
        let args = [x, read(cell, 4), filter];
        let mut cleared = RawCall::new(Conv::Thiscall, crate::abi::target(0x0062_72F0), this as u32, &args);
        cleared.eax = filter;
        cleared.edx = x;
        call_raw(&mut cleared);
        let mut edx = cleared.edx;
        let manager = tool.occupant_manager();
        if !manager.is_null() {
            let data = tool.insertion_data();
            let manager_table: u32 = read(manager, 0);
            write(s, 0x50, manager_table);
            let occupant: *mut u8 = read(s, 0x10);
            let object = virtual_registers(occupant, 0xC0, &[], manager_table, read(occupant, 0));
            let result = call_registers(
                manager,
                read(manager_table as *mut u8, 0x50),
                &[object as u32, data],
                manager_table,
                (object >> 32) as u32,
            );
            edx = (result >> 32) as u32;
            write(cell, 0x54, result as u8);
        }
        let occupant: *mut u8 = read(s, 0x10);
        let flags = virtual_registers(occupant, 0x30, &[], read(occupant, 0), edx);
        let result = if flags as u32 & 0x1000_0000 != 0 {
            virtual_registers(occupant, 0x2C, &[0x4000], flags as u32, read(occupant, 0))
        } else {
            virtual_registers(occupant, 0x28, &[0x4000], read(occupant, 0), (flags >> 32) as u32)
        };
        edx = (result >> 32) as u32;
        let output: *mut u8 = read(s, 0xD0);
        if !output.is_null() {
            vcall!(occupant, 4 => fn());
            let head: *mut u8 = read(output, 0);
            let allocated = allocate(12);
            let node = allocated as u32 as *mut u8;
            edx = (allocated >> 32) as u32;
            if !node.wrapping_add(8).is_null() {
                edx = read(s, 0x10);
                write(node, 8, edx);
            }
            let previous: *mut u8 = read(head, 4);
            write(node, 0, head);
            write(node, 4, previous);
            write(previous, 0, node);
            write(head, 4, node);
        }
        let occupant: *mut u8 = read(s, 0x10);
        let mut call = RawCall::new(Conv::Thiscall, crate::abi::vslot(occupant, 8), occupant as u32, &[]);
        call.eax = read(occupant, 0);
        call.edx = edx;
        call_raw(&mut call);
        registers(call.eax, call.edx)
    }
}

pub const PORTS: &[Port] = &[fc4p_006278a0::PORT];

#[cfg(test)]
mod tests;
