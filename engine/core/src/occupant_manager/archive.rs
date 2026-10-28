// SPDX-License-Identifier: GPL-3.0-or-later
//! Saved spatial index, occupant-manager §13. Unlike live Read, an archive
//! keeps unresolved identities, repeated occupants, empty cells and raw counts.

use crate::store::archive::ObjectReference;
use crate::stream::{ByteReader, InStream, OutStream};

#[derive(Clone, Debug)]
pub struct OccupantManagerRecord {
    pub size: [f32; 2],
    pub counts: [i32; 4],
    pub rows: Vec<Vec<Vec<ObjectReference>>>,
}

impl OccupantManagerRecord {
    pub fn read(r: &mut ByteReader<'_>) -> Option<Self> {
        if r.get_u16()? != 1 {
            return None;
        }
        let size = [r.get_f32()?, r.get_f32()?];
        let counts = [r.get_i32()?, r.get_i32()?, r.get_i32()?, r.get_i32()?];
        let row_count = count(r)?;
        let mut rows = Vec::new();
        for _ in 0..row_count {
            let cell_count = count(r)?;
            let mut row = Vec::new();
            for _ in 0..cell_count {
                let occupants = count(r)?;
                let mut cell = Vec::new();
                for _ in 0..occupants {
                    cell.push(ObjectReference::read(r)?);
                }
                row.push(cell);
            }
            rows.push(row);
        }
        Some(Self { size, counts, rows })
    }

    pub fn write(&self, w: &mut dyn OutStream) -> bool {
        w.put_u16(1);
        for value in self.size {
            w.put_f32(value);
        }
        for value in self.counts {
            w.put_i32(value);
        }
        w.put_u32(self.rows.len() as u32);
        for row in &self.rows {
            w.put_u32(row.len() as u32);
            for cell in row {
                w.put_u32(cell.len() as u32);
                for reference in cell {
                    reference.write(w);
                }
            }
        }
        w.error() == 0
    }
}

fn count(r: &mut ByteReader<'_>) -> Option<usize> {
    let count = r.get_u32()? as usize;
    (count <= r.remaining() / 4).then_some(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::ByteWriter;

    #[test]
    fn ragged_cells_keep_order_nulls_repeats_and_raw_geometry() {
        let occupant = ObjectReference {
            id: 91,
            class: 0xA9BC_9AB6,
        };
        let mut model = OccupantManagerRecord {
            size: [f32::from_bits(0x7FC0_4321), -0.0],
            counts: [-1, 37, i32::MIN, 0],
            rows: vec![
                vec![vec![occupant, ObjectReference { id: 0, class: 0 }, occupant], vec![]],
                vec![],
            ],
        };
        let mut w = ByteWriter::new();
        assert!(model.write(&mut w));
        let bytes = w.into_bytes();
        let mut r = ByteReader::new(&bytes);
        let loaded = OccupantManagerRecord::read(&mut r).unwrap();
        assert_eq!(r.remaining(), 0);
        assert_eq!(loaded.size.map(f32::to_bits), model.size.map(f32::to_bits));
        assert_eq!(loaded.counts, model.counts);
        assert_eq!(loaded.rows, model.rows);
        for end in 0..bytes.len() {
            assert!(OccupantManagerRecord::read(&mut ByteReader::new(&bytes[..end])).is_none());
        }
        model.rows[0][0].reverse();
        model.rows[0][0][0].id = 73;
        let mut edited = ByteWriter::new();
        assert!(model.write(&mut edited));
        assert_ne!(edited.bytes(), bytes);
    }
}
