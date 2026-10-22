// SPDX-License-Identifier: GPL-3.0-or-later
//! Typed independent flora occupant codec for native verification.
use crate::stream::{fc4_ostream, COut};
use fc4_core::flora::occupant::OccupantRecord;
use fc4_core::flora::occupant::OccupantReferences;
use fc4_core::stream::{ByteReader, ByteWriter};

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FloraOccupantInput {
    pub flora_type: u32,
    pub birth: u32,
    pub seeding: u32,
    pub state: i32,
    pub orientation: u32,
    pub lot_tag: u32,
    pub position: [f32; 3],
    pub flags: u8,
    pub highlight: u8,
    pub removal: u16,
    pub cells: [u8; 4],
}

impl FloraOccupantInput {
    fn record(self) -> OccupantRecord {
        let mut r = OccupantRecord {
            flora_type: self.flora_type,
            birth: self.birth,
            seeding: self.seeding,
            state: self.state as i8,
            orientation: self.orientation as u8,
            lot_tag: self.lot_tag,
            position: self.position,
            ..OccupantRecord::default()
        };
        r.base.flags = self.flags;
        r.base.highlight = self.highlight;
        r.base.removal = self.removal;
        r.base.cell_rect = self.cells;
        r
    }
}

/// # Safety
/// `input` must be readable. Returns UINT32_MAX for null/unknown operation;
/// state is returned as a sign-extended i32 bit pattern. Operations are documented
/// beside OccupantRecord::query, including single-bit-only flag query credit.
#[no_mangle]
pub unsafe extern "C" fn fc4_flora_occupant_query(
    input: *const FloraOccupantInput,
    operation: u32,
    argument: u32,
) -> u32 {
    input
        .as_ref()
        .and_then(|input| input.record().query(operation, argument))
        .unwrap_or(u32::MAX)
}

/// # Safety
/// `input` must be writable for the duration of the call.
#[no_mangle]
pub unsafe extern "C" fn fc4_flora_occupant_mutate(
    input: *mut FloraOccupantInput,
    operation: u32,
    value: u32,
) -> bool {
    let Some(input) = input.as_mut() else {
        return false;
    };
    let mut record = input.record();
    let result = record.mutate(operation, value);
    input.flags = record.base.flags;
    input.highlight = record.base.highlight;
    input.removal = record.base.removal;
    result
}

/// # Safety
/// `input` and `out` must be valid; stream callbacks must obey fc4_ostream.
#[no_mangle]
pub unsafe extern "C" fn fc4_flora_occupant_write(
    input: *const FloraOccupantInput,
    out: *const fc4_ostream,
) -> bool {
    let (Some(input), Some(out)) = (input.as_ref(), out.as_ref()) else {
        return false;
    };
    if out.struct_size < std::mem::size_of::<fc4_ostream>() as u32 {
        return false;
    }
    input.record().write(&mut COut(out))
}

/// # Safety
/// `handle` must be writable, initially null, then only managed by this function.
/// Operations: 0 construct, 1 AddRef, 2 Release (clears handle at destruction).
#[no_mangle]
pub unsafe extern "C" fn fc4_flora_occupant_reference(
    handle: *mut *mut OccupantReferences,
    operation: u32,
) -> u32 {
    let Some(handle) = handle.as_mut() else {
        return u32::MAX;
    };
    match operation {
        0 if handle.is_null() => {
            *handle = Box::into_raw(Box::default());
            1
        }
        1 if !handle.is_null() => (**handle).add_ref(),
        2 if !handle.is_null() => {
            let count = (**handle).release();
            if count == 0 {
                drop(Box::from_raw(*handle));
                *handle = std::ptr::null_mut();
            }
            count
        }
        _ => u32::MAX,
    }
}

/// # Safety
/// `input` and `bytes` must name valid storage of the declared sizes.
#[no_mangle]
pub unsafe extern "C" fn fc4_flora_occupant_encode(
    input: *const FloraOccupantInput,
    bytes: *mut u8,
    capacity: usize,
) -> usize {
    let Some(input) = input.as_ref() else {
        return 0;
    };
    let mut out = ByteWriter::new();
    if !input.record().write(&mut out) {
        return 0;
    }
    let data = out.into_bytes();
    if bytes.is_null() || capacity < data.len() {
        return 0;
    }
    std::ptr::copy_nonoverlapping(data.as_ptr(), bytes, data.len());
    data.len()
}

/// # Safety
/// `bytes` must name `size` readable bytes; `output` must be writable.
#[no_mangle]
pub unsafe extern "C" fn fc4_flora_occupant_decode(
    bytes: *const u8,
    size: usize,
    output: *mut FloraOccupantInput,
) -> bool {
    let Some(output) = output.as_mut() else {
        return false;
    };
    if bytes.is_null() {
        return false;
    }
    let mut r = OccupantRecord::default();
    let result = r.read(&mut ByteReader::new(std::slice::from_raw_parts(bytes, size)));
    *output = FloraOccupantInput {
        flora_type: r.flora_type,
        birth: r.birth,
        seeding: r.seeding,
        state: r.state as i32,
        orientation: r.orientation as u32,
        lot_tag: r.lot_tag,
        position: r.position,
        flags: r.base.flags,
        highlight: r.base.highlight,
        removal: r.base.removal,
        cells: r.base.cell_rect,
    };
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_boundary_refusals_and_owned_lifetime() {
        unsafe {
            let input = FloraOccupantInput {
                state: 255,
                orientation: 257,
                ..Default::default()
            };
            let mut bytes = [0; 128];
            assert_eq!(fc4_flora_occupant_encode(&input, bytes.as_mut_ptr(), 1), 0);
            assert_eq!(
                fc4_flora_occupant_encode(std::ptr::null(), bytes.as_mut_ptr(), bytes.len()),
                0
            );
            let size = fc4_flora_occupant_encode(&input, bytes.as_mut_ptr(), bytes.len());
            let mut output = FloraOccupantInput::default();
            assert!(fc4_flora_occupant_decode(bytes.as_ptr(), size, &mut output));
            assert_eq!((output.state, output.orientation), (-1, 1));
            let mut refs = std::ptr::null_mut();
            assert_eq!(fc4_flora_occupant_reference(&mut refs, 0), 1);
            assert_eq!(fc4_flora_occupant_reference(&mut refs, 0), u32::MAX);
            assert_eq!(fc4_flora_occupant_reference(&mut refs, 1), 2);
            assert_eq!(fc4_flora_occupant_reference(&mut refs, 2), 1);
            assert_eq!(fc4_flora_occupant_reference(&mut refs, 2), 0);
            assert!(refs.is_null());
            assert_eq!(fc4_flora_occupant_reference(&mut refs, 2), u32::MAX);
        }
    }
}
