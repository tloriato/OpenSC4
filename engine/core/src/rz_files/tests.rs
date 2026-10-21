//! In-memory host tests for cRZFile / cRZIniFile (framework-rz-files.md §2-§4).

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use super::file::*;
use super::host::*;
use super::ini::*;

#[derive(Default)]
struct MemHost {
    files: BTreeMap<Vec<u8>, Vec<u8>>,
    handles: BTreeMap<OsHandle, (Vec<u8>, u32, u32)>,
    next: OsHandle,
    lnk: BTreeMap<Vec<u8>, Vec<u8>>,
    os_reads: usize,
    os_writes: usize,
    fail_writes: bool,
    deny_opens: u32,
    micros: i64,
    sleeps: u32,
}

impl MemHost {
    fn with(path: &[u8], data: &[u8]) -> Self {
        let mut h = MemHost::default();
        h.files.insert(path.to_vec(), data.to_vec());
        h
    }
    fn data(&self, path: &[u8]) -> &[u8] {
        &self.files[path]
    }
}

impl RzFileHost for MemHost {
    fn open(&mut self, path: &[u8], access: u32, creation: u32, _share: u32) -> Result<(OsHandle, bool), OsError> {
        let existed = self.files.contains_key(path);
        if self.deny_opens > 0 && existed {
            self.deny_opens -= 1;
            return Err(32);
        }
        match (creation, existed) {
            (CREATE_NEW, true) => return Err(OS_ERROR_ALREADY_EXISTS),
            (OPEN_EXISTING | TRUNCATE_EXISTING, false) => return Err(2),
            (CREATE_ALWAYS | TRUNCATE_EXISTING, _) | (_, false) => {
                self.files.insert(path.to_vec(), Vec::new());
            }
            _ => {}
        }
        self.next += 1;
        self.handles.insert(self.next, (path.to_vec(), 0, access));
        Ok((self.next, existed))
    }
    fn close(&mut self, h: OsHandle) -> Result<(), OsError> {
        self.handles.remove(&h).map(|_| ()).ok_or(6)
    }
    fn read(&mut self, h: OsHandle, buf: &mut [u8]) -> Result<usize, OsError> {
        self.os_reads += 1;
        let (path, pos, _) = self.handles.get_mut(&h).ok_or(6u32)?;
        let data = &self.files[path.as_slice()];
        let start = (*pos as usize).min(data.len());
        let k = (data.len() - start).min(buf.len());
        buf[..k].copy_from_slice(&data[start..start + k]);
        *pos += k as u32;
        Ok(k)
    }
    fn write(&mut self, h: OsHandle, buf: &[u8]) -> Result<usize, OsError> {
        self.os_writes += 1;
        if self.fail_writes {
            return Err(112);
        }
        let (path, pos, access) = self.handles.get_mut(&h).ok_or(6u32)?;
        if *access & ACCESS_WRITE == 0 {
            return Err(OS_ERROR_ACCESS_DENIED);
        }
        let data = self.files.get_mut(path.as_slice()).unwrap();
        let end = *pos as usize + buf.len();
        if data.len() < end {
            data.resize(end, 0);
        }
        data[*pos as usize..end].copy_from_slice(buf);
        *pos = end as u32;
        Ok(buf.len())
    }
    fn seek(&mut self, h: OsHandle, offset: i32, from: SeekFrom) -> Result<u32, OsError> {
        let (path, pos, _) = self.handles.get_mut(&h).ok_or(6u32)?;
        let len = self.files[path.as_slice()].len() as i64;
        let base = match from {
            SeekFrom::Begin => 0,
            SeekFrom::Current => *pos as i64,
            SeekFrom::End => len,
        };
        let t = base + offset as i64;
        if t < 0 {
            return Err(131);
        }
        *pos = t as u32;
        Ok(*pos)
    }
    fn tell(&mut self, h: OsHandle) -> u32 {
        self.handles.get(&h).map_or(0, |e| e.1)
    }
    fn size(&mut self, h: OsHandle) -> Result<u32, OsError> {
        let (path, _, _) = self.handles.get(&h).ok_or(6u32)?;
        Ok(self.files[path.as_slice()].len() as u32)
    }
    fn set_end(&mut self, h: OsHandle) -> Result<(), OsError> {
        let (path, pos, _) = self.handles.get(&h).ok_or(6u32)?.clone();
        self.files.get_mut(&path).unwrap().resize(pos as usize, 0);
        Ok(())
    }
    fn commit(&mut self, _h: OsHandle) -> Result<(), OsError> {
        Ok(())
    }
    fn delete(&mut self, path: &[u8]) -> Result<(), OsError> {
        self.files.remove(path).map(|_| ()).ok_or(2)
    }
    fn rename(&mut self, old: &[u8], new: &[u8]) -> Result<(), OsError> {
        let d = self.files.remove(old).ok_or(2u32)?;
        self.files.insert(new.to_vec(), d);
        Ok(())
    }
    fn copy(&mut self, src: &[u8], dst: &[u8], fail_if_exists: bool) -> Result<(), OsError> {
        if fail_if_exists && self.files.contains_key(dst) {
            return Err(80);
        }
        let d = self.files.get(src).ok_or(2u32)?.clone();
        self.files.insert(dst.to_vec(), d);
        Ok(())
    }
    fn exists(&mut self, path: &[u8]) -> bool {
        self.files.contains_key(path)
    }
    fn resolve_shortcut(&mut self, path: &[u8]) -> Option<Vec<u8>> {
        self.lnk.get(path).cloned()
    }
    fn attributes(&mut self, path: &[u8]) -> Option<OsAttributes> {
        self.files.get(path).map(|_| OsAttributes::default())
    }
    fn set_attributes(&mut self, _path: &[u8], _a: OsAttributes) -> Result<(), OsError> {
        Err(5)
    }
    fn first_match_size(&mut self, path: &[u8]) -> Option<u32> {
        self.files.get(path).map(|d| d.len() as u32)
    }
    fn temp_directory(&mut self) -> Vec<u8> {
        b"C:\\Temp\\".to_vec()
    }
    fn sleep_micros(&mut self, micros: u32) {
        self.sleeps += 1;
        self.micros += micros as i64;
    }
    fn time_seconds(&mut self) -> i64 {
        self.micros / 1_000_000
    }
}

