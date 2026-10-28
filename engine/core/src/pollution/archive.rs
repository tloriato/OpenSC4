// SPDX-License-Identifier: GPL-3.0-or-later
//! Pollution's persisted fields (pollution §6), before live reference resolution.

use crate::grid::CellIterator;
use crate::store::archive::{fields::array, ObjectReference};
use crate::stream::{ByteReader, InStream, OutStream};

#[derive(Clone, Debug, Default)]
pub struct GarbageTotals {
    pub produced: i64,
    pub imported: i64,
    pub exported: i64,
    pub recycled: f64,
    pub incinerated: i64,
    pub waste_to_energy: i64,
    pub landfilled: i64,
}

impl GarbageTotals {
    fn read(r: &mut dyn InStream) -> Option<Self> {
        Some(Self {
            produced: r.get_i64()?,
            imported: r.get_i64()?,
            exported: r.get_i64()?,
            recycled: r.get_f64()?,
            incinerated: r.get_i64()?,
            waste_to_energy: r.get_i64()?,
            landfilled: r.get_i64()?,
        })
    }

    fn write(&self, w: &mut dyn OutStream) {
        w.put_i64(self.produced);
        w.put_i64(self.imported);
        w.put_i64(self.exported);
        w.put_f64(self.recycled);
        w.put_i64(self.incinerated);
        w.put_i64(self.waste_to_energy);
        w.put_i64(self.landfilled);
    }
}

#[derive(Clone, Debug, Default)]
pub enum FacilityList {
    /// The original reads no references for counts at or above 10,000,000.
    Skipped(u32),
    #[default]
    Empty,
    Entries(Vec<ObjectReference>),
}

impl FacilityList {
    fn read(r: &mut ByteReader<'_>) -> Option<Self> {
        let n = r.get_u32()?;
        if n == 0 {
            return Some(Self::Empty);
        }
        if n >= 10_000_000 {
            return Some(Self::Skipped(n));
        }
        if n as usize > r.remaining() / 4 {
            return None;
        }
        let mut entries = Vec::with_capacity(n as usize);
        for _ in 0..n {
            entries.push(ObjectReference::read(r)?);
        }
        Some(Self::Entries(entries))
    }

    fn write(&self, w: &mut dyn OutStream) {
        match self {
            Self::Skipped(n) => {
                w.put_u32(*n);
            }
            Self::Empty => {
                w.put_u32(0);
            }
            Self::Entries(entries) => {
                w.put_u32(entries.len() as u32);
                for reference in entries {
                    reference.write(w);
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct PollutionRecord {
    pub version: u16,
    pub grids: [ObjectReference; 8],
    pub cell: [u32; 2],
    pub facilities: [FacilityList; 4],
    pub averages: [u32; 3],
    pub sums: [i32; 6],
    pub developed_tracts: u32,
    pub legacy_name: Option<Vec<u8>>,
    pub average_garbage: u16,
    pub capacities: [u32; 2],
    pub recycling_effect: f32,
    pub uncollected_flag: u8,
    pub total_uncollected: u32,
    pub unused_and_cost: [i64; 2],
    pub decomposed_last_month: Option<u32>,
    pub last_month: GarbageTotals,
    pub month: GarbageTotals,
    pub decomposed: u32,
    pub export_capacity: i32,
    pub active_landfill_cells: u32,
    pub tracts_per_day: u32,
    pub iterator: CellIterator,
    pub dirty: [u8; 4],
    pub legacy_unused: Option<u32>,
    pub efficiency: f32,
    pub garbage_supply_capacity: [i32; 2],
    pub department: ObjectReference,
}

impl PollutionRecord {
    pub fn read(r: &mut ByteReader<'_>) -> Option<Self> {
        let version = r.get_u16()?;
        if version > 4 {
            return None;
        }
        let grids = array(r, |r| ObjectReference::read(r))?;
        let cell = array(r, |r| r.get_u32())?;
        let mut facilities: [FacilityList; 4] = Default::default();
        for list in &mut facilities {
            *list = FacilityList::read(r)?;
        }
        Some(Self {
            version,
            grids,
            cell,
            facilities,
            averages: array(r, |r| r.get_u32())?,
            sums: array(r, |r| r.get_i32())?,
            developed_tracts: r.get_u32()?,
            legacy_name: if version < 3 { Some(r.get_str()?) } else { None },
            average_garbage: r.get_u16()?,
            capacities: array(r, |r| r.get_u32())?,
            recycling_effect: r.get_f32()?,
            uncollected_flag: r.get_u8()?,
            total_uncollected: r.get_u32()?,
            unused_and_cost: array(r, |r| r.get_i64())?,
            decomposed_last_month: if version >= 4 { Some(r.get_u32()?) } else { None },
            last_month: GarbageTotals::read(r)?,
            month: GarbageTotals::read(r)?,
            decomposed: r.get_u32()?,
            export_capacity: r.get_i32()?,
            active_landfill_cells: r.get_u32()?,
            tracts_per_day: r.get_u32()?,
            iterator: CellIterator::read(r)?,
            dirty: array(r, |r| r.get_u8())?,
            legacy_unused: if version < 3 { Some(r.get_u32()?) } else { None },
            efficiency: r.get_f32()?,
            garbage_supply_capacity: array(r, |r| r.get_i32())?,
            department: ObjectReference::read(r)?,
        })
    }

    pub fn write(&self, w: &mut dyn OutStream) -> bool {
        w.put_u16(self.version);
        for reference in self.grids {
            reference.write(w);
        }
        for v in self.cell {
            w.put_u32(v);
        }
        for list in &self.facilities {
            list.write(w);
        }
        for v in self.averages {
            w.put_u32(v);
        }
        for v in self.sums {
            w.put_i32(v);
        }
        w.put_u32(self.developed_tracts);
        if let Some(name) = &self.legacy_name {
            w.put_str(name);
        }
        w.put_u16(self.average_garbage);
        for v in self.capacities {
            w.put_u32(v);
        }
        w.put_f32(self.recycling_effect);
        w.put_u8(self.uncollected_flag);
        w.put_u32(self.total_uncollected);
        for v in self.unused_and_cost {
            w.put_i64(v);
        }
        if let Some(v) = self.decomposed_last_month {
            w.put_u32(v);
        }
        self.last_month.write(w);
        self.month.write(w);
        w.put_u32(self.decomposed);
        w.put_i32(self.export_capacity);
        w.put_u32(self.active_landfill_cells);
        w.put_u32(self.tracts_per_day);
        self.iterator.write(w);
        for v in self.dirty {
            w.put_u8(v);
        }
        if let Some(v) = self.legacy_unused {
            w.put_u32(v);
        }
        w.put_f32(self.efficiency);
        for v in self.garbage_supply_capacity {
            w.put_i32(v);
        }
        self.department.write(w);
        w.error() == 0
    }
}
