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

// Game containers whose memory belongs to the game's STL node allocator
// (instance B, docs/abi/allocators.md).
//
// The game has three pooled allocator instances; a block must go back to the
// instance it came from. The SDK's SC4List / SC4Vector / SC4String use the
// framework allocator service (A), so they must not cross the boundary where
// the game's side declares std::list / std::vector / cRZString (B): every
// block freed to the wrong instance seeds a foreign block into its pools
// (in-game runs 13-16).
//
// These types have the game's layouts (the same as the SDK's) and do all
// their allocation through one allocator policy, by default B:
//   GameList<T>    one pointer to a 12-byte head node; nodes {next, prev, value}
//   GameVector<T>  {begin, end, capacity_end}; elements copied bitwise
//   GameString     a cRZString {vtable, begin, end, capacity_end, refcount}
//                  with a B character buffer and the game's vtable
//   GameHashSetU32 STLport's hash_set<uint32_t> {functors, bucket vector,
//                  count}: 8-byte B nodes, a B bucket buffer, prime bucket
//                  counts (allocators.md, hash_set)
// The game owns the containers it passes us by reference: we fill, clear and
// grow them, never destroy them. Containers we create ourselves are destroyed
// by their destructors (elements released / destroyed, memory freed to B).
//
// An allocator policy has static Available(), Allocate(uint32_t), Free(void*)
// and StringVtable(); tests inject counting ones.
#pragma once

#include "SC4List.h"
#include "cIGZUnknown.h"
#include "compat/FC4Compat.h"

#include <cstddef>
#include <cstdint>
#include <cstring>
#include <new>
#include <type_traits>

namespace fc4 {

// The game's STL node allocator and cRZString vtable (compat table; refused
// on an unknown executable).
struct StlNodeAllocator {
    static bool Available() { return compat::StlNodeAllocator(nullptr, nullptr); }
    static void* Allocate(uint32_t size) {
        compat::StlNodeAllocateFn a = nullptr;
        return compat::StlNodeAllocator(&a, nullptr) ? a(size) : nullptr;
    }
    static void Free(void* p) {
        compat::StlNodeFreeFn f = nullptr;
        if (p != nullptr && compat::StlNodeAllocator(nullptr, &f)) f(p);
    }
    static const void* StringVtable() { return compat::GameStringVtable(); }
};

// ---- strings ------------------------------------------------------------------------------------

// The game's cRZString (allocators.md, Strings). Plain data: copying it
// bitwise moves the ownership of its buffer.
struct GameString {
    const void* vtable;
    char* begin;
    char* end;
    char* capacityEnd;
    uint32_t refCount;

    const char* data() const { return begin; }
    size_t size() const { return static_cast<size_t>(end - begin); }
};
static_assert(sizeof(void*) != 4 || sizeof(GameString) == 0x14, "cRZString is 0x14 bytes");

// Builds `s` (raw storage) as a game string holding `n` bytes of `text`, with
// a character buffer from Alloc. False (s untouched) without the allocator or
// the vtable.
//
// [abi] As the game's char* constructor does (allocators.md, Strings): exactly
// n + 1 bytes, end at the terminator, capacity_end = begin + n + 1 (begin plus
// the bytes allocated, terminator slot included), reference count 0 (embedded
// strings are destroyed inline, never reference-counted).
template <class Alloc = StlNodeAllocator>
bool GameStringInit(GameString& s, const char* text, size_t n) {
    const void* vt = Alloc::StringVtable();
    if (vt == nullptr || n > 0x7FFFFF00u) return false;
    char* b = static_cast<char*>(Alloc::Allocate(static_cast<uint32_t>(n + 1)));
    if (b == nullptr) return false;
    if (n != 0) std::memcpy(b, text, n);
    b[n] = 0;
    s.vtable = vt;
    s.begin = b;
    s.end = b + n;
    s.capacityEnd = b + n + 1;
    s.refCount = 0;
    return true;
}

// What the game does inline (allocators.md): free the buffer with B.
template <class Alloc = StlNodeAllocator>
void GameStringDestroy(GameString& s) {
    Alloc::Free(s.begin);
    s.begin = s.end = s.capacityEnd = nullptr;
}

// ---- element destruction ------------------------------------------------------------------------

// How a GameVector destroys an element (clear, destructor). Plain data by
// default; types holding game strings specialize it.
template <class T>
struct GameElement {
    static_assert(std::is_trivially_copyable_v<T> && std::is_trivially_destructible_v<T>,
                  "GameVector elements are plain data copied bitwise");
    template <class Alloc>
    static void Destroy(T&) {}
};

template <>
struct GameElement<GameString> {
    template <class Alloc>
    static void Destroy(GameString& s) {
        GameStringDestroy<Alloc>(s);
    }
};

// cISC4BudgetSimulator::BudgetGroupInfo as the game's callers build and
// destroy it (allocators.md table (2): 0x28 bytes, the name's buffer is B).
struct GameBudgetGroupInfo {
    uint32_t groupID;
    GameString groupName;
    int64_t totalExpense;
    int64_t totalIncome;
};
static_assert(sizeof(void*) != 4 || (sizeof(GameBudgetGroupInfo) == 0x28 && offsetof(GameBudgetGroupInfo, groupName) == 4 &&
                                     offsetof(GameBudgetGroupInfo, totalExpense) == 0x18 &&
                                     offsetof(GameBudgetGroupInfo, totalIncome) == 0x20),
              "BudgetGroupInfo layout (allocators.md)");

template <>
struct GameElement<GameBudgetGroupInfo> {
    template <class Alloc>
    static void Destroy(GameBudgetGroupInfo& g) {
        GameStringDestroy<Alloc>(g.groupName);
    }
};

// ---- vectors ------------------------------------------------------------------------------------

template <typename T, typename Alloc = StlNodeAllocator>
class GameVector {
public:
    GameVector() = default;
    GameVector(const GameVector&) = delete;
    GameVector& operator=(const GameVector&) = delete;
    // Only for vectors we create; the game destroys the ones it passes us.
    ~GameVector() {
        clear();
        Alloc::Free(begin_);
        begin_ = end_ = capacityEnd_ = nullptr;
    }

