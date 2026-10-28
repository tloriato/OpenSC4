// SPDX-License-Identifier: GPL-3.0-or-later
use fc4_core::save::{SaveRecord, RECORD_SIZE_V4};
use fc4_core::store::archive::{read_class, write_class, ClassIndex, StoredObject};
use fc4_core::store::{INDEX_TYPE, STORE_GROUP};
use fc4_formats::city_save::{first_difference, CitySave, Report};
use fc4_formats::dbpf::Compression;
use fc4_formats::{Package, PackageBuilder, Tgi};
use std::path::PathBuf;

fn fixture() -> Vec<u8> {
    let mut simulator = vec![0; RECORD_SIZE_V4];
    simulator[0] = 4;
    let payload = SaveRecord::decode(&simulator).unwrap().encode();
    let mut builder = PackageBuilder::new();
    let index = ClassIndex {
        entries_written: 2,
        classes: vec![(0x2990_C1E5, 1), (0xDEAD_BEEF, 1)],
    };
    builder
        .add(
            Tgi::new(INDEX_TYPE, STORE_GROUP, 0),
            &index.write(),
            Compression::Always,
        )
        .unwrap();
    for (class, id, payload) in [(0x2990_C1E5, 57, payload), (0xDEAD_BEEF, 99, vec![12, 13, 14])] {
        let bytes = write_class(&[StoredObject { id, payload }]).unwrap();
        builder
            .add(Tgi::new(class, STORE_GROUP, 0), &bytes, Compression::Always)
            .unwrap();
    }
    builder.build().unwrap()
}

#[test]
fn codecs_are_counted_separately_from_unknown_payloads_and_ids_survive() {
    let source = Package::from_bytes(fixture()).unwrap();
    let model = CitySave::read(&source).unwrap();
    let output = model.write().unwrap();
    let report = model.audit(&output).unwrap();
    assert!(report.differences.is_empty());
    assert_eq!(report.types[&0x2990_C1E5].identical, 1);
    assert_eq!(report.types[&0xDEAD_BEEF].identical, 0);
    assert_eq!(report.types[&0xDEAD_BEEF].not_handled, 1);
    let written = Package::from_bytes(&output).unwrap();
    let objects = read_class(&written.read_tgi(&Tgi::new(0x2990_C1E5, STORE_GROUP, 0)).unwrap()).unwrap();
    assert_eq!(objects[0].id, 57);
    let second = CitySave::read(&written).unwrap().write().unwrap();
    assert_eq!(output, second);
}

