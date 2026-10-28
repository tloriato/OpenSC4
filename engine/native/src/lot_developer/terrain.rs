// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Vertex queries, rectangular scans and slope wrappers. Floating results keep their x87 exponent range.

use super::*;

game_fn! { fn curve_output(curve: *mut u8, input: f32) -> St0 = "thiscall" 0x005F_4250; }
game_data! { static slope_scale: f32 = 0x00AB_1B04; }
game_data! { static coverage_inset: f32 = 0x00A8_80D8; }
game_data! { static level_sentinel: f32 = 0x00AB_1B00; }
game_data! { static level_slope_limit: f32 = 0x00A8_FAF0; }

/// The entry adapter retains the integer results as well as the unrounded floating result.
#[repr(C)]
struct FloatReturn {
    eax: u32,
    edx: u32,
    st0: F80,
}

// The original caller's scratch space remains available above the adapter's result area. The Rust body
// receives the original return-address pointer so stack arguments and local output addresses keep their role.
macro_rules! float_port {
    ($address:literal, $export:ident, $name:literal, $body:ident, $pop:literal) => {
        port! {
            /// Returns an unrounded x87 value together with the original integer return registers.
            #[port(address = $address, owner = "lotdeveloper", name = $name, naked)]
            pub unsafe extern "C" fn $export() {
                core::arch::naked_asm!(
                    "sub esp, 64", "mov dword ptr [esp], ebp", "lea ebp, [esp + 64]",
                    "mov dword ptr [esp + 4], eax", "mov dword ptr [esp + 8], edx",
                    "mov edx, ebp", "lea eax, [esp + 4]", "push eax", "call {body}",
                    "fld tbyte ptr [esp + 12]", "mov eax, dword ptr [esp + 4]",
                    "mov edx, dword ptr [esp + 8]", "mov ebp, dword ptr [esp]", "add esp, 64",
                    concat!("ret ", stringify!($pop)), body = sym $body,
                )
            }
        }
    };
}

/// Arithmetic with a stored operand, retaining its role when NaN payloads differ.
macro_rules! stored_op {
    ($name:ident, $ty:ty, $instruction:literal) => {
        pub(super) fn $name(value: F80, operand: $ty) -> F80 {
            let mut output = F80::ZERO;
            unsafe {
                core::arch::asm!("fld tbyte ptr [{value}]", $instruction, "fstp tbyte ptr [{output}]",
                    value = in(reg) &value, operand = in(reg) &operand, output = in(reg) &mut output,
                    out("st(0)") _,);
            }
            output
        }
    };
}
stored_op!(subtract_float, f32, "fsub dword ptr [{operand}]");
stored_op!(add_float, f32, "fadd dword ptr [{operand}]");
stored_op!(multiply_float, f32, "fmul dword ptr [{operand}]");
stored_op!(multiply_integer, i32, "fimul dword ptr [{operand}]");
stored_op!(divide_from_float, f32, "fdivr dword ptr [{operand}]");

/// A comparison performed while one returned value still occupies ST0 has TOP one below the body's TOP.
fn with_live_value(status: u16) -> u16 {
    (status & !0x3800) | (status.wrapping_sub(0x0800) & 0x3800)
}

/// Compares the returned water value with stored altitude, retaining the live value for the status word.
/// The stored operand is not loaded or quieted before the water callback.
fn compare_stored_altitude(value: F80, altitude: f32) -> u16 {
    let status: u16;
    unsafe {
        core::arch::asm!("fld tbyte ptr [{value}]", "fcom dword ptr [{altitude}]", "fnstsw ax", "fstp st(0)",
            value = in(reg) &value, altitude = in(reg) &altitude, out("ax") status, out("st(0)") _,);
    }
    status
}

unsafe fn finish(output: *mut FloatReturn, call: RawCall, st0: F80) {
    unsafe {
        output.write(FloatReturn {
            eax: call.eax,
            edx: call.edx,
            st0,
        });
    }
}