    size_t size() const { return static_cast<size_t>(end_ - begin_); }
    size_t capacity() const { return static_cast<size_t>(capacityEnd_ - begin_); }
    bool empty() const { return begin_ == end_; }
    T* begin() { return begin_; }
    T* end() { return end_; }
    T& operator[](size_t i) { return begin_[i]; }

    // Destroys the elements; keeps the buffer (as the game's clear does).
    void clear() {
        for (T* p = begin_; p != end_; ++p) GameElement<T>::template Destroy<Alloc>(*p);
        end_ = begin_;
    }

    // Grows the buffer to at least `n` elements with Alloc, freeing the old
    // buffer (even one the game allocated: it is B too) with Alloc.
    bool reserve(size_t n) {
        if (n <= capacity()) return true;
        if (n > 0x7FFFFFFFu / sizeof(T)) return false;
        T* fresh = static_cast<T*>(Alloc::Allocate(static_cast<uint32_t>(n * sizeof(T))));
        if (fresh == nullptr) return false;
        const size_t count = size();
        if (count != 0) std::memcpy(static_cast<void*>(fresh), static_cast<const void*>(begin_), count * sizeof(T));
        Alloc::Free(begin_);
        begin_ = fresh;
        end_ = fresh + count;
        capacityEnd_ = fresh + n;
        return true;
    }

    // Room for `n` elements as the game's vector resize grows (crime.md
    // §11.4): past the capacity, a new buffer of size + max(size, n - size)
    // elements (exactly n when empty); within it, the buffer is kept.
    bool reserve_for_resize(size_t n) {
        if (n <= capacity()) return true;
        const size_t s = size();
        const size_t added = n - s;
        return reserve(s + (s > added ? s : added));
    }

    // Appends `value` bitwise: the vector takes over whatever it owns (a
    // string buffer, a reference).
    bool push_back(const T& value) {
        if (end_ == capacityEnd_) {
            const size_t c = capacity();
            if (!reserve(c < 4 ? 4 : c * 2)) return false;
        }
        std::memcpy(static_cast<void*>(end_), static_cast<const void*>(&value), sizeof(T));
        ++end_;
        return true;
    }

private:
    T* begin_ = nullptr;
    T* end_ = nullptr;
    T* capacityEnd_ = nullptr;
};

static_assert(sizeof(GameVector<uint32_t>) == 3 * sizeof(void*), "vector layout: begin, end, capacity_end");

// ---- lists --------------------------------------------------------------------------------------

template <typename T, typename Alloc = StlNodeAllocator>
class GameList {
public:
    using Node = SC4ListNode<T>;
    using iterator = SC4ListIterator<T>;

