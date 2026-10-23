// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use std::sync::Arc;

/// OVV2 record. None represents the native uninitialized repeat slot.
pub struct PathInfo {
    colour_bits: u32,
    priority: f32,
    flags: u32,
    points: Vec<Point>,
    texture: Option<Arc<dyn Texture>>,
    repeat: Option<f32>,
    sequence: u64,
}
impl PathInfo {
    /// Compatibility name: packed colour, never an ID lookup.
    pub fn id(&self) -> u32 { self.colour_bits }
    pub fn colour_bits(&self) -> u32 { self.colour_bits }
    pub fn priority(&self) -> f32 { self.priority }
    pub fn flags(&self) -> u32 { self.flags }
    pub fn points(&self) -> &[Point] { &self.points }
    pub fn texture(&self) -> Option<&Arc<dyn Texture>> { self.texture.as_ref() }
    pub fn repeat(&self) -> Option<f32> { self.repeat }
}
#[derive(Clone, Copy)]
enum Selected { Sentinel, Record(u64) }
#[allow(non_camel_case_types)]
pub struct cSC4PathView {
    paths: Vec<PathInfo>,
    thickness: f32,
    next_sequence: u64,
    selected: Selected,
    states: Option<[DrawState; 2]>,
}
impl Default for cSC4PathView {
    fn default() -> Self { Self::new(2.0).expect("constructor only stores thickness") }
}
impl cSC4PathView {
    pub fn new(thickness: f32) -> Result<Self, Error> {
        Ok(Self { paths: Vec::new(), thickness, next_sequence: 0, selected: Selected::Sentinel, states: None })
    }
    pub fn init(&mut self, prefer_opaque: bool) {
        let plain = DrawState { colour_write: Some(true), depth_write: false, depth: DepthTest::Disabled,
            culling: Some(true), blend: if prefer_opaque { Blend::Disabled } else { Blend::SourceAlpha },
            textures: 0, alpha_greater_than: None };
        self.states = Some([plain, DrawState { textures: 1, ..plain }]);
    }
    pub fn states(&self) -> Option<[DrawState; 2]> { self.states }
    pub fn paths(&self) -> &[PathInfo] { &self.paths }
    pub fn thickness(&self) -> f32 { self.thickness }
    pub fn set_thickness(&mut self, thickness: f32) -> Result<(), Error> { self.thickness = thickness; Ok(()) }
    fn selected_mut(&mut self) -> Result<&mut PathInfo, Error> {
        let Selected::Record(sequence) = self.selected else { return Err(Error::NoCurrentPath) };
        // Stable token preserves stale selection without a dangling Rust dereference.
        self.paths.iter_mut().find(|p| p.sequence == sequence).ok_or(Error::UnsupportedDomain)
    }
    pub fn set_path_texture(&mut self, key: u32, repeat: f32, factory: &dyn TextureFactory) -> Result<(), Error> {
        let path = self.selected_mut()?;
        path.texture = None; // release BEFORE factory, including failed replacements
        path.texture = factory.bind(key);
        path.repeat = Some(repeat);
        Ok(())
    }
    pub fn new_path(&mut self, colour_bits: u32, priority: f32, flags: u32) -> Result<&mut PathInfo, Error> {
        let sequence = self.next_sequence;
        self.next_sequence = sequence.checked_add(1).ok_or(Error::Overflow)?;
        // C0-only: unordered continues; equal inserts before the older record.
        let at = self.paths.iter().position(|p| p.priority >= priority).unwrap_or(self.paths.len());
        self.paths.insert(at, PathInfo { colour_bits, priority, flags, points: Vec::new(), texture: None, repeat: None, sequence });
        self.selected = Selected::Record(sequence);
        Ok(&mut self.paths[at])
    }
    pub fn add_path_vertex(&mut self, point: Point) -> Result<bool, Error> {
        if matches!(self.selected, Selected::Sentinel) { return Ok(false) }
        let path = self.selected_mut()?;
        if let Some(prev) = path.points.last() {
            let dx = f64::from(prev.x) - f64::from(point.x);
            let dz = f64::from(prev.z) - f64::from(point.z);
            if dx * dx + dz * dz < 1.0e-5_f64 { return Ok(false) }
        }
        path.points.push(point); Ok(true)
    }
    pub fn clear_paths(&mut self) { self.paths.clear(); } // selected deliberately retained
    pub fn endpoint_normal(a: Point, b: Point) -> Result<[f32; 2], Error> {
        let dx = f64::from(b.x) - f64::from(a.x);
        let dz = f64::from(b.z) - f64::from(a.z);
        let length = (dx * dx + dz * dz).sqrt();
        if length == 0.0 || !length.is_finite() { return Err(Error::UnsupportedDomain) }
        Ok([(dz / length) as f32, (-dx / length) as f32])
    }
    pub fn joint_normal(a: Point, b: Point, c: Point) -> Result<[f32; 2], Error> { Self::joint_offset(a, b, c, 1.0) }
    pub fn joint_offset(a: Point, b: Point, c: Point, t: f32) -> Result<[f32; 2], Error> {
        let u = Self::endpoint_normal(a, b)?; let v = Self::endpoint_normal(b, c)?;
        let m = [(f64::from(u[0])+f64::from(v[0]))/2.0, (f64::from(u[1])+f64::from(v[1]))/2.0];
        let q = m[0]*m[0]+m[1]*m[1];
        Ok(if q < 0.0001_f64 { v } else { [(m[0]*f64::from(t)/q) as f32, (m[1]*f64::from(t)/q) as f32] })
    }
    pub fn draw_arrowhead(&self, tip: &mut Point, inward: Point, colour_bits: u32) -> Result<Option<Draw>, Error> {
        let t = self.thickness; let length = tip.distance(inward);
        let a = (length-t).min((4.5_f64*f64::from(t)) as f32);
        if a < t { return Ok(None) }
        let normal = Self::endpoint_normal(*tip, inward)?;
        let base = Point::new(tip.x+(inward.x-tip.x)*a/length, tip.y+(inward.y-tip.y)*a/length, tip.z+(inward.z-tip.z)*a/length);
        let positions = [*tip, Point::new(base.x-2.0*t*normal[0], base.y, base.z-2.0*t*normal[1]),
            Point::new(base.x+2.0*t*normal[0], base.y, base.z+2.0*t*normal[1])];
        *tip = base;
        Ok(Some(Draw { format: Some(10), primitive: Primitive::Triangles,
            vertices: positions.map(|position| Vertex { position, colour: Colour(colour_bits.to_le_bytes()), uv: [0.0; 2] }).to_vec(), indices: Vec::new() }))
    }
    /// Binary32 stores, wider normal/miter intermediates; no universal x87 bit claim.
    pub fn path_draws(&self, path: &PathInfo) -> Result<Vec<Draw>, Error> {
        let n = path.points.len(); if n < 2 { return Ok(Vec::new()) }
        if !self.thickness.is_finite() || self.thickness <= 0.0 || path.points.iter().any(|p| !p.finite()) { return Err(Error::UnsupportedDomain) }
        let repeat = path.repeat.ok_or(Error::Gap(Gap::PathAttributesOvv2))?;
        if !repeat.is_finite() { return Err(Error::UnsupportedDomain) }
        let mut points = path.points.clone(); let mut draws = Vec::new();
        if path.flags & 1 != 0 {
            let inward = points[1];
            if let Some(draw) = self.draw_arrowhead(&mut points[0], inward, path.colour_bits)? { draws.push(draw) }
        }
        let mut vertices = Vec::with_capacity(n*2); let k = repeat/(2.0*self.thickness);
        let mut s = 0.0_f32; let mut d = [0.0_f32; 3]; let mut b = 0.0;
        for i in 0..n {
            if i+1 < n {
                let a = points[i]; let next = points[i+1]; let length = a.distance(next);
                d = [(next.x-a.x)*k/length, (next.y-a.y)*k/length, (next.z-a.z)*k/length];
                b = s-(d[0]*a.x+d[1]*a.y+d[2]*a.z); s += length*k;
            } else if path.flags & 2 != 0 {
                let inward = points[n-2];
                if let Some(draw) = self.draw_arrowhead(&mut points[i], inward, path.colour_bits)? { draws.push(draw) }
            }
            let p = points[i];
            let offset = if i == 0 || i+1 == n {
                let normal = if i == 0 { Self::endpoint_normal(p, points[1])? } else { Self::endpoint_normal(points[i-1], p)? };
                normal.map(|v| v*self.thickness)
            } else { Self::joint_offset(points[i-1], p, points[i+1], self.thickness)? };
            for (sign, side) in [(1.0, 0.0), (-1.0, 1.0)] {
                let position = Point::new(p.x+sign*offset[0], p.y, p.z+sign*offset[1]);
                vertices.push(Vertex { position, colour: Colour(path.colour_bits.to_le_bytes()), uv: [d[0]*position.x+d[1]*position.y+d[2]*position.z+b, side] });
            }
        }
        draws.push(Draw { format: Some(10), primitive: Primitive::TriangleStrip, vertices, indices: Vec::new() }); Ok(draws)
    }
    pub fn draw_path(&self, path: &PathInfo) -> Result<Option<Draw>, Error> { Ok(self.path_draws(path)?.pop()) }
    pub fn update(&self, context: &mut dyn DrawContext) -> Result<(), Error> {
        if self.paths.is_empty() { return Ok(()) }
        let states = self.states.ok_or(Error::NotInitialized)?;
        let draws = self.paths.iter().map(|p| self.path_draws(p)).collect::<Result<Vec<_>, _>>()?;
        let old = context.depth_offset();
        context.set_depth_offset(15.0); context.set_identity_model_transform(); context.set_state(states[0]);
        let result = (|| {
            for (path, draws) in self.paths.iter().zip(draws) {
                if draws.is_empty() { continue }
                if let Some(texture) = &path.texture { context.set_state(states[1]); context.set_texture(Some(texture)); context.set_repeat_wrapping(); }
                let result = draws.iter().try_for_each(|draw| context.draw(draw));
                if path.texture.is_some() { context.set_state(states[0]); } result?;
            }
            Ok(())
        })(); context.set_depth_offset(old); result
    }
}
