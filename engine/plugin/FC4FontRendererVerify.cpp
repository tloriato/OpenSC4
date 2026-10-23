// SPDX-License-Identifier: GPL-3.0-or-later
// Constructed raw-primitive target only. Native original performs its own raster.
#include "FC4FontRendererVerify.h"
#include "FC4FontRendererFixtures.h"
#include "FC4Config.h"
#include "FC4Log.h"
#include "FC4RefAudit.h"
#include "compat/FC4Compat.h"
#include "cIGZCOM.h"
#include "cRZCOMDllDirector.h"
#include <windows.h>

namespace fc4 {
namespace {
using namespace font_verify;
bool Original(SmallRenderer* p,unsigned slot) {
    if (!p || !compat::IdentifiedVersion()) return false;
    MEMORY_BASIC_INFORMATION object{}, table{};
    if (!VirtualQuery(p,&object,sizeof(object)) || object.State!=MEM_COMMIT ||
        (object.Protect & (PAGE_GUARD|PAGE_NOACCESS)) ||
        reinterpret_cast<uintptr_t>(p)+4 > reinterpret_cast<uintptr_t>(object.BaseAddress)+object.RegionSize)
        return false;
    uintptr_t vptr=0; std::memcpy(&vptr,p,4); // identity data, never slot probing
    if (!VirtualQuery(reinterpret_cast<void*>(vptr),&table,sizeof(table)) || table.Type!=MEM_IMAGE ||
        table.State!=MEM_COMMIT || (table.Protect & (PAGE_GUARD|PAGE_NOACCESS)) ||
        vptr+sizeof(RendererSlots)>reinterpret_cast<uintptr_t>(table.BaseAddress)+table.RegionSize)
        return false;
    const auto image=reinterpret_cast<uintptr_t>(GetModuleHandleW(nullptr));
    if(!OriginalIdentity(vptr,reinterpret_cast<uintptr_t>(table.AllocationBase),image)) return false;
    // Inspect ONLY the published typed entry for provenance. All dispatches
    // below still use compiler-generated virtual calls, never these addresses.
    const auto* typed=reinterpret_cast<const RendererSlots*>(vptr);
    uintptr_t entry=0;
    switch(slot) {
        case 0: entry=reinterpret_cast<uintptr_t>(typed->query); break;
        case 1: entry=reinterpret_cast<uintptr_t>(typed->addRef); break;
        case 2: entry=reinterpret_cast<uintptr_t>(typed->release); break;
        case 5: entry=reinterpret_cast<uintptr_t>(typed->lock); break;
        case 6: entry=reinterpret_cast<uintptr_t>(typed->unlock); break;
        case 7: entry=reinterpret_cast<uintptr_t>(typed->draw); break;
        default: return false;
    }
    MEMORY_BASIC_INFORMATION method{};
    if(!VirtualQuery(reinterpret_cast<void*>(entry),&method,sizeof(method)) ||
        method.Type!=MEM_IMAGE || method.State!=MEM_COMMIT ||
        (method.Protect & (PAGE_GUARD|PAGE_NOACCESS)) ||
        !(method.Protect & (PAGE_EXECUTE|PAGE_EXECUTE_READ|PAGE_EXECUTE_READWRITE|PAGE_EXECUTE_WRITECOPY)))
        return false;
    return OriginalEntry(entry,reinterpret_cast<uintptr_t>(method.AllocationBase),image,slot);
}
struct Native {
    SmallRenderer* p=nullptr;
    uint32_t gates=0, refusal=0, dispatches=0;
    bool Enter(unsigned slot) {
        ++gates;
        if (!Original(p,slot)) { ++refusal; return false; }
        ++dispatches; return true;
    }
    bool Execute(Run& r,uint8_t& result) {
        if(!Enter(r.input.operation==1?5:r.input.operation==2?6:7)) return false;
        auto& f=r.input; auto* target=f.nullTarget?nullptr:&r.host;
        if(f.operation==1) result=uint8_t(p->Lock(nullptr,target,f.mode));
        else if(f.operation==2) result=uint8_t(p->Unlock(nullptr,target,f.mode));
        else result=uint8_t(p->Draw(nullptr,target,f.count?f.records.data():nullptr,
            f.count,f.mode,f.colours.data(),f.clipped?&f.clip:nullptr));
        return true;
    }
    bool Release(uint32_t expected) {
        if(!Enter(2)) return false;
        uint32_t actual=FC4_RELEASE_CREATED(p);
        Log("font-renderer-verify lifetime release actual=%u expected=%u",actual,expected);
        return actual==expected;
    }
};
void Verify(const char* phase) {
    auto* director=RZGetCOMDllDirector();
    auto* com=director?director->GZCOM():nullptr;
    Native n;
    void* out=nullptr;
    if(!compat::IdentifiedVersion() || !com || !com->GetClassObject(0x08cda30b,0xa8bc2a75,&out) || !out) {
        Log("font-renderer-verify phase=%s refusal=1 reason=typed-factory no-fallback=1",phase); return;
    }
    n.p=static_cast<SmallRenderer*>(out);
    // Audit only after identity; auditing itself reads object identity data.
    if(!Original(n.p,0)) {
        Log("font-renderer-verify phase=%s refusal=1 reason=original-module-vtable release-withheld=1",phase); return;
    }
    // SDK cRZCOMDllDirector::GetClassObject's factory route calls QI; section 2
    // specifies that successful small-renderer QI contributes the owned count.
    FC4_ACQUIRED(n.p,"small renderer typed factory owned reference");
    uint32_t checks=0,diff=0,operations=0,pixels=0,callbacks=0,immutable=0;
    auto check=[&](bool ok) { ++checks; if(!ok) ++diff; };
    for(uint32_t iid:{1u,0xa8bc2a75u,0u}) {
        void* qi=reinterpret_cast<void*>(uintptr_t(0x1234));
        if(!n.Enter(0)) break;
        bool ok=n.p->QueryInterface(iid,&qi);
        check(ok==(iid!=0)); check(qi==(iid?out:reinterpret_cast<void*>(uintptr_t(0x1234))));
        if(ok && qi) {
            Native view; view.p=static_cast<SmallRenderer*>(qi);
            if(!Original(view.p,2)) { ++n.refusal; break; }
            FC4_ACQUIRED(view.p,"small renderer successful QI reference");
            check(view.Release(1)); n.gates+=view.gates; n.dispatches+=view.dispatches; n.refusal+=view.refusal;
        }
    }
    // An ordinary balanced AddRef/Release pair, never a count-field mutation.
    if(n.Enter(1)) { uint32_t refs=FC4_ADDREF(n.p); check(refs==2); check(n.Release(1)); }
    for(const auto& fixture:Fixtures()) {
        Run native(fixture),rust(fixture);
        const auto beforeN=native.ImmutableHash(),beforeR=rust.ImmutableHash();
        uint8_t nr=0xff,rr=0xff;
        if(!n.Execute(native,nr)) break;
        uint32_t status=rust.Rust(rr);
        ++operations; pixels+=fixture.pixelChecks; callbacks+=uint32_t(fixture.callbacks.size());
        bool ni=beforeN==native.ImmutableHash(),ri=beforeR==rust.ImmutableHash();
        immutable+=2; check(ni); check(ri); check(status==0);
        const bool nl=Matches(native.pixels,native.host.trace,fixture.expected,fixture.callbacks,nr,fixture.expectedResult);
        const bool rl=Matches(rust.pixels,rust.host.trace,fixture.expected,fixture.callbacks,rr,fixture.expectedResult);
        const bool pair=native.pixels==rust.pixels && SameTrace(native.host.trace,rust.host.trace) && nr==rr;
        check(nl); check(rl); check(pair);
        Log("font-renderer-verify fixture=%s op=%u mode=%u depth=%u format=%u pixels=%u callbacks=%u/%u native-literal=%u rust-literal=%u pair=%u status=%u immutable=%u/%u input-hash=%016llx/%016llx final-hash=%016llx/%016llx",
            fixture.name,fixture.operation,fixture.mode,fixture.target.depth,fixture.target.format,fixture.pixelChecks,
            uint32_t(native.host.trace.size()),uint32_t(fixture.callbacks.size()),nl,rl,pair,status,ni,ri,
            beforeN,beforeR,Hash(native.pixels.data(),native.pixels.size()),Hash(rust.pixels.data(),rust.pixels.size()));
    }
    check(n.Release(0));
    Log("font-renderer-verify phase=%s constructed operations=%u/%u pixels=%u callbacks=%u immutable-checks=%u checks=%u diff=%u refusal=%u provenance=%u/%u natural=0 source-read-observability=0 same-value-store-observability=0 target=plugin-raw-primitive factory=08CDA30B iid=A8BC2A75",
        phase,operations,uint32_t(Fixtures().size()),pixels,callbacks,immutable,checks,diff,n.refusal,n.dispatches,n.gates);
}
}
void FontRendererVerify(const char* phase) {
    // Default OFF gate precedes fixture construction and all native acquisition.
    WhenEnabled(SettingInt("font_renderer_verify","FC4_FONT_RENDERER_VERIFY",0)!=0,[&] { Verify(phase); });
}
}
