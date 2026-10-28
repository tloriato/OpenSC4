// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Population-weighted EQ and HQ queries. Float results retain the x87 register's exponent range.

use super::ResidentialSim;
use crate::abi::{call_raw, vslot, Conv, RawCall};
use crate::port::Port;
use crate::x87::F80;
use core::ptr::{read_unaligned, write_unaligned};

game_fn! {
    fn tract_quotient(this: *mut u8, quotients: *mut *mut u8, populations: *mut *mut u8, x: u32, z: u32) -> St0 = "thiscall" 0x006F_6FC0;
}

/// Register results stored for the entry shim. The shim reloads ST0 without a float32/float64 return conversion.
#[repr(C)]
struct FloatResult {
    value: F80,
    eax: u32,
    edx: u32,
}
const _: () = assert!(core::mem::size_of::<FloatResult>() == 20);
const _: () = assert!(core::mem::offset_of!(FloatResult, eax) == 12);
const _: () = assert!(core::mem::offset_of!(FloatResult, edx) == 16);

macro_rules! float_port {
    ($address:literal, $name:literal, $export:ident, $body:ident, $pop:literal) => {
        port! {
            /// The entry passes the original argument words to Rust and returns its unrounded ST0 and EDX:EAX.
            #[port(address = $address, owner = "residential", name = $name, naked)]
            pub unsafe extern "C" fn $export() {
                core::arch::naked_asm!(
                    "sub esp, 20", "mov dword ptr [esp + 12], eax", "mov dword ptr [esp + 16], edx",
                    "lea edx, [esp + 20]", "push esp", "call {body}",
                    "fld tbyte ptr [esp]", "mov eax, dword ptr [esp + 12]",
                    "mov edx, dword ptr [esp + 16]", "add esp, 20", "ret {pop}",
                    body = sym $body, pop = const $pop,
                )
            }
        }
    };
}

unsafe fn pointer(p: *mut u8, offset: usize) -> *mut u8 {
    unsafe { read_unaligned(p.add(offset).cast()) }
}

unsafe fn virtual_call(object: *mut u8, slot: usize, args: &[u32], float: bool, eax: u32, edx: u32) -> RawCall {
    unsafe {
        let mut call = RawCall::new(Conv::Thiscall, vslot(object, slot), object as u32, args);
        call.take_st0 = u32::from(float);
        call.eax = eax;
        call.edx = edx;
        call_raw(&mut call);
        call
    }
}

unsafe fn population(grid: *mut u8, x: u32, z: u32) -> RawCall {
    unsafe {
        virtual_call(
            grid,
            0x58,
            &[x, z],
            false,
            pointer(grid, 0) as u32,
            pointer(grid, 0) as u32,
        )
    }
}

unsafe fn quotient(grid: *mut u8, x: u32, z: u32, eax: u32) -> RawCall {
    unsafe { virtual_call(grid, 0x58, &[x, z], true, eax, pointer(grid, 0) as u32) }
}

float_port!(
    0x006F_6FC0,
    "cSC4ResidentialSimulator::GetTractQuotient",
    fc4p_006f6fc0,
    get_tract_quotient,
    16
);

