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

#include "FC4FormatsShadow.h"

#include "FC4Config.h"
#include "FC4ExemplarFactory.h"
#include "FC4Log.h"
#include "FC4Properties.h"
#include "FC4RefAudit.h"
#include "FC4Checked.h"
#include "fc4.h"

#include "GZServPtrs.h"
#include "cGZPersistResourceKey.h"
#include "cIGZPersistResourceManager.h"
#include "cISCPropertyHolder.h"
#include "cISCResExemplar.h"
#include "cISCResExemplarCohort.h"

#include <windows.h>

#include <algorithm>
#include <cstdarg>
#include <cstring>
#include <string>
#include <vector>

namespace fc4 {
namespace {

constexpr uint32_t kExemplarType = 0x6534284A;  // data-formats §8

bool g_enabled = false;
bool g_done = false;
long g_maxPerFile = 0;
long g_lines = 8;
long g_shown = 0;

struct Totals {
    uint32_t files = 0, unreadable = 0, entries = 0, compared = 0, exact = 0, differ = 0, coreFail = 0,
             gameMissing = 0, props = 0;
};

void Detail(const char* fmt, ...) {
    if (g_shown++ >= g_lines) return;
    char buf[512];
    va_list ap;
    va_start(ap, fmt);
    _vsnprintf_s(buf, sizeof(buf), _TRUNCATE, fmt, ap);
    va_end(ap);
    Log("formats shadow (cont.) %s", buf);
}

std::string Utf8(const std::wstring& w) {
    if (w.empty()) return {};
    const int n = WideCharToMultiByte(CP_UTF8, 0, w.c_str(), static_cast<int>(w.size()), nullptr, 0, nullptr, nullptr);
    std::string s(static_cast<size_t>(n > 0 ? n : 0), '\0');
    if (n > 0) WideCharToMultiByte(CP_UTF8, 0, w.c_str(), static_cast<int>(w.size()), &s[0], n, nullptr, nullptr);
    return s;
}

// [choice] The install folder is the parent of the folder holding the game
// executable (<install>\Apps\SimCity 4.exe); §3.1 step 1 reads its *.dat.
std::wstring InstallDir() {
    wchar_t buf[MAX_PATH * 2];
    const DWORD n = GetModuleFileNameW(nullptr, buf, static_cast<DWORD>(sizeof(buf) / sizeof(buf[0])));
    if (n == 0 || n >= sizeof(buf) / sizeof(buf[0])) return {};
    std::wstring p(buf, n);
    for (int up = 0; up < 2; ++up) {
        const size_t cut = p.find_last_of(L"\\/");
        if (cut == std::wstring::npos) return {};
        p.resize(cut);
    }
    return p;
}

void AppendBytes(void* ctx, const uint8_t* data, size_t len) {
    static_cast<std::vector<uint8_t>*>(ctx)->assign(data, data + len);
}

std::vector<uint8_t> VariantBytes(const fc4_props* p, uint32_t id) {
    std::vector<uint8_t> out;
    if (!fc4_props_write_variant(p, id, AppendBytes, &out)) out.clear();
    return out;
}

// True when the file starts with the DBPF magic "DBPF" (data-formats §2.1).
bool HasDbpfMagic(const std::wstring& path) {
    HANDLE f = CreateFileW(path.c_str(), GENERIC_READ, FILE_SHARE_READ | FILE_SHARE_WRITE, nullptr, OPEN_EXISTING,
                           FILE_ATTRIBUTE_NORMAL, nullptr);
    if (f == INVALID_HANDLE_VALUE) return false;
    char magic[4] = {};
    DWORD got = 0;
    const BOOL ok = ReadFile(f, magic, 4, &got, nullptr);
    CloseHandle(f);
    return ok && got == 4 && std::memcmp(magic, "DBPF", 4) == 0;
}

void CompareFile(const std::wstring& path, const std::string& name, Totals& t) {
    const std::string u8 = Utf8(path);
    fc4_dbpf* pkg = fc4_dbpf_open(reinterpret_cast<const uint8_t*>(u8.data()), u8.size());
    ++t.files;
    if (pkg == nullptr) {
        ++t.unreadable;
        // [choice] A *.dat whose first four bytes are not "DBPF" (§2.1; the GOG installer's unins000.dat) is
        // listed, not a difference: nothing of it reaches the game's exemplars either. A file with the magic
        // that the core still rejects is a difference.
        if (HasDbpfMagic(path)) {
            ++t.coreFail;
            Log("formats shadow: %s: DBPF magic but rejected by the core's DBPF reader (a difference)", name.c_str());
        } else {
            Log("formats shadow: %s: no DBPF magic (not a package; skipped, not a difference)", name.c_str());
        }
        return;
    }
    const size_t count = fc4_dbpf_count_type(pkg, kExemplarType);
    Totals f;
    for (size_t i = 0; i < count; ++i) {
        if (g_maxPerFile > 0 && f.compared >= static_cast<uint32_t>(g_maxPerFile)) break;
        uint32_t group = 0, instance = 0;
        if (!fc4_dbpf_key_of_type(pkg, kExemplarType, i, &group, &instance)) break;
        ++f.entries;
        fc4_props* core = fc4_dbpf_read_exemplar(pkg, kExemplarType, group, instance, nullptr);
        if (core == nullptr) {
            ++f.coreFail;
            Detail("%s %08X-%08X-%08X: the core's reader rejects it", name.c_str(), kExemplarType, group, instance);
            continue;
        }
        props::Exemplar ex;
        if (!ex.Load(kExemplarType, group, instance) || ex.Holder() == nullptr) {
            ++f.gameMissing;
            fc4_props_destroy(core);
            continue;
        }
        ++f.compared;
        fc4_props* game = fc4_props_create();
        uint32_t own = 0;
        for (uint32_t id : props::PropertyIds(ex.Holder())) {
            if (props::CopyToCore(ex.Holder(), id, game)) ++own;
        }
        bool same = fc4_props_count(core) == fc4_props_count(game);
        bool reported = false;
        if (!same) {
            Detail("%s %08X-%08X-%08X: %u own properties in the core's parse, %u in the game's", name.c_str(),
                   kExemplarType, group, instance, static_cast<unsigned>(fc4_props_count(core)), own);
        }
        for (uint32_t id : props::PropertyIds(ex.Holder())) {
            const std::vector<uint8_t> g = VariantBytes(game, id);
            if (g.empty()) continue;  // not copied (object, empty): left out on both sides
            ++f.props;
            if (VariantBytes(core, id) != g) {
                same = false;
                if (!reported) {
                    reported = true;
                    Detail("%s %08X-%08X-%08X: property 0x%08X differs (type, shape or value)", name.c_str(),
                           kExemplarType, group, instance, id);
                }
            }
        }
        fc4_props_destroy(game);
        fc4_props_destroy(core);
        if (same) {
            ++f.exact;
        } else {
            ++f.differ;
        }
    }
    fc4_dbpf_close(pkg);
    Log("formats shadow: %s: %u exemplar entries, %u compared, %u exact, %u differ, %u rejected by the core, "
        "%u not loaded by the game, %u properties compared",
        name.c_str(), f.entries, f.compared, f.exact, f.differ, f.coreFail, f.gameMissing, f.props);
    t.entries += f.entries;
    t.compared += f.compared;
    t.exact += f.exact;
    t.differ += f.differ;
    t.coreFail += f.coreFail;
    t.gameMissing += f.gameMissing;
    t.props += f.props;
}

// ---- combined store: cohorts and effective lists (§3.1, §4.4) -------------------------------------

constexpr uint32_t kCohortType = 0x05342861;  // data-formats §8

bool IsDat(const std::wstring& name) {
    if (name.size() < 4) return false;
    return _wcsicmp(name.c_str() + name.size() - 4, L".dat") == 0;
}

// §3.1 listing order: a directory's files first, then its subdirectories, recursively (pre-order), each
// directory in the order the OS returns its entries [compat-limit]. `dat` selects *.dat files (true) or
// every other file (false).
void ListTree(const std::wstring& dir, bool dat, std::vector<std::wstring>& out) {
    std::vector<std::wstring> subdirs;
    WIN32_FIND_DATAW fd;
    HANDLE h = FindFirstFileW((dir + L"\\*").c_str(), &fd);
    if (h == INVALID_HANDLE_VALUE) return;
    do {
        const std::wstring n = fd.cFileName;
        if (fd.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY) {
            if (n != L"." && n != L"..") subdirs.push_back(dir + L"\\" + n);
        } else if (IsDat(n) == dat) {
            out.push_back(dir + L"\\" + n);
        }
    } while (FindNextFileW(h, &fd));
    FindClose(h);
    for (const std::wstring& s : subdirs) ListTree(s, dat, out);
}

// The game's resource under `key` as interface `iid` (GetResource), one reference held; nullptr if absent.
void* GameResource(uint32_t type, uint32_t group, uint32_t instance, uint32_t iid) {
    cIGZPersistResourceManagerPtr rm;
    if (!rm) return nullptr;
    cGZPersistResourceKey key(type, group, instance);
    void* out = nullptr;
    if (!rm->GetResource(key, iid, &out, 0, nullptr) || out == nullptr) return nullptr;
    return out;
}

struct SetTotals {
    uint32_t total = 0, clean = 0, props = 0, untyped = 0;
    uint64_t bytes = 0;
};

// One list of the core's (own or effective) against the game's IDs for the same list, values read through
// the game holder's effective lookup (slot 9; for own IDs the own value wins, §4.4). Clean when the ID sets
// are equal and every value's variant bytes are equal; a property the core leaves out (Empty, kind-1
// resource) must be one the game's CopyToCore leaves out as well.
bool CompareList(const char* what, const uint32_t* key, fc4_props* core, const std::vector<uint32_t>& coreUntyped,
                 size_t coreTotal, const std::vector<uint32_t>& gameIds, const cISCPropertyHolder* holder,
                 SetTotals& s) {
    ++s.total;
    bool same = gameIds.size() == coreTotal;
    if (!same) {
        Detail("%s %08X-%08X-%08X: %u properties in the core's list, %u in the game's", what, key[0], key[1], key[2],
               static_cast<unsigned>(coreTotal), static_cast<unsigned>(gameIds.size()));
    }
    fc4_props* game = fc4_props_create();
    for (uint32_t id : gameIds) {
        ++s.props;
        const bool copied = props::CopyToCore(holder, id, game);
        const std::vector<uint8_t> g = copied ? VariantBytes(game, id) : std::vector<uint8_t>();
        bool ok;
        if (g.empty()) {
            ++s.untyped;
            ok = std::find(coreUntyped.begin(), coreUntyped.end(), id) != coreUntyped.end();
        } else {
            s.bytes += g.size();
            ok = VariantBytes(core, id) == g;
        }
        if (!ok && same) {
            Detail("%s %08X-%08X-%08X: property 0x%08X differs (missing, type, shape or value)", what, key[0], key[1],
                   key[2], id);
        }
        same = same && ok;
    }
    fc4_props_destroy(game);
    if (same) ++s.clean;
    return same;
}

// The game cohort's effective IDs: its own (EnumProperties), then each parent's not already seen
// (GetParent, slot 6, borrowed; confirmed with QueryInterface as in props::EffectiveProperties).
// [choice] At most 32 parents, stopping at a repeated one (§4.4's limit).
std::vector<uint32_t> CohortEffectiveIds(cISCResExemplarCohort* c) {
    std::vector<uint32_t> out = props::PropertyIds(c->AsISCPropertyHolder());
    std::vector<const void*> visited{c};
    cIGZUnknown* parent = static_cast<cIGZUnknown*>(c->GetParent());
    for (int depth = 0; parent != nullptr && depth < 32; ++depth) {
        void* p = nullptr;
        if (!parent->QueryInterface(GZIID_cISCResExemplarCohort, &p) || p == nullptr) break;
        auto* pc = static_cast<cISCResExemplarCohort*>(p);
        FC4_ACQUIRED(pc, "QueryInterface");
        const bool cycle = std::find(visited.begin(), visited.end(), pc) != visited.end();
        if (!cycle) {
            visited.push_back(pc);
            for (uint32_t id : props::PropertyIds(pc->AsISCPropertyHolder())) {
                if (std::find(out.begin(), out.end(), id) == out.end()) out.push_back(id);
            }
        }
        parent = cycle ? nullptr : static_cast<cIGZUnknown*>(pc->GetParent());
        FC4_RELEASE(pc);
    }
    return out;
}

std::vector<uint32_t> CoreUntyped(fc4_exstore* s, size_t i, bool effective) {
    std::vector<uint32_t> ids(fc4_exstore_untyped_ids(s, i, effective, nullptr, 0));
    if (!ids.empty()) fc4_exstore_untyped_ids(s, i, effective, ids.data(), ids.size());
    return ids;
}

void StoreShadow(const std::wstring& dir) {
    const DWORD start = GetTickCount();
    // Packages in §3.1 registration order (each at the front: a later one wins).
    std::vector<std::wstring> order;
    WIN32_FIND_DATAW fd;
    HANDLE h = FindFirstFileW((dir + L"\\*.dat").c_str(), &fd);  // step 1
    if (h != INVALID_HANDLE_VALUE) {
        do {
            if (!(fd.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY)) order.push_back(dir + L"\\" + fd.cFileName);
        } while (FindNextFileW(h, &fd));
        FindClose(h);
    }
    // Step 2 (the language folder) is not read: §6.2 / §12 question 1 leave the folder unresolved [choice].
    // Step 3: Sku_Data recursively, one multi-package segment (last listed wins = added later). [choice] Its
    // *.dat files only (the multi segment of step 4b holds .dat files; question EX45).
    ListTree(dir + L"\\Sku_Data", true, order);
    // Step 4: install Plugins: (a) each non-.dat file that opens as a DBPF package, then (b) the .dat files.
    ListTree(dir + L"\\Plugins", false, order);
    ListTree(dir + L"\\Plugins", true, order);
    // Step 5 (the user's Documents\SimCity 4\Plugins) is not read by the shadow [choice]: a key a user plug-in
    // overrides shows as a difference.
    fc4_exstore* s = fc4_exstore_create();
    uint32_t pkgs = 0, noMagic = 0, rejected = 0, ex = 0, co = 0, bad = 0;
    for (const std::wstring& p : order) {
        const std::string u8 = Utf8(p);
        uint32_t c[3] = {};
        if (fc4_exstore_add_package(s, reinterpret_cast<const uint8_t*>(u8.data()), u8.size(), c)) {
            ++pkgs;
            ex += c[0];
            co += c[1];
            bad += c[2];
        } else if (!HasDbpfMagic(p)) {
            ++noMagic;  // not a package (§3.1 step 4a: files that do not open are not registered)
        } else {
            ++rejected;
            Detail("store: %s: DBPF magic but rejected by the core's reader", u8.c_str());
        }
    }
    size_t failed = 0;
    const size_t n = fc4_exstore_resolve(s, &failed);
    Log("formats shadow: store: %u files listed, %u packages, %u without DBPF magic (skipped), %u rejected; "
        "%u exemplar + %u cohort records read, %u unreadable; %u loaded, %u failed to load",
        static_cast<unsigned>(order.size()), pkgs, noMagic, rejected, ex, co, bad, static_cast<unsigned>(n),
        static_cast<unsigned>(failed));
    SetTotals sets[4];  // exemplar own, exemplar effective, cohort own, cohort effective
    uint32_t gameMissing = 0;
    for (size_t i = 0; i < n; ++i) {
        if (g_maxPerFile > 0 && i >= static_cast<size_t>(g_maxPerFile)) break;
        uint32_t key[3] = {};
        const int kind = fc4_exstore_key(s, i, key);
        if (kind < 0) break;
        const bool cohort = kind == 1;
        void* res = GameResource(key[0], key[1], key[2], cohort ? GZIID_cISCResExemplarCohort : GZIID_cISCResExemplar);
        if (res == nullptr) {
            ++gameMissing;
            Detail("%08X-%08X-%08X: loaded by the core, not by the game", key[0], key[1], key[2]);
            continue;
        }
        const cISCPropertyHolder* holder = nullptr;
        std::vector<uint32_t> effIds;
        if (cohort) {
            auto* c = static_cast<cISCResExemplarCohort*>(res);
            FC4_ACQUIRED(c, "GetResource");
            holder = c->AsISCPropertyHolder();
            effIds = CohortEffectiveIds(c);
        } else {
            auto* e = static_cast<cISCResExemplar*>(res);
            FC4_ACQUIRED(e, "GetResource");
            holder = e->AsISCPropertyHolder();
            for (const props::EffectiveProperty& p : props::EffectiveProperties(holder, e)) effIds.push_back(p.id);
        }
        for (int eff = 0; eff < 2; ++eff) {
            size_t total = 0;
            fc4_props* core = fc4_exstore_props(s, i, eff != 0, &total, nullptr);
            const std::vector<uint32_t> ids = eff ? effIds : props::PropertyIds(holder);
            const char* what = cohort ? (eff ? "cohort effective" : "cohort own") : (eff ? "exemplar effective"
                                                                                         : "exemplar own");
            if (core != nullptr) {
                CompareList(what, key, core, CoreUntyped(s, i, eff != 0), total, ids, holder,
                            sets[(cohort ? 2 : 0) + eff]);
                fc4_props_destroy(core);
            }
        }
        if (cohort) {
            FC4_RELEASE(static_cast<cISCResExemplarCohort*>(res));
        } else {
            FC4_RELEASE(static_cast<cISCResExemplar*>(res));
        }
    }
    // Records the core does not load: the game must not load them either.
    uint32_t failAgree = 0, failDiffer = 0;
    for (size_t i = 0; i < failed; ++i) {
        uint32_t key[3] = {};
        char why[200] = {};
        if (!fc4_exstore_failure(s, i, key, reinterpret_cast<uint8_t*>(why), sizeof(why))) break;
        const bool cohort = key[0] == kCohortType;
        void* res = GameResource(key[0], key[1], key[2], cohort ? GZIID_cISCResExemplarCohort : GZIID_cISCResExemplar);
        if (res == nullptr) {
            ++failAgree;
            continue;
        }
        ++failDiffer;
        Detail("%08X-%08X-%08X: the core does not load it (%s), the game does", key[0], key[1], key[2], why);
        if (cohort) {
            auto* c = static_cast<cISCResExemplarCohort*>(res);
            FC4_ACQUIRED(c, "GetResource");
            FC4_RELEASE(c);
        } else {
            auto* e = static_cast<cISCResExemplar*>(res);
            FC4_ACQUIRED(e, "GetResource");
            FC4_RELEASE(e);
        }
    }
    fc4_exstore_destroy(s);
    static const char* names[4] = {"exemplar own", "exemplar effective", "cohort own", "cohort effective"};
    bool exact = gameMissing == 0 && failDiffer == 0 && rejected == 0;
    for (int k = 0; k < 4; ++k) {
        exact = exact && sets[k].clean == sets[k].total;
        Log("formats shadow: store set %s: %u/%u clean, %u properties, %u untyped, %llu value bytes compared",
            names[k], sets[k].clean, sets[k].total, sets[k].props, sets[k].untyped,
            static_cast<unsigned long long>(sets[k].bytes));
    }
    Log("formats shadow: store total: %u not loaded by the game, failures %u agree / %u loaded by the game, "
        "%lu ms; %s",
        gameMissing, failAgree, failDiffer, static_cast<unsigned long>(GetTickCount() - start),
        exact && sets[0].total > 0 ? "EXACT" : "DIFFERENCES");
}

}  // namespace

void FormatsShadowStartup() {
    g_enabled = SettingInt("formats_shadow", "FC4_FORMATS_SHADOW", 0) != 0;
    if (!g_enabled) return;  // off by default: not a word in the log
    g_maxPerFile = SettingInt("formats_shadow_max", nullptr, 0);
    g_lines = SettingInt("formats_shadow_lines", nullptr, 8);
    const char* src = "";
    Setting("formats_shadow", "FC4_FORMATS_SHADOW", &src);
    Log("formats shadow: ON (formats_shadow from %s): at the first city load each *.dat of the install folder "
        "is read by the core's DBPF and exemplar readers and every exemplar's own properties are compared with "
        "the game's loaded exemplar of the same key (nothing replaced)",
        src);
}

void FormatsShadowCityLoaded() {
    if (!g_enabled || g_done) return;
    g_done = true;
    // glue-exemplars4: with `enable = exemplars` GetResource returns the
    // replacement's objects, so this run compares the Rust core with OUR
    // factory (a consistency check of the replacement), not with the original.
    const bool replaced = ExemplarFactoryActive();
    Log("formats shadow: game side = %s", replaced ? "REPLACEMENT (enable = exemplars; ours vs core, not a native "
                                                    "verification)"
                                                  : "ORIGINAL factories (native verification)");
    const std::wstring dir = InstallDir();
    if (dir.empty()) {
        Log("formats shadow: install folder not found; not run");
        return;
    }
    const DWORD start = GetTickCount();
    Totals t;
    WIN32_FIND_DATAW fd;
    HANDLE h = FindFirstFileW((dir + L"\\*.dat").c_str(), &fd);
    if (h != INVALID_HANDLE_VALUE) {
        do {
            if (fd.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY) continue;
            CompareFile(dir + L"\\" + fd.cFileName, Utf8(fd.cFileName), t);
        } while (FindNextFileW(h, &fd));
        FindClose(h);
    }
    Log("formats shadow: total: %u files (%u unreadable), %u exemplar entries, %u compared, %u exact, %u differ, "
        "%u rejected by the core, %u not loaded by the game, %u properties compared, %lu ms; %s",
        t.files, t.unreadable, t.entries, t.compared, t.exact, t.differ, t.coreFail, t.gameMissing, t.props,
        static_cast<unsigned long>(GetTickCount() - start),
        t.compared > 0 && t.differ == 0 && t.coreFail == 0 ? "EXACT" : "DIFFERENCES");
    g_shown = 0;  // the store phase gets its own detail lines
    StoreShadow(dir);
}

}  // namespace fc4
