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

// What the history warehouse (cFC4History.h) and the 24-hour clock
// (cFC4Clock24.h) share: the lock around their cores and the test seam's
// hooks (op 54, FC4TestSeam.h).
//
// The lock (foundations §19): the game calls these objects from other
// threads too (the clock's getters from audio and rendering), and the cores
// sit in a RefCell, so every core call runs under one critical section per
// object. The thread holding it never calls the game: the effects a core
// call asks for (subscriptions, agents, the framework, cheats, posts) are
// queued while it runs and carried out, in order, once the outermost section
// is left; the values a call needs from the game (the simulation date) are
// read before it is entered. So another thread waits at most for one core
// call (plus the game stream of a save or a load, which runs on the main
// thread), and nothing can wait on it in turn.
#pragma once

#include "fc4.h"

#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#include <windows.h>

#include <atomic>
#include <cstdint>
#include <functional>
#include <vector>

class cIGZMessage2;
class cIGZMessageTarget2;
class cISC4City;

// ---- test seam op 54 ----------------------------------------------------------------------------------
//
// What the two classes and their shadows call in the game, answered by a test
// instead (the DLL loaded without a framework). Every callback may be null
// (the call does nothing; a query answers 0 / false / none).
struct FC4HistClockHooks {
    uint32_t struct_size;  // sizeof(FC4HistClockHooks)
    void* ctx;
    // The simulation date number (history Store, the yearly pass), instead of
    // the city's simulator (slot 8).
    uint32_t (*sim_date)(void* ctx);
    // The message server's AddNotification (add) / RemoveNotification.
    void (*notification)(void* ctx, cIGZMessageTarget2* target, uint32_t type, bool add);
    // The simulator's AddAgent (add: level, name, flags) / RemoveAgentAllLevels.
    void (*agent)(void* ctx, cIGZMessageTarget2* target, uint32_t level, const char* name, uint32_t flags, bool add);
    // MessageSend of a standard message (the history record; the hook may
    // deliver it, as the server does) and MessagePost (the clock; the hook
    // may keep it with an AddRef).
    void (*send)(void* ctx, cIGZMessage2* msg);
    void (*post)(void* ctx, cIGZMessage2* msg);
    // The framework, for `service` (the clock's cIGZSystemService): op 0
    // AddSystemService, 1 AddToOnIdle, 2 RemoveFromOnIdle, 3 RemoveSystemService.
    void (*framework)(void* ctx, void* service, uint32_t op);
    // The cheat code manager: op 0 asks whether one exists (the result), 1
    // RegisterCheatCode(id, name), 2 AddNotification2(target, id), 3
    // RemoveNotification2(target, id), 4 UnregisterCheatCode(id).
    bool (*cheat)(void* ctx, uint32_t op, uint32_t id, const char* name);
    // The clock's real-time timer, in microseconds.
    uint64_t (*now_micros)(void* ctx);
    // The "Main Simulator" exemplar's clock properties; false = no exemplar.
    bool (*clock_tuning)(void* ctx, fc4_clock24_tuning* out);
    // The city the shadows compare (borrowed; null: none).
    cISC4City* (*city)(void* ctx);
};

// Op 54's input: `op` 0 installs `hooks` (copied; null: the game again) and
// returns 1; the other ops are the shadows' (FC4HistoryShadow.h,
// FC4Clock24Shadow.h).
struct FC4HistClockTestInput {
    uint32_t op;
    const FC4HistClockHooks* hooks;
    void* arg;  // the op's own input
};
intptr_t HistClockTestSeam(const FC4HistClockTestInput* in);

namespace fc4 {

// The installed hooks, or null (the game). Main thread (tests only).
const FC4HistClockHooks* HistClockHooks();

// One critical section with the queue of game calls its holder asked for.
class CoreLock {
public:
    CoreLock();
    ~CoreLock();
    CoreLock(const CoreLock&) = delete;
    CoreLock& operator=(const CoreLock&) = delete;

    // The thread the object was created on (the class store's, the main one).
    uint32_t MainThread() const { return mainTid_; }
    // Off-main calls seen (each method logged once through fc4::NoteOffMainThreadCall).
    uint32_t OffMainCalls() const { return offMain_.load(); }

    // A game call to make once the outermost section is left (only while the
    // lock is held; immediately otherwise).
    void Defer(std::function<void()> op);

    // Synchronous native callbacks can call this owner again from another
    // thread. Only use while this thread owns a Section, with core guards
    // released. Preserve this call's deferred operations across reentry.
    class Unlocked {
    public:
        explicit Unlocked(CoreLock& lock);
        ~Unlocked();
        Unlocked(const Unlocked&) = delete;
        Unlocked& operator=(const Unlocked&) = delete;
    private:
        CoreLock& l_;
        int depth_;
        std::vector<std::function<void()>> pending_;
    };

    class Section {
    public:
        Section(CoreLock& lock, const char* what);
        ~Section();
        Section(const Section&) = delete;
        Section& operator=(const Section&) = delete;

    private:
        CoreLock& l_;
    };

private:
    CRITICAL_SECTION cs_;
    uint32_t mainTid_ = 0;
    int depth_ = 0;
    std::atomic<uint32_t> offMain_{0};
    std::vector<std::function<void()>> pending_;
};

// The caller's x87 precision as the history queries take it (history.md
// §4.7, foundations §18): 24 for PC = 24-bit, 53 otherwise (HW10).
uint32_t CallerPrecisionBits();

// Sets the x87 precision control to 24 or 53 bits for its lifetime and puts
// the caller's control word back (the shadows' queries at both precisions).
class PrecisionScope {
public:
    explicit PrecisionScope(uint32_t bits);
    ~PrecisionScope();
    PrecisionScope(const PrecisionScope&) = delete;
    PrecisionScope& operator=(const PrecisionScope&) = delete;

private:
    uint16_t saved_ = 0;
};

// In-memory core streams (fc4_ostream / fc4_istream) for the shadows: a core
// record written to bytes, or read from the bytes a game object wrote.
class MemOStream {
public:
    MemOStream();
    const fc4_ostream* Get() const { return &s_; }
    std::vector<uint8_t> bytes;

private:
    fc4_ostream s_{};
};

class MemIStream {
public:
    explicit MemIStream(const std::vector<uint8_t>& b);
    const fc4_istream* Get() const { return &s_; }
    size_t Remaining() const { return pos_ <= b_.size() ? b_.size() - pos_ : 0; }
    bool Take(void* out, size_t n);
    bool Bad() const { return bad_; }

private:
    const std::vector<uint8_t>& b_;
    size_t pos_ = 0;
    bool bad_ = false;
    fc4_istream s_{};
};

}  // namespace fc4
