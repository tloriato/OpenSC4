//! Lot and growth-occupant save records (`docs/specs/lot-records.md`, "lr").
//!
//! The records growth writes for a lot that are not owned by another core
//! module: the lot (`cSC4Lot`, §3), the business unit (§4), the building
//! occupant (§5), the lot base-texture occupant (§6), the retaining-wall
//! occupant (§7), the foundation occupant (§8) and the bare occupant (§12).
//! Props and effect props live in [`crate::prop_occupant`], flora in
//! [`crate::flora`], trips in [`crate::traffic_simulator`] (reused here).
//!
//! Every record is a plain value type with `write` / `read` following §2.2,
//! and a [`SaveObject`] wrapper ([`RecordObject`], [`lot::Lot`],
//! [`lot::BusinessUnit`]) so the class-object store can create, write and read
//! them by class ID ([`create`]).
//!
//! The lot's after-load work (§3.5 `Init` / `SetFlags` / `PostCityInit` /
//! `Shutdown`) and business-unit membership are in [`lifecycle`]. The
//! construction record and supported lifecycle are in [`construction`].
//!
//! Not here (partial family): the construction visual/schedule contracts
//! OP13-OP16, signpost and text decal (§10.3, §10.4), retaining-wall
//! construction (§7, §15), and the occupant-manager cell-rect rule (§16 Q1).

pub mod construction;
pub mod lifecycle;
pub mod lot;
pub mod mutation;
pub mod walls;

#[cfg(test)]
mod lifecycle_tests;
#[cfg(test)]
mod mutation_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_construction;
#[cfg(test)]
mod walls_tests;

use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;

use crate::automata::OccupantBase;
use crate::stream::{InStream, OutStream, RefValue, SaveObject};

pub use crate::automata::OCCUPANT_BASE_VERSION;
pub use lot::{BusinessUnit, BusinessUnitRecord, Lot, LotRecord};

// ---- class IDs (§1) --------------------------------------------------------------------------------

pub const CLSID_LOT: u32 = 0xC9BD_5D4A;
pub const CLSID_BUSINESS_UNIT: u32 = 0x4A23_2DA8;
pub const CLSID_OCCUPANT: u32 = 0xA9BC_9AB6;
pub const CLSID_BUILDING_OCCUPANT: u32 = 0xA9BD_882D;
pub const CLSID_BASE_TEXTURE_OCCUPANT: u32 = 0xC97F_987C;
pub const CLSID_RETAINING_WALL_OCCUPANT: u32 = 0x49C0_5C9F;
pub const CLSID_FOUNDATION_OCCUPANT: u32 = 0x49C0_5C8F;

// ---- interface IDs requested on read (§1, §14) ----------------------------------------------------

pub const IID_LOT: u32 = 0xE818_FCED;
pub const IID_BUSINESS_UNIT: u32 = 0x6A23_3069;
pub const IID_BUILDING_OCCUPANT: u32 = 0x87DD_A3A9;

// ---- occupant types (§2.3 field 5) ----------------------------------------------------------------

pub const OCCUPANT_TYPE_BUILDING: u32 = 0x2781_28A0;
pub const OCCUPANT_TYPE_BASE_TEXTURE: u32 = 0x497F_6D9D;
pub const OCCUPANT_TYPE_RETAINING_WALL: u32 = 0x895D_1169;
pub const OCCUPANT_TYPE_FOUNDATION: u32 = 0x68FD_0C69;
pub const OCCUPANT_TYPE_BARE: u32 = 0x7475_8925;

/// `CreateOccupant`, posted by `Init` for occupants that were not loaded (§2.4).
pub const MSG_CREATE_OCCUPANT: u32 = 0x99EF_1140;

// ---- record versions (§1 table) -------------------------------------------------------------------

