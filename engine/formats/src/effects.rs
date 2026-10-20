// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The effect directory resource (`docs/specs/effects.md` §3).
//!
//! **The record tables of §3 give *in-memory* sizes, not on-disk ones.** A
//! visual-effect description is `0x38` bytes and a component record `0x6C`
//! bytes once loaded, but on disk the component record is **variable-length**
//! (it opens with a `u32` length and that many characters) and is nested inside
//! the effect record as a counted list. There is no fixed `0x6C`-byte stride on
//! disk; a reader that strides `0x6C` is wrong.
//!
//! On disk (readers `0x005ABF50` effect, `0x005AB690` component, `0x005ABEC0`
//! binding):
//!
//! ```text
//! visual effect description
//!     u32   flags & 0x1FF
//!     u32   priority
//!     u32   component count, then that many variable-length component records
//!     u32   binding count, then that many bindings
//!     str
//!     3 x u32   only when the section version is above 1
//!
//! component record (variable-length)
//!     str   name
//!     u8    type
//!     u32   flags & 3
//!     56 bytes   placement (S3DTransformLegacyLoad)
//!     u8, u8    LOD range [min, max)
//!     u16, u16  particle copies and spacing
//!     4 x f32
//!     u16, u16  select group and weight
//!     u32   description index
//!
//! binding (24 bytes)
//!     u32 & 0xF, str, f32, u32
//! ```
//!
//! Strings here are a `u32` length followed by the characters with **no
//! terminator** — not the `GetGZStr` form of foundations §2.3.

use crate::error::Result;
use crate::reader::Reader;

/// The effect directory's type ID (§3: type `0xEA5118B0`, group `0xEA5118B1`,
/// instance 1).
pub const EFFECT_RESOURCE_TYPE: u32 = 0xEA51_18B0;
/// The effect directory's group ID.
pub const EFFECT_RESOURCE_GROUP: u32 = 0xEA51_18B1;

/// The **in-memory** size of a visual-effect description (§3). Not its on-disk
/// size, which is variable.
pub const VISUAL_EFFECT_DESC_MEM_SIZE: usize = 0x38;
/// The **in-memory** size of a component record (§3). Not its on-disk size,
/// which is variable.
pub const COMPONENT_MEM_SIZE: usize = 0x6C;
/// A parameter binding is 24 bytes on disk (§3).
pub const BINDING_SIZE: usize = 24;
/// The placement transform of a component record (§3).
pub const PLACEMENT_SIZE: usize = 56;

/// The component types of §3 (`+0x0C`, a `u8`).
pub mod component_type {
    pub const PARTICLES: u8 = 0;
    pub const DECAL: u8 = 1;
    /// A nested visual effect, found by name.
    pub const META: u8 = 2;
    pub const BRUSH: u8 = 3;
    pub const ATTRACTOR: u8 = 4;
    /// The game ("scrubber") records.
    pub const GAME: u8 = 5;
    pub const SEQUENCE: u8 = 6;
    pub const SOUND: u8 = 7;
    pub const CAMERA: u8 = 8;
    pub const MODEL: u8 = 16;
}

/// Description-side component flags (`desc + 0x10`, masked to `& 3`) (§3, §7).
pub mod desc_flags {
    /// Bit 0: the component is **ignored by the finish test**. Copied to
    /// [`runtime_flags::IGNORED`], which is what `0x005BFD70` actually tests.
    pub const IGNORED: u32 = 1;
    /// Bit 1: render.
    pub const RENDER: u32 = 2;
}

/// Runtime component flags (`comp + 0x40`) (§7). The finish test tests bit 2,
/// **not** the description bit directly.
pub mod runtime_flags {
    /// Bit 1 (value 2): set on every table component of type 3-15.
    pub const TABLE: u8 = 2;
    /// Bit 2 (value 4): the copy of [`desc_flags::IGNORED`], and the bit
    /// `0x005BFD70` tests.
    pub const IGNORED: u8 = 4;
}

/// The runtime component flags for a description component (§7, conformance
/// test 12). `0x00591730` copies `desc+0x10 & 1` into bit 2 (value 4), and a
/// table component (types 3-15) additionally gets bit 1 (value 2).
pub fn runtime_flags_for(desc_flags: u32, component_type: u8) -> u8 {
    let mut f = 0;
    if desc_flags & desc_flags::IGNORED != 0 {
        f |= runtime_flags::IGNORED;
    }
    // "Table types 3-15": the brush, game, sequence, sound and camera records
    // held in the description tables, and nothing outside that range.
    if (3..=15).contains(&component_type) {
        f |= runtime_flags::TABLE;
    }
    f
}

/// A component's level-of-detail range `[min, max)` (§3, `+0x4C`/`+0x4D`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LodRange {
    pub min: u8,
    pub max: u8,
}

