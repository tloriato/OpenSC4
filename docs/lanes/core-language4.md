| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| core-language4: pure Rust core for cRZLanguageUtility and cRZLanguageManager (language-utility.md, strings-language.md §4) | `core-language4` / (engine lane) | ready. New engine/core/src/language (utility complete, manager §4 complete, behind a LocaleHost platform trait). No ABI, no plugin/CMake, no game run. Gate: cargo test 3696 passed / 0 failed (baseline 3676; +20 language tests), clippy 0, ctest 62/62. Questions LU1-LU3, SL1 filed. | 1. Run tools/questions.py index (new LU and SL prefixes). 2. Plugin lane: a LocaleHost on the MSVC CRT / Win32 ANSI calls and the GZCOM glue for both classes (shadow/replace). |

## Report

What changed (commits 1ae2b2af and the follow-up export fix):
- `engine/core/src/language/` (new; `pub mod language` added to engine/core/src/lib.rs):
  - `utility.rs` `LanguageUtility` = cRZLanguageUtility, every slot of language-utility.md: QueryInterface/AddRef/Release
    (`Released::Alive(n)` / `Destroy`), Init, SetLanguage/SetCountry, slots 23..29 attribute queries, currency symbol
    (flag 0 `$`; flag-1 table with the garbled euro `E2 2C 82 2C AC`, `Kč{`, out-not-assigned for other IDs,
    `Untranslated`/false when empty), separators (group space U+00A0), MakeNumberString (trailing minus for Finnish,
    empty for 0/0x0E/0x15+, `--9,223,...` for i64::MIN), MakeMoneyString (Danish/Hebrew/Norwegian negative forms),
    MakeFormattedMoneyString (`%`/`%%` scan, f/e/g/E/G only, `value <= 0` negated so 0.0 prints from -0.0, grouping stops
    at a space, template `1` replacement), MakeFormattedNumberString, MakeTimeString (12h and the four 24h rows, `%u` of
    negatives), MakeDateString (six groups, styles 0..5, `Untranslated` names), case conversion over the length of out on
    entry, CompareStrings, and the four day/month name tables as raw bytes (low-byte index, `(none)` keeps out).
  - `manager.rs` `LanguageManager` = cRZLanguageManager §4.1..§4.8: QueryInterface (not iid 1), GetLanguageUtility /
    GetNewLanguageUtility, SetCurrentLanguage (0x2D+ switched but false; setlocale then _setmbcp), SetCurrentCountry,
    available set, GetNextAvailableLanguage (0x2C -> 0x2D quirk), slot 46 skip-every-other list, slot 47, system LANGID
    maps both ways, English/local (inverted test)/directory names, alias, ISO 639/3166, runtime locale names, countries
    (`Hungray`, `UnitedStates` fallback), primary language/default country, reverse lookups via CompareStrings, slot 20
    as the spec's [choice] (no recursion), money stubs.
  - `mod.rs` `LocaleHost` trait: sprintf of one double, UTF-8<->ANSI, CharLower/UpperBuffA, CompareStringA,
    setlocale(LC_ALL), _setmbcp(_MB_CP_ANSI), GetUserDefaultLCID. The core never emulates these.
  - `names.rs` tables, `tests.rs` 20 tests from the specs' worked examples and tables (test host is ASCII/Rust-printf).
- Owners: cRZLanguageUtility complete in core (every slot of language-utility.md); cRZLanguageManager complete for
  strings-language.md §4 (service registration itself is plugin work). Not touched: cRZString, cSCStringDetokenizer,
  cRZLanguage (no spec section), ConvertStringEncoding (§9 unspecified). Implemented in core only: no shadow, no replace.
- Questions: LU1 (year sign/padding in MakeDateString), LU2 (case-conversion bytes past the text, return value),
  LU3 (slot 4 return), SL1 (GetNewLanguageUtility(0) verbatim or current) in docs/specs/questions/language-utility.md and
  strings-language.md (new files; QUESTIONS.md index not regenerated: lanes may not run tools/).
- Gate: qbuild all PASS: cargo test 3696/0 failed, clippy 0 diagnostics, CMake build PASS, ctest 62/62. Game: none (task game=false).
