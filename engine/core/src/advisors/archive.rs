// SPDX-License-Identifier: GPL-3.0-or-later
//! Saved advisor fields (advisors §11; save-records §7.2). These are archive
//! models, not live Read: no dirty-bit transition, Lua setup or active-map rebuild.

use super::{clsid, AdvisorKind};
use crate::store::archive::{fields::count, ObjectReference};
use crate::stream::{ByteReader, InStream, OutStream};

#[derive(Clone, Debug)]
pub struct AdviceEntry {
    pub advice: ObjectReference,
    pub expiry: i32,
    pub last_month: i32,
    pub next_day: i32,
    pub active: u8,
    pub event: i32,
}

#[derive(Clone, Debug, Default)]
pub struct AdvisorRecord {
    pub table_index: i32,
    pub caption: Vec<u8>,
    pub name: Vec<u8>,
    pub month_counter: i32,
    pub current_day: i32,
    pub advisor_id: i32,
    pub cursor: i32,
    pub advice: Vec<AdviceEntry>,
    pub events: Vec<(i32, ObjectReference)>,
}

impl AdvisorRecord {
    pub fn read(r: &mut ByteReader<'_>) -> Option<Self> {
        if r.get_u16()? != 3 {
            return None;
        }
        let table_index = r.get_i32()?;
        let caption = r.get_str()?;
        let name = r.get_str()?;
        let month_counter = r.get_i32()?;
        let current_day = r.get_i32()?;
        let advisor_id = r.get_i32()?;
        let cursor = r.get_i32()?;
        let n = count(r, 21)?;
        let mut advice = Vec::with_capacity(n);
        for _ in 0..n {
            advice.push(AdviceEntry {
                advice: ObjectReference::read(r)?,
                expiry: r.get_i32()?,
                last_month: r.get_i32()?,
                next_day: r.get_i32()?,
                active: r.get_u8()?,
                event: r.get_i32()?,
            });
        }
        let n = count(r, 8)?;
        let mut events = Vec::with_capacity(n);
        for _ in 0..n {
            events.push((r.get_i32()?, ObjectReference::read(r)?));
        }
        Some(Self {
            table_index,
            caption,
            name,
            month_counter,
            current_day,
            advisor_id,
            cursor,
            advice,
            events,
        })
    }

    pub fn write(&self, w: &mut dyn OutStream) -> bool {
        w.put_u16(3);
        w.put_i32(self.table_index);
        w.put_str(&self.caption);
        w.put_str(&self.name);
        for v in [self.month_counter, self.current_day, self.advisor_id, self.cursor] {
            w.put_i32(v);
        }
        w.put_u32(self.advice.len() as u32);
        for entry in &self.advice {
            entry.advice.write(w);
            w.put_i32(entry.expiry);
            w.put_i32(entry.last_month);
            w.put_i32(entry.next_day);
            w.put_u8(entry.active);
            w.put_i32(entry.event);
        }
        w.put_u32(self.events.len() as u32);
        for (event, subject) in &self.events {
            w.put_i32(*event);
            subject.write(w);
        }
        w.error() == 0
    }
}

#[derive(Clone, Debug)]
pub struct AdviceRecord {
    pub version: u16,
    pub table_index: i32,
    pub guid: u32,
    pub flags: u32,
    pub advisor_id: i32,
    pub priority: i32,
    pub frequency: i32,
    pub timeout: i32,
    pub mood: i32,
    pub effects: Vec<u32>,
    pub subject: ObjectReference,
    pub deal_type: Option<i32>,
}

impl AdviceRecord {
    pub fn read(deal: bool, r: &mut ByteReader<'_>) -> Option<Self> {
        if deal && r.get_u16()? != 2 {
            return None;
        }
        let version = r.get_u16()?;
        let table_index = r.get_i32()?;
        let guid = r.get_u32()?;
        let flags = r.get_u32()?;
        let advisor_id = r.get_i32()?;
        let priority = r.get_i32()?;
        let frequency = r.get_i32()?;
        let timeout = r.get_i32()?;
        let mood = r.get_i32()?;
        let mut effects = Vec::new();
        if version > 3 {
            let n = count(r, 4)?;
            effects.reserve(n);
            for _ in 0..n {
                effects.push(r.get_u32()?);
            }
        }
        let subject = ObjectReference::read(r)?;
        let deal_type = if deal { Some(r.get_i32()?) } else { None };
        Some(Self {
            version,
            table_index,
            guid,
            flags,
            advisor_id,
            priority,
            frequency,
            timeout,
            mood,
            effects,
            subject,
            deal_type,
        })
    }

    pub fn write(&self, w: &mut dyn OutStream) -> bool {
        if self.deal_type.is_some() {
            w.put_u16(2);
        }
        w.put_u16(self.version);
        w.put_i32(self.table_index);
        w.put_u32(self.guid);
        w.put_u32(self.flags);
        for v in [self.advisor_id, self.priority, self.frequency, self.timeout, self.mood] {
            w.put_i32(v);
        }
        if self.version > 3 {
            w.put_u32(self.effects.len() as u32);
            for effect in &self.effects {
                w.put_u32(*effect);
            }
        }
        self.subject.write(w);
        if let Some(deal) = self.deal_type {
            w.put_i32(deal);
        }
        w.error() == 0
    }
}

