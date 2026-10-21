// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The save sweep for the class records FreeCity4 reads and writes
//! (commercial-industrial §12 test 9, civic-buildings §12 test 10, parks §12
//! test 8, prop-developer.md §7, city-detail.md §4): every class-record entry
//! of those classes in every distinct city save is read into a fresh object
//! over the C ABI and written again.
//!
//! - commercial (`0x8990C080`), industrial (`0xA990C022`), parks (`0xAA3FE6BC`)
//!   and the city detail manager (`0xEAD4BA36`): the re-write must be
//!   byte-identical;
//! - civic (`0x0A413481`): the re-write must equal the record with every
//!   multi-entry bucket chain of the exclusions and the statuses reversed
//!   (civic §7, QUESTIONS CB6), and a second read → write must give the
//!   record back byte-identically;
//! - the prop developer (`0x89C48F47`): `Write` always emits version 5, so a
//!   corpus version-4 record is written back converted (and that conversion
//!   must itself be stable) and a version-5 record byte-identically.
//!
//! Records are found as foundations §3.2 says: the class record
//! `(classID, 0x299B2D1B, 0)`, entries `u32 entrySize, u32 crc, u32 objectID,
//! payload` (§3.3). References (§2.7: `u32 objectID`, then `u32 classID` when
//! the ID is not 0) are read as the token `classID << 32 | objectID` and
//! written back from it, so the sweep needs no other record of the save.
//!
//! Saves: every `.sc4` below `FC4_SAVES_DIR` (as `fc4-formats`'s record mode,
//! `formats/tests/save_records.rs`; `FC4_SAVE_DIR` also works), else
//! `FC4_GAME_DIR`, else the
//! default GOG install (read-only; identical files counted once). The test
//! passes with a note when the directory does not exist. Run with output:
//!
//! ```text
//! cargo test -p fc4-capi --release save_sweep -- --nocapture
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::c_void;
use std::path::{Path, PathBuf};

use fc4_formats::{Package, Tgi};

use crate::commercial::*;
use crate::stream::{fc4_bytes_sink, fc4_istream, fc4_ostream, fc4_ref};

pub(crate) const CLSID_COMMERCIAL: u32 = 0x8990_C080;
pub(crate) const CLSID_INDUSTRIAL: u32 = 0xA990_C022;
pub(crate) const CLSID_CIVIC: u32 = 0x0A41_3481;
pub(crate) const CLSID_PARKS: u32 = 0xAA3F_E6BC;
/// The prop developer (prop-developer.md §7; the glue21 lane).
pub(crate) const CLSID_PROP_DEVELOPER: u32 = 0x89C4_8F47;
/// The city detail manager (city-detail.md §4).
pub(crate) const CLSID_CITY_DETAIL: u32 = 0xEAD4_BA36;
const CLASS_GROUP: u32 = 0x299B_2D1B;

// ---- the save's stream encoding ---------------------------------------------------------------------

struct SaveIn<'a> {
    bytes: &'a [u8],
    pos: usize,
}

