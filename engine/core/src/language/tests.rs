//! Tests from the worked examples and tables of language-utility.md and
//! strings-language.md §4.

use std::cell::RefCell;

use super::utility::IID_LANGUAGE_UTILITY;
use super::*;

/// Test host. Not the game's platform: `%[w][.p]f` only, ASCII case
/// mapping, byte-wise comparison, identity ANSI conversion.
#[derive(Default)]
struct TestHost {
    locales: RefCell<Vec<Vec<u8>>>,
    mbcp: RefCell<u32>,
    lcid: u32,
    reject_locale: bool,
}

impl LocaleHost for TestHost {
    fn sprintf_double(&self, format: &[u8], value: f64) -> Vec<u8> {
        let f = std::str::from_utf8(format).unwrap();
        let p = f.find('%').unwrap();
        let e = p + f[p..].find('f').unwrap();
        let spec = &f[p + 1..e];
        let (w, prec) = match spec.split_once('.') {
            Some((w, pr)) => (w.parse().unwrap_or(0), pr.parse().unwrap_or(0)),
            None => (spec.parse().unwrap_or(0), 6),
        };
        let n = format!("{:>w$.prec$}", value, w = w, prec = prec);
        format!("{}{}{}", &f[..p], n, &f[e + 1..]).into_bytes()
    }
    fn utf8_to_ansi(&self, t: &[u8]) -> Vec<u8> {
        t.to_vec()
    }
    fn ansi_to_utf8(&self, t: &[u8]) -> Vec<u8> {
        t.to_vec()
    }
    fn char_lower_buff(&self, b: &mut [u8]) {
        b.make_ascii_lowercase()
    }
    fn char_upper_buff(&self, b: &mut [u8]) {
        b.make_ascii_uppercase()
    }
    fn compare_ansi(&self, a: &[u8], b: &[u8], ignore_case: bool) -> Option<core::cmp::Ordering> {
        if ignore_case {
            Some(a.to_ascii_lowercase().cmp(&b.to_ascii_lowercase()))
        } else {
            Some(a.cmp(b))
        }
    }
    fn set_locale_all(&self, name: &[u8]) -> bool {
        self.locales.borrow_mut().push(name.to_vec());
        !self.reject_locale && name != b"latin"
    }
    fn set_mbcp_ansi(&self) {
        *self.mbcp.borrow_mut() += 1;
    }
    fn user_default_lcid(&self) -> u32 {
        self.lcid
    }
}

fn util(lang: u32) -> LanguageUtility {
    let mut u = LanguageUtility::new();
    u.set_language(lang);
    u
}

fn num(lang: u32, v: i64) -> Vec<u8> {
    let mut o = b"junk".to_vec();
    assert!(util(lang).make_number_string(v, &mut o));
    o
}

fn money(lang: u32, v: i64) -> Vec<u8> {
    let mut o = Vec::new();
    assert!(util(lang).make_money_string(v, &mut o, None));
    o
}

fn fmoney(lang: u32, v: f64, f: &str) -> (bool, Vec<u8>) {
    let mut o = b"old".to_vec();
    let r = util(lang).make_formatted_money_string(&TestHost::default(), v, &mut o, f.as_bytes(), None);
    (r, o)
}

fn time(lang: u32, h: i32, m: i32, s: i32) -> String {
    let mut o = Vec::new();
    assert!(util(lang).make_time_string(h, m, s, &mut o));
    String::from_utf8(o).unwrap()
}

fn date(lang: u32, m: u32, d: u32, y: i32, style: u32) -> Vec<u8> {
    let mut o = Vec::new();
    assert!(util(lang).make_date_string(m, d, y, &mut o, style));
    o
}

// ---- utility §1 ---------------------------------------------------------

