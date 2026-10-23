// SPDX-License-Identifier: GPL-3.0-or-later
//! Pure owners from overlay-views.md and network-overlay-views.md.
//!
//! This module is unbound. `Gap` is a real failure, never a successful no-op.
//! Constructors take documented state explicitly rather than invent native defaults.
//! Providers expose foreign primitives only; geometry, traversal and batching stay here.
//! Owned `Arc`s / `Box`es model acquired resources; borrowed providers are never released.
//! See METHOD_CHECKLIST.md for the public coverage and the outstanding contracts.

mod cells;
mod network;
mod path;
mod vertex_buffer;

pub use cells::{Cell, CellTerrain, Lot, LotFilter, SC4MarkedCellView, Split, LOT_FILTER_MASK};
pub use network::{
    Network, NetworkOccupant, NetworkPath, RouteSet, SC4PathInfoView, SC4TrainJunctionView, TrainResourceFactory,
    TrainResources,
};
pub use path::{cSC4PathView, PathInfo};
pub use vertex_buffer::{cSTEVertBufExtn, TerrainRows, VertexBuffer, VertexBufferDriver};

use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gap {
    PathAttributesOvv2,
    ArrowheadGeometryOvv3,
    PackedColourOvv4,
    PathInfoColourNov2,
    TrainGeometryNov3,
    PublishedLifecycleOvv6,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Gap(Gap),
    NotInitialized,
    NoCurrentPath,
    InvalidInput,
    OutOfBounds,
    MissingIndexSource,
    ResourceUnavailable,
    LockFailed,
    DrawFailed,
    Overflow,
    /// Portable safety boundary, not a native false return.
    UnsupportedDomain,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Point {
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub(crate) fn finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }

    pub(crate) fn distance(self, other: Self) -> f32 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2) + (self.z - other.z).powi(2)).sqrt()
    }
}

pub(crate) fn unit_xz(a: Point, b: Point) -> Option<[f32; 2]> {
    let dx = b.x - a.x;
    let dz = b.z - a.z;
    let len = dx.hypot(dz);
    (len > 0.0).then(|| [dx / len, dz / len])
}

/// RGBA channels, deliberately not a claimed native packed word (OVV4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Colour(pub [u8; 4]);

pub trait Texture: Send + Sync {}
pub trait Model: Send + Sync {}

pub trait TextureFactory {
    /// An opaque key; no guessed resource-key layout.
    fn bind(&self, key: u32) -> Option<Arc<dyn Texture>>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DepthTest {
    Disabled,
    LessOrEqual,
    Greater,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Blend {
    Disabled,
    SourceAlpha,
    Additive,
}

/// Semantic state fields only. No guessed native packed flags.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DrawState {
    pub colour_write: Option<bool>,
    pub depth_write: bool,
    pub depth: DepthTest,
    pub culling: Option<bool>,
    pub blend: Blend,
    pub textures: u8,
    pub alpha_greater_than: Option<f32>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vertex {
    pub position: Point,
    pub colour: Colour,
    pub uv: [f32; 2],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Primitive {
    TriangleStrip,
    Triangles,
    LineStrip,
    Lines,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Draw {
    /// None when the public spec does not identify a native format (NOV2).
    pub format: Option<u32>,
    pub primitive: Primitive,
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

/// Raw draw/device operations; no original-owner geometry comes back from this interface.
pub trait DrawContext {
    fn depth_offset(&self) -> f32;
    fn set_depth_offset(&mut self, value: f32);
    fn set_identity_model_transform(&mut self);
    fn set_state(&mut self, state: DrawState);
    fn set_texture(&mut self, texture: Option<&Arc<dyn Texture>>);
    fn set_repeat_wrapping(&mut self);
    fn draw(&mut self, draw: &Draw) -> Result<(), Error>;
}

/// Half-open bounds are a core representation, not a claim about a native RECT ABI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellRect {
    pub min: [i32; 2],
    pub max: [i32; 2],
}

impl CellRect {
    pub fn cells(self) -> impl Iterator<Item = [i32; 2]> {
        // Literal row traversal; native traversal order is part of NOV2/NOV3.
        (self.min[1]..self.max[1]).flat_map(move |z| (self.min[0]..self.max[0]).map(move |x| [x, z]))
    }
}

#[cfg(test)]
mod tests;

