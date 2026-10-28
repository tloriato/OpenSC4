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

//! `cSC4LandValueSimulator`: tract and altitude predicates (006bd5c0 TractHasLand, 006bd640 AddDesirability, 006bd750 SetupAltitudeEffect).

use super::*;

/// `cISTETerrain::MaxAllowedHeight()` and `IsLandVertex(x, z)`.
const TERRAIN_MAX_HEIGHT: usize = 0x60;
const TERRAIN_IS_LAND: usize = 0x90;

// Keep the stored operand as a memory operand: loading it separately would quiet a signaling NaN before
// the arithmetic chooses its result payload. Reverse subtraction also keeps the register operand's role.
macro_rules! stored_float_op {
    ($name:ident, $instruction:literal) => {
        fn $name(value: F80, operand: f32) -> F80 {
            let input = value.to_bytes();
            let mut output = [0u8; 10];
            // SAFETY: the pointers cover their operands; one register is pushed and popped.
            unsafe {
                core::arch::asm!(
                    "fld tbyte ptr [{input}]", $instruction, "fstp tbyte ptr [{output}]",
                    input = in(reg) input.as_ptr(), operand = in(reg) &operand,
                    output = in(reg) output.as_mut_ptr(), out("st(0)") _,
                );
            }
            F80::from_bytes(output)
        }
    };
}

stored_float_op!(subtract_float, "fsub dword ptr [{operand}]");
stored_float_op!(subtract_from_float, "fsubr dword ptr [{operand}]");
stored_float_op!(multiply_float, "fmul dword ptr [{operand}]");

layout! {
    /// The two words passed as context to the desirability callback.
    struct DesirabilityContext {
        0x00 => weight / set_weight: f32,
        0x04 => curve / set_curve: *mut u8,
    }
}

port! {
    /// Visits the tract's vertices, including its far edges, x outer and z inner, stopping at the first land
    /// vertex. The grid pointer is read before each geometry call and the terrain pointer before each vertex.
    /// Coordinates wrap at 32 bits and the shift is modulo 32. AL is the answer; the other EAX bytes come from
    /// the successful terrain result, or from the inclusive upper z bound when no vertex succeeds.
    #[port(address = 0x006B_D5C0, owner = "landvalue", name = "cSC4LandValueSimulator::TractHasLand")]
    pub unsafe extern "thiscall" fn fc4p_006bd5c0(this: *mut u8, tx: i32, tz: i32) -> u32 {
        // SAFETY: the game supplies its simulator and the live services it holds.
        let lv = unsafe { LandValueSim::from_ptr(this) };
        let shift = unsafe { vcall!(lv.intrinsic_grid(), GRID_TRACT_SHIFT => fn() -> u32) };
        let size = unsafe { vcall!(lv.intrinsic_grid(), GRID_TRACT_SIZE => fn() -> i32) };
        let (x0, z0) = (tx.wrapping_shl(shift), tz.wrapping_shl(shift));
        let (x1, z1) = (x0.wrapping_add(size), z0.wrapping_add(size));
        let mut x = x0;
        while x <= x1 {
            let mut z = z0;
            while z <= z1 {
                let land = unsafe { vcall!(lv.terrain(), TERRAIN_IS_LAND => fn(i32, i32) -> u32, x, z) };
                if land as u8 != 0 {
                    return (land & !0xFF) | 1;
                }
                z = z.wrapping_add(1);
            }
            x = x.wrapping_add(1);
        }
        z1 as u32 & !0xFF
    }
}

port! {
    /// Cdecl callback `(value: *mut u8, desirability: i16, context: *mut u8)`. The entry passes the original
    /// stack to the Rust body because the callback also overwrites the caller's third argument word.
    /// EDX:EAX is the float-to-integer result with its low half clamped to 1..=255; EDX keeps the high half.
    #[port(address = 0x006B_D640, owner = "landvalue", name = "AddDesirability", naked)]
    pub unsafe extern "C" fn fc4p_006bd640() {
        core::arch::naked_asm!(
            "mov ecx, esp",
            "jmp {body}",
            body = sym add_desirability,
        )
    }
}