#[test]
fn utility_identity_and_lifetime() {
    let mut u = LanguageUtility::new();
    assert_eq!(
        (u.ref_count(), u.language(), u.country(), u.currency_flag()),
        (0, 1, 0x36, 0)
    );
    assert!(!u.query_interface(0x1234));
    assert_eq!(u.ref_count(), 0);
    assert!(u.query_interface(IID_LANGUAGE_UTILITY));
    assert!(u.query_interface(1));
    assert_eq!(u.ref_count(), 2);
    assert_eq!(u.release(), Released::Alive(1));
    assert_eq!(u.release(), Released::Destroy);
    u.set_language(0x999);
    u.set_country(0);
    assert_eq!((u.language(), u.country()), (0x999, 0));
    assert!(u.init());
    let mut sep = Vec::new();
    util(0x0B).thousand_separator(&mut sep);
    assert_eq!(sep, b"\xC2\xA0");
}

// ---- utility §2 ---------------------------------------------------------

#[test]
fn attribute_queries() {
    assert_eq!(util(2).road_driving_side(), 0);
    assert_eq!(util(0x0F).road_driving_side(), 0);
    assert_eq!(util(0).road_driving_side(), 1);
    let mut u = util(3);
    assert!(!u.measurement_system());
    u.set_country(0x11);
    assert!(u.measurement_system());
    assert!(util(0x14).are_first_and_last_names_reversed() && !util(0x13).are_first_and_last_names_reversed());
    assert!(util(0x11).uses_input_method_editing() && !util(1).uses_input_method_editing());
    assert!(!util(1).uses_multibyte_characters() && !util(2).uses_multibyte_characters());
    assert!(util(3).uses_multibyte_characters() && util(0).uses_multibyte_characters());
    assert!(util(0x13).uses_multibyte_characters_for_code_page(1252));
    assert!(!util(3).uses_multibyte_characters_for_code_page(1252));
    assert!(!util(0x0F).uses_multibyte_characters_for_code_page(16));
}

#[test]
fn character_sets() {
    assert_eq!(util(0x0D).character_set_from_language(), 1250); // Russian: 1250
    assert_eq!(util(0x1C).character_set_from_language(), 1252);
    assert_eq!(util(0x1E).character_set_from_language(), 1252);
    assert_eq!(util(0x17).character_set_from_language(), 1251);
    assert_eq!(util(0x13).character_set_from_language(), 874);
    assert_eq!(util(0x0F).character_set_from_language(), 932);
    assert_eq!(util(0x14).character_set_from_language(), 949);
    assert_eq!(util(0x16).character_set_from_language(), 1256);
    for none in [0, 0x15, 0x19, 0x2B, 0x2C] {
        assert_eq!(util(none).character_set_from_language(), 1252);
    }
    assert!(!util(1).uses_character_set(1257));
    assert!(util(0x2A).uses_character_set(1252) && !util(0x2B).uses_character_set(1252));
}

// ---- utility §3 ---------------------------------------------------------

#[test]
fn currency_symbol() {
    for l in [0, 1, 2, 3, 0x2C, 0x99] {
        let mut o = b"x".to_vec();
        assert!(util(l).currency_symbol(&mut o));
        assert_eq!(o, b"$");
    }
    let cases: &[(u32, &[u8])] = &[
        (2, "£".as_bytes()),
        (3, b"\xE2\x2C\x82\x2C\xAC"),
        (0x26, b"\xE2\x2C\x82\x2C\xAC"),
        (0x14, b"\\"),
        (0x1A, b"K\xC4\x8D{"),
        (0x23, b"Esc."),
        (0x0F, "¥".as_bytes()),
    ];
    for &(l, want) in cases {
        let mut u = util(l);
        u.set_currency_flag_for_test(1);
        let mut o = Vec::new();
        assert!(u.currency_symbol(&mut o));
        assert_eq!(o, want, "lang {l:#x}");
    }
    // Flag 1, unlisted ID: out not assigned (keeps its content, true) or,
    // when empty, Untranslated and false.
    let mut u = util(0x15);
    u.set_currency_flag_for_test(1);
    let mut o = b"kept".to_vec();
    assert!(u.currency_symbol(&mut o));
    assert_eq!(o, b"kept");
    o.clear();
    assert!(!u.currency_symbol(&mut o));
    assert_eq!(o, b"Untranslated");
}

