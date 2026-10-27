# Spec: strings and language (language IDs, framework string encoding, `cRZLanguageManager`, `cRZString`)

Status: **behaviour spec, partial**, clean-room input. This document describes *behaviour*, not code. It was written
by the analysis team from observing the original program and its data files. Implementers must work from this spec,
the public LGPL gzcom-dll SDK headers and general knowledge only. They must **not** consult decompiled or
disassembled code (see [CONTRIBUTING.md](../../CONTRIBUTING.md)).

Normative words and tags (*[compat]*, *[choice]*, *[inferred]*, *[quirk]*) as in [foundations.md](foundations.md).
Written 2026-10-21 by the re-strings4 lane. Evidence: the Ghidra export (instruction bytes, jump tables, string data)
and the decomp/ pseudocode; every name table below was produced by walking the instructions of each function for
every language ID 0..0x2d and every index, and every string is quoted from the bytes stored in the program. Every
address is SimCity 4.exe GOG 1.1.641. No modern Unicode semantics are implied: strings are byte strings.

Outline: §1 language IDs; §2 framework encoding; §3 the language utility (canonical contract in
[language-utility.md](language-utility.md), reconciliation notes here); §4 the language manager;
the string object `cRZString` and base `cSCStringDetokenizer` are specified in
[rz-strings.md](rz-strings.md); §9 lists other boundaries.

## 1. Language IDs

A language is a 32-bit ID. The English names the language manager reports (`GetLanguageEnglishName`, 00924798):

| ID | name | ID | name | ID | name | ID | name |
|---|---|---|---|---|---|---|---|
| 1 | English | 0x0c | Hebrew | 0x17 | Bulgarian | 0x22 | Turkish |
| 2 | UK English | 0x0d | Russian | 0x18 | Cyrillic | 0x23 | Brazilian Portuguese |
| 3 | French | 0x0e | Portuguese | 0x19 | Ukranian (sic) | 0x24 | Swiss French |
| 4 | German | 0x0f | Japanese | 0x1a | Czech | 0x25 | Canadian French |
| 5 | Italian | 0x10 | Polish | 0x1b | Greek | 0x26 | Belgian French |
| 6 | Spanish | 0x11 | Simplified Chinese | 0x1c | Hungarian | 0x27 | Swiss German |
| 7 | Dutch | 0x12 | Traditional Chinese | 0x1d | Icelandic | 0x28 | Swiss Italian |
| 8 | Danish | 0x13 | Thai | 0x1e | Romanian | 0x29 | Belgian Dutch |
| 9 | Swedish | 0x14 | Korean | 0x1f | Latin | 0x2a | Mexican Spanish |
| 0x0a | Norwegian | 0x15 | Hindi | 0x20 | Slovak | 0x2b | Tagalog |
| 0x0b | Finnish | 0x16 | Arabic | 0x21 | Albanian | 0x2c | Vietnamese |

IDs 0x2D and above have no name (§4.4). The rest of the language manager is §4.

## 2. Framework string encoding

The standalone counted-byte helper `UTF8IsStringValid` has its own
[validation contract](utf8-validation.md). It is a separate free-function boundary from the
language owners, `cRZString` storage and `ConvertStringEncoding`; its accepted byte forms
must be taken from that contract rather than inferred from the encoding label.

- The framework's current code page (00940da8) is the constant **UTF-8** in this build. Encoding codes used by the
  string converter (`ConvertStringEncoding`, 00910e18): 8 = UTF-8, 0x10 = UTF-16, 0xFFFFFFFE = "the framework
  encoding" (treated as 8), 0xFFFFFFFF = "the system ANSI code page". The converter's full behaviour is §9.
- Every name/symbol getter in §4 and §5 ends with a "convert UTF-8 to the destination format" step (00923d58). With
  the framework code page fixed at UTF-8 that step returns immediately: **the stored bytes are delivered unchanged**
  *[compat]*. (If the framework code page were neither UTF-8 nor UTF-16, the step would convert the string from
  UTF-8 into that code page through a 256-byte buffer; unreachable in this build.)