/// The register argument points at the callback's return address and three caller-owned argument words.
/// With no stack arguments of its own, this body returns without popping the cdecl arguments.
unsafe extern "thiscall" fn add_desirability(stack: *mut u32) -> u64 {
    // SAFETY: the entry supplies its original stack. Capture the context before overwriting that word.
    let context = unsafe { DesirabilityContext::from_ptr(stack.add(3).read() as *mut u8) };
    let curve = context.curve();
    let d = unsafe { stack.add(2).cast::<i16>().read_unaligned() };
    unsafe { stack.add(3).write(i32::from(d) as u32) };
    let effect = if curve.is_null() {
        F80::from_i32(i32::from(d))
    } else {
        let input = F80::from_i32(i32::from(d)).to_f32();
        // SAFETY: the context's live response curve. Its output remains unrounded.
        unsafe { game::response_curve_output(curve, input) }
    };
    let weighted = multiply_float(effect, context.weight());
    let value = unsafe { stack.add(1).read() as *mut u8 };
    let previous = unsafe { value.read() };
    unsafe { stack.add(3).write(u32::from(previous)) };
    let converted = (weighted + F80::from_i32(i32::from(previous))).ftol();
    let result = (converted as i32).clamp(1, 255) as u32;
    unsafe { value.write(result as u8) };
    (converted as u64 & 0xFFFF_FFFF_0000_0000) | u64::from(result)
}

port! {
    /// Sets the altitude pivot to mode when mode < mean, otherwise mean. Stores the two ranges, clamping
    /// ordered values below 1 to 1. For a stored high range above 100, calls the terrain's maximum height and
    /// sets topIndex to the low 32 bits of trunc((highRange - 100) / (maximum - 100) * 100 + topIndex), capped
    /// at 150. topIndex starts at 100 and is read again after the terrain call. The numerator is stored as
    /// f32 before that call; the returned height and subsequent arithmetic retain x87 register precision.
    #[port(address = 0x006B_D750, owner = "landvalue", name = "cSC4LandValueSimulator::SetupAltitudeEffect")]
    pub unsafe extern "thiscall" fn fc4p_006bd750(this: *mut u8) {
        // SAFETY: the game's simulator and terrain service.
        let lv = unsafe { LandValueSim::from_ptr(this) };
        let mode = F80::from_f32(lv.mode_altitude());
        let mean = F80::from_f32(lv.mean_altitude());
        let pivot = if mode.compare(mean).less() { mode } else { mean };
        lv.set_pivot(pivot.to_f32());
        let low = subtract_float(pivot, lv.min_altitude());
        lv.set_low_range(low.to_f32());
        if low.compare(F80::from_f32(1.0)).less() {
            lv.set_low_range(1.0);
        }
        let high = subtract_from_float(pivot, lv.max_altitude());
        lv.set_high_range(high.to_f32());
        if high.compare(F80::from_f32(1.0)).less() {
            lv.set_high_range(1.0);
        }
        let hundred = F80::from_f32(100.0);
        let high = F80::from_f32(lv.high_range());
        lv.set_top_index(100);
        let cmp = high.compare(hundred);
        if !cmp.less() && !cmp.equal() && !cmp.unordered() {
            let numerator = (F80::from_f32(lv.high_range()) - hundred).to_f32();
            let maximum = unsafe { vcall_st0!(lv.terrain(), TERRAIN_MAX_HEIGHT) };
            let denominator = maximum - hundred;
            let result = (F80::from_f32(numerator) / denominator * hundred
                + F80::from_i32(lv.top_index())).ftol() as i32;
            lv.set_top_index(result);
            if result > 150 {
                lv.set_top_index(150);
            }
        }
    }
}

/// This file's replacements.
pub const PORTS: &[Port] = &[fc4p_006bd5c0::PORT, fc4p_006bd640::PORT, fc4p_006bd750::PORT];

#[cfg(test)]
mod tests {
    // The predicate and the altitude/desirability cases from the land-value core's conformance vectors,
    // exercised through the native entries with objects at their original offsets.
    use super::*;
    use crate::testing::{fake, GameFpu, Object};
    use std::cell::RefCell;

