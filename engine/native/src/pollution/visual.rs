// SPDX-License-Identifier: GPL-3.0-or-later
//! Pollution effect scanning, worst blocks and effect lifetimes.

use super::*;

game_fn! { fn reset(iterator: *mut u8, shuffle: u32) -> u32 = "thiscall" 0x0049_D1C0; }
game_fn! { fn curve_output(curve: *mut u8, value: f32) -> St0 = "thiscall" 0x005F_4250; }
game_fn! { fn random_range(state: *mut u8, low_lo: u32, low_hi: u32, high_lo: u32, high_hi: u32) -> St0 = "thiscall" 0x0091_3A46; }
game_data! { static city: *mut u8 = 0x00B4_3CEC; }
game_data! { static terrain: *mut u8 = 0x00B4_3CF4; }
game_data! { static water_curve: *mut u8 = 0x00B4_9BB0; }
game_data! { static air_curve: *mut u8 = 0x00B4_9BB4; }
game_data! { static radiation_curve: *mut u8 = 0x00B4_9BB8; }
game_data! { static garbage_curve: *mut u8 = 0x00B4_9BBC; }
game_data! { static random_state: u8 = 0x00B4_9BD0; }
game_data! { static block_size: f32 = 0x00A8_E178; }
game_data! { static scale: f32 = 0x00AB_25E4; }
game_data! { static threshold: f32 = 0x00A8_80D8; }
game_data! { static radiation_height: f32 = 0x00A8_5074; }

layout! {
    /// Visual-effect services and slot-array beginnings in the primary simulator.
    struct VisualView {
        0x18 => city / set_city: *mut u8,
        0x38 => effects / set_effects: *mut u8,
        0x78 => iterator / set_iterator: [u8; 48],
        0xA8 => water_effects / set_water_effects: *mut *mut u8,
        0xB4 => air_effects / set_air_effects: *mut *mut u8,
        0xC0 => radiation_effects / set_radiation_effects: *mut *mut u8,
        0xCC => garbage_effects / set_garbage_effects: *mut *mut u8,
        0x20C => visible / set_visible: u8,
    }
}

#[repr(C)]
struct Transform {
    flags: u32,
    matrix: [f32; 13],
}

impl Transform {
    fn at(position: [f32; 3]) -> Self {
        Self {
            flags: 0x0101,
            matrix: [
                1.0,
                0.0,
                0.0,
                0.0,
                1.0,
                0.0,
                0.0,
                0.0,
                1.0,
                position[0],
                position[1],
                position[2],
                1.0,
            ],
        }
    }
}

macro_rules! stored_op {
    ($name:ident, $ty:ty, $instruction:literal) => {
        fn $name(value: F80, operand: $ty) -> F80 {
            let input=value.to_bytes();
            let mut output=[0u8;10];
            // SAFETY: live memory operands; the x87 stack is unchanged on return.
            unsafe {
                core::arch::asm!("fld tbyte ptr [{input}]",$instruction,"fstp tbyte ptr [{output}]",
                    input=in(reg)&input,operand=in(reg)&operand,output=in(reg)&mut output,out("st(0)") _);
            }
            F80::from_bytes(output)
        }
    };
}
stored_op!(multiply_float, f32, "fmul dword ptr [{operand}]");
stored_op!(multiply_integer, i32, "fimul dword ptr [{operand}]");
stored_op!(add_integer, i32, "fiadd dword ptr [{operand}]");
stored_op!(add_double, f64, "fadd qword ptr [{operand}]");
stored_op!(add_float, f32, "fadd dword ptr [{operand}]");

unsafe fn jitter(quarter: i32) -> f64 {
    let high = F80::from_i32(quarter);
    let low = (-high).to_f64().to_bits();
    let high = high.to_f64().to_bits();
    // SAFETY: the game's random state and the original four stack words for two doubles.
    unsafe {
        random_range(
            random_state(),
            low as u32,
            (low >> 32) as u32,
            high as u32,
            (high >> 32) as u32,
        )
        .to_f64()
    }
}