pub const BUILDING_RECORD_VERSION: u16 = 2;
pub const BASE_TEXTURE_RECORD_VERSION: u16 = 2;
pub const RETAINING_WALL_RECORD_VERSION: u16 = 2;
pub const FOUNDATION_RECORD_VERSION: u16 = 2;

/// An axis-aligned box `(min, max)` (§2.1 `box`).
pub type Bounds = ([f32; 3], [f32; 3]);

// ---- §2.1 encodings ---------------------------------------------------------------------------------

pub(crate) fn put_vec3(out: &mut dyn OutStream, v: &[f32; 3]) {
    for &c in v {
        out.put_f32(c);
    }
}

pub(crate) fn get_vec3(inp: &mut dyn InStream) -> Option<[f32; 3]> {
    Some([inp.get_f32()?, inp.get_f32()?, inp.get_f32()?])
}

pub(crate) fn put_box(out: &mut dyn OutStream, b: &Bounds) {
    put_vec3(out, &b.0);
    put_vec3(out, &b.1);
}

pub(crate) fn get_box(inp: &mut dyn InStream) -> Option<Bounds> {
    Some((get_vec3(inp)?, get_vec3(inp)?))
}

/// Reads the subclass version (must equal `version`) and then the base block
/// inline (§2.2): `None` when either fails, the base untouched then.
fn read_header(inp: &mut dyn InStream, version: u16, base: &mut OccupantBase) -> Option<()> {
    if inp.get_u16()? != version {
        return None;
    }
    *base = OccupantBase::read(inp)?;
    Some(())
}

// ---- §2.4 occupant Init after a load --------------------------------------------------------------

/// Status bit 0: initialised (§2.4).
pub const STATUS_INITIALISED: u8 = 1;
/// Status bit 1: loaded from a save (§2.3 `Read`).
pub const STATUS_LOADED: u8 = 2;

/// What occupant `Init` (`0x005E8C90`, §2.4) did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InitOutcome {
    /// `IsInitialized()` was already true: nothing changed.
    AlreadyInitialised,
    /// Not loaded: the caller posts `PostOccupantMessage(CreateOccupant, 0)`.
    PostCreateOccupant,
    /// Loaded: `SetHighlight(0, false)` was applied, no message.
    HighlightCleared,
}

/// §2.4 on the base block. Returns true in the original in every case.
pub fn occupant_init(base: &mut OccupantBase) -> InitOutcome {
    if base.status & STATUS_INITIALISED != 0 {
        return InitOutcome::AlreadyInitialised;
    }
    base.status |= STATUS_INITIALISED;
    if base.status & STATUS_LOADED == 0 {
        InitOutcome::PostCreateOccupant
    } else {
        // SetHighlight(0, false): §2.3 keeps the highlight in the status
        // byte's high nibble after a read, so both views go to 0 (literal
        // reading of "the highlight byte becomes the status byte as
        // (highlight << 4) | 2").
        base.highlight = 0;
        base.status &= 0x0F;
        InitOutcome::HighlightCleared
    }
}

// ---- the record trait and its store object --------------------------------------------------------

/// One occupant record of this module (§5–§8, §12).
pub trait OccupantRecord: Clone + std::fmt::Debug + 'static {
    /// The class ID of the record (§1).
    const CLASS_ID: u32;
    /// A freshly constructed record (constructor defaults).
    fn new() -> Self;
    /// The occupant base block.
    fn base(&self) -> &OccupantBase;
    fn base_mut(&mut self) -> &mut OccupantBase;
    /// `Write` (§2.2): true exactly when the stream has no error afterwards.
    fn write(&self, out: &mut dyn OutStream) -> bool;
    /// `Read` (§2.2): false on a rejected version, else `GetError() == 0`.
    fn read(&mut self, inp: &mut dyn InStream) -> bool;
}

/// A store object around one [`OccupantRecord`].
#[derive(Debug)]
pub struct RecordObject<R: OccupantRecord> {
    pub rec: RefCell<R>,
}

