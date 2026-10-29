// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the Free
// Software Foundation, either version 3 of the License, or (at your option)
// any later version. This program is distributed WITHOUT ANY WARRANTY; see
// the GNU General Public License for more details. You should have received a
// copy of the license along with this program (see LICENSE at the repository
// root); if not, see <https://www.gnu.org/licenses/>.

//! `cSTETerrain`, converted one function at a time. Its game functions and globals are declared in this file.

use crate::port::Port;
use crate::x87::F80;

layout! {
    /// Terrain fields, relative to the primary interface.
    pub struct Terrain {
        0x00 => vtable / set_vtable: u32,
        0x10 => references / set_references: u32,
        0x28 => cells_x / set_cells_x: u32,
        0x2C => cells_z / set_cells_z: u32,
        0x38 => verts_x / set_verts_x: u32,
        0x3C => verts_z / set_verts_z: u32,
        0x5C => vertex_count / set_vertex_count: u32,
        0x6C => vertices / set_vertices: *mut u8,
        0x7C => sea_level / set_sea_level: f32,
        0xC0 => accumulating / set_accumulating: u8,
        0xC4 => update_left / set_update_left: i32,
        0xC8 => update_top / set_update_top: i32,
        0xCC => update_right / set_update_right: i32,
        0xD0 => update_bottom / set_update_bottom: i32,
        0xE8 => view / set_view: *mut u8,
        0xEC => view_ready / set_view_ready: u8
    }
}

game_data! { static zero: f32 = 0x00A8_1054; }
game_data! { static plumbing_state: u8 = 0x00B4_BC5C; }
game_fn! {
    #[allow(dead_code)]
    fn set_triangle_indices(view: *mut u8, z: u32, x: u32) -> u32 = "thiscall" 0x0075_2AF0;
}

/// The entry saves the three volatile integer registers here. Bodies change only the registers the
/// original changes. Arguments remain in their incoming slots, including when an output aliases one.
#[repr(C)]
struct Registers {
    eax: u32,
    ecx: u32,
    edx: u32,
}

impl Registers {
    unsafe fn arg(&self, index: usize) -> u32 {
        // SAFETY: the entry's return address follows this buffer, then the original argument words.
        unsafe { (self as *const Self).cast::<u32>().add(4 + index).read_unaligned() }
    }
    unsafe fn terrain(&self) -> Terrain {
        // SAFETY: the entry receives the terrain's primary interface in ECX.
        unsafe { Terrain::from_ptr(self.ecx as *mut u8) }
    }
}

// These entries only marshal registers. All bounds, loops, decisions, writes and calls live in Rust.
macro_rules! integer_port {
    ($address:literal, $export:ident, $body:ident, $pop:literal, $name:literal, $doc:literal) => {
        port! {
            #[doc = $doc]
            #[port(address = $address, owner = "terrain", name = $name, naked)]
            pub unsafe extern "C" fn $export() {
                core::arch::naked_asm!(
                    "push edx", "push ecx", "push eax", "push esp", "call {body}",
                    "add esp, 4", "pop eax", "pop ecx", "pop edx", "ret {pop}",
                    body = sym $body, pop = const $pop,
                );
            }
        }
    };
}

macro_rules! altitude_port {
    ($address:literal, $export:ident, $body:ident, $pop:literal, $name:literal) => {
        port! {
            /// Reads an altitude onto ST0, retaining the integer results of the index calculation.
            #[port(address = $address, owner = "terrain", name = $name, naked)]
            pub unsafe extern "C" fn $export() {
                core::arch::naked_asm!(
                    "sub esp, 12", "push edx", "push ecx", "push eax", "push esp", "call {body}",
                    "add esp, 4", "pop eax", "pop ecx", "pop edx", "fld tbyte ptr [esp]",
                    "add esp, 12", "ret {pop}", body = sym $body, pop = const $pop,
                );
            }
        }
    };
}

