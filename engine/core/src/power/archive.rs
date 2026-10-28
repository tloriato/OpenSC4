// SPDX-License-Identifier: GPL-3.0-or-later
//! Power's saved fields (power §9), before live Read resets the strike byte.
//! The consumption map, pole list and four wire bitmaps retain file order.

use crate::store::archive::{
    fields::{array, count},
    ObjectReference,
};
use crate::stream::{ByteReader, InStream, OutStream};

#[derive(Clone, Debug)]
pub struct PowerRecord {
    pub department: ObjectReference,
    pub ring_index: i32,
    pub pass_totals: [i32; 2],
    pub deficit: f32,
    pub counts: [i32; 4],
    pub cost_base: i64,
    pub last_totals: [i32; 5],
    pub grid: ObjectReference,
    pub dimensions: [u32; 2],
    pub consumption: Vec<i16>,
    pub ring: [i32; 120],
    pub annual: [i32; 10],
    pub per_type: [i32; 10],
    pub plants: Vec<(u32, ObjectReference)>,
    pub broken_count: u32,
    pub poles: Vec<ObjectReference>,
    pub bitmaps: [Vec<u32>; 4],
    pub unused_flag: u8,
    pub strike_flag: u8,
    pub strike_start: i32,
    pub recompute: u8,
    pub age_plants: u8,
    pub state: u32,
}

impl PowerRecord {
    pub fn read(r: &mut ByteReader<'_>) -> Option<Self> {
        let version = r.get_u16()?;
        if version > 6 {
            return None;
        }
        let department = ObjectReference::read(r)?;
        let ring_index = r.get_i32()?;
        let pass_totals = array(r, |r| r.get_i32())?;
        let deficit = if version < 6 {
            (f64::from(r.get_i32()?) * f64::from(0.01f32)) as f32
        } else {
            r.get_f32()?
        };
        let counts = array(r, |r| r.get_i32())?;
        let cost_base = r.get_i64()?;
        let last_totals = array(r, |r| r.get_i32())?;
        let grid = ObjectReference::read(r)?;
        let dimensions = array(r, |r| r.get_u32())?;
        let cells = (dimensions[0] as usize).checked_mul(dimensions[1] as usize)?;
        if cells > r.remaining() / 2 {
            return None;
        }
        let mut consumption = Vec::with_capacity(cells);
        for _ in 0..cells {
            consumption.push(r.get_i16()?);
        }
        let ring = array(r, |r| r.get_i32())?;
        let annual = array(r, |r| r.get_i32())?;
        let n = count(r, 8)?;
        let per_type = array(r, |r| r.get_i32())?;
        let mut plants = Vec::with_capacity(n);
        for _ in 0..n {
            plants.push((r.get_u32()?, ObjectReference::read(r)?));
        }
        let n = count(r, 4)?;
        let broken_count = r.get_u32()?;
        let mut poles = Vec::with_capacity(n);
        for _ in 0..n {
            poles.push(ObjectReference::read(r)?);
        }
        let words = count(r, 16)?;
        let mut bitmaps: [Vec<u32>; 4] = Default::default();
        for bitmap in &mut bitmaps {
            for _ in 0..words {
                bitmap.push(r.get_u32()?);
            }
        }
        Some(Self {
            department,
            ring_index,
            pass_totals,
            deficit,
            counts,
            cost_base,
            last_totals,
            grid,
            dimensions,
            consumption,
            ring,
            annual,
            per_type,
            plants,
            broken_count,
            poles,
            bitmaps,
            unused_flag: r.get_u8()?,
            strike_flag: r.get_u8()?,
            strike_start: r.get_i32()?,
            recompute: r.get_u8()?,
            age_plants: r.get_u8()?,
            state: r.get_u32()?,
        })
    }

    pub fn write(&self, w: &mut dyn OutStream) -> bool {
        if (self.dimensions[0] as usize).checked_mul(self.dimensions[1] as usize) != Some(self.consumption.len())
            || self.bitmaps.iter().any(|b| b.len() != self.bitmaps[0].len())
        {
            w.set_error();
            return false;
        }
        w.put_u16(6);
        self.department.write(w);
        w.put_i32(self.ring_index);
        for value in self.pass_totals {
            w.put_i32(value);
        }
        w.put_f32(self.deficit);
        for value in self.counts {
            w.put_i32(value);
        }
        w.put_i64(self.cost_base);
        for value in self.last_totals {
            w.put_i32(value);
        }
        self.grid.write(w);
        for value in self.dimensions {
            w.put_u32(value);
        }
        for &value in &self.consumption {
            w.put_i16(value);
        }
        for value in self.ring {
            w.put_i32(value);
        }
        for value in self.annual {
            w.put_i32(value);
        }
        w.put_u32(self.plants.len() as u32);
        for value in self.per_type {
            w.put_i32(value);
        }
        for &(visited, plant) in &self.plants {
            w.put_u32(visited);
            plant.write(w);
        }
        w.put_u32(self.poles.len() as u32);
        w.put_u32(self.broken_count);
        for &pole in &self.poles {
            pole.write(w);
        }
        w.put_u32(self.bitmaps[0].len() as u32);
        for bitmap in &self.bitmaps {
            for &value in bitmap {
                w.put_u32(value);
            }
        }
        w.put_u8(self.unused_flag);
        w.put_u8(self.strike_flag);
        w.put_i32(self.strike_start);
        w.put_u8(self.recompute);
        w.put_u8(self.age_plants);
        w.put_u32(self.state);
        w.error() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::ByteWriter;

    #[test]
    fn saved_strike_and_nan_bits_are_not_live_read_transitions() {
        let mut record = PowerRecord {
            department: ObjectReference {
                id: 71,
                class: 0xE990BFFC,
            },
            ring_index: -1,
            pass_totals: [-1, i32::MIN],
            deficit: f32::from_bits(0x7FC01234),
            counts: [7, 8, 9, 10],
            cost_base: i64::MIN,
            last_totals: [1, 2, 3, 4, 5],
            grid: ObjectReference {
                id: 19,
                class: 0x49B9E602,
            },
            dimensions: [2, 3],
            consumption: vec![-1, 2, -3, 4, -5, i16::MIN],
            ring: std::array::from_fn(|i| i as i32),
            annual: [i32::MAX; 10],
            per_type: [-1; 10],
            plants: vec![(
                u32::MAX,
                ObjectReference {
                    id: 99,
                    class: 0xA9BD882D,
                },
            )],
            broken_count: 1,
            poles: vec![
                ObjectReference::default(),
                ObjectReference {
                    id: 29,
                    class: 0x09C05C6A,
                },
            ],
            bitmaps: [vec![1, 2], vec![3, 4], vec![5, 6], vec![7, 8]],
            unused_flag: 0xA5,
            strike_flag: 1,
            strike_start: -17,
            recompute: 1,
            age_plants: 0,
            state: 3,
        };
        let mut w = ByteWriter::new();
        assert!(record.write(&mut w));
        let input = w.into_bytes();
        let mut r = ByteReader::new(&input);
        record = PowerRecord::read(&mut r).unwrap();
        assert_eq!(r.remaining(), 0);
        assert_eq!(record.deficit.to_bits(), 0x7FC01234);
        assert_eq!(record.strike_flag, 1);
        assert_eq!(record.consumption, [-1, 2, -3, 4, -5, i16::MIN]);
        let mut w = ByteWriter::new();
        assert!(record.write(&mut w));
        assert_eq!(w.bytes(), input);
        for len in 0..input.len() {
            assert!(PowerRecord::read(&mut ByteReader::new(&input[..len])).is_none());
        }
    }
}
