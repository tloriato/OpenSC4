// SPDX-License-Identifier: GPL-3.0-or-later
//! Standalone class-store framing, foundations §3.3–§3.5. Object IDs remain
//! unchanged: payload codecs can therefore preserve references to other owners.

use super::{crc32_mpeg2, CRC_LIMIT, MAX_OBJECTS_PER_CLASS};
use crate::stream::{ByteReader, ByteWriter, InStream, OutStream};
use std::collections::BTreeSet;

pub mod references;
pub mod fields;

/// A saved identity, without constructing or renumbering the referenced owner.
/// The null encoding omits the class word (foundations §2.7).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ObjectReference {
    pub id: u32,
    pub class: u32,
}

impl ObjectReference {
    pub fn read(r: &mut dyn InStream) -> Option<Self> {
        let id = r.get_u32()?;
        let class = if id == 0 { 0 } else { r.get_u32()? };
        Some(Self { id, class })
    }

    pub fn write(self, w: &mut dyn OutStream) -> bool {
        w.put_u32(self.id) && (self.id == 0 || w.put_u32(self.class))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClassIndex {
    pub entries_written: i32,
    pub classes: Vec<(u32, u32)>,
}

impl ClassIndex {
    pub fn read(bytes: &[u8]) -> Result<Self, String> {
        let mut r = ByteReader::new(bytes);
        if r.get_u32() != Some(1) {
            return Err("unsupported class-index version".into());
        }
        let count = r.get_u32().ok_or("truncated class index")? as usize;
        let entries_written = r.get_i32().ok_or("truncated class index")?;
        if count > r.remaining() / 8 {
            return Err("truncated class-index pairs".into());
        }
        let mut classes = Vec::with_capacity(count);
        for _ in 0..count {
            let class = r.get_u32().ok_or("truncated class ID")?;
            let objects = r.get_u32().ok_or("truncated object count")?;
            if objects > MAX_OBJECTS_PER_CLASS || classes.last().is_some_and(|&(prev, _)| prev >= class) {
                return Err("invalid class-index order or count".into());
            }
            classes.push((class, objects));
        }
        if r.remaining() != 0 {
            return Err("trailing class-index bytes".into());
        }
        Ok(Self {
            entries_written,
            classes,
        })
    }

    pub fn write(&self) -> Vec<u8> {
        let mut w = ByteWriter::new();
        w.put_u32(1);
        w.put_u32(self.classes.len() as u32);
        w.put_i32(self.entries_written);
        for &(class, count) in &self.classes {
            w.put_u32(class);
            w.put_u32(count);
        }
        w.into_bytes()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredObject {
    pub id: u32,
    pub payload: Vec<u8>,
}

/// Parses the complete record, checking every entry boundary and checksum.
pub fn read_class(bytes: &[u8]) -> Result<Vec<StoredObject>, String> {
    let mut r = ByteReader::new(bytes);
    let mut objects = Vec::new();
    let mut ids = BTreeSet::new();
    while r.remaining() != 0 {
        let offset = bytes.len() - r.remaining();
        let size = r.get_u32().ok_or("truncated entry size")? as usize;
        let crc = r.get_u32().ok_or("truncated entry CRC")?;
        let entry = bytes
            .get(offset..)
            .and_then(|tail| tail.get(..size))
            .filter(|entry| entry.len() >= 12)
            .ok_or_else(|| format!("invalid entry size at {offset}"))?;
        if crc32_mpeg2(&entry[8..8 + (size - 8).min(CRC_LIMIT)]) != crc {
            return Err(format!("entry CRC mismatch at {offset}"));
        }
        let id = r.get_u32().ok_or("truncated object ID")?;
        if id == 0 || !ids.insert(id) {
            return Err(format!("zero or duplicate object ID at {offset}"));
        }
        let payload = r.get_bytes(size - 12).ok_or("truncated payload")?;
        objects.push(StoredObject { id, payload });
        if objects.len() > MAX_OBJECTS_PER_CLASS as usize {
            return Err("too many objects in class".into());
        }
    }
    Ok(objects)
}

/// Rebuilds entry sizes and CRCs rather than retaining the input framing.
pub fn write_class(objects: &[StoredObject]) -> Result<Vec<u8>, String> {
    let mut w = ByteWriter::new();
    for object in objects {
        let size = object.payload.len().checked_add(12).and_then(|n| u32::try_from(n).ok());
        let size = size.ok_or("class entry exceeds u32 length")?;
        let start = w.len();
        w.put_u32(size);
        w.put_u32(0);
        w.put_u32(object.id);
        w.put_bytes(&object.payload);
        let crc = crc32_mpeg2(&w.bytes()[start + 8..start + 8 + (size as usize - 8).min(CRC_LIMIT)]);
        w.patch_u32(start + 4, crc);
    }
    Ok(w.into_bytes())
}