unsafe fn position(sim: VisualView, kind: usize, x: i32, z: i32, width: i32, height: i32) -> [f32; 3] {
    // SAFETY: original city and terrain services. Each city pointer is captured before its corresponding random call.
    unsafe {
        if kind == 0 {
            let city = sim.city();
            let offset = jitter(width / 4);
            let px = add_double(multiply_integer(vcall_st0!(city, 0x16C), x), offset).to_f32();
            let city = sim.city();
            let offset = jitter(height / 4);
            let pz = add_double(multiply_integer(vcall_st0!(city, 0x170), z), offset).to_f32();
            [px, 0.0, pz]
        } else {
            let px = add_integer(multiply_integer(vcall_st0!(sim.city(), 0x16C), x), width / 2).to_f32();
            let pz = add_integer(multiply_integer(vcall_st0!(sim.city(), 0x170), z), height / 2).to_f32();
            let altitude = vcall_st0!(*terrain(), 0x34, px, pz);
            let py = if kind == 2 {
                add_float(altitude, *radiation_height())
            } else {
                altitude
            };
            [px, py.to_f32(), pz]
        }
    }
}

unsafe fn update(sim: VisualView, kind: usize, slot: *mut *mut u8, mut strength: f32, shown: bool, block: [i32; 4]) {
    // SAFETY: the original slot destination, effects manager and effect interfaces.
    unsafe {
        let parameter = if kind == 0 { 3 } else { 2 };
        if shown {
            if read_unaligned(slot).is_null() {
                let [x, z, width, height] = block;
                let mut transform = Transform::at(position(sim, kind, x, z, width, height));
                vcall!(sim.effects(),0x18 => fn(u32,u32,*mut *mut u8),0x0A78_747A,kind as u32+1,slot);
                if !read_unaligned(slot).is_null() {
                    vcall!(read_unaligned(slot),0x20 => fn(*mut Transform),&mut transform);
                    vcall!(read_unaligned(slot),0x30 => fn(u32,*mut f32,u32),parameter,&mut strength,1);
                    vcall!(read_unaligned(slot),0x0C => fn(u32),0);
                }
            } else {
                vcall!(read_unaligned(slot),0x30 => fn(u32,*mut f32,u32),parameter,&mut strength,1);
            }
        } else if !read_unaligned(slot).is_null() {
            if vcall!(read_unaligned(slot),0x14 => fn() -> u32) as u8 != 0 {
                vcall!(read_unaligned(slot),0x10 => fn(u32),0);
            }
            vcall!(read_unaligned(slot),0x08 => fn());
            write_unaligned(slot, core::ptr::null_mut());
        }
    }
}

port! {
    /// Resets the shuffled block iterator, sums cells x before z in water/air/radiation/garbage order,
    /// and tracks strictly worse unsigned block averages. Water effects ignore the visibility flag.
    /// Existing effects update strength only; new effects receive a transform and start; hidden effects
    /// stop when started, release, and clear their slots. EAX retains the final iterator result.
    #[port(address = 0x006E_63C0, owner = "pollution", name = "cSC4PollutionSimulator::UpdatePollutionVisualEffect", naked)]
    pub unsafe extern "thiscall" fn fc4p_006e63c0(this: *mut u8) -> u32 {
        core::arch::naked_asm!(
            "push ebp", "mov ebp, esp", "and esp, -8", "sub esp, 0x168",
            "push ebx", "push ebp", "push esi", "push edi", "mov edx, esp",
            "call {body}", "pop edi", "pop esi", "pop ebp", "pop ebx",
            "mov esp, ebp", "pop ebp", "ret", body = sym visual_body,
        );
    }
}

// GetNext changes only AL on exhaustion. Its incoming EAX is the original z-local
// address, which survives in the upper return bytes. Keep both the frame and input.
unsafe fn next(iterator: *mut u8, x: *mut i32, z: *mut i32, eax: u32, edx: u32) -> u32 {
    let args = [x as u32, z as u32];
    let mut call = RawCall::new(Conv::Thiscall, crate::abi::target(0x0049_CB10), iterator as u32, &args);
    call.eax = eax;
    call.edx = edx;
    unsafe { call_raw(&mut call) };
    call.eax
}

