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

#include "FC4HistClock.h"

#include "FC4Clock24Shadow.h"
#include "FC4Fpu.h"
#include "FC4HistoryShadow.h"
#include "FC4Watchdog.h"

#include <cstring>
#include <utility>

namespace fc4 {
namespace {

bool g_hooksOn = false;
FC4HistClockHooks g_hooks{};

}  // namespace

const FC4HistClockHooks* HistClockHooks() { return g_hooksOn ? &g_hooks : nullptr; }

CoreLock::CoreLock() : mainTid_(GetCurrentThreadId()) { InitializeCriticalSection(&cs_); }

CoreLock::~CoreLock() { DeleteCriticalSection(&cs_); }

CoreLock::Unlocked::Unlocked(CoreLock& lock) : l_(lock), depth_(lock.depth_) {
    pending_.swap(l_.pending_);
    l_.depth_ = 0;
    for (int i = 0; i < depth_; ++i) LeaveCriticalSection(&l_.cs_);
}

CoreLock::Unlocked::~Unlocked() {
    for (int i = 0; i < depth_; ++i) EnterCriticalSection(&l_.cs_);
    l_.depth_ = depth_;
    l_.pending_.swap(pending_);
}

void CoreLock::Defer(std::function<void()> op) {
    // depth_ is only non-zero for the thread inside the section (owner check
    // through the critical section's own owner field would be private API).
    if (TryEnterCriticalSection(&cs_)) {
        const bool held = depth_ > 0;  // re-entered by its holder: this thread is inside a section
        if (held) pending_.push_back(std::move(op));
        LeaveCriticalSection(&cs_);
        if (held) return;
    }
    op();
}

CoreLock::Section::Section(CoreLock& lock, const char* what) : l_(lock) {
    if (GetCurrentThreadId() != l_.mainTid_ && l_.mainTid_ != 0) {
        ++l_.offMain_;
        NoteOffMainThreadCall(what);
    }
    EnterCriticalSection(&l_.cs_);
    ++l_.depth_;
}

CoreLock::Section::~Section() {
    std::vector<std::function<void()>> ops;
    if (--l_.depth_ == 0) ops.swap(l_.pending_);
    LeaveCriticalSection(&l_.cs_);
    for (auto& op : ops) op();
}

uint32_t CallerPrecisionBits() { return ((X87ControlWord() >> 8) & 3u) == 0 ? 24u : 53u; }

namespace {

void LoadControlWord(uint16_t cw) {
#if defined(_M_IX86)
    __asm fldcw cw
#else
    (void)cw;
#endif
}

}  // namespace

PrecisionScope::PrecisionScope(uint32_t bits) : saved_(X87ControlWord()) {
    // PC (bits 8-9): 00 = 24-bit, 10 = 53-bit.
    const uint16_t pc = bits == 24 ? 0x0000 : 0x0200;
    LoadControlWord(static_cast<uint16_t>((saved_ & ~0x0300) | pc));
}

PrecisionScope::~PrecisionScope() { LoadControlWord(saved_); }

namespace {

MemOStream* Out(void* u) { return static_cast<MemOStream*>(u); }
bool PutRaw(void* u, const void* p, size_t n) {
    const uint8_t* b = static_cast<const uint8_t*>(p);
    Out(u)->bytes.insert(Out(u)->bytes.end(), b, b + n);
    return true;
}
bool PutU8(void* u, uint8_t v) { return PutRaw(u, &v, 1); }
bool PutU16(void* u, uint16_t v) { return PutRaw(u, &v, 2); }
bool PutU32(void* u, uint32_t v) { return PutRaw(u, &v, 4); }
bool PutU64(void* u, uint64_t v) { return PutRaw(u, &v, 8); }
bool PutF32(void* u, float v) { return PutRaw(u, &v, 4); }
bool PutF64(void* u, double v) { return PutRaw(u, &v, 8); }
bool PutBytes(void* u, const uint8_t* d, size_t n) { return PutRaw(u, d, n); }
int32_t OutError(void*) { return 0; }

MemIStream* In(void* u) { return static_cast<MemIStream*>(u); }
bool GetU8(void* u, uint8_t* o) { return In(u)->Take(o, 1); }
bool GetU16(void* u, uint16_t* o) { return In(u)->Take(o, 2); }
bool GetU32(void* u, uint32_t* o) { return In(u)->Take(o, 4); }
bool GetU64(void* u, uint64_t* o) { return In(u)->Take(o, 8); }
bool GetF32(void* u, float* o) { return In(u)->Take(o, 4); }
bool GetF64(void* u, double* o) { return In(u)->Take(o, 8); }
bool GetBytes(void* u, uint8_t* o, size_t n) { return In(u)->Take(o, n); }
bool Skip(void* u, size_t n) { return In(u)->Take(nullptr, n); }
int32_t InError(void* u) { return In(u)->Bad() ? 1 : 0; }

}  // namespace

MemOStream::MemOStream() {
    s_.struct_size = sizeof(s_);
    s_.user = this;
    s_.put_u8 = &PutU8;
    s_.put_u16 = &PutU16;
    s_.put_u32 = &PutU32;
    s_.put_u64 = &PutU64;
    s_.put_f32 = &PutF32;
    s_.put_f64 = &PutF64;
    s_.put_bytes = &PutBytes;
    s_.error = &OutError;  // strings and references: not used by these records (null: refused)
}

MemIStream::MemIStream(const std::vector<uint8_t>& b) : b_(b) {
    s_.struct_size = sizeof(s_);
    s_.user = this;
    s_.get_u8 = &GetU8;
    s_.get_u16 = &GetU16;
    s_.get_u32 = &GetU32;
    s_.get_u64 = &GetU64;
    s_.get_f32 = &GetF32;
    s_.get_f64 = &GetF64;
    s_.get_bytes = &GetBytes;
    s_.skip = &Skip;
    s_.error = &InError;
}

// A failed read leaves its destination unchanged and fails every later read
// (foundations §2.1).
bool MemIStream::Take(void* out, size_t n) {
    if (bad_ || n > Remaining()) {
        bad_ = true;
        return false;
    }
    if (out != nullptr && n > 0) std::memcpy(out, b_.data() + pos_, n);
    pos_ += n;
    return true;
}

}  // namespace fc4

intptr_t HistClockTestSeam(const FC4HistClockTestInput* in) {
    if (in == nullptr) return -1;
    switch (in->op) {
        case 0:
            if (in->hooks != nullptr && in->hooks->struct_size >= sizeof(FC4HistClockHooks)) {
                fc4::g_hooks = *in->hooks;
                fc4::g_hooksOn = true;
            } else {
                fc4::g_hooks = FC4HistClockHooks{};
                fc4::g_hooksOn = false;
            }
            return 1;
        case 1:  // the history shadow (FC4HistoryShadow.h)
            return HistoryShadowTest(static_cast<FC4HistoryShadowTestInput*>(in->arg));
        case 2:  // the 24-hour clock shadow (FC4Clock24Shadow.h)
            return Clock24ShadowTest(static_cast<FC4Clock24ShadowTestInput*>(in->arg));
        default:
            return -1;
    }
}
