// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Temporary response curves for life expectancy, workforce, and property-holder tuning.

use crate::abi::{call_raw, target, Conv, RawCall};
use crate::port::Port;
use crate::x87::F80;
use core::mem::MaybeUninit;
use core::ptr::null_mut;

layout! {
    /// Three storage pointers used by a temporary vector of response-curve points.
    pub struct Points {
        0x00 => begin / set_begin: *mut u32,
        0x04 => end / set_end: *mut u32,
        0x08 => capacity / set_capacity: *mut u32,
    }
}
layout! {
    /// Only the first storage pointer of a response curve is owned by its destructor.
    struct Curve {
        0x00 => storage / set_storage: *mut u8,
    }
}
game_fn! { fn curve_new(this:*mut u8)->u32 = "thiscall" 0x007D_0410; }
game_fn! { pub(super) fn insert_points(this:*mut u8,position:*mut u32,point:*const u32,tag:*const u8,count:u32,stride:u32)->u32 = "thiscall" 0x007C_61A0; }
game_fn! { pub(super) fn set_points(this:*mut u8,points:*mut u8)->u32 = "thiscall" 0x005F_4480; }
game_fn! { fn set_property(this:*mut u8,property:*mut u8)->u32 = "thiscall" 0x005F_4620; }
game_fn! { pub(super) fn set_resource(this:*mut u8,key:*const u32,id:u32)->u32 = "thiscall" 0x005F_4700; }
game_fn! {
    // The raw call below also takes EDX, which survives when both storage pointers are null.
    #[allow(dead_code)]
    fn curve_output(this:*mut u8,input:f32)->St0 = "thiscall" 0x005F_4250;
}
game_fn! { pub(super) fn free(storage:*mut u8)->u64 = "C" 0x0090_CF63; }
game_data! { pub(super) static resources:*mut u8 = 0x00B4_3CA8; }
game_data! { static zero:u32 = 0x00B4_AF90; }
game_data! { static percent:f32 = 0x00A8_23A4; }

pub(super) const KEY: [u32; 3] = [0x6534_284A, 0xE7E2_C2DB, 0x6918_9C84];

/// The resource lookup may return only AL, so supply the original vtable and key register inputs.
pub(super) unsafe fn lookup(key: *const u32, out: *mut *mut u8) -> u64 {
    unsafe {
        let manager = *resources();
        let args = [key as u32, 0xA521_60F5, out as u32, 0, 0];
        let mut call = RawCall::new(Conv::Thiscall, crate::abi::vslot(manager, 0x0C), manager as u32, &args);
        call.eax = manager.cast::<u32>().read_unaligned();
        call.edx = key as u32;
        call_raw(&mut call);
        registers(call.eax, call.edx)
    }
}

fn registers(eax: u32, edx: u32) -> u64 {
    (u64::from(edx) << 32) | u64::from(eax)
}

/// The second push uses spare vector capacity when it exists, with the same null-slot guard.
pub(super) unsafe fn default_points(
    points: Points,
    point: &mut [u32; 2],
    first: [u32; 2],
    second: [u32; 2],
    tag: *const u8,
) {
    unsafe {
        points.set_begin(null_mut());
        points.set_end(null_mut());
        points.set_capacity(null_mut());
        *point = first;
        insert_points(points.as_ptr(), null_mut(), point.as_ptr(), tag, 1, 1);
        *point = second;
        let end = points.end();
        if end == points.capacity() {
            insert_points(points.as_ptr(), end, point.as_ptr(), tag, 1, 1);
        } else {
            if !end.is_null() {
                end.write_unaligned(point[0]);
                end.wrapping_add(1).write_unaligned(point[1]);
            }
            points.set_end(points.end().wrapping_add(2));
        }
    }
}

port! {
    /// Set three wealth curves from the selected property IDs. Each missing property uses (0,100),
    /// (100,100); present properties are passed through even when their conversion fails.
    #[port(address=0x006F_C400,owner="residential",name="cSC4ResidentialSimulator::GetPropertyHolderCurveData")]
    pub unsafe extern "thiscall" fn fc4p_006fc400(_this:*mut u8,holder:*mut u8,low:*mut u8,medium:*mut u8,high:*mut u8,school:u32) {
        unsafe {
            let ids=if school as u8!=0 {[0xC929_9662,0xC92D_7EC0,0xE921_A936]}else{[0xA92A_E3BD,0xA92D_9FCD,0x2921_A9CC]};
            let mut storage=[0u32;3];let points=Points::from_ptr(storage.as_mut_ptr().cast());let mut point=[0u32;2];
            default_points(points,&mut point,[0,100f32.to_bits()],[100f32.to_bits(),100f32.to_bits()],(&school as *const u32).cast());
            for (id,curve) in ids.into_iter().zip([low,medium,high]) {
                let property=vcall!(holder,0x24 => fn(u32)->*mut u8,id);
                if property.is_null() {set_points(curve,points.as_ptr());}else{set_property(curve,property);}
            }
            let storage=points.begin();if !storage.is_null(){free(storage.cast());}
        }
    }
}

/// Multiply the returned register by a stored single without first loading the stored operand.
pub(super) fn multiply_percent(value: F80, operand: f32) -> F80 {
    let mut out = F80::ZERO;
    unsafe {
        core::arch::asm!("fld tbyte ptr [{value}]","fmul dword ptr [{operand}]","fstp tbyte ptr [{out}]",
        value=in(reg)&value,operand=in(reg)&operand,out=in(reg)&mut out,out("st(0)") _);
    }
    out
}

