//! `cRZLanguageManager` (strings-language.md §4).

use std::collections::BTreeSet;

use super::{LanguageUtility, LocaleHost, COUNTRY_US};

/// Service ID `kGZLanguageManagerSysServiceID` and priority (§4.1).
pub const SERVICE_ID: u32 = 0x441E_5070;
pub const SERVICE_PRIORITY: u32 = 0xFFD2_3940;
pub const IID_LANGUAGE_MANAGER: u32 = 0x0054_B7D5;
pub const IID_SYSTEM_SERVICE: u32 = 0x287F_B697;

/// What `QueryInterface` hands out (§4.1). The unknown interface (1) is not
/// answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManagerInterface {
    /// The object itself.
    Manager,
    /// The system-service sub-object (object + 4).
    SystemService,
}

/// §1 English names, index = ID - 1.
const ENGLISH_NAMES: [&str; 0x2C] = [
    "English",
    "UK English",
    "French",
    "German",
    "Italian",
    "Spanish",
    "Dutch",
    "Danish",
    "Swedish",
    "Norwegian",
    "Finnish",
    "Hebrew",
    "Russian",
    "Portuguese",
    "Japanese",
    "Polish",
    "Simplified Chinese",
    "Traditional Chinese",
    "Thai",
    "Korean",
    "Hindi",
    "Arabic",
    "Bulgarian",
    "Cyrillic",
    "Ukranian",
    "Czech",
    "Greek",
    "Hungarian",
    "Icelandic",
    "Romanian",
    "Latin",
    "Slovak",
    "Albanian",
    "Turkish",
    "Brazilian Portuguese",
    "Swiss French",
    "Canadian French",
    "Belgian French",
    "Swiss German",
    "Swiss Italian",
    "Belgian Dutch",
    "Mexican Spanish",
    "Tagalog",
    "Vietnamese",
];

/// §4.4 ISO 639 codes, index = ID - 1.
const ISO639: [&str; 0x2C] = [
    "en", "uk", "fr", "de", "it", "es", "nl", "da", "sv", "no", "fi", "iw", "ru", "pt", "ja", "pl", "zh", "zh", "th",
    "ko", "hi", "ar", "bg", "ru", "uk", "cs", "el", "hu", "is", "ro", "la", "sk", "al", "tr", "pt", "fr", "fr", "fr",
    "gr", "it", "nl", "sp", "tl", "vi",
];

/// §4.5 runtime locale names, index = ID - 1.
const RUNTIME_NAMES: [&str; 0x2C] = [
    "english-us",
    "english-uk",
    "french",
    "german",
    "italian",
    "spanish",
    "dutch",
    "danish",
    "swedish",
    "norwegian",
    "finnish",
    "hebrew",
    "russian",
    "portuguese",
    "japanese",
    "polish",
    "chinese-simplified",
    "chinese-traditional",
    "thai",
    "korean",
    "hindi",
    "arabic",
    "russian",
    "russian",
    "russian",
    "czech",
    "greek",
    "hungarian",
    "icelandic",
    "romanian",
    "latin",
    "slovak",
    "albanian",
    "turkish",
    "portuguese",
    "french-swiss",
    "french-canadian",
    "french-belgian",
    "german-swiss",
    "italian-swiss",
    "dutch-belgian",
    "spanish-mexican",
    "tagalog",
    "vietnamese",
];