fn in_of(u: *mut c_void) -> &'static mut SaveIn<'static> {
    unsafe { &mut *(u as *mut SaveIn<'static>) }
}
fn take<const N: usize>(u: *mut c_void) -> Option<[u8; N]> {
    let i = in_of(u);
    let b: [u8; N] = i.bytes.get(i.pos..i.pos + N)?.try_into().ok()?;
    i.pos += N;
    Some(b)
}
macro_rules! get {
    ($name:ident, $t:ty, $n:expr) => {
        unsafe extern "C" fn $name(u: *mut c_void, v: *mut $t) -> bool {
            take::<$n>(u).map(|b| *v = <$t>::from_le_bytes(b)).is_some()
        }
    };
}
get!(get_u8, u8, 1);
get!(get_u16, u16, 2);
get!(get_u32, u32, 4);
get!(get_u64, u64, 8);
get!(get_f32, f32, 4);
get!(get_f64, f64, 8);
unsafe extern "C" fn get_bytes(u: *mut c_void, p: *mut u8, n: usize) -> bool {
    let i = in_of(u);
    let Some(s) = i.bytes.get(i.pos..i.pos + n) else {
        return false;
    };
    if n > 0 {
        std::ptr::copy_nonoverlapping(s.as_ptr(), p, n);
    }
    i.pos += n;
    true
}
/// Strings (foundations §2): `u32` length, then the bytes.
unsafe extern "C" fn get_str(u: *mut c_void, sink: fc4_bytes_sink, ctx: *mut c_void) -> bool {
    let Some(n) = take::<4>(u).map(u32::from_le_bytes) else {
        return false;
    };
    let i = in_of(u);
    let Some(s) = i.bytes.get(i.pos..i.pos + n as usize) else {
        return false;
    };
    i.pos += n as usize;
    if let Some(f) = sink {
        f(ctx, s.as_ptr(), s.len());
    }
    true
}
/// A reference (§2.7) as the token `classID << 32 | objectID`.
unsafe extern "C" fn get_ref(u: *mut c_void, _iid: u32, out: *mut fc4_ref) -> bool {
    let Some(id) = take::<4>(u).map(u32::from_le_bytes) else {
        return false;
    };
    let mut r = fc4_ref::default();
    if id != 0 {
        let Some(class) = take::<4>(u).map(u32::from_le_bytes) else {
            return false;
        };
        r.token = (u64::from(class) << 32) | u64::from(id);
        r.class_id = class;
    }
    *out = r;
    true
}
unsafe extern "C" fn skip(u: *mut c_void, n: usize) -> bool {
    let i = in_of(u);
    if i.pos + n > i.bytes.len() {
        return false;
    }
    i.pos += n;
    true
}
unsafe extern "C" fn no_error(_: *mut c_void) -> i32 {
    0
}

fn save_istream(i: &mut SaveIn) -> fc4_istream {
    fc4_istream {
        struct_size: std::mem::size_of::<fc4_istream>() as u32,
        user: i as *mut SaveIn as *mut c_void,
        get_u8: Some(get_u8),
        get_u16: Some(get_u16),
        get_u32: Some(get_u32),
        get_u64: Some(get_u64),
        get_f32: Some(get_f32),
        get_f64: Some(get_f64),
        get_bytes: Some(get_bytes),
        get_str: Some(get_str),
        get_ref: Some(get_ref),
        skip: Some(skip),
        error: Some(no_error),
    }
}

fn out_of(u: *mut c_void) -> &'static mut Vec<u8> {
    unsafe { &mut *(u as *mut Vec<u8>) }
}
macro_rules! put {
    ($name:ident, $t:ty) => {
        unsafe extern "C" fn $name(u: *mut c_void, v: $t) -> bool {
            out_of(u).extend_from_slice(&v.to_le_bytes());
            true
        }
    };
}
put!(put_u8, u8);
put!(put_u16, u16);
put!(put_u32, u32);
put!(put_u64, u64);
put!(put_f32, f32);
put!(put_f64, f64);
unsafe extern "C" fn put_bytes(u: *mut c_void, p: *const u8, n: usize) -> bool {
    if n > 0 {
        out_of(u).extend_from_slice(std::slice::from_raw_parts(p, n));
    }
    true
}
unsafe extern "C" fn put_str(u: *mut c_void, p: *const u8, n: usize) -> bool {
    put_u32(u, n as u32);
    put_bytes(u, p, n)
}
/// The token back to `objectID` (and `classID` when the ID is not 0).
unsafe extern "C" fn put_ref(u: *mut c_void, r: *const fc4_ref) -> bool {
    let t = (*r).token;
    let o = out_of(u);
    o.extend_from_slice(&(t as u32).to_le_bytes());
    if t as u32 != 0 {
        o.extend_from_slice(&((t >> 32) as u32).to_le_bytes());
    }
    true
}

fn save_ostream(o: &mut Vec<u8>) -> fc4_ostream {
    fc4_ostream {
        struct_size: std::mem::size_of::<fc4_ostream>() as u32,
        user: o as *mut Vec<u8> as *mut c_void,
        put_u8: Some(put_u8),
        put_u16: Some(put_u16),
        put_u32: Some(put_u32),
        put_u64: Some(put_u64),
        put_f32: Some(put_f32),
        put_f64: Some(put_f64),
        put_bytes: Some(put_bytes),
        put_str: Some(put_str),
        put_ref: Some(put_ref),
        error: Some(no_error),
    }
}

/// Reads `rec` with `read`, then writes with `write`. Every byte must be
/// consumed (the record parses exactly).
pub(crate) fn read_write(
    rec: &[u8],
    read: impl FnOnce(*const fc4_istream) -> bool,
    write: impl FnOnce(*const fc4_ostream) -> bool,
) -> Result<Vec<u8>, String> {
    let mut i = SaveIn { bytes: rec, pos: 0 };
    let is = save_istream(&mut i);
    if !read(&is) {
        return Err("Read returned false".into());
    }
    if i.pos != rec.len() {
        return Err(format!("Read consumed {} of {} bytes", i.pos, rec.len()));
    }
    let mut out = Vec::new();
    let os = save_ostream(&mut out);
    if !write(&os) {
        return Err("Write returned false".into());
    }
    Ok(out)
}

/// The tokens of the references at the start of `rec[from..]`, in order.
pub(crate) fn ref_tokens(rec: &[u8], from: usize, n: usize) -> Vec<u64> {
    let mut v = Vec::new();
    let mut p = from;
    let u32_at = |p: usize| rec.get(p..p + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap()));
    for _ in 0..n {
        let Some(id) = u32_at(p) else { break };
        p += 4;
        if id != 0 {
            let Some(class) = u32_at(p) else { break };
            p += 4;
            v.push((u64::from(class) << 32) | u64::from(id));
        }
    }
    v
}

