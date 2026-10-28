// SPDX-License-Identifier: GPL-3.0-or-later
//! Regional-city wire state (foundations §11.2). Unlike live Read, an archive
//! neither localizes names, merges deal lists, sets Silent nor inserts defaults.
//! Keep the source version and its workforce units for lossless legacy saves.

use super::Commodity;
use crate::neighbors::archive::{ConnectionRecord, DealRecord};
use crate::store::archive::fields::{array, count};
use crate::stream::{ByteReader, InStream, OutStream};

#[derive(Clone, Debug)]
pub struct PendingRecord {
    pub kind: u32,
    pub cell: [i32; 2],
    pub flag: u8,
}

#[derive(Clone, Debug)]
pub struct RegionalCityRecord {
    pub version: u16,
    pub position: [i32; 2],
    pub size: [i32; 2],
    pub population: i32,
    pub commercial_jobs: i32,
    pub industrial_jobs: i32,
    pub workforce: Option<f32>,
    pub mayor_rating: i8,
    pub difficulty: Option<u8>,
    pub moved: u8,
    pub serial: u32,
    pub original_locale: [i32; 2],
    pub last_locale: [i32; 2],
    pub birth_date: u32,
    pub established: u8,
    /// city, unknown 1, mayor, unknown 2, utility advisor, description, unknown 3.
    pub names: [Vec<u8>; 7],
    pub world: [f32; 3],
    pub tutorial_id: i32,
    pub populations: [Vec<(u32, i32)>; 3],
    pub tax_rates: Vec<f32>,
    pub connections: Vec<ConnectionRecord>,
    pub pending: Vec<PendingRecord>,
    pub budget: f64,
    pub income: f64,
    pub commodities: Vec<(i32, Commodity)>,
    pub other_deals: Vec<DealRecord>,
    pub local_deals: Vec<DealRecord>,
    pub points: Vec<(u32, Vec<[f32; 3]>)>,
}

fn bounded(r: &mut ByteReader<'_>, minimum: usize, limit: usize) -> Option<usize> {
    let n = count(r, minimum)?;
    (n <= limit).then_some(n)
}

impl RegionalCityRecord {
    /// Includes the auto-stream prefix; publishes nothing on a partial read.
    pub fn read(r: &mut ByteReader<'_>) -> Option<Self> {
        if r.get_u16()? != 1 {
            return None;
        }
        let version = r.get_u16()?;
        if version >= 14 {
            return None;
        }
        let position = array(r, |r| r.get_i32())?;
        let size = array(r, |r| r.get_i32())?;
        let population = r.get_i32()?;
        let commercial_jobs = r.get_i32()?;
        let industrial_jobs = r.get_i32()?;
        let workforce = if version >= 10 { Some(r.get_f32()?) } else { None };
        let mayor_rating = r.get_i8()?;
        let difficulty = if version >= 13 { Some(r.get_u8()?) } else { None };
        let moved = r.get_u8()?;
        let serial = r.get_u32()?;
        let original_locale = array(r, |r| r.get_i32())?;
        let last_locale = array(r, |r| r.get_i32())?;
        let birth_date = r.get_u32()?;
        let established = r.get_u8()?;
        let mut names = std::array::from_fn(|_| Vec::new());
        for name in &mut names {
            *name = r.get_str()?;
        }
        let world = array(r, |r| r.get_f32())?;
        let tutorial_id = r.get_i32()?;
        let mut populations = std::array::from_fn(|_| Vec::new());
        for map in &mut populations {
            for _ in 0..bounded(r, 8, 0x2000_0000)? {
                map.push((r.get_u32()?, r.get_i32()?));
            }
        }
        let mut tax_rates = Vec::new();
        for _ in 0..bounded(r, 4, 12)? {
            tax_rates.push(r.get_f32()?);
        }
        let mut connections = Vec::new();
        for _ in 0..bounded(r, 27, 0x10_0000)? {
            connections.push(ConnectionRecord::read(r)?);
        }
        let mut pending = Vec::new();
        for _ in 0..bounded(r, 13, 0x10_0000)? {
            pending.push(PendingRecord {
                kind: r.get_u32()?,
                cell: array(r, |r| r.get_i32())?,
                flag: r.get_u8()?,
            });
        }
        let budget = r.get_f64()?;
        let income = r.get_f64()?;
        let mut commodities = Vec::new();
        for _ in 0..bounded(r, 44, 0x10_0000)? {
            commodities.push((
                r.get_i32()?,
                Commodity {
                    produced: r.get_f64()?,
                    demanded: r.get_f64()?,
                    imported: r.get_f64()?,
                    exported: r.get_f64()?,
                    cost_per_unit: r.get_f64()?,
                },
            ));
        }
        let mut other_deals = Vec::new();
        let mut local_deals = Vec::new();
        for list in [&mut other_deals, &mut local_deals] {
            for _ in 0..count(r, 68)? {
                list.push(DealRecord::read(r)?);
            }
        }
        let mut points = Vec::new();
        if version >= 12 {
            for _ in 0..count(r, 8)? {
                let kind = r.get_u32()?;
                let mut positions = Vec::new();
                for _ in 0..count(r, 12)? {
                    positions.push(array(r, |r| r.get_f32())?);
                }
                points.push((kind, positions));
            }
        }
        Some(Self {
            version,
            position,
            size,
            population,
            commercial_jobs,
            industrial_jobs,
            workforce,
            mayor_rating,
            difficulty,
            moved,
            serial,
            original_locale,
            last_locale,
            birth_date,
            established,
            names,
            world,
            tutorial_id,
            populations,
            tax_rates,
            connections,
            pending,
            budget,
            income,
            commodities,
            other_deals,
            local_deals,
            points,
        })
    }

