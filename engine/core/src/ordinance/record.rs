// SPDX-License-Identifier: GPL-3.0-or-later
//! Archive models for ordinance.md §9. Runtime loading additionally refreshes
//! exemplar fields; an offline archive keeps the saved fields and parent key.

use crate::property::archive::ExemplarRecord;
use crate::store::archive::ObjectReference;
use crate::stream::{ByteReader, InStream, OutStream};

#[derive(Clone, Debug)]
pub struct OrdinanceRecord {
    pub initialized: bool,
    pub id: u32,
    pub name: Vec<u8>,
    pub description: Vec<u8>,
    pub year_first_available: u32,
    pub chance_availability: f32,
    pub enactment_income: i64,
    pub retracment_income: i64,
    pub monthly_constant_income: i64,
    pub monthly_income_factor: f32,
    pub advisor_id: u32,
    pub available: bool,
    pub on: bool,
    pub enabled: bool,
    pub adjusted: i64,
    pub is_income_ordinance: bool,
    pub properties: ExemplarRecord,
}

impl OrdinanceRecord {
    pub fn read(r: &mut dyn InStream) -> Option<Self> {
        let version = r.get_u16()?;
        let initialized = r.get_bool()?;
        let id = r.get_u32()?;
        let name = r.get_str()?;
        let description = r.get_str()?;
        let year_first_available = r.get_u32()?;
        let chance = r.get_f32()?;
        Some(Self {
            initialized,
            id,
            name,
            description,
            year_first_available,
            chance_availability: if version < 4 { chance * 0.01 } else { chance },
            enactment_income: r.get_i64()?,
            retracment_income: r.get_i64()?,
            monthly_constant_income: r.get_i64()?,
            monthly_income_factor: r.get_f32()?,
            advisor_id: r.get_u32()?,
            available: r.get_bool()?,
            on: r.get_bool()?,
            enabled: r.get_bool()?,
            adjusted: r.get_i64()?,
            is_income_ordinance: if version < 2 { id == 0xA0D0_7129 } else { r.get_bool()? },
            properties: ExemplarRecord::read(r)?,
        })
    }

    pub fn write(&self, w: &mut dyn OutStream) -> bool {
        w.put_u16(4);
        w.put_bool(self.initialized);
        w.put_u32(self.id);
        w.put_str(&self.name);
        w.put_str(&self.description);
        w.put_u32(self.year_first_available);
        w.put_f32(self.chance_availability);
        w.put_i64(self.enactment_income);
        w.put_i64(self.retracment_income);
        w.put_i64(self.monthly_constant_income);
        w.put_f32(self.monthly_income_factor);
        w.put_u32(self.advisor_id);
        w.put_bool(self.available);
        w.put_bool(self.on);
        w.put_bool(self.enabled);
        w.put_i64(self.adjusted);
        w.put_bool(self.is_income_ordinance);
        self.properties.write(w) && w.error() == 0
    }
}

#[derive(Clone, Debug)]
pub struct OrdinanceSimulatorRecord {
    pub monthly_expense: i64,
    pub monthly_income: i64,
    pub ordinances: Vec<ObjectReference>,
    pub pending: i64,
}

impl OrdinanceSimulatorRecord {
    pub fn read(r: &mut ByteReader<'_>) -> Option<Self> {
        if r.get_u16()? != 4 {
            return None;
        }
        let monthly_expense = r.get_i64()?;
        let monthly_income = r.get_i64()?;
        let count = r.get_u32()? as usize;
        if count > r.remaining() / 4 {
            return None;
        }
        let ordinances = (0..count).map(|_| ObjectReference::read(r)).collect::<Option<_>>()?;
        Some(Self {
            monthly_expense,
            monthly_income,
            ordinances,
            pending: r.get_i64()?,
        })
    }

    pub fn write(&self, w: &mut dyn OutStream) -> bool {
        w.put_u16(4);
        w.put_i64(self.monthly_expense);
        w.put_i64(self.monthly_income);
        w.put_u32(self.ordinances.len() as u32);
        for reference in &self.ordinances {
            reference.write(w);
        }
        w.put_i64(self.pending) && w.error() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::ByteWriter;

    // Independent field-order fixture, ordinance §9.2. Legacy chance is a percentage.
    fn fixture(version: u16) -> Vec<u8> {
        let mut w = ByteWriter::new();
        w.put_u16(version);
        w.put_bool(true);
        w.put_u32(0xA0D0_7129);
        w.put_str(b"Gambling");
        w.put_str(b"Description");
        w.put_u32(1950);
        w.put_f32(if version < 4 { 25.0 } else { 0.25 });
        w.put_i64(-125);
        w.put_i64(254);
        w.put_i64(-1500);
        w.put_f32(-0.0);
        w.put_u32(21);
        w.put_bool(true);
        w.put_bool(false);
        w.put_bool(true);
        w.put_i64(-999);
        if version >= 2 {
            w.put_bool(true);
        }
        w.put_u16(2); // exemplar holder
        w.put_u16(2); // plain holder
        w.put_u32(0); // no own properties
        w.put_u32(71); // group, type, instance
        w.put_u32(82);
        w.put_u32(93);
        w.into_bytes()
    }

    #[test]
    fn ordinance_fields_decode_mutate_and_legacy_output_stabilizes() {
        let current = fixture(4);
        for version in [1, 3, 4] {
            let bytes = fixture(version);
            let mut r = ByteReader::new(&bytes);
            let mut record = OrdinanceRecord::read(&mut r).unwrap();
            assert_eq!(r.remaining(), 0);
            assert_eq!(record.chance_availability, 0.25);
            assert!(record.is_income_ordinance);
            assert_eq!(record.enactment_income, -125);
            assert_eq!(record.properties.parent.instance, 93);
            let mut w = ByteWriter::new();
            assert!(record.write(&mut w));
            assert_eq!(w.bytes(), current);
            record.adjusted = 12345;
            record.properties.parent.instance = 42;
            let mut w = ByteWriter::new();
            assert!(record.write(&mut w));
            let decoded = OrdinanceRecord::read(&mut ByteReader::new(w.bytes())).unwrap();
            assert_eq!(decoded.adjusted, 12345);
            assert_eq!(decoded.properties.parent.instance, 42);
            for end in 0..bytes.len() {
                assert!(OrdinanceRecord::read(&mut ByteReader::new(&bytes[..end])).is_none());
            }
        }
    }

    #[test]
    fn simulator_null_and_shared_references_keep_their_saved_ids() {
        let refs = vec![
            ObjectReference {
                id: 71,
                class: 0x00D0_723D,
            },
            ObjectReference { id: 0, class: 0 },
            ObjectReference {
                id: 71,
                class: 0x00D0_723D,
            },
        ];
        let record = OrdinanceSimulatorRecord {
            monthly_expense: -71,
            monthly_income: 93,
            ordinances: refs.clone(),
            pending: -123,
        };
        let mut w = ByteWriter::new();
        assert!(record.write(&mut w));
        let mut r = ByteReader::new(w.bytes());
        let decoded = OrdinanceSimulatorRecord::read(&mut r).unwrap();
        assert_eq!(decoded.ordinances, refs);
        assert_eq!(decoded.pending, -123);
        assert_eq!(r.remaining(), 0);
        let mut bad = w.into_bytes();
        bad[18..22].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(OrdinanceSimulatorRecord::read(&mut ByteReader::new(&bad)).is_none());
    }
}
