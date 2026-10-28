// SPDX-License-Identifier: GPL-3.0-or-later
//! Saved residential/education/health fields, residential §9. Archive decoding
//! keeps percentages, state, dirty bytes and null list entries unchanged; the
//! live loader's state transitions and Init's tuning resets do not run here.

use crate::store::archive::{
    fields::{array, count, references},
    ObjectReference,
};
use crate::stream::{ByteReader, InStream, OutStream};

#[derive(Clone, Debug)]
pub struct SavedDepartment {
    pub f0: f32,
    pub max_funding: f32,
    pub strike_efficiency: f32,
    pub strike_duration: i32,
    pub min_grade: f32,
    pub strike: u8,
    pub strike_start: i32,
    pub departments: [ObjectReference; 2],
    pub initial_funding: f32,
    pub hundred: f32,
    pub id: u32,
}

impl SavedDepartment {
    fn read(r: &mut dyn InStream) -> Option<Self> {
        Some(Self {
            f0: r.get_f32()?,
            max_funding: r.get_f32()?,
            strike_efficiency: r.get_f32()?,
            strike_duration: r.get_i32()?,
            min_grade: r.get_f32()?,
            strike: r.get_u8()?,
            strike_start: r.get_i32()?,
            departments: array(r, ObjectReference::read)?,
            initial_funding: r.get_f32()?,
            hundred: r.get_f32()?,
            id: r.get_u32()?,
        })
    }

    fn write(&self, w: &mut dyn OutStream) {
        w.put_f32(self.f0);
        w.put_f32(self.max_funding);
        w.put_f32(self.strike_efficiency);
        w.put_i32(self.strike_duration);
        w.put_f32(self.min_grade);
        w.put_u8(self.strike);
        w.put_i32(self.strike_start);
        for department in self.departments {
            department.write(w);
        }
        w.put_f32(self.initial_funding);
        w.put_f32(self.hundred);
        w.put_u32(self.id);
    }
}

#[derive(Clone, Debug)]
pub struct SavedIterator {
    pub state: u8,
    // Signed coordinates and the unsigned step share their exact stream bits.
    pub coordinates_step: [u32; 7],
    pub le_sum: u64,
}

#[derive(Clone, Debug)]
pub struct ResidentialRecord {
    pub version: u16,
    pub proximity_size: i32,
    pub proximity: [ObjectReference; 3],
    pub population_size: i32,
    pub population_grids: [ObjectReference; 3],
    pub age: ObjectReference,
    pub school_coverage: Vec<ObjectReference>,
    pub hospital_coverage: Vec<ObjectReference>,
    pub grades: [ObjectReference; 2],
    pub eq_size: i32,
    pub eq: [ObjectReference; 3],
    pub hq: [ObjectReference; 3],
    pub unused: [f32; 2],
    pub state: u32,
    pub dirty: u8,
    pub population: i32,
    pub health: SavedDepartment,
    pub education: SavedDepartment,
    pub pending_school: Vec<ObjectReference>,
    pub pending_hospital: Vec<ObjectReference>,
    pub pending_health: Vec<ObjectReference>,
    pub school_buildings: Vec<Vec<ObjectReference>>,
    pub hospital_buildings: Vec<Vec<ObjectReference>>,
    pub health_buildings: Vec<ObjectReference>,
    pub iterator: Option<SavedIterator>,
    pub running_population: Option<i32>,
    pub auto_budget: Option<[u8; 2]>,
}

