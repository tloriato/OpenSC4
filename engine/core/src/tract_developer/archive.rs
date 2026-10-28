// SPDX-License-Identifier: GPL-3.0-or-later
//! Saved style selection, tract-developer §6. Retain an empty style list and an
//! out-of-range index without the changes Init applies to a running city.
use crate::store::archive::fields::count;
use crate::stream::{ByteReader, InStream, OutStream};

#[derive(Clone, Debug)]
pub struct TractRecord {
    pub version: u16,
    pub current_style: i32,
    pub years_since_change: i32,
    pub styles: Option<(u8, Vec<u32>)>,
    pub years_between_changes: Option<i32>,
}

impl TractRecord {
    pub fn read(r: &mut ByteReader<'_>) -> Option<Self> {
        let version = r.get_u16()?;
        if version > 3 {
            return None;
        }
        let current_style = r.get_i32()?;
        let years_since_change = r.get_i32()?;
        let styles = if version >= 2 {
            let single_style = r.get_u8()?;
            let n = count(r, 4)?;
            let mut styles = Vec::with_capacity(n);
            for _ in 0..n {
                styles.push(r.get_u32()?);
            }
            Some((single_style, styles))
        } else {
            None
        };
        let years_between_changes = if version >= 3 { Some(r.get_i32()?) } else { None };
        Some(Self {
            version,
            current_style,
            years_since_change,
            styles,
            years_between_changes,
        })
    }

    pub fn write(&self, w: &mut dyn OutStream) -> bool {
        w.put_u16(self.version);
        w.put_i32(self.current_style);
        w.put_i32(self.years_since_change);
        if let Some((single_style, styles)) = &self.styles {
            w.put_u8(*single_style);
            w.put_u32(styles.len() as u32);
            for style in styles {
                w.put_u32(*style);
            }
        }
        if let Some(years) = self.years_between_changes {
            w.put_i32(years);
        }
        w.error() == 0
    }
}