/// §4.6 countries 1..0x38: (English name, ISO 3166 code; "" = none).
const COUNTRIES: [(&str, &str); 0x38] = [
    ("Albania", "al"),
    ("Argentina", "ar"),
    ("Australia", "au"),
    ("Austria", "at"),
    ("Belgium", "be"),
    ("Brazil", "br"),
    ("Bulgaria", "bg"),
    ("Canada", "ca"),
    ("Chile", "cl"),
    ("China", "cn"),
    ("Colombia", "co"),
    ("Cuba", "cu"),
    ("Czech Republic", "cz"),
    ("Denmark", "dk"),
    ("Egypt", "eg"),
    ("Finland", "fi"),
    ("France", "fr"),
    ("Georgia", "ge"),
    ("Germany", "de"),
    ("Greece", "gr"),
    ("Holy See", ""),
    ("Hong Kong", "hk"),
    ("Hungray", "hu"),
    ("Iceland", "is"),
    ("India", "in"),
    ("Iran", "ir"),
    ("Iraq", "iq"),
    ("Ireland", "ie"),
    ("Israel", "il"),
    ("Italy", "it"),
    ("Japan", "jp"),
    ("Korea", "kr"),
    ("Mexico", "mx"),
    ("Netherlands", "nl"),
    ("New Zealand", "nz"),
    ("Norway", "no"),
    ("Philippines", "ph"),
    ("Poland", "pl"),
    ("Portugal", "pt"),
    ("Romania", "ro"),
    ("Russia", "ru"),
    ("Singapore", "sg"),
    ("Slovakia", "sk"),
    ("Slovenia", "si"),
    ("Spain", "es"),
    ("Sweden", "se"),
    ("Switzerland", "ch"),
    ("Syria", "sy"),
    ("Taiwan", "tw"),
    ("Thailand", "th"),
    ("Turkey", "tr"),
    ("Ukraine", "ua"),
    ("United Kingdom", "gb"),
    ("United States", "us"),
    ("Vietnam", "vn"),
    ("Yugoslavia", "yu"),
];

/// §4.3 `GetSystemLanguageFromLanguage`, (language, LANGID).
const SYSTEM_LANGIDS: [(u32, u32); 35] = [
    (1, 0x409),
    (2, 0x809),
    (3, 0x40C),
    (4, 0x407),
    (5, 0x410),
    (6, 0x40A),
    (7, 0x413),
    (8, 0x406),
    (9, 0x41D),
    (0x0A, 0x414),
    (0x0B, 0x40B),
    (0x0C, 0x40D),
    (0x0D, 0x419),
    (0x0E, 0x816),
    (0x0F, 0x411),
    (0x10, 0x415),
    (0x11, 0x804),
    (0x12, 0x404),
    (0x13, 0x41E),
    (0x14, 0x412),
    (0x15, 0x439),
    (0x16, 0x401),
    (0x17, 0x402),
    (0x19, 0x422),
    (0x1A, 0x405),
    (0x1B, 0x408),
    (0x1C, 0x40E),
    (0x1D, 0x40F),
    (0x1E, 0x418),
    (0x20, 0x41B),
    (0x21, 0x41C),
    (0x22, 0x41F),
    (0x23, 0x416),
    (0x2A, 0x80A),
    (0x2C, 0x42A),
    // 0x18, 0x1F, 0x24..0x29, 0x2B and above: 1.
];

/// Manager-side state (§4.1).
#[derive(Debug, Clone)]
pub struct LanguageManager {
    current_language: u32,
    current_country: u32,
    available: BTreeSet<u32>,
    utility: LanguageUtility,
}

impl Default for LanguageManager {
    fn default() -> Self {
        Self::new()
    }
}

fn set(out: &mut Vec<u8>, s: &str) {
    *out = s.as_bytes().to_vec();
}

impl LanguageManager {
    /// Constructor: utility built, `Init`, `SetLanguage(1)`, `SetCountry(0x36)`.
    pub fn new() -> Self {
        let mut utility = LanguageUtility::new();
        utility.init();
        utility.set_language(1);
        utility.set_country(COUNTRY_US);
        Self {
            current_language: 1,
            current_country: COUNTRY_US,
            available: BTreeSet::new(),
            utility,
        }
    }

    /// Slots 3/4 do nothing and return true.
    pub fn init(&mut self) -> bool {
        true
    }

    pub fn shutdown(&mut self) -> bool {
        true
    }

    /// §4.1: the interface for `iid`, or None (including iid 1).
    pub fn query_interface(iid: u32) -> Option<ManagerInterface> {
        match iid {
            IID_LANGUAGE_MANAGER => Some(ManagerInterface::Manager),
            IID_SYSTEM_SERVICE => Some(ManagerInterface::SystemService),
            _ => None,
        }
    }

