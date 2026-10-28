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

//! The game's floating point, done on the x87 itself.
//!
//! The game runs the x87 with control word 0x007F: every exception masked, 24-bit precision control, round to
//! nearest. Its float code keeps intermediate values in x87 registers: each operation rounds the significand to
//! 24 bits, but the exponent keeps the register's 15-bit range, and integer loads (`fild`) are exact. SSE `f32`
//! arithmetic differs from that at the edges (a large `int32` divided by a float, an intermediate beyond the
//! `f32` range that comes back into it), so converted code computes with [`F80`], one x87 register value, and
//! every operation runs on the x87 under the caller's control word. Loads and stores of an [`F80`] are exact
//! (`fld`/`fstp tbyte`), so a chain of [`F80`] operations gives the same bits as the same chain held in
//! registers.
//!
//! Float-to-integer conversion is [`F80::ftol`], which runs our port of the game's helper at 0x009EEF04
//! ([`fc4p_009eef04`]). The helper does not touch the control word: it rounds the value to a 64-bit integer under
//! the current rounding mode, subtracts that integer from the value, and moves the integer one step toward zero
//! when the difference (stored as an `f32`) shows it was rounded away from zero. The result, in EDX:EAX, is the
//! value truncated toward zero; code that wants an `int32` keeps the low 32 bits. NaN, infinities and values
//! outside the `i64` range give the x87 integer indefinite, 0x8000_0000_0000_0000, so the low 32 bits are 0.
//! Neither `cvttss2si` (0x8000_0000 out of range) nor Rust's `as` (saturating) does that. The port runs the same
//! x87 operations as the helper (the same loads, stores and subtraction on the same values), so it leaves the
//! same x87 status flags and condition codes.
//!
//! Every asm block here leaves the x87 register stack as it found it (empty, as Rust code keeps it). Only
//! [`set_control_word`] changes the control word.

use core::arch::asm;
use core::ops::{Add, Div, Mul, Neg, Sub};

/// Runs an asm block that uses the x87 register stack (and leaves it as it found it).
macro_rules! x87 {
    ($($body:tt)*) => {
        // SAFETY: the callers' asm reads and writes only the memory their pointer operands name, and pops
        // every register it pushes.
        unsafe {
            asm!(
                $($body)*
                out("st(0)") _, out("st(1)") _, out("st(2)") _, out("st(3)") _,
                out("st(4)") _, out("st(5)") _, out("st(6)") _, out("st(7)") _,
            )
        }
    };
}

/// The control word the game runs with (see the module docs).
pub const GAME_CONTROL_WORD: u16 = 0x007F;

/// One x87 register value as `fstp tbyte` stores it: 64-bit significand (with its explicit integer bit), then
/// the sign and the 15-bit exponent.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct F80 {
    bytes: [u8; 10],
}

/// The status word after an x87 comparison (`fcom` family): C0, C2 and C3 hold the result.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Compared(u16);

impl Compared {
    const C0: u16 = 1 << 8;
    const C2: u16 = 1 << 10;
    const C3: u16 = 1 << 14;

    /// The whole status word, as `fnstsw ax` stores it right after the comparison.
    pub fn status_word(self) -> u16 {
        self.0
    }
    /// Ordered and less: C0 set, C2 and C3 clear. False for unordered (NaN). This is the test behind the
    /// game's `fnstsw ax; test ah, 5; jp` idiom: the jump is not taken exactly when this is true.
    pub fn less(self) -> bool {
        self.0 & (Self::C0 | Self::C2 | Self::C3) == Self::C0
    }
    /// Ordered and equal: C3 set, C0 and C2 clear.
    pub fn equal(self) -> bool {
        self.0 & (Self::C0 | Self::C2 | Self::C3) == Self::C3
    }
    /// Unordered (at least one NaN): C0, C2 and C3 all set.
    pub fn unordered(self) -> bool {
        self.0 & (Self::C0 | Self::C2 | Self::C3) == (Self::C0 | Self::C2 | Self::C3)
    }
}

impl F80 {
    pub const ZERO: F80 = F80 { bytes: [0; 10] };

