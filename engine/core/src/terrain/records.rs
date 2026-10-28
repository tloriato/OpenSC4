// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The terrain's saved records (terrain.md §13.1). The surface water saves
//! nothing (§13.3): the water is rebuilt from the sea level on load.

use crate::stream::{ByteReader, ByteWriter, InStream, OutStream};

use super::field::{Terrain, TerrainTuning, MAX_CELLS};
use crate::store::archive::ObjectReference;

/// The terrain's class-store payload (§13.2), separate from its height field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerrainRecord {
    pub view: ObjectReference,
}

impl TerrainRecord {
    pub fn read(r: &mut dyn InStream) -> Option<Self> {
        if r.get_u16()? > 2 {
            return None;
        }
        Some(Self {
            view: ObjectReference::read(r)?,
        })
    }

    pub fn write(&self, w: &mut dyn OutStream) -> bool {
        w.put_u16(2) && self.view.write(w)
    }
}

/// The saved terrain-view byte (§13.2); its raw value is preserved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerrainViewRecord {
    pub value: u8,
}

impl TerrainViewRecord {
    pub fn read(r: &mut dyn InStream) -> Option<Self> {
        if r.get_u16()? != 1 {
            return None;
        }
        Some(Self { value: r.get_u8()? })
    }

    pub fn write(&self, w: &mut dyn OutStream) -> bool {
        w.put_u16(1) && w.put_u8(self.value)
    }
}

/// Surface-water class payload (§13.3); water arrays are rebuilt, not saved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SurfaceWaterRecord {
    pub terrain: ObjectReference,
}

impl SurfaceWaterRecord {
    pub fn read(r: &mut dyn InStream) -> Option<Self> {
        if r.get_u16()? != 1 {
            return None;
        }
        Some(Self {
            terrain: ObjectReference::read(r)?,
        })
    }

    pub fn write(&self, w: &mut dyn OutStream) -> bool {
        w.put_u16(1) && self.terrain.write(w)
    }
}

/// The misc record's version (`0x8A91E7E3 / 0xE98F9525 / 3`).
pub const MISC_VERSION: u16 = 2;
/// The altitude record's version (`0xA9DD6FF4 / 0xE98F9525 / 1`).
pub const ALTITUDES_VERSION: u16 = 2;
/// The vertex property record's version (`0x8A91E7E0 / 0xE98F9525 / 2`).
pub const PROPERTIES_VERSION: u16 = 1;
/// The edge records' version (`0xA9DD6FF4 / 0xE98F9525 / 0x64 + side`).
pub const EDGE_VERSION: u16 = 1;

/// Why a record did not load.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordError {
    /// The payload ended early.
    Truncated,
    /// [choice] A version above the one this code writes.
    Version(u16),
    /// The vertex counts are not usable (fewer than 2 or more than
    /// `MAX_CELLS + 1` per side).
    Size,
}

fn trunc<T>(v: Option<T>) -> Result<T, RecordError> {
    v.ok_or(RecordError::Truncated)
}

fn version(r: &mut ByteReader<'_>, max: u16) -> Result<u16, RecordError> {
    let v = trunc(r.get_u16())?;
    if v > max {
        return Err(RecordError::Version(v));
    }
    Ok(v)
}

impl Terrain {
    /// The misc record (26 bytes): u16 2, u32 `vertsX`, u32 `vertsZ`, i32
    /// terrain type, f32 cached minimum, f32 cached maximum, f32 `+0x78`.
    pub fn write_misc(&self) -> Vec<u8> {
        let mut w = ByteWriter::new();
        w.put_u16(MISC_VERSION);
        w.put_u32(self.verts_x);
        w.put_u32(self.verts_z);
        w.put_i32(self.terrain_type);
        w.put_f32(self.min_cache);
        w.put_f32(self.max_cache);
        w.put_f32(self.saved_78);
        w.into_bytes()
    }

    /// The altitude record: u16 2, then every altitude in index order.
    pub fn write_altitudes(&self) -> Vec<u8> {
        let mut w = ByteWriter::new();
        w.put_u16(ALTITUDES_VERSION);
        for &a in &self.alt {
            w.put_f32(a);
        }
        w.into_bytes()
    }

    /// The vertex property record: u16 1, then one u16 flag word per vertex.
    pub fn write_properties(&self) -> Vec<u8> {
        let mut w = ByteWriter::new();
        w.put_u16(PROPERTIES_VERSION);
        for &f in &self.flags {
            w.put_u16(f);
        }
        w.into_bytes()
    }