/// The caller's argument words are writable scratch space as well as inputs to this query.
unsafe extern "fastcall" fn get_tract_quotient(_this: *mut u8, stack: *mut u32, out: *mut FloatResult) {
    unsafe {
        let populations = stack.add(2).read() as *mut u8;
        let p1 = pointer(populations, 4);
        let p2 = pointer(populations, 8);
        let x = stack.add(3).read();
        let z = stack.add(4).read();
        stack.add(2).write(p1 as u32);
        let a = population(pointer(populations, 0), x, z);
        let b = population(p1, x, z);
        let c = population(p2, x, z);
        let total = (a.eax as u16).wrapping_add(b.eax as u16).wrapping_add(c.eax as u16);
        if total == 0 {
            out.write(FloatResult {
                value: F80::from_f32(0.0),
                eax: c.eax,
                edx: c.edx,
            });
            return;
        }
        let quotients = stack.add(1).read() as *mut u8;
        let q2 = pointer(quotients, 8);
        let p2 = pointer(populations, 8);
        stack.add(4).write(q2 as u32);
        let p1 = pointer(populations, 4);
        let p0 = pointer(populations, 0);
        stack.add(3).write(p1 as u32);
        let q1 = pointer(quotients, 4);
        let q0 = pointer(quotients, 0);
        stack.add(1).write(q1 as u32);
        let c = population(p2, x, z);
        stack.add(2).write(u32::from(c.eax as u16));
        stack
            .add(2)
            .write(F80::from_i32(u32::from(c.eax as u16) as i32).to_f32().to_bits());
        let c = quotient(stack.add(4).read() as *mut u8, x, z, u32::from(c.eax as u16));
        let weighted = c.st0 * F80::from_f32(f32::from_bits(stack.add(2).read()));
        stack.add(4).write(weighted.to_f32().to_bits());
        let b = population(stack.add(3).read() as *mut u8, x, z);
        stack.add(2).write(u32::from(b.eax as u16));
        stack
            .add(2)
            .write(F80::from_i32(u32::from(b.eax as u16) as i32).to_f32().to_bits());
        let b = quotient(stack.add(1).read() as *mut u8, x, z, b.eax);
        let weighted = b.st0 * F80::from_f32(f32::from_bits(stack.add(2).read()));
        let subtotal = weighted + F80::from_f32(f32::from_bits(stack.add(4).read()));
        stack.add(4).write(subtotal.to_f32().to_bits());
        let a = population(p0, x, z);
        stack.add(2).write(u32::from(a.eax as u16));
        stack
            .add(2)
            .write(F80::from_i32(u32::from(a.eax as u16) as i32).to_f32().to_bits());
        let a = quotient(q0, x, z, a.eax);
        let weighted = a.st0 * F80::from_f32(f32::from_bits(stack.add(2).read()));
        let numerator = weighted + F80::from_f32(f32::from_bits(stack.add(4).read()));
        stack.add(2).write(u32::from(total));
        out.write(FloatResult {
            value: numerator / F80::from_i32(i32::from(total)),
            eax: u32::from(total),
            edx: a.edx,
        });
    }
}

float_port!(
    0x006F_7720,
    "cSC4ResidentialSimulator::GetGlobalEQ",
    fc4p_006f7720,
    get_global_eq,
    0
);
float_port!(
    0x006F_7870,
    "cSC4ResidentialSimulator::GetGlobalHQ",
    fc4p_006f7870,
    get_global_hq,
    0
);

unsafe extern "fastcall" fn get_global_eq(this: *mut u8, _stack: *mut u32, out: *mut FloatResult) {
    unsafe { global_quotient(this, 0x7C, out) }
}
unsafe extern "fastcall" fn get_global_hq(this: *mut u8, _stack: *mut u32, out: *mut FloatResult) {
    unsafe { global_quotient(this, 0x88, out) }
}

