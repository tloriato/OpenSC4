// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use crate::stream::ByteWriter;

fn pole(version: u16, height: f32) -> PoleArchive {
    PoleArchive {
        version,
        base: OccupantBase::default(),
        transform: Xform {
            mask: 0x83,
            translation: [-0.0, f32::from_bits(0x7fc0_1234), -19.0],
            scale: 1.0,
            ..Xform::default()
        },
        legacy_counter: (version == 4).then_some(0x1234_5678),
        position: [-0.0, height, 31.0],
        connections: vec![SavedPoleConnection {
            pole: ObjectReference {
                id: 71,
                class: 0x09C0_5C6A,
            },
            lines: vec![
                ObjectReference::default(),
                ObjectReference {
                    id: 93,
                    class: 0xC9C0_5C5D,
                },
            ],
            flags: [0x80, 1],
            words: [-17, i32::MIN],
            points: [vec![vec![], vec![[height, -0.0, 17.0]]], vec![]],
        }],
        exemplar_index: 7,
        model_ids: [0xDEAD_BEEF, 0],
        cell: [-1, 31],
        attachments: [[height, 1.0, -0.0]; 4],
        geometry: [
            vec![Vertex {
                pos: [height, 17.0, -0.0],
                color: 0x1234_5678,
                u: 1.0,
                v: 2.0,
            }],
            vec![],
            vec![],
        ],
        model_y: height,
        ignored: 0x80,
        bytes_54_55: [0x81, 0x7f],
        scale: -0.0,
        distressed: 0x83,
    }
}

fn encode(record: &PoleArchive) -> Vec<u8> {
    let mut w = ByteWriter::new();
    assert!(record.write(&mut w));
    w.into_bytes()
}

#[test]
fn attachments_remain_per_object_and_raw_masks_and_legacy_counter_survive() {
    for version in [4, 5] {
        let first = encode(&pole(version, 71.0));
        let second = encode(&pole(version, 93.0));
        let mut r = ByteReader::new(&first);
        let mut a = PoleArchive::read(&mut r).unwrap();
        assert_eq!(r.remaining(), 0);
        let mut r = ByteReader::new(&second);
        let b = PoleArchive::read(&mut r).unwrap();
        assert_eq!(r.remaining(), 0);
        assert_eq!(a.attachments[0][0], 71.0);
        assert_eq!(b.attachments[0][0], 93.0);
        assert_eq!(a.connections[0].points[0][0].len(), 0);
        assert_eq!(a.connections[0].words, [-17, i32::MIN]);
        assert_eq!(a.connections[0].lines[1].id, 93);
        assert_eq!(a.transform.translation[1].to_bits(), 0x7fc0_1234);
        assert_eq!(a.geometry[0][0].color, 0x1234_5678);
        assert_eq!(encode(&a), first);
        assert_eq!(encode(&b), second);
        a.attachments[0][0] = 17.0;
        assert_ne!(encode(&a), first);
        assert_eq!(b.attachments[0][0], 93.0);
        for end in 0..first.len() {
            assert!(
                PoleArchive::read(&mut ByteReader::new(&first[..end])).is_none(),
                "prefix {end}"
            );
        }
    }
}
