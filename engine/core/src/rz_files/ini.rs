//! cRZIniFile (docs/specs/framework-rz-files.md §3): INI reader/writer over a shared [`RzFile`].

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};

use super::file::{RzFile, ACCESS_READ, ACCESS_WRITE, OPEN_ALWAYS, OPEN_EXISTING, SEEK_BEGIN, SEEK_END, SHARE_READ};
use super::host::RzFileHost;

/// A reference-counted file shared between an INI object and its other holders (§3.1 "reference held").
pub type SharedFile = Arc<Mutex<RzFile>>;

/// Retry sleep of Open (§3.4): 250,000 microseconds.
pub const OPEN_RETRY_SLEEP_MICROS: u32 = 250_000;
/// Retry window of Open (§3.4): give up once a retry would start this many whole seconds late.
pub const OPEN_RETRY_SECONDS: i64 = 3;
/// Chunk size of the line reader (§3.3); observable only through the OS read pattern.
pub const LINE_CHUNK: usize = 40;

/// Trim set of every INI trim: space and tab only (§3.3).
fn is_blank(b: u8) -> bool {
    b == b' ' || b == b'\t'
}

fn trim_left(s: &[u8]) -> &[u8] {
    let i = s.iter().position(|&b| !is_blank(b)).unwrap_or(s.len());
    &s[i..]
}

fn trim_right(s: &[u8]) -> &[u8] {
    let i = s.iter().rposition(|&b| !is_blank(b)).map_or(0, |i| i + 1);
    &s[..i]
}

/// Lower-casing of section names and keys (§3.3): ASCII A-Z; a string with any byte above 0x7E is
/// lower-cased as a whole through UTF-16, unit by unit.
pub fn ini_lower(s: &[u8]) -> Vec<u8> {
    if s.iter().all(|&b| b <= 0x7E) {
        return s.to_ascii_lowercase();
    }
    // Invalid UTF-8 is not covered by the spec; literal reading: decoded lossily (question RZ4).
    let text = String::from_utf8_lossy(s);
    let units: Vec<u16> = text
        .encode_utf16()
        .map(|u| {
            // Wide lower-case of one unit: a unit that is a whole BMP character with a single-unit
            // lower case maps to it; surrogates and multi-character lower cases stay unchanged.
            match char::from_u32(u as u32) {
                Some(c) => {
                    let mut l = c.to_lowercase();
                    match (l.next(), l.next()) {
                        (Some(x), None) if (x as u32) <= 0xFFFF => x as u32 as u16,
                        _ => u,
                    }
                }
                None => u,
            }
        })
        .collect();
    String::from_utf16_lossy(&units).into_bytes()
}

/// One parsed line (§3.3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IniLine {
    /// Empty or comment.
    Ignored,
    /// A line starting with `[`; `None` for `[]` or a `[` without `]` (still ends a block).
    Header(Option<Vec<u8>>),
    /// An entry: key right-trimmed, value trimmed both sides.
    Entry { key: Vec<u8>, value: Vec<u8> },
}

/// Classifies one line (§3.3).
pub fn parse_line(line: &[u8]) -> IniLine {
    let t = trim_left(line);
    match t.first() {
        None | Some(b';') => IniLine::Ignored,
        Some(b'[') => match t[1..].iter().position(|&b| b == b']') {
            Some(0) | None => IniLine::Header(None),
            Some(end) => IniLine::Header(Some(t[1..1 + end].to_vec())),
        },
        Some(_) => match t.iter().position(|&b| b == b'=') {
            Some(eq) => IniLine::Entry {
                key: trim_right(&t[..eq]).to_vec(),
                value: trim_left(trim_right(&t[eq + 1..])).to_vec(),
            },
            // "A line with no `=` has an empty key"; the value is not specified: literal reading
            // = empty (question RZ5).
            None => IniLine::Entry {
                key: Vec::new(),
                value: Vec::new(),
            },
        },
    }
}