    fn resolve(&self, id: u32) -> u32 {
        if id == 0 {
            self.current_language
        } else {
            id
        }
    }

    fn resolve_country(&self, id: u32) -> u32 {
        if id == 0 {
            self.current_country
        } else {
            id
        }
    }

    // ---- §4.1 / §4.2 ----------------------------------------------------

    /// Slot 7.
    pub fn current_language(&self) -> u32 {
        self.current_language
    }

    /// Slot 28.
    pub fn current_country(&self) -> u32 {
        self.current_country
    }

    /// Slot 38: the embedded utility when `id` (0 = current) is the current
    /// language; no reference is added.
    pub fn language_utility(&self, id: u32) -> Option<&LanguageUtility> {
        if self.resolve(id) == self.current_language {
            Some(&self.utility)
        } else {
            None
        }
    }

    pub fn language_utility_mut(&mut self) -> &mut LanguageUtility {
        &mut self.utility
    }

    /// Slot 39: a fresh utility with count 1 and `SetLanguage(id)`; no `Init`,
    /// country 0x36. `id` is stored verbatim (0 is not resolved: the spec
    /// does not say it means current; question SL1).
    pub fn new_language_utility(&self, id: u32) -> LanguageUtility {
        let mut u = LanguageUtility::new();
        u.add_ref();
        u.set_language(id);
        u
    }

    /// Slot 8.
    pub fn set_current_language(&mut self, host: &dyn LocaleHost, id: u32) -> bool {
        let id = if id == 0 { 1 } else { id };
        if id == self.current_language {
            return true;
        }
        if !self.can_switch_to_language(id) {
            return false;
        }
        self.current_language = id;
        self.utility.set_language(id);
        let mut name = Vec::new();
        if !self.language_runtime_library_name(&mut name, id) {
            // Already switched, still false (quirk).
            return false;
        }
        if host.set_locale_all(&name) {
            host.set_mbcp_ansi();
        }
        true
    }

    /// Slot 29.
    pub fn set_current_country(&mut self, id: u32) -> bool {
        let id = if id == 0 { COUNTRY_US } else { id };
        self.current_country = id;
        self.utility.set_country(id);
        true
    }

    /// Slot 9.
    pub fn can_switch_to_language(&self, id: u32) -> bool {
        self.available.is_empty() || self.available.contains(&id)
    }

    /// Slot 5: ignored above 0x2C.
    pub fn add_available_language(&mut self, id: u32) {
        if id <= 0x2C {
            self.available.insert(id);
        }
    }

    /// Slot 6.
    pub fn remove_available_language(&mut self, id: u32) {
        self.available.remove(&id);
    }

    /// Slot 14.
    pub fn next_available_language(&self, id: &mut u32) -> bool {
        loop {
            if *id > 0x2C {
                *id = id.wrapping_add(1);
                return false;
            }
            *id += 1;
            if self.can_switch_to_language(*id) {
                return true;
            }
        }
    }

    /// Slot 47: the set in ascending order.
    pub fn all_available_languages(&self, list: &mut Vec<u32>) {
        list.clear();
        list.extend(self.available.iter().copied());
    }

    /// Slot 46: names with the double advance (skips every other language).
    pub fn available_language_names(&self, list: &mut Vec<Vec<u8>>) {
        list.clear();
        let mut id = 1u32;
        while id <= 0x2C {
            if self.next_available_language(&mut id) {
                let mut name = Vec::new();
                self.language_english_name(&mut name, id);
                list.push(name);
            }
            id = id.wrapping_add(1);
        }
    }

    // ---- §4.3 ------------------------------------------------------------

    /// Slot 10.
    pub fn current_system_language(&self, host: &dyn LocaleHost) -> u32 {
        Self::language_from_system_language(host.user_default_lcid() & 0xFFFF)
    }

