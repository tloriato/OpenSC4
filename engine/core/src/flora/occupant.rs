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
    pub fn flags(&self) -> u32 {
        u32::from(self.base.flags)
    }

    pub fn highlight(&self) -> u32 {
        u32::from(self.base.highlight & 15)
    }

    pub fn is_flag_set(&self, bit: u32) -> bool {
        // Literal single-bit flag query; multi-bit semantics remain FQ18.
        self.flags() & bit != 0
    }

    pub fn can_remove(&self, mask: u32) -> bool {
        if mask & 1 != 0 {
            self.base.removal >> 8 == 0
        } else if mask & 2 != 0 {
            self.base.removal & 255 == 0
        } else {
            true
        }
    }

    /// Typed production query boundary. No bounds, static or default inference.
    pub fn query(&self, operation: u32, argument: u32) -> Option<u32> {
        Some(match operation {
            0 => self.base.occupant_type,
            1 => self.flora_type,
            2 => self.birth,
            3 => self.seeding,
            4 => self.state as i32 as u32,
            5 => u32::from(self.orientation),
            6 => self.highlight(),
            7 => self.flags(),
            8 => u32::from(self.is_flag_set(argument)),
            9 => u32::from(self.can_remove(argument)),
            _ => return None,
        })
    }

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
            6 => return self.can_remove(value),
            // occupant-manager §13.1 slot18, post=false at this boundary.
            7 => {
                if value != 0 {
                    self.base.flags |= 4;
                } else {
                    self.base.flags &= !4;
                }
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
    fn normal_cross_queries_and_visibility_preserve_unrelated_flags() {
        for state in 0..5 {
            for orientation in 0..4 {
                let mut r = OccupantRecord {
                    state,
                    orientation,
                    ..OccupantRecord::default()
                };
                assert_eq!(r.query(4, 0), Some(state as u32));
                assert_eq!(r.query(5, 0), Some(u32::from(orientation)));
                r.mutate(0, 0x181);
                assert_eq!(r.flags(), 0x81);
                r.mutate(7, 1);
                assert_eq!(r.flags(), 0x85);
                assert!(r.is_flag_set(4));
                assert!(!r.is_flag_set(256));
                r.mutate(7, 0);
                assert_eq!(r.flags(), 0x81);
                assert!(!r.is_flag_set(4));
            }
        }
    }

    #[test]
    fn query_removal_precedence_and_highlight_narrowing() {
        let mut r = OccupantRecord::default();
        r.base.removal = 1;
        for (mask, expected) in [(0, true), (1, true), (2, false), (3, true), (4, true)] {
            assert_eq!(r.can_remove(mask), expected);
            assert_eq!(r.query(9, mask), Some(u32::from(expected)));
        }
        r.mutate(3, 0x1e);
        assert_eq!(r.query(6, 0), Some(14));
        r.mutate(3, 0x100);
        assert_eq!(r.query(6, 0), Some(0));
        assert_eq!(r.query(99, 0), None);
    }

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