    #[derive(Debug, PartialEq)]
    enum Call {
        Shift(usize),
        Size(usize),
        Land(usize, i32, i32),
        Maximum(usize, [u32; 4]),
        Curve(usize, u32),
    }

    thread_local! {
        static CALLS: RefCell<Vec<Call>> = const { RefCell::new(Vec::new()) };
    }

    fn log(call: Call) {
        CALLS.with(|c| c.borrow_mut().push(call));
    }

    fn calls() -> Vec<Call> {
        CALLS.with(|c| std::mem::take(&mut *c.borrow_mut()))
    }

    fn vtable(slots: &[(usize, usize)]) -> Vec<usize> {
        let mut table = vec![0; slots.iter().map(|s| s.0).max().unwrap() / 4 + 1];
        for &(offset, function) in slots {
            table[offset / 4] = function;
        }
        table
    }

    #[repr(C)]
    struct Grid {
        vtable: *const usize,
        shift: u32,
        size: i32,
        swap: Option<(*mut u8, *mut u8)>,
    }

    extern "thiscall" fn shift(this: *mut Grid) -> u32 {
        log(Call::Shift(this as usize));
        // SAFETY: the fixture owns the grid and simulator.
        let g = unsafe { &*this };
        if let Some((lv, next)) = g.swap {
            unsafe { LandValueSim::from_ptr(lv) }.set_intrinsic_grid(next);
        }
        g.shift
    }

    extern "thiscall" fn size(this: *mut Grid) -> i32 {
        log(Call::Size(this as usize));
        // SAFETY: the fixture's grid.
        unsafe { (*this).size }
    }

    #[repr(C)]
    struct Terrain {
        vtable: *const usize,
        land: Vec<(i32, i32, u32)>,
        swap: Option<(*mut u8, *mut u8)>,
        maximum: f32,
        lv: *mut u8,
        replace_top: Option<i32>,
    }

    extern "thiscall" fn land(this: *mut Terrain, x: i32, z: i32) -> u32 {
        log(Call::Land(this as usize, x, z));
        // SAFETY: the fixture owns the terrain and simulator.
        let t = unsafe { &*this };
        if let Some((lv, next)) = t.swap {
            unsafe { LandValueSim::from_ptr(lv) }.set_terrain(next);
        }
        t.land.iter().find(|p| (p.0, p.1) == (x, z)).map_or(0, |p| p.2)
    }

    extern "thiscall" fn maximum(this: *mut Terrain) -> f32 {
        // SAFETY: the fixture's terrain and simulator.
        let t = unsafe { &*this };
        let lv = unsafe { LandValueSim::from_ptr(t.lv) };
        log(Call::Maximum(this as usize, parameters(lv)));
        if let Some(top) = t.replace_top {
            lv.set_top_index(top);
        }
        t.maximum
    }

    struct World {
        lv: Object,
        grid: Box<Grid>,
        terrain: Box<Terrain>,
        grid_vt: Vec<usize>,
        terrain_vt: Vec<usize>,
    }

    impl World {
        fn new(tract_shift: u32, tract_size: i32, vertices: &[(i32, i32, u32)]) -> Self {
            let grid_vt = vtable(&[
                (GRID_TRACT_SHIFT, shift as *const () as usize),
                (GRID_TRACT_SIZE, size as *const () as usize),
            ]);
            let terrain_vt = vtable(&[
                (TERRAIN_IS_LAND, land as *const () as usize),
                (TERRAIN_MAX_HEIGHT, maximum as *const () as usize),
            ]);
            let mut w = Self {
                lv: Object::new(0x100),
                grid: Box::new(Grid {
                    vtable: grid_vt.as_ptr(),
                    shift: tract_shift,
                    size: tract_size,
                    swap: None,
                }),
                terrain: Box::new(Terrain {
                    vtable: terrain_vt.as_ptr(),
                    land: vertices.to_vec(),
                    swap: None,
                    maximum: 2500.0,
                    lv: core::ptr::null_mut(),
                    replace_top: None,
                }),
                grid_vt,
                terrain_vt,
            };
            w.terrain.lv = w.lv.ptr();
            let g = (&mut *w.grid as *mut Grid).cast();
            let t = (&mut *w.terrain as *mut Terrain).cast();
            w.sim().set_intrinsic_grid(g);
            w.sim().set_terrain(t);
            calls();
            w
        }

