// SPDX-License-Identifier: GPL-3.0-or-later
//! Archive fields for MySim/My Street (mysims §6.2; save-records §7.3).
//! Keep historical fields and identities; do not create dispatch tools, update
//! Lua state or apply the live advisor/advice load transitions.

use crate::advisors::archive::AdvisorRecord;
use crate::store::archive::{fields::array, ObjectReference};
use crate::stream::{ByteReader, InStream, OutStream};

pub mod dispatch;
use dispatch::DispatchRecord;

#[derive(Clone, Debug)]
pub enum SavedDispatch {
    Inline(Box<DispatchRecord>),
    Reference(ObjectReference),
}

#[derive(Clone, Debug, Default)]
pub struct ActiveMySim {
    pub slot: u8,
    pub residence: ObjectReference,
    pub home_cell: [u32; 2],
    pub job: ObjectReference,
    pub commute: [i32; 4],
    pub jobless_days: u32,
    pub vehicle: [u32; 3],
    pub wealth: u8,
    pub generation: u16,
    pub zodiac: u8,
    pub male: u8,
    pub bio: Vec<u8>,
    pub age: u8,
    pub birth_month: u8,
    pub portrait_id: u32,
    pub portrait_path: Vec<u8>,
    pub last_commute: u32,
    pub retired: u8,
    pub last_disaster: u32,
    pub home_position: [f32; 3],
    pub eq: SavedRing,
    pub hq: SavedRing,
    pub legacy: Option<[i32; 5]>,
    pub models: Option<[[u32; 3]; 2]>,
    pub reserved: Option<[i32; 2]>,
    pub dispatch: Option<SavedDispatch>,
    pub timer: Option<SavedTimer>,
    pub pylon_locked: Option<u8>,
    pub last_player_trip_day: Option<i32>,
}

#[derive(Clone, Debug, Default)]
pub struct SavedRing {
    pub index: u8,
    pub delta: f32,
    pub values: [f32; 6],
}

impl SavedRing {
    fn read(r: &mut ByteReader<'_>) -> Option<Self> {
        Some(Self {
            index: r.get_u8()?,
            delta: r.get_f32()?,
            values: array(r, |r| r.get_f32())?,
        })
    }

    fn write(&self, w: &mut dyn OutStream) {
        w.put_u8(self.index);
        w.put_f32(self.delta);
        for v in self.values {
            w.put_f32(v);
        }
    }
}

#[derive(Clone, Debug)]
pub struct SavedTimer {
    pub units: u8,
    pub elapsed: u64,
    pub running: u8,
    pub has_tool: u8,
    pub tool: Option<ObjectReference>,
}

#[derive(Clone, Debug)]
pub struct MySimRecord {
    pub version: u16,
    pub state: u8,
    pub active: Option<Box<ActiveMySim>>,
    pub advisor: AdvisorRecord,
}

impl MySimRecord {
    pub fn read(r: &mut ByteReader<'_>) -> Option<Self> {
        let version = r.get_u16()?;
        let state = r.get_u8()?;
        let active = if state != 0 {
            Some(Box::new(ActiveMySim::read(version, r)?))
        } else {
            None
        };
        let advisor = AdvisorRecord::read(r)?;
        Some(Self {
            version,
            state,
            active,
            advisor,
        })
    }

    pub fn write(&self, w: &mut dyn OutStream) -> bool {
        w.put_u16(self.version);
        w.put_u8(self.state);
        if let Some(active) = &self.active {
            active.write(w);
        }
        self.advisor.write(w)
    }
}

