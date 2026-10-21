//! `cRZLanguageUtility` (language-utility.md §1..§5).

use super::names::{self, NameRow};
use super::{LocaleHost, COUNTRY_US, UNTRANSLATED};

/// U+00A0 NO-BREAK SPACE in UTF-8, the group-space string and the money space.
const NBSP: &[u8] = b"\xC2\xA0";

/// Interface IDs answered by `QueryInterface` (§1).
pub const IID_LANGUAGE_UTILITY: u32 = 0x1054_B7D5;
pub const IID_UNKNOWN: u32 = 1;

/// Result of [`LanguageUtility::release`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Released {
    /// Count was > 1: decremented to this value.
    Alive(u32),
    /// Count was <= 1: the deleting destructor runs; `Release` returns 0.
    Destroy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageUtility {
    ref_count: u32,
    language: u32,
    country: u32,
    /// Byte +0x10. No writer is known (§1, Q1); kept settable for tests.
    currency_flag: u8,
    group_space: Vec<u8>,
}

impl Default for LanguageUtility {
    fn default() -> Self {
        Self::new()
    }
}

fn contains(ids: &[u32], id: u32) -> bool {
    ids.contains(&id)
}

/// `%u` of a signed int (two's-complement reinterpretation).
fn fmt_u(v: i32) -> String {
    (v as u32).to_string()
}

/// `%.2u`: at least two digits, zero padded.
fn fmt_u2(v: i32) -> String {
    format!("{:02}", v as u32)
}

/// Replace every `%%` with `%` (§4.4 step 3).
fn unescape_percent(text: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len());
    let mut i = 0;
    while i < text.len() {
        if text[i] == b'%' && i + 1 < text.len() && text[i + 1] == b'%' {
            out.push(b'%');
            i += 2;
        } else {
            out.push(text[i]);
            i += 1;
        }
    }
    out
}

impl LanguageUtility {
    /// Constructor: count 0, English, United States, flag 0, group space U+00A0.
    pub fn new() -> Self {
        Self {
            ref_count: 0,
            language: 1,
            country: COUNTRY_US,
            currency_flag: 0,
            group_space: NBSP.to_vec(),
        }
    }

    // ---- §1 identity and lifetime -------------------------------------

