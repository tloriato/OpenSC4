// SPDX-License-Identifier: GPL-3.0-or-later
//! View3D's non-class record (view3d-window §6k; auto-records §2).
//! Archive identity and trigger order survive without constructing a UI window.

use crate::store::archive::{
    fields::{array, count},
    ObjectReference,
};
use crate::stream::{ByteReader, InStream, OutStream};

#[derive(Clone, Debug)]
pub struct View3DArchive {
    pub version: u16,
    pub references: [ObjectReference; 4],
    pub triggers: Vec<(u32, u32)>,
}

impl View3DArchive {
    pub fn read(r: &mut ByteReader<'_>) -> Option<Self> {
        let version = r.get_u16()?;
        if version >= 5 {
            return None;
        }
        let references = array(r, |r| ObjectReference::read(r))?;
        let n = count(r, 8)?;
        let mut triggers = Vec::with_capacity(n);
        for _ in 0..n {
            triggers.push((r.get_u32()?, r.get_u32()?));
        }
        Some(Self {
            version,
            references,
            triggers,
        })
    }

    pub fn write(&self, w: &mut dyn OutStream) -> bool {
        w.put_u16(self.version);
        for reference in self.references {
            reference.write(w);
        }
        w.put_u32(self.triggers.len() as u32);
        for (trigger, count) in &self.triggers {
            w.put_u32(*trigger);
            w.put_u32(*count);
        }
        w.error() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::ByteWriter;

    #[test]
    fn trigger_duplicates_keep_stream_order_and_saved_object_ids() {
        let mut bytes = vec![4, 0];
        for (id, class) in [
            (3u32, 0xC9C6_28ECu32),
            (4, 0xE9C6_22D8),
            (5, 0xA9C7_41B3),
            (6, 0x2990_C1E5),
        ] {
            bytes.extend_from_slice(&id.to_le_bytes());
            bytes.extend_from_slice(&class.to_le_bytes());
        }
        for word in [3u32, 12, 3, 4, 1, 12, 7] {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
        let mut record = View3DArchive::read(&mut ByteReader::new(&bytes)).unwrap();
        assert_eq!(record.triggers, [(12, 3), (4, 1), (12, 7)]);
        assert_eq!(record.references[3].id, 6);
        let mut w = ByteWriter::new();
        assert!(record.write(&mut w));
        assert_eq!(w.into_bytes(), bytes);
        record.triggers[1].1 = 99;
        let mut w = ByteWriter::new();
        assert!(record.write(&mut w));
        let changed = w.into_bytes();
        assert_eq!(
            View3DArchive::read(&mut ByteReader::new(&changed)).unwrap().triggers[1],
            (4, 99)
        );
        assert!(View3DArchive::read(&mut ByteReader::new(&bytes[..bytes.len() - 1])).is_none());
    }
}
