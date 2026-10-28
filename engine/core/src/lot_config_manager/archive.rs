// SPDX-License-Identifier: GPL-3.0-or-later
//! The three saved manager references (prop-lotconfig §B6). The installed lot
//! catalogue is not saved and must not be queried during an archive load.
use crate::store::archive::ObjectReference;
use crate::stream::{InStream, OutStream};

#[derive(Clone, Debug)]
pub struct LotConfigManagerRecord {
    pub development: ObjectReference,
    pub flora: ObjectReference,
    pub props: ObjectReference,
}

impl LotConfigManagerRecord {
    pub fn read(r: &mut dyn InStream) -> Option<Self> {
        if r.get_u16()? != 1 {
            return None;
        }
        Some(Self {
            development: ObjectReference::read(r)?,
            flora: ObjectReference::read(r)?,
            props: ObjectReference::read(r)?,
        })
    }

    pub fn write(&self, w: &mut dyn OutStream) -> bool {
        w.put_u16(1) && self.development.write(w) && self.flora.write(w) && self.props.write(w)
    }
}
