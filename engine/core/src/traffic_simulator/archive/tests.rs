// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use crate::stream::ByteWriter;

fn fixture(v: u16) -> TrafficRecord {
    let dimensions = [
        if v >= 24 { 13 } else { 8 },
        1,
        if v >= 28 { 13 } else { 4 },
        if v >= 28 { 9 } else { 7 },
    ];
    let [a, b, c, d] = dimensions.map(|n| n as usize);
    let statistics = std::array::from_fn(|_| Statistics {
        rows: vec![StatRow::default(); if v >= 35 { a } else { c }],
        bridges: vec![StatRow::default(); if v >= 37 { b } else { 0 }],
        records: vec![65535; a],
        bridge_records: vec![237; if v >= 37 { b } else { 0 }],
        switches: vec![SwitchStat::default(); if v >= 34 { 12 } else { 3 }],
        ..Statistics::default()
    });
    let reference = ObjectReference {
        id: 997,
        class: 0xC97F_987C,
    };
    let density = if v >= 20 {
        Density::Current {
            last: [65535; 16],
            usage: 33553,
            current: [32768; 16],
        }
    } else {
        Density::Legacy([f32::from_bits(0x7F80_1234); 34])
    };
    let switches = vec![Switch {
        id: 423,
        values: SwitchValues {
            building: reference,
            load: -0.0,
            capacity: f32::from_bits(0x7FC0_1234),
            time_factor: 1.0,
        },
        legacy_route: if v < 29 { vec![0, 17, 255] } else { vec![] },
        rules: if v >= 29 {
            vec![Rule {
                kind: 1,
                sides: [1, 0, 1, 0],
                from: 256,
                to: 65535,
            }]
        } else {
            vec![]
        },
        toll: f32::from_bits(0x7F80_4321),
        intersection: 1,
        ..Switch::default()
    }];
    TrafficRecord {
        version: v,
        dimensions,
        published: [reference; 6],
        destination_size: [2, 3],
        destinations: vec![255, 4, 3, 2, 1, 0],
        volumes: vec![reference, ObjectReference::default(), reference],
        float_grids: [reference; 2],
        densities: vec![(731, density)],
        switch_grids: [reference; 2],
        next_switch_id: 79,
        switches,
        departments: [reference, ObjectReference::default()],
        income_accumulator: f64::from_bits(0x7FF0_0000_0000_4321),
        legacy_income_accumulator: f32::from_bits(0x7F80_4321),
        road: Damage {
            cells: 7,
            damaged: 2,
            rate: -0.0,
            intact_keys: vec![0xFFFF, 17, 0x0080],
            damaged_keys: vec![55, 0x8000],
        },
        rail: Damage::default(),
        scan: Scan {
            state: 157,
            step: 3,
            cell: [-1, i32::MIN],
            rectangle: [1, 2, 3, 4],
        },
        phase_positions: [255, 128],
        context: Context {
            flags: [5, 1],
            value: f32::from_bits(0x7F80_1234),
            ..Context::default()
        },
        context_table: std::array::from_fn(|_| vec![0x7654_3210; d]),
        statistics,
        tunnels: vec![(
            TunnelHeader {
                key: 0xFFFF,
                other_cell: [255, 0],
                mask: 0x8001,
                network: 0xFF80,
            },
            vec![-0.0; d],
        )],
        cells_per_network: vec![331; a],
        free_destinations: vec![u32::MAX, 17, 18, 19],
        pools: vec![(37, PropertyHolder::new()), (37, PropertyHolder::new())],
        travel_counts: if v >= 27 { vec![[99, 32768]; d] } else { vec![] },
        ferry_total: 7,
        ferries: if v >= 30 {
            vec![Ferry {
                legacy_word: 333,
                header: FerryHeader {
                    cells: [1, 2, 3, 4, 5, 6],
                    ferry_type: 1,
                    mask: 0xF0F0,
                    riders: [17, 65535],
                },
                lot_cell: [13, 14],
                terminals: vec![TerminalLink {
                    header: TerminalLinkHeader {
                        cost: f32::from_bits(0x7F80_1234),
                        arrival: [-2, 7],
                        departure: 0xF0F0,
                    },
                    route: vec![255, 37, 16],
                    counters: [1, 3, 2, 4],
                }],
                neighbors: vec![NeighborLink {
                    header: NeighborLinkHeader {
                        cost: -0.0,
                        cells: [-1, 0, 1, 2],
                    },
                    route_out: vec![5, 0, 17],
                    route_back: if v >= 31 { vec![37, 99] } else { vec![] },
                    counters: [5, 7, 6, 8],
                }],
            }]
        } else {
            vec![]
        },
        ..TrafficRecord::default()
    }
}

#[test]
fn every_version_gate_preserves_saved_values_and_nested_container_order() {
    for v in 19..=42 {
        let model = fixture(v);
        let mut w = ByteWriter::new();
        assert!(model.write(&mut w));
        let bytes = w.into_bytes();
        let mut r = ByteReader::new(&bytes);
        let loaded = TrafficRecord::read(&mut r).unwrap_or_else(|| panic!("v{v}, offset {}", r.position()));
        assert_eq!(r.remaining(), 0, "v{v}");
        assert_eq!(loaded.dimensions, model.dimensions);
        assert_eq!(loaded.volumes, model.volumes);
        assert_eq!(loaded.switches[0].rules.len(), usize::from(v >= 29));
        assert_eq!(loaded.tunnels[0].0.network, 0xFF80);
        assert_eq!(loaded.road.intact_keys, vec![0xFFFF, 17, 0x0080]);
        assert_eq!(loaded.pools.iter().map(|(key, _)| *key).collect::<Vec<_>>(), [37, 37]);
        let mut rewritten = ByteWriter::new();
        assert!(loaded.write(&mut rewritten));
        assert_eq!(rewritten.bytes(), bytes, "v{v}");
    }
}

#[test]
fn decoded_reference_and_map_edits_reach_the_wire_and_truncations_fail() {
    let mut w = ByteWriter::new();
    assert!(fixture(41).write(&mut w));
    let bytes = w.into_bytes();
    // Independent offsets from save-records §7.1.1: u16 + four s32, then refs.
    assert_eq!(&bytes[18..26], &[0xE5, 3, 0, 0, 0x7C, 0x98, 0x7F, 0xC9]);
    for end in 0..bytes.len() {
        assert!(
            TrafficRecord::read(&mut ByteReader::new(&bytes[..end])).is_none(),
            "prefix {end}"
        );
    }
    let mut loaded = TrafficRecord::read(&mut ByteReader::new(&bytes)).unwrap();
    loaded.published[0].id = 71;
    loaded.destinations[0] = 64;
    loaded.switches[0].rules[0].to = 7;
    let mut edited = ByteWriter::new();
    assert!(loaded.write(&mut edited));
    assert_eq!(&edited.bytes()[18..22], &71u32.to_le_bytes());
    let changed = TrafficRecord::read(&mut ByteReader::new(edited.bytes())).unwrap();
    assert_eq!(changed.destinations[0], 64);
    assert_eq!(changed.switches[0].rules[0].to, 7);
    assert_eq!(changed.volumes, loaded.volumes);
    let mut impossible = bytes.clone();
    impossible[2..6].copy_from_slice(&i32::MAX.to_le_bytes());
    assert!(TrafficRecord::read(&mut ByteReader::new(&impossible)).is_none());
}
