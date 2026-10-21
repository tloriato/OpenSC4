// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the Free
// Software Foundation, either version 3 of the License, or (at your option)
// any later version. This program is distributed WITHOUT ANY WARRANTY; see
// the GNU General Public License for more details. You should have received a
// copy of the license along with this program (see LICENSE at the repository
// root); if not, see <https://www.gnu.org/licenses/>.

//! DBPF packages and exemplars read by `fc4_formats` (data-formats §2, §4),
//! for the plug-in's formats shadow: it opens a shipped package itself and
//! compares each exemplar's own properties, as the core parses them, with the
//! ones the game's resource manager loaded for the same key.

use fc4_formats::dbpf::{FileSource, Package};
use fc4_formats::exemplar::{Exemplar, ExemplarStore, Kind, LoadedSet, Property, Values};
use fc4_formats::tgi::Tgi;

use crate::props::{fc4_props, fc4_props_create, fc4_props_destroy, fc4_props_set_typed};
use crate::{guard, slice_arg};

/// An open DBPF package (`fc4_formats::dbpf::Package` over a file).
#[repr(C)]
pub struct fc4_dbpf {
    _private: [u8; 0],
}

unsafe fn pkg_ref<'a>(p: *const fc4_dbpf) -> Option<&'a Package<FileSource>> {
    (p as *const Package<FileSource>).as_ref()
}

/// Opens the package at `path` (UTF-8 bytes, no terminator). NULL when the
/// file cannot be read or its header or index fail the checks of
/// data-formats §2.1-§2.3.
#[no_mangle]
pub unsafe extern "C" fn fc4_dbpf_open(path: *const u8, len: usize) -> *mut fc4_dbpf {
    let Some(bytes) = slice_arg(path, len) else {
        return std::ptr::null_mut();
    };
    let Ok(path) = std::str::from_utf8(bytes) else {
        return std::ptr::null_mut();
    };
    guard(std::ptr::null_mut(), || match Package::<FileSource>::open(path) {
        Ok(p) => Box::into_raw(Box::new(p)) as *mut fc4_dbpf,
        Err(_) => std::ptr::null_mut(),
    })
}

#[no_mangle]
pub unsafe extern "C" fn fc4_dbpf_close(p: *mut fc4_dbpf) {
    if !p.is_null() {
        guard((), || drop(Box::from_raw(p as *mut Package<FileSource>)));
    }
}

/// How many index entries of type `type_id` the package holds (duplicates
/// included, index order).
#[no_mangle]
pub unsafe extern "C" fn fc4_dbpf_count_type(p: *const fc4_dbpf, type_id: u32) -> usize {
    let Some(pkg) = pkg_ref(p) else { return 0 };
    guard(0, || pkg.entries_of_type(type_id).count())
}

/// The group and instance of the `index`-th entry of type `type_id` (index
/// order). False when out of range.
#[no_mangle]
pub unsafe extern "C" fn fc4_dbpf_key_of_type(
    p: *const fc4_dbpf,
    type_id: u32,
    index: usize,
    group: *mut u32,
    instance: *mut u32,
) -> bool {
    let Some(pkg) = pkg_ref(p) else { return false };
    let Some(e) = guard(None, || pkg.entries_of_type(type_id).nth(index).map(|e| e.tgi)) else {
        return false;
    };
    if let Some(g) = group.as_mut() {
        *g = e.group;
    }
    if let Some(i) = instance.as_mut() {
        *i = e.instance;
    }
    true
}

/// Native-endian bytes of a slice of plain numbers.
fn ne_bytes<T: Copy, const N: usize>(v: &[T], f: impl Fn(T) -> [u8; N]) -> Vec<u8> {
    v.iter().flat_map(|&x| f(x)).collect()
}