#[test]
fn lots_units_and_walls_preserve_cycles_configuration_ids_and_saved_masks() {
    use fc4_core::lot_records::{
        BusinessUnitRecord, Lot, LotRecord, OccupantRecord, RetainingWallRecord, CLSID_BUSINESS_UNIT, CLSID_LOT,
        CLSID_RETAINING_WALL_OCCUPANT,
    };
    use fc4_core::store::archive::references::{ReferenceWriter, SavedReferences};
    use fc4_core::stream::{ByteReader, RefValue, SaveObject};
    let lot_ref = RefValue::Host {
        token: 719,
        class_id: CLSID_LOT,
    };
    let lot = LotRecord {
        configuration_id: 0xDEAD_BEEF,
        business_unit: RefValue::Host {
            token: 37,
            class_id: CLSID_BUSINESS_UNIT,
        },
        travel_designate: lot_ref.clone(),
        altitude: f32::from_bits(0x7fc0_1234),
        slope: -0.0,
        ..LotRecord::default()
    };
    let unit = BusinessUnitRecord {
        lots: vec![lot_ref.clone(), RefValue::Null, lot_ref.clone()],
    };
    let mut wall = RetainingWallRecord::new();
    wall.edge_mask = 0x80;
    wall.edge_hashes = [31, 0, -17, 0];
    wall.lot = lot_ref;
    let mut lot_bytes = ReferenceWriter::default();
    assert!(lot.write(&mut lot_bytes));
    let lot_bytes = lot_bytes.into_bytes();
    let mut wall_bytes = ReferenceWriter::default();
    assert!(wall.write(&mut wall_bytes));
    let wall_bytes = wall_bytes.into_bytes();
    let mut unit_bytes = ReferenceWriter::default();
    assert!(unit.write(&mut unit_bytes));
    let mut builder = PackageBuilder::new();
    for (class, id, payload) in [
        (CLSID_LOT, 719, lot_bytes.clone()),
        (CLSID_BUSINESS_UNIT, 37, unit_bytes.into_bytes()),
        (CLSID_RETAINING_WALL_OCCUPANT, 83, wall_bytes.clone()),
    ] {
        let bytes = write_class(&[StoredObject { id, payload }]).unwrap();
        builder
            .add(Tgi::new(class, STORE_GROUP, 0), &bytes, Compression::Always)
            .unwrap();
    }
    let source = Package::from_bytes(builder.build().unwrap()).unwrap();
    let model = CitySave::read(&source).unwrap();
    let output = model.write().unwrap();
    let report = model.audit(&output).unwrap();
    assert!(report.differences.is_empty());
    for ty in [CLSID_LOT, CLSID_BUSINESS_UNIT, CLSID_RETAINING_WALL_OCCUPANT] {
        assert_eq!(report.types[&ty].identical, 1);
    }
    // The live load still performs its mask fixup; the archive must not.
    let mut refs = SavedReferences;
    let mut live_wall = RetainingWallRecord::new();
    assert!(live_wall.read(&mut ByteReader::with_resolver(&wall_bytes, &mut refs)));
    assert_eq!(live_wall.edge_mask, 0x85);
    // A live lot without Init still writes configuration 0. The record codec
    // preserves it, and writes later edits from its fields rather than a cache.
    let live_lot = Lot::constructed();
    assert!(live_lot.read(&mut ByteReader::with_resolver(&lot_bytes, &mut refs)));
    let mut live_bytes = ReferenceWriter::default();
    assert!(live_lot.write(&mut live_bytes));
    assert_eq!(&live_bytes.into_bytes()[2..6], &0u32.to_le_bytes());
    let mut decoded = LotRecord::default();
    assert!(decoded.read(&mut ByteReader::with_resolver(&lot_bytes, &mut refs)));
    decoded.configuration_id = 43;
    let mut changed = ReferenceWriter::default();
    assert!(decoded.write(&mut changed));
    assert_eq!(&changed.into_bytes()[2..6], &43u32.to_le_bytes());
}

#[test]
fn damage_and_first_differences_are_reported() {
    let mut record = write_class(&[StoredObject {
        id: 3,
        payload: vec![1, 2],
    }])
    .unwrap();
    record[12] ^= 1;
    assert!(read_class(&record).unwrap_err().contains("CRC"));
    for length in 1..12 {
        assert!(read_class(&record[..length]).is_err());
    }
    assert_eq!(first_difference(&[1, 2], &[1, 3]), Some(1));
    assert_eq!(first_difference(&[1], &[1, 2]), Some(1));
    assert_eq!(first_difference(&[1], &[1]), None);
    let source = Package::from_bytes(fixture()).unwrap();
    let model = CitySave::read(&source).unwrap();
    let mut changed = PackageBuilder::new();
    for entry in source.entries().iter().filter(|e| e.tgi != fc4_formats::dbpf::DIR_TGI) {
        let mut bytes = source.read(entry).unwrap();
        if entry.tgi.type_id == 0x2990_C1E5 {
            bytes[12] ^= 1;
        }
        changed.add(entry.tgi, &bytes, Compression::Always).unwrap();
    }
    let report = model.audit(&changed.build().unwrap()).unwrap();
    assert_eq!(report.types[&0x2990_C1E5].differs, 1);
    assert!(report.differences[0].contains("byte 12"));
}

