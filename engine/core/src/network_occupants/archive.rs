// SPDX-License-Identifier: GPL-3.0-or-later
//! The saved network family before live load fixups, network-occupants §§2–5.
//! Layouts and encoders stay with their owners; this dispatch selects the
//! appropriate envelope without a city, model services, or initialization.

use super::power_line::{read_power_line_record, write_power_line_record, PowerLineRecord};
use super::record::{read_saved_network_record, write_network_record, NetworkBaseRecord, PrebuiltRecord, TunnelRecord};
use super::underground::{
    read_saved_underground_record, write_pipe_record, write_subway_record, write_underground_record, UndergroundRecord,
    PIPE_RECORD_VERSION, SUBWAY_RECORD_VERSION,
};
use crate::stream::{InStream, OutStream, RefValue};

mod utility;
pub use utility::{ConnectionArchive, PoleArchive};

pub enum NetworkRecord {
    Base(NetworkBaseRecord),
    Prebuilt(PrebuiltRecord),
    Tunnel(TunnelRecord),
    Underground(UndergroundRecord),
    Pipe(UndergroundRecord, u32),
    Subway(UndergroundRecord),
    PowerLine(PowerLineRecord),
}

impl NetworkRecord {
    pub fn handles(class: u32) -> bool {
        matches!(
            class,
            0xC9C0_5C6E
                | 0x49C1_A034
                | 0x49CC_1BCD
                | 0x8A4B_D52B
                | 0x4A10_AFC2
                | 0x49C0_5B9F
                | 0xCA16_374F
                | 0xC9C0_5C5D
        )
    }

    pub fn read(class: u32, input: &mut dyn InStream) -> Option<Self> {
        Some(match class {
            0xC9C0_5C6E => Self::Base(read_saved_network_record(input)?),
            0x49C1_A034 | 0x49CC_1BCD => {
                let mut record = PrebuiltRecord::default();
                if !record.read_saved_into(input) {
                    return None;
                }
                Self::Prebuilt(record)
            }
            0x8A4B_D52B => {
                let mut record = TunnelRecord {
                    prebuilt: PrebuiltRecord::default(),
                    pair: RefValue::Null,
                };
                if !record.read_saved_into(input) {
                    return None;
                }
                Self::Tunnel(record)
            }
            0x4A10_AFC2 => Self::Underground(read_saved_underground_record(input)?),
            0x49C0_5B9F => {
                if input.get_u16()? != PIPE_RECORD_VERSION {
                    return None;
                }
                Self::Pipe(read_saved_underground_record(input)?, input.get_u32()?)
            }
            0xCA16_374F => {
                if input.get_u16()? != SUBWAY_RECORD_VERSION {
                    return None;
                }
                Self::Subway(read_saved_underground_record(input)?)
            }
            0xC9C0_5C5D => Self::PowerLine(read_power_line_record(input)?),
            _ => return None,
        })
    }