    /// Slot 11.
    pub fn system_language_from_language(&self, id: u32) -> u32 {
        let id = self.resolve(id);
        SYSTEM_LANGIDS
            .iter()
            .find(|(l, _)| *l == id)
            .map_or(1, |(_, langid)| *langid)
    }

    /// Slot 12 (low 16 bits only).
    pub fn language_from_system_language(langid: u32) -> u32 {
        let l = langid & 0xFFFF;
        match l {
            0x809 | 0xC09 | 0x1409 | 0x1809 | 0x1C09 | 0x2009 | 0x2409 | 0x2809 | 0x2C09 | 0x3009 | 0x3409 => 2,
            0x40C | 0x80C | 0xC0C | 0x100C | 0x140C | 0x180C => 3,
            0x407 | 0x807 | 0xC07 | 0x1007 | 0x1407 => 4,
            0x410 | 0x810 => 5,
            0x413 | 0x813 => 7,
            0x41D | 0x81D => 9,
            0x414 | 0x814 => 0x0A,
            0x412 | 0x812 => 0x14,
            0x40A => 6,
            0xC0A..=0x500A if (l - 0xC0A) % 0x400 == 0 => 6,
            0x804 | 0x1004 | 0x1404 => 0x11,
            0x404 | 0xC04 => 0x12,
            0x401..=0x4001 if (l - 0x401) % 0x400 == 0 => 0x16,
            _ => {
                const SINGLE: [u32; 22] = [
                    8, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10, 0x13, 0x15, 0x17, 0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x20,
                    0x21, 0x22, 0x23, 0x2A, 0x2C,
                ];
                SINGLE
                    .iter()
                    .copied()
                    .find(|&lang| SYSTEM_LANGIDS.iter().any(|&(x, sys)| x == lang && sys == l))
                    .unwrap_or(1)
            }
        }
    }

    /// Slot 32.
    pub fn current_system_country(&self) -> u32 {
        0xFFFF_FFFF
    }

    // ---- §4.4 ------------------------------------------------------------

    /// Slot 16 (and 21).
    pub fn language_english_name(&self, out: &mut Vec<u8>, id: u32) -> bool {
        let id = self.resolve(id);
        match id {
            1..=0x2C => {
                set(out, ENGLISH_NAMES[id as usize - 1]);
                true
            }
            _ => {
                out.clear();
                false
            }
        }
    }

    /// Slot 17 (and 22), with the inverted emptiness test.
    pub fn language_local_name(&self, out: &mut Vec<u8>, id: u32) -> bool {
        let id = self.resolve(id);
        let entry: Option<&[u8]> = match id {
            1 | 2 => Some(b"English"),
            3 => Some(b"Fran\xC3\x87ais"),
            4 => Some(b"Deutsch"),
            5 => Some(b"Italiano"),
            6 => Some("Español".as_bytes()),
            7 => Some(b"Nederlands"),
            8 => Some(b"Dansk"),
            9 => Some(b"Svensk"),
            0x0B => Some(b"Suomi"),
            0x0E | 0x23 => Some(b"Portugues"),
            0x0A | 0x0C | 0x0D | 0x0F..=0x14 => Some(b"Untranslated"),
            _ => None,
        };
        if let Some(e) = entry {
            *out = e.to_vec();
        }
        if out.is_empty() {
            true
        } else {
            *out = super::UNTRANSLATED.to_vec();
            false
        }
    }

    /// Slot 15.
    pub fn language_directory_name(&self, out: &mut Vec<u8>, id: u32) -> bool {
        let id = self.resolve(id);
        let special = match id {
            2 => Some("UKEnglsh"),
            0x0A => Some("Norwgian"),
            0x0E => Some("Portuges"),
            0x11 => Some("SChinese"),
            0x12 => Some("TChinese"),
            0x17 => Some("Blgarian"),
            0x1C => Some("Hngarian"),
            0x1D => Some("Icelandc"),
            0x23 => Some("Portgese"),
            0x24 => Some("SFrench"),
            0x25 => Some("CFrench"),
            0x26 => Some("BFrench"),
            0x27 => Some("SGerman"),
            0x28 => Some("SItalian"),
            0x29 => Some("BDutch"),
            0x2A => Some("MSpanish"),
            0x2C => Some("Vietnmse"),
            _ => None,
        };
        match special {
            Some(s) => set(out, s),
            None => {
                self.language_english_name(out, id);
            }
        }
        !out.is_empty()
    }