const RW: u32 = ACCESS_READ | ACCESS_WRITE;

#[test]
fn open_creation_modes_and_error_codes() {
    let mut h = MemHost::with(b"a", b"xyz");
    let mut f = RzFile::with_path(Some(b"a"));
    assert_eq!(f.last_error(), RZ_ERROR_NONE);
    assert!(!f.open(&mut h, RW, CREATE_NEW, 0));
    assert_eq!(f.last_error(), RZ_ERROR_ALREADY_EXISTS);
    assert!(!f.open(&mut h, RW, 5, 0));
    assert_eq!(
        f.last_error(),
        RZ_ERROR_ALREADY_EXISTS,
        "invalid creation sets no error"
    );
    assert!(f.open(&mut h, RW, OPEN_ALWAYS, 0));
    assert_eq!(f.last_error(), RZ_ERROR_ALREADY_EXISTS);
    assert_eq!(f.open_modes(), Some((RW, OPEN_ALWAYS, 0)));
    assert!(
        f.open(&mut h, ACCESS_READ, CREATE_ALWAYS, 1),
        "already open: true, modes ignored"
    );
    assert_eq!(f.open_modes(), Some((RW, OPEN_ALWAYS, 0)));
    assert_eq!(h.data(b"a"), b"xyz", "no truncate on the ignored reopen");
    assert!(f.close(&mut h));
    assert!(!f.close(&mut h));
    assert_eq!(f.position(), NPOS);
    assert_eq!(f.open_modes(), None);
    let mut g = RzFile::with_path(Some(b"new"));
    assert!(g.open(&mut h, RW, CREATE_ALWAYS, 0));
    assert_eq!(g.last_error(), 0, "did not exist: OS code 0");
    let mut m = RzFile::with_path(Some(b"missing"));
    assert!(!m.open(&mut h, ACCESS_READ, OPEN_EXISTING, 1));
    assert_eq!(m.last_error(), 2);
    assert_eq!(map_os_error(5), RZ_ERROR_ACCESS_DENIED);
}