- *[quirk]* The tables are not uniformly UTF-8. Many entries are stored in a legacy single-byte code page (Latin-1 /
  cp1252 for French, German, Portuguese, Finnish, Swedish; cp1250 for Polish; cp1251 for Russian; cp874 for Thai) or
  are double-encoded. Because the conversion is a no-op those raw bytes reach the caller. This spec writes such bytes
  as `\xNN`; an implementation must return exactly those bytes, not a "corrected" UTF-8 form.

## 3. The language utility (`cRZLanguageUtility`): see language-utility.md

The canonical contract for every one of the 33 slots of `cRZLanguageUtility` (identity and lifetime, attribute
queries, currency, separators, `MakeNumberString`, `MakeMoneyString`, `MakeFormattedMoneyString`,
`MakeFormattedNumberString`, `MakeTimeString`, `MakeDateString`, `SetLanguage`, `SetCountry`, case conversion,
comparison and the four day/month name tables) is [language-utility.md](language-utility.md). This spec does not
repeat it. The two specs were written independently from the same bytes; they were compared row by row on
2026-10-21 (all 79 (table, language) rows of the four name tables, the currency, separator, flag, number and time
tables). Every disagreement was settled from the bytes in favour of language-utility.md; nothing in it is
contradicted. The rows below record those checks and the details it does not state (they add to it, they do not
replace it):

| Point | Rule (checked in the program) |
|---|---|
| Swedish abbreviated Monday (`GetAbbrDayName`, ID 9, day 1) | the stored bytes are `4D E5 06 45` (`M`, `\xE5`, U+0006, `E`); language-utility.md §5.2 is right. |
| Index width in the four name getters | only the low byte of the day/month index is read (00925a53, 009265df): index 0x101 behaves as 1. |
| `MakeTimeString` with second ≥ 0 and minute < 0 | hour, minute and second are all printed (the minute as a large unsigned number); a negative minute only drops the minute when the second is also negative. |
| `GetCharacterSetFromLanguage` order | asks 1252, 1250, 874, 932, 936, 950, 949, 1251, 1251 again (00925780, 0092578c), 1253, 1254, 1255, 1256, 1257; the repeated query changes nothing, and the result is as language-utility.md §2 states. |
| `MakeNumberString` with the most negative 64-bit value | negation leaves it negative, so the printed magnitude starts with `-`, which the grouping counts as a digit: `-9,223,372,036,854,775,808` gets an extra `-` in front for the `-` + grouped languages (`--9,223,...`) *[quirk]*. |
| Who sets the country | `cRZLanguageManager`: its constructor (0x36) and `SetCurrentCountry` (§4.2); `GetNewLanguageUtility` leaves it at 0x36. |
| Who sets the currency flag (+0x10; language-utility.md Q1) | in the code of both language classes (00923ded..0092960f) the only write is the utility constructor's store of 0 (0092849f); neither vtable has a setter, so through the manager every currency symbol is `$`. Writes from outside these classes were not searched. |

## 4. The language manager (`cRZLanguageManager`)

Data-formats §6.2 says which language folder and LTEXT group the game uses; this section is the manager's own
contract. Language IDs are §1; country IDs are §4.6. All tables below were produced by running the original
instructions of each function concretely for every input (languages 0..0x2f, countries 0..0x3f, Windows LANGIDs
0..0xFFFF) and are quoted from the stored bytes.

### 4.1 Identity, state and lifetime *[compat]*

