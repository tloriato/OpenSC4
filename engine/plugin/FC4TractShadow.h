// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the Free
// Software Foundation, either version 3 of the License, or (at your option)
// any later version. This program is distributed WITHOUT ANY WARRANTY; see
// the GNU General Public License for more details. You should have received a
// copy of the license along with this program (see LICENSE at the repository
// root); if not, see <https://www.gnu.org/licenses/>.

// The tract developer shadow: a same-process differential of the tract
// developer (docs/specs/tract-developer.md; tooling, no spec behaviour).
// Opt-in, off by default:
//
//   tract_shadow = 1             (env FC4_TRACT_SHADOW)
//
// It only reads the city's tract developer. Two modes, decided at game start
// by whether `enable = tract` put FreeCity4's class where the original's was
// (FC4TractShadow.cpp TractShadowStartup):
//
//   differential (the default, `tract` not enabled): the reference is the
//   ORIGINAL tract developer and the private core is on its record.
//
//   self-check (`enable = tract`): the original is no longer reachable
//   through the city, so the reference is FreeCity4's own live class and the
//   private core is on that class's record. Both sides are ours, so a
//   difference is a defect in the replacement (a record that does not round
//   trip, a getter that disagrees with the core it was built from) rather
//   than a difference from the original. The slot 7 forwarder is only ever
//   installed over the original's code, so UpdatePopulation is compared on
//   sample tracts only. This is the shape glue21 chose for the prop and lot
//   configuration managers. In both modes the shadow lines name the live
//   class "orig" and the private core "ours", so the line format is unchanged.
//
// At PostCityInit ("load") and at every SimNewMonth ("month"), on the main
// thread:
//   1. the live class's style state through cISC4TractDeveloper slots 8, 9, 11
//      and 13 (never a setter);
//   2. FreeCity4's core in shadow mode (fc4_tract_create_shadow) reads the
//      reference's own record (its cIGZSerializable::Write into memory, §6),
//      over a read-only host table on the city's borrowed services, then
//      ReadTuning (§4.21, the fix-ups Init makes). Its Write must give the
//      same bytes back and its getters must match the reference's;
//   3. UpdatePopulation (§4.18), at every call. On the identified executable,
//      when the original tract developer's vtable slot 7 holds the original
//      UpdatePopulation (0x00706BC0), that entry is pointed at a forwarder in
//      this DLL at the first city load (same this, argument and EAX): per
//      call the core computes the populations first (fc4_tract_shadow_update_
//      population, nothing written), then the original runs, then each lot's
//      population (lot slot 53 GetPopulation(devType)) is compared with
//      ours. The entry is put back at PreAppShutdown and DLL detach. One
//      "calls" line per month. Without the forwarder (another executable),
//      a month-end sample instead: for every growth developer of the BDS
//      (GetDeveloperOf(purpose 1-8, wealth 1-3), slot 33) and up to
//      kSampleTracts tracts holding lots, the populations the core would set
//      now against what the lots hold; that sample drifts (the original set
//      them at its last call, with that day's desirability and capacities).
//      Tract (0, 0) is never compared (§4.18 [quirk]: the original seeds it
//      from the clock).
//
// Not compared (logged at game start): Grow and Decline (they change the
// city and draw from the generators; a read-only rerun cannot reproduce
// them), the histogram, the filter map. They are tallied instead: slots 5
// (Grow) and 6 (Decline) get counting forwarders next to slot 7's, and one
// "tract tally (original)" line per month gives, per growth developer, the
// calls, the capacity returned and the outcome codes appended. FreeCity4's
// developer (enable = tract) writes the same line as "tract tally (ours)",
// so a replace run and a shadow run compare month by month.
//
// Log lines (always written, not the verbose channel), per run:
//   tract shadow (styles) YYYY-MM-DD load|month: record=N bytes roundtrip=same|DIFFERENT current=0x... index=...
//     styles=... all=... years=... mismatches=M first=...
//   tract shadow (calls) YYYY-MM-DD month: calls=N compared=C origin=O failures=F lots=L mismatches=M first=...
//   tract shadow (population) YYYY-MM-DD load|month: developers=D tracts=T lots=L mismatches=M first=...
//     (the sample; only without the forwarder)
//   tract tally (original|ours) YYYY-MM-DD month: grows=G built=B codes=A successes=S declines=D declined=X
//     growMs=T unlisted=U devs=0xSUBTYPE:gG/bB/dD/xX/tAVG,MAX/code:count,... (ascending subtype; the
//     Grow wall times in microseconds, the month's total in milliseconds)
// A run that cannot compare logs "... INCOMPLETE (why)" instead.
#pragma once

#include "FC4DevelopmentInterfaces.h"
#include "FC4GameList.h"
#include "fc4.h"

#include <cstddef>
#include <cstdint>

class cISC4City;

namespace fc4 {

// The Grow / Decline tally (both developers; any thread, locked). Count the
// caller's outcome list before the call, then pass that count with the
// list after it: the codes past it are counted.
size_t TractTallyListSize(GameList<int32_t>& outcomes);
// QueryPerformanceCounter; `ticks` is the call's wall time in those units.
uint64_t TractTallyNow();
void TractTallyGrow(const FC4GrowthProfile& profile, int32_t built, GameList<int32_t>& outcomes, size_t before,
                    uint64_t ticks);
void TractTallyDecline(const FC4GrowthProfile& profile, int32_t declined);
// One "tract tally (who)" line with the counts since the last one.
void TractTallyMonth(const char* who, int32_t julian);

// Game start (after the subsystems are resolved): reads the setting and logs
// whether the shadow can run in this configuration.
void TractShadowStartup(bool tractReplaced);

// FC4Observer.cpp at PostCityInit: one run. No-op unless tract_shadow = 1.
void TractShadowCityLoaded(cISC4City* city);

// FC4Observer.cpp at every SimNewMonth: one run (and the forwarder's "calls"
// line). No-op unless tract_shadow = 1.
void TractShadowMonth(cISC4City* city, int32_t julian);

// FC4Observer.cpp at PreCityShutdown: the forwarder's core goes with the city.
void TractShadowCityShutdown();

// cFC4Director PreAppShutdown / dllmain DLL_PROCESS_DETACH: the original
// UpdatePopulation goes back into the vtable (no-op when not installed).
void TractShadowStop();
void TractShadowDetach();

// Test seam (op 45, kFC4TractTestShadow): one run whatever the setting.
// `event` 0 = load, 1 = month. With `testHost` set, its non-NULL callbacks
// answer the shadow core's host calls instead of the game. Returns the run's
// mismatches (both lines), -1 without a city or tract developer, -2 when the
// tract developer is FreeCity4's (nothing compared), -3 when a line was
// INCOMPLETE.
intptr_t TractShadowTestRun(cISC4City* city, int32_t julian, uint32_t event, const fc4_tract_host* testHost);

}  // namespace fc4
