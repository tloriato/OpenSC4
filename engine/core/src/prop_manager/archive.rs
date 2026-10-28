// SPDX-License-Identifier: GPL-3.0-or-later
//! Saved manager references without acquiring live services (prop-lotconfig §A8).
use crate::store::archive::ObjectReference;
use crate::stream::{InStream, OutStream};

#[derive(Clone, Debug)]
pub struct PropManagerRecord {
    pub occupant_manager: ObjectReference,
    pub lot_manager: ObjectReference,
}

impl PropManagerRecord {
    pub fn read(r: &mut dyn InStream) -> Option<Self> {
        if r.get_u16()? != 1 {
            return None;
        }
        Some(Self {
            occupant_manager: ObjectReference::read(r)?,
            lot_manager: ObjectReference::read(r)?,
        })
    }

    pub fn write(&self, w: &mut dyn OutStream) -> bool {
        w.put_u16(1) && self.occupant_manager.write(w) && self.lot_manager.write(w)
    }
}