    /// From the stored bytes (`fstp tbyte` layout).
    pub const fn from_bytes(bytes: [u8; 10]) -> F80 {
        F80 { bytes }
    }
    pub const fn to_bytes(self) -> [u8; 10] {
        self.bytes
    }
    /// The 64-bit significand, explicit integer bit included.
    pub fn significand(self) -> u64 {
        let mut s = [0u8; 8];
        s.copy_from_slice(&self.bytes[..8]);
        u64::from_le_bytes(s)
    }
    /// Sign bit and 15-bit biased exponent.
    pub fn sign_exponent(self) -> u16 {
        u16::from_le_bytes([self.bytes[8], self.bytes[9]])
    }

    /// `fld dword`: exact.
    pub fn from_f32(v: f32) -> F80 {
        let mut o = F80::ZERO;
        x87!("fld dword ptr [{v}]", "fstp tbyte ptr [{o}]", v = in(reg) &v, o = in(reg) &mut o,);
        o
    }
    /// `fld qword`: exact.
    pub fn from_f64(v: f64) -> F80 {
        let mut o = F80::ZERO;
        x87!("fld qword ptr [{v}]", "fstp tbyte ptr [{o}]", v = in(reg) &v, o = in(reg) &mut o,);
        o
    }
    /// `fild dword`: exact.
    pub fn from_i32(v: i32) -> F80 {
        let mut o = F80::ZERO;
        x87!("fild dword ptr [{v}]", "fstp tbyte ptr [{o}]", v = in(reg) &v, o = in(reg) &mut o,);
        o
    }
    /// `fild qword`: exact.
    pub fn from_i64(v: i64) -> F80 {
        let mut o = F80::ZERO;
        x87!("fild qword ptr [{v}]", "fstp tbyte ptr [{o}]", v = in(reg) &v, o = in(reg) &mut o,);
        o
    }
    /// `fstp dword`: rounded to `f32` by the control word's rounding mode (overflow gives infinity, as the
    /// game's stores do with the exceptions masked).
    pub fn to_f32(self) -> f32 {
        let mut o = 0f32;
        x87!("fld tbyte ptr [{x}]", "fstp dword ptr [{o}]", x = in(reg) &self, o = in(reg) &mut o,);
        o
    }
    /// `fstp qword`: rounded to `f64` by the control word's rounding mode.
    pub fn to_f64(self) -> f64 {
        let mut o = 0f64;
        x87!("fld tbyte ptr [{x}]", "fstp qword ptr [{o}]", x = in(reg) &self, o = in(reg) &mut o,);
        o
    }

    /// `self <op> rhs` with `self` in ST0 and `rhs` in ST1 (`fadd st(0), st(1)` and so on).
    fn binary(self, rhs: F80, op: Op) -> F80 {
        let mut o = F80::ZERO;
        let (a, b, p) = (&self, &rhs, &mut o);
        match op {
            Op::Add => x87!("fld tbyte ptr [{b}]", "fld tbyte ptr [{a}]", "fadd st(0), st(1)", "fstp tbyte ptr [{o}]",
                "fstp st(0)", a = in(reg) a, b = in(reg) b, o = in(reg) p,),
            Op::Sub => x87!("fld tbyte ptr [{b}]", "fld tbyte ptr [{a}]", "fsub st(0), st(1)", "fstp tbyte ptr [{o}]",
                "fstp st(0)", a = in(reg) a, b = in(reg) b, o = in(reg) p,),
            Op::Mul => x87!("fld tbyte ptr [{b}]", "fld tbyte ptr [{a}]", "fmul st(0), st(1)", "fstp tbyte ptr [{o}]",
                "fstp st(0)", a = in(reg) a, b = in(reg) b, o = in(reg) p,),
            Op::Div => x87!("fld tbyte ptr [{b}]", "fld tbyte ptr [{a}]", "fdiv st(0), st(1)", "fstp tbyte ptr [{o}]",
                "fstp st(0)", a = in(reg) a, b = in(reg) b, o = in(reg) p,),
        }
        o
    }

    /// Compares `self` (ST0) with `rhs` (`fcompp`, which also raises the invalid flag for any NaN, like `fcom`).
    pub fn compare(self, rhs: F80) -> Compared {
        let sw: u16;
        x87!("fld tbyte ptr [{b}]", "fld tbyte ptr [{a}]", "fcompp", "fnstsw ax", a = in(reg) &self,
            b = in(reg) &rhs, out("ax") sw,);
        Compared(sw)
    }

