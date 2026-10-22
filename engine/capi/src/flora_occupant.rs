// SPDX-License-Identifier: GPL-3.0-or-later
//! Typed independent flora occupant codec for native verification.
use fc4_core::flora::occupant::OccupantRecord;
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
/// `input` and `bytes` must name valid storage of the declared sizes.
#[no_mangle]
pub unsafe extern "C" fn fc4_flora_occupant_encode(
    input: *const FloraOccupantInput,
    bytes: *mut u8,
    capacity: usize,
) -> usize {
    let Some(input) = input.as_ref() else { return 0 };
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
    let Some(output) = output.as_mut() else { return false };
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