/// A virtual call with the original input registers and all three integer output registers.
unsafe fn invoke(r: &mut Registers, target: usize, this: u32, args: &[u32]) {
    let mut c = crate::abi::RawCall::new(crate::abi::Conv::Thiscall, target, this, args);
    c.eax = r.eax;
    c.edx = r.edx;
    // SAFETY: each caller supplies a thiscall target with exactly these stack words. EDI holds the
    // descriptor across the call; the callee preserves it and removes its own arguments.
    unsafe {
        core::arch::asm!(
            "mov ecx, [edi + 20]", "mov eax, [edi + 16]", "2:",
            "test ecx, ecx", "jz 3f", "dec ecx", "push dword ptr [eax + ecx*4]", "jmp 2b", "3:",
            "mov eax, [edi + 4]", "mov ecx, [edi + 8]", "mov edx, [edi + 12]", "call dword ptr [edi]",
            "mov [edi + 4], eax", "mov [edi + 8], ecx", "mov [edi + 12], edx",
            in("edi") &mut c, clobber_abi("C"),
        );
    }
    r.eax = c.eax;
    r.ecx = c.ecx;
    r.edx = c.edx;
}

unsafe fn word(p: *const u8, offset: usize) -> u32 {
    unsafe { p.wrapping_add(offset).cast::<u32>().read_unaligned() }
}

unsafe fn put(p: *mut u8, offset: usize, value: u32) {
    unsafe { p.wrapping_add(offset).cast::<u32>().write_unaligned(value) }
}

fn vertex_offset(t: Terrain, x: u32, z: u32) -> usize {
    t.verts_x().wrapping_mul(z).wrapping_add(x).wrapping_mul(0x24) as usize
}

unsafe fn flags(t: Terrain, offset: usize) -> *mut u16 {
    t.vertices().wrapping_add(offset.wrapping_add(0x22)).cast()
}

integer_port!(
    0x0074_04E0,
    fc4p_007404e0,
    vertex_in_bounds,
    8,
    "VertexIsInBounds",
    "Tests both vertex coordinates using unsigned bounds."
);
unsafe extern "C" fn vertex_in_bounds(r: *mut Registers) {
    unsafe {
        let r = &mut *r;
        let t = r.terrain();
        r.eax = r.arg(0);
        if r.eax >= t.verts_x() {
            r.eax = 0;
            return;
        }
        r.edx = r.arg(1);
        r.eax = u32::from(r.edx < t.verts_z());
    }
}

port! {
    /// Returns the terrain class identifier without reading the interface.
    #[port(address = 0x0074_05D0, owner = "terrain", name = "cSTETerrain::GetGZCLSID", naked)]
    pub unsafe extern "C" fn fc4p_007405d0() {
        core::arch::naked_asm!("mov eax, 0xe98f9525", "ret");
    }
}

integer_port!(
    0x0074_0890,
    fc4p_00740890,
    clear_texture_layers,
    4,
    "ClearTextureLayers",
    "Clears the three texture IDs, retaining their weights, over an inclusive vertex rectangle."
);
unsafe extern "C" fn clear_texture_layers(r: *mut Registers) {
    unsafe {
        let r = &mut *r;
        let t = r.terrain();
        let rect = r.arg(0) as *mut u8;
        let mut z = word(rect, 4) as i32;
        if z > word(rect, 12) as i32 {
            return;
        }
        loop {
            r.edx = word(rect, 0);
            let mut offset = vertex_offset(t, r.edx, z as u32);
            r.eax = offset as u32 / 4;
            if r.edx as i32 <= word(rect, 8) as i32 {
                loop {
                    for field in [0x16, 0x1A, 0x1E] {
                        t.vertices()
                            .wrapping_add(offset.wrapping_add(field))
                            .cast::<u16>()
                            .write_unaligned(0);
                    }
                    r.edx = r.edx.wrapping_add(1);
                    offset = offset.wrapping_add(0x24);
                    if r.edx as i32 > word(rect, 8) as i32 {
                        break;
                    }
                }
            }
            r.eax = word(rect, 12);
            z = z.wrapping_add(1);
            if z > r.eax as i32 {
                break;
            }
        }
    }
}

