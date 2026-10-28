// SPDX-License-Identifier: GPL-3.0-or-later
//! Network manager archives, network-manager §4. Saved configuration IDs and
//! hash traversal order survive without resolving configurations during Init.
use crate::store::archive::{
    fields::{count, references},
    ObjectReference,
};
use crate::stream::{ByteReader, InStream, OutStream};

#[derive(Clone, Debug)]
pub struct SavedNetworkLot {
    pub key: u16,
    pub occupant: ObjectReference,
    pub configuration: u32,
}

#[derive(Clone, Debug)]
pub enum NetworkManagerArchive {
    Managed(Vec<ObjectReference>),
    Lots {
        lots: Vec<SavedNetworkLot>,
        potholes: Vec<(u16, ObjectReference)>,
    },
}

impl NetworkManagerArchive {
    pub fn read(class: u32, r: &mut ByteReader<'_>) -> Option<Self> {
        match class {
            0xC990_BD46 => {
                if r.get_u16()? != 1 {
                    return None;
                }
                Some(Self::Managed(references(r)?))
            }
            0xE9A6_CD3B => {
                if r.get_u16()? != 2 {
                    return None;
                }
                let n = count(r, 10)?;
                let mut lots = Vec::with_capacity(n);
                for _ in 0..n {
                    lots.push(SavedNetworkLot {
                        key: r.get_u16()?,
                        occupant: ObjectReference::read(r)?,
                        configuration: r.get_u32()?,
                    });
                }
                let n = count(r, 6)?;
                let mut potholes = Vec::with_capacity(n);
                for _ in 0..n {
                    potholes.push((r.get_u16()?, ObjectReference::read(r)?));
                }
                Some(Self::Lots { lots, potholes })
            }
            _ => None,
        }
    }

    pub fn write(&self, w: &mut dyn OutStream) -> bool {
        match self {
            Self::Managed(references) => {
                w.put_u16(1);
                w.put_u32(references.len() as u32);
                for reference in references {
                    reference.write(w);
                }
            }
            Self::Lots { lots, potholes } => {
                w.put_u16(2);
                w.put_u32(lots.len() as u32);
                for lot in lots {
                    w.put_u16(lot.key);
                    lot.occupant.write(w);
                    w.put_u32(lot.configuration);
                }
                w.put_u32(potholes.len() as u32);
                for (key, reference) in potholes {
                    w.put_u16(*key);
                    reference.write(w);
                }
            }
        }
        w.error() == 0
    }
}
