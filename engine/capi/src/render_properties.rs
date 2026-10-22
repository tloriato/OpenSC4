// SPDX-License-Identifier: GPL-3.0-or-later
//! Byte-preserving native text adapter; callbacks are borrowed for the owner's lifetime.
use fc4_core::render_properties::{ByteCtype, RenderProperties};
use std::sync::Mutex;

type Primitive = unsafe extern "C" fn(i32) -> i32;

struct Text {
    upper: Primitive,
    lower: Primitive,
}
impl ByteCtype for Text {
    fn isupper(&self, byte: i32) -> bool {
        unsafe { (self.upper)(byte) != 0 }
    }
    fn tolower(&self, byte: i32) -> i32 {
        unsafe { (self.lower)(byte) }
    }
}

#[allow(non_camel_case_types)]
pub struct fc4_renderprops {
    text: Text,
    state: Mutex<RenderProperties>,
}

/// # Safety
/// Both callbacks must implement the executable CRT ABI and remain callable until destroy.
#[no_mangle]
pub unsafe extern "C" fn fc4_renderprops_create(
    upper: Option<Primitive>,
    lower: Option<Primitive>,
) -> *mut fc4_renderprops {
    let (Some(upper), Some(lower)) = (upper, lower) else {
        return std::ptr::null_mut();
    };
    let text = Text { upper, lower };
    let state = Mutex::new(RenderProperties::with_ctype(&text));
    Box::into_raw(Box::new(fc4_renderprops { text, state }))
}

/// # Safety
/// Null or a live handle from create, exclusively owned, with no calls in flight.
#[no_mangle]
pub unsafe extern "C" fn fc4_renderprops_destroy(owner: *mut fc4_renderprops) {
    if !owner.is_null() {
        drop(Box::from_raw(owner));
    }
}

/// # Safety
/// Owner must be live; kind 0/1/2 means bool/int/float. Returns -1 on invalid input.
#[no_mangle]
pub unsafe extern "C" fn fc4_renderprops_count(owner: *const fc4_renderprops, kind: u32) -> i32 {
    let Some(owner) = owner.as_ref() else {
        return -1;
    };
    let Ok(state) = owner.state.lock() else {
        return -1;
    };
    match kind {
        0 => state.num_bool_properties(),
        1 => state.num_int_properties(),
        2 => state.num_float_properties(),
        _ => -1,
    }
}

/// # Safety
/// Owner must be live; bytes must be readable for len bytes (null allowed only at len zero).
/// First-NUL semantics apply; a counted buffer need not contain a terminator.
#[no_mangle]
pub unsafe extern "C" fn fc4_renderprops_find(
    owner: *const fc4_renderprops,
    kind: u32,
    bytes: *const u8,
    len: usize,
) -> i32 {
    let Some(owner) = owner.as_ref() else {
        return -1;
    };
    if bytes.is_null() && len != 0 {
        return -1;
    }
    let bytes = if len == 0 {
        &[]
    } else {
        std::slice::from_raw_parts(bytes, len)
    };
    let Ok(state) = owner.state.lock() else {
        return -1;
    };
    match kind {
        0 => state.find_bool_with_ctype(bytes, &owner.text),
        1 => state.find_int_with_ctype(bytes, &owner.text),
        2 => state.find_float_with_ctype(bytes, &owner.text),
        _ => -1,
    }
}

/// # Safety
/// Owner live; output writable for capacity bytes, unless capacity zero. Copies raw name
/// bytes, no terminator. Returns required length, -1 invalid, -2 insufficient capacity.
#[no_mangle]
pub unsafe extern "C" fn fc4_renderprops_name(
    owner: *const fc4_renderprops,
    kind: u32,
    index: i32,
    output: *mut u8,
    capacity: usize,
) -> i32 {
    let Some(owner) = owner.as_ref() else {
        return -1;
    };
    let Ok(state) = owner.state.lock() else {
        return -1;
    };
    let name = match kind {
        0 => state.bool_name_bytes(index),
        1 => state.int_name_bytes(index),
        2 => state.float_name_bytes(index),
        _ => None,
    };
    let Some(name) = name else { return -1 };
    if capacity == 0 {
        return name.len() as i32;
    }
    if output.is_null() || capacity < name.len() {
        return -2;
    }
    std::ptr::copy_nonoverlapping(name.as_ptr(), output, name.len());
    name.len() as i32
}

