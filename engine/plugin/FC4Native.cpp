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

// The fc4native.dll loader; see FC4Native.h.

#include "FC4Native.h"

#include "FC4Config.h"
#include "FC4Log.h"
#include "compat/FC4Compat.h"

#include <windows.h>
#include <tlhelp32.h>

#include <cstdint>
#include <cstdio>
#include <cstring>
#include <string>
#include <vector>

namespace fc4 {
namespace {

// The table fc4native.dll exports as `fc4native_ports` (engine/native/src/port.rs, `Port` / `PortTable`).
struct NativePort {
    uint32_t address;   // the original's entry in the exe
    const void* entry;  // the replacement (the fc4p_<address> export)
    const char* owner;  // e.g. "landvalue"
    const char* name;   // e.g. "cSC4LandValueSimulator::TractIsZoned"
};
static_assert(sizeof(NativePort) == 16, "fc4native port entry layout");

struct NativePortTable {
    uint32_t magic;  // "FC4P" in memory order
    uint32_t version;
    uint32_t count;
    uint32_t entrySize;
    const NativePort* entries;
};

constexpr uint32_t kTableMagic = 0x50344346;  // 'F' 'C' '4' 'P'
constexpr uint32_t kTableVersion = 1;
// The build the ports' addresses belong to (compat table: 1.1.641, GOG).
constexpr uint32_t kPortsTimeDateStamp = 0x4C12BFB7;
constexpr uint32_t kPortsSizeOfImage = 0x007C8000;

std::string Trimmed(const std::string& s) {
    size_t a = 0, b = s.size();
    while (a < b && (s[a] == ' ' || s[a] == '\t')) ++a;
    while (b > a && (s[b - 1] == ' ' || s[b - 1] == '\t')) --b;
    return s.substr(a, b - a);
}

std::string Lowered(std::string s) {
    for (char& c : s) {
        if (c >= 'A' && c <= 'Z') c = static_cast<char>(c - 'A' + 'a');
    }
    return s;
}

std::vector<std::string> Items(const std::string& list) {
    std::vector<std::string> out;
    size_t i = 0;
    while (i < list.size()) {
        while (i < list.size() && std::strchr(",; \t", list[i]) != nullptr) ++i;
        size_t j = i;
        while (j < list.size() && std::strchr(",; \t", list[j]) == nullptr) ++j;
        if (j > i) out.push_back(Lowered(list.substr(i, j - i)));
        i = j;
    }
    return out;
}

// "006bd540" or "0x006bd540" -> true and the address.
bool ParseAddress(std::string item, uint32_t* address) {
    if (item.size() == 10 && item[0] == '0' && item[1] == 'x') item.erase(0, 2);
    if (item.size() != 8) return false;
    uint32_t v = 0;
    for (char c : item) {
        uint32_t d;
        if (c >= '0' && c <= '9') d = static_cast<uint32_t>(c - '0');
        else if (c >= 'a' && c <= 'f') d = static_cast<uint32_t>(c - 'a' + 10);
        else return false;
        v = v << 4 | d;
    }
    *address = v;
    return true;
}

std::string DefaultDllPath() {
    const std::string self = ModulePath();
    const size_t slash = self.find_last_of("\\/");
    return slash == std::string::npos ? std::string("fc4native.dll") : self.substr(0, slash + 1) + "fc4native.dll";
}

bool ImageRange(HMODULE module, uintptr_t* base, uint32_t* size) {
    const auto* dos = reinterpret_cast<const IMAGE_DOS_HEADER*>(module);
    if (dos == nullptr || dos->e_magic != IMAGE_DOS_SIGNATURE) return false;
    const auto* nt = reinterpret_cast<const IMAGE_NT_HEADERS32*>(reinterpret_cast<const char*>(module) + dos->e_lfanew);
    if (nt->Signature != IMAGE_NT_SIGNATURE) return false;
    *base = reinterpret_cast<uintptr_t>(module);
    *size = nt->OptionalHeader.SizeOfImage;
    return true;
}

// The 5 bytes at `address` are mapped executable image code of the exe.
bool IsExeCode(uint32_t address, const compat::ExecutableVersion* v) {
    if (address < v->imageBase || address + 5 > v->imageBase + v->sizeOfImage) return false;
    MEMORY_BASIC_INFORMATION m{};
    if (VirtualQuery(reinterpret_cast<const void*>(static_cast<uintptr_t>(address)), &m, sizeof(m)) == 0) return false;
    const DWORD p = m.Protect & 0xFF;
    const bool exec = p == PAGE_EXECUTE || p == PAGE_EXECUTE_READ || p == PAGE_EXECUTE_READWRITE ||
                      p == PAGE_EXECUTE_WRITECOPY;
    return m.State == MEM_COMMIT && m.Type == MEM_IMAGE && exec &&
           reinterpret_cast<uintptr_t>(m.BaseAddress) + m.RegionSize >= static_cast<uintptr_t>(address) + 5;
}

// One enabled port, checked and waiting for its jump.
struct Pending {
    uint32_t address = 0;
    const void* entry = nullptr;
    const char* owner = "";
    const char* name = "?";
    char exportName[16] = {};
    bool written = false;
    DWORD error = 0;
    uint8_t replaced[5] = {};
};

// The game's other threads, suspended while the jumps are written; none may be inside the 5 bytes of any
// entry (as FC4Schedule.cpp does for its hooks). Nothing here allocates or logs, since a suspended thread may
// hold the heap or the log.
class PausedThreads {
public:
    explicit PausedThreads(const std::vector<Pending>& pending) {
        const HANDLE snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0);
        if (snapshot == INVALID_HANDLE_VALUE) {
            ok_ = false;
            return;
        }
        THREADENTRY32 t{};
        t.dwSize = sizeof(t);
        for (BOOL more = Thread32First(snapshot, &t); more; more = Thread32Next(snapshot, &t)) {
            if (t.th32OwnerProcessID != GetCurrentProcessId() || t.th32ThreadID == GetCurrentThreadId()) continue;
            if (count_ == kMax) {
                ok_ = false;
                break;
            }
            const HANDLE h = OpenThread(THREAD_SUSPEND_RESUME | THREAD_GET_CONTEXT, FALSE, t.th32ThreadID);
            if (h == nullptr) continue;  // a thread that has just exited
            if (SuspendThread(h) == static_cast<DWORD>(-1)) {
                CloseHandle(h);
                continue;
            }
            handles_[count_++] = h;
            CONTEXT c{};
            c.ContextFlags = CONTEXT_CONTROL;
            if (!GetThreadContext(h, &c)) {
                ok_ = false;
                break;
            }
            for (const Pending& p : pending) {
                if (c.Eip >= p.address && c.Eip < p.address + 5) ok_ = false;
            }
            if (!ok_) break;
        }
        CloseHandle(snapshot);
    }
    ~PausedThreads() {
        for (unsigned i = count_; i > 0; --i) {
            ResumeThread(handles_[i - 1]);
            CloseHandle(handles_[i - 1]);
        }
    }
    PausedThreads(const PausedThreads&) = delete;
    PausedThreads& operator=(const PausedThreads&) = delete;
    bool ok() const { return ok_; }

private:
    static constexpr unsigned kMax = 256;
    HANDLE handles_[kMax] = {};
    unsigned count_ = 0;
    bool ok_ = true;
};

// Writes `jmp p.entry` (E9 rel32) at p.address; p.replaced receives the 5 bytes it overwrote, p.error the
// VirtualProtect error. No allocation and no logging (other threads are suspended).
void WriteJump(Pending& p) {
    auto* at = reinterpret_cast<uint8_t*>(static_cast<uintptr_t>(p.address));
    DWORD old = 0;
    if (!VirtualProtect(at, 5, PAGE_EXECUTE_READWRITE, &old)) {
        p.error = GetLastError();
        return;
    }
    std::memcpy(p.replaced, at, 5);
    const int32_t rel =
        static_cast<int32_t>(reinterpret_cast<uintptr_t>(p.entry) - (static_cast<uintptr_t>(p.address) + 5));
    uint8_t jump[5] = {0xE9};
    std::memcpy(jump + 1, &rel, 4);
    std::memcpy(at, jump, 5);
    DWORD ignored = 0;
    VirtualProtect(at, 5, old, &ignored);
    FlushInstructionCache(GetCurrentProcess(), at, 5);
    p.written = true;
}

}  // namespace

