// SPDX-License-Identifier: GPL-3.0-or-later
//! Night-light deque and cursor, lot-developer §13, before any Init resets.
use crate::store::archive::{fields::references, ObjectReference};
use crate::stream::{ByteReader, InStream, OutStream};

#[derive(Clone, Debug)]
pub struct LotDeveloperRecord {
    pub verts_x: i32,
    pub extent: [f32; 2],
    pub night: Vec<ObjectReference>,
    pub cursor: u32,
    pub mode: u8,
    pub lit: u8,
}

impl LotDeveloperRecord {
    pub fn read(r: &mut ByteReader<'_>) -> Option<Self> {
        if r.get_u16()? != 1 {
            return None;
        }
        Some(Self {
            verts_x: r.get_i32()?,
            extent: [r.get_f32()?, r.get_f32()?],
            night: references(r)?,
            cursor: r.get_u32()?,
            mode: r.get_u8()?,
            lit: r.get_u8()?,
        })
    }

    pub fn write(&self, w: &mut dyn OutStream) -> bool {
        w.put_u16(1);
        w.put_i32(self.verts_x);
        for extent in self.extent {
            w.put_f32(extent);
        }
        w.put_u32(self.night.len() as u32);
        for reference in &self.night {
            reference.write(w);
        }
        w.put_u32(self.cursor);
        w.put_u8(self.mode);
        w.put_u8(self.lit);
        w.error() == 0
    }
}
