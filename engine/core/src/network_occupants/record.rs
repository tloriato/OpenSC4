// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.
//! The base network-occupant save record (`network-occupants.md` §2.3,
//! `Write` `0x0061A190` v8 / `Read` `0x0061CC00` v ≥ 4).
//!
//! The record runs with `this = object+8`; every field here is named by its
//! **object** offset (§2.2, NO16 c / NO17 b). Implemented per the answers to
//! NO15/NO16: one quad of four 24-byte vertices, the four scalars after it,
//! the cell-info sub-record, 76-byte triangles (alpha forced on read), the
//! interleaved bounding box, `ConvertV5FlagsToCurrentFlags` below v6, the
//! v < 7 / v < 8 read tail (as [`LoadEffects`]) and the resource key + int64
//! in every version. The final render-state alpha fix-up needs the model
//! instance and is [`NetworkBaseRecord::finish_read_alpha`].

use super::xform::Xform;
use crate::automata::OccupantBase;
use crate::stream::{InStream, OutStream, RefValue, ResKey};

/// The version `Write` emits.
pub const NETWORK_RECORD_VERSION: u16 = 8;
/// The oldest version `Read` accepts.
pub const NETWORK_RECORD_MIN_VERSION: u16 = 4;
/// `Read` masks the saved flags with this, every version.
pub const SAVED_FLAGS_MASK: u32 = 0xFFFF_1FFF;
/// The cell-info sub-record's only accepted version.
pub const CELL_INFO_VERSION: u16 = 2;
/// The property the v < 7 tail sets to `(z << 16) + x` (slot 72).
pub const PROP_LEGACY_CELL: u32 = 0x4835_C6AB;
/// The empty property every v < 8 tail adds (slot 7 with 1).
pub const PROP_LEGACY_MARK: u32 = 0x4835_C6AC;

/// One quad/triangle/wall vertex (0x18 bytes in memory): f32 x, y, z
/// `+0/+4/+8`, the little-endian ARGB colour dword `+0xC`, f32 u `+0x10`,
/// v `+0x14`. Stream form: x, y, z, u, v, then R, G, B, A (24 bytes).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vertex {
    pub pos: [f32; 3],
    /// `0xAARRGGBB`.
    pub color: u32,
    pub u: f32,
    pub v: f32,
}

impl Vertex {
    /// The alpha byte (`+0xF`).
    pub fn alpha(&self) -> u8 {
        (self.color >> 24) as u8
    }

    /// Force the alpha byte to `0xFF`.
    pub fn force_alpha(&mut self) {
        self.color |= 0xFF00_0000;
    }

    /// The 24-byte stream form.
    pub fn write(&self, out: &mut dyn OutStream) {
        for &f in &self.pos {
            out.put_f32(f);
        }
        out.put_f32(self.u);
        out.put_f32(self.v);
        let c = self.color;
        out.put_u8((c >> 16) as u8);
        out.put_u8((c >> 8) as u8);
        out.put_u8(c as u8);
        out.put_u8((c >> 24) as u8);
    }

    /// The 24-byte stream form (no alpha forcing).
    pub fn read(inp: &mut dyn InStream) -> Option<Vertex> {
        let pos = [inp.get_f32()?, inp.get_f32()?, inp.get_f32()?];
        let u = inp.get_f32()?;
        let v = inp.get_f32()?;
        let (r, g, b, a) = (inp.get_u8()?, inp.get_u8()?, inp.get_u8()?, inp.get_u8()?);
        let color = (a as u32) << 24 | (r as u32) << 16 | (g as u32) << 8 | b as u32;
        Some(Vertex { pos, color, u, v })
    }
}

/// Field 6: the cell-info block at `+0x108` (helper `0x00619C10` /
/// `0x00619C90`): u16 2, u8 count, u8 `+0x10C`, u32 `+0x110`, then
/// `count` × {u8, u32}.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CellInfo {
    pub byte_10c: u8,
    pub word_110: u32,
    /// The `+0x114` array (its length is the saved u8 count).
    pub entries: Vec<(u8, u32)>,
}

