// SPDX-License-Identifier: GPL-3.0-or-later
//! Plumbing §9, including the separately saved current-producer record.
//! Archive decoding does not match it to or discard it in the live list.

use crate::store::archive::{
    fields::{array, count},
    ObjectReference,
};
use crate::stream::{ByteReader, InStream, OutStream};

#[derive(Clone, Debug)]
pub struct ProducerRecord {
    pub cells: [i32; 6],
    pub maximum_capacity: u32,
    pub production: u32,
    pub occupant: ObjectReference,
}

impl ProducerRecord {
    fn read(r: &mut dyn InStream) -> Option<Self> {
        Some(Self {
            cells: array(r, |r| r.get_i32())?,
            maximum_capacity: r.get_u32()?,
            production: r.get_u32()?,
            occupant: ObjectReference::read(r)?,
        })
    }

    fn write(&self, w: &mut dyn OutStream) {
        for value in self.cells {
            w.put_i32(value);
        }
        w.put_u32(self.maximum_capacity);
        w.put_u32(self.production);
        self.occupant.write(w);
    }
}

#[derive(Clone, Debug)]
pub struct PlumbingRecord {
    pub grids: [ObjectReference; 2],
    pub dimensions: [u32; 2],
    pub flags: Vec<u8>,
    pub pass_counter: i32,
    pub average_pollution: u32,
    pub total_cost: i64,
    pub polluted_count: u32,
    pub producers: Vec<ProducerRecord>,
    pub current: ProducerRecord,
    pub iterator: SavedScan,
    pub demand: i64,
    pub distressed: u32,
    pub burst: u32,
    pub pipes: Vec<ObjectReference>,
    pub department: ObjectReference,
    pub last_totals: [i64; 4],
}

impl PlumbingRecord {
    pub fn read(r: &mut ByteReader<'_>) -> Option<Self> {
        if r.get_u16()? != 4 {
            return None;
        }
        let grids = array(r, ObjectReference::read)?;
        let dimensions = array(r, |r| r.get_u32())?;
        let cells = (dimensions[0] as usize).checked_mul(dimensions[1] as usize)?;
        let flags = r.get_bytes(cells)?;
        let pass_counter = r.get_i32()?;
        let average_pollution = r.get_u32()?;
        let total_cost = r.get_i64()?;
        let n = count(r, 36)?;
        let polluted_count = r.get_u32()?;
        let mut producers = Vec::with_capacity(n);
        for _ in 0..n {
            producers.push(ProducerRecord::read(r)?);
        }
        let current = ProducerRecord::read(r)?;
        let iterator = SavedScan::read(r)?;
        let demand = r.get_i64()?;
        let n = count(r, 4)?;
        let distressed = r.get_u32()?;
        let burst = r.get_u32()?;
        let mut pipes = Vec::with_capacity(n);
        for _ in 0..n {
            pipes.push(ObjectReference::read(r)?);
        }
        let department = ObjectReference::read(r)?;
        let last_totals = array(r, |r| r.get_i64())?;
        Some(Self {
            grids,
            dimensions,
            flags,
            pass_counter,
            average_pollution,
            total_cost,
            polluted_count,
            producers,
            current,
            iterator,
            demand,
            distressed,
            burst,
            pipes,
            department,
            last_totals,
        })
    }

    pub fn write(&self, w: &mut dyn OutStream) -> bool {
        if (self.dimensions[0] as usize).checked_mul(self.dimensions[1] as usize) != Some(self.flags.len()) {
            w.set_error();
            return false;
        }
        w.put_u16(4);
        for grid in self.grids {
            grid.write(w);
        }
        for dimension in self.dimensions {
            w.put_u32(dimension);
        }
        w.put_bytes(&self.flags);
        w.put_i32(self.pass_counter);
        w.put_u32(self.average_pollution);
        w.put_i64(self.total_cost);
        w.put_u32(self.producers.len() as u32);
        w.put_u32(self.polluted_count);
        for producer in &self.producers {
            producer.write(w);
        }
        self.current.write(w);
        self.iterator.write(w);
        w.put_i64(self.demand);
        w.put_u32(self.pipes.len() as u32);
        w.put_u32(self.distressed);
        w.put_u32(self.burst);
        for &pipe in &self.pipes {
            pipe.write(w);
        }
        self.department.write(w);
        for value in self.last_totals {
            w.put_i64(value);
        }
        w.error() == 0
    }
}

/// The 29 saved bytes, without live iterator validation or step clamping.
#[derive(Clone, Debug, Default)]
pub struct SavedScan {
    pub state: u8,
    pub cell: [i32; 2],
    pub step: u32,
    pub rectangle: [i32; 4],
}

impl SavedScan {
    fn read(r: &mut dyn InStream) -> Option<Self> {
        Some(Self {
            state: r.get_u8()?,
            cell: array(r, |r| r.get_i32())?,
            step: r.get_u32()?,
            rectangle: array(r, |r| r.get_i32())?,
        })
    }

    fn write(&self, w: &mut dyn OutStream) {
        w.put_u8(self.state);
        for v in self.cell {
            w.put_i32(v);
        }
        w.put_u32(self.step);
        for v in self.rectangle {
            w.put_i32(v);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::ByteWriter;

    #[test]
    fn independently_saved_current_producer_is_not_replaced_by_a_list_match() {
        let producer = ProducerRecord {
            cells: [1, -2, 3, -4, 5, -6],
            maximum_capacity: 100,
            production: 91,
            occupant: ObjectReference {
                id: 19,
                class: 0xA9BD882D,
            },
        };
        let record = PlumbingRecord {
            grids: [ObjectReference {
                id: 71,
                class: 0x49B9E602,
            }; 2],
            dimensions: [2, 3],
            flags: vec![0, 1, 2, 3, 128, 255],
            pass_counter: -1,
            average_pollution: u32::MAX,
            total_cost: i64::MIN,
            polluted_count: 5,
            producers: vec![producer.clone(), producer.clone()],
            current: ProducerRecord {
                production: 17,
                ..producer
            },
            iterator: SavedScan {
                state: 2,
                cell: [-1, 3],
                step: u32::MAX,
                rectangle: [0, 0, 1, 2],
            },
            demand: i64::MAX,
            distressed: 2,
            burst: 1,
            pipes: vec![
                ObjectReference::default(),
                ObjectReference {
                    id: 99,
                    class: 0x49C05B9F,
                },
            ],
            department: ObjectReference {
                id: 57,
                class: 0xE990BFFC,
            },
            last_totals: [i64::MIN, -1, 0, i64::MAX],
        };
        let mut w = ByteWriter::new();
        assert!(record.write(&mut w));
        let input = w.into_bytes();
        let mut r = ByteReader::new(&input);
        let decoded = PlumbingRecord::read(&mut r).unwrap();
        assert_eq!(r.remaining(), 0);
        assert_eq!(decoded.current.production, 17);
        assert_eq!(decoded.producers[0].production, 91);
        assert_eq!(decoded.iterator.step, u32::MAX);
        let mut w = ByteWriter::new();
        assert!(decoded.write(&mut w));
        assert_eq!(w.bytes(), input);
        for len in 0..input.len() {
            assert!(PlumbingRecord::read(&mut ByteReader::new(&input[..len])).is_none());
        }
    }
}