impl LodRange {
    /// Whether the level of detail `L` is inside `[min, max)` (§5.2).
    pub fn contains(&self, lod: u8) -> bool {
        self.min <= lod && lod < self.max
    }

    /// The full range the shipped data mostly uses, `[1, 6)`.
    pub fn is_full(&self) -> bool {
        self.min == 1 && self.max == 6
    }

    /// Whether this is a range that reaches a brush or game record (§5.2).
    ///
    /// "Every component of every shipped effect has the range `[1, 6)`" is
    /// false: 2533 of 3373 do and 840 do not. The narrower claim the
    /// simulation depends on, and the one worth asserting, is that every
    /// component that reaches a brush or game record — directly or through a
    /// meta component — is `[1, 6)`.
    pub fn reaches_table(&self) -> bool {
        self.is_full()
    }
}

/// One parameter binding of a visual effect (§3).
#[derive(Clone, Debug, PartialEq)]
pub struct Binding {
    /// `u32 & 0xF`.
    pub kind: u32,
    pub name: Vec<u8>,
    pub value: f32,
    pub index: u32,
}

/// One component record (§3). The struct holds the decoded fields; the on-disk
/// size depends on the length of [`Component::name`].
#[derive(Clone, Debug, PartialEq)]
pub struct Component {
    /// `+0x00`: the child effect's name for a meta component, the asset name
    /// for particles and decals, empty otherwise.
    pub name: Vec<u8>,
    /// `+0x0C`: [`component_type`].
    pub kind: u8,
    /// `+0x10`: [`desc_flags`], masked to 3 bits.
    pub flags: u32,
    /// `+0x14`: the 56-byte local transform.
    pub placement: [u8; PLACEMENT_SIZE],
    /// `+0x4C`, `+0x4D`.
    pub lod: LodRange,
    /// `+0x4E`, `+0x50`: particle copies and spacing (render).
    pub copies: u16,
    pub spacing: u16,
    /// `+0x54`..`+0x60`.
    pub params: [f32; 4],
    /// `+0x64`: the select group (0 = always created).
    pub select_group: u16,
    /// `+0x66`.
    pub select_weight: u16,
    /// `+0x68`: the index into the description table of the component's type.
    pub index: u32,
}

impl Component {
    /// The runtime flags this component is built with (§7, conformance test
    /// 12). This is the value the finish test tests, not [`Component::flags`].
    pub fn runtime_flags(&self) -> u8 {
        runtime_flags_for(self.flags, self.kind)
    }

    /// Whether the finish test skips this component (§7).
    pub fn ignored_by_finish(&self) -> bool {
        self.runtime_flags() & runtime_flags::IGNORED != 0
    }

    /// Whether this component is one of the table types (brush, game,
    /// sequence, sound, camera) (§7).
    pub fn is_table(&self) -> bool {
        (3..=15).contains(&self.kind)
    }
}

/// One visual-effect description (§3).
#[derive(Clone, Debug, PartialEq)]
pub struct VisualEffectDescription {
    /// `+0x00`, masked to 9 bits.
    pub flags: u32,
    /// `+0x04`: 0 is never chosen.
    pub priority: u32,
    /// `+0x08`: the nested component records.
    pub components: Vec<Component>,
    /// `+0x14`: the parameter bindings.
    pub bindings: Vec<Binding>,
    /// `+0x2C`.
    pub name: Vec<u8>,
    /// `+0x20`..`+0x28`, only when the section version is above 1.
    pub tail: [u32; 3],
}

/// The visual-effect section of the directory (§3 step 7).
#[derive(Clone, Debug, PartialEq)]
pub struct EffectSection {
    /// The section version (shipped: 2).
    pub version: u16,
    pub descriptions: Vec<VisualEffectDescription>,
}

/// A `u32` length and that many characters, with no terminator (§3).
fn get_str(r: &mut Reader) -> Result<Vec<u8>> {
    let n = r.u32()? as usize;
    Ok(r.bytes(n)?.to_vec())
}

/// A component record: variable-length on disk (§3). [EF1]
pub(crate) fn read_component(r: &mut Reader) -> Result<Component> {
    let name = get_str(r)?;
    let kind = r.u8()?;
    let flags = r.u32()? & 3;
    let mut placement = [0u8; PLACEMENT_SIZE];
    placement.copy_from_slice(r.bytes(PLACEMENT_SIZE)?);
    Ok(Component {
        name,
        kind,
        flags,
        placement,
        lod: LodRange {
            min: r.u8()?,
            max: r.u8()?,
        },
        copies: r.u16()?,
        spacing: r.u16()?,
        params: [r.f32()?, r.f32()?, r.f32()?, r.f32()?],
        select_group: r.u16()?,
        select_weight: r.u16()?,
        index: r.u32()?,
    })
}

