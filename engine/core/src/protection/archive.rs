// SPDX-License-Identifier: GPL-3.0-or-later
//! Fire and police persisted state (protection §11). The outer and base
//! versions are independent; older percentages remain in their saved units.

use crate::grid::CellIterator;
use crate::store::archive::{
    fields::{array, references},
    ObjectReference,
};
use crate::stream::{ByteReader, InStream, OutStream};

#[derive(Clone, Debug)]
pub struct BaseRecord {
    pub version: u16,
    pub department: ObjectReference,
    pub iterator: CellIterator,
    pub grids: [ObjectReference; 2],
    pub stations: Vec<ObjectReference>,
    pub counts: [i32; 3],
    pub average: f32,
    pub funding: f32,
    pub strike: u8,
    pub strike_date: u32,
    pub next_call: u32,
}

impl BaseRecord {
    fn read(r: &mut ByteReader<'_>) -> Option<Self> {
        let version = r.get_u16()?;
        if version > 3 {
            return None;
        }
        Some(Self {
            version,
            department: ObjectReference::read(r)?,
            iterator: CellIterator::read(r)?,
            grids: array(r, |r| ObjectReference::read(r))?,
            stations: references(r)?,
            counts: array(r, |r| r.get_i32())?,
            average: r.get_f32()?,
            funding: r.get_f32()?,
            strike: r.get_u8()?,
            strike_date: r.get_u32()?,
            next_call: r.get_u32()?,
        })
    }

    fn write(&self, w: &mut dyn OutStream) {
        w.put_u16(self.version);
        self.department.write(w);
        self.iterator.write(w);
        for reference in self.grids {
            reference.write(w);
        }
        w.put_u32(self.stations.len() as u32);
        for reference in &self.stations {
            reference.write(w);
        }
        for v in self.counts {
            w.put_i32(v);
        }
        w.put_f32(self.average);
        w.put_f32(self.funding);
        w.put_u8(self.strike);
        w.put_u32(self.strike_date);
        w.put_u32(self.next_call);
    }
}

#[derive(Clone, Debug)]
pub struct PoliceRecord {
    pub jail_department: ObjectReference,
    pub local_jails: Vec<ObjectReference>,
    pub jails: Vec<ObjectReference>,
    pub counts: [i32; 8],
}

#[derive(Clone, Debug)]
pub struct ProtectionRecord {
    pub version: u16,
    pub base: BaseRecord,
    pub police: Option<PoliceRecord>,
    pub dispatch_source: Option<ObjectReference>,
}

impl ProtectionRecord {
    pub fn read(class: u32, r: &mut ByteReader<'_>) -> Option<Self> {
        let police = match class {
            0x2990_C05A => false,
            0x6990_C04F => true,
            _ => return None,
        };
        let version = r.get_u16()?;
        if version > if police { 8 } else { 3 } {
            return None;
        }
        let base = BaseRecord::read(r)?;
        let police = if police {
            Some(PoliceRecord {
                jail_department: ObjectReference::read(r)?,
                local_jails: references(r)?,
                jails: references(r)?,
                counts: array(r, |r| r.get_i32())?,
            })
        } else {
            None
        };
        let dispatch_source = if version > if police.is_some() { 7 } else { 2 } {
            Some(ObjectReference::read(r)?)
        } else {
            None
        };
        Some(Self {
            version,
            base,
            police,
            dispatch_source,
        })
    }

    pub fn write(&self, w: &mut dyn OutStream) -> bool {
        w.put_u16(self.version);
        self.base.write(w);
        if let Some(police) = &self.police {
            police.jail_department.write(w);
            w.put_u32(police.local_jails.len() as u32);
            for reference in &police.local_jails {
                reference.write(w);
            }
            w.put_u32(police.jails.len() as u32);
            for reference in &police.jails {
                reference.write(w);
            }
            for v in police.counts {
                w.put_i32(v);
            }
        }
        if let Some(source) = self.dispatch_source {
            source.write(w);
        }
        w.error() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::ByteWriter;

    #[test]
    fn outer_and_base_versions_and_repeated_station_ids_are_independent() {
        let department = ObjectReference {
            id: 73,
            class: 0xE990_BFFC,
        };
        let original = ProtectionRecord {
            version: 3,
            base: BaseRecord {
                version: 2,
                department,
                iterator: CellIterator::default(),
                grids: [ObjectReference::default(); 2],
                stations: vec![department, ObjectReference::default(), department],
                counts: [-1, 5, 3],
                average: 0.5,
                funding: 75.0,
                strike: 0xFF,
                strike_date: 12,
                next_call: 13,
            },
            police: None,
            dispatch_source: Some(ObjectReference {
                id: 99,
                class: 0x4C0A_E114,
            }),
        };
        let mut w = ByteWriter::new();
        assert!(original.write(&mut w));
        let bytes = w.into_bytes();
        let mut r = ByteReader::new(&bytes);
        let parsed = ProtectionRecord::read(0x2990_C05A, &mut r).unwrap();
        assert_eq!(r.remaining(), 0);
        assert_eq!(parsed.base.funding, 75.0);
        assert_eq!(parsed.base.stations, original.base.stations);
        assert_eq!(parsed.dispatch_source, original.dispatch_source);
        let mut w = ByteWriter::new();
        assert!(parsed.write(&mut w));
        assert_eq!(w.into_bytes(), bytes);
        for end in 0..bytes.len() {
            assert!(ProtectionRecord::read(0x2990_C05A, &mut ByteReader::new(&bytes[..end])).is_none());
        }
    }
}