void NativeStartup() {
    const char* source = "";
    const std::string list = Trimmed(Setting("native functions", "FC4_NATIVE_FUNCTIONS", &source));
    if (list.empty()) return;  // not configured: nothing loaded, nothing written, nothing logged
    Log("native: functions = '%s' (%s)", list.c_str(), source);

    const compat::ExecutableVersion* v = compat::IdentifiedVersion();
    if (v == nullptr || v->timeDateStamp != kPortsTimeDateStamp || v->sizeOfImage != kPortsSizeOfImage) {
        Log("native: NOT loaded: the executable is not the build the ports are written for (%s)",
            v != nullptr ? v->name : "not identified");
        return;
    }

    const char* dllSource = "";
    std::string path = Trimmed(Setting("native dll", "FC4_NATIVE_DLL", &dllSource));
    if (path.empty()) {
        path = DefaultDllPath();
        dllSource = "next to FreeCity4.dll";
    }
    HMODULE module = LoadLibraryA(path.c_str());
    if (module == nullptr) {
        Log("native: NOT loaded: LoadLibrary('%s') failed, error %lu", path.c_str(),
            static_cast<unsigned long>(GetLastError()));
        return;
    }
    uintptr_t moduleBase = 0;
    uint32_t moduleSize = 0;
    const auto* table = reinterpret_cast<const NativePortTable*>(GetProcAddress(module, "fc4native_ports"));
    if (table == nullptr || !ImageRange(module, &moduleBase, &moduleSize) || table->magic != kTableMagic ||
        table->version != kTableVersion || table->entrySize != sizeof(NativePort) || table->entries == nullptr) {
        Log("native: NOT used: '%s' has no valid port table (fc4native_ports)", path.c_str());
        FreeLibrary(module);
        return;
    }
    Log("native: loaded %s (%s) at 0x%08lX, %lu ports", path.c_str(), dllSource,
        static_cast<unsigned long>(moduleBase), static_cast<unsigned long>(table->count));

    const std::vector<std::string> items = Items(list);
    bool all = false;
    for (const std::string& item : items) all = all || item == "all";
    std::vector<bool> itemUsed(items.size(), false);

    unsigned installed = 0, enabled = 0;
    std::vector<Pending> pending;
    pending.reserve(table->count);
    for (uint32_t i = 0; i < table->count; ++i) {
        const NativePort& p = table->entries[i];
        const std::string owner = p.owner != nullptr ? p.owner : "";
        const char* name = p.name != nullptr ? p.name : "?";
        bool on = all;
        for (size_t k = 0; k < items.size(); ++k) {
            uint32_t a = 0;
            if (items[k] == owner || (ParseAddress(items[k], &a) && a == p.address)) {
                on = true;
                itemUsed[k] = true;
            }
        }
        if (!on) continue;
        ++enabled;
        char exportName[16];
        std::snprintf(exportName, sizeof(exportName), "fc4p_%08lx", static_cast<unsigned long>(p.address));
        const void* exported = reinterpret_cast<const void*>(GetProcAddress(module, exportName));
        const auto entry = reinterpret_cast<uintptr_t>(p.entry);
        if (exported == nullptr || exported != p.entry || entry < moduleBase || entry >= moduleBase + moduleSize) {
            Log("native: WARNING 0x%08lX %s (%s): export %s missing or not the table's entry; not installed",
                static_cast<unsigned long>(p.address), name, owner.c_str(), exportName);
            continue;
        }
        if (!IsExeCode(p.address, v)) {
            Log("native: WARNING 0x%08lX %s (%s): not executable code of the exe; not installed",
                static_cast<unsigned long>(p.address), name, owner.c_str());
            continue;
        }
        const auto* at = reinterpret_cast<const uint8_t*>(static_cast<uintptr_t>(p.address));
        if (at[0] == 0xE9) {
            Log("native: WARNING 0x%08lX %s (%s): the entry is already a jump; not installed",
                static_cast<unsigned long>(p.address), name, owner.c_str());
            continue;
        }
        Pending w;
        w.address = p.address;
        w.entry = p.entry;
        w.owner = p.owner != nullptr ? p.owner : "";
        w.name = name;
        std::memcpy(w.exportName, exportName, sizeof(exportName));
        pending.push_back(w);
    }
    // All jumps at once, with the game's other threads suspended and none inside an entry (a few retries).
    bool paused = pending.empty();
    for (int attempt = 0; attempt < 50 && !paused; ++attempt) {
        {
            PausedThreads threads(pending);
            if (threads.ok()) {
                paused = true;
                for (Pending& w : pending) WriteJump(w);
            }
        }
        if (!paused) Sleep(1);
    }
    if (!paused) {
        Log("native: WARNING could not pause the game's other threads outside the entries; nothing installed");
    }
    for (const Pending& w : pending) {
        if (!w.written) {
            if (paused) {
                Log("native: WARNING 0x%08lX %s (%s): VirtualProtect failed, error %lu; not installed",
                    static_cast<unsigned long>(w.address), w.name, w.owner, static_cast<unsigned long>(w.error));
            }
            continue;
        }
        ++installed;
        Log("native: jump installed 0x%08lX -> %s at 0x%08lX (%s, %s); entry bytes were %02X %02X %02X %02X %02X",
            static_cast<unsigned long>(w.address), w.exportName,
            static_cast<unsigned long>(reinterpret_cast<uintptr_t>(w.entry)), w.owner, w.name, w.replaced[0],
            w.replaced[1], w.replaced[2], w.replaced[3], w.replaced[4]);
    }
    for (size_t k = 0; k < items.size(); ++k) {
        if (!itemUsed[k] && items[k] != "all") {
            Log("native: WARNING '%s' in functions matches no port (ignored)", items[k].c_str());
        }
    }
    Log("native: %u of %u enabled ports installed (%lu in the table)", installed, enabled,
        static_cast<unsigned long>(table->count));
}

}  // namespace fc4