    pub fn write(&self, output: &mut dyn OutStream) -> bool {
        match self {
            Self::Base(record) => write_network_record(record, output),
            Self::Prebuilt(record) => record.write(output),
            Self::Tunnel(record) => record.write(&record.pair, output),
            Self::Underground(record) => write_underground_record(record, output),
            Self::Pipe(record, word) => write_pipe_record(record, *word, output),
            Self::Subway(record) => write_subway_record(record, output),
            Self::PowerLine(record) => write_power_line_record(record, output),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::record::{read_network_record, Triangle, Vertex, SAVED_FLAGS_MASK};
    use super::super::underground::UndergroundTail;
    use super::*;
    use crate::store::archive::references::{ReferenceWriter, SavedReferences};
    use crate::stream::ByteReader;

    #[test]
    fn nested_envelopes_keep_saved_flags_colors_optional_data_and_pair_ids() {
        let vertex = Vertex {
            pos: [-0.0, f32::from_bits(0x7fc0_1234), 17.0],
            color: 0x3157_9BDF,
            u: -0.0,
            v: 1.0,
        };
        let base = NetworkBaseRecord {
            network_flags: 0xFFFF_FFFF,
            quad: [vertex; 4],
            triangles: vec![Triangle {
                word0: 37,
                verts: [vertex; 3],
            }],
            ..NetworkBaseRecord::default()
        };
        let prebuilt = PrebuiltRecord {
            base: base.clone(),
            saved_178: [1.0, -0.0, 3.0],
            gate_19c: 5,
            saved_1a0: [1.0, 2.0, -0.0, f32::from_bits(0x7fc0_5678)],
            ..PrebuiltRecord::default()
        };
        let underground = UndergroundRecord {
            base: base.clone(),
            tail: UndergroundTail {
                walls: std::array::from_fn(|_| vec![vertex]),
                bytes_248: [3, 5, 7],
                cell: [-17, 43],
                word_268: 0xDEAD_BEEF,
                ..UndergroundTail::default()
            },
        };
        let pair = RefValue::Host {
            token: 719,
            class_id: 0x8A4B_D52B,
        };
        let records = [
            (0xC9C0_5C6E, NetworkRecord::Base(base.clone())),
            (0x49C1_A034, NetworkRecord::Prebuilt(prebuilt.clone())),
            (0x49CC_1BCD, NetworkRecord::Prebuilt(prebuilt.clone())),
            (0x8A4B_D52B, NetworkRecord::Tunnel(TunnelRecord { prebuilt, pair })),
            (0x4A10_AFC2, NetworkRecord::Underground(underground.clone())),
            (0x49C0_5B9F, NetworkRecord::Pipe(underground.clone(), 0xDEAD_BEEF)),
            (0xCA16_374F, NetworkRecord::Subway(underground)),
            (
                0xC9C0_5C5D,
                NetworkRecord::PowerLine(PowerLineRecord {
                    version: 2,
                    base: base.base.clone(),
                    position_bits: [0x8000_0000, 0x7fc0_1234, 0xDEAD_BEEF],
                    cell: [-3, 5],
                    flag: true,
                    refs: [
                        RefValue::Host {
                            token: 997,
                            class_id: 0x09C0_5C6A,
                        },
                        RefValue::Null,
                    ],
                }),
            ),
        ];
        for (class, record) in records {
            let mut output = ReferenceWriter::default();
            assert!(record.write(&mut output));
            let bytes = output.into_bytes();
            let mut refs = SavedReferences;
            let mut input = ByteReader::with_resolver(&bytes, &mut refs);
            let loaded = NetworkRecord::read(class, &mut input).unwrap();
            assert_eq!(input.remaining(), 0);
            let mut written = ReferenceWriter::default();
            assert!(loaded.write(&mut written));
            assert_eq!(bytes, written.into_bytes(), "{class:08X}");
            // A truncated envelope must never be reported as a complete codec.
            for n in 0..bytes.len() {
                let mut input = ByteReader::with_resolver(&bytes[..n], &mut refs);
                assert!(NetworkRecord::read(class, &mut input).is_none(), "{class:08X} at {n}");
            }
        }
        let mut bytes = ReferenceWriter::default();
        assert!(write_network_record(&base, &mut bytes));
        let bytes = bytes.into_bytes();
        let live = read_network_record(&mut ByteReader::new(&bytes)).unwrap();
        assert_eq!(live.network_flags, SAVED_FLAGS_MASK);
        assert_eq!(live.triangles[0].verts[0].color, 0xFF57_9BDF);
        let mut saved = read_saved_network_record(&mut ByteReader::new(&bytes)).unwrap();
        saved.network_flags = 0x1234_5678;
        let mut changed = ReferenceWriter::default();
        assert!(write_network_record(&saved, &mut changed));
        let changed = changed.into_bytes();
        let saved = read_saved_network_record(&mut ByteReader::new(&changed)).unwrap();
        assert_eq!(saved.network_flags, 0x1234_5678);
    }
}