#[test]
fn separators_and_currency_flags() {
    let sep = |l| {
        let mut t = Vec::new();
        let mut d = Vec::new();
        let rt = util(l).thousand_separator(&mut t);
        let rd = util(l).decimal_separator(&mut d);
        (t, rt, d, rd)
    };
    assert_eq!(sep(1), (b",".to_vec(), true, b".".to_vec(), true));
    assert_eq!(sep(4), (b".".to_vec(), true, b",".to_vec(), true));
    assert_eq!(sep(0x0A), (b"\xC2\xA0".to_vec(), true, b",".to_vec(), true));
    assert_eq!(sep(0x23), (b".".to_vec(), true, b",".to_vec(), true));
    assert_eq!(sep(0), (b",".to_vec(), false, b".".to_vec(), false));
    assert_eq!(sep(0x15), (b",".to_vec(), false, b".".to_vec(), false));
    assert!(util(0x23).currency_symbol_precedes_amount() && !util(3).currency_symbol_precedes_amount());
    assert!(!util(0x0A).space_between_currency_symbol_and_amount());
    assert!(util(0).space_between_currency_symbol_and_amount());
}

// ---- utility §4 ---------------------------------------------------------

#[test]
fn number_strings() {
    assert_eq!(num(1, 1234567), b"1,234,567");
    assert_eq!(num(0x0B, 1234567), b"1\xC2\xA0234\xC2\xA0567");
    assert_eq!(num(1, 0), b"0");
    assert_eq!(num(1, 123), b"123");
    assert_eq!(num(1, 1000), b"1,000");
    assert_eq!(num(1, 123456), b"123,456");
    assert_eq!(num(4, -1234567), b"-1.234.567");
    assert_eq!(num(0x0B, -1234), b"1\xC2\xA0234-");
    assert_eq!(num(0x0E, -5), b""); // Portuguese: empty
    assert_eq!(num(0x23, -5), b"");
    assert_eq!(num(0, -5), b"");
    assert_eq!(num(1, i64::MIN), b"--9,223,372,036,854,775,808");
    assert_eq!(num(1, i64::MAX), b"9,223,372,036,854,775,807");
}

#[test]
fn money_strings() {
    assert_eq!(money(1, 1234), b"$1,234");
    assert_eq!(money(1, -1234), b"-$1,234");
    assert_eq!(money(3, 1234), b"1.234\xC2\xA0$");
    assert_eq!(money(8, -1234), b"$\xC2\xA0-1.234");
    assert_eq!(money(0x0A, -1234), b"$-1\xC2\xA0234");
    assert_eq!(money(0x0C, -5), b"$\xC2\xA0-5");
    assert_eq!(money(1, 0), b"$0");
    let mut o = Vec::new();
    util(1).make_money_string(-7, &mut o, Some(b"EUR"));
    assert_eq!(o, b"-EUR7");
}

#[test]
fn formatted_money_strings() {
    assert_eq!(fmoney(1, 1234.5, "Total: %.2f"), (true, b"Total: $1,234.50".to_vec()));
    assert_eq!(fmoney(1, -2.0, "Total: %.2f"), (true, b"Total: -$2.00".to_vec()));
    assert_eq!(fmoney(1, 1234567.0, "%.1f"), (true, b"$1,234,567.0".to_vec()));
    assert_eq!(fmoney(1, 100.0, "%.0f%% off"), (true, b"$100% off".to_vec()));
    assert_eq!(fmoney(1, 5.0, "50%% %.0f"), (true, b"50% $5".to_vec()));
    // Q3: 0.0 is printed from -0.0 (the host prints `-0.00`), template `$1`.
    assert_eq!(fmoney(1, 0.0, "%.2f"), (true, b"$-0.00".to_vec()));
    // Leading spaces from a width: grouping stops at a space.
    assert_eq!(fmoney(1, 1234.0, "%8.0f"), (true, b"$    1,234".to_vec()));
    assert_eq!(fmoney(3, 1234.5, "%.2f"), (true, b"1.234.50\xC2\xA0$".to_vec()));
    // No usable `%`, or a non-floating conversion: false, out erased.
    assert_eq!(fmoney(1, 1.0, "no spec"), (false, Vec::new()));
    assert_eq!(fmoney(1, 1.0, "%d"), (false, Vec::new()));
    assert_eq!(fmoney(1, 1.0, "%%"), (false, Vec::new()));
    assert_eq!(fmoney(1, 1.0, "%.2"), (false, Vec::new()));
}