/// The element memory type and element bytes of `values`, as
/// `fc4_props_set_typed` takes them; `None` for the values the game holds as
/// no data or an object (Empty, a kind-1 resource): those are not compared
/// (the host's CopyToCore skips them as well).
fn typed(values: &Values) -> Option<(u16, Vec<u8>, usize)> {
    use fc4_core::variant::mem;
    Some(match values {
        Values::Empty | Values::Resource(_) => return None,
        Values::Bool(v) => (mem::BOOL, v.iter().map(|&b| u8::from(b)).collect(), v.len()),
        Values::Uint8(v) => (mem::UINT8, v.clone(), v.len()),
        Values::Sint8(v) => (mem::SINT8, v.iter().map(|&x| x as u8).collect(), v.len()),
        Values::Uint16(v) => (mem::UINT16, ne_bytes(v, u16::to_ne_bytes), v.len()),
        Values::Sint16(v) => (mem::SINT16, ne_bytes(v, i16::to_ne_bytes), v.len()),
        Values::Uint32(v) => (mem::UINT32, ne_bytes(v, u32::to_ne_bytes), v.len()),
        Values::Sint32(v) => (mem::SINT32, ne_bytes(v, i32::to_ne_bytes), v.len()),
        Values::Uint64(v) => (mem::UINT64, ne_bytes(v, u64::to_ne_bytes), v.len()),
        Values::Sint64(v) => (mem::SINT64, ne_bytes(v, i64::to_ne_bytes), v.len()),
        Values::Float32(v) => (mem::FLOAT32, ne_bytes(v, f32::to_ne_bytes), v.len()),
        Values::Float64(v) => (mem::FLOAT64, ne_bytes(v, f64::to_ne_bytes), v.len()),
        Values::String(v) => (mem::RZ_CHAR, v.clone(), v.len()),
        Values::Char(v) => (mem::CHAR, v.clone(), v.len()),
        Values::UnicodeChar(v) => (mem::UNICODE_CHAR, ne_bytes(v, u16::to_ne_bytes), v.len()),
        // The void array: memory type 0 with the array flag (fc4_props_set_typed, NG37).
        Values::Void(v) => (mem::EMPTY, v.clone(), v.len()),
    })
}

/// Reads entry (`type_id`, `group`, `instance`) (the first one in index
/// order, data-formats Q3) and parses it as an exemplar or cohort
/// (data-formats §4, `Exemplar::parse_record`): a new fc4_props with its OWN
/// properties, each with the variant type and shape the game holds
/// (`fc4_props_set_typed`), Empty and kind-1 resource properties left out.
/// `parent` (may be NULL) receives the parent cohort key (type, group,
/// instance; all zero = none). NULL when the entry is absent, fails to
/// decompress, or is not a valid exemplar of that type. The caller destroys
/// the result.
#[no_mangle]
pub unsafe extern "C" fn fc4_dbpf_read_exemplar(
    p: *const fc4_dbpf,
    type_id: u32,
    group: u32,
    instance: u32,
    parent: *mut u32,
) -> *mut fc4_props {
    let Some(pkg) = pkg_ref(p) else {
        return std::ptr::null_mut();
    };
    let tgi = Tgi {
        type_id,
        group,
        instance,
    };
    let Some(ex) = guard(None, || {
        let data = pkg.read_tgi(&tgi).ok()?;
        Exemplar::parse_record(type_id, &data).ok()
    }) else {
        return std::ptr::null_mut();
    };
    write_key(parent, ex.parent);
    props_of(ex.properties())
}

/// Writes `key` as (type, group, instance) to `out` when it is not NULL.
unsafe fn write_key(out: *mut u32, key: Tgi) {
    if !out.is_null() {
        std::slice::from_raw_parts_mut(out, 3).copy_from_slice(&[key.type_id, key.group, key.instance]);
    }
}

/// A new fc4_props holding `list` in the game's variant types (Empty and
/// kind-1 resource properties left out); NULL on failure.
unsafe fn props_of(list: &[Property]) -> *mut fc4_props {
    let props = fc4_props_create();
    if props.is_null() {
        return props;
    }
    for prop in list {
        let Some((mem_type, bytes, count)) = typed(&prop.values) else {
            continue;
        };
        if !fc4_props_set_typed(
            props,
            prop.id,
            mem_type,
            prop.stored_as_array(),
            bytes.as_ptr().cast(),
            count,
        ) {
            fc4_props_destroy(props);
            return std::ptr::null_mut();
        }
    }
    props
}

/// A set of packages combined as the resource manager combines them
/// (data-formats §3: each package added registered at the front, so a later
/// one wins) and resolved with parents and the effective list (§4.4), by
/// `fc4_formats::exemplar::ExemplarStore`.
#[repr(C)]
pub struct fc4_exstore {
    _private: [u8; 0],
}