        fn sim(&mut self) -> LandValueSim {
            // SAFETY: the fixture owns 0x100 readable and writable bytes.
            unsafe { LandValueSim::from_ptr(self.lv.ptr()) }
        }

        fn has_land(&mut self, x: i32, z: i32) -> u32 {
            // SAFETY: the fixture owns every object the entry accesses.
            unsafe { fc4p_006bd5c0(self.lv.ptr(), x, z) }
        }

        fn setup(&mut self, mode: f32, mean: f32, min: f32, max: f32) -> [u32; 4] {
            let lv = self.sim();
            lv.set_mode_altitude(mode);
            lv.set_mean_altitude(mean);
            lv.set_min_altitude(min);
            lv.set_max_altitude(max);
            // SAFETY: the fixture's simulator and terrain.
            unsafe { fc4p_006bd750(self.lv.ptr()) };
            parameters(lv)
        }
    }

    fn parameters(lv: LandValueSim) -> [u32; 4] {
        [
            lv.pivot().to_bits(),
            lv.low_range().to_bits(),
            lv.high_range().to_bits(),
            lv.top_index() as u32,
        ]
    }

    fn params(pivot: f32, low: f32, high: f32, top: i32) -> [u32; 4] {
        [pivot.to_bits(), low.to_bits(), high.to_bits(), top as u32]
    }

    #[test]
    fn land_includes_both_far_edges_and_scans_x_outer() {
        let mut w = World::new(1, 2, &[(8, 10, 1)]);
        let (g, t) = (&*w.grid as *const Grid as usize, &*w.terrain as *const Terrain as usize);
        assert_eq!(w.has_land(3, 4), 1);
        let mut expected = vec![Call::Shift(g), Call::Size(g)];
        for x in 6..=8 {
            for z in 8..=10 {
                expected.push(Call::Land(t, x, z));
            }
        }
        assert_eq!(calls(), expected);
    }

    #[test]
    fn land_stops_at_first_nonzero_low_byte_and_keeps_upper_bytes() {
        let mut w = World::new(1, 2, &[(0, 0, 0x1234_5600), (0, 1, 0xAABB_CC80)]);
        let (g, t) = (&*w.grid as *const Grid as usize, &*w.terrain as *const Terrain as usize);
        assert_eq!(w.has_land(0, 0), 0xAABB_CC01);
        assert_eq!(
            calls(),
            [Call::Shift(g), Call::Size(g), Call::Land(t, 0, 0), Call::Land(t, 0, 1)]
        );
    }

    #[test]
    fn water_only_tract_keeps_upper_z_bound_in_eax() {
        let mut w = World::new(2, 4, &[]);
        assert_eq!(w.has_land(1, 200), 0x300);
        assert_eq!(calls().len(), 27);
        // Size zero still tests the one vertex at the tract origin.
        w.grid.size = 0;
        assert_eq!(w.has_land(1, 201), 0x300);
        assert_eq!(calls().len(), 3);
    }

    #[test]
    fn land_coordinates_are_signed_shifted_and_wrapping() {
        let mut w = World::new(33, 0, &[(i32::MIN, -2, 1)]);
        assert_eq!(w.has_land(0x4000_0000, -1), 1);
        let t = &*w.terrain as *const Terrain as usize;
        assert_eq!(calls()[2..], [Call::Land(t, i32::MIN, -2)]);
        w.grid.shift = 0;
        w.grid.size = 2;
        assert_eq!(w.has_land(i32::MAX, 0), 0);
        assert_eq!(calls().len(), 2);
        assert_eq!(w.has_land(0, i32::MAX), 0x8000_0000);
        assert_eq!(calls().len(), 2);
        w.grid.size = -1;
        assert_eq!(w.has_land(0, 0), 0xFFFF_FF00);
        assert_eq!(calls().len(), 2);
    }

