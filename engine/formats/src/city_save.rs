// SPDX-License-Identifier: GPL-3.0-or-later
//! Standalone city-save audit. Unknown owner payloads are retained for incremental
//! work, but never counted as handled. IDs are preserved throughout the archive.

use crate::dbpf::{Compression, DIR_TGI};
use crate::{Package, PackageBuilder, Source, Tgi};
use fc4_core::advisors::archive::AdvisorArchive;
use fc4_core::application::archive::ApplicationRecord;
use fc4_core::aura::archive::AuraRecord;
use fc4_core::automata::archive::AutomataRecord;
use fc4_core::budget::archive::{BudgetRecord, DepartmentRecord, LineItemRecord};
use fc4_core::camera::{CameraControl, CAMERA_CONTROL_CLASS_ID};
use fc4_core::city_detail::DetailRecord;
use fc4_core::civic::archive::CivicRecord;
use fc4_core::clock24::{Clock24, CLSID_24_HOUR_CLOCK};
use fc4_core::commercial::CommercialRecord;
use fc4_core::commercial::IndustrialSim;
use fc4_core::crime::archive::CrimeRecord;
use fc4_core::demand::archive::{DemandNodeRecord, DemandRecord};
use fc4_core::demolition::archive::DemolitionRecord;
use fc4_core::development_simulator::archive::DevelopmentRecord;
use fc4_core::dispatch::archive::DispatchManagerRecord;
use fc4_core::flammability::archive::FlammabilityRecord;
use fc4_core::flora::archive::FloraSimulatorRecord;
use fc4_core::flora::occupant::FloraInstance;
use fc4_core::growth_developer::archive::GrowthRecord;
use fc4_core::history::{HistoryWarehouse, CLSID_HISTORY_WAREHOUSE};
use fc4_core::landfill::archive::{LandfillOccupantRecord, LandfillRecord};
use fc4_core::landvalue::archive::LandValueRecord;
use fc4_core::lot_config_manager::archive::LotConfigManagerRecord;
use fc4_core::lot_developer::archive::LotDeveloperRecord;
use fc4_core::lot_records::{
    BusinessUnitRecord, LotRecord, OccupantRecord, RetainingWallRecord, CLSID_BUSINESS_UNIT, CLSID_LOT,
    CLSID_RETAINING_WALL_OCCUPANT,
};
use fc4_core::model_maker::archive::ModelMakerRecord;
use fc4_core::mysims::archive::{dispatch::DispatchRecord, MySimRecord};
use fc4_core::neighbors::archive::NeighborsRecord;
use fc4_core::network_manager::archive::NetworkManagerArchive;
use fc4_core::network_occupants::archive::{ConnectionArchive, NetworkRecord, PoleArchive};
use fc4_core::occupant_manager::archive::OccupantManagerRecord;
use fc4_core::ordinance::record::{OrdinanceRecord, OrdinanceSimulatorRecord};
use fc4_core::ordinance::{OrdinanceKind, CLSID_ORDINANCE_SIMULATOR};
use fc4_core::parks::archive::ParksRecord;
use fc4_core::plumbing::archive::PlumbingRecord;
use fc4_core::pollution::archive::PollutionRecord;
use fc4_core::port_developer::archive::PortRecord;
use fc4_core::power::archive::PowerRecord;
use fc4_core::prop_developer::archive::PropDeveloperRecord;
use fc4_core::prop_manager::archive::PropManagerRecord;
use fc4_core::prop_occupant::{
    read_effect_record, read_prop_record, write_effect_record, write_prop_record, EffectRecord, PropRecord, ReadOutcome,
};
use fc4_core::protection::archive::ProtectionRecord;
use fc4_core::regional_city::archive::RegionalCityRecord;
use fc4_core::render3d::archive::RendererRecord;
use fc4_core::residential::archive::ResidentialRecord;
use fc4_core::save::SaveRecord;
use fc4_core::store::archive::references::{ReferenceWriter, SavedReferences};
use fc4_core::store::archive::{read_class, write_class, ClassIndex, StoredObject};
use fc4_core::store::{INDEX_TYPE, STORE_GROUP};
use fc4_core::stream::{ByteReader, SaveObject};
use fc4_core::terrain::{SurfaceWaterRecord, Terrain, TerrainRecord, TerrainViewRecord};
use fc4_core::tract_developer::archive::TractRecord;
use fc4_core::traffic_simulator::archive::TrafficRecord;
use fc4_core::ui_windows::view3d::archive::View3DArchive;
use fc4_core::weather::{WeatherRng, WeatherSim};
use fc4_core::zones::archive::ZoneRecord;
use std::collections::BTreeMap;
use std::rc::Rc;