/// C `sscanf("%ld")` as §3.5 describes it. Overflow wraps (question RZ6).
pub fn scan_long(s: &[u8]) -> Option<i32> {
    let mut i = 0;
    while i < s.len() && matches!(s[i], b' ' | b'\t' | b'\n' | b'\r' | 0x0B | 0x0C) {
        i += 1;
    }
    let mut neg = false;
    if i < s.len() && (s[i] == b'+' || s[i] == b'-') {
        neg = s[i] == b'-';
        i += 1;
    }
    let start = i;
    let mut v: u32 = 0;
    while i < s.len() && s[i].is_ascii_digit() {
        v = v.wrapping_mul(10).wrapping_add((s[i] - b'0') as u32);
        i += 1;
    }
    if i == start {
        return None;
    }
    Some(if neg { (v as i32).wrapping_neg() } else { v as i32 })
}

fn hex_digit(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'A'..=b'F' => Some(b - b'A' + 10),
        b'a'..=b'f' => Some(b - b'a' + 10),
        _ => None,
    }
}

/// Upper-case hex text, two characters per byte (§3.5).
pub fn to_hex(data: &[u8]) -> Vec<u8> {
    const D: &[u8; 16] = b"0123456789ABCDEF";
    data.iter()
        .flat_map(|&b| [D[(b >> 4) as usize], D[(b & 15) as usize]])
        .collect()
}

/// Converts `chars` (even length) into `out`; invalid characters count as 0 and fail the conversion.
fn from_hex(chars: &[u8], out: &mut [u8]) -> bool {
    let mut ok = true;
    for (i, pair) in chars.chunks(2).enumerate() {
        let hi = hex_digit(pair[0]).unwrap_or_else(|| {
            ok = false;
            0
        });
        let lo = pair.get(1).and_then(|&c| hex_digit(c)).unwrap_or_else(|| {
            ok = false;
            0
        });
        out[i] = (hi << 4) | lo;
    }
    ok
}

/// Result of the allocating ReadBinaryEntry (slot 9).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BinaryAlloc {
    /// Entry missing: returns 0, pointer unchanged.
    Missing,
    /// Conversion failed: buffer freed, pointer := null, returns 0.
    Failed,
    /// Returns length/2 and the buffer.
    Ok(Vec<u8>),
}

/// One cRZIniFile object (§3.1).
#[derive(Debug)]
pub struct RzIniFile {
    path: Vec<u8>,
    leave_open: bool,
    loaded: bool,
    index: BTreeMap<Vec<u8>, u32>,
    display: BTreeMap<Vec<u8>, Vec<u8>>,
    file: SharedFile,
    open_for_write: bool,
}

fn new_shared(path: &[u8]) -> SharedFile {
    let mut f = RzFile::with_path(Some(path));
    f.add_ref();
    Arc::new(Mutex::new(f))
}

impl Default for RzIniFile {
    fn default() -> Self {
        Self::new()
    }
}

impl RzIniFile {
    /// Default constructor: own cRZFile with an empty path.
    pub fn new() -> Self {
        Self::with_file(b"", None)
    }

    /// Constructor from a path: own cRZFile on that path.
    pub fn with_path(path: &[u8]) -> Self {
        Self::with_file(path, None)
    }

    /// Constructor from a `cIGZFile*` (AddRef), or an own file when `None`. The +0x38 flag starts at 0
    /// (spec Q4: the path / file constructors do not write it; a reimplementation starts it at 0).
    pub fn with_file(path: &[u8], file: Option<SharedFile>) -> Self {
        let file = match file {
            Some(f) => {
                lock(&f).add_ref();
                f
            }
            None => new_shared(path),
        };
        RzIniFile {
            path: path.to_vec(),
            leave_open: true,
            loaded: false,
            index: BTreeMap::new(),
            display: BTreeMap::new(),
            file,
            open_for_write: false,
        }
    }

