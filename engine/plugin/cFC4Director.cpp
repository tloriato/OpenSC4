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

// The FreeCity4.dll COM director. It registers the class factory for the
// simulator (CLSID 0x2990C1E5) so the game builds our cFC4Simulator instead
// of its own. Setting FC4_DISABLE_SIMULATOR=1 in the environment selects
// "passthrough" mode: the DLL loads and logs, but registers nothing, so the
// original simulator runs (for A/B comparisons).
//
// Diagnostics (FC4_LOG_LEVEL=2, the default): every COM entry point logs on
// ENTRY and on exit, and the class-object path runs under a structured-
// exception guard that logs the fault and fails the request instead of
// letting it escape into the game.

#include "FC4Autotest.h"
#include "FC4Config.h"
#include "FC4ClockProbe.h"
#include "FC4Diag.h"
#include "FC4Dialogs.h"
#include "FC4EffectsShadow.h"
#include "FC4FlammabilityShadow.h"
#include "FC4Clock24Shadow.h"
#include "FC4HistoryShadow.h"
#include "FC4NetMgrShadow.h"
#include "FC4MySimsShadow.h"  // the mysims1 lane: mysims_shadow
#include "FC4TerrainShadow.h"  // the glue16 lane
#include "FC4Fpu.h"
#include "FC4GrowthProbe.h"
#include "FC4LandValueShadow.h"
#include "FC4Log.h"
#include "FC4Observer.h"
#include "FC4Popups.h"
#include "FC4Subsystems.h"
#include "FC4TractShadow.h"
#include "FC4DisasterShadow.h"
#include "FC4PropLotShadow.h"  // the glue15 lane
#include "FC4ExemplarFactory.h"  // the glue-exemplars4 lane
#include "FC4Span565Bridge.h"    // the glue-span565-impl4 lane
#include "FC4SpanRender4444.h"  // the glue-span4444-impl4 lane: enable = span4444
#include "FC4SpanContext.h"  // the glue-span-context-final4 lane
#include "FC4SpanRender8888.h"  // the opt-in 8888 native bridge
#include "FC4RenderProperties.h"
#include "cFC4PropManager.h"  // the glue21 lane: fc4::PropManagerAppShutdown
#include "cFC4LotConfigManager.h"  // the glue21 lane: fc4::LotConfigManagerAppShutdown
#include "cFC4Simulator.h"

#include "cIGZCOM.h"
#include "cIGZFrameWork.h"
#include "cIGZString.h"
#include "cRZCOMDllDirector.h"
#include "fc4.h"

#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#include <windows.h>

#include <cstring>

#ifndef FC4_VERSION_STRING
#define FC4_VERSION_STRING "0.1.0"
#endif

namespace {

// Random, unique director ID for FreeCity4.dll.
constexpr uint32_t kFC4DirectorID = 0x5C7A43E1;

// Calls for other class IDs, and director refcount calls, are logged up to this many times.
constexpr uint32_t kMaxOtherClassLogs = 500;
constexpr uint32_t kMaxRefLogs = 100;

bool EnvIsOne(const char* name) {
    char value[16] = {};
    const DWORD n = GetEnvironmentVariableA(name, value, sizeof(value));
    return n > 0 && n < sizeof(value) && strcmp(value, "1") == 0;
}

uint32_t g_factoryCalls = 0;  // incremented on entry to the factory
bool g_probing = false;       // true while PostAppInit's diagnostic probe runs

struct EnumContext {
    cIGZCOMDirector::ClassObjectEnumerationCallback callback;
    void* context;
    uint32_t count;
};

void EnumThunk(uint32_t clsid, uint32_t reserved, void* pContext) {
    auto* ctx = static_cast<EnumContext*>(pContext);
    ++ctx->count;
    FC4_LOGV("EnumClassObjects: reporting clsid 0x%08X (reserved 0x%08X)", clsid, reserved);
    if (ctx->callback != nullptr) ctx->callback(clsid, reserved, ctx->context);
}

// ---- guarded steps (each runs under fc4::SehGuard) ----------------------------------------

struct NewObjectCall {
    fc4::ObjectFactory create;
    cIGZUnknown* result;
};

void NewObjectStep(void* p) {
    auto* c = static_cast<NewObjectCall*>(p);
    c->result = c->create();
}

struct QueryCall {
    cIGZUnknown* obj;
    uint32_t iid;
    void** out;
    bool result;
};

void QueryStep(void* p) {
    auto* c = static_cast<QueryCall*>(p);
    c->result = c->obj->QueryInterface(c->iid, c->out);
}

struct ReleaseCall {
    cIGZUnknown* obj;
    uint32_t result;
};

void ReleaseStep(void* p) {
    auto* c = static_cast<ReleaseCall*>(p);
    c->result = c->obj->Release();
}

}  // namespace