/// A 24-byte parameter binding (§3).
pub(crate) fn read_binding(r: &mut Reader) -> Result<Binding> {
    Ok(Binding {
        kind: r.u32()? & 0xF,
        name: get_str(r)?,
        value: r.f32()?,
        index: r.u32()?,
    })
}

/// One visual-effect description (§3), nested components and all. [EF1]
pub(crate) fn read_description(r: &mut Reader, section_version: u16) -> Result<VisualEffectDescription> {
    let flags = r.u32()? & 0x1FF;
    let priority = r.u32()?;
    let n = r.u32()?;
    // The component records are variable-length, so a count alone cannot be
    // turned into a byte span; read them one at a time.
    r.ensure(u64::from(n), 1)?;
    let mut components = Vec::new();
    for _ in 0..n {
        components.push(read_component(r)?);
    }
    let m = r.u32()?;
    r.ensure(u64::from(m), 1)?;
    let mut bindings = Vec::new();
    for _ in 0..m {
        bindings.push(read_binding(r)?);
    }
    let name = get_str(r)?;
    let tail = if section_version > 1 {
        [r.u32()?, r.u32()?, r.u32()?]
    } else {
        [0; 3]
    };
    Ok(VisualEffectDescription {
        flags,
        priority,
        components,
        bindings,
        name,
        tail,
    })
}

/// The visual-effect section: a `u16` version, a `u32` count, then the records
/// (§3 step 7). [EF1]
pub fn read_section(data: &[u8]) -> Result<EffectSection> {
    let mut r = Reader::new(data, "effect section");
    let version = r.u16()?;
    let n = r.u32()?;
    r.ensure(u64::from(n), 1)?;
    let mut descriptions = Vec::new();
    for _ in 0..n {
        descriptions.push(read_description(&mut r, version)?);
    }
    Ok(EffectSection { version, descriptions })
}

/// One brush description (`cSC4BrushDescription`, §8.1). The in-memory object
/// is `0x34` bytes; the binary record (read `0x00581380`, write `0x00581400`)
/// is **39 bytes**, packed with no padding after the one-shot byte.
#[derive(Clone, Debug, PartialEq)]
pub struct BrushDescription {
    /// Record version; shipped is 0.
    pub version: u16,
    /// `+0x0C`: a terrain brush's ID (terrain.md §9.1).
    pub brush_id: u32,
    /// `+0x10`: applications per effect-second, or the count of a one-shot.
    pub rate: f32,
    /// `+0x14`: stroke duration in effect-seconds; 0 means no limit.
    pub length: f32,
    /// `+0x18`: set by the text option `-apply`.
    pub one_shot: u8,
    /// `+0x1C`: `tBrushParameters.level`. The text option `-zoom n` stores `n − 1`.
    pub level: u32,
    /// `+0x20`, `+0x24`: the strength range. Every shipped record is degenerate.
    pub strength: [f32; 2],
    /// `+0x28`, `+0x2C`: the radius-scale range. Every shipped record is degenerate.
    pub radius_scale: [f32; 2],
    /// `+0x30`: the target height. Every shipped record uses 100.
    pub target_height: f32,
}

/// The on-disk size of one brush description record (§8.1).
pub const BRUSH_RECORD_SIZE: usize = 39;

/// One brush description record (§8.1).
pub(crate) fn read_brush_description(r: &mut Reader) -> Result<BrushDescription> {
    Ok(BrushDescription {
        version: r.u16()?,
        brush_id: r.u32()?,
        rate: r.f32()?,
        length: r.f32()?,
        one_shot: r.u8()?,
        level: r.u32()?,
        strength: [r.f32()?, r.f32()?],
        radius_scale: [r.f32()?, r.f32()?],
        target_height: r.f32()?,
    })
}

/// The brush section (§3 step 3): a `u32` count, then the records. Shipped: 25
/// records, the count at `0x98CA2` and the records at `0x98CA6`.
pub fn read_brush_section(data: &[u8]) -> Result<Vec<BrushDescription>> {
    let mut r = Reader::new(data, "brush descriptions");
    let n = r.u32()?;
    r.ensure(u64::from(n), BRUSH_RECORD_SIZE as u64)?;
    let mut out = Vec::new();
    for _ in 0..n {
        out.push(read_brush_description(&mut r)?);
    }
    Ok(out)
}

/// One attractor description (§3 step 4): a `u16` version, a string and a
/// `u8`. Shipped: 64 records. The attractor layer is out of scope (§Scope);
/// the record is read so the directory can be walked in full.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttractorDescription {
    pub version: u16,
    pub name: Vec<u8>,
    /// The trailing `u8`; the spec does not name it. *[choice]*
    pub field: u8,
}

