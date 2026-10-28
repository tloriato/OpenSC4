// SPDX-License-Identifier: GPL-3.0-or-later
//! Standalone commercial payload, commercial-industrial.md §8.1. Referenced
//! grids are owned by the class store, so no running-city host is required.
use crate::store::archive::ObjectReference;
use crate::stream::{InStream, OutStream};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommercialRecord {
    pub dirty: bool,
    pub grids: [ObjectReference; 2],
}

impl CommercialRecord {
    pub fn read(r: &mut dyn InStream) -> Option<Self> {
        if r.get_u16()? != 1 {
            return None;
        }
        Some(Self {
            dirty: r.get_u8()? != 0,
            grids: [ObjectReference::read(r)?, ObjectReference::read(r)?],
        })
    }

    pub fn write(&self, w: &mut dyn OutStream) -> bool {
        w.put_u16(1) && w.put_u8(u8::from(self.dirty)) && self.grids.iter().all(|reference| reference.write(w))
    }
}
