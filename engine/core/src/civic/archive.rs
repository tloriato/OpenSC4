// SPDX-License-Identifier: GPL-3.0-or-later
//! Civic saved fields, civic-buildings §7. Retain the file's hash-chain order;
//! the live loader's head insertion would reverse each chain on every write.
use crate::store::archive::{
    fields::{count, references},
    ObjectReference,
};
use crate::stream::{ByteReader, InStream, OutStream};

#[derive(Clone, Debug)]
pub struct SavedStatus {
    pub key: u32,
    pub status: u8,
    pub buildings: u16,
    pub tooltip: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct CivicRecord {
    pub exclusions: Vec<u32>,
    pub statuses: Vec<SavedStatus>,
    pub landmarks: ObjectReference,
    pub disease_centers: Vec<ObjectReference>,
    pub maintained: Vec<ObjectReference>,
}

impl CivicRecord {
    pub fn read(r: &mut ByteReader<'_>) -> Option<Self> {
        if r.get_u16()? != 6 {
            return None;
        }
        let n = count(r, 4)?;
        let mut exclusions = Vec::with_capacity(n);
        for _ in 0..n {
            exclusions.push(r.get_u32()?);
        }
        let n = count(r, 11)?;
        let mut statuses = Vec::with_capacity(n);
        for _ in 0..n {
            statuses.push(SavedStatus {
                key: r.get_u32()?,
                status: r.get_u8()?,
                buildings: r.get_u16()?,
                tooltip: r.get_str()?,
            });
        }
        Some(Self {
            exclusions,
            statuses,
            landmarks: ObjectReference::read(r)?,
            disease_centers: references(r)?,
            maintained: references(r)?,
        })
    }

    pub fn write(&self, w: &mut dyn OutStream) -> bool {
        w.put_u16(6);
        w.put_u32(self.exclusions.len() as u32);
        for key in &self.exclusions {
            w.put_u32(*key);
        }
        w.put_u32(self.statuses.len() as u32);
        for status in &self.statuses {
            w.put_u32(status.key);
            w.put_u8(status.status);
            w.put_u16(status.buildings);
            w.put_str(&status.tooltip);
        }
        self.landmarks.write(w);
        for list in [&self.disease_centers, &self.maintained] {
            w.put_u32(list.len() as u32);
            for reference in list {
                reference.write(w);
            }
        }
        w.error() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::ByteWriter;

    #[test]
    fn colliding_status_keys_keep_file_order_and_fields_are_editable() {
        // Both keys have bucket 1 in the specified 193-bucket hash table.
        // Live Read reverses the chain; an archive must retain its order.
        let bytes = [
            6, 0, 0, 0, 0, 0, 2, 0, 0, 0, 194, 0, 0, 0, 3, 2, 0, 1, 0, 0, 0, b'A', 1, 0, 0, 0, 7, 4, 0, 0, 0, 0, 0, 19,
            0, 0, 0, 0xFC, 0xBF, 0x90, 0xE9, 0, 0, 0, 0, 0, 0, 0, 0,
        ];
        let mut r = ByteReader::new(&bytes);
        let mut record = CivicRecord::read(&mut r).unwrap();
        assert_eq!(r.remaining(), 0);
        assert_eq!(record.landmarks.id, 19);
        assert_eq!(record.statuses[0].key, 194);
        let mut w = ByteWriter::new();
        assert!(record.write(&mut w));
        assert_eq!(w.into_bytes(), bytes);
        record.statuses[0].tooltip[0] = b'B';
        let mut w = ByteWriter::new();
        assert!(record.write(&mut w));
        let mut expected = bytes;
        expected[21] = b'B';
        assert_eq!(w.into_bytes(), expected);
    }
}
