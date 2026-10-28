// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! Properties and property holders in saves, `docs/specs/foundations.md` §10:
//! the plain holder ("property holder record") and the exemplar holder with
//! its parent exemplar.

use std::collections::BTreeMap;
use std::rc::Rc;

use crate::stream::{InStream, OutStream, ResKey};
use crate::variant::Variant;

pub mod archive;

pub const HOLDER_VERSION: u16 = 2;

/// A plain property holder: at most one property per ID, kept in ascending
/// unsigned ID order (§10.2).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PropertyHolder {
    props: BTreeMap<u32, Variant>,
}

impl PropertyHolder {
    pub fn new() -> PropertyHolder {
        PropertyHolder::default()
    }

    /// `AddProperty`: replaces an existing property with the same ID.
    pub fn add(&mut self, id: u32, value: Variant) {
        self.props.insert(id, value);
    }

    pub fn get(&self, id: u32) -> Option<&Variant> {
        self.props.get(&id)
    }

    pub fn has(&self, id: u32) -> bool {
        self.props.contains_key(&id)
    }

    pub fn remove(&mut self, id: u32) -> bool {
        self.props.remove(&id).is_some()
    }

    pub fn clear(&mut self) {
        self.props.clear();
    }

    pub fn len(&self) -> usize {
        self.props.len()
    }

    pub fn is_empty(&self) -> bool {
        self.props.is_empty()
    }

    /// Property IDs in holder order.
    pub fn ids(&self) -> Vec<u32> {
        self.props.keys().copied().collect()
    }

    /// Write (§10.2): `key` = the property ID [choice].
    pub fn write(&self, out: &mut dyn OutStream) -> bool {
        out.put_u16(HOLDER_VERSION);
        out.put_u32(self.props.len() as u32);
        for (&id, v) in &self.props {
            out.put_u32(id); // key
            out.put_u32(id); // property ID
            if !v.write(out) {
                return false;
            }
        }
        out.error() == 0
    }

    /// Read (§10.2). A version other than 2 returns false with the stream
    /// still good and only the version consumed.
    pub fn read(&mut self, inp: &mut dyn InStream) -> bool {
        let Some(version) = inp.get_u16() else { return false };
        if version != HOLDER_VERSION {
            return false;
        }
        let Some(count) = inp.get_u32() else { return false };
        self.props.clear();
        for _ in 0..count {
            if inp.error() != 0 {
                break; // every later read fails anyway (§2.1)
            }
            let _key = inp.get_u32();
            let Some(id) = inp.get_u32() else { break };
            let Some(v) = Variant::read(inp) else { break };
            let keep = match &v {
                Variant::Empty => false,
                // [compat] every object-valued entry is dropped (foundations
                // §10.2, Errata round 2): a non-null one by the IGZUnknown rule;
                // a null one reads into a fresh Empty value (§9.2) and is
                // dropped by the "not Empty" rule.
                Variant::Object(_) => false,
                _ => true,
            };
            if keep {
                self.props.insert(id, v);
            }
        }
        inp.error() == 0
    }
}

/// Looks up exemplars by key (the resource manager, `cISCResExemplar`).
pub trait ExemplarSource {
    fn exemplar(&self, key: ResKey) -> Option<Rc<PropertyHolder>>;
}

/// An exemplar holder: own properties plus an optional parent (§10.3).
#[derive(Clone, Debug, Default)]
pub struct ExemplarHolder {
    pub own: PropertyHolder,
    pub parent: Option<(ResKey, Rc<PropertyHolder>)>,
}

impl ExemplarHolder {
    /// Own properties first, then the parent's.
    pub fn get(&self, id: u32) -> Option<&Variant> {
        self.own
            .get(id)
            .or_else(|| self.parent.as_ref().and_then(|(_, p)| p.get(id)))
    }

    pub fn has(&self, id: u32) -> bool {
        self.get(id).is_some()
    }

    pub fn parent_key(&self) -> Option<ResKey> {
        self.parent.as_ref().map(|(k, _)| *k)
    }

    /// Write: version, the own properties, then the parent key as group,
    /// type, instance (zeros = no parent). [choice] The extra parent copy of
    /// §10.3 ("write side extra") is not written yet.
    pub fn write(&self, out: &mut dyn OutStream) -> bool {
        out.put_u16(HOLDER_VERSION);
        if !self.own.write(out) {
            return false;
        }
        let k = self.parent_key().unwrap_or(ResKey {
            type_id: 0,
            group: 0,
            instance: 0,
        });
        out.put_u32(k.group);
        out.put_u32(k.type_id);
        out.put_u32(k.instance);
        out.error() == 0
    }

    /// Read (§10.3). A missing parent leaves the current parent unchanged.
    pub fn read(&mut self, inp: &mut dyn InStream, source: &dyn ExemplarSource) -> bool {
        let Some(version) = inp.get_u16() else { return false };
        if version != HOLDER_VERSION || !self.own.read(inp) {
            return false;
        }
        let (Some(group), Some(type_id), Some(instance)) = (inp.get_u32(), inp.get_u32(), inp.get_u32()) else {
            return false;
        };
        if group == 0 && type_id == 0 && instance == 0 {
            self.parent = None;
        } else {
            let key = ResKey {
                type_id,
                group,
                instance,
            };
            if let Some(p) = source.exemplar(key) {
                self.parent = Some((key, p));
            }
        }
        inp.error() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::{ByteReader, ByteWriter};
    use crate::variant::Elem;

    fn hex(s: &str) -> Vec<u8> {
        let c: String = s.chars().filter(|c| !c.is_whitespace()).collect();
        (0..c.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&c[i..i + 2], 16).unwrap())
            .collect()
    }

    #[test]
    fn example_c_round_trips_in_id_order() {
        let mut h = PropertyHolder::new();
        h.add(0x8000_0001, Variant::Scalar(Elem::F32(1.5)));
        h.add(0x10, Variant::Scalar(Elem::U8(41)));
        let mut w = ByteWriter::new();
        assert!(h.write(&mut w));
        let want = hex("02 00 02 00 00 00
             10 00 00 00 10 00 00 00 00 00 00 00 01 00 00 00 29
             01 00 00 80 01 00 00 80 00 00 00 00 09 00 00 00 00 00 C0 3F");
        assert_eq!(w.bytes(), want.as_slice());
        let mut back = PropertyHolder::new();
        let mut r = ByteReader::new(&want);
        assert!(back.read(&mut r));
        assert_eq!(back, h);
    }

    #[test]
    fn wrong_version_consumes_two_bytes() {
        let b = hex("03 00 00 00 00 00");
        let mut r = ByteReader::new(&b);
        let mut h = PropertyHolder::new();
        h.add(1, Variant::Scalar(Elem::U8(1)));
        assert!(!h.read(&mut r));
        assert_eq!(r.position(), 2);
        assert_eq!(r.error(), 0);
        assert!(h.has(1), "nothing removed");
    }
}