    /// The game's float-to-integer helper (0x009EEF04), run as our port of it ([`fc4p_009eef04`]) on this value:
    /// truncation toward zero to 64 bits, whatever the rounding mode; the x87 integer indefinite
    /// (0x8000_0000_0000_0000) for NaN, infinities and values outside the `i64` range. Callers that want an
    /// `int32` keep the low 32 bits (`as i32`). The x87 flags it raises are the helper's (see the module docs).
    pub fn ftol(self) -> i64 {
        let (lo, hi): (u32, u32);
        // SAFETY: the helper takes ST0 (popped) and returns in EDX:EAX; it changes ECX and the flags and uses
        // 48 bytes of stack below ESP. The register stack is empty again afterwards.
        unsafe {
            asm!(
                "fld tbyte ptr [{x}]",
                "call {helper}",
                x = in(reg) &self,
                helper = sym fc4p_009eef04,
                lateout("eax") lo, lateout("edx") hi, lateout("ecx") _,
                out("st(0)") _, out("st(1)") _, out("st(2)") _, out("st(3)") _,
                out("st(4)") _, out("st(5)") _, out("st(6)") _, out("st(7)") _,
            );
        }
        ((u64::from(hi) << 32) | u64::from(lo)) as i64
    }

    /// `fsqrt` (rounded by the precision control, like the arithmetic).
    pub fn sqrt(self) -> F80 {
        self.unary(Unary::Sqrt)
    }
    /// `fsin` (ST0 unchanged when |x| >= 2^63, as the instruction leaves it).
    pub fn sin(self) -> F80 {
        self.unary(Unary::Sin)
    }
    /// `fcos`.
    pub fn cos(self) -> F80 {
        self.unary(Unary::Cos)
    }
    /// `fptan` (the 1.0 it pushes is dropped).
    pub fn tan(self) -> F80 {
        self.unary(Unary::Tan)
    }
    /// `f2xm1`: 2^x − 1 for |x| <= 1.
    pub fn exp2_m1(self) -> F80 {
        self.unary(Unary::Exp2M1)
    }
    /// `frndint`: rounded to an integer by the control word's rounding mode.
    pub fn round_int(self) -> F80 {
        self.unary(Unary::RoundInt)
    }
    /// `fabs`.
    pub fn abs(self) -> F80 {
        self.unary(Unary::Abs)
    }

    fn unary(self, op: Unary) -> F80 {
        let mut o = F80::ZERO;
        let (a, p) = (&self, &mut o);
        match op {
            Unary::Sqrt => x87!("fld tbyte ptr [{a}]", "fsqrt", "fstp tbyte ptr [{o}]", a = in(reg) a, o = in(reg) p,),
            Unary::Sin => x87!("fld tbyte ptr [{a}]", "fsin", "fstp tbyte ptr [{o}]", a = in(reg) a, o = in(reg) p,),
            Unary::Cos => x87!("fld tbyte ptr [{a}]", "fcos", "fstp tbyte ptr [{o}]", a = in(reg) a, o = in(reg) p,),
            Unary::Tan => x87!("fld tbyte ptr [{a}]", "fptan", "fstp st(0)", "fstp tbyte ptr [{o}]", a = in(reg) a,
                o = in(reg) p,),
            Unary::Exp2M1 => x87!("fld tbyte ptr [{a}]", "f2xm1", "fstp tbyte ptr [{o}]", a = in(reg) a,
                o = in(reg) p,),
            Unary::RoundInt => x87!("fld tbyte ptr [{a}]", "frndint", "fstp tbyte ptr [{o}]", a = in(reg) a,
                o = in(reg) p,),
            Unary::Abs => x87!("fld tbyte ptr [{a}]", "fabs", "fstp tbyte ptr [{o}]", a = in(reg) a, o = in(reg) p,),
            Unary::Neg => x87!("fld tbyte ptr [{a}]", "fchs", "fstp tbyte ptr [{o}]", a = in(reg) a, o = in(reg) p,),
        }
        o
    }

