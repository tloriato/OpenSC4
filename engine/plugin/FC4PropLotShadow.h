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

// The prop manager, lot configuration manager and tutorial shadows: same-
// process differentials of the ORIGINAL cSC4PropManager (city component 5),
// cSC4LotConfigurationManager (component 9) and cSC4TutorialSystem
// (component 48) (docs/specs/prop-lotconfig.md, tutorial.md; tooling, no spec
// behaviour). Opt-in, off by default:
//
//   propmgr_shadow = 1          (env FC4_PROPMGR_SHADOW)
//   propmgr_shadow_cells = N    sample cells per run (env FC4_PROPMGR_SHADOW_CELLS, default 32, 1..256)
//   lotcfg_shadow = 1           (env FC4_LOTCFG_SHADOW)
//   tutorial_shadow = 1         (env FC4_TUTORIAL_SHADOW)
//
// Each shadow only reads the class the city holds - the game's original, or
// FreeCity4's replacement when `enable = propmgr` / `lotcfg` / `tutorial` is
// on (glue21: a replacement does not turn its shadow off; over a replacement
// the comparison is a class-layer self-check). At PostCityInit ("load"), at every
// SimNewMonth ("month") and at PreSave ("save"), on the main thread, a core
// of FreeCity4's (fc4_prop_manager / fc4_lot_config_manager / fc4_tutorial) reads
// the original's own record (its cIGZSerializable::Write into memory,
// references resolved to the original's objects), runs Init over a
// read-only host table, and its answers are compared with the original's:
//
//   propmgr (catalogue, load only): GetAllPropTypes and GetAllPropFamilyTypes
//     in bucket order (slots 5, 6), each type's key (7) and profile (14,
//     0x1C bytes; the three bytes after the wealth byte are not written by
//     the profile reader, §A3.2, and are counted apart), each family's members
//     (9), and the catalogue message's data2 (PL2) against the type count;
//   propmgr (queries): on sample cells drawn from the date (no game random
//     number is drawn) GetCityProps by cell rectangle (18) and world rectangle
//     (17), GetLotProps(x, z) (23) and GetStreetProps by rectangle (30) and by
//     cell (31): the result and the props in order;
//   propmgr (CreateProp): slot 15 on sample prop types: the result, the
//     prop's class, its prop type and its bounding box; ours runs on a
//     recording host (the class FreeCity4 would create, SetPropType,
//     SetBoundingBox), the original's prop is created and released;
//   propmgr / lotcfg / tutorial (record): ours Written after reading the
//     original's record, byte for byte and reference by reference;
//   lotcfg (map, families): the configuration map (slot 20) and the family
//     set (slot 21) in bucket order;
//   lotcfg (arrays, load only): each configuration's object array (slot 23
//     of the configuration) against fc4_lot_config_object_array on its
//     exemplar at the city detail (§B3.3), the trailing foundation object
//     (lot-configuration.md §3.7) excluded;
//   lotcfg (queries): GetLotConfigurationIDsByFamily / BySize (slots 14, 15)
//     as sets with their counts, LotConfigurationHasBuildingFamily / Type
//     (16, 17) and GetObjectIDArray (27) on sample configurations;
//   tutorial (state): IsTutorial (slot 8), the task number (11), the GUID (12).
//
// The lot configuration shadow's catalogue is FreeCity4's (keys, group
// filter, map and family-set order) over the ORIGINAL's configuration
// objects: a configuration ours loads stands for the original's
// configuration of the same ID (lot-configuration.md §3.1 step 4: the ID is
// the key's instance), so the configuration class itself is not shadowed.
//
// Not compared (logged at game start): GetBuildingProps (the 0xA8322B7F
// property object's layout is not in docs/abi), every call that changes the
// city (adding or removing props, the text decal command, adding, removing
// or fixing configurations, CreateGenericLotConfiguration), the filter
// caches, the footprints (slots 24-26) and the building information of
// PostCityInit; the tutorial window, registry and cheat (TU6, TU7).
//
// Log lines (always written), each with mismatches=N:
//   propmgr shadow (catalogue|queries|CreateProp|record) YYYY-MM-DD load|month|save: ...
//   lotcfg shadow (map|families|arrays|queries|record) YYYY-MM-DD load|month|save: ...
//   tutorial shadow (state|record) YYYY-MM-DD load|month|save: ...
// and at city shutdown one totals line per shadow:
//   <name> shadow totals: lines=N exact=E mismatched=M incomplete=I
#pragma once

#include <cstdint>

class cISC4City;

namespace fc4 {

// Game start (after the subsystems are resolved): reads the settings and logs
// whether each shadow can run (each compares an ORIGINAL class).
void PropLotShadowStartup(bool propmgrReplaced, bool lotcfgReplaced, bool tutorialReplaced);

// FC4Observer.cpp: PostCityInit, every SimNewMonth, PreSave (0x26C63343) and
// PreCityShutdown (the totals). No-ops unless a shadow is on.
void PropLotShadowCityLoaded(cISC4City* city);
void PropLotShadowMonth(cISC4City* city, int32_t julian);
void PropLotShadowPreSave(cISC4City* city);
void PropLotShadowCityShutdown();

// The director's PreAppShutdown: the shadows' process catalogues are torn
// down (fc4_prop_catalogue_app_shutdown) and freed, and the lot configuration
// manager's process unload callback (fc4_lot_config_unload) runs over the
// process catalogue. It is not run per shadow run: unload empties the map the
// runs share, and a rebuild would put the configurations in a different bucket
// order than the original's.
void PropLotShadowAppShutdown();

}  // namespace fc4
