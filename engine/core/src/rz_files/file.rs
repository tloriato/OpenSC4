//! cRZFile (docs/specs/framework-rz-files.md §2): buffered file object over [`RzFileHost`].
//!
//! The object does not own its host: every operation that may touch the OS takes `&mut dyn
//! RzFileHost`. The C++ destructor closes the file (§2.1); in Rust that is [`RzFile::destroy`], since a
//! `Drop` impl has no host to close with.

use super::host::{
    OsAttributes, OsError, OsHandle, RzFileHost, SeekFrom, OS_ERROR_ACCESS_DENIED, OS_ERROR_ALREADY_EXISTS,
};

/// Interface id of `cIGZFile` (§1).
pub const IID_IGZFILE: u32 = 0xE020_364F;
/// Base interface id QueryInterface also accepts (§1).
pub const IID_BASE: u32 = 1;

/// Initial value of the error field +0x30 (§2.8).
pub const RZ_ERROR_NONE: u32 = 0x2000_0000;
/// OS error 183 "already exists" as stored (§2.8, §2.3).
pub const RZ_ERROR_ALREADY_EXISTS: u32 = 0x2000_0001;
/// OS error 5 "access denied" as stored (§2.8).
pub const RZ_ERROR_ACCESS_DENIED: u32 = 0x2000_0002;

/// Default read / write buffer capacity (§2.1, §2.5).
pub const DEFAULT_BUFFER: u32 = 0x200;
/// Largest buffer capacity (§2.5).
pub const MAX_BUFFER: u32 = 0x1F_4000;

/// Position / Length / Seek result for "closed" or "failed" (-1 in the 32-bit original).
pub const NPOS: u32 = u32::MAX;

/// Access mode bits (§2.3).
pub const ACCESS_READ: u32 = 1;
pub const ACCESS_WRITE: u32 = 2;
/// Creation modes (§2.3).
pub const CREATE_NEW: u32 = 0;
pub const CREATE_ALWAYS: u32 = 1;
pub const OPEN_EXISTING: u32 = 2;
pub const OPEN_ALWAYS: u32 = 3;
pub const TRUNCATE_EXISTING: u32 = 4;
/// Share mode bits (§2.3).
pub const SHARE_READ: u32 = 1;
pub const SHARE_READ_WRITE: u32 = 2;

/// Seek modes (§2.4).
pub const SEEK_BEGIN: u32 = 0;
pub const SEEK_CURRENT: u32 = 1;
pub const SEEK_END: u32 = 2;

/// Maps an OS error to the stored code (§2.8).
pub fn map_os_error(err: OsError) -> u32 {
    match err {
        OS_ERROR_ACCESS_DENIED => RZ_ERROR_ACCESS_DENIED,
        OS_ERROR_ALREADY_EXISTS => RZ_ERROR_ALREADY_EXISTS,
        other => other,
    }
}

/// Buffer-size rule of SetBufferSizes (§2.5).
pub fn normalize_buffer_size(size: i32) -> u32 {
    if size == -1 {
        return DEFAULT_BUFFER;
    }
    // Spec silent on other negative values: taken literally as the 32-bit unsigned value, so they
    // clamp to MAX_BUFFER. The rules apply in the order written: 1 rounds to 0 (disabled) before the
    // below-4 rule (question RZ1).
    let mut v = (size as u32) & !1;
    if v != 0 && v < 4 {
        v = 4;
    }
    v.min(MAX_BUFFER)
}

/// True if `path` ends in `.lnk`, case-insensitive (§2.3).
pub fn has_lnk_extension(path: &[u8]) -> bool {
    path.len() >= 4 && path[path.len() - 4..].eq_ignore_ascii_case(b".lnk")
}

/// One cRZFile object (§2.1 layout, by meaning).
#[derive(Debug)]
pub struct RzFile {
    path: Vec<u8>,
    open: bool,
    handle: OsHandle,
    access: u32,
    creation: u32,
    share: u32,
    ref_count: u32,
    last_error: u32,
    logical: u32,
    physical: u32,
    read_cap: u32,
    read_buf: Vec<u8>,
    read_base: u32,
    read_len: u32,
    write_cap: u32,
    write_buf: Vec<u8>,
    write_base: u32,
    write_len: u32,
}