mod grids;
mod hosts;
mod terrain;

enum Payload {
    Demolition(DemolitionRecord),
    PropManager(PropManagerRecord),
    LotConfigManager(LotConfigManagerRecord),
    NetworkManager(NetworkManagerArchive),
    Pole(Box<PoleArchive>),
    Connection(Box<ConnectionArchive>),
    Civic(CivicRecord),
    Development(DevelopmentRecord),
    DispatchManager(DispatchManagerRecord),
    Landfill(LandfillRecord),
    LandfillOccupant(LandfillOccupantRecord),
    LotDeveloper(LotDeveloperRecord),
    Parks(ParksRecord),
    Port(PortRecord),
    Residential(Box<ResidentialRecord>),
    Growth(GrowthRecord),
    Tract(TractRecord),
    Neighbors(Box<NeighborsRecord>),
    MySim(Box<MySimRecord>),
    Dispatch(Box<DispatchRecord>),
    Advisor(Box<AdvisorArchive>),
    PropDeveloper(PropDeveloperRecord),
    ModelMaker(ModelMakerRecord),
    Renderer(RendererRecord),
    Protection(Box<ProtectionRecord>),
    Crime(CrimeRecord),
    Pollution(Box<PollutionRecord>),
    Aura(AuraRecord),
    LandValue(LandValueRecord),
    Flammability(FlammabilityRecord),
    Budget(Box<BudgetRecord>),
    Department(DepartmentRecord),
    LineItem(LineItemRecord),
    Demand(DemandRecord),
    DemandNode(DemandNodeRecord),
    Simulator(SaveRecord),
    History(Box<HistoryWarehouse>),
    Clock(Box<Clock24>),
    Camera(Box<CameraControl>),
    Serializable(Rc<dyn SaveObject>),
    Terrain(TerrainRecord),
    TerrainView(TerrainViewRecord),
    SurfaceWater(SurfaceWaterRecord),
    Commercial(CommercialRecord),
    Detail(DetailRecord),
    Ordinance(OrdinanceRecord),
    OrdinanceSimulator(OrdinanceSimulatorRecord),
    Prop(Box<PropRecord>),
    EffectProp(Box<EffectRecord>),
    Lot(Box<LotRecord>),
    BusinessUnit(BusinessUnitRecord),
    RetainingWall(Box<RetainingWallRecord>),
    Network(Box<NetworkRecord>),
    OccupantManager(OccupantManagerRecord),
    Traffic(Box<TrafficRecord>),
    Flora(Box<FloraInstance>),
    FloraSimulator(FloraSimulatorRecord),
    Power(Box<PowerRecord>),
    Plumbing(Box<PlumbingRecord>),
    Zone(ZoneRecord),
    Automata(AutomataRecord),
    Unhandled(Vec<u8>),
}

