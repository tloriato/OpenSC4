// SPDX-License-Identifier: GPL-3.0-or-later
//! Prop developer's saved state, prop-developer §7. Keep reference-list order,
//! the scanner byte and saved manager identities without initializing services.

use crate::store::archive::{
    fields::{array, count, references},
    ObjectReference,
};
use crate::stream::{ByteReader, InStream, OutStream};

#[derive(Clone, Debug)]
pub struct PropDeveloperRecord {
    pub version: u16,
    pub scan_flag: i8,
    pub scan_state: u8,
    pub scanner: [i32; 7],
    pub wealth_added: u8,
    pub requesters: Vec<ObjectReference>,
    pub managers: [ObjectReference; 3],
    pub night_counters: [u32; 4],
    pub is_night: u8,
    pub night: Vec<ObjectReference>,
    pub triggers: Vec<(u32, Vec<ObjectReference>)>,
    pub time_of_day: Vec<ObjectReference>,
    pub dated: Vec<ObjectReference>,
    pub power: Vec<ObjectReference>,
    pub signs: Option<(u8, Vec<ObjectReference>)>,
}

impl PropDeveloperRecord {
    pub fn read(r: &mut ByteReader<'_>) -> Option<Self> {
        let version = r.get_u16()?;
        if version < 4 {
            return None;
        }
        let scan_flag = r.get_i8()?;
        let scan_state = r.get_u8()?;
        let scanner = array(r, |r| r.get_i32())?;
        let wealth_added = r.get_u8()?;
        let requesters = references(r)?;
        let managers = array(r, ObjectReference::read)?;
        let night_counters = array(r, |r| r.get_u32())?;
        let is_night = r.get_u8()?;
        let night = references(r)?;
        let n = count(r, 8)?;
        let mut triggers = Vec::with_capacity(n);
        for _ in 0..n {
            triggers.push((r.get_u32()?, references(r)?));
        }
        let time_of_day = references(r)?;
        let dated = references(r)?;
        let power = references(r)?;
        let signs = if version >= 5 {
            Some((r.get_u8()?, references(r)?))
        } else {
            None
        };
        Some(Self {
            version,
            scan_flag,
            scan_state,
            scanner,
            wealth_added,
            requesters,
            managers,
            night_counters,
            is_night,
            night,
            triggers,
            time_of_day,
            dated,
            power,
            signs,
        })
    }

    pub fn write(&self, w: &mut dyn OutStream) -> bool {
        w.put_u16(self.version);
        w.put_i8(self.scan_flag);
        w.put_u8(self.scan_state);
        for v in self.scanner {
            w.put_i32(v);
        }
        w.put_u8(self.wealth_added);
        put_references(w, &self.requesters);
        for reference in self.managers {
            reference.write(w);
        }
        for v in self.night_counters {
            w.put_u32(v);
        }
        w.put_u8(self.is_night);
        put_references(w, &self.night);
        w.put_u32(self.triggers.len() as u32);
        for (trigger, refs) in &self.triggers {
            w.put_u32(*trigger);
            put_references(w, refs);
        }
        put_references(w, &self.time_of_day);
        put_references(w, &self.dated);
        put_references(w, &self.power);
        if let Some((visible, signs)) = &self.signs {
            w.put_u8(*visible);
            put_references(w, signs);
        }
        w.error() == 0
    }
}

fn put_references(w: &mut dyn OutStream, refs: &[ObjectReference]) {
    w.put_u32(refs.len() as u32);
    for reference in refs {
        reference.write(w);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::ByteWriter;

    #[test]
    fn archive_keeps_legacy_scanner_manager_ids_and_list_aliases() {
        let prop = ObjectReference {
            id: 17,
            class: 0x2977_AA47,
        };
        for version in [4, 5] {
            let source = PropDeveloperRecord {
                version,
                scan_flag: -17,
                scan_state: 255,
                scanner: [-1, 8, 2, 0, 0, 15, 15],
                wealth_added: 1,
                requesters: vec![prop, ObjectReference::default()],
                managers: [prop; 3],
                night_counters: [12, 2, 0xBAAD_F00D, 184],
                is_night: 0,
                night: vec![prop, prop],
                triggers: vec![(u32::MAX, vec![prop])],
                time_of_day: vec![prop],
                dated: vec![prop],
                power: vec![prop],
                signs: (version >= 5).then(|| (1, vec![prop])),
            };
            let mut w = ByteWriter::new();
            assert!(source.write(&mut w));
            let bytes = w.into_bytes();
            let mut r = ByteReader::new(&bytes);
            let mut loaded = PropDeveloperRecord::read(&mut r).unwrap();
            assert_eq!(r.remaining(), 0);
            assert_eq!(loaded.scan_state, 255);
            assert_eq!(loaded.managers, [prop; 3]);
            assert_eq!(loaded.night, vec![prop, prop]);
            let mut w = ByteWriter::new();
            assert!(loaded.write(&mut w));
            assert_eq!(w.into_bytes(), bytes);
            loaded.scan_state = 3;
            let mut w = ByteWriter::new();
            assert!(loaded.write(&mut w));
            let mut expected = bytes.clone();
            expected[3] = 3;
            assert_eq!(w.into_bytes(), expected);
            for end in 0..bytes.len() {
                assert!(PropDeveloperRecord::read(&mut ByteReader::new(&bytes[..end])).is_none());
            }
        }
    }
}