unsafe fn global_quotient(this: *mut u8, offset: usize, out: *mut FloatResult) {
    unsafe {
        let first = virtual_call(this, 0x40, &[], false, pointer(this, 0) as u32, (*out).edx);
        if first.eax == 0 {
            out.write(FloatResult {
                value: F80::from_f32(0.0),
                eax: 0,
                edx: first.edx,
            });
            return;
        }
        let grid = pointer(this, offset);
        let count_x = virtual_call(grid, 0x30, &[], false, first.eax, pointer(grid, 0) as u32);
        let grid = pointer(this, offset);
        let count_z = virtual_call(grid, 0x34, &[], false, pointer(grid, 0) as u32, count_x.edx);
        let (nx, nz) = (count_x.eax, count_z.eax);
        let mut last_edx = count_z.edx;
        let mut sum = 0.0;
        for x in 0..nx {
            for z in 0..nz {
                // All six pointers are captured before the first grid call of this tract.
                let q2 = pointer(this, offset + 8);
                let p1 = pointer(this, 0x4C);
                let q1 = pointer(this, offset + 4);
                let p2 = pointer(this, 0x50);
                let p0 = pointer(this, 0x48);
                let q0 = pointer(this, offset);
                let a = population(p0, x, z);
                let weight = F80::from_i32(i32::from(a.eax as u16)).to_f32();
                let a = quotient(q0, x, z, a.eax);
                let first = (a.st0 * F80::from_f32(weight)).to_f32();
                let b = population(p1, x, z);
                let weight = F80::from_i32(i32::from(b.eax as u16)).to_f32();
                let b = quotient(q1, x, z, b.eax);
                let subtotal = (b.st0 * F80::from_f32(weight) + F80::from_f32(first)).to_f32();
                let c = population(p2, x, z);
                let weight = F80::from_i32(i32::from(c.eax as u16)).to_f32();
                let c = quotient(q2, x, z, c.eax);
                last_edx = c.edx;
                sum = (c.st0 * F80::from_f32(weight) + F80::from_f32(subtotal) + F80::from_f32(sum)).to_f32();
            }
        }
        let last = virtual_call(this, 0x40, &[], false, pointer(this, 0) as u32, last_edx);
        out.write(FloatResult {
            value: F80::from_f32(sum) / F80::from_i32(last.eax as i32),
            eax: last.eax,
            edx: last.edx,
        });
    }
}

float_port!(
    0x006F_79C0,
    "cSC4ResidentialSimulator::GetCellEQ",
    fc4p_006f79c0,
    get_cell_eq,
    8
);
float_port!(
    0x006F_7A10,
    "cSC4ResidentialSimulator::GetCellHQ",
    fc4p_006f7a10,
    get_cell_hq,
    8
);

unsafe extern "fastcall" fn get_cell_eq(this: *mut u8, stack: *mut u32, out: *mut FloatResult) {
    unsafe { cell_quotient(this, stack, 0x7C, out) }
}
unsafe extern "fastcall" fn get_cell_hq(this: *mut u8, stack: *mut u32, out: *mut FloatResult) {
    unsafe { cell_quotient(this, stack, 0x88, out) }
}

/// Use unsigned machine division, including its divide-error behaviour for a zero tract size.
fn divide(value: u32, divisor: u32) -> u32 {
    let mut result = value;
    unsafe {
        core::arch::asm!("xor edx, edx", "div {divisor}",divisor=in(reg) divisor,inout("eax") result,out("edx") _);
    }
    result
}

unsafe fn cell_quotient(this: *mut u8, stack: *mut u32, offset: usize, out: *mut FloatResult) {
    unsafe {
        let grid = pointer(this, offset);
        let size = vcall!(grid,0x28 => fn() -> u32);
        let z = divide(stack.add(2).read(), size);
        let size = vcall!(grid,0x28 => fn() -> u32);
        let x = divide(stack.add(1).read(), size);
        let args = [this.add(offset) as u32, this.add(0x48) as u32, x, z];
        let mut call = RawCall::new(Conv::Thiscall, crate::abi::target(0x006F_6FC0), this as u32, &args);
        call.eax = x;
        call.edx = this.add(0x48) as u32;
        call.take_st0 = 1;
        call_raw(&mut call);
        out.write(FloatResult {
            value: call.st0,
            eax: call.eax,
            edx: call.edx,
        });
    }
}

/// The four coordinates begin with the caller's existing scratch words. If no quotient passes a
/// strict comparison, those words are still used to form the returned cell coordinates.
macro_rules! extrema_port {
    ($address:literal,$name:literal,$export:ident,$body:ident) => {
        port! {
            /// Snapshot the original coordinate scratch words before entering the Rust implementation.
            #[port(address=$address,owner="residential",name=$name,naked)]
            pub unsafe extern "C" fn $export() {
                core::arch::naked_asm!(
                    "mov edx, esp", "sub esp, 40", "push dword ptr [edx - 4]",
                    "push dword ptr [edx - 8]", "push dword ptr [edx - 12]", "push dword ptr [edx - 16]",
                    "call {body}", "add esp, 40", "ret 24",body=sym $body,
                )
            }
        }
    };
}