float_port!(
    0x006C_4310,
    fc4p_006c4310,
    "cSC4LotDeveloper::GetTerrainAltitudeByVertex",
    vertex_altitude,
    12
);

unsafe extern "fastcall" fn vertex_altitude(this: *mut u8, stack: *mut u32, output: *mut FloatReturn) {
    unsafe {
        let developer = LotDeveloper::from_ptr(this);
        let index = developer
            .vertices_x()
            .wrapping_mul(stack.add(2).read() as i32)
            .wrapping_add(stack.add(1).read() as i32);
        let terrain = developer.terrain();
        let mut call = virtual_call(terrain, 0x30, &[index as u32], table(terrain), (*output).edx, true);
        let altitude = call.st0.to_f32();
        let water = stack.add(3).read();
        call.eax = (call.eax & !255) | (water & 255);
        let answer = if water as u8 != 0 {
            let terrain = developer.terrain();
            call = virtual_call(terrain, 0x7C, &[index as u32], call.eax, table(terrain), true);
            let status = compare_stored_altitude(call.st0, altitude);
            call.eax = (call.eax & 0xFFFF_0000) | u32::from(status);
            if status & 0x4100 == 0 {
                call.st0
            } else {
                F80::from_f32(altitude)
            }
        } else {
            F80::from_f32(altitude)
        };
        finish(output, call, answer);
    }
}

port! {
    /// Samples the four corners clockwise, rereading the rectangle before every call and storing each result
    /// immediately. Output may alias the rectangle or be observed by the next callback.
    #[port(address = 0x006C_4360, owner = "lotdeveloper", name = "cSC4LotDeveloper::GetCornerAltitudes")]
    pub unsafe extern "fastcall" fn fc4p_006c4360(
        this: *mut u8, entry_edx: u32, rect: *const i32, output: *mut f32, water: u32,
    ) -> u64 {
        unsafe {
            let mut call = RawCall::new(Conv::Thiscall, 0, 0, &[]);
            call.edx = entry_edx;
            for corner in 0..4 {
                let (x, z) = match corner {
                    0 => (rect.read_unaligned(), rect.add(1).read_unaligned()),
                    1 => (rect.add(2).read_unaligned().wrapping_add(1), rect.add(1).read_unaligned()),
                    2 => (rect.add(2).read_unaligned().wrapping_add(1), rect.add(3).read_unaligned().wrapping_add(1)),
                    _ => (rect.read_unaligned(), rect.add(3).read_unaligned().wrapping_add(1)),
                };
                call = virtual_call(this, 0x3C, &[x as u32, z as u32, water], table(this), x as u32, true);
                output.add(corner).write_unaligned(call.st0.to_f32());
            }
            result(call)
        }
    }
}

float_port!(
    0x006C_5620,
    fc4p_006c5620,
    "cSC4LotDeveloper::GetLowestOrHighestAreaAltitude(cells)",
    extreme_altitude,
    12
);

unsafe extern "fastcall" fn extreme_altitude(this: *mut u8, stack: *mut u32, output: *mut FloatReturn) {
    unsafe {
        let rect = stack.add(1).read() as *const i32;
        let lowest = stack.add(2).read() as u8;
        let water = stack.add(3).read();
        let x0 = rect.read_unaligned();
        let z0 = rect.add(1).read_unaligned();
        let x1 = rect.add(2).read_unaligned().wrapping_add(1);
        let z1 = rect.add(3).read_unaligned().wrapping_add(1);
        let mut altitude = if lowest != 0 { f32::MAX } else { -f32::MAX };
        let mut call = RawCall::new(Conv::Thiscall, 0, 0, &[]);
        call.eax = (rect as u32 & !255) | u32::from(lowest);
        call.edx = (*output).edx;
        let mut x = x0;
        while x <= x1 {
            let mut z = z0;
            while z <= z1 {
                call = virtual_call(this, 0x3C, &[x as u32, z as u32, water], table(this), call.edx, true);
                let cmp = F80::from_f32(altitude).compare(call.st0);
                call.eax = (call.eax & 0xFFFF_0000) | u32::from(with_live_value(cmp.status_word()));
                let replace = if lowest != 0 {
                    !cmp.less() && !cmp.equal() && !cmp.unordered()
                } else {
                    cmp.less()
                };
                if replace {
                    altitude = call.st0.to_f32();
                }
                z = z.wrapping_add(1);
            }
            x = x.wrapping_add(1);
        }
        finish(output, call, F80::from_f32(altitude));
    }
}