#[test]
fn lnk_resolution_and_fallback() {
    let mut h = MemHost::with(b"target", b"T");
    h.files.insert(b"x.LNK".to_vec(), b"L".to_vec());
    h.files.insert(b"y.lnk".to_vec(), b"Y".to_vec());
    h.lnk.insert(b"x.LNK".to_vec(), b"target".to_vec());
    let mut buf = [0u8; 1];
    let mut f = RzFile::with_path(Some(b"x.LNK"));
    assert!(f.open(&mut h, ACCESS_READ, OPEN_EXISTING, 1) && f.read(&mut h, &mut buf));
    assert_eq!(&buf, b"T");
    let mut g = RzFile::with_path(Some(b"y.lnk"));
    assert!(g.open(&mut h, ACCESS_READ, OPEN_EXISTING, 1) && g.read(&mut h, &mut buf));
    assert_eq!(&buf, b"Y", "unresolvable shortcut opens the .lnk itself");
}

#[test]
fn buffered_read_boundaries_and_reuse() {
    let data: Vec<u8> = (0..10u8).collect();
    let mut h = MemHost::with(b"f", &data);
    let mut f = RzFile::with_path(Some(b"f"));
    assert!(f.set_buffer_sizes(&mut h, 4, 4));
    assert!(f.open(&mut h, ACCESS_READ, OPEN_EXISTING, 1));
    let mut b = [0u8; 16];
    let mut n = 3;
    assert!(f.read_with_count(&mut h, &mut b, &mut n));
    assert_eq!((n, &b[..3], h.os_reads), (3, &[0, 1, 2][..], 1));
    n = 6;
    assert!(f.read_with_count(&mut h, &mut b, &mut n));
    assert_eq!((n, &b[..6], h.os_reads), (6, &[3, 4, 5, 6, 7, 8][..], 3));
    assert_eq!((f.position(), f.physical_position()), (9, 10));
    // Seek back into the buffered window [8, 10): no OS read.
    assert_eq!(f.seek(&mut h, 8, SEEK_BEGIN), 8);
    let reads = h.os_reads;
    n = 2;
    assert!(f.read_with_count(&mut h, &mut b, &mut n));
    assert_eq!((n, &b[..2], h.os_reads), (2, &[8, 9][..], reads));
    // Short read at end of file: success with smaller n; the all-or-nothing form fails.
    n = 5;
    assert!(f.read_with_count(&mut h, &mut b, &mut n));
    assert_eq!(n, 0);
    assert_eq!(f.seek(&mut h, -3, SEEK_END), 7);
    assert!(!f.read(&mut h, &mut b[..5]));
    n = 0;
    assert!(f.read_with_count(&mut h, &mut b, &mut n));
    // Unbuffered: one OS read of n.
    f.set_buffer_sizes(&mut h, 0, 0);
    f.seek(&mut h, 0, SEEK_BEGIN);
    let reads = h.os_reads;
    assert!(f.read(&mut h, &mut b[..10]));
    assert_eq!(h.os_reads, reads + 1);
    let mut closed = RzFile::new();
    assert!(!closed.read_with_count(&mut h, &mut b, &mut n));
}

#[test]
fn seek_current_converts_when_buffered_ahead() {
    let data: Vec<u8> = (0..20u8).collect();
    let mut h = MemHost::with(b"f", &data);
    let mut f = RzFile::with_path(Some(b"f"));
    f.open(&mut h, ACCESS_READ, OPEN_EXISTING, 1);
    let mut b = [0u8; 2];
    f.read(&mut h, &mut b);
    assert_eq!((f.position(), f.physical_position()), (2, 20));
    assert_eq!(f.seek_relative(&mut h, 3), 5);
    assert_eq!(f.seek(&mut h, 0, 7), 5, "unknown mode returns Position");
    assert_eq!(f.seek(&mut h, -100, SEEK_CURRENT), NPOS);
    assert_eq!(f.last_error(), 131);
    let mut closed = RzFile::new();
    assert_eq!(closed.seek(&mut h, 0, SEEK_BEGIN), NPOS);
}

