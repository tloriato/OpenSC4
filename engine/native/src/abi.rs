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

//! Working on the game's own objects and calling the game's own functions.
//!
//! - [`layout!`] declares fixed-offset fields of a game object (`docs/abi/layouts.md`) as a pointer-sized view
//!   with a getter and a setter per field. The only `unsafe` is the view's constructor: whoever makes the view
//!   promises the pointer is such an object.
//! - [`vcall!`] calls a virtual method through the object's vtable slot (thiscall, the object in ECX).
//! - [`game_fn!`] declares a game function by its original address, convention and signature. In the DLL it
//!   calls that address, so a later replacement of the callee (a jump at its entry) is picked up. Under
//!   `cfg(test)` the address resolves to a fake the test installed (`crate::testing::fake`), else to our own
//!   port when one exists. A function that returns a float in ST0 is declared `-> St0` and returns the
//!   register value as an [`F80`], unrounded.
//! - [`game_data!`] declares a game global by address (a fake location under `cfg(test)`).
//! - [`call_raw`] calls any address with chosen EAX, ECX and EDX and stack words, for conventions Rust has no
//!   name for, and to take an ST0 result.

use crate::x87::F80;

/// Calling conventions of the game's functions.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Conv {
    /// `this` in ECX, stack arguments, the callee pops them.
    Thiscall,
    /// Stack arguments, the callee pops them.
    Stdcall,
    /// Stack arguments, the caller pops them.
    Cdecl,
}

/// A stack argument as the 4-byte word the caller pushes (small integers widened, floats as their bits).
pub trait Word: Copy {
    fn word(self) -> u32;
}