impl Payload {
    fn read(class: u32, bytes: &[u8]) -> Result<Self, String> {
        let mut references = SavedReferences;
        let mut r = ByteReader::with_resolver(bytes, &mut references);
        let (record, ok) = match class {
            0x0990_C38A => (
                Self::Demolition(DemolitionRecord::read(&mut r).ok_or("demolition Read failed")?),
                true,
            ),
            0xA990_BD10 => (
                Self::PropManager(PropManagerRecord::read(&mut r).ok_or("prop manager Read failed")?),
                true,
            ),
            0x2990_62C0 => (
                Self::LotConfigManager(
                    LotConfigManagerRecord::read(&mut r).ok_or("lot configuration manager Read failed")?,
                ),
                true,
            ),
            0xC990_BD46 | 0xE9A6_CD3B => (
                Self::NetworkManager(NetworkManagerArchive::read(class, &mut r).ok_or("network manager Read failed")?),
                true,
            ),
            0x09C0_5C6A => (
                Self::Pole(Box::new(PoleArchive::read(&mut r).ok_or("pole Read failed")?)),
                true,
            ),
            0xEA0A_5ACF | 0xCA15_ED6B => (
                Self::Connection(Box::new(
                    ConnectionArchive::read(&mut r).ok_or("connection Read failed")?,
                )),
                true,
            ),
            0x0A41_3481 => (Self::Civic(CivicRecord::read(&mut r).ok_or("civic Read failed")?), true),
            0x8990_BE7A => (
                Self::Development(DevelopmentRecord::read(&mut r).ok_or("development Read failed")?),
                true,
            ),
            0x4990_BD93 => (
                Self::DispatchManager(DispatchManagerRecord::read(&mut r).ok_or("dispatch manager Read failed")?),
                true,
            ),
            0xEA09_D4AE => (
                Self::Landfill(LandfillRecord::read(&mut r).ok_or("landfill Read failed")?),
                true,
            ),
            0x8A22_BEBE => (
                Self::LandfillOccupant(LandfillOccupantRecord::read(&mut r).ok_or("landfill occupant Read failed")?),
                true,
            ),
            0xA990_BFE0 => (
                Self::LotDeveloper(LotDeveloperRecord::read(&mut r).ok_or("lot developer Read failed")?),
                true,
            ),
            0xAA3F_E6BC => (Self::Parks(ParksRecord::read(&mut r).ok_or("parks Read failed")?), true),
            0xAA20_49A1 | 0xEA23_0A28 => (Self::Port(PortRecord::read(&mut r).ok_or("port Read failed")?), true),
            0x4990_C013 => (
                Self::Residential(Box::new(
                    ResidentialRecord::read(&mut r).ok_or("residential Read failed")?,
                )),
                true,
            ),
            0x0990_C138 => (
                Self::Growth(GrowthRecord::read(&mut r).ok_or("growth Read failed")?),
                true,
            ),
            0x2990_C142 => (Self::Tract(TractRecord::read(&mut r).ok_or("tract Read failed")?), true),
            0xC990_C0A4 | 0x0A13_4F24 | 0x2A13_4F42 => (
                Self::Neighbors(Box::new(
                    NeighborsRecord::read(class, &mut r).ok_or("neighbors Read failed")?,
                )),
                true,
            ),
            0x4A1D_BBBF | 0x0BF6_06A5 => (
                Self::MySim(Box::new(MySimRecord::read(&mut r).ok_or("MySim Read failed")?)),
                true,
            ),
            0xCBC1_4674 => (
                Self::Dispatch(Box::new(DispatchRecord::read(&mut r).ok_or("dispatch Read failed")?)),
                true,
            ),
            _ if AdvisorArchive::handles(class) => (
                Self::Advisor(Box::new(
                    AdvisorArchive::read(class, &mut r).ok_or("advisor Read failed")?,
                )),
                true,
            ),
            0x89C4_8F47 => (
                Self::PropDeveloper(PropDeveloperRecord::read(&mut r).ok_or("prop developer Read failed")?),
                true,
            ),
            0xA9C7_41B3 => (
                Self::ModelMaker(ModelMakerRecord::read(&mut r).ok_or("model maker Read failed")?),
                true,
            ),
            0xE9C6_22D8 => (
                Self::Renderer(RendererRecord::read(&mut r).ok_or("renderer Read failed")?),
                true,
            ),
            0x2990_C05A | 0x6990_C04F => (
                Self::Protection(Box::new(
                    ProtectionRecord::read(class, &mut r).ok_or("protection Read failed")?,
                )),
                true,
            ),
            0x8990_C09A => (Self::Crime(CrimeRecord::read(&mut r).ok_or("crime Read failed")?), true),
            0x8990_C065 => (
                Self::Pollution(Box::new(PollutionRecord::read(&mut r).ok_or("pollution Read failed")?)),
                true,
            ),
            0x0990_BDD8 => (Self::Aura(AuraRecord::read(&mut r).ok_or("aura Read failed")?), true),
            0xE990_C18E => (
                Self::LandValue(LandValueRecord::read(&mut r).ok_or("land value Read failed")?),
                true,
            ),
            0xE990_C045 => (
                Self::Flammability(FlammabilityRecord::read(&mut r).ok_or("flammability Read failed")?),
                true,
            ),
            0xE990_BE01 => (
                Self::Budget(Box::new(BudgetRecord::read(&mut r).ok_or("budget Read failed")?)),
                true,
            ),
            0xE990_BFFC => (
                Self::Department(DepartmentRecord::read(&mut r).ok_or("department Read failed")?),
                true,
            ),
            0xAA31_3C9F => (
                Self::LineItem(LineItemRecord::read(&mut r).ok_or("line item Read failed")?),
                true,
            ),
            0x2990_BFF2 => (
                Self::Demand(DemandRecord::read(&mut r).ok_or("demand Read failed")?),
                true,
            ),
            0x8990_C12D => (
                Self::DemandNode(DemandNodeRecord::read(&mut r).ok_or("demand node Read failed")?),
                true,
            ),
            0x0990_C005 => (
                Self::Power(Box::new(PowerRecord::read(&mut r).ok_or("power Read failed")?)),
                true,
            ),
            0x0990_C075 => (
                Self::Plumbing(Box::new(PlumbingRecord::read(&mut r).ok_or("plumbing Read failed")?)),
                true,
            ),
            0xC990_C034 => (
                Self::FloraSimulator(FloraSimulatorRecord::read(&mut r).ok_or("flora simulator Read failed")?),
                true,
            ),
            0xA9C0_5C85 => {
                let mut record = Box::new(FloraInstance::default());
                let ok = record.read(&mut r);
                (Self::Flora(record), ok)
            }
            _ if ZoneRecord::handles(class) => (
                Self::Zone(ZoneRecord::read(class, &mut r).ok_or("zones Read failed")?),
                true,
            ),
            0x6990_C1AA => (
                Self::Traffic(Box::new(TrafficRecord::read(&mut r).ok_or("traffic Read failed")?)),
                true,
            ),
            0x098F_964D => (
                Self::OccupantManager(OccupantManagerRecord::read(&mut r).ok_or("occupant manager Read failed")?),
                true,
            ),
            0x2990_C1E5 => {
                return SaveRecord::decode(bytes)
                    .map(Self::Simulator)
                    .map_err(|e| e.to_string())
            }
            CLSID_HISTORY_WAREHOUSE => {
                let mut record = Box::new(HistoryWarehouse::new(Box::new(hosts::Offline)));
                let ok = record.read(&mut r);
                (Self::History(record), ok)
            }
            CLSID_24_HOUR_CLOCK => {
                let mut record = Box::new(Clock24::new(Box::new(hosts::Offline)));
                let ok = record.read(&mut r);
                (Self::Clock(record), ok)
            }
            CAMERA_CONTROL_CLASS_ID => {
                let mut record = Box::new(CameraControl::new());
                let ok = record.read(&mut r);
                (Self::Camera(record), ok)
            }
            0xA990_C022 => {
                let record = IndustrialSim::new(0);
                let ok = record.read(&mut r);
                (Self::Serializable(record), ok)
            }
            0xE98F_9525 => (
                Self::Terrain(TerrainRecord::read(&mut r).ok_or("terrain class Read failed")?),
                true,
            ),
            0xC9B8_4E10 => (
                Self::TerrainView(TerrainViewRecord::read(&mut r).ok_or("terrain view Read failed")?),
                true,
            ),
            0x0B8D_A7DB => (
                Self::SurfaceWater(SurfaceWaterRecord::read(&mut r).ok_or("water Read failed")?),
                true,
            ),
            0x8990_C080 => (
                Self::Commercial(CommercialRecord::read(&mut r).ok_or("commercial Read failed")?),
                true,
            ),
            0xEAD4_BA36 => (
                Self::Detail(DetailRecord::read(&mut r).ok_or("city detail Read failed")?),
                true,
            ),
            CLSID_ORDINANCE_SIMULATOR => (
                Self::OrdinanceSimulator(
                    OrdinanceSimulatorRecord::read(&mut r).ok_or("ordinance simulator Read failed")?,
                ),
                true,
            ),
            _ if OrdinanceKind::of_class(class).is_some() => (
                Self::Ordinance(OrdinanceRecord::read(&mut r).ok_or("ordinance Read failed")?),
                true,
            ),
            0x2977_AA47 => {
                let ReadOutcome::Ok(record) = read_prop_record(&mut r) else {
                    return Err("prop Read failed".into());
                };
                (Self::Prop(Box::new(record)), true)
            }
            0x2977_AA49 => (
                Self::EffectProp(Box::new(read_effect_record(&mut r).ok_or("effect prop Read failed")?)),
                true,
            ),
            0xA9BC_9AB6 | 0xA9BD_882D | 0xC97F_987C | 0x49C0_5C8F => {
                let Some(fc4_core::stream::RefValue::Object(record)) = fc4_core::lot_records::create(class) else {
                    return Err("occupant factory failed".into());
                };
                let ok = record.read(&mut r);
                (Self::Serializable(record), ok)
            }
            0x2990_C1BC => {
                let record = WeatherSim::new(Rc::new(hosts::Offline), 0, Box::new(WeatherRng::from_session(0)));
                let ok = record.read(&mut r);
                (Self::Serializable(record), ok)
            }
            CLSID_LOT => {
                let mut record = Box::new(LotRecord::default());
                let ok = record.read(&mut r);
                (Self::Lot(record), ok)
            }
            CLSID_BUSINESS_UNIT => {
                let mut record = BusinessUnitRecord::default();
                let ok = record.read(&mut r);
                (Self::BusinessUnit(record), ok)
            }
            CLSID_RETAINING_WALL_OCCUPANT => {
                let mut record = Box::new(RetainingWallRecord::new());
                let ok = record.read_saved(&mut r);
                (Self::RetainingWall(record), ok)
            }
            _ if NetworkRecord::handles(class) => (
                Self::Network(Box::new(
                    NetworkRecord::read(class, &mut r).ok_or("network Read failed")?,
                )),
                true,
            ),
            // SPEC-GAP: traffic network map persistence awaits SR33-2; opaque bytes are not codec coverage.
            0x6A0F_82B2 => return Ok(Self::Unhandled(bytes.to_vec())),
            _ => match grids::create(class) {
                Some(record) => {
                    let ok = record.read(&mut r);
                    (Self::Serializable(record), ok)
                }
                None => match AutomataRecord::create(class) {
                    Some(mut record) => {
                        let ok = record.read(&mut r);
                        (Self::Automata(record), ok)
                    }
                    None => return Ok(Self::Unhandled(bytes.to_vec())),
                },
            },
        };
        if !ok || r.remaining() != 0 {
            return Err(format!(
                "class {class:08X}: Read failed or left {} bytes",
                r.remaining()
            ));
        }
        Ok(record)
    }