impl<R: OccupantRecord> RecordObject<R> {
    pub fn new(rec: R) -> Rc<Self> {
        Rc::new(RecordObject { rec: RefCell::new(rec) })
    }
}

impl<R: OccupantRecord> SaveObject for RecordObject<R> {
    fn class_id(&self) -> u32 {
        R::CLASS_ID
    }
    fn write(&self, out: &mut dyn OutStream) -> bool {
        self.rec.borrow().write(out)
    }
    fn read(&self, inp: &mut dyn InStream) -> bool {
        self.rec.borrow_mut().read(inp)
    }
    fn as_any_rc(self: Rc<Self>) -> Rc<dyn Any> {
        self
    }
}

/// Creates a fresh object of one of this module's classes, as the class
/// registry would for `GetClassObject(classID, cIGZSerializable)` (§1, §15);
/// `None` for any other class.
pub fn create(class_id: u32) -> Option<RefValue> {
    let o: Rc<dyn SaveObject> = match class_id {
        CLSID_LOT => Rc::new(Lot::default()),
        CLSID_BUSINESS_UNIT => Rc::new(BusinessUnit::default()),
        CLSID_OCCUPANT => RecordObject::new(BareOccupantRecord::new()),
        CLSID_BUILDING_OCCUPANT => RecordObject::new(BuildingRecord::new()),
        CLSID_BASE_TEXTURE_OCCUPANT => RecordObject::new(BaseTextureRecord::new()),
        CLSID_RETAINING_WALL_OCCUPANT => RecordObject::new(RetainingWallRecord::new()),
        CLSID_FOUNDATION_OCCUPANT => RecordObject::new(FoundationRecord::new()),
        construction::CLSID_CONSTRUCTION_OCCUPANT => RecordObject::new(construction::ConstructionRecord::new()),
        _ => return None,
    };
    Some(RefValue::Object(o))
}

// ---- §12 bare occupant ------------------------------------------------------------------------------

/// `cSC4Occupant` bare instance (§12): the base block only.
#[derive(Clone, Debug, PartialEq)]
pub struct BareOccupantRecord {
    pub base: OccupantBase,
}

impl OccupantRecord for BareOccupantRecord {
    const CLASS_ID: u32 = CLSID_OCCUPANT;
    fn new() -> Self {
        BareOccupantRecord {
            base: OccupantBase::new(OCCUPANT_TYPE_BARE),
        }
    }
    fn base(&self) -> &OccupantBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut OccupantBase {
        &mut self.base
    }
    fn write(&self, out: &mut dyn OutStream) -> bool {
        self.base.write(out) && out.error() == 0
    }
    /// §2.3: version 4 only; anything else → false, nothing more read.
    fn read(&mut self, inp: &mut dyn InStream) -> bool {
        match OccupantBase::read(inp) {
            Some(b) => {
                self.base = b;
                inp.error() == 0
            }
            None => false,
        }
    }
}

// ---- §5 building occupant ---------------------------------------------------------------------------

/// `cSC4BuildingOccupant` record (§5).
#[derive(Clone, Debug, PartialEq)]
pub struct BuildingRecord {
    pub base: OccupantBase,
    /// #3: the building type ID.
    pub building_type: u32,
    /// #4: bit 0 lit.
    pub building_flags: u8,
    /// #5: world bounding box.
    pub bounds: Bounds,
    /// #6: 0…3.
    pub orientation: i8,
    /// #7: `SC4Percentage` completion, 0…1.
    pub completion: f32,
}