    /// Destructor: closes the file if open and releases it.
    pub fn destroy(self, host: &mut dyn RzFileHost) {
        let mut f = lock(&self.file);
        if f.is_open() {
            f.close(host);
        }
        f.release();
    }

    pub fn path(&self) -> &[u8] {
        &self.path
    }

    /// SetPath (slot 2): closes the INI if its index is loaded, then stores the path.
    pub fn set_path(&mut self, host: &mut dyn RzFileHost, path: &[u8]) {
        if self.loaded {
            self.close(host);
        }
        self.path = path.to_vec();
    }

    /// GetFile (slot 3).
    pub fn file(&self) -> SharedFile {
        self.file.clone()
    }

    /// SetFile (slot 4). Always true.
    pub fn set_file(&mut self, host: &mut dyn RzFileHost, file: Option<SharedFile>) -> bool {
        let same = matches!(&file, Some(f) if Arc::ptr_eq(f, &self.file));
        if !same {
            self.close(host);
            let new = match file {
                Some(f) => {
                    lock(&f).add_ref();
                    f
                }
                None => new_shared(&self.path),
            };
            let old = std::mem::replace(&mut self.file, new);
            lock(&old).release();
        }
        true
    }

    pub fn is_index_loaded(&self) -> bool {
        self.loaded
    }

    pub fn is_open_for_write(&self) -> bool {
        self.open_for_write
    }

    /// Close (slot 5): closes the file if open, clears the index and the loaded flag; the display-name
    /// map is kept.
    pub fn close(&mut self, host: &mut dyn RzFileHost) -> bool {
        {
            let mut f = lock(&self.file);
            if f.is_open() {
                f.close(host);
            }
        }
        self.index.clear();
        self.loaded = false;
        true
    }

    /// SetLeaveFileOpenBetweenOperations (slot 16).
    pub fn set_leave_file_open(&mut self, host: &mut dyn RzFileHost, flag: bool) {
        self.leave_open = flag;
        if !flag {
            self.close(host);
        }
    }

    /// Open(forWrite) (slot 19) with the 250 ms / 3 s retry.
    pub fn open(&mut self, host: &mut dyn RzFileHost, for_write: bool) -> bool {
        let access = if for_write {
            ACCESS_READ | ACCESS_WRITE
        } else {
            ACCESS_READ
        };
        let mut f = lock(&self.file);
        if let Some((a, _, _)) = f.open_modes() {
            if a == access {
                return true;
            }
        }
        let (creation, share) = if for_write {
            (OPEN_ALWAYS, 0)
        } else {
            (OPEN_EXISTING, SHARE_READ)
        };
        let first = host.time_seconds();
        loop {
            if f.open(host, access, creation, share) {
                self.open_for_write = for_write;
                return true;
            }
            if !f.exists(host) {
                return false;
            }
            host.sleep_micros(OPEN_RETRY_SLEEP_MICROS);
            if host.time_seconds() - first >= OPEN_RETRY_SECONDS {
                return false;
            }
        }
    }

    /// GetFileLine (slot 18): `None` at end of file with nothing read.
    pub fn get_file_line(&mut self, host: &mut dyn RzFileHost) -> Option<Vec<u8>> {
        let mut f = lock(&self.file);
        read_line(&mut f, host)
    }

    /// LoadSectionNames(forWrite) (slot 17).
    pub fn load_section_names(&mut self, host: &mut dyn RzFileHost, for_write: bool) -> bool {
        self.index.clear();
        let is_open = lock(&self.file).is_open();
        if !is_open {
            lock(&self.file).set_path(Some(&self.path));
            if !self.open(host, for_write) {
                return false;
            }
        }
        let mut f = lock(&self.file);
        f.seek(host, 0, SEEK_BEGIN);
        loop {
            let off = f.position();
            let Some(line) = read_line(&mut f, host) else { break };
            if let IniLine::Header(Some(name)) = parse_line(&line) {
                let lower = ini_lower(&name);
                self.index.entry(lower.clone()).or_insert(off);
                self.display.entry(lower).or_insert(name);
            }
        }
        drop(f);
        self.loaded = true;
        true
    }

