// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;

/// Raw acquired driver interface. Drop releases that reference, never unlocks/draws.
pub trait VertexBuffer: Send + Sync {
    fn format(&self) -> u32;
    fn capacity(&self) -> usize;
    fn lock(&mut self) -> bool;
    fn write_vertices(&mut self, offset: usize, vertices: &[[u8; 32]]);
    fn unlock(&mut self);
    fn draw_triangles(&mut self, indices: &[u16]) -> bool;
}
pub trait VertexBufferDriver {
    fn create(&self, interface: u32, format: u32) -> Option<Box<dyn VertexBuffer>>;
}
pub trait TerrainRows {
    fn vertex_bytes(&self, row: usize, column: usize) -> [u8; 32];
}
/// One caller-managed process helper. No implicit per-view cache or singleton reset.
#[allow(non_camel_case_types)]
pub struct cSTEVertBufExtn<'a> {
    buffer: Option<Box<dyn VertexBuffer>>,
    width: usize,
    height: usize,
    capacity: usize,
    used_vertices: usize,
    index_capacity: usize,
    indices: Vec<u16>,
    source: Option<(&'a [u16], u8)>,
    locked: bool,
}
impl<'a> cSTEVertBufExtn<'a> {
    pub fn get_vertex_buffer_extension(driver: &dyn VertexBufferDriver, enabled: bool, width: usize, height: usize) -> Result<Option<Self>, Error> {
        if !enabled { return Ok(None) }
        let stride = width.checked_add(1).ok_or(Error::Overflow)?;
        let minimum = stride.checked_mul(2).ok_or(Error::Overflow)?;
        let Some(buffer) = driver.create(0x09CD86F9, 11) else { return Ok(None) };
        let capacity = buffer.capacity();
        if buffer.format() != 11 || capacity < minimum { return Ok(None) }
        let index_capacity = (capacity/stride).min(height).checked_mul(width).and_then(|n| n.checked_mul(6)).ok_or(Error::Overflow)?;
        Ok(Some(Self { buffer: Some(buffer), width, height, capacity, used_vertices: 0, index_capacity,
            indices: Vec::with_capacity(index_capacity), source: None, locked: false }))
    }
    pub fn index_capacity(&self) -> usize { self.index_capacity }
    pub fn used_vertices(&self) -> usize { self.used_vertices }
    pub fn pending_indices(&self) -> &[u16] { &self.indices }
    /// Logical stored lock pointer, including the retained pointer after Release.
    pub fn is_locked(&self) -> bool { self.locked }
    /// Borrowed raw words; 12-byte cell addressing. Never copied/acquired.
    pub fn set_index_buffer(&mut self, source: &'a [u16], byte_flag: u8) { self.source = Some((source, byte_flag)); }
    pub fn create_buffer(&mut self) -> Result<(), Error> {
        let buffer = self.buffer.as_mut().ok_or(Error::UnsupportedDomain)?;
        self.locked = buffer.lock();
        if !self.locked { return Err(Error::LockFailed) }
        self.used_vertices = 0; self.indices.clear(); Ok(())
    }
    pub fn copy_terrain_rows(&self, terrain: &dyn TerrainRows, row: usize, column: usize, n: usize) -> Result<Vec<[u8; 32]>, Error> {
        if row >= self.height || column.checked_add(n).ok_or(Error::Overflow)? > self.width { return Err(Error::UnsupportedDomain) }
        let count = n.checked_add(1).and_then(|v| v.checked_mul(2)).ok_or(Error::Overflow)?;
        let mut bytes = Vec::with_capacity(count);
        for r in row..=row+1 { for c in column..=column+n { bytes.push(terrain.vertex_bytes(r,c)); } }
        Ok(bytes)
    }
    pub fn draw_prims_indexed_using_vert_buf(&mut self, terrain: &dyn TerrainRows, row: usize, column: usize, n: usize) -> Result<(), Error> {
        if self.buffer.is_none() { return Err(Error::UnsupportedDomain) }
        let needed = n.checked_add(1).and_then(|v| v.checked_mul(2)).ok_or(Error::Overflow)?;
        // Native has no oversize/range guard or retry. Portable storage domain is explicit.
        if needed > self.capacity || row >= self.height || column.checked_add(n).ok_or(Error::Overflow)? > self.width { return Err(Error::UnsupportedDomain) }
        if self.used_vertices.checked_add(needed).ok_or(Error::Overflow)? > self.capacity {
            self.flush_buffer()?;
            // Null relock is an ignored native submission, not a lazy initial lock.
            if self.create_buffer() == Err(Error::LockFailed) { return Ok(()) }
        }
        if !self.locked { return Ok(()) }
        let count = n.checked_mul(6).ok_or(Error::Overflow)?;
        let words = if count == 0 { &[][..] } else {
            let (source, flag) = self.source.ok_or(Error::MissingIndexSource)?;
            let cell = if flag != 0 { row.checked_mul(self.width).and_then(|v| v.checked_add(column)).ok_or(Error::Overflow)? } else { column };
            let start = cell.checked_mul(6).ok_or(Error::Overflow)?;
            source.get(start..start.checked_add(count).ok_or(Error::Overflow)?).ok_or(Error::UnsupportedDomain)?
        };
        let end = self.indices.len().checked_add(count).ok_or(Error::Overflow)?;
        if end > self.index_capacity {
            self.index_capacity = end.checked_add(16).ok_or(Error::Overflow)?;
            self.indices.reserve(self.index_capacity-self.indices.len());
        }
        let a = self.used_vertices; let b = a+n+1;
        for &q in words {
            let base = if usize::from(q) <= self.width { a.wrapping_sub(column) } else { b.wrapping_sub(self.width+1).wrapping_sub(column) };
            self.indices.push(q.wrapping_add(base as u16));
        }
        let bytes = self.copy_terrain_rows(terrain,row,column,n)?;
        self.buffer.as_mut().ok_or(Error::UnsupportedDomain)?.write_vertices(a,&bytes);
        self.used_vertices += needed; Ok(())
    }
    pub fn flush_buffer(&mut self) -> Result<(), Error> {
        if !self.locked { return Ok(()) }
        let buffer = self.buffer.as_mut().ok_or(Error::UnsupportedDomain)?;
        buffer.unlock(); self.locked = false;
        let count = self.indices.len()/3*3;
        if count > 0 { let _ = buffer.draw_triangles(&self.indices[..count]); }
        self.used_vertices = 0; self.indices.clear(); Ok(())
    }
    pub fn release_vertex_buffer_extension(&mut self) {
        // Drop of the acquired interface is slot-2 release only. Keep counts and lock token.
        self.buffer = None;
    }
}