#[derive(Clone, Debug)]
pub enum AdvisorArchive {
    System(Vec<(ObjectReference, i32)>),
    Advisor(AdvisorRecord),
    Planning {
        version: u16,
        advisor: AdvisorRecord,
        counts: [i32; 3],
        landmarks: Vec<(i32, i32)>,
    },
    Advice(AdviceRecord),
    Subject {
        has_location: u8,
        location: Option<[f32; 2]>,
        data: [u32; 2],
        object: ObjectReference,
    },
}

impl AdvisorArchive {
    pub fn handles(class: u32) -> bool {
        matches!(
            class,
            clsid::SYSTEM
                | clsid::SUBJECT
                | 0x8A09_F5F4
                | 0xAA37_1C32
                | 0x8BE3_753B
                | 0x6A38_48DB
                | 0x6A93_35DE
                | 0x6B70_EFAD
        ) || AdvisorKind::from_class(class).is_some()
    }

    pub fn read(class: u32, r: &mut ByteReader<'_>) -> Option<Self> {
        Some(match class {
            clsid::SYSTEM => {
                if r.get_u16()? != 1 {
                    return None;
                }
                let n = count(r, 8)?;
                let mut advisors = Vec::with_capacity(n);
                for _ in 0..n {
                    advisors.push((ObjectReference::read(r)?, r.get_i32()?));
                }
                Self::System(advisors)
            }
            clsid::SUBJECT => {
                if r.get_u16()? != 4 {
                    return None;
                }
                let has_location = r.get_u8()?;
                let location = if has_location != 0 {
                    Some([r.get_f32()?, r.get_f32()?])
                } else {
                    None
                };
                Self::Subject {
                    has_location,
                    location,
                    data: [r.get_u32()?, r.get_u32()?],
                    object: ObjectReference::read(r)?,
                }
            }
            clsid::CITY_PLANNING => {
                let version = r.get_u16()?;
                let advisor = AdvisorRecord::read(r)?;
                let counts = [r.get_i32()?, r.get_i32()?, r.get_i32()?];
                let mut landmarks = Vec::new();
                if version > 2 {
                    let n = count(r, 8)?;
                    landmarks.reserve(n);
                    for _ in 0..n {
                        landmarks.push((r.get_i32()?, r.get_i32()?));
                    }
                }
                Self::Planning {
                    version,
                    advisor,
                    counts,
                    landmarks,
                }
            }
            _ if AdvisorKind::from_class(class).is_some() => Self::Advisor(AdvisorRecord::read(r)?),
            _ if Self::handles(class) => Self::Advice(AdviceRecord::read(class == 0x6A38_48DB, r)?),
            _ => return None,
        })
    }

    pub fn write(&self, w: &mut dyn OutStream) -> bool {
        match self {
            Self::System(advisors) => {
                w.put_u16(1);
                w.put_u32(advisors.len() as u32);
                for (reference, id) in advisors {
                    reference.write(w);
                    w.put_i32(*id);
                }
            }
            Self::Advisor(advisor) => {
                advisor.write(w);
            }
            Self::Planning {
                version,
                advisor,
                counts,
                landmarks,
            } => {
                w.put_u16(*version);
                advisor.write(w);
                for v in counts {
                    w.put_i32(*v);
                }
                if *version > 2 {
                    w.put_u32(landmarks.len() as u32);
                    for (key, count) in landmarks {
                        w.put_i32(*key);
                        w.put_i32(*count);
                    }
                }
            }
            Self::Advice(advice) => {
                advice.write(w);
            }
            Self::Subject {
                has_location,
                location,
                data,
                object,
            } => {
                w.put_u16(4);
                w.put_u8(*has_location);
                if let Some(location) = location {
                    for v in location {
                        w.put_f32(*v);
                    }
                }
                for v in data {
                    w.put_u32(*v);
                }
                object.write(w);
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
    fn advice_archive_preserves_clean_flags_versions_and_subject_identity() {
        for version in [3, 4, 5] {
            for deal in [false, true] {
                let mut w = ByteWriter::new();
                if deal {
                    w.put_u16(2);
                }
                w.put_u16(version);
                // Table, guid, clean flags, advisor, priority, frequency, timeout, mood.
                for v in [7, 0xDEAD_BEEF, 0x8000_0002, 25, u32::MAX, 3, 4, 5] {
                    w.put_u32(v);
                }
                if version > 3 {
                    w.put_u32(2);
                    w.put_u32(91);
                    w.put_u32(91);
                }
                w.put_u32(37);
                w.put_u32(clsid::SUBJECT);
                if deal {
                    w.put_i32(-17);
                }
                let bytes = w.into_bytes();
                let mut r = ByteReader::new(&bytes);
                let mut record = AdviceRecord::read(deal, &mut r).unwrap();
                assert_eq!(r.remaining(), 0);
                assert_eq!(record.flags, 0x8000_0002); // Live Read would add 0x10.
                let mut w = ByteWriter::new();
                assert!(record.write(&mut w));
                assert_eq!(w.into_bytes(), bytes);
                // Writing uses the decoded fields, including a changed saved identity.
                record.flags = 0x8000_0012;
                record.subject.id = 101;
                let mut w = ByteWriter::new();
                assert!(record.write(&mut w));
                let mut expected = bytes.clone();
                expected[10 + usize::from(deal) * 2] |= 0x10;
                let subject_offset = expected.len() - if deal { 12 } else { 8 };
                expected[subject_offset..subject_offset + 4].copy_from_slice(&101u32.to_le_bytes());
                assert_eq!(w.into_bytes(), expected);
                for end in 0..bytes.len() {
                    assert!(AdviceRecord::read(deal, &mut ByteReader::new(&bytes[..end])).is_none());
                }
            }
        }
    }
}