    fn ensure_loaded(&mut self, host: &mut dyn RzFileHost) -> bool {
        self.loaded || self.load_section_names(host, false)
    }

    /// End of a read operation: the INI is closed when leave-open is off. The spec says "closes the
    /// file"; the INI Close (index dropped too) is taken so the next operation reloads (question RZ7).
    fn finish(&mut self, host: &mut dyn RzFileHost) {
        if !self.leave_open {
            self.close(host);
        }
    }

    /// Positions the file after the header line of `section`; `None` when missing or past the end.
    fn enter_section(&mut self, host: &mut dyn RzFileHost, section: &[u8]) -> Option<()> {
        let off = *self.index.get(&ini_lower(section))?;
        let mut f = lock(&self.file);
        if off >= f.length(host) {
            return None;
        }
        f.seek(host, off as i32, SEEK_BEGIN);
        read_line(&mut f, host);
        Some(())
    }

    /// Scans the current block; returns the value of the first entry with the key.
    fn find_in_block(&mut self, host: &mut dyn RzFileHost, key: &[u8]) -> Option<Vec<u8>> {
        let want = ini_lower(key);
        let mut f = lock(&self.file);
        while let Some(line) = read_line(&mut f, host) {
            match parse_line(&line) {
                IniLine::Header(_) => return None,
                IniLine::Entry { key, value } if ini_lower(&key) == want => return Some(value),
                _ => {}
            }
        }
        None
    }

    /// ReadEntry (slot 6). On a miss `out` is unchanged.
    pub fn read_entry(&mut self, host: &mut dyn RzFileHost, section: &[u8], key: &[u8], out: &mut Vec<u8>) -> bool {
        if !self.ensure_loaded(host) {
            return false;
        }
        let found = match self.enter_section(host, section) {
            Some(()) => self.find_in_block(host, key),
            None => None,
        };
        self.finish(host);
        match found {
            Some(v) => {
                *out = v;
                true
            }
            None => false,
        }
    }

    /// ReadNumber (slot 7). `out` is written only when a number was parsed.
    pub fn read_number(&mut self, host: &mut dyn RzFileHost, section: &[u8], key: &[u8], out: &mut i32) -> bool {
        let mut s = Vec::new();
        if !self.read_entry(host, section, key, &mut s) {
            return false;
        }
        match scan_long(&s) {
            Some(v) => {
                *out = v;
                true
            }
            None => false,
        }
    }

    /// ReadBinaryEntry(section, key, buffer, size) (slot 8): bytes produced, 0 on miss or failure.
    pub fn read_binary_entry(
        &mut self,
        host: &mut dyn RzFileHost,
        section: &[u8],
        key: &[u8],
        buf: &mut [u8],
    ) -> usize {
        let mut s = Vec::new();
        if !self.read_entry(host, section, key, &mut s) {
            return 0;
        }
        let chars = s.len().min(2 * buf.len());
        // "An odd value length fails": taken as an odd count of converted characters (question RZ8).
        if chars % 2 != 0 {
            return 0;
        }
        if from_hex(&s[..chars], buf) {
            chars / 2
        } else {
            0
        }
    }

    /// ReadBinaryEntry(section, key, &ptr) (slot 9).
    pub fn read_binary_entry_alloc(&mut self, host: &mut dyn RzFileHost, section: &[u8], key: &[u8]) -> BinaryAlloc {
        let mut s = Vec::new();
        if !self.read_entry(host, section, key, &mut s) {
            return BinaryAlloc::Missing;
        }
        // Odd length: the same rule as slot 8 (question RZ8).
        if s.len() % 2 != 0 {
            return BinaryAlloc::Failed;
        }
        let mut out = vec![0u8; s.len() / 2];
        if from_hex(&s, &mut out) {
            BinaryAlloc::Ok(out)
        } else {
            BinaryAlloc::Failed
        }
    }