impl ActiveMySim {
    fn read(version: u16, r: &mut ByteReader<'_>) -> Option<Self> {
        let slot = r.get_u8()?;
        let residence = ObjectReference::read(r)?;
        let home_cell = array(r, |r| r.get_u32())?;
        let job = ObjectReference::read(r)?;
        let commute = array(r, |r| r.get_i32())?;
        let jobless_days = r.get_u32()?;
        let vehicle = array(r, |r| r.get_u32())?;
        let wealth = r.get_u8()?;
        let generation = r.get_u16()?;
        let zodiac = r.get_u8()?;
        let male = r.get_u8()?;
        let bio = r.get_str()?;
        let age = r.get_u8()?;
        let birth_month = r.get_u8()?;
        let portrait_id = r.get_u32()?;
        let portrait_path = r.get_str()?;
        let last_commute = r.get_u32()?;
        let retired = r.get_u8()?;
        let last_disaster = r.get_u32()?;
        let home_position = array(r, |r| r.get_f32())?;
        let eq = SavedRing::read(r)?;
        let hq = SavedRing::read(r)?;
        let legacy = if (11..15).contains(&version) {
            Some(array(r, |r| r.get_i32())?)
        } else {
            None
        };
        let models = if version >= 15 {
            Some([array(r, |r| r.get_u32())?, array(r, |r| r.get_u32())?])
        } else {
            None
        };
        let reserved = if version >= 17 {
            Some(array(r, |r| r.get_i32())?)
        } else {
            None
        };
        let dispatch = match version {
            0..=11 => None,
            12 => Some(SavedDispatch::Inline(Box::new(DispatchRecord::read(r)?))),
            _ => Some(SavedDispatch::Reference(ObjectReference::read(r)?)),
        };
        let timer = if version >= 12 {
            let units = r.get_u8()?;
            let elapsed = r.get_u64()?;
            let running = r.get_u8()?;
            let has_tool = r.get_u8()?;
            let tool = if has_tool != 0 {
                Some(ObjectReference::read(r)?)
            } else {
                None
            };
            Some(SavedTimer {
                units,
                elapsed,
                running,
                has_tool,
                tool,
            })
        } else {
            None
        };
        let pylon_locked = if version >= 14 { Some(r.get_u8()?) } else { None };
        let last_player_trip_day = if version >= 16 { Some(r.get_i32()?) } else { None };
        Some(Self {
            slot,
            residence,
            home_cell,
            job,
            commute,
            jobless_days,
            vehicle,
            wealth,
            generation,
            zodiac,
            male,
            bio,
            age,
            birth_month,
            portrait_id,
            portrait_path,
            last_commute,
            retired,
            last_disaster,
            home_position,
            eq,
            hq,
            legacy,
            models,
            reserved,
            dispatch,
            timer,
            pylon_locked,
            last_player_trip_day,
        })
    }