/// One attractor description record (§3 step 4).
pub(crate) fn read_attractor_description(r: &mut Reader) -> Result<AttractorDescription> {
    Ok(AttractorDescription {
        version: r.u16()?,
        name: get_str(r)?,
        field: r.u8()?,
    })
}

/// The attractor section (§3 step 4): a `u32` count, then the records.
/// Shipped: 64 records.
pub fn read_attractor_section(data: &[u8]) -> Result<Vec<AttractorDescription>> {
    let mut r = Reader::new(data, "attractor descriptions");
    let n = r.u32()?;
    r.ensure(u64::from(n), 1)?;
    let mut out = Vec::new();
    for _ in 0..n {
        out.push(read_attractor_description(&mut r)?);
    }
    Ok(out)
}

/// One game ("scrubber") description (`cSC4ScrubberDescription`, §9.1). The
/// in-memory object is `0x50` bytes; the binary record (read `0x005BD260`,
/// write `0x005BCA60`) is **70 bytes at version 1**.
///
/// **The stream order is not the offset order.** `+0x28` and `+0x2C` (the toxic
/// and extinguish amounts) are written last and only when the version is above
/// 0.
#[derive(Clone, Debug, PartialEq)]
pub struct GameDescription {
    /// Shipped is 1.
    pub version: u16,
    /// `+0x0C`, masked to 7 bits: [`game_flags`].
    pub flags: u32,
    /// `+0x10`: read and written with the other `u32`s but named by no text
    /// option in §9.1. *[choice]*
    pub field_10: u32,
    /// `+0x14`: `-demolish r`, in cells.
    pub demolish_radius: u32,
    /// `+0x18`: the demolition flags (demolition.md §4.9).
    pub demolish_flags: u32,
    /// `+0x1C`: `-minDemolishSize`, metres; 0 = no limit.
    pub min_demolish_size: f32,
    /// `+0x20`: `-maxDemolishSize`, metres; 0 = no limit.
    pub max_demolish_size: f32,
    /// `+0x24`: `-burn r`.
    pub burn_radius: u32,
    /// `+0x28`: `-toxic n`. Read only when the version is above 0.
    pub toxic_amount: u32,
    /// `+0x2C`: `-extinguishFire u`. Read only when the version is above 0.
    pub extinguish_amount: u32,
    /// `+0x30`: `-message type …`.
    pub message_type: u32,
    /// `+0x34`: the message's data. It becomes the game component's `+0x58`
    /// `data` field until parameter 13 replaces it (§9.2).
    pub message_data: u32,
    /// `+0x38`: `-blob k …` / `-rect …`, the effect-map shape's `k` (1-8).
    /// Render (§Scope).
    pub map_kind: u32,
    /// `+0x3C`..`+0x48`: the effect-map shape's remaining numbers. `+0x3C`
    /// defaults to 16.0. Render. The record stores `+0x40` as 8 raw bytes.
    pub map_params: [f32; 4],
    /// `+0x4C`: the `d` of `-pauseSim`, in effect-seconds; 0 = none.
    pub duration: f32,
}

/// The game-description flags (`+0x0C`, §9.1).
pub mod game_flags {
    /// `noNetworks`.
    pub const NO_NETWORKS: u32 = 1;
    /// `noFlora`.
    pub const NO_FLORA: u32 = 2;
    /// `dezone`.
    pub const DEZONE: u32 = 4;
    /// `single`: apply once on the first update, then stop.
    pub const SINGLE: u32 = 8;
    /// `-pauseSim [d]`.
    pub const PAUSE_SIM: u32 = 0x10;
    /// `-pauseSimHidden`.
    pub const PAUSE_SIM_HIDDEN: u32 = 0x20;
    /// `-pauseClock`.
    pub const PAUSE_CLOCK: u32 = 0x40;
}

/// The on-disk size of one game description record at version 1 (§9.1).
pub const GAME_RECORD_SIZE_V1: usize = 70;

/// One game description record (§9.1).
pub(crate) fn read_game_description(r: &mut Reader) -> Result<GameDescription> {
    let version = r.u16()?;
    let flags = r.u32()? & 0x7F;
    let field_10 = r.u32()?;
    let demolish_radius = r.u32()?;
    let demolish_flags = r.u32()?;
    let min_demolish_size = r.f32()?;
    let max_demolish_size = r.f32()?;
    let burn_radius = r.u32()?;
    let message_type = r.u32()?;
    let message_data = r.u32()?;
    let map_kind = r.u32()?;
    let map_params = [r.f32()?, r.f32()?, r.f32()?, r.f32()?];
    let duration = r.f32()?;
    // Only when the version is above 0: `+0x28` and `+0x2C` close the record.
    let (toxic_amount, extinguish_amount) = if version > 0 { (r.u32()?, r.u32()?) } else { (0, 0) };
    Ok(GameDescription {
        version,
        flags,
        field_10,
        demolish_radius,
        demolish_flags,
        min_demolish_size,
        max_demolish_size,
        burn_radius,
        toxic_amount,
        extinguish_amount,
        message_type,
        message_data,
        map_kind,
        map_params,
        duration,
    })
}