// ---- the expected civic re-write (CB6) --------------------------------------------------------------

/// `rec` with every multi-entry bucket chain (`key mod 193`) of the
/// exclusions and the statuses reversed; `None` when the record does not
/// parse that far or a container holds 194 or more entries (the table then
/// grows while reading). Also returns the number of reversed chains.
fn civic_chains_reversed(rec: &[u8]) -> Option<(Vec<u8>, usize)> {
    let u32_at = |p: usize| rec.get(p..p + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap()));
    let mut p = 2;
    let n = u32_at(p)? as usize;
    p += 4;
    if n >= 194 {
        return None;
    }
    let mut ex = Vec::new();
    for _ in 0..n {
        ex.push((u32_at(p)?, rec.get(p..p + 4)?.to_vec()));
        p += 4;
    }
    let m = u32_at(p)? as usize;
    let ex_end = p;
    p += 4;
    if m >= 194 {
        return None;
    }
    let mut st = Vec::new();
    for _ in 0..m {
        let key = u32_at(p)?;
        let len = u32_at(p + 7)? as usize;
        let end = p + 11 + len;
        st.push((key, rec.get(p..end)?.to_vec()));
        p = end;
    }
    let mut chains = 0;
    let mut out = rec[..6].to_vec();
    for list in [ex, st] {
        let mut i = 0;
        while i < list.len() {
            let mut j = i;
            while j < list.len() && list[j].0 % 193 == list[i].0 % 193 {
                j += 1;
            }
            if j - i > 1 {
                chains += 1;
            }
            for e in list[i..j].iter().rev() {
                out.extend_from_slice(&e.1);
            }
            i = j;
        }
        if out.len() == ex_end {
            out.extend_from_slice(&rec[ex_end..ex_end + 4]);
        }
    }
    out.extend_from_slice(&rec[p..]);
    Some((out, chains))
}

// ---- the sweep --------------------------------------------------------------------------------------

/// What the prop developer must write back for a corpus record
/// (prop-developer.md §7).
///
/// `Write` always emits version 5. A version-5 record round-trips byte for
/// byte. A version-4 record (the 122-byte shipped template: it stops after the
/// power list) comes back with the version word set to 5 and the five bytes of
/// the v5 tail appended - `signsVisible` (the read leaves it 1) and an empty
/// sign list. Any other version is None: `Read` rejects it (version < 4) or
/// reads it as 5, and neither is a corpus shape.
fn prop_developer_expected(rec: &[u8]) -> Option<Vec<u8>> {
    let version = u16::from_le_bytes([*rec.first()?, *rec.get(1)?]);
    match version {
        5 => Some(rec.to_vec()),
        4 => {
            let mut out = rec.to_vec();
            out[0] = 5;
            out[1] = 0;
            // signsVisible = 1, then the sign list's count (0).
            out.extend_from_slice(&[1, 0, 0, 0, 0]);
            Some(out)
        }
        _ => None,
    }
}

const DEFAULT_DIR: &str = r"C:\Program Files (x86)\GOG Galaxy\Games\SimCity 4 Deluxe Edition";

fn save_dir() -> Option<PathBuf> {
    let dir = std::env::var_os("FC4_SAVES_DIR")
        .or_else(|| std::env::var_os("FC4_SAVE_DIR"))
        .or_else(|| std::env::var_os("FC4_GAME_DIR"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_DIR));
    dir.is_dir().then_some(dir)
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut items: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
    items.sort();
    for p in items {
        if p.is_dir() {
            collect(&p, out);
        } else if p
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("sc4"))
        {
            out.push(p);
        }
    }
}

fn fnv64(b: &[u8]) -> u64 {
    let mut h = 0xCBF2_9CE4_8422_2325u64;
    for &x in b {
        h = (h ^ u64::from(x)).wrapping_mul(0x0100_0000_01B3);
    }
    h
}