struct ExStore {
    store: ExemplarStore,
    /// `ExemplarStore::load_all` after `fc4_exstore_resolve`.
    set: LoadedSet,
    /// The keys of `set.exemplars`, in key order.
    keys: Vec<Tgi>,
}

unsafe fn exstore_mut<'a>(p: *mut fc4_exstore) -> Option<&'a mut ExStore> {
    (p as *mut ExStore).as_mut()
}

unsafe fn exstore_ref<'a>(p: *const fc4_exstore) -> Option<&'a ExStore> {
    (p as *const ExStore).as_ref()
}

#[no_mangle]
pub extern "C" fn fc4_exstore_create() -> *mut fc4_exstore {
    let s = ExStore {
        store: ExemplarStore::new(),
        set: LoadedSet::default(),
        keys: Vec::new(),
    };
    Box::into_raw(Box::new(s)) as *mut fc4_exstore
}

#[no_mangle]
pub unsafe extern "C" fn fc4_exstore_destroy(p: *mut fc4_exstore) {
    if !p.is_null() {
        guard((), || drop(Box::from_raw(p as *mut ExStore)));
    }
}

/// Adds the package at `path` (UTF-8, no terminator) at the front of the
/// store (`ExemplarStore::load_package`). False, nothing added, when the core
/// does not open it as a DBPF package (§2.1-§2.3). `counts` (may be NULL)
/// receives exemplar records read, cohort records read, records that failed.
#[no_mangle]
pub unsafe extern "C" fn fc4_exstore_add_package(
    p: *mut fc4_exstore,
    path: *const u8,
    len: usize,
    counts: *mut u32,
) -> bool {
    let Some(s) = exstore_mut(p) else { return false };
    let Some(path) = slice_arg(path, len).and_then(|b| std::str::from_utf8(b).ok()) else {
        return false;
    };
    let Some(r) = guard(None, || {
        let pkg = Package::<FileSource>::open(path).ok()?;
        Some(s.store.load_package(&pkg))
    }) else {
        return false;
    };
    if !counts.is_null() {
        let c = [r.exemplars, r.cohorts, r.failures.len()].map(|n| n as u32);
        std::slice::from_raw_parts_mut(counts, 3).copy_from_slice(&c);
    }
    true
}

/// Loads every record of the store (`ExemplarStore::load_all`: parents
/// resolved, effective lists built, §4.4) and returns how many loaded; the
/// keys are then indexed 0..n in key order. `failed` (may be NULL) receives
/// how many did not load.
#[no_mangle]
pub unsafe extern "C" fn fc4_exstore_resolve(p: *mut fc4_exstore, failed: *mut usize) -> usize {
    let Some(s) = exstore_mut(p) else { return 0 };
    guard((), || {
        s.set = s.store.load_all();
        s.keys = s.set.exemplars.keys().copied().collect();
    });
    if let Some(f) = failed.as_mut() {
        *f = s.set.failures.len();
    }
    s.keys.len()
}

/// The key (type, group, instance) of loaded record `index` and its kind
/// (0 exemplar, 1 cohort); -1 when out of range.
#[no_mangle]
pub unsafe extern "C" fn fc4_exstore_key(p: *const fc4_exstore, index: usize, key: *mut u32) -> i32 {
    let Some(k) = exstore_ref(p).and_then(|s| s.keys.get(index).copied()) else {
        return -1;
    };
    write_key(key, k);
    match exstore_ref(p).and_then(|s| s.set.exemplars.get(&k)).map(|l| l.kind()) {
        Some(Kind::Cohort) => 1,
        Some(_) => 0,
        None => -1,
    }
}

