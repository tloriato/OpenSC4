# Spec: timers (`cRZTimer`, `cRZCallbackTimer`)

Status: **behaviour spec**, clean-room input. This document describes *behaviour*, not code. It was written by the
analysis team from the Windows 1.1.641 executable (read in the instruction bytes and in a Ghidra 12.1.4 project of
the Steam build, whose code equals the GOG build's; evidence in `docs/lanes/re-dates.md`). Implementers must work
from this spec, the public LGPL gzcom-dll SDK headers and general knowledge only. They must **not** consult decompiled
or disassembled code (see [CONTRIBUTING.md](../../CONTRIBUTING.md)).

Normative words: **must**, **should**, **may**. Tagged notes: *[compat]* is required for compatibility with the
original game and its plugins; *[choice]* is a behaviour we chose where the original was arbitrary or undefined;
*[inferred]* is consistent with all evidence but not proven; *[quirk]* is surprising original behaviour that is
kept on purpose; *[compat-limit]* depends on something FreeCity4 cannot reproduce (here the host's clocks), so only
the rule is matched, not each value.

Related specs: [clock24.md](clock24.md) (§4.3, the 24-hour clock's timer model and CK2/CK15, which this spec
extends), [foundations.md](foundations.md) (§18, the x87 precision the conversions run at),
[simulator.md](simulator.md) (§5, the slice budget).

## 0. Summary

`cRZTimer` is a stopwatch: an accumulated 64-bit tick count plus a start stamp, read in one of seven units.
`cRZCallbackTimer` calls a function after a delay, once or repeatedly, through a framework timer service. Neither
is saved. All their values come from the host's clocks *[compat-limit]*; what must match is the arithmetic below.

## 1. `cRZTimer` *[compat]*

### 1.1 State (24 bytes, usually embedded in another object)

The constructor `cRZTimer(unit)` (`0x0088FEDF`, 98 call sites) stores the unit and zeroes the rest.

| Offset | Field | Initial |
|---|---|---|
| `+0x00` | `unit` (int32, §1.2) | the constructor's argument (`SetUnits` changes it) |
| `+0x08` | `start`: the raw counter value when it was started (u64); **0 means stopped** | 0 |
| `+0x10` | `accumulated` raw ticks (u64) | 0 |

### 1.2 Units

| `unit` | Counter read | Reported as |
|---|---|---|
| 0 | the CPU time-stamp counter (`cpuid` then `rdtsc`) | raw cycles |
| 1 | `QueryPerformanceCounter` | raw ticks |
| 2 | `QueryPerformanceCounter` | nanoseconds |
| 3 | `QueryPerformanceCounter` | microseconds (the 24-hour clock's unit, clock24.md CK2) |
| 4 | `QueryPerformanceCounter` | milliseconds |
| 5 | `QueryPerformanceCounter` | seconds |
| 6 | `QueryPerformanceCounter` | minutes |
| other | as unit 1 | always 0 |

Units 2–6 convert raw ticks with a float factor `ticks × f[unit]`, where `f` is
`1e9 / F`, `1e6 / F`, `1000 / F`, `1 / F` and `(1/60) / F` for `F` the performance-counter frequency (§1.5).

### 1.3 Methods

| Method | Address | Behaviour |
|---|---|---|
| `SetUnits(u)` | `0x00890181` | `unit := u`; `start := 0`; `accumulated := 0` |
| `Reset()` | `0x0088FF0A` | `start := 0`; `accumulated := 0` (unit kept) |
| `IsTimerRunning()` | `0x0088FEFB` | `start ≠ 0` (EAX 0 / 1) |
| `Start()` | `0x008905C4` | `start :=` now (the unit's counter, §1.2). It does not test whether it already runs: a running timer loses its open interval (clock24.md §4.3) |
| `Stop()` | `0x008905F0` | if running: `accumulated += interval(now − start)`, `start := 0`. **Now is always read with `QueryPerformanceCounter`**, also for unit 0, whose `start` came from `rdtsc` *[quirk]* |
| `Restart()` | `0x0089058F` | `start :=` now; `accumulated := 0`, running or not (clock24.md CK15) |
| `GetElapsedTime()` → u32 | `0x00890198` | `t := accumulated`, plus `interval(now − start)` when running; units 0–1: the low 32 bits of `t`; units 2–6: `trunc(t × f[unit])` as a 32-bit integer; other units: 0 |
| `GetElapsedTime(uint64& out)` | `0x00890279` | the same `t`; units 0–1: all 64 bits; units 2–6: `trunc(t × f[unit])` in the low half, 0 in the high half; other: 0 |
| `GetElapsedTime(float& out)` | `0x00890377` | the same `t` as a float; units 2–6 multiplied by `f[unit]`; other units 0.0 |
| `SetElapsedTime(uint64 v)` | `0x0089045A` | units 0–1: `accumulated := v`; units 2–6: `accumulated := trunc(v × g[unit])` (`v` loaded as a signed 64-bit integer); other units: unchanged. Then `start :=` now, so the timer runs |
| `SetElapsedTime(float v)` | `0x00890501` | units 0–1: `accumulated := trunc(v)`; units 2–6: `trunc(v × g[unit])`; other units: unchanged. Then `start :=` now |
| `Delay(n)` | `0x0089064D` | a temporary timer of the same unit is started; until its `GetElapsedTime()` reaches `n`, `Sleep(0)` |
| `GetTimerCycle(out)` | `0x0088FF19` | writes `QueryPerformanceCounter` into `out` |

`interval(d)` is the overhead rule: with `o` the calibrated call overhead (§1.5), `d − o` when `d > o`, else **1**
(at least one tick per interval; clock24.md §4.3). Unit 0 uses the same `o`, measured on the performance counter
*[quirk]*.

`g[unit]` is the stored reciprocal `1 / f[unit]` (§1.5). `trunc` is the CRT's float-to-integer truncation toward
zero to 64 bits, `_ftol2` (`0x009EEF04`, which other specs call `_ftol`); the 32-bit `GetElapsedTime` returns its
low half. The products run on the x87 unit at the caller's precision (24-bit on the game's main thread,
foundations.md §18), so a value near an integer can truncate one lower, or one higher, than exact arithmetic
*[compat-limit]*.

### 1.4 Static factor getters

Six functions return one stored float on the x87 stack (each pops 0): nanoseconds per counter tick `0x0088FF24`,
microseconds `0x0088FF2B`, milliseconds `0x0088FF32`, seconds `0x0088FF39`, minutes `0x0088FF40`, and
milliseconds per **CPU cycle** `0x0088FF47`. The command-system support (time-stamped output) and the glyph-texture
code call them.

### 1.5 Calibration at program start (`0x0088FF4E`, from the static initialisers)

1. Raise the process to real-time priority class and the thread to time-critical, measure the two frequencies
   (`0x0088FDC0`), then restore both priorities.
2. `0x0088FDC0(&F, &C)` (as clock24.md §4.3 describes): up to five times, read `QueryPerformanceCounter` and the
   cycle counter; if the ratio of the two **raw values** (cycles / ticks) is within (0.98, 1.02), the performance
   counter is the cycle counter and `C := F`. Otherwise `C` is measured: `SleepEx(100, alertable)` repeated until at
   least 0.33 s of performance-counter time has passed, then `C := trunc(Δcycles / Δseconds)`. `F` is always
   `QueryPerformanceFrequency` *[compat-limit]*.
3. Store the five counter factors of §1.2 and their reciprocals, then the same five for cycles (`1e9 / C` …
   `(1/60) / C`) and their reciprocals, as floats.
4. The overhead `o` is the **smallest** difference of two back-to-back `QueryPerformanceCounter` calls over 8 tries
   (stored as u64); a cycle overhead (8 `cpuid`+`rdtsc` pairs) is stored too but no `cRZTimer` method reads it.

*[choice]* FreeCity4 may take `F` from the OS and `C` from any estimate, and may measure `o` the same way; tests
must inject `F`, `C` and `o`.

## 2. `cRZCallbackTimer` *[compat]*

A heap object (vtable `0x00AD1F14`) that asks a timer service to call it back.

### 2.1 State

| Offset | Field | Initial |
|---|---|---|
| `+0x04` | uses the framework service (bool) | true after construction (§2.3) |
| `+0x08` / `+0x0C` | callback function / its argument | 0 / 0 |
| `+0x10` | `delay` (u32) | 100 |
| `+0x14` | `period` (u32) | 1000 |
| `+0x18` | running (bool) | false |
| `+0x19` | one-shot (bool) | false |
| `+0x1C` | the service's timer ID | 0 |
| `+0x20` | the timer service (counted reference, or the built-in default `0x00B62AB0`) | default |
| `+0x24` | an optional counted object kept alive while the timer runs | null |

Every live callback timer is in a global set (`0x00B62A9C`) guarded by a critical section (`0x00B62AA8`); the
constructor adds it, the destructor removes it.

### 2.2 Methods (vtable `0x00AD1F14`)

| Slot | Method | Address | Behaviour |
|---|---|---|---|
| 0 | deleting destructor | `0x00932C9C` | destructor (`0x00932AFE`: leave the set, `Stop` if running, release `+0x24` and the service), free with flag bit 0 |
| 1 | `SetDelay(v)` | `0x0093294A` | `delay := v`; a running timer is restarted (below); true |
| 2 | `GetDelay(out)` | `0x009329A0` (shared getter) | `out := delay`; true |
| 3 | `SetPeriod(v)` | `0x009329AE` | `period := v`; a running timer is restarted (below); true |
| 4 | `GetPeriod(out)` | `0x00932A04` (shared getter) | `out := period`; true |
| 5 | `Start(period, object)` | `0x00932CB8` | refused (false) when running or when no callback is set. Keeps `object` (AddRef) in `+0x24`; a non-zero `period` replaces `period`; running := true; `ID := service.AddTimer(handler, this, period, 0, 0)` (service slot 3). An ID of 0 undoes it (not running, object released) and returns false; else true |
| 6 | `StartOnce(delay, object)` | `0x00932D21` | as slot 5 but one-shot: the delay is the argument, or `period` when it is 0; one-shot := true; the service is called with 1 in place of the first 0 |
| 7 | `Stop()` | `0x00932A12` | if it has an ID: `service.RemoveTimer(ID)` (service slot 4), release `+0x24`; ID, running and one-shot := 0. True |
| 8 | `IsRunning()` | `0x00932B83` (shared) | `+0x18` |
| 9, 10 | `SetCallback(fn, arg)` | `0x00932A4A` (shared) | `+0x08 := fn`, `+0x0C := arg`; true |
| 11 | `UseTimerService(bool)` | `0x00932A5D` | on a change: true looks up the framework system service `0xA76C78E7` (interface `0x276C784B`) and keeps it; if there is none, the flag stays false and the default is kept (returns false); false switches to the default object |
| 12 | `GetTimerService()` | `0x009D8564` (shared getter) | `+0x20` |

**Restart** (slots 1 and 3): keep a reference on `+0x24`, call `Stop` (slot 7), then call slot 5 or 6 by the
one-shot flag with `(0, null)`, and put the kept object back in `+0x24`. `Stop` has already cleared the one-shot
flag, so the restart is **always slot 5**: a running one-shot timer becomes periodic with `period` *[quirk]*.

`delay` (`+0x10`) is stored, reported and restarts the timer, but neither start passes it to the service: it has no
other effect in this class *[quirk]*.

### 2.3 Construction (`0x00932C23`)

Sets the initial values of §2.1, joins the global set, takes the default service, then calls slot 11 with true:
with a framework timer service present it uses that one.

### 2.4 When the service fires (handler `0x00932B87`)

Under the global lock: if the timer is no longer in the set (destroyed), nothing happens. Otherwise read the
callback and its argument, take a reference on `+0x24`, and for a **one-shot** timer clear it (release `+0x24`,
ID := 0, running := false, one-shot := false). Leave the lock, then call `fn(arg)` (cdecl, one argument) if `fn` is
set, then release the reference taken. A periodic timer stays registered and keeps firing until `Stop`.

How often the service calls back, and on which thread, is the service's behaviour (TM1).

## 3. Users

- `cRZTimer` (set up with `SetUnits`): the 24-hour clock and the simulator's slice budget (clock24.md), the graphics
  system, the traffic simulator, the disaster layer and the riot and fire instances, the automata script system,
  the input-control dispatchers and the drive control, the region-view clouds. The command-system support and the
  glyph-texture manager read the factor getters (§1.4).
- `cRZCallbackTimer` (constructed inside its owner): the text-edit window, the scrollbar, the tooltip managers,
  the news ticker, the splash screen, the catalog view, the tertiary menu frame, the effects manager, the query tool
  (its coverage-ring callback) and the network stream socket's `Connect`.

## 4. Quirks

1. `Stop` reads the performance counter even for a unit-0 (cycle) timer (§1.3).
2. Unit 0 uses the performance counter's call overhead (§1.3).
3. Every interval counts at least one tick (§1.3).
4. `Start` on a running timer drops the open interval (§1.3).
5. Conversions truncate at the caller's x87 precision (§1.3).
6. `cRZCallbackTimer`'s `delay` is never passed to the service (§2.2).
7. `SetDelay` or `SetPeriod` on a running one-shot timer makes it periodic (§2.2).

## 5. Open questions

- **TM1:** the framework timer service (`0xA76C78E7` / `0x276C784B`) and the built-in default object at
  `0x00B62AB0` (constructed by `0x009335F4`): their callback thread and the meaning of `AddTimer`'s last two
  arguments (the one-shot flag and the always-0 fifth).

## 6. Conformance tests

With an injected counter of frequency `F = 1,048,576` ticks/s (2²⁰: every factor but the minutes one is exact in a
float, so the results do not depend on the x87 precision) and overhead `o = 3`:

1. **Units:** a timer started at tick 1000 and read at 1,049,579 (no stop) has `t = 1,048,576`; it reports
   1,048,576 ticks (unit 1), 1,000,000,000 ns (unit 2), 1,000,000 µs (unit 3), 1000 ms (unit 4), 1 s (unit 5) and
   0 min (unit 6, truncated).
2. **Overhead:** start at 100, stop at 102 (difference 2 ≤ `o`): `accumulated = 1`; start at 100, stop at 103:
   `accumulated = 1`; start at 100, stop at 200: `accumulated = 97`.
3. **Accumulation:** two start/stop pairs of 1003 ticks each give 2000 ticks; `Reset` gives 0 and stopped;
   `Restart` on a stopped timer starts it with 0.
4. **SetElapsedTime:** unit 5, `SetElapsedTime(uint64 2)` gives `accumulated = 2,097,152` ticks and a running timer;
   unit 9, `SetElapsedTime(7)` leaves `accumulated` unchanged and starts the timer.
5. **Unknown unit:** unit 9 reports 0 from all three `GetElapsedTime` forms while counting like unit 1.
6. **Callback timer:** with a scripted service, `Start(0, null)` without a callback returns false; with a callback
   it registers `(handler, timer, 1000, 0, 0)`; `StartOnce(250, obj)` registers `(…, 250, 1, 0)`, and one firing
   calls the callback once and leaves the timer stopped; `Stop` removes the ID; `SetPeriod(50)` on a running
   periodic timer re-registers it with `(…, 50, 0, 0)`, and on a running one-shot timer too (it is then periodic);
   the kept object survives the restart with one reference.
