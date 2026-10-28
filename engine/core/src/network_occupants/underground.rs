// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.
//! The underground pieces (`network-occupants.md` §3): the shared
//! `cSC4UndergroundOccupant` behaviour of §3.2, its §3.4 save record, and
//! the pipe/subway members and records of §3.3/§3.5. All offsets are object
//! offsets (the record code runs at `object+8`, NO17 b).
//!
//! Implemented: `GetSortValue`, `GetPosition` (4-vertex average, NO17 f),
//! `GetTerrainHeights`/`SetTerrainHeights` (`+0x24C…+0x258`, NO17 d), the
//! five-parameter `Init`'s height store, `SetVertexHeights`' stores, slot
//! 104's model choice, `IsImaginaryConnection` (NO17 e), pipe
//! `SetWateredState`, pipe `Init`'s `+0x270` reset, the folded pipe/subway
//! `Shutdown`, and the underground (v3), pipe (v3) and subway (v2) records.
//!
//! Not implemented (spec silent or out of scope): the uv ratios (class-info
//! layout), `GetZipperPieceType`'s result mapping, geometry, the wall-quad
//! builder and drawing.

use super::identity::NetOccClass;
use super::piece::{NetworkPiece, PieceHost};
use super::record::{read_network_record, read_saved_network_record, write_network_record, NetworkBaseRecord, Vertex};
use crate::stream::{InStream, OutStream};

/// `GetSortValue`'s fallback for the underground base (§3.2).
pub const UNDERGROUND_SORT_DEFAULT: u32 = 0x4A10_AFC2;
/// The network flags that make `GetSortValue` return the stored value.
pub const SORT_STORED_FLAGS: u32 = 0x0600_0000;
/// Slot 104: the burst-model flag.
pub const FLAG_BURST: u32 = 0x0002_0000;
/// Slot 104: the distressed-model flag.
pub const FLAG_DISTRESSED: u32 = 0x0001_0000;
/// `IsImaginaryConnection`'s bit (network flag bit 31).
pub const FLAG_IMAGINARY: u32 = 0x8000_0000;
/// `GetPosition`'s guard value (`FLT_MAX`, `0x00AAABB0`).
pub const HEIGHT_GUARD: f32 = f32::MAX;
/// The underground record's version (§3.4).
pub const UNDERGROUND_RECORD_VERSION: u16 = 3;
/// The pipe record's version (§3.3).
pub const PIPE_RECORD_VERSION: u16 = 3;
/// The subway record's version (§3.5).
pub const SUBWAY_RECORD_VERSION: u16 = 2;
/// `IsImaginaryConnection`'s neighbour offsets `(dx, dz)` by direction
/// (table `0x00B0CFE8`).
pub const NEIGHBOUR_OFFSETS: [(i32, i32); 4] = [(-1, 0), (0, -1), (1, 0), (0, 1)];

/// Slot 104's result (plumbing.md cites `0x0065A880`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UndergroundModel {
    Normal,
    Burst,
    Distressed,
}

/// The calls an underground piece makes into the game.
pub trait UndergroundHost {
    /// `FindUndergroundOccupant` (`0x006584D0`) for class `class_id` at cell
    /// `(x, z)`: that piece's network flags, or `None` when there is none.
    fn find_underground_flags(&mut self, class_id: u32, x: i32, z: i32) -> Option<u32>;
    /// Primary slot 100 (`0x0065AF10`): sets `+0x260` from city slot 0x18C.
    fn refresh_cell_value(&mut self);
    /// `SetTerrainHeights`' follow-up: primary slot 80 (the bounding box) and
    /// `ComputeModelYPosition` (`0x0065AB50`).
    fn terrain_heights_changed(&mut self);
}

/// The §3.4 tail after the base network record (object offsets).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UndergroundTail {
    /// `+0x50`, `+0x51`.
    pub bytes_50: [u8; 2],
    /// `+0x60`.
    pub float_60: f32,
    /// `+0x17C`.
    pub word_17c: u32,
    /// The six wall vertex lists `+0x180`, `+0x18C`, `+0x198`, `+0x1A4`,
    /// `+0x1B4`, `+0x1C0`.
    pub walls: [Vec<Vertex>; 6],
    /// `+0x1FC…+0x23B`, row by row.
    pub matrix: [f32; 16],
    /// `+0x23C`, `+0x240`.
    pub cell: [i32; 2],
    /// `+0x248…+0x24A`.
    pub bytes_248: [u8; 3],
    /// `+0x24C…+0x258`.
    pub heights: [f32; 4],
    /// `+0x25C`.
    pub float_25c: f32,
    /// `+0x264`.
    pub class_id: u32,
    /// `+0x268`.
    pub word_268: u32,
}