    // Allocates the head node; check valid() (false if the allocator is
    // unavailable or out of memory).
    GameList() : root_(nullptr) {
        if (!Alloc::Available()) return;
        Node* head = static_cast<Node*>(Alloc::Allocate(static_cast<uint32_t>(sizeof(Node))));
        if (head == nullptr) return;
        head->next = head;
        head->prev = head;
        if constexpr (std::is_pointer_v<T>) {
            head->value = nullptr;
        } else {
            new (&head->value) T();
        }
        root_ = head;
    }
    GameList(const GameList&) = delete;
    GameList& operator=(const GameList&) = delete;

    // Only for lists we create; the game destroys the ones it passes us.
    ~GameList() {
        if (root_ == nullptr) return;
        Node* n = root_->next;
        while (n != root_) {
            Node* next = n->next;
            DestroyValue(n->value);
            Alloc::Free(n);
            n = next;
        }
        if constexpr (!std::is_pointer_v<T>) root_->value.~T();
        Alloc::Free(root_);
        root_ = nullptr;
    }

    bool valid() const { return root_ != nullptr; }

    // Appends a node from Alloc holding `value` (the list takes over whatever
    // the value owns, e.g. a reference). False if out of memory.
    bool push_back(const T& value) {
        Node* n = static_cast<Node*>(Alloc::Allocate(static_cast<uint32_t>(sizeof(Node))));
        if (n == nullptr) return false;
        new (&n->value) T(value);
        n->next = root_;
        n->prev = root_->prev;
        root_->prev->next = n;
        root_->prev = n;
        return true;
    }

    Node* head() const { return root_; }
    iterator begin() { return iterator(root_->next); }
    iterator end() { return iterator(root_); }

private:
    static void DestroyValue(T& v) {
        if constexpr (std::is_pointer_v<T>) {
            if constexpr (std::is_base_of_v<cIGZUnknown, std::remove_pointer_t<T>>) {
                if (v != nullptr) v->Release();  // the list's reference
            }
            v = nullptr;
        } else {
            v.~T();
        }
    }

    Node* root_;  // the only member: the game's list layout
};

static_assert(sizeof(GameList<cIGZUnknown*>) == sizeof(void*), "list layout: the head pointer");

// ---- hash sets ----------------------------------------------------------------------------------

// The game's hash_set<uint32_t> (docs/abi/allocators.md, hash_set): STLport's,
// declared with the default allocator. The object is 0x14 bytes: an empty
// functor member padded to 4 bytes, the bucket vector {begin, end,
// capacity_end} of Node*, and the element count. Nodes are 8 bytes {next,
// value} from Alloc; the bucket buffer is an Alloc block. The hash is the
// identity: v lives in the chain of bucket v mod n (n = end - begin).
//
// The game passes its sets by reference (flora slots 13 and 14): we insert,
// never construct or destroy them. Sets we create ourselves (tests) are
// destroyed by the destructor (nodes and buckets freed to Alloc).
// The bucket of a value (allocators.md, hash_set: the identity for
// hash_set<uint32_t>).
struct IdentityHashU32 {
    static uint32_t Of(uint32_t v) { return v; }
};

// The bucket hash of the game's hash_set<cISC4LotConfiguration*> (slots 9-11
// of the lot configuration manager). MEASURED, not specified (question PL20):
// glue21fix2-pin-1's growth-pass walk over a slot-9 set we filled with v mod n
// jumped between buckets exactly as an increment that recomputes the bucket
// as (v >> 2) mod n does (26 of 26 chain-end transitions; identity: 0), and
// cycled over 6 nodes forever. The walk only terminates when the nodes sit
// in the buckets the game's own hasher gives.
struct PointerHashU32 {
    static uint32_t Of(uint32_t v) { return v >> 2; }
};

template <typename Alloc = StlNodeAllocator, typename Hash = IdentityHashU32>
class GameHashSetU32 {
public:
    struct Node {
        Node* next;
        uint32_t value;
    };

    // Only for sets we create: buckets for the first prime >= hint.
    explicit GameHashSetU32(uint32_t hint) { Rehash(hint); }
    GameHashSetU32(const GameHashSetU32&) = delete;
    GameHashSetU32& operator=(const GameHashSetU32&) = delete;
    ~GameHashSetU32() {
        for (Node** b = begin_; b != end_; ++b) {
            Node* n = *b;
            while (n != nullptr) {
                Node* next = n->next;
                Alloc::Free(n);
                n = next;
            }
            *b = nullptr;
        }
        Alloc::Free(begin_);
        begin_ = end_ = capacityEnd_ = nullptr;
        count_ = 0;
    }