integer_port!(
    0x0074_0900,
    fc4p_00740900,
    clear_cliff_flip,
    4,
    "cSTETerrain::ClearVertexCliffFlipTriangulationFlags",
    "Calls the internal flip slot for marked vertices, then clears their cliff-flip mark."
);
unsafe extern "C" fn clear_cliff_flip(r: *mut Registers) {
    unsafe {
        let r = &mut *r;
        let t = r.terrain();
        let rect = r.arg(0) as *mut u8;
        r.ecx = rect as u32;
        let mut z = word(rect, 4) as i32;
        if z > word(rect, 12) as i32 {
            return;
        }
        loop {
            let mut x = word(rect, 0) as i32;
            r.edx = word(rect, 8);
            let mut offset = vertex_offset(t, x as u32, z as u32);
            if x <= r.edx as i32 {
                loop {
                    r.eax = t.vertices() as u32;
                    if t.vertices().wrapping_add(offset.wrapping_add(0x23)).read() & 8 != 0 {
                        r.edx = t.vtable();
                        invoke(
                            r,
                            crate::abi::vslot(t.as_ptr(), 0x174),
                            t.as_ptr() as u32,
                            &[x as u32, z as u32, 0],
                        );
                        r.eax = t.vertices() as u32;
                        let p = t.vertices().wrapping_add(offset.wrapping_add(0x23));
                        p.write(p.read() & !8);
                        r.ecx = rect as u32;
                    }
                    r.eax = word(rect, 8);
                    x = x.wrapping_add(1);
                    offset = offset.wrapping_add(0x24);
                    if x > r.eax as i32 {
                        break;
                    }
                }
            }
            r.eax = word(rect, 12);
            z = z.wrapping_add(1);
            if z > r.eax as i32 {
                break;
            }
        }
    }
}

integer_port!(
    0x0074_09B0,
    fc4p_007409b0,
    clear_vertex_flags,
    8,
    "ClearVertexFlags",
    "Clears a 16-bit mask over the inclusive rectangle, retaining every other flag."
);
unsafe extern "C" fn clear_vertex_flags(r: *mut Registers) {
    unsafe {
        let r = &mut *r;
        let t = r.terrain();
        let rect = r.arg(0) as *mut u8;
        let mask = !(r.arg(1) as u16);
        let mut z = word(rect, 4) as i32;
        r.eax = word(rect, 12);
        if z > r.eax as i32 {
            return;
        }
        loop {
            r.edx = word(rect, 0);
            let mut offset = vertex_offset(t, r.edx, z as u32);
            if r.edx as i32 <= word(rect, 8) as i32 {
                loop {
                    let p = flags(t, offset);
                    p.write_unaligned(p.read_unaligned() & mask);
                    r.edx = r.edx.wrapping_add(1);
                    offset = offset.wrapping_add(0x24);
                    if r.edx as i32 > word(rect, 8) as i32 {
                        break;
                    }
                }
            }
            r.eax = word(rect, 12);
            z = z.wrapping_add(1);
            if z > r.eax as i32 {
                break;
            }
        }
    }
}

integer_port!(
    0x0074_1180,
    fc4p_00741180,
    flip_internal,
    12,
    "cSTETerrain::FlipInternalTriangulation",
    "Changes bit 0x1000 of a cell and refreshes its indices only when the bit changes and the view is ready."
);
integer_port!(
    0x0074_11F0,
    fc4p_007411f0,
    flip_external,
    12,
    "cSTETerrain::FlipExternalTriangulation",
    "Changes bit 0x2000 of a cell and refreshes its indices only when the bit changes and the view is ready."
);
unsafe extern "C" fn flip_internal(r: *mut Registers) {
    unsafe { flip(&mut *r, 0x1000) }
}
unsafe extern "C" fn flip_external(r: *mut Registers) {
    unsafe { flip(&mut *r, 0x2000) }
}
unsafe fn flip(r: &mut Registers, mask: u16) {
    unsafe {
        let t = r.terrain();
        let x = r.arg(0);
        r.eax = t.cells_x();
        if x >= r.eax {
            return;
        }
        let z = r.arg(1);
        r.eax = t.cells_z();
        if z >= r.eax {
            return;
        }
        let p = flags(t, vertex_offset(t, x, z));
        r.edx = p as u32;
        let old = p.read_unaligned();
        let new = if r.arg(2) as u8 == 0 { old & !mask } else { old | mask };
        r.eax = u32::from(new);
        if new == old {
            return;
        }
        p.write_unaligned(new);
        r.eax = (r.eax & !0xFF) | u32::from(t.view_ready());
        if t.view_ready() != 0 {
            invoke(r, crate::abi::target(0x0075_2AF0), t.view() as u32, &[z, x]);
        }
    }
}