#[test]
fn terrain_field_edges_and_class_references_roundtrip_independently() {
    use fc4_core::store::archive::ObjectReference;
    use fc4_core::stream::{ByteWriter, OutStream};
    use fc4_core::terrain::{SurfaceWaterRecord, Terrain, TerrainRecord, TerrainTuning, TerrainViewRecord};
    let terrain = Terrain::create(2, 2, TerrainTuning::default()).unwrap();
    let mut builder = PackageBuilder::new();
    for (key, bytes) in [
        (Tgi::new(0x8A91_E7E3, 0xE98F_9525, 3), terrain.write_misc()),
        (Tgi::new(0xA9DD_6FF4, 0xE98F_9525, 1), terrain.write_altitudes()),
        (Tgi::new(0x8A91_E7E0, 0xE98F_9525, 2), terrain.write_properties()),
        // These deliberately differ from the interior's edges, including raw float bits.
        (
            Tgi::new(0xA9DD_6FF4, 0xE98F_9525, 0x64),
            Terrain::write_edge_altitudes(&[f32::from_bits(0x8000_0000), f32::from_bits(0x7FC0_1234)]),
        ),
    ] {
        builder.add(key, &bytes, Compression::Always).unwrap();
    }
    let mut w = ByteWriter::new();
    TerrainRecord {
        view: ObjectReference {
            id: 44,
            class: 0xC9B8_4E10,
        },
    }
    .write(&mut w);
    let terrain_payload = w.into_bytes();
    let mut w = ByteWriter::new();
    TerrainViewRecord { value: 1 }.write(&mut w);
    let view_payload = w.into_bytes();
    let mut w = ByteWriter::new();
    SurfaceWaterRecord {
        terrain: ObjectReference {
            id: 59,
            class: 0xE98F_9525,
        },
    }
    .write(&mut w);
    let water_payload = w.into_bytes();
    for (class, id, payload) in [
        (0xE98F_9525, 59, terrain_payload),
        (0xC9B8_4E10, 44, view_payload),
        (0x0B8D_A7DB, 56, water_payload),
    ] {
        let bytes = write_class(&[StoredObject { id, payload }]).unwrap();
        builder
            .add(Tgi::new(class, STORE_GROUP, 0), &bytes, Compression::Always)
            .unwrap();
    }
    let bytes = builder.build().unwrap();
    let package = Package::from_bytes(bytes).unwrap();
    let model = CitySave::read(&package).unwrap();
    let report = model.audit(&model.write().unwrap()).unwrap();
    assert!(report.differences.is_empty(), "{:?}", report.differences);
    assert!(report.types.values().all(|s| s.not_handled == 0 && s.differs == 0));
    // Null references consume exactly the ID word, leaving the next field intact.
    let mut w = ByteWriter::new();
    ObjectReference { id: 0, class: 999 }.write(&mut w);
    w.put_u8(7);
    assert_eq!(w.into_bytes(), [0, 0, 0, 0, 7]);
}

