// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The class-object store of a city save (`docs/specs/foundations.md` §3.3 to
//! §3.5): object references are resolved through it, and every referenced
//! object is written once into the record of its class.
//!
//! In plug-in mode the game runs the store; this implementation serves tests
//! (round trips of our records) and standalone mode.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::rc::Rc;

use crate::stream::{ByteReader, ByteWriter, InStream, OutStream, RefResolver, RefValue, SaveObject, SaveRegistry};

pub mod archive;

/// Group of every class record and of the index (§3.2).
pub const STORE_GROUP: u32 = 0x299B_2D1B;
/// Type of the index record (§3.2).
pub const INDEX_TYPE: u32 = 0x499B_23FE;
/// Bytes covered by an entry's CRC at most (§3.4).
pub const CRC_LIMIT: usize = 250_000;
/// More entries than this in one class fail the class (§3.5).
pub const MAX_OBJECTS_PER_CLASS: u32 = 9_999_999;

/// CRC-32/MPEG-2: polynomial 0x04C11DB7, not reflected, init 0xFFFFFFFF, no
/// final XOR (§3.4).
pub fn crc32_mpeg2(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &b in data {
        crc = (crc << 8) ^ CRC_TABLE[((crc >> 24) ^ u32::from(b)) as usize];
    }
    crc
}

const CRC_TABLE: [u32; 256] = {
    let mut table = [0; 256];
    let mut i = 0;
    while i < 256 {
        let mut crc = (i as u32) << 24;
        let mut bit = 0;
        while bit < 8 {
            crc = if crc & 0x8000_0000 != 0 {
                (crc << 1) ^ 0x04C1_1DB7
            } else {
                crc << 1
            };
            bit += 1;
        }
        table[i] = crc;
        i += 1;
    }
    table
};

/// A package: records keyed by (type, group, instance).
pub type Package = BTreeMap<(u32, u32, u32), Vec<u8>>;

/// Result of [`save_store`].
#[derive(Debug, Default)]
pub struct SaveReport {
    pub entries_written: u32,
    pub entries_discarded: u32,
    pub aborted: bool,
}

/// Saves every object reachable from `roots` (already registered through
/// `registry`, e.g. by writing a root record with references) into `pkg`,
/// following §3.5: passes over classes ascending; in each pass, the objects
/// unwritten at the start of the pass in ascending ID; objects registered
/// during a pass are written in a later pass and appended.
pub fn save_store(registry: &Rc<RefCell<SaveRegistry>>, pkg: &mut Package) -> SaveReport {
    let mut report = SaveReport::default();
    let mut writers: BTreeMap<u32, ByteWriter> = BTreeMap::new();
    let mut written: BTreeSet<(u32, u32)> = BTreeSet::new();
    let mut registered_per_class: BTreeMap<u32, u32> = BTreeMap::new();

    loop {
        // Snapshot the unwritten objects at the start of the pass.
        let pending: Vec<(u32, u32, Rc<dyn SaveObject>)> = {
            let reg = registry.borrow();
            let mut v: Vec<_> = reg
                .entries
                .iter()
                .filter(|(c, id, o, _)| o.is_some() && !written.contains(&(*c, *id)))
                .map(|(c, id, o, _)| (*c, *id, o.clone().unwrap()))
                .collect();
            v.sort_by_key(|(c, id, _)| (*c, *id));
            v
        };
        if pending.is_empty() {
            break;
        }
        for (class, id, obj) in pending {
            if !registered_per_class.contains_key(&class) {
                // First registration of a class in this save: start a fresh record.
                pkg.remove(&(class, STORE_GROUP, 0));
            }
            let w = writers
                .entry(class)
                .or_insert_with(|| ByteWriter::with_registry(registry.clone()));
            let p = w.len();
            w.put_u32(0);
            w.put_u32(0);
            w.put_u32(id);
            let ok = obj.write(w);
            if ok && w.is_good() {
                let entry_size = (w.len() - p) as u32;
                let span = ((entry_size as usize) - 8).min(CRC_LIMIT);
                let crc = crc32_mpeg2(&w.bytes()[p + 8..p + 8 + span]);
                w.patch_u32(p, entry_size);
                w.patch_u32(p + 4, crc);
                report.entries_written += 1;
            } else {
                w.truncate(p);
                report.entries_discarded += 1;
            }
            written.insert((class, id));
            *registered_per_class.entry(class).or_insert(0) += 0;
            if !w.is_good() {
                report.aborted = true;
                return report;
            }
        }
    }

    // objectCount = distinct objects registered per class (§3.3), host
    // objects included (their host writes their records).
    for (class, _, _, _) in registry.borrow().entries.iter() {
        *registered_per_class.entry(*class).or_insert(0) += 1;
    }
    for (class, w) in writers {
        pkg.insert((class, STORE_GROUP, 0), w.into_bytes());
    }
    if registered_per_class.is_empty() {
        return report; // nothing registered: no index
    }
    let mut idx = ByteWriter::new();
    idx.put_u32(1);
    idx.put_u32(registered_per_class.len() as u32);
    idx.put_i32(report.entries_written as i32);
    for (class, count) in &registered_per_class {
        idx.put_u32(*class);
        idx.put_u32(*count);
    }
    pkg.insert((INDEX_TYPE, STORE_GROUP, 0), idx.into_bytes());
    report
}