altitude_port!(
    0x0074_13C0,
    fc4p_007413c0,
    altitude_index,
    4,
    "cSTETerrain::GetAltitude(index)"
);
altitude_port!(
    0x0074_13E0,
    fc4p_007413e0,
    altitude_vertex,
    8,
    "cSTETerrain::GetAltitudeAtVertex"
);
unsafe fn altitude_args(r: *mut Registers, i: usize) -> u32 {
    // SAFETY: float entries reserve twelve additional bytes between registers and the return address.
    unsafe { r.cast::<u32>().add(7 + i).read_unaligned() }
}
unsafe fn return_altitude(r: *mut Registers, p: *const f32) {
    unsafe {
        r.cast::<u8>()
            .add(12)
            .cast::<F80>()
            .write_unaligned(F80::from_f32(p.read_unaligned()))
    }
}
unsafe extern "C" fn altitude_index(r: *mut Registers) {
    unsafe {
        let t = (*r).terrain();
        (*r).eax = altitude_args(r, 0);
        let p = if (*r).eax < t.vertex_count() {
            (*r).ecx = t.vertices() as u32;
            (*r).eax = (*r).eax.wrapping_mul(9);
            t.vertices()
                .wrapping_add((*r).eax.wrapping_mul(4) as usize)
                .cast::<f32>()
        } else {
            zero()
        };
        return_altitude(r, p);
    }
}
unsafe extern "C" fn altitude_vertex(r: *mut Registers) {
    unsafe {
        let t = (*r).terrain();
        (*r).eax = t.verts_x();
        let x = altitude_args(r, 0);
        if x < (*r).eax {
            (*r).edx = altitude_args(r, 1);
            if (*r).edx < t.verts_z() {
                (*r).ecx = t.vertices() as u32;
                (*r).eax = t.verts_x().wrapping_mul((*r).edx).wrapping_add(x).wrapping_mul(9);
                return_altitude(r, t.vertices().wrapping_add((*r).eax.wrapping_mul(4) as usize).cast());
                return;
            }
        }
        return_altitude(r, zero());
    }
}

integer_port!(
    0x0074_15F0,
    fc4p_007415f0,
    cell_corners,
    12,
    "cSTETerrain::GetCellCornerAltitudes",
    "Copies corner bits in the order NW, SW, SE, NE; an invalid cell leaves the destination untouched."
);
unsafe extern "C" fn cell_corners(r: *mut Registers) {
    unsafe {
        let r = &mut *r;
        let t = r.terrain();
        let x = r.arg(0);
        r.eax = t.cells_x();
        if x >= r.eax {
            return;
        }
        r.edx = r.arg(1);
        if r.edx >= t.cells_z() {
            return;
        }
        let index = t.verts_x().wrapping_mul(r.edx).wrapping_add(x);
        let offset = index.wrapping_mul(0x24) as usize;
        let out = r.arg(2) as *mut u8;
        put(out, 0, word(t.vertices(), offset));
        let sw = index.wrapping_add(t.verts_x());
        put(out, 4, word(t.vertices(), sw.wrapping_mul(0x24) as usize));
        let se = index.wrapping_add(t.verts_x()).wrapping_add(1);
        r.eax = word(t.vertices(), se.wrapping_mul(0x24) as usize);
        put(out, 8, r.eax);
        r.ecx = t.vertices() as u32;
        r.edx = word(t.vertices(), offset.wrapping_add(0x24));
        put(out, 12, r.edx);
    }
}

