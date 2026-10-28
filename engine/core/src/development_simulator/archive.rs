// SPDX-License-Identifier: GPL-3.0-or-later
//! Building development archive, building-development §6. Saved map/list order
//! and null references survive without constructing developers or merging marks.
use crate::store::archive::{
    fields::{count, references},
    ObjectReference,
};
use crate::stream::{ByteReader, InStream, OutStream};

#[derive(Clone, Debug)]
pub struct PlannedGrowth {
    pub developer: ObjectReference,
    pub cell: [i32; 2],
    pub stage: u8,
    pub day: u32,
}

#[derive(Clone, Debug)]
pub struct DevelopmentRecord {
    pub building_map: ObjectReference,
    pub construction: ObjectReference,
    pub developers: Vec<(u32, ObjectReference)>,
    pub special: Vec<ObjectReference>,
    pub stages: Vec<(u32, u8)>,
    pub marks: Vec<(u32, Vec<[i32; 2]>)>,
    pub planned: Vec<PlannedGrowth>,
    pub ignore: [u8; 3],
}

impl DevelopmentRecord {
    pub fn read(r: &mut ByteReader<'_>) -> Option<Self> {
        if r.get_u16()? != 4 {
            return None;
        }
        let building_map = ObjectReference::read(r)?;
        let construction = ObjectReference::read(r)?;
        let n = count(r, 8)?;
        let mut developers = Vec::with_capacity(n);
        for _ in 0..n {
            developers.push((r.get_u32()?, ObjectReference::read(r)?));
        }
        let special = references(r)?;
        let n = count(r, 5)?;
        let mut stages = Vec::with_capacity(n);
        for _ in 0..n {
            stages.push((r.get_u32()?, r.get_u8()?));
        }
        let n = count(r, 8)?;
        let mut marks = Vec::with_capacity(n);
        for _ in 0..n {
            let ty = r.get_u32()?;
            let n = count(r, 8)?;
            let mut cells = Vec::with_capacity(n);
            for _ in 0..n {
                cells.push([r.get_i32()?, r.get_i32()?]);
            }
            marks.push((ty, cells));
        }
        let n = count(r, 17)?;
        let mut planned = Vec::with_capacity(n);
        for _ in 0..n {
            planned.push(PlannedGrowth {
                developer: ObjectReference::read(r)?,
                cell: [r.get_i32()?, r.get_i32()?],
                stage: r.get_u8()?,
                day: r.get_u32()?,
            });
        }
        Some(Self {
            building_map,
            construction,
            developers,
            special,
            stages,
            marks,
            planned,
            ignore: [r.get_u8()?, r.get_u8()?, r.get_u8()?],
        })
    }

    pub fn write(&self, w: &mut dyn OutStream) -> bool {
        w.put_u16(4);
        self.building_map.write(w);
        self.construction.write(w);
        w.put_u32(self.developers.len() as u32);
        for (ty, reference) in &self.developers {
            w.put_u32(*ty);
            reference.write(w);
        }
        w.put_u32(self.special.len() as u32);
        for reference in &self.special {
            reference.write(w);
        }
        w.put_u32(self.stages.len() as u32);
        for (ty, stage) in &self.stages {
            w.put_u32(*ty);
            w.put_u8(*stage);
        }
        w.put_u32(self.marks.len() as u32);
        for (ty, cells) in &self.marks {
            w.put_u32(*ty);
            w.put_u32(cells.len() as u32);
            for [x, z] in cells {
                w.put_i32(*x);
                w.put_i32(*z);
            }
        }
        w.put_u32(self.planned.len() as u32);
        for plan in &self.planned {
            plan.developer.write(w);
            for cell in plan.cell {
                w.put_i32(cell);
            }
            w.put_u8(plan.stage);
            w.put_u32(plan.day);
        }
        w.put_bytes(&self.ignore);
        w.error() == 0
    }
}
