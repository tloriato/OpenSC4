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

// The occupant manager shadow: a same-process differential of the city's
// ORIGINAL cISC4OccupantManager against the fc4_occmgr core
// (occupant-manager.md; tooling, no spec behaviour). Opt-in, off by default:
//
//   occmgr_shadow = 1           (env FC4_OCCMGR_SHADOW)
//   occmgr_shadow_samples = N   sampled calls per query slot and month (default 4, 1..64)
//   occmgr_shadow_probe = 0/1   the thread probe below (default 1)
//
// Nothing replaces the original (FreeCity4 has no occupant manager
// replacement): the shadow core has a read-only host and only follows.
//   - At PostCityInit it starts from the original's record (its own Write into
//     memory, §13): the stored box and flag 1 of every occupant in it are read
//     from the occupant (GetOccupantManagerBBox, IsFlagSet) into the shadow's
//     own per-occupant state, then the record is Read and the core Init'ed.
//     The host keeps that state (SetOccupantManagerBBox / SetFlag / ClearFlag
//     are recorded, never made), AddRefs the occupants it holds (audited) and
//     posts, initialises and shuts down nothing.
//   - Its own subscription receives the occupant messages the original posts
//     (Insert 0x99EF1142, Remove 0x99EF1143, Move 0x99EF1146, §5.5) and runs
//     the same call on the core (InsertOccupant(occ, data4), RemoveOccupant(occ,
//     false, data4), MoveOccupant(occ, false)); after each Insert or Move the
//     core's box is compared with the box the original stored in the occupant
//     (§5.1 steps 5-7: the shrink, slot 15 and the bytes).
//   - At each SimNewMonth: occupants whose stored box changed without a Move
//     message (MoveOccupant(occ, false)) are moved in the core; the original's
//     record is captured and compared cell by cell, in list order, with the
//     core's cells (an occupant whose Insert or Remove message is still queued
//     is set aside, by its flag 1); a difference left after that re-starts
//     the core from the record (`resynced`). Then the read-only queries are
//     sampled, on the followed core when nothing was set aside and otherwise
//     on a fresh core read from the month's record: slots 25, 27 and 28-47
//     each run `occmgr_shadow_samples` times on the original (through its
//     vtable, slots 46/47 in the game's order) and on the core with the same
//     arguments, from a generator seeded by the month: points, cells and
//     rectangles around occupants of the record or anywhere in and around the
//     city, a null filter or one deterministic cISC4OccupantFilter of ours
//     shared by both, types of occupants of the record, 0 or none, list
//     maxCounts -1/0/1/2/7, iteration callbacks that stop after 0/1/2/5
//     calls. The return value and the occupants (in order) must be equal;
//     FindOccupant's x and z too.
//   - The thread probe (OM1, OM3): the original's vtable entries of slots 1,
//     2, 12, 20-22, 24, 25, 27, 28, 30, 32, 34, 36, 38, 40 (the model loader's
//     slot, OM1), 42 and 44 forward to
//     counting stubs (installed at PostCityInit only while each holds the
//     expected 1.1.641 address, removed at PreCityShutdown), which count the
//     calls per thread (the shadow's own calls excluded) and the entries made
//     while another thread was inside one of those slots ("overlaps", an
//     approximate count of lock contention).
//   - The message-order comparison (glue25): the manager's own messages
//     (Insert 0x99EF1142, Remove 0x99EF1143, Move 0x99EF1146, §5.5) are kept
//     in delivery order as the original posts them, and in post order as the
//     followed core posts them (its host records them; the Move call is made
//     with post = true so the core posts one too, §5.4 step 9). The two
//     streams must match element by element on kind, occupant and data4.
//     Destroy 0x99EF1141 is subscribed but never replayed: it is only checked
//     against §5.3 step 4's interleaving (Remove(o1), Destroy(o1), Remove(o2),
//     Destroy(o2), ...). A Destroy that follows a Remove of ANOTHER occupant
//     means the batch was posted the other way round and counts as a mismatch;
//     a Destroy after anything else is an occupant shut down outside the
//     manager and is only counted.
//
// Log lines (always written):
//   occmgr shadow: ON (...) | requested but INACTIVE (...)            game start
//   occmgr shadow: following the original occupant manager ...        city load
//   occmgr shadow month YYYY-MM-DD: followed=yes|first|resynced occupants=N refs=R
//     record=exact|D cells differ (P set aside); silent moves=S; messages ins/rem/move=a/b/c
//     refused=r off-thread=t boxes=x/y; msgorder=a/b (exact)| destroy=o/n/e; queries on
//     followed|fresh core q/Q (every slot exact)| differ: sN=e/t ...;
//     probe: ...; stray releases=0; mismatches=M[; differences: ...]
//   occmgr shadow: city shutdown: N month lines, M with mismatches=0; per slot ...; probe per thread ...
#pragma once

#include <cstdint>

class cISC4City;

namespace fc4 {

void OccMgrShadowStartup();

// FC4Observer.cpp, PostCityInit: start from the original's record. No-op
// unless occmgr_shadow = 1.
void OccMgrShadowCityLoaded(cISC4City* city);

// FC4Observer.cpp, SimNewMonth: the month's comparison. No-op unless
// occmgr_shadow = 1.
void OccMgrShadowMonth(cISC4City* city, int32_t julian);

// FC4Observer.cpp, PreCityShutdown: the probe and the shadow core (and its
// references) go.
void OccMgrShadowCityShutdown();

}  // namespace fc4