| Item | Rule |
|---|---|
| Service | a framework system service with service ID `0x441E5070` (`kGZLanguageManagerSysServiceID`) and priority `0xFFD23940`. `Init` and `Shutdown` (slots 3, 4) do nothing and return true. |
| QueryInterface (00923ded) | iid `0x0054B7D5` (the language-manager interface): stores the object itself. iid `0x287FB697` (the system-service interface): stores the service sub-object (the object + 4; null stays null). Both AddRef the object and return true. Any other iid, **including 1** (the unknown interface): false, out untouched. AddRef/Release are the shared framework bodies. |
| State | current language (starts 1), current country (starts 0x36), a set of "available" language IDs (starts empty), and one embedded language utility. |
| Constructor (00928afe) | builds the embedded utility, calls its `Init`, `SetLanguage(1)`, `SetCountry(0x36)`. |
| `GetCurrentLanguage` (slot 7) / `GetCurrentCountry` (slot 28) | return the stored IDs. |
| `GetLanguageUtility(id)` (slot 38, 00924b50) | id 0 means the current language. Returns the embedded utility when id equals the current language, otherwise null. No reference is added. |
| `GetNewLanguageUtility(id)` (slot 39, 00928beb) | a new utility with reference count 1 (the caller owns it), `SetLanguage(id)` with **id stored verbatim, including 0**. It does not substitute the current language (`0x00928C0C–0x00928C14`, `0x009B2970–0x009B2979`); `Init` is not called and the country stays 0x36. *[compat]* |

### 4.2 Switching language and country *[compat]*

- `SetCurrentLanguage(id)` (slot 8, 00928021). id 0 means 1. If id equals the current language: return true,
  nothing else happens. Otherwise, if `CanSwitchToLanguage(id)` is false: return false, nothing changes.
  Otherwise the manager stores id, calls the embedded utility's `SetLanguage(id)`, then asks
  `GetLanguageRuntimeLibraryName(id)` (§4.5). When that succeeds, the C runtime locale is set with
  `setlocale(LC_ALL, name)` and, if that call succeeds, the multibyte code page is set to the ANSI code page
  (`_setmbcp(_MB_CP_ANSI)`); the result is true. When the name lookup fails (IDs 0x2D and above), the result is
  **false although the language has already been switched** *[quirk]*.
- `SetCurrentCountry(id)` (slot 29, 0092832a): id 0 means 0x36. Stores it, calls the utility's `SetCountry(id)`,
  returns true. No validation.
- `CanSwitchToLanguage(id)` (slot 9, 00927db5): true when the available set is **empty**, or when it contains id;
  false otherwise. So with no `AddAvailableLanguage` call every ID, even an unknown one, is accepted.
- `AddAvailableLanguage(id)` (slot 5, 00927e04): adds id to the set when id ≤ 0x2C (unsigned); larger IDs are
  ignored. `RemoveAvailableLanguage(id)` (slot 6, 00928bd2) removes it (no effect when absent).
- `GetNextAvailableLanguage(&id)` (slot 14, 00924652): repeat: if id > 0x2C, add 1 to id and return false; else
  add 1 to id and stop with true when `CanSwitchToLanguage(id)` is true. Start with id 0 to get the first one.
  *[quirk]*: from id 0x2C with an empty set it returns true with id 0x2D.
- `GetAllAvailableLanguagesIntoStringList(list)` (slot 47, 00927e28): empties the caller's list of IDs and
  appends every ID in the set in ascending order (an empty set gives an empty list, although every language is
  then switchable).
- Slot 46 (00929609, the export calls it a destructor; it is not) takes a list of strings: empties it, then
  for id = 1 while id ≤ 0x2C: `GetNextAvailableLanguage(&id)` (which itself advances id); when true, append
  `GetLanguageEnglishName(id)`; then add 1 to id. The double advance **skips every other available language**
  *[quirk]*: with an empty set the list is the names of 2, 4, 6, …, 0x2C (UK English, German, Spanish, …), and
  English (1) is never listed.

### 4.3 System language mapping *[compat]*

`GetCurrentSystemLanguage` (slot 10, 00923e28) = `GetLanguageFromSystemLanguage(GetUserDefaultLCID() & 0xFFFF)`.

