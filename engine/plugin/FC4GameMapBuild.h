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

// Game maps WE own (docs/abi/allocators.md, "map and set"): an STLport
// map<uint32_t, 4-byte value> built from scratch with an allocator policy
// (FC4GameList.h: static Allocate(uint32_t) / Free(void*); the plugin passes
// StlNodeAllocator, B), for the maps a replacement class hands out by
// pointer or embeds in a structure the game copies (the BDS developer map,
// slot 34; the building profiles' capacity maps, slots 39/40; the growth
// developer's style map, slot 26).
//
//   BuildGameMap  a balanced red-black tree of sorted, strictly increasing
//                 keys: the middle key is the root of every subtree, so every
//                 root-to-null path holds h or h + 1 nodes (h =
//                 floor(log2(n + 1))); nodes above depth h are black and the
//                 nodes at depth h red, which satisfies both red-black rules.
//                 A B header (colour 0, parent = root, left / right = the
//                 smallest / largest node, or the header itself when empty)
//                 and one 0x18-byte B node per key.
//   FreeGameMap   frees every node and the header with the same policy.
//
// The game reads these maps (find, iteration, copy construction) and never
// inserts into them. Header-only, no game calls.
#pragma once

#include "FC4GameMap.h"

#include <cstddef>
#include <cstdint>
#include <cstring>
#include <vector>

namespace fc4 {

namespace detail {

template <class Alloc>
GameMapNode* NewGameMapBlock() {
    auto* n = static_cast<GameMapNode*>(Alloc::Allocate(static_cast<uint32_t>(sizeof(GameMapNode))));
    if (n != nullptr) std::memset(static_cast<void*>(n), 0, sizeof(GameMapNode));
    return n;
}

// Links nodes[lo, hi) (already holding their keys) as the subtree under
// `parent`; returns its root.
inline GameMapNode* LinkGameSubtree(GameMapNode* const* nodes, size_t lo, size_t hi, uint32_t depth, uint32_t redDepth,
                                    GameMapNode* parent) {
    if (lo >= hi) return nullptr;
    const size_t mid = lo + (hi - lo) / 2;
    GameMapNode* n = nodes[mid];
    n->color = depth == redDepth ? kGameMapRed : kGameMapBlack;
    n->parent = parent;
    n->left = LinkGameSubtree(nodes, lo, mid, depth + 1, redDepth, n);
    n->right = LinkGameSubtree(nodes, mid + 1, hi, depth + 1, redDepth, n);
    return n;
}

inline void FreeGameSubtree(GameMapNode* n, void (*freeFn)(void*)) {
    while (n != nullptr) {
        FreeGameSubtree(n->right, freeFn);
        GameMapNode* left = n->left;
        freeFn(n);
        n = left;
    }
}

}  // namespace detail

// Fills `m` (raw storage: whatever it held is NOT freed; the comparator word
// is left alone) with the n entries keys[i] -> values[i]. False, with `m`
// untouched and nothing allocated, when the keys are not strictly
// increasing or the allocator fails.
template <class Alloc, class Less>
bool BuildGameMapBy(GameMapObject& m, const uint32_t* keys, const uint32_t* values, size_t n, Less less);

template <class Alloc>
bool BuildGameMap(GameMapObject& m, const uint32_t* keys, const uint32_t* values, size_t n) {
    return BuildGameMapBy<Alloc>(m, keys, values, n, [](uint32_t a, uint32_t b) { return a < b; });
}

// BuildGameMap for a map whose comparator is not the unsigned one (a
// map<int32_t, T> compares signed: FC4AdvisorMaps.h): the keys must be
// strictly increasing under `less`, which the tree's order then follows.
template <class Alloc, class Less>
bool BuildGameMapBy(GameMapObject& m, const uint32_t* keys, const uint32_t* values, size_t n, Less less) {
    for (size_t i = 1; i < n; ++i) {
        if (!less(keys[i - 1], keys[i])) return false;
    }
    std::vector<GameMapNode*> nodes(n + 1, nullptr);
    for (size_t i = 0; i <= n; ++i) {
        nodes[i] = detail::NewGameMapBlock<Alloc>();
        if (nodes[i] == nullptr) {
            for (size_t j = 0; j < i; ++j) Alloc::Free(nodes[j]);
            return false;
        }
    }
    GameMapNode* header = nodes[n];  // the last block
    header->color = kGameMapRed;
    for (size_t i = 0; i < n; ++i) {
        nodes[i]->key = keys[i];
        nodes[i]->value = values[i];
    }
    uint32_t h = 0;  // floor(log2(n + 1))
    while ((static_cast<uint64_t>(2) << h) <= static_cast<uint64_t>(n) + 1) ++h;
    GameMapNode* root = detail::LinkGameSubtree(nodes.data(), 0, n, 0, h, header);
    header->parent = root;
    header->left = n != 0 ? nodes[0] : header;
    header->right = n != 0 ? nodes[n - 1] : header;
    m.header = header;
    m.count = static_cast<uint32_t>(n);
    return true;
}

// Frees what BuildGameMap allocated (nodes, then the header); `m` becomes
// {null, 0}. A map with a null header is left alone.
template <class Alloc>
void FreeGameMap(GameMapObject& m) {
    if (m.header == nullptr) return;
    detail::FreeGameSubtree(m.header->parent, [](void* p) { Alloc::Free(p); });
    Alloc::Free(m.header);
    m.header = nullptr;
    m.count = 0;
}

}  // namespace fc4