    fn write(&self) -> Result<Vec<u8>, String> {
        let mut w = ReferenceWriter::default();
        let ok = match self {
            Self::Demolition(record) => record.write(&mut w),
            Self::PropManager(record) => record.write(&mut w),
            Self::LotConfigManager(record) => record.write(&mut w),
            Self::NetworkManager(record) => record.write(&mut w),
            Self::Pole(record) => record.write(&mut w),
            Self::Connection(record) => record.write(&mut w),
            Self::Civic(record) => record.write(&mut w),
            Self::Development(record) => record.write(&mut w),
            Self::DispatchManager(record) => record.write(&mut w),
            Self::Landfill(record) => record.write(&mut w),
            Self::LandfillOccupant(record) => record.write(&mut w),
            Self::LotDeveloper(record) => record.write(&mut w),
            Self::Parks(record) => record.write(&mut w),
            Self::Port(record) => record.write(&mut w),
            Self::Residential(record) => record.write(&mut w),
            Self::Growth(record) => record.write(&mut w),
            Self::Tract(record) => record.write(&mut w),
            Self::Neighbors(record) => record.write(&mut w),
            Self::MySim(record) => record.write(&mut w),
            Self::Dispatch(record) => record.write(&mut w),
            Self::Advisor(record) => record.write(&mut w),
            Self::PropDeveloper(record) => record.write(&mut w),
            Self::ModelMaker(record) => record.write(&mut w),
            Self::Renderer(record) => record.write(&mut w),
            Self::Protection(record) => record.write(&mut w),
            Self::Crime(record) => record.write(&mut w),
            Self::Pollution(record) => record.write(&mut w),
            Self::Aura(record) => record.write(&mut w),
            Self::LandValue(record) => record.write(&mut w),
            Self::Flammability(record) => record.write(&mut w),
            Self::Budget(record) => record.write(&mut w),
            Self::Department(record) => record.write(&mut w),
            Self::LineItem(record) => record.write(&mut w),
            Self::Demand(record) => record.write(&mut w),
            Self::DemandNode(record) => record.write(&mut w),
            Self::Simulator(record) => return Ok(record.encode()),
            Self::Unhandled(bytes) => return Ok(bytes.clone()),
            Self::History(record) => record.write(&mut w),
            Self::Clock(record) => record.write(&mut w),
            Self::Camera(record) => record.write(&mut w),
            Self::Serializable(record) => record.write(&mut w),
            Self::Terrain(record) => record.write(&mut w),
            Self::TerrainView(record) => record.write(&mut w),
            Self::SurfaceWater(record) => record.write(&mut w),
            Self::Commercial(record) => record.write(&mut w),
            Self::Detail(record) => record.write(&mut w),
            Self::Ordinance(record) => record.write(&mut w),
            Self::OrdinanceSimulator(record) => record.write(&mut w),
            Self::Prop(record) => write_prop_record(record, &mut w),
            Self::EffectProp(record) => write_effect_record(record, false, &mut w),
            Self::Lot(record) => record.write(&mut w),
            Self::BusinessUnit(record) => record.write(&mut w),
            Self::RetainingWall(record) => record.write(&mut w),
            Self::Network(record) => record.write(&mut w),
            Self::OccupantManager(record) => record.write(&mut w),
            Self::Traffic(record) => record.write(&mut w),
            Self::Flora(record) => record.write(&mut w),
            Self::FloraSimulator(record) => record.write(&mut w),
            Self::Power(record) => record.write(&mut w),
            Self::Plumbing(record) => record.write(&mut w),
            Self::Zone(record) => record.write(&mut w),
            Self::Automata(record) => record.write(&mut w),
        };
        if !ok {
            return Err("owner Write failed".into());
        }
        Ok(w.into_bytes())
    }