    /// Slot 23.
    pub fn language_id_alias(&self, id: u32) -> u32 {
        match self.resolve(id) {
            2 => 1,
            0x24..=0x26 => 3,
            0x27 => 4,
            0x28 => 5,
            0x29 => 7,
            0x2A => 6,
            0x17..=0x19 => 0x0D,
            other => other,
        }
    }

    /// Slot 24.
    pub fn iso639_code(&self, out: &mut Vec<u8>, id: u32) -> bool {
        let id = self.resolve(id);
        match id {
            1..=0x2C => {
                set(out, ISO639[id as usize - 1]);
                true
            }
            _ => {
                out.clear();
                false
            }
        }
    }

    /// Slot 13 (§4.5): 0x2D and above leave `out` untouched.
    pub fn language_runtime_library_name(&self, out: &mut Vec<u8>, id: u32) -> bool {
        let id = self.resolve(id);
        match id {
            1..=0x2C => {
                set(out, RUNTIME_NAMES[id as usize - 1]);
                true
            }
            _ => false,
        }
    }

    // ---- §4.6 ------------------------------------------------------------

    /// Slot 34 (and 35).
    pub fn country_english_name(&self, out: &mut Vec<u8>, id: u32) -> bool {
        let id = self.resolve_country(id);
        match id {
            1..=0x38 => {
                set(out, COUNTRIES[id as usize - 1].0);
                true
            }
            _ => {
                // Spec states the fallback for IDs above 0x38; the current
                // country is never 0, so that is every other case.
                set(out, "UnitedStates");
                false
            }
        }
    }

    /// Slot 26.
    pub fn iso3166_code(&self, out: &mut Vec<u8>, id: u32) -> bool {
        let id = self.resolve_country(id);
        let code = match id {
            1..=0x38 => COUNTRIES[id as usize - 1].1,
            _ => "",
        };
        set(out, code);
        !code.is_empty()
    }

    /// Slot 30.
    pub fn primary_language_for_country(&self, country: u32) -> u32 {
        match self.resolve_country(country) {
            8 | 0x0F | 0x19 | 0x2C | 0x30 | 0x36 => 1,
            2 | 9 | 0x0B | 0x0C | 0x2D => 6,
            3 | 0x1C | 0x23 | 0x2F | 0x35 => 2,
            4 | 0x13 => 4,
            5 => 0x26,
            6 => 0x23,
            7 => 0x17,
            0x0A | 0x2A => 0x11,
            0x0D => 0x1A,
            0x0E => 8,
            0x10 => 0x0B,
            0x12 | 0x29 => 0x0D,
            0x14 => 0x1B,
            0x16 | 0x31 => 0x12,
            0x17 => 0x1C,
            0x18 => 0x1D,
            0x1A | 0x1B => 0x16,
            0x1D => 0x0C,
            0x1E => 5,
            0x1F => 0x0F,
            0x20 => 0x14,
            0x21 => 0x2A,
            0x22 => 7,
            0x24 => 0x0A,
            0x25 => 0x2B,
            0x26 => 0x10,
            0x27 => 0x0E,
            0x28 => 0x1E,
            0x2B | 0x38 => 0x20,
            0x2E => 9,
            0x32 => 0x13,
            0x34 => 0x19,
            0x37 => 0x2C,
            _ => 0,
        }
    }