    #[test]
    fn land_reloads_grid_and_terrain_pointers_after_calls() {
        let mut w = World::new(1, 0, &[]);
        let mut next_grid = Box::new(Grid {
            vtable: w.grid_vt.as_ptr(),
            shift: 9,
            size: 1,
            swap: None,
        });
        let mut next_terrain = Box::new(Terrain {
            vtable: w.terrain_vt.as_ptr(),
            land: vec![(0, 1, 0x5502)],
            swap: None,
            maximum: 0.0,
            lv: w.lv.ptr(),
            replace_top: None,
        });
        let (ng, nt) = (&mut *next_grid as *mut Grid, &mut *next_terrain as *mut Terrain);
        w.grid.swap = Some((w.lv.ptr(), ng.cast()));
        w.terrain.swap = Some((w.lv.ptr(), nt.cast()));
        let (g, t) = (&*w.grid as *const Grid as usize, &*w.terrain as *const Terrain as usize);
        assert_eq!(w.has_land(0, 0), 0x5501);
        assert_eq!(
            calls(),
            [
                Call::Shift(g),
                Call::Size(ng as usize),
                Call::Land(t, 0, 0),
                Call::Land(nt as usize, 0, 1)
            ]
        );
    }

    #[repr(C)]
    struct Curve {
        output: f32,
        identity: bool,
        mutate: Option<(*mut u8, *mut u8)>,
    }

    extern "thiscall" fn curve(this: *mut Curve, x: f32) -> f32 {
        log(Call::Curve(this as usize, x.to_bits()));
        // SAFETY: the fixture's curve, value byte and callback context.
        let c = unsafe { &*this };
        if let Some((context, value)) = c.mutate {
            unsafe { DesirabilityContext::from_ptr(context) }.set_weight(0.5);
            unsafe { value.write(20) };
        }
        if c.identity {
            x.clamp(-255.0, 255.0)
        } else {
            c.output
        }
    }

    /// Calls the cdecl entry and retains the caller's overwritten argument word before releasing the arguments.
    fn desirability(value: *mut u8, d: u32, context: *mut u8) -> (u32, u32, u32) {
        let mut frame = [
            fc4p_006bd640 as *const () as u32,
            value as u32,
            d,
            context as u32,
            0,
            0,
            0,
        ];
        // SAFETY: the entry expects three cdecl argument words. EDI is callee-saved; all arguments and the
        // result slots are in `frame`. The entry leaves the arguments for this caller to pop.
        unsafe {
            core::arch::asm!(
                "push dword ptr [edi + 12]", "push dword ptr [edi + 8]", "push dword ptr [edi + 4]",
                "call dword ptr [edi]",
                "mov dword ptr [edi + 16], eax", "mov dword ptr [edi + 20], edx",
                "mov ecx, dword ptr [esp + 8]", "mov dword ptr [edi + 24], ecx", "add esp, 12",
                in("edi") frame.as_mut_ptr(), clobber_abi("C"),
            );
        }
        (frame[4], frame[5], frame[6])
    }

    fn apply(v: u8, d: u32, weight: f32, response: Option<&mut Curve>) -> ([u8; 3], (u32, u32, u32)) {
        let mut context = Object::new(8);
        // SAFETY: the fixture's eight-byte context.
        let ctx = unsafe { DesirabilityContext::from_ptr(context.ptr()) };
        ctx.set_weight(weight);
        ctx.set_curve(response.map_or(core::ptr::null_mut(), |c| (c as *mut Curve).cast()));
        let mut value = [0xA5, v, 0x5A];
        let result = desirability(value[1..].as_mut_ptr(), d, context.ptr());
        (value, result)
    }