/// One `+0x118` triangle (0x4C bytes): u32 `+0`, three vertices.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Triangle {
    pub word0: u32,
    pub verts: [Vertex; 3],
}

/// The base network record, fields by object offset.
#[derive(Clone, Debug)]
pub struct NetworkBaseRecord {
    /// The version read (written as [`NETWORK_RECORD_VERSION`]).
    pub version: u16,
    pub base: OccupantBase,
    /// `+0x64` transform.
    pub transform: Xform,
    /// `+0x9C…+0xFB`: the road quad.
    pub quad: [Vertex; 4],
    /// `+0xFC`.
    pub word_fc: u32,
    /// `+0x105` rotation.
    pub rotation: u8,
    /// `+0x100` variation.
    pub variation: u32,
    /// `+0x104` orientation + flip bit.
    pub orientation: u8,
    /// `+0x108` cell info.
    pub cell_info: CellInfo,
    /// `+0x118` triangles.
    pub triangles: Vec<Triangle>,
    /// `+0x124/+0x128/+0x12C`.
    pub bbox_min: [f32; 3],
    /// `+0x130/+0x134/+0x138`.
    pub bbox_max: [f32; 3],
    /// `+0x13C`; after a read converted (v < 6) and masked.
    pub network_flags: u32,
    /// `+0x164` path-id override.
    pub path_id: u32,
    /// The two f32 offsets of a v < 7 record with a non-zero path id
    /// (never written by the current writer).
    pub legacy_offsets: Option<[f32; 2]>,
    /// The resource key saved for cached path info `+0x140`, not an object
    /// reference. All-zero means absent. This segment-record codec retains
    /// the key; resolving the live resource belongs to the host (§2.3 field 12).
    pub path_info_key: ResKey,
    /// `+0x170` (low) / `+0x174` (high).
    pub int64: i64,
}

impl Default for NetworkBaseRecord {
    fn default() -> Self {
        NetworkBaseRecord {
            version: NETWORK_RECORD_VERSION,
            base: OccupantBase::default(),
            transform: Xform::default(),
            quad: [Vertex::default(); 4],
            word_fc: 0,
            rotation: 0,
            variation: 0,
            orientation: 0,
            cell_info: CellInfo::default(),
            triangles: Vec::new(),
            bbox_min: [0.0; 3],
            bbox_max: [0.0; 3],
            network_flags: 0,
            path_id: 0,
            legacy_offsets: None,
            path_info_key: ResKey::default(),
            int64: 0,
        }
    }
}

/// What `Read`'s tail does to the live object after field 11 (§2.3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LoadEffects {
    /// v < 7, path id ≠ 0: slot 72 with this cell `(x, z)`, which sets
    /// [`PROP_LEGACY_CELL`] to [`legacy_cell_value`].
    pub legacy_cell: Option<(i32, i32)>,
    /// v < 8, path id ≠ 0: slot 7 with 1 adds [`PROP_LEGACY_MARK`].
    pub add_legacy_mark: bool,
}

/// Slot 72's property value: `(z << 16) + x` (32-bit wrapping).
pub fn legacy_cell_value(x: i32, z: i32) -> u32 {
    ((z as u32) << 16).wrapping_add(x as u32)
}

/// The v < 7 cell of one axis: `floor(((a + b) · 0.5 + d) · 0.0625)`.
/// *[literal, NO19: computed in f64 (x87), rounded to f32, then floored; the
/// spec's "exact integers floor to themselves" fix-up is what `floor` does]*
fn legacy_axis(a: f32, b: f32, d: f32) -> i32 {
    let s = ((a as f64 + b as f64) * 0.5 + d as f64) * 0.0625;
    (s as f32).floor() as i32
}

