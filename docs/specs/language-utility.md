# Language utility (`cRZLanguageUtility`): locale-dependent names, numbers, money, dates

Status: RE spec, 2026-10-21, lane re-rzservices4. Behaviour of SimCity 4 Deluxe 1.1.641 (GOG). Owner class
`cRZLanguageUtility` (RZ framework, vtable `0x00AD1250`, 33 slots); its creator/holder is
`cRZLanguageManager` (only the parts that create, hold and configure the utility are described here; the
manager's language list, folders and `SetCurrentLanguage` locale switch belong to data-formats §6.2).

Sources: the Ghidra export and decomp, string data read from the exe image, the name tables (§5) produced by
running the original instructions of the four name getters concretely for every language ID 0..0x2D and every
index 0..13 (helper outside the repo, no code from the game in this document). Tags: *[compat]* the game's
observable rule, *[inferred]* meaning drawn from names and use, not from a measured effect.

## 1. Identity, state and lifetime *[compat]*

| Item | Rule |
|---|---|
| Interface | QueryInterface answers iid `0x1054B7D5` and `1` (the unknown interface): stores itself, AddRefs, returns true. Any other iid: false, out untouched. |
| Object size | 0x28 bytes: vtable, reference count (+0x04), **language ID** (+0x08), **country ID** (+0x0C), **local-currency flag** byte (+0x10), **group-space string** (+0x14, a string object). |
| Constructor `0x0092847C` | count 0, language 1 (English), country `0x36` (United States), currency flag 0, group-space string = U+00A0 NO-BREAK SPACE (UTF-8 `C2 A0`). |
| `Init` (slot 3, `0x00928FDD`) | resets the group-space string to U+00A0 and passes it through the UTF-8-to-destination conversion (§1.1); returns true. |
| `Shutdown` (slot 4, `0x00949ADE`) | returns true in AL; performs no calls or state writes. Language, country, currency flag, reference count and group-space string are all preserved. |
| `SetLanguage(id)` (slot 31, `0x009B2970`) | stores the language ID verbatim; no validation, no return value. |
| `SetCountry(id)` (slot 32, `0x00894F6D`) | stores the country ID verbatim. |
| `Release` (slot 2, `0x00927C4F`) | count > 1: decrement and return the new count; otherwise call the deleting destructor (slot 30) and return 0. |
| Currency flag (+0x10) | set to 0 by the constructor; no slot of this class writes it, and no writer was found (open question Q1). With the flag 0 the currency symbol is always `$` (§3.1). |

Holders:

- `cRZLanguageManager` embeds one utility at manager offset +0x38. Its constructor builds it, calls `Init`,
  `SetLanguage(1)` and `SetCountry(0x36)`; the manager's own current language starts at 1 and country at 0x36.
- `SetCurrentLanguage(id)` (id 0 means 1): if the id differs from the current one and the manager accepts it,
  the manager stores it and calls `SetLanguage(id)` on the embedded utility (then switches the C locale; see
  data-formats §6.2).
- `SetCurrentCountry(id)` (id 0 means 0x36) stores it and calls `SetCountry(id)` on the embedded utility.
- `GetLanguageUtility(id)` (`0x00924B50`): id 0 means the current language; returns the embedded utility when
  the id equals the current language, otherwise null. No AddRef is taken.
- `GetNewLanguageUtility(id)` (`0x00928BEB`): allocates a fresh 0x28-byte utility, AddRefs it (count 1),
  `SetLanguage(id)`, returns it; the caller owns that reference. `Init` is **not** called (the constructor
  already set the same group-space string). Country stays 0x36. ID **0 is stored verbatim**, regardless of
  the manager's current language: this factory forwards its argument unchanged to slot 31
  (`0x00928C0C–0x00928C14`, setter `0x009B2970–0x009B2979`).

### 1.1 Output strings and encoding *[compat]*

Every function writes its result into a caller-supplied string object by **replacing** its content, except
where a row below says otherwise. Name and currency getters finish with "convert UTF-8 to destination
format": in this build the framework code page is the constant 8 (UTF-8, `GetCurrentFrameworkCodePage`
`0x00940DA8`), so the conversion leaves the bytes unchanged. Name strings are byte strings: several are not
valid UTF-8 (§5 shows such bytes as `\xNN`); they are returned as stored.

