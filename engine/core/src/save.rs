// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The simulator's save record (spec §8, [compat]).
//!
//! ```text
//! uint16  version = 4
//! uint8   inTick
//! uint8   cityEstablished
//! int32   pauseCount
//! int32   simSpeed
//! int32   simTime
//! uint32  frameBudgetMs
//! 11 x { uint32 levelCounter[L]; uint8 levelFired[L] }
//! uint32  julianDay
//! ```
//! All little-endian.

use crate::ids::LEVEL_COUNT;
use std::fmt;

/// The only version we read and write.
pub const SAVE_VERSION: u16 = 4;

/// Byte width of every field of a version-4 record, in order. Hosts whose
/// streams are typed (GZCOM `cIGZOStream::SetUint8/16/32`) walk this table.
pub const RECORD_LAYOUT_V4: [u8; 7 + 2 * LEVEL_COUNT + 1] = {
    let mut l = [0u8; 7 + 2 * LEVEL_COUNT + 1];
    l[0] = 2; // version
    l[1] = 1; // inTick
    l[2] = 1; // cityEstablished
    l[3] = 4; // pauseCount
    l[4] = 4; // simSpeed
    l[5] = 4; // simTime
    l[6] = 4; // frameBudgetMs
    let mut i = 0;
    while i < LEVEL_COUNT {
        l[7 + 2 * i] = 4; // levelCounter
        l[8 + 2 * i] = 1; // levelFired
        i += 1;
    }
    l[7 + 2 * LEVEL_COUNT] = 4; // julianDay
    l
};

/// Total size in bytes of a version-4 record.
pub const RECORD_SIZE_V4: usize = 2 + 1 + 1 + 4 + 4 + 4 + 4 + LEVEL_COUNT * 5 + 4;

/// The persisted fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SaveRecord {
    pub version: u16,
    pub in_tick: bool,
    pub city_established: bool,
    pub pause_count: i32,
    pub sim_speed: i32,
    pub sim_time: i32,
    pub frame_budget_ms: u32,
    pub level_counters: [u32; LEVEL_COUNT],
    pub level_fired: [bool; LEVEL_COUNT],
    pub julian_day: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadError {
    /// Version >= 5 (spec: must fail).
    UnsupportedVersion(u16),
    /// The buffer ended before the record did.
    Truncated,
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::UnsupportedVersion(v) => write!(f, "unsupported simulator record version {v}"),
            LoadError::Truncated => write!(f, "truncated simulator record"),
        }
    }
}

impl std::error::Error for LoadError {}

impl SaveRecord {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(RECORD_SIZE_V4);
        out.extend_from_slice(&self.version.to_le_bytes());
        out.push(u8::from(self.in_tick));
        out.push(u8::from(self.city_established));
        out.extend_from_slice(&self.pause_count.to_le_bytes());
        out.extend_from_slice(&self.sim_speed.to_le_bytes());
        out.extend_from_slice(&self.sim_time.to_le_bytes());
        out.extend_from_slice(&self.frame_budget_ms.to_le_bytes());
        for l in 0..LEVEL_COUNT {
            out.extend_from_slice(&self.level_counters[l].to_le_bytes());
            out.push(u8::from(self.level_fired[l]));
        }
        out.extend_from_slice(&self.julian_day.to_le_bytes());
        out
    }

    /// Decode a record. Trailing bytes after the record are ignored.
    pub fn decode(bytes: &[u8]) -> Result<SaveRecord, LoadError> {
        let mut r = Reader { bytes, pos: 0 };
        let version = r.u16()?;
        if version >= 5 {
            return Err(LoadError::UnsupportedVersion(version));
        }
        let in_tick = r.u8()? != 0;
        let city_established = r.u8()? != 0;
        let pause_count = r.u32()? as i32;
        let sim_speed = r.u32()? as i32;
        let sim_time = r.u32()? as i32;
        let frame_budget_ms = r.u32()?;
        let mut level_counters = [0u32; LEVEL_COUNT];
        let mut level_fired = [false; LEVEL_COUNT];
        for l in 0..LEVEL_COUNT {
            level_counters[l] = r.u32()?;
            level_fired[l] = r.u8()? != 0;
        }
        if version < 4 {
            // SPEC-GAP: simulator §8 says 11 bytes; stream-helpers §2.1 and its named fields specify 10.
            r.take::<10>()?;
        }
        let julian_day = r.u32()?;
        Ok(SaveRecord {
            version: SAVE_VERSION,
            in_tick,
            city_established,
            pause_count,
            sim_speed,
            sim_time,
            frame_budget_ms,
            level_counters,
            level_fired,
            julian_day,
        })
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl Reader<'_> {
    fn take<const N: usize>(&mut self) -> Result<[u8; N], LoadError> {
        let end = self.pos.checked_add(N).ok_or(LoadError::Truncated)?;
        let slice = self.bytes.get(self.pos..end).ok_or(LoadError::Truncated)?;
        self.pos = end;
        let mut out = [0u8; N];
        out.copy_from_slice(slice);
        Ok(out)
    }
    fn u8(&mut self) -> Result<u8, LoadError> {
        Ok(self.take::<1>()?[0])
    }
    fn u16(&mut self) -> Result<u16, LoadError> {
        Ok(u16::from_le_bytes(self.take()?))
    }
    fn u32(&mut self) -> Result<u32, LoadError> {
        Ok(u32::from_le_bytes(self.take()?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_matches_size() {
        let sum: usize = RECORD_LAYOUT_V4.iter().map(|&w| w as usize).sum();
        assert_eq!(sum, RECORD_SIZE_V4);
        assert_eq!(RECORD_SIZE_V4, 79);
    }

    #[test]
    fn round_trip() {
        let mut rec = SaveRecord {
            version: 4,
            in_tick: false,
            city_established: true,
            pause_count: -2,
            sim_speed: 1,
            sim_time: 12345,
            frame_budget_ms: 40,
            level_counters: [0; LEVEL_COUNT],
            level_fired: [false; LEVEL_COUNT],
            julian_day: 2_451_600,
        };
        rec.level_counters[4] = 56;
        rec.level_fired[6] = true;
        let bytes = rec.encode();
        assert_eq!(bytes.len(), RECORD_SIZE_V4);
        assert_eq!(SaveRecord::decode(&bytes), Ok(rec));
        assert_eq!(SaveRecord::decode(&bytes[..78]), Err(LoadError::Truncated));
        let mut v5 = bytes.clone();
        v5[0] = 5;
        assert_eq!(SaveRecord::decode(&v5), Err(LoadError::UnsupportedVersion(5)));
    }

    #[test]
    fn legacy_timer_is_discarded_and_current_output_is_stable() {
        let mut current = vec![0; RECORD_SIZE_V4];
        current[0] = 4;
        let date = 2_451_600u32.to_le_bytes();
        current[RECORD_SIZE_V4 - 4..].copy_from_slice(&date);
        let mut legacy = current.clone();
        legacy[0] = 3;
        legacy.splice(RECORD_SIZE_V4 - 4..RECORD_SIZE_V4 - 4, [3, 1, 2, 3, 4, 5, 6, 7, 8, 1]);
        let record = SaveRecord::decode(&legacy).unwrap();
        assert_eq!(record.julian_day, 2_451_600);
        assert_eq!(record.encode(), current);
        assert_eq!(SaveRecord::decode(&record.encode()).unwrap(), record);
        assert_eq!(
            SaveRecord::decode(&legacy[..legacy.len() - 1]),
            Err(LoadError::Truncated)
        );
    }
}
