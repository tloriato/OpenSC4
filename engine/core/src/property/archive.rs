// SPDX-License-Identifier: GPL-3.0-or-later
//! Saved exemplar properties without resolving the parent against installed data.
//! foundations §10.3: only the parent's key belongs to the archive.

use super::{PropertyHolder, HOLDER_VERSION};
use crate::stream::{InStream, OutStream, ResKey};

#[derive(Clone, Debug, Default)]
pub struct ExemplarRecord {
    pub own: PropertyHolder,
    pub parent: ResKey,
}

impl ExemplarRecord {
    pub fn read(r: &mut dyn InStream) -> Option<Self> {
        if r.get_u16()? != HOLDER_VERSION {
            return None;
        }
        let mut own = PropertyHolder::new();
        if !own.read(r) {
            return None;
        }
        let parent = ResKey {
            group: r.get_u32()?,
            type_id: r.get_u32()?,
            instance: r.get_u32()?,
        };
        Some(Self { own, parent })
    }

    pub fn write(&self, w: &mut dyn OutStream) -> bool {
        w.put_u16(HOLDER_VERSION)
            && self.own.write(w)
            && w.put_u32(self.parent.group)
            && w.put_u32(self.parent.type_id)
            && w.put_u32(self.parent.instance)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::{ByteReader, ByteWriter};
    use crate::variant::{Elem, Variant};

    #[test]
    fn parent_key_and_own_values_survive_without_resource_resolution() {
        let mut record = ExemplarRecord {
            parent: ResKey {
                group: 71,
                type_id: 82,
                instance: 93,
            },
            ..ExemplarRecord::default()
        };
        record
            .own
            .add(17, Variant::Scalar(Elem::F32(f32::from_bits(0x7fc0_1234))));
        let mut w = ByteWriter::new();
        assert!(record.write(&mut w));
        let bytes = w.into_bytes();
        let mut r = ByteReader::new(&bytes);
        let mut decoded = ExemplarRecord::read(&mut r).unwrap();
        assert_eq!(r.remaining(), 0);
        let mut rewritten = ByteWriter::new();
        assert!(decoded.write(&mut rewritten));
        assert_eq!(rewritten.bytes(), bytes);
        decoded.parent.instance += 1;
        let mut changed = ByteWriter::new();
        assert!(decoded.write(&mut changed));
        assert_ne!(changed.bytes(), bytes);
        for end in 0..bytes.len() {
            assert!(ExemplarRecord::read(&mut ByteReader::new(&bytes[..end])).is_none());
        }
    }
}
