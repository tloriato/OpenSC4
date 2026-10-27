# Spec: dates and times (`cRZDate`, `cGZDate`, `cRZTime`)

Status: **behaviour spec**, clean-room input. This document describes *behaviour*, not code. It was written by the
analysis team from the Windows 1.1.641 executable (read in the instruction bytes and in a Ghidra 12.1.4 project of
the Steam build, which has the GOG build's `.text` bounds, PE time stamp, all 28,766 vtable slots of
`docs/abi/vtables.tsv` and all 648 entries of `re/class_id_table.tsv`). Implementers must work from this spec, the
public LGPL gzcom-dll SDK headers and general knowledge only. They must **not** consult decompiled or disassembled
code (see [CONTRIBUTING.md](../../CONTRIBUTING.md)).

Normative words: **must**, **should**, **may**. Tagged notes: *[compat]* is required for compatibility with the
original game and its plugins; *[choice]* is a behaviour we chose where the original was arbitrary or undefined;
*[inferred]* is consistent with all evidence but not proven; *[quirk]* is surprising original behaviour that is
kept on purpose; *[compat-limit]* depends on something FreeCity4 cannot reproduce (the wall clock, the host's time
zone), so only the rule is matched, not each value.

Related specs: [simulator.md](simulator.md) (§2.1 the simulation date, §7 `GetSimDate`),
[foundations.md](foundations.md) (§17.4 the `cIGZDate` slot table and the SDK's return-type error),
[crime.md](crime.md) (§9, the value of `cRZTime()`), [lot-records.md](lot-records.md) (§2.1, dates in records).

## 0. Summary

A **date** is one unsigned 32-bit **day number**: the Julian day number of a day in the proleptic Gregorian
calendar (2000-01-01 is 2,451,545). 0 means "no date". `cRZDate` is the value class that converts between day
numbers and month / day / year; `cGZDate` is the reference-counted COM object around it that implements
`cIGZDate` (the simulator's `GetSimDate()` returns one); `cRZTime` is a wall-clock instant, an unsigned count of
seconds since 1901-01-01 00:00 UTC.

The calendar part is exact integer arithmetic and fully reproducible. The time part reads the host's clock and
the C runtime's time-zone variables, and applies a fixed pre-2007 US daylight-saving rule *[compat-limit]*.

## 1. Day numbers and the calendar (`cRZDate`) *[compat]*

All arithmetic is on **unsigned 32-bit** integers: division truncates, subtraction wraps modulo 2³². This matters
only at the edges noted below.

### 1.1 Validity

`DayWithinMonth(m, d, y)` (`0x00912B08`) is true when `1 ≤ m ≤ 12`, `d ≥ 1` and `d ≤` the month's length:
31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31, plus 1 for February in a leap year. `LeapYear(y)` (`0x00912741`) is
the Gregorian rule: `y` divisible by 4 and not by 100, or divisible by 400. Year 0 is a leap year by this rule;
there is no lower or upper limit on the year.

### 1.2 Month / day / year to day number: `Jday(m, d, y)` (`0x00912B5F`)

If the date is not valid (§1.1) the result is **0**. Otherwise:

```
if m > 2:  m' := m − 3,  y' := y
else:      m' := m + 9,  y' := y − 1
Jday := (146097 × (y' / 100)) / 4  +  (1461 × (y' mod 100)) / 4  +  (153 × m' + 2) / 5  +  d  +  1721119
```

(`/` is unsigned integer division and each product is taken modulo 2³² before its division.) This is the standard
Julian day number for every year ≥ 1 and gives 2,451,545 for 2000-01-01 and 2,415,386 for 1901-01-01. For January
and February of **year 0**, `y − 1` wraps to 2³² − 1, so the result is a large meaningless number (1 January 0 gives
1,043,250,630) *[quirk]*.

### 1.3 Day number to month / day / year: `Mdy(n)` (`0x00912789`)

The inverse of §1.2, in the same unsigned arithmetic:

```
t := 4 × (n − 1721119)
c := (t − 1) / 146097
u := 4 × ((t − 146097 × c − 1) / 4)
j := (u + 3) / 1461
v := 5 × ((u − 1461 × j + 7) / 4)
m := (v − 3) / 153
d := (v − 153 × m + 2) / 5
y := 100 × c + j
if m < 10: m := m + 3   else: m := m − 9, y := y + 1
```

For every valid date with year ≥ 1, `Mdy(Jday(m, d, y)) = (m, d, y)`. Applied to the "no date" value 0 it gives
**28 February 2,935,093** *[quirk]*: an invalid `cIGZDate` reports month 2, day 28 and that year.

`Month()` (`0x0091286F`), `DayOfMonth()` (`0x00912BEB`) and `Year()` (`0x009128A3`) each run `Mdy` and return one
field.

### 1.4 Other queries

| Query | Address | Result |
|---|---|---|
| `WeekDay()` | `0x0091288B` | `((n + 1) mod 7 + 6) mod 7 + 1`: 1 = Monday … 7 = Sunday, so 2000-01-01 is 6 (simulator.md §2.1) |
| `DayOfYear()` | `0x00912BCC` | `n − Jday(12, 31, Year() − 1)`: 1 for 1 January |
| `FirstDayOfMonth(m)` | `0x00912C07` | the day of year of the 1st of month `m` in this date's year: 1, 32, 60, 91, 121, 152, 182, 213, 244, 274, 305, 335, plus 1 for `m > 2` in a leap year; **0** for `m` outside 1–12 |
| `DaysInYear(y)` | `0x00912B49` | 366 in a leap year, else 365 |
| `Previous(w)` | `0x00912C42` | the latest date **on or before** this one whose `WeekDay()` is `w` (1–7): with `a := WeekDay() − 1`, `b := w − 1` (unsigned), the result is `n − (a − b)` when `a ≥ b`, else `n − (a + 7 − b)` |
| `CompareTo(other)` | `0x0091276F` | −1, 0 or 1 by unsigned day number |
| `Between(a, b)` | `0x00912AD9` | true when `min(a, b) ≤ n ≤ max(a, b)` (inclusive, either order) |
| `Max(other)` / `Min(other)` | `0x0091282D` / `0x0091284E` | the later / earlier of the two (this date on a tie) |

A `Previous` argument outside 1–7 follows the same formula and gives a date that is not on any particular weekday.

### 1.5 Setting a date

| Setter | Address | Effect on the day number `n` |
|---|---|---|
| `Set(m, d, y)` | `0x00913113` | `Jday(m, d, y)` (0 when invalid) |
| `Set(dayOfYear, y)` | `0x009130E4` | `y = 0`: `2415386 + dayOfYear` (days after 1901-01-01, so 0 is 1901-01-01); otherwise `Jday(12, 31, y − 1) + dayOfYear`, with no range check (day 366 of a common year is 1 January of the next) |
| `Set(monthName, d, y)` | `0x00913131` | a bounded, case-insensitive **three-byte** comparison against `Jan`, `Feb`, … `Dec`, using the executable's current CRT `LC_CTYPE`; the **last** match wins and no match gives **January** *[quirk]*; then `Jday(month, d, y)`; details below (DT2) |
| `Set(n)` | `0x009B2C1C` | `n` as given, unchecked |
| `Set(time)` | `0x00913216` | 0 when the time is 0; otherwise `LocalSecs(time) / 86400 + 2415386`, the **local** date of the instant (§2.3) |

A new `cRZDate()` (`0x0091307C`) is **today**: the local date of the C runtime's `localtime(time())`
*[compat-limit]*. A static date at `0x00B62898` holds 1901-01-01 (2,415,386), built at program start with
`Set(0, 0)`.

**Month-name native contract (DT2).** The month starts at 1. All twelve entries in the pointer table at
`0x00B168FC` are examined in order; they point to the NUL-terminated English abbreviations at
`0x00ACE9C8`–`0x00ACE9F4`. At `0x0091313B`–`0x00913148`, the setter passes the table entry first,
the supplied name second, and a 32-bit count of **3**, to `_strnicmp` at `0x00A044F0`. Equality updates the
month to that entry's one-based index, without ending the loop (`0x00913150`–`0x0091315B`). This is a byte
comparison, not a Unicode, multibyte-character, or full-name comparison. A fourth byte, including an embedded
NUL followed by more text, is irrelevant after three equal bytes. Three readable matching bytes need no fourth
readable byte or terminating NUL.

The comparison stops at a mismatch or NUL. A shorter string is **not** accepted as a partial month name:
`"Ma"`, `"M"`, `""`, and the bytes `4D 00 61 79` all match nothing and therefore select January. A null name
has **no guard or January fallback**: the first comparison reads through it and faults; the date has not yet
been stored. These rules assume ordinary readable memory up to the stopping byte, not arbitrary invalid
pointers. The COM setter at `0x009131DD` forwards to this same implementation.

Case folding follows the **current locale of the executable's own CRT**, not a locale supplied by the caller
and not necessarily the locale of another DLL's CRT. `_strnicmp` obtains thread data and refreshes its locale
pointer against `0x00B1921C` (`0x00A044F5`–`0x00A0450A`). If that locale's `LC_CTYPE` handle at `+0x14` is
zero, it calls `__ascii_strnicmp` (`0x00A04512`–`0x00A04526`): only bytes `41`–`5A` are lowered by adding
`20`; comparisons are unsigned and the helper returns −1, 0, or 1 (`0x00A0533C`–`0x00A0537A`). Otherwise,
each input byte is zero-extended to 32 bits and folded separately by `__tolower_mt`, using that locale pointer;
the folded integers are subtracted (`0x00A04533`–`0x00A04565`). `__tolower_mt` includes an ASCII shortcut
when permitted by the locale, and otherwise uses character classification and the CRT's locale mapping
routine (`0x009F0487`–`0x009F053C`, locale mapping call `0x009F050C`). The setter tests only equality.
There is no fixed-C-locale override in this path. This identifies the dependency without assuming which
`LC_CTYPE` a particular game/plugin session has selected.

### 1.6 Text: `DateString(out, format)` (`0x00912C79`)

Writes the date with `printf`-style patterns (`%u` without leading zeros, `%4u` right-aligned in four characters
with spaces):

| `format` | Pattern | Order |
|---|---|---|
| 0, 1, `0xFFFFFFFF` | `%u/%u/%4u` | month / day / year |
| 3, 4, 10, 11 | `%u.%u.%4u` | day . month . year |
| 7, 8, 14 | `%u-%u-%4u` | day - month - year |
| 9 | `%4u.%u.%u` | year . month . day |
| 15 | `%4u/%u/%u` | year / month / day |
| any other (2, 5, 6, 12, 13, …) | `%u/%u/%4u` | day / month / year |

Both callers in the game pass format **0** (the query tool's desirability text `0x004C9D76` and the configuration
log `0x007CEFB3`), so the shipped game only ever writes month/day/year. The numbering looks like the language IDs
(1 US, 2 UK English …) *[inferred]* (DT1). The formatter itself never asks the language manager or the
CRT locale: it maps argument 0 to 1, then selects solely by the numeric argument
(`0x00912C7C–0x00912C99`, `0x00912CC7–0x00912CF3`). In particular 0 does **not** mean the current
language *[compat]*. The COM wrapper forwards the supplied code unchanged (`0x00913245–0x00913260`).
This proves the numeric behavior, but neither a shared public enum identity nor third-party usage.

For the COM wrapper's replacement-copy, return-register and failure rules, see §4.2; the base formatter
dispatches to the supplied string's `Sprintf` slot 27 (`0x00912DA4`–`0x00912DAF`).

## 2. Wall-clock instants (`cRZTime`) *[compat-limit]*

A `cRZTime` holds one unsigned 32-bit value: seconds since **1901-01-01 00:00 UTC** (day 2,415,386); 0 means "no
time". It uses the C runtime's `_timezone` (seconds west of UTC) and `_daylight` (non-zero when the zone has
daylight time), and its own daylight rule (§2.2) whatever the host's zone. crime.md §9 gives the value of
`cRZTime()` (`0x00470B50`, now) and how it relates to `time()`.

### 2.1 From a local date and time: `cRZTime(date, h, m, s)` (`0x00470010`)

1. A date of 0 gives 0.
2. `t := ((day number − 2415386) × 24 + h − 1) × 3600 + m × 60 + s`, that is, local time **one hour early**; if
   `t ≠ 0`, `t := t + _timezone`.
3. If `IsDST(t)`: `t + 3600` is kept unless `IsDST(t + 3600)` (then `t` is kept). Otherwise `t + 3600` is kept,
   unless `IsDST(t + 3600)`, in which case the result is **0**: a local time inside the spring-forward hour is
   invalid *[quirk]*.

The effect is the UTC instant of a local wall-clock time, with the gap hour rejected.

### 2.2 Daylight time: `IsDST(t)` (`0x00417730`)

False when `_daylight` is 0. Otherwise let `Y` be the year of the date `Set(t / 86400, 0)` (§1.5); daylight time
holds when `BeginDST(Y) ≤ t < EndDST(Y)`:

| Rule | Address | Instant (local standard time, then `+ _timezone`) |
|---|---|---|
| `BeginDST(Y)` | `0x00417600` | 1974: 6 January 02:00; 1975: 23 February 02:00; **every other year**: the Sunday on or before **8 April**, 02:00 |
| `EndDST(Y)` | `0x004176E0` | the Sunday on or before **31 October**, 01:00 |

So daylight time starts on the first Sunday of April, except that when 1 April is a Sunday it starts on 8 April
(1990, 2001, 2007, 2012 …) *[quirk]*, and the year is taken from the instant itself, not from local time.

### 2.3 Reading it back

| Query | Address | Result |
|---|---|---|
| `LocalSecs()` | `0x00417840` | `t − _timezone`, plus 3600 when `IsDST(t − _timezone)` |
| `Hour()` | `0x00417870` | `(LocalSecs() mod 86400) / 3600` |
| `Minute()` | `0x004178B0` | `(LocalSecs() mod 3600) / 60` |

`IsDST` is tested on `t − _timezone`, the local standard time, against limits that already include
`_timezone` *[quirk]*; near a transition the local reading can be an hour off.

## 3. The COM date (`cGZDate`, `cIGZDate`) *[compat]*

### 3.1 Object

| Offset | Content |
|---|---|
| `+0x00` | vtable `0x00ACEAF0` (the date's own methods: two do-nothing slots, `DateString` `0x00912C79`, the deleting destructor `0x00912DD6`, postfix `++` `0x00913018` and postfix `--` `0x00912FEF`, each returning a copy of the old value) |
| `+0x04` | the day number |
| `+0x08` | vtable `0x00ACEA38`: `cIGZDate` (46 slots, §4) |
| `+0x0C` | reference count, 0 at construction |

16 bytes, global heap. A plain `cRZDate` is the first 8 bytes (vtable `0x00A827D8`: the two do-nothing slots and
`DateString`).

Constructors: today `0x00913177`; `(m, d, y)` `0x0091319B`; from a day number `0x00912E7B`; from a `cRZDate`
`0x00912E4E`; from a `cIGZDate` `0x00912DFA` (it reads the other's `DayNumber()`); copy `0x00912E24`. The game
creates them mostly on the stack or inside other objects (history warehouse, advisors, prop occupants,
neighbours).

`QueryInterface` (slot 0, `0x009128E7`): IID `1` or **`0x43378255`** → the `cIGZDate` pointer (null if the object
pointer is null), `AddRef`, true; anything else → false and the out pointer unwritten. `AddRef` (`0x00968B60`,
shared) increments `+0x0C` and returns it. `Release` (`0x00912912`): when the count is **below 2, unsigned**, it
calls the deleting destructor (freeing to the global heap) and returns 0; otherwise decrements and returns the new
count.

### 3.2 Return values that differ from the SDK

- **The five `Set` overloads (slots 3–7) return the new day number in EAX**, not a computed `bool`. A caller that
  reads AL gets the day number's low byte: 0 for an invalid date, but also 0 for any valid date whose day number
  is a multiple of 256 *[quirk]*. A replacement must return the day number in EAX.
- Slots 33–36 return a day number, not a date (foundations.md §17.4).
- `Copy` (slot 8) returns what slot 4 returns, the new day number; `MaxDate`, `MinDate` and `Previous` (slots 21,
  22, 24) return what the out-date's `Copy` returns.
- Slots 37–40 (`++`, `--`, `+=`, `-=`) return the `cIGZDate` pointer itself.
- `Clone` (slot 9) returns full EAX = **1** on successful construction, from the new object's `AddRef`;
  it has no normally returning false path (§4.1).
- `DateString` (slot 41) supplies **no success boolean**. With the original string allocator, its final
  cleanup defines full EAX as the allocator lock's remaining nesting count, normally **0 even on success**
  (§4.2). Reading AL does not turn it into a success flag.

## 4. `cIGZDate` slot table *[compat]*

vtable `0x00ACEA38`. `n` is this date's day number. Pops are bytes popped by `ret`. The SDK's overload order for
`Set` and `FirstDayOfMonth` is reversed in the executable (foundations.md §17.4).

| Slot | Offset | Method | Address | Pops | Behaviour / return |
|---|---|---|---|---|---|
| 0 | `+0x00` | `QueryInterface(iid, void**)` | `0x009128E7` | 8 | §3.1; AL |
| 1 | `+0x04` | `AddRef()` | `0x00968B60` (shared) | 0 | EAX count |
| 2 | `+0x08` | `Release()` | `0x00912912` | 0 | EAX count, 0 when freed |
| 3 | `+0x0C` | `Set(char const* monthName, d, y)` | `0x009131DD` | 12 | §1.5; EAX `n` |
| 4 | `+0x10` | `Set(uint32 dayNumber)` | `0x0091292E` | 4 | `n := dayNumber`, unchecked; EAX `n` |
| 5 | `+0x14` | `Set(cRZTime const&)` | `0x0091323D` | 4 | §1.5; EAX `n` |
| 6 | `+0x18` | `Set(m, d, y)` | `0x009131D5` | 12 | `Jday(m, d, y)`; EAX `n` |
| 7 | `+0x1C` | `Set(dayOfYear, y)` | `0x009131CD` | 8 | §1.5; EAX `n` |
| 8 | `+0x20` | `Copy(cIGZDate const& from)` | `0x009128CD` | 4 | slot 4 with `from.DayNumber()` |
| 9 | `+0x24` | `Clone(cIGZDate** out)` | `0x00912E9F` | 4 | §4.1; new independently owned date into `*out`; full EAX = 1 on success |
| 10 | `+0x28` | `Between(a, b)` | `0x00912ED7` | 8 | §1.4; AL |
| 11 | `+0x2C` | `CompareTo(other)` | `0x00912938` | 4 | −1 / 0 / 1 |
| 12 | `+0x30` | `WeekDay()` | `0x0091295C` | 0 | 1 = Monday … 7 = Sunday |
| 13 | `+0x34` | `Year()` | `0x00912964` | 0 | |
| 14 | `+0x38` | `DayOfYear()` | `0x00912F0B` | 0 | |
| 15 | `+0x3C` | `DayOfMonth()` | `0x00912F13` | 0 | |
| 16 | `+0x40` | `FirstDayOfMonth(uint32 month)` | `0x00912F1B` | 4 | §1.4 |
| 17 | `+0x44` | `FirstDayOfMonth()` | `0x009131E5` | 0 | for `Month()` |
| 18 | `+0x48` | `Hash()` | `0x0091297E` | 0 | `n` |
| 19 | `+0x4C` | `IsValid()` | `0x0091296C` | 0 | `n ≠ 0`; AL |
| 20 | `+0x50` | `Leap()` | `0x00912F23` | 0 | `LeapYear(Year())`; AL |
| 21 | `+0x54` | `MaxDate(other, out)` | `0x00912F33` | 8 | `out.Copy(max(this, other))` |
| 22 | `+0x58` | `MinDate(other, out)` | `0x00912F71` | 8 | `out.Copy(min(this, other))` |
| 23 | `+0x5C` | `Month()` | `0x00912976` | 0 | |
| 24 | `+0x60` | `Previous(weekday, out)` | `0x00912FAF` | 8 | `out.Copy(Previous(weekday))` (§1.4) |
| 25 | `+0x64` | `YearLastTwoDigits()` | `0x00912FDD` | 0 | `Year() mod 100` |
| 26 | `+0x68` | `DayNumber()` | `0x0091297E` | 0 | `n` (the same body as slot 18) |
| 27 | `+0x6C` | `operator<(other)` | `0x00912982` | 4 | unsigned day numbers; AL |
| 28 | `+0x70` | `operator<=(other)` | `0x009129A4` | 4 | AL |
| 29 | `+0x74` | `operator>(other)` | `0x009129C5` | 4 | AL |
| 30 | `+0x78` | `operator>=(other)` | `0x009129E7` | 4 | AL |
| 31 | `+0x7C` | `operator==(other)` | `0x00912A08` | 4 | AL |
| 32 | `+0x80` | `operator!=(other)` | `0x00912A2B` | 4 | AL |
| 33 | `+0x84` | `operator-(int32 days)` | `0x00912A78` | 4 | EAX `n − days`; `n` unchanged |
| 34 | `+0x88` | `operator-(cIGZDate const&)` | `0x00912A4E` | 4 | EAX `n − other.n` |
| 35 | `+0x8C` | `operator+(int32 days)` | `0x00912A82` | 4 | EAX `n + days` |
| 36 | `+0x90` | `operator+(cIGZDate const&)` | `0x00912A65` | 4 | EAX `n + other.n` |
| 37 | `+0x94` | `operator++()` | `0x00912A8C` | 0 | `n := n + 1`; EAX this |
| 38 | `+0x98` | `operator--()` | `0x00912A99` | 0 | `n := n − 1`; EAX this |
| 39 | `+0x9C` | `operator+=(int32 days)` | `0x00912AA6` | 4 | `n := n + days`; EAX this |
| 40 | `+0xA0` | `operator-=(int32 days)` | `0x00912AB9` | 4 | `n := n − days`; EAX this |
| 41 | `+0xA4` | `DateString(cIGZString& out, uint32 format)` | `0x00913245` | 8 | §§1.6, 4.2; replacement through `out.Copy(temp)`; EAX = cleanup lock depth, normally 0 |
| 42 | `+0xA8` | `DayWithinMonth(m, d, y)` | `0x00913041` | 12 | §1.1; AL (does not use `n`) |
| 43 | `+0xAC` | `DaysInYear(y)` | `0x00913058` | 4 | 365 / 366 (does not use `n`) |
| 44 | `+0xB0` | `Jday(m, d, y)` | `0x00913065` | 12 | §1.2 (does not use `n`) |
| 45 | `+0xB4` | `LeapYear(y)` | `0x00912ACC` | 4 | §1.1; EAX 0 / 1 |

The arithmetic slots wrap modulo 2³² and never check validity: `--` on day 0 gives 2³² − 1.

### 4.1 Clone: signature, reference and failure contract (DT3)

At `0x00912E9F`, ECX is the source **cIGZDate interface pointer** (object base + 8). There is exactly one
32-bit stack argument: the address of a writable `cIGZDate*` output cell. The callee pops **4 bytes**
(`0x00912ED4`); there is no hidden return-object argument. This establishes the machine signature independently
of the SDK's `bool` declaration. The returned value is neither the output interface pointer nor a date value.

The function allocates **16 bytes** on the global heap (`0x00912EA0`–`0x00912EA4`), copy-constructs a fresh
object, adjusts its address by +8 to the interface, and overwrites the caller's output cell. Construction
obtains the source's day number through its virtual slot 26 (`0x00912715`–`0x00912723`), sets the two native
vtables, and initializes the new reference count to **0** (`0x00912E30`–`0x00912E41`). The source's reference
count is not copied, incremented, or released. After the output store at `0x00912ECC`, the new interface's
slot 1 is called (`0x00912ECE`–`0x00912ED0`). That slot increments the count and loads it into full EAX
(`0x00968B60`–`0x00968B66`): **EAX = 00000001, AL = 01** on normal return, regardless of incoming EAX.
No later instruction alters EAX. Thus a boolean caller sees true, but the complete register is also defined.

The caller owns **one reference** to the new allocation. Source and clone can be changed or released
independently; cloning day number 0 still produces an independently owned date with day number 0. One
`Release` on the fresh clone frees it and returns 0 (`0x00912912`–`0x00912928`, deleting destructor/free
`0x00912DD6`–`0x00912DF7`). Overwriting a non-null output cell does **not** release its former pointee.

There are no output-pointer or allocation-failure checks that return false:

| Setup | Native effect |
|---|---|
| Valid source, allocation succeeds, writable `out` containing a sentinel | Sentinel replaced by new allocation + 8; new reference count 1; EAX = 1. |
| Allocation succeeds, `out` is null | Allocation/construction occur first; the unconditional output store faults at `0x00912ECC`, before AddRef. No return value; no rollback frees the fresh allocation. |
| Allocator returns null, `out` is writable | The address adjustment explicitly preserves null (`0x00912EBB`–`0x00912EC6`); `*out` becomes null, then loading its vtable faults at `0x00912ECE`. No AddRef or normal return. |
| Allocator/copy construction does not return normally | No output store has yet occurred. No local handler translates the failure to false. |

### 4.2 DateString: copy, lifetime and return-register contract (DT3)

At `0x00913245`, ECX is the date interface pointer (object base + 8); the two 32-bit stack arguments are
the **address of the output cIGZString object** (a reference, not a pointer-to-pointer) and the format value.
The callee pops **8 bytes** (`0x00913289`). It constructs a local 20-byte `cRZString`, calls the base
formatter on the date at interface − 8, then invokes **output slot 14, offset +0x38**, with the local string
as its sole argument (`0x00913251`–`0x0091326E`). The local string has vtable `0x00A80810`; its slot 14 is
`cRZString::Copy` at `0x0090E20A`, as independently confirmed by the PE vtable data.

For an ordinary native `cRZString` output this **replaces** its contents. `Copy` reads the source length
and character pointer through slots 7 and 6, then calls the destination's `FromChar(pointer, length)` slot 3
(`0x0090E210`–`0x0090E227`); native `FromChar` uses range assignment, not append (`0x0090F546`–`0x0090F559`,
assignment `0x0044D3A0`). An existing `"PREFIX"` becomes `"1/2/2000"` for 2000-01-02, format 0; an empty
output gives the same result. The destination owns its resulting characters after the local string's buffer
is freed. A foreign string receives the same virtual Copy call; its own implementation determines mutation
and must not retain the borrowed local string or its buffer after this call returns.

**Full EAX is cleanup residue, not a success flag.** The Copy result is neither checked nor saved. The
wrapper next frees the local buffer through `0x0090CF63` if non-null (`0x00913271`–`0x00913287`), and returns
without assigning EAX or AL. The normal constructor allocates an 8-byte buffer and writes its first NUL
(`0x0057839C`–`0x005783BA`); successfully returning formatting preserves an allocated buffer. Therefore the
free branch is taken on ordinary returning calls. Its call chain is:

| Boundary | Effect on EAX |
|---|---|
| `0x0090CF63` → `0x005051C0` | Frees through the string allocator/lock object at `0x00B62698`; no independent return assignment. |
| `0x005051E6`–`0x00505201` | Both free routes finish by calling that object's slot 2 (+0x08); no later EAX assignment. |
| Native lock vtable `0x00AD1EF4`, slot 2 → `0x00930B6D` | Decrements its nesting counter at lock-state +0x18, saves the resulting 32-bit count, leaves the critical section, then explicitly returns the saved count in EAX (`0x00930B70`–`0x00930B81`). |

The allocator lock is initialized by `0x00A791B0` → `0x00930B91`, with nesting initially 0
(`0x00930AA8`); slot 1 at `0x00930B35` increments it on entry. Thus with no outer acquisition on the
calling thread, **successful DateString returns EAX = 0, AL = 0**. With an outer acquisition still held,
EAX is the remaining nesting count and AL its low byte. This is defined incidental register output of the
original cleanup path; it is not a date/string success convention and should not be inferred from SDK
`bool`. Were the local buffer null at cleanup, EAX would instead be whatever Copy left; the wrapper has no
explicit normalization on that branch either. No such branch is reached by the normal successfully
constructed/formatted local string. No boolean failure propagation or return-value guarantee exists for
arbitrary foreign string behavior that violates the Copy contract.

**Failure behavior.** The wrapper has no output-null check, checks no formatting or Copy result, and has no
local exception/fault recovery or output rollback. A null output faults when its vtable is read at
`0x00913268`, after temporary formatting. A foreign Copy that returns normally with AL = 0 and leaves the
output untouched is followed by the same temporary cleanup and normal EAX = 0; returning AL = 1 from Copy
does not force DateString to return 1. This applies equally to a foreign implementation that reports failure
after partially changing the output. If formatting/Copy throws or faults, there is no normal DateString
return and no wrapper promise of transactional mutation.

In particular, an allocator **returning null** for the local string's initial allocation leads to the NUL
store through null in its constructor (`0x005783B2`), before the output is accessed. Growth likewise uses
the returned allocation without a null-to-false conversion (`0x0040EC37`–`0x0040EC6B`). Destination assignment
can first overwrite an existing prefix before growing (`0x0044D403`–`0x0044D429`), so allocation failure
during Copy need not preserve the previous string. If the allocator instead does not return normally, the
wrapper supplies no additional recovery. These are failure-path rules, not assertions that allocation
failure was observed in a running game.

The export reports no **direct** callers of either COM wrapper; this does not exclude indirect interface
calls. Vtable `0x00ACEA38` independently identifies slots 9 and 41 at `0x00912E9F` and `0x00913245`.
The two game call sites cited in §1.6 call the **base** formatter directly, so they do not establish a
boolean return convention for the COM wrapper.

## 5. Who uses it

- The simulator's date (simulator.md §2.1, §7: `GetSimDate`) and the day number the agent messages carry.
- Records that store a day number (lot-records.md §2.1; the history warehouse; advisors' `g_date`).
- `cRZTime()` seeds several file-local random generators (crime.md §9, foundations.md §6.2).
- Text: the query tool's desirability readout and the configuration log, both with format 0 (§1.6).

## 6. Quirks

1. January and February of year 0 give meaningless day numbers (§1.2).
2. The "no date" 0 reads back as 28 February 2,935,093 (§1.3).
3. `Set(monthName, …)` keeps the last matching month and falls back to January (§1.5).
4. `Set(dayOfYear, y)` does not check the day; with year 0 it counts from 1901-01-01 (§1.5).
5. The `Set` overloads return the day number in EAX, so their SDK `bool` is the low byte (§3.2).
6. Daylight time follows the pre-2007 US rule, starts on 8 April when 1 April is a Sunday, and is applied whatever
   the host's zone (§2.2).
7. A local time in the spring-forward hour converts to 0 (§2.1).
8. `Release` frees the object when the count is 0 or 1 (§3.1).

## 7. Open questions

- **DT1:** are `DateString`'s format codes (§1.6) the language manager's language IDs? Only format 0 is used by
  the game, so the mapping matters only to plugins.

## 8. Conformance tests

1. **Day numbers:** `Jday(1, 1, 2000) = 2451545`, `Jday(1, 1, 1901) = 2415386`, `Jday(2, 29, 2000) = 2451604`,
   `Jday(12, 31, 1999) = 2451544`, `Jday(3, 1, 2000) = 2451605`, `Jday(10, 15, 1582) = 2299161`,
   `Jday(1, 1, 2100) = 2488070`, `Jday(3, 1, 0) = 1721120`, `Jday(1, 1, 0) = 1043250630`.
2. **Invalid dates give 0:** `Jday(2, 29, 1900)`, `Jday(13, 1, 2000)`, `Jday(0, 1, 2000)`, `Jday(2, 0, 2000)`.
3. **Round trip and fields:** for 2451604, `Mdy = (2, 29, 2000)`, `WeekDay = 2`, `DayOfYear = 60`; for 2451545,
   `WeekDay = 6`, `DayOfYear = 1`; for 2451544, `DayOfYear = 365`; `Mdy(0) = (2, 28, 2935093)`, `WeekDay(0) = 1`.
4. **Leap years:** 2000 and 2400 leap, 1900 and 2100 not, 0 leap; `DaysInYear(1999) = 365`, `(2000) = 366`.
5. **FirstDayOfMonth:** for a date in 2000, month 3 gives 61, month 2 gives 32, month 13 gives 0; in 1999, month 3
   gives 60.
6. **Previous:** from 2000-01-01 (a Saturday), `Previous(7)` = 1999-12-26, `Previous(6)` = 2000-01-01,
   `Previous(1)` = 1999-12-27.
7. **Set(dayOfYear, y):** `(0, 0)` = 1901-01-01, `(1, 0)` = 1901-01-02, `(60, 2000)` = 2000-02-29,
   `(366, 1999)` = 2000-01-01.
8. **Month names:** `Set("march", 1, 2000)` = 2000-03-01, `Set("MAY", 1, 2000)` = 2000-05-01,
   `Set("Smarch", 1, 2000)` = 2000-01-01.
9. **Text:** 2000-03-04 with format 0 is `3/4/2000`, format 2 `4/3/2000`, format 4 `4.3.2000`, format 7
   `4-3-2000`, format 9 `2000.3.4`, format 15 `2000/3/4`.
10. **COM object:** `Set(m, d, y)` for 2000-01-01 returns 2451545 in EAX; slot 33 with 1 returns 2451544 and leaves
    the date unchanged; slot 37 returns the interface pointer and makes the date 2451546; a date with day number 0
    reports `IsValid() = false`, `Month() = 2`; `QueryInterface(0x43378255)` succeeds, `QueryInterface(0x12345678)`
    fails.
11. **Daylight rule** (with `_daylight ≠ 0`): `BeginDST` falls on 2000-04-02, 1996-04-07 and 1990-04-08;
    `EndDST` on 2000-10-29 and 1990-10-28; with `_daylight = 0`, `IsDST` is always false.

### 8.1 DT2/DT3 discriminating native-contract vectors

These vectors distinguish the disputed contracts independently of an implementation. The outcomes below
are established by the cited instruction/data paths in §§1.5 and 4.1–4.2; **they have not been executed in
a game or native harness**. Fault cases describe the first relevant dereference, not a required host-process
test. Locale conditions refer to the executable's CRT; selecting a locale in an unrelated CRT is insufficient.

| Vector / setup | Expected observation |
|---|---|
| Month setter, day 1, year 2000, C `LC_CTYPE`: bytes `4D 61 00`, `00`, or `4D 00 61 79` | January 1, day number 2451545; no partial-prefix acceptance. |
| Same: bytes `6D 41 79 58` (`mAyX`), `4D 61 79 00 4A 75 6E` (`May` NUL `Jun`), or exactly three readable bytes `4D 61 79` ending at a readable-page boundary | May 1, day number 2451666; mixed case works and no fourth byte is read. |
| Same: bytes `4D 61 DD 00` or `4D 61 FD 00` | January 1 in the C branch; high bytes are not masked or folded as ASCII. |
| Month setter with null name; preexisting day number sentinel | Fault on the first comparison's name read; sentinel day number remains, rather than becoming January. |
| `mAy` with locale handle 0, then a nonzero `LC_CTYPE` handle | Same May result, but ASCII-helper dispatch versus per-byte locale-fold dispatch. No abbreviation contains `i`, so this month result alone cannot prove fixed-ASCII folding. |
| Discriminating helper-boundary probe: `_strnicmp("I", "i", 1)` with C locale, then Turkish `LC_CTYPE`/CP1254 whose lowercase mapping is `49` → `FD` | C helper gives 0; locale path gives `0xFD − 0x69 = 148`. This tests the called primitive's locale dependence; it is not a new date-format code or a claim about the game's session locale. |
| Clone: source day `0x12345678`, source refs 7; incoming EAX `0xA5A55A00`; writable `*out = 0xDEADBEEF` | Different allocation/interface; day preserved; source refs still 7, clone refs 1, sentinel overwritten; full EAX `00000001`. Former output pointee not released. |
| Release the source after cloning; modify/read clone; finally Release clone once | Clone remains independently usable until its own Release, which frees it and returns 0. Repeat with source day 0: cloning still succeeds. |
| Clone with null out, allocation succeeds; separately allocator returns null with writable sentinel output | First case faults at output store; second changes sentinel to null then faults loading the new vtable. Neither returns false. |
| DateString, date 2000-01-02, format 0; native output initially `"PREFIX"`, then initially empty | Both finish with owning output `"1/2/2000"`, valid after temporary destruction. Without an outer allocator lock acquisition, full EAX = 0 in both successful cases. |
| Foreign output slot 14 records its source and returns `0xA5A55A00` without mutation; repeat returning `0x5A5AA501` | Receives a temporary string containing `"1/2/2000"`; output remains unchanged; final ordinary EAX = 0 in both cases. Copy's low-byte failure/success sentinel is discarded. |
| Repeat DateString while the calling thread already holds the original string allocator lock once | On ordinary completion EAX = 1, the remaining depth; the same formatted text as above. Without that outer acquisition it is 0. |
| Null DateString output; separately initial temporary allocation returns null | Output-null faults at `0x00913268` after formatting; allocation-null faults at `0x005783B2` before touching output. No normal boolean return. |
| Destination native string has length 6 (`"PREFIX"`), capacity 8 including NUL; date text `"1/2/2000"`; its required growth allocation returns null | Assignment overwrites its first six bytes (`"1/2/20"`) before requesting growth, then faults using null in the growth path. The original string is not transactionally preserved; no false return. |