`GetSystemLanguageFromLanguage(id)` (slot 11, 00923e3e), id 0 meaning the current language, returns a Windows
LANGID: 1 0x409, 2 0x809, 3 0x40C, 4 0x407, 5 0x410, 6 0x40A, 7 0x413, 8 0x406, 9 0x41D, 0x0A 0x414, 0x0B 0x40B,
0x0C 0x40D, 0x0D 0x419, 0x0E 0x816, 0x0F 0x411, 0x10 0x415, 0x11 0x804, 0x12 0x404, 0x13 0x41E, 0x14 0x412,
0x15 0x439, 0x16 0x401, 0x17 0x402, 0x19 0x422, 0x1A 0x405, 0x1B 0x408, 0x1C 0x40E, 0x1D 0x40F, 0x1E 0x418,
0x20 0x41B, 0x21 0x41C, 0x22 0x41F, 0x23 0x416, 0x2A 0x80A, 0x2C 0x42A. Every other ID (0x18, 0x1F, 0x24..0x29,
0x2B, 0x2D and above) returns **1** (not a valid LANGID) *[quirk]*.

`GetLanguageFromSystemLanguage(langid)` (slot 12, 00924038) uses the low 16 bits only and returns:

| Language | LANGIDs |
|---|---|
| 2 | 0x809, 0xC09, 0x1409, 0x1809, 0x1C09, 0x2009, 0x2409, 0x2809, 0x2C09, 0x3009, 0x3409 |
| 3 | 0x40C, 0x80C, 0xC0C, 0x100C, 0x140C, 0x180C |
| 4 | 0x407, 0x807, 0xC07, 0x1007, 0x1407 |
| 5 / 7 / 9 / 0x0A / 0x14 | 0x410, 0x810 / 0x413, 0x813 / 0x41D, 0x81D / 0x414, 0x814 / 0x412, 0x812 |
| 6 | 0x40A and 0xC0A, 0x100A, 0x140A, …, 0x500A (every 0x400 step from 0xC0A to 0x500A) |
| 0x11 / 0x12 | 0x804, 0x1004, 0x1404 / 0x404, 0xC04 |
| 0x16 | 0x401, 0x801, 0xC01, …, 0x4001 (every 0x400 step) |
| 8, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10, 0x13, 0x15, 0x17, 0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x20, 0x21, 0x22, 0x23, 0x2A, 0x2C | the single LANGID listed for that ID above (e.g. 0x0E only from 0x816, 0x23 only from 0x416, 0x2A only from 0x80A) |
| 1 | everything else, including 0x409 and 0 |

So the mapping is not a round trip: 0x80A (Mexican Spanish) goes to 0x2A but 0xC0A (modern Spanish) to 6, and
Portuguese (Portugal) 0x816 to 0x0E but Brazilian 0x416 to 0x23.

`GetCurrentSystemCountry` (slot 32, 009253d7) always returns 0xFFFFFFFF.

### 4.4 Language names and codes *[compat]*

Common shape: id 0 means the current language. The getter replaces `out` with the entry and returns true; IDs
with no entry get the stated fallback and false. All names are byte strings (§2).

- `GetLanguageEnglishName(out, id)` (slot 16, 00924798): §1 for 1..0x2C; others: `out` = empty, false.
  `GetLanguageEnglishNameFromLanguageID` (slot 21) is the same call.