    fn handled(&self) -> bool {
        !matches!(self, Self::Unhandled(_))
    }
}

enum RecordData {
    Application(ApplicationRecord),
    RegionalCity(Box<RegionalCityRecord>),
    View3D(View3DArchive),
    Index(ClassIndex),
    Class(Vec<(u32, Payload)>),
    Terrain(Rc<Terrain>, terrain::Part),
    Edge(Vec<f32>),
    Unhandled(Vec<u8>),
}

struct Record {
    key: Tgi,
    compressed: bool,
    original: Vec<u8>,
    stored: Vec<u8>,
    data: RecordData,
}

impl Record {
    fn write(&self) -> Result<Vec<u8>, String> {
        match &self.data {
            RecordData::Application(record) => {
                let mut w = ReferenceWriter::default();
                if !record.write(&mut w) {
                    return Err("application Write failed".into());
                }
                Ok(w.into_bytes())
            }
            RecordData::RegionalCity(record) => {
                let mut w = ReferenceWriter::default();
                if !record.write(&mut w) {
                    return Err("regional city Write failed".into());
                }
                Ok(w.into_bytes())
            }
            RecordData::View3D(record) => {
                let mut w = ReferenceWriter::default();
                if !record.write(&mut w) {
                    return Err("View3D Write failed".into());
                }
                Ok(w.into_bytes())
            }
            RecordData::Index(index) => Ok(index.write()),
            RecordData::Class(objects) => write_class(
                &objects
                    .iter()
                    .map(|(id, p)| {
                        Ok(StoredObject {
                            id: *id,
                            payload: p.write()?,
                        })
                    })
                    .collect::<Result<Vec<_>, String>>()?,
            ),
            RecordData::Unhandled(bytes) => Ok(bytes.clone()),
            RecordData::Terrain(terrain, part) => Ok(part.write(terrain)),
            RecordData::Edge(values) => Ok(Terrain::write_edge_altitudes(values)),
        }
    }