#[test]
fn buffered_write_flush_length_and_readback() {
    let mut h = MemHost::default();
    let mut f = RzFile::with_path(Some(b"w"));
    f.set_buffer_sizes(&mut h, 4, 4);
    assert!(f.open(&mut h, RW, CREATE_ALWAYS, 0));
    assert!(f.write(&mut h, b"hello"));
    assert_eq!((h.os_writes, f.pending_writes(), f.position()), (1, 1, 5));
    assert_eq!(h.data(b"w"), b"hell");
    assert_eq!(f.length(&mut h), 5, "pending writes: logical position");
    f.seek_to_begin(&mut h);
    assert_eq!(h.data(b"w"), b"hello", "seek flushes");
    let mut b = [0u8; 5];
    assert!(f.read(&mut h, &mut b));
    assert_eq!(&b, b"hello");
    // Write after read discards the read buffer and moves the OS pointer back.
    f.seek_to_position(&mut h, 1);
    f.read(&mut h, &mut b[..1]);
    assert!(f.write(&mut h, b"A"));
    assert!(f.flush(&mut h));
    assert_eq!(h.data(b"w"), b"heAlo");
    assert!(f.set_length(&mut h, 2));
    assert_eq!((f.position(), h.data(b"w")), (2, &b"he"[..]));
    assert!(f.close(&mut h));
}

#[test]
fn failed_flush_drops_data_but_advances_logical() {
    let mut h = MemHost::default();
    let mut f = RzFile::with_path(Some(b"w"));
    f.set_buffer_sizes(&mut h, 4, 4);
    f.open(&mut h, RW, CREATE_ALWAYS, 0);
    h.fail_writes = true;
    let mut n = 6;
    assert!(!f.write_with_count(&mut h, b"abcdef", &mut n));
    assert_eq!((n, f.position(), f.pending_writes(), f.last_error()), (6, 6, 0, 112));
    assert!(f.flush(&mut h), "nothing pending: true");
    h.fail_writes = false;
    assert_eq!(h.data(b"w"), b"");
}

#[test]
fn buffer_size_rules() {
    assert_eq!(normalize_buffer_size(-1), 0x200);
    assert_eq!(normalize_buffer_size(0), 0);
    assert_eq!(
        normalize_buffer_size(1),
        0,
        "rounded to even first, then the below-4 rule (RZ1)"
    );
    assert_eq!(normalize_buffer_size(2), 4);
    assert_eq!(normalize_buffer_size(3), 4);
    assert_eq!(normalize_buffer_size(7), 6);
    assert_eq!(normalize_buffer_size(0x7FFF_FFFF), MAX_BUFFER);
    let mut h = MemHost::default();
    let mut f = RzFile::new();
    assert_eq!(f.buffer_sizes(), (0x200, 0x200));
    f.set_buffer_sizes(&mut h, 9, -1);
    assert_eq!(f.buffer_sizes(), (8, 0x200));
}

#[test]
fn refcount_and_path_operations() {
    let mut h = MemHost::with(b"a", b"1");
    let mut f = RzFile::with_path(Some(b"a"));
    assert!(f.query_interface(IID_IGZFILE) && !f.query_interface(7));
    assert_eq!((f.add_ref(), f.release(), f.release()), (2, 1, 0));
    f.set_path(None);
    assert_eq!(f.path(), b"a");
    assert!(f.exists(&mut h));
    let mut c = RzFile::with_path(Some(b"c"));
    assert!(c.copy_from_file(&mut h, &f, false));
    assert!(!c.copy_from(&mut h, b"a", false));
    assert!(c.copy_from(&mut h, b"a", true));
    assert!(c.rename(&mut h, b"zz"));
    let mut m = RzFile::with_path(Some(b"nope"));
    assert!(!m.rename(&mut h, b"other"));
    assert_eq!(m.path(), b"other", "path set even when the move failed");
    f.open(&mut h, ACCESS_READ, OPEN_EXISTING, 1);
    assert!(f.remove(&mut h) && !f.is_open() && !h.exists(b"a"));
    assert_eq!(get_length(&mut h, b"zz"), 1);
    assert!(file_create(&mut h, b"zz", true));
    assert_eq!(get_length(&mut h, b"zz"), 0);
    assert!(!get_attribute(&mut h, b"missing", ATTR_WRITABLE));
    assert!(get_attribute(&mut h, b"missing", ATTR_DIRECTORY | ATTR_HIDDEN));
    assert!(get_attribute(&mut h, b"zz", ATTR_WRITABLE));
    assert!(
        set_attribute(&mut h, b"zz", ATTR_WRITABLE, true),
        "no change: no OS call"
    );
    assert!(!set_attribute(&mut h, b"zz", ATTR_DIRECTORY, true));
    assert!(is_file_writable(&mut h, b"zz") && is_file_writable(&mut h, b"fresh"));
    assert!(!h.exists(b"fresh"));
}