class cFC4Director final : public cRZCOMDllDirector {
public:
    cFC4Director() {
        fc4::Log("cFC4Director constructed (FreeCity4 %s, dll %s, log level %d)", FC4_VERSION_STRING,
                 fc4::ModulePath(), fc4::LogLevel());
        // Register the classes of every enabled subsystem (FC4Subsystems.cpp).
        size_t n = 0;
        const fc4::Subsystem* subsystems = fc4::Subsystems(&n);
        for (size_t i = 0; i < n; ++i) {
            const fc4::Subsystem& s = subsystems[i];
            if (!fc4::SubsystemEnabled(s)) continue;
            for (size_t k = 0; k < s.classCount; ++k) {
                // The effects replacement fails closed (glue22 question 9): its class is
                // registered only when the run also sets effects_replace = 1.
                if (strcmp(s.key, "effects") == 0 &&
                    fc4::SettingInt("effects_replace", "FC4_EFFECTS_REPLACE", 0) == 0) {
                    FC4_LOGV("AddCls skipped for subsystem 'effects' (glue22 question 9); "
                             "effects_shadow = 1 is the verified path; set effects_replace = 1 to force it");
                    continue;
                }
                AddCls(s.classes[k].clsid, s.classes[k].create);
                FC4_LOGV("AddCls(0x%08X) registered (%s, subsystem '%s')", s.classes[k].clsid, s.classes[k].name, s.key);
            }
        }
        simulatorReplaced_ = fc4::FindEnabledClass(kFC4_CLSID_cSC4Simulator) != nullptr;
        if (EnvIsOne("FC4_DIAG_SELFTEST")) {
            // Deliberate access violation inside our image, to prove that the
            // vectored handler and the SEH guard log it (used by CTest).
            const bool clean = fc4::SehGuard(
                "diagnostics self-test",
                [](void*) {
                    volatile int* volatile p = nullptr;
                    *p = 1;
                },
                nullptr);
            fc4::Log("diagnostics self-test: guard returned %d (expected 0)", clean ? 1 : 0);
        }
    }

    uint32_t GetDirectorID() const override { return kFC4DirectorID; }

    // ---- cIGZUnknown (traced: the framework holds the director as a hook) ----

    bool QueryInterface(uint32_t riid, void** ppvObj) override {
        const bool ok = cRZCOMDllDirector::QueryInterface(riid, ppvObj);
        if (refLogs_ < kMaxRefLogs) {
            ++refLogs_;
            FC4_LOGV("director QueryInterface(0x%08X) -> %d (refcount %u)", riid, ok ? 1 : 0, RefCount());
        }
        return ok;
    }

    uint32_t AddRef() override {
        const uint32_t r = cRZCOMDllDirector::AddRef();
        if (refLogs_ < kMaxRefLogs) {
            ++refLogs_;
            FC4_LOGV("director AddRef -> %u", r);
            // foundations §18.2: the precision window. These two calls are the
            // only ones into our DLL between the PreAppInit reference and the
            // PostAppInit measurement, so each one halves the window.
            fc4::FpuLog("director AddRef");
        }
        return r;
    }

    uint32_t Release() override {
        const uint32_t r = cRZCOMDllDirector::Release();
        if (refLogs_ < kMaxRefLogs) {
            ++refLogs_;
            FC4_LOGV("director Release -> %u", r);
            fc4::FpuLog("director Release");  // §18.2, as in AddRef
        }
        return r;
    }

    // ---- cIGZCOMDirector -----------------------------------------------------------------

    bool InitializeCOM(cIGZCOM* pCOM, const cIGZString& sLibraryPath) override {
        const char* path = sLibraryPath.ToChar();
        FC4_LOGV("InitializeCOM enter (com=%p, path='%s')", static_cast<void*>(pCOM), path ? path : "");
        const bool ok = cRZCOMDllDirector::InitializeCOM(pCOM, sLibraryPath);
        FC4_LOGV("InitializeCOM exit -> %d, framework=%p", ok ? 1 : 0, static_cast<void*>(FrameWork()));
        return ok;
    }

