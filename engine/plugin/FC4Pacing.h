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

// Deterministic pacing (docs/specs/simulator.md §5.7 [choice]; fc4.h "pacing";
// core fc4_core::pacing). FreeCity4.ini:
//   pacing = deterministic | original   (env FC4_PACING)
// The default is deterministic, except when a conformance shadow is on
// (`<x>_shadow` set and not 0) and `pacing` is unset: then original, so the
// shadows compare our cores with the original's under the original's pacing.
// Read once, at game start (PacingStartup), which also sets the cores' process
// mode (fc4_pacing_set_process) before any core exists and logs one line.
//
// Under deterministic pacing (and only while our simulator runs):
// - our simulator settles every day once its day-level work has drained
//   (fc4_sim_set_pacing): one SimIdle to each of OUR idle agents (an agent
//   whose vtable lies in FreeCity4.dll, PacingIsOurs), which runs its cycle to
//   completion in the core, then PacingFlush; the frame idle passes reach only
//   the original's agents;
// - the messages that the original posts and frames deliver late (§5.7
//   item 4) are deferred instead: the simulator's SimNewDay / Week / Month /
//   Year posts and our lot developer's lot StateChanged (0xE8BDA2E7) posts made
//   while a day is open are queued (PacingDefer) and sent synchronously
//   (MessageSend), in posting order, at the day's settle point (PacingFlush),
//   so every subscriber (ours and the original's, the observer's DIGEST and
//   month lines among them) sees them before the next day's agents.
// Under original pacing nothing here changes behaviour.
#pragma once

#include <cstdint>

class cIGZMessage2;
class cIGZMessageServer2;
class cIGZMessageTarget2;

namespace fc4 {

enum class PacingMode : uint32_t {
    Original = 0,       // FC4_PACING_ORIGINAL
    Deterministic = 1,  // FC4_PACING_DETERMINISTIC
};

// The process's pacing (read at the first call).
PacingMode Pacing();
inline bool PacingDeterministic() { return Pacing() == PacingMode::Deterministic; }
const char* PacingName(PacingMode m);

// Game start: sets the cores' process mode and logs
//   pacing: deterministic (pacing = deterministic, FreeCity4.ini; simulator.md section 5.7 choice: …)
void PacingStartup();

// Whether `target` is one of FreeCity4's own objects (its vtable lies in this
// DLL's image): a paced agent under deterministic pacing.
bool PacingIsOurs(const cIGZMessageTarget2* target);

// [choice] Under deterministic pacing, the SimIdle deliveries per day at the
// settle point for an idle agent registered under `name` (level 10; the
// original's or FreeCity4's own, since glue-propdev4 both get the same
// count), or 0 (not paced: it keeps the frame idle passes). Only agents that do a
// fixed amount of work per delivery are paced: the prop developer (one block
// step per Idle, prop-developer.md §3.3) gets 20, the idle deliveries of a
// day at the middle speed (1000 ms per day at 20 frames per second,
// simulator.md §7).
uint32_t PacingFixedIdleSteps(const char* name);

// Our simulator (main thread): a day was opened (SimNewDay posted) / settled.
// Between the two, PacingDefer queues the deferrable posts.
void PacingDayOpened(int32_t julian);
// Under deterministic pacing while a day is open: AddRefs `msg` and queues it
// for the day's settle point (true); otherwise false (post it as usual).
// Main thread only.
bool PacingDefer(cIGZMessage2* msg);
// The day's settle point: sends every deferred message with `ms` (including
// those deferred while sending), in order, releases them and closes the day;
// then runs the message server's tick until its post queue is empty (at most
// 64 ticks), so every other post of the day is delivered too. A null `ms`
// releases them unsent. Returns the number sent.
uint32_t PacingFlush(cIGZMessageServer2* ms, int32_t julian);
// City shutdown: releases anything still queued, unsent; closes the day.
void PacingDrop(const char* why);

// Counters for the month status line: messages deferred / sent, days settled.
struct PacingStats {
    uint64_t deferred = 0;
    uint64_t sent = 0;
    uint64_t settles = 0;
    uint64_t dropped = 0;
    uint64_t pumps = 0;      // message-server ticks run at settle points
    uint64_t leftovers = 0;  // settle points that left posts queued
};
PacingStats PacingCounters();

}  // namespace fc4
