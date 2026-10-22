// SPDX-License-Identifier: GPL-3.0-or-later
//! Bounded, portable cSC43DRender owner (render3d §§1–12, view3d §§3–4).
//!
//! No native layout/binding, complete Draw, or whole-owner equivalence is claimed.
//! Foreign boundaries are individual reference/model/grid/driver primitives.
//! Errors can follow published prefix effects: callers must not retry as though
//! an unsupported call were a clean false. Allocation, overflow and reentry are
//! checked portable domains, not invented native failure cleanup (RND5).
//! State getters use shared borrows, never RefCell. An embedding may put this
//! owner behind a lock; shared backing buffers have their own explicit mutex.

pub mod backing;
pub mod models;
pub mod record;
pub mod shake;

use crate::view_utilities::{CameraRayProvider, PixelRect, ViewUtilities};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unsupported {
    Algorithm(&'static str),
    Domain(&'static str),
    Allocation,
    Overflow,
    Reentry,
    SharedStatePoisoned,
}
pub type Result<T> = std::result::Result<T, Unsupported>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Object(pub u64);

/// Individual foreign reference operations; never renderer algorithm outcomes.
pub trait References {
    fn add_ref(&mut self, object: Object);
    fn release(&mut self, object: Object);
}
pub trait CameraPrimitives: References {
    fn driver_depth_query(&mut self);
    fn set_depth_size(&mut self, camera: Object, depth: f32);
    fn camera_field_17c(&mut self, camera: Object) -> i32;
}

pub type Rect = [i32; 4];

/// Signed native filter, including the reversed-rectangle acceptance quirk.
pub fn filter_rect(rect: &mut Rect, width: i32, height: i32) -> bool {
    if rect[2] <= 0 || rect[0] >= width || rect[3] <= 0 || rect[1] >= height {
        return false;
    }
    rect[0] = rect[0].max(0);
    rect[1] = rect[1].max(0);
    rect[2] = rect[2].min(width);
    rect[3] = rect[3].min(height);
    true
}

fn area(r: Rect) -> Result<i32> {
    r[3].checked_sub(r[1])
        .and_then(|h| r[2].checked_sub(r[0]).and_then(|w| h.checked_mul(w)))
        .ok_or(Unsupported::Overflow)
}
fn union(a: Rect, b: Rect) -> Rect {
    [
        a[0].min(b[0]),
        a[1].min(b[1]),
        a[2].max(b[2]),
        a[3].max(b[3]),
    ]
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirtyNode {
    pub value_15c: i32,
    pub mode: u32,
    pub rect: Rect,
    pub model: Option<Object>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ViewNode {
    pub object: Object,
    pub key: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InterfaceFace {
    Renderer,
    MessageTarget,
    Serializable,
}

#[derive(Clone, Debug)]
pub struct Defaults {
    /// Raw numeric offsets whose semantics are not published.
    pub bytes: [(u32, u8); 5],
    pub floats: [(u32, f32); 12],
    pub words: [(u32, i32); 7],
}
impl Default for Defaults {
    fn default() -> Self {
        Self {
            bytes: [(0x60, 1), (0x63, 1), (0x64, 1), (0x1fc, 1), (0x61, 0)],
            floats: [
                (0xec, 1.0),
                (0xf0, 1.0),
                (0xf4, 1.0),
                (0xf8, 1.0),
                (0xfc, 1.0),
                (0x100, 10000.0),
                (0x104, 15000.0),
                (0x10c, 4096.0),
                (0x110, 4096.0),
                (0x1b8, 1.0),
                (0x80, 0.0),
                (0x84, 0.0),
            ],
            words: [
                (0x1a4, 0x5000),
                (0x1e4, i32::MAX),
                (0x1f4, i32::MAX),
                (0x210, 0x40),
                (0x230, 10000),
                (0x264, 1),
                (0x238, 0),
            ],
        }
    }
}

/// State actually owned by the portable renderer. Helper allocation failure
/// cannot be represented by a valid Rust value; construction is a portable
/// successful-allocation domain, not an emulation of native null dereferences.
pub struct SceneRenderer {
    pub view_utilities: ViewUtilities,
    pub defaults: Defaults,
    pub models: models::ModelQueue,
    pub shakes: shake::Shakes,
    viewport: (i32, i32),
    viewport_rect: Rect,
    refs: u32,
    destroyed: bool,
    initialized: bool,
    camera: Option<Object>,
    camera_control: Option<Object>,
    terrain: Option<Object>,
    lists: [Vec<ViewNode>; 4], // +188,+18c,+190,+194
    dirty: Vec<DirtyNode>,
    dirty_area: i32,
    area_limit: i32,
    deadline: i32,
    draw_counter: i32,
    value_15c: i32,
    full_pending: bool,
    backing_saved: bool,
    flags_160_174: [bool; 2],
    reason: i32,
    subpixel_bias: (f32, f32),
    camera_derived: u32,
    camera_view: u32,
    camera_mode: i32,
    lighting: bool,
    fog: bool,
    background_enabled: bool,
    transparent: bool,
    borrowed: bool,
    clear_color: [f32; 4],
    viewport_pushed: bool,
    map_scale: i32,
}
impl Default for SceneRenderer {
    fn default() -> Self {
        Self {
            view_utilities: ViewUtilities::new(),
            defaults: Defaults::default(),
            models: models::ModelQueue::default(),
            shakes: shake::Shakes::default(),
            viewport: (0, 0),
            viewport_rect: [0; 4],
            refs: 0,
            destroyed: false,
            initialized: false,
            camera: None,
            camera_control: None,
            terrain: None,
            lists: std::array::from_fn(|_| Vec::new()),
            dirty: Vec::new(),
            dirty_area: 0,
            area_limit: i32::MAX,
            deadline: 0,
            draw_counter: 0,
            value_15c: 0,
            full_pending: false,
            backing_saved: false,
            flags_160_174: [false; 2],
            reason: 0,
            subpixel_bias: (0.0, 0.0),
            camera_derived: u32::MAX,
            camera_view: u32::MAX,
            camera_mode: 0,
            lighting: true,
            fog: false,
            background_enabled: true,
            transparent: false,
            borrowed: false,
            clear_color: [0.0; 4],
            viewport_pushed: false,
            map_scale: 1,
        }
    }
}
impl SceneRenderer {
    pub const CLSID: u32 = 0xe9c622d8;
    pub fn ref_count(&self) -> u32 {
        self.refs
    }
    pub fn add_ref(&mut self) -> Result<u32> {
        if self.destroyed {
            return Err(Unsupported::Domain("destroyed renderer"));
        }
        self.refs = self.refs.checked_add(1).ok_or(Unsupported::Overflow)?;
        Ok(self.refs)
    }
    pub fn query_interface(&mut self, iid: u32, out: &mut Option<InterfaceFace>) -> Result<bool> {
        let face = match iid {
            1 | 0x681bd529 | Self::CLSID => InterfaceFace::Renderer,
            0x452294aa => InterfaceFace::MessageTarget,
            0xe4fda3d4 => InterfaceFace::Serializable,
            _ => return Ok(false), // out untouched
        };
        self.add_ref()?;
        *out = Some(face);
        Ok(true)
    }
    /// Last release executes known Shutdown first and stops at the unknown
    /// destructor. It does NOT report a successful deletion or decrement 0/1.
    pub fn release(&mut self) -> Result<u32> {
        if self.destroyed {
            return Err(Unsupported::Domain("destroyed renderer"));
        }
        if self.refs > 1 {
            self.refs -= 1;
            return Ok(self.refs);
        }
        self.shutdown()?;
        Err(Unsupported::Algorithm("destructor 0x007ca020"))
    }
    pub fn init(&mut self) -> Result<bool> {
        if self.initialized {
            return Ok(true);
        }
        Err(Unsupported::Algorithm("Init/CreateQuadGrid/StaticShutdown"))
    }
    pub fn shutdown(&mut self) -> Result<bool> {
        if !self.initialized {
            return Ok(true);
        }
        Err(Unsupported::Algorithm(
            "Shutdown/model drain and native services",
        ))
    }
    pub fn draw(&mut self) -> Result<bool> {
        Err(Unsupported::Algorithm(
            "Draw/translated updates/static drawing",
        ))
    }
    pub fn viewport_size(&self) -> (i32, i32) {
        self.viewport
    }
    pub fn viewport_rect(&self) -> Rect {
        self.viewport_rect
    }
    pub fn set_viewport_size(&mut self, w: i32, h: i32) {
        self.viewport = (w, h);
        self.viewport_rect[2] = w;
        self.viewport_rect[3] = h;
    }
    pub fn set_map_scale_factor(&mut self, factor: i32) {
        self.map_scale = factor;
    }
    pub fn map_scale_factor(&self) -> i32 {
        self.map_scale
    }
    pub fn set_subpixel_image_bias(&mut self, x: f32, y: f32) {
        self.subpixel_bias = (x, y);
    }
    pub fn subpixel_image_bias(&self) -> (f32, f32) {
        self.subpixel_bias
    }
    pub fn camera(&self) -> Option<Object> {
        self.camera
    }
    pub fn camera_control(&self) -> Option<Object> {
        self.camera_control
    }
    pub fn saved_record(&self) -> record::Record {
        record::Record {
            width_bits: self.viewport.0 as u32,
            height_bits: self.viewport.1 as u32,
            rect_bits: self.viewport_rect.map(|v| v as u32),
            camera_control_token: self.camera_control.map(|o| o.0),
            camera_derived: self.camera_derived,
            camera_view: self.camera_view,
            camera_mode_bits: self.camera_mode as u32,
        }
    }
    pub fn set_camera(
        &mut self,
        camera: Option<Object>,
        io: &mut dyn CameraPrimitives,
    ) -> Result<()> {
        if self.camera != camera {
            if let Some(camera) = camera {
                io.add_ref(camera);
            }
            if let Some(old) = self.camera {
                io.release(old);
            }
            self.camera = camera;
        }
        if let Some(camera) = camera {
            io.driver_depth_query();
            io.set_depth_size(camera, 65536.0);
            self.camera_derived = io
                .camera_field_17c(camera)
                .checked_sub(1)
                .ok_or(Unsupported::Overflow)? as u32;
        }
        Ok(())
    }
    /// Raw camera/control fields feed the existing view-utilities algorithm.
    /// No external renderer/camera-math result can be supplied.
    pub fn set_camera_control(
        &mut self,
        control: Option<Object>,
        raw: Option<&crate::view_utilities::CameraState>,
        refs: &mut dyn References,
    ) -> Result<()> {
        if self.camera_control != control {
            if let Some(control) = control {
                refs.add_ref(control);
            }
            if let Some(old) = self.camera_control {
                refs.release(old);
            }
            self.camera_control = control;
        }
        if control.is_some() && !self.view_utilities.update_camera_position(raw) {
            return Err(Unsupported::Domain("existing view-utilities camera domain"));
        }
        Ok(())
    }
    pub fn set_zoom(control: &mut crate::camera::CameraControl, zoom: i32) -> bool {
        control.set_zoom(zoom as f32)
    }
    pub fn set_rotation(control: &mut crate::camera::CameraControl, rotation: i32) -> bool {
        // Existing API accepts unsigned bits; preserve the original int bits.
        control.set_rotation(rotation as u32)
    }
    pub fn set_zoom_and_rotation(
        control: &mut crate::camera::CameraControl,
        zoom: i32,
        rotation: i32,
    ) -> bool {
        control.set_zoom_and_rotation(zoom as u32, rotation as u32)
    }
    pub fn backing_saved(&self) -> bool {
        self.backing_saved
    }
    /// Raw +65 write for an embedding after an actual successful save.
    pub fn set_backing_saved(&mut self, saved: bool) {
        self.backing_saved = saved;
    }
    pub fn lighting_enabled(&self) -> bool {
        self.lighting
    }
    pub fn view_fog_enabled(&self) -> bool {
        self.fog
    }
    pub fn enable_background_picture(&mut self, b: bool) {
        self.background_enabled = b;
    }
    pub fn get_background_picture(&self, enabled: Option<&mut bool>) -> Option<Object> {
        if let Some(enabled) = enabled {
            *enabled = true;
        } // RND2: +60 stays 1
        None // no SetBackgroundPicture implementation
    }
    pub fn enable_transparent_background(&mut self, b: bool) {
        self.transparent = b;
    }
    pub fn set_viewport_clear_color(&mut self, color: [f32; 4]) {
        self.clear_color = color;
        self.viewport_pushed = false;
    }
    pub fn borrow_render_system(&mut self) -> bool {
        self.borrowed = true;
        true
    }
    pub fn return_render_system(&mut self) -> Result<bool> {
        self.borrowed = false;
        Err(Unsupported::Algorithm(
            "ReturnRenderSystem/camera and driver states",
        ))
    }
    pub fn set_view_fog_enabled(
        &mut self,
        b: bool,
        refs: &mut dyn References,
        camera: Option<&dyn CameraRayProvider>,
    ) -> Result<bool> {
        if self.fog != b {
            self.fog = b;
            self.force_full_redraw(refs, camera)?;
        }
        Ok(true)
    }
    pub fn set_lighting_enabled(
        &mut self,
        b: bool,
        refs: &mut dyn References,
        camera: Option<&dyn CameraRayProvider>,
    ) -> Result<bool> {
        if self.lighting != b {
            self.force_full_redraw(refs, camera)?;
            self.lighting = b;
        }
        Ok(true)
    }
    pub fn full_redraw_pending(&self) -> bool {
        self.full_pending
    }
    pub fn redraw_reason(&self) -> i32 {
        self.reason
    }
    pub fn force_full_redraw(
        &mut self,
        refs: &mut dyn References,
        camera: Option<&dyn CameraRayProvider>,
    ) -> Result<()> {
        self.do_full_view_update(10, refs, camera)
    }
    pub fn do_full_view_update(
        &mut self,
        reason: i32,
        refs: &mut dyn References,
        camera: Option<&dyn CameraRayProvider>,
    ) -> Result<()> {
        self.reason = reason;
        if self.full_pending {
            return Ok(());
        }
        self.full_pending = true;
        self.backing_saved = false;
        let r = self.viewport_rect;
        if !self
            .view_utilities
            .build_draw_frustum_data_from_viewport_rect(
                PixelRect::new(r[0], r[1], r[2], r[3]),
                camera,
            )
        {
            return Err(Unsupported::Domain(
                "existing view-utilities frustum domain",
            ));
        }
        self.flags_160_174 = [false; 2];
        self.clear_dirty_rectangle_queue(refs);
        Ok(())
    }
    pub fn dirty_nodes(&self) -> &[DirtyNode] {
        &self.dirty
    }
    pub fn dirty_area(&self) -> i32 {
        self.dirty_area
    }
    pub fn dirty_deadline(&self) -> i32 {
        self.deadline
    }
    /// Raw published counters/limit, for portable integration and saved state.
    pub fn set_dirty_limits(&mut self, limit: i32, counter: i32, value_15c: i32) -> Result<()> {
        if limit <= 0 {
            return Err(Unsupported::Domain("nonpositive dirty limit"));
        }
        self.area_limit = limit;
        self.draw_counter = counter;
        self.value_15c = value_15c;
        Ok(())
    }
    pub fn add_dirty_rectangle(
        &mut self,
        mode: u32,
        mut rect: Rect,
        model: Option<Object>,
        delay: i32,
        refs: &mut dyn References,
        camera: Option<&dyn CameraRayProvider>,
    ) -> Result<()> {
        if self.full_pending || !filter_rect(&mut rect, self.viewport.0, self.viewport.1) {
            return Ok(());
        }
        self.add_dirty_rectangle_internal(mode, rect, model, delay, true, refs, camera)
    }
    pub fn add_dirty_box(
        &mut self,
        mode: u32,
        bounds: crate::view_utilities::Aabb,
        model: Option<Object>,
        delay: i32,
        refs: &mut dyn References,
        camera: Option<&dyn CameraRayProvider>,
    ) -> Result<()> {
        if self.full_pending {
            return Ok(());
        }
        let mut rect = PixelRect::new(0, 0, 0, 0);
        if !self
            .view_utilities
            .project_bounding_box_to_viewport_rectangle(bounds, &mut rect)
        {
            return Err(Unsupported::Domain(
                "existing view-utilities box projection domain",
            ));
        }
        self.add_dirty_rectangle(
            mode,
            [
                rect.top_left.x,
                rect.top_left.y,
                rect.bottom_right.x,
                rect.bottom_right.y,
            ],
            model,
            delay,
            refs,
            camera,
        )
    }
    #[allow(clippy::too_many_arguments)]
    pub fn add_dirty_rectangle_internal(
        &mut self,
        mode: u32,
        rect: Rect,
        model: Option<Object>,
        delay: i32,
        merging: bool,
        refs: &mut dyn References,
        camera: Option<&dyn CameraRayProvider>,
    ) -> Result<()> {
        let model = if mode == 0 { model } else { None };
        let new_area = area(rect)?;
        if self.dirty.is_empty() {
            self.deadline = self
                .draw_counter
                .checked_sub(delay)
                .ok_or(Unsupported::Overflow)?;
        } else if merging {
            self.deadline = self
                .deadline
                .checked_sub(delay)
                .ok_or(Unsupported::Overflow)?;
            let mut best = self.area_limit.checked_neg().ok_or(Unsupported::Overflow)?;
            let mut chosen = None;
            let mut growth = 0;
            for (i, node) in self.dirty.iter().enumerate() {
                let node_area = area(node.rect)?;
                let union_area = area(union(node.rect, rect))?;
                let mut score = node_area
                    .checked_add(new_area)
                    .and_then(|v| v.checked_sub(union_area))
                    .ok_or(Unsupported::Overflow)?;
                if mode == 0 || node.mode == 0 {
                    score = score
                        .checked_sub(new_area.min(node_area))
                        .ok_or(Unsupported::Overflow)?;
                }
                if score > best {
                    best = score;
                    chosen = Some(i);
                    growth = union_area
                        .checked_sub(node_area)
                        .ok_or(Unsupported::Overflow)?;
                }
            }
            if self.dirty.len() >= 16 || f64::from(best) >= -2500.0 {
                let i = chosen.ok_or(Unsupported::Domain("merge has no best node"))?;
                self.dirty_area = self
                    .dirty_area
                    .checked_add(growth)
                    .ok_or(Unsupported::Overflow)?;
                if self.dirty_area >= self.area_limit {
                    return self.do_full_view_update(4, refs, camera);
                }
                let combined = union(self.dirty[i].rect, rect);
                if let Some(model) = self.dirty[i].model.take() {
                    refs.release(model);
                }
                if growth > 0 && self.dirty.len() > 1 {
                    self.dirty.remove(i); // deliberately do not subtract its area
                    return self
                        .add_dirty_rectangle_internal(6, combined, None, 0, merging, refs, camera);
                }
                self.dirty[i].mode = 6;
                self.dirty[i].rect = combined;
                return Ok(());
            }
        }
        self.dirty_area = self
            .dirty_area
            .checked_add(new_area)
            .ok_or(Unsupported::Overflow)?;
        if self.dirty_area >= self.area_limit {
            return self.do_full_view_update(4, refs, camera);
        }
        self.dirty
            .try_reserve(1)
            .map_err(|_| Unsupported::Allocation)?;
        if let Some(model) = model {
            refs.add_ref(model);
        }
        self.dirty.push(DirtyNode {
            value_15c: self.value_15c,
            mode,
            rect,
            model,
        });
        Ok(())
    }
    pub fn clear_dirty_rectangle_queue(&mut self, refs: &mut dyn References) {
        if self.dirty.is_empty() {
            return;
        }
        for node in self.dirty.drain(..) {
            if let Some(model) = node.model {
                refs.release(model);
            }
        }
        self.dirty_area = 0;
    }
    pub fn process_dirty_rectangle_queue(&mut self) -> Result<()> {
        Err(Unsupported::Algorithm(
            "ProcessDirtyRectangleQueue/own draw helpers",
        ))
    }
    pub fn view_list(&self, type_: u32) -> Option<&[ViewNode]> {
        list_index(type_).map(|i| self.lists[i].as_slice())
    }
    pub fn add_view_object(
        &mut self,
        object: Object,
        type_: u32,
        key: u32,
        refs: &mut dyn References,
    ) -> Result<bool> {
        let Some(i) = list_index(type_) else {
            return Ok(false);
        };
        let list = &mut self.lists[i];
        list.try_reserve(1).map_err(|_| Unsupported::Allocation)?;
        refs.add_ref(object);
        let at = list
            .iter()
            .position(|node| node.key > key)
            .unwrap_or(list.len());
        list.insert(at, ViewNode { object, key });
        if i >= 2 {
            self.backing_saved = false;
        }
        Ok(true)
    }
    pub fn find_view_object(&self, object: Option<Object>) -> bool {
        if self.terrain == object {
            return true;
        }
        [1, 0, 3, 2]
            .into_iter()
            .any(|i| self.lists[i].iter().any(|n| Some(n.object) == object))
    }
    pub fn remove_view_object(
        &mut self,
        object: Option<Object>,
        refs: &mut dyn References,
    ) -> bool {
        let mut found = false;
        if self.terrain == object {
            if let Some(object) = self.terrain.take() {
                refs.release(object);
            }
            found = true;
        }
        for i in [1, 0, 3, 2] {
            if let Some(at) = self.lists[i].iter().position(|n| Some(n.object) == object) {
                let node = self.lists[i].remove(at);
                refs.release(node.object);
                found = true;
                if i >= 2 {
                    self.backing_saved = false;
                }
            }
        }
        found
    }
    pub fn clear_view_lists(&mut self, refs: &mut dyn References) {
        for list in &mut self.lists {
            for node in list.drain(..) {
                refs.release(node.object);
            }
        }
    }
}
fn list_index(type_: u32) -> Option<usize> {
    match type_ {
        3 => Some(0),
        5 => Some(1),
        0 => Some(2),
        2 => Some(3),
        _ => None,
    }
}

#[cfg(test)]
mod fixtures;
#[cfg(test)]
mod tests;