    bool OnStart(cIGZCOM* pCOM) override {
        FC4_LOGV("OnStart enter (com=%p)", static_cast<void*>(pCOM));
        fc4::Log("FreeCity4 %s loaded (core ABI %u, %s mode); dll: %s; log: %s", FC4_VERSION_STRING, fc4_abi_version(),
                 simulatorReplaced_ ? "replacement" : "PASSTHROUGH (simulator disabled, original simulator in use)",
                 fc4::ModulePath(), fc4::LogPath());
        fc4::FpuLog("OnStart");
        cIGZFrameWork* fw = RZGetFrameWork();
        FC4_LOGV("OnStart: framework=%p state=%d", static_cast<void*>(fw), fw ? static_cast<int>(fw->GetState()) : -1);
        if (fw != nullptr) {
            if (fw->GetState() < cIGZFrameWork::kStatePreAppInit) {
                const bool hooked = fw->AddHook(static_cast<cIGZFrameWorkHooks*>(this));
                FC4_LOGV("OnStart: AddHook -> %d", hooked ? 1 : 0);
            } else {
                PostAppInit();
            }
        }
        const bool r = cRZCOMDllDirector::OnStart(pCOM);
        FC4_LOGV("OnStart exit -> %d", r ? 1 : 0);
        return r;
    }

    bool GetClassObject(uint32_t rclsid, uint32_t riid, void** ppvObj) override {
        const bool ours = fc4::FindEnabledClass(rclsid) != nullptr;
        const bool logIt = ours || otherClassLogs_ < kMaxOtherClassLogs;
        if (!ours && logIt) ++otherClassLogs_;
        if (logIt) {
            FC4_LOGV("GetClassObject enter (clsid=0x%08X, iid=0x%08X, out=%p)", rclsid, riid,
                     static_cast<void*>(ppvObj));
        }
        bool ok = false;
        const fc4::ClassEntry* entry = fc4::FindEnabledClass(rclsid);
        if (entry != nullptr) {
            ok = CreateOurs(*entry, riid, ppvObj);
        } else {
            struct BaseCall {
                cFC4Director* self;
                uint32_t clsid, iid;
                void** out;
                bool result;
            } call{this, rclsid, riid, ppvObj, false};
            const bool clean = fc4::SehGuard(
                "director GetClassObject (base lookup)",
                [](void* p) {
                    auto* c = static_cast<BaseCall*>(p);
                    c->result = c->self->cRZCOMDllDirector::GetClassObject(c->clsid, c->iid, c->out);
                },
                &call);
            ok = clean && call.result;
        }
        if (logIt) {
            FC4_LOGV("GetClassObject exit (clsid=0x%08X, iid=0x%08X) -> %d, obj=%p", rclsid, riid, ok ? 1 : 0,
                     (ok && ppvObj) ? *ppvObj : nullptr);
        }
        return ok;
    }

    void EnumClassObjects(ClassObjectEnumerationCallback pCallback, void* pContext) override {
        FC4_LOGV("EnumClassObjects enter (callback=%p, context=%p)", reinterpret_cast<void*>(pCallback), pContext);
        EnumContext ctx{pCallback, pContext, 0};
        cRZCOMDllDirector::EnumClassObjects(&EnumThunk, &ctx);
        FC4_LOGV("EnumClassObjects exit: %u class(es) reported", ctx.count);
    }

    bool CanUnloadNow() override {
        const bool r = cRZCOMDllDirector::CanUnloadNow();
        FC4_LOGV("CanUnloadNow -> %d (refcount %u)", r ? 1 : 0, RefCount());
        return r;
    }

    // ---- cIGZFrameWorkHooks (all eight, logged on entry) ------------------------------------

    bool PreFrameWorkInit() override {
        FC4_LOGV("hook PreFrameWorkInit");
        fc4::FpuLog("hook PreFrameWorkInit", true);
        return true;
    }

    bool PreAppInit() override {
        FC4_LOGV("hook PreAppInit");
        // The first hook after the game's PreFrameWorkInit has run.
        fc4::FpuSetReference("hook PreAppInit");
        return true;
    }

