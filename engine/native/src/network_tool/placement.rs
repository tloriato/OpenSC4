// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Occupant construction and placement of one network piece by instance id.

use super::occupants::virtual_call;
use super::*;
use crate::x87::F80;

game_data! { static unsigned_correction: f32 = 0x00A8_0AA8; }
game_data! { static network_flags: [u32; 13] = 0x00AA_835C; }
game_fn! { #[allow(dead_code)] fn allocate_plain(size: u32) -> *mut u8 = "C" 0x0061_9E90; }
game_fn! { #[allow(dead_code)] fn allocate_model(size: u32) -> *mut u8 = "C" 0x0061_D200; }
game_fn! { #[allow(dead_code)] fn allocate_extended(size: u32) -> *mut u8 = "C" 0x0060_5040; }
game_fn! { #[allow(dead_code)] fn allocate_tunnel(size: u32) -> *mut u8 = "C" 0x0064_7230; }
game_fn! { #[allow(dead_code)] fn construct_plain(this: *mut u8, initialize: u32) -> *mut u8 = "thiscall" 0x0061_BDD0; }
game_fn! { #[allow(dead_code)] fn construct_model(this: *mut u8, initialize: u32) -> *mut u8 = "thiscall" 0x0061_E1A0; }
game_fn! { #[allow(dead_code)] fn construct_extended(this: *mut u8, initialize: u32) -> *mut u8 = "thiscall" 0x0060_5160; }
game_fn! { #[allow(dead_code)] fn construct_tunnel(this: *mut u8, initialize: u32) -> *mut u8 = "thiscall" 0x0064_76F0; }
game_fn! { #[allow(dead_code)] fn reset_model(this: *mut u8) = "thiscall" 0x0061_D230; }
game_fn! { fn construct_cell(this: *mut u8) = "thiscall" 0x0064_7850; }
game_fn! { fn allocate(size: u32) -> *mut u8 = "C" 0x0090_CF54; }
game_fn! { fn free(this: *mut u8) -> u64 = "C" 0x0090_CF63; }

macro_rules! stored_op {
    ($name:ident, $instruction:literal) => {
        fn $name(value: F80, operand: f32) -> F80 {
            let mut out = F80::ZERO;
            unsafe { core::arch::asm!("fld tbyte ptr [{value}]", $instruction, "fstp tbyte ptr [{out}]",
                value = in(reg) &value, operand = in(reg) &operand, out = in(reg) &mut out, out("st(0)") _); }
            out
        }
    };
}
stored_op!(add_float, "fadd dword ptr [{operand}]");
stored_op!(subtract_float, "fsub dword ptr [{operand}]");
stored_op!(multiply_float, "fmul dword ptr [{operand}]");
unsafe fn unsigned(value: u32) -> F80 {
    let value = value as i32;
    let mut out = F80::from_i32(value);
    if value < 0 {
        out = add_float(out, unsafe { *unsigned_correction() });
    }
    out
}
// A preparation step returns before the game call. The entry then marshals the registers and
// arguments at the original frame depth; game code may retain those stack addresses.
#[repr(C)]
pub(super) struct PlacementCall {
    pub target: usize,
    pub eax: u32,
    pub ecx: u32,
    pub edx: u32,
    count: u32,
    args: [u32; 8],
    pub depth: i32,
}
impl PlacementCall {
    pub(super) fn set(&mut self, target: usize, this: *mut u8, args: &[u32], eax: u32, edx: u32) {
        self.depth = 0;
        self.target = target;
        self.eax = eax;
        self.ecx = this as u32;
        self.edx = edx;
        self.count = args.len() as u32;
        for (out, &arg) in self.args.iter_mut().zip(args) {
            // Each word is marshaled separately; this crate does not import a memory-copy helper.
            unsafe { core::ptr::write_volatile(out, arg) };
        }
    }
    pub(super) unsafe fn virtual_call(&mut self, this: *mut u8, slot: usize, args: &[u32], eax: u32, edx: u32) {
        unsafe {
            self.set(crate::abi::vslot(this, slot), this, args, eax, edx);
        }
    }
}
port! {
    /// Construct and configure a network occupant from four cell corners. Temporary model interfaces
    /// and caller argument aliases retain their original locations throughout the virtual calls.
    #[port(address = 0x0062_4210, owner = "networktool", name = "cSC4NetworkTool::CreateNetworkOccupant", naked)]
    pub unsafe extern "C" fn fc4p_00624210() {
        core::arch::naked_asm!(
            "sub esp, 12", "push ebx", "push ebp", "push esi", "push edi",
            "mov ebp, esp", "mov esi, ecx", "xor edi, edi", "xor ebx, ebx",
            "2:", "sub esp, 128", "mov ecx, esp", "push ecx", "push edx", "push eax", "push ebx", "push edi",
            "mov ecx, esi", "mov edx, ebp", "call {step}", "mov edi, eax", "mov ebx, edx",
            "cmp dword ptr [esp], 0", "jz 5f", "mov ecx, [esp + 16]", "mov edx, esp", "mov esp, ebp",
            "3:", "test ecx, ecx", "jz 4f", "dec ecx", "push dword ptr [edx + ecx*4 + 20]", "jmp 3b",
            "4:", "mov eax, [ebp - 124]", "mov ecx, [ebp - 120]", "mov edx, [ebp - 116]",
            "call dword ptr [ebp - 128]", "mov esp, ebp", "cmp edi, 1", "jne 2b",
            "fstp dword ptr [ebp + 16]", "jmp 2b",
            "5:", "mov eax, [esp + 4]", "mov edx, [esp + 12]", "mov esp, ebp",
            "pop edi", "pop esi", "pop ebp", "pop ebx", "add esp, 12", "ret 16",
            step = sym placement_step,
        );
    }
}
unsafe extern "fastcall" fn placement_step(
    this: *mut u8,
    frame: *mut u32,
    mut stage: u32,
    mut occupant: *mut u8,
    eax: u32,
    edx: u32,
    command: *mut PlacementCall,
) -> u64 {
    unsafe {
        let c = &mut *command;
        let points = frame.add(10).read() as *mut u8;
        let class = frame.add(11).read();
        loop {
            let cell = if stage == 0 {
                core::ptr::null_mut()
            } else {
                frame.add(5).read() as *mut u8
            };
            match stage {
                0 => {
                    frame.add(5).write(frame.add(8).read());
                    let terrain = NetworkTool::from_ptr(this).terrain();
                    c.virtual_call(terrain, 0x18, &[], read(terrain, 0), edx);
                    stage = 1;
                }
                1 => {
                    let scale = f32::from_bits(frame.add(4).read());
                    write(points, 0, multiply_float(unsigned(read(cell, 0)), scale).to_f32());
                    let z = multiply_float(unsigned(read(cell, 4)), scale);
                    let x_bits: u32 = read(points, 0);
                    write(points, 8, z.to_f32());
                    write(points, 4, read::<u32>(cell, 0x58));
                    let high_z = add_float(F80::from_f32(scale), read(points, 8));
                    write(points, 0xC, x_bits);
                    let z_bits: u32 = read(points, 8);
                    write(points, 0x14, high_z.to_f32());
                    write(points, 0x10, read::<u32>(cell, 0x5C));
                    let high_x = add_float(F80::from_f32(scale), read(points, 0));
                    write(points, 0x18, high_x.to_f32());
                    write(points, 0x20, high_z.to_f32());
                    write(points, 0x1C, read::<u32>(cell, 0x60));
                    write(points, 0x24, high_x.to_f32());
                    write(points, 0x2C, z_bits);
                    write(points, 0x28, read::<u32>(cell, 0x64));
                    let (target, size) = match class {
                        0 => (0x0061_9E90, 0x180),
                        0x49C1_A034 => (0x0061_D200, 0x1D8),
                        0x49CC_1BCD => (0x0060_5040, 0x1F0),
                        0x8A4B_D52B => (0x0064_7230, 0x1E0),
                        _ => {
                            occupant = class as *mut u8;
                            stage = 4;
                            continue;
                        }
                    };
                    c.set(
                        crate::abi::target(target),
                        core::ptr::null_mut(),
                        &[size],
                        class,
                        read(cell, 0x64),
                    );
                    stage = 2;
                }
                2 => {
                    occupant = eax as *mut u8;
                    if occupant.is_null() {
                        stage = 4;
                        continue;
                    }
                    let target = match class {
                        0 => 0x0061_BDD0,
                        0x49C1_A034 => 0x0061_E1A0,
                        0x49CC_1BCD => 0x0060_5160,
                        _ => 0x0064_76F0,
                    };
                    c.set(crate::abi::target(target), occupant, &[1], eax, edx);
                    stage = 3;
                }
                3 => {
                    occupant = eax as *mut u8;
                    stage = 4;
                    continue;
                }
                4 => {
                    (frame.add(9).read() as *mut *mut u8).write_unaligned(occupant);
                    c.virtual_call(occupant, 4, &[], read(occupant, 0), edx);
                    stage = 5;
                }
                5 => {
                    if read::<u8>(this, 0x50) == 0 {
                        let flags = read::<u32>(cell, 8) & 0xC0000;
                        c.virtual_call(occupant, 0x28, &[flags], flags, read(occupant, 0));
                        stage = 8;
                    } else {
                        let old: *mut u8 = read(cell, 12);
                        let new = (frame.add(9).read() as *mut *mut u8).read_unaligned();
                        frame.add(8).write(old as u32);
                        if new == old {
                            stage = 7;
                            continue;
                        }
                        frame.add(4).write(new as u32);
                        if new.is_null() {
                            stage = 6;
                            continue;
                        }
                        c.virtual_call(
                            new,
                            4,
                            &[],
                            (eax & !255) | u32::from(read::<u8>(this, 0x50)),
                            read(new, 0),
                        );
                        stage = 6;
                    }
                }
                6 => {
                    let old = frame.add(8).read() as *mut u8;
                    write(cell, 12, frame.add(4).read());
                    if old.is_null() {
                        stage = 7;
                        continue;
                    }
                    c.virtual_call(old, 8, &[], read(old, 0), edx);
                    stage = 7;
                }
                7 => {
                    let flags = read(cell, 8);
                    c.virtual_call(occupant, 0x28, &[flags], flags, read(occupant, 0));
                    stage = 8;
                }
                8 => {
                    c.virtual_call(occupant, 0xC, &[points as u32], eax, read(occupant, 0));
                    stage = 9;
                }
                9 => {
                    let real = read::<u8>(this, 0x50);
                    let (slot, a, d) = if real != 0 {
                        (0x2C, read(occupant, 0), edx)
                    } else {
                        (0x28, eax & !255, read(occupant, 0))
                    };
                    c.virtual_call(occupant, slot, &[0x8000_0000], a, d);
                    stage = 10;
                }
                10 => {
                    frame.add(6).write(0);
                    c.virtual_call(occupant, 0, &[0x49C1_A034, frame.add(6) as u32], read(occupant, 0), edx);
                    stage = 11;
                }
                11 => {
                    if eax as u8 == 0 {
                        stage = 12;
                        continue;
                    }
                    c.set(
                        crate::abi::target(0x0061_D230),
                        frame.add(6).read() as *mut u8,
                        &[],
                        eax,
                        edx,
                    );
                    stage = 12;
                }
                12 => {
                    if class == 0x8A4B_D52B {
                        c.virtual_call(occupant, 0x28, &[0x1000_0000], read(occupant, 0), edx);
                        stage = 13;
                    } else {
                        c.virtual_call(occupant, 0x28, &[0x4000], eax, read(occupant, 0));
                        stage = 14;
                    }
                }
                13 => {
                    c.virtual_call(occupant, 0x2C, &[0x4000], eax, read(occupant, 0));
                    stage = 14;
                }
                14 => {
                    stage = if read::<u8>(this, 0x50) != 0 { 0x100 } else { 15 };
                    continue;
                }
                0x100..=0x10C => {
                    let i = (stage - 0x100) as usize;
                    stage += 1;
                    if read::<u32>(cell, 8) & (*network_flags())[i] == 0 {
                        continue;
                    }
                    let value = read(cell, 0x18 + i * 4);
                    c.virtual_call(occupant, 0x94, &[i as u32, value], value, read(occupant, 0));
                }
                0x10D => {
                    let direction = u32::from(read::<u8>(cell, 0x50));
                    c.virtual_call(occupant, 0x118, &[direction], direction, read(occupant, 0));
                    stage = 15;
                }
                15 => {
                    let model = read(this, 0x32C);
                    c.virtual_call(occupant, 0x100, &[model, 0], model, read(occupant, 0));
                    stage = 16;
                }
                16 => {
                    let interface = frame.add(6).read() as *mut u8;
                    if interface.is_null() {
                        c.target = 0;
                        c.eax = eax;
                        c.edx = edx;
                        break;
                    }
                    c.virtual_call(interface, 8, &[], eax, read(interface, 0));
                    stage = 17;
                }
                _ => {
                    c.target = 0;
                    c.eax = eax;
                    c.edx = edx;
                    break;
                }
            }
            break;
        }
        registers(stage, occupant as u32)
    }
}
port! {
    /// Place one piece at one cell. Query and transform its four corner heights, force real mode only
    /// for insertion and occupant finalization, then restore it before freeing the output list and
    /// releasing the cell's retained occupant. Usability examines only the supplied low byte.
    #[port(address = 0x0062_4FD0, owner = "networktool", name = "cSC4NetworkTool::PlaceNetworkOccupantById")]
    pub unsafe extern "thiscall" fn fc4p_00624fd0(this: *mut u8, x: u32, z: u32, piece: u32, usable: u32, kind: u32) -> u64 {
        unsafe {
            let mut cell = [0u8; 0xB8]; let c = cell.as_mut_ptr();
            construct_cell(c);
            write(c, 8, read::<u32>(network_flags().cast(), kind.wrapping_mul(4) as usize));
            for i in 0..4 { write(c, 0x58+i*4, 0u32); }
            let stride: u32 = read(this, 0x248); let index = stride.wrapping_mul(z).wrapping_add(x);
            let far = index.wrapping_add(stride);
            let vertices = [index, far, far.wrapping_add(1), index.wrapping_add(1)];
            for (i, vertex) in vertices.into_iter().enumerate() { write(c, 0xA8+i*4, vertex); }
            write(c, 0, x); write(c, 4, z); write(c, 0x10, 0u8);
            for i in 0..4 {
                let terrain = NetworkTool::from_ptr(this).terrain();
                let transform = crate::abi::vslot(this, 0x128);
                let altitude = vcall_st0!(terrain, 0x30, read::<u32>(c, 0xA8+i*4)).to_f32();
                let height = crate::abi::call_st0(crate::abi::Conv::Thiscall, transform, this as u32, &[altitude.to_bits()]);
                write(c, 0x58+i*4, height.to_f32());
                write(c, 0x68+i*8, subtract_float(height, read(this, 0x2C4)).to_f32());
                let upper = add_float(F80::from_f32(read(this, 0x2C4)), read(c, 0x58+i*4));
                write(c, 0x6C+i*8, upper.to_f32());
            }
            let saved_real: u8 = read(this, 0x50);
            let descriptor = [piece, 2u32, 0];
            write(this, 0x50, 1u8);
            let sentinel = allocate(12);
            write(sentinel, 0, sentinel); write(sentinel, 4, sentinel);
            let mut head = sentinel;
            vcall!(this, 0x120 => fn(*mut u8, *const u32, *mut *mut u8) -> u32, c, descriptor.as_ptr(), &mut head);
            let end = head; let mut node: *mut u8 = read(head, 0);
            while node != end {
                let occupant: *mut u8 = read(node, 8);
                if usable as u8 != 0 {
                    vcall!(occupant, 0x50 => fn(u32) -> u32, 1);
                    vcall!(occupant, 0x2C => fn(u32) -> u32, 0x4000);
                }
                vcall!(occupant, 8 => fn() -> u32);
                node = read(node, 0);
            }
            write(this, 0x50, saved_real);
            let mut node: *mut u8 = read(head, 0);
            while node != head {
                let old = node; node = read(node, 0);
                free(old);
            }
            write(head, 0, head); write(head, 4, head);
            let mut release = crate::abi::RawCall::new(crate::abi::Conv::Cdecl, crate::abi::target(0x0090_CF63), 0, &[]);
            let arguments = [head as u32]; release.args = arguments.as_ptr(); release.count = 1;
            release.eax = head as u32; release.edx = head as u32;
            crate::abi::call_raw(&mut release);
            let retained: *mut u8 = read(c, 0xC);
            if !retained.is_null() {
                let result = virtual_call(retained, 8, &[], read(retained, 0), release.edx);
                registers(result.eax, result.edx)
            } else { registers(release.eax, release.edx) }
        }
    }
}
pub const PORTS: &[Port] = &[fc4p_00624210::PORT, fc4p_00624fd0::PORT];
#[cfg(test)]
mod tests;