#[test]
fn every_numeric_grid_uses_its_owner_codec_including_ieee_bits() {
    use fc4_core::grid::{CityDims, SimGrid};
    use fc4_core::stream::{ByteWriter, SaveObject};
    let mut builder = PackageBuilder::new();
    macro_rules! add_grid {
        ($ty:ty, $value:expr) => {{
            let grid = SimGrid::<$ty>::create(&CityDims::square(1), 73, 1);
            grid.data.borrow_mut().values[0] = $value;
            let mut w = ByteWriter::new();
            assert!(grid.write(&mut w));
            let bytes = write_class(&[StoredObject {
                id: grid.class_id() + 100,
                payload: w.into_bytes(),
            }])
            .unwrap();
            builder
                .add(Tgi::new(grid.class_id(), STORE_GROUP, 0), &bytes, Compression::Always)
                .unwrap();
        }};
    }
    add_grid!(u8, 255);
    add_grid!(i8, -127);
    add_grid!(u16, 65535);
    add_grid!(i16, -32767);
    add_grid!(u32, 0xCDCD_CDCD);
    add_grid!(i32, i32::MIN);
    add_grid!(u64, 0xBAAD_F00D_CCCC_CCCC);
    add_grid!(i64, i64::MIN);
    add_grid!(f32, f32::from_bits(0x7FC0_1234));
    add_grid!(f64, f64::from_bits(0x7FF8_1234_5678_ABCD));
    let package = Package::from_bytes(builder.build().unwrap()).unwrap();
    let model = CitySave::read(&package).unwrap();
    let report = model.audit(&model.write().unwrap()).unwrap();
    assert!(report.differences.is_empty(), "{:?}", report.differences);
    assert_eq!(report.types.len(), 11); // ten classes plus regenerated DIR
    assert!(report.types.values().all(|s| s.identical == 1));
}

fn golden_directory() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("FC4_GOLDEN_DIR") {
        let path = PathBuf::from(path);
        assert!(path.is_dir(), "FC4_GOLDEN_DIR does not exist");
        return Some(path);
    }
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .map(|path| path.join("build-runs/golden"))
        .find(|path| path.is_dir())
}