#[test]
fn formatted_number_string_uses_host_printf() {
    let mut o = Vec::new();
    assert!(util(4).make_formatted_number_string(&TestHost::default(), 1234.5, &mut o, b"x=%.1f"));
    assert_eq!(o, b"x=1234.5");
}

#[test]
fn time_strings() {
    assert_eq!(time(1, 13, 5, -1), "1:05 pm");
    assert_eq!(time(1, 0, -1, -1), "12 am");
    assert_eq!(time(1, 12, 0, 0), "12:00:00 pm");
    assert_eq!(time(1, 25, 61, 75), "1:01:15 am");
    assert_eq!(time(4, 7, 3, 9), "07:03:09");
    assert_eq!(time(4, 7, -1, -1), "07:00");
    assert_eq!(time(5, 7, 3, -1), "7.03");
    assert_eq!(time(9, 7, 3, 9), "07.03.09");
    assert_eq!(time(0x0D, 7, 3, -1), "7:03");
    assert_eq!(time(0x0D, 7, -1, -1), "7:00");
    // second >= 0 with a negative minute: the minute prints as unsigned.
    assert_eq!(time(0x0D, 7, -1, 0), "7:4294967295:00");
}

#[test]
fn date_strings() {
    assert_eq!(date(1, 3, 7, 2004, 2), b"March 7, 2004");
    assert_eq!(date(4, 3, 7, 2004, 0), b"7.3.2004");
    assert_eq!(date(0x0F, 2, 1, 2004, 2), b"2004.Untranslated.1");
    assert_eq!(date(1, 3, 7, 5, 0), b"3/7/05");
    assert_eq!(date(1, 3, 7, 2004, 1), b"3/2004");
    assert_eq!(date(1, 3, 7, 2004, 4), b"Mar. 7, 2004");
    assert_eq!(date(1, 3, 7, 2004, 5), b"Mar. 2004");
    assert_eq!(date(2, 3, 7, 2004, 0), b"7/3/2004");
    assert_eq!(date(2, 3, 7, 2004, 4), b"7 Mar., 2004");
    assert_eq!(date(7, 3, 7, 2004, 0), b"7-3-2004");
    assert_eq!(date(3, 2, 7, 2004, 2), b"7 F\xe9vrier, 2004");
    assert_eq!(date(0x11, 3, 7, 2004, 0), b"2004-3-7");
    assert_eq!(date(0x14, 2, 7, 2004, 3), "2004 이월".as_bytes());
    let mut o = b"keep".to_vec();
    assert!(!util(1).make_date_string(1, 1, 2000, &mut o, 6));
    assert_eq!(o, b"keep");
}

#[test]
fn case_and_compare() {
    let h = TestHost::default();
    let u = util(1);
    let mut o = b"xxxxxxxx".to_vec();
    assert!(u.convert_to_upper_case(&h, b"Hello", &mut o));
    assert_eq!(o, b"HELLO");
    // The length mapped is out's length on entry (Q4).
    let mut o = b"xx".to_vec();
    u.convert_to_lower_case(&h, b"HELLO", &mut o);
    assert_eq!(o, b"heLLO");
    assert_eq!(u.compare_strings(&h, b"abc", b"ABC", false), 0);
    assert_eq!(u.compare_strings(&h, b"abc", b"ABC", true), 1);
    assert_eq!(u.compare_strings(&h, b"a", b"b", true), -1);
}

// ---- utility §5 ---------------------------------------------------------