unsafe extern "fastcall" fn visual_body(this: *mut u8, frame: *mut u8) -> u32 {
    // SAFETY: original grids, slot arrays, curves, iterator, city and effect services.
    unsafe {
        let sim = VisualView::from_ptr(this);
        let statistics = PollutionSim::from_ptr(this);
        let width = multiply_float(vcall_st0!(sim.city(), 0x16C), *block_size()).ftol() as i32;
        let height = multiply_float(vcall_st0!(sim.city(), 0x170), *block_size()).ftol() as i32;
        let stride = vcall!(*city(),0x174 => fn() -> i32) / 8;
        let reset_result = reset(this.add(0x78), 1);
        statistics.set_worst_air_value(0);
        statistics.set_worst_water_value(0);
        statistics.set_worst_garbage_value(0);
        let bx_slot = frame.add(0x34).cast::<i32>();
        let bz_slot = frame.add(0x30).cast::<i32>();
        let mut result = next(this.add(0x78), bx_slot, bz_slot, reset_result, bx_slot as u32);
        while result as u8 != 0 {
            let bx = bx_slot.read_unaligned();
            let bz = bz_slot.read_unaligned();
            let x0 = bx.wrapping_mul(8);
            let z0 = bz.wrapping_mul(8);
            let mut sums = [0u32; 4];
            let mut x = x0;
            while x <= x0.wrapping_add(7) {
                let mut z = z0;
                while z <= z0.wrapping_add(7) {
                    let water = vcall!(statistics.water(),0x1C => fn(i32,i32) -> u32,x,z);
                    sums[0] = sums[0].wrapping_add(water as i16 as i32 as u32);
                    let air = vcall!(statistics.air(),0x1C => fn(i32,i32) -> u32,x,z);
                    sums[1] = sums[1].wrapping_add(air as i16 as i32 as u32);
                    let radiation = vcall!(statistics.radiation_source(),0x1C => fn(i32,i32) -> u32,x,z);
                    sums[2] = sums[2].wrapping_add(radiation as i16 as i32 as u32);
                    let garbage = vcall!(statistics.garbage(),0x1C => fn(i32,i32) -> u32,x,z);
                    sums[3] = sums[3].wrapping_add(garbage & 0x7FFF);
                    z = z.wrapping_add(1);
                }
                x = x.wrapping_add(1);
            }
            let averages = sums.map(|sum| sum >> 6);
            if averages[1] > statistics.worst_air_value() {
                statistics.set_worst_air_value(averages[1]);
                statistics.set_worst_air_x(x0.wrapping_add(4) as u16);
                statistics.set_worst_air_z(z0.wrapping_add(4) as u16);
            }
            if averages[0] > statistics.worst_water_value() {
                statistics.set_worst_water_x(x0.wrapping_add(4) as u16);
                statistics.set_worst_water_value(averages[0]);
                statistics.set_worst_water_z(z0.wrapping_add(4) as u16);
            }
            if averages[3] > statistics.worst_garbage_value() {
                statistics.set_worst_garbage_value(averages[3]);
                statistics.set_worst_garbage_x(x0.wrapping_add(4) as u16);
                statistics.set_worst_garbage_z(z0.wrapping_add(4) as u16);
            }
            let offset = bz.wrapping_mul(stride).wrapping_add(bx).wrapping_mul(4);
            for (kind, &average) in averages.iter().enumerate() {
                let curve = match kind {
                    0 => *water_curve(),
                    1 => *air_curve(),
                    2 => *radiation_curve(),
                    _ => *garbage_curve(),
                };
                let input = multiply_float(F80::from_i32(average as i32), *scale()).to_f32();
                let value = curve_output(curve, input);
                let strength = value.to_f32();
                let comparison = value.compare(F80::from_f32(*threshold()));
                let above = !comparison.less() && !comparison.equal() && !comparison.unordered();
                let slots = match kind {
                    0 => sim.water_effects(),
                    1 => sim.air_effects(),
                    2 => sim.radiation_effects(),
                    _ => sim.garbage_effects(),
                };
                let slot = slots.cast::<u8>().wrapping_offset(offset as isize).cast::<*mut u8>();
                update(
                    sim,
                    kind,
                    slot,
                    strength,
                    above && (kind == 0 || sim.visible() != 0),
                    [x0, z0, width, height],
                );
            }
            result = next(this.add(0x78), bx_slot, bz_slot, bz_slot as u32, 0);
        }
        result
    }
}

/// This file's replacements.
pub const PORTS: &[Port] = &[fc4p_006e63c0::PORT];

#[cfg(test)]
mod tests;
