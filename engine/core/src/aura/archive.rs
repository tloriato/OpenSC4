// SPDX-License-Identifier: GPL-3.0-or-later
//! Aura's saved grids, departments and ordered hotspots (aura §11.2–§11.3).

use crate::store::archive::{
    fields::{array, count},
    ObjectReference,
};
use crate::stream::{ByteReader, InStream, OutStream};

#[derive(Clone, Debug)]
pub enum LandmarkGrids {
    Legacy(ObjectReference),
    Departments(Vec<(u32, ObjectReference)>),
}

#[derive(Clone, Debug)]
pub struct AuraRecord {
    pub grids: [ObjectReference; 7],
    pub landmarks: LandmarkGrids,
    pub ratings: [i32; 2],
    pub max_location: [i32; 2],
    pub max_rating: i16,
    pub min_location: [i32; 2],
    pub min_rating: i16,
    pub hotspots: Vec<[i32; 3]>,
}

impl AuraRecord {
    pub fn read(r: &mut ByteReader<'_>) -> Option<Self> {
        let version = r.get_u16()?;
        if !(8..=9).contains(&version) {
            return None;
        }
        let grids = array(r, |r| ObjectReference::read(r))?;
        let landmarks = if version == 8 {
            LandmarkGrids::Legacy(ObjectReference::read(r)?)
        } else {
            let n = count(r, 8)?;
            let mut departments = Vec::with_capacity(n);
            for _ in 0..n {
                departments.push((r.get_u32()?, ObjectReference::read(r)?));
            }
            LandmarkGrids::Departments(departments)
        };
        let ratings = array(r, |r| r.get_i32())?;
        let max_location = array(r, |r| r.get_i32())?;
        let max_rating = r.get_i16()?;
        let min_location = array(r, |r| r.get_i32())?;
        let min_rating = r.get_i16()?;
        let n = count(r, 12)?;
        let mut hotspots = Vec::with_capacity(n);
        for _ in 0..n {
            hotspots.push(array(r, |r| r.get_i32())?);
        }
        Some(Self {
            grids,
            landmarks,
            ratings,
            max_location,
            max_rating,
            min_location,
            min_rating,
            hotspots,
        })
    }

    pub fn write(&self, w: &mut dyn OutStream) -> bool {
        w.put_u16(if matches!(self.landmarks, LandmarkGrids::Legacy(_)) {
            8
        } else {
            9
        });
        for reference in self.grids {
            reference.write(w);
        }
        match &self.landmarks {
            LandmarkGrids::Legacy(reference) => {
                reference.write(w);
            }
            LandmarkGrids::Departments(departments) => {
                w.put_u32(departments.len() as u32);
                for (id, reference) in departments {
                    w.put_u32(*id);
                    reference.write(w);
                }
            }
        }
        for v in self.ratings {
            w.put_i32(v);
        }
        for v in self.max_location {
            w.put_i32(v);
        }
        w.put_i16(self.max_rating);
        for v in self.min_location {
            w.put_i32(v);
        }
        w.put_i16(self.min_rating);
        w.put_u32(self.hotspots.len() as u32);
        for hotspot in &self.hotspots {
            for v in hotspot {
                w.put_i32(*v);
            }
        }
        w.error() == 0
    }
}
