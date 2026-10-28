// SPDX-License-Identifier: GPL-3.0-or-later
//! Dispatch manager archive, dispatch §7. The active/available pylons and
//! tool type survive here; live Init intentionally drops/resets these fields.
use crate::store::archive::{
    fields::{count, references},
    ObjectReference,
};
use crate::stream::{ByteReader, InStream, OutStream};

#[derive(Clone, Debug)]
pub struct SavedDispatch {
    pub kind: i32,
    pub id: u32,
    pub call_id: u32,
    pub active: u8,
    pub target: [f32; 3],
    pub station: ObjectReference,
    pub automaton: ObjectReference,
    pub pylon: ObjectReference,
}

#[derive(Clone, Debug)]
pub struct DispatchManagerRecord {
    pub dispatches: Vec<SavedDispatch>,
    pub next_id: u32,
    pub active_pylon: ObjectReference,
    pub available: Vec<ObjectReference>,
    pub tool_type: u32,
}

impl DispatchManagerRecord {
    pub fn read(r: &mut ByteReader<'_>) -> Option<Self> {
        if r.get_u16()? != 2 {
            return None;
        }
        let n = count(r, 37)?;
        let mut dispatches = Vec::with_capacity(n);
        for _ in 0..n {
            dispatches.push(SavedDispatch {
                kind: r.get_i32()?,
                id: r.get_u32()?,
                call_id: r.get_u32()?,
                active: r.get_u8()?,
                target: [r.get_f32()?, r.get_f32()?, r.get_f32()?],
                station: ObjectReference::read(r)?,
                automaton: ObjectReference::read(r)?,
                pylon: ObjectReference::read(r)?,
            });
        }
        Some(Self {
            dispatches,
            next_id: r.get_u32()?,
            active_pylon: ObjectReference::read(r)?,
            available: references(r)?,
            tool_type: r.get_u32()?,
        })
    }

    pub fn write(&self, w: &mut dyn OutStream) -> bool {
        w.put_u16(2);
        w.put_u32(self.dispatches.len() as u32);
        for dispatch in &self.dispatches {
            w.put_i32(dispatch.kind);
            w.put_u32(dispatch.id);
            w.put_u32(dispatch.call_id);
            w.put_u8(dispatch.active);
            for coordinate in dispatch.target {
                w.put_f32(coordinate);
            }
            dispatch.station.write(w);
            dispatch.automaton.write(w);
            dispatch.pylon.write(w);
        }
        w.put_u32(self.next_id);
        self.active_pylon.write(w);
        w.put_u32(self.available.len() as u32);
        for reference in &self.available {
            reference.write(w);
        }
        w.put_u32(self.tool_type);
        w.error() == 0
    }
}
