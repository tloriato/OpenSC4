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

// The advisor family's game-layout containers (docs/specs/advisors.md §12.2,
// *[compat]*): the original hands out references to two STLport maps,
//
//   system slot 8      map<int32, cRZAutoRefCount<cISC4Advisor>>  (system +0x4C)
//   advisor slots 10/11 map<cRZAutoRefCount<cISC4Advice>, int32>  (advisor +0x70)
//
// and every consumer outside the family only reads them (MySim agent Init
// walks slot 8; the advisor and MySim win procs copy slot 10/11 with the
// game's own tree copy). So the replacement keeps its state in the Rust core
// and rebuilds a mirror in the game's layout (FC4GameMapBuild.h, the game's
// node allocator) whenever the content changes, before any consumer can run.
// Every listed object holds one reference this mirror takes (AddRef) and gives
// back (Release) when the entry leaves, so it stays alive while listed, as the
// original's cRZAutoRefCount members do.
//
// Order: the advisor map compares its int32 key signed; the active map
// compares the advice pointer (cRZAutoRefCount's operator< on the pointer,
// unsigned in a 32-bit process). Duplicate keys are refused (a std::map holds
// each key once): Rebuild returns false and keeps the previous mirror.
//
// Threading (HANDOFF §5.1): call Rebuild on the simulation thread only. The
// map object's address never changes; its header and count are replaced
// after the new tree is complete and the old nodes are freed afterwards, the
// same window the original's in-place insert / erase leaves a second thread.
//
// Header-only; no game calls except through Alloc (tests pass a fake).
#pragma once

#include "FC4GameMapBuild.h"
#include "cIGZUnknown.h"

#include <algorithm>
#include <cstddef>
#include <cstdint>
#include <vector>

namespace fc4 {

enum class AdvisorMapOrder { SignedKey, UnsignedKey };

struct AdvisorMapEntry {
    uint32_t key;        // int32 advisor id, or the advice pointer
    uint32_t value;      // the advisor pointer, or the int32 value
    cIGZUnknown* held;   // the object the entry keeps alive (advisor or advice), may be null
};

template <class Alloc>
class AdvisorGameMap {
public:
    explicit AdvisorGameMap(AdvisorMapOrder order) : order_(order) { map_.header = nullptr; map_.count = 0; map_.comparator = 0; }
    ~AdvisorGameMap() { Clear(); }
    AdvisorGameMap(const AdvisorGameMap&) = delete;
    AdvisorGameMap& operator=(const AdvisorGameMap&) = delete;

    // The map object the interface slot returns, or nullptr before the first
    // successful Rebuild (an empty Rebuild gives a valid empty map).
    GameMapObject* Object() { return map_.header != nullptr ? &map_ : nullptr; }
    size_t Size() const { return entries_.size(); }
    const std::vector<AdvisorMapEntry>& Entries() const { return entries_; }

    // Replaces the content with `entries` (any order). Unchanged content keeps
    // the current tree. False (nothing changed) on a duplicate key or an
    // allocation failure.
    bool Rebuild(std::vector<AdvisorMapEntry> entries) {
        const AdvisorMapOrder order = order_;
        auto less = [order](uint32_t a, uint32_t b) {
            return order == AdvisorMapOrder::SignedKey ? static_cast<int32_t>(a) < static_cast<int32_t>(b) : a < b;
        };
        std::sort(entries.begin(), entries.end(),
                  [&](const AdvisorMapEntry& a, const AdvisorMapEntry& b) { return less(a.key, b.key); });
        for (size_t i = 1; i < entries.size(); ++i) {
            if (!less(entries[i - 1].key, entries[i].key)) return false;
        }
        if (map_.header != nullptr && Same(entries)) return true;
        std::vector<uint32_t> keys, values;
        keys.reserve(entries.size());
        values.reserve(entries.size());
        for (const AdvisorMapEntry& e : entries) {
            keys.push_back(e.key);
            values.push_back(e.value);
        }
        GameMapObject fresh{};
        if (!BuildGameMapBy<Alloc>(fresh, keys.data(), values.data(), keys.size(), less)) return false;
        for (const AdvisorMapEntry& e : entries) {
            if (e.held != nullptr) e.held->AddRef();
        }
        GameMapObject old = map_;
        map_.header = fresh.header;
        map_.count = fresh.count;
        FreeGameMap<Alloc>(old);
        std::vector<AdvisorMapEntry> previous;
        previous.swap(entries_);
        entries_ = std::move(entries);
        for (const AdvisorMapEntry& e : previous) {
            if (e.held != nullptr) e.held->Release();
        }
        return true;
    }

    // Frees the tree and releases every listed object (Shutdown, destructor).
    void Clear() {
        FreeGameMap<Alloc>(map_);
        std::vector<AdvisorMapEntry> previous;
        previous.swap(entries_);
        for (const AdvisorMapEntry& e : previous) {
            if (e.held != nullptr) e.held->Release();
        }
    }

private:
    bool Same(const std::vector<AdvisorMapEntry>& sorted) const {
        if (sorted.size() != entries_.size()) return false;
        for (size_t i = 0; i < sorted.size(); ++i) {
            if (sorted[i].key != entries_[i].key || sorted[i].value != entries_[i].value ||
                sorted[i].held != entries_[i].held)
                return false;
        }
        return true;
    }

    AdvisorMapOrder order_;
    GameMapObject map_;
    std::vector<AdvisorMapEntry> entries_;  // sorted, each `held` referenced once
};

}  // namespace fc4