    bool PostAppInit() override {
        fc4::FpuLog("hook PostAppInit (enter)", true);
        // foundations §18.2: our DLL's presence raised the main thread's x87
        // precision somewhere between PreAppInit and here, and nothing in the
        // game can restore 24-bit afterwards. The original subsystems round by
        // PC alone, so without this they compute different results than they
        // would in the unmodified game (run 19: 122/104 instead of 123/105).
        // Restore it before anything of ours or the game's runs on this thread.
        fc4::FpuReassert24Bit("PostAppInit");
        fc4::Log("hook PostAppInit (%s)", simulatorReplaced_ ? "simulator class 0x2990C1E5 provided by FreeCity4"
                                                             : "passthrough: simulator class not registered");
        // Ask GZCOM itself whether it now resolves the simulator class, and
        // through which director. This creates (and then releases) one object.
        // Set FC4_NO_PROBE=1 to skip it.
        if (simulatorReplaced_ && fc4::LogLevel() >= fc4::kLogVerbose && !EnvIsOne("FC4_NO_PROBE")) {
            cIGZCOM* com = GZCOM();
            g_probing = true;
            const uint32_t before = g_factoryCalls;
            cIGZUnknown* probe = nullptr;
            FC4_LOGV("PostAppInit probe: calling GZCOM(%p)->GetClassObject(0x%08X, IUnknown)", static_cast<void*>(com),
                     kFC4_CLSID_cSC4Simulator);
            const bool ok = com != nullptr &&
                            com->GetClassObject(kFC4_CLSID_cSC4Simulator, GZIID_cIGZUnknown,
                                                reinterpret_cast<void**>(&probe));
            g_probing = false;
            FC4_LOGV("PostAppInit probe: -> %d, obj=%p, our factory %s", ok ? 1 : 0, static_cast<void*>(probe),
                     g_factoryCalls != before ? "CALLED" : "NOT called (another factory answered)");
            if (ok && probe != nullptr) probe->Release();
        }
        // close_dialogs (FC4Dialogs.h, modal-dialogs.md §3.2): the modal
        // begin / end notices from here on, before any city load (the two
        // load-time dialogs of §4 come before ConnectionsReady).
        fc4::DialogsStart();
        // popup_dialogs (FC4Popups.h, advisors §6.8.1 option 1): before any
        // city view, so before the first popup tick. Once per process.
        fc4::PopupDialogsApply("PostAppInit");
        fc4::ExemplarFactoryInstall();  // enable = exemplars (FC4ExemplarFactory.h; glue-exemplars4)
        fc4::Span565Install();  // enable = span565 (FC4Span565Bridge.h; glue-span565-impl4)
        // Tooling that runs in every mode (autotest, digest, tick feed).
        fc4::ObserverStart();
        fc4::ClockProbeStart();  // clock_probe (FC4ClockProbe.h): the M1 clock differential
        fc4::GrowthProbeStart();  // growth_probe (FC4GrowthProbe.h): building-development.md §12
        fc4::LandValueShadowStart();     // landvalue_shadow (FC4LandValueShadow.h)
        fc4::FlammabilityShadowStart();  // flammability_shadow (FC4FlammabilityShadow.h)
        fc4::HistoryShadowStart();       // history_shadow (FC4HistoryShadow.h)
        fc4::Clock24ShadowStart();       // clock24_shadow (FC4Clock24Shadow.h)
        fc4::NetMgrShadowStart();        // netmgr_shadow (FC4NetMgrShadow.h): before any city subscribes
        fc4::TerrainShadowStart();       // terrain_shadow (FC4TerrainShadow.h)
        fc4::MySimsShadowStart();        // mysims_shadow (FC4MySimsShadow.h)
        fc4::SpanRender4444Start();      // enable = span4444 (FC4SpanRender4444.h)
        fc4::SpanRender8888Start();      // enable = span8888 (opt-in)
        fc4::RenderPropertiesStart();   // enable = renderprops (verification only)
        fc4::FpuLog("hook PostAppInit (exit)");
        return true;
    }