/// The game section (§3 step 5): a `u32` count, then the records. Shipped: 57
/// records.
pub fn read_game_section(data: &[u8]) -> Result<Vec<GameDescription>> {
    let mut r = Reader::new(data, "game descriptions");
    let n = r.u32()?;
    r.ensure(u64::from(n), 1)?;
    let mut out = Vec::new();
    for _ in 0..n {
        out.push(read_game_description(&mut r)?);
    }
    Ok(out)
}

/// One entry of the name map (§3 step 8).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NameEntry {
    pub name: Vec<u8>,
    pub description_index: u32,
}

/// The name map: `u32` length, the name characters and a `u32` description
/// index per entry, ending at the name `end` or a length of `0xFFFFFFFF`
/// (§3 step 8). [EF1]
pub fn read_name_map(data: &[u8]) -> Result<Vec<NameEntry>> {
    const END: u32 = 0xFFFF_FFFF;
    let mut r = Reader::new(data, "effect name map");
    let mut out = Vec::new();
    loop {
        let n = r.u32()?;
        if n == END {
            break;
        }
        let name = r.bytes(n as usize)?.to_vec();
        let index = r.u32()?;
        let finished = name == b"end";
        out.push(NameEntry {
            name,
            description_index: index,
        });
        if finished {
            break;
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A component record with a short name, so its on-disk size is not
    /// `COMPONENT_MEM_SIZE`.
    fn sample_component(name: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&(name.len() as u32).to_le_bytes());
        v.extend_from_slice(name);
        v.push(component_type::DECAL);
        v.extend_from_slice(&1u32.to_le_bytes()); // flags: ignored
        v.extend_from_slice(&[0u8; PLACEMENT_SIZE]);
        v.push(1); // lod min
        v.push(6); // lod max
        v.extend_from_slice(&3u16.to_le_bytes()); // copies
        v.extend_from_slice(&4u16.to_le_bytes()); // spacing
        for f in [1.5f32, 2.5, 3.5, 4.5] {
            v.extend_from_slice(&f.to_le_bytes());
        }
        v.extend_from_slice(&5u16.to_le_bytes()); // select group
        v.extend_from_slice(&6u16.to_le_bytes()); // select weight
        v.extend_from_slice(&7u32.to_le_bytes()); // index
        v
    }

    /// [EF1] §3 Corrects: the on-disk component record is variable-length and
    /// nested in the effect record. A `0x6C` stride would be wrong.
    #[test]
    fn component_record_is_variable_length_on_disk() {
        let short = sample_component(b"a");
        let long = sample_component(b"much longer asset name");
        // Same field set, different on-disk size: the name is the difference.
        assert_eq!(long.len() - short.len(), b"much longer asset name".len() - b"a".len());
        assert_ne!(short.len(), COMPONENT_MEM_SIZE);
        let mut r = Reader::new(&short, "t");
        let c = read_component(&mut r).unwrap();
        assert_eq!(r.remaining(), 0, "parses to its exact end");
        assert_eq!(c.name, b"a");
        assert_eq!(c.kind, component_type::DECAL);
        assert_eq!(c.flags, desc_flags::IGNORED);
        assert_eq!(c.lod, LodRange { min: 1, max: 6 });
        assert_eq!(c.copies, 3);
        assert_eq!(c.spacing, 4);
        assert_eq!(c.params, [1.5, 2.5, 3.5, 4.5]);
        assert_eq!(c.select_group, 5);
        assert_eq!(c.select_weight, 6);
        assert_eq!(c.index, 7);
    }

    /// [EF1] Two components of different name lengths nest in one effect
    /// record and the record still parses to its exact end.
    #[test]
    fn effect_record_nests_variable_length_components() {
        let mut v = Vec::new();
        v.extend_from_slice(&0x1FFu32.to_le_bytes()); // flags, masked to 9 bits
        v.extend_from_slice(&3u32.to_le_bytes()); // priority
        v.extend_from_slice(&2u32.to_le_bytes()); // component count
        v.extend_from_slice(&sample_component(b"a"));
        v.extend_from_slice(&sample_component(b"a much longer asset name"));
        v.extend_from_slice(&1u32.to_le_bytes()); // binding count
        v.extend_from_slice(&0xFu32.to_le_bytes());
        v.extend_from_slice(&1u32.to_le_bytes());
        v.extend_from_slice(b"k");
        v.extend_from_slice(&1.25f32.to_le_bytes());
        v.extend_from_slice(&9u32.to_le_bytes());
        v.extend_from_slice(&4u32.to_le_bytes());
        v.extend_from_slice(b"stem");
        // section version 2 > 1: three trailing u32
        for x in [10u32, 0xCCCC_CCCC, 0xCCCC_CCCC] {
            v.extend_from_slice(&x.to_le_bytes());
        }

        let mut r = Reader::new(&v, "t");
        let d = read_description(&mut r, 2).unwrap();
        assert_eq!(r.remaining(), 0);
        assert_eq!(d.flags, 0x1FF);
        assert_eq!(d.priority, 3);
        assert_eq!(d.components.len(), 2);
        assert_eq!(d.components[0].name, b"a");
        assert_eq!(d.components[1].name, b"a much longer asset name");
        assert_eq!(d.bindings.len(), 1);
        assert_eq!(d.bindings[0].kind, 0xF);
        assert_eq!(d.bindings[0].name, b"k");
        assert_eq!(d.bindings[0].value, 1.25);
        assert_eq!(d.name, b"stem");
        assert_eq!(d.tail, [10, 0xCCCC_CCCC, 0xCCCC_CCCC]);
    }

    /// A section version of 1 reads no trailing `3 x u32`.
    #[test]
    fn section_version_one_has_no_tail() {
        let mut v = Vec::new();
        v.extend_from_slice(&0u32.to_le_bytes()); // flags
        v.extend_from_slice(&0u32.to_le_bytes()); // priority
        v.extend_from_slice(&0u32.to_le_bytes()); // no components
        v.extend_from_slice(&0u32.to_le_bytes()); // no bindings
        v.extend_from_slice(&1u32.to_le_bytes());
        v.extend_from_slice(b"x");
        let mut r = Reader::new(&v, "t");
        let d = read_description(&mut r, 1).unwrap();
        assert_eq!(r.remaining(), 0);
        assert_eq!(d.tail, [0; 3]);
    }

    /// [EF2] §5.2 Corrects: `[1, 6)` is not universal. The narrower claim —
    /// everything that reaches a brush or game record is `[1, 6)` — is the one
    /// worth asserting.
    #[test]
    fn lod_ranges_are_not_uniform() {
        // The measured census over the 3373 shipped components.
        assert!(LodRange { min: 1, max: 6 }.is_full());
        for r in [
            LodRange { min: 5, max: 6 },
            LodRange { min: 4, max: 5 },
            LodRange { min: 3, max: 4 },
            LodRange { min: 2, max: 3 },
            LodRange { min: 1, max: 2 },
            LodRange { min: 4, max: 6 },
            LodRange { min: 1, max: 3 },
            LodRange { min: 3, max: 6 },
            LodRange { min: 3, max: 5 },
            LodRange { min: 1, max: 4 },
            LodRange { min: 2, max: 6 },
            LodRange { min: 1, max: 5 },
        ] {
            assert!(!r.is_full(), "{r:?} is not [1, 6)");
            assert!(!r.reaches_table(), "{r:?} cannot reach a brush or game record");
        }
        // The claim the simulation depends on: a component that reaches a
        // brush or game record is `[1, 6)`, so it exists at every level of
        // detail the game can show.
        assert!(LodRange { min: 1, max: 6 }.reaches_table());
        assert!(LodRange { min: 1, max: 6 }.contains(1));
        assert!(LodRange { min: 1, max: 6 }.contains(5));
        assert!(!LodRange { min: 1, max: 6 }.contains(6), "the range is [min, max)");
    }

    /// [EF3] §7 Corrects: the description bit is not the tested bit. It is
    /// copied to runtime `comp+0x40` bit 2 (value 4), and a table component
    /// also gets bit 1 (value 2).
    #[test]
    fn the_finish_test_tests_the_runtime_bit() {
        // Particles and decals carry flags = 0 and are exactly the components
        // that hold an effect: they count.
        for kind in [
            component_type::PARTICLES,
            component_type::DECAL,
            component_type::META,
            component_type::MODEL,
        ] {
            assert_eq!(runtime_flags_for(0, kind), 0, "kind {kind} counts");
            assert_eq!(runtime_flags_for(desc_flags::IGNORED, kind), 4);
        }
        // The ignore bit is uniform on game, attractor and camera.
        for kind in [component_type::ATTRACTOR, component_type::GAME, component_type::CAMERA] {
            assert_eq!(runtime_flags_for(desc_flags::IGNORED, kind), 6);
            assert_eq!(runtime_flags_for(0, kind), 2);
        }
        // A brush is a table type but is not ignored in the shipped data.
        assert_eq!(runtime_flags_for(0, component_type::BRUSH), 2);
        assert_eq!(runtime_flags_for(desc_flags::IGNORED, component_type::BRUSH), 6);
        // Model (16) is outside the 3-15 table range.
        assert_eq!(runtime_flags_for(0, component_type::MODEL), 0);
    }

    /// The name map ends at the name `end` or at a length of `0xFFFFFFFF`.
    #[test]
    fn name_map_ends_at_end_or_the_sentinel() {
        let mut v = Vec::new();
        v.extend_from_slice(&3u32.to_le_bytes());
        v.extend_from_slice(b"abc");
        v.extend_from_slice(&7u32.to_le_bytes());
        v.extend_from_slice(&3u32.to_le_bytes());
        v.extend_from_slice(b"end");
        v.extend_from_slice(&8u32.to_le_bytes());
        v.extend_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
        let entries = read_name_map(&v).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].description_index, 7);
        assert_eq!(entries[1].name, b"end");

        let mut w = Vec::new();
        w.extend_from_slice(&1u32.to_le_bytes());
        w.extend_from_slice(b"z");
        w.extend_from_slice(&2u32.to_le_bytes());
        w.extend_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
        assert_eq!(read_name_map(&w).unwrap().len(), 1);
    }

    /// A truncated component record is a clean error, never a panic.
    #[test]
    fn truncation_is_an_error() {
        let v = sample_component(b"abc");
        for cut in [0, 1, 4, 8, 20] {
            let mut r = Reader::new(&v[..cut], "t");
            assert!(read_component(&mut r).is_err(), "cut at {cut}");
        }
    }

    /// §8.1: the brush record is 39 bytes packed, with no padding after the
    /// one-shot byte.
    #[test]
    fn brush_record_is_39_bytes() {
        let mut v = Vec::new();
        v.extend_from_slice(&0u16.to_le_bytes()); // version
        v.extend_from_slice(&0xCA31_3035u32.to_le_bytes()); // VolcanoBrush
        v.extend_from_slice(&30f32.to_le_bytes()); // rate
        v.extend_from_slice(&3f32.to_le_bytes()); // length
        v.push(0); // one-shot
        v.extend_from_slice(&1u32.to_le_bytes()); // level
        v.extend_from_slice(&1f32.to_le_bytes()); // strength s0
        v.extend_from_slice(&1f32.to_le_bytes()); // s1
        v.extend_from_slice(&0.5f32.to_le_bytes()); // radius w0
        v.extend_from_slice(&0.5f32.to_le_bytes()); // w1
        v.extend_from_slice(&100f32.to_le_bytes()); // target height
        assert_eq!(v.len(), BRUSH_RECORD_SIZE, "§8.1 pins the record at 39 bytes");
        assert_eq!(BRUSH_RECORD_SIZE, 2 + 4 + 4 + 4 + 1 + 4 + 4 + 4 + 4 + 4 + 4);

        let mut r = Reader::new(&v, "brush");
        let d = read_brush_description(&mut r).unwrap();
        assert_eq!(r.remaining(), 0);
        assert_eq!(d.version, 0);
        assert_eq!(d.brush_id, 0xCA31_3035, "VolcanoBrush");
        assert_eq!(d.rate, 30.0);
        assert_eq!(d.length, 3.0);
        assert_eq!(d.one_shot, 0);
        assert_eq!(d.level, 1);
        assert_eq!(d.strength, [1.0, 1.0]);
        assert_eq!(d.radius_scale, [0.5, 0.5]);
        assert_eq!(d.target_height, 100.0);

        // Section form: a u32 count, then the records.
        let mut s = Vec::new();
        s.extend_from_slice(&1u32.to_le_bytes());
        s.extend_from_slice(&v);
        let list = read_brush_section(&s).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0], d);
    }

    /// §3 step 4: an attractor record is a `u16` version, a string and a `u8`.
    #[test]
    fn attractor_record_is_version_string_byte() {
        let mut v = Vec::new();
        v.extend_from_slice(&1u16.to_le_bytes());
        v.extend_from_slice(&7u32.to_le_bytes());
        v.extend_from_slice(b"volcano");
        v.push(3);
        let mut r = Reader::new(&v, "attractor");
        let d = read_attractor_description(&mut r).unwrap();
        assert_eq!(r.remaining(), 0);
        assert_eq!(d.version, 1);
        assert_eq!(d.name, b"volcano");
        assert_eq!(d.field, 3);

        let mut s = Vec::new();
        s.extend_from_slice(&1u32.to_le_bytes());
        s.extend_from_slice(&v);
        assert_eq!(read_attractor_section(&s).unwrap().len(), 1);
    }

    /// §9.1: the game record is 70 bytes at version 1, and `+0x28` / `+0x2C`
    /// come **last** — the stream order is not the offset order.
    #[test]
    fn game_record_is_70_bytes_at_version_one() {
        let mut v = Vec::new();
        v.extend_from_slice(&1u16.to_le_bytes()); // version
        v.extend_from_slice(&0x1302u32.to_le_bytes()); // flags, masked to 7 bits
        v.extend_from_slice(&9u32.to_le_bytes()); // +0x10
        v.extend_from_slice(&10u32.to_le_bytes()); // +0x14 demolish radius
        v.extend_from_slice(&2u32.to_le_bytes()); // +0x18 demolish flags
        v.extend_from_slice(&3f32.to_le_bytes()); // +0x1C min
        v.extend_from_slice(&4f32.to_le_bytes()); // +0x20 max
        v.extend_from_slice(&14u32.to_le_bytes()); // +0x24 burn
        v.extend_from_slice(&0x2A7B_582Eu32.to_le_bytes()); // +0x30 message type
        v.extend_from_slice(&7u32.to_le_bytes()); // +0x34 message data
        v.extend_from_slice(&5u32.to_le_bytes()); // +0x38 map kind
        v.extend_from_slice(&16f32.to_le_bytes()); // +0x3C
        v.extend_from_slice(&2f32.to_le_bytes()); // +0x40
        v.extend_from_slice(&3f32.to_le_bytes()); // +0x44
        v.extend_from_slice(&4f32.to_le_bytes()); // +0x48
        v.extend_from_slice(&5f32.to_le_bytes()); // +0x4C duration
        v.extend_from_slice(&20u32.to_le_bytes()); // +0x28 toxic, last
        v.extend_from_slice(&10000u32.to_le_bytes()); // +0x2C extinguish, last
        assert_eq!(v.len(), GAME_RECORD_SIZE_V1, "§9.1 pins the record at 70 bytes");

        let mut r = Reader::new(&v, "game");
        let d = read_game_description(&mut r).unwrap();
        assert_eq!(r.remaining(), 0);
        assert_eq!(d.version, 1);
        // 0x1302 has bits outside the low 7, so the mask bites.
        assert_eq!(d.flags, 0x1302 & 0x7F);
        assert_eq!(d.field_10, 9);
        assert_eq!(d.demolish_radius, 10);
        assert_eq!(d.demolish_flags, 2);
        assert_eq!(d.min_demolish_size, 3.0);
        assert_eq!(d.max_demolish_size, 4.0);
        assert_eq!(d.burn_radius, 14);
        assert_eq!(d.message_type, 0x2A7B_582E);
        assert_eq!(d.message_data, 7);
        assert_eq!(d.map_kind, 5);
        assert_eq!(d.map_params, [16.0, 2.0, 3.0, 4.0]);
        assert_eq!(d.duration, 5.0);
        assert_eq!(d.toxic_amount, 20, "+0x28 is read after +0x4C");
        assert_eq!(d.extinguish_amount, 10000);

        let mut s = Vec::new();
        s.extend_from_slice(&1u32.to_le_bytes());
        s.extend_from_slice(&v);
        assert_eq!(read_game_section(&s).unwrap().len(), 1);
    }

    /// §9.1: at version 0 the toxic and extinguish amounts are not in the
    /// record at all, so they read as 0 and the record is 62 bytes.
    #[test]
    fn game_record_version_zero_has_no_toxic_or_extinguish() {
        let mut v = Vec::new();
        v.extend_from_slice(&0u16.to_le_bytes());
        // 15 four-byte fields (flags .. duration), no trailing +0x28 / +0x2C.
        for _ in 0..15 {
            v.extend_from_slice(&0u32.to_le_bytes());
        }
        assert_eq!(v.len(), GAME_RECORD_SIZE_V1 - 8);
        let mut r = Reader::new(&v, "game");
        let d = read_game_description(&mut r).unwrap();
        assert_eq!(r.remaining(), 0);
        assert_eq!(d.version, 0);
        assert_eq!(d.toxic_amount, 0);
        assert_eq!(d.extinguish_amount, 0);
    }

    /// A truncated brush or game record is a clean error, never a panic.
    #[test]
    fn brush_and_game_truncation_is_an_error() {
        let mut brush = Vec::new();
        brush.extend_from_slice(&0u16.to_le_bytes());
        brush.extend_from_slice(&[0u8; BRUSH_RECORD_SIZE - 2]);
        for cut in [0, 1, 2, 6, BRUSH_RECORD_SIZE - 1] {
            let mut r = Reader::new(&brush[..cut], "brush");
            assert!(read_brush_description(&mut r).is_err(), "cut at {cut}");
        }

        let mut game = Vec::new();
        game.extend_from_slice(&1u16.to_le_bytes());
        game.extend_from_slice(&[0u8; GAME_RECORD_SIZE_V1 - 2]);
        for cut in [0, 2, 20, GAME_RECORD_SIZE_V1 - 1] {
            let mut r = Reader::new(&game[..cut], "game");
            assert!(read_game_description(&mut r).is_err(), "cut at {cut}");
        }
    }
}
