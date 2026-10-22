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

// Scripted actions (tooling; the harness lane; no spec behaviour of its own).
// `script = <path>` (FreeCity4.ini, else FC4_SCRIPT; a relative path is
// relative to the folder of FreeCity4.ini, the DLL's) names a text file of
// dated actions, one per line, `YYYY-MM-DD <action> <args>`, `#` comments
// (the grammar: FC4HarnessLogic.h, ParseScript). The whole file is parsed at
// game start; every parse error is logged with its line number and a script
// with any error is refused (no action of it runs). Each action runs once, on
// the main thread, inside the observer's SimNewDay at the first new day on or
// after its date; several on one date run in file order. A `save` is the
// exception: it waits for the observer's next framework tick ([choice], as
// the save autotest: not inside a simulator message), and every action due
// after it waits behind it, so the file order holds (under turbo several days
// can pass before that tick; the log shows "(run on <date>)"). Actions, all
// through the game's interfaces (ours or the original's, whichever the city
// has):
//   zone x0 z0 x1 z1 type      cISC4ZoneManager::PlaceZone (city slot 36, zone
//                              manager slot 11, zones.md §6): commit = 1, the
//                              normal funds rules; the region in the §11.4
//                              layout in our own memory (§13.2: the game only
//                              clears the bits of rejected cells)
//   pricezone x0 z0 x1 z1 type the same with commit = 0 (a dry run)
//   dezone x0 z0 x1 z1         the same with type 0, commit = 1
//   speed 0|1|2                cISC4Simulator::SetSimSpeed (city slot 50, 28)
//   pause | resume             cISC4Simulator::Pause (9) / Resume (12), one call
//   turbo on|off               our simulator's turbo clock (FC4Turbo.h)
//   digest                     the observer's DIGEST line, now
//   mark <text>                a log line
//   dispatch type              real station: begin tool, dispatch, record/shadow,
//                              revoke, end; INCOMPLETE unless a vehicle dispatched
//   save [fast]                cISC4App::SaveCity(fast) (docs/abi/vtables.tsv
//                              cSC4App::vftable_3 slot 16, pop 4): the city
//                              saved in place, a full save unless `fast`;
//                              logs `-> ok=<0|1> (SaveCity(full|fast) in N ms)`
//   ordinance hex-ID on|available|enabled 0|1   real city ordinance setter (explicit fixture action)
//   terrain x0 z0 x1 z1 h [kind]  cISTETerrain::SetAltitude(rect, h, true, kind)
//                              (city slot 84, terrain slot 49, terrain.md §6): the
//                              inclusive VERTEX rectangle set to h metres, its
//                              water reset, RedisplayTerrain(1, 1, rect, kind)
//                              (kind default 1); a rectangle outside the vertices
//                              is skipped; with terrain_shadow the shadow mirrors
//                              the call (FC4TerrainShadow.h)
// Log: `script: line N <date> <action> <args> -> <result>`. Off (no key),
// nothing is read and nothing runs. Not available: `save_as <name>` (saving
// under another path; engine/README.md says why).
#pragma once

#include <cstdint>

namespace fc4 {

// Game start (FC4Subsystems.cpp): reads `script`, loads and parses the file.
void ScriptReload();
// Loads the script at `path` (relative: to FreeCity4.ini's folder) in place
// of the current one. Returns the number of actions it will run, -1 when the
// file cannot be read, -2 when it is refused (parse errors). `why` is logged.
int ScriptLoad(const char* path, const char* why);
// The observer's SimNewDay (main thread): runs the actions due (a `save`
// and the actions after it wait for ScriptOnTick).
void ScriptOnNewDay(int32_t julian);
// The observer's OnTick (main thread): runs the waiting actions, in order.
void ScriptOnTick();
// True while actions wait for the next tick (the autotest's end date waits
// for them: FC4Autotest.cpp).
bool ScriptWaiting();

}  // namespace fc4