    fn handled(&self) -> bool {
        match &self.data {
            RecordData::Application(_) => true,
            RecordData::RegionalCity(_) => true,
            RecordData::Index(_) | RecordData::View3D(_) => true,
            RecordData::Terrain(..) | RecordData::Edge(_) => true,
            RecordData::Class(objects) => objects.iter().all(|(_, p)| p.handled()),
            RecordData::Unhandled(_) => false,
        }
    }
}

/// Parsed package, class framing and owner models. No source file is modified.
pub struct CitySave {
    records: Vec<Record>,
    created: u32,
    modified: u32,
    directory: Option<Vec<u8>>,
}

impl CitySave {
    pub fn read<S: Source>(package: &Package<S>) -> Result<Self, String> {
        let mut records = Vec::new();
        let terrain = terrain::load(package)?;
        let mut directory = None;
        let mut keys = std::collections::BTreeSet::new();
        for entry in package.entries() {
            if !keys.insert(entry.tgi) {
                return Err(format!("duplicate record {}", entry.tgi));
            }
            if entry.tgi == DIR_TGI {
                directory = Some(package.read(entry).map_err(|e| e.to_string())?);
                continue; // PackageBuilder regenerates compression metadata.
            }
            let original = package.read(entry).map_err(|e| e.to_string())?;
            let key = entry.tgi;
            let data = if key == Tgi::new(INDEX_TYPE, STORE_GROUP, 0) {
                RecordData::Index(ClassIndex::read(&original)?)
            } else if key.group == STORE_GROUP && key.instance == 0 {
                let objects = read_class(&original)?;
                RecordData::Class(
                    objects
                        .into_iter()
                        .map(|o| {
                            let payload = Payload::read(key.type_id, &o.payload)
                                .map_err(|error| format!("class {:08X} object {}: {error}", key.type_id, o.id))?;
                            Ok((o.id, payload))
                        })
                        .collect::<Result<_, String>>()?,
                )
            } else if [
                Tgi::new(0x49DD_6F4C, 0x49DD_6F4E, 1),
                Tgi::new(0xEA3F_F4F5, 0xEA3F_F506, 0xEA3F_F4F5),
                Tgi::new(0x0B7D_45E0, 0x0B7D_45E1, 0x0B7D_45E2),
            ]
            .contains(&key)
            {
                let mut r = ByteReader::new(&original);
                let record = ApplicationRecord::read(key.type_id, &mut r).ok_or("application Read failed")?;
                if r.remaining() != 0 {
                    return Err("trailing application bytes".into());
                }
                RecordData::Application(record)
            } else if key == Tgi::new(0xCA02_7EDB, 0xCA02_7EE1, 0) {
                let mut r = ByteReader::new(&original);
                let record = RegionalCityRecord::read(&mut r).ok_or("regional city Read failed")?;
                if r.remaining() != 0 {
                    return Err("trailing regional city bytes".into());
                }
                RecordData::RegionalCity(Box::new(record))
            } else if let Some(part) = terrain::Part::from_key(key) {
                RecordData::Terrain(terrain.clone().ok_or("missing terrain")?, part)
            } else if terrain::is_edge(key) {
                RecordData::Edge(Terrain::read_edge(&original).map_err(|e| format!("terrain edge Read: {e:?}"))?)
            } else if key == Tgi::new(0xA9C8_647C, 0xE9C8_647E, 1) {
                let mut r = ByteReader::new(&original);
                let record = View3DArchive::read(&mut r).ok_or("View3D Read failed")?;
                if r.remaining() != 0 {
                    return Err("trailing View3D bytes".into());
                }
                RecordData::View3D(record)
            } else {
                RecordData::Unhandled(original.clone())
            };
            records.push(Record {
                key,
                compressed: package.is_compressed(entry),
                original,
                stored: package.read_raw(entry).map_err(|e| e.to_string())?,
                data,
            });
        }
        Ok(Self {
            records,
            created: package.header().created,
            modified: package.header().modified,
            directory,
        })
    }