float_port!(
    0x006C_5B70,
    fc4p_006c5b70,
    "cSC4LotDeveloper::GetAverageAreaAltitude",
    average_altitude,
    8
);

unsafe extern "fastcall" fn average_altitude(this: *mut u8, stack: *mut u32, output: *mut FloatReturn) {
    unsafe {
        let rect = stack.add(1).read() as *const i32;
        let water = stack.add(2).read();
        let x0 = rect.read_unaligned();
        let z0 = rect.add(1).read_unaligned();
        let x1 = rect.add(2).read_unaligned().wrapping_add(1);
        let z1 = rect.add(3).read_unaligned().wrapping_add(1);
        let mut sum = 0.0;
        let mut count = 0i32;
        let mut call = RawCall::new(Conv::Thiscall, 0, 0, &[]);
        call.eax = rect as u32;
        call.edx = (*output).edx;
        let mut x = x0;
        while x <= x1 {
            let mut z = z0;
            if z <= z1 {
                count = count.wrapping_add(z1.wrapping_sub(z).wrapping_add(1));
                call.edx = count as u32;
            }
            while z <= z1 {
                call = virtual_call(this, 0x3C, &[x as u32, z as u32, water], table(this), call.edx, true);
                sum = add_float(call.st0, sum).to_f32();
                z = z.wrapping_add(1);
            }
            x = x.wrapping_add(1);
        }
        finish(output, call, divide_from_float(F80::from_i32(count), sum));
    }
}

float_port!(
    0x006C_5F20,
    fc4p_006c5f20,
    "cSC4LotDeveloper::GetLotSlope",
    lot_slope,
    8
);

unsafe extern "fastcall" fn lot_slope(this: *mut u8, stack: *mut u32, output: *mut FloatReturn) {
    unsafe {
        let lot = stack.add(1).read() as *mut u8;
        let rect = stack.sub(4);
        vcall!(lot, 0x38 => fn(*mut u32) -> u32, rect);
        let call = virtual_call(
            this,
            0x74,
            &[rect as u32, stack.add(2).read()],
            table(this),
            rect as u32,
            true,
        );
        finish(output, call, call.st0);
    }
}

float_port!(
    0x006C_5F50,
    fc4p_006c5f50,
    "cSC4LotDeveloper::GetOccupantSlope",
    occupant_slope,
    8
);

unsafe extern "fastcall" fn occupant_slope(this: *mut u8, stack: *mut u32, output: *mut FloatReturn) {
    unsafe {
        let occupant = stack.add(1).read();
        let rect = stack.sub(4);
        vcall!(this, 0x58 => fn(u32, *mut u32) -> u32, occupant, rect);
        let call = virtual_call(
            this,
            0x74,
            &[rect as u32, stack.add(2).read()],
            table(this),
            rect as u32,
            true,
        );
        finish(output, call, call.st0);
    }
}

port! {
    /// Evaluates the slope cost curve, multiplying by z count and then x count before the game's int64 truncation.
    /// Rectangle dimensions are captured before callbacks and wrap as signed 32-bit integers.
    #[port(address = 0x006C_5F80, owner = "lotdeveloper", name = "cSC4LotDeveloper::GetLotDevelopmentCost")]
    pub unsafe extern "thiscall" fn fc4p_006c5f80(this: *mut u8, rect: *const i32) -> i64 {
        unsafe {
            let x = rect.add(2).read_unaligned().wrapping_sub(rect.read_unaligned()).wrapping_add(1);
            let z = rect.add(3).read_unaligned().wrapping_sub(rect.add(1).read_unaligned()).wrapping_add(1);
            let slope = vcall_st0!(this, 0x74, rect, 1u32);
            let input = multiply_float(slope, *slope_scale()).to_f32();
            let value = curve_output(this.add(0xEC), input);
            multiply_integer(multiply_integer(value, z), x).ftol()
        }
    }
}