    bool PreAppShutdown() override {
        FC4_LOGV("hook PreAppShutdown");
        fc4::FpuLog("hook PreAppShutdown");
        fc4::NetMgrShadowStop();
        fc4::MySimsShadowStop();  // mysims_shadow (FC4MySimsShadow.h)
        fc4::EffectsShadowStop();  // effects_shadow: the summary line
        fc4::TerrainShadowStop();
        fc4::Clock24ShadowStop();
        fc4::HistoryShadowStop();
        fc4::FlammabilityShadowStop();
        fc4::LandValueShadowStop();
        fc4::GrowthProbeStop();
        fc4::DisastersShadowStop();  // disasters_shadow (FC4DisasterShadow.h): the original vtable entries back
        fc4::TractShadowStop();  // tract_shadow (FC4TractShadow.h): the original UpdatePopulation back
        fc4::PropLotShadowAppShutdown();  // the prop / lot configuration shadows' process catalogues
        fc4::PropManagerAppShutdown();  // the glue21 replacement's process prop catalogue
        fc4::LotConfigManagerAppShutdown();  // its process configuration catalogue (unload once)
        fc4::ClockProbeStop();
        fc4::SpanContextShutdown();  // glue-span-context-final4: caller counts (before the entries go back)
        fc4::SpanRender4444Stop();  // enable = span4444: the original vtable entries back
        fc4::SpanRender8888Stop();
        fc4::RenderPropertiesStop();
        fc4::DialogsStop();  // close_dialogs (FC4Dialogs.h)
        fc4::ObserverStop();
        fc4::ExemplarFactoryRestore();  // the game's factories back (data-formats §8.1)
        fc4::Span565Restore();  // the native 565 entries back, counter lines (glue-span565-impl4)
        return true;
    }

    bool PostAppShutdown() override {
        fc4::Log("hook PostAppShutdown");
        fc4::FpuLog("hook PostAppShutdown");
        return true;
    }

    bool PostSystemServiceShutdown() override {
        FC4_LOGV("hook PostSystemServiceShutdown");
        fc4::FpuLog("hook PostSystemServiceShutdown");
        fc4::AutotestOnShutdownComplete("PostSystemServiceShutdown");
        return true;
    }

    bool AbortiveQuit() override {
        fc4::Log("hook AbortiveQuit");
        fc4::FpuLog("hook AbortiveQuit");
        return true;
    }

    bool OnInstall() override {
        FC4_LOGV("hook OnInstall");
        fc4::FpuLog("hook OnInstall");
        return true;
    }

private:
    // Our own class-object path for 0x2990C1E5, so each step can be logged and
    // guarded separately: factory, then QueryInterface for the requested IID.
    bool CreateOurs(const fc4::ClassEntry& entry, uint32_t riid, void** ppvObj) {
        if (ppvObj == nullptr) {
            FC4_LOGV("CreateOurs: null out pointer");
            return false;
        }
        *ppvObj = nullptr;
        cIGZUnknown* obj = CreateGuarded(entry);
        if (obj == nullptr) {
            fc4::Log("CreateOurs: factory produced no object");
            return false;
        }
        FC4_LOGV("CreateOurs: before QueryInterface(0x%08X) on %p", riid, static_cast<void*>(obj));
        QueryCall q{obj, riid, ppvObj, false};
        const bool clean = fc4::SehGuard("QueryInterface on the new simulator", &QueryStep, &q);
        FC4_LOGV("CreateOurs: after QueryInterface -> %d (clean=%d), out=%p", q.result ? 1 : 0, clean ? 1 : 0,
                 *ppvObj);
        if (clean && q.result) return true;
        // Same as the SDK director: AddRef + Release destroys an unwanted object.
        obj->AddRef();
        ReleaseCall r{obj, 0};
        fc4::SehGuard("Release of the rejected simulator", &ReleaseStep, &r);
        *ppvObj = nullptr;
        return false;
    }

    // Runs a class factory under the SEH guard, with logging.
    static cIGZUnknown* CreateGuarded(const fc4::ClassEntry& entry) {
        ++g_factoryCalls;
        FC4_LOGV("factory enter: %s (0x%08X), factory call #%u%s", entry.name, entry.clsid, g_factoryCalls,
                 g_probing ? ", PostAppInit diagnostic probe" : "");
        NewObjectCall c{entry.create, nullptr};
        const bool clean = fc4::SehGuard(entry.name, &NewObjectStep, &c);
        if (!clean || c.result == nullptr) {
            fc4::Log("factory %s: construction FAILED (clean=%d, obj=%p)", entry.name, clean ? 1 : 0,
                     static_cast<void*>(c.result));
            return nullptr;
        }
        fc4::Log("%s object created (%p), factory call #%u%s", entry.name, static_cast<void*>(c.result), g_factoryCalls,
                 g_probing ? " [PostAppInit diagnostic probe; released immediately]" : "");
        return c.result;
    }

    bool simulatorReplaced_ = false;
    uint32_t otherClassLogs_ = 0;
    uint32_t refLogs_ = 0;
};

cRZCOMDllDirector* RZGetCOMDllDirector() {
    static cFC4Director sDirector;
    return &sDirector;
}
