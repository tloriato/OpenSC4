// SPDX-License-Identifier: GPL-3.0-or-later
//! Model maker saved fields (model-maker §4), without live service handles.

use crate::store::archive::{fields::array, ObjectReference};
use crate::stream::{InStream, OutStream};

#[derive(Clone, Debug)]
pub struct ModelMakerRecord {
    pub renderer: ObjectReference,
    pub view: [i32; 8],
    pub word: u32,
    pub components: [ObjectReference; 4],
    pub camera: [f32; 4],
    pub previous_target_view: [i32; 4],
}

impl ModelMakerRecord {
    pub fn read(r: &mut dyn InStream) -> Option<Self> {
        if r.get_u16()? != 2 {
            return None;
        }
        Some(Self {
            renderer: ObjectReference::read(r)?,
            view: array(r, |r| r.get_i32())?,
            word: r.get_u32()?,
            components: array(r, |r| ObjectReference::read(r))?,
            camera: array(r, |r| r.get_f32())?,
            previous_target_view: array(r, |r| r.get_i32())?,
        })
    }

    pub fn write(&self, w: &mut dyn OutStream) -> bool {
        w.put_u16(2);
        self.renderer.write(w);
        for v in self.view {
            w.put_i32(v);
        }
        w.put_u32(self.word);
        for reference in self.components {
            reference.write(w);
        }
        for v in self.camera {
            w.put_f32(v);
        }
        for v in self.previous_target_view {
            w.put_i32(v);
        }
        w.error() == 0
    }
}
