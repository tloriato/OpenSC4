// SPDX-License-Identifier: GPL-3.0-or-later
// Opt-in, independent native fixtures; advisors.md sections 1.1, 2.5, 12.4.
#pragma once

#include "FC4Config.h"
#include "FC4AdviceSubjectNative.h"
#include "FC4Log.h"
#include "FC4PropCityCommon.h"
#include "FC4RefAudit.h"
#include "cFC4AdviceSubject.h"
#include "cIGZCOM.h"
#include "cISC4City.h"
#include "cRZCOMDllDirector.h"
#include "fc4.h"

#include <array>
#include <cstring>
#include <vector>

namespace fc4::subjectverify {

// A stack fixture keeps its storage even at zero so a bad release ordering is
// recorded without causing use-after-free. The creator owns its initial ref.
class Object final : public cIGZUnknown {
public:
    uint32_t refs = 1, zero = 0;
    std::vector<int> events;
    bool QueryInterface(uint32_t iid, void** out) override {
        if (iid != 1 || out == nullptr) return false;
        *out = static_cast<cIGZUnknown*>(this);
        AddRef();
        return true;
    }
    uint32_t AddRef() override { events.push_back(1); return ++refs; }
    uint32_t Release() override {
        events.push_back(-1);
        if (refs == 0) { ++zero; return 0; }
        if (--refs == 0) ++zero;
        return refs;
    }
};

struct Host {
    uint32_t x, z;
    fc4_advisors* core = nullptr;
    uint32_t created = 0, dropped = 0;
    static fc4_adv_obj* New(void* u) {
        return fc4_advisors_new_subject(static_cast<Host*>(u)->core);
    }
    static uint64_t Created(void* u, uint32_t kind, uint32_t, fc4_adv_obj* h) {
        if (kind != FC4_ADV_OBJ_SUBJECT) { fc4_adv_obj_destroy(h); return 0; }
        ++static_cast<Host*>(u)->created;
        auto* front = new cFC4AdviceSubject(h, &New, u);
        return front->Token();
    }
    static void Dropped(void* u, uint64_t token) {
        if (token == 0) return;
        ++static_cast<Host*>(u)->dropped;
        AdvFront::Dropped(token);
    }
    static void Counts(void* u, uint32_t* x, uint32_t* z) {
        const auto& h = *static_cast<Host*>(u);
        *x = h.x; *z = h.z;
    }
    static void Hold(void*, uint64_t token) { FromToken(token)->AddRef(); }
    static void Drop(void*, uint64_t token) { FromToken(token)->Release(); }
    cFC4ISC4AdviceSubject* Subject() {
        fc4_adv_obj* h = New(this);
        if (h == nullptr) return nullptr;
        auto* p = static_cast<cFC4ISC4AdviceSubject*>(FromToken(fc4_adv_obj_token(h)));
        if (p != nullptr) p->AddRef();
        fc4_adv_obj_destroy(h);
        return p;
    }
};

struct Compare {
    std::array<uint32_t, 14> calls{};
    uint32_t checks = 0, differences = 0;
    void Check(const char* what, uint32_t a, uint32_t b) {
        ++checks;
        if (a == b) return;
        ++differences;
        FC4_LOG("advice-subject-verify: DIFF %s native=%08X core=%08X", what, a, b);
    }
    static uint32_t Bits(float value) {
        uint32_t bits; std::memcpy(&bits, &value, sizeof(bits)); return bits;
    }
    void State(Native* n, cFC4ISC4AdviceSubject* c, Object* no, Object* co) {
        ++calls[4]; Check("GetLocationX", Bits(n->GetLocationX()), Bits(c->GetLocationX()));
        ++calls[5]; Check("GetLocationZ", Bits(n->GetLocationZ()), Bits(c->GetLocationZ()));
        ++calls[6]; Check("HasLocation", n->HasLocation(), c->HasLocation());
        ++calls[8];
        const uint32_t nr = no ? no->refs : 0, cr = co ? co->refs : 0;
        Check("GetObjectA native identity", n->GetObjectA() == no, true);
        Check("GetObjectA core identity", c->GetObjectA() == co, true);
        if (no) Check("GetObjectA borrowed/native", no->refs, nr);
        if (co) Check("GetObjectA borrowed/core", co->refs, cr);
        ++calls[9]; Check("GetData1", n->GetData1(), c->GetData1());
        ++calls[11]; Check("GetData2", n->GetData2(), c->GetData2());
    }
    void Objects(Object& n, Object& c) {
        Check("object refs", n.refs, c.refs);
        Check("object zero transitions", n.zero, c.zero);
        Check("object AddRef/Release order", n.events == c.events, true);
    }
};

inline void Run() {
    static bool done = false;
    if (done || SettingInt("advice_subject_verify", "FC4_ADVICE_SUBJECT_VERIFY", 0) != 1) return;
    cISC4City* city = propcity::AppCity(); // borrowed; only primitive dimensions shared
    if (city == nullptr) return;
    done = true;
    const long warnings = RefWarningCount();
    Host h{city->CellCountX(), city->CellCountZ()};
    Native* n = nullptr;
    // No AdvisorSystem is created, initialised, replaced or accessed here.
    // AD17: the public SDK calls its class-instantiation operation
    // GetClassObject (documented to create clsid and return iid), while the
    // behaviour spec calls it CreateInstance. Use only that public operation;
    // do not probe private constructors or another factory on refusal.
    if (!GZCOM() || !GZCOM()->GetClassObject(kFC4_CLSID_cSC4AdviceSubject,
            kFC4_IID_cISC4AdviceSubject, reinterpret_cast<void**>(&n)) || n == nullptr) {
        FC4_LOG("advice-subject-verify: REFUSED factory CLSID=6A26D049 IID=6A26D050; comparisons=0");
        return; // Do not infer failure-output ownership or try another endpoint.
    }
    FC4_ACQUIRED(n, "AdviceSubject CreateInstance independent fixture");
    Compare v;
    // Only published supported QI; no failure-output probes, private layouts or addresses.
    void* serialOut = nullptr;
    const bool serialOK = n->QueryInterface(GZIID_cIGZSerializable, &serialOut);
    v.Check("QI Serializable", serialOK && serialOut != nullptr, true);
    if (!serialOK || serialOut == nullptr) {
        FC4_RELEASE_CREATED(n);
        FC4_LOG("advice-subject-verify: REFUSED required Serializable ownership/identity");
        return;
    }
    auto* serial = static_cast<cIGZSerializable*>(serialOut);
    FC4_ACQUIRED(serial, "AdviceSubject QI Serializable");
    const uint32_t clsid = serial->GetGZCLSID();
    v.Check("native class", clsid, kFC4_CLSID_cSC4AdviceSubject);
    FC4_RELEASE(serial);
    if (clsid != kFC4_CLSID_cSC4AdviceSubject) {
        FC4_RELEASE_CREATED(n);
        FC4_LOG("advice-subject-verify: REFUSED native class identity");
        return;
    }
    for (uint32_t iid : {1u, kFC4_IID_cISC4AdviceSubject}) {
        void* out = nullptr;
        const bool ok = n->QueryInterface(iid, &out);
        v.Check("QI supported", ok && out != nullptr, true);
        if (ok && out != nullptr) {
            auto* p = static_cast<cIGZUnknown*>(out);
            FC4_ACQUIRED(p, "AdviceSubject QI supported");
            FC4_RELEASE(p);
        }
    }
    fc4_advisor_host callbacks{};
    callbacks.struct_size = sizeof(callbacks); callbacks.user = &h;
    callbacks.cell_counts = &Host::Counts;
    callbacks.add_ref = &Host::Hold; callbacks.release = &Host::Drop;
    callbacks.object_created = &Host::Created; callbacks.object_dropped = &Host::Dropped;
    h.core = fc4_advisors_create(&callbacks, nullptr, 0);
    auto* c = h.core ? h.Subject() : nullptr;
    if (c == nullptr) {
        FC4_RELEASE_CREATED(n);
        if (h.core) fc4_advisors_destroy(h.core);
        FC4_LOG("advice-subject-verify: REFUSED core fixture");
        return;
    }
    FC4_LOG("advice-subject-verify: factory established class=%08X cells=%u,%u independent=1", clsid, h.x, h.z);
    v.State(n, c, nullptr, nullptr);
    ++v.calls[6]; ++v.calls[4]; ++v.calls[5]; ++v.calls[9]; ++v.calls[11];
    v.Check("default HasLocation", n->HasLocation(), false);
    v.Check("default X", Compare::Bits(n->GetLocationX()), 0);
    v.Check("default Z", Compare::Bits(n->GetLocationZ()), 0);
    v.Check("default data1", n->GetData1(), 0);
    v.Check("default data2", n->GetData2(), 0);
    const std::array<std::array<float, 2>, 5> locations{{
        {{3.25f, 7.5f}}, {{-4.5f, -2.25f}},
        {{static_cast<float>(h.x) + 8.0f, static_cast<float>(h.z) + 4.0f}},
        {{-1.0f, static_cast<float>(h.z) + 1.0f}}, {{0.0f, 0.0f}}
    }};
    for (const auto& xy : locations) {
        ++v.calls[3]; v.Check("SetLocation return", n->SetLocation(xy[0], xy[1]), c->SetLocation(xy[0], xy[1]));
        v.State(n, c, nullptr, nullptr);
    }
    for (uint32_t word : {0u, 1u, 0x12345600u, 0x80000000u, 0xFFFFFFFFu}) {
        ++v.calls[10]; v.Check("SetData1 integer return", n->SetData1(word), word);
        v.Check("SetData1 replacement true", c->SetData1(word), true);
        ++v.calls[12]; v.Check("SetData2 integer return", n->SetData2(word), word);
        v.Check("SetData2 replacement true", c->SetData2(word), true);
        v.State(n, c, nullptr, nullptr);
    }
    Object na, nb, ca, cb;
    auto setObject = [&](Object* no, Object* co) {
        ++v.calls[7]; v.Check("SetObject return", n->SetObject(no), c->SetObject(co));
        v.State(n, c, no, co); v.Objects(na, ca); v.Objects(nb, cb);
    };
    setObject(nullptr, nullptr);
    setObject(&na, &ca);
    // Give up the creator reference, leaving exactly the subject's reference.
    na.Release(); ca.Release();
    setObject(&na, &ca);
    v.Check("same-object native stays live", na.zero, 0);
    v.Check("same-object core stays live", ca.zero, 0);
    na.AddRef(); ca.AddRef();
    setObject(&nb, &cb); setObject(&nb, &cb); setObject(nullptr, nullptr);
    setObject(&na, &ca);
    Native* nc = nullptr;
    cFC4ISC4AdviceSubject* cc = nullptr;
    ++v.calls[13];
    const bool nr = n->Clone(&nc), cr = c->Clone(&cc);
    v.Check("Clone return", nr, cr);
    v.Check("Clone native success", nr && nc != nullptr, true);
    v.Check("Clone core success", cr && cc != nullptr, true);
    if (!nr || nc == nullptr || !cr || cc == nullptr)
        FC4_LOG("advice-subject-verify: REFUSED Clone native_ok=%u native_out=%p core_ok=%u core_out=%p",
                nr, static_cast<void*>(nc), cr, static_cast<void*>(cc));
    if (nr && nc != nullptr) FC4_ACQUIRED(nc, "AdviceSubject Clone independent fixture");
    if (nr && nc != nullptr && cr && cc != nullptr) {
        v.Check("Clone native distinct", nc != n, true);
        v.Check("Clone core distinct", cc != c, true);
        v.State(nc, cc, &na, &ca); v.Objects(na, ca);
        v.Check("Clone native shares held object", na.refs, 3);
        v.Check("Clone core shares held object", ca.refs, 3);
        ++v.calls[3]; v.Check("Clone SetLocation", nc->SetLocation(13.5f, 17.25f), cc->SetLocation(13.5f, 17.25f));
        ++v.calls[10]; v.Check("Clone SetData1", nc->SetData1(42), 42); v.Check("Clone core SetData1", cc->SetData1(42), true);
        ++v.calls[12]; v.Check("Clone SetData2", nc->SetData2(43), 43); v.Check("Clone core SetData2", cc->SetData2(43), true);
        ++v.calls[7]; v.Check("Clone SetObject", nc->SetObject(&nb), cc->SetObject(&cb));
        v.State(nc, cc, &nb, &cb); v.State(n, c, &na, &ca);
        ++v.calls[9]; ++v.calls[11]; ++v.calls[4];
        v.Check("original data1 independent", n->GetData1(), 0xFFFFFFFFu);
        v.Check("original data2 independent", n->GetData2(), 0xFFFFFFFFu);
        v.Check("original X independent", Compare::Bits(n->GetLocationX()), 0);
        v.Objects(na, ca); v.Objects(nb, cb);
    }
    if (nr && nc != nullptr) v.Check("native clone final reference", FC4_RELEASE_CREATED(nc), 0);
    if (cr && cc != nullptr) v.Check("core clone final reference", cc->Release(), 0);
    v.Objects(na, ca); v.Objects(nb, cb);
    v.Check("native subject final reference", FC4_RELEASE_CREATED(n), 0);
    v.Check("core subject final reference", c->Release(), 0);
    fc4_advisors_destroy(h.core);
    v.Objects(na, ca); v.Objects(nb, cb);
    v.Check("native A cleanup", na.refs, 1); v.Check("native B cleanup", nb.refs, 1);
    v.Check("core A cleanup", ca.refs, 1); v.Check("core B cleanup", cb.refs, 1);
    na.Release(); nb.Release(); ca.Release(); cb.Release(); // creator-owned refs
    v.Objects(na, ca); v.Objects(nb, cb);
    v.Check("native A creator cleanup", na.refs, 0); v.Check("native B creator cleanup", nb.refs, 0);
    v.Check("core A creator cleanup", ca.refs, 0); v.Check("core B creator cleanup", cb.refs, 0);
    v.Check("core subject cleanup", h.created, h.dropped);
    v.Check("ref warnings", static_cast<uint32_t>(RefWarningCount() - warnings), 0);
    for (unsigned slot = 3; slot != 14; ++slot)
        FC4_LOG("advice-subject-verify: slot=%u native_comparisons=%u", slot, v.calls[slot]);
    FC4_LOG("advice-subject-verify: checks=%u differences=%u created=%u dropped=%u ref_warnings=%ld serialization=withheld",
            v.checks, v.differences, h.created, h.dropped, RefWarningCount() - warnings);
    FC4_LOG("advice-subject-verify: unsupported=serialization/no-independent-typed-stream; direct-Rust-clone_subject; "
            "direct-Rust-setter-return-values; nonfinite-locations; failed-QI-output-semantics");
}
} // namespace fc4::subjectverify