/// Release only the two owned allocations, preserving the final EAX and EDX words.
unsafe fn destroy(points: Points, curve: *mut u8, edx: u32) -> u64 {
    unsafe {
        let mut result = registers(points.begin() as u32, edx);
        if !points.begin().is_null() {
            result = free(points.begin().cast());
        }
        let storage = Curve::from_ptr(curve).storage();
        result = (result & 0xFFFF_FFFF_0000_0000) | u64::from(storage as u32);
        if !storage.is_null() {
            result = free(storage);
        }
        result
    }
}

/// Shared construction preserves the different callback/release order of the four queries.
unsafe fn query(this: *mut u8, out: *mut u32, x: u32, z: u32, global: bool, workforce: bool) -> u64 {
    unsafe {
        let key = KEY;
        let mut resource = MaybeUninit::<*mut u8>::uninit();
        let result = lookup(key.as_ptr(), resource.as_mut_ptr());
        if result as u8 == 0 {
            if workforce {
                *out = *zero();
                return registers(out as u32, if global { *zero() } else { (result >> 32) as u32 });
            }
            return result & !0xFF;
        }
        let resource = resource.assume_init();
        let mut curve_storage = [MaybeUninit::<u32>::uninit(); 4];
        let curve = curve_storage.as_mut_ptr().cast::<u8>();
        curve_new(curve);
        let mut storage = [0u32; 3];
        let points = Points::from_ptr(storage.as_mut_ptr().cast());
        let mut point = [0u32; 2];
        let tag = MaybeUninit::<u8>::uninit();
        let arg_tag = if workforce { out as u32 } else { x };
        let tag_pointer = if global && !workforce {
            tag.as_ptr()
        } else {
            (&arg_tag as *const u32).cast()
        };
        default_points(
            points,
            &mut point,
            [0, 0],
            [
                200f32.to_bits(),
                if workforce { 100f32.to_bits() } else { 200f32.to_bits() },
            ],
            tag_pointer,
        );
        set_points(curve, points.as_ptr());
        set_resource(curve, key.as_ptr(), if workforce { 0x4941_A92B } else { 0x6941_A67F });
        let input;
        if global && !workforce {
            vcall!(resource,8 => fn()->u32);
            input = vcall_st0!(this, 0x84).to_f32();
        } else if workforce {
            let value = if global {
                vcall!(this,0x88 => fn()->u32)
            } else {
                vcall!(this,0x74 => fn(u32,u32)->u32,x,z)
            };
            vcall!(resource,8 => fn()->u32);
            input = F80::from_i32(i32::from(value as u8)).to_f32();
        } else {
            input = vcall_st0!(this, 0x90, x, z).to_f32();
            vcall!(resource,8 => fn()->u32);
        }
        let args = [input.to_bits()];
        let mut call = RawCall::new(Conv::Thiscall, target(0x005F_4250), curve as u32, &args);
        call.take_st0 = 1;
        call_raw(&mut call);
        if workforce {
            *out = multiply_percent(call.st0, *percent()).to_f32().to_bits();
            let cleanup = destroy(points, curve, call.edx);
            registers(out as u32, (cleanup >> 32) as u32)
        } else {
            let converted = call.st0.ftol();
            let cleanup = destroy(points, curve, ((converted as u64) >> 32) as u32);
            (cleanup & !0xFF) | u64::from(converted as u8)
        }
    }
}

port! {
    /// Build a fresh HQ-to-life-expectancy curve, round the HQ callback to single, truncate and wrap to a byte.
    #[port(address=0x006F_C570,owner="residential",name="cSC4ResidentialSimulator::GetCellLifeExpectancy")]
    pub unsafe extern "thiscall" fn fc4p_006fc570(this:*mut u8,x:u32,z:u32)->u64 {
        unsafe{query(this,null_mut(),x,z,false,false)}
    }
}
port! {
    /// Build a fresh LE-to-workforce curve and store its output times the single-precision percent constant.
    #[port(address=0x006F_C6E0,owner="residential",name="cSC4ResidentialSimulator::GetCellWorkforcePercent")]
    pub unsafe extern "thiscall" fn fc4p_006fc6e0(this:*mut u8,out:*mut u32,x:u32,z:u32)->u64 {
        unsafe{query(this,out,x,z,false,true)}
    }
}
port! {
    /// Use the global life-expectancy byte with a fresh workforce curve; return the output pointer.
    #[port(address=0x006F_C860,owner="residential",name="cSC4ResidentialSimulator::GetGlobalWorkforcePercent")]
    pub unsafe extern "thiscall" fn fc4p_006fc860(this:*mut u8,out:*mut u32)->u64 {
        unsafe{query(this,out,0,0,true,true)}
    }
}
port! {
    /// Release the tuning resource before querying global HQ; truncate the fresh curve result to a byte.
    #[port(address=0x006F_C9E0,owner="residential",name="cSC4ResidentialSimulator::GetGlobalLE")]
    pub unsafe extern "thiscall" fn fc4p_006fc9e0(this:*mut u8)->u64 {
        unsafe{query(this,null_mut(),0,0,true,false)}
    }
}
pub const PORTS: &[Port] = &[
    fc4p_006fc400::PORT,
    fc4p_006fc570::PORT,
    fc4p_006fc6e0::PORT,
    fc4p_006fc860::PORT,
    fc4p_006fc9e0::PORT,
];

#[cfg(test)]
mod tests;
