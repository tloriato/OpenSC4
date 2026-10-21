// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! `cGZCommandParameterSet` (command-system.md §1).

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use super::{RefCount, ValueConvert};
use crate::variant::{Elem, Variant};

/// The status parameter (§1.5, the SDK's `kStatusParameterID`).
pub const STATUS_PARAMETER_ID: u32 = 0x2B99_9D2B;
/// `CompareParameterString`'s "any parameter" ID, −2 (§1.4).
pub const ANY_PARAMETER_ID: u32 = 0xFFFF_FFFE;
/// The end of iteration and the first ID of an empty set (§1.3).
pub const NO_PARAMETER_ID: u32 = 0xFFFF_FFFF;
/// The IIDs `QueryInterface` answers (slot 0).
pub const PARAMETER_SET_IIDS: [u32; 3] = [0x4B8C_39E4, 0x4B8C_39E5, 1];

/// A shared parameter set, as the generator, the dispatcher and handlers hold it.
pub type SetRef = Rc<RefCell<CommandParameterSet>>;

/// A sparse map from parameter ID to variant, iterated in ascending unsigned ID order (§1.1).
#[derive(Debug)]
pub struct CommandParameterSet {
    refs: RefCount,
    map: BTreeMap<u32, Variant>,
    /// Handed out by `operator[]` for every missing ID, writable, never reset (§1.4) *[quirk]*.
    fallback: Variant,
}

impl Default for CommandParameterSet {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandParameterSet {
    /// The constructor: count 0, empty map, `fallback` Empty (§1.2).
    pub fn new() -> Self {
        Self {
            refs: RefCount::default(),
            map: BTreeMap::new(),
            fallback: Variant::Empty,
        }
    }

    /// A new set wrapped for sharing, count 0.
    pub fn new_ref() -> SetRef {
        Rc::new(RefCell::new(Self::new()))
    }

    /// A new set holding one reference, the caller's (server slot 5, §3.4 step 6).
    pub fn new_owned() -> SetRef {
        let s = Self::new_ref();
        s.borrow_mut().add_ref();
        s
    }

    pub fn ref_count(&self) -> u32 {
        self.refs.count()
    }

    pub fn is_destroyed(&self) -> bool {
        self.refs.is_destroyed()
    }

    /// Slot 0: true (and one more reference) for the set's IIDs.
    pub fn query_interface(&mut self, iid: u32) -> bool {
        if PARAMETER_SET_IIDS.contains(&iid) {
            self.add_ref();
            true
        } else {
            false
        }
    }

    /// Slot 1.
    pub fn add_ref(&mut self) -> u32 {
        self.refs.add_ref()
    }

    /// Slot 2 (§1.2). On destruction the destructor runs: `fallback` and every value are dropped.
    pub fn release(&mut self) -> u32 {
        let n = self.refs.release();
        if self.refs.is_destroyed() {
            self.fallback = Variant::Empty;
            self.map.clear();
        }
        n
    }

    /// Slot 3: the number of parameters, the status parameter included.
    pub fn get_parameter_count(&self) -> u32 {
        self.map.len() as u32
    }

    /// Slot 4 (§1.4): grow with Empty appends, then shrink by removing the highest IDs.
    pub fn set_parameter_count(&mut self, n: u32) {
        while self.get_parameter_count() < n {
            let before = self.get_parameter_count();
            self.append_parameter(&Variant::Empty);
            // [choice] (§1.4): the original loops forever when an append wraps onto an
            // existing ID 0; FreeCity4 stops growing when an append does not raise the count.
            if self.get_parameter_count() == before {
                break;
            }
        }
        while self.get_parameter_count() > n {
            let Some((&top, _)) = self.map.iter().next_back() else {
                break;
            };
            self.map.remove(&top);
        }
    }

    /// Slot 5, `operator[]`: the value under `id`, or the shared writable `fallback` (never
    /// null; nothing is inserted) *[quirk]*.
    pub fn get_or_fallback_mut(&mut self, id: u32) -> &mut Variant {
        match self.map.get_mut(&id) {
            Some(v) => v,
            None => &mut self.fallback,
        }
    }

    /// Slot 5 for readers.
    pub fn get_or_fallback(&self, id: u32) -> &Variant {
        self.map.get(&id).unwrap_or(&self.fallback)
    }

    /// Slot 6: the value under `id`, or `None` (null).
    pub fn get_parameter(&self, id: u32) -> Option<&Variant> {
        self.map.get(&id)
    }

    /// Slot 6 for writers.
    pub fn get_parameter_mut(&mut self, id: u32) -> Option<&mut Variant> {
        self.map.get_mut(&id)
    }