extrema_port!(
    0x006F_7380,
    "cSC4ResidentialSimulator::GetEQMinAndMaxCellCoords",
    fc4p_006f7380,
    get_eq_extrema
);
extrema_port!(
    0x006F_7550,
    "cSC4ResidentialSimulator::GetHQMinAndMaxCellCoords",
    fc4p_006f7550,
    get_hq_extrema
);

unsafe extern "fastcall" fn get_eq_extrema(this: *mut u8, stack: *mut u32, x0: u32, z0: u32, x1: u32, z1: u32) -> u64 {
    unsafe { extrema(this, stack, 0x7C, [x0, z0, x1, z1]) }
}
unsafe extern "fastcall" fn get_hq_extrema(this: *mut u8, stack: *mut u32, x0: u32, z0: u32, x1: u32, z1: u32) -> u64 {
    unsafe { extrema(this, stack, 0x88, [x0, z0, x1, z1]) }
}

unsafe fn extrema(this: *mut u8, stack: *mut u32, offset: usize, mut coordinates: [u32; 4]) -> u64 {
    unsafe {
        if vcall!(this,0x40 => fn() -> u32) == 0 {
            for i in [3, 1, 4, 2, 5, 6] {
                write_unaligned(stack.add(i).read() as *mut u32, 0);
            }
            return (u64::from(stack.add(5).read()) << 32) | u64::from(stack.add(6).read() & !0xFF);
        }
        let nx = vcall!(pointer(this,offset),0x30 => fn() -> u32);
        let nz = vcall!(pointer(this,offset),0x34 => fn() -> u32);
        write_unaligned(stack.add(5).read() as *mut f32, 200.0);
        write_unaligned(stack.add(6).read() as *mut f32, -200.0);
        for x in 0..nx {
            for z in 0..nz {
                let sim = ResidentialSim::from_ptr(this);
                if population(sim.population_low(), x, z).eax as u16 == 0
                    && population(sim.population_medium(), x, z).eax as u16 == 0
                    && population(sim.population_high(), x, z).eax as u16 == 0
                {
                    continue;
                }
                let q = tract_quotient(this, this.add(offset).cast(), this.add(0x48).cast(), x, z);
                let min = stack.add(5).read() as *mut f32;
                if q.compare(F80::from_f32(read_unaligned(min))).less() {
                    write_unaligned(min, q.to_f32());
                    coordinates[0] = x;
                    coordinates[1] = z;
                }
                let max = stack.add(6).read() as *mut f32;
                if q.compare(F80::from_f32(read_unaligned(max))).status_word() & 0x4100 == 0 {
                    write_unaligned(max, q.to_f32());
                    coordinates[2] = x;
                    coordinates[3] = z;
                }
            }
        }
        let mut result = 0;
        for (i, coordinate) in coordinates.into_iter().enumerate() {
            let grid = pointer(this, offset);
            let size = vcall!(grid,0x28 => fn() -> i32);
            let half = size.wrapping_sub(size >> 31) >> 1;
            let call = virtual_call(grid, 0x28, &[], false, pointer(grid, 0) as u32, (size >> 31) as u32);
            let product = call.eax.wrapping_mul(coordinate);
            write_unaligned(stack.add(i + 1).read() as *mut u32, product.wrapping_add(half as u32));
            result = (u64::from(call.edx) << 32) | u64::from((product & !0xFF) | 1);
        }
        result
    }
}

pub const PORTS: &[Port] = &[
    fc4p_006f6fc0::PORT,
    fc4p_006f7380::PORT,
    fc4p_006f7550::PORT,
    fc4p_006f7720::PORT,
    fc4p_006f7870::PORT,
    fc4p_006f79c0::PORT,
    fc4p_006f7a10::PORT,
];

#[cfg(test)]
mod tests;