port! {
    /// Converts the occupant's inset minimum and maximum bounds to cell coordinates. The two city calls
    /// write directly to the caller's rectangle, minimum first.
    #[port(address = 0x006C_4590, owner = "lotdeveloper", name = "cSC4LotDeveloper::GetOccupantCellCoverage")]
    pub unsafe extern "thiscall" fn fc4p_006c4590(this: *mut u8, occupant: *mut u8, rect: *mut u32) -> u64 {
        unsafe {
            let developer = LotDeveloper::from_ptr(this);
            let mut min = core::mem::MaybeUninit::<[f32; 3]>::uninit();
            let mut max = core::mem::MaybeUninit::<[f32; 3]>::uninit();
            vcall!(occupant, 0x28 => fn(*mut f32, *mut f32) -> u32, min.as_mut_ptr().cast(), max.as_mut_ptr().cast());
            let min = min.assume_init();
            let max = max.assume_init();
            let min_x = add_float(F80::from_f32(min[0]), *coverage_inset()).to_f32();
            let city = developer.city();
            let min_z = add_float(F80::from_f32(min[2]), *coverage_inset()).to_f32();
            let max_x = subtract_float(F80::from_f32(max[0]), *coverage_inset()).to_f32();
            let max_z = subtract_float(F80::from_f32(max[2]), *coverage_inset()).to_f32();
            vcall!(city, 0x17C => fn(f32, f32, *mut u32, *mut u32) -> u32,
                min_x, min_z, rect, rect.add(1));
            let city = developer.city();
            result(virtual_call(city, 0x17C,
                &[max_x.to_bits(), max_z.to_bits(), rect.add(2) as u32, rect.add(3) as u32],
                table(city), max_x.to_bits(), false))
        }
    }
}

port! {
    /// Levels the inclusive vertex rectangle through the demolition service. The third stack word is ignored.
    #[port(address = 0x006C_44B0, owner = "lotdeveloper", name = "cSC4LotDeveloper::LevelBorderingVertices")]
    pub unsafe extern "thiscall" fn fc4p_006c44b0(this: *mut u8, rect: *const i32, target: u32, _unused: u32, lot: u32) -> u64 {
        unsafe {
            let demolition = LotDeveloper::from_ptr(this).demolition();
            let x = rect.read_unaligned();
            result(virtual_call(demolition, 0x24, &[1, 1, x as u32, rect.add(1).read_unaligned() as u32,
                rect.add(2).read_unaligned().wrapping_add(1) as u32, rect.add(3).read_unaligned().wrapping_add(1) as u32,
                target, lot, 5, 0], x as u32, table(demolition), false))
        }
    }
}

