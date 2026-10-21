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

#include "FC4Diag.h"

#include "FC4Autotest.h"
#include "FC4DiagContext.h"
#include "FC4Log.h"
#include "FC4Watchdog.h"

#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#include <windows.h>

#include <cstdio>
#include <cstring>

namespace fc4 {
namespace {

uintptr_t g_base = 0;
uintptr_t g_size = 0;
void* g_vehHandle = nullptr;
volatile LONG g_vehLogged = 0;
constexpr LONG kMaxVehLogs = 200;

// Ring of the last interface-method entries (single simulation thread).
constexpr unsigned kRing = 64;
const char* g_ring[kRing] = {};
DWORD g_ringTick[kRing] = {};
unsigned g_ringNext = 0;
unsigned long long g_ringTotal = 0;

bool InImage(uintptr_t addr) { return g_base != 0 && addr >= g_base && addr - g_base < g_size; }

void DumpCallRingWait(unsigned wait, const char* indent) {
    const unsigned long long total = g_ringTotal;
    const unsigned next = g_ringNext;
    const unsigned n = total < kRing ? static_cast<unsigned>(total) : kRing;
    LogWait(wait, "%slast %u of %llu FreeCity4 interface calls (oldest first):", indent, n, total);
    for (unsigned k = 0; k < n; ++k) {
        const unsigned i = (next + kRing - n + k) % kRing;
        const char* what = g_ring[i];
        LogWait(wait, "%s  [%lu ms] %s", indent, static_cast<unsigned long>(g_ringTick[i]), what ? what : "?");
    }
}

const char* CodeName(DWORD code) {
    switch (code) {
        case EXCEPTION_ACCESS_VIOLATION: return "ACCESS_VIOLATION";
        case EXCEPTION_ILLEGAL_INSTRUCTION: return "ILLEGAL_INSTRUCTION";
        case EXCEPTION_INT_DIVIDE_BY_ZERO: return "INT_DIVIDE_BY_ZERO";
        case EXCEPTION_STACK_OVERFLOW: return "STACK_OVERFLOW";
        case EXCEPTION_PRIV_INSTRUCTION: return "PRIV_INSTRUCTION";
        case EXCEPTION_BREAKPOINT: return "BREAKPOINT";
        case 0xE06D7363: return "C++ exception";
        case 0xC0000409: return "STACK_BUFFER_OVERRUN / fail-fast";
        default: return "other";
    }
}

// Formats and writes one exception line. `wait` = how long the log lock may
// be waited for (0 in the vectored handler: never block there).
void LogException(unsigned wait, const char* prefix, const EXCEPTION_RECORD* rec, const CONTEXT* ctx) {
    const uintptr_t addr = reinterpret_cast<uintptr_t>(rec->ExceptionAddress);
    const bool ours = InImage(addr);
    char detail[96] = "";
    if (rec->ExceptionCode == EXCEPTION_ACCESS_VIOLATION && rec->NumberParameters >= 2) {
        const ULONG_PTR kind = rec->ExceptionInformation[0];
        _snprintf_s(detail, sizeof(detail), _TRUNCATE, " (%s address 0x%08lX)",
                    kind == 0 ? "reading" : (kind == 1 ? "writing" : "executing"),
                    static_cast<unsigned long>(rec->ExceptionInformation[1]));
    }
    unsigned long eip = 0, esp = 0;
#if defined(_M_IX86)
    if (ctx != nullptr) {
        eip = ctx->Eip;
        esp = ctx->Esp;
    }
#else
    (void)ctx;
#endif
    LogWait(wait, "%s: code 0x%08lX %s%s at 0x%08lX (%s FreeCity4.dll base 0x%08lX%s0x%lX) eip=0x%08lX esp=0x%08lX",
            prefix, static_cast<unsigned long>(rec->ExceptionCode), CodeName(rec->ExceptionCode), detail,
            static_cast<unsigned long>(addr), ours ? "inside" : "OUTSIDE", static_cast<unsigned long>(g_base),
            ours ? " + " : ", image size ", static_cast<unsigned long>(ours ? addr - g_base : g_size), eip, esp);
}

// Faulting addresses already reported (lock-free claim; at most 2 reports per
// address, at most kMaxForeignSites addresses).
constexpr LONG kMaxForeignSites = 16;
constexpr LONG kReportsPerSite = 2;
volatile LONG g_siteAddr[kMaxForeignSites] = {};
volatile LONG g_siteCount[kMaxForeignSites] = {};

// Returns the report number for this address (1-based), or 0 if it must not
// be reported (over the per-site limit or no free slot).
LONG ClaimForeignReport(uintptr_t addr) {
    const LONG key = static_cast<LONG>(addr | 1u);  // never 0
    for (LONG i = 0; i < kMaxForeignSites; ++i) {
        LONG cur = g_siteAddr[i];
        if (cur == 0) cur = InterlockedCompareExchange(&g_siteAddr[i], key, 0);
        if (cur == 0 || cur == key) {
            const LONG n = InterlockedIncrement(&g_siteCount[i]);
            return n <= kReportsPerSite ? n : 0;
        }
    }
    return 0;
}

// Registers, then the return addresses on the faulting stack, into the
// executable or our image (who called what). Shared by the foreign and the
// in-image fault reports. Runs on the faulting thread: no allocation, no
// loader-lock calls, no waiting for the log lock (LogWait 0), and the stack
// is read only within the thread's own stack bounds.
void LogContextStack(const CONTEXT* c) {
#if defined(_M_IX86)
    if (c == nullptr) return;
    LogWait(0, "  registers: eax=0x%08lX ebx=0x%08lX ecx=0x%08lX edx=0x%08lX esi=0x%08lX edi=0x%08lX ebp=0x%08lX",
            static_cast<unsigned long>(c->Eax), static_cast<unsigned long>(c->Ebx),
            static_cast<unsigned long>(c->Ecx), static_cast<unsigned long>(c->Edx),
            static_cast<unsigned long>(c->Esi), static_cast<unsigned long>(c->Edi),
            static_cast<unsigned long>(c->Ebp));
    // The x87 state at the fault (FC4DiagContext.h): a stack full of
    // leftovers or the indefinite NaN points at a wrong return channel.
    char line[256];
    FormatContextFpu(c, line, sizeof(line));
    LogWait(0, "  %s", line);
    const NT_TIB* tib = reinterpret_cast<const NT_TIB*>(NtCurrentTeb());
    const uintptr_t top = reinterpret_cast<uintptr_t>(tib->StackBase);
    const uintptr_t esp = c->Esp;
    const uintptr_t* sp = reinterpret_cast<const uintptr_t*>(esp);
    // The first 8 raw dwords at esp (arguments of the faulting call),
    // read only within the thread's own stack.
    const bool espReadable = esp >= reinterpret_cast<uintptr_t>(tib->StackLimit) && esp < top &&
                             top - esp >= 8 * sizeof(uint32_t);
    FormatEspDwords(line, sizeof(line), static_cast<uint32_t>(esp),
                    espReadable ? reinterpret_cast<const uint32_t*>(esp) : nullptr);
    LogWait(0, "  %s", line);
    int found = 0;
    int ours = 0;
    if (esp >= reinterpret_cast<uintptr_t>(tib->StackLimit) && esp < top) {
        const unsigned words = static_cast<unsigned>((top - esp) / sizeof(uintptr_t));
        for (unsigned i = 0; i < 1024 && i < words && found < 24; ++i) {
            const uintptr_t v = sp[i];
            if (!(InImage(v) || InMainExe(v)) || !IsLikelyReturnAddress(v)) continue;
            char w[96];
            DescribeAddress(v, w, sizeof(w));
            LogWait(0, "  stack[esp+0x%03X] = %s", i * 4, w);
            ++found;
            if (InImage(v)) ++ours;
        }
    }
    if (found == 0) LogWait(0, "  no return address into the executable or FreeCity4.dll in the top 4 KiB of the stack");
    else if (ours == 0) LogWait(0, "  no return address into FreeCity4.dll among them");
#else
    (void)c;
#endif
}

// Runs on the faulting thread, which may hold any lock (the heap, the loader
// lock, a game lock). So: no allocation, no loader-lock calls (module names
// come from the watchdog's snapshot), no waiting for the log lock (LogWait 0
// writes through a second handle when the lock is busy), and the stack is
// read only within the thread's own stack bounds.
void LogForeign(EXCEPTION_POINTERS* ep, LONG report) {
    const uintptr_t addr = reinterpret_cast<uintptr_t>(ep->ExceptionRecord->ExceptionAddress);
    char where[96];
    DescribeAddress(addr, where, sizeof(where));
    char prefix[160];
    _snprintf_s(prefix, sizeof(prefix), _TRUNCATE, "first-chance exception OUTSIDE FreeCity4.dll in %s (report %ld of "
                "at most %ld for this address)", where, report, kReportsPerSite);
    LogException(0, prefix, ep->ExceptionRecord, ep->ContextRecord);
#if defined(_M_IX86)
    if (ep->ContextRecord != nullptr) {
        LogContextStack(ep->ContextRecord);
    }
#endif
    DumpCallRingWait(0, "  ");
}

LONG CALLBACK VectoredHandler(EXCEPTION_POINTERS* ep) {
    if (ep == nullptr || ep->ExceptionRecord == nullptr) return EXCEPTION_CONTINUE_SEARCH;
    const uintptr_t addr = reinterpret_cast<uintptr_t>(ep->ExceptionRecord->ExceptionAddress);
    // 0x406D1388 = thread naming; 0x40010006/0x4001000A = OutputDebugString.
    const DWORD code = ep->ExceptionRecord->ExceptionCode;
    if (code == EXCEPTION_STACK_OVERFLOW) {
        // Almost no stack left on this thread: count only.
        if (!InImage(addr)) NoteForeignException();
        return EXCEPTION_CONTINUE_SEARCH;
    }
    if (InImage(addr) && code != 0x406D1388 && code != 0x40010006 && code != 0x4001000A) {
        if (InterlockedIncrement(&g_vehLogged) <= kMaxVehLogs) {
            LogException(0, "first-chance exception in FreeCity4.dll", ep->ExceptionRecord, ep->ContextRecord);
            // The same register + return-address walk the foreign faults get:
            // an in-image AV needs its caller chain named to be fixable.
            LogContextStack(ep->ContextRecord);
        }
    } else if (!InImage(addr) && (code == EXCEPTION_ACCESS_VIOLATION || code == EXCEPTION_ILLEGAL_INSTRUCTION ||
                                  code == EXCEPTION_PRIV_INSTRUCTION || code == EXCEPTION_INT_DIVIDE_BY_ZERO ||
                                  code == 0xC0000409)) {
        NoteForeignException();
        const LONG report = ClaimForeignReport(addr);
        if (report != 0) LogForeign(ep, report);
        AutotestOnForeignException();  // verdict "corrupt" under autotest (after the report)
    }
    return EXCEPTION_CONTINUE_SEARCH;  // never swallow
}

struct Captured {
    EXCEPTION_RECORD record;
    CONTEXT context;
    bool valid;
};

int Filter(EXCEPTION_POINTERS* ep, Captured* out) {
    if (ep != nullptr && ep->ExceptionRecord != nullptr) {
        out->record = *ep->ExceptionRecord;
        if (ep->ContextRecord != nullptr) out->context = *ep->ContextRecord;
        out->valid = true;
    }
    return EXCEPTION_EXECUTE_HANDLER;
}

// No C++ objects with destructors in this function (required for __try).
bool RunGuarded(void (*fn)(void*), void* ctx, Captured* captured) {
    __try {
        fn(ctx);
        return true;
    } __except (Filter(GetExceptionInformation(), captured)) {
        return false;
    }
}

}  // namespace

void DiagAttach(void* moduleBase) {
    g_base = reinterpret_cast<uintptr_t>(moduleBase);
    const auto* dos = static_cast<const IMAGE_DOS_HEADER*>(moduleBase);
    if (dos != nullptr && dos->e_magic == IMAGE_DOS_SIGNATURE) {
        const auto* nt = reinterpret_cast<const IMAGE_NT_HEADERS*>(static_cast<const char*>(moduleBase) + dos->e_lfanew);
        if (nt->Signature == IMAGE_NT_SIGNATURE) g_size = nt->OptionalHeader.SizeOfImage;
    }
    g_vehHandle = AddVectoredExceptionHandler(1, &VectoredHandler);
    Log("diagnostics: image 0x%08lX..0x%08lX, vectored handler %s", static_cast<unsigned long>(g_base),
        static_cast<unsigned long>(g_base + g_size), g_vehHandle ? "installed" : "NOT installed");
}

void DiagDetach() {
    if (g_vehHandle != nullptr) {
        RemoveVectoredExceptionHandler(g_vehHandle);
        g_vehHandle = nullptr;
    }
}

void CallTrace(const char* what, unsigned* counter) {
    NoteInterfaceCall();
    if (MainThreadId() == 0 || OnMainThread()) {
        // The ring records the simulation thread only (single writer); before
        // the simulator's Init names that thread, every caller.
        g_ring[g_ringNext] = what;
        g_ringTick[g_ringNext] = GetTickCount();
        g_ringNext = (g_ringNext + 1) % kRing;
        ++g_ringTotal;
    }
    if (counter != nullptr && *counter < 20) {
        ++*counter;
        if (LogLevel() >= kLogVerbose) LogVerboseImpl("call %s (#%u)", what, *counter);
    } else if (LogLevel() >= kLogVerbose) {
        NoteSuppressed();
    }
}

void DumpCallRing(const char* indent) { DumpCallRingWait(1000, indent); }

CallScope::CallScope(const char* what, unsigned* counter) : activity_(what) { CallTrace(what, counter); }

uintptr_t ImageBase() { return g_base; }
uintptr_t ImageSize() { return g_size; }

bool SehGuard(const char* what, void (*fn)(void* ctx), void* ctx) {
    Captured captured{};
    if (RunGuarded(fn, ctx, &captured)) return true;
    char prefix[160];
    _snprintf_s(prefix, sizeof(prefix), _TRUNCATE, "STRUCTURED EXCEPTION caught in %s", what);
    if (captured.valid) {
        LogException(1000, prefix, &captured.record, &captured.context);
    } else {
        Log("%s (no exception record)", prefix);
    }
    return false;
}

}  // namespace fc4
