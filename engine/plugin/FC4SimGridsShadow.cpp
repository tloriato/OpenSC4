// SPDX-License-Identifier: GPL-3.0-or-later
// Same-process original-interface/shipped-cFC4SimGrid verification. No original layout reads,
// vtable patches, host-backed Rust grids, fallback, or post-bootstrap resync.
#include "FC4SimGridsShadow.h"
#include "FC4Config.h"
#include "FC4Log.h"
#include "FC4RefAudit.h"
#include "FC4Streams.h"
#include "FC4Subsystems.h"
#include "FC4WriteCapture.h"
#include "FC4SimGridsMethods.h"
#include "cFC4SimGrid.h"
#include "cIGZCOM.h"
#include "cIGZApp.h"
#include "cIGZFrameWork.h"
#include "cIGZSerializable.h"
#include "cISC4App.h"
#include "cISC4City.h"
#include "cISC4SimGrid.h"
#include "cRZCOMDllDirector.h"
#include <algorithm>
#include <cstring>
#include <map>
#include <memory>
#include <mutex>
#include <string>
#include <type_traits>
#include <vector>

namespace fc4 { namespace {
bool Enabled() { static const bool on = SettingInt("simgrids_shadow", "FC4_SIMGRIDS_SHADOW", 0) != 0; return on; }
template<class T> void Append(std::vector<uint8_t>& b, const T& v) {
    const auto* p = reinterpret_cast<const uint8_t*>(&v); b.insert(b.end(), p, p + sizeof(v));
}
template<class T> T Field(const std::vector<uint8_t>& b, size_t off) {
    T v{}; if (off + sizeof(v) <= b.size()) std::memcpy(&v, b.data() + off, sizeof(v)); return v;
}
struct Production {
    uint64_t reads=0, writes=0;
    virtual ~Production() = default;
    virtual bool Read(const std::vector<uint8_t>&) = 0;
    virtual bool Write(std::vector<uint8_t>&) = 0;
    virtual bool Run(uint32_t,const int32_t*,const uint8_t*,uint32_t,std::vector<uint8_t>&) = 0;
    virtual uint64_t Calls() const = 0;
    virtual uint64_t Gets() const = 0;
    virtual uint64_t Sets() const = 0;
    virtual uint64_t BulkGets() const = 0;
};
std::unique_ptr<Production> MakeProduction(uint32_t kind);
struct Shadow {
    uint32_t kind = 0;
    std::string owner;
    std::vector<uint8_t> record;
    std::unique_ptr<Production> production;
    uint64_t detectionChecks=0;
    uint64_t comparisons = 0, exact = 0, refused = 0, failures = 0, calls = 0, changed = 0;
    uint64_t get = 0, bulkRead = 0, set = 0, bulkWrite = 0, fill = 0, shutdown = 0;
    bool Apply(uint32_t op, std::vector<uint8_t>& out, int32_t x = 0, int32_t z = 0,
               int32_t w = 0, int32_t h = 0, const void* data = nullptr, uint32_t n = 0) {
        int32_t a[4] = {x,z,w,h};
        out.clear();
        if(op==0) { // Exactly one natural bootstrap; later Read only from the fixture tape.
            if(production) { ++refused; return false; }
            production=MakeProduction(kind);
            if(!production || !production->Read(record) || !production->Write(out)) { ++failures; return false; }
            return true;
        }
        if(!production || !production->Run(op,a,static_cast<const uint8_t*>(data),n,out)) { ++refused; return false; }
        if(op==1 || op==2 || op==3 || op==4 || op==5 || op==8 || op==9 || op==12 || op==13 || op==14) return true;
        if(!production->Write(out)) { ++failures; return false; }
        return true;
    }
    void Check(const void* p, size_t n, const std::vector<uint8_t>& expected, const char* what) {
        ++comparisons;
        if (SimGridsBuffersEqual(p,n,expected)) ++exact;
        else if (comparisons - exact <= 8) {
            const auto* bytes=static_cast<const uint8_t*>(p); size_t at=0;
            while(at<std::min(n,expected.size()) && bytes[at]==expected[at]) ++at;
            Log("simgrids-shadow: difference op=%s clsid=%08X actual_bytes=%u expected_bytes=%u comparison=%llu offset=%u actual=%02X expected=%02X", what, kind, static_cast<unsigned>(n), static_cast<unsigned>(expected.size()), comparisons,static_cast<unsigned>(at),at<n?bytes[at]:0,at<expected.size()?expected[at]:0);
        }
    }
    void Update(uint32_t op, int32_t x = 0, int32_t z = 0, const void* data = nullptr, uint32_t n = 0) {
        std::vector<uint8_t> out;
        if (Apply(op, out, x, z, 0, 0, data, n) && op!=5 && op!=8 && op!=9) { changed += out != record; record.swap(out); }
        // Refusals retain the previous expected state, never native fallback.
    }
    bool Capture(cIGZUnknown* obj, std::vector<uint8_t>& b) {
        WriteCapture cap;
        if (!cap.Capture(obj)) { ++failures; return false; }
        b = cap.Bytes(); return true;
    }
    void RecordCheck(cIGZUnknown* obj) {
        if(!production || !production->Write(record)) { ++failures; return; }
        std::vector<uint8_t> b;
        if (Capture(obj,b)) Check(b.data(), b.size(), record, "complete-write");
    }
    void DetectionCheck() {
        std::vector<uint8_t> a,b;
        if(!production || !production->Write(a) || a.empty()) { ++failures; return; }
        b=a; b.back()^=1;
        // Same complete-buffer comparator used for observed results; injected
        // corruption is a separate negative control, never equality credit.
        if(!SimGridsBuffersEqual(b.data(),b.size(),a) && !SimGridsBuffersEqual(a.data(),a.size()-1,a) && SimGridsBuffersEqual(a.data(),a.size(),a)) ++detectionChecks; else ++failures;
    }
    void Boolean(bool native,uint32_t op,const char* what) {
        std::vector<uint8_t> out; const uint8_t actual=native?1:0;
        if(Apply(op,out)) Check(&actual,1,out,what);
    }
    void Report(const char* source, uint64_t token) const {
        Log("simgrids-shadow: source=%s owner=%s token=%p clsid=%08X exact=%llu/%llu differences=%llu refused=%llu failures=%llu calls=%llu get=%llu bulk_read=%llu set=%llu bulk_write=%llu fill=%llu shutdown=%llu changed=%llu bootstrap=1 resync=0 fallback=0 backend=shipped-cpp method_calls=%llu method_get=%llu method_set=%llu method_bulk_get=%llu method_read=%llu method_write=%llu detection_checks=%llu", source, owner.c_str(), FromToken(token), kind, exact, comparisons, comparisons-exact, refused, failures, calls, get, bulkRead, set, bulkWrite, fill, shutdown, changed,production?production->Calls():0,production?production->Gets():0,production?production->Sets():0,production?production->BulkGets():0,production?production->reads:0,production?production->writes:0,detectionChecks);
    }
};
std::mutex mu;
std::map<uint64_t, Shadow> natural;
std::map<uint64_t, std::pair<std::string,uint64_t>> rejected;
std::map<uint64_t, Shadow>::iterator Find(uint64_t t) {
    auto r=rejected.find(t); if(r!=rejected.end()) ++r->second.second;
    return natural.find(t);
}

// Input stream exclusively for constructed native Read cases. No segment refs.
class Input final : public cIGZIStream {
public:
    explicit Input(const std::vector<uint8_t>& b) : b_(b) {}
    bool QueryInterface(uint32_t id, void** out) override { if (id != 1) return false; *out = this; return true; }
    uint32_t AddRef() override { return 1; } uint32_t Release() override { return 1; }
    bool Skip(uint32_t n) override { if (at_ + n > b_.size()) { good_ = false; return false; } at_ += n; return true; }
#define SG_READ(Name, T) bool Name(T& v) override { return GetVoid(&v, sizeof(v)); }
    SG_READ(GetSint8,int8_t) SG_READ(GetUint8,uint8_t) SG_READ(GetSint16,int16_t) SG_READ(GetUint16,uint16_t)
    SG_READ(GetSint32,int32_t) SG_READ(GetUint32,uint32_t) SG_READ(GetSint64,int64_t) SG_READ(GetUint64,uint64_t)
    SG_READ(GetFloat32,float) SG_READ(GetFloat64,double)
#undef SG_READ
    bool GetRZCharStr(char*,uint32_t) override { good_=false; return false; }
    bool GetGZStr(cIGZString&) override { good_=false; return false; }
    bool GetGZSerializable(cIGZSerializable&) override { good_=false; return false; }
    bool GetVoid(void* p, uint32_t n) override { if (!good_ || at_ + n > b_.size()) { good_=false; return false; } std::memcpy(p,b_.data()+at_,n); at_+=n; return true; }
    int32_t GetError() override { return good_ ? 0 : 1; }
    int32_t SetUserData(cIGZVariant*) override { return 0; } int32_t GetUserData() override { return 0; }
    size_t Position() const { return at_; }
private: const std::vector<uint8_t>& b_; size_t at_=0; bool good_=true;
};

// These references belong only to the verifier. The direct shipped factories
// return count-0 objects; take one reference, QI typed/serializable references,
// then release the factory reference. Never expose these objects to the host.
// Natural map operations and destruction run under mu (including other getter
// threads); fixtures are local to the startup thread and never published.
template<class T> class OwnedProduction final : public Production {
public:
    OwnedProduction(cISC4SimGrid<T>* g,cIGZSerializable* s) : grid(g),serial(s),methods(g) {}
    ~OwnedProduction() override {
        const uint32_t remaining=FC4_RELEASE(serial);
        const uint32_t last=FC4_RELEASE(grid);
        Log("simgrids-shadow: source=production-cleanup backend=shipped-cpp serial_release=%u final_release=%u expected=1,0",remaining,last);
    }
    bool Read(const std::vector<uint8_t>& b) override { Input input(b); ++reads; return serial->Read(input) && input.Position()==b.size() && input.GetError()==0; }
    bool Write(std::vector<uint8_t>& b) override { WriteCapture cap; ++writes; if(!cap.Capture(grid)) return false; b=cap.Bytes(); return true; }
    bool Run(uint32_t op,const int32_t* a,const uint8_t* d,uint32_t n,std::vector<uint8_t>& out) override { return methods.Run(op,a,d,n,out); }
    uint64_t Calls() const override { return methods.calls; }
    uint64_t Gets() const override { return methods.scalarGet; }
    uint64_t Sets() const override { return methods.scalarSet; }
    uint64_t BulkGets() const override { return methods.bulkGet; }
private:
    cISC4SimGrid<T>* grid;
    cIGZSerializable* serial;
    SimGridsMethods<T> methods;
};
template<class T> std::unique_ptr<Production> Own(cIGZUnknown* raw) {
    if(!raw) return {};
    FC4_ADDREF(raw);
    cISC4SimGrid<T>* grid=nullptr; cIGZSerializable* serial=nullptr;
    if(raw->QueryInterface(0xC9BD939A,reinterpret_cast<void**>(&grid)) && grid) FC4_ACQUIRED(grid,"QueryInterface (owned simgrids production)");
    if(raw->QueryInterface(GZIID_cIGZSerializable,reinterpret_cast<void**>(&serial)) && serial) FC4_ACQUIRED(serial,"QueryInterface (owned simgrids production)");
    FC4_RELEASE(raw);
    if(!grid || !serial) { if(grid) FC4_RELEASE(grid); if(serial) FC4_RELEASE(serial); return {}; }
    return std::make_unique<OwnedProduction<T>>(grid,serial);
}
std::unique_ptr<Production> MakeProduction(uint32_t kind) {
    switch(kind) {
    case 0x49B9E602: return Own<uint8_t>(CreateFC4SimGridU8());
    case 0x49B9E603: return Own<int8_t>(CreateFC4SimGridS8());
    case 0x49B9E604: return Own<uint16_t>(CreateFC4SimGridU16());
    case 0x49B9E605: return Own<int16_t>(CreateFC4SimGridS16());
    case 0x49B9E606: return Own<uint32_t>(CreateFC4SimGridU32());
    case 0x49B9E607: return Own<int32_t>(CreateFC4SimGridS32());
    case 0x49B9E608: return Own<uint64_t>(CreateFC4SimGridU64());
    case 0x49B9E609: return Own<int64_t>(CreateFC4SimGridS64());
    case 0x49B9E60A: return Own<float>(CreateFC4SimGridF32());
    case 0x49B9E60B: return Own<double>(CreateFC4SimGridF64());
    default: return {};
    }
}

template<class T> void Fixture(uint32_t kind, const std::vector<uint8_t>& city) {
    Shadow s; s.kind = kind;
    Log("simgrids-shadow: source=fixture phase=factory clsid=%08X",kind);
    cISC4SimGrid<T>* g = nullptr;
    cIGZCOM* com = GZCOM();
    if (!com || !com->GetClassObject(kind, 0xC9BD939A, reinterpret_cast<void**>(&g)) || !g) {
        Log("simgrids-shadow: source=fixture clsid=%08X allocation_failed=1 comparisons=0",kind); return;
    }
    FC4_ACQUIRED(g,"GetClassObject (simgrids fixture)");
    // The published contract specifies sized/initialized grids, not a
    // factory object's uninitialized Write. Establish that precondition before
    // capturing the input. This is fixture setup, not equality credit.
    Log("simgrids-shadow: source=fixture phase=initialize clsid=%08X",kind);
    const bool initialInit=g->Init();
    g->SetTractSize(8);
    Log("simgrids-shadow: source=fixture phase=capture-sized clsid=%08X",kind);
    // Bootstrap is an input, excluded from equality numerator/denominator.
    s.Capture(g,s.record);
    std::vector<uint8_t> bootstrap;
    if(!s.Apply(0,bootstrap) || bootstrap!=s.record) { ++s.failures; FC4_RELEASE_CREATED(g); s.Report("fixture-bootstrap-failed",0); return; }
    s.DetectionCheck();
    s.Boolean(initialInit,8,"initial-Init-return");
    s.Boolean(g->Init(),8,"Init-return"); s.RecordCheck(g);
    g->SetInstanceID(0x51A00000u + kind); s.Update(10,static_cast<int32_t>(0x51A00000u+kind));
    for (int size : {8, 4, 3, 8, 8}) {
        g->SetTractSize(size); // §4.3: original bool return deliberately ignored.
        std::vector<uint8_t> out;
        if (s.Apply(7,out,size,0,0,0,city.data(),static_cast<uint32_t>(city.size()))) s.record.swap(out);
        s.RecordCheck(g);
        T v = static_cast<T>(7); g->SetTractValues(v); s.Update(6,0,0,&v,sizeof(v)); s.RecordCheck(g);
        // Signed/fractional/high-bit formats exercise their actual T channel.
        T a = static_cast<T>(-3); T b = static_cast<T>(1.9);
        if constexpr (std::is_unsigned<T>::value) a = static_cast<T>(~T(0));
        if constexpr (sizeof(T) == 8 && std::is_integral<T>::value) a = static_cast<T>(0x123456789ABCDEFull);
        g->SetTractValue(1,1,a); s.Update(5,1,1,&a,sizeof(a));
        g->SetTractValue(0,0,b); s.Update(5,0,0,&b,sizeof(b)); s.RecordCheck(g);
        for (uint32_t op : {1u,2u,3u,4u}) {
            T value{}; int x=0,z=0,w=2,h=2;
            if (op==1) { x=1; z=1; value=g->GetTractValue(x,z); }
            if (op==2) { x=size+1; z=size+1; value=g->GetCellValue(x,z); }
            if (op==3) { w=size*2; h=size*2; value=g->GetAverageValueInCellRect(x,z,w,h); }
            if (op==4) value=g->GetAverageValueInTractRect(x,z,w,h);
            if(s.Apply(op,out,x,z,w,h)) s.Check(&value,sizeof(value),out,"typed-return");
        }
        std::vector<uint8_t> geometry;
        for (int32_t v2 : {g->GetTractSize(),g->GetTractShift(),g->GetTractCountX(),g->GetTractCountZ()}) Append(geometry,v2);
        for(float v2 : {g->GetTractWidthX(),g->GetTractWidthZ(),g->GetOneOverTractWidthX(),g->GetOneOverTractWidthZ()}) Append(geometry,v2);
        if(s.Apply(12,out)) s.Check(geometry.data(),geometry.size(),out,"geometry");
        geometry.clear(); int32_t tx=0,tz=0; float px=0,pz=0;
        g->PositionToTract(100.0f,-10.0f,tx,tz); Append(geometry,tx); Append(geometry,tz);
        g->TractCornerToPosition(3,4,px,pz); Append(geometry,px); Append(geometry,pz);
        g->TractCenterToPosition(3,4,px,pz); Append(geometry,px); Append(geometry,pz);
        for(uint8_t v2 : {uint8_t(g->TractIsInBounds(uint32_t(-1),0)),uint8_t(g->TractIsInBounds(g->GetTractCountX()-1,g->GetTractCountZ()-1)),uint8_t(g->TractIsInBounds(g->GetTractCountX(),0))}) Append(geometry,v2);
        Append(geometry,g->GetInstanceID());
        if(s.Apply(13,out)) s.Check(geometry.data(),geometry.size(),out,"coordinates-bounds-instance");
    }
    cIGZSerializable* serial=nullptr;
    if(g->QueryInterface(GZIID_cIGZSerializable,reinterpret_cast<void**>(&serial)) && serial) {
        FC4_ACQUIRED(serial,"QueryInterface (simgrids fixture)");
        auto saved=s.record;
        T v=static_cast<T>(23); g->SetTractValues(v); s.Update(6,0,0,&v,sizeof(v));
        Input input(saved); uint8_t readResult=serial->Read(input)?1:0;
        // Explicit fixture operation input, replayed to both objects. This is
        // not a later native snapshot: saved was written by production state.
        uint8_t expectedRead=s.production->Read(saved)?1:0;
        s.Check(&readResult,1,std::vector<uint8_t>{expectedRead},"Read-return"); s.RecordCheck(g);
        // Valid fields are literal, not recomputed from the city on Read.
        auto stored=saved; uint32_t storedClass=0x49B9E603;
        std::memcpy(stored.data()+3,&storedClass,4);
        float storedWidth=37.0f; std::memcpy(stored.data()+27,&storedWidth,4);
        Input literal(stored); readResult=serial->Read(literal)?1:0;
        expectedRead=s.production->Read(stored)?1:0;
        s.Check(&readResult,1,std::vector<uint8_t>{expectedRead},"Read-literal-return"); s.RecordCheck(g);
        std::vector<uint8_t> consumed; Append(consumed,static_cast<uint32_t>(saved.size()));
        uint32_t pos=static_cast<uint32_t>(input.Position()); s.Check(&pos,4,consumed,"Read-consumed");
        saved[0]=2; Input invalid(saved); uint8_t result=serial->Read(invalid)?1:0;
        expectedRead=s.production->Read(saved)?1:0;
        s.Check(&result,1,std::vector<uint8_t>{expectedRead},"invalid-version-return"); s.RecordCheck(g);
        FC4_RELEASE(serial);
    } else ++s.failures;
    s.Boolean(g->Shutdown(),9,"Shutdown-return"); s.RecordCheck(g);
    // Only our extra reference is released; final reference belongs to this fixture.
    const uint32_t raised=FC4_ADDREF(g); uint32_t lowered=FC4_RELEASE(g);
    std::vector<uint8_t> raisedExpected; Append(raisedExpected,uint32_t(2)); s.Check(&raised,4,raisedExpected,"AddRef-count");
    std::vector<uint8_t> expected; Append(expected,raised-1); s.Check(&lowered,4,expected,"Release-decrement");
    uint32_t last=FC4_RELEASE_CREATED(g); expected.clear(); Append(expected,uint32_t(0)); s.Check(&last,4,expected,"Release-destroy");
    Log("simgrids-shadow: source=fixture clsid=%08X exact=%llu/%llu differences=%llu refused=%llu failures=%llu allocation=1 final_release=%u bootstrap=1 resync=0 fallback=0",kind,s.exact,s.comparisons,s.comparisons-s.exact,s.refused,s.failures,last);
    s.Report("fixture-methods",0);
}
} // namespace

void SimGridsShadowFixtures() {
    if (!Enabled()) return;
    if (FindEnabledClass(0x49B9E602)) { Log("simgrids-shadow: source=fixture refused=10 reason=replacement-factories comparisons=0"); return; }
    // Use the public application/city interface; no inferred city dimensions.
    cISC4App* app=nullptr;
    auto* fw=RZGetFrameWork(); auto* base=fw?fw->Application():nullptr;
    if(!base || !base->QueryInterface(GZIID_cISC4App,reinterpret_cast<void**>(&app)) || !app) return;
    FC4_ACQUIRED(app,"QueryInterface (simgrids fixture city)");
    auto* city=app->GetCity(); std::vector<uint8_t> dims;
    if(city) { Append(dims,static_cast<int32_t>(city->CellCountX())); Append(dims,static_cast<int32_t>(city->CellCountZ())); Append(dims,city->CellWidthX()); Append(dims,city->CellWidthZ()); }
    FC4_RELEASE(app);
    if(dims.size()!=16) { Log("simgrids-shadow: source=fixture refused=10 reason=no-city comparisons=0"); return; }
    Fixture<uint8_t>(0x49B9E602,dims); Fixture<int8_t>(0x49B9E603,dims);
    Fixture<uint16_t>(0x49B9E604,dims); Fixture<int16_t>(0x49B9E605,dims);
    Fixture<uint32_t>(0x49B9E606,dims); Fixture<int32_t>(0x49B9E607,dims);
    Fixture<uint64_t>(0x49B9E608,dims); Fixture<int64_t>(0x49B9E609,dims);
    Fixture<float>(0x49B9E60A,dims); Fixture<double>(0x49B9E60B,dims);
}

void SimGridsShadowTrack(const char* owner,uint64_t t) {
    if(!Enabled() || !t || !owner || (std::strcmp(owner,"power")!=0 && std::strcmp(owner,"pollution")!=0)) return;
    std::lock_guard<std::mutex> lock(mu);
    if(natural.count(t) || rejected.count(t)) return;
    Shadow s; s.owner=owner;
    if(!s.Capture(FromToken(t),s.record)) { Log("simgrids-shadow: source=natural owner=power bootstrap_failed=1 token=%p",FromToken(t)); return; }
    s.kind=Field<uint32_t>(s.record,3);
    if(FindEnabledClass(s.kind)) { // Public factory provenance, no vtable guesses.
        rejected.emplace(t,std::make_pair(std::string(owner),uint64_t(0)));
        Log("simgrids-shadow: source=natural owner=%s refused_replacement=1 token=%p comparisons=0",owner,FromToken(t));
        return;
    }
    std::vector<uint8_t> roundtrip;
    if(!s.Apply(0,roundtrip) || roundtrip!=s.record) { Log("simgrids-shadow: source=natural owner=power bootstrap_failed=1 kind=%08X",s.kind); return; }
    Log("simgrids-shadow: source=natural owner=%s token=%p clsid=%08X instance=%08X shape=%dx%d bootstrap=1 comparisons=0",owner,FromToken(t),s.kind,Field<uint32_t>(s.record,7),Field<int32_t>(s.record,19),Field<int32_t>(s.record,23));
    natural.emplace(t,std::move(s));
}
void SimGridsShadowValue(uint64_t t,int32_t x,int32_t z,const void* value,uint32_t size) {
    if(!Enabled()) return; std::lock_guard<std::mutex> lock(mu); auto it=Find(t); if(it==natural.end()) return;
    auto& s=it->second; ++s.calls; ++s.get; std::vector<uint8_t> out;
    if(s.Apply(1,out,x,z)) s.Check(value,size,out,"natural-get");
}
void SimGridsShadowBulkRead(uint64_t t,const void* values,uint32_t count,uint32_t size) {
    if(!Enabled()) return; std::lock_guard<std::mutex> lock(mu); auto it=Find(t); if(it==natural.end()) return;
    auto& s=it->second; ++s.calls; ++s.bulkRead;
    std::vector<uint8_t> expected;
    if(s.Apply(14,expected)) s.Check(values,static_cast<size_t>(count)*size,expected,"natural-bulk-read");
    s.RecordCheck(FromToken(t));
}
void SimGridsShadowSet(uint64_t t,int32_t x,int32_t z,const void* value,uint32_t size) {
    if(!Enabled()) return; std::lock_guard<std::mutex> lock(mu); auto it=Find(t); if(it==natural.end()) return;
    auto& s=it->second; ++s.calls; ++s.set; s.Update(5,x,z,value,size);
    // Full result checked at bulk read/write, Shutdown and release; scalar get checked immediately.
}
void SimGridsShadowFill(uint64_t t,double value) {
    if(!Enabled()) return; std::lock_guard<std::mutex> lock(mu); auto it=Find(t); if(it==natural.end()) return;
    auto& s=it->second; ++s.calls; ++s.fill;
    std::vector<uint8_t> v;
    switch(s.kind) {
        case 0x49B9E602: Append(v,static_cast<uint8_t>(value)); break;
        case 0x49B9E603: Append(v,static_cast<int8_t>(value)); break;
        case 0x49B9E604: Append(v,static_cast<uint16_t>(value)); break;
        case 0x49B9E605: Append(v,static_cast<int16_t>(value)); break;
        case 0x49B9E606: Append(v,static_cast<uint32_t>(value)); break;
        case 0x49B9E60A: Append(v,static_cast<float>(value)); break;
        default: ++s.refused; return;
    }
    s.Update(6,0,0,v.data(),static_cast<uint32_t>(v.size())); s.RecordCheck(FromToken(t));
}
void SimGridsShadowBulkWrite(uint64_t t,const uint32_t* indices,const void* values,uint32_t count,uint32_t size) {
    if(!Enabled()) return; std::lock_guard<std::mutex> lock(mu); auto it=Find(t); if(it==natural.end()) return;
    auto& s=it->second; ++s.calls; ++s.bulkWrite; std::vector<uint8_t> data;
    const auto* v=static_cast<const uint8_t*>(values);
    for(uint32_t i=0;i<count;++i) { Append(data,indices[i]); data.insert(data.end(),v+i*size,v+(i+1)*size); }
    s.Update(11,0,0,data.data(),static_cast<uint32_t>(data.size())); s.RecordCheck(FromToken(t));
}
void SimGridsShadowShutdown(uint64_t t,bool result) {
    if(!Enabled()) return; std::lock_guard<std::mutex> lock(mu); auto it=Find(t); if(it==natural.end()) return;
    auto& s=it->second; ++s.calls; ++s.shutdown; uint8_t r=result?1:0;
    std::vector<uint8_t> expected; if(s.Apply(9,expected)) s.Check(&r,1,expected,"natural-Shutdown-return"); s.RecordCheck(FromToken(t)); s.Report("natural-shutdown",t);
}
void SimGridsShadowRelease(uint64_t t) {
    if(!Enabled()) return; std::lock_guard<std::mutex> lock(mu);
    auto r=rejected.find(t);
    if(r!=rejected.end()) { Log("simgrids-shadow: source=natural-release owner=%s token=%p refused_replacement=1 unverified_calls=%llu exact=0/0 resync=0 fallback=0",r->second.first.c_str(),FromToken(t),r->second.second); rejected.erase(r); return; }
    auto it=natural.find(t); if(it==natural.end()) return;
    it->second.RecordCheck(FromToken(t)); it->second.Report("natural-release",t); natural.erase(it);
}
} // namespace fc4
