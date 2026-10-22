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
    /// Inherited public mutations, occupant-manager §13.1. Undefined native
    /// return channels are deliberately absent; CanRemove is a defined bool.
    pub fn mutate(&mut self, operation: u32, value: u32) -> bool {
        match operation {
            0 => self.base.flags = value as u8,
            1 => self.base.flags |= value as u8,
            2 => self.base.flags &= !(value as u8),
            3 => self.base.highlight = (value & 15) as u8,
            4 | 5 => {
                let mut low = self.base.removal as u8;
                let mut high = (self.base.removal >> 8) as u8;
                let step = if operation == 4 { 1 } else { 255 };
                if value & 1 != 0 {
                    high = high.wrapping_add(step);
                }
                if value & 2 != 0 {
                    low = low.wrapping_add(step);
                }
                self.base.removal = u16::from(low) | (u16::from(high) << 8);
            }
            6 => {
                return if value & 1 != 0 {
                    self.base.removal >> 8 == 0
                } else if value & 2 != 0 {
                    self.base.removal & 255 == 0
                } else {
                    true
                };
            }
            _ => return false,
        }
        true
    }

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

/// Counted native-interface boundary (SDK cIGZUnknown, flora §3.4).
/// Destruction is performed by the owning C ABI handle at the zero transition.
pub struct OccupantReferences(std::sync::atomic::AtomicU32);

impl Default for OccupantReferences {
    fn default() -> Self {
        Self(std::sync::atomic::AtomicU32::new(1))
    }
}

impl OccupantReferences {
    pub fn add_ref(&self) -> u32 {
        self.0.fetch_add(1, std::sync::atomic::Ordering::AcqRel) + 1
    }

    pub fn release(&self) -> u32 {
        self.0.fetch_sub(1, std::sync::atomic::Ordering::AcqRel) - 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::{ByteReader, ByteWriter};

    #[test]
    fn removal_counter_wrap_and_combined_mask_precedence() {
        let mut record = OccupantRecord::default();
        for _ in 0..256 {
            record.mutate(4, 1);
        }
        assert!(record.mutate(6, 1));
        record.mutate(5, 2);
        assert!(!record.mutate(6, 2));
        assert!(record.mutate(6, 3)); // bit 1 has priority, not a combined test
    }

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