impl UndergroundTail {
    /// Byte length: 137 + 24 per wall vertex.
    pub fn encoded_len(&self) -> usize {
        137 + 24 * self.walls.iter().map(Vec::len).sum::<usize>()
    }

    fn write(&self, out: &mut dyn OutStream) {
        out.put_u8(self.bytes_50[0]);
        out.put_u8(self.bytes_50[1]);
        out.put_f32(self.float_60);
        out.put_u32(self.word_17c);
        for w in &self.walls {
            write_wall(w, out);
        }
        for &f in &self.matrix {
            out.put_f32(f);
        }
        out.put_u32(self.cell[0] as u32);
        out.put_u32(self.cell[1] as u32);
        for &b in &self.bytes_248 {
            out.put_u8(b);
        }
        for &h in &self.heights {
            out.put_f32(h);
        }
        out.put_f32(self.float_25c);
        out.put_u32(self.class_id);
        out.put_u32(self.word_268);
    }

    fn read(inp: &mut dyn InStream) -> Option<UndergroundTail> {
        let mut t = UndergroundTail {
            bytes_50: [inp.get_u8()?, inp.get_u8()?],
            float_60: inp.get_f32()?,
            word_17c: inp.get_u32()?,
            ..UndergroundTail::default()
        };
        for w in t.walls.iter_mut() {
            *w = read_wall(inp)?;
        }
        for f in t.matrix.iter_mut() {
            *f = inp.get_f32()?;
        }
        t.cell = [inp.get_u32()? as i32, inp.get_u32()? as i32];
        for b in t.bytes_248.iter_mut() {
            *b = inp.get_u8()?;
        }
        for h in t.heights.iter_mut() {
            *h = inp.get_f32()?;
        }
        t.float_25c = inp.get_f32()?;
        t.class_id = inp.get_u32()?;
        t.word_268 = inp.get_u32()?;
        Some(t)
    }
}

/// A wall vertex list (helper `0x0065B070` / `0x006574A0`, NO17 c): u32
/// count, then `count` 24-byte vertices.
pub(super) fn write_wall(w: &[Vertex], out: &mut dyn OutStream) {
    out.put_u32(w.len() as u32);
    for v in w {
        v.write(out);
    }
}

/// The reader does not force the alpha byte.
pub(super) fn read_wall(inp: &mut dyn InStream) -> Option<Vec<Vertex>> {
    let n = inp.get_u32()?;
    // No capacity from the stream's count (a corrupt count fails short).
    let mut w = Vec::new();
    for _ in 0..n {
        w.push(Vertex::read(inp)?);
    }
    Some(w)
}

/// The underground record (§3.4): u16 3, the base record, the tail.
#[derive(Clone, Debug, Default)]
pub struct UndergroundRecord {
    pub base: NetworkBaseRecord,
    pub tail: UndergroundTail,
}

/// Underground `Write` (`0x0065B140`): the tail is skipped when the base
/// record fails.
pub fn write_underground_record(rec: &UndergroundRecord, out: &mut dyn OutStream) -> bool {
    out.put_u16(UNDERGROUND_RECORD_VERSION);
    if !write_network_record(&rec.base, out) {
        return false;
    }
    rec.tail.write(out);
    out.error() == 0
}

/// Underground `Read` (`0x0065E510`): exactly v3 (else `None` after the
/// u16), then the base record (failure: `None`, tail skipped), then the
/// tail. The live `+0x261 = 1` / `+0x262 = 0` stores are
/// [`UndergroundPiece::apply_record`]'s.
pub fn read_underground_record(inp: &mut dyn InStream) -> Option<UndergroundRecord> {
    read_underground_fields(inp, false)
}

pub(super) fn read_saved_underground_record(inp: &mut dyn InStream) -> Option<UndergroundRecord> {
    read_underground_fields(inp, true)
}

fn read_underground_fields(inp: &mut dyn InStream, saved: bool) -> Option<UndergroundRecord> {
    if inp.get_u16()? != UNDERGROUND_RECORD_VERSION {
        return None;
    }
    let base = if saved {
        read_saved_network_record(inp)?
    } else {
        read_network_record(inp)?
    };
    let tail = UndergroundTail::read(inp)?;
    if inp.error() != 0 {
        return None;
    }
    Some(UndergroundRecord { base, tail })
}

