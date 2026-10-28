// SPDX-License-Identifier: GPL-3.0-or-later
//! Landfill saved zones/heaps and occupants, landfill §6. Historical zone
//! versions retain their encoding; no mask expansion or altitude lookup runs.
use crate::automata::OccupantBase;
use crate::store::archive::{fields::count, ObjectReference};
use crate::stream::{ByteReader, InStream, OutStream};

#[derive(Clone, Debug)]
pub struct SavedHeap {
    pub cell: [u32; 2],
    pub valid: u16,
    pub active: u16,
    pub contained: u32,
    pub capacity: u32,
}

#[derive(Clone, Debug)]
pub struct SavedZone {
    pub version: u16,
    pub bounds: [i32; 4],
    pub mask: Vec<u8>,
    pub size: [i32; 2],
    pub heaps: Vec<SavedHeap>,
    pub connected: Option<u8>,
    pub zot: Option<ObjectReference>,
}

fn area(width: i64, height: i64) -> Option<usize> {
    usize::try_from(width).ok()?.checked_mul(usize::try_from(height).ok()?)
}

impl SavedZone {
    fn read(r: &mut ByteReader<'_>) -> Option<Self> {
        let version = r.get_u16()?;
        let bounds = [r.get_i32()?, r.get_i32()?, r.get_i32()?, r.get_i32()?];
        let mask_len = area(
            i64::from(bounds[2]) - i64::from(bounds[0]),
            i64::from(bounds[3]) - i64::from(bounds[1]),
        )?;
        let mask = r.get_bytes(mask_len)?;
        let size = [r.get_i32()?, r.get_i32()?];
        let n = area(i64::from(size[0]), i64::from(size[1]))?;
        if n > r.remaining() / 22 {
            return None;
        }
        let mut heaps = Vec::with_capacity(n);
        for _ in 0..n {
            if r.get_u16()? != 2 {
                return None;
            }
            heaps.push(SavedHeap {
                cell: [r.get_u32()?, r.get_u32()?],
                valid: r.get_u16()?,
                active: r.get_u16()?,
                contained: r.get_u32()?,
                capacity: r.get_u32()?,
            });
        }
        let connected = if version >= 3 { Some(r.get_u8()?) } else { None };
        let zot = if version >= 4 {
            Some(ObjectReference::read(r)?)
        } else {
            None
        };
        Some(Self {
            version,
            bounds,
            mask,
            size,
            heaps,
            connected,
            zot,
        })
    }

    fn write(&self, w: &mut dyn OutStream) {
        w.put_u16(self.version);
        for bound in self.bounds {
            w.put_i32(bound);
        }
        w.put_bytes(&self.mask);
        for size in self.size {
            w.put_i32(size);
        }
        for heap in &self.heaps {
            w.put_u16(2);
            for cell in heap.cell {
                w.put_u32(cell);
            }
            w.put_u16(heap.valid);
            w.put_u16(heap.active);
            w.put_u32(heap.contained);
            w.put_u32(heap.capacity);
        }
        if let Some(connected) = self.connected {
            w.put_u8(connected);
        }
        if let Some(zot) = self.zot {
            zot.write(w);
        }
    }
}

#[derive(Clone, Debug)]
pub struct LandfillRecord {
    pub zones: Vec<SavedZone>,
}

impl LandfillRecord {
    pub fn read(r: &mut ByteReader<'_>) -> Option<Self> {
        if r.get_u16()? != 3 {
            return None;
        }
        // The nonnegative signed zone count has the same wire encoding as u32.
        let n = count(r, 26)?;
        let mut zones = Vec::with_capacity(n);
        for _ in 0..n {
            zones.push(SavedZone::read(r)?);
        }
        Some(Self { zones })
    }

    pub fn write(&self, w: &mut dyn OutStream) -> bool {
        w.put_u16(3);
        w.put_u32(self.zones.len() as u32);
        for zone in &self.zones {
            zone.write(w);
        }
        w.error() == 0
    }
}

#[derive(Clone, Debug)]
pub struct LandfillOccupantRecord {
    pub base: OccupantBase,
    pub tile: u32,
    pub orientation: i32,
    pub position: [f32; 3],
    pub empty: u8,
}

impl LandfillOccupantRecord {
    pub fn read(r: &mut dyn InStream) -> Option<Self> {
        if r.get_u16()? != 3 {
            return None;
        }
        Some(Self {
            base: OccupantBase::read(r)?,
            tile: r.get_u32()?,
            orientation: r.get_i32()?,
            position: [r.get_f32()?, r.get_f32()?, r.get_f32()?],
            empty: r.get_u8()?,
        })
    }

    pub fn write(&self, w: &mut dyn OutStream) -> bool {
        w.put_u16(3);
        self.base.write(w);
        w.put_u32(self.tile);
        w.put_i32(self.orientation);
        for coordinate in self.position {
            w.put_f32(coordinate);
        }
        w.put_u8(self.empty);
        w.error() == 0
    }
}