    #[test]
    fn shipped_desirability_curve_and_raw_cases() {
        let _fpu = GameFpu::new();
        let _fake = fake(0x005F_4250, curve as *const () as usize);
        let mut response = Curve {
            output: 0.0,
            identity: true,
            mutate: None,
        };
        let c = &response as *const Curve as usize;
        for (v, d, expected) in [
            (100, 40, 200),
            (200, 40, 255),
            (100, -50, 1),
            (10, -3, 2),
            (1, 0, 1),
            (0, 0, 1),
            (100, 400, 255),
            (100, -400, 1),
            (0, 1, 2),
        ] {
            let (bytes, (eax, edx, slot)) = apply(v, d as u32, 2.5, Some(&mut response));
            assert_eq!(bytes, [0xA5, expected, 0x5A]);
            assert_eq!(eax, u32::from(expected));
            assert_eq!(slot, u32::from(v));
            assert_eq!(edx, if d < -40 { u32::MAX } else { 0 });
            assert_eq!(calls(), [Call::Curve(c, f32::from(d as i16).to_bits())]);
        }
        for (v, d, expected) in [(100, 400, 255), (100, -60, 70), (7, -1, 6)] {
            let (bytes, result) = apply(v, d as u32, 0.5, None);
            assert_eq!(bytes, [0xA5, expected, 0x5A]);
            assert_eq!(result, (u32::from(expected), 0, u32::from(v)));
            assert!(calls().is_empty());
        }
    }

    #[test]
    fn desirability_ignores_upper_argument_bits_and_sign_extends_the_low_word() {
        let _fpu = GameFpu::new();
        assert_eq!(apply(100, 0xABCD_FFFF, 0.5, None).0, [0xA5, 99, 0x5A]);
        assert_eq!(apply(100, 0xABCD_8000, 1.0, None).1, (1, u32::MAX, 100));
        assert_eq!(apply(100, 0xABCD_7FFF, 1.0, None).1, (255, 0, 100));
        assert!(calls().is_empty());
    }

    #[test]
    fn desirability_clamps_the_signed_low_half_and_preserves_the_high_half() {
        let _fpu = GameFpu::new();
        for (weight, expected) in [
            (3e9, (1, 0, 0)),
            (5e9, (255, 1, 0)),
            (-5e9, (1, 0xFFFF_FFFE, 0)),
            (f32::INFINITY, (1, 0x8000_0000, 0)),
            (f32::NAN, (1, 0x8000_0000, 0)),
        ] {
            assert_eq!(apply(0, 1, weight, None).1, expected);
        }
        // The product exceeds the f32 exponent range. The helper still accepts the x87 value and returns
        // integer indefinite, rather than saturating to the signed integer limits.
        assert_eq!(apply(0, 32767, f32::MAX, None).1, (1, 0x8000_0000, 0));
    }

    #[test]
    fn desirability_reads_weight_and_value_after_the_curve_call() {
        let _fpu = GameFpu::new();
        let _fake = fake(0x005F_4250, curve as *const () as usize);
        let mut context = Object::new(8);
        let mut value = 100u8;
        let mut response = Curve {
            output: 8.0,
            identity: false,
            mutate: Some((context.ptr(), &mut value)),
        };
        // SAFETY: the fixture's eight-byte context.
        let ctx = unsafe { DesirabilityContext::from_ptr(context.ptr()) };
        ctx.set_weight(10.0);
        ctx.set_curve((&mut response as *mut Curve).cast());
        assert_eq!(desirability(&mut value, 9, context.ptr()), (24, 0, 20));
        assert_eq!(value, 24);
        assert_eq!(
            calls(),
            [Call::Curve(&response as *const Curve as usize, 9.0f32.to_bits())]
        );
    }

    extern "thiscall" fn wide_curve(_this: *mut u8, x: f32) -> f64 {
        log(Call::Curve(_this as usize, x.to_bits()));
        0.333_333_325
    }

    extern "thiscall" fn wide_maximum(this: *mut Terrain) -> f64 {
        // Record the same pre-call fields as the ordinary maximum-height fake.
        maximum(this);
        100.000_000_953_674_32
    }

