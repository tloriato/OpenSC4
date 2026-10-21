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

// The network manager shadow: a same-process differential of
// cSC4NetworkManager (component 53) and cSC4NetworkLotManager (component 11)
// (docs/specs/network-manager.md; tooling, no spec behaviour). Opt-in, off by
// default:
//
//   netmgr_shadow = 1        (env FC4_NETMGR_SHADOW)
//
// Two shadow cores (fc4_netmgr, fc4_netlot) with read-only hosts
// (FC4NetMgrHost.h: nothing is written to the game) follow the ORIGINAL
// managers:
//   - at PostCityInit both start from the originals' own records (their Write
//     into memory, §4), the lot core with the network lot configurations of
//     the exemplars (§3.2 step 4, read once per process);
//   - a listener registered at PostAppInit, before any city subscribes, so it
//     runs before the originals at every MessageSend (foundations §5.1),
//     receives the 11 messages of §3.1 and feeds them to the manager core;
//     the lot core hears its 6 (§3.2) through a second target subscribed at
//     PostCityInit, after the original (netmgr_shadow_order = late, the
//     default; early: the first listener), so it reads the game as the
//     original did: occupant removals, the usable message, zoning, lot state
//     and road damage (§6); the first listener then snapshots each lot
//     message's cells, so the road style writes are also compared message by
//     message;
//   - the cores' writes are recorded, not made: the terrain rule's
//     destructions (§7.6) are checked at the next framework tick against the
//     occupants the original removed (each decided occupant gone; no other
//     candidate gone, except the decided ones' neighbours, which the tool
//     rebuilds); the road style writes (§8.7) against the occupants'
//     GetVariation (slot 28) and flags 0x6000000;
//   - at PostCityInit, each SimNewMonth and before a scripted save: the
//     managed list (our component 53 Write against the original's, byte for
//     byte and object for object), the network lots (our component 11 Write
//     against the original's, entry by entry: occupant and configuration ID;
//     the pothole keys), the original's GetZoneTypeForNetwork /
//     GetWealthForNetwork (slots 9, 10) against ours at every lot cell, and
//     for a configuration that differs, whether both are compatible now
//     (IsLotCompatible, §8.4): then it is a random draw (the original's
//     generator is seeded from the clock, §8.5, §17), counted, not a
//     mismatch. A core whose record differs restarts from the original's.
//   - NM10/NM11 (answered, network-manager.md §6.3): the lot state message
//     walks from (0, 0); the core does too, and every cell is compared (the
//     per-message comparison also covers the walk's cells off the rect).
//
// Log lines (always written, not the verbose channel):
//   netmgr shadow: ON (...) | requested but INACTIVE (...)        game start
//   netmgr shadow (load|month|save) YYYY-MM-DD: mismatches=N managed=a/b ...
//   netmgr shadow: city shutdown: N lines, M with mismatches=0 (...)
//   netmgr shadow: lot state changed (0xE8BDA2E7) border cells ... by place
#pragma once

#include <cstdint>

class cISC4City;

namespace fc4 {

// FC4Subsystems.cpp Resolve: reads the setting.
void NetMgrShadowStartup();
// cFC4Director PostAppInit / PreAppShutdown: the listener.
void NetMgrShadowStart();
void NetMgrShadowStop();

// FC4Observer.cpp: PostCityInit, SimNewMonth, each framework tick,
// PreCityShutdown.
void NetMgrShadowCityLoaded(cISC4City* city);
void NetMgrShadowMonth(cISC4City* city, int32_t julian);
void NetMgrShadowTick();
void NetMgrShadowCityShutdown();

// FC4Script.cpp: before a scripted save (the records the save writes).
void NetMgrShadowBeforeSave(cISC4City* city, int32_t julian);

}  // namespace fc4