#[test]
fn day_and_month_names() {
    let get = |l: u32, f: fn(&LanguageUtility, u32, &mut Vec<u8>) -> bool, i: u32| {
        let mut o = Vec::new();
        let r = f(&util(l), i, &mut o);
        (r, o)
    };
    assert_eq!(get(1, LanguageUtility::day_name, 1), (true, b"Monday".to_vec()));
    assert_eq!(get(0x10, LanguageUtility::day_name, 7), (true, b"Sunday".to_vec()));
    assert_eq!(
        get(9, LanguageUtility::abbr_day_name, 1),
        (true, b"M\xE5\x06E".to_vec())
    );
    assert_eq!(
        get(0x10, LanguageUtility::abbr_day_name, 3),
        (true, b"\xC2\x8Cr".to_vec())
    );
    assert_eq!(
        get(0x0E, LanguageUtility::day_name, 2),
        (true, b"Ter\xe7a-feira".to_vec())
    );
    assert_eq!(get(0x0B, LanguageUtility::month_name, 1), (true, b"tamikuu".to_vec()));
    assert_eq!(
        get(0x10, LanguageUtility::month_name, 10),
        (true, b"Pa\xC2\x9Fdziernik".to_vec())
    );
    assert_eq!(
        get(0x10, LanguageUtility::abbr_month_name, 10),
        (true, b"Pa\x9f".to_vec())
    );
    assert_eq!(
        get(0x0F, LanguageUtility::abbr_month_name, 2),
        (true, "2 月".as_bytes().to_vec())
    );
    assert_eq!(
        get(0x14, LanguageUtility::month_name, 12),
        (true, "십이월".as_bytes().to_vec())
    );
    // Only the low byte of the index: 0x101 is 1.
    assert_eq!(get(1, LanguageUtility::month_name, 0x101), (true, b"January".to_vec()));
    // Missing pairs with an empty out: Untranslated, false.
    for (l, i) in [(1, 0), (1, 13), (0x0A, 1), (0x0F, 2), (0, 1)] {
        assert_eq!(
            get(l, LanguageUtility::month_name, i),
            (false, b"Untranslated".to_vec()),
            "{l} {i}"
        );
    }
    assert_eq!(
        get(0x0A, LanguageUtility::abbr_month_name, 1),
        (false, b"Untranslated".to_vec())
    );
    // A missing pair with a non-empty out: out kept, true.
    let mut o = b"old".to_vec();
    assert!(util(0x0F).month_name(2, &mut o));
    assert_eq!(o, b"old");
}

// ---- manager §4 -----------------------------------------------------------

#[test]
fn manager_identity_and_utilities() {
    let m = LanguageManager::new();
    assert_eq!((m.current_language(), m.current_country()), (1, 0x36));
    assert_eq!(
        LanguageManager::query_interface(0x0054_B7D5),
        Some(ManagerInterface::Manager)
    );
    assert_eq!(
        LanguageManager::query_interface(0x287F_B697),
        Some(ManagerInterface::SystemService)
    );
    assert_eq!(LanguageManager::query_interface(1), None);
    assert!(m.language_utility(0).is_some() && m.language_utility(1).is_some());
    assert!(m.language_utility(3).is_none());
    let u = m.new_language_utility(4);
    assert_eq!((u.ref_count(), u.language(), u.country()), (1, 4, 0x36));
    assert_eq!(m.current_system_country(), 0xFFFF_FFFF);
    assert_eq!(m.exchange_rate_for_locale(7), 1.0);
    let mut out = 0i64;
    assert!(m.convert_money(-42i64, &mut out));
    assert_eq!(out, -42);
}

