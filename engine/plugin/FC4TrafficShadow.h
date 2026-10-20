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

// The traffic shadow: a same-process differential of the traffic simulator
// (tooling; no spec behaviour). Opt-in, off by default:
//
//   traffic_shadow = 1           (env FC4_TRAFFIC_SHADOW)
//   traffic_shadow_diffs = N     differences named per line
//                                (env FC4_TRAFFIC_SHADOW_DIFFS, default 8, 1..64)
//   traffic_shadow_lines = N     delivery lines per month
//                                (env FC4_TRAFFIC_SHADOW_LINES, default 8)
//   traffic_shadow_hypotheses = N  retired (ABI 69; env
//                                FC4_TRAFFIC_SHADOW_HYPOTHESES): accepted, ignored
//                                and logged once; its three readings were
//                                confirmed (traffic-simulator TS55-TS57) and are
//                                the core's only behaviour
//
// It needs FreeCity4's simulator (it wraps the clock deliveries that simulator
// makes). Two modes, decided at game start by whether `enable = traffic` put
// FreeCity4's class where the original's was (FC4TrafficShadow.cpp
// TrafficShadowStartup):
//
//   differential (the default, `traffic` not enabled): the reference is the
//   ORIGINAL traffic simulator and the copy is FreeCity4's core on its record;
//   this is the original-vs-ours comparison.
//
//   self-check (`enable = traffic`): the original is no longer reachable
//   through the city, so the reference is FreeCity4's own live class and the
//   copy is a second instance of the same core on that class's record. Both
//   sides are ours, so a difference is a defect in the replacement (a host
//   callback that disagrees with the copy's read-only one, a record that does
//   not round trip, or non-determinism the record does not carry) rather than
//   a difference from the original. This is the shape glue21 chose for the
//   prop and lot configuration managers.
//
// Our simulator delivers the clock to the reference's agents
// ("cSC4TrafficSimulator": one-shot, day, month), and the shadow wraps every
// delivery (cFC4Simulator::HostDeliver):
//   1. before: the reference's own Write (cIGZSerializable) into a capture
//      stream (bytes, and each reference queried for the grid, occupant and
//      department interfaces); fc4_traffic_shadow_before starts FreeCity4's
//      simulator on it (a private copy, at the first delivery), or resyncs
//      the copy when the record differs from the copy's (something between
//      two deliveries changed one side only), and snapshots the inputs the
//      delivery may change (trip lists, lot and connection holder arrays);
//   2. the reference's DoMessage, unchanged;
//   3. after: the record again; fc4_traffic_shadow_after steps the copy one
//      unit at a time to the cycle state and phase progress the reference
//      stopped at (its day ends on a 4 ms wall-clock deadline,
//      traffic-simulator.md §3.3), then compares bitwise its record (fields
//      named), every source's trip list, the lot and connection holder
//      arrays and the grids with the reference's.
// The copy reads the game through the traffic host table (cFC4TrafficSimulator's
// read callbacks over this city; every write callback NULL but the lot
// deactivation, which is a lookup of the holder's budget item) and the
// original's grids through the grid table's geometry and get. The messages
// the original subscribes to (§1 Init step 3) are forwarded to the copy (a
// message that arrives during a wrapped delivery is forwarded after it).
// The lot strategy byte (§4.1.2) has no getter: it is read as the last byte
// of the lot's own Write record (§6.3).
//
// Not compared (logged at game start): the strategy draws' property
// 0x4A611773 (the original's generator is not readable, §4.15), the pothole
// and accident draws' choices, the posted messages, history, incomes,
// network costs, budget items and accidents (the copy's are counted).
//
// Log lines (always written; a longer line is split into "traffic shadow
// (cont.)" lines):
//   traffic shadow YYYY-MM-DD day|month|begin (state a -> b, progress p,
//     units u): mismatches=N; ...          a delivery with a difference
//   traffic shadow YYYY-MM-DD kind: INCOMPLETE (why); not compared
//   traffic shadow month to YYYY-MM-DD: clean|mismatches=N|INCOMPLETE;
//     deliveries=.. compared=.. clean=.. incomplete=..; record fields ...;
//     trip lists ...; holder properties ...; grid tracts ...; between
//     deliveries: resyncs=..; ours: ...; first differences: ...
#pragma once

#include <cstdint>

class cIGZMessage2;
class cIGZMessageTarget2;
class cISC4City;

namespace fc4 {

// Game start (after the subsystems are resolved): reads the settings and logs
// which of the two modes the shadow runs in (or why it cannot run).
void TrafficShadowStartup(bool simulatorReplaced, bool trafficReplaced);

// cFC4Simulator::HostDeliver, for every agent delivery: if the shadow is on
// and `target` is the reference traffic simulator's agent (the original's, or
// ours in the self-check), runs before / target->DoMessage(msg) / after,
// stores the agent's result in `finished` and returns true. Otherwise returns
// false and does nothing (the caller delivers).
bool TrafficShadowDeliver(cIGZMessageTarget2* target, const char* agentName, cIGZMessage2* msg, uint32_t type,
                          int64_t data1, bool* finished);

}  // namespace fc4

// Test seam op 51 (FC4TestSeam.h): one shadowed delivery to a fake original.
// Only the city's GetTrafficSimulator is called on `city`; the copy's reads
// of the other services see none (neutral values).
struct FC4TrafficShadowTestInput {
    cISC4City* city;
    cIGZMessageTarget2* target;  // the delivery's target
    uint32_t msgType;            // 0x66956810, 0x66956814 or 0x66956816
    int32_t julian;              // data1
    uint32_t fresh;              // bit 0: a new shadow before (else the previous call's is kept);
                                 // bit 1: the shadow dropped after
};
// Returns the delivery's difference count, -1 if the shadow did not take the
// delivery, -2 if it delivered but compared nothing.
intptr_t TrafficShadowTestDeliver(const FC4TrafficShadowTestInput* in);
