// SPDX-License-Identifier: GPL-3.0-or-later
//! Utility occupants before live geometry/table fixups, network-occupants
//! §§3.6 and 4.4. Attachment points remain private to each archived pole.
use super::super::record::Vertex;
use super::super::underground::write_wall;
use super::super::xform::Xform;
use crate::automata::OccupantBase;
use crate::store::archive::{
    fields::{array, count, references},
    ObjectReference,
};
use crate::stream::{ByteReader, InStream, OutStream};

fn xyz(r: &mut dyn InStream) -> Option<[f32; 3]> {
    array(r, |r| r.get_f32())
}

fn vertices(r: &mut ByteReader<'_>) -> Option<Vec<Vertex>> {
    let n = count(r, 24)?;
    let mut values = Vec::with_capacity(n);
    for _ in 0..n {
        values.push(Vertex::read(r)?);
    }
    Some(values)
}

#[derive(Clone, Debug)]
pub struct SavedPoleConnection {
    pub pole: ObjectReference,
    pub lines: Vec<ObjectReference>,
    pub flags: [u8; 2],
    pub words: [i32; 2],
    pub points: [Vec<Vec<[f32; 3]>>; 2],
}

#[derive(Clone, Debug)]
pub struct PoleArchive {
    pub version: u16,
    pub base: OccupantBase,
    pub transform: Xform,
    pub legacy_counter: Option<u32>,
    pub position: [f32; 3],
    pub connections: Vec<SavedPoleConnection>,
    pub exemplar_index: u32,
    pub model_ids: [u32; 2],
    pub cell: [i32; 2],
    pub attachments: [[f32; 3]; 4],
    pub geometry: [Vec<Vertex>; 3],
    pub model_y: f32,
    pub ignored: u8,
    pub bytes_54_55: [u8; 2],
    pub scale: f32,
    pub distressed: u8,
}

impl PoleArchive {
    pub fn read(r: &mut ByteReader<'_>) -> Option<Self> {
        let version = r.get_u16()?;
        if version < 4 {
            return None;
        }
        let base = OccupantBase::read(r)?;
        let mut transform = Xform::default();
        let legacy_counter = if version == 4 {
            transform.matrix = array(r, |r| r.get_f32())?;
            transform.translation = xyz(r)?;
            transform.scale = r.get_f32()?;
            Some(r.get_u32()?)
        } else {
            // Retain the mask even for an explicitly stored unit scale.
            transform.mask = r.get_u8()?;
            if transform.mask & 4 != 0 {
                transform.matrix = array(r, |r| r.get_f32())?;
            }
            if transform.mask & 1 != 0 {
                transform.translation = xyz(r)?;
            }
            if transform.mask & 2 != 0 {
                transform.scale = r.get_f32()?;
            }
            None
        };
        let position = xyz(r)?;
        let n = count(r, 26)?;
        let mut connections = Vec::with_capacity(n);
        for _ in 0..n {
            let pole = ObjectReference::read(r)?;
            let lines = references(r)?;
            let flags = [r.get_u8()?, r.get_u8()?];
            let words = [r.get_i32()?, r.get_i32()?];
            let mut points = [Vec::new(), Vec::new()];
            for collection in &mut points {
                let n = count(r, 4)?;
                for _ in 0..n {
                    let n = count(r, 12)?;
                    let mut line = Vec::with_capacity(n);
                    for _ in 0..n {
                        line.push(xyz(r)?);
                    }
                    collection.push(line);
                }
            }
            connections.push(SavedPoleConnection {
                pole,
                lines,
                flags,
                words,
                points,
            });
        }
        let exemplar_index = r.get_u32()?;
        let model_ids = [r.get_u32()?, r.get_u32()?];
        let cell = [r.get_i32()?, r.get_i32()?];
        let attachments = [xyz(r)?, xyz(r)?, xyz(r)?, xyz(r)?];
        let geometry = [vertices(r)?, vertices(r)?, vertices(r)?];
        Some(Self {
            version,
            base,
            transform,
            legacy_counter,
            position,
            connections,
            exemplar_index,
            model_ids,
            cell,
            attachments,
            geometry,
            model_y: r.get_f32()?,
            ignored: r.get_u8()?,
            bytes_54_55: [r.get_u8()?, r.get_u8()?],
            scale: r.get_f32()?,
            distressed: r.get_u8()?,
        })
    }

