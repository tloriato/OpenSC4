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

// The exemplar / cohort resource-factory replacement (glue-exemplars4;
// data-formats.md §4, §8, §8.1). With `enable = exemplars` the plugin, at
// PostAppInit (after the game's PreAppInit registered its two factories),
// takes over resource types 0x6534284A (exemplar, factory class 0x453429B3)
// and 0x05342861 (cohort, class 0x053429C8) as §8.1 prescribes: keep a
// reference to the game's factory (FindObjectFactory, slot 21), unregister
// the game's class ID (slot 20), register ours under the SAME class ID with a
// non-null pointer (slot 19). At PreAppShutdown the procedure is repeated
// with the game's factory, which restores the original.
//
// Our factory reads the record the manager opened (position 0, already
// decompressed: no QFS here, §8.1), parses it with the Rust core
// (fc4_exrec_*, engine/formats Exemplar::parse_record: §4.1/§4.2), loads the
// parent cohort and kind-1 resources through the resource manager (§4.2,
// §4.6) and builds the own and effective lists (§4.4). The manager closes
// the record (§8.1, EX44). Off by default.
#pragma once

#include <cstdint>

namespace fc4 {

// Config load: remember `enable = exemplars`.
void ExemplarFactorySetEnabled(bool enabled);
// PostAppInit: install when enabled. Logs one line.
void ExemplarFactoryInstall();
// PreAppShutdown: restore the game's factories and log the counters.
void ExemplarFactoryRestore();
// True while ours is registered (formats_shadow then must not claim a native
// comparison: GetResource returns our objects).
bool ExemplarFactoryActive();

}  // namespace fc4
