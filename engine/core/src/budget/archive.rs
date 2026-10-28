// SPDX-License-Identifier: GPL-3.0-or-later
//! Budget saved fields (budget §11), before live Read resolves names, owners
//! and funding. File order and saved references are independent of live objects.

use crate::store::archive::{
    fields::{array, count, references},
    ObjectReference,
};
use crate::stream::{ByteReader, InStream, OutStream};

#[derive(Clone, Debug, Default)]
pub struct TaxGroupRecord {
    pub rate: f32,
    pub max_rate: f32,
    pub history: [i64; 12],
    pub reserved: i64,
    pub weighted_assessed: i64,
    pub monthly_tax: i64,
}

#[derive(Clone, Debug)]
pub struct BondRecord {
    pub due_month: i32,
    pub due_year: i32,
    pub amounts: [i64; 6],
}

#[derive(Clone, Debug)]
pub struct GroupRecord {
    pub key: u32,
    pub id: u32,
    pub name: Vec<u8>,
    pub total_expense: i64,
    pub total_income: i64,
    pub departments: Vec<u32>,
}

#[derive(Clone, Debug)]
pub struct BudgetRecord {
    pub version: u16,
    pub funds: [i64; 4],
    pub departments: Vec<(u32, ObjectReference)>,
    pub tax: [TaxGroupRecord; 12],
    pub ytd_ordinance: [i64; 2],
    pub ordinance_expense: [i64; 12],
    pub ordinance_income: [i64; 12],
    pub ytd_tax: i64,
    pub ytd_reserved: [i64; 5],
    pub bonds: Vec<BondRecord>,
    pub income_expense: [i64; 5],
    pub groups: Vec<GroupRecord>,
}

impl BudgetRecord {
    pub fn read(r: &mut ByteReader<'_>) -> Option<Self> {
        let version = r.get_u16()?;
        if !(4..=5).contains(&version) {
            return None;
        }
        let funds = array(r, |r| r.get_i64())?;
        let n = count(r, 8)?;
        let mut departments = Vec::with_capacity(n);
        for _ in 0..n {
            departments.push((r.get_u32()?, ObjectReference::read(r)?));
        }
        let mut tax: [TaxGroupRecord; 12] = Default::default();
        for g in &mut tax {
            g.rate = r.get_f32()?;
            g.max_rate = r.get_f32()?;
            g.history = array(r, |r| r.get_i64())?;
            g.reserved = r.get_i64()?;
            g.weighted_assessed = r.get_i64()?;
            g.monthly_tax = r.get_i64()?;
        }
        let ytd_ordinance = array(r, |r| r.get_i64())?;
        let ordinance_expense = array(r, |r| r.get_i64())?;
        let ordinance_income = array(r, |r| r.get_i64())?;
        let ytd_tax = r.get_i64()?;
        let ytd_reserved = array(r, |r| r.get_i64())?;
        let n = count(r, 56)?;
        let mut bonds = Vec::with_capacity(n);
        for _ in 0..n {
            bonds.push(BondRecord {
                due_month: r.get_i32()?,
                due_year: r.get_i32()?,
                amounts: array(r, |r| r.get_i64())?,
            });
        }
        let income_expense = array(r, |r| r.get_i64())?;
        let n = count(r, 32)?;
        let mut groups = Vec::with_capacity(n);
        for _ in 0..n {
            let key = r.get_u32()?;
            let id = r.get_u32()?;
            let name = r.get_str()?;
            let total_expense = r.get_i64()?;
            let total_income = r.get_i64()?;
            let n = count(r, 4)?;
            let mut departments = Vec::with_capacity(n);
            for _ in 0..n {
                departments.push(r.get_u32()?);
            }
            groups.push(GroupRecord {
                key,
                id,
                name,
                total_expense,
                total_income,
                departments,
            });
        }
        Some(Self {
            version,
            funds,
            departments,
            tax,
            ytd_ordinance,
            ordinance_expense,
            ordinance_income,
            ytd_tax,
            ytd_reserved,
            bonds,
            income_expense,
            groups,
        })
    }

    pub fn write(&self, w: &mut dyn OutStream) -> bool {
        w.put_u16(self.version);
        for v in self.funds {
            w.put_i64(v);
        }
        w.put_u32(self.departments.len() as u32);
        for (id, reference) in &self.departments {
            w.put_u32(*id);
            reference.write(w);
        }
        for g in &self.tax {
            w.put_f32(g.rate);
            w.put_f32(g.max_rate);
            for v in g.history {
                w.put_i64(v);
            }
            for v in [g.reserved, g.weighted_assessed, g.monthly_tax] {
                w.put_i64(v);
            }
        }
        for v in self.ytd_ordinance {
            w.put_i64(v);
        }
        for v in self.ordinance_expense {
            w.put_i64(v);
        }
        for v in self.ordinance_income {
            w.put_i64(v);
        }
        w.put_i64(self.ytd_tax);
        for v in self.ytd_reserved {
            w.put_i64(v);
        }
        w.put_u32(self.bonds.len() as u32);
        for b in &self.bonds {
            w.put_i32(b.due_month);
            w.put_i32(b.due_year);
            for v in b.amounts {
                w.put_i64(v);
            }
        }
        for v in self.income_expense {
            w.put_i64(v);
        }
        w.put_u32(self.groups.len() as u32);
        for g in &self.groups {
            w.put_u32(g.key);
            w.put_u32(g.id);
            w.put_str(&g.name);
            w.put_i64(g.total_expense);
            w.put_i64(g.total_income);
            w.put_u32(g.departments.len() as u32);
            for v in &g.departments {
                w.put_u32(*v);
            }
        }
        w.error() == 0
    }
}