    #[test]
    fn desirability_uses_the_unrounded_st0_curve_result() {
        let _fpu = GameFpu::new();
        let _fake = fake(0x005F_4250, wide_curve as *const () as usize);
        let mut response = Curve {
            output: 0.0,
            identity: false,
            mutate: None,
        };
        assert_eq!(apply(0, 9, 6.0, Some(&mut response)).1, (2, 0, 0));
        assert_eq!(
            calls(),
            [Call::Curve(&response as *const Curve as usize, 9.0f32.to_bits())]
        );
        // A premature f32 store gives 0.333333313465... and then 1.99999988079..., which truncates to 1.
    }

    #[test]
    fn altitude_uses_the_unrounded_st0_maximum_height() {
        let _fpu = GameFpu::new();
        let mut w = World::new(0, 0, &[]);
        w.terrain_vt[TERRAIN_MAX_HEIGHT / 4] = wide_maximum as *const () as usize;
        assert_eq!(w.setup(0.0, 0.0, 0.0, 101.0), params(0.0, 1.0, 101.0, 150));
        assert_eq!(
            calls(),
            [Call::Maximum(
                &*w.terrain as *const Terrain as usize,
                params(0.0, 1.0, 101.0, 100)
            )]
        );
        // Rounding the returned maximum to f32 would make it 100: the zero denominator gives topIndex 0.
    }

    #[test]
    fn shipped_altitude_setup_cases_and_call_conditions() {
        let _fpu = GameFpu::new();
        let mut w = World::new(1, 2, &[]);
        for (input, expected, needs_maximum) in [
            ([272.0, 300.0, 260.0, 322.0], params(272.0, 12.0, 50.0, 100), false),
            ([0.0, 0.0, 0.0, 100.0], params(0.0, 1.0, 100.0, 100), false),
            ([0.0, 0.0, 0.0, 340.0], params(0.0, 1.0, 340.0, 110), true),
            ([0.0, 0.0, 0.0, 3000.0], params(0.0, 1.0, 3000.0, 150), true),
            ([300.0; 4], params(300.0, 1.0, 1.0, 100), false),
            ([300.0, 272.0, 260.0, 322.0], params(272.0, 12.0, 50.0, 100), false),
        ] {
            assert_eq!(w.setup(input[0], input[1], input[2], input[3]), expected);
            if needs_maximum {
                assert_eq!(
                    calls(),
                    [Call::Maximum(
                        &*w.terrain as *const Terrain as usize,
                        [expected[0], expected[1], expected[2], 100]
                    )]
                );
            } else {
                assert!(calls().is_empty());
            }
        }
    }

    #[test]
    fn altitude_equal_unordered_and_subunit_ranges() {
        let _fpu = GameFpu::new();
        let mut w = World::new(0, 0, &[]);
        // Equal pivot candidates select the mean, including the sign of its zero.
        assert_eq!(w.setup(0.0, -0.0, -1.0, 1.0), params(-0.0, 1.0, 1.0, 100));
        assert_eq!(w.setup(0.0, 0.0, 1.0, -1.0), params(0.0, 1.0, 1.0, 100));
        assert_eq!(w.setup(0.0, 0.0, -0.5, 0.5), params(0.0, 1.0, 1.0, 100));
        assert_eq!(w.setup(f32::NAN, 2.0, 0.0, 3.0), params(2.0, 2.0, 1.0, 100));
        w.setup(2.0, f32::NAN, 0.0, 3.0);
        let lv = w.sim();
        assert!(lv.pivot().is_nan());
        assert!(lv.low_range().is_nan());
        assert!(lv.high_range().is_nan());
        assert_eq!(lv.top_index(), 100);
        w.setup(0.0, 0.0, f32::NAN, f32::NAN);
        assert!(w.sim().low_range().is_nan());
        assert!(w.sim().high_range().is_nan());
        assert!(calls().is_empty());
    }

    #[test]
    fn altitude_keeps_the_quiet_nan_payload_with_signaling_range_inputs() {
        let _fpu = GameFpu::new();
        let mut w = World::new(0, 0, &[]);
        let pivot = f32::from_bits(0x7FC0_0100);
        let signaling = f32::from_bits(0x7F80_0200);
        assert_eq!(
            w.setup(0.0, pivot, signaling, signaling),
            [0x7FC0_0100, 0x7FC0_0100, 0x7FC0_0100, 100]
        );
        assert!(calls().is_empty());
    }