#[test]
fn manager_switching() {
    let h = TestHost::default();
    let mut m = LanguageManager::new();
    assert!(m.set_current_language(&h, 0)); // 0 means 1, already current
    assert!(h.locales.borrow().is_empty());
    assert!(m.set_current_language(&h, 4));
    assert_eq!(h.locales.borrow().last().unwrap(), b"german");
    assert_eq!(*h.mbcp.borrow(), 1);
    assert_eq!(m.language_utility(0).unwrap().language(), 4);
    // setlocale failure is ignored.
    assert!(m.set_current_language(&h, 0x1F));
    assert_eq!(*h.mbcp.borrow(), 1);
    // 0x2D: switched although false.
    assert!(!m.set_current_language(&h, 0x2D));
    assert_eq!(m.current_language(), 0x2D);
    assert_eq!(m.language_utility(0).unwrap().language(), 0x2D);
    // Available set.
    m.add_available_language(3);
    m.add_available_language(0x2D); // ignored
    assert!(!m.set_current_language(&h, 5));
    assert_eq!(m.current_language(), 0x2D);
    assert!(m.set_current_language(&h, 3));
    m.remove_available_language(3);
    assert!(m.can_switch_to_language(0x99));
    assert!(m.set_current_country(0));
    assert_eq!(m.current_country(), 0x36);
    assert!(m.set_current_country(0x11));
    assert!(m.language_utility(0).unwrap().measurement_system());
}

#[test]
fn manager_available_lists() {
    let mut m = LanguageManager::new();
    let mut id = 0x2C;
    assert!(m.next_available_language(&mut id));
    assert_eq!(id, 0x2D);
    assert!(!m.next_available_language(&mut id));
    assert_eq!(id, 0x2E);
    let mut names = Vec::new();
    m.available_language_names(&mut names);
    assert_eq!(names.len(), 22);
    assert_eq!(names[0], b"UK English");
    assert_eq!(names[1], b"German");
    assert_eq!(names[21], b"Vietnamese");
    let mut ids = Vec::new();
    m.all_available_languages(&mut ids);
    assert!(ids.is_empty());
    for l in [9, 3, 1, 4] {
        m.add_available_language(l);
    }
    m.all_available_languages(&mut ids);
    assert_eq!(ids, [1, 3, 4, 9]);
    m.available_language_names(&mut names);
    // id 1 -> next gives 3 (listed), +1 = 4 -> next gives 9 (listed), skipping German.
    assert_eq!(names, [b"French".to_vec(), b"Swedish".to_vec()]);
}

#[test]
fn manager_system_language() {
    let m = LanguageManager::new();
    assert_eq!(m.system_language_from_language(0), 0x409);
    assert_eq!(m.system_language_from_language(0x0E), 0x816);
    assert_eq!(m.system_language_from_language(0x18), 1);
    assert_eq!(m.system_language_from_language(0x2D), 1);
    let f = LanguageManager::language_from_system_language;
    assert_eq!((f(0x809), f(0x3409), f(0x180C), f(0x1407)), (2, 2, 3, 4));
    assert_eq!((f(0x80A), f(0xC0A), f(0x500A), f(0x40A), f(0x540A)), (0x2A, 6, 6, 6, 1));
    assert_eq!(
        (f(0x816), f(0x416), f(0x4001), f(0x1404), f(0xC04)),
        (0x0E, 0x23, 0x16, 0x11, 0x12)
    );
    assert_eq!((f(0x409), f(0), f(0x1_0809)), (1, 1, 2));
    let h = TestHost {
        lcid: 0x0407,
        ..TestHost::default()
    };
    assert_eq!(m.current_system_language(&h), 4);
}