/// The old → new bit pairs of `ConvertV5FlagsToCurrentFlags` (§2.3 table).
const V5_FLAG_MAP: [(u32, u32); 24] = [
    (0x1, 0x1),
    (0x2, 0x2),
    (0x4, 0x4),
    (0x8, 0x8),
    (0x10, 0x10),
    (0x20, 0x40000),
    (0x40, 0x80000),
    (0x200, 0x80_0000),
    (0x400, 0x100_0000),
    (0x800, 0x200_0000),
    (0x1000, 0x400_0000),
    (0x2000, 0x40),
    (0x4000, 0x20),
    (0x8000, 0x40_0000),
    (0x1_0000, 0x1000_0000),
    (0x4_0000, 0x2000_0000),
    (0x8_0000, 0x4000_0000),
    (0x10_0000, 0x80),
    (0x20_0000, 0x100),
    (0x40_0000, 0x1_0000),
    (0x80_0000, 0x2_0000),
    (0x100_0000, 0x10_0000),
    (0x400_0000, 0x8000_0000),
    (0x2000_0000, 0x20_0000),
];

/// `ConvertV5FlagsToCurrentFlags` (`0x006191A0`, NO16 d): map each old bit,
/// then, when old bit `0x80` is clear, drop `0x6000000` and add `0x8000000`.
pub fn convert_v5_flags(old: u32) -> u32 {
    let mut new = 0;
    for &(o, n) in &V5_FLAG_MAP {
        if old & o != 0 {
            new |= n;
        }
    }
    if old & 0x80 == 0 {
        new = (new & !0x0600_0000) | 0x0800_0000;
    }
    new
}

impl NetworkBaseRecord {
    /// The tail effects of the read that produced this record.
    pub fn load_effects(&self) -> LoadEffects {
        if self.path_id == 0 {
            return LoadEffects::default();
        }
        let legacy_cell = match (self.version < 7, self.legacy_offsets) {
            (true, Some([dx, dz])) => Some((
                legacy_axis(self.quad[0].pos[0], self.quad[2].pos[0], dx),
                legacy_axis(self.quad[0].pos[2], self.quad[2].pos[2], dz),
            )),
            _ => None,
        };
        LoadEffects {
            legacy_cell,
            add_legacy_mark: self.version < 8,
        }
    }

    /// The end of `Read`: if vertex 0's alpha is not `0xFF` or the model's
    /// render-state bit 2 is set, all four quad alphas become `0xFF`; returns
    /// true when the caller must then clear bit 2 and store alpha 1.0 in the
    /// render-state block (slot `+0x40`).
    pub fn finish_read_alpha(&mut self, render_bit2: bool) -> bool {
        if self.quad[0].alpha() != 0xFF || render_bit2 {
            for v in self.quad.iter_mut() {
                v.force_alpha();
            }
            true
        } else {
            false
        }
    }
}

/// Segment-stream `Write` (`0x0061A190`): always version 8; false (and nothing after it)
/// when the occupant base record fails; else true when the stream has no
/// error afterwards.
pub fn write_network_record(rec: &NetworkBaseRecord, out: &mut dyn OutStream) -> bool {
    out.put_u16(NETWORK_RECORD_VERSION);
    if !rec.base.write(out) {
        return false;
    }
    rec.transform.write(out);
    for v in &rec.quad {
        v.write(out);
    }
    out.put_u32(rec.word_fc);
    out.put_u8(rec.rotation);
    out.put_u32(rec.variation);
    out.put_u8(rec.orientation);
    out.put_u16(CELL_INFO_VERSION);
    out.put_u8(rec.cell_info.entries.len() as u8);
    out.put_u8(rec.cell_info.byte_10c);
    out.put_u32(rec.cell_info.word_110);
    for &(b, w) in &rec.cell_info.entries {
        out.put_u8(b);
        out.put_u32(w);
    }
    out.put_u32(rec.triangles.len() as u32);
    for t in &rec.triangles {
        out.put_u32(t.word0);
        for v in &t.verts {
            v.write(out);
        }
    }
    for i in 0..3 {
        out.put_f32(rec.bbox_min[i]);
        out.put_f32(rec.bbox_max[i]);
    }
    out.put_u32(rec.network_flags);
    out.put_u32(rec.path_id);
    out.put_res_key(rec.path_info_key);
    out.put_i64(rec.int64);
    out.error() == 0
}

