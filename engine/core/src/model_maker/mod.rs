// SPDX-License-Identifier: GPL-3.0-or-later
//! Portable cSC4ModelMaker, from docs/specs/model-maker.md.
//!
//! Handles identify foreign objects; they are never dereferenced here. All foreign
//! operations go through traits. Mutators require exclusive access; getters use
//! shared borrows, with no interior mutability or thread-affine storage.

use std::collections::BTreeMap;

pub mod dynamic_textures;
pub mod entries;
mod geometry;
mod lifecycle;
pub mod materials;
pub mod model_loading;
mod terrain_hiding;
pub mod transform;
pub use lifecycle::{LifecycleHost, INIT_MESSAGES};
pub use terrain_hiding::{TerrainHost, TerrainOccupant};
pub mod persistence;
pub mod archive;
pub use geometry::{
    fast_ceil, fast_floor, lowest_rotation, lowest_zoom, HolderMasks, LoadRectHost, LoadView, UnloadHost,
};

pub const CLASS_ID: u32 = 0xA9C7_41B3;
pub const SERIALIZABLE: u32 = 0xE4FD_A3D4;
pub const ALL_ZOOMS: u32 = 0x1F;
pub const ALL_ROTATIONS: u32 = 0x0F;
pub type Handle = u64;

/// Foreign counted references. Getters borrow unless their trait says otherwise.
pub trait ObjectHost {
    fn retain(&mut self, object: Handle);
    fn release(&mut self, object: Handle);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModelKey {
    pub type_id: u32,
    pub group: u32,
    pub instance: u32,
}

impl ModelKey {
    pub const fn s3d(instance: u32) -> Self {
        Self {
            type_id: 0x5AD0_E817,
            group: 0xBADB_57F1,
            instance,
        }
    }
}

#[derive(Debug)]
struct DeferredEntry {
    occupant: Handle,
    operation: u8,
    arguments: [u8; 2],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interface {
    Primary,
    Save,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rect {
    pub x0: i32,
    pub z0: i32,
    pub x1: i32,
    pub z1: i32,
}

impl Rect {
    pub const fn new(x0: i32, z0: i32, x1: i32, z1: i32) -> Self {
        Self { x0, z0, x1, z1 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Port {
    ResourceManager,
    Renderer,
    TickManager,
    OccupantManager,
    ViewFilter,
    TypeFilter,
    HidingFilter,
    ZoomRotationFilter,
    LotManager,
    Development,
    Terrain,
    AnimationCoordinator,
    BackgroundLoader,
}

/// Fields in §4, independent of city size or Init order.
#[derive(Clone, Debug, PartialEq)]
pub struct SavedState {
    pub zoom: i32,
    pub rotation: i32,
    pub cells: [i32; 2],
    pub city: Rect,
    pub word: u32,
    pub camera: [f32; 4],
    pub previous_view: [i32; 2],
    pub target_view: [i32; 2],
}

impl Default for SavedState {
    fn default() -> Self {
        Self {
            zoom: -1,
            rotation: -1,
            cells: [0; 2],
            city: Rect::default(),
            word: 0,
            camera: [0.0; 4],
            previous_view: [-1; 2],
            target_view: [-1; 2],
        }
    }
}

#[derive(Debug)]
pub struct ModelMaker {
    pub saved: SavedState,
    pub initialized: bool,
    pub city_subscribed: bool,
    pub read_from_save: bool,
    pub view_suppressed: bool,
    pub suppressed_view: [i32; 2],
    pub bulk_reload: bool,
    pub load_radii: [i32; 5],
    pub load_rect: Rect,
    pub focus: [i32; 2],
    pub background_started: bool,
    pub background_focus: [i32; 2],
    pub background_enabled: bool,
    pub saving: bool,
    pub ignore_missing_model_data: bool,
    pub unload_pending: bool,
    pub translating: bool,
    pub light_colour: [f32; 3],
    pub textures: [Option<Handle>; 5],
    pub detail: i32,
    pub renderer_aux: Option<Handle>,
    pub defer_count: i32,
    deferred: Vec<DeferredEntry>,
    references: u32,
    disposed: bool,
    ports: BTreeMap<Port, Handle>,
}

impl Default for ModelMaker {
    fn default() -> Self {
        Self {
            saved: SavedState::default(),
            initialized: false,
            city_subscribed: false,
            read_from_save: false,
            view_suppressed: false,
            suppressed_view: [0; 2],
            bulk_reload: false,
            load_radii: [0x100; 5],
            load_rect: Rect::default(),
            focus: [0; 2],
            background_started: false,
            background_focus: [0; 2],
            background_enabled: true,
            saving: false,
            ignore_missing_model_data: false,
            unload_pending: false,
            translating: false,
            // SPEC-GAP: the constructor's external colour seed is supplied by Init.
            light_colour: [0.0; 3],
            textures: [None; 5],
            detail: 0,
            renderer_aux: None,
            defer_count: 0,
            deferred: Vec::new(),
            references: 0,
            disposed: false,
            ports: BTreeMap::new(),
        }
    }
}

impl ModelMaker {
    pub fn query_interface(&mut self, iid: u32, out: &mut Option<Interface>) -> bool {
        let interface = match iid {
            CLASS_ID | 1 | 0x4522_94AA => Interface::Primary,
            SERIALIZABLE => Interface::Save,
            _ => return false,
        };
        if self.disposed {
            return false;
        }
        *out = Some(interface);
        self.add_ref();
        true
    }

    pub fn add_ref(&mut self) -> u32 {
        self.references = self.references.wrapping_add(1);
        self.references
    }

    pub fn reference_count(&self) -> u32 {
        self.references
    }

    pub fn port(&self, port: Port) -> Option<Handle> {
        self.ports.get(&port).copied()
    }

    pub fn disposed(&self) -> bool {
        self.disposed
    }

    pub fn deferred_entries(&self) -> impl Iterator<Item = (Handle, u8, [u8; 2])> + '_ {
        self.deferred.iter().map(|e| (e.occupant, e.operation, e.arguments))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn m1_query_interfaces_leave_failed_output_untouched() {
        let mut maker = ModelMaker::default();
        let mut out = None;
        for iid in [CLASS_ID, 1, 0x4522_94AA] {
            assert!(maker.query_interface(iid, &mut out));
            assert_eq!(out, Some(Interface::Primary));
        }
        assert!(maker.query_interface(SERIALIZABLE, &mut out));
        assert_eq!(out, Some(Interface::Save));
        assert!(!maker.query_interface(0x1234_5678, &mut out));
        assert_eq!(out, Some(Interface::Save));
        assert_eq!(maker.reference_count(), 4);
    }

    #[test]
    fn constructor_state_and_thread_safe_borrows() {
        fn send_sync<T: Send + Sync>() {}
        send_sync::<ModelMaker>();
        let maker = ModelMaker::default();
        assert_eq!(maker.saved.zoom, -1);
        assert_eq!(maker.saved.target_view, [-1; 2]);
        assert_eq!(maker.load_radii, [256; 5]);
        assert!(maker.background_enabled);
        assert!(maker.ports.is_empty());
        assert_eq!(maker.textures, [None; 5]);
    }
}