    pub fn write(&self, w: &mut dyn OutStream) -> bool {
        if self.version >= 14
            || self.workforce.is_some() != (self.version >= 10)
            || self.difficulty.is_some() != (self.version >= 13)
            || (self.version < 12 && !self.points.is_empty())
        {
            return false;
        }
        w.put_u16(1);
        w.put_u16(self.version);
        for v in self.position.into_iter().chain(self.size) {
            w.put_i32(v);
        }
        w.put_i32(self.population);
        w.put_i32(self.commercial_jobs);
        w.put_i32(self.industrial_jobs);
        if let Some(v) = self.workforce {
            w.put_f32(v);
        }
        w.put_i8(self.mayor_rating);
        if let Some(v) = self.difficulty {
            w.put_u8(v);
        }
        w.put_u8(self.moved);
        w.put_u32(self.serial);
        for v in self.original_locale.into_iter().chain(self.last_locale) {
            w.put_i32(v);
        }
        w.put_u32(self.birth_date);
        w.put_u8(self.established);
        for name in &self.names {
            w.put_str(name);
        }
        for v in self.world {
            w.put_f32(v);
        }
        w.put_i32(self.tutorial_id);
        for map in &self.populations {
            w.put_u32(map.len() as u32);
            for &(kind, value) in map {
                w.put_u32(kind);
                w.put_i32(value);
            }
        }
        w.put_u32(self.tax_rates.len() as u32);
        for &v in &self.tax_rates {
            w.put_f32(v);
        }
        w.put_u32(self.connections.len() as u32);
        for connection in &self.connections {
            if !connection.write(w) {
                return false;
            }
        }
        w.put_u32(self.pending.len() as u32);
        for change in &self.pending {
            w.put_u32(change.kind);
            for v in change.cell {
                w.put_i32(v);
            }
            w.put_u8(change.flag);
        }
        w.put_f64(self.budget);
        w.put_f64(self.income);
        w.put_u32(self.commodities.len() as u32);
        for &(kind, c) in &self.commodities {
            w.put_i32(kind);
            w.put_f64(c.produced);
            w.put_f64(c.demanded);
            w.put_f64(c.imported);
            w.put_f64(c.exported);
            w.put_f64(c.cost_per_unit);
        }
        for deals in [&self.other_deals, &self.local_deals] {
            w.put_u32(deals.len() as u32);
            for deal in deals {
                if !deal.write(w) {
                    return false;
                }
            }
        }
        if self.version >= 12 {
            w.put_u32(self.points.len() as u32);
            for (kind, positions) in &self.points {
                w.put_u32(*kind);
                w.put_u32(positions.len() as u32);
                for &point in positions {
                    for v in point {
                        w.put_f32(v);
                    }
                }
            }
        }
        w.error() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::property::PropertyHolder;
    use crate::stream::ByteWriter;

    fn fixture(version: u16) -> Vec<u8> {
        let mut w = ByteWriter::new();
        w.put_u16(1);
        w.put_u16(version);
        for value in [7, -8, 2, 4, 11, 12, 13] {
            w.put_i32(value);
        }
        if version >= 10 {
            // v10 percentages must not be scaled then rounded back.
            w.put_u32(0x4240_0001);
        }
        w.put_i8(-17);
        if version >= 13 {
            w.put_u8(2);
        }
        w.put_u8(0x80);
        w.put_u32(0xCDCD_CDCD);
        for value in [1, 44, 2, 33] {
            w.put_i32(value);
        }
        w.put_u32(999);
        w.put_u8(0); // unestablished: never localize the saved name
        for name in [
            b"source".as_slice(),
            b"",
            b"mayor",
            b"a\0b",
            b"utility",
            b"desc",
            b"\xff",
        ] {
            w.put_str(name);
        }
        for bits in [0x8000_0000, 0x7FC0_1234, 0x3F80_0000] {
            w.put_u32(bits);
        }
        w.put_i32(0x123456); // no tutorial registry remapping
        for _ in 0..3 {
            w.put_u32(2);
            w.put_u32(0x1010);
            w.put_i32(1);
            w.put_u32(0x1010);
            w.put_i32(2);
        }
        w.put_u32(1); // do not pad to twelve tax rates
        w.put_u32(0x8000_0000);
        w.put_u32(1);
        assert!(ConnectionRecord {
            kind: 5,
            cell: [-2, 3],
            other_region: [8, 9],
            flags: 0xE7,
            properties: PropertyHolder::new(),
        }
        .write(&mut w));
        w.put_u32(1);
        w.put_u32(21);
        w.put_i32(-3);
        w.put_i32(4);
        w.put_u8(0xFE);
        w.put_u64(0x7FF8_0000_0000_1234);
        w.put_u64(0x8000_0000_0000_0000);
        w.put_u32(1); // no constructor commodity defaults
        w.put_i32(2);
        for bits in [1, 2, 3, 4, 5] {
            w.put_u64(bits);
        }
        for version in [4, 5] {
            w.put_u32(1);
            assert!(DealRecord {
                version,
                kind: 0xE2,
                other: [1, 2],
                amount: 27,
                first_amount_used: 11,
                cost: -123,
                flags: 0,
                city: [3, 4],
                incremental: -1,
                last: 5,
                amount_used: 9,
                dates: [11, 12, 13],
                properties: PropertyHolder::new(),
                suspension: (version > 4).then_some(31),
            }
            .write(&mut w));
        }
        if version >= 12 {
            w.put_u32(2);
            for n in [1, 0] {
                w.put_u32(7); // same key, separate saved lists
                w.put_u32(n);
                if n != 0 {
                    for bits in [0x7FC0_4321, 0x8000_0000, 1] {
                        w.put_u32(bits);
                    }
                }
            }
        }
        w.into_bytes()
    }

    #[test]
    fn archive_keeps_legacy_units_names_raw_flags_and_both_deal_lists() {
        for version in [9, 10, 11, 12, 13] {
            let source = fixture(version);
            let mut r = ByteReader::new(&source);
            let mut record = RegionalCityRecord::read(&mut r).unwrap();
            assert_eq!(r.remaining(), 0);
            assert_eq!(record.other_deals.len(), 1);
            assert_eq!(record.local_deals.len(), 1);
            assert_eq!(record.other_deals[0].flags, 0);
            assert_eq!(record.commodities.len(), 1);
            assert_eq!(record.populations[0].len(), 2);
            let mut out = ByteWriter::new();
            assert!(record.write(&mut out));
            assert_eq!(out.into_bytes(), source);
            record.local_deals[0].amount = 12345;
            let mut out = ByteWriter::new();
            assert!(record.write(&mut out));
            let edited = out.into_bytes();
            let reread = RegionalCityRecord::read(&mut ByteReader::new(&edited)).unwrap();
            assert_eq!(reread.local_deals[0].amount, 12345);
            assert_eq!(reread.other_deals[0].amount, 27);
        }
    }

    #[test]
    fn every_truncation_and_future_version_fail_without_publishing_state() {
        let source = fixture(13);
        for end in 0..source.len() {
            assert!(
                RegionalCityRecord::read(&mut ByteReader::new(&source[..end])).is_none(),
                "prefix {end}"
            );
        }
        let mut source = source;
        source[2] = 14;
        assert!(RegionalCityRecord::read(&mut ByteReader::new(&source)).is_none());
        source[0] = 2;
        source[2] = 13;
        assert!(RegionalCityRecord::read(&mut ByteReader::new(&source)).is_none());
    }
}
