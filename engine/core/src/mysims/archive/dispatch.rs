// SPDX-License-Identifier: GPL-3.0-or-later
//! Dispatch archive fields, mysims §6.2.1. References may form cycles with the
//! owner MySim; the class-store IDs are retained without service resolution.

use crate::store::archive::{
    fields::{array, count},
    ObjectReference,
};
use crate::stream::{ByteReader, InStream, OutStream};

#[derive(Clone, Copy, Debug, Default)]
pub struct LocationRecord {
    pub occupant: ObjectReference,
    pub world: [f32; 2],
    pub cell: [i32; 2],
    pub regional_city: [i32; 2],
    pub in_city: u8,
    pub by_commute: u8,
    pub duration: i32,
    pub deadline: i32,
}

impl LocationRecord {
    fn read(r: &mut dyn InStream) -> Option<Self> {
        Some(Self {
            occupant: ObjectReference::read(r)?,
            world: array(r, |r| r.get_f32())?,
            cell: array(r, |r| r.get_i32())?,
            regional_city: array(r, |r| r.get_i32())?,
            in_city: r.get_u8()?,
            by_commute: r.get_u8()?,
            duration: r.get_i32()?,
            deadline: r.get_i32()?,
        })
    }

    fn write(&self, w: &mut dyn OutStream) {
        self.occupant.write(w);
        for v in self.world {
            w.put_f32(v);
        }
        for v in self.cell {
            w.put_i32(v);
        }
        for v in self.regional_city {
            w.put_i32(v);
        }
        w.put_u8(self.in_city);
        w.put_u8(self.by_commute);
        w.put_i32(self.duration);
        w.put_i32(self.deadline);
    }
}

#[derive(Clone, Debug)]
pub struct DispatchRecord {
    pub version: u32,
    pub auxiliary_flag: u8,
    pub words: [u32; 18],
    pub locations: [LocationRecord; 4],
    pub auxiliary_occupant: ObjectReference,
    pub auxiliary_value: u32,
    pub auxiliary_string: Vec<u8>,
    pub automata: ObjectReference,
    pub owner: ObjectReference,
    pub service: ObjectReference,
    pub route: Vec<[u8; 3]>,
    pub models: Vec<(i32, [u32; 3], i32)>,
    pub marker: i32,
    pub destinations: Vec<LocationRecord>,
}

impl DispatchRecord {
    pub fn read(r: &mut ByteReader<'_>) -> Option<Self> {
        let version = r.get_u32()?;
        let auxiliary_flag = r.get_u8()?;
        let words = array(r, |r| r.get_u32())?;
        let locations = array(r, LocationRecord::read)?;
        let auxiliary_occupant = ObjectReference::read(r)?;
        let auxiliary_value = r.get_u32()?;
        let auxiliary_string = r.get_str()?;
        let automata = ObjectReference::read(r)?;
        let owner = ObjectReference::read(r)?;
        let service = ObjectReference::read(r)?;
        let n = count(r, 3)?;
        let mut route = Vec::with_capacity(n);
        for _ in 0..n {
            route.push(array(r, |r| r.get_u8())?);
        }
        let n = count(r, 20)?;
        let mut models = Vec::with_capacity(n);
        for _ in 0..n {
            models.push((r.get_i32()?, array(r, |r| r.get_u32())?, r.get_i32()?));
        }
        let marker = r.get_i32()?;
        let n = count(r, 38)?;
        let mut destinations = Vec::with_capacity(n);
        for _ in 0..n {
            destinations.push(LocationRecord::read(r)?);
        }
        Some(Self {
            version,
            auxiliary_flag,
            words,
            locations,
            auxiliary_occupant,
            auxiliary_value,
            auxiliary_string,
            automata,
            owner,
            service,
            route,
            models,
            marker,
            destinations,
        })
    }

    pub fn write(&self, w: &mut dyn OutStream) -> bool {
        w.put_u32(self.version);
        w.put_u8(self.auxiliary_flag);
        for v in self.words {
            w.put_u32(v);
        }
        for location in &self.locations {
            location.write(w);
        }
        self.auxiliary_occupant.write(w);
        w.put_u32(self.auxiliary_value);
        w.put_str(&self.auxiliary_string);
        self.automata.write(w);
        self.owner.write(w);
        self.service.write(w);
        w.put_u32(self.route.len() as u32);
        for route in &self.route {
            w.put_bytes(route);
        }
        w.put_u32(self.models.len() as u32);
        for (travel, key, offset) in &self.models {
            w.put_i32(*travel);
            for v in key {
                w.put_u32(*v);
            }
            w.put_i32(*offset);
        }
        w.put_i32(self.marker);
        w.put_u32(self.destinations.len() as u32);
        for location in &self.destinations {
            location.write(w);
        }
        w.error() == 0
    }
}