/// Segment-stream `Read` (`0x0061CC00`) into a fresh record. `None` for a version below 4
/// (only the u16 read), a failed occupant base, a short stream or a stream
/// error. A cell-info version other than 2 consumes only its u16 and the
/// read **continues** (the caller ignores that result; the stream is then
/// misaligned), leaving the cell info as it was.
pub fn read_network_record(inp: &mut dyn InStream) -> Option<NetworkBaseRecord> {
    read_network_fields(inp, true)
}

/// Persisted values before the live reader masks flags and forces triangle
/// alpha (§2.3). Legacy versions still convert to the current writer's layout.
pub fn read_saved_network_record(inp: &mut dyn InStream) -> Option<NetworkBaseRecord> {
    read_network_fields(inp, false)
}

fn read_network_fields(inp: &mut dyn InStream, load_fixups: bool) -> Option<NetworkBaseRecord> {
    let mut rec = NetworkBaseRecord {
        version: inp.get_u16()?,
        ..NetworkBaseRecord::default()
    };
    if rec.version < NETWORK_RECORD_MIN_VERSION {
        return None;
    }
    rec.base = OccupantBase::read(inp)?;
    if rec.version >= 5 {
        rec.transform.read_into(inp)?;
    } else {
        rec.transform.legacy_load_into(inp)?;
    }
    for v in rec.quad.iter_mut() {
        *v = Vertex::read(inp)?;
    }
    rec.word_fc = inp.get_u32()?;
    rec.rotation = inp.get_u8()?;
    rec.variation = inp.get_u32()?;
    rec.orientation = inp.get_u8()?;
    if inp.get_u16()? == CELL_INFO_VERSION {
        let count = inp.get_u8()?;
        rec.cell_info.byte_10c = inp.get_u8()?;
        rec.cell_info.word_110 = inp.get_u32()?;
        let mut entries = Vec::with_capacity(count as usize);
        for _ in 0..count {
            entries.push((inp.get_u8()?, inp.get_u32()?));
        }
        rec.cell_info.entries = entries;
    }
    let count = inp.get_u32()?;
    // No capacity from the stream's count: a corrupt count fails on the
    // first short read instead of allocating.
    for _ in 0..count {
        let mut t = Triangle {
            word0: inp.get_u32()?,
            ..Triangle::default()
        };
        for v in t.verts.iter_mut() {
            *v = Vertex::read(inp)?;
            if load_fixups {
                v.force_alpha();
            }
        }
        rec.triangles.push(t);
    }
    for i in 0..3 {
        rec.bbox_min[i] = inp.get_f32()?;
        rec.bbox_max[i] = inp.get_f32()?;
    }
    let mut flags = inp.get_u32()?;
    if rec.version < 6 {
        flags = convert_v5_flags(flags);
    }
    rec.network_flags = if load_fixups { flags & SAVED_FLAGS_MASK } else { flags };
    rec.path_id = inp.get_u32()?;
    if rec.path_id != 0 && rec.version < 7 {
        rec.legacy_offsets = Some([inp.get_f32()?, inp.get_f32()?]);
    }
    rec.path_info_key = inp.get_res_key()?;
    rec.int64 = inp.get_i64()?;
    if inp.error() != 0 {
        return None;
    }
    Some(rec)
}

/// §2.8.1: shared envelope of prebuilt and bridge occupants. Field names
/// retain the primary-object offsets; their meanings are opaque (NO4).
#[derive(Clone, Debug, Default)]
pub struct PrebuiltRecord {
    pub base: NetworkBaseRecord,
    pub saved_178: [f32; 3],
    pub saved_1b0: u32,
    pub saved_1b4: u8,
    pub gate_19c: u32,
    pub saved_1a0: [f32; 4],
}

