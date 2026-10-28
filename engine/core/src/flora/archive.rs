// SPDX-License-Identifier: GPL-3.0-or-later
//! Flora simulator saved state, flora §10.1. No city, registry, clock or live
//! occupant resolution is needed to decode the iterator and burnt-flora list.

use crate::grid::CellIterator;
use crate::store::archive::{
    fields::{array, references},
    ObjectReference,
};
use crate::stream::{ByteReader, InStream, OutStream};

#[derive(Clone, Debug)]
pub struct FloraSimulatorRecord {
    pub flags: [u8; 4],
    pub size: [f32; 2],
    pub cells: [i32; 2],
    pub dates_and_rates: [u32; 4],
    pub keep_iterator: u8,
    pub iterator: CellIterator,
    pub lcg: u32,
    pub mul: u32,
    pub cached_gaussian_flag: i32,
    pub cached_gaussian: f64,
    pub burnt: Vec<ObjectReference>,
    pub burn_accumulators: [f32; 2],
}

impl FloraSimulatorRecord {
    pub fn read(r: &mut ByteReader<'_>) -> Option<Self> {
        if r.get_u16()? != 2 {
            return None;
        }
        Some(Self {
            flags: array(r, |r| r.get_u8())?,
            size: array(r, |r| r.get_f32())?,
            cells: array(r, |r| r.get_i32())?,
            dates_and_rates: array(r, |r| r.get_u32())?,
            keep_iterator: r.get_u8()?,
            iterator: CellIterator::read(r)?,
            lcg: r.get_u32()?,
            mul: r.get_u32()?,
            cached_gaussian_flag: r.get_i32()?,
            cached_gaussian: r.get_f64()?,
            burnt: references(r)?,
            burn_accumulators: array(r, |r| r.get_f32())?,
        })
    }

    pub fn write(&self, w: &mut dyn OutStream) -> bool {
        w.put_u16(2);
        for flag in self.flags {
            w.put_u8(flag);
        }
        for size in self.size {
            w.put_f32(size);
        }
        for cells in self.cells {
            w.put_i32(cells);
        }
        for value in self.dates_and_rates {
            w.put_u32(value);
        }
        w.put_u8(self.keep_iterator);
        self.iterator.write(w);
        w.put_u32(self.lcg);
        w.put_u32(self.mul);
        w.put_i32(self.cached_gaussian_flag);
        w.put_f64(self.cached_gaussian);
        w.put_u32(self.burnt.len() as u32);
        for &reference in &self.burnt {
            reference.write(w);
        }
        for value in self.burn_accumulators {
            w.put_f32(value);
        }
        w.error() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::ByteWriter;

    #[test]
    fn dormant_rng_gaussian_and_iterator_table_survive_without_simulation() {
        let record = FloraSimulatorRecord {
            flags: [1, 0, 0, 1],
            size: [2048.0, 4096.0],
            cells: [128, 256],
            dates_and_rates: [2451545, 182, 3, 7],
            keep_iterator: 0xCD,
            iterator: CellIterator {
                state_raw: -1,
                mode: 0,
                dirty: 0xCD,
                x0: -1,
                z0: -2,
                x1: 3,
                z1: 4,
                count: 20,
                table: vec![0, 17, u16::MAX],
                index: -1,
                current: -2,
                step: 7,
                ..CellIterator::default()
            },
            lcg: u32::MAX,
            mul: 17,
            cached_gaussian_flag: 0,
            cached_gaussian: f64::from_bits(0x7FF8123456789ABC),
            burnt: vec![
                ObjectReference {
                    id: 99,
                    class: 0xA9C05C85,
                };
                2
            ],
            burn_accumulators: [-0.0, f32::from_bits(0x7FC01234)],
        };
        let mut w = ByteWriter::new();
        assert!(record.write(&mut w));
        let input = w.into_bytes();
        let mut r = ByteReader::new(&input);
        let decoded = FloraSimulatorRecord::read(&mut r).unwrap();
        assert_eq!(r.remaining(), 0);
        assert_eq!(decoded.cached_gaussian.to_bits(), 0x7FF8123456789ABC);
        assert_eq!(decoded.burnt[0], decoded.burnt[1]);
        let mut w = ByteWriter::new();
        assert!(decoded.write(&mut w));
        assert_eq!(w.bytes(), input);
    }
}