/// Pipe `Write` (`0x00648800`): u16 3, the underground record, u32 `+0x270`.
pub fn write_pipe_record(rec: &UndergroundRecord, word_270: u32, out: &mut dyn OutStream) -> bool {
    out.put_u16(PIPE_RECORD_VERSION);
    if !write_underground_record(rec, out) {
        return false;
    }
    out.put_u32(word_270);
    out.error() == 0
}

/// Pipe `Read` (`0x00648840`): exactly v3; returns the record and `+0x270`.
/// *[literal, NO18: §3.3 does not say whether a failed underground read skips the
/// u32; here it does]*
pub fn read_pipe_record(inp: &mut dyn InStream) -> Option<(UndergroundRecord, u32)> {
    if inp.get_u16()? != PIPE_RECORD_VERSION {
        return None;
    }
    let rec = read_underground_record(inp)?;
    let w = inp.get_u32()?;
    Some((rec, w))
}

/// Subway `Write` (`0x00653010`): u16 2, the underground record.
pub fn write_subway_record(rec: &UndergroundRecord, out: &mut dyn OutStream) -> bool {
    out.put_u16(SUBWAY_RECORD_VERSION);
    write_underground_record(rec, out)
}

/// Subway `Read` (`0x00653040`): exactly v2, then the underground record.
pub fn read_subway_record(inp: &mut dyn InStream) -> Option<UndergroundRecord> {
    if inp.get_u16()? != SUBWAY_RECORD_VERSION {
        return None;
    }
    read_underground_record(inp)
}

/// The shared underground state on top of a [`NetworkPiece`].
#[derive(Clone, Debug)]
pub struct UndergroundPiece {
    pub piece: NetworkPiece,
    /// The class's `GetSortValue` default: the underground base ID, or
    /// the subway's (`0xCA16374F`) / pipe's own class ID (§3.2 "the pipe
    /// shape is the same" *[literal: its own class ID]*).
    pub sort_default: u32,
    /// The stored sort value (its field is NO7).
    pub sort_value: u32,
    /// `+0x9C…+0xFB`: the quad (`GetPosition`, `SetVertexHeights`).
    pub quad: [Vertex; 4],
    /// `+0x23C`, `+0x240`: the cell.
    pub cell: [i32; 2],
    /// `+0x24C…+0x258`: the four terrain heights (NO17 d).
    pub heights: [f32; 4],
    /// `+0x260` (pipe/subway `Read` set it to 1; slot 100 sets it).
    pub byte_260: u8,
    /// `+0x261`, `+0x262` (underground `Read`: 1, 0).
    pub bytes_261: [u8; 2],
    /// `+0x264`: the class ID `IsImaginaryConnection` searches for.
    pub class_id: u32,
    /// Pipe `+0x270` (reset by pipe `Init`).
    pub pipe_270: u32,
    /// Pipe watered state (`SetWateredState`).
    pub watered: u32,
    /// The owned pieces `Shutdown` releases (textures, wall vectors,
    /// `+0x244`), as a count of held resources.
    pub held_resources: u32,
}

impl UndergroundPiece {
    /// The underground state over `piece` (an underground, pipe or subway
    /// piece; the class picks the sort default and `+0x264`).
    pub fn new(piece: NetworkPiece) -> UndergroundPiece {
        let sort_default = match piece.class {
            NetOccClass::Subway | NetOccClass::Pipe => piece.class.class_id(),
            _ => UNDERGROUND_SORT_DEFAULT,
        };
        UndergroundPiece {
            sort_default,
            sort_value: 0,
            quad: [Vertex::default(); 4],
            cell: [0; 2],
            heights: [0.0; 4],
            byte_260: 0,
            bytes_261: [0; 2],
            class_id: piece.class.class_id(),
            pipe_270: 0,
            watered: 0,
            held_resources: 0,
            piece,
        }
    }

    /// `GetSortValue` (`0x0065A0E0`).
    pub fn get_sort_value(&self) -> u32 {
        if self.piece.has_any_network_flag(SORT_STORED_FLAGS) {
            self.sort_value
        } else {
            self.sort_default
        }
    }

    /// `GetPosition` (`0x0065A720`, NO17 f): x/z the average of the four quad
    /// vertices (f32 partial sums `((v0 + v1) + v2) + v3`, then × 0.25); y
    /// the minimum of the four heights by strict `<` from `FLT_MAX`.
    pub fn get_position(&self) -> [f32; 3] {
        let y = self
            .heights
            .iter()
            .fold(HEIGHT_GUARD, |m, &h| if h < m { h } else { m });
        let sum = |k: usize| {
            let q = &self.quad;
            ((q[0].pos[k] + q[1].pos[k]) + q[2].pos[k]) + q[3].pos[k]
        };
        [sum(0) * 0.25, y, sum(2) * 0.25]
    }