/// # Safety
/// Owner live; output a writable u32. Values use bool 0/1, int bits, or IEEE binary32 bits.
/// Returns 1 on success and 0 on invalid input, leaving output unchanged.
#[no_mangle]
pub unsafe extern "C" fn fc4_renderprops_get(
    owner: *const fc4_renderprops,
    kind: u32,
    index: i32,
    output: *mut u32,
) -> i32 {
    let (Some(owner), Some(output)) = (owner.as_ref(), output.as_mut()) else {
        return 0;
    };
    let Ok(state) = owner.state.lock() else {
        return 0;
    };
    let bits = match kind {
        0 => state.get_bool(index).map(u32::from),
        1 => state.get_int(index).map(|x| x as u32),
        2 => state.get_float(index).map(f32::to_bits),
        _ => None,
    };
    let Some(bits) = bits else { return 0 };
    *output = bits;
    1
}

/// # Safety
/// Owner live; same bit encoding as get. Bool accepts only 0/1; all other bits preserved.
/// Invalid indices are rejected (adapter safety extension, not native behavior).
#[no_mangle]
pub unsafe extern "C" fn fc4_renderprops_set(
    owner: *const fc4_renderprops,
    kind: u32,
    index: i32,
    bits: u32,
) -> i32 {
    let Some(owner) = owner.as_ref() else {
        return 0;
    };
    let Ok(mut state) = owner.state.lock() else {
        return 0;
    };
    i32::from(match kind {
        0 if bits <= 1 => state.set_bool(index, bits != 0),
        1 => state.set_int(index, bits as i32),
        2 => state.set_float(index, f32::from_bits(bits)),
        _ => false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    static TRACE: Mutex<Vec<i32>> = Mutex::new(Vec::new());
    unsafe extern "C" fn upper(x: i32) -> i32 {
        if (65..=90).contains(&x) {
            0x100
        } else {
            0
        }
    }
    unsafe extern "C" fn lower(x: i32) -> i32 {
        TRACE.lock().unwrap().push(x);
        (if (65..=90).contains(&x) { x + 32 } else { x }) | 0x10000
    }
    #[test]
    fn foreign_byte_abi_and_independent_lifetimes() {
        unsafe {
            assert!(fc4_renderprops_create(None, Some(lower)).is_null());
            assert!(fc4_renderprops_create(Some(upper), None).is_null());
            let a = fc4_renderprops_create(Some(upper), Some(lower));
            let b = fc4_renderprops_create(Some(upper), Some(lower));
            assert_eq!(fc4_renderprops_count(a, 0), 38);
            assert_eq!(fc4_renderprops_count(a, 1), 24);
            assert_eq!(fc4_renderprops_count(a, 2), 15);
            TRACE.lock().unwrap().clear();
            let query = [b'R', b'S', 0, 0x80, 0xff];
            assert_eq!(fc4_renderprops_find(a, 0, query.as_ptr(), query.len()), 9);
            assert_eq!(*TRACE.lock().unwrap(), vec![82, 83]);
            TRACE.lock().unwrap().clear();
            let query = [0x49, 0x80, 0xd0, 0xff, 0];
            assert_eq!(fc4_renderprops_find(a, 0, query.as_ptr(), query.len()), -1);
            assert_eq!(*TRACE.lock().unwrap(), vec![73, -128, -48, -1]);
            let mut name = [0x5a; 32];
            assert_eq!(fc4_renderprops_name(a, 0, 9, name.as_mut_ptr(), 32), 13);
            assert_eq!(&name[..14], b"rendershadowsZ");
            assert_eq!(fc4_renderprops_name(a, 0, 38, name.as_mut_ptr(), 32), -1);
            assert_eq!(fc4_renderprops_set(a, 2, 3, 0x80000000), 1);
            let mut bits = 0;
            assert_eq!(fc4_renderprops_get(a, 2, 3, &mut bits), 1);
            assert_eq!(bits, 0x80000000);
            assert_eq!(fc4_renderprops_get(b, 2, 3, &mut bits), 1);
            assert_eq!(bits, 1f32.to_bits());
            fc4_renderprops_destroy(a);
            assert_eq!(fc4_renderprops_count(b, 0), 38);
            fc4_renderprops_destroy(b);
        }
    }
}