## 2. Language IDs and attribute queries *[compat]*

Language IDs (English names from `cRZLanguageManager::GetLanguageEnglishName`): 1 English, 2 UK English,
3 French, 4 German, 5 Italian, 6 Spanish, 7 Dutch, 8 Danish, 9 Swedish, 0x0A Norwegian, 0x0B Finnish,
0x0C Hebrew, 0x0D Russian, 0x0E Portuguese, 0x0F Japanese, 0x10 Polish, 0x11 Simplified Chinese,
0x12 Traditional Chinese, 0x13 Thai, 0x14 Korean, 0x15 Hindi, 0x16 Arabic, 0x17 Bulgarian, 0x18 Cyrillic,
0x19 Ukranian, 0x1A Czech, 0x1B Greek, 0x1C Hungarian, 0x1D Icelandic, 0x1E Romanian, 0x1F Latin,
0x20 Slovak, 0x21 Albanian, 0x22 Turkish, 0x23 Brazilian Portuguese, 0x24 Swiss French, 0x25 Canadian French,
0x26 Belgian French, 0x27 Swiss German, 0x28 Swiss Italian, 0x29 Belgian Dutch, 0x2A Mexican Spanish,
0x2B Tagalog, 0x2C Vietnamese. 0 is "no language". Every query below reads the stored ID; IDs outside the
lists fall to the stated default.

| Slot | Query | True for language IDs | Otherwise |
|---|---|---|---|
| 25 `0x00927C17` | `GetLanguageRoadDrivingSide` | all IDs except 2 and 0x0F (returns 1) | 0 for UK English and Japanese *[inferred: 0 = drive on the left]* |
| 26 `0x00925991` | `GetMeasurementSystem` | country ID ≠ 0x36 (reads the **country**, not the language) | false for United States *[inferred: true = metric]* |
| 27 `0x0092599B` | `AreFirstAndLastNamesReversed` | 0x0F, 0x11, 0x12, 0x14 | false |
| 28 `0x009259B8` | `DoesLanguageUseInputMethodEditing` | 0x0F, 0x11, 0x12, 0x14 | false |
| 29 `0x009259D6` | `DoesLanguageUseMultiByteCharacters` | with framework code page 8 (this build): every ID except 1 and 2 (so also French, ID 0, ...) | false for English, UK English |

`DoesLanguageUseMultiByteCharacters` for other code pages (unreachable in this build): code page 0 or 16
gives false; any other code page gives true only for 0x0F, 0x11, 0x12, 0x13, 0x14.

`DoesLanguageUseCharacterSet(cp)` (slot 24, `0x0092580A`):

| Windows code page `cp` | True for language IDs |
|---|---|
| 1252 (`0x4E4`) | 1..0x0B, 0x0E, 0x1C..0x1F, 0x23..0x2A |
| 1250 (`0x4E2`) | 0x0D, 0x10, 0x1A, 0x1C, 0x1E, 0x20, 0x21 |
| 1251 (`0x4E3`) | 0x0D, 0x17, 0x18 |
| 932 / 936 / 950 / 949 | 0x0F / 0x11 / 0x12 / 0x14 |
| 874 (`0x36A`) | 0x13 |
| 1253 / 1254 / 1255 / 1256 | 0x1B / 0x22 / 0x0C / 0x16 |
| anything else (including 1257) | none (false) |

`GetCharacterSetFromLanguage` (slot 23, `0x009256F5`) asks slot 24 in this order and returns the first code
page that answers true: 1252, 1250, 874, 932, 936, 950, 949, 1251, 1253, 1254, 1255, 1256, 1257; if none
answers, 1252. Consequences: Russian (0x0D) gets **1250**, Hungarian and Romanian get 1252, IDs in no row
(0, 0x15, 0x19, 0x2B, 0x2C, ...) get 1252; 1257 is never returned.

## 3. Currency and separators *[compat]*

### 3.1 `GetCurrencySymbol(out)` (slot 11, `0x009279B0`)

