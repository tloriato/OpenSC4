// SPDX-License-Identifier: GPL-3.0-or-later
// Independent constructed ORIGINAL owner fixture, persistence.md section 5.
// Public typed SDK calls only; no manager lists, registry mutations or layouts.
#include "FC4KeyListVerify.h"
#include "FC4Config.h"
#include "FC4Log.h"
#include "FC4RefAudit.h"
#include "compat/FC4Compat.h"
#include "cIGZCOM.h"
#include "cIGZPersistResourceKeyList.h"
#include "cGZPersistResourceKey.h"
#include "cRZCOMDllDirector.h"
#include <windows.h>
#include <array>
#include <initializer_list>

// Private static-link declarations, deliberately absent from fc4.h/ABI tables.
struct KLKey { uint32_t ty, group, instance; };
extern "C" {
void* fc4_kl_create();
void fc4_kl_destroy(void*);
uint32_t fc4_kl_op(void*, uint32_t, uint32_t, KLKey, uint32_t*);
uint32_t fc4_kl_get(const void*, uint32_t, uint32_t, KLKey*);
uint32_t fc4_kl_enum(const void*, uint32_t, void (*)(const KLKey*, void*), void*);
}

namespace fc4 {
namespace {
constexpr KLKey A{9,8,7}, T{0,8,7}, G{9,0xFFFFFFFF,7}, I{9,8,0};
constexpr KLKey Z{0,0,0}, F{0xFFFFFFFF,0xFFFFFFFF,0xFFFFFFFF};
constexpr KLKey Absent{9,8,0xFFFFFFFF};
bool Equal(KLKey a, KLKey b) {
    return a.ty == b.ty && a.group == b.group && a.instance == b.instance;
}
KLKey Words(const cGZPersistResourceKey& k) { return {k.type,k.group,k.instance}; }
cGZPersistResourceKey Native(KLKey k) { return {k.ty,k.group,k.instance}; }

// Same public-interface memory/provenance checks as FC4DbpfNative::Interface.
// Only vtable metadata and method module ownership are read, never code bytes.
// `safe` permits balancing the factory reference even on wrong-module refusal.
bool Provenance(cIGZPersistResourceKeyList* object, unsigned list, bool& safe) {
    MEMORY_BASIC_INFORMATION m{};
    safe = false;
    const auto* version = compat::IdentifiedVersion();
    if (!object || !VirtualQuery(object,&m,sizeof(m)) || m.State != MEM_COMMIT ||
        (m.Protect & (PAGE_GUARD | PAGE_NOACCESS))) return false;
    const auto table = *reinterpret_cast<const void* const* const*>(object);
    if (!VirtualQuery(table,&m,sizeof(m)) || m.State != MEM_COMMIT || m.Type != MEM_IMAGE ||
        (m.Protect & (PAGE_GUARD | PAGE_NOACCESS)) ||
        reinterpret_cast<uintptr_t>(table) + 11 * sizeof(void*) >
            reinterpret_cast<uintptr_t>(m.BaseAddress) + m.RegionSize) return false;
    const auto tableModule = reinterpret_cast<uintptr_t>(m.AllocationBase);
    char module[MAX_PATH]{};
    GetModuleFileNameA(static_cast<HMODULE>(m.AllocationBase),module,MAX_PATH);
    unsigned original = 0;
    for (unsigned slot = 0; slot < 11; ++slot) {
        if (!VirtualQuery(table[slot],&m,sizeof(m)) || m.State != MEM_COMMIT || m.Type != MEM_IMAGE ||
            !(m.Protect & (PAGE_EXECUTE | PAGE_EXECUTE_READ | PAGE_EXECUTE_READWRITE | PAGE_EXECUTE_WRITECOPY)))
            return false;
        if (version && reinterpret_cast<uintptr_t>(m.AllocationBase) == version->imageBase) ++original;
        if (slot == 2) safe = true; // All three published IUnknown methods validated.
    }
    const bool ok = version && tableModule == version->imageBase && original == 11;
    Log("keylist-verify: provenance list=%u object=%p vtable=%p module=%s moduleBase=%08lX original_methods=%u/11 verdict=%s",
        list,object,table,module,static_cast<unsigned long>(tableModule),original,ok ? "ORIGINAL" : "REFUSAL");
    return ok;
}

struct Collected {
    std::array<KLKey,32> keys{};
    std::array<const cGZPersistResourceKey*,32> ownerAddresses{};
    uint32_t count = 0, ownerCount = 0;
    bool aliases = false;
};
void CollectNative(const cGZPersistResourceKey& key, void* context) {
    auto& c = *static_cast<Collected*>(context);
    for (uint32_t i = 0; i < c.ownerCount; ++i) if (&key == c.ownerAddresses[i]) c.aliases = true;
    if (c.count < c.keys.size()) c.keys[c.count] = Words(key);
    ++c.count;
}
void CollectRust(const KLKey* key, void* context) {
    auto& c = *static_cast<Collected*>(context);
    if (c.count < c.keys.size()) c.keys[c.count] = *key;
    ++c.count;
}

struct Fixture {
    cIGZPersistResourceKeyList* lists[2]{};
    bool safe[2]{};
    void* rust = nullptr;
    uint32_t assertions = 0, exact = 0, diff = 0, refusal = 0, positive = 0, negative = 0;
    uint32_t methods = 0, created = 0, acquired = 0, released = 0, extraAdds = 0, extraReleases = 0;
    ~Fixture() {
        if (rust) fc4_kl_destroy(rust);
        for (unsigned i = 0; i < 2; ++i) {
            if (lists[i] && safe[i]) {
                FC4_RELEASE_CREATED(lists[i]);
                ++released;
            }
        }
        Log("keylist-verify: summary assertions=%u exact=%u diff=%u refusal=%u positive=%u negative=%u methods_mask=%02X factory_owned=%u created_acquired=%u released=%u extra_adds=%u extra_releases=%u natural=0 owner_attribution=22/1071 full_owner=UNVERIFIED",
            assertions,exact,diff,refusal,positive,negative,methods,created,acquired,released,extraAdds,extraReleases);
    }
    void Refuse(const char* reason) {
        ++refusal; Log("keylist-verify: REFUSAL reason=%s",reason);
    }
    void Check(const char* name, uint32_t native, uint32_t core, uint32_t expected) {
        ++assertions;
        if (expected) ++positive; else ++negative;
        const bool ok = native == expected && core == expected;
        if (ok) ++exact; else ++diff;
        Log("keylist-verify: assertion=%u %s name=%s native=%u rust=%u literal=%u",
            assertions,ok ? "exact" : "DIFF",name,native,core,expected);
    }
    void KeyCheck(const char* name, KLKey native, KLKey core, KLKey expected) {
        Check(name,Equal(native,expected),Equal(core,expected),1);
        if (!Equal(native,expected) || !Equal(core,expected))
            Log("keylist-verify: words native=%08X/%08X/%08X rust=%08X/%08X/%08X literal=%08X/%08X/%08X",
                native.ty,native.group,native.instance,core.ty,core.group,core.instance,
                expected.ty,expected.group,expected.instance);
    }
    bool Snapshot(unsigned list, std::initializer_list<KLKey> expected) {
        uint32_t size = 0;
        if (!fc4_kl_op(rust,list,4,Z,&size)) { Refuse("rust-size-transport"); return false; }
        const uint32_t nativeSize = lists[list]->Size(); methods |= 1 << 6;
        Check("Size",nativeSize,size,static_cast<uint32_t>(expected.size()));
        if (nativeSize != expected.size() || size != expected.size()) {
            Refuse("unexpected-size-no-invalid-indices"); return false;
        }
        Collected native, core;
        uint32_t index = 0;
        for (auto key : expected) {
            const auto& value = lists[list]->GetKey(index); methods |= 1 << 7;
            native.ownerAddresses[index] = &value;
            KLKey result{};
            if (!fc4_kl_get(rust,list,index,&result)) { Refuse("rust-get-transport"); return false; }
            KeyCheck("GetKey-valid",Words(value),result,key);
            ++index;
        }
        native.ownerCount = index;
        lists[list]->EnumKeys(CollectNative,&native); methods |= 1 << 4;
        if (!fc4_kl_enum(rust,list,CollectRust,&core)) { Refuse("rust-enum-transport"); return false; }
        Check("EnumKeys-count",native.count,core.count,static_cast<uint32_t>(expected.size()));
        Check("EnumKeys-copy-address",native.aliases,0,0);
        index = 0;
        for (auto key : expected) {
            KeyCheck("EnumKeys-ordered-copy",native.keys[index],core.keys[index],key);
            // Change retained copies; follow-up GetKey proves owner unchanged.
            native.keys[index] = Absent; core.keys[index] = Absent;
            KLKey result{};
            if (!fc4_kl_get(rust,list,index,&result)) { Refuse("rust-copy-transport"); return false; }
            KeyCheck("EnumKeys-retained-copy",Words(lists[list]->GetKey(index)),result,key);
            ++index;
        }
        return true;
    }
    bool Op(unsigned list, unsigned op, KLKey key, uint32_t expected) {
        uint32_t result = 0;
        if (!fc4_kl_op(rust,list,op,key,&result)) { Refuse("rust-op-transport"); return false; }
        uint32_t native = 0;
        switch (op) {
        case 0: native = lists[list]->Insert(Native(key)); methods |= 1 << 0; break;
        case 1: native = lists[list]->Erase(Native(key)); methods |= 1 << 2; break;
        case 2: native = lists[list]->EraseAll(); methods |= 1 << 3; break;
        case 3: native = lists[list]->IsPresent(Native(key)); methods |= 1 << 5; break;
        case 5:
            // SDK bool is observable, but section 5 promises effects only.
            native = lists[list]->Insert(*lists[1-list]); methods |= 1 << 1;
            Log("keylist-verify: Insert(list) bool=%u observed-only; score appended effects",native);
            return true;
        default: Refuse("invalid-fixture-op"); return false;
        }
        Check(op == 0 ? "Insert-key" : op == 1 ? "Erase-first" : op == 2 ? "EraseAll" : "IsPresent",
            native,result,expected);
        return true;
    }
};
} // namespace

void KeyListVerify() {
    static bool attempted = false;
    if (SettingInt("keylist_verify","FC4_KEYLIST_VERIFY",0) == 0 || attempted) return;
    attempted = true;
    Fixture f;
    auto* com = RZGetCOMDllDirector() ? GZCOM() : nullptr; // borrowed
    if (!com) { f.Refuse("no-GZCOM"); return; }
    for (unsigned i = 0; i < 2; ++i) {
        void* out = nullptr;
        const bool ok = com->GetClassObject(GZCLSID_cIGZPersistResourceKeyList,GZIID_cIGZPersistResourceKeyList,&out);
        Log("keylist-verify: factory list=%u clsid=656B8F2D iid=656B8F12 success=%u pointer=%p",i,ok,out);
        f.lists[i] = static_cast<cIGZPersistResourceKeyList*>(out);
        if (!out) { f.Refuse("factory-unavailable-or-null"); return; }
        ++f.created;
        const bool original = Provenance(f.lists[i],i,f.safe[i]);
        if (f.safe[i]) { FC4_ACQUIRED(f.lists[i],"KeyList GetClassObject created"); ++f.acquired; }
        if (!ok || !original) { f.Refuse("factory-failure-or-wrong-shape-or-module"); return; }
    }
    if (f.lists[0] == f.lists[1]) { f.Refuse("factory-pointer-alias"); return; }
    // No native assertion is scored before BOTH interface provenances pass.
    if (f.lists[0]->Size() != 0 || f.lists[1]->Size() != 0) { f.Refuse("fresh-not-empty"); return; }
    f.rust = fc4_kl_create();
    if (!f.rust) { f.Refuse("rust-creation"); return; }
    Log("keylist-verify: independent fresh empty ORIGINAL pair verified; synchronous pre-city fixture begins");
    if (!f.Snapshot(0,{}) || !f.Snapshot(1,{})) return;
    f.Op(0,3,A,0); f.Op(0,1,A,0); f.Op(0,2,Z,1);
    for (auto k : {A,T,A,G,I,Z,F}) f.Op(0,0,k,1);
    if (!f.Snapshot(0,{A,T,A,G,I,Z,F}) || !f.Snapshot(1,{})) return;
    for (auto k : {A,T,G,I,Z,F}) f.Op(0,3,k,1);
    f.Op(0,3,Absent,0);
    f.Op(0,1,A,1);
    if (!f.Snapshot(0,{T,A,G,I,Z,F})) return;
    f.Op(0,3,A,1); f.Op(0,1,A,1); f.Op(0,3,A,0); f.Op(0,1,A,0);
    if (!f.Snapshot(0,{T,G,I,Z,F})) return;
    for (auto k : {G,Z,G,T}) f.Op(1,0,k,1);
    if (!f.Snapshot(1,{G,Z,G,T})) return;
    f.Op(0,5,Z,0);
    if (!f.Snapshot(0,{T,G,I,Z,F,G,Z,G,T}) || !f.Snapshot(1,{G,Z,G,T})) return;
    f.Op(1,2,Z,1); f.Op(1,0,I,1);
    if (!f.Snapshot(1,{I}) || !f.Snapshot(0,{T,G,I,Z,F,G,Z,G,T})) return;
    f.Op(0,2,Z,1); f.Op(0,3,A,0); f.Op(0,1,A,0);
    if (!f.Snapshot(0,{})) return;
    f.Op(0,0,F,1); f.Op(0,0,Z,1); f.Op(0,5,Z,0);
    if (!f.Snapshot(0,{F,Z,I}) || !f.Snapshot(1,{I})) return;
    f.Op(1,2,Z,1); f.Op(0,5,Z,0);
    if (!f.Snapshot(0,{F,Z,I}) || !f.Snapshot(1,{})) return;
    // Observe balanced reference operations without assuming constructor count.
    for (auto* list : f.lists) {
        const uint32_t added = FC4_ADDREF(list);
        ++f.extraAdds;
        const uint32_t remaining = FC4_RELEASE(list);
        ++f.extraReleases;
        Log("keylist-verify: balanced-reference observation add=%u release=%u unscored-native-initial-count",added,remaining);
    }
    Log("keylist-verify: fixture complete; constructor/QI/overflow/reentrant/self-insert/invalid-index/unknown-aliases UNVERIFIED");
}
} // namespace fc4
