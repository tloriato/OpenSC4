// SPDX-License-Identifier: GPL-3.0-or-later
//! Independent occupant record, flora §3.4/11.4 and lot-records §2.3/11.
use crate::automata::OccupantBase;
use crate::stream::{InStream, OutStream};

#[derive(Clone, Debug, PartialEq)]
pub struct OccupantRecord {
    pub base: OccupantBase,
    pub flora_type: u32,
    pub position: [f32; 3],
    pub birth: u32,
    pub seeding: u32,
    pub state: i8,
    pub orientation: u8,
    pub lot_tag: u32,
}

impl Default for OccupantRecord {
    fn default() -> Self {
        Self {
            base: OccupantBase::new(0x74758926),
            flora_type: 0,
            position: [0.0; 3],
            birth: 0,
            seeding: 0,
            state: 0,
            orientation: 0,
            lot_tag: 0,
        }
    }
}

impl OccupantRecord {
    pub fn write(&self, out: &mut dyn OutStream) -> bool {
        out.put_u16(3);
        self.base.write(out);
        out.put_u32(self.flora_type);
        for p in self.position {
            out.put_f32(p);
        }
        out.put_u32(self.birth);
        out.put_u32(self.seeding);
        out.put_i8(self.state);
        out.put_u8(self.orientation);
        out.put_u32(self.lot_tag);
        out.error() == 0
    }

    pub fn read(&mut self, inp: &mut dyn InStream) -> bool {
        if inp.get_u16() != Some(3) {
            return false;
        }
        let Some(base) = OccupantBase::read(inp) else {
            // Literal lr §11: a rejected base version with no stream error succeeds.
            return inp.error() == 0;
        };
        self.base = base;
        // Partial-field state on stream failure is unspecified: commit fields only
        // after the complete payload; the native probe refuses partial-state credit.
        let fields = (|| {
            Some((
                inp.get_u32()?,
                [inp.get_f32()?, inp.get_f32()?, inp.get_f32()?],
                inp.get_u32()?,
                inp.get_u32()?,
                inp.get_i8()?,
                inp.get_u8()?,
                inp.get_u32()?,
            ))
        })();
        let Some((t, p, b, s, state, o, lot)) = fields else {
            return false;
        };
        self.flora_type = t;
        self.position = p;
        self.birth = b;
        self.seeding = s;
        self.state = state;
        self.orientation = o;
        self.lot_tag = lot;
        inp.error() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::{ByteReader, ByteWriter};

    #[test]
    fn versions_truncation_and_normalization() {
        let record = OccupantRecord {
            flora_type: 0xdeadbeef,
            state: 255_u32 as i8,
            orientation: 257_u32 as u8,
            position: [-1.5, 0.0, 300.25],
            ..OccupantRecord::default()
        };
        let mut out = ByteWriter::new();
        assert!(record.write(&mut out));
        let owned = out.into_bytes();
        let bytes = owned.as_slice();
        for n in 0..bytes.len() {
            assert!(!OccupantRecord::default().read(&mut ByteReader::new(&bytes[..n])));
        }
        let mut loaded = OccupantRecord::default();
        assert!(loaded.read(&mut ByteReader::new(bytes)));
        assert_eq!(loaded.state, -1);
        assert_eq!(loaded.orientation, 1);
        assert_eq!(loaded.position, record.position);
        assert_eq!(loaded.base.status, 2);
        assert!(!loaded.read(&mut ByteReader::new(&[2, 0])));
        assert!(loaded.read(&mut ByteReader::new(&[3, 0, 5, 0])));
    }
}