#[test]
fn golden_supported_owners_roundtrip_when_corpus_is_present() {
    let Some(directory) = golden_directory() else {
        eprintln!("golden corpus absent; set FC4_GOLDEN_DIR to enable the save sweep");
        return;
    };
    let mut paths: Vec<_> = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path().join("end.sc4"))
        .filter(|p| p.is_file())
        .collect();
    paths.sort();
    assert!(!paths.is_empty(), "golden directory contains no end.sc4 saves");
    let mut total = Report::default();
    for path in &paths {
        let package = Package::open(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let model = CitySave::read(&package).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let output = model.write().unwrap();
        let report = model.audit(&output).unwrap();
        assert!(
            report.differences.is_empty(),
            "{}: {:?}",
            path.display(),
            report.differences
        );
        total.merge(report);
    }
    let summary = format!("{} golden saves, 0 failed\n{}", paths.len(), total.table());
    if let Some(path) = std::env::var_os("FC4_ROUNDTRIP_REPORT") {
        std::fs::write(path, &summary).expect("write requested round-trip report");
    }
    eprintln!("{summary}");
    // Keep the measured milestone honest if a dispatch arm is accidentally
    // removed: a retained payload must not silently replace a tested codec.
    for ty in [
        0x0990_C38A,
        0xA990_BD10,
        0x2990_62C0,
        0x49DD_6F4C,
        0xEA3F_F4F5,
        0x0B7D_45E0,
        0xCA02_7EDB,
        0x2990_C1E5,
        0x499B_23FE,
        0x49B9_E602,
        0x49B9_E603,
        0x49B9_E604,
        0x49B9_E605,
        0x49B9_E606,
        0x49B9_E60A,
        0x8990_C080,
        0x89EF_A536,
        0x8A91_E7E0,
        0x8A91_E7E3,
        0xA7E7_F929,
        0xA990_C022,
        0xA9DD_6FF4,
        0xC9B8_4E10,
        0xC9C6_28EC,
        0xE86B_1EEF,
        0xE98F_9525,
        0xEAD4_BA36,
        0xA990_C08E,
        0x00D0_723D,
        0x02BF_1DFA,
        0x22F6_E80C,
        0x22F6_E81B,
        0x40D0_7236,
        0x42BF_1E18,
        0x62BF_1DAA,
        0x62BF_1DB9,
        0x62F6_E7CF,
        0x815B_4CEF,
        0x82B9_999B,
        0xA0D0_7129,
        0xA2BF_1DDC,
        0xA2BF_1DE5,
        0xA2BF_1E43,
        0xC2BF_1DC5,
        0xC2BF_1E04,
        0xC2F6_E81F,
        0xE0D0_722E,
        0xE0D0_7233,
        0x2977_AA47,
        0x2977_AA49,
        0xA9BC_9AB6,
        0xA9BD_882D,
        0xC97F_987C,
        0x49C0_5C8F,
        0x2990_C1BC,
        0xC9BD_5D4A,
        0x4A23_2DA8,
        0x49C0_5C9F,
        0xC9C0_5C6E,
        0x49C1_A034,
        0x49CC_1BCD,
        0x8A4B_D52B,
        0x49C0_5B9F,
        0xCA16_374F,
        0xC9C0_5C5D,
        0x098F_964D,
        0x6990_C1AA,
        0xA9C0_5C85,
        0x298F_9B2D,
        0x498F_9B01,
        0x0990_C005,
        0x0990_C075,
        0xC990_C034,
        0x4A1B_664E,
        0xCA14_CA03,
        0xA98F_9D3B,
        0xC98F_9D17,
        0xC98F_9D4A,
        0xA98F_9D28,
        0x4A16_28C5,
        0x6A16_28BA,
        0x896E_75AF,
        0x896E_714A,
        0x4A22_2B98,
        0x4A5B_69ED,
        0xE990_BE01,
        0xE990_BFFC,
        0xAA31_3C9F,
        0x2990_BFF2,
        0x8990_C12D,
        0x0990_BDD8,
        0xE990_C18E,
        0xE990_C045,
        0x8990_C065,
        0x8990_C09A,
        0x2990_C05A,
        0x6990_C04F,
        0xA9C7_41B3,
        0xA9C8_647C,
        0xE9C6_22D8,
        0x4A0D_D616,
        0xEAC9_37B4,
        0x6A5F_8755,
        0xAA52_92D7,
        0xCA2C_2A2F,
        0xEA19_E825,
        0x4A3A_D3E1,
        0x2A5F_877D,
        0x4BE3_72CD,
        0x8A09_F5F4,
        0xAA37_1C32,
        0x8BE3_753B,
        0x6A38_48DB,
        0x6A26_D049,
        0x6A93_35DE,
        0x6B70_EFAD,
        0x89C4_8F47,
        0x4A1D_BBBF,
        0x0BF6_06A5,
        0xCBC1_4674,
        0x0990_C138,
        0x2990_C142,
        0xC990_C0A4,
        0x0A13_4F24,
        0x2A13_4F42,
        0x4990_C013,
        0x0A41_3481,
        0x8990_BE7A,
        0x4990_BD93,
        0xEA09_D4AE,
        0x8A22_BEBE,
        0xA990_BFE0,
        0xAA3F_E6BC,
        0xAA20_49A1,
        0xEA23_0A28,
        0xC990_BD46,
        0xE9A6_CD3B,
        0x09C0_5C6A,
        0xEA0A_5ACF,
        0xCA15_ED6B,
    ] {
        // A partial corpus (references being re-recorded) may hold no record of a type.
        let Some(stats) = total.types.get(&ty) else { continue };
        assert_eq!(stats.identical, stats.count, "{ty:08X}: codec coverage regressed");
    }
}