integer_port!(
    0x0074_1660,
    fc4p_00741660,
    city_rectangle,
    4,
    "GetWholeCityVertexRectangle",
    "Writes the inclusive rectangle (0, 0, cellsX, cellsZ)."
);
unsafe extern "C" fn city_rectangle(r: *mut Registers) {
    unsafe {
        let r = &mut *r;
        let t = r.terrain();
        r.eax = r.arg(0);
        let out = r.eax as *mut u8;
        put(out, 4, 0);
        put(out, 0, 0);
        r.edx = t.cells_x();
        put(out, 8, r.edx);
        r.ecx = t.cells_z();
        put(out, 12, r.ecx);
    }
}

integer_port!(
    0x0074_1710,
    fc4p_00741710,
    neighbors,
    8,
    "cSTETerrain::GetNeighbors",
    "Returns the eight-neighbor mask in AL, retaining upper EAX and ECX and leaving z in EDX."
);
unsafe extern "C" fn neighbors(r: *mut Registers) {
    unsafe {
        let r = &mut *r;
        let t = r.terrain();
        let x = r.arg(0);
        let mut mask = if (x as i32) < 1 { 0x7C } else { 0xFF };
        if x >= t.cells_x() {
            mask &= 0xC7;
        }
        r.edx = r.arg(1);
        if (r.edx as i32) < 1 {
            mask &= 0xF1;
        }
        if r.edx >= t.cells_z() {
            mask &= 0x1F;
        }
        r.eax = (r.eax & !0xFF) | mask;
    }
}

port! {
    /// Loads the sea-level word directly onto the caller's x87 stack.
    #[port(address = 0x0074_1760, owner = "terrain", name = "cSTETerrain::GetSeaLevel", naked)]
    pub unsafe extern "C" fn fc4p_00741760() {
        core::arch::naked_asm!("fld dword ptr [ecx + 0x7c]", "ret");
    }
}

integer_port!(
    0x0074_1770,
    fc4p_00741770,
    cell_flags,
    8,
    "cSTETerrain::GetTerrainCellFlags",
    "Maps stored bits 0x20, 0x40, 0x200 and 0x2000 to public bits 1, 2, 4 and 8."
);
unsafe extern "C" fn cell_flags(r: *mut Registers) {
    unsafe {
        let r = &mut *r;
        let t = r.terrain();
        let x = r.arg(0);
        r.eax = 0;
        if x >= t.cells_x() {
            return;
        }
        r.edx = r.arg(1);
        if r.edx >= t.cells_z() {
            return;
        }
        let f = u32::from(flags(t, vertex_offset(t, x, r.edx)).read_unaligned());
        r.ecx = f;
        r.eax = ((f >> 5) & 3) | ((f >> 7) & 4) | ((f >> 10) & 8);
    }
}

integer_port!(
    0x0074_17D0,
    fc4p_007417d0,
    vertex_normal,
    12,
    "cSTETerrain::GetVertexNormal",
    "Copies the three normal words sequentially for a valid vertex, preserving their bits."
);
unsafe extern "C" fn vertex_normal(r: *mut Registers) {
    unsafe {
        let r = &mut *r;
        let t = r.terrain();
        let x = r.arg(0);
        r.eax = t.verts_x();
        if x >= r.eax {
            return;
        }
        r.edx = r.arg(1);
        if r.edx >= t.verts_z() {
            return;
        }
        let p = t.vertices().wrapping_add(vertex_offset(t, x, r.edx).wrapping_add(4));
        r.eax = p as u32;
        r.edx = word(p, 0);
        r.ecx = r.arg(2);
        put(r.ecx as *mut u8, 0, r.edx);
        r.edx = word(p, 4);
        put(r.ecx as *mut u8, 4, r.edx);
        r.eax = word(p, 8);
        put(r.ecx as *mut u8, 8, r.eax);
    }
}

integer_port!(
    0x0074_1810,
    fc4p_00741810,
    get_view,
    0,
    "cSTETerrain::GetView",
    "Returns the view interface at object base plus twelve, or null."
);
unsafe extern "C" fn get_view(r: *mut Registers) {
    unsafe {
        let r = &mut *r;
        r.eax = r.terrain().view() as u32;
        if r.eax != 0 {
            r.eax = r.eax.wrapping_add(12);
        }
    }
}