impl OccupantRecord for BuildingRecord {
    const CLASS_ID: u32 = CLSID_BUILDING_OCCUPANT;
    fn new() -> Self {
        // Fields the spec gives no constructor default for are 0 [choice].
        BuildingRecord {
            base: OccupantBase::new(OCCUPANT_TYPE_BUILDING),
            building_type: 0,
            building_flags: 0,
            bounds: ([0.0; 3], [0.0; 3]),
            orientation: 0,
            completion: 0.0,
        }
    }
    fn base(&self) -> &OccupantBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut OccupantBase {
        &mut self.base
    }
    fn write(&self, out: &mut dyn OutStream) -> bool {
        out.put_u16(BUILDING_RECORD_VERSION);
        self.base.write(out);
        out.put_u32(self.building_type);
        out.put_u8(self.building_flags);
        put_box(out, &self.bounds);
        out.put_i8(self.orientation);
        out.put_f32(self.completion);
        out.error() == 0
    }
    /// §5: version 2 and a good base, else false.
    fn read(&mut self, inp: &mut dyn InStream) -> bool {
        if read_header(inp, BUILDING_RECORD_VERSION, &mut self.base).is_none() {
            return false;
        }
        let _ = (|| -> Option<()> {
            self.building_type = inp.get_u32()?;
            self.building_flags = inp.get_u8()?;
            self.bounds = get_box(inp)?;
            self.orientation = inp.get_i8()?;
            self.completion = inp.get_f32()?;
            Some(())
        })();
        inp.error() == 0
    }
}

// ---- §6 lot base-texture occupant -------------------------------------------------------------------

/// The frame byte value that `SetTextureSpecification` replaces (§6).
pub const FRAME_UNSET: u8 = 0xFF;

/// One textured cell of a base-texture occupant (§6, 14 bytes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextureRecord {
    pub texture_instance: u32,
    pub cell_x: u8,
    pub cell_z: u8,
    /// 0…3.
    pub orientation: u8,
    /// The configuration object's byte 3 (0, 1).
    pub config_byte: u8,
    /// Five unnamed bytes, constructor default `0xFF` (§16 Q4).
    pub unnamed: [u8; 5],
    /// 0…7.
    pub frame: u8,
}

impl Default for TextureRecord {
    /// The constructor (`0x005CA770`) sets the five unnamed bytes to `0xFF`;
    /// the other fields have no stated default and are 0 [choice].
    fn default() -> Self {
        TextureRecord {
            texture_instance: 0,
            cell_x: 0,
            cell_z: 0,
            orientation: 0,
            config_byte: 0,
            unnamed: [0xFF; 5],
            frame: 0,
        }
    }
}

impl TextureRecord {
    pub fn write(&self, out: &mut dyn OutStream) {
        out.put_u32(self.texture_instance);
        out.put_u8(self.cell_x);
        out.put_u8(self.cell_z);
        out.put_u8(self.orientation);
        out.put_u8(self.config_byte);
        for &b in &self.unnamed {
            out.put_u8(b);
        }
        out.put_u8(self.frame);
    }

    pub fn read(&mut self, inp: &mut dyn InStream) -> Option<()> {
        self.texture_instance = inp.get_u32()?;
        self.cell_x = inp.get_u8()?;
        self.cell_z = inp.get_u8()?;
        self.orientation = inp.get_u8()?;
        self.config_byte = inp.get_u8()?;
        for b in &mut self.unnamed {
            *b = inp.get_u8()?;
        }
        self.frame = inp.get_u8()?;
        Some(())
    }
}

/// `cSC4LotBaseTextureOccupant` record (§6).
#[derive(Clone, Debug, PartialEq)]
pub struct BaseTextureRecord {
    pub base: OccupantBase,
    pub bounds: Bounds,
    /// 1 or 2.
    pub usage: i8,
    pub textures: Vec<TextureRecord>,
}

/// The largest vector a failed read pads to n with default records
/// [choice]: the original resizes to n with no bound (§6) before reading; the
/// core reads record by record and only pads a short read up to this many, so
/// a corrupt count cannot exhaust memory.
pub const MAX_TEXTURE_PAD: usize = 65_536;