macro_rules! word_as {
    ($($t:ty),*) => { $(impl Word for $t { #[inline(always)] fn word(self) -> u32 { self as u32 } })* };
}
word_as!(u32, i32, u16, i16, u8, i8, usize, isize);

impl Word for f32 {
    #[inline(always)]
    fn word(self) -> u32 {
        self.to_bits()
    }
}
impl Word for bool {
    #[inline(always)]
    fn word(self) -> u32 {
        u32::from(self)
    }
}
impl<T> Word for *mut T {
    #[inline(always)]
    fn word(self) -> u32 {
        self as usize as u32
    }
}
impl<T> Word for *const T {
    #[inline(always)]
    fn word(self) -> u32 {
        self as usize as u32
    }
}

/// Where a call to the game function at `address` goes: the address itself in the DLL.
#[cfg(not(test))]
#[inline(always)]
pub fn target(address: u32) -> usize {
    address as usize
}

/// Where a call to the game function at `address` goes: the test's fake, else our port of it.
#[cfg(test)]
pub fn target(address: u32) -> usize {
    crate::testing::resolve(address)
}

/// Where the game global at `address` lives: the address itself in the DLL.
#[cfg(not(test))]
#[inline(always)]
pub fn data_target(address: u32) -> usize {
    address as usize
}

/// Where the game global at `address` lives: the test's stand-in.
#[cfg(test)]
pub fn data_target(address: u32) -> usize {
    crate::testing::resolve_data(address)
}

/// The function in the vtable slot at byte `offset` of `obj`'s vtable.
///
/// # Safety
///
/// `obj` points to an object whose first word is a vtable with at least `offset / 4 + 1` slots.
#[inline(always)]
pub unsafe fn vslot(obj: *mut u8, offset: usize) -> usize {
    // SAFETY: the caller's contract.
    unsafe { *(*(obj as *const *const u8)).add(offset).cast::<usize>() }
}

/// A call with chosen registers: [`call_raw`] reads `target`, `eax`, `ecx`, `edx` and the stack words, and writes
/// back EAX, EDX and, with `take_st0`, the ST0 result (popped). `#[repr(C)]`: the asm uses the offsets.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct RawCall {
    /// +0: the function.
    pub target: usize,
    /// +4: EAX at the call, EAX after it.
    pub eax: u32,
    /// +8: ECX at the call (`this` for thiscall).
    pub ecx: u32,
    /// +12: EDX at the call, EDX after it.
    pub edx: u32,
    /// +16: the stack words, first argument first (pushed last).
    pub args: *const u32,
    /// +20: how many.
    pub count: u32,
    /// +24: nonzero: the caller pops the words (cdecl).
    pub caller_pops: u32,
    /// +28: nonzero: the function returns a value in ST0, stored to `st0` and popped.
    pub take_st0: u32,
    /// +32: the ST0 result.
    pub st0: F80,
}

impl RawCall {
    pub fn new(conv: Conv, target: usize, this: u32, args: &[u32]) -> RawCall {
        RawCall {
            target,
            eax: 0,
            ecx: this,
            edx: 0,
            args: args.as_ptr(),
            count: args.len() as u32,
            caller_pops: u32::from(conv == Conv::Cdecl),
            take_st0: 0,
            st0: F80::ZERO,
        }
    }
}

// One call implementation, with an optional globally named return instruction.
macro_rules! raw_call_site {
    ($c:expr; $($label:expr),* $(,)?) => {
        core::arch::asm!(
            "mov ecx, dword ptr [edi + 20]",
            "mov eax, dword ptr [edi + 16]",
            "2:",
            "test ecx, ecx",
            "jz 3f",
            "dec ecx",
            "push dword ptr [eax + ecx*4]",
            "jmp 2b",
            "3:",
            "mov eax, dword ptr [edi + 4]",
            "mov ecx, dword ptr [edi + 8]",
            "mov edx, dword ptr [edi + 12]",
            "call dword ptr [edi]",
            $($label,)*
            "mov dword ptr [edi + 4], eax",
            "mov dword ptr [edi + 12], edx",
            "cmp dword ptr [edi + 24], 0",
            "je 4f",
            "mov ecx, dword ptr [edi + 20]",
            "lea esp, [esp + ecx*4]",
            "4:",
            "cmp dword ptr [edi + 28], 0",
            "je 5f",
            "fstp tbyte ptr [edi + 32]",
            "5:",
            in("edi") $c as *mut $crate::abi::RawCall,
            clobber_abi("C"),
        );
    };
}

/// Makes the call `c` describes.
///
/// # Safety
///
/// `c.target` is a function that takes what `c` passes, preserves EBX, ESI, EDI and EBP, pops exactly
/// `c.count` words unless `c.caller_pops`, and returns a value in ST0 if (and only if) `c.take_st0`. `c.args`
/// points to `c.count` readable words.
pub unsafe fn call_raw(c: &mut RawCall) {
    // SAFETY: the caller's contract. EDI keeps the descriptor across the call.
    unsafe {
        raw_call_site!(c;);
    }
}

/// Calls a game function that returns a float in ST0 and returns that register value unrounded.
///
/// # Safety
///
/// As [`call_raw`]: `target` takes `this` (thiscall) and `args` with convention `conv`.
pub unsafe fn call_st0(conv: Conv, target: usize, this: u32, args: &[u32]) -> F80 {
    let mut c = RawCall::new(conv, target, this, args);
    c.take_st0 = 1;
    // SAFETY: the caller's contract.
    unsafe { call_raw(&mut c) };
    c.st0
}

/// `"thiscall"` / `"stdcall"` / `"C"` (or `"cdecl"`) as a [`Conv`].
pub const fn conv_of(abi: &str) -> Conv {
    match abi.as_bytes() {
        b"thiscall" => Conv::Thiscall,
        b"stdcall" => Conv::Stdcall,
        b"C" | b"cdecl" => Conv::Cdecl,
        _ => panic!("unknown calling convention"),
    }
}

/// Declares fixed-offset fields of a game object; see the module docs.
///
/// ```ignore
/// layout! {
///     /// cSC4Example, offsets from its primary interface.
///     pub struct Example {
///         /// A field.
///         0x24 => grid / set_grid: *mut u8,
///     }
/// }
/// ```
#[allow(unused_macros)]
macro_rules! layout {
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident {
            $( $(#[$fmeta:meta])* $offset:literal => $get:ident / $set:ident : $ty:ty ),* $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        #[repr(transparent)]
        $vis struct $name(*mut u8);

        // The setters store pointer values; they dereference only the view's own pointer (`from_ptr`'s contract).
        #[allow(dead_code, clippy::not_unsafe_ptr_arg_deref)]
        impl $name {
            /// The declared fields as (getter name, offset, size).
            pub const FIELDS: &'static [(&'static str, usize, usize)] =
                &[$((stringify!($get), $offset, core::mem::size_of::<$ty>())),*];

            /// A view of the object at `p`.
            ///
            /// # Safety
            ///
            /// `p` points to a live object with this layout, readable and writable at every declared field for
            /// as long as the view (or a copy of it) is used.
            #[inline(always)]
            pub const unsafe fn from_ptr(p: *mut u8) -> Self {
                Self(p)
            }

            /// The object's address.
            #[inline(always)]
            pub const fn as_ptr(self) -> *mut u8 {
                self.0
            }

            $(
                $(#[$fmeta])*
                #[inline(always)]
                pub fn $get(self) -> $ty {
                    // SAFETY: `from_ptr`'s contract.
                    unsafe { core::ptr::read_unaligned(self.0.add($offset).cast::<$ty>()) }
                }

                #[doc = concat!("Writes the field read by [`Self::", stringify!($get), "`].")]
                #[inline(always)]
                pub fn $set(self, value: $ty) {
                    // SAFETY: `from_ptr`'s contract.
                    unsafe { core::ptr::write_unaligned(self.0.add($offset).cast::<$ty>(), value) }
                }
            )*
        }
    };
}

/// Calls the virtual method in vtable slot `offset` (bytes) of `obj` (thiscall, `obj` in ECX). Use inside an
/// `unsafe` block: the caller vouches that `obj` is a live object whose slot has that signature.
///
/// `vcall!(obj, 0x30 => fn(i32, i32) -> u32, x, z)`
#[allow(unused_macros)]
macro_rules! vcall {
    ($obj:expr, $offset:expr => fn($($ty:ty),* $(,)?) $(-> $ret:ty)? $(, $arg:expr)* $(,)?) => {{
        let obj: *mut u8 = $obj;
        let f = core::mem::transmute::<usize, unsafe extern "thiscall" fn(*mut u8 $(, $ty)*) $(-> $ret)?>(
            $crate::abi::vslot(obj, $offset));
        f(obj $(, $arg)*)
    }};
}

/// Calls the virtual method in vtable slot `offset` of `obj` that returns a float in ST0; the result is the
/// unrounded register value (an [`F80`]). Arguments are passed as stack words ([`Word`]). Use inside `unsafe`.
#[allow(unused_macros)]
macro_rules! vcall_st0 {
    ($obj:expr, $offset:expr $(, $arg:expr)* $(,)?) => {{
        let obj: *mut u8 = $obj;
        let args: &[u32] = &[$($crate::abi::Word::word($arg)),*];
        $crate::abi::call_st0($crate::abi::Conv::Thiscall, $crate::abi::vslot(obj, $offset),
            $crate::abi::Word::word(obj), args)
    }};
}

/// Declares a game function by original address; see the module docs.
///
/// ```ignore
/// game_fn! {
///     /// cSC4ResponseCurve::GetOutputValue
///     pub fn curve_output(curve: *mut u8, x: f32) -> St0 = "thiscall" 0x005F_4250;
/// }
/// game_fn! {
///     /// Some stdcall function returning EAX.
///     pub fn example(a: i32, b: *mut u8) -> u32 = "stdcall" 0x0040_0000;
/// }
/// ```
#[allow(unused_macros)]
macro_rules! game_fn {
    // ST0 result, thiscall.
    (
        $(#[$meta:meta])*
        $vis:vis fn $name:ident($this:ident: $this_ty:ty $(, $arg:ident: $ty:ty)* $(,)?) -> St0 = "thiscall" $address:literal;
    ) => {
        $(#[$meta])*
        #[doc = ""]
        #[doc = concat!("Game function at ", stringify!($address), " (thiscall), ST0 result unrounded.")]
        #[doc = ""]
        #[doc = "# Safety"]
        #[doc = ""]
        #[doc = "The arguments must be what the game function expects."]
        #[inline]
        $vis unsafe fn $name($this: $this_ty $(, $arg: $ty)*) -> $crate::x87::F80 {
            let args: &[u32] = &[$($crate::abi::Word::word($arg)),*];
            // SAFETY: the caller's contract and the function's convention.
            unsafe {
                $crate::abi::call_st0($crate::abi::Conv::Thiscall, $crate::abi::target($address),
                    $crate::abi::Word::word($this), args)
            }
        }
    };
    // ST0 result, stdcall or cdecl.
    (
        $(#[$meta:meta])*
        $vis:vis fn $name:ident($($arg:ident: $ty:ty),* $(,)?) -> St0 = $abi:literal $address:literal;
    ) => {
        $(#[$meta])*
        #[doc = ""]
        #[doc = concat!("Game function at ", stringify!($address), " (", $abi, "), ST0 result unrounded.")]
        #[doc = ""]
        #[doc = "# Safety"]
        #[doc = ""]
        #[doc = "The arguments must be what the game function expects."]
        #[inline]
        $vis unsafe fn $name($($arg: $ty),*) -> $crate::x87::F80 {
            let args: &[u32] = &[$($crate::abi::Word::word($arg)),*];
            // SAFETY: the caller's contract and the function's convention.
            unsafe { $crate::abi::call_st0($crate::abi::conv_of($abi), $crate::abi::target($address), 0, args) }
        }
    };
    // EAX, EDX:EAX or no result: a typed call.
    (
        $(#[$meta:meta])*
        $vis:vis fn $name:ident($($arg:ident: $ty:ty),* $(,)?) $(-> $ret:ty)? = $abi:tt $address:literal;
    ) => {
        $(#[$meta])*
        #[doc = ""]
        #[doc = concat!("Game function at ", stringify!($address), " (", $abi, ").")]
        #[doc = ""]
        #[doc = "# Safety"]
        #[doc = ""]
        #[doc = "The arguments must be what the game function expects."]
        #[inline]
        $vis unsafe fn $name($($arg: $ty),*) $(-> $ret)? {
            // SAFETY: the caller's contract; the address holds a function of this signature.
            unsafe {
                let f = core::mem::transmute::<usize, unsafe extern $abi fn($($ty),*) $(-> $ret)?>(
                    $crate::abi::target($address));
                f($($arg),*)
            }
        }
    };
}

/// Calls the current rel32 target of an original E8 call instruction, including later hooks.
///
/// # Safety
/// `site` points at a readable E8 rel32 instruction in the game image.
pub unsafe fn call_site_target(site: u32) -> usize {
    let p = data_target(site) as *const u8;
    // SAFETY: the caller promises a live call instruction; volatile reads observe later hooks.
    unsafe {
        debug_assert_eq!(p.read_volatile(), 0xE8);
        let rel = i32::from_le_bytes(p.add(1).cast::<[u8; 4]>().read_volatile());
        (p as usize).wrapping_add(5).wrapping_add(rel as usize)
    }
}

/// A typed call through an original call instruction, resolved anew on every invocation.
#[allow(unused_macros)]
macro_rules! game_call {
    ($(#[$meta:meta])* $vis:vis fn $name:ident($($arg:ident: $ty:ty),* $(,)?) $(-> $ret:ty)? = $abi:tt $site:literal;) => {
        $(#[$meta])*
        #[allow(dead_code)]
        #[inline]
        $vis unsafe fn $name($($arg: $ty),*) $(-> $ret)? {
            // SAFETY: caller supplies the original call's arguments and convention.
            unsafe {
                let f = core::mem::transmute::<usize, unsafe extern $abi fn($($ty),*) $(-> $ret)?>(
                    $crate::abi::call_site_target($site));
                f($($arg),*)
            }
        }
    };
}

/// A thiscall with an integer result and a globally labelled return instruction. Define once per site.
/// The label is exported independently of the wrapper, which cannot be inlined or cloned.
macro_rules! game_return_fn {
    ($vis:vis fn $name:ident($this:ident: $this_ty:ty $(, $arg:ident: $ty:ty)*) -> $ret:ty = $address:literal, $label:ident;) => {
        #[cfg(not(test))]
        core::arch::global_asm!(
            ".pushsection .drectve,\"yn\"",
            concat!(".ascii \" /EXPORT:", stringify!($label), "=_", stringify!($label), "\""),
            ".popsection",
        );
        #[inline(never)]
        #[no_mangle]
        // This externally visible, non-inlined wrapper has exactly one copy of the labelled call.
        #[allow(named_asm_labels)]
        $vis unsafe extern "thiscall" fn $name($this: $this_ty $(, $arg: $ty)*) -> $ret {
            let args: &[u32] = &[$($crate::abi::Word::word($arg)),*];
            let mut call = $crate::abi::RawCall::new($crate::abi::Conv::Thiscall,
                $crate::abi::target($address), $crate::abi::Word::word($this), args);
            // SAFETY: caller supplies the original function's arguments and convention.
            unsafe {
                raw_call_site!(&mut call;
                    concat!(".globl _", stringify!($label)), concat!("_", stringify!($label), ":"));
            }
            ((u64::from(call.edx) << 32) | u64::from(call.eax)) as $ret
        }
    };
}

/// Follow a pool stamp's callable patch, if installed, before unlinking the object.
///
/// # Safety
/// `site` is a pool-page load in the supported exe, optionally patched to a no-argument call.
pub unsafe fn pool_take(site: u32) {
    #[cfg(test)]
    let Some(p) = crate::testing::optional_data(site) else {
        return;
    };
    #[cfg(not(test))]
    let p = site as usize;
    // SAFETY: the site is either the original load (no stamp) or a callable patch preserving registers.
    unsafe {
        if (p as *const u8).read_volatile() == 0xE8 {
            // The original load may write a callee-saved register (including EDI). This thunk has
            // an instruction-site convention, so keep all registers around it instead of using RawCall.
            core::arch::asm!(
                "pushfd", "pushad", "call eax", "popad", "popfd",
                in("eax") call_site_target(site), clobber_abi("C"),
            );
        }
    }
}

/// Declares a game global by address: `game_data! { pub static name: T = 0x00B4_3D14; }` defines
/// `name() -> *mut T`.
#[allow(unused_macros)]
macro_rules! game_data {
    ($(#[$meta:meta])* $vis:vis static $name:ident: $ty:ty = $address:literal;) => {
        $(#[$meta])*
        #[doc = ""]
        #[doc = concat!("The game global at ", stringify!($address), ".")]
        #[inline(always)]
        $vis fn $name() -> *mut $ty {
            $crate::abi::data_target($address) as *mut $ty
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{fake, fake_data, GameFpu};

    layout! {
        /// A test object.
        struct Thing {
            0x04 => count / set_count: i32,
            0x09 => odd / set_odd: u32,
            0x10 => scale / set_scale: f32,
        }
    }

    #[test]
    fn layout_fields() {
        let mut mem = [0u8; 0x20];
        // SAFETY: 0x20 bytes cover every field.
        let t = unsafe { Thing::from_ptr(mem.as_mut_ptr()) };
        t.set_count(-5);
        t.set_odd(0xAABB_CCDD);
        t.set_scale(2.5);
        assert_eq!((t.count(), t.odd(), t.scale()), (-5, 0xAABB_CCDD, 2.5));
        assert_eq!(&mem[4..8], &(-5i32).to_le_bytes());
        assert_eq!(&mem[9..13], &0xAABB_CCDDu32.to_le_bytes());
        assert_eq!(Thing::FIELDS, &[("count", 4, 4), ("odd", 9, 4), ("scale", 0x10, 4)]);
    }

    extern "thiscall" fn add_slot(this: *mut u8, a: i32, b: i32) -> i32 {
        // SAFETY: the test's object (vtable word, then a bias).
        a + b + unsafe { *this.add(4).cast::<i32>() }
    }

    extern "thiscall" fn half_slot(_this: *mut u8, x: f32) -> f32 {
        x * 0.5
    }

    #[test]
    fn virtual_calls() {
        let mut vtable = [0usize; 8];
        vtable[3] = add_slot as *const () as usize;
        vtable[5] = half_slot as *const () as usize;
        let mut obj = [vtable.as_ptr() as usize, 100usize];
        let p = obj.as_mut_ptr().cast::<u8>();
        // SAFETY: the object above.
        let sum = unsafe { vcall!(p, 0x0C => fn(i32, i32) -> i32, 2, 3) };
        assert_eq!(sum, 105);
        let _fpu = GameFpu::new();
        // SAFETY: the object above.
        let half = unsafe { vcall_st0!(p, 0x14, 5.0f32) };
        assert_eq!(half.to_f32(), 2.5);
    }

    game_fn! {
        /// A stdcall test function.
        fn sub_std(a: i32, b: i32) -> i32 = "stdcall" 0x0000_1234;
    }
    game_fn! {
        /// A cdecl test function returning in ST0.
        fn twice_c(x: f32) -> St0 = "C" 0x0000_1238;
    }
    game_data! {
        /// A test global.
        static counter: u32 = 0x0000_2000;
    }

    extern "stdcall" fn sub_impl(a: i32, b: i32) -> i32 {
        a - b
    }

    extern "C" fn twice_impl(x: f32) -> f32 {
        x * 2.0
    }

    #[test]
    fn game_functions_resolve_to_fakes() {
        let _a = fake(0x0000_1234, sub_impl as *const () as usize);
        let _b = fake(0x0000_1238, twice_impl as *const () as usize);
        let mut value = 7u32;
        let _c = fake_data(0x0000_2000, (&mut value as *mut u32).cast());
        let _fpu = GameFpu::new();
        // SAFETY: the fakes above.
        unsafe {
            assert_eq!(sub_std(10, 4), 6);
            assert_eq!(twice_c(1.25).to_f32(), 2.5);
            *counter() += 1;
        }
        assert_eq!(value, 8);
    }

    game_call! { fn patched_sub(a: i32, b: i32) -> i32 = "stdcall" 0x0000_1240; }
    extern "stdcall" fn add_impl(a: i32, b: i32) -> i32 {
        a + b
    }

    #[test]
    fn call_sites_follow_a_later_rel32_patch() {
        let mut code = [0xE8, 0, 0, 0, 0];
        let _site = fake_data(0x0000_1240, code.as_mut_ptr());
        for (target, expected) in [(sub_impl as *const (), 6), (add_impl as *const (), 14)] {
            let rel = (target as usize).wrapping_sub(code.as_ptr() as usize + 5) as u32;
            code[1..].copy_from_slice(&rel.to_le_bytes());
            // SAFETY: the mapped instruction now targets a stdcall function with this signature.
            assert_eq!(unsafe { patched_sub(10, 4) }, expected);
        }
    }

    #[test]
    #[should_panic(expected = "no fake and no port")]
    fn missing_fake_panics() {
        // SAFETY: never reached (no target).
        unsafe {
            sub_std(1, 2);
        }
    }

    extern "fastcall" fn regs_impl(ecx: u32, edx: u32, a: u32) -> u64 {
        (u64::from(ecx) << 32) | u64::from(edx.wrapping_add(a))
    }

    #[test]
    fn raw_call_registers() {
        let args = [5u32];
        let mut c = RawCall::new(Conv::Stdcall, regs_impl as *const () as usize, 0x1111, &args);
        c.edx = 0x20;
        // SAFETY: fastcall: ECX, EDX, one stack word, callee pops.
        unsafe { call_raw(&mut c) };
        assert_eq!((c.edx, c.eax), (0x1111, 0x25));
    }
}