const EXAMPLE: &[u8] =
    b"; top comment\r\n[Video]\r\nWidth = 1024\r\nheight=768\r\n\r\n[video]\r\nWidth = 640\r\n[Audio]\r\nVolume = 80 ; loud";

fn read(ini: &mut RzIniFile, h: &mut MemHost, s: &[u8], k: &[u8]) -> Option<Vec<u8>> {
    let mut out = b"default".to_vec();
    if ini.read_entry(h, s, k, &mut out) {
        Some(out)
    } else {
        assert_eq!(out, b"default", "miss leaves out unchanged");
        None
    }
}

#[test]
fn worked_example_reads_and_enum() {
    let mut h = MemHost::with(b"a.ini", EXAMPLE);
    let mut ini = RzIniFile::with_path(b"a.ini");
    assert_eq!(
        read(&mut ini, &mut h, b"VIDEO", b"width").as_deref(),
        Some(&b"1024"[..])
    );
    assert_eq!(
        read(&mut ini, &mut h, b"Audio", b"volume").as_deref(),
        Some(&b"80 ; loud"[..])
    );
    assert_eq!(read(&mut ini, &mut h, b"Audio", b"none"), None);
    assert_eq!(read(&mut ini, &mut h, b"Nope", b"x"), None);
    let mut v = 0;
    assert!(ini.read_number(&mut h, b"Audio", b"Volume", &mut v) && v == 80);
    let mut secs = Vec::new();
    assert!(ini.enum_sections(&mut h, &mut |n, v| secs.push((n.to_vec(), v.to_vec()))));
    assert_eq!(secs, vec![(b"Audio".to_vec(), vec![]), (b"Video".to_vec(), vec![])]);
    assert!(ini.section_exists(&mut h, b"aUdIo") && !ini.section_exists(&mut h, b"net"));
    assert!(lock_open(&ini), "leave-open default keeps the handle");
}

fn lock_open(ini: &RzIniFile) -> bool {
    ini.file().lock().unwrap().is_open()
}

fn write_example(s: &[u8], k: &[u8], v: &[u8]) -> Vec<u8> {
    let mut h = MemHost::with(b"a.ini", EXAMPLE);
    let mut ini = RzIniFile::with_path(b"a.ini");
    assert!(ini.write_entry(&mut h, s, k, v));
    ini.destroy(&mut h);
    h.data(b"a.ini").to_vec()
}

#[test]
fn worked_example_writeback() {
    assert_eq!(
        write_example(b"Video", b"HEIGHT", b"800"),
        b"; top comment\r\n[Video]\r\nWidth = 1024\r\nHEIGHT = 800\r\n[video]\r\nWidth = 640\r\n[Audio]\r\nVolume = 80 ; loud"
    );
    assert_eq!(
        write_example(b"Video", b"Depth", b"32"),
        b"; top comment\r\n[Video]\r\nWidth = 1024\r\nheight=768\r\n\r\nDepth = 32\r\n[video]\r\nWidth = 640\r\n[Audio]\r\nVolume = 80 ; loud"
    );
    let net = write_example(b"Net", b"Port", b"80");
    assert_eq!(&net[EXAMPLE.len()..], b"\r\n\r\n[Net]\r\nPort = 80\r\n");
    // Append at end of the last block: joined onto a line without a trailing newline.
    let app = write_example(b"Audio", b"Mute", b"1");
    assert_eq!(&app[EXAMPLE.len() - 9..], b"80 ; loudMute = 1\r\n");
    assert!(!RzIniFile::with_path(b"a.ini").write_entry(&mut MemHost::default(), b"", b"k", b"v"));
}

