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

// The tutorial system's GZCOM class (docs/specs/tutorial.md §1): city
// component 48, the cISC4City slot 82 getter. The record, IsTutorial and the
// three messages live in the Rust core (fc4_tutorial); this class adapts the
// interfaces to it and owns the game services it uses.
//
// Layout (§1): cISC4TutorialSystem and cIGZUnknown at +0, cIGZMessageTarget2
// at +4 (the three messages), the cheat handler (cIGZMessageTarget,
// 0xE98B2F57) at +8, cIGZSerializable at +0x10 (here: wherever MSVC puts
// it). QueryInterface answers those four IIDs and 1.
//
// TU6-TU9: use the version-gated native registry and original tutorial window.
// Bind the borrowed singleton to the actual cISC4TutorialSystem interface view
// for the object's lifetime, restoring the prior binding only if still ours.
// Plain cheat notifications and filter records are decoded per §§4.3.1/4.4.2.
//
// IsTutorial (§4.1) is read on the fire, flammability and advisor paths,
// also off the main thread: CoreLock protects entry; synchronous native calls
// leave it explicitly. The core never retains its state mutex across callbacks.
//
// OFF by default: FreeCity4.ini `enable = tutorial`; `disable` still wins.
#pragma once

#include "FC4HistClock.h"
#include "FC4Interfaces.h"
#include "cIGZMessageTarget.h"
#include "cIGZMessageTarget2.h"
#include "cIGZSerializable.h"
#include "fc4.h"

#include <atomic>
#include <cstdint>

class cIGZCheatCodeManager;
class cIGZMessageServer2;

static const uint32_t kFC4_CLSID_cSC4TutorialSystem = 0xAA3836FB;
static const uint32_t kFC4_IID_cISC4TutorialSystem = 0xAA3836F6;

class cFC4TutorialSystem;

// The cheat handler sub-object (+8, cIGZMessageTarget 0xE98B2F57).
class cFC4TutorialCheatHandler final : public cIGZMessageTarget {
public:
    explicit cFC4TutorialCheatHandler(cFC4TutorialSystem* owner) : owner_(owner) {}
    bool QueryInterface(uint32_t riid, void** ppvObj) override;
    uint32_t AddRef() override;
    uint32_t Release() override;
    bool DoMessage(cGZMessage& msg) override;

private:
    cFC4TutorialSystem* owner_;
};

class cFC4TutorialSystem final : public cFC4ISC4TutorialSystem, public cIGZMessageTarget2, public cIGZSerializable {
public:
    cFC4TutorialSystem();
    ~cFC4TutorialSystem();

    // cIGZUnknown (every sub-object)
    bool QueryInterface(uint32_t riid, void** ppvObj) override;
    uint32_t AddRef() override;
    uint32_t Release() override;

    // cISC4TutorialSystem (§6)
    bool Init() override;
    bool Shutdown() override;
    bool startTutorial() override;
    bool endTutorial(bool bEndTutorialRegistry) override;
    cISCLua* GetScriptingContext() override;
    bool IsTutorial() override;
    bool IsEdgeScrollingAllowed() override;
    cISC4TutorialTask* GetCurrentTask() override;
    int32_t GetTutorialTaskNumber() override;
    int32_t GetTutorialGUID() override;
    bool FilterMessage(uint32_t msg) override;
    cISC4TutorialSystem* ShowUIHintsForUsers() override;
    bool HandleCatalogMenu(cISC4WinCatalogView* pMenu) override;
    bool FilterLeftMouseClick(int32_t& nX, int32_t& nY, int32_t n) override;
    bool ResetTutorialNumAndIndex(int32_t a, int32_t b) override;
    bool OnTutorialEvent(uint32_t e) override;
    void SetTutorialTaskNumber(uint32_t n) override;
    void IncrementTutorialTaskNumber() override;

    // cIGZMessageTarget2 (§4.3)
    bool DoMessage(cIGZMessage2* pMessage) override;

    // cIGZSerializable (§3)
    bool Write(cIGZOStream& gzOut) override;
    bool Read(cIGZIStream& gzIn) override;
    uint32_t GetGZCLSID() override;

    // For the host callbacks and the cheat handler (cFC4Tutorial.cpp).
    cIGZMessageTarget2* Target() { return static_cast<cIGZMessageTarget2*>(this); }
    cIGZMessageTarget* CheatTarget() { return &cheat_; }
    cIGZUnknown* Primary() { return static_cast<cIGZUnknown*>(static_cast<cFC4ISC4TutorialSystem*>(this)); }
    fc4::CoreLock& Lock() { return lock_; }
    void Subscribe(uint32_t type, bool add);
    void CheatRegistration(bool add);
    bool CheatsAvailable();
    bool HasCheatManager();
    uint64_t FetchLuaContext();
    bool BindNativeOwner();
    void CheatCommand(const char* line, uint32_t cheatID = 0x6A382BAF);

    // fc4_tutorial_is_tutorial under the lock (the fire and flammability readers).
    uint32_t CoreIsTutorial();
    fc4_tutorial_state State();

private:
    uint32_t refCount_ = 0;
    bool destroying_ = false;
    bool subscribed_ = false;  // Init ran without a matching Shutdown (the last Release shuts down first)
    fc4::CoreLock lock_;
    fc4_tutorial_host host_{};
    fc4_tutorial* core_ = nullptr;
    cFC4TutorialCheatHandler cheat_;
    std::atomic<uint32_t> isTutorialCalls_{0};
    void* previousNativeOwner_ = nullptr;  // borrowed
    bool nativeOwnerBound_ = false;
};

// Factory for the subsystem table (reference count 0).
cIGZUnknown* CreateFC4TutorialSystem();

namespace fc4 {

// IsTutorial (§4.1) of a city's tutorial system: fc4_tutorial_is_tutorial
// when it is FreeCity4's (enable = tutorial), else the game's slot 8.
// `ours` (optional) says which one answered. A null system answers false.
bool TutorialIsTutorial(cISC4TutorialSystem* t, bool* ours = nullptr);

// The live FreeCity4 tutorial system behind `t`, or nullptr (an original).
cFC4TutorialSystem* OurTutorialSystem(cISC4TutorialSystem* t);

}  // namespace fc4
