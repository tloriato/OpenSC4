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

// See FC4ExemplarFactory.h. Spec: data-formats.md §4.1-§4.6, §8, §8.1;
// foundations.md §10.1/§10.2/§17.2/§17.5/§17.6. Open points are questions
// EX46-EX49 (docs/specs/questions/data-formats.md).

#include "FC4ExemplarFactory.h"

#include "FC4Interfaces.h"
#include "FC4Log.h"
#include "FC4PropertyHolder.h"
#include "FC4RefAudit.h"
#include "fc4.h"
#include "GZServPtrs.h"
#include "cGZPersistResourceKey.h"
#include "cIGZPersistDBRecord.h"
#include "cIGZPersistDBSerialRecord.h"
#include "cIGZPersistResource.h"
#include "cIGZPersistResourceFactory.h"
#include "cIGZPersistResourceManager.h"
#include "cISCResExemplar.h"
#include "cISCResExemplarCohort.h"
#include <atomic>
#include <cstdint>
#include <mutex>
#include <vector>

namespace fc4 {
namespace {

constexpr uint32_t kTypeExemplar = 0x6534284A;
constexpr uint32_t kTypeCohort = 0x05342861;
constexpr uint32_t kClsExemplarFactory = 0x453429B3;  // §8.1: the game's registration
constexpr uint32_t kClsCohortFactory = 0x053429C8;
constexpr uint32_t kIIDUnknown = 0x00000001;
constexpr uint32_t kIIDExemplar = 0xA52160F5;  // §4 table
constexpr uint32_t kIIDCohort = 0x452160FB;
constexpr uint32_t kIIDHolder = 0x25216283;
constexpr uint32_t kIIDResource = 0x456B8F1D;
constexpr uint32_t kIIDFactory = 0xA56B8F17;
// Private to FreeCity4: answers our object core (with a reference). Never
// asked by the game.
constexpr uint32_t kIIDFC4Core = 0x4FC4E5A1;
// §4.4 [choice]: at most 32 ancestors load below the record requested.
constexpr size_t kMaxDepth = 32;

std::atomic<bool> g_enabled{false};
std::atomic<bool> g_active{false};

struct Counters {
    std::atomic<uint32_t> reads{0}, readFails{0}, parentFails{0}, cycleFails{0}, resourceProps{0}, emptyProps{0};
    std::atomic<uint32_t> created{0}, lists{0}, listFails{0}, unsupported{0}, msgAdds{0};
};
Counters g_count;

void Unsupported(const char* what) {
    // [choice, EX46-EX49] a slot whose behaviour the spec does not give: fail
    // and log the first few calls (evidence of whether the game uses it).
    const uint32_t n = ++g_count.unsupported;
    if (n <= 8) Log("exemplars: unsupported call %s (spec question; returns failure)", what);
}

bool SameKey(const cGZPersistResourceKey& a, const cGZPersistResourceKey& b) {
    return a.type == b.type && a.group == b.group && a.instance == b.instance;
}

// The records being read on this thread (outermost first): §4.4 cycle and
// depth guard. Failures are not remembered (§4.4).
thread_local std::vector<cGZPersistResourceKey> t_loading;

cIGZPersistResourceFactory* g_original[2] = {nullptr, nullptr};  // the game's, one reference each
cIGZPersistResourceFactory* g_ours[2] = {nullptr, nullptr};

int KindIndex(uint32_t type) { return type == kTypeCohort ? 1 : 0; }

class ExCore;

// The holder interface of our exemplar / cohort (§4.4): lookups on the
// effective list, EnumProperties on the own list (DG29), GetPropertyList the
// effective list. Lifetime goes to the owning object.
class ExHolder final : public cFC4ISCPropertyHolder {
public:
    explicit ExHolder(ExCore* o) : o_(o) {}
    bool QueryInterface(uint32_t riid, void** ppvObj) override;
    uint32_t AddRef() override;
    uint32_t Release() override;
    bool HasProperty(uint32_t id) const override;
    bool GetPropertyList(cIGZUnknownList** list) const override;
    bool GetPropertyData(uint32_t id, void* buffer, uint32_t& length) const override;
    bool GetPropertyInterface(uint32_t id, uint32_t riid, void** ppv) const override;
    bool GetPropertyString(uint32_t id, cIGZString& value) const override;
    bool GetPropertyU32(uint32_t id, uint32_t& value) const override;
    cISCProperty* GetProperty(uint32_t id) const override;
    bool AddPropertyData(uint32_t id, void* data, uint32_t length, bool b) override;
    bool AddPropertyObject(uint32_t id, cIGZUnknown* value, bool b) override;
    bool AddPropertyString(uint32_t id, cIGZString const& value, bool b) override;
    bool AddPropertyU32(uint32_t id, uint32_t value, bool b) override;
    bool AddPropertyVariant(uint32_t id, cFC4IGZVariant const* value, bool b) override;
    bool AddPropertyRecord(cISCProperty* property, bool b) override;
    bool CopyAddProperty(cISCProperty* property, bool b) override;
    bool RemoveProperty(uint32_t id, bool b) override;
    bool RemoveAllProperties(bool b) override;
    bool EnumPropertiesFiltered(FilterFn filter, EnumFn fn, void* data) const override;
    bool EnumProperties(EnumFn fn, void* data) const override;
    bool CompactProperties() override { return true; }

private:
    ExCore* o_;
};

// What both objects share: reference count, key, own / effective lists,
// parent cohort (one reference of ours).
class ExCore {
public:
    explicit ExCore(uint32_t type, cIGZUnknown* outer)
        : type_(type), key_(type, 0, 0), own_(outer), eff_(outer), holder_(this) {}
    virtual ~ExCore() {
        own_.Clear();
        eff_.Clear();
        if (parent_ != nullptr) FC4_RELEASE(parent_);
    }
    virtual cIGZUnknown* Outer() = 0;

