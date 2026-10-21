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

// FC4AdvisorMaps.h (advisors.md §12.2): the game-layout mirrors of the advisor
// map (signed int32 key) and an advisor's active map (pointer key): order,
// layout walk, references held while listed, unchanged content kept, a
// duplicate refused, Clear.

#include "FC4AdvisorMaps.h"
#include "FC4GameMap.h"

#include <algorithm>
#include <cstdio>
#include <cstdlib>
#include <set>
#include <vector>

namespace {

int g_failures = 0;
#define CHECK(cond)                                                     \
    do {                                                                \
        if (!(cond)) {                                                  \
            std::printf("FAIL %s:%d: %s\n", __FILE__, __LINE__, #cond); \
            ++g_failures;                                               \
        }                                                               \
    } while (0)

std::set<void*> g_live;
struct FakeAlloc {
    static void* Allocate(uint32_t size) {
        void* p = std::calloc(1, size);
        g_live.insert(p);
        return p;
    }
    static void Free(void* p) {
        CHECK(g_live.erase(p) == 1);
        std::free(p);
    }
};

class FakeObj : public cIGZUnknown {
public:
    bool QueryInterface(uint32_t, void**) override { return false; }
    uint32_t AddRef() override { return ++refs; }
    uint32_t Release() override { return --refs; }
    uint32_t refs = 0;
};

using Map = fc4::AdvisorGameMap<FakeAlloc>;

std::vector<uint32_t> Keys(Map& m) {
    std::vector<uint32_t> out;
    uint32_t visited = 0;
    CHECK(fc4::WalkGameMap(m.Object(), 100, [&](const fc4::GameMapNode& n) { out.push_back(n.key); }, &visited) ==
          fc4::GameMapStatus::Ok);
    CHECK(visited == out.size());
    return out;
}

// WalkGameMap checks unsigned key order; the signed advisor map is checked
// by a plain in-order walk from the root with parent links verified.
void InOrder(const fc4::GameMapNode* n, const fc4::GameMapNode* parent, std::vector<uint32_t>& out) {
    if (n == nullptr) return;
    CHECK(n->parent == parent);
    InOrder(n->left, n, out);
    out.push_back(n->key);
    InOrder(n->right, n, out);
}

std::vector<uint32_t> SignedKeys(Map& m) {
    std::vector<uint32_t> out;
    const fc4::GameMapNode* h = m.Object()->header;
    InOrder(h->parent, h, out);
    CHECK(out.size() == m.Object()->count);
    if (!out.empty()) {
        CHECK(h->left->key == out.front() && h->right->key == out.back());
    }
    return out;
}

uint32_t P(const void* p) { return static_cast<uint32_t>(reinterpret_cast<uintptr_t>(p)); }

}  // namespace

int main() {
    FakeObj a, b, c;
    {
        Map advisors(fc4::AdvisorMapOrder::SignedKey);
        CHECK(advisors.Object() == nullptr);
        CHECK(advisors.Rebuild({}));
        CHECK(advisors.Object() != nullptr && advisors.Object()->count == 0);
        CHECK(advisors.Object()->header->left == advisors.Object()->header);
        // signed order: -1 before 2 before 30, not the unsigned order
        CHECK(advisors.Rebuild({{30u, P(&c), &c}, {static_cast<uint32_t>(-1), P(&a), &a}, {2u, P(&b), &b}}));
        CHECK(SignedKeys(advisors) == (std::vector<uint32_t>{static_cast<uint32_t>(-1), 2u, 30u}));
        CHECK(a.refs == 1 && b.refs == 1 && c.refs == 1);
        CHECK(advisors.Object()->header->left->value == P(&a));
        // unchanged content: same tree, no extra references
        fc4::GameMapNode* header = advisors.Object()->header;
        CHECK(advisors.Rebuild({{2u, P(&b), &b}, {30u, P(&c), &c}, {static_cast<uint32_t>(-1), P(&a), &a}}));
        CHECK(advisors.Object()->header == header && a.refs == 1);
        // a duplicate key is refused and the old content kept
        CHECK(!advisors.Rebuild({{2u, P(&b), &b}, {2u, P(&c), &c}}));
        CHECK(advisors.Size() == 3 && b.refs == 1 && c.refs == 1);
        // an entry leaves: its reference goes back
        CHECK(advisors.Rebuild({{2u, P(&b), &b}}));
        CHECK(a.refs == 0 && b.refs == 1 && c.refs == 0);
        CHECK(Keys(advisors) == (std::vector<uint32_t>{2u}));
        advisors.Clear();
        CHECK(b.refs == 0 && advisors.Object() == nullptr);
        CHECK(advisors.Rebuild({{5u, P(&a), &a}}));
    }
    CHECK(a.refs == 0);  // the destructor released it
    {
        Map active(fc4::AdvisorMapOrder::UnsignedKey);
        std::vector<fc4::AdvisorMapEntry> e{{P(&c), 7u, &c}, {P(&a), 3u, &a}, {P(&b), 5u, &b}};
        CHECK(active.Rebuild(e));
        std::vector<uint32_t> want{P(&a), P(&b), P(&c)};
        std::sort(want.begin(), want.end());
        CHECK(Keys(active) == want);
        CHECK(a.refs == 1 && b.refs == 1 && c.refs == 1);
    }
    CHECK(a.refs == 0 && b.refs == 0 && c.refs == 0);
    CHECK(g_live.empty());
    if (g_failures == 0) std::printf("advisor_maps_test: all passed\n");
    return g_failures == 0 ? 0 : 1;
}