    #[test]
    fn altitude_top_index_uses_the_helper_and_signed_low_half() {
        let _fpu = GameFpu::new();
        let mut w = World::new(0, 0, &[]);
        for (maximum, expected) in [
            (200.0, 150),
            (50.0, -100),
            (100.0, 0),
            (f32::INFINITY, 100),
            (f32::NAN, 0),
        ] {
            w.terrain.maximum = maximum;
            assert_eq!(w.setup(0.0, 0.0, 0.0, 200.0)[3], expected as u32);
            assert_eq!(calls().len(), 1);
        }
        w.terrain.maximum = 200.0;
        w.terrain.replace_top = Some(i32::MAX);
        // Adding 100 to exactly loaded i32::MAX at 24-bit precision gives 2^31, whose signed low half is MIN.
        assert_eq!(w.setup(0.0, 0.0, 0.0, 200.0)[3], i32::MIN as u32);
        assert_eq!(calls().len(), 1);
    }

    #[test]
    fn setup_only_writes_its_four_output_fields() {
        let _fpu = GameFpu::new();
        let mut w = World::new(0, 0, &[]);
        let lv = w.sim();
        lv.set_mode_altitude(272.0);
        lv.set_mean_altitude(300.0);
        lv.set_min_altitude(260.0);
        lv.set_max_altitude(322.0);
        let mut expected = unsafe { core::slice::from_raw_parts(w.lv.ptr(), 0x100) }.to_vec();
        for (i, word) in params(272.0, 12.0, 50.0, 100).into_iter().enumerate() {
            expected[0x60 + i * 4..0x64 + i * 4].copy_from_slice(&word.to_le_bytes());
        }
        unsafe { fc4p_006bd750(w.lv.ptr()) };
        assert_eq!(unsafe { core::slice::from_raw_parts(w.lv.ptr(), 0x100) }, expected);
        assert!(calls().is_empty());
    }

    fn float_environment() -> (u16, u16, u32, u32) {
        let (mut cw, mut mxcsr) = (0u16, 0u32);
        let (sw, flags): (u16, u32);
        // SAFETY: the output pointers cover their values and the temporary flags push is balanced.
        unsafe {
            core::arch::asm!(
                "fnstcw word ptr [{cw}]", "fnstsw ax", "stmxcsr dword ptr [{mxcsr}]",
                "pushfd", "pop {flags:e}",
                cw = in(reg) &mut cw, mxcsr = in(reg) &mut mxcsr,
                flags = lateout(reg) flags, out("ax") sw,
            );
        }
        (cw, sw & 0x3800, mxcsr, flags & 0x400)
    }

    #[test]
    fn ports_preserve_the_float_environment_and_an_existing_x87_value() {
        let _fpu = GameFpu::new();
        let mut w = World::new(0, 0, &[(0, 0, 1)]);
        let sentinel = 123.25f64;
        let mut restored = 0.0f64;
        // SAFETY: the stack has room; the value is popped below before the test returns.
        unsafe {
            core::arch::asm!("fld qword ptr [{p}]", p = in(reg) &sentinel, out("st(0)") _);
        }
        let before = float_environment();
        let land_result = w.has_land(0, 0);
        let after_land = float_environment();
        let desirability_result = apply(100, 40, 2.5, None);
        let after_desirability = float_environment();
        let altitude_result = w.setup(0.0, 0.0, 0.0, 340.0);
        let after_altitude = float_environment();
        // SAFETY: pop the sentinel left in place by the three entries.
        unsafe {
            core::arch::asm!("fstp qword ptr [{p}]", p = in(reg) &mut restored, out("st(0)") _);
        }
        assert_eq!(land_result, 1);
        assert_eq!(desirability_result.1, (200, 0, 100));
        assert_eq!(altitude_result, params(0.0, 1.0, 340.0, 110));
        assert_eq!(after_land, before);
        assert_eq!(after_desirability, before);
        assert_eq!(after_altitude, before);
        assert_eq!(restored.to_bits(), sentinel.to_bits());
    }
}