    /// Slot 31.
    pub fn default_country_for_language(&self, language: u32) -> u32 {
        match self.resolve(language) {
            2 => 0x35,
            3 => 0x11,
            4 => 0x13,
            5 | 0x1F => 0x1E,
            6 => 0x2D,
            7 => 0x22,
            8 => 0x0E,
            9 => 0x2E,
            0x0A => 0x24,
            0x0B => 0x10,
            0x0C => 0x1D,
            0x0D | 0x18 => 0x29,
            0x0E => 0x27,
            0x0F => 0x1F,
            0x10 => 0x26,
            0x11 => 0x0A,
            0x12 => 0x16,
            0x13 => 0x32,
            0x14 => 0x20,
            0x15 => 0x19,
            0x16 => 0x1A,
            0x17 => 7,
            0x19 => 0x34,
            0x1A => 0x0D,
            0x1B => 0x14,
            0x1C => 0x17,
            0x1D => 0x18,
            0x1E => 0x28,
            0x20 => 0x2B,
            0x21 => 1,
            0x22 => 0x33,
            0x23 => 6,
            0x24 | 0x27 | 0x28 => 0x2F,
            0x25 => 8,
            0x26 | 0x29 => 5,
            0x2A => 0x21,
            0x2B => 0x25,
            0x2C => 0x37,
            _ => COUNTRY_US,
        }
    }

    // ---- §4.7 reverse lookups -------------------------------------------

    fn reverse(
        &self,
        host: &dyn LocaleHost,
        text: &[u8],
        last: u32,
        fallback: u32,
        id: &mut u32,
        getter: impl Fn(&Self, &mut Vec<u8>, u32) -> bool,
    ) -> bool {
        for candidate in 1..=last {
            let mut entry = Vec::new();
            if !getter(self, &mut entry, candidate) {
                continue;
            }
            if self.utility.compare_strings(host, text, &entry, false) == 0 {
                *id = candidate;
                return true;
            }
        }
        *id = fallback;
        false
    }

    /// Slot 18.
    pub fn language_id_from_english_name(&self, host: &dyn LocaleHost, text: &[u8], id: &mut u32) -> bool {
        self.reverse(host, text, 0x2C, 1, id, Self::language_english_name)
    }

    /// Slot 19 (never matches: every local-name call is false).
    pub fn language_id_from_local_name(&self, host: &dyn LocaleHost, text: &[u8], id: &mut u32) -> bool {
        self.reverse(host, text, 0x2C, 1, id, Self::language_local_name)
    }

    /// Slot 25.
    pub fn language_id_from_iso639_code(&self, host: &dyn LocaleHost, text: &[u8], id: &mut u32) -> bool {
        self.reverse(host, text, 0x2C, 1, id, Self::iso639_code)
    }

    /// Slot 27.
    pub fn country_id_from_iso3166_code(&self, host: &dyn LocaleHost, text: &[u8], id: &mut u32) -> bool {
        self.reverse(host, text, 0x39, COUNTRY_US, id, Self::iso3166_code)
    }

    /// Slots 36 and 37.
    pub fn country_id_from_english_name(&self, host: &dyn LocaleHost, text: &[u8], id: &mut u32) -> bool {
        self.reverse(host, text, 0x39, COUNTRY_US, id, Self::country_english_name)
    }

    /// Slot 20. The original recurses into itself without end; this is the
    /// spec's *[choice]*: English-name lookup, then local-name lookup, and on
    /// success the matched entry is written back into `text`.
    pub fn language_id_from_either_name(&self, host: &dyn LocaleHost, text: &mut Vec<u8>, id: &mut u32) -> bool {
        if self.language_id_from_english_name(host, text, id) {
            let mut name = Vec::new();
            self.language_english_name(&mut name, *id);
            *text = name;
            return true;
        }
        if self.language_id_from_local_name(host, text, id) {
            let mut name = Vec::new();
            self.language_local_name(&mut name, *id);
            *text = name;
            return true;
        }
        false
    }

    // ---- §4.8 stubs -------------------------------------------------------

    /// Slot 40.
    pub fn exchange_rate_for_locale(&self, _locale: u32) -> f32 {
        1.0
    }

    /// Slots 42..45: the amount is copied unchanged; true.
    pub fn convert_money<T: Copy>(&self, amount: T, out: &mut T) -> bool {
        *out = amount;
        true
    }
}