/// Class-record entries (foundations §3.3): (objectID, payload).
fn entries(rec: &[u8]) -> Result<Vec<(u32, &[u8])>, String> {
    let mut v = Vec::new();
    let mut p = 0;
    while p < rec.len() {
        let size = rec
            .get(p..p + 4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()) as usize)
            .ok_or("truncated entry header")?;
        if size < 12 || p + size > rec.len() {
            return Err(format!("entry size {size} at {p}"));
        }
        let id = u32::from_le_bytes(rec[p + 8..p + 12].try_into().unwrap());
        v.push((id, &rec[p + 12..p + size]));
        p += size;
    }
    Ok(v)
}

fn round_trip(class: u32, payload: &[u8]) -> Result<Vec<u8>, String> {
    match class {
        CLSID_COMMERCIAL => crate::commercial_tests::sweep_round_trip(payload),
        CLSID_INDUSTRIAL => unsafe {
            let p = fc4_industrial_create();
            let r = read_write(payload, |i| fc4_industrial_read(p, i), |o| fc4_industrial_write(p, o));
            fc4_industrial_destroy(p);
            r
        },
        CLSID_CIVIC => crate::civic_tests::sweep_round_trip(payload),
        CLSID_PARKS => crate::parks_tests::sweep_round_trip(payload),
        CLSID_PROP_DEVELOPER => crate::prop_developer_tests::sweep_round_trip(payload),
        CLSID_CITY_DETAIL => crate::city_detail_tests::sweep_round_trip(payload),
        _ => Err(format!("class {class:08X}: no sweep")),
    }
}

#[derive(Default)]
struct Tally {
    records: usize,
    identical: usize,
    /// civic: the re-write equals the record with its chains reversed (not identical).
    reversed: usize,
    chains: usize,
    /// civic: a second round trip gives the record back.
    twice: usize,
    /// prop developer: a version-4 record written back as version 5.
    converted: usize,
    /// a save with no record of this class (the city detail manager is only
    /// written by a city that ran; the shipped templates have none).
    missing: usize,
    sizes: BTreeMap<usize, usize>,
    failures: Vec<String>,
}

