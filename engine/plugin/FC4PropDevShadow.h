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

// The prop developer shadow: a same-process differential of the prop
// developer (prop-developer.md; tooling, no spec behaviour). Opt-in, off by
// default:
//
//   propdev_shadow = 1        (env FC4_PROPDEV_SHADOW)
//
// It needs FreeCity4's simulator (`simulator` enabled: the original's agent
// deliveries pass through it) and the ORIGINAL prop developer (`propdev` not
// enabled). At the first block step it follows (Idle 0x6695681A or Day
// 0x66956814 to the agent "cSC4PropDeveloper"), a shadow core (read-only
// host: it changes nothing in the game) reads the original's record (§7)
// and inits; it then follows the 13 messages of §3.1 step 5 and the
// animation tick through its own listener. Each block step of the original
// is wrapped: ours steps first (its removals and creations recorded, not
// made), the block's props are listed, the original steps (exactly once),
// and then
//   - the scan flag and the block scanner (§2, raw +0x14 / +0x18..+0x34,
//     vtable-checked) are compared with ours (exact);
//   - when every keep check of ours was certain (wealth, pollution, a crime
//     prop with incident ID 0: crime.md §11.6), the number of props the
//     original removed from the block with ours, and every prop ours removed
//     in the cell walk (not kept, §4.3 step 3.9) must be among them.
// Not compared: which kept prop the member generator removes (§4.3 step 5)
// and the creations (member and wealth generators, §5). At each SimNewMonth
// native Write bytes and reference identities are compared directly with Rust
// Write and the signs-visible query is probed. These are diagnostic probes.
// Bootstrap happens once per owner; no resync or regrowth after faults. Missing
// identical RNG and callback capture (PV14/PV15) refuses operation credit for
// every observed natural Idle/Day call, including matching returns/scanners.
//
// Log lines (always written):
//   propdev shadow: ON (...) | requested but INACTIVE (...)                  game start
//   propdev shadow month YYYY-MM-DD: followed=first|yes|resynced steps=.. blocks=.. certain=.. removed orig/ours ..;
//     created orig/ours .. (not compared); record ...; mismatches=N[; differences: ...]
//   propdev shadow: city shutdown: N month lines, M with mismatches=0
#pragma once

#include <cstdint>

class cIGZMessage2;
class cIGZMessageTarget2;
class cISC4City;

namespace fc4 {

void PropDevShadowStartup(bool simulatorReplaced, bool propdevReplaced);

// cFC4Simulator.cpp HostDeliver: the original's Idle and Day deliveries.
bool PropDevShadowDeliver(cIGZMessageTarget2* target, const char* agentName, cIGZMessage2* msg, uint32_t type,
                          int64_t data1, bool* finished);

// FC4Observer.cpp: SimNewMonth and PreCityShutdown.
void PropDevShadowMonth(cISC4City* city, int32_t julian);
void PropDevShadowCityShutdown();

}  // namespace fc4
