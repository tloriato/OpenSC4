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

// The MySims shadow: a same-process differential of the MySims (mysims.md;
// tooling, no spec behaviour). Opt-in, off by default:
//
//   mysims_shadow = 1              (env FC4_MYSIMS_SHADOW)
//
// It needs the ORIGINAL classes (the default: no `enable` key makes any of
// them ours). At PostCityInit ("load") and at every SimNewMonth ("month"), on
// the main thread:
//   1. every MySim of the original agent (advisor IDs 1-5, 30, via the city's
//      advisor system, mysims.md §1.3) is captured read-only: its own record
//      (its cIGZSerializable::Write, version 17, §6.2) and its getters;
//   2. the records are read into a shadow core (FreeCity4's advisor system
//      with MySims, fc4_advisors) through a custom stream whose references
//      become core objects (advices, subjects) or the game tokens the
//      original holds (buildings, dispatches);
//   3. the shadow core then follows the original: the ticker (the daily and
//      monthly passes, advisors.md §5.1) and the agent's messages (mysims.md
//      §3.2) are forwarded, so both sides take the same steps;
//   4. the compared state: per MySim the decoded saved fields of the original
//      record against the shadow core's (fc4_mysim_get_state), the advisor
//      part (personal name, month counter, day, cursor, advice count), the
//      agent's slots and the getters (GetActiveMySims,
//      MySimLivesHere, GetActiveUIPanelSim).
//
// No Lua in the shadow (create_table false): the original's advices carry
// their triggers as text and activate through the original's Lua state; the
// shadow's are plain records whose trigger never runs (compareTrigger None),
// so nothing activates on either side of the comparison. The trip variables,
// the pylon and the dispatch tool are UI/automata state the agent's idle
// MySims do not change; they are not compared and named on the lines.
//
// Log lines (always written):
//   mysims shadow: ON (...)                                   game start
//   mysims shadow YYYY-MM-DD load: sims=N ... mismatches=N    city load
//   mysims shadow YYYY-MM-DD month: sims=N ... mismatches=N   each month
//   mysims shadow (cont.) ...                                 details
#pragma once

#include <cstdint>

class cISC4City;

namespace fc4 {

// Game start (after the subsystems are resolved): reads the setting and logs
// whether the shadow can run in this configuration.
void MySimsShadowStartup();

// PostAppInit / PreAppShutdown (cFC4Director.cpp): the shadow's message
// target. No-ops unless mysims_shadow = 1.
void MySimsShadowStart();
void MySimsShadowStop();

// PostCityInit (FC4Observer.cpp): captures the original's MySims into the
// shadow core and logs the load line.
void MySimsShadowCityLoaded(cISC4City* city);

// SimNewMonth (FC4Observer.cpp): the month comparison.
void MySimsShadowMonth(cISC4City* city, int32_t julian);

// PreCityShutdown (FC4Observer.cpp): the core is dropped.
void MySimsShadowCityShutdown();

// Test seam op 68 (FC4TestSeam.h).
struct FC4MySimsShadowTestInput {
    uint32_t struct_size;  // sizeof(FC4MySimsShadowTestInput)
    uint32_t event;        // 0 = one month compare, 1 = a state dump
    int32_t julian;
    // 1 (out): created, had_mysims, months, clean_months, last_mismatches,
    // the agent's slots (out), the last sims compared.
    uint32_t created, had_mysims, months, clean_months, last_mismatches, last_sims, compared;
    int32_t slots[6];
};
intptr_t MySimsShadowTest(FC4MySimsShadowTestInput* in);  // fc4::

}  // namespace fc4