impl BaseTextureRecord {
    /// `SetTextureSpecification` (`0x005E0B20`, §6): sets the records; each
    /// frame byte of `0xFF` becomes `draw(frames)` (the base-texture file's
    /// `RandomUint32Uniform`) when `frame_count(texture)` is ≥ 2, else 0: one
    /// draw per such record, in record order. Saved frames are not touched by
    /// a load (this is not called by `read`).
    pub fn set_textures(
        &mut self,
        textures: Vec<TextureRecord>,
        frame_count: &dyn Fn(u32) -> u32,
        draw: &mut dyn FnMut(u32) -> u32,
    ) {
        self.textures = textures;
        for t in &mut self.textures {
            if t.frame == FRAME_UNSET {
                let frames = frame_count(t.texture_instance);
                // The drawn value is stored in the u8 frame byte (truncation
                // of a draw below `frames` [choice: frames ≤ 256 in practice]).
                t.frame = if frames >= 2 { draw(frames) as u8 } else { 0 };
            }
        }
    }
}

impl OccupantRecord for BaseTextureRecord {
    const CLASS_ID: u32 = CLSID_BASE_TEXTURE_OCCUPANT;
    fn new() -> Self {
        BaseTextureRecord {
            base: OccupantBase::new(OCCUPANT_TYPE_BASE_TEXTURE),
            bounds: ([0.0; 3], [0.0; 3]),
            usage: 0,
            textures: Vec::new(),
        }
    }
    fn base(&self) -> &OccupantBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut OccupantBase {
        &mut self.base
    }
    fn write(&self, out: &mut dyn OutStream) -> bool {
        out.put_u16(BASE_TEXTURE_RECORD_VERSION);
        self.base.write(out);
        put_box(out, &self.bounds);
        out.put_i8(self.usage);
        out.put_u32(self.textures.len() as u32);
        for t in &self.textures {
            t.write(out);
        }
        out.error() == 0
    }
    fn read(&mut self, inp: &mut dyn InStream) -> bool {
        if read_header(inp, BASE_TEXTURE_RECORD_VERSION, &mut self.base).is_none() {
            return false;
        }
        let _ = (|| -> Option<()> {
            self.bounds = get_box(inp)?;
            self.usage = inp.get_i8()?;
            let n = inp.get_u32()? as usize;
            // §6: the vector is resized to n, then each record is read over
            // the existing element (or a default one).
            self.textures.truncate(n);
            for i in 0..n {
                if i == self.textures.len() {
                    self.textures.push(TextureRecord::default());
                }
                if self.textures[i].read(inp).is_none() {
                    if n <= MAX_TEXTURE_PAD {
                        self.textures.resize(n, TextureRecord::default());
                    }
                    return None;
                }
            }
            Some(())
        })();
        inp.error() == 0
    }
}

// ---- §7 retaining-wall occupant ---------------------------------------------------------------------

/// `cSC4LotRetainingWallOccupant` record (§7).
#[derive(Clone, Debug)]
pub struct RetainingWallRecord {
    pub base: OccupantBase,
    /// #3: `0xC96D2135…7`.
    pub wall_type: u32,
    /// #4: bit i = wall on edge i.
    pub edge_mask: i32,
    /// #5: one per edge, 0 = none.
    pub edge_hashes: [i32; 4],
    pub position: [f32; 3],
    pub size: [f32; 3],
    /// #8: the lot (read IID `0xE818FCED`); never null in saves.
    pub lot: RefValue,
}

impl RetainingWallRecord {
    /// Compares every field except the lot reference (`RefValue` has no
    /// equality).
    pub fn same_fields(&self, o: &Self) -> bool {
        self.base == o.base
            && self.wall_type == o.wall_type
            && self.edge_mask == o.edge_mask
            && self.edge_hashes == o.edge_hashes
            && self.position == o.position
            && self.size == o.size
    }
}