impl Default for RzFile {
    fn default() -> Self {
        Self::new()
    }
}

impl RzFile {
    /// Default constructor: empty path (§2.1).
    pub fn new() -> Self {
        RzFile {
            path: Vec::new(),
            open: false,
            handle: -1,
            access: 0,
            creation: 0,
            share: 0,
            ref_count: 0,
            last_error: RZ_ERROR_NONE,
            logical: 0,
            physical: 0,
            read_cap: DEFAULT_BUFFER,
            read_buf: Vec::new(),
            read_base: 0,
            read_len: 0,
            write_cap: DEFAULT_BUFFER,
            write_buf: Vec::new(),
            write_base: 0,
            write_len: 0,
        }
    }

    /// Constructor from a path (`cIGZString` or C string; `None` = null pointer leaves the path empty).
    pub fn with_path(path: Option<&[u8]>) -> Self {
        let mut f = Self::new();
        if let Some(p) = path {
            f.path = p.to_vec();
        }
        f
    }

    /// Destructor (§2.1): closes the file if open (flushing pending writes) and frees both buffers.
    pub fn destroy(mut self, host: &mut dyn RzFileHost) {
        if self.open {
            self.close(host);
        }
    }

    // ---- reference counting and identity (§2.1, §2.2 slots 0-2) ----

    pub fn query_interface(&mut self, iid: u32) -> bool {
        if iid == IID_IGZFILE || iid == IID_BASE {
            self.add_ref();
            true
        } else {
            false
        }
    }

    pub fn add_ref(&mut self) -> u32 {
        self.ref_count = self.ref_count.wrapping_add(1);
        self.ref_count
    }

    /// Release: decrements when above 1 and returns the new count; otherwise the object is destroyed
    /// and 0 returned. The caller drops the object when this returns 0.
    pub fn release(&mut self) -> u32 {
        if self.ref_count > 1 {
            self.ref_count -= 1;
            self.ref_count
        } else {
            0
        }
    }

    pub fn ref_count(&self) -> u32 {
        self.ref_count
    }

    /// The +0x30 error field (§2.8; no vtable slot returns it).
    pub fn last_error(&self) -> u32 {
        self.last_error
    }

    fn record(&mut self, err: OsError) {
        self.last_error = map_os_error(err);
    }

    // ---- path (§2.2 slots 24-28) ----

    pub fn path(&self) -> &[u8] {
        &self.path
    }

    /// SetPath(const char*): ignored when null. Does not close an open file.
    pub fn set_path(&mut self, path: Option<&[u8]>) {
        if let Some(p) = path {
            self.path = p.to_vec();
        }
    }

    // ---- open / state ----