    /// SectionExists (slot 15).
    pub fn section_exists(&mut self, host: &mut dyn RzFileHost, section: &[u8]) -> bool {
        if !self.ensure_loaded(host) {
            return false;
        }
        let found = self.index.contains_key(&ini_lower(section));
        self.finish(host);
        found
    }

    /// EnumEntries (slot 13): every entry with non-empty key and value, in file order.
    pub fn enum_entries(
        &mut self,
        host: &mut dyn RzFileHost,
        section: &[u8],
        cb: &mut dyn FnMut(&[u8], &[u8]),
    ) -> bool {
        if !self.ensure_loaded(host) {
            return false;
        }
        let mut entries = Vec::new();
        let ok = self.enter_section(host, section).is_some();
        if ok {
            let mut f = lock(&self.file);
            while let Some(line) = read_line(&mut f, host) {
                match parse_line(&line) {
                    IniLine::Header(_) => break,
                    IniLine::Entry { key, value } if !key.is_empty() && !value.is_empty() => entries.push((key, value)),
                    _ => {}
                }
            }
        }
        self.finish(host);
        for (k, v) in &entries {
            cb(k, v);
        }
        ok
    }

    /// EnumSections (slot 14): ascending byte order of the lower-cased names; value is empty.
    pub fn enum_sections(&mut self, host: &mut dyn RzFileHost, cb: &mut dyn FnMut(&[u8], &[u8])) -> bool {
        if !self.ensure_loaded(host) {
            return false;
        }
        let names: Vec<Vec<u8>> = self
            .index
            .keys()
            .map(|lower| self.display.get(lower).cloned().unwrap_or_else(|| lower.clone()))
            .collect();
        self.finish(host);
        for n in &names {
            cb(n, b"");
        }
        true
    }

    /// WriteNumber (slot 11): `"%ld"`.
    pub fn write_number(&mut self, host: &mut dyn RzFileHost, section: &[u8], key: &[u8], value: i32) -> bool {
        self.write_entry(host, section, key, value.to_string().as_bytes())
    }

    /// WriteBinaryEntry (slot 12): upper-case hex.
    pub fn write_binary_entry(&mut self, host: &mut dyn RzFileHost, section: &[u8], key: &[u8], data: &[u8]) -> bool {
        self.write_entry(host, section, key, &to_hex(data))
    }

    /// WriteEntry (slot 10, §3.6).
    pub fn write_entry(&mut self, host: &mut dyn RzFileHost, section: &[u8], key: &[u8], value: &[u8]) -> bool {
        if section.is_empty() || key.is_empty() {
            return false;
        }
        let entry = [key, b" = ", value, b"\r\n"].concat();
        let header = [b"[", section, b"]\r\n"].concat();
        // Step 1: no file at the path.
        if !self.path.is_empty() && !host.exists(&self.path) {
            self.loaded = false;
            self.close(host);
            lock(&self.file).set_path(Some(&self.path));
            // Spec silent on an open failure here; literal: false, nothing written (question RZ7).
            if !self.open(host, true) {
                return false;
            }
            {
                let mut f = lock(&self.file);
                f.write(host, &header);
                f.write(host, &entry);
            }
            self.finish(host);
            return true;
        }
        // Step 2: index loaded, file open for writing.
        if !self.loaded && !self.load_section_names(host, true) {
            return false;
        }
        let is_open = lock(&self.file).is_open();
        if !self.open_for_write || !is_open {
            lock(&self.file).close(host);
            if !self.open(host, true) {
                return false;
            }
        }
        let lower = ini_lower(section);
        let result = match self.index.get(&lower).copied() {
            None => {
                // Step 3: new section at end of file.
                let mut f = lock(&self.file);
                let len = f.seek(host, 0, SEEK_END);
                let mut last = [0u8; 1];
                let ends_newline = len != 0
                    && len != super::file::NPOS
                    && f.seek(host, len as i32 - 1, SEEK_BEGIN) != super::file::NPOS
                    && f.read(host, &mut last)
                    && (last[0] == b'\r' || last[0] == b'\n');
                let pos = f.seek(host, 0, SEEK_END);
                f.write(host, if ends_newline { b"\r\n" } else { b"\r\n\r\n" });
                let header_off = pos.wrapping_add(if ends_newline { 2 } else { 4 });
                f.write(host, &header);
                f.write(host, &entry);
                drop(f);
                self.index.insert(lower, header_off);
                true
            }
            Some(off) => self.write_in_section(host, off, key, &entry),
        };
        self.finish(host);
        result
    }