/// Loaded record `index`: a new fc4_props of its effective list (§4.4) when
/// `effective`, else of its own properties, in the game's variant types
/// (Empty and kind-1 resource properties left out). `total` (may be NULL)
/// receives the list's full length, those left out included; `parent` (may be
/// NULL) the parent cohort key. NULL when out of range.
#[no_mangle]
pub unsafe extern "C" fn fc4_exstore_props(
    p: *const fc4_exstore,
    index: usize,
    effective: bool,
    total: *mut usize,
    parent: *mut u32,
) -> *mut fc4_props {
    let Some(l) = exstore_ref(p).and_then(|s| s.set.exemplars.get(s.keys.get(index)?)) else {
        return std::ptr::null_mut();
    };
    let list = if effective { l.properties() } else { l.own_properties() };
    if let Some(t) = total.as_mut() {
        *t = list.len();
    }
    write_key(parent, l.parent());
    props_of(list)
}

/// The IDs of loaded record `index`'s effective (or own) properties that
/// `fc4_exstore_props` leaves out (Empty values and kind-1 resources), up to
/// `cap` written to `ids`; returns how many there are.
#[no_mangle]
pub unsafe extern "C" fn fc4_exstore_untyped_ids(
    p: *const fc4_exstore,
    index: usize,
    effective: bool,
    ids: *mut u32,
    cap: usize,
) -> usize {
    let Some(l) = exstore_ref(p).and_then(|s| s.set.exemplars.get(s.keys.get(index)?)) else {
        return 0;
    };
    let list = if effective { l.properties() } else { l.own_properties() };
    let untyped: Vec<u32> = list
        .iter()
        .filter(|q| typed(&q.values).is_none())
        .map(|q| q.id)
        .collect();
    if !ids.is_null() {
        let n = untyped.len().min(cap);
        std::slice::from_raw_parts_mut(ids, n).copy_from_slice(&untyped[..n]);
    }
    untyped.len()
}

/// Failed record `index` (key order): its key, and the reason as text
/// (`LoadError`'s Display, truncated, NUL-terminated) in `buf`. False when out
/// of range.
#[no_mangle]
pub unsafe extern "C" fn fc4_exstore_failure(
    p: *const fc4_exstore,
    index: usize,
    key: *mut u32,
    buf: *mut u8,
    cap: usize,
) -> bool {
    let Some((k, e)) = exstore_ref(p).and_then(|s| s.set.failures.get(index)) else {
        return false;
    };
    write_key(key, *k);
    if !buf.is_null() && cap > 0 {
        let text = guard(String::new(), || e.to_string());
        let n = text.len().min(cap - 1);
        let out = std::slice::from_raw_parts_mut(buf, cap);
        out[..n].copy_from_slice(&text.as_bytes()[..n]);
        out[n] = 0;
    }
    true
}

/// One exemplar or cohort record parsed for the plug-in factory (data-formats
/// §4.1, §4.2, §8.1 SerialRead): the record bytes as the manager hands them
/// (position 0, already decompressed; no QFS here), checked against the type
/// it is stored under. Unlike `fc4_dbpf_read_exemplar` every property is kept,
/// in record order (duplicates included: the holder's add replaces, §4.5),
/// with Empty and kind-1 resource-key properties reported by kind.
#[repr(C)]
pub struct fc4_exrec {
    _private: [u8; 0],
}

/// Parses `data` (`len` bytes) as a record stored under `type_id`
/// (`Exemplar::parse_record`). NULL when the read fails. Destroy with
/// `fc4_exrec_destroy`.
#[no_mangle]
pub unsafe extern "C" fn fc4_exrec_parse(type_id: u32, data: *const u8, len: usize) -> *mut fc4_exrec {
    let Some(bytes) = slice_arg(data, len) else {
        return std::ptr::null_mut();
    };
    match guard(None, || Exemplar::parse_record(type_id, bytes).ok()) {
        Some(ex) => Box::into_raw(Box::new(ex)).cast(),
        None => std::ptr::null_mut(),
    }
}

/// Frees a record from `fc4_exrec_parse` (NULL is ignored).
#[no_mangle]
pub unsafe extern "C" fn fc4_exrec_destroy(p: *mut fc4_exrec) {
    if !p.is_null() {
        drop(Box::from_raw(p.cast::<Exemplar>()));
    }
}

unsafe fn exrec_ref<'a>(p: *const fc4_exrec) -> Option<&'a Exemplar> {
    (p as *const Exemplar).as_ref()
}