impl PrebuiltRecord {
    pub fn write(&self, out: &mut dyn OutStream) -> bool {
        out.put_u16(4);
        if !write_network_record(&self.base, out) {
            return false;
        }
        for f in self.saved_178 {
            out.put_f32(f);
        }
        out.put_u32(self.saved_1b0);
        out.put_u8(self.saved_1b4);
        out.put_u32(self.gate_19c);
        if self.gate_19c != 0 {
            for f in self.saved_1a0 {
                out.put_f32(f);
            }
        }
        out.error() == 0
    }

    /// An omitted optional block leaves the existing live fields intact.
    pub fn read_into(&mut self, inp: &mut dyn InStream) -> bool {
        self.read_fields(inp, true).is_some() && inp.error() == 0
    }

    /// Archive load without the inherited live flag/alpha fixups.
    pub fn read_saved_into(&mut self, inp: &mut dyn InStream) -> bool {
        self.read_fields(inp, false).is_some() && inp.error() == 0
    }

    fn read_fields(&mut self, inp: &mut dyn InStream, load_fixups: bool) -> Option<()> {
        let version = inp.get_u16()?;
        if !(3..=4).contains(&version) {
            return None;
        }
        let Some(base) = read_network_fields(inp, load_fixups) else {
            // The inherited result gates the suffix; the envelope's final
            // result still tests the stream error (§2.8.1).
            return (inp.error() == 0).then_some(());
        };
        self.base = base;
        for f in &mut self.saved_178 {
            *f = inp.get_f32()?;
        }
        if version == 3 {
            for _ in 0..6 {
                inp.get_u32()?;
            }
        }
        self.saved_1b0 = inp.get_u32()?;
        self.saved_1b4 = inp.get_u8()?;
        self.gate_19c = inp.get_u32()?;
        if self.gate_19c != 0 {
            for f in &mut self.saved_1a0 {
                *f = inp.get_f32()?;
            }
        }
        if version == 3 {
            inp.get_u32()?;
        }
        Some(())
    }
}

/// §2.8.1: tunnel v2 wraps the complete prebuilt envelope and a segment
/// reference. RefValue owns resolved Rust references; capture IDs own none.
#[derive(Clone, Debug)]
pub struct TunnelRecord {
    pub prebuilt: PrebuiltRecord,
    pub pair: RefValue,
}

impl TunnelRecord {
    /// `serializable_pair` is the result of the caller's serialization QI.
    pub fn write(&self, serializable_pair: &RefValue, out: &mut dyn OutStream) -> bool {
        out.put_u16(2);
        if !self.prebuilt.write(out) {
            return false;
        }
        out.put_u8(u8::from(!serializable_pair.is_null()));
        if !serializable_pair.is_null() {
            out.put_ref(serializable_pair);
        }
        out.error() == 0
    }

    pub fn read_into(&mut self, inp: &mut dyn InStream) -> bool {
        self.read_fields(inp, true).is_some() && inp.error() == 0
    }

    /// Archive load preserving the pair identity and inherited saved values.
    pub fn read_saved_into(&mut self, inp: &mut dyn InStream) -> bool {
        self.read_fields(inp, false).is_some() && inp.error() == 0
    }

    fn read_fields(&mut self, inp: &mut dyn InStream, load_fixups: bool) -> Option<()> {
        if inp.get_u16()? != 2 {
            return None;
        }
        let ok = if load_fixups {
            self.prebuilt.read_into(inp)
        } else {
            self.prebuilt.read_saved_into(inp)
        };
        if !ok {
            return None;
        }
        let present = inp.get_u8()? != 0;
        self.pair = RefValue::Null;
        if present {
            self.pair = inp.get_ref_as(0x8A4B_D52B)?;
        }
        Some(())
    }
}
