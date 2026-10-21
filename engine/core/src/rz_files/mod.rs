//! RZ framework file owners: cRZFile and cRZIniFile (docs/specs/framework-rz-files.md §1-§5).
//!
//! Pure Rust; every OS detail goes through [`host::RzFileHost`]. Open questions: RZ1-RZ8 in
//! docs/specs/questions/framework-rz-files.md.

pub mod file;
pub mod host;
pub mod ini;

#[cfg(test)]
mod tests;

pub use file::RzFile;
pub use host::{OsAttributes, OsError, OsHandle, RzFileHost, SeekFrom};
pub use ini::{BinaryAlloc, RzIniFile, SharedFile};