#[test]
fn write_then_read_round_trip_and_new_file() {
    let mut h = MemHost::default();
    let mut ini = RzIniFile::with_path(b"n.ini");
    assert!(ini.write_entry(&mut h, b"S", b"k", b"v"));
    assert_eq!(h.data(b"n.ini"), b"", "leave-open: still in the write buffer");
    assert!(ini.file().lock().unwrap().flush(&mut h));
    assert_eq!(h.data(b"n.ini"), b"[S]\r\nk = v\r\n");
    assert!(ini.write_number(&mut h, b"S", b"n", -42));
    assert!(ini.write_entry(&mut h, b"T", b"x", b"1"));
    assert!(ini.write_entry(&mut h, b"s", b"K", b"w"));
    assert_eq!(h.data(b"n.ini"), b"[S]\r\nK = w\r\nn = -42\r\n\r\n[T]\r\nx = 1\r\n");
    let mut n = 0;
    assert!(ini.read_number(&mut h, b"S", b"N", &mut n) && n == -42);
    assert!(ini.write_binary_entry(&mut h, b"B", b"k", &[0x0A, 0xFF]));
    assert_eq!(read(&mut ini, &mut h, b"B", b"k").as_deref(), Some(&b"0AFF"[..]));
    let mut buf = [0u8; 2];
    assert_eq!(ini.read_binary_entry(&mut h, b"B", b"k", &mut buf), 2);
    assert_eq!(buf, [0x0A, 0xFF]);
    assert_eq!(
        ini.read_binary_entry_alloc(&mut h, b"B", b"k"),
        BinaryAlloc::Ok(vec![0x0A, 0xFF])
    );
    assert_eq!(ini.read_binary_entry_alloc(&mut h, b"B", b"zz"), BinaryAlloc::Missing);
}

#[test]
fn binary_and_number_parsing() {
    let mut h = MemHost::with(
        b"b.ini",
        b"[B]\nlo=0aff\nbad=0A F\nodd=0AF\nn1=  12abc\nn2=0x10\nn3=abc\n",
    );
    let mut ini = RzIniFile::with_path(b"b.ini");
    let mut buf = [0u8; 4];
    assert_eq!(ini.read_binary_entry(&mut h, b"B", b"lo", &mut buf), 2);
    assert_eq!(ini.read_binary_entry(&mut h, b"B", b"bad", &mut buf), 0);
    assert_eq!(ini.read_binary_entry(&mut h, b"B", b"odd", &mut buf), 0);
    assert_eq!(
        ini.read_binary_entry(&mut h, b"B", b"lo", &mut buf[..1]),
        1,
        "min(len, 2 x size)"
    );
    assert_eq!(ini.read_binary_entry_alloc(&mut h, b"B", b"bad"), BinaryAlloc::Failed);
    let mut v = 7;
    assert!(ini.read_number(&mut h, b"B", b"n1", &mut v) && v == 12);
    assert!(ini.read_number(&mut h, b"B", b"n2", &mut v) && v == 0);
    assert!(!ini.read_number(&mut h, b"B", b"n3", &mut v) && v == 0);
    assert_eq!(scan_long(b"-2147483648"), Some(i32::MIN));
}

#[test]
fn syntax_duplicates_and_enum_entries() {
    let text =
        b"k=orphan\n[S]\r\n\r\n  a = 1 = 2 \n\tA=dup\n;c=3\n#h = x\nnoeq\nempty =\n[S]\nz=9\n[] \n[open\n[T]x\ny=1";
    let mut h = MemHost::with(b"s.ini", text);
    let mut ini = RzIniFile::with_path(b"s.ini");
    assert_eq!(read(&mut ini, &mut h, b"s", b"A").as_deref(), Some(&b"1 = 2"[..]));
    assert_eq!(read(&mut ini, &mut h, b"S", b"z"), None, "duplicate section invisible");
    assert_eq!(
        read(&mut ini, &mut h, b"S", b"#h").as_deref(),
        Some(&b"x"[..]),
        "# is not a comment"
    );
    assert_eq!(read(&mut ini, &mut h, b"S", b";c"), None);
    assert_eq!(read(&mut ini, &mut h, b"T", b"y").as_deref(), Some(&b"1"[..]));
    let mut e = Vec::new();
    assert!(ini.enum_entries(&mut h, b"S", &mut |k, v| e.push((k.to_vec(), v.to_vec()))));
    assert_eq!(
        e,
        vec![
            (b"a".to_vec(), b"1 = 2".to_vec()),
            (b"A".to_vec(), b"dup".to_vec()),
            (b"#h".to_vec(), b"x".to_vec())
        ]
    );
    assert!(!ini.enum_entries(&mut h, b"none", &mut |_, _| {}));
    let mut secs = Vec::new();
    ini.enum_sections(&mut h, &mut |n, _| secs.push(n.to_vec()));
    assert_eq!(secs, vec![b"S".to_vec(), b"T".to_vec()]);
    // A BOM stays in the first line: the header is not recognised.
    let mut hb = MemHost::with(b"bom.ini", b"\xEF\xBB\xBF[A]\nk=1\n");
    let mut ib = RzIniFile::with_path(b"bom.ini");
    assert!(!ib.section_exists(&mut hb, b"A"));
}

