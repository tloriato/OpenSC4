// SPDX-License-Identifier: GPL-3.0-or-later
//! Saved zone state (zones §10), without rebuilding the live master lot list
//! or replacing saved tuning during Init. References keep their saved IDs.

use crate::store::archive::{fields::array, ObjectReference};
use crate::stream::{ByteReader, InStream, OutStream};

#[derive(Clone, Debug, Default)]
pub struct ZoneManagerRecord {
    pub grid: ObjectReference,
    pub zone_counts: [u32; 16],
    pub undeveloped: [u32; 16],
    pub unnamed_per_type: [u32; 16],
    pub totals: [u32; 4],
    pub minimum_sizes: [i32; 16],
    pub maximum_sizes: [i32; 16],
    pub creation_costs: [i64; 16],
    pub destruction_costs: [i64; 16],
    pub textures: [u32; 16],
    pub drag_colors: [u32; 16],
    pub alpha: u8,
    pub count_maps: [Vec<ZoneResultCount>; 2],
    pub services: [ObjectReference; 5],
}

#[derive(Clone, Copy, Debug)]
pub struct ZoneResultCount {
    pub zone: u8,
    pub result: u8,
    pub count: u32,
}

/// Cell order is x outer, z inner. Empty and repeated lot references are
/// significant here; deduplication belongs to the live developer's Read.
#[derive(Clone, Debug, Default)]
pub struct ZoneDeveloperRecord {
    pub count_x: u32,
    pub count_z: u32,
    pub lots: Vec<ObjectReference>,
}

#[derive(Clone, Debug)]
pub enum ZoneRecord {
    Manager(Box<ZoneManagerRecord>),
    Developer(ZoneDeveloperRecord),
}

impl ZoneRecord {
    pub fn handles(class: u32) -> bool {
        matches!(class, 0x298F_9B2D | 0x498F_9B01)
    }

    pub fn read(class: u32, r: &mut ByteReader<'_>) -> Option<Self> {
        if r.get_u16()? != 1 {
            return None;
        }
        match class {
            0x298F_9B2D => {
                let mut value = ZoneManagerRecord {
                    grid: ObjectReference::read(r)?,
                    zone_counts: array(r, |r| r.get_u32())?,
                    undeveloped: array(r, |r| r.get_u32())?,
                    unnamed_per_type: array(r, |r| r.get_u32())?,
                    totals: array(r, |r| r.get_u32())?,
                    minimum_sizes: array(r, |r| r.get_i32())?,
                    maximum_sizes: array(r, |r| r.get_i32())?,
                    creation_costs: array(r, |r| r.get_i64())?,
                    destruction_costs: array(r, |r| r.get_i64())?,
                    textures: array(r, |r| r.get_u32())?,
                    drag_colors: array(r, |r| r.get_u32())?,
                    alpha: r.get_u8()?,
                    ..ZoneManagerRecord::default()
                };
                for map in &mut value.count_maps {
                    let count = r.get_u32()? as usize;
                    if count > r.remaining() / 6 {
                        return None;
                    }
                    for _ in 0..count {
                        map.push(ZoneResultCount {
                            zone: r.get_u8()?,
                            result: r.get_u8()?,
                            count: r.get_u32()?,
                        });
                    }
                }
                value.services = array(r, ObjectReference::read)?;
                Some(Self::Manager(Box::new(value)))
            }
            0x498F_9B01 => {
                let count_x = r.get_u32()?;
                let count_z = r.get_u32()?;
                let count = (count_x as usize).checked_mul(count_z as usize)?;
                if count > r.remaining() / 4 {
                    return None;
                }
                let mut lots = Vec::with_capacity(count);
                for _ in 0..count {
                    lots.push(ObjectReference::read(r)?);
                }
                Some(Self::Developer(ZoneDeveloperRecord { count_x, count_z, lots }))
            }
            _ => None,
        }
    }