    /// Open(access, creation, share) (§2.3).
    pub fn open(&mut self, host: &mut dyn RzFileHost, access: u32, creation: u32, share: u32) -> bool {
        if self.open {
            return true;
        }
        if creation > TRUNCATE_EXISTING {
            return false;
        }
        let mut target = self.path.clone();
        if has_lnk_extension(&target) {
            if let Some(resolved) = host.resolve_shortcut(&target) {
                target = resolved;
            }
        }
        match host.open(&target, access, creation, share) {
            Ok((handle, existed)) => {
                self.handle = handle;
                self.open = true;
                self.access = access;
                self.creation = creation;
                self.share = share;
                let pos = host.tell(handle);
                self.logical = pos;
                self.physical = pos;
                self.read_len = 0;
                self.read_base = 0;
                self.write_len = 0;
                if creation == CREATE_ALWAYS || creation == OPEN_ALWAYS {
                    // "the OS code, normally 0" when the file did not exist.
                    self.last_error = if existed { RZ_ERROR_ALREADY_EXISTS } else { 0 };
                }
                true
            }
            Err(e) => {
                self.record(e);
                false
            }
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    /// GetOpenModes (slot 29): `None` when closed.
    pub fn open_modes(&self) -> Option<(u32, u32, u32)> {
        if self.open {
            Some((self.access, self.creation, self.share))
        } else {
            None
        }
    }

    /// Position (slot 6): logical position, NPOS when closed.
    pub fn position(&self) -> u32 {
        if self.open {
            self.logical
        } else {
            NPOS
        }
    }

    /// Physical (OS) position, +0x4C.
    pub fn physical_position(&self) -> u32 {
        self.physical
    }

    // ---- buffers ----

    /// FlushWriteBuffer (§2.6).
    fn flush_write_buffer(&mut self, host: &mut dyn RzFileHost) -> bool {
        if self.write_len == 0 {
            return true;
        }
        let n = self.write_len as usize;
        self.write_len = 0;
        match host.write(self.handle, &self.write_buf[..n]) {
            Ok(k) => {
                self.physical = self.physical.wrapping_add(k as u32);
                true
            }
            Err(e) => {
                self.physical = host.tell(self.handle);
                self.record(e);
                false
            }
        }
    }

    pub fn buffer_sizes(&self) -> (u32, u32) {
        (self.read_cap, self.write_cap)
    }

    /// Pending bytes in the write buffer (+0x7C).
    pub fn pending_writes(&self) -> u32 {
        self.write_len
    }

    /// SetBufferSizes (§2.5). Always true.
    pub fn set_buffer_sizes(&mut self, host: &mut dyn RzFileHost, read: i32, write: i32) -> bool {
        let r = normalize_buffer_size(read);
        let w = normalize_buffer_size(write);
        if r < self.read_cap {
            self.read_len = 0;
        }
        self.read_cap = r;
        self.read_buf.truncate(r as usize);
        if w < self.write_cap {
            if self.open {
                self.flush_write_buffer(host);
            } else {
                self.write_len = 0;
            }
        }
        self.write_cap = w;
        self.write_buf.truncate(w as usize);
        true
    }

    // ---- seek (§2.4) ----

    pub fn seek(&mut self, host: &mut dyn RzFileHost, offset: i32, mode: u32) -> u32 {
        if !self.open {
            return NPOS;
        }
        let (offset, from) = match mode {
            SEEK_BEGIN => (offset, SeekFrom::Begin),
            SEEK_CURRENT if self.logical != self.physical => {
                (self.logical.wrapping_add(offset as u32) as i32, SeekFrom::Begin)
            }
            SEEK_CURRENT => (offset, SeekFrom::Current),
            SEEK_END => (offset, SeekFrom::End),
            _ => return self.position(),
        };
        self.flush_write_buffer(host);
        match host.seek(self.handle, offset, from) {
            Ok(pos) => {
                self.logical = pos;
                self.physical = pos;
                pos
            }
            Err(e) => {
                let pos = host.tell(self.handle);
                self.logical = pos;
                self.physical = pos;
                self.record(e);
                NPOS
            }
        }
    }

    pub fn seek_to_begin(&mut self, host: &mut dyn RzFileHost) -> u32 {
        self.seek(host, 0, SEEK_BEGIN)
    }
    pub fn seek_to_end(&mut self, host: &mut dyn RzFileHost) -> u32 {
        self.seek(host, 0, SEEK_END)
    }
    pub fn seek_relative(&mut self, host: &mut dyn RzFileHost, n: i32) -> u32 {
        self.seek(host, n, SEEK_CURRENT)
    }
    pub fn seek_to_position(&mut self, host: &mut dyn RzFileHost, n: i32) -> u32 {
        self.seek(host, n, SEEK_BEGIN)
    }

    /// Moves the OS pointer to the logical position if they differ. Failure: error recorded, false.
    fn sync_physical(&mut self, host: &mut dyn RzFileHost) -> bool {
        if self.physical == self.logical {
            return true;
        }
        match host.seek(self.handle, self.logical as i32, SeekFrom::Begin) {
            Ok(pos) => {
                self.physical = pos;
                true
            }
            Err(e) => {
                self.physical = host.tell(self.handle);
                self.record(e);
                false
            }
        }
    }

    // ---- read (§2.5) ----

    /// ReadWithCount: `n` is in/out (requested / delivered).
    pub fn read_with_count(&mut self, host: &mut dyn RzFileHost, buf: &mut [u8], n: &mut u32) -> bool {
        if !self.open {
            return false;
        }
        let want = (*n as usize).min(buf.len());
        if want == 0 {
            *n = 0;
            return true;
        }
        self.flush_write_buffer(host);
        if self.read_cap == 0 {
            return match host.read(self.handle, &mut buf[..want]) {
                Ok(k) => {
                    *n = k as u32;
                    self.logical = self.logical.wrapping_add(k as u32);
                    self.physical = self.physical.wrapping_add(k as u32);
                    true
                }
                Err(e) => {
                    *n = 0;
                    self.record(e);
                    false
                }
            };
        }
        let mut done = 0usize;
        if self.read_len > 0
            && self.logical >= self.read_base
            && self.logical < self.read_base.wrapping_add(self.read_len)
        {
            let start = (self.logical - self.read_base) as usize;
            let take = (self.read_len as usize - start).min(want);
            buf[..take].copy_from_slice(&self.read_buf[start..start + take]);
            self.logical = self.logical.wrapping_add(take as u32);
            done = take;
        }
        while done < want {
            self.read_len = 0;
            // Failure of the re-seek or the fill: spec says "ends with false"; n reports the bytes
            // already delivered (literal reading, question RZ2).
            if !self.sync_physical(host) {
                *n = done as u32;
                return false;
            }
            let cap = self.read_cap as usize;
            if self.read_buf.len() < cap {
                self.read_buf.resize(cap, 0);
            }
            let filled = match host.read(self.handle, &mut self.read_buf[..cap]) {
                Ok(k) => k,
                Err(e) => {
                    self.record(e);
                    *n = done as u32;
                    return false;
                }
            };
            self.read_base = self.physical;
            self.read_len = filled as u32;
            self.physical = self.physical.wrapping_add(filled as u32);
            if filled == 0 {
                break;
            }
            let take = filled.min(want - done);
            buf[done..done + take].copy_from_slice(&self.read_buf[..take]);
            self.logical = self.logical.wrapping_add(take as u32);
            done += take;
        }
        *n = done as u32;
        true
    }

    /// Read (slot 14): true only if it succeeded and all bytes arrived.
    pub fn read(&mut self, host: &mut dyn RzFileHost, buf: &mut [u8]) -> bool {
        let mut n = buf.len() as u32;
        self.read_with_count(host, buf, &mut n) && n as usize == buf.len()
    }

    // ---- write (§2.6) ----

    /// WriteWithCount: `n` is in/out.
    pub fn write_with_count(&mut self, host: &mut dyn RzFileHost, data: &[u8], n: &mut u32) -> bool {
        if !self.open {
            return false;
        }
        let len = (*n as usize).min(data.len());
        if self.read_len > 0 {
            self.read_len = 0;
            // Spec silent on a failing move-back; literal: the error is recorded and the write goes
            // on at the OS position (question RZ2).
            self.sync_physical(host);
        }
        if self.write_cap == 0 {
            return match host.write(self.handle, &data[..len]) {
                Ok(k) => {
                    *n = k as u32;
                    self.logical = self.logical.wrapping_add(k as u32);
                    self.physical = self.physical.wrapping_add(k as u32);
                    true
                }
                Err(e) => {
                    *n = 0;
                    self.record(e);
                    false
                }
            };
        }
        let cap = self.write_cap as usize;
        if self.write_buf.len() < cap {
            self.write_buf.resize(cap, 0);
        }
        let mut ok = true;
        let mut done = 0usize;
        while done < len {
            if self.write_len == 0 {
                self.write_base = self.physical;
            }
            let at = self.write_len as usize;
            let take = (cap - at).min(len - done);
            self.write_buf[at..at + take].copy_from_slice(&data[done..done + take]);
            self.write_len += take as u32;
            done += take;
            if self.write_len as usize == cap && !self.flush_write_buffer(host) {
                ok = false;
                break;
            }
        }
        self.logical = self.logical.wrapping_add(len as u32);
        ok
    }

    /// Write (slot 16): true only if it succeeded and the count is unchanged.
    pub fn write(&mut self, host: &mut dyn RzFileHost, data: &[u8]) -> bool {
        let mut n = data.len() as u32;
        self.write_with_count(host, data, &mut n) && n as usize == data.len()
    }

    /// File offset the write buffer flushes to (+0x78).
    pub fn write_base(&self) -> u32 {
        self.write_base
    }

    // ---- close, length, set length, flush (§2.7) ----

    pub fn close(&mut self, host: &mut dyn RzFileHost) -> bool {
        if !self.open {
            return false;
        }
        self.flush_write_buffer(host);
        self.read_len = 0;
        self.write_len = 0;
        if let Err(e) = host.close(self.handle) {
            self.record(e);
        }
        self.handle = -1;
        self.access = 0;
        self.creation = 0;
        self.share = 0;
        self.physical = 0;
        self.open = false;
        true
    }

    pub fn length(&mut self, host: &mut dyn RzFileHost) -> u32 {
        if !self.open {
            return NPOS;
        }
        match host.size(self.handle) {
            Ok(size) => {
                if self.write_len > 0 && size < self.logical {
                    self.logical
                } else {
                    size
                }
            }
            Err(e) => {
                self.record(e);
                NPOS
            }
        }
    }

    pub fn set_length(&mut self, host: &mut dyn RzFileHost, n: u32) -> bool {
        if !self.open {
            return false;
        }
        self.read_len = 0;
        self.flush_write_buffer(host);
        let mut ok = true;
        if let Err(e) = host.seek(self.handle, n as i32, SeekFrom::Begin) {
            self.record(e);
            ok = false;
        } else if let Err(e) = host.set_end(self.handle) {
            self.record(e);
            ok = false;
        }
        let pos = host.tell(self.handle);
        self.logical = pos;
        self.physical = pos;
        ok
    }

    pub fn flush(&mut self, host: &mut dyn RzFileHost) -> bool {
        if !self.open {
            return false;
        }
        let pending = self.write_len > 0;
        self.flush_write_buffer(host);
        if pending {
            if let Err(e) = host.commit(self.handle) {
                self.record(e);
                return false;
            }
        }
        true
    }

    // ---- path-level operations (§2.9) ----

    pub fn remove(&mut self, host: &mut dyn RzFileHost) -> bool {
        if self.open {
            self.close(host);
        }
        match host.delete(&self.path) {
            Ok(()) => true,
            Err(e) => {
                self.record(e);
                false
            }
        }
    }

    /// Rename: the stored path becomes `new_path` whether or not the move succeeded.
    pub fn rename(&mut self, host: &mut dyn RzFileHost, new_path: &[u8]) -> bool {
        if self.open {
            self.close(host);
        }
        let result = host.rename(&self.path, new_path);
        self.path = new_path.to_vec();
        match result {
            Ok(()) => true,
            Err(e) => {
                self.record(e);
                false
            }
        }
    }

    pub fn exists(&mut self, host: &mut dyn RzFileHost) -> bool {
        host.exists(&self.path)
    }

    /// CopyFrom (slots 22, 23): copies `source_path` onto this path; the object is not opened.
    pub fn copy_from(&mut self, host: &mut dyn RzFileHost, source_path: &[u8], overwrite: bool) -> bool {
        match host.copy(source_path, &self.path, !overwrite) {
            Ok(()) => true,
            Err(e) => {
                self.record(e);
                false
            }
        }
    }

    /// CopyFrom(cIGZFile&) (slot 22): the source object's path.
    pub fn copy_from_file(&mut self, host: &mut dyn RzFileHost, source: &RzFile, overwrite: bool) -> bool {
        let src = source.path.clone();
        self.copy_from(host, &src, overwrite)
    }
}

// ---- static helpers (§2.10) ----

/// Attribute flags of GetAttribute / SetAttribute (§2.10).
pub const ATTR_WRITABLE: u32 = 0x02;
pub const ATTR_DIRECTORY: u32 = 0x08;
pub const ATTR_SHORTCUT: u32 = 0x10;
pub const ATTR_ARCHIVE: u32 = 0x20;
pub const ATTR_HIDDEN: u32 = 0x40;
pub const ATTR_SYSTEM: u32 = 0x80;
pub const ATTR_NO_SHORTCUT_CHECK: u32 = 0x400;
pub const ATTR_READ_ONLY: u32 = 0x1000;

/// FileCreate(path, truncate) (0x00919DC2).
pub fn file_create(host: &mut dyn RzFileHost, path: &[u8], truncate: bool) -> bool {
    let mut f = RzFile::with_path(Some(path));
    if !f.open(host, ACCESS_READ, OPEN_ALWAYS, SHARE_READ_WRITE) {
        return false;
    }
    if truncate {
        f.set_length(host, 0);
    }
    f.close(host);
    true
}

/// GetAttribute(path, flags) (0x00919F17).
pub fn get_attribute(host: &mut dyn RzFileHost, path: &[u8], flags: u32) -> bool {
    let a = host.attributes(path).unwrap_or(OsAttributes::ALL);
    if flags & ATTR_WRITABLE != 0 && a.read_only {
        return false;
    }
    if flags & ATTR_DIRECTORY != 0 && !a.directory {
        return false;
    }
    if flags & ATTR_SHORTCUT != 0 && flags & ATTR_NO_SHORTCUT_CHECK == 0 {
        // "resolvable .lnk shortcut": literal reading = the extension plus a successful resolution.
        if !has_lnk_extension(path) || host.resolve_shortcut(path).is_none() {
            return false;
        }
    }
    if flags & ATTR_ARCHIVE != 0 && !a.archive {
        return false;
    }
    if flags & ATTR_HIDDEN != 0 && !a.hidden {
        return false;
    }
    if flags & ATTR_SYSTEM != 0 && !a.system {
        return false;
    }
    if flags & ATTR_READ_ONLY != 0 && !a.read_only {
        return false;
    }
    true
}

/// SetAttribute(path, flags, on) (0x0091A035).
pub fn set_attribute(host: &mut dyn RzFileHost, path: &[u8], flags: u32, on: bool) -> bool {
    if flags & (ATTR_DIRECTORY | ATTR_SHORTCUT) != 0 {
        return false;
    }
    // Missing file: read as all set (same rule as GetAttribute); the OS call then fails.
    let old = host.attributes(path).unwrap_or(OsAttributes::ALL);
    let mut new = old;
    if flags & ATTR_WRITABLE != 0 {
        new.read_only = !on;
    }
    if flags & ATTR_ARCHIVE != 0 {
        new.archive = on;
    }
    if flags & ATTR_HIDDEN != 0 {
        new.hidden = on;
    }
    if flags & ATTR_SYSTEM != 0 {
        new.system = on;
    }
    if flags & ATTR_READ_ONLY != 0 {
        new.read_only = on;
    }
    if new == old {
        return true;
    }
    host.set_attributes(path, new).is_ok()
}

pub fn remove_path(host: &mut dyn RzFileHost, path: &[u8]) -> bool {
    host.delete(path).is_ok()
}

pub fn rename_path(host: &mut dyn RzFileHost, old: &[u8], new: &[u8]) -> bool {
    host.rename(old, new).is_ok()
}

pub fn copy_path(host: &mut dyn RzFileHost, src: &[u8], dst: &[u8], overwrite: bool) -> bool {
    host.copy(src, dst, !overwrite).is_ok()
}

/// GetLength(path): size of the first directory match, NPOS if none.
pub fn get_length(host: &mut dyn RzFileHost, path: &[u8]) -> u32 {
    host.first_match_size(path).unwrap_or(NPOS)
}

/// IsFileWritable(path) (0x0091AC34).
pub fn is_file_writable(host: &mut dyn RzFileHost, path: &[u8]) -> bool {
    if !host.exists(path) {
        // Spec gives no modes for the trial create; literal reading: write access, create-new,
        // exclusive (question RZ3).
        return match host.open(path, ACCESS_WRITE, CREATE_NEW, 0) {
            Ok((h, _)) => {
                let _ = host.close(h);
                let _ = host.delete(path);
                true
            }
            Err(_) => false,
        };
    }
    match host.open(path, ACCESS_WRITE, OPEN_EXISTING, SHARE_READ) {
        Ok((h, _)) => {
            let _ = host.close(h);
            true
        }
        Err(_) => false,
    }
}

pub fn get_temp_directory(host: &mut dyn RzFileHost) -> Vec<u8> {
    host.temp_directory()
}
