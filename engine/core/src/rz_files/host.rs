//! Host contract for the RZ file owners (docs/specs/framework-rz-files.md §1-§3).
//!
//! The spec describes cRZFile in terms of OS calls (open, read, write, move the file pointer, set end
//! of file, commit, delete, move, copy, shortcut resolution, attributes) and cRZIniFile adds a sleep and
//! the C `time` clock for its open retry (§3.4). Everything OS-specific goes through this trait so the
//! core stays deterministic and testable with an in-memory host. Paths are byte strings in the
//! framework encoding (UTF-8, §1); converting them to the OS encoding is the host's job.

/// OS error code as the OS reports it (the file object maps 5 and 183, §2.8).
pub type OsError = u32;

/// OS error "access denied" (§2.8).
pub const OS_ERROR_ACCESS_DENIED: OsError = 5;
/// OS error "already exists" (§2.8).
pub const OS_ERROR_ALREADY_EXISTS: OsError = 183;

/// Opaque OS handle. The file object stores -1 while closed (§2.1).
pub type OsHandle = i64;

/// Origin of an OS file-pointer move.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeekFrom {
    Begin,
    Current,
    End,
}

/// File attributes the static helpers read and write (§2.10). A missing file reads as all set.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OsAttributes {
    pub read_only: bool,
    pub directory: bool,
    pub archive: bool,
    pub hidden: bool,
    pub system: bool,
}

impl OsAttributes {
    /// "A missing file reads as all attribute bits set" (§2.10).
    pub const ALL: OsAttributes = OsAttributes {
        read_only: true,
        directory: true,
        archive: true,
        hidden: true,
        system: true,
    };
}

/// The OS services cRZFile and cRZIniFile use.
pub trait RzFileHost {
    /// Opens `path` with the framework modes (§2.3: access bits 1 read / 2 write; creation 0..=4; share
    /// bits 1 read / 2 read+write). On success returns the handle and whether the file already existed
    /// (only consulted for creation modes 1 and 3). The OS position of a new handle is 0.
    fn open(&mut self, path: &[u8], access: u32, creation: u32, share: u32) -> Result<(OsHandle, bool), OsError>;
    fn close(&mut self, handle: OsHandle) -> Result<(), OsError>;
    /// One OS read at the OS position; returns the bytes read (0 at end of file).
    fn read(&mut self, handle: OsHandle, buf: &mut [u8]) -> Result<usize, OsError>;
    /// One OS write at the OS position; returns the bytes written.
    fn write(&mut self, handle: OsHandle, buf: &[u8]) -> Result<usize, OsError>;
    /// Moves the OS file pointer; returns the new absolute position.
    fn seek(&mut self, handle: OsHandle, offset: i32, from: SeekFrom) -> Result<u32, OsError>;
    /// Current OS file pointer (the "re-read from the OS" of §2.4 / §2.6).
    fn tell(&mut self, handle: OsHandle) -> u32;
    /// OS file size (32-bit, §2.7).
    fn size(&mut self, handle: OsHandle) -> Result<u32, OsError>;
    /// Sets end of file at the OS position (§2.7 SetLength).
    fn set_end(&mut self, handle: OsHandle) -> Result<(), OsError>;
    /// Asks the OS to commit file buffers (§2.7 Flush).
    fn commit(&mut self, handle: OsHandle) -> Result<(), OsError>;
    fn delete(&mut self, path: &[u8]) -> Result<(), OsError>;
    /// Moves a file (with the wide API the move may copy across volumes, §2.9).
    fn rename(&mut self, old: &[u8], new: &[u8]) -> Result<(), OsError>;
    /// Copies a file; `fail_if_exists` = the caller's `overwrite` false (§2.9, §2.10).
    fn copy(&mut self, src: &[u8], dst: &[u8], fail_if_exists: bool) -> Result<(), OsError>;
    /// True if `path` names an existing file (§2.9 Exists).
    fn exists(&mut self, path: &[u8]) -> bool;
    /// Resolves a Windows shell shortcut; `None` when resolution fails (§2.3).
    fn resolve_shortcut(&mut self, path: &[u8]) -> Option<Vec<u8>>;
    /// Attributes of `path`; `None` when it does not exist (§2.10).
    fn attributes(&mut self, path: &[u8]) -> Option<OsAttributes>;
    fn set_attributes(&mut self, path: &[u8], attrs: OsAttributes) -> Result<(), OsError>;
    /// Size of the first directory match for `path` (§2.10 GetLength).
    fn first_match_size(&mut self, path: &[u8]) -> Option<u32>;
    /// The OS temporary directory, long-name form (§2.10).
    fn temp_directory(&mut self) -> Vec<u8>;
    /// Sleeps the current thread (§3.4 retry: 250,000 microseconds).
    fn sleep_micros(&mut self, micros: u32);
    /// C `time()`: whole seconds (§3.4 retry clock).
    fn time_seconds(&mut self) -> i64;
}
