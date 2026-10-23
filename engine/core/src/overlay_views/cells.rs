// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use std::sync::Arc;

/// OVV5 lower-vertex bias/store conversion in its bounded nonnegative domain.
pub(super) fn lower_vertex(coordinate: f32) -> Result<u32, Error> {
    if !coordinate.is_finite() || !(0.0..4_194_304.0).contains(&coordinate) {
        return Err(Error::UnsupportedDomain);
    }
    let biased = ((f64::from(coordinate) - 0.5) + 12_582_912.0) as f32;
    let candidate = biased.to_bits().wrapping_sub(0x4B400000);
    Ok(if f64::from(coordinate) == f64::from(candidate) + 1.0 { candidate + 1 } else { candidate })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub x: i32,
    pub z: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Split {
    Unflipped,
    Flipped,
}

pub trait Lot: Send + Sync {
    fn flags(&self) -> u32;
    fn shows_marks(&self) -> bool;
    fn altitude_at(&self, x: f32, z: f32) -> f32;
}

pub trait LotFilter: Send + Sync {
    fn accepts(&self, lot: &dyn Lot) -> bool;
}

/// Public filter construction input; acceptance remains a raw foreign query.
pub const LOT_FILTER_MASK: u32 = 0x1F4F;

/// Raw foreign terrain/camera/occupant queries, never precomputed marked-cell quads.
pub trait CellTerrain {
    fn dimensions(&self) -> [u32; 2];
    fn vertex(&self, x: u32, z: u32) -> Point;
    fn split(&self, x: u32, z: u32) -> Split;
    fn water_altitude(&self) -> f32;
    fn in_frustum(&self, point: Point) -> bool;
    fn lot_at(&self, x: u32, z: u32) -> Option<&dyn Lot>;
    fn ground_altitude(&self, x: f32, z: f32) -> f32;
}

pub struct SC4MarkedCellView {
    cells: Vec<(Cell, Option<Colour>)>,
    colour: Colour,
    outline: Colour,
    cell_size: [f32; 2],
    inverse_size: [f32; 2],
    ignore_depth: bool,
    use_water: bool,
    filter: Option<Arc<dyn LotFilter>>,
    state: Option<DrawState>,
}

impl SC4MarkedCellView {
    /// Explicit state because construction defaults are not published (OVV6).
    pub fn new(
        cell_size: [f32; 2],
        colour: Colour,
        outline: Colour,
        ignore_depth: bool,
        use_water: bool,
    ) -> Result<Self, Error> {
        let mut view = Self {
            cells: Vec::new(),
            colour,
            outline,
            cell_size,
            inverse_size: [0.0; 2],
            ignore_depth,
            use_water,
            filter: None,
            state: None,
        };
        view.set_cell_size(cell_size)?;
        Ok(view)
    }

    pub fn init(&mut self, filter: Arc<dyn LotFilter>) {
        self.filter = Some(filter);
        self.state = Some(DrawState {
            colour_write: Some(true),
            depth_write: false,
            depth: if self.ignore_depth {
                DepthTest::Disabled
            } else {
                DepthTest::LessOrEqual
            },
            culling: Some(true),
            blend: Blend::SourceAlpha,
            textures: 0,
            alpha_greater_than: None,
        });
    }

    pub fn set_should_ignore_depth_buffer(&mut self, ignore: bool) {
        self.ignore_depth = ignore;
        if let Some(state) = &mut self.state {
            state.depth = if ignore {
                DepthTest::Disabled
            } else {
                DepthTest::LessOrEqual
            };
        }
    }

    pub fn set_should_use_water_altitude(&mut self, use_water: bool) {
        self.use_water = use_water;
    }
    pub fn set_cell_colour_bytes(&mut self, colour: Colour) {
        self.colour = colour;
    }
    pub fn set_outline_colour_bytes(&mut self, colour: Colour) {
        self.outline = colour;
    }
    pub fn colour(&self) -> Colour {
        self.colour
    }
    pub fn cell_size(&self) -> [f32; 2] {
        self.cell_size
    }
    pub fn inverse_cell_size(&self) -> [f32; 2] {
        self.inverse_size
    }
    pub fn cells(&self) -> &[(Cell, Option<Colour>)] {
        &self.cells
    }

    pub fn set_cell_color(&mut self, _r: f32, _g: f32, _b: f32, _a: f32) -> Result<(), Error> {
        // No published range, conversion, saturation or byte order. OVV4.
        Err(Error::Gap(Gap::PackedColourOvv4))
    }

    pub fn set_cell_size(&mut self, size: [f32; 2]) -> Result<(), Error> {
        self.cell_size = size;
        self.inverse_size = [size[0].recip(), size[1].recip()];
        Ok(())
    }

    /// A core input surface, not a claim about unpublished native add/remove slots.
    pub fn replace_cells(&mut self, cells: Vec<(Cell, Option<Colour>)>) {
        self.cells = cells;
    }

    pub fn draw_cells(&self, terrain: &dyn CellTerrain, override_colour: Option<Colour>) -> Result<Vec<Draw>, Error> {
        if self.cells.is_empty() { return Ok(Vec::new()); }
        // Setter accepts every float; checked portable Draw cannot promise native unchecked access.
        if self.cell_size.iter().any(|v| !v.is_finite() || *v < 0.0) { return Err(Error::UnsupportedDomain); }
        let dims = terrain.dimensions();
        let standard = self.cell_size == [16.0, 16.0];
        let mut draws = Vec::new();
        let mut batch = Draw {
            format: Some(1),
            primitive: Primitive::Triangles,
            vertices: Vec::new(),
            indices: Vec::new(),
        };
        for (cell, colour) in &self.cells {
            let (mut corners, split) = if standard {
                let cx = cell.x as u32;
                let cz = cell.z as u32;
                if cx >= dims[0] || cz >= dims[1] { continue; }
                let corners = [
                    terrain.vertex(cx, cz),
                    terrain.vertex(cx, cz + 1),
                    terrain.vertex(cx + 1, cz + 1),
                    terrain.vertex(cx + 1, cz),
                ];
                (corners, terrain.split(cx, cz))
            } else {
                let mut corners = [Point::new(0.0, 0.0, 0.0); 4];
                for (out, [dx, dz]) in corners.iter_mut().zip([[0,0], [0,1], [1,1], [1,0]]) {
                    let px = (i64::from(cell.x)+dx).clamp(0, i64::from(dims[0])) as f32 * self.cell_size[0];
                    let pz = (i64::from(cell.z)+dz).clamp(0, i64::from(dims[1])) as f32 * self.cell_size[1];
                    let vx = lower_vertex(px/16.0)?; let vz = lower_vertex(pz/16.0)?;
                    if vx > dims[0] || vz > dims[1] { return Err(Error::UnsupportedDomain); }
                    *out = Point::new(px, terrain.vertex(vx, vz).y, pz);
                }
                (corners, Split::Unflipped)
            };
            if self.use_water {
                let water = terrain.water_altitude();
                for p in &mut corners {
                    if water > p.y { p.y = water; }
                }
            }
            if !terrain.in_frustum(corners[0]) && !terrain.in_frustum(corners[2]) { continue; }
            if standard {
                if let (Some(filter), Some(lot)) = (&self.filter, terrain.lot_at(cell.x as u32, cell.z as u32)) {
                    if filter.accepts(lot) {
                        if lot.flags() & 0x100000 != 0 && !lot.shows_marks() {
                            continue;
                        }
                        let lift = if lot.flags() & 0x304 != 0 { 15.5 } else { 0.0 };
                        // Literal lot replacement after water; ordering is queried in OVV5.
                        for p in &mut corners {
                            p.y = lot.altitude_at(p.x, p.z) + lift;
                        }
                    }
                }
            }
            if batch.vertices.len() == 1024 {
                draws.push(batch);
                batch = Draw {
                    format: Some(1),
                    primitive: Primitive::Triangles,
                    vertices: Vec::new(),
                    indices: Vec::new(),
                };
            }
            let base = batch.vertices.len() as u32;
            let colour = override_colour.unwrap_or(colour.unwrap_or(self.colour));
            batch.vertices.extend(corners.map(|position| Vertex {
                position,
                colour,
                uv: [0.0; 2],
            }));
            // Corner order matches terrain-view §4.2, without taking on terrain-owner flag logic.
            let indices = match split {
                Split::Unflipped => [0, 1, 2, 0, 2, 3],
                Split::Flipped => [0, 1, 3, 3, 1, 2],
            };
            batch.indices.extend(indices.map(|i| base + i));
        }
        if !batch.vertices.is_empty() {
            draws.push(batch);
        }
        Ok(draws)
    }

    pub fn update(
        &self,
        context: &mut dyn DrawContext,
        terrain: &dyn CellTerrain,
        prefer_opaque: bool,
    ) -> Result<(), Error> {
        if self.cells.is_empty() {
            return Ok(());
        }
        let state = self.state.ok_or(Error::NotInitialized)?;
        let normal = self.draw_cells(terrain, None)?;
        let hidden = if prefer_opaque {
            Vec::new()
        } else {
            self.draw_cells(terrain, Some(self.outline))?
        };
        let old_offset = context.depth_offset();
        context.set_depth_offset(4.0);
        context.set_identity_model_transform();
        context.set_state(state);
        context.set_texture(None);
        let result = (|| {
            for draw in &normal {
                context.draw(draw)?;
            }
            if !prefer_opaque {
                context.set_state(DrawState {
                    depth: DepthTest::Greater,
                    blend: Blend::Additive,
                    ..state
                });
                for draw in &hidden {
                    context.draw(draw)?;
                }
            }
            Ok(())
        })();
        context.set_depth_offset(old_offset);
        result
    }
}