impl OccupantRecord for RetainingWallRecord {
    const CLASS_ID: u32 = CLSID_RETAINING_WALL_OCCUPANT;
    fn new() -> Self {
        RetainingWallRecord {
            base: OccupantBase::new(OCCUPANT_TYPE_RETAINING_WALL),
            wall_type: 0,
            edge_mask: 0,
            edge_hashes: [0; 4],
            position: [0.0; 3],
            size: [0.0; 3],
            lot: RefValue::Null,
        }
    }
    fn base(&self) -> &OccupantBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut OccupantBase {
        &mut self.base
    }
    fn write(&self, out: &mut dyn OutStream) -> bool {
        out.put_u16(RETAINING_WALL_RECORD_VERSION);
        self.base.write(out);
        out.put_u32(self.wall_type);
        out.put_i32(self.edge_mask);
        for &h in &self.edge_hashes {
            out.put_i32(h);
        }
        put_vec3(out, &self.position);
        put_vec3(out, &self.size);
        out.put_ref(&self.lot);
        out.error() == 0
    }
    /// §7: releases the old lot first (literal order: before the version
    /// check), then reads; each non-zero hash i ORs bit i into the mask, so
    /// the mask grows on load *[quirk]* (T7).
    fn read(&mut self, inp: &mut dyn InStream) -> bool {
        self.lot = RefValue::Null;
        if read_header(inp, RETAINING_WALL_RECORD_VERSION, &mut self.base).is_none() {
            return false;
        }
        let _ = (|| -> Option<()> {
            self.wall_type = inp.get_u32()?;
            self.edge_mask = inp.get_i32()?;
            for i in 0..4 {
                let h = inp.get_i32()?;
                self.edge_hashes[i] = h;
                if h != 0 {
                    self.edge_mask |= 1 << i;
                }
            }
            self.position = get_vec3(inp)?;
            self.size = get_vec3(inp)?;
            self.lot = inp.get_ref_as(IID_LOT)?;
            Some(())
        })();
        inp.error() == 0
    }
}

// ---- §8 foundation occupant -------------------------------------------------------------------------

/// `cSC4FoundationOccupant` record (§8).
#[derive(Clone, Debug, PartialEq)]
pub struct FoundationRecord {
    pub base: OccupantBase,
    pub foundation_type: u32,
    /// 0…32.
    pub max_height: f32,
    pub position: [f32; 3],
    pub size: [f32; 3],
    /// 1, 2.
    pub usage: i8,
    /// 0…3.
    pub orientation: i8,
}

impl OccupantRecord for FoundationRecord {
    const CLASS_ID: u32 = CLSID_FOUNDATION_OCCUPANT;
    fn new() -> Self {
        FoundationRecord {
            base: OccupantBase::new(OCCUPANT_TYPE_FOUNDATION),
            foundation_type: 0,
            max_height: 0.0,
            position: [0.0; 3],
            size: [0.0; 3],
            usage: 0,
            orientation: 0,
        }
    }
    fn base(&self) -> &OccupantBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut OccupantBase {
        &mut self.base
    }
    fn write(&self, out: &mut dyn OutStream) -> bool {
        out.put_u16(FOUNDATION_RECORD_VERSION);
        self.base.write(out);
        out.put_u32(self.foundation_type);
        out.put_f32(self.max_height);
        put_vec3(out, &self.position);
        put_vec3(out, &self.size);
        out.put_i8(self.usage);
        out.put_i8(self.orientation);
        out.error() == 0
    }
    /// §8 / §1 (version 2 written and accepted): any other version → false.
    fn read(&mut self, inp: &mut dyn InStream) -> bool {
        if read_header(inp, FOUNDATION_RECORD_VERSION, &mut self.base).is_none() {
            return false;
        }
        let _ = (|| -> Option<()> {
            self.foundation_type = inp.get_u32()?;
            self.max_height = inp.get_f32()?;
            self.position = get_vec3(inp)?;
            self.size = get_vec3(inp)?;
            self.usage = inp.get_i8()?;
            self.orientation = inp.get_i8()?;
            Some(())
        })();
        inp.error() == 0
    }
}