port! {
    /// Levels internal vertices in x then z order, counting modifications. Negative targets use the average;
    /// water vertices may be skipped, and unforced changes require a strict positive altitude delta.
    #[port(address = 0x006C_43D0, owner = "lotdeveloper", name = "cSC4LotDeveloper::LevelAreaInternalVertices")]
    pub unsafe extern "fastcall" fn fc4p_006c43d0(this: *mut u8, entry_edx: u32, rect: *const i32,
        target: f32, skip_water: u32, force: u32, lot: u32) -> u64 {
        unsafe {
            let developer = LotDeveloper::from_ptr(this);
            let mut target = target;
            let mut edx = entry_edx;
            if F80::from_f32(target).compare(F80::ZERO).less() {
                let call = virtual_call(this, 0x48, &[rect as u32, 1], table(this), edx, true);
                target = call.st0.to_f32();
                edx = call.edx;
            }
            let mut count = 0u32;
            let mut x = rect.read_unaligned().wrapping_add(1);
            while x <= rect.add(2).read_unaligned() {
                let mut z = rect.add(1).read_unaligned().wrapping_add(1);
                while z <= rect.add(3).read_unaligned() {
                    let mut wet = false;
                    if skip_water as u8 != 0 {
                        wet = vcall!(developer.terrain(), 0x94 => fn(i32, i32) -> u32, x, z) as u8 != 0;
                    }
                    if !wet {
                        let call = virtual_call(this, 0x3C, &[x as u32, z as u32, 1], table(this), edx, true);
                        let delta = subtract_float(call.st0, target);
                        let change = if force as u8 != 0 { true } else {
                            let cmp = delta.compare(F80::from_f32(developer.level_delta()));
                            !cmp.less() && !cmp.equal() && !cmp.unordered()
                        };
                        if change {
                            let demolition = developer.demolition();
                            vcall!(demolition, 0x24 => fn(u32, u32, i32, i32, i32, i32, f32, u32, u32, u32) -> u32,
                                1, 1, x, z, x, z, target, lot, 5, 0);
                            count = count.wrapping_add(1);
                        }
                    }
                    z = z.wrapping_add(1);
                }
                edx = rect as u32;
                x = x.wrapping_add(1);
            }
            (u64::from(edx) << 32) | u64::from(count)
        }
    }
}

/// Quiet comparison used for the sentinel test; unordered values select the tuned maximum too.
fn is_sentinel(value: f32, sentinel: f32) -> bool {
    let status: u16;
    unsafe {
        core::arch::asm!("fld dword ptr [{sentinel}]", "fld dword ptr [{value}]", "fucompp", "fnstsw ax",
            value = in(reg) &value, sentinel = in(reg) &sentinel, out("ax") status,
            out("st(0)") _, out("st(1)") _,);
    }
    status & 0x4400 != 0
}

port! {
    /// Levels the whole rectangle to its stored vertex average unless an enabled slope limit rejects it.
    /// The sentinel or an unordered maximum selects the tuned limit. The second stack word is unused.
    #[port(address = 0x006C_44F0, owner = "lotdeveloper", name = "cSC4LotDeveloper::AttemptAreaLevelling")]
    pub unsafe extern "thiscall" fn fc4p_006c44f0(this: *mut u8, rect: *const i32, _unused: u32,
        water: u32, per_vertex: u32, max_slope: f32, lot: u32) -> u64 {
        unsafe {
            let developer = LotDeveloper::from_ptr(this);
            let average = vcall_st0!(this, 0x48, rect, water).to_f32();
            let maximum = if is_sentinel(max_slope, *level_sentinel()) { developer.level_max_slope() } else { max_slope };
            if F80::from_f32(maximum).compare(F80::from_f32(*level_slope_limit())).less() {
                let mut call = virtual_call(this, 0x74, &[rect as u32, water], water, table(this), true);
                let cmp = call.st0.compare(F80::from_f32(maximum));
                if !cmp.less() && !cmp.equal() && !cmp.unordered() {
                    call.eax = 0;
                    return result(call);
                }
            }
            let demolition = developer.demolition();
            result(virtual_call(demolition, 0x24,
                &[1, per_vertex, rect.read_unaligned() as u32, rect.add(1).read_unaligned() as u32,
                    rect.add(2).read_unaligned().wrapping_add(1) as u32, rect.add(3).read_unaligned().wrapping_add(1) as u32,
                    average.to_bits(), lot, 5, 0], table(demolition), per_vertex, false))
        }
    }
}

pub const PORTS: &[Port] = &[
    fc4p_006c4310::PORT,
    fc4p_006c4360::PORT,
    fc4p_006c43d0::PORT,
    fc4p_006c44b0::PORT,
    fc4p_006c44f0::PORT,
    fc4p_006c4590::PORT,
    fc4p_006c5620::PORT,
    fc4p_006c5b70::PORT,
    fc4p_006c5f20::PORT,
    fc4p_006c5f50::PORT,
    fc4p_006c5f80::PORT,
];