    /// `fpatan` with `self` = y in ST1 and `x` in ST0: atan(y / x) in the quadrant of (x, y).
    pub fn atan2(self, x: F80) -> F80 {
        let mut o = F80::ZERO;
        x87!("fld tbyte ptr [{y}]", "fld tbyte ptr [{x}]", "fpatan", "fstp tbyte ptr [{o}]", y = in(reg) &self,
            x = in(reg) &x, o = in(reg) &mut o,);
        o
    }
    /// `fyl2x` with `self` = y in ST1 and `x` in ST0: y · log2(x).
    pub fn mul_log2(self, x: F80) -> F80 {
        let mut o = F80::ZERO;
        x87!("fld tbyte ptr [{y}]", "fld tbyte ptr [{x}]", "fyl2x", "fstp tbyte ptr [{o}]", y = in(reg) &self,
            x = in(reg) &x, o = in(reg) &mut o,);
        o
    }
    /// `fscale` with `self` in ST0 and `n` in ST1: self · 2^trunc(n).
    pub fn scale(self, n: F80) -> F80 {
        let mut o = F80::ZERO;
        x87!("fld tbyte ptr [{n}]", "fld tbyte ptr [{a}]", "fscale", "fstp tbyte ptr [{o}]", "fstp st(0)",
            a = in(reg) &self, n = in(reg) &n, o = in(reg) &mut o,);
        o
    }
}

#[derive(Clone, Copy)]
enum Op {
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Clone, Copy)]
enum Unary {
    Sqrt,
    Sin,
    Cos,
    Tan,
    Exp2M1,
    RoundInt,
    Abs,
    Neg,
}

/// `fchs`.
impl Neg for F80 {
    type Output = F80;
    fn neg(self) -> F80 {
        self.unary(Unary::Neg)
    }
}

impl Add for F80 {
    type Output = F80;
    fn add(self, rhs: F80) -> F80 {
        self.binary(rhs, Op::Add)
    }
}
impl Sub for F80 {
    type Output = F80;
    fn sub(self, rhs: F80) -> F80 {
        self.binary(rhs, Op::Sub)
    }
}
impl Mul for F80 {
    type Output = F80;
    fn mul(self, rhs: F80) -> F80 {
        self.binary(rhs, Op::Mul)
    }
}
impl Div for F80 {
    type Output = F80;
    fn div(self, rhs: F80) -> F80 {
        self.binary(rhs, Op::Div)
    }
}

/// The x87 control word (`fnstcw`).
pub fn control_word() -> u16 {
    let mut cw = 0u16;
    x87!("fnstcw word ptr [{p}]", p = in(reg) &mut cw,);
    cw
}

/// Sets the x87 control word (`fldcw`). The game sets its own; converted code never calls this (tests do).
pub fn set_control_word(cw: u16) {
    x87!("fldcw word ptr [{p}]", p = in(reg) &cw,);
}

/// The x87 status word (`fnstsw`).
pub fn status_word() -> u16 {
    let sw: u16;
    x87!("fnstsw ax", out("ax") sw,);
    sw
}

// The helper itself, with its own register convention: the value in ST0 (popped), the result in EDX:EAX. Like the
// original it keeps EBX, ESI, EDI and EBP, leaves the control word alone, and changes ECX only when it corrects
// the rounded integer (ECX is then the correction's sum, as in the original). Its scratch area is a 16-byte
// aligned block below the entry ESP (dead after the return): +0 the difference as an f32, +16 the rounded
// integer, +24 the value as an f32. `F80::ftol` calls it.
port! {
    /// The float-to-integer helper: ST0 in (popped), EDX:EAX out, the value truncated toward zero to 64 bits.
    #[port(address = 0x009E_EF04, owner = "crt", name = "float to int64 helper (ftol)", naked)]
    pub unsafe extern "C" fn fc4p_009eef04() {
        core::arch::naked_asm!(
            "push ebp",
            "mov ebp, esp",
            "sub esp, 32",
            "and esp, -16",
            // r = the value rounded by the current mode; the value's f32 image keeps its sign.
            "fld st(0)",
            "fst dword ptr [esp + 24]",
            "fistp qword ptr [esp + 16]",
            "fild qword ptr [esp + 16]",
            "mov eax, dword ptr [esp + 16]",
            "mov edx, dword ptr [esp + 20]",
            // r = 0, or the integer indefinite (NaN, infinities, out of range; also exactly -2^63): r is the
            // answer as it stands.
            "test eax, eax",
            "jnz 2f",
            "test edx, 0x7FFFFFFF",
            "jz 5f",
            // |r| >= 2^32: r's sign is the value's.
            "mov ecx, edx",
            "jmp 3f",
            "2:",
            "mov ecx, dword ptr [esp + 24]",
            "3:",
            // The difference value - r, as an f32.
            "fsubp st(1), st(0)",
            "fstp dword ptr [esp]",
            "test ecx, ecx",
            "mov ecx, dword ptr [esp]",
            "jns 4f",
            // A negative value: one up when the difference is above +0 (r was rounded down).
            "xor ecx, 0x80000000",
            "add ecx, 0x7FFFFFFF",
            "adc eax, 0",
            "adc edx, 0",
            "leave",
            "ret",
            "4:",
            // A positive value: one down when the difference is below -0 (r was rounded up).
            "add ecx, 0x7FFFFFFF",
            "sbb eax, 0",
            "sbb edx, 0",
            "leave",
            "ret",
            // No correction: drop r and the value with the same stores the helper makes.
            "5:",
            "fstp dword ptr [esp + 24]",
            "fstp dword ptr [esp + 24]",
            "leave",
            "ret",
        )
    }
}

