//! Language utility (`cRZLanguageUtility`, docs/specs/language-utility.md) and
//! language manager (`cRZLanguageManager`, docs/specs/strings-language.md §4).
//!
//! All strings are byte strings: outputs are `Vec<u8>` that a call replaces
//! (or, where the spec says so, leaves alone). No Unicode handling is
//! implied; stored bytes are returned as stored.
//!
//! The parts of the contract that are the platform's C runtime or Windows ANSI
//! behaviour (printf of a double, `CharLowerBuffA`/`CharUpperBuffA`,
//! `CompareStringA`, the UTF-8/ANSI conversions, `setlocale`, `_setmbcp`,
//! `GetUserDefaultLCID`) go through [`LocaleHost`]; this module never guesses
//! them.

mod manager;
mod names;
mod utility;

#[cfg(test)]
mod tests;

pub use manager::{
    LanguageManager, ManagerInterface, IID_LANGUAGE_MANAGER, IID_SYSTEM_SERVICE, SERVICE_ID, SERVICE_PRIORITY,
};
pub use utility::{LanguageUtility, Released, IID_LANGUAGE_UTILITY, IID_UNKNOWN};

/// Language IDs (strings-language.md §1).
pub mod lang {
    pub const NONE: u32 = 0;
    pub const ENGLISH: u32 = 1;
    pub const UK_ENGLISH: u32 = 2;
    pub const FRENCH: u32 = 3;
    pub const GERMAN: u32 = 4;
    pub const JAPANESE: u32 = 0x0F;
    /// Highest named language ID.
    pub const LAST: u32 = 0x2C;
}

/// Country ID of the United States, the default country.
pub const COUNTRY_US: u32 = 0x36;

/// The platform boundary: what the original takes from the C runtime and the
/// Windows ANSI API. An implementation for the plugin uses the real calls.
pub trait LocaleHost {
    /// C `sprintf(format, value)` with one double argument, as the string's
    /// own printf-style format call does (language-utility.md §4.4, §4.5).
    fn sprintf_double(&self, format: &[u8], value: f64) -> Vec<u8>;
    /// UTF-8 to the system ANSI code page.
    fn utf8_to_ansi(&self, text: &[u8]) -> Vec<u8>;
    /// System ANSI code page to UTF-8.
    fn ansi_to_utf8(&self, text: &[u8]) -> Vec<u8>;
    /// `CharLowerBuffA` over the whole slice.
    fn char_lower_buff(&self, buf: &mut [u8]);
    /// `CharUpperBuffA` over the whole slice.
    fn char_upper_buff(&self, buf: &mut [u8]);
    /// `CompareStringA` with the user-default locale. `None` when the call
    /// fails; otherwise the ordering of `a` against `b`.
    fn compare_ansi(&self, a: &[u8], b: &[u8], ignore_case: bool) -> Option<core::cmp::Ordering>;
    /// `setlocale(LC_ALL, name)`; true when it succeeds.
    fn set_locale_all(&self, name: &[u8]) -> bool;
    /// `_setmbcp(_MB_CP_ANSI)`.
    fn set_mbcp_ansi(&self);
    /// `GetUserDefaultLCID()`.
    fn user_default_lcid(&self) -> u32;
}

/// The text a name/currency getter writes when its result is empty.
pub const UNTRANSLATED: &[u8] = b"Untranslated";