    pub fn write(&self) -> Result<Vec<u8>, String> {
        let mut builder = PackageBuilder::new();
        builder.created = self.created;
        builder.modified = self.modified;
        for record in &self.records {
            let written = record.write()?;
            // Compression is a container cache only: every owner has already
            // encoded its model, and equality is checked before reusing it.
            if written == record.original {
                let size = record.compressed.then_some(written.len() as u32);
                builder.add_stored(record.key, record.stored.clone(), size);
                continue;
            }
            let compression = if record.compressed {
                Compression::Always
            } else {
                Compression::None
            };
            builder
                .add(record.key, &written, compression)
                .map_err(|e| e.to_string())?;
        }
        builder.build().map_err(|e| e.to_string())
    }

    /// Compare actual output after reopening and decompressing the new package.
    pub fn audit(&self, written: &[u8]) -> Result<Report, String> {
        let package = Package::from_bytes(written).map_err(|e| e.to_string())?;
        let mut report = Report::default();
        let expected = self.records.len() + usize::from(self.directory.is_some());
        if package.entries().len() != expected {
            return Err("output record count changed".into());
        }
        for record in &self.records {
            let bytes = package.read_tgi(&record.key).map_err(|e| e.to_string())?;
            let offset = first_difference(&record.original, &bytes);
            let outcome = if let Some(offset) = offset {
                report.differences.push(format!(
                    "{} differs at byte {offset} ({} -> {} bytes)",
                    record.key,
                    record.original.len(),
                    bytes.len()
                ));
                Outcome::Differs
            } else if record.handled() {
                Outcome::Identical
            } else {
                Outcome::NotHandled
            };
            let stats = report.types.entry(record.key.type_id).or_default();
            stats.count += 1;
            stats.bytes += record.original.len() as u64;
            match outcome {
                Outcome::Identical => {
                    stats.identical += 1;
                    stats.exact_bytes += record.original.len() as u64;
                }
                Outcome::Differs => stats.differs += 1,
                Outcome::NotHandled => stats.not_handled += 1,
            }
        }
        if let Some(original) = &self.directory {
            let bytes = package.read_tgi(&DIR_TGI).map_err(|e| e.to_string())?;
            let stats = report.types.entry(DIR_TGI.type_id).or_default();
            stats.count += 1;
            stats.bytes += original.len() as u64;
            if let Some(offset) = first_difference(original, &bytes) {
                stats.differs += 1;
                report.differences.push(format!("{DIR_TGI} differs at byte {offset}"));
            } else {
                stats.identical += 1;
                stats.exact_bytes += original.len() as u64;
            }
        }
        Ok(report)
    }
}