    uint32_t type_;
    std::atomic<uint32_t> refs_{0};
    mutable std::recursive_mutex lock_;  // key and parent
    cGZPersistResourceKey key_;
    cFC4PropertyHolder own_;
    cFC4PropertyHolder eff_;
    ExHolder holder_;
    cISCResExemplarCohort* parent_ = nullptr;

    // §4.4: the effective list = the parent's effective list (its chain's own
    // lists, oldest first, through public interfaces: GetParent + the holder's
    // own-list EnumProperties), then the own properties on top. Entries are
    // the same property objects (shared with the parent, as the original's
    // {id, property} vector).
    void Rebuild() {
        std::lock_guard<std::recursive_mutex> g(lock_);
        eff_.Clear();
        std::vector<cFC4ISCPropertyHolder*> chain;
        cISCResExemplarCohort* c = parent_;
        while (c != nullptr && chain.size() <= kMaxDepth) {
            void* h = nullptr;
            if (!c->QueryInterface(kIIDHolder, &h) || h == nullptr) break;
            FC4_ACQUIRED(static_cast<cIGZUnknown*>(h), "QueryInterface");
            chain.push_back(static_cast<cFC4ISCPropertyHolder*>(h));
            c = c->GetParent();  // borrowed [choice: ownership not specified, EX48]
        }
        for (auto it = chain.rbegin(); it != chain.rend(); ++it) (*it)->EnumProperties(&AddTo, &eff_);
        own_.EnumProperties(&AddTo, &eff_);
        for (auto* h : chain) FC4_RELEASE(h);
    }
    static void AddTo(cISCProperty* p, void* holder) {
        static_cast<cFC4PropertyHolder*>(holder)->AddPropertyRecord(p, false);
    }
    // After an add to own_: the effective entry for `id` is the own object.
    void Mirror(uint32_t id) {
        if (cISCProperty* p = own_.GetProperty(id)) eff_.AddPropertyRecord(p, false);
    }
    void NoteMessage(bool b) {
        // §8 table: with the bool true the original posts message 0xC53D10AA
        // (exemplar AddProperty) / 0x853D11F1 (SetCohort). The message's
        // payload is not specified (EX48): not posted [choice], counted.
        if (b) ++g_count.msgAdds;
    }
    bool SetParent(cISCResExemplarCohort* c, bool b) {
        {
            std::lock_guard<std::recursive_mutex> g(lock_);
            if (c != nullptr) FC4_ADDREF(c);
            if (parent_ != nullptr) FC4_RELEASE(parent_);
            parent_ = c;
            Rebuild();
        }
        NoteMessage(b);
        return true;
    }
    bool GetKey(cGZPersistResourceKey& k) const {
        std::lock_guard<std::recursive_mutex> g(lock_);
        k = key_;
        return true;
    }
    bool SetKey(cGZPersistResourceKey const& k) {
        std::lock_guard<std::recursive_mutex> g(lock_);
        key_ = k;
        return true;
    }
    bool CoreQI(uint32_t riid, void** ppv, void* face, uint32_t faceIID, cIGZPersistResource* res) {
        if (ppv == nullptr) return false;
        void* out = nullptr;
        if (riid == faceIID || riid == kIIDUnknown) out = face;
        else if (riid == kIIDHolder) out = static_cast<cFC4ISCPropertyHolder*>(&holder_);
        else if (riid == kIIDResource) out = res;
        else if (riid == kIIDFC4Core) out = this;
        if (out == nullptr) {
            *ppv = nullptr;
            return false;
        }
        ++refs_;
        *ppv = out;
        return true;
    }
    uint32_t CoreRelease() {
        const uint32_t n = --refs_;
        if (n == 0) delete this;
        return n;
    }