#[test]
fn line_reader_newline_runs() {
    let mut h = MemHost::with(b"l", b"\r\n\nabc\r\r\n\ndef\nx");
    let mut ini = RzIniFile::with_path(b"l");
    assert!(ini.open(&mut h, false));
    let mut lines = Vec::new();
    while let Some(l) = ini.get_file_line(&mut h) {
        lines.push(l);
    }
    assert_eq!(
        lines,
        vec![b"".to_vec(), b"abc".to_vec(), b"def".to_vec(), b"x".to_vec()]
    );
    let long = [b"[L]\n".to_vec(), vec![b'k'; 100], b"=v\n".to_vec()].concat();
    let mut hl = MemHost::with(b"l2", &long);
    let mut il = RzIniFile::with_path(b"l2");
    assert_eq!(read(&mut il, &mut hl, b"L", &[b'K'; 100]).as_deref(), Some(&b"v"[..]));
}

#[test]
fn open_retry_window_and_missing_file() {
    let mut h = MemHost::with(b"r.ini", b"[S]\nk=v\n");
    h.deny_opens = 1000;
    let mut ini = RzIniFile::with_path(b"r.ini");
    assert_eq!(read(&mut ini, &mut h, b"S", b"k"), None);
    assert_eq!(h.sleeps, 12, "250 ms steps until 3 whole seconds");
    h.deny_opens = 3;
    h.sleeps = 0;
    assert_eq!(read(&mut ini, &mut h, b"S", b"k").as_deref(), Some(&b"v"[..]));
    assert_eq!(h.sleeps, 3);
    let mut hm = MemHost::default();
    let mut im = RzIniFile::with_path(b"missing.ini");
    assert_eq!(read(&mut im, &mut hm, b"S", b"k"), None);
    assert_eq!(hm.sleeps, 0, "missing file fails at once");
}

#[test]
fn leave_open_off_closes_and_shared_file_ownership() {
    let mut h = MemHost::with(b"o.ini", b"[S]\nk=v\n");
    let shared: SharedFile = Arc::new(Mutex::new(RzFile::with_path(Some(b"o.ini"))));
    shared.lock().unwrap().add_ref();
    let mut ini = RzIniFile::with_file(b"o.ini", Some(shared.clone()));
    assert_eq!(shared.lock().unwrap().ref_count(), 2);
    ini.set_leave_file_open(&mut h, false);
    assert_eq!(read(&mut ini, &mut h, b"S", b"k").as_deref(), Some(&b"v"[..]));
    assert!(!lock_open(&ini) && !ini.is_index_loaded());
    ini.set_leave_file_open(&mut h, true);
    assert!(ini.section_exists(&mut h, b"S") && lock_open(&ini));
    assert!(ini.set_file(&mut h, Some(shared.clone())), "same file: no change");
    assert!(lock_open(&ini));
    assert!(ini.set_file(&mut h, None));
    assert!(!shared.lock().unwrap().is_open());
    assert_eq!(shared.lock().unwrap().ref_count(), 1);
    assert!(!Arc::ptr_eq(&ini.file(), &shared));
    ini.set_path(&mut h, b"other.ini");
    assert_eq!(ini.path(), b"other.ini");
    // Read-mode file is reopened for writing by WriteEntry.
    let mut iw = RzIniFile::with_path(b"o.ini");
    assert!(iw.section_exists(&mut h, b"s") && !iw.is_open_for_write());
    assert!(iw.write_entry(&mut h, b"S", b"k", b"w"));
    assert!(iw.is_open_for_write());
    assert_eq!(h.data(b"o.ini"), b"[S]\nk = w\r\n");
}

#[test]
fn lower_case_rules() {
    assert_eq!(ini_lower(b"AbC_["), b"abc_[");
    assert_eq!(ini_lower("ÄBC".as_bytes()), "äbc".as_bytes());
    assert_eq!(ini_lower(b"A\x7E"), b"a\x7E");
}