    /// Slot 0: true for [`IID_LANGUAGE_UTILITY`] and [`IID_UNKNOWN`] (the
    /// caller then stores the object and the count is incremented here).
    pub fn query_interface(&mut self, iid: u32) -> bool {
        if iid == IID_LANGUAGE_UTILITY || iid == IID_UNKNOWN {
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

    /// Slot 2: count > 1 decrements and returns it; otherwise destroy.
    pub fn release(&mut self) -> Released {
        if self.ref_count > 1 {
            self.ref_count -= 1;
            Released::Alive(self.ref_count)
        } else {
            Released::Destroy
        }
    }

    pub fn ref_count(&self) -> u32 {
        self.ref_count
    }

    /// Slot 3: reset the group-space string to U+00A0 (the UTF-8 to framework
    /// conversion is a no-op with code page 8). Returns true.
    pub fn init(&mut self) -> bool {
        self.group_space = NBSP.to_vec();
        true
    }

    /// Slot 4: shared shutdown body, not described by the spec; returns true
    /// (literal reading; question LU3).
    pub fn shutdown(&mut self) -> bool {
        true
    }

    /// Slot 31: stored verbatim.
    pub fn set_language(&mut self, id: u32) {
        self.language = id;
    }

    /// Slot 32: stored verbatim.
    pub fn set_country(&mut self, id: u32) {
        self.country = id;
    }

    pub fn language(&self) -> u32 {
        self.language
    }

    pub fn country(&self) -> u32 {
        self.country
    }

    pub fn currency_flag(&self) -> u8 {
        self.currency_flag
    }

    /// No slot writes byte +0x10 (§1, Q1). Test-only access to the flag-1
    /// branch of §3.1.
    pub fn set_currency_flag_for_test(&mut self, flag: u8) {
        self.currency_flag = flag;
    }

    // ---- §2 attribute queries -----------------------------------------

    /// Slot 23.
    pub fn character_set_from_language(&self) -> u32 {
        // 1251 is asked twice in the original (strings-language.md §3); no effect.
        const ORDER: [u32; 14] = [
            1252, 1250, 874, 932, 936, 950, 949, 1251, 1251, 1253, 1254, 1255, 1256, 1257,
        ];
        ORDER
            .iter()
            .copied()
            .find(|&cp| self.uses_character_set(cp))
            .unwrap_or(1252)
    }

    /// Slot 24.
    pub fn uses_character_set(&self, cp: u32) -> bool {
        let l = self.language;
        match cp {
            1252 => matches!(l, 1..=0x0B | 0x0E | 0x1C..=0x1F | 0x23..=0x2A),
            1250 => contains(&[0x0D, 0x10, 0x1A, 0x1C, 0x1E, 0x20, 0x21], l),
            1251 => contains(&[0x0D, 0x17, 0x18], l),
            932 => l == 0x0F,
            936 => l == 0x11,
            950 => l == 0x12,
            949 => l == 0x14,
            874 => l == 0x13,
            1253 => l == 0x1B,
            1254 => l == 0x22,
            1255 => l == 0x0C,
            1256 => l == 0x16,
            _ => false,
        }
    }

    /// Slot 25: 0 for UK English and Japanese, else 1.
    pub fn road_driving_side(&self) -> u32 {
        if self.language == 2 || self.language == 0x0F {
            0
        } else {
            1
        }
    }

    /// Slot 26: reads the country.
    pub fn measurement_system(&self) -> bool {
        self.country != COUNTRY_US
    }

    /// Slot 27.
    pub fn are_first_and_last_names_reversed(&self) -> bool {
        contains(&[0x0F, 0x11, 0x12, 0x14], self.language)
    }

    /// Slot 28.
    pub fn uses_input_method_editing(&self) -> bool {
        contains(&[0x0F, 0x11, 0x12, 0x14], self.language)
    }

    /// Slot 29 with the framework code page of this build (8).
    pub fn uses_multibyte_characters(&self) -> bool {
        self.uses_multibyte_characters_for_code_page(8)
    }

    /// Slot 29 for any framework code page (only 8 is reachable).
    pub fn uses_multibyte_characters_for_code_page(&self, framework_cp: u32) -> bool {
        match framework_cp {
            8 => !(self.language == 1 || self.language == 2),
            0 | 16 => false,
            _ => contains(&[0x0F, 0x11, 0x12, 0x13, 0x14], self.language),
        }
    }

    // ---- §3 currency and separators -----------------------------------

    /// Slot 11.
    pub fn currency_symbol(&self, out: &mut Vec<u8>) -> bool {
        if self.currency_flag == 0 {
            *out = b"$".to_vec();
        } else {
            let sym: Option<&[u8]> = match self.language {
                1 | 0x11 | 0x12 => Some(b"$"),
                2 => Some("£".as_bytes()),
                3..=8 | 0x0A | 0x0B | 0x0E | 0x1B | 0x26 => Some(b"\xE2\x2C\x82\x2C\xAC"),
                9 | 0x1D => Some(b"kr"),
                0x0C => Some("₪".as_bytes()),
                0x0D => Some("б".as_bytes()),
                0x0F => Some("¥".as_bytes()),
                0x10 => Some("ł".as_bytes()),
                0x13 => Some("฿".as_bytes()),
                0x14 => Some(b"\\"),
                0x1A => Some(b"\x4B\xC4\x8D\x7B"),
                0x1C => Some(b"FT"),
                0x1E => Some(b"Lei"),
                0x21 => Some(b"Lek"),
                0x22 => Some(b"TL"),
                0x23 => Some(b"Esc."),
                _ => None, // out not assigned
            };
            if let Some(s) = sym {
                *out = s.to_vec();
            }
        }
        if out.is_empty() {
            *out = UNTRANSLATED.to_vec();
            false
        } else {
            true
        }
    }

    /// Slot 12.
    pub fn currency_symbol_precedes_amount(&self) -> bool {
        matches!(
            self.language,
            1 | 2 | 5 | 7 | 8 | 0x0A | 0x0C | 0x0F | 0x11..=0x14 | 0x22 | 0x23
        )
    }

    /// Slot 13.
    pub fn space_between_currency_symbol_and_amount(&self) -> bool {
        !matches!(self.language, 1 | 2 | 0x0A | 0x0C | 0x0F | 0x11..=0x14)
    }

    /// Slot 14.
    pub fn thousand_separator(&self, out: &mut Vec<u8>) -> bool {
        match self.language {
            1 | 2 | 0x0C | 0x0F | 0x11..=0x14 => {
                *out = b",".to_vec();
                true
            }
            3..=9 | 0x0D | 0x0E | 0x23 => {
                *out = b".".to_vec();
                true
            }
            0x0A | 0x0B | 0x10 => {
                *out = self.group_space.clone();
                true
            }
            _ => {
                *out = b",".to_vec();
                false
            }
        }
    }

    /// Slot 15.
    pub fn decimal_separator(&self, out: &mut Vec<u8>) -> bool {
        match self.language {
            1 | 2 | 0x0C | 0x0F | 0x11..=0x14 => {
                *out = b".".to_vec();
                true
            }
            3..=0x0B | 0x0D | 0x0E | 0x10 | 0x23 => {
                *out = b",".to_vec();
                true
            }
            _ => {
                *out = b".".to_vec();
                false
            }
        }
    }

    // ---- §4 formatting --------------------------------------------------

    /// §4.1: `%lld` of `magnitude`, cut into groups from the left.
    fn grouped(&self, magnitude: i64) -> Vec<u8> {
        let digits = magnitude.to_string().into_bytes();
        let mut sep = Vec::new();
        self.thousand_separator(&mut sep);
        let first = match digits.len() % 3 {
            0 => 3,
            n => n,
        };
        let mut out = Vec::with_capacity(digits.len() + 8);
        let mut i = 0;
        while i < digits.len() {
            let n = if i == 0 { first.min(digits.len()) } else { 3 };
            if i > 0 {
                out.extend_from_slice(&sep);
            }
            out.extend_from_slice(&digits[i..(i + n).min(digits.len())]);
            i += n;
        }
        out
    }

    /// Slot 18. For i64::MIN the negation stays negative and its `-` is
    /// grouped as a digit (strings-language.md §3: `--9,223,...`).
    pub fn make_number_string(&self, value: i64, out: &mut Vec<u8>) -> bool {
        if value >= 0 {
            *out = self.grouped(value);
            return true;
        }
        let g = self.grouped(value.wrapping_neg());
        match self.language {
            1..=0x0A | 0x0C | 0x0D | 0x0F..=0x14 => {
                let mut s = b"-".to_vec();
                s.extend_from_slice(&g);
                *out = s;
            }
            0x0B => {
                let mut s = g;
                s.push(b'-');
                *out = s;
            }
            // 0, 0x0E, 0x15 and above: nothing is built, out is empty.
            _ => out.clear(),
        }
        true
    }

    /// Slot 16. `symbol` None means [`Self::currency_symbol`].
    pub fn make_money_string(&self, value: i64, out: &mut Vec<u8>, symbol: Option<&[u8]>) -> bool {
        let sym = match symbol {
            Some(s) => s.to_vec(),
            None => {
                let mut s = Vec::new();
                self.currency_symbol(&mut s);
                s
            }
        };
        let g = self.grouped(if value < 0 { value.wrapping_neg() } else { value });
        let space = self.space_between_currency_symbol_and_amount();
        let positive = |g: &[u8]| {
            let mut s = Vec::new();
            if self.currency_symbol_precedes_amount() {
                s.extend_from_slice(&sym);
                if space {
                    s.extend_from_slice(NBSP);
                }
                s.extend_from_slice(g);
            } else {
                s.extend_from_slice(g);
                if space {
                    s.extend_from_slice(NBSP);
                }
                s.extend_from_slice(&sym);
            }
            s
        };
        let mut s = Vec::new();
        if value >= 0 {
            s = positive(&g);
        } else {
            match self.language {
                8 | 0x0C => {
                    s.extend_from_slice(&sym);
                    s.extend_from_slice(NBSP);
                    s.push(b'-');
                    s.extend_from_slice(&g);
                }
                0x0A => {
                    s.extend_from_slice(&sym);
                    s.push(b'-');
                    s.extend_from_slice(&g);
                }
                _ => {
                    s.push(b'-');
                    s.extend_from_slice(&positive(&g));
                }
            }
        }
        *out = s;
        true
    }

    /// Slot 17.
    pub fn make_formatted_money_string(
        &self,
        host: &dyn LocaleHost,
        value: f64,
        out: &mut Vec<u8>,
        format: &[u8],
        symbol: Option<&[u8]>,
    ) -> bool {
        out.clear();
        // Step 2: the first `%` that does not start `%%`; one within the last
        // two characters ends the search and is taken as is.
        let len = format.len();
        let mut i = 0;
        let mut start = None;
        while i < len {
            if format[i] == b'%' {
                if i + 2 >= len || format[i + 1] != b'%' {
                    start = Some(i);
                    break;
                }
                i += 2;
            } else {
                i += 1;
            }
        }
        let Some(start) = start else { return false };
        // Step 3: the conversion character (searched after the `%`).
        const CONV: &[u8] = b"fegEGcCdiouxXnpsSlLIh";
        let Some(rel) = format[start + 1..].iter().position(|c| CONV.contains(c)) else {
            return false;
        };
        let end = start + 1 + rel; // index of the conversion character
        if !b"fegEG".contains(&format[end]) {
            return false;
        }
        let prefix = unescape_percent(&format[..start]);
        let spec = &format[start..=end];
        let suffix = unescape_percent(&format[end + 1..]);
        // Step 4: `value <= 0` is negated, so 0.0 prints from -0.0 (Q3).
        let magnitude = if value <= 0.0 { -value } else { value };
        let mut number = host.sprintf_double(spec, magnitude);
        let mut sep = Vec::new();
        self.thousand_separator(&mut sep);
        let first = number
            .iter()
            .position(|c| !(c.is_ascii_digit() || *c == b' '))
            .unwrap_or(number.len());
        let mut pos = first;
        while pos >= 3 {
            pos -= 3;
            if pos == 0 || number[pos - 1] == b' ' {
                break;
            }
            number.splice(pos..pos, sep.iter().copied());
        }
        // Step 5: template from MakeMoneyString(+1 / -1); NaN counts as >= 0.
        let mut template = Vec::new();
        let unit = if value.is_nan() || value >= 0.0 { 1 } else { -1 };
        self.make_money_string(unit, &mut template, symbol);
        if let Some(p) = template.iter().position(|&c| c == b'1') {
            template.splice(p..p + 1, number);
        }
        let mut s = prefix;
        s.extend_from_slice(&template);
        s.extend_from_slice(&suffix);
        *out = s;
        true
    }

    /// Slot 19: printf-style through the string's own format call.
    pub fn make_formatted_number_string(
        &self,
        host: &dyn LocaleHost,
        value: f64,
        out: &mut Vec<u8>,
        format: &[u8],
    ) -> bool {
        *out = host.sprintf_double(format, value);
        true
    }

    /// Slot 9.
    pub fn make_time_string(&self, hour: i32, minute: i32, second: i32, out: &mut Vec<u8>) -> bool {
        let hour = if hour > 23 { hour % 24 } else { hour };
        let minute = if minute > 59 { minute % 60 } else { minute };
        let second = if second > 59 { second % 60 } else { second };
        // 0 = H:M:S, 1 = H:M, 2 = hour only.
        let parts = if second >= 0 {
            0
        } else if minute < 0 {
            2
        } else {
            1
        };
        let s = match self.language {
            1 | 0x2A => {
                let suffix = if hour > 11 { "pm" } else { "am" };
                let h = if hour == 0 {
                    12
                } else if hour > 12 {
                    hour - 12
                } else {
                    hour
                };
                match parts {
                    0 => format!("{}:{}:{} {}", fmt_u(h), fmt_u2(minute), fmt_u2(second), suffix),
                    1 => format!("{}:{} {}", fmt_u(h), fmt_u2(minute), suffix),
                    _ => format!("{} {}", fmt_u(h), suffix),
                }
            }
            2 | 3 | 4 | 0x0A | 0x10 | 0x1D => match parts {
                0 => format!("{}:{}:{}", fmt_u2(hour), fmt_u2(minute), fmt_u2(second)),
                1 => format!("{}:{}", fmt_u2(hour), fmt_u2(minute)),
                _ => format!("{}:00", fmt_u2(hour)),
            },
            5 | 0x1C => match parts {
                0 => format!("{}.{}.{}", fmt_u(hour), fmt_u2(minute), fmt_u2(second)),
                1 => format!("{}.{}", fmt_u(hour), fmt_u2(minute)),
                _ => format!("{}:00", fmt_u(hour)),
            },
            8 | 9 | 0x24 | 0x27 | 0x28 => match parts {
                0 => format!("{}.{}.{}", fmt_u2(hour), fmt_u2(minute), fmt_u2(second)),
                1 => format!("{}.{}", fmt_u2(hour), fmt_u2(minute)),
                _ => format!("{}:00", fmt_u2(hour)),
            },
            _ => match parts {
                0 => format!("{}:{}:{}", fmt_u(hour), fmt_u2(minute), fmt_u2(second)),
                1 => format!("{}:{}", fmt_u(hour), fmt_u2(minute)),
                _ => format!("{}:00", fmt_u(hour)),
            },
        };
        *out = s.into_bytes();
        true
    }

    /// Slot 10. Style > 5: false, out untouched.
    pub fn make_date_string(&self, month: u32, day: u32, year: i32, out: &mut Vec<u8>, style: u32) -> bool {
        if style > 5 {
            return false;
        }
        // Spec: "decimal year padded to at least 2 digits". Read literally as
        // C `%.2d` (sign, then at least two digits); question LU1.
        let y = if year < 0 {
            format!("-{:02}", year.unsigned_abs())
        } else {
            format!("{:02}", year)
        };
        let y = y.as_bytes();
        let m = month.to_string();
        let d = day.to_string();
        let (m, d) = (m.as_bytes(), d.as_bytes());
        let mut n = Vec::new();
        let mut a = Vec::new();
        if (2..=5).contains(&style) {
            self.month_name(month, &mut n);
            self.abbr_month_name(month, &mut a);
        }
        let cat = |parts: &[&[u8]]| parts.concat();
        let l = self.language;
        let s = match l {
            0x0F => match style {
                0 => cat(&[y, b"/", m, b"/", d]),
                1 => cat(&[y, b"/", m]),
                2 => cat(&[y, b".", &n, b".", d]),
                3 => cat(&[y, b".", &n]),
                4 => cat(&[y, b".", &a, b".", d]),
                _ => cat(&[y, b".", &a]),
            },
            0x11 | 0x12 | 0x14 => match style {
                0 => cat(&[y, b"-", m, b"-", d]),
                1 => cat(&[y, b"-", m]),
                2 => cat(&[y, b" ", &n, b" ", d]),
                3 => cat(&[y, b" ", &n]),
                4 => cat(&[y, b" ", &a, b" ", d]),
                _ => cat(&[y, b" ", &a]),
            },
            2 | 3 | 5 | 6 | 0x0D | 0x13 | 0x23 | 4 | 0x0A | 0x0B | 0x10 | 7 | 8 | 9 | 0x0E => {
                let sep: &[u8] = match l {
                    4 | 0x0A | 0x0B | 0x10 => b".",
                    7 | 8 | 9 | 0x0E => b"-",
                    _ => b"/",
                };
                match style {
                    0 => cat(&[d, sep, m, sep, y]),
                    1 => cat(&[m, sep, y]),
                    2 => cat(&[d, b" ", &n, b", ", y]),
                    3 => cat(&[&n, b" ", y]),
                    4 => cat(&[d, b" ", &a, b"., ", y]),
                    _ => cat(&[&a, b". ", y]),
                }
            }
            _ => match style {
                0 => cat(&[m, b"/", d, b"/", y]),
                1 => cat(&[m, b"/", y]),
                2 => cat(&[&n, b" ", d, b", ", y]),
                3 => cat(&[&n, b" ", y]),
                4 => cat(&[&a, b". ", d, b", ", y]),
                _ => cat(&[&a, b". ", y]),
            },
        };
        *out = s;
        true
    }

    /// Slot 20. Maps over the length of `out` on entry (Q4), clamped to the
    /// converted text (bytes past it are not specified; question LU2).
    pub fn convert_to_lower_case(&self, host: &dyn LocaleHost, input: &[u8], out: &mut Vec<u8>) -> bool {
        self.convert_case(host, input, out, false)
    }

    /// Slot 21, as slot 20 with `CharUpperBuffA`.
    pub fn convert_to_upper_case(&self, host: &dyn LocaleHost, input: &[u8], out: &mut Vec<u8>) -> bool {
        self.convert_case(host, input, out, true)
    }

    fn convert_case(&self, host: &dyn LocaleHost, input: &[u8], out: &mut Vec<u8>, upper: bool) -> bool {
        let n = out.len();
        let mut ansi = host.utf8_to_ansi(input);
        let n = n.min(ansi.len());
        if upper {
            host.char_upper_buff(&mut ansi[..n]);
        } else {
            host.char_lower_buff(&mut ansi[..n]);
        }
        *out = host.ansi_to_utf8(&ansi);
        // The return value is not stated by the spec; true (question LU2).
        true
    }

    /// Slot 22: -1, 0 or +1 (0 also when the comparison fails).
    pub fn compare_strings(&self, host: &dyn LocaleHost, a: &[u8], b: &[u8], case_sensitive: bool) -> i32 {
        let a = host.utf8_to_ansi(a);
        let b = host.utf8_to_ansi(b);
        match host.compare_ansi(&a, &b, !case_sensitive) {
            Some(core::cmp::Ordering::Less) => -1,
            Some(core::cmp::Ordering::Greater) => 1,
            _ => 0,
        }
    }

    // ---- §5 names -------------------------------------------------------

    fn name(&self, table: &[NameRow], index: u32, out: &mut Vec<u8>) -> bool {
        if let Some(s) = names::lookup(table, self.language, index) {
            *out = s.to_vec();
        }
        if out.is_empty() {
            *out = UNTRANSLATED.to_vec();
            false
        } else {
            true
        }
    }

    /// Slot 5: `day` 1 = Monday .. 7 = Sunday.
    pub fn day_name(&self, day: u32, out: &mut Vec<u8>) -> bool {
        self.name(names::DAY_NAMES, day, out)
    }

    /// Slot 6.
    pub fn abbr_day_name(&self, day: u32, out: &mut Vec<u8>) -> bool {
        self.name(names::ABBR_DAY_NAMES, day, out)
    }

    /// Slot 7.
    pub fn month_name(&self, month: u32, out: &mut Vec<u8>) -> bool {
        self.name(names::MONTH_NAMES, month, out)
    }

    /// Slot 8.
    pub fn abbr_month_name(&self, month: u32, out: &mut Vec<u8>) -> bool {
        self.name(names::ABBR_MONTH_NAMES, month, out)
    }
}