    fn write(&self, w: &mut dyn OutStream) {
        w.put_u8(self.slot);
        self.residence.write(w);
        for v in self.home_cell {
            w.put_u32(v);
        }
        self.job.write(w);
        for v in self.commute {
            w.put_i32(v);
        }
        w.put_u32(self.jobless_days);
        for v in self.vehicle {
            w.put_u32(v);
        }
        w.put_u8(self.wealth);
        w.put_u16(self.generation);
        w.put_u8(self.zodiac);
        w.put_u8(self.male);
        w.put_str(&self.bio);
        w.put_u8(self.age);
        w.put_u8(self.birth_month);
        w.put_u32(self.portrait_id);
        w.put_str(&self.portrait_path);
        w.put_u32(self.last_commute);
        w.put_u8(self.retired);
        w.put_u32(self.last_disaster);
        for v in self.home_position {
            w.put_f32(v);
        }
        self.eq.write(w);
        self.hq.write(w);
        if let Some(values) = self.legacy {
            for v in values {
                w.put_i32(v);
            }
        }
        if let Some(models) = self.models {
            for key in models {
                for v in key {
                    w.put_u32(v);
                }
            }
        }
        if let Some(values) = self.reserved {
            for v in values {
                w.put_i32(v);
            }
        }
        if let Some(dispatch) = &self.dispatch {
            match dispatch {
                SavedDispatch::Inline(record) => {
                    record.write(w);
                }
                SavedDispatch::Reference(reference) => {
                    reference.write(w);
                }
            }
        }
        if let Some(timer) = &self.timer {
            w.put_u8(timer.units);
            w.put_u64(timer.elapsed);
            w.put_u8(timer.running);
            w.put_u8(timer.has_tool);
            if let Some(tool) = timer.tool {
                tool.write(w);
            }
        }
        if let Some(v) = self.pylon_locked {
            w.put_u8(v);
        }
        if let Some(v) = self.last_player_trip_day {
            w.put_i32(v);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::ByteWriter;
    use dispatch::LocationRecord;

    fn dispatch() -> DispatchRecord {
        let occupant = ObjectReference {
            id: 71,
            class: 0xA9BD_882D,
        };
        let location = LocationRecord {
            occupant,
            world: [f32::from_bits(0x7FC0_1234), -0.0],
            cell: [-1, 17],
            in_city: 1,
            duration: -7,
            ..LocationRecord::default()
        };
        DispatchRecord {
            version: 1,
            auxiliary_flag: 0xBA,
            words: [u32::MAX; 18],
            locations: [location; 4],
            auxiliary_occupant: occupant,
            auxiliary_value: 0xDEAD_BEEF,
            auxiliary_string: b"auxiliary".to_vec(),
            automata: ObjectReference::default(),
            owner: ObjectReference {
                id: 37,
                class: 0x4A1D_BBBF,
            },
            service: ObjectReference::default(),
            route: vec![[1, 2, 0xFE], [3, 4, 1]],
            models: vec![(-1, [1, 2, 3], -17)],
            marker: -1,
            destinations: vec![location, location],
        }
    }

    #[test]
    fn dispatch_keeps_location_offset_full_width_state_and_reference_cycle() {
        let mut w = ByteWriter::new();
        assert!(dispatch().write(&mut w));
        let bytes = w.into_bytes();
        assert_eq!(&bytes[0..5], &[1, 0, 0, 0, 0xBA]);
        assert_eq!(&bytes[77..81], &71u32.to_le_bytes());
        let mut r = ByteReader::new(&bytes);
        let mut record = DispatchRecord::read(&mut r).unwrap();
        assert_eq!(r.remaining(), 0);
        assert_eq!(record.owner.id, 37);
        assert_eq!(record.locations[0].world[0].to_bits(), 0x7FC0_1234);
        let mut w = ByteWriter::new();
        assert!(record.write(&mut w));
        assert_eq!(w.into_bytes(), bytes);
        record.words[17] = 0;
        let mut w = ByteWriter::new();
        assert!(record.write(&mut w));
        let mut expected = bytes.clone();
        expected[73..77].fill(0);
        assert_eq!(w.into_bytes(), expected);
        for end in 0..bytes.len() {
            assert!(DispatchRecord::read(&mut ByteReader::new(&bytes[..end])).is_none());
        }
    }

    #[test]
    fn mysim_archive_keeps_inactive_and_all_historical_field_gates() {
        for version in [10, 11, 12, 13, 14, 15, 16, 17] {
            for state in [0, 1] {
                let source = MySimRecord {
                    version,
                    state,
                    active: (state != 0).then(|| {
                        Box::new(ActiveMySim {
                            legacy: (11..15).contains(&version).then_some([-17; 5]),
                            models: (version >= 15).then_some([[1, 2, 3], [4, 5, 6]]),
                            reserved: (version >= 17).then_some([-1, 29]),
                            dispatch: match version {
                                0..=11 => None,
                                12 => Some(SavedDispatch::Inline(Box::new(dispatch()))),
                                _ => Some(SavedDispatch::Reference(ObjectReference {
                                    id: 57,
                                    class: 0xCBC1_4674,
                                })),
                            },
                            timer: (version >= 12).then_some(SavedTimer {
                                units: 3,
                                elapsed: u64::MAX,
                                running: 1,
                                has_tool: 1,
                                tool: Some(ObjectReference::default()),
                            }),
                            pylon_locked: (version >= 14).then_some(1),
                            last_player_trip_day: (version >= 16).then_some(-1),
                            ..ActiveMySim::default()
                        })
                    }),
                    advisor: AdvisorRecord {
                        name: b"saved name".to_vec(),
                        cursor: -1,
                        ..AdvisorRecord::default()
                    },
                };
                let mut w = ByteWriter::new();
                assert!(source.write(&mut w));
                let bytes = w.into_bytes();
                let mut r = ByteReader::new(&bytes);
                let loaded = MySimRecord::read(&mut r).unwrap();
                assert_eq!(r.remaining(), 0);
                assert_eq!(loaded.advisor.name, b"saved name");
                assert_eq!(loaded.active.is_some(), state != 0);
                let mut w = ByteWriter::new();
                assert!(loaded.write(&mut w));
                assert_eq!(w.into_bytes(), bytes);
                for end in 0..bytes.len() {
                    assert!(MySimRecord::read(&mut ByteReader::new(&bytes[..end])).is_none());
                }
            }
        }
    }
}