    /// `GetTerrainHeights` slot 94 (`0x00659DD0`): `+0x24C…+0x258`.
    pub fn get_terrain_heights(&self) -> [f32; 4] {
        self.heights
    }

    /// `SetTerrainHeights` slot 95 (`0x0065B6B0`): stores the four, then the
    /// host's bounding-box and model-y recompute.
    pub fn set_terrain_heights(&mut self, h: &[f32; 4], host: &mut dyn UndergroundHost) {
        self.heights = *h;
        host.terrain_heights_changed();
    }

    /// The five-parameter `Init`'s height store (`0x0065C6B0`): the floats at
    /// `p2+4/+0x10/+0x1C/+0x28` into `+0x24C…+0x258`.
    pub fn store_init_heights(&mut self, p2: &[f32; 4]) {
        self.heights = *p2;
    }

    /// Slot 84 `SetVertexHeights` (`0x0065AE30`), the stores only: quad
    /// vertex y `+0xA0/+0xB8/+0xD0/+0xE8`. The rebuild is the host's.
    pub fn set_vertex_heights(&mut self, h: &[f32; 4]) {
        for (v, &y) in self.quad.iter_mut().zip(h.iter()) {
            v.pos[1] = y;
        }
    }

    /// Slot 104 (`0x0065A880`): burst before distressed.
    pub fn model_choice(&self) -> UndergroundModel {
        if self.piece.network_flags & FLAG_BURST != 0 {
            UndergroundModel::Burst
        } else if self.piece.network_flags & FLAG_DISTRESSED != 0 {
            UndergroundModel::Distressed
        } else {
            UndergroundModel::Normal
        }
    }

    /// `IsImaginaryConnection` (`0x0065A640`, NO17 e): false unless my bit
    /// 31 is set; then false when no same-class piece is at the neighbour
    /// cell, else true iff its bit 31 is clear. A direction above 3 is
    /// outside the table: false *[literal, NO20: the table has four entries]*.
    pub fn is_imaginary_connection(&self, dir: u32, host: &mut dyn UndergroundHost) -> bool {
        if self.piece.network_flags & FLAG_IMAGINARY == 0 {
            return false;
        }
        let Some(&(dx, dz)) = NEIGHBOUR_OFFSETS.get(dir as usize) else {
            return false;
        };
        let (x, z) = (self.cell[0].wrapping_add(dx), self.cell[1].wrapping_add(dz));
        match host.find_underground_flags(self.class_id, x, z) {
            Some(f) => f & FLAG_IMAGINARY == 0,
            None => false,
        }
    }

    /// Pipe `SetWateredState` (`0x00648540`, non-virtual): store; on a change
    /// call primary slot 100.
    pub fn set_watered_state(&mut self, state: u32, host: &mut dyn UndergroundHost) {
        if self.watered != state {
            self.watered = state;
            host.refresh_cell_value();
        }
    }

    /// Pipe `Init`'s own step after the underground `Init`: `+0x270 = 0`.
    pub fn pipe_init_tail(&mut self) {
        self.pipe_270 = 0;
    }

    /// The live fields of an underground record read: the base fields this
    /// type holds (quad, flags, orientation, rotation, variation, path id),
    /// the tail's cell, heights and class ID, then the underground `Read`'s
    /// `+0x261 = 1`, `+0x262 = 0`. The pipe/subway `Read`s then set
    /// `+0x260 = 1` ([`UndergroundPiece::mark_loaded_subclass`]).
    pub fn apply_record(&mut self, rec: &UndergroundRecord) {
        let b = &rec.base;
        self.quad = b.quad;
        self.piece.network_flags = b.network_flags;
        self.piece.orientation = b.orientation;
        self.piece.rotation = b.rotation;
        self.piece.variation = b.variation;
        self.piece.path_id_override = b.path_id;
        self.cell = rec.tail.cell;
        self.heights = rec.tail.heights;
        self.class_id = rec.tail.class_id;
        self.bytes_261 = [1, 0];
    }

    /// Pipe/subway `Read`'s last step: `+0x260 = 1`.
    pub fn mark_loaded_subclass(&mut self) {
        self.byte_260 = 1;
    }

    /// The underground `Shutdown` (`0x0065CD90`), also the folded pipe/subway
    /// slot 4 (`0x00652CF0`): release the owned resources, then the base
    /// `Shutdown`.
    pub fn shutdown(&mut self, host: &mut dyn PieceHost) {
        self.held_resources = 0;
        self.piece.shutdown(host);
    }
}