- `GetLanguageLocalName(out, id)` (slot 17, 00924a02): entries 1, 2 `English`; 3 `Fran` C3 87 `ais` (the stored
  byte pair is capital `Ç`) *[quirk]*; 4 `Deutsch`; 5 `Italiano`; 6 `Español` (UTF-8); 7 `Nederlands`; 8 `Dansk`;
  9 `Svensk`; 0x0B `Suomi`; 0x0E, 0x23 `Portugues`; 0x0A, 0x0C, 0x0D, 0x0F..0x14 `Untranslated`; other IDs:
  `out` untouched. **Then**, if `out` is non-empty, it is replaced by `Untranslated` and the result is false;
  if it is empty the result is true (the test is inverted compared with the utility's name getters)
  *[quirk]*. Net effect: every call yields `Untranslated`/false except an empty `out` passed for an ID with no
  entry, which stays empty with true. `GetLanguageLocalNameFromLanguageID` (slot 22) is the same call.
- `GetLanguageDirectoryName(out, id)` (slot 15, 0092467e): the folder name. Special names: 2 `UKEnglsh`,
  0x0A `Norwgian`, 0x0E `Portuges`, 0x11 `SChinese`, 0x12 `TChinese`, 0x17 `Blgarian`, 0x1C `Hngarian`,
  0x1D `Icelandc`, 0x23 `Portgese`, 0x24 `SFrench`, 0x25 `CFrench`, 0x26 `BFrench`, 0x27 `SGerman`,
  0x28 `SItalian`, 0x29 `BDutch`, 0x2A `MSpanish`, 0x2C `Vietnmse`; every other ID uses
  `GetLanguageEnglishName` (so `English`, `French`, `Thai`, `Ukranian`, `Tagalog`, …; empty for 0x2D+). Returns
  true when `out` is then non-empty.
- `GetLanguageIDAlias(id)` (slot 23, 00924aef): id 0 means current; 2 → 1; 0x24, 0x25, 0x26 → 3; 0x27 → 4;
  0x28 → 5; 0x29 → 7; 0x2A → 6; 0x17, 0x18, 0x19 → 0x0D; any other id is returned unchanged.
- `GetISO639LanguageCodeFromLanguageID(out, id)` (slot 24, 00924bbd): 1 `en`, 2 `uk`, 3 `fr`, 4 `de`, 5 `it`,
  6 `es`, 7 `nl`, 8 `da`, 9 `sv`, 0x0A `no`, 0x0B `fi`, 0x0C `iw`, 0x0D `ru`, 0x0E `pt`, 0x0F `ja`, 0x10 `pl`,
  0x11 `zh`, 0x12 `zh`, 0x13 `th`, 0x14 `ko`, 0x15 `hi`, 0x16 `ar`, 0x17 `bg`, 0x18 `ru`, 0x19 `uk`, 0x1A `cs`,
  0x1B `el`, 0x1C `hu`, 0x1D `is`, 0x1E `ro`, 0x1F `la`, 0x20 `sk`, 0x21 `al`, 0x22 `tr`, 0x23 `pt`, 0x24..0x26
  `fr`, 0x27 `gr`, 0x28 `it`, 0x29 `nl`, 0x2A `sp`, 0x2B `tl`, 0x2C `vi`; 0x2D+: empty, false. *[quirk]* UK
  English reports `uk` (the Ukrainian code), Albanian `al`, Swiss German `gr`, Mexican Spanish `sp`.

### 4.5 Runtime locale names *[compat]*

`GetLanguageRuntimeLibraryName(out, id)` (slot 13, 0092440f), id 0 meaning current: 1 `english-us`,
2 `english-uk`, 3 `french`, 4 `german`, 5 `italian`, 6 `spanish`, 7 `dutch`, 8 `danish`, 9 `swedish`,
0x0A `norwegian`, 0x0B `finnish`, 0x0C `hebrew`, 0x0E and 0x23 `portuguese`, 0x0F `japanese`, 0x10 `polish`,
0x11 `chinese-simplified`, 0x12 `chinese-traditional`, 0x13 `thai`, 0x14 `korean`, 0x15 `hindi`, 0x16 `arabic`,
0x1A `czech`, 0x1B `greek`, 0x1C `hungarian`, 0x1D `icelandic`, 0x1E `romanian`, 0x1F `latin`, 0x20 `slovak`,
0x21 `albanian`, 0x22 `turkish`, 0x24 `french-swiss`, 0x25 `french-canadian`, 0x26 `french-belgian`,
0x27 `german-swiss`, 0x28 `italian-swiss`, 0x29 `dutch-belgian`, 0x2A `spanish-mexican`, 0x2B `tagalog`,
0x2C `vietnamese`; 0x0D, 0x17, 0x18, 0x19 `russian`. Returns true for 1..0x2C; for 0x2D and above `out` is
untouched and the result is false. These strings are what `SetCurrentLanguage` passes to `setlocale`; names
the C runtime does not know (e.g. `latin`) make that call fail, which `SetCurrentLanguage` ignores.

### 4.6 Countries *[compat]*

Country IDs 1..0x38 with `GetCountryEnglishName(out, id)` (slot 34, 009253db) and
`GetISO3166CountryCodeFromCountryID(out, id)` (slot 26, 00924dcd); id 0 means the current country:

| ID | name | code | ID | name | code | ID | name | code | ID | name | code |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | Albania | al | 0x0F | Egypt | eg | 0x1D | Israel | il | 0x2B | Slovakia | sk |
| 2 | Argentina | ar | 0x10 | Finland | fi | 0x1E | Italy | it | 0x2C | Slovenia | si |
| 3 | Australia | au | 0x11 | France | fr | 0x1F | Japan | jp | 0x2D | Spain | es |
| 4 | Austria | at | 0x12 | Georgia | ge | 0x20 | Korea | kr | 0x2E | Sweden | se |
| 5 | Belgium | be | 0x13 | Germany | de | 0x21 | Mexico | mx | 0x2F | Switzerland | ch |
| 6 | Brazil | br | 0x14 | Greece | gr | 0x22 | Netherlands | nl | 0x30 | Syria | sy |
| 7 | Bulgaria | bg | 0x15 | Holy See | (none) | 0x23 | New Zealand | nz | 0x31 | Taiwan | tw |
| 8 | Canada | ca | 0x16 | Hong Kong | hk | 0x24 | Norway | no | 0x32 | Thailand | th |
| 9 | Chile | cl | 0x17 | `Hungray` (sic) | hu | 0x25 | Philippines | ph | 0x33 | Turkey | tr |
| 0x0A | China | cn | 0x18 | Iceland | is | 0x26 | Poland | pl | 0x34 | Ukraine | ua |
| 0x0B | Colombia | co | 0x19 | India | in | 0x27 | Portugal | pt | 0x35 | United Kingdom | gb |
| 0x0C | Cuba | cu | 0x1A | Iran | ir | 0x28 | Romania | ro | 0x36 | United States | us |
| 0x0D | Czech Republic | cz | 0x1B | Iraq | iq | 0x29 | Russia | ru | 0x37 | Vietnam | vn |
| 0x0E | Denmark | dk | 0x1C | Ireland | ie | 0x2A | Singapore | sg | 0x38 | Yugoslavia | yu |

Fallbacks (result false): the name getter writes `UnitedStates` (no space) for IDs above 0x38; the code getter
writes an empty string for 0x15 and IDs above 0x38. `GetCountryLocalName` (slot 35) is the English name call.

`GetPrimaryLanguageForCountry(country)` (slot 30, 009250d5), country 0 meaning current, returns a language ID:
8, 0x0F, 0x19, 0x2C, 0x30,
0x36 → 1; 2, 9, 0x0B, 0x0C, 0x2D → 6; 3, 0x1C, 0x23, 0x2F, 0x35 → 2; 4, 0x13 → 4; 5 → 0x26; 6 → 0x23;
7 → 0x17; 0x0A, 0x2A → 0x11; 0x0D → 0x1A; 0x0E → 8; 0x10 → 0x0B; 0x12, 0x29 → 0x0D; 0x14 → 0x1B; 0x16, 0x31 →
0x12; 0x17 → 0x1C; 0x18 → 0x1D; 0x1A, 0x1B → 0x16; 0x1D → 0x0C; 0x1E → 5; 0x1F → 0x0F; 0x20 → 0x14;
0x21 → 0x2A; 0x22 → 7; 0x24 → 0x0A; 0x25 → 0x2B; 0x26 → 0x10; 0x27 → 0x0E; 0x28 → 0x1E; 0x2B, 0x38 → 0x20;
0x2E → 9; 0x32 → 0x13; 0x34 → 0x19; 0x37 → 0x2C; every other country (1, 0x11, 0x15, 0x33, above 0x38) → 0.
*[quirk]* Canada → English, Switzerland and Australia → UK English, France → 0 (none), Georgia → Russian.

`GetDefaultCountryForLanguage(language)` (slot 31, 0092525a), 0 meaning current: 2 → 0x35, 3 → 0x11, 4 → 0x13,
5 and 0x1F → 0x1E, 6 → 0x2D, 7 → 0x22, 8 → 0x0E, 9 → 0x2E, 0x0A → 0x24, 0x0B → 0x10, 0x0C → 0x1D, 0x0D and
0x18 → 0x29, 0x0E → 0x27, 0x0F → 0x1F, 0x10 → 0x26, 0x11 → 0x0A, 0x12 → 0x16, 0x13 → 0x32, 0x14 → 0x20,
0x15 → 0x19, 0x16 → 0x1A, 0x17 → 7, 0x19 → 0x34, 0x1A → 0x0D, 0x1B → 0x14, 0x1C → 0x17, 0x1D → 0x18,
0x1E → 0x28, 0x20 → 0x2B, 0x21 → 1, 0x22 → 0x33, 0x23 → 6, 0x24, 0x27, 0x28 → 0x2F, 0x25 → 8, 0x26 and
0x29 → 5, 0x2A → 0x21, 0x2B → 0x25, 0x2C → 0x37; everything else (1, 0x2D and above) → 0x36.

### 4.7 Reverse lookups *[compat]*

Each walks the IDs upward, asks the getter, skips IDs whose getter returns false, and compares with the
embedded utility's `CompareStrings(text, entry, caseSensitive = false)` (language-utility.md §4.8): the first
equal entry wins and its ID is stored, result true. No match: the fallback ID is stored, result false.

| Function (slot) | IDs walked | getter | fallback |
|---|---|---|---|
| `GetLanguageIDFromLanguageEnglishName(text, &id)` (18, 009280b8) | 1..0x2C | English name | 1 |
| `GetLanguageIDFromLanguageLocalName` (19, 0092813e) | 1..0x2C | local name (§4.4) | 1 |
| `GetISO639LanguageIDFromLanguageCode` (25, 0092821e) | 1..0x2C | ISO 639 code | 1 |
| `GetISO3166CountryIDFromCountryCode` (27, 009282a4) | 1..0x39 | ISO 3166 code | 0x36 |
| `GetCountryIDFromCountryEnglishName` / `…LocalName` (36, 37; 0092836a, 009283f3) | 1..0x39 | country name | 0x36 |

Consequences *[quirk]*: the local-name lookup never matches (§4.4 makes every local-name call false), so it
always gives 1/false; `uk` finds 2 (UK English), `fr` 3, `pt` 0x0E, `zh` 0x11, `ru` 0x0D; `us` finds 0x36.

`GetLanguageIDFromEitherEnglishOrLocalName(text, &id)` (slot 20, 009281c4) copies `text` and calls **its own slot
20** again with the copy, so it recurses until the stack is exhausted; the original never returns from it
*[quirk]*. No caller was found. *[choice]* An implementation should try the English-name lookup, then the
local-name lookup, and on success write the matched name back into `text` (what the code after the call does).

### 4.8 Money conversion stubs *[compat]*

`GetExchangeRateForLocale(locale)` (slot 40) returns 1.0 (float). The four `ConvertMoney…` slots (42..45:
64-bit and 32-bit forms of base-to-locale and locale-to-locale) copy the input amount unchanged into the output
and return true. Slot 33 takes one argument and returns true; slot 41 takes two and does nothing (both
folded no-op bodies whose names are not known).

## 9. Not yet specified

The standalone `ConvertStringEncoding` core, bridge and wrappers are specified in
[string-encoding.md](string-encoding.md), including their counted units and partial-output behavior.

The string object and base token replacement service are specified in
[rz-strings.md](rz-strings.md), including escaping, callback precedence and partial failures.
Still unspecified here: the UTF-16 / Thai helpers; `gAdviceDetokenizerFunction`;
which game code calls the language manager and how the startup language is chosen
(`cSC4App::DoPreferencesChanged` reads the registry values `HKEY_LOCAL_MACHINE\Software\Maxis\SimCity 4 Deluxe\1.0\Language`,
then `...\SimCity 4\1.0\Language`; data-formats §6.2 question 1).
