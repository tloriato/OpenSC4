// SPDX-License-Identifier: GPL-3.0-or-later
//! Demolition §7: only a version word is persisted, never the utility's cache.
use crate::stream::{InStream, OutStream};

#[derive(Clone, Debug)]
pub struct DemolitionRecord;

impl DemolitionRecord {
    pub fn read(r: &mut dyn InStream) -> Option<Self> {
        (r.get_u16()? == 1).then_some(Self)
    }

    pub fn write(&self, w: &mut dyn OutStream) -> bool {
        w.put_u16(1)
    }
}