    /// Slot 7: find or insert `id`, then copy `v` into it. An object value is shared (one
    /// more reference), everything else duplicated.
    pub fn set_parameter(&mut self, id: u32, v: &Variant) {
        self.map.insert(id, v.clone());
    }

    /// Slot 8: `SetParameter(k, v)` with `k` = 0 for an empty set, else the highest ID + 1,
    /// wrapping to 0 (§1.4) *[quirk]*.
    pub fn append_parameter(&mut self, v: &Variant) {
        let k = match self.map.keys().next_back() {
            Some(&top) => top.wrapping_add(1),
            None => 0,
        };
        self.set_parameter(k, v);
    }

    /// Slot 9: removes `id`; a missing ID does nothing.
    pub fn remove_parameter(&mut self, id: u32) {
        self.map.remove(&id);
    }

    /// Slot 10: `SetParameter(0x2B999D2B, Sint32 status)` (§1.5).
    pub fn set_status_parameter_value(&mut self, status: i32) {
        self.set_parameter(STATUS_PARAMETER_ID, &Variant::Scalar(Elem::I32(status)));
    }

    /// Slot 11 (§1.4).
    pub fn compare_parameter_string(
        &self,
        id: u32,
        text: &[u8],
        case_sensitive: bool,
        conv: &dyn ValueConvert,
    ) -> bool {
        if id == ANY_PARAMETER_ID {
            // [choice] GC5: every parameter in ID order, ID 0xFFFFFFFF included; a parameter
            // stored under 0xFFFFFFFE itself is compared directly (the original would recurse).
            return self
                .map
                .keys()
                .any(|&k| self.compare_one(k, text, case_sensitive, conv));
        }
        self.compare_one(id, text, case_sensitive, conv)
    }

    fn compare_one(&self, id: u32, text: &[u8], case_sensitive: bool, conv: &dyn ValueConvert) -> bool {
        let Some(v) = self.map.get(&id) else { return false };
        let mut s = Vec::new();
        if !conv.get_val_string(v, &mut s) {
            return false;
        }
        if case_sensitive {
            s == text
        } else {
            make_lower(&s) == make_lower(text)
        }
    }

    /// Slot 12: true only when `id` is present **and** `kind = 0xFF` *[quirk]* (GC2).
    pub fn validate_parameter(&self, id: u32, kind: u32) -> bool {
        self.map.contains_key(&id) && kind == 0xFF
    }

    /// Slot 13: the lowest ID, or `0xFFFFFFFF`.
    pub fn get_first_parameter_id(&self) -> u32 {
        self.map.keys().next().copied().unwrap_or(NO_PARAMETER_ID)
    }

    /// Slot 14: the next higher ID after a **present** `prev`; a missing `prev` or the highest
    /// ID gives `0xFFFFFFFF` *[quirk]*.
    pub fn get_next_parameter_id(&self, prev: u32) -> u32 {
        if prev == NO_PARAMETER_ID || !self.map.contains_key(&prev) {
            return NO_PARAMETER_ID;
        }
        self.map.range(prev + 1..).next().map_or(NO_PARAMETER_ID, |(&k, _)| k)
    }

    /// Every parameter in ascending ID order (what the slot 13/14 loops of §3.6 visit; a
    /// parameter under `0xFFFFFFFF` ends such a loop and is not visited, §1.4).
    pub fn iter_by_slots(&self) -> impl Iterator<Item = (u32, &Variant)> + '_ {
        self.map
            .iter()
            .map(|(&k, v)| (k, v))
            .take_while(|&(k, _)| k != NO_PARAMETER_ID)
    }

    /// All IDs, `0xFFFFFFFF` included (for tests and diagnostics).
    pub fn ids(&self) -> Vec<u32> {
        self.map.keys().copied().collect()
    }
}

/// `cRZString::MakeLower` as FreeCity4 does it (graphics-formats.md §3.11 *[choice]*): ASCII
/// letters lowered byte by byte; for valid UTF-8 with non-ASCII bytes, non-ASCII letters also
/// get Unicode simple case mapping (a letter whose lower case is several characters is kept).
pub fn make_lower(s: &[u8]) -> Vec<u8> {
    if s.is_ascii() {
        return s.to_ascii_lowercase();
    }
    match std::str::from_utf8(s) {
        Ok(t) => {
            let mut out = String::with_capacity(t.len());
            for c in t.chars() {
                let mut l = c.to_lowercase();
                match (l.next(), l.next()) {
                    (Some(one), None) => out.push(one),
                    _ => out.push(c),
                }
            }
            out.into_bytes()
        }
        Err(_) => s.to_ascii_lowercase(),
    }
}
