// SPDX-License-Identifier: GPL-3.0-or-later
//! City renderer owner, from docs/specs/render3d.md. No ABI or plugin wiring.
//!
//! This is a partial owner; see README.md for section closure. Init, initialized
//! teardown, model processing and drawing are not implemented. Do not wire it
//! into a live city until those sections are complete.
//!
//! Handles denote foreign interfaces, never memory addresses. Getters are pure
//! shared borrows. The caller serializes mutations; there is no interior cache.

mod state;
pub mod archive;
pub use state::*;
mod view;
pub use view::*;
mod grid;
pub use grid::*;
mod dirty;
pub use dirty::*;
mod backing;
pub use backing::*;
mod shake;
pub use shake::*;
mod container;
pub use container::*;
mod objects;
pub use objects::*;
mod picking;
pub use picking::*;

pub const CLASS_ID: u32 = 0xE9C6_22D8;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Handle(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Helper {
    ViewUtilities,
    DrawContext,
}

/// Creation transfers ownership, unlike acquisition of an existing interface.
pub trait References {
    fn create_helper(&mut self, kind: Helper) -> Option<Handle>;
    fn destroy_helper(&mut self, handle: Handle);
    fn add_ref(&mut self, handle: Handle);
    fn release(&mut self, handle: Handle);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interface {
    Primary,
    MessageTarget,
    Serializable,
}

impl Interface {
    pub fn offset(self) -> usize {
        match self {
            Self::Primary => 0,
            Self::MessageTarget => 4,
            Self::Serializable => 8,
        }
    }
}

/// Explicit boundaries for the two questions left open by the specification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unsupported {
    Rnd1StreamSlot8,
    Rnd4ShakeOverload,
}

/// One city render owner. Foreign objects are retained only at specified sites.
#[derive(Debug)]
pub struct Render3d {
    references: u32,
    destroyed: bool,
    initialized: bool,
    utilities: Option<Handle>,
    context: Option<Handle>,
    camera: Option<Handle>,
    control: Option<Handle>,
    camera_2d: Option<Handle>,
    lighting_manager: Option<Handle>,
    picture: Option<Handle>,
    ui_matrix: [f32; 16],
    dirty: DirtyQueue,
    grid: Option<QuadGrid>,
    shakes: Shakes,
    geometry: ContainerGeometry,
    objects: ViewObjects,
    state: State,
}

impl Render3d {
    pub fn new(host: &mut impl References) -> Self {
        Self {
            references: 0,
            destroyed: false,
            initialized: false,
            utilities: host.create_helper(Helper::ViewUtilities),
            context: host.create_helper(Helper::DrawContext),
            camera: None,
            control: None,
            camera_2d: None,
            lighting_manager: None,
            picture: None,
            ui_matrix: [0.0; 16],
            dirty: DirtyQueue::default(),
            grid: None,
            shakes: Shakes::default(),
            geometry: ContainerGeometry::default(),
            objects: ViewObjects::default(),
            state: State::default(),
        }
    }

    /// On failure the supplied output is deliberately left untouched (§1.1).
    pub fn query_interface(&mut self, iid: u32, out: &mut Option<Interface>) -> bool {
        let face = match iid {
            1 | 0x681B_D529 | CLASS_ID => Interface::Primary,
            0x4522_94AA => Interface::MessageTarget,
            0xE4FD_A3D4 => Interface::Serializable,
            _ => return false,
        };
        self.add_ref();
        *out = Some(face);
        true
    }

    pub fn add_ref(&mut self) -> u32 {
        self.references = self.references.wrapping_add(1);
        self.references
    }

    pub fn reference_count(&self) -> u32 {
        self.references
    }

    pub fn release(&mut self, host: &mut impl References) -> u32 {
        if self.destroyed {
            // SPEC-GAP: safe Rust retains the tombstone after native destruction.
            return 0;
        }
        if self.references > 1 {
            self.references -= 1;
            return self.references;
        }
        self.shutdown(host);
        if let Some(utilities) = self.utilities.take() {
            host.destroy_helper(utilities);
        }
        if let Some(context) = self.context.take() {
            host.destroy_helper(context);
        }
        self.clear_view_object_lists(host);
        self.clear_dirty_rectangle_queue(host);
        self.shakes.clear();
        self.references = 0;
        self.destroyed = true;
        0
    }

    /// The uninitialized guard of §9.3; initialized teardown is added with Init.
    pub fn shutdown(&mut self, _host: &mut impl References) -> bool {
        if !self.initialized {
            return true;
        }
        self.initialized = false;
        true
    }

    pub fn class_id(&self) -> u32 {
        CLASS_ID
    }

    pub fn destroyed(&self) -> bool {
        self.destroyed
    }

    pub fn state(&self) -> &State {
        &self.state
    }

    pub fn view_utilities(&self) -> Option<Handle> {
        self.utilities
    }

    pub fn draw_context(&self) -> Option<Handle> {
        self.context
    }

    /// The serialized representation is blocked as a unit by RND1 (§2).
    pub fn write(&self) -> Result<(), Unsupported> {
        Err(Unsupported::Rnd1StreamSlot8)
    }

    pub fn read(&mut self, _bytes: &[u8]) -> Result<(), Unsupported> {
        Err(Unsupported::Rnd1StreamSlot8)
    }

    /// RND4 leaves the overload's fade-out length unresolved (§10.1).
    pub fn start_screen_shake_overload(&mut self, _amplitude: f32, _duration: f32) -> Result<u32, Unsupported> {
        Err(Unsupported::Rnd4ShakeOverload)
    }
}

#[cfg(test)]
mod backing_tests;
#[cfg(test)]
mod container_tests;
#[cfg(test)]
mod dirty_tests;
#[cfg(test)]
mod grid_tests;
#[cfg(test)]
mod picking_tests;
#[cfg(test)]
mod shake_tests;
#[cfg(test)]
mod tests;
