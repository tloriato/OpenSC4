// SPDX-License-Identifier: GPL-3.0-or-later
//! Saved renderer viewport and camera identity (render3d §2).

use crate::store::archive::{fields::array, ObjectReference};
use crate::stream::{InStream, OutStream};

#[derive(Clone, Debug)]
pub struct RendererRecord {
    pub viewport: [i32; 6],
    pub camera: ObjectReference,
    pub camera_state: [u32; 2],
    pub mode: i32,
}

impl RendererRecord {
    pub fn read(r: &mut dyn InStream) -> Option<Self> {
        if r.get_u16()? != 2 {
            return None;
        }
        Some(Self {
            viewport: array(r, |r| r.get_i32())?,
            camera: ObjectReference::read(r)?,
            camera_state: array(r, |r| r.get_u32())?,
            mode: r.get_i32()?,
        })
    }

    pub fn write(&self, w: &mut dyn OutStream) -> bool {
        w.put_u16(2);
        for v in self.viewport {
            w.put_i32(v);
        }
        self.camera.write(w);
        for v in self.camera_state {
            w.put_u32(v);
        }
        w.put_i32(self.mode);
        w.error() == 0
    }
}