    uint32_t size() const { return count_; }
    size_t bucket_count() const { return static_cast<size_t>(end_ - begin_); }
    Node* bucket(size_t i) const { return begin_[i]; }
    Node** buckets() const { return begin_; }  // the bucket buffer (an Alloc block)

    bool contains(uint32_t v) const {
        const size_t n = bucket_count();
        if (n == 0) return false;
        for (Node* p = begin_[Hash::Of(v) % n]; p != nullptr; p = p->next) {
            if (p->value == v) return true;
        }
        return false;
    }

    // Visits every value in bucket order (slot-12-style release walks).
    template <typename F>
    void ForEach(F&& f) {
        for (Node** b = begin_; b != end_; ++b) {
            for (Node* n = *b; n != nullptr; n = n->next) f(n->value);
        }
    }

    // Empties the set: every node is freed to Alloc and every bucket head is
    // nulled, so the game's own destructor afterwards finds empty chains and
    // only frees the bucket buffer. Values are untouched: the caller has
    // already released whatever references it added per element.
    void ClearNodes() {
        for (Node** b = begin_; b != end_; ++b) {
            Node* n = *b;
            while (n != nullptr) {
                Node* next = n->next;
                Alloc::Free(n);
                n = next;
            }
            *b = nullptr;
        }
        count_ = 0;
    }

    // As the game's insert (0x00624DD0): resize(count + 1) first, then a value
    // already present is not inserted again, else a new node goes to the head
    // of its chain. True if inserted; false if present or out of memory.
    bool insert(uint32_t v) {
        if (!Rehash(count_ + 1)) return false;
        const size_t n = bucket_count();
        if (n == 0) return false;
        Node*& head = begin_[Hash::Of(v) % n];
        for (Node* p = head; p != nullptr; p = p->next) {
            if (p->value == v) return false;
        }
        Node* node = static_cast<Node*>(Alloc::Allocate(static_cast<uint32_t>(sizeof(Node))));
        if (node == nullptr) return false;
        node->value = v;
        node->next = head;
        head = node;
        ++count_;
        return true;
    }

    // The first bucket count >= h from STLport's prime table (the only counts
    // the game creates); 4294967291 above it.
    static uint32_t NextPrime(uint32_t h) {
        static const uint32_t kPrimes[28] = {53u,        97u,        193u,       389u,       769u,        1543u,
                                             3079u,      6151u,      12289u,     24593u,     49157u,      98317u,
                                             196613u,    393241u,    786433u,    1572869u,   3145739u,    6291469u,
                                             12582917u,  25165843u,  50331653u,  100663319u, 201326611u,  402653189u,
                                             805306457u, 1610612741u, 3221225473u, 4294967291u};
        for (uint32_t p : kPrimes) {
            if (p >= h) return p;
        }
        return kPrimes[27];
    }

private:
    // As the game's resize (0x004F0940): when `hint` exceeds the bucket count,
    // a new zeroed bucket buffer for the first prime >= hint, every node moved
    // (kept) to the head of its new chain, the old buffer freed to Alloc.
    bool Rehash(uint32_t hint) {
        const size_t old = bucket_count();
        if (hint <= old) return true;
        const uint32_t n = NextPrime(hint);
        if (n > 0x0FFFFFFFu / sizeof(Node*)) return false;
        Node** fresh = static_cast<Node**>(Alloc::Allocate(static_cast<uint32_t>(n * sizeof(Node*))));
        if (fresh == nullptr) return false;
        std::memset(static_cast<void*>(fresh), 0, n * sizeof(Node*));
        for (size_t i = 0; i < old; ++i) {
            Node* p = begin_[i];
            while (p != nullptr) {
                Node* next = p->next;
                Node*& head = fresh[Hash::Of(p->value) % n];
                p->next = head;
                head = p;
                p = next;
            }
            begin_[i] = nullptr;
        }
        Alloc::Free(begin_);
        begin_ = fresh;
        end_ = fresh + n;
        capacityEnd_ = fresh + n;
        return true;
    }

    uint32_t functors_ = 0;  // hasher, key-equal, key-extractor: empty, never read
    Node** begin_ = nullptr;
    Node** end_ = nullptr;
    Node** capacityEnd_ = nullptr;
    uint32_t count_ = 0;
};

static_assert(sizeof(void*) != 4 || sizeof(GameHashSetU32<>) == 0x14, "hash_set<uint32_t> is 0x14 bytes");

}  // namespace fc4