    /// §3.6 steps 4-5.
    fn write_in_section(&mut self, host: &mut dyn RzFileHost, off: u32, key: &[u8], entry: &[u8]) -> bool {
        let want = ini_lower(key);
        let mut f = lock(&self.file);
        let len = f.length(host);
        if off >= len {
            return false;
        }
        f.seek(host, off as i32, SEEK_BEGIN);
        read_line(&mut f, host);
        loop {
            let line_start = f.position();
            let Some(line) = read_line(&mut f, host) else {
                // End of file: append, no newline inserted first.
                f.seek(host, 0, SEEK_END);
                f.write(host, entry);
                return true;
            };
            match parse_line(&line) {
                IniLine::Header(_) => {
                    // Insert at the first byte of the next header line.
                    splice(&mut f, host, line_start, line_start, entry);
                    self.loaded = false;
                    return true;
                }
                IniLine::Entry { key, .. } if ini_lower(&key) == want => {
                    let tail_start = f.position();
                    splice(&mut f, host, line_start, tail_start, entry);
                    self.loaded = false;
                    return true;
                }
                _ => {}
            }
        }
    }
}

/// Reads the tail from `tail_start`, writes `entry` at `at`, the tail after it, and truncates at the
/// new end. A failed tail read writes nothing (§3.6 step 5).
fn splice(f: &mut RzFile, host: &mut dyn RzFileHost, at: u32, tail_start: u32, entry: &[u8]) {
    let len = f.length(host);
    let tail_len = len.saturating_sub(tail_start) as usize;
    let mut tail = vec![0u8; tail_len];
    f.seek(host, tail_start as i32, SEEK_BEGIN);
    if !f.read(host, &mut tail) {
        return;
    }
    f.seek(host, at as i32, SEEK_BEGIN);
    f.write(host, entry);
    f.write(host, &tail);
    let end = f.position();
    f.set_length(host, end);
}

fn lock(f: &SharedFile) -> MutexGuard<'_, RzFile> {
    f.lock().unwrap_or_else(|e| e.into_inner())
}

/// GetFileLine (§3.3): the bytes up to the first CR or LF, then every following CR and LF consumed.
/// Read in `LINE_CHUNK`-byte chunks; the file is left at the first byte after the newline run.
fn read_line(f: &mut RzFile, host: &mut dyn RzFileHost) -> Option<Vec<u8>> {
    let mut line = Vec::new();
    let mut any = false;
    let mut in_newlines = false;
    let mut chunk = [0u8; LINE_CHUNK];
    loop {
        let start = f.position();
        let mut n = LINE_CHUNK as u32;
        if !f.read_with_count(host, &mut chunk, &mut n) || n == 0 {
            break;
        }
        any = true;
        let got = &chunk[..n as usize];
        for (i, &b) in got.iter().enumerate() {
            let nl = b == b'\r' || b == b'\n';
            if in_newlines && !nl {
                f.seek(host, start.wrapping_add(i as u32) as i32, SEEK_BEGIN);
                return Some(line);
            }
            if nl {
                in_newlines = true;
            } else {
                line.push(b);
            }
        }
    }
    if any {
        Some(line)
    } else {
        None
    }
}