pub fn first_difference(a: &[u8], b: &[u8]) -> Option<usize> {
    a.iter()
        .zip(b)
        .position(|(a, b)| a != b)
        .or_else(|| (a.len() != b.len()).then_some(a.len().min(b.len())))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Identical,
    Differs,
    NotHandled,
}

#[derive(Default, Debug)]
pub struct TypeStats {
    pub count: u64,
    pub bytes: u64,
    pub identical: u64,
    pub exact_bytes: u64,
    pub differs: u64,
    pub not_handled: u64,
}

#[derive(Default, Debug)]
pub struct Report {
    pub types: BTreeMap<u32, TypeStats>,
    pub differences: Vec<String>,
}

impl Report {
    pub fn merge(&mut self, other: Self) {
        for (ty, s) in other.types {
            let t = self.types.entry(ty).or_default();
            t.count += s.count;
            t.bytes += s.bytes;
            t.identical += s.identical;
            t.exact_bytes += s.exact_bytes;
            t.differs += s.differs;
            t.not_handled += s.not_handled;
        }
        self.differences.extend(other.differences);
    }

    pub fn table(&self) -> String {
        let mut out = String::from(
            "| Record type | Count | Bytes | Identical | Differs | Not handled |\n|---|---:|---:|---:|---:|---:|\n",
        );
        for (ty, s) in &self.types {
            out.push_str(&format!(
                "| {ty:08X} | {} | {} | {} | {} | {} |\n",
                s.count, s.bytes, s.identical, s.differs, s.not_handled
            ));
        }
        let exact_types = self.types.values().filter(|s| s.identical == s.count).count();
        let bytes: u64 = self.types.values().map(|s| s.bytes).sum();
        let exact_bytes: u64 = self.types.values().map(|s| s.exact_bytes).sum();
        let opaque: u64 = self.types.values().map(|s| s.not_handled).sum();
        out.push_str(&format!(
            "\nFully handled types: {exact_types}/{}; decoded identical bytes: {exact_bytes}/{bytes}; \
             not handled records: {opaque}. Retained payloads do not count as codec coverage.\n",
            self.types.len()
        ));
        out
    }
}
