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

#pragma once

#include <cstdint>
#include <cstddef>

// The fixed schedule (docs/specs/simulator.md §5.8) for the original game:
// `schedule = fixed` in FreeCity4.ini (tooling; default off) holds its work to
// rules that do not depend on the machine, so the same inputs give the same
// results. In-memory hooks of the 1.1.641 executable only; nothing happens with
// any other executable.

namespace fc4 {

// Every fixed-schedule patch or return-address key, and its enclosing exe function.
// A zero function denotes an import slot, vtable slot or other global data.
struct ScheduleSite { uint32_t site; uint32_t function; };
const ScheduleSite* ScheduleSites(size_t* count);
inline uint32_t MissingScheduleSite(const ScheduleSite* keys, size_t keyCount, uint32_t function,
                                    const uint32_t* honoured, size_t honouredCount) {
    for (size_t i = 0; i < keyCount; ++i) {
        if (keys[i].function == 0 || keys[i].function != function) continue;
        bool found = false;
        for (size_t j = 0; j < honouredCount; ++j) found = found || honoured[j] == keys[i].site;
        if (!found) return keys[i].site;
    }
    return 0;
}
// Resolve fc4r_<site> exports once, before installing any native entry jumps.
void ScheduleNativeModule(void* module);
// First site lost by replacing this function without a declaration (zero means none).
uint32_t ScheduleMissingSite(uint32_t function, const uint32_t* sites, size_t count);

bool ScheduleFixed();               // `schedule = fixed` and the executable is the supported one
void ScheduleStartup();             // install the hooks; one log line per lever
void ScheduleBeforeCityLoad();      // re-seed the file-local generators again, set rand()
void ScheduleDayOpened(int32_t julian);
void ScheduleFrame();               // one simulator frame (cFC4Simulator::OnTick, fixed step)
uint32_t ScheduleClockMs();         // the main thread's virtual clock in ms since the load (0 before it)
// QueryPerformanceCounter as the original's hooked reads see it: our cores call this where the original reads the
// counter (a stopwatch), so the virtual counter advances by the same ticks (one per read on the main thread when fixed).
int64_t ScheduleQpc();
// One digest line (the rand() state the delivery's predecessors left, a hash of
// the 34 generators, the target's class), then the rand() rule. Main thread only.
void ScheduleBeforeDelivery(const void* target, uint32_t msgType);
void ScheduleMonthLine(int32_t julian);  // counters, once a month
void ScheduleStampsRestart();       // [diagnostic] after `heap perturb`'s blocks: the next block gets stamp 1 again

}  // namespace fc4