/// The parent cohort key (type, group, instance) into `key3`; all zero = none.
#[no_mangle]
pub unsafe extern "C" fn fc4_exrec_parent(p: *const fc4_exrec, key3: *mut u32) -> bool {
    let Some(ex) = exrec_ref(p) else {
        return false;
    };
    write_key(key3, ex.parent);
    true
}

/// The number of properties in record order.
#[no_mangle]
pub unsafe extern "C" fn fc4_exrec_count(p: *const fc4_exrec) -> usize {
    exrec_ref(p).map_or(0, |ex| ex.properties().len())
}

/// Property `index` in record order. Returns its kind: 0 a value (`mem_type`,
/// `is_array`, `data`, `count` as `fc4_props_set_typed` takes them; `data`
/// borrowed until the record is destroyed), 1 Empty (no value), 2 a resource
/// key (`key3`), -1 out of range. Pointers may be NULL.
#[no_mangle]
pub unsafe extern "C" fn fc4_exrec_prop(
    p: *const fc4_exrec,
    index: usize,
    id: *mut u32,
    mem_type: *mut u16,
    is_array: *mut bool,
    data: *mut *const u8,
    count: *mut usize,
    key3: *mut u32,
) -> i32 {
    let Some(prop) = exrec_ref(p).and_then(|ex| ex.properties().get(index)) else {
        return -1;
    };
    if !id.is_null() {
        *id = prop.id;
    }
    match &prop.values {
        Values::Empty => 1,
        Values::Resource(key) => {
            write_key(key3, *key);
            2
        }
        values => {
            let (mt, ptr, n) = raw_of(values);
            if !mem_type.is_null() {
                *mem_type = mt;
            }
            if !is_array.is_null() {
                *is_array = prop.stored_as_array();
            }
            if !data.is_null() {
                *data = ptr;
            }
            if !count.is_null() {
                *count = n;
            }
            0
        }
    }
}