    pub fn write(&self, w: &mut dyn OutStream) -> bool {
        w.put_u16(1);
        match self {
            Self::Manager(value) => {
                value.grid.write(w);
                for values in [
                    &value.zone_counts[..],
                    &value.undeveloped,
                    &value.unnamed_per_type,
                    &value.totals,
                ] {
                    for &v in values {
                        w.put_u32(v);
                    }
                }
                for values in [&value.minimum_sizes, &value.maximum_sizes] {
                    for &v in values {
                        w.put_i32(v);
                    }
                }
                for values in [&value.creation_costs, &value.destruction_costs] {
                    for &v in values {
                        w.put_i64(v);
                    }
                }
                for values in [&value.textures, &value.drag_colors] {
                    for &v in values {
                        w.put_u32(v);
                    }
                }
                w.put_u8(value.alpha);
                for map in &value.count_maps {
                    w.put_u32(map.len() as u32);
                    for entry in map {
                        w.put_u8(entry.zone);
                        w.put_u8(entry.result);
                        w.put_u32(entry.count);
                    }
                }
                for reference in value.services {
                    reference.write(w);
                }
            }
            Self::Developer(value) => {
                if (value.count_x as usize).checked_mul(value.count_z as usize) != Some(value.lots.len()) {
                    w.set_error();
                    return false;
                }
                w.put_u32(value.count_x);
                w.put_u32(value.count_z);
                for &lot in &value.lots {
                    lot.write(w);
                }
            }
        }
        w.error() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::ByteWriter;

    #[test]
    fn null_repeated_and_forward_cell_references_survive_in_cell_order() {
        // Hand-written wire data, including the same object at three cells.
        let words = [0u32, 19, 0xC9BD5D4A, 19, 0xC9BD5D4A, 71, 0xC9BD5D4A, 0, 19, 0xC9BD5D4A];
        let mut input = vec![1, 0];
        input.extend_from_slice(&2u32.to_le_bytes());
        input.extend_from_slice(&3u32.to_le_bytes());
        input.extend(words.iter().flat_map(|v| v.to_le_bytes()));
        let mut reader = ByteReader::new(&input);
        let value = ZoneRecord::read(0x498F_9B01, &mut reader).unwrap();
        assert_eq!(reader.remaining(), 0);
        let ZoneRecord::Developer(ref cells) = value else {
            panic!()
        };
        assert_eq!(
            cells.lots.iter().map(|r| r.id).collect::<Vec<_>>(),
            [0, 19, 19, 71, 0, 19]
        );
        let mut output = ByteWriter::new();
        assert!(value.write(&mut output));
        assert_eq!(output.bytes(), input);
        for len in 0..input.len() {
            assert!(ZoneRecord::read(0x498F_9B01, &mut ByteReader::new(&input[..len])).is_none());
        }
    }

    #[test]
    fn rejects_impossible_map_dimensions_without_allocating() {
        let mut bytes = vec![1, 0];
        bytes.extend_from_slice(&u32::MAX.to_le_bytes());
        bytes.extend_from_slice(&u32::MAX.to_le_bytes());
        assert!(ZoneRecord::read(0x498F_9B01, &mut ByteReader::new(&bytes)).is_none());
    }

    #[test]
    fn manager_model_keeps_saved_tuning_and_can_edit_a_field() {
        let mut wire = ByteWriter::new();
        wire.put_u16(1);
        wire.put_u32(19);
        wire.put_u32(0x49B9E602);
        for value in 0..52 {
            wire.put_u32(value);
        }
        for value in -32..0 {
            wire.put_i32(value);
        }
        for value in 0..32 {
            wire.put_i64(i64::MIN + value);
        }
        for value in 0..32 {
            wire.put_u32(0xFFFF0000 + value);
        }
        wire.put_u8(117);
        wire.put_u32(1);
        wire.put_u8(15);
        wire.put_u8(18);
        wire.put_u32(0x80000000);
        wire.put_u32(0);
        for id in [0, 71, 0, 71, u32::MAX] {
            wire.put_u32(id);
            if id != 0 {
                wire.put_u32(0x8990C372);
            }
        }
        let input = wire.into_bytes();
        let mut r = ByteReader::new(&input);
        let mut value = ZoneRecord::read(0x298F_9B2D, &mut r).unwrap();
        assert_eq!(r.remaining(), 0);
        let ZoneRecord::Manager(ref mut manager) = value else {
            panic!()
        };
        assert_eq!(manager.creation_costs[0], i64::MIN);
        assert_eq!(manager.count_maps[0][0].count, 0x80000000);
        manager.alpha = 3;
        let mut expected = input;
        expected[730] = 3;
        let mut w = ByteWriter::new();
        assert!(value.write(&mut w));
        assert_eq!(w.bytes(), expected);
    }
}
