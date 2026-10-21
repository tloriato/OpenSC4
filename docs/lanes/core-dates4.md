| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| core-dates4: dates and times core (cRZDate, cRZTime, cGZDate per docs/specs/dates.md) | `core-dates4` / (engine lane) | ready, no ABI change; core `61f55348`, conformance `4e0c2e5c`; gate cargo 3827/0, clippy 0, ctest 63/63; no game runs (core lane, no glue); DT2-DT3 filed | 1. coordinator: coverage row `dates` core path `core/src/dates` + `conformance/vectors/dt8_*`; 2. a glue lane: cGZDate COM wrapper over `fc4_core::dates::GzDate` and a shadow of GetSimDate's date |

## Report

What changed (core only; no plugin / CAPI / CMake / ABI / tools edits):
- `engine/core/src/dates/mod.rs`: §1 calendar in exact unsigned 32-bit wrapping arithmetic: `leap_year`,
  `days_in_year`, `day_within_month`, `jday` (0 when invalid, year-0 Jan/Feb wrap quirk), `mdy` (Mdy(0) =
  2/28/2935093 quirk), `weekday`, `month_from_name` (last match wins, January fallback), `date_string` (all format
  families, `%4u` padding), and `RzDate` with every query and setter (Previous, Between, CompareTo, Max/Min,
  FirstDayOfMonth, the five Set overloads, own-vtable postfix ++/--). `cRZDate()` "today" takes host input
  (`LocalTm` or a `WallClock`).
- `engine/core/src/dates/time.rs`: §2 `RzTime` (from_local with the gap-hour-gives-0 quirk, now, LocalSecs, Hour,
  Minute, date), `begin_dst` / `end_dst` / `is_dst` with the library's fixed pre-2007 US rule (1974/1975 special
  starts, 8 April when 1 April is a Sunday). The CRT `_timezone` / `_daylight` are a typed `TimeZone`; no host DST
  rule is assumed.
- `engine/core/src/dates/com.rs`: §3/§4 `GzDate`, one method per all 46 `cIGZDate` slots with the original return
  values (Set/Copy return the day number, slots 33-36 day numbers without mutation, 37-40 in place),
  QueryInterface (IIDs 1 and 0x43378255, AddRef), Release freeing below 2 (`ReleaseResult.freed`), Clone with
  count 1; out dates through the `DateSink` trait (`out.Copy`). Nothing can panic on any input (tested at
  0, 1721119, 2^32-1, i32::MIN time zones).
- `engine/conformance/src/dates.rs` + README "Dates and times ops" / "Coverage (dates §8)": ops `dt_fn`, `dt_new`,
  `dt_call`, `dt_expect`; vectors `dt8_01_day_numbers`, `dt8_02_setters_text`, `dt8_03_com_object` (every slot),
  `dt8_04_daylight_time` (incl. crime.md §9 worked example 3969450000).
- 26 core unit tests (all of §8, every day of years 1-3000 round-tripped, DST boundaries, spring-forward hour,
  fixed-clock today/now).

Measured: baseline gate cargo 3803/0, clippy 0, ctest 63/63; final gate cargo 3827/0, clippy 0, ctest 63/63, fmt
clean on touched files. All 4 dt8 vectors pass.

Questions filed (docs/specs/questions/dates.md): DT2 (month names shorter than 3 characters, null pointer, locale
folding; implemented as C-locale strnicmp), DT3 (return values of Clone and DateString; implemented as true).

Not done / next: no game run (task forbids plugin edits, so there is no shadow to compare); a glue lane should wrap
`GzDate` as the plugin's cIGZDate (EAX day number for the Set overloads, §3.2) and shadow the simulator's date.
`engine/core/src/julian.rs` (signed simulator helper) is untouched; it could later delegate to `dates`.