/// This module's replacements.
pub const PORTS: &[crate::port::Port] = &[fc4p_009eef04::PORT];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::GameFpu;

    const INDEFINITE: i64 = i64::MIN;

    /// Truncation to i64 with the x87's out-of-range answer, computed without the x87.
    fn reference(x: f64) -> i64 {
        if x.is_nan() || x >= 9_223_372_036_854_775_808.0 || x < -9_223_372_036_854_775_808.0 {
            INDEFINITE
        } else {
            x.trunc() as i64
        }
    }

    /// Calls the exported helper the way the game does: value in ST0, result in EDX:EAX. Also returns the x87
    /// stack top before and after (the helper pops its argument) and ECX after.
    fn call_export(x: f64) -> (i64, u16, u16, u32) {
        let (lo, hi, ecx): (u32, u32, u32);
        let before = status_word();
        // SAFETY: the export takes ST0 and returns in EDX:EAX, preserving the callee-saved registers.
        unsafe {
            asm!(
                "fld qword ptr [{x}]",
                "mov ecx, 0x12345678",
                "call {f}",
                x = in(reg) &x,
                f = sym fc4p_009eef04,
                lateout("eax") lo, lateout("edx") hi, lateout("ecx") ecx,
                out("st(0)") _, out("st(1)") _, out("st(2)") _, out("st(3)") _,
                out("st(4)") _, out("st(5)") _, out("st(6)") _, out("st(7)") _,
            );
        }
        let after = status_word();
        ((u64::from(hi) << 32 | u64::from(lo)) as i64, before, after, ecx)
    }

    #[test]
    fn arithmetic_operand_order() {
        let _fpu = GameFpu::new();
        let (five, three) = (F80::from_f32(5.0), F80::from_f32(3.0));
        assert_eq!((five - three).to_f32(), 2.0);
        assert_eq!((three - five).to_f32(), -2.0);
        assert_eq!((F80::from_f32(6.0) / three).to_f32(), 2.0);
        assert_eq!((three / F80::from_f32(6.0)).to_f32(), 0.5);
        assert_eq!((five + three).to_f32(), 8.0);
        assert_eq!((five * three).to_f32(), 15.0);
    }

    #[test]
    fn precision_control_rounds_to_24_bits() {
        let one = F80::from_f32(1.0);
        let tiny = F80::from_f64(2f64.powi(-30));
        {
            let _fpu = GameFpu::new();
            assert_eq!((one + tiny).to_f64(), 1.0, "24-bit precision drops 2^-30");
        }
        let saved = control_word();
        set_control_word(0x037F); // 64-bit precision
        let sum = (one + tiny).to_f64();
        set_control_word(saved);
        assert_eq!(sum, 1.0 + 2f64.powi(-30));
    }

    #[test]
    fn exponent_range_is_the_registers() {
        let _fpu = GameFpu::new();
        // 2^100 · 2^100 overflows f32 but not an x87 register; dividing back gives 2^100 again.
        let big = F80::from_f32(2f32.powi(100));
        assert_eq!((big * big / big).to_f32(), 2f32.powi(100));
        assert_eq!((big * big).to_f32(), f32::INFINITY);
        // 1e30 · 1e30 is rounded to 24 bits on the way: dividing back is one unit off.
        let big = F80::from_f32(1e30);
        assert_eq!((big * big / big).to_f32(), f32::from_bits(1e30f32.to_bits() - 1));
    }

    #[test]
    fn integer_loads_are_exact() {
        let _fpu = GameFpu::new();
        assert_eq!(F80::from_i32(i32::MAX).ftol(), i64::from(i32::MAX));
        assert_eq!(F80::from_i32(i32::MIN).ftol(), i64::from(i32::MIN));
        assert_eq!(F80::from_i64(i64::MAX - 1).ftol(), i64::MAX - 1);
    }

    #[test]
    fn comparisons() {
        let _fpu = GameFpu::new();
        let (a, b) = (F80::from_f32(1.0), F80::from_f32(2.0));
        assert!(a.compare(b).less());
        assert!(!b.compare(a).less());
        assert!(a.compare(a).equal());
        let nan = F80::from_f32(f32::NAN);
        assert!(nan.compare(a).unordered());
        assert!(!nan.compare(a).less());
        assert!(!a.compare(nan).less());
        // The stack top in the status word is the caller's (empty stack: 0).
        assert_eq!(a.compare(b).status_word() & 0x3800, 0);
    }

    #[test]
    fn ftol_boundaries() {
        let _fpu = GameFpu::new();
        let two63 = 9_223_372_036_854_775_808.0f64;
        let cases: &[f64] = &[
            0.0,
            -0.0,
            0.4,
            0.5,
            0.6,
            -0.5,
            -0.6,
            0.999_999,
            -0.999_999,
            1.0,
            -1.0,
            1.5,
            -1.5,
            2.5,
            -2.5,
            8_388_607.5,
            -8_388_607.5,
            2_147_483_647.0,
            2_147_483_647.5,
            2_147_483_648.0,
            -2_147_483_648.0,
            -2_147_483_649.0,
            3_000_000_000.0,
            -3_000_000_000.0,
            4_294_967_295.5,
            4_294_967_296.0,
            1e18,
            -1e18,
            two63 - 1024.0,
            -two63,
            two63,
            -two63 - 2048.0,
            1e19,
            -1e19,
            1e300,
            f64::MIN_POSITIVE,
            -f64::MIN_POSITIVE,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NAN,
            -f64::NAN,
        ];
        for &x in cases {
            let want = reference(x);
            assert_eq!(F80::from_f64(x).ftol(), want, "ftol({x:e})");
            assert_eq!(
                F80::from_f64(x).ftol() as i32,
                want as i32,
                "low 32 bits of ftol({x:e})"
            );
        }
        // Not saturation, not cvttss2si: 3e9 keeps its low 32 bits, 1e19 gives 0.
        assert_eq!(F80::from_f32(3e9).ftol() as i32, 3_000_000_000u32 as i32);
        assert_eq!(F80::from_f32(1e19).ftol() as i32, 0);
        assert_eq!(F80::from_f32(f32::NAN).ftol() as i32, 0);
    }

    #[test]
    fn ftol_truncates_under_every_rounding_mode() {
        let saved = control_word();
        for rc in [0u16, 1, 2, 3] {
            set_control_word(GAME_CONTROL_WORD | (rc << 10));
            for x in [2.7f64, -2.7, 0.5, -0.5, 1e10 + 0.75, -1e10 - 0.75] {
                assert_eq!(F80::from_f64(x).ftol(), reference(x), "rc {rc} x {x}");
            }
            assert_eq!(
                control_word(),
                GAME_CONTROL_WORD | (rc << 10),
                "ftol restores the control word"
            );
        }
        set_control_word(saved);
    }

    #[test]
    fn ftol_sweep_against_reference() {
        let _fpu = GameFpu::new();
        // Every float of a few binades around the int32 and int64 edges, by bit pattern.
        for base in [
            0x3F00_0000u32,
            0x4EFF_FF00,
            0x4F00_0000,
            0x4F7F_FF00,
            0x5EFF_FF00,
            0x5F00_0000,
        ] {
            for bits in base..base + 0x200 {
                for sign in [0u32, 0x8000_0000] {
                    let f = f32::from_bits(bits | sign);
                    assert_eq!(F80::from_f32(f).ftol(), reference(f64::from(f)), "ftol({f:e})");
                }
            }
        }
    }

    #[test]
    fn exported_helper_convention() {
        let _fpu = GameFpu::new();
        // (value, ECX after): ECX is kept when the rounded integer r is 0 or the integer indefinite; otherwise it
        // is the difference value - r as f32 bits (sign flipped for a negative value) plus 0x7FFF_FFFF.
        let flip = |d: f32| (d.to_bits() ^ 0x8000_0000).wrapping_add(0x7FFF_FFFF);
        let keep = |d: f32| d.to_bits().wrapping_add(0x7FFF_FFFF);
        let cases: &[(f64, u32)] = &[
            (0.0, 0x1234_5678),
            (0.4, 0x1234_5678),
            (-0.4, 0x1234_5678),
            (2.9, keep((2.9f64 - 3.0) as f32)),
            (-2.9, flip((-2.9f64 + 3.0) as f32)),
            (2.5, keep(0.5)),
            (-2.5, flip(-0.5)),
            (3e9, keep(0.0)),
            (-3e9, flip(0.0)),
            (1e19, 0x1234_5678),
            (f64::NAN, 0x1234_5678),
            (f64::NEG_INFINITY, 0x1234_5678),
            (-9_223_372_036_854_775_808.0, 0x1234_5678),
            // r = 2^32 (low word 0): the sign comes from r's high word.
            (4_294_967_296.5, keep(0.5)),
            (-4_294_967_296.5, flip(-0.5)),
        ];
        for &(x, want_ecx) in cases {
            let (got, before, after, ecx) = call_export(x);
            assert_eq!(got, reference(x), "helper({x:e})");
            assert_eq!(before & 0x3800, after & 0x3800, "the helper pops its argument");
            assert_eq!(ecx, want_ecx, "ECX after helper({x:e})");
        }
        assert_eq!(control_word(), GAME_CONTROL_WORD);
    }

    #[test]
    fn helper_rounds_by_the_mode_then_corrects() {
        // Under round-to-nearest 2.5 rounds to 2 and 3.5 to 4: inexact either way (PE, bit 5), and the
        // correction runs only for 3.5. Under truncation (RC 3) no correction is needed for 3.5.
        let saved = control_word();
        for rc in [0u16, 3] {
            set_control_word(GAME_CONTROL_WORD | (rc << 10));
            // SAFETY: clears the sticky exception flags only.
            unsafe { asm!("fnclex") };
            assert_eq!(F80::from_f32(3.5).ftol(), 3, "rc {rc}");
            assert_ne!(status_word() & 0x20, 0, "inexact raised, rc {rc}");
            let (_, _, _, ecx) = call_export(3.5);
            assert_eq!(ecx, if rc == 0 { keep_bits(-0.5) } else { keep_bits(0.5) }, "rc {rc}");
        }
        set_control_word(saved);
        fn keep_bits(d: f32) -> u32 {
            d.to_bits().wrapping_add(0x7FFF_FFFF)
        }
    }

    #[test]
    fn transcendentals_run_on_the_x87() {
        let saved = control_word();
        set_control_word(0x037F);
        let half_pi = F80::from_f64(core::f64::consts::FRAC_PI_2);
        assert_eq!(half_pi.sin().to_f64(), 1.0);
        // cos of the double nearest π/2: the double's own error, about 6.1e-17.
        let c = half_pi.cos().to_f64();
        assert!(c > 6.0e-17 && c < 6.2e-17, "{c:e}");
        assert_eq!(F80::from_f64(2.0).sqrt().to_f64(), 2f64.sqrt());
        assert_eq!(
            F80::from_f64(1.0).atan2(F80::from_f64(1.0)).to_f64(),
            core::f64::consts::FRAC_PI_4
        );
        assert_eq!(F80::from_f64(3.0).mul_log2(F80::from_f64(8.0)).to_f64(), 9.0);
        assert_eq!(F80::from_f64(1.0).exp2_m1().to_f64(), 1.0);
        assert_eq!(F80::from_f64(3.0).scale(F80::from_f64(2.0)).to_f64(), 12.0);
        assert_eq!(F80::from_f64(0.0).tan().to_f64(), 0.0);
        assert_eq!(F80::from_f64(-2.5).abs().to_f64(), 2.5);
        assert_eq!((-F80::from_f64(2.5)).to_f64(), -2.5);
        assert_eq!(F80::from_f64(2.5).round_int().to_f64(), 2.0);
        // The results keep the register's 64-bit significand.
        let sin_one = F80::from_f64(1.0).sin();
        assert_ne!(sin_one.significand() & 0x7FF, 0, "64-bit significand");
        set_control_word(saved);
    }
}
