// SPDX-License-Identifier: GPL-3.0-or-later
//! Park and zoo lists, parks §7. References retain their saved identities.
use crate::store::archive::{fields::references, ObjectReference};
use crate::stream::{ByteReader, InStream, OutStream};

#[derive(Clone, Debug)]
pub struct ParksRecord {
    pub parks: Vec<ObjectReference>,
    pub zoos: Vec<ObjectReference>,
}

impl ParksRecord {
    pub fn read(r: &mut ByteReader<'_>) -> Option<Self> {
        if r.get_u16()? != 2 {
            return None;
        }
        Some(Self {
            parks: references(r)?,
            zoos: references(r)?,
        })
    }

    pub fn write(&self, w: &mut dyn OutStream) -> bool {
        w.put_u16(2);
        for list in [&self.parks, &self.zoos] {
            w.put_u32(list.len() as u32);
            for reference in list {
                reference.write(w);
            }
        }
        w.error() == 0
    }
}