- Currency flag (+0x10) **0** (the only state seen): out = `$`, return true.
- Flag 1: by language: 1, 0x11, 0x12 `$`; 2 `£`; 3..8, 0x0A, 0x0B, 0x0E, 0x1B, 0x26 the bytes
  `E2 2C 82 2C AC` (the euro sign's three UTF-8 bytes with commas between them, as stored); 9, 0x1D `kr`;
  0x0C `₪`; 0x0D `б`; 0x0F `¥`; 0x10 `ł`; 0x13 `฿`; 0x14 `\` (single backslash); 0x1A the bytes
  `4B C4 8D 7B` (`Kč{`); 0x1C `FT`; 0x1E `Lei`; 0x21 `Lek`; 0x22 `TL`; 0x23 `Esc.`. Any other ID: out is
  **not assigned** (keeps its content).
- In all cases, if out is then empty it becomes `Untranslated` and the return is false; otherwise true.

| Slot | Query | True (1) for | Otherwise |
|---|---|---|---|
| 12 `0x00927AE3` | `DoesCurrencySymbolPrecedeAmount` | 1, 2, 5, 7, 8, 0x0A, 0x0C, 0x0F, 0x11..0x14, 0x22, 0x23 | 0 |
| 13 `0x00927B2F` | `IsSpaceBetweenCurrencySymbolAndAmount` | every ID **except** 1, 2, 0x0A, 0x0C, 0x0F, 0x11..0x14 | 0 for those |

### 3.2 Separators

| Language IDs | `GetThousandSeparator` (slot 14, `0x00927C66`) | `GetDecimalSeparator` (slot 15, `0x00927B7B`) |
|---|---|---|
| 1, 2, 0x0C, 0x0F, 0x11..0x14 | `,` (true) | `.` (true) |
| 3..9, 0x0D, 0x0E, 0x23 | `.` (true) | `,` (true) |
| 0x0A, 0x0B, 0x10 | the group-space string, U+00A0 (true) | `,` (true) |
| every other ID (0, 0x15.., except 0x23) | `,` (**false**) | `.` (**false**) |

(0x0A and 0x0B appear in the decimal-comma row; the decimal getter's own list is 3..0x0B, 0x0D, 0x0E,
0x10, 0x23.)

## 4. Formatting *[compat]*

### 4.1 Digit grouping (shared by §4.2 and §4.3)

The magnitude is printed as a plain decimal (`%lld` of a 64-bit integer). Groups are cut from the left: the
first group has `len mod 3` digits (3 if that is 0), every later group 3; the thousand separator of §3.2 is
put between groups. Strings of 3 digits or fewer get no separator. Example (English): 1234567 → `1,234,567`;
Finnish 1234567 → `1` U+00A0 `234` U+00A0 `567`.

### 4.2 `MakeNumberString(int64 value, out)` (slot 18, `0x00929379`)

Non-negative: out = grouped magnitude. Negative (the magnitude is the two's-complement negation):

| Language IDs | Negative result |
|---|---|
| 1..0x0A, 0x0C, 0x0D, 0x0F..0x14 | `-` + grouped |
| 0x0B (Finnish) | grouped + `-` (trailing minus) |
| 0, 0x0E, 0x15 and above (incl. 0x23 Brazilian Portuguese) | **empty string** (nothing is built) |

Always returns true. Worked: German −1234567 → `-1.234.567`; Portuguese −5 → `` (empty).

### 4.3 `MakeMoneyString(int64 value, out, symbol)` (slot 16, `0x0092901A`)

`symbol` is an optional string object; when null the symbol is `GetCurrencySymbol` (§3.1, `$` in practice).
Let G be the grouped magnitude (§4.1) and S the symbol. Positive and zero:

- precedes (§3.1 table): S, then U+00A0 if "space" is true, then G;
- otherwise: G, then U+00A0 if "space" is true, then S.

Negative, by language: 8 (Danish) and 0x0C (Hebrew): S + U+00A0 + `-` + G (the space is added whatever slot
13 says); 0x0A (Norwegian): S + `-` + G; every other ID: `-` + (the positive form). Returns true.

Worked (currency flag 0, so S = `$`): English 1234 → `$1,234`; English −1234 → `-$1,234`; French 1234 →
`1.234` U+00A0 `$`; Danish −1234 → `$` U+00A0 `-1.234`; Norwegian −1234 → `$-1` U+00A0 `234`.

### 4.4 `MakeFormattedMoneyString(double value, out, format, symbol)` (slot 17, `0x00928C55`)

1. out is erased first.
2. Find the first `%` in `format` that is not the start of `%%` (a `%` within the last two characters
   ends the search and is taken as is). None: return false (out stays empty).
3. Prefix = text before it; spec = from that `%` up to and including the first character from
   `fegEGcCdiouxXnpsSlLIh`; suffix = text after the spec. If no such character, or it is not one of
   `f e g E G`: return false. Each `%%` in prefix and suffix becomes `%`.
4. The magnitude is printed with the spec; the sign test is `value <= 0` → negate, so **0.0 is printed as
   −0.0** (see Q3). Thousand separators (§3.2) are inserted into the integer part: from the first character
   that is not a digit or space (or the end) go left in steps of 3 while the position is > 0 and the
   character before it is not a space.
5. A template is made with `MakeMoneyString(1)` for `value >= 0` (NaN counts as non-negative) or
   `MakeMoneyString(-1)` otherwise, with the same `symbol`; its first `1` is replaced by the printed number.
6. out = prefix + template + suffix; return true.

Worked (English): format `Total: %.2f`, value 1234.5 → `Total: $1,234.50`; value −2 → `Total: -$2.00`.

### 4.5 `MakeFormattedNumberString(double value, out, format)` (slot 19, `0x00927BEE`)

out = `format` applied printf-style to `value` through the string's own format call; no grouping, no
language rule. Returns true.

### 4.6 `MakeTimeString(hour, minute, second, out)` (slot 9, `0x009284BA`)

Signed inputs. hour > 23 → hour mod 24; minute > 59 → mod 60; second > 59 → mod 60 (negative values are
not normalised). Which parts are printed: second ≥ 0 → H, M and S (even if minute is negative, which then
prints as a large unsigned); else minute < 0 → hour only; else H and M. `%u` is unsigned decimal, `%.2u`
at least two digits zero-padded.

| Language IDs | H:M:S | H:M | hour only |
|---|---|---|---|
| 1, 0x2A (12-hour) | `%u:%.2u:%.2u %s` | `%u:%.2u %s` | `%u %s` |
| 2, 3, 4, 0x0A, 0x10, 0x1D | `%.2u:%.2u:%.2u` | `%.2u:%.2u` | `%.2u:00` |
| 5, 0x1C | `%u.%.2u.%.2u` | `%u.%.2u` | `%u:00` |
| 8, 9, 0x24, 0x27, 0x28 | `%.2u.%.2u.%.2u` | `%.2u.%.2u` | `%.2u:00` |
| all others | `%u:%.2u:%.2u` | `%u:%.2u` | `%u:00` |

12-hour rule: suffix `pm` when hour > 11, else `am`; displayed hour 12 for 0, hour − 12 above 12.
Worked: English (13, 5, −1) → `1:05 pm`; English (0, −1, −1) → `12 am`; German (7, 3, 9) → `07:03:09`.
Returns true.

### 4.7 `MakeDateString(month, day, year, out, style)` (slot 10, `0x00928699`)

Year text Y = decimal year padded to at least 2 digits (`5` → `05`). For styles 2..5 the month name N
(slot 7) and abbreviation A (slot 8) are fetched into fresh strings, so a missing name gives `Untranslated`.
Style > 5: return false, out untouched. Otherwise out = the pattern below, return true.

| Group (language IDs) | 0 | 1 | 2 | 3 | 4 | 5 |
|---|---|---|---|---|---|---|
| US: 1, 0x0C, 0 and every ID not listed | `M/D/Y` | `M/Y` | `N D, Y` | `N Y` | `A. D, Y` | `A. Y` |
| 2, 3, 5, 6, 0x0D, 0x13, 0x23 | `D/M/Y` | `M/Y` | `D N, Y` | `N Y` | `D A., Y` | `A. Y` |
| 4, 0x0A, 0x0B, 0x10 | `D.M.Y` | `M.Y` | as above | as above | as above | as above |
| 7, 8, 9, 0x0E | `D-M-Y` | `M-Y` | as above | as above | as above | as above |
| 0x0F (Japanese) | `Y/M/D` | `Y/M` | `Y.N.D` | `Y.N` | `Y.A.D` | `Y.A` |
| 0x11, 0x12, 0x14 | `Y-M-D` | `Y-M` | `Y N D` | `Y N` | `Y A D` | `Y A` |

M and D are unsigned decimals, no padding. Worked: English (3, 7, 2004, style 2) → `March 7, 2004`;
German (3, 7, 2004, 0) → `7.3.2004`; Japanese (2, 1, 2004, 2) → `2004.Untranslated.1` (no Japanese
February name, §5).

### 4.8 Case and comparison (slots 20..22)

- `ConvertToLowerCase(in, out)` / `ConvertToUpperCase(in, out)`: the text is converted from UTF-8 to the
  ANSI code page, case-mapped by the Windows ANSI rules (`CharLowerBuffA` / `CharUpperBuffA` over the
  length of `out` as it was on entry, see Q4), and converted back into out.
- `CompareStrings(a, b, caseSensitive)`: both converted to ANSI and compared with the user-default locale's
  collation, ignoring case unless `caseSensitive`; returns −1, 0 or +1 (0 also when the comparison fails).

## 5. Day and month names *[compat]*

`GetDayName(day, out)` (slot 5, `0x00925A26`), `GetAbbrDayName` (slot 6, `0x00925F69`), `GetMonthName`
(slot 7, `0x009265B2`), `GetAbbrMonthName` (slot 8, `0x009270B2`). `day` is 1 = Monday .. 7 = Sunday,
`month` 1..12 (only the low byte of the index is read). When the (language, index) pair has an entry, out =
that string; when it has none (index 0 or out of range, a language without a row below, or a `(none)` cell),
out is **left as it was**. Then, if out is empty it becomes `Untranslated` and the result is false;
otherwise true. So a caller passing a non-empty out for a missing pair gets true and its old text.

Rows list the language IDs sharing identical entries; `\xNN` is a raw byte that is not part of valid UTF-8,
`[U+NNNN]` a C1/C0 control character encoded as UTF-8. Original typos are kept (Finnish `tamikuu`, the
Swedish Monday abbreviation `M\xE5` + U+0006 + `E`, Polish `[U+008C]r` and `Pa[U+009F]dziernik`).

### 5.1 Day names (`GetDayName`)

| IDs | 1 | 2 | 3 | 4 | 5 | 6 | 7 |
|---|---|---|---|---|---|---|---|
| 0x01, 0x02, 0x09, 0x0C, 0x0D, 0x10, 0x13 | `Monday` | `Tuesday` | `Wednesday` | `Thursday` | `Friday` | `Saturday` | `Sunday` |
| 0x03 | `Lundi` | `Mardi` | `Mercredi` | `Jeudi` | `Vendredi` | `Samedi` | `Dimanche` |
| 0x04 | `Montag` | `Dienstag` | `Mittwoch` | `Donnerstag` | `Freitag` | `Samstag` | `Sonntag` |
| 0x05 | `Lunedì` | `Martedì` | `Mercoledì` | `Giovedì` | `Venerdì` | `Sabato` | `Domenica` |
| 0x06 | `Lunes` | `Martes` | `Miércoles` | `Jueves` | `Viernes` | `Sábado` | `Domingo` |
| 0x07 | `Maandag` | `Dinsdag` | `Woensdag` | `Donderdag` | `Vrijdag` | `Zaterdag` | `Zondag` |
| 0x08, 0x0A | `Mandag` | `Tirsdag` | `Onsdag` | `Torsdag` | `Fredag` | `Lørdag` | `Søndag` |
| 0x0B | `Maanantai` | `Tiistai` | `Keskiviikko` | `Torstai` | `Perjantai` | `Lauantai` | `Sunnuntai` |
| 0x0E, 0x23 | `Segunda-feira` | `Ter\xe7a-feira` | `Quarta-feira` | `Quinta-feira` | `Sexta-feira` | `S\xe1bado` | `Domingo` |
| 0x0F | `月曜日` | `火曜日` | `水曜日` | `木曜日` | `金曜日` | `土曜日` | `日曜日` |
| 0x11, 0x12 | `星期一` | `星期二` | `星期三` | `星期四` | `星期五` | `星期六` | `星期天` |
| 0x14 | `월요일` | `화요일` | `수요일` | `목요일` | `금요일` | `토요일` | `일요일` |

### 5.2 Abbreviated day names (`GetAbbrDayName`)

| IDs | 1 | 2 | 3 | 4 | 5 | 6 | 7 |
|---|---|---|---|---|---|---|---|
| 0x01, 0x02 | `Mon` | `Tue` | `Wed` | `Thur` | `Fri` | `Sat` | `Sun` |
| 0x03 | `lun` | `mar` | `mer` | `jeu` | `ven` | `sam` | `dim` |
| 0x04 | `Mo` | `Di` | `Mi` | `Do` | `Fr` | `Sa` | `So` |
| 0x05 | `Lun` | `Mar` | `Mer` | `Gio` | `Ven` | `Sab` | `Dom` |
| 0x06 | `L` | `M` | `X` | `J` | `V` | `S` | `D` |
| 0x07 | `Ma` | `Di` | `Wo` | `Do` | `Vr` | `Za` | `Zo` |
| 0x09 | `M\xe5[U+0006]E` | `Tis` | `Ons` | `Tor` | `Fre` | `L\xf6r` | `S\xf6n` |
| 0x0B | `ma` | `ti` | `ke` | `to` | `pe` | `la` | `su` |
| 0x0D | `\xcf\xed` | `\xc4\xf2` | `\xd1\xf0` | `\xd7\xf2` | `\xcf\xf2` | `\xd1\xe1` | `\xc4\xf1` |
| 0x0E, 0x23 | `Seg` | `Ter` | `Qua` | `Qui` | `Sex` | `Sab` | `Dom` |
| 0x0F | `月曜日` | `火曜日` | `水曜日` | `木曜日` | `金曜日` | `土曜日` | `日曜日` |
| 0x10 | `Pn` | `Wt` | `[U+008C]r` | `Cz` | `Pt` | `So` | `N` |
| 0x11, 0x12 | `星期一` | `星期二` | `星期三` | `星期四` | `星期五` | `星期六` | `星期天` |
| 0x13 | `\xa8` | `\xcd` | `\xbe` | `\xbe\xc4` | `\xc9` | `\xca` | `\xcd\xd2` |
| 0x14 | `월요일` | `화요일` | `수요일` | `목요일` | `금요일` | `토요일` | `일요일` |

### 5.3 Month names (`GetMonthName`)

| IDs | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 0x01, 0x02, 0x0C, 0x0D, 0x13 | `January` | `February` | `March` | `April` | `May` | `June` | `July` | `August` | `September` | `October` | `November` | `December` |
| 0x03 | `janvier` | `F\xe9vrier` | `mars` | `avril` | `mai` | `juin` | `juillet` | `Ao\xfbt` | `septembre` | `octobre` | `novembre` | `D\xe9cembre` |
| 0x04 | `Januar` | `Februar` | `M\xe4rz` | `April` | `Mai` | `Juni` | `Juli` | `August` | `September` | `Oktober` | `November` | `Dezember` |
| 0x05 | `Gennaio` | `Febbraio` | `Marzo` | `Aprile` | `Maggio` | `Giugno` | `Luglio` | `Agosto` | `Settembre` | `Ottobre` | `Novembre` | `Dicembre` |
| 0x06 | `Enero` | `Febrero` | `Marzo` | `Abril` | `Mayo` | `Junio` | `Julio` | `Agosto` | `Septiembre` | `Octubre` | `Noviembre` | `Diciembre` |
| 0x07 | `Januari` | `Februari` | `Maart` | `April` | `Mei` | `Juni` | `Juli` | `Augustus` | `September` | `Oktober` | `November` | `December` |
| 0x08 | `januar` | `februar` | `marts` | `april` | `maj` | `juni` | `juli` | `august` | `september` | `oktober` | `november` | `december` |
| 0x09 | `Januari` | `Februari` | `Mars` | `April` | `Maj` | `Juni` | `Juli` | `Augusti` | `September` | `Oktober` | `November` | `December` |
| 0x0B | `tamikuu` | `helmikuu` | `maaliskuu` | `huhtikuu` | `toukokuu` | `kes\xe4kuu` | `hein\xe4kuu` | `elokuu` | `syyskuu` | `lokakuu` | `marraskuu` | `joulukuu` |
| 0x0E, 0x23 | `Janeiro` | `Fevereiro` | `Mar\xe7o` | `Abril` | `Maio` | `Junho` | `Julho` | `Agosto` | `Setembro` | `Outubro` | `Novembro` | `Dezembro` |
| 0x0F | `1 月` | (none) | `3 月` | `4 月` | `5 月` | `6 月` | `7 月` | `8 月` | `9 月` | `10 月` | `11 月` | `12 月` |
| 0x10 | `Stycze\xf1` | `Luty` | `Marzec` | `Kwiecie\xf1` | `Maj` | `Czerwiec` | `Lipiec` | `Sierpie\xf1` | `Wrzesie\xf1` | `Pa[U+009F]dziernik` | `Listopad` | `Grudzie\xf1` |
| 0x11, 0x12 | `1月` | `2月` | `3月` | `4月` | `5月` | `6月` | `7月` | `8月` | `9月` | `10月` | `11月` | `12月` |
| 0x14 | `1월` | `이월` | `3월` | `4월` | `오월` | `유월` | `칠월` | `팔월` | `9월` | `10월` | `11월` | `십이월` |

### 5.4 Abbreviated month names (`GetAbbrMonthName`)

| IDs | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 0x01, 0x02, 0x08, 0x0B, 0x0C, 0x0D, 0x13 | `Jan` | `Feb` | `Mar` | `Apr` | `May` | `Jun` | `Jul` | `Aug` | `Sep` | `Oct` | `Nov` | `Dec` |
| 0x03 | `jan` | `F\xe9v` | `mar` | `Avr` | `mai` | `juin` | `juil` | `Ao\xfbt` | `sept` | `oct` | `nov` | `D\xe9c` |
| 0x04 | `Jan` | `Feb` | `M\xe4r` | `Apr` | `Mai` | `Jun` | `Jul` | `Aug` | `Sep` | `Okt` | `Nov` | `Dez` |
| 0x05 | `Gen` | `Feb` | `Mar` | `Apr` | `Mag` | `Giu` | `Lug` | `Ago` | `Set` | `Ott` | `Nov` | `Dic` |
| 0x06 | `En` | `Feb` | `Mar` | `Abr` | `Mayo` | `Jun` | `Jul` | `Ag` | `Sep` | `Oct` | `Nov` | `Dic` |
| 0x07 | `Jan` | `Feb` | `Mrt` | `Apr` | `Mei` | `Jun` | `Jul` | `Aug` | `Sep` | `Okt` | `Nov` | `Dec` |
| 0x09 | `Jan` | `Feb` | `Mar` | `Apr` | `Maj` | `Jun` | `Jul` | `Aug` | `Sep` | `Okt` | `Nov` | `Dec` |
| 0x0E, 0x23 | `Jan` | `Fev` | `Mar` | `Abr` | `Mai` | `Jun` | `Jul` | `Ago` | `Set` | `Out` | `Nov` | `Dez` |
| 0x0F | `1 月` | `2 月` | `3 月` | `4 月` | `5 月` | `6 月` | `7 月` | `8 月` | `9 月` | `10 月` | `11 月` | `12 月` |
| 0x10 | `Sty` | `Lut` | `Mar` | `Kwi` | `Maj` | `Cze` | `Lip` | `Sie` | `Wrz` | `Pa\x9f` | `Lis` | `Gru` |
| 0x11, 0x12 | `1月` | `2月` | `3月` | `4月` | `5月` | `6月` | `7月` | `8月` | `9月` | `10月` | `11月` | `12月` |
| 0x14 | `1월` | `이월` | `3월` | `4월` | `오월` | `유월` | `칠월` | `팔월` | `9월` | `10월` | `11월` | `십이월` |

## 6. Open questions (RE side)

- **Q1 - currency flag (+0x10):** no write to the utility's byte +0x10 was found in this class or in
  `cRZLanguageManager`; if nothing sets it, every `GetCurrencySymbol` and money string uses `$` in every
  language. An in-game check in a non-English install (does the budget show `$`?) or a search for writes at
  offset +0x48 of the language manager would settle it.
- **Q2 - callers:** which game code calls slots 9, 10, 16..19 (budget, query and date displays) and with which
  `style` / `format` values was not traced; this spec covers the service's outputs only.
- **Q3 - negative zero:** step 4 of §4.4 prints the negated value for `value <= 0`, so 0.0 is printed from
  −0.0; whether the game's C runtime prints `-0.00` (making `$-0.00`) or `0.00` was not measured.
- Q4 and Q5 were checked against the instructions and are rules, not questions: the case converters take
  the length from the **out** string on entry (`0x00927EA9`, slot +0x1C of out) and `MakeFormattedNumberString`
  makes one varargs call to out's slot +0x6C with (format text, double) (`0x00927C0A`).