integer_port!(
    0x0074_1C20,
    fc4p_00741c20,
    triangulation_flipped,
    8,
    "cSTETerrain::IsTriangulationFlipped",
    "Tests either triangulation bit; an invalid cell clears only AL of cellsX."
);
unsafe extern "C" fn triangulation_flipped(r: *mut Registers) {
    unsafe {
        let r = &mut *r;
        let t = r.terrain();
        let x = r.arg(0);
        r.eax = t.cells_x();
        if x < r.eax {
            r.edx = r.arg(1);
            if r.edx < t.cells_z() {
                r.ecx = t.vertices() as u32;
                r.eax = u32::from(flags(t, vertex_offset(t, x, r.edx)).read_unaligned() & 0x3000 != 0);
                return;
            }
        }
        r.eax &= !0xFF;
    }
}

integer_port!(
    0x0074_1E30,
    fc4p_00741e30,
    pad_rectangle,
    8,
    "cSTETerrain::PadRectangle",
    "Grows the rectangle with wrapping arithmetic, then clamps its four edges using signed comparisons."
);
unsafe extern "C" fn pad_rectangle(r: *mut Registers) {
    unsafe {
        let r = &mut *r;
        let t = r.terrain();
        r.eax = r.arg(0);
        r.edx = r.arg(1);
        let p = r.eax as *mut u8;
        let right = word(p, 8).wrapping_add(r.edx) as i32;
        let left = word(p, 0).wrapping_sub(r.edx) as i32;
        let top = word(p, 4).wrapping_sub(r.edx) as i32;
        put(p, 0, left as u32);
        let bottom = word(p, 12).wrapping_add(r.edx) as i32;
        put(p, 4, top as u32);
        put(p, 8, right as u32);
        put(p, 12, bottom as u32);
        if top < 0 {
            put(p, 4, 0);
        }
        if left < 0 {
            put(p, 0, 0);
        }
        r.edx = t.cells_x();
        if right > r.edx as i32 {
            put(p, 8, r.edx);
        }
        r.ecx = t.cells_z();
        if bottom > r.ecx as i32 {
            put(p, 12, r.ecx);
        }
    }
}

integer_port!(
    0x0074_1E90,
    fc4p_00741e90,
    query_interface,
    8,
    "cSTETerrain::QueryInterface",
    "Writes supported interface pointers before AddRef; rejection retains the comparison arithmetic in upper EAX."
);
unsafe extern "C" fn query_interface(r: *mut Registers) {
    unsafe {
        let r = &mut *r;
        let t = r.terrain();
        let iid = r.arg(0);
        let offset = match iid {
            1 | 0xAB95_3253 | 0xC70B_D37E => 0,
            0x4522_94AA => 4,
            0xE4FD_A3D4 => 8,
            _ => {
                r.eax = if iid <= 0xAB95_3253 {
                    iid.wrapping_sub(0x4522_94AA)
                } else {
                    iid
                } & !0xFF;
                return;
            }
        };
        r.edx = r.arg(1);
        let out = r.edx as *mut u32;
        let interface = if t.as_ptr().is_null() {
            0
        } else {
            (t.as_ptr() as u32).wrapping_add(offset)
        };
        out.write_unaligned(interface);
        r.eax = t.vtable();
        invoke(r, crate::abi::vslot(t.as_ptr(), 4), t.as_ptr() as u32, &[]);
        r.eax = (r.eax & !0xFF) | 1;
    }
}

integer_port!(
    0x0074_1F70,
    fc4p_00741f70,
    release,
    0,
    "cSTETerrain::Release",
    "Decrements unsigned reference counts greater than one; otherwise calls the deleting destructor with flag one."
);
unsafe extern "C" fn release(r: *mut Registers) {
    unsafe {
        let r = &mut *r;
        let t = r.terrain();
        r.eax = t.references();
        if r.eax > 1 {
            r.eax = r.eax.wrapping_sub(1);
            t.set_references(r.eax);
        } else {
            r.eax = t.vtable();
            invoke(r, crate::abi::vslot(t.as_ptr(), 0x16C), t.as_ptr() as u32, &[1]);
            r.eax = 0;
        }
    }
}