/// Creates a fresh object for `(classID, objectID)` while loading (the host's
/// `GZCOM()->GetClassObject(classID, GZIID_cIGZSerializable)`). Core objects
/// come back as `RefValue::Object`, host objects as `RefValue::Host`.
pub type Factory<'a> = dyn FnMut(u32, u32) -> Option<RefValue> + 'a;

/// Load-side registry (§2.7 "Reading").
pub struct LoadRegistry<'a> {
    pub index: BTreeMap<u32, u32>,
    pub objects: HashMap<(u32, u32), RefValue>,
    factory: Box<Factory<'a>>,
}

impl<'a> LoadRegistry<'a> {
    pub fn new(index: BTreeMap<u32, u32>, factory: Box<Factory<'a>>) -> Self {
        LoadRegistry {
            index,
            objects: HashMap::new(),
            factory,
        }
    }

    pub fn count_for_class(&self, class: u32) -> u32 {
        self.objects.keys().filter(|(c, _)| *c == class).count() as u32
    }
}

impl RefResolver for LoadRegistry<'_> {
    fn resolve(&mut self, class_id: u32, object_id: u32) -> Option<RefValue> {
        if let Some(o) = self.objects.get(&(class_id, object_id)) {
            return Some(o.clone());
        }
        if !self.index.contains_key(&class_id) {
            return None; // class not in the save's index: failure
        }
        let obj = (self.factory)(class_id, object_id)?;
        self.objects.insert((class_id, object_id), obj.clone());
        Some(obj)
    }
}

/// Result of [`load_store`].
#[derive(Debug, Default, PartialEq, Eq)]
pub struct LoadReport {
    /// The index was readable and had version 1.
    pub index_ok: bool,
    /// Classes that failed (§3.5); [choice] reported per class.
    pub failed_classes: Vec<u32>,
    pub entries_read: u32,
    pub entries_skipped: u32,
}