    pub fn write(&self, w: &mut dyn OutStream) -> bool {
        w.put_u16(self.version);
        self.base.write(w);
        if let Some(counter) = self.legacy_counter {
            for value in self.transform.matrix.into_iter().chain(self.transform.translation) {
                w.put_f32(value);
            }
            w.put_f32(self.transform.scale);
            w.put_u32(counter);
        } else {
            self.transform.write(w);
        }
        for value in self.position {
            w.put_f32(value);
        }
        w.put_u32(self.connections.len() as u32);
        for connection in &self.connections {
            connection.pole.write(w);
            w.put_u32(connection.lines.len() as u32);
            for line in &connection.lines {
                line.write(w);
            }
            w.put_bytes(&connection.flags);
            for word in connection.words {
                w.put_i32(word);
            }
            for collection in &connection.points {
                w.put_u32(collection.len() as u32);
                for line in collection {
                    w.put_u32(line.len() as u32);
                    for point in line {
                        for value in point {
                            w.put_f32(*value);
                        }
                    }
                }
            }
        }
        w.put_u32(self.exemplar_index);
        for word in self.model_ids {
            w.put_u32(word);
        }
        for cell in self.cell {
            w.put_i32(cell);
        }
        for point in self.attachments {
            for value in point {
                w.put_f32(value);
            }
        }
        for geometry in &self.geometry {
            write_wall(geometry, w);
        }
        w.put_f32(self.model_y);
        w.put_u8(self.ignored);
        w.put_bytes(&self.bytes_54_55);
        w.put_f32(self.scale);
        w.put_u8(self.distressed);
        w.error() == 0
    }
}

#[derive(Clone, Debug)]
pub struct ConnectionArchive {
    pub base: OccupantBase,
    pub active_walls: u32,
    pub walls: [Vec<Vertex>; 7],
    pub cell: [i32; 2],
    pub heights: [f32; 4],
    pub saved_170: f32,
    pub saved_178: i32,
    pub model_state: u8,
    pub attached: ObjectReference,
    pub linked: ObjectReference,
    pub saved_240: u32,
    pub position: [f32; 3],
}

impl ConnectionArchive {
    pub fn read(r: &mut ByteReader<'_>) -> Option<Self> {
        if r.get_u16()? != 2 {
            return None;
        }
        Some(Self {
            base: OccupantBase::read(r)?,
            active_walls: r.get_u32()?,
            walls: [
                vertices(r)?,
                vertices(r)?,
                vertices(r)?,
                vertices(r)?,
                vertices(r)?,
                vertices(r)?,
                vertices(r)?,
            ],
            cell: [r.get_i32()?, r.get_i32()?],
            heights: array(r, |r| r.get_f32())?,
            saved_170: r.get_f32()?,
            saved_178: r.get_i32()?,
            model_state: r.get_u8()?,
            attached: ObjectReference::read(r)?,
            linked: ObjectReference::read(r)?,
            saved_240: r.get_u32()?,
            position: xyz(r)?,
        })
    }

    pub fn write(&self, w: &mut dyn OutStream) -> bool {
        w.put_u16(2);
        self.base.write(w);
        w.put_u32(self.active_walls);
        for wall in &self.walls {
            write_wall(wall, w);
        }
        for cell in self.cell {
            w.put_i32(cell);
        }
        for height in self.heights {
            w.put_f32(height);
        }
        w.put_f32(self.saved_170);
        w.put_i32(self.saved_178);
        w.put_u8(self.model_state);
        self.attached.write(w);
        self.linked.write(w);
        w.put_u32(self.saved_240);
        for value in self.position {
            w.put_f32(value);
        }
        w.error() == 0
    }
}

#[cfg(test)]
mod tests;