#[derive(Clone, Debug)]
pub struct DepartmentRecord {
    pub version: u16,
    pub initialized: u8,
    pub id: u32,
    pub name: Vec<u8>,
    pub budget_group: u32,
    /// Persisted units: percentages before v10, fractions in v10.
    pub funding: [f32; 2],
    pub total_spending: i64,
    pub fixed: u8,
    pub lines: Vec<ObjectReference>,
    pub locals: Vec<(ObjectReference, u32)>,
    pub name_ltext: [u32; 2],
}

impl DepartmentRecord {
    pub fn read(r: &mut ByteReader<'_>) -> Option<Self> {
        let version = r.get_u16()?;
        if version > 10 {
            return None;
        }
        let initialized = r.get_u8()?;
        let id = r.get_u32()?;
        let name = r.get_str()?;
        let budget_group = r.get_u32()?;
        let funding = array(r, |r| r.get_f32())?;
        let total_spending = r.get_i64()?;
        let fixed = r.get_u8()?;
        let lines = references(r)?;
        let n = count(r, 8)?;
        let mut locals = Vec::with_capacity(n);
        for _ in 0..n {
            locals.push((ObjectReference::read(r)?, r.get_u32()?));
        }
        let name_ltext = array(r, |r| r.get_u32())?;
        Some(Self {
            version,
            initialized,
            id,
            name,
            budget_group,
            funding,
            total_spending,
            fixed,
            lines,
            locals,
            name_ltext,
        })
    }

    pub fn write(&self, w: &mut dyn OutStream) -> bool {
        w.put_u16(self.version);
        w.put_u8(self.initialized);
        w.put_u32(self.id);
        w.put_str(&self.name);
        w.put_u32(self.budget_group);
        for v in self.funding {
            w.put_f32(v);
        }
        w.put_i64(self.total_spending);
        w.put_u8(self.fixed);
        w.put_u32(self.lines.len() as u32);
        for reference in &self.lines {
            reference.write(w);
        }
        w.put_u32(self.locals.len() as u32);
        for (reference, purpose) in &self.locals {
            reference.write(w);
            w.put_u32(*purpose);
        }
        for v in self.name_ltext {
            w.put_u32(v);
        }
        w.error() == 0
    }
}

#[derive(Clone, Debug)]
pub struct LineItemRecord {
    pub name: Vec<u8>,
    pub amounts: [i64; 4],
    pub id: u32,
    pub display_flags: u32,
    pub is_local: u8,
    pub kind: u16,
    pub owner: ObjectReference,
    pub name_ltext: [u32; 2],
}

impl LineItemRecord {
    pub fn read(r: &mut ByteReader<'_>) -> Option<Self> {
        if r.get_u16()? != 4 {
            return None;
        }
        Some(Self {
            name: r.get_str()?,
            amounts: array(r, |r| r.get_i64())?,
            id: r.get_u32()?,
            display_flags: r.get_u32()?,
            is_local: r.get_u8()?,
            kind: r.get_u16()?,
            owner: ObjectReference::read(r)?,
            name_ltext: array(r, |r| r.get_u32())?,
        })
    }

    pub fn write(&self, w: &mut dyn OutStream) -> bool {
        w.put_u16(4);
        w.put_str(&self.name);
        for v in self.amounts {
            w.put_i64(v);
        }
        w.put_u32(self.id);
        w.put_u32(self.display_flags);
        w.put_u8(self.is_local);
        w.put_u16(self.kind);
        self.owner.write(w);
        for v in self.name_ltext {
            w.put_u32(v);
        }
        w.error() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::ByteWriter;

    #[test]
    fn line_item_retains_saved_identity_and_can_be_edited() {
        let original = LineItemRecord {
            name: b"\xffsaved name".to_vec(),
            amounts: [-1, 2, 3, 4],
            id: 7,
            display_flags: 0xA5,
            is_local: 1,
            kind: 2,
            owner: ObjectReference {
                id: 0x1234,
                class: 0xE990_BFFC,
            },
            name_ltext: [8, 9],
        };
        let mut w = ByteWriter::new();
        assert!(original.write(&mut w));
        let bytes = w.into_bytes();
        let mut parsed = LineItemRecord::read(&mut ByteReader::new(&bytes)).unwrap();
        assert_eq!(parsed.owner, original.owner);
        assert_eq!(parsed.name, original.name);
        parsed.amounts[0] = -42;
        let mut w = ByteWriter::new();
        assert!(parsed.write(&mut w));
        let changed = w.into_bytes();
        assert_eq!(
            LineItemRecord::read(&mut ByteReader::new(&changed)).unwrap().amounts[0],
            -42
        );
        for end in 0..bytes.len() {
            assert!(LineItemRecord::read(&mut ByteReader::new(&bytes[..end])).is_none());
        }
    }
}
