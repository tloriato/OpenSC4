// SPDX-License-Identifier: GPL-3.0-or-later
//! Neighbour save fields (neighbors §4.3, §5.6, §13). Keep both saved copies of
//! amountUsed; no wall-clock construction, repair, deal reset or linking.
use crate::property::PropertyHolder;
use crate::store::archive::{
    fields::{array, references},
    ObjectReference,
};
use crate::stream::{ByteReader, InStream, OutStream};

#[derive(Clone, Debug)]
pub struct ConnectionRecord {
    pub kind: u32,
    pub cell: [i32; 2],
    pub other_region: [i32; 2],
    pub flags: u8,
    pub properties: PropertyHolder,
}

impl ConnectionRecord {
    pub fn read(r: &mut dyn InStream) -> Option<Self> {
        if r.get_u16()? != 1 {
            return None;
        }
        let kind = r.get_u32()?;
        let cell = array(r, |r| r.get_i32())?;
        let other_region = array(r, |r| r.get_i32())?;
        let flags = r.get_u8()?;
        let mut properties = PropertyHolder::new();
        properties.read(r).then_some(Self {
            kind,
            cell,
            other_region,
            flags,
            properties,
        })
    }

    pub fn write(&self, w: &mut dyn OutStream) -> bool {
        w.put_u16(1);
        w.put_u32(self.kind);
        for v in self.cell {
            w.put_i32(v);
        }
        for v in self.other_region {
            w.put_i32(v);
        }
        w.put_u8(self.flags);
        self.properties.write(w) && w.error() == 0
    }
}

#[derive(Clone, Debug)]
pub struct DealRecord {
    pub version: u16,
    pub kind: u32,
    pub other: [i32; 2],
    pub amount: i32,
    pub first_amount_used: i32,
    pub cost: i64,
    pub flags: i32,
    pub city: [i32; 2],
    pub incremental: i32,
    pub last: i32,
    pub amount_used: i32,
    pub dates: [u32; 3],
    pub properties: PropertyHolder,
    pub suspension: Option<u32>,
}

impl DealRecord {
    pub fn read(r: &mut dyn InStream) -> Option<Self> {
        let version = r.get_u16()?;
        let kind = r.get_u32()?;
        let other = array(r, |r| r.get_i32())?;
        let amount = r.get_i32()?;
        let first_amount_used = r.get_i32()?;
        let cost = r.get_i64()?;
        let flags = r.get_i32()?;
        let city = array(r, |r| r.get_i32())?;
        let incremental = r.get_i32()?;
        let last = r.get_i32()?;
        let amount_used = r.get_i32()?;
        let dates = array(r, |r| r.get_u32())?;
        let mut properties = PropertyHolder::new();
        if !properties.read(r) {
            return None;
        }
        let suspension = if version > 4 { Some(r.get_u32()?) } else { None };
        Some(Self {
            version,
            kind,
            other,
            amount,
            first_amount_used,
            cost,
            flags,
            city,
            incremental,
            last,
            amount_used,
            dates,
            properties,
            suspension,
        })
    }

    pub fn write(&self, w: &mut dyn OutStream) -> bool {
        w.put_u16(self.version);
        w.put_u32(self.kind);
        for v in self.other {
            w.put_i32(v);
        }
        w.put_i32(self.amount);
        w.put_i32(self.first_amount_used);
        w.put_i64(self.cost);
        w.put_i32(self.flags);
        for v in self.city {
            w.put_i32(v);
        }
        w.put_i32(self.incremental);
        w.put_i32(self.last);
        w.put_i32(self.amount_used);
        for v in self.dates {
            w.put_u32(v);
        }
        self.properties.write(w);
        if let Some(date) = self.suspension {
            w.put_u32(date);
        }
        w.error() == 0
    }
}

#[derive(Clone, Debug)]
pub enum NeighborsRecord {
    Simulator(Vec<ObjectReference>),
    Connection(ConnectionRecord),
    Deal(DealRecord),
}

impl NeighborsRecord {
    pub fn read(class: u32, r: &mut ByteReader<'_>) -> Option<Self> {
        Some(match class {
            0xC990_C0A4 => {
                if r.get_u16()? != 2 {
                    return None;
                }
                Self::Simulator(references(r)?)
            }
            0x0A13_4F24 => Self::Connection(ConnectionRecord::read(r)?),
            0x2A13_4F42 => Self::Deal(DealRecord::read(r)?),
            _ => return None,
        })
    }

    pub fn write(&self, w: &mut dyn OutStream) -> bool {
        match self {
            Self::Simulator(connections) => {
                w.put_u16(2);
                w.put_u32(connections.len() as u32);
                for connection in connections {
                    connection.write(w);
                }
                w.error() == 0
            }
            Self::Connection(connection) => connection.write(w),
            Self::Deal(deal) => deal.write(w),
        }
    }
}