#[test]
fn manager_names_and_codes() {
    let m = LanguageManager::new();
    let call = |f: fn(&LanguageManager, &mut Vec<u8>, u32) -> bool, init: &[u8], id: u32| {
        let mut o = init.to_vec();
        let r = f(&m, &mut o, id);
        (r, o)
    };
    use LanguageManager as M;
    assert_eq!(call(M::language_english_name, b"x", 0x19), (true, b"Ukranian".to_vec()));
    assert_eq!(call(M::language_english_name, b"x", 0x2D), (false, Vec::new()));
    assert_eq!(call(M::language_english_name, b"", 0), (true, b"English".to_vec()));
    assert_eq!(call(M::language_local_name, b"", 3), (false, b"Untranslated".to_vec()));
    assert_eq!(call(M::language_local_name, b"", 0x15), (true, Vec::new()));
    assert_eq!(
        call(M::language_local_name, b"z", 0x15),
        (false, b"Untranslated".to_vec())
    );
    assert_eq!(call(M::language_directory_name, b"", 2), (true, b"UKEnglsh".to_vec()));
    assert_eq!(call(M::language_directory_name, b"", 0x13), (true, b"Thai".to_vec()));
    assert_eq!(call(M::language_directory_name, b"x", 0x2D), (false, Vec::new()));
    assert_eq!(call(M::iso639_code, b"", 2), (true, b"uk".to_vec()));
    assert_eq!(call(M::iso639_code, b"", 0x2A), (true, b"sp".to_vec()));
    assert_eq!(call(M::iso639_code, b"x", 0x2D), (false, Vec::new()));
    assert_eq!(
        call(M::language_runtime_library_name, b"", 0x19),
        (true, b"russian".to_vec())
    );
    assert_eq!(
        call(M::language_runtime_library_name, b"keep", 0x2D),
        (false, b"keep".to_vec())
    );
    assert_eq!(call(M::country_english_name, b"", 0x17), (true, b"Hungray".to_vec()));
    assert_eq!(call(M::country_english_name, b"", 0), (true, b"United States".to_vec()));
    assert_eq!(
        call(M::country_english_name, b"", 0x39),
        (false, b"UnitedStates".to_vec())
    );
    assert_eq!(call(M::iso3166_code, b"x", 0x15), (false, Vec::new()));
    assert_eq!(call(M::iso3166_code, b"", 0x35), (true, b"gb".to_vec()));
    assert_eq!(call(M::iso3166_code, b"x", 0x39), (false, Vec::new()));
    assert_eq!(
        (
            m.language_id_alias(2),
            m.language_id_alias(0x26),
            m.language_id_alias(0x19)
        ),
        (1, 3, 0x0D)
    );
    assert_eq!((m.language_id_alias(0), m.language_id_alias(0x2B)), (1, 0x2B));
}

#[test]
fn manager_countries() {
    let m = LanguageManager::new();
    assert_eq!(m.primary_language_for_country(8), 1); // Canada
    assert_eq!(m.primary_language_for_country(0x2F), 2); // Switzerland
    assert_eq!(m.primary_language_for_country(0x11), 0); // France
    assert_eq!(m.primary_language_for_country(0x12), 0x0D); // Georgia
    assert_eq!(m.primary_language_for_country(0), 1);
    assert_eq!(m.primary_language_for_country(0x99), 0);
    assert_eq!(m.default_country_for_language(2), 0x35);
    assert_eq!(m.default_country_for_language(0x26), 5);
    assert_eq!(m.default_country_for_language(0), 0x36);
    assert_eq!(m.default_country_for_language(0x2D), 0x36);
}

#[test]
fn manager_reverse_lookups() {
    let h = TestHost::default();
    let m = LanguageManager::new();
    let mut id = 0;
    assert!(m.language_id_from_iso639_code(&h, b"uk", &mut id) && id == 2);
    assert!(m.language_id_from_iso639_code(&h, b"PT", &mut id) && id == 0x0E);
    assert!(m.language_id_from_iso639_code(&h, b"zh", &mut id) && id == 0x11);
    assert!(m.language_id_from_iso639_code(&h, b"ru", &mut id) && id == 0x0D);
    assert!(!m.language_id_from_iso639_code(&h, b"xx", &mut id) && id == 1);
    assert!(m.language_id_from_english_name(&h, b"german", &mut id) && id == 4);
    assert!(!m.language_id_from_local_name(&h, b"Deutsch", &mut id) && id == 1);
    assert!(m.country_id_from_iso3166_code(&h, b"us", &mut id) && id == 0x36);
    assert!(!m.country_id_from_iso3166_code(&h, b"", &mut id) && id == 0x36);
    assert!(m.country_id_from_english_name(&h, b"Hungray", &mut id) && id == 0x17);
    let mut text = b"FRENCH".to_vec();
    assert!(m.language_id_from_either_name(&h, &mut text, &mut id));
    assert_eq!((id, text), (3, b"French".to_vec()));
}