integer_port!(
    0x0074_1FF0,
    fc4p_00741ff0,
    plumbing_display,
    4,
    "cSTETerrain::SetPlumbingInfoDisplayState",
    "Stores the argument's low byte in the shared plumbing display state, retaining upper EAX."
);
unsafe extern "C" fn plumbing_display(r: *mut Registers) {
    unsafe {
        let r = &mut *r;
        let value = r.arg(0) as u8;
        plumbing_state().write(value);
        r.eax = (r.eax & !0xFF) | u32::from(value);
    }
}

integer_port!(
    0x0074_2000,
    fc4p_00742000,
    set_cell_flags,
    12,
    "cSTETerrain::SetTerrainCellFlags",
    "Stores hide and occupancy bits, calls the external flip slot, refreshes the view if ready, and reports a change."
);
unsafe extern "C" fn set_cell_flags(r: *mut Registers) {
    unsafe {
        let r = &mut *r;
        let t = r.terrain();
        let x = r.arg(0);
        if x >= t.cells_x() {
            r.eax &= !0xFF;
            return;
        }
        let z = r.arg(1);
        if z >= t.cells_z() {
            r.eax &= !0xFF;
            return;
        }
        let value = r.arg(2);
        let offset = vertex_offset(t, x, z);
        let p = flags(t, offset);
        r.edx = p as u32;
        let old = p.read_unaligned();
        let new = (old & 0xFD9F) | ((value as u16 & 3) << 5) | ((value as u16 & 4) << 7);
        r.eax = u32::from(new);
        p.write_unaligned(new);
        r.edx = t.vtable();
        invoke(
            r,
            crate::abi::vslot(t.as_ptr(), 0x178),
            t.as_ptr() as u32,
            &[x, z, (value >> 3) & 0xFFFF_FF01],
        );
        r.eax = (r.eax & !0xFF) | u32::from(t.view_ready());
        if t.view_ready() != 0 {
            invoke(r, crate::abi::target(0x0075_2AF0), t.view() as u32, &[z, x]);
        }
        r.eax = t.vertices() as u32;
        r.edx = (r.edx & 0xFFFF_0000) | u32::from(old);
        r.ecx = u32::from(flags(t, offset).read_unaligned() != old);
        r.eax = (r.eax & !0xFF) | r.ecx;
    }
}

integer_port!(
    0x0074_20B0,
    fc4p_007420b0,
    start_texture_updates,
    0,
    "cSTETerrain::StartAccumulatingTextureUpdates",
    "Initializes an empty update rectangle once; subsequent calls leave it untouched."
);
unsafe extern "C" fn start_texture_updates(r: *mut Registers) {
    unsafe {
        let r = &mut *r;
        let t = r.terrain();
        r.eax = (r.eax & !0xFF) | u32::from(t.accumulating());
        if t.accumulating() != 0 {
            return;
        }
        t.set_update_top(i32::MAX);
        t.set_update_left(i32::MAX);
        r.eax = 0x8000_0000;
        t.set_accumulating(1);
        t.set_update_bottom(i32::MIN);
        t.set_update_right(i32::MIN);
    }
}

/// This module's replacements.
pub const PORTS: &[Port] = &[
    fc4p_007404e0::PORT,
    fc4p_007405d0::PORT,
    fc4p_00740890::PORT,
    fc4p_00740900::PORT,
    fc4p_007409b0::PORT,
    fc4p_00741180::PORT,
    fc4p_007411f0::PORT,
    fc4p_007413c0::PORT,
    fc4p_007413e0::PORT,
    fc4p_007415f0::PORT,
    fc4p_00741660::PORT,
    fc4p_00741710::PORT,
    fc4p_00741760::PORT,
    fc4p_00741770::PORT,
    fc4p_007417d0::PORT,
    fc4p_00741810::PORT,
    fc4p_00741c20::PORT,
    fc4p_00741e30::PORT,
    fc4p_00741e90::PORT,
    fc4p_00741f70::PORT,
    fc4p_00741ff0::PORT,
    fc4p_00742000::PORT,
    fc4p_007420b0::PORT,
];

#[cfg(test)]
#[path = "terrain/tests.rs"]
mod tests;