#[test]
fn save_sweep_class_records() {
    let Some(dir) = save_dir() else {
        eprintln!("save sweep: no save directory (FC4_SAVES_DIR / FC4_GAME_DIR / the GOG default); skipped");
        return;
    };
    let mut files = Vec::new();
    collect(&dir, &mut files);
    let classes = [
        ("commercial", CLSID_COMMERCIAL, true),
        ("industrial", CLSID_INDUSTRIAL, true),
        ("civic", CLSID_CIVIC, true),
        ("parks", CLSID_PARKS, true),
        ("prop-developer", CLSID_PROP_DEVELOPER, true),
        // city-detail.md §4: only a city that ran and saved carries one.
        ("city-detail", CLSID_CITY_DETAIL, false),
    ];
    let mut seen = BTreeSet::new();
    let mut distinct = 0;
    let mut tallies: BTreeMap<&str, Tally> = BTreeMap::new();
    let mut open_failures = Vec::new();
    for f in &files {
        let Ok(bytes) = std::fs::read(f) else {
            open_failures.push(format!("{}: unreadable", f.display()));
            continue;
        };
        if !seen.insert(fnv64(&bytes)) {
            continue;
        }
        distinct += 1;
        let name = f.strip_prefix(&dir).unwrap_or(f).display().to_string();
        let pkg = match Package::from_bytes(bytes) {
            Ok(p) => p,
            Err(e) => {
                open_failures.push(format!("{name}: {e}"));
                continue;
            }
        };
        for (label, class, required) in classes {
            let t = tallies.entry(label).or_default();
            let Ok(rec) = pkg.read_tgi(&Tgi::new(class, CLASS_GROUP, 0)) else {
                if required {
                    t.failures.push(format!("{name}: no class record"));
                } else {
                    t.missing += 1;
                }
                continue;
            };
            let list = match entries(&rec) {
                Ok(l) => l,
                Err(e) => {
                    t.failures.push(format!("{name}: {e}"));
                    continue;
                }
            };
            for (id, payload) in list {
                t.records += 1;
                *t.sizes.entry(payload.len()).or_default() += 1;
                let once = match round_trip(class, payload) {
                    Ok(b) => b,
                    Err(e) => {
                        t.failures.push(format!("{name} object {id:08X}: {e}"));
                        continue;
                    }
                };
                if class == CLSID_PROP_DEVELOPER {
                    // The only class whose re-write is allowed to differ:
                    // version 4 -> version 5 (prop_developer_expected).
                    match prop_developer_expected(payload) {
                        Some(expected) if once == expected => {
                            if once == payload {
                                t.identical += 1;
                            } else {
                                t.converted += 1;
                            }
                        }
                        Some(_) => t.failures.push(format!(
                            "{name} object {id:08X}: re-write is not the record as version 5"
                        )),
                        None => t
                            .failures
                            .push(format!("{name} object {id:08X}: unexpected record version")),
                    }
                    // The conversion must be stable: reading what we wrote and
                    // writing it again gives the same bytes back.
                    match round_trip(class, &once) {
                        Ok(b) if b == once => t.twice += 1,
                        Ok(_) => t
                            .failures
                            .push(format!("{name} object {id:08X}: second round trip differs")),
                        Err(e) => t
                            .failures
                            .push(format!("{name} object {id:08X}: second round trip: {e}")),
                    }
                    continue;
                }
                if class != CLSID_CIVIC {
                    if once == payload {
                        t.identical += 1;
                    } else {
                        t.failures.push(format!("{name} object {id:08X}: re-write differs"));
                    }
                    continue;
                }
                match civic_chains_reversed(payload) {
                    Some((expected, chains)) if once == expected => {
                        if chains == 0 {
                            t.identical += 1;
                        } else {
                            t.reversed += 1;
                            t.chains += chains;
                        }
                    }
                    Some(_) => t.failures.push(format!(
                        "{name} object {id:08X}: re-write is not the record with its chains reversed"
                    )),
                    None => t
                        .failures
                        .push(format!("{name} object {id:08X}: 194 or more entries (not checked)")),
                }
                match round_trip(class, &once) {
                    Ok(b) if b == payload => t.twice += 1,
                    Ok(_) => t
                        .failures
                        .push(format!("{name} object {id:08X}: second round trip differs")),
                    Err(e) => t
                        .failures
                        .push(format!("{name} object {id:08X}: second round trip: {e}")),
                }
            }
        }
    }
    eprintln!(
        "save sweep: {} .sc4 files below {}, {} distinct",
        files.len(),
        dir.display(),
        distinct
    );
    for f in &open_failures {
        eprintln!("save sweep: {f}");
    }
    let mut failed = !open_failures.is_empty();
    for (label, _, _) in classes {
        let t = tallies.remove(label).unwrap_or_default();
        let sizes: Vec<String> = t.sizes.iter().map(|(s, n)| format!("{s} B x{n}")).collect();
        eprintln!(
            "save sweep {label}: records={} identical={} converted={} chains-reversed={} (chains {}) second-trip-identical={} missing={} failures={}; sizes: {}",
            t.records,
            t.identical,
            t.converted,
            t.reversed,
            t.chains,
            t.twice,
            t.missing,
            t.failures.len(),
            sizes.join(", ")
        );
        for f in t.failures.iter().take(20) {
            eprintln!("  {f}");
        }
        failed |= !t.failures.is_empty();
    }
    assert!(!failed, "save sweep: failures above");
}

#[test]
fn civic_chain_reversal_helper() {
    // Exclusions 1 and 194 share bucket 1; 2 is alone. Statuses 1, 194.
    let mut rec = vec![6, 0];
    rec.extend(3u32.to_le_bytes());
    for k in [194u32, 1, 2] {
        rec.extend(k.to_le_bytes());
    }
    rec.extend(2u32.to_le_bytes());
    for k in [194u32, 1] {
        rec.extend(k.to_le_bytes());
        rec.extend([0, 1, 0]);
        rec.extend(1u32.to_le_bytes());
        rec.push(b'x');
    }
    rec.extend([0u8; 12]);
    let (out, chains) = civic_chains_reversed(&rec).unwrap();
    assert_eq!(chains, 2);
    assert_eq!(out.len(), rec.len());
    let mut want = vec![6, 0];
    want.extend(3u32.to_le_bytes());
    for k in [1u32, 194, 2] {
        want.extend(k.to_le_bytes());
    }
    want.extend(2u32.to_le_bytes());
    for k in [1u32, 194] {
        want.extend(k.to_le_bytes());
        want.extend([0, 1, 0]);
        want.extend(1u32.to_le_bytes());
        want.push(b'x');
    }
    want.extend([0u8; 12]);
    assert_eq!(out, want);
    // The save encoding: a null reference is one u32, a reference two.
    let mut r = vec![1, 0, 1];
    r.extend(0u32.to_le_bytes());
    r.extend(7u32.to_le_bytes());
    r.extend(0x49B9_E602u32.to_le_bytes());
    assert_eq!(ref_tokens(&r, 3, 2), vec![(0x49B9_E602u64 << 32) | 7]);
}