impl ResidentialRecord {
    pub fn read(r: &mut ByteReader<'_>) -> Option<Self> {
        let version = r.get_u16()?;
        let proximity_size = r.get_i32()?;
        let proximity = array(r, ObjectReference::read)?;
        let population_size = r.get_i32()?;
        let population_grids = array(r, ObjectReference::read)?;
        let age = ObjectReference::read(r)?;
        let school_coverage = references(r)?;
        let hospital_coverage = references(r)?;
        let grades = array(r, ObjectReference::read)?;
        let eq_size = r.get_i32()?;
        let eq = array(r, ObjectReference::read)?;
        let hq = array(r, ObjectReference::read)?;
        let unused = array(r, |r| r.get_f32())?;
        let state = r.get_u32()?;
        let dirty = r.get_u8()?;
        let population = r.get_i32()?;
        let health = SavedDepartment::read(r)?;
        let education = SavedDepartment::read(r)?;
        let pending_school = references(r)?;
        let pending_hospital = references(r)?;
        let pending_health = references(r)?;
        let school_buildings = read_lists(r)?;
        let hospital_buildings = read_lists(r)?;
        let health_buildings = references(r)?;
        let iterator = if version >= 3 {
            Some(SavedIterator {
                state: r.get_u8()?,
                coordinates_step: array(r, |r| r.get_u32())?,
                le_sum: r.get_u64()?,
            })
        } else {
            None
        };
        let running_population = if version >= 4 { Some(r.get_i32()?) } else { None };
        let auto_budget = if version >= 7 {
            Some(array(r, |r| r.get_u8())?)
        } else {
            None
        };
        Some(Self {
            version,
            proximity_size,
            proximity,
            population_size,
            population_grids,
            age,
            school_coverage,
            hospital_coverage,
            grades,
            eq_size,
            eq,
            hq,
            unused,
            state,
            dirty,
            population,
            health,
            education,
            pending_school,
            pending_hospital,
            pending_health,
            school_buildings,
            hospital_buildings,
            health_buildings,
            iterator,
            running_population,
            auto_budget,
        })
    }

    pub fn write(&self, w: &mut dyn OutStream) -> bool {
        w.put_u16(self.version);
        w.put_i32(self.proximity_size);
        for reference in self.proximity {
            reference.write(w);
        }
        w.put_i32(self.population_size);
        for reference in self.population_grids {
            reference.write(w);
        }
        self.age.write(w);
        put_list(w, &self.school_coverage);
        put_list(w, &self.hospital_coverage);
        for reference in self.grades {
            reference.write(w);
        }
        w.put_i32(self.eq_size);
        for reference in self.eq {
            reference.write(w);
        }
        for reference in self.hq {
            reference.write(w);
        }
        for v in self.unused {
            w.put_f32(v);
        }
        w.put_u32(self.state);
        w.put_u8(self.dirty);
        w.put_i32(self.population);
        self.health.write(w);
        self.education.write(w);
        put_list(w, &self.pending_school);
        put_list(w, &self.pending_hospital);
        put_list(w, &self.pending_health);
        put_lists(w, &self.school_buildings);
        put_lists(w, &self.hospital_buildings);
        put_list(w, &self.health_buildings);
        if let Some(iterator) = &self.iterator {
            w.put_u8(iterator.state);
            for v in iterator.coordinates_step {
                w.put_u32(v);
            }
            w.put_u64(iterator.le_sum);
        }
        if let Some(population) = self.running_population {
            w.put_i32(population);
        }
        if let Some(auto_budget) = self.auto_budget {
            for v in auto_budget {
                w.put_u8(v);
            }
        }
        w.error() == 0
    }
}

fn read_lists(r: &mut ByteReader<'_>) -> Option<Vec<Vec<ObjectReference>>> {
    let n = count(r, 4)?;
    let mut lists = Vec::with_capacity(n);
    for _ in 0..n {
        lists.push(references(r)?);
    }
    Some(lists)
}

fn put_list(w: &mut dyn OutStream, refs: &[ObjectReference]) {
    w.put_u32(refs.len() as u32);
    for reference in refs {
        reference.write(w);
    }
}

fn put_lists(w: &mut dyn OutStream, lists: &[Vec<ObjectReference>]) {
    w.put_u32(lists.len() as u32);
    for list in lists {
        put_list(w, list);
    }
}