    /// The altitudes along one edge (`side` 0 = the column `x = 0`, 1 = the
    /// row `z = 0`, 2 = the column `x = cellsX`, 3 = the row `z = cellsZ`),
    /// [choice] in increasing z or x.
    pub fn edge_altitudes(&self, side: u32) -> Option<Vec<f32>> {
        let (cx, cz) = (self.cells_x as i32, self.cells_z as i32);
        Some(match side {
            0 => (0..=cz).map(|z| self.h(0, z)).collect(),
            1 => (0..=cx).map(|x| self.h(x, 0)).collect(),
            2 => (0..=cz).map(|z| self.h(cx, z)).collect(),
            3 => (0..=cx).map(|x| self.h(x, cz)).collect(),
            _ => return None,
        })
    }

    /// An edge record: u16 1, u32 `n`, `n` f32.
    pub fn write_edge(&self, side: u32) -> Option<Vec<u8>> {
        let e = self.edge_altitudes(side)?;
        Some(Self::write_edge_altitudes(&e))
    }

    /// An independently loaded edge can differ from the current field. Preserve
    /// that saved edge rather than deriving it from the interior on round trip.
    pub fn write_edge_altitudes(e: &[f32]) -> Vec<u8> {
        let mut w = ByteWriter::new();
        w.put_u16(EDGE_VERSION);
        w.put_u32(e.len() as u32);
        for &a in e {
            w.put_f32(a);
        }
        w.into_bytes()
    }

    /// Reads an edge record's altitudes.
    pub fn read_edge(bytes: &[u8]) -> Result<Vec<f32>, RecordError> {
        let mut r = ByteReader::new(bytes);
        version(&mut r, EDGE_VERSION)?;
        let n = trunc(r.get_u32())?;
        if n as usize > r.remaining() / 4 {
            return Err(RecordError::Truncated);
        }
        (0..n).map(|_| trunc(r.get_f32())).collect()
    }

    /// `Load` (slot 96): the misc, altitude and vertex property records.
    /// The vertex counts come from the misc record (not checked against the
    /// city); the cached minimum and maximum are taken as saved, stale or not
    /// (§4.6); a misc version below 2 with a terrain type ≥ 1 loads type 0.
    /// The water is rebuilt from the sea level (§10.1, §13.3).
    ///
    /// No record holds the sea level (TR10, quirk 15): `Init` sets it from
    /// the tuning's new-terrain sea level (clamped to at least the minimum
    /// height) and `Load` never writes it, so a level set by a script before
    /// the save is lost. [choice] The terrain is not view-ready until
    /// `post_city_init`; trailing bytes are ignored.
    pub fn load(
        misc: &[u8],
        altitudes: &[u8],
        properties: &[u8],
        tuning: TerrainTuning,
    ) -> Result<Terrain, RecordError> {
        let mut r = ByteReader::new(misc);
        let v = version(&mut r, MISC_VERSION)?;
        let vx = trunc(r.get_u32())?;
        let vz = trunc(r.get_u32())?;
        let mut ty = trunc(r.get_i32())?;
        let lo = trunc(r.get_f32())?;
        let hi = trunc(r.get_f32())?;
        let s78 = trunc(r.get_f32())?;
        if v < 2 && ty >= 1 {
            ty = 0;
        }
        if !(2..=MAX_CELLS + 1).contains(&vx) || !(2..=MAX_CELLS + 1).contains(&vz) {
            return Err(RecordError::Size);
        }
        let mut t = Terrain::blank(vx - 1, vz - 1, tuning, 0.0).ok_or(RecordError::Size)?;
        t.terrain_type = ty;
        t.min_cache = lo;
        t.max_cache = hi;
        t.saved_78 = s78;

        let mut r = ByteReader::new(altitudes);
        version(&mut r, ALTITUDES_VERSION)?;
        for a in t.alt.iter_mut() {
            *a = trunc(r.get_f32())?;
        }
        let mut r = ByteReader::new(properties);
        version(&mut r, PROPERTIES_VERSION)?;
        for f in t.flags.iter_mut() {
            *f = trunc(r.get_u16())?;
        }
        t.sea_level = tuning.new_sea_level.max(tuning.min_height);
        t.reset_water_to_sea_level();
        Ok(t)
    }
}
