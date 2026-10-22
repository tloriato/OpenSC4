// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.
//
// cSC4AdviceSubject (CLSID 0x6A26D049; advisors.md §1.1, §2.5, §11.4, §12.4):
// the GZCOM front of a core subject (fc4_adv_obj of kind FC4_ADV_OBJ_SUBJECT).
// The core holds the fields; every slot forwards to fc4_adv_subject_get/_set.

#pragma once

#include "FC4AdvLife.h"
#include "FC4Streams.h"  // cIFC4CoreObject
#include "cIGZSerializable.h"
#include "cISC4AdviceSubject.h"

#include <cstddef>
#include <cstdint>

struct fc4_adv_obj;

constexpr uint32_t kFC4_CLSID_cSC4AdviceSubject = 0x6A26D049;
constexpr uint32_t kFC4_IID_cISC4AdviceSubject = 0x6A26D050;

// §12.4: the SDK lacks slot 13 Clone; the game's vtable has 14 slots.
class cFC4ISC4AdviceSubject : public cISC4AdviceSubject {
public:
    virtual bool Clone(cFC4ISC4AdviceSubject** out) = 0;  // 13 (+0x34)
};

class cFC4AdviceSubject final : public cFC4ISC4AdviceSubject, public cIGZSerializable, public cIFC4CoreObject, public fc4::AdvFront {
public:
    // Takes over `handle` (FC4AdvLife.h). The factory arguments are retained
    // for existing callers; Clone uses the actual Rust clone transport.
    using NewSubjectFn = fc4_adv_obj* (*)(void* ctx);
    cFC4AdviceSubject(fc4_adv_obj* handle, NewSubjectFn newSubject, void* ctx);

    bool QueryInterface(uint32_t riid, void** ppvObj) override;
    uint32_t AddRef() override;
    uint32_t Release() override;

    bool SetLocation(float x, float z) override;  // 3
    float GetLocationX() override;                 // 4
    float GetLocationZ() override;                 // 5
    bool HasLocation() override;                   // 6
    bool SetObject(cIGZUnknown* object) override;  // 7 (AddRef through the core's add_ref)
    cIGZUnknown* GetObjectA() override;            // 8 (not AddRef'd)
    uint32_t GetData1() override;                  // 9
    bool SetData1(uint32_t v) override;            // 10 (§12.4: a replacement returns true)
    uint32_t GetData2() override;                  // 11
    bool SetData2(uint32_t v) override;            // 12
    bool Clone(cFC4ISC4AdviceSubject** out) override;  // 13

    bool Write(cIGZOStream& out) override;  // §11.4
    bool Read(cIGZIStream& in) override;
    uint32_t GetGZCLSID() override { return kFC4_CLSID_cSC4AdviceSubject; }

    // [choice] the strong handle while referenced (a reference read hands
    // the object over referenced), null otherwise.
    void* CoreHandle() override { return StrongHandle(); }
    uint32_t CoreClass() override { return kFC4_CLSID_cSC4AdviceSubject; }

    // The token the core keeps for this object (its cIGZUnknown*).
    uint64_t Token() { return fc4::TokenOf(static_cast<cIGZUnknown*>(static_cast<cFC4ISC4AdviceSubject*>(this))); }

    // Lifetime: fc4::AdvFront (the core says object_dropped; no leak).

private:
    bool Get(struct fc4_adv_subject_state* out);
};