    // §4.1/§4.2/§4.6 and §8.1 SerialRead: the record at position 0, already
    // decompressed. Reads the whole record, parses it in the core, loads the
    // parent (cohort IID, failure fails the read) and kind-1 resources.
    bool ReadRecord(cIGZPersistDBRecord& record);
};

bool ExHolder::QueryInterface(uint32_t riid, void** ppvObj) { return o_->Outer()->QueryInterface(riid, ppvObj); }
uint32_t ExHolder::AddRef() { return o_->Outer()->AddRef(); }
uint32_t ExHolder::Release() { return o_->Outer()->Release(); }
bool ExHolder::HasProperty(uint32_t id) const { return o_->eff_.HasProperty(id); }
bool ExHolder::GetPropertyData(uint32_t id, void* buffer, uint32_t& length) const {
    return o_->eff_.GetPropertyData(id, buffer, length);
}
bool ExHolder::GetPropertyInterface(uint32_t id, uint32_t riid, void** ppv) const {
    return o_->eff_.GetPropertyInterface(id, riid, ppv);
}
bool ExHolder::GetPropertyString(uint32_t id, cIGZString& value) const { return o_->eff_.GetPropertyString(id, value); }
bool ExHolder::GetPropertyU32(uint32_t id, uint32_t& value) const { return o_->eff_.GetPropertyU32(id, value); }
cISCProperty* ExHolder::GetProperty(uint32_t id) const { return o_->eff_.GetProperty(id); }
bool ExHolder::AddPropertyData(uint32_t id, void* data, uint32_t length, bool b) {
    const bool ok = o_->own_.AddPropertyData(id, data, length, b);
    o_->Mirror(id);
    o_->NoteMessage(b);
    return ok;
}
bool ExHolder::AddPropertyObject(uint32_t id, cIGZUnknown* value, bool b) {
    const bool ok = o_->own_.AddPropertyObject(id, value, b);
    o_->Mirror(id);
    o_->NoteMessage(b);
    return ok;
}
bool ExHolder::AddPropertyString(uint32_t id, cIGZString const& value, bool b) {
    const bool ok = o_->own_.AddPropertyString(id, value, b);
    o_->Mirror(id);
    o_->NoteMessage(b);
    return ok;
}
bool ExHolder::AddPropertyU32(uint32_t id, uint32_t value, bool b) {
    const bool ok = o_->own_.AddPropertyU32(id, value, b);
    o_->Mirror(id);
    o_->NoteMessage(b);
    return ok;
}
bool ExHolder::AddPropertyVariant(uint32_t id, cFC4IGZVariant const* value, bool b) {
    const bool ok = o_->own_.AddPropertyVariant(id, value, b);
    o_->Mirror(id);
    o_->NoteMessage(b);
    return ok;
}
bool ExHolder::AddPropertyRecord(cISCProperty* property, bool b) {
    if (property == nullptr) return false;
    const bool ok = o_->own_.AddPropertyRecord(property, b);
    o_->Mirror(property->GetPropertyID());
    o_->NoteMessage(b);
    return ok;
}
bool ExHolder::CopyAddProperty(cISCProperty* property, bool b) {
    if (property == nullptr) return false;
    const bool ok = o_->own_.CopyAddProperty(property, b);
    o_->Mirror(property->GetPropertyID());
    o_->NoteMessage(b);
    return ok;
}
// [choice, EX47] the spec gives no removal semantics for exemplars: remove
// from the own list and rebuild the effective list (a parent's property
// becomes visible again).
bool ExHolder::RemoveProperty(uint32_t id, bool b) {
    const bool ok = o_->own_.RemoveProperty(id, b);
    o_->Rebuild();
    return ok;
}
bool ExHolder::RemoveAllProperties(bool b) {
    const bool ok = o_->own_.RemoveAllProperties(b);
    o_->Rebuild();
    return ok;
}
bool ExHolder::EnumPropertiesFiltered(FilterFn filter, EnumFn fn, void* data) const {
    return o_->own_.EnumPropertiesFiltered(filter, fn, data);
}
bool ExHolder::EnumProperties(EnumFn fn, void* data) const { return o_->own_.EnumProperties(fn, data); }

// §4.4: a new list with every effective property in ascending ID order, one
// reference for the caller; false for a null out-pointer. The list class's
// compiled slot order is not in docs/abi (EX49), so the list is not built
// here: the game's own factory (kept since the takeover, §8.1) creates an
// empty object of this type through the public CreateInstance(type, IID)
// path, the effective properties are added to it (holder slot 15, bool
// false: no message) and its GetPropertyList makes the list.
bool ExHolder::GetPropertyList(cIGZUnknownList** list) const {
    if (list == nullptr) return false;
    *list = nullptr;
    ++g_count.lists;
    cIGZPersistResourceFactory* orig = g_original[KindIndex(o_->type_)];
    void* tmp = nullptr;
    if (orig == nullptr || !orig->CreateInstance(o_->type_, kIIDHolder, &tmp, 0, nullptr) || tmp == nullptr) {
        ++g_count.listFails;
        return false;
    }
    auto* h = static_cast<cFC4ISCPropertyHolder*>(tmp);
    o_->eff_.EnumProperties(
        [](cISCProperty* p, void* d) { static_cast<cFC4ISCPropertyHolder*>(d)->AddPropertyRecord(p, false); }, h);
    // [abi-ret] §4.4: GetPropertyList returns the list with one reference and
    // false (no list) only for a null out-pointer; `list` is not null here.
    const bool ok = h->GetPropertyList(list);
    if (!ok) ++g_count.listFails;
    FC4_RELEASE_CREATED(h);
    return ok;
}

struct LoadingScope {
    explicit LoadingScope(const cGZPersistResourceKey& k) { t_loading.push_back(k); }
    ~LoadingScope() { t_loading.pop_back(); }
};

bool ExCore::ReadRecord(cIGZPersistDBRecord& record) {
    ++g_count.reads;
    cGZPersistResourceKey rkey;
    record.GetKey(rkey);
    // §4.1: the type the record is stored under must be ours (the core
    // parse_record checks it with the signature).
    const uint32_t size = record.GetSize();
    std::vector<uint8_t> bytes(size);
    if (size > 0 && !record.GetFieldVoid(bytes.data(), size)) return false;
    fc4_exrec* rec = fc4_exrec_parse(rkey.type, bytes.data(), bytes.size());
    if (rec == nullptr) return false;
    struct Free {
        fc4_exrec* r;
        ~Free() { fc4_exrec_destroy(r); }
    } free_{rec};
    LoadingScope scope(rkey);
    uint32_t pk[3] = {0, 0, 0};
    fc4_exrec_parent(rec, pk);
    cIGZPersistResourceManagerPtr rm;
    cISCResExemplarCohort* parent = nullptr;
    if (pk[0] != 0 || pk[1] != 0 || pk[2] != 0) {
        const cGZPersistResourceKey parentKey(pk[0], pk[1], pk[2]);
        for (const auto& k : t_loading) {
            if (SameKey(k, parentKey)) {
                ++g_count.cycleFails;
                return false;  // §4.4 [choice]: first repeated key
            }
        }
        if (t_loading.size() > kMaxDepth) {
            ++g_count.cycleFails;
            return false;  // §4.4 [choice]: depth 32
        }
        void* out = nullptr;
        if (!rm || !rm->GetResource(parentKey, kIIDCohort, &out, 0, nullptr) || out == nullptr) {
            ++g_count.parentFails;
            return false;  // §4.2: the whole read fails
        }
        parent = static_cast<cISCResExemplarCohort*>(out);
        FC4_ACQUIRED(parent, "GetResource");
    }
    {
        std::lock_guard<std::recursive_mutex> g(lock_);
        if (parent_ != nullptr) FC4_RELEASE(parent_);
        parent_ = parent;  // our reference from GetResource
        own_.Clear();
        Rebuild();
    }
    // §4.2: properties in record order; a later one with the same ID
    // replaces the earlier (§4.5); each add updates the effective list.
    const size_t n = fc4_exrec_count(rec);
    for (size_t i = 0; i < n; ++i) {
        uint32_t id = 0, key3[3] = {0, 0, 0};
        uint16_t mt = 0;
        bool arr = false;
        const uint8_t* data = nullptr;
        size_t count = 0;
        const int32_t kind = fc4_exrec_prop(rec, i, &id, &mt, &arr, &data, &count, key3);
        if (kind == 0) {
            cFC4PropertyHolder::FromCoreProperty(&own_, id, mt, arr, data, count);
        } else if (kind == 1) {
            // §4.2: Empty is added like any other (memory type 0, no data).
            auto* p = new cFC4Property(id);
            own_.AddPropertyRecord(p, false);  // the holder takes its reference
            ++g_count.emptyProps;
        } else if (kind == 2) {
            // §4.6: the key is loaded through the resource manager as
            // cIGZPersistResource; failure fails the read.
            void* out = nullptr;
            if (!rm || !rm->GetResource(cGZPersistResourceKey(key3[0], key3[1], key3[2]), kIIDResource, &out, 0,
                                         nullptr) ||
                out == nullptr) {
                return false;
            }
            auto* r = static_cast<cIGZUnknown*>(out);
            FC4_ACQUIRED(r, "GetResource");
            own_.AddPropertyObject(id, r, false);  // the variant keeps its own reference
            FC4_RELEASE(r);
            ++g_count.resourceProps;
        } else {
            return false;
        }
        Mirror(id);
    }
    return true;
}

// cSCResExemplar replacement: cISCResExemplar (16 slots, foundations §17.5),
// the holder, cIGZPersistResource.
class cFC4ResExemplar final : public cISCResExemplar, public cIGZPersistResource, public ExCore {
public:
    cFC4ResExemplar() : ExCore(kTypeExemplar, static_cast<cISCResExemplar*>(this)) {}
    cIGZUnknown* Outer() override { return static_cast<cISCResExemplar*>(this); }
    bool QueryInterface(uint32_t riid, void** ppv) override {
        return CoreQI(riid, ppv, static_cast<cISCResExemplar*>(this), kIIDExemplar,
                      static_cast<cIGZPersistResource*>(this));
    }
    uint32_t AddRef() override { return ++refs_; }
    uint32_t Release() override { return CoreRelease(); }
    // cIGZSerializable (slots 3-5): not specified (EX46).
    bool Write(cIGZOStream&) override {
        Unsupported("cISCResExemplar::Write");
        return false;
    }
    bool Read(cIGZIStream&) override {
        Unsupported("cISCResExemplar::Read");
        return false;
    }
    uint32_t GetGZCLSID() override {
        Unsupported("cISCResExemplar::GetGZCLSID");
        return 0;
    }
    cISCPropertyHolder* AsISCPropertyHolder() override { return HolderSdk(); }
    cISCPropertyHolder* AsISCPropertyHolder() const override { return const_cast<cFC4ResExemplar*>(this)->HolderSdk(); }
    bool IsPropertyLocal(uint32_t id) override { return own_.HasProperty(id); }  // §4.4: own list only
    // [choice, EX48] borrowed: the reference stays with the exemplar.
    cISCResExemplarCohort* GetCohort() override {
        std::lock_guard<std::recursive_mutex> g(lock_);
        return parent_;
    }
    bool SetCohort(cISCResExemplarCohort* c, bool b) override { return SetParent(c, b); }
    bool GetKey(cGZPersistResourceKey& k) override { return ExCore::GetKey(k); }
    bool SetKey(cGZPersistResourceKey const& k) override { return ExCore::SetKey(k); }
    bool CompactProperties() override { return true; }
    // Slots 14 / 15 (data-formats §8 table: 15 rebuilds the effective list
    // from the parent). Their arguments are not in the spec (EX47); both are
    // taken as argument-free rebuilds [choice].
    virtual bool Slot14Rebuild() {
        Rebuild();
        return true;
    }
    virtual bool Slot15Rebuild() {
        Rebuild();
        return true;
    }
    // cIGZPersistResource
    void GetKey(cGZPersistResourceKey& k) const override { ExCore::GetKey(k); }
    uint32_t GetType() const override { return Key().type; }
    uint32_t GetGroup() const override { return Key().group; }
    uint32_t GetInstance() const override { return Key().instance; }
    bool SetType(uint32_t v) override { return SetPart(0, v); }
    bool SetGroup(uint32_t v) override { return SetPart(1, v); }
    bool SetInstance(uint32_t v) override { return SetPart(2, v); }
    uint32_t GetRefCount() const override { return refs_; }

private:
    cISCPropertyHolder* HolderSdk() {
        return reinterpret_cast<cISCPropertyHolder*>(static_cast<cFC4ISCPropertyHolder*>(&holder_));
    }
    cGZPersistResourceKey Key() const {
        cGZPersistResourceKey k;
        ExCore::GetKey(k);
        return k;
    }
    bool SetPart(int i, uint32_t v) {
        std::lock_guard<std::recursive_mutex> g(lock_);
        (i == 0 ? key_.type : i == 1 ? key_.group : key_.instance) = v;
        return true;
    }
};

// cSCResExemplarCohort replacement: the cohort interface (IID 0x452160FB,
// 16 slots; §8 table: slot 7 SetCohort, slot 9 the holder, slot 15 rebuild).
class cFC4ResCohort final : public cISCResExemplarCohort, public cIGZPersistResource, public ExCore {
public:
    cFC4ResCohort() : ExCore(kTypeCohort, static_cast<cISCResExemplarCohort*>(this)) {}
    cIGZUnknown* Outer() override { return static_cast<cISCResExemplarCohort*>(this); }
    bool QueryInterface(uint32_t riid, void** ppv) override {
        return CoreQI(riid, ppv, static_cast<cISCResExemplarCohort*>(this), kIIDCohort,
                      static_cast<cIGZPersistResource*>(this));
    }
    uint32_t AddRef() override { return ++refs_; }
    uint32_t Release() override { return CoreRelease(); }
    bool Write(cIGZOStream&) override {
        Unsupported("cohort::Write");
        return false;
    }
    bool Read(cIGZIStream&) override {
        Unsupported("cohort::Read");
        return false;
    }
    uint32_t GetGZCLSID() override {
        Unsupported("cohort::GetGZCLSID");
        return 0;
    }
    // [choice, EX48] borrowed, as GetCohort.
    cISCResExemplarCohort* GetParent() override {
        std::lock_guard<std::recursive_mutex> g(lock_);
        return parent_;
    }
    bool SetParent(cISCResExemplarCohort* c, bool b) override { return ExCore::SetParent(c, b); }
    cISCPropertyHolder* AsISCPropertyHolder() override { return HolderSdk(); }
    cISCPropertyHolder* AsISCPropertyHolder() const override { return const_cast<cFC4ResCohort*>(this)->HolderSdk(); }
    bool IsPropertyLocal(uint32_t id) override { return own_.HasProperty(id); }
    bool GetKey(cGZPersistResourceKey& k) override { return ExCore::GetKey(k); }
    bool SetKey(cGZPersistResourceKey const& k) override { return ExCore::SetKey(k); }
    bool CompactProperties() override { return true; }
    virtual bool Slot14Rebuild() {  // EX47, as the exemplar's
        Rebuild();
        return true;
    }
    virtual bool Slot15Rebuild() {
        Rebuild();
        return true;
    }
    void GetKey(cGZPersistResourceKey& k) const override { ExCore::GetKey(k); }
    uint32_t GetType() const override { return Key().type; }
    uint32_t GetGroup() const override { return Key().group; }
    uint32_t GetInstance() const override { return Key().instance; }
    bool SetType(uint32_t v) override { return SetPart(0, v); }
    bool SetGroup(uint32_t v) override { return SetPart(1, v); }
    bool SetInstance(uint32_t v) override { return SetPart(2, v); }
    uint32_t GetRefCount() const override { return refs_; }

private:
    cISCPropertyHolder* HolderSdk() {
        return reinterpret_cast<cISCPropertyHolder*>(static_cast<cFC4ISCPropertyHolder*>(&holder_));
    }
    cGZPersistResourceKey Key() const {
        cGZPersistResourceKey k;
        ExCore::GetKey(k);
        return k;
    }
    bool SetPart(int i, uint32_t v) {
        std::lock_guard<std::recursive_mutex> g(lock_);
        (i == 0 ? key_.type : i == 1 ? key_.group : key_.instance) = v;
        return true;
    }
};

// The factory (§8.1): slots 3-6 from the SDK interface (compiled slot 3 is
// the record path), then slots 7-12 with the argument counts of docs/abi
// (vtable 0x00AAEEF8) so the table has the original's shape.
class cFC4ExemplarFactory final : public cIGZPersistResourceFactory {
public:
    explicit cFC4ExemplarFactory(uint32_t type) : type_(type) {}
    bool QueryInterface(uint32_t riid, void** ppv) override {
        if (ppv == nullptr) return false;
        if (riid == kIIDFactory || riid == kIIDUnknown) {
            AddRef();
            *ppv = static_cast<cIGZPersistResourceFactory*>(this);
            return true;
        }
        *ppv = nullptr;
        return false;
    }
    uint32_t AddRef() override { return ++refs_; }
    uint32_t Release() override {
        const uint32_t n = --refs_;
        if (n == 0) delete this;
        return n;
    }
    // Slot 4 (type path): the trailing values are ignored (§8.1, base slot 4).
    bool CreateInstance(uint32_t type, uint32_t riid, void** ppv, uint32_t, cIGZUnknown*) override {
        return CreateInstance3(type, riid, ppv);
    }
    // Slot 3 (record path, §8.1 step 2): create, give the key, read, then
    // QueryInterface for the caller; our own reference released in every
    // case. The manager closes or aborts the record.
    bool CreateInstance(cIGZPersistDBRecord& record, uint32_t riid, void** ppv, uint32_t, cIGZUnknown*) override {
        std::lock_guard<std::recursive_mutex> g(lock_);
        if (ppv == nullptr) return false;
        *ppv = nullptr;
        cGZPersistResourceKey key;
        record.GetKey(key);
        ExCore* core = NewObject(key.type);
        if (core == nullptr) return false;
        core->SetKey(key);
        bool ok = core->ReadRecord(record);
        if (ok) ok = core->Outer()->QueryInterface(riid, ppv);
        if (!ok) ++g_count.readFails;
        core->CoreRelease();
        return ok;
    }
    // Slot 5 Read(resource, record) (§8.1: through the serial record to
    // SerialRead).
    bool Read(cIGZPersistResource& resource, cIGZPersistDBRecord& record) override {
        void* c = nullptr;
        if (!resource.QueryInterface(kIIDFC4Core, &c) || c == nullptr) return false;
        auto* core = static_cast<ExCore*>(c);
        const bool ok = core->type_ == type_ && core->ReadRecord(record);
        core->CoreRelease();
        return ok;
    }
    bool Write(cIGZPersistResource const&, cIGZPersistDBRecord&) override {
        Unsupported("factory Write (SerialWrite)");
        return false;
    }
    // Slot 7: the original's destructor slot. Our lifetime is the reference
    // count; nothing is deleted here [choice].
    virtual void* Slot7Destroy(uint32_t) { return this; }
    virtual bool Slot8(uint32_t, uint32_t, void**) {  // 3 arguments (docs/abi); behaviour not specified, EX46
        Unsupported("factory slot 8");
        return false;
    }
    // Slot 9 CreateInstance(type, IID, out) (§8.1).
    virtual bool CreateInstance3(uint32_t type, uint32_t riid, void** ppv) {
        if (ppv == nullptr) return false;
        *ppv = nullptr;
        if (type != type_) return false;
        ExCore* core = NewObject(type);
        const bool ok = core != nullptr && core->Outer()->QueryInterface(riid, ppv);
        if (core != nullptr) core->CoreRelease();  // unknown IID: destroyed, null, false
        return ok;
    }
    // Slot 10 SerialRead(resource, serial record).
    virtual bool SerialRead(cIGZPersistResource* resource, cIGZPersistDBSerialRecord* record) {
        if (resource == nullptr || record == nullptr) return false;
        cIGZPersistDBRecord* base = record->AsIGZPersistDBRecord();
        return base != nullptr && Read(*resource, *base);
    }
    virtual bool SerialWrite(cIGZPersistResource*, cIGZPersistDBSerialRecord*) {
        Unsupported("factory SerialWrite");
        return false;
    }
    virtual void SetWriteMode(uint32_t v) { writeMode_ = v; }  // slot 12: no reader uses it [choice]

private:
    ExCore* NewObject(uint32_t type) {
        if (type != type_) return nullptr;
        ExCore* core = type == kTypeCohort ? static_cast<ExCore*>(new cFC4ResCohort())
                                           : static_cast<ExCore*>(new cFC4ResExemplar());
        ++core->refs_;
        ++g_count.created;
        return core;
    }
    uint32_t type_;
    std::atomic<uint32_t> refs_{0};
    std::recursive_mutex lock_;
    uint32_t writeMode_ = 0;
};

// §8.1: unregister (slot 20) takes a class ID; the SDK declares a factory
// pointer there (same size on x86), so the class ID is passed in it.
bool UnregisterClass(cIGZPersistResourceManager* rm, uint32_t cls) {
    // [abi-ret] §8.1 slot 20: true if at least one entry was removed.
    return rm->UnregisterObjectFactory(reinterpret_cast<cIGZPersistResourceFactory*>(static_cast<uintptr_t>(cls)));
}

bool TakeOver(cIGZPersistResourceManager* rm, int i, uint32_t type, uint32_t cls) {
    cIGZPersistResourceFactory* orig = nullptr;
    // The (type, out) overload is slot 21 (MSVC places overloads in reverse
    // declaration order), which docs/abi gives as 8 argument bytes like the
    // SDK. Taken as a member pointer so the abi_conformance name scan does not
    // read it as the (out) overload (slot 22, SDK 4 bytes, game pops 8), which
    // the plugin never calls.
    constexpr auto kFindByType = static_cast<bool (cIGZPersistResourceManager::*)(
        uint32_t, cIGZPersistResourceFactory**)>(&cIGZPersistResourceManager::FindObjectFactory);
    // [abi-ret] §8.1 slot 21: true with the factory and one added reference.
    if (!(rm->*kFindByType)(type, &orig) || orig == nullptr) {  // slot 21: one reference for us
        Log("exemplars: no factory registered for type 0x%08X; not replaced", type);
        return false;
    }
    FC4_ACQUIRED(orig, "FindObjectFactory");
    auto* ours = new cFC4ExemplarFactory(type);
    FC4_ADDREF(ours);
    const bool removed = UnregisterClass(rm, cls);
    // [abi-ret] §8.1 slot 19: returns true when registered.
    const bool added = rm->RegisterObjectFactory(cls, type, ours);  // the manager AddRefs
    if (!added) {
        if (removed) rm->RegisterObjectFactory(cls, type, orig);
        FC4_RELEASE(ours);
        FC4_RELEASE(orig);
        Log("exemplars: register failed for type 0x%08X; original kept", type);
        return false;
    }
    g_original[i] = orig;
    g_ours[i] = ours;
    Log("exemplars: type 0x%08X class 0x%08X now ours (unregistered original: %s)", type, cls,
        removed ? "yes" : "no");
    return true;
}

void Restore(cIGZPersistResourceManager* rm, int i, uint32_t type, uint32_t cls) {
    if (g_ours[i] == nullptr) return;
    const bool removed = UnregisterClass(rm, cls);
    // [abi-ret] §8.1 slot 19: returns true when registered.
    const bool added = g_original[i] != nullptr && rm->RegisterObjectFactory(cls, type, g_original[i]);
    Log("exemplars: type 0x%08X restored (ours unregistered: %s, original registered: %s)", type,
        removed ? "yes" : "no", added ? "yes" : "no");
    FC4_RELEASE(g_ours[i]);
    g_ours[i] = nullptr;
    if (g_original[i] != nullptr) FC4_RELEASE(g_original[i]);
    g_original[i] = nullptr;
}

}  // namespace

void ExemplarFactorySetEnabled(bool enabled) { g_enabled = enabled; }

void ExemplarFactoryInstall() {
    if (!g_enabled || g_active) return;
    cIGZPersistResourceManagerPtr rm;
    if (!rm) {
        Log("exemplars: no resource manager; not replaced");
        return;
    }
    const bool e = TakeOver(rm, 0, kTypeExemplar, kClsExemplarFactory);
    const bool c = TakeOver(rm, 1, kTypeCohort, kClsCohortFactory);
    g_active = e || c;
}

void ExemplarFactoryRestore() {
    if (!g_active) return;
    cIGZPersistResourceManagerPtr rm;
    Log("exemplars: summary reads=%u failed=%u parent_failed=%u cycle_or_depth=%u created=%u empty=%u "
        "resource_props=%u property_lists=%u list_failed=%u message_adds=%u unsupported=%u",
        g_count.reads.load(), g_count.readFails.load(), g_count.parentFails.load(), g_count.cycleFails.load(),
        g_count.created.load(), g_count.emptyProps.load(), g_count.resourceProps.load(), g_count.lists.load(),
        g_count.listFails.load(), g_count.msgAdds.load(), g_count.unsupported.load());
    if (rm) {
        Restore(rm, 0, kTypeExemplar, kClsExemplarFactory);
        Restore(rm, 1, kTypeCohort, kClsCohortFactory);
    }
    g_active = false;
}

bool ExemplarFactoryActive() { return g_active; }

}  // namespace fc4