/// Loads the store (§3.5). Objects are created through `registry`'s factory
/// and read in ascending class ID; references resolve through the same
/// registry, so a referenced object may be created before it is read.
pub fn load_store(pkg: &Package, registry: &mut LoadRegistry<'_>) -> LoadReport {
    let mut report = LoadReport::default();
    let Some(index) = pkg.get(&(INDEX_TYPE, STORE_GROUP, 0)) else {
        return report;
    };
    {
        let mut r = ByteReader::new(index);
        let (Some(version), Some(class_count), Some(_entries)) = (r.get_u32(), r.get_u32(), r.get_i32()) else {
            return report;
        };
        if version != 1 {
            return report;
        }
        for _ in 0..class_count {
            let (Some(c), Some(n)) = (r.get_u32(), r.get_u32()) else {
                return report;
            };
            registry.index.insert(c, n);
        }
    }
    report.index_ok = true;

    let classes: Vec<(u32, u32)> = registry.index.iter().map(|(c, n)| (*c, *n)).collect();
    for (class, count) in classes {
        let Some(record) = pkg.get(&(class, STORE_GROUP, 0)) else {
            continue;
        };
        if count > MAX_OBJECTS_PER_CLASS {
            report.failed_classes.push(class);
            continue;
        }
        let mut failed = false;
        let mut pos = 0usize;
        for _ in 0..count {
            // Entry framing; the CRC check does not move the position.
            if record.len() < pos + 12 {
                failed = true;
                break;
            }
            let entry_size = u32::from_le_bytes(record[pos..pos + 4].try_into().unwrap()) as usize;
            let crc = u32::from_le_bytes(record[pos + 4..pos + 8].try_into().unwrap());
            // [choice] reject entries smaller than 12 bytes or past the record end.
            if entry_size < 12 || pos + entry_size > record.len() {
                failed = true;
                break;
            }
            let span = (entry_size - 8).min(CRC_LIMIT);
            if crc32_mpeg2(&record[pos + 8..pos + 8 + span]) != crc {
                failed = true;
                break;
            }
            let object_id = u32::from_le_bytes(record[pos + 8..pos + 12].try_into().unwrap());
            let next = pos + entry_size;
            match registry.resolve(class, object_id) {
                Some(RefValue::Object(obj)) => {
                    let payload = &record[pos + 12..next];
                    let mut r = ByteReader::with_resolver(payload, registry);
                    let _ = obj.read(&mut r); // result ignored (§3.5)
                    report.entries_read += 1;
                }
                _ => report.entries_skipped += 1,
            }
            pos = next;
        }
        if failed || registry.count_for_class(class) != count {
            report.failed_classes.push(class);
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::any::Any;
    use std::cell::Cell;

    #[test]
    fn crc_check_values() {
        assert_eq!(crc32_mpeg2(b"123456789"), 0x0376_E6E7);
        assert_eq!(crc32_mpeg2(b""), 0xFFFF_FFFF);
        assert_eq!(crc32_mpeg2(&[1, 0, 0, 0, 1, 0]), 0xF34B_70D6);
    }

    /// A test object: a u16 payload and one optional reference.
    struct Node {
        class: u32,
        value: Cell<u16>,
        next: RefCell<RefValue>,
    }

    impl SaveObject for Node {
        fn class_id(&self) -> u32 {
            self.class
        }
        fn write(&self, out: &mut dyn OutStream) -> bool {
            out.put_u16(self.value.get()) && out.put_ref(&self.next.borrow()) && out.error() == 0
        }
        fn read(&self, inp: &mut dyn InStream) -> bool {
            let Some(v) = inp.get_u16() else { return false };
            self.value.set(v);
            let Some(r) = inp.get_ref() else { return false };
            *self.next.borrow_mut() = r;
            true
        }
        fn as_any_rc(self: Rc<Self>) -> Rc<dyn Any> {
            self
        }
    }

    fn node(class: u32, v: u16) -> Rc<Node> {
        Rc::new(Node {
            class,
            value: Cell::new(v),
            next: RefCell::new(RefValue::Null),
        })
    }

    #[test]
    fn class_record_entry_bytes() {
        // An object with ID 1 whose Write emits 01 00 (§7 item 7).
        struct One;
        impl SaveObject for One {
            fn class_id(&self) -> u32 {
                0x1234_5678
            }
            fn write(&self, out: &mut dyn OutStream) -> bool {
                out.put_bytes(&[1, 0])
            }
            fn read(&self, _: &mut dyn InStream) -> bool {
                true
            }
            fn as_any_rc(self: Rc<Self>) -> Rc<dyn Any> {
                self
            }
        }
        let reg = Rc::new(RefCell::new(SaveRegistry::new()));
        reg.borrow_mut().register(&RefValue::Object(Rc::new(One)));
        let mut pkg = Package::new();
        let rep = save_store(&reg, &mut pkg);
        assert_eq!(rep.entries_written, 1);
        assert_eq!(
            pkg[&(0x1234_5678, STORE_GROUP, 0)],
            vec![0x0E, 0, 0, 0, 0xD6, 0x70, 0x4B, 0xF3, 1, 0, 0, 0, 1, 0]
        );
        assert_eq!(
            pkg[&(INDEX_TYPE, STORE_GROUP, 0)],
            vec![1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0x78, 0x56, 0x34, 0x12, 1, 0, 0, 0]
        );
    }

    #[test]
    fn save_order_and_cycles_round_trip() {
        // a (class 2) -> b (class 1) -> a : a cycle across classes.
        let a = node(2, 10);
        let b = node(1, 20);
        *a.next.borrow_mut() = RefValue::Object(b.clone());
        *b.next.borrow_mut() = RefValue::Object(a.clone());
        let reg = Rc::new(RefCell::new(SaveRegistry::new()));
        // The "root record" refers to a.
        let mut root = ByteWriter::with_registry(reg.clone());
        root.put_ref(&RefValue::Object(a.clone()));
        let mut pkg = Package::new();
        let rep = save_store(&reg, &mut pkg);
        assert_eq!((rep.entries_written, rep.entries_discarded, rep.aborted), (2, 0, false));
        // a got ID 1 (registered first), b ID 2 (registered while writing a, so in pass 2).
        let rec_a = &pkg[&(2, STORE_GROUP, 0)];
        let rec_b = &pkg[&(1, STORE_GROUP, 0)];
        assert_eq!(&rec_a[8..12], &[1, 0, 0, 0]);
        assert_eq!(&rec_b[8..12], &[2, 0, 0, 0]);

        // Load into fresh objects; the cycle resolves to one instance each.
        let mut factory =
            |c: u32, _: u32| -> Option<RefValue> { Some(RefValue::Object(node(c, 0) as Rc<dyn SaveObject>)) };
        let mut lreg = LoadRegistry::new(BTreeMap::new(), Box::new(&mut factory));
        let lrep = load_store(&pkg, &mut lreg);
        assert!(lrep.index_ok && lrep.failed_classes.is_empty());
        assert_eq!(lrep.entries_read, 2);
        let la = lreg.objects[&(2, 1)].object::<Node>().unwrap();
        let lb = lreg.objects[&(1, 2)].object::<Node>().unwrap();
        assert_eq!((la.value.get(), lb.value.get()), (10, 20));
        assert!(Rc::ptr_eq(&la.next.borrow().object::<Node>().unwrap(), &lb));
        assert!(Rc::ptr_eq(&lb.next.borrow().object::<Node>().unwrap(), &la));

        // Write -> load -> write is byte-identical.
        let reg2 = Rc::new(RefCell::new(SaveRegistry::new()));
        let mut root2 = ByteWriter::with_registry(reg2.clone());
        root2.put_ref(&RefValue::Object(la.clone()));
        let mut pkg2 = Package::new();
        save_store(&reg2, &mut pkg2);
        assert_eq!(pkg, pkg2);
    }

    #[test]
    fn crc_mismatch_fails_only_that_class() {
        let a = node(2, 10);
        let b = node(1, 20);
        let reg = Rc::new(RefCell::new(SaveRegistry::new()));
        reg.borrow_mut().register(&RefValue::Object(a));
        reg.borrow_mut().register(&RefValue::Object(b));
        let mut pkg = Package::new();
        save_store(&reg, &mut pkg);
        pkg.get_mut(&(1, STORE_GROUP, 0)).unwrap()[12] ^= 0xFF; // corrupt class 1's payload
        let mut factory =
            |c: u32, _: u32| -> Option<RefValue> { Some(RefValue::Object(node(c, 0) as Rc<dyn SaveObject>)) };
        let mut lreg = LoadRegistry::new(BTreeMap::new(), Box::new(&mut factory));
        let lrep = load_store(&pkg, &mut lreg);
        assert_eq!(lrep.failed_classes, vec![1]);
        assert_eq!(lrep.entries_read, 1);
    }

    #[test]
    fn unknown_class_reference_fails_the_stream() {
        let data = [5, 0, 0, 0, 0x2D, 0xC1, 0x90, 0x89];
        let mut factory =
            |c: u32, _: u32| -> Option<RefValue> { Some(RefValue::Object(node(c, 0) as Rc<dyn SaveObject>)) };
        let mut lreg = LoadRegistry::new(BTreeMap::new(), Box::new(&mut factory));
        let mut r = ByteReader::with_resolver(&data, &mut lreg);
        assert!(r.get_ref().is_none());
        assert_ne!(r.error(), 0);
        // With the class in the index it resolves, and twice to the same instance.
        let mut idx = BTreeMap::new();
        idx.insert(0x8990_C12D, 1);
        let mut factory2 =
            |c: u32, _: u32| -> Option<RefValue> { Some(RefValue::Object(node(c, 0) as Rc<dyn SaveObject>)) };
        let mut lreg2 = LoadRegistry::new(idx, Box::new(&mut factory2));
        let twice = [data, data].concat();
        let mut r = ByteReader::with_resolver(&twice, &mut lreg2);
        let x = r.get_ref().unwrap().object::<Node>().unwrap();
        let y = r.get_ref().unwrap().object::<Node>().unwrap();
        assert!(Rc::ptr_eq(&x, &y));
    }

    #[test]
    fn reference_bytes() {
        let reg = Rc::new(RefCell::new(SaveRegistry::new()));
        let mut w = ByteWriter::with_registry(reg);
        w.put_ref(&RefValue::Null);
        let n = node(0x8990_C12D, 0);
        for _ in 0..4 {
            w.put_ref(&RefValue::Host {
                token: 99,
                class_id: 0x1,
            });
        }
        w.put_ref(&RefValue::Object(n));
        // null, then host object ID 1 (x4, same ID), then n gets ID 2.
        assert_eq!(&w.bytes()[0..4], &[0, 0, 0, 0]);
        assert_eq!(&w.bytes()[4..12], &[1, 0, 0, 0, 1, 0, 0, 0]);
        assert_eq!(&w.bytes()[28..36], &[1, 0, 0, 0, 1, 0, 0, 0]);
        assert_eq!(&w.bytes()[36..44], &[2, 0, 0, 0, 0x2D, 0xC1, 0x90, 0x89]);
    }
}