#[test]
fn occupant_archive_keeps_parent_keys_frames_and_nonlive_effect_names() {
    use fc4_core::lot_records::{BaseTextureRecord, OccupantRecord, TextureRecord};
    use fc4_core::prop_occupant::{write_effect_record, write_prop_record, EffectRecord, EffectRecordHead, PropRecord};
    use fc4_core::stream::{ByteWriter, OutStream, ResKey};
    use fc4_core::variant::{Elem, Variant};
    use std::rc::Rc;

    let mut prop = PropRecord::default();
    prop.base.removal = 257;
    prop.base.flags = 0x85;
    prop.base.holder.parent = Some((
        ResKey {
            type_id: 0x6534_284A,
            group: 71,
            instance: 93,
        },
        Rc::new(fc4_core::property::PropertyHolder::new()),
    ));
    prop.base
        .holder
        .own
        .add(17, Variant::Scalar(Elem::F32(f32::from_bits(0x7fc0_1234))));
    prop.prop_type = 0xDEAD_BEEF;
    prop.bounds.0[1] = -0.0;
    prop.timer_bits = 15;
    let mut w = ByteWriter::new();
    assert!(write_prop_record(&prop, &mut w));
    let mut builder = PackageBuilder::new();
    let prop_bytes = write_class(&[StoredObject {
        id: 71,
        payload: w.into_bytes(),
    }])
    .unwrap();
    builder
        .add(Tgi::new(0x2977_AA47, STORE_GROUP, 0), &prop_bytes, Compression::Always)
        .unwrap();

    let effect = EffectRecord {
        version: 3,
        prop,
        head: EffectRecordHead { byte0: 1, byte1: 0 },
        name: b"welding_sparks".to_vec(),
        trailer: 0x1234_5678,
    };
    let mut w = ByteWriter::new();
    assert!(write_effect_record(&effect, false, &mut w));
    let effect_bytes = write_class(&[StoredObject {
        id: 72,
        payload: w.into_bytes(),
    }])
    .unwrap();
    builder
        .add(
            Tgi::new(0x2977_AA49, STORE_GROUP, 0),
            &effect_bytes,
            Compression::Always,
        )
        .unwrap();

    let mut texture = BaseTextureRecord::new();
    let mut w = ByteWriter::new();
    w.put_u32(0x1234_5678);
    w.put_bytes(&[1, 2, 3, 1, 0xFF, 0xB2, 0x33, 0xAA, 3, 0xFF]);
    let mut cell = TextureRecord::default();
    cell.read(&mut fc4_core::stream::ByteReader::new(w.bytes())).unwrap();
    texture.textures.push(cell);
    let mut w = ByteWriter::new();
    assert!(texture.write(&mut w));
    let texture_bytes = write_class(&[StoredObject {
        id: 73,
        payload: w.into_bytes(),
    }])
    .unwrap();
    builder
        .add(
            Tgi::new(0xC97F_987C, STORE_GROUP, 0),
            &texture_bytes,
            Compression::Always,
        )
        .unwrap();
    let source = Package::from_bytes(builder.build().unwrap()).unwrap();
    let model = CitySave::read(&source).unwrap();
    let report = model.audit(&model.write().unwrap()).unwrap();
    assert!(report.differences.is_empty());
    assert!(report.types.values().all(|s| s.not_handled == 0 && s.identical == 1));
}

#[test]
fn shipped_regions_reach_stable_second_write_when_present() {
    let install = std::env::var_os("FC4_GAME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Program Files (x86)\GOG Galaxy\Games\SimCity 4 Deluxe Edition"));
    let regions = install.join("Regions");
    if !regions.is_dir() {
        eprintln!("shipped Regions absent; set FC4_GAME_DIR to enable the legacy save sweep");
        return;
    }
    fn collect(path: &std::path::Path, files: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                collect(&path, files);
            } else if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("sc4")) {
                files.push(path);
            }
        }
    }
    let mut files = Vec::new();
    collect(&regions, &mut files);
    files.sort();
    assert!(!files.is_empty(), "shipped Regions contains no saves");
    for path in &files {
        let original = Package::open(path).unwrap();
        let model = CitySave::read(&original).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let first = model.write().unwrap();
        let package = Package::from_bytes(&first).unwrap();
        let loaded = CitySave::read(&package).unwrap();
        let second = loaded.write().unwrap();
        let report = loaded.audit(&second).unwrap();
        assert!(
            report.differences.is_empty(),
            "{}: {:?}",
            path.display(),
            report.differences
        );
        assert_eq!(first, second, "{}: second output is unstable", path.display());
    }
    eprintln!("{} shipped saves have a stable second write", files.len());
}