/// The memory type, a pointer to the native-endian elements and the element
/// count of a value (the same mapping as `typed`, without copying).
fn raw_of(values: &Values) -> (u16, *const u8, usize) {
    use fc4_core::variant::mem;
    fn p<T>(v: &[T]) -> (*const u8, usize) {
        (v.as_ptr().cast(), v.len())
    }
    let (mt, (ptr, n)) = match values {
        Values::Empty | Values::Resource(_) => (mem::EMPTY, (std::ptr::null(), 0)),
        Values::Bool(v) => (mem::BOOL, p(v)),
        Values::Uint8(v) => (mem::UINT8, p(v)),
        Values::Sint8(v) => (mem::SINT8, p(v)),
        Values::Uint16(v) => (mem::UINT16, p(v)),
        Values::Sint16(v) => (mem::SINT16, p(v)),
        Values::Uint32(v) => (mem::UINT32, p(v)),
        Values::Sint32(v) => (mem::SINT32, p(v)),
        Values::Uint64(v) => (mem::UINT64, p(v)),
        Values::Sint64(v) => (mem::SINT64, p(v)),
        Values::Float32(v) => (mem::FLOAT32, p(v)),
        Values::Float64(v) => (mem::FLOAT64, p(v)),
        Values::String(v) => (mem::RZ_CHAR, p(v)),
        Values::Char(v) => (mem::CHAR, p(v)),
        Values::UnicodeChar(v) => (mem::UNICODE_CHAR, p(v)),
        Values::Void(v) => (mem::EMPTY, p(v)),
    };
    (mt, ptr, n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::props::props_ref;
    use fc4_core::variant::{Elem, Variant};
    use fc4_formats::dbpf::{Compression, PackageBuilder};
    use fc4_formats::exemplar::{Kind, Property};

    #[test]
    fn opens_a_package_and_reads_own_properties() {
        let mut ex = Exemplar::new(Kind::Exemplar);
        ex.parent = Tgi {
            type_id: 0x0534_2861,
            group: 1,
            instance: 2,
        };
        ex.set(Property::single(0x10, Values::Uint32(vec![7])));
        ex.set(Property::array(0x20, Values::Float32(vec![1.5, 2.5])));
        ex.set(Property::single(0x30, Values::Empty));
        let key = Tgi {
            type_id: 0x6534_284A,
            group: 3,
            instance: 4,
        };
        let mut b = PackageBuilder::new();
        b.add(key, &ex.to_binary(), Compression::Always).unwrap();
        let dir = std::env::temp_dir().join(format!("fc4_dbpf_capi_{}.dat", std::process::id()));
        std::fs::write(&dir, b.build().unwrap()).unwrap();
        let path = dir.to_str().unwrap().as_bytes();
        unsafe {
            let p = fc4_dbpf_open(path.as_ptr(), path.len());
            assert!(!p.is_null());
            assert_eq!(fc4_dbpf_count_type(p, key.type_id), 1);
            let (mut g, mut i) = (0, 0);
            assert!(fc4_dbpf_key_of_type(p, key.type_id, 0, &mut g, &mut i));
            assert_eq!((g, i), (3, 4));
            assert!(!fc4_dbpf_key_of_type(p, key.type_id, 1, &mut g, &mut i));
            let mut parent = [0u32; 3];
            let props = fc4_dbpf_read_exemplar(p, key.type_id, 3, 4, parent.as_mut_ptr());
            assert!(!props.is_null());
            assert_eq!(parent, [0x0534_2861, 1, 2]);
            let h = props_ref(props).unwrap();
            assert_eq!(h.get(0x10), Some(&Variant::Scalar(Elem::U32(7))));
            assert!(matches!(h.get(0x20), Some(Variant::Array { items, .. }) if items.len() == 2));
            assert!(h.get(0x30).is_none());
            fc4_props_destroy(props);
            assert!(fc4_dbpf_read_exemplar(p, key.type_id, 9, 9, std::ptr::null_mut()).is_null());
            fc4_dbpf_close(p);
        }
        let _ = std::fs::remove_file(dir);
        assert!(unsafe { fc4_dbpf_open(b"no/such.dat".as_ptr(), 11) }.is_null());
    }

    fn write_pkg(name: &str, records: &[(Tgi, &Exemplar)]) -> std::path::PathBuf {
        let mut b = PackageBuilder::new();
        for (k, ex) in records {
            b.add(*k, &ex.to_binary(), Compression::None).unwrap();
        }
        let path = std::env::temp_dir().join(format!("fc4_exstore_{}_{name}.dat", std::process::id()));
        std::fs::write(&path, b.build().unwrap()).unwrap();
        path
    }

    #[test]
    fn store_combines_packages_and_builds_effective_lists() {
        let ck = Tgi::new(0x0534_2861, 1, 2);
        let ek = Tgi::new(0x6534_284A, 3, 4);
        let orphan = Tgi::new(0x6534_284A, 3, 5);
        let mut cohort = Exemplar::new(Kind::Cohort);
        cohort.set(Property::single(0x10, Values::Uint32(vec![2])));
        cohort.set(Property::single(0x20, Values::Uint32(vec![5])));
        let mut ex = Exemplar::new(Kind::Exemplar);
        ex.parent = ck;
        ex.set(Property::single(0x20, Values::Uint32(vec![9])));
        ex.set(Property::single(0x30, Values::Empty));
        let mut lost = Exemplar::new(Kind::Exemplar);
        lost.parent = Tgi::new(0x0534_2861, 7, 7);
        // A later package overrides the cohort's 0x10 (it is added at the front).
        let mut cohort2 = cohort.clone();
        cohort2.set(Property::single(0x10, Values::Uint32(vec![3])));
        let a = write_pkg("a", &[(ck, &cohort), (ek, &ex), (orphan, &lost)]);
        let b = write_pkg("b", &[(ck, &cohort2)]);
        unsafe {
            let s = fc4_exstore_create();
            let mut counts = [0u32; 3];
            let pa = a.to_str().unwrap().as_bytes();
            assert!(fc4_exstore_add_package(s, pa.as_ptr(), pa.len(), counts.as_mut_ptr()));
            assert_eq!(counts, [2, 1, 0]);
            let pb = b.to_str().unwrap().as_bytes();
            assert!(fc4_exstore_add_package(s, pb.as_ptr(), pb.len(), std::ptr::null_mut()));
            assert!(!fc4_exstore_add_package(
                s,
                b"no/such.dat".as_ptr(),
                11,
                std::ptr::null_mut()
            ));
            let mut failed = 0;
            assert_eq!(fc4_exstore_resolve(s, &mut failed), 2);
            assert_eq!(failed, 1);
            let mut key = [0u32; 3];
            assert_eq!(fc4_exstore_key(s, 0, key.as_mut_ptr()), 1);
            assert_eq!(key, [0x0534_2861, 1, 2]);
            assert_eq!(fc4_exstore_key(s, 1, key.as_mut_ptr()), 0);
            assert_eq!(fc4_exstore_key(s, 2, key.as_mut_ptr()), -1);
            let (mut total, mut parent) = (0, [0u32; 3]);
            let eff = fc4_exstore_props(s, 1, true, &mut total, parent.as_mut_ptr());
            assert_eq!((total, parent), (3, [0x0534_2861, 1, 2]));
            let h = props_ref(eff).unwrap();
            assert_eq!(h.get(0x10), Some(&Variant::Scalar(Elem::U32(3))));
            assert_eq!(h.get(0x20), Some(&Variant::Scalar(Elem::U32(9))));
            assert!(h.get(0x30).is_none());
            fc4_props_destroy(eff);
            let own = fc4_exstore_props(s, 1, false, &mut total, std::ptr::null_mut());
            assert_eq!(total, 2);
            assert!(props_ref(own).unwrap().get(0x10).is_none());
            fc4_props_destroy(own);
            let mut ids = [0u32; 4];
            assert_eq!(fc4_exstore_untyped_ids(s, 1, true, ids.as_mut_ptr(), 4), 1);
            assert_eq!(ids[0], 0x30);
            let mut buf = [0u8; 64];
            assert!(fc4_exstore_failure(s, 0, key.as_mut_ptr(), buf.as_mut_ptr(), buf.len()));
            assert_eq!(key, [0x6534_284A, 3, 5]);
            assert!(buf[0] != 0 && buf.contains(&0));
            assert!(!fc4_exstore_failure(
                s,
                1,
                key.as_mut_ptr(),
                buf.as_mut_ptr(),
                buf.len()
            ));
            fc4_exstore_destroy(s);
        }
        let _ = std::fs::remove_file(a);
        let _ = std::fs::remove_file(b);
    }

    #[test]
    fn exrec_keeps_empty_and_record_order() {
        let mut rec = b"EQZB1###".to_vec();
        for v in [0x0534_2861u32, 1, 2, 2] {
            rec.extend_from_slice(&v.to_le_bytes());
        }
        rec.extend_from_slice(&0x10u32.to_le_bytes());
        rec.push(0);
        rec.extend_from_slice(&0u32.to_le_bytes());
        rec.extend_from_slice(&0x11u32.to_le_bytes());
        rec.push(0);
        rec.extend_from_slice(&3u32.to_le_bytes());
        rec.extend_from_slice(&7u32.to_le_bytes());
        unsafe {
            assert!(fc4_exrec_parse(0x0534_2861, rec.as_ptr(), rec.len()).is_null());
            let p = fc4_exrec_parse(0x6534_284A, rec.as_ptr(), rec.len());
            assert!(!p.is_null());
            let mut key = [9u32; 3];
            assert!(fc4_exrec_parent(p, key.as_mut_ptr()));
            assert_eq!(key, [0x0534_2861, 1, 2]);
            assert_eq!(fc4_exrec_count(p), 2);
            let (mut id, mut mt, mut arr, mut n) = (0u32, 0u16, true, 0usize);
            let mut data: *const u8 = std::ptr::null();
            let null = std::ptr::null_mut();
            assert_eq!(
                fc4_exrec_prop(p, 0, &mut id, &mut mt, &mut arr, &mut data, &mut n, null),
                1
            );
            assert_eq!(id, 0x10);
            assert_eq!(
                fc4_exrec_prop(p, 1, &mut id, &mut mt, &mut arr, &mut data, &mut n, null),
                0
            );
            assert_eq!((id, mt, arr, n), (0x11, fc4_core::variant::mem::UINT32, false, 1));
            assert_eq!(std::ptr::read_unaligned(data.cast::<u32>()), 7);
            assert_eq!(
                fc4_exrec_prop(p, 2, &mut id, null.cast(), null.cast(), null.cast(), null.cast(), null),
                -1
            );
            fc4_exrec_destroy(p);
        }
    }
}
