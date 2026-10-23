// SPDX-License-Identifier: GPL-3.0-or-later
#pragma once
#include "fc4_font_renderer_verify.h"
#include "cIGZUnknown.h"
#include <array>
#include <cstddef>
#include <cstdint>
#include <cstdlib>
#include <cstring>
#include <vector>

namespace fc4::font_verify {
// Public font-renderer.md sections 2–4, not a buffer owner or SDK buffer shim.
struct NativeRecord {
    fc4_fr_rect destination;
    const uint8_t* source;
    uint32_t pitch;
    int32_t width, height;
    uint32_t character;
};
static_assert(sizeof(void*) == 4 && sizeof(uint32_t) == 4 && sizeof(int32_t) == 4);
static_assert(sizeof(fc4_fr_rect) == 0x10 && sizeof(NativeRecord) == 0x24);
static_assert(offsetof(NativeRecord, destination) == 0);
static_assert(offsetof(fc4_fr_rect, left) == 0 && offsetof(fc4_fr_rect, top) == 4);
static_assert(offsetof(fc4_fr_rect, right) == 8 && offsetof(fc4_fr_rect, bottom) == 12);
static_assert(offsetof(NativeRecord, source) == 0x10 && offsetof(NativeRecord, pitch) == 0x14);
static_assert(offsetof(NativeRecord, width) == 0x18 && offsetof(NativeRecord, height) == 0x1c);
static_assert(offsetof(NativeRecord, character) == 0x20);
static_assert(sizeof(fc4_fr_trace) == 12);

class RawTarget;
class SmallRenderer : public cIGZUnknown {
public:
    virtual bool Unknown3(uint32_t) = 0; // FR1 withheld: never called.
    virtual bool Unknown4(uint32_t) = 0;
    virtual bool Lock(void*, RawTarget*, uint32_t) = 0;
    virtual bool Unlock(void*, RawTarget*, uint32_t) = 0;
    virtual bool Draw(void*, RawTarget*, const NativeRecord*, uint32_t, uint32_t,
                      const uint32_t*, const fc4_fr_rect*) = 0;
};
struct RendererSlots {
    bool(__thiscall* query)(SmallRenderer*,uint32_t,void**);
    uint32_t(__thiscall* addRef)(SmallRenderer*);
    uint32_t(__thiscall* release)(SmallRenderer*);
    bool(__thiscall* unknown3)(SmallRenderer*,uint32_t);
    bool(__thiscall* unknown4)(SmallRenderer*,uint32_t);
    bool(__thiscall* lock)(SmallRenderer*,void*,RawTarget*,uint32_t);
    bool(__thiscall* unlock)(SmallRenderer*,void*,RawTarget*,uint32_t);
    bool(__thiscall* draw)(SmallRenderer*,void*,RawTarget*,const NativeRecord*,uint32_t,uint32_t,
                           const uint32_t*,const fc4_fr_rect*);
};
static_assert(sizeof(RendererSlots)==0x20);
static_assert(offsetof(RendererSlots,query)==0 && offsetof(RendererSlots,addRef)==4);
static_assert(offsetof(RendererSlots,release)==8 && offsetof(RendererSlots,lock)==0x14);
static_assert(offsetof(RendererSlots,unlock)==0x18 && offsetof(RendererSlots,draw)==0x1c);
struct Format { uint32_t format, depth; };
static_assert(sizeof(Format) == 8);
// Layout assertions describe typed i686 thiscall entries. No raw slot calls.
struct TargetSlots {
    using Unused = void(__thiscall*)(RawTarget*);
    using Lock = bool(__thiscall*)(RawTarget*, uint32_t);
    using Dimension = int32_t(__thiscall*)(const RawTarget*);
    using GetFormat = void(__thiscall*)(const RawTarget*, Format*);
    using Word = uint32_t(__thiscall*)(const RawTarget*);
    using Pixels = void*(__thiscall*)(RawTarget*);
    Unused s00[6]; Lock lock, unlock; Unused s20;
    Dimension width, height; Unused s2c[3]; GetFormat format;
    Unused s3c; Word depth; Unused s44[17]; Pixels pixels; Word pitch;
};
static_assert(offsetof(TargetSlots, lock) == 0x18 && offsetof(TargetSlots, unlock) == 0x1c);
static_assert(offsetof(TargetSlots, width) == 0x24 && offsetof(TargetSlots, height) == 0x28);
static_assert(offsetof(TargetSlots, format) == 0x38 && offsetof(TargetSlots, depth) == 0x40);
static_assert(offsetof(TargetSlots, pixels) == 0x88 && offsetof(TargetSlots, pitch) == 0x8c);
static_assert(sizeof(TargetSlots) == 0x90);

// Plugin-owned raw primitive host. These getters do no clipping, addressing,
// blending, palette indexing or raster work. Unexpected slots are unreachable.
class RawTarget {
public:
    fc4_fr_target raw{};
    mutable std::vector<fc4_fr_trace> trace;
    void Note(uint32_t slot, uint32_t a = 0, uint32_t b = 0) const {
        if (trace.size() >= 128) std::abort();
        trace.push_back({slot,a,b});
    }
#define FR_UNUSED(n) virtual void Unused##n() { std::abort(); }
    FR_UNUSED(00) FR_UNUSED(04) FR_UNUSED(08) FR_UNUSED(0c) FR_UNUSED(10) FR_UNUSED(14)
    virtual bool Lock(uint32_t flags) { Note(0x18,flags,raw.lock_al); return raw.lock_al != 0; }
    virtual bool Unlock(uint32_t flags) { Note(0x1c,flags,raw.unlock_al); return raw.unlock_al != 0; }
    FR_UNUSED(20)
    virtual int32_t Width() const { Note(0x24,uint32_t(raw.width)); return raw.width; }
    virtual int32_t Height() const { Note(0x28,uint32_t(raw.height)); return raw.height; }
    FR_UNUSED(2c) FR_UNUSED(30) FR_UNUSED(34)
    virtual void GetFormat(Format* f) const { *f={raw.format,raw.format_depth}; Note(0x38,f->format,f->depth); }
    FR_UNUSED(3c)
    virtual uint32_t Depth() const { Note(0x40,raw.depth); return raw.depth; }
    FR_UNUSED(44) FR_UNUSED(48) FR_UNUSED(4c) FR_UNUSED(50) FR_UNUSED(54) FR_UNUSED(58)
    FR_UNUSED(5c) FR_UNUSED(60) FR_UNUSED(64) FR_UNUSED(68) FR_UNUSED(6c) FR_UNUSED(70)
    FR_UNUSED(74) FR_UNUSED(78) FR_UNUSED(7c) FR_UNUSED(80) FR_UNUSED(84)
    virtual void* Pixels() { Note(0x88); return raw.pixels; }
    virtual uint32_t Pitch() const { Note(0x8c,raw.pitch); return raw.pitch; }
#undef FR_UNUSED
};

inline bool OriginalIdentity(uintptr_t table, uintptr_t module, uintptr_t image) {
    // Published 00AC55B4 minus preferred image base 00400000. Comparison only.
    return image != 0 && module == image && table == image + 0x006c55b4;
}
inline uintptr_t EntryRva(unsigned slot) {
    // Published identities, comparison only. FR1's unnamed slots stay refused.
    switch(slot) {
        case 0: return 0x49620d;
        case 1: return 0x568b60;
        case 2: return 0x4961f6;
        case 5: return 0x496232;
        case 6: return 0x496243;
        case 7: return 0x497246;
        default: return 0;
    }
}
inline bool OriginalEntry(uintptr_t entry,uintptr_t module,uintptr_t image,unsigned slot) {
    return image!=0 && module==image && EntryRva(slot)!=0 && entry==image+EntryRva(slot);
}
template<class F> bool WhenEnabled(bool enabled, F&& run) {
    if (!enabled) return false;
    run(); return true;
}
inline bool SameTrace(const std::vector<fc4_fr_trace>& a, const std::vector<fc4_fr_trace>& b) {
    if(a.size()!=b.size()) return false;
    for(size_t i=0;i<a.size();++i)
        if(a[i].slot!=b[i].slot || a[i].a!=b[i].a || a[i].b!=b[i].b) return false;
    return true;
}
inline bool Matches(const std::vector<uint8_t>& pixels, const std::vector<fc4_fr_trace>& trace,
                    const std::vector<uint8_t>& expected, const std::vector<fc4_fr_trace>& callbacks,
                    uint8_t result, uint8_t expectedResult) {
    return pixels == expected && SameTrace(trace,callbacks) && result == expectedResult;
}
struct Fixture {
    const char* name;
    uint32_t operation=0, mode=1, count=1, pixelChecks=4;
    fc4_fr_target target{4,2,32,9,32,19,nullptr,128,0,1};
    std::array<uint8_t,128> source{};
    std::array<uint32_t,256> colours{};
    std::array<NativeRecord,2> records{};
    bool nullTarget=false, clipped=false;
    fc4_fr_rect clip{};
    std::vector<uint8_t> initial=std::vector<uint8_t>(160,0xa5);
    std::vector<uint8_t> expected=initial;
    std::vector<fc4_fr_trace> callbacks;
    uint8_t expectedResult=1;
    explicit Fixture(const char* n):name(n) {
        source[0]=0; source[1]=127; source[2]=128; source[3]=255;
        colours[0]=0x89abcdef;
        records[0]={{0,0,4,1},nullptr,12,1,1,0x20};
    }
    void Put(std::vector<uint8_t>& dst, uint32_t offset, uint32_t value, unsigned bytes) {
        for(unsigned i=0;i<bytes;++i) dst[16+offset+i]=uint8_t(value>>(i*8));
    }
    void Expected(uint32_t offset, uint32_t value, unsigned bytes) { Put(expected,offset,value,bytes); }
    void Seed(uint32_t offset,uint32_t value,unsigned bytes) {
        Put(initial,offset,value,bytes); Put(expected,offset,value,bytes);
    }
    void Queries(unsigned rasters=1,bool backing=true) {
        // Public SDK buffer colour-type depth agrees with the raw depth getter.
        target.format_depth=target.depth;
        for(unsigned i=0;i<rasters;++i) {
            callbacks.push_back({0x24,uint32_t(target.width),0});
            callbacks.push_back({0x28,uint32_t(target.height),0});
            callbacks.push_back({0x40,target.depth,0});
            if(mode==3) callbacks.push_back({0x38,target.format,target.format_depth});
            if(backing) { callbacks.push_back({0x8c,target.pitch,0}); callbacks.push_back({0x88,0,0}); }
        }
    }
};
inline std::vector<Fixture> Fixtures() {
    std::vector<Fixture> f;
    for(uint32_t depth:{16u,24u,32u}) for(uint32_t mode:{1u,2u}) {
        Fixture x(mode==1?"threshold":"palette"); x.target.depth=depth; x.mode=mode;
        unsigned bytes=depth==16?2:4;
        if(mode==1) {
            x.Expected(2*bytes,0x89abcdef,bytes); x.Expected(3*bytes,0x89abcdef,bytes);
        } else {
            x.source[1]=1; x.colours[0]=0xdeadbeef; x.colours[1]=0x12345678;
            x.colours[128]=0x89abcdef; x.colours[255]=0x76543210;
            x.Expected(bytes,0x12345678,bytes); x.Expected(2*bytes,0x89abcdef,bytes);
            x.Expected(3*bytes,0x76543210,bytes);
        }
        x.Queries(); f.push_back(x);
    }
    for(uint32_t format:{4u,5u,6u,7u}) {
        Fixture x("packed16"); x.target.depth=16; x.target.format=format; x.mode=3;
        x.colours[0]=format==4||format==6?0x7fff:0xffff;
        x.source[0]=0; x.source[1]=1; x.source[2]=7; x.source[3]=255;
        for(unsigned i=0;i<4;++i) x.Seed(2*i,0,2);
        x.Expected(2,format==7?0xf000:0,2); x.Expected(4,format==7?0xf000:0,2);
        x.Expected(6,format==7?0xfeee:format==5?0xf79e:0x7bde,2);
        x.Queries(); f.push_back(x);
        Fixture y("forced-bits16"); y.target.depth=16; y.target.format=format; y.mode=3;
        y.records[0].destination.right=1; y.pixelChecks=1; y.source[0]=1; y.colours[0]=0;
        y.Seed(0,format==5?0x20:0x8000,2); y.Expected(0,format==7?0xf000:0,2);
        y.Queries(); f.push_back(y);
    }
    { Fixture x("asymmetric-dyadic"); x.target.depth=16; x.target.format=4; x.mode=3;
      x.records[0].destination.right=1; x.pixelChecks=1; x.source[0]=64; x.colours[0]=0x1467;
      x.Seed(0,0x0443,2); x.Expected(0,0x0844,2); x.Queries(); f.push_back(x); }
    for(uint32_t depth:{24u,32u}) {
        Fixture x("packed-dword"); x.target.depth=depth; x.mode=3; x.colours[0]=0xffffffff;
        x.source[0]=0; x.source[1]=128; x.source[2]=128; x.source[3]=255;
        x.Seed(0,0,4); x.Seed(4,0,4); x.Seed(8,0,4); x.Seed(12,0,4);
        x.Expected(4,0x80808080,4); x.Expected(8,0x80808080,4); x.Expected(12,0xfefefefe,4);
        x.Queries(); f.push_back(x);
        Fixture y("packed-dword-asymmetric"); y.target.depth=depth; y.mode=3; y.colours[0]=0xffeeddcc;
        y.source[0]=0; y.source[1]=128; y.records[0].destination.right=2; y.pixelChecks=2;
        y.Seed(0,0x11223344,4); y.Seed(4,0x11223344,4);
        y.Expected(4,0x88888888,4); y.Queries(); f.push_back(y);
    }
    { Fixture x("mode4-default-format"); x.target.depth=16; x.mode=4; x.source.fill(255);
      x.Queries(); f.push_back(x); }
    { Fixture x("clip5x3-sourcewidth1"); x.target={8,6,32,9,32,36,nullptr,256,0,1};
      x.initial.assign(288,0xa5); x.expected=x.initial; x.source.fill(255); x.clipped=true;
      x.clip={1,1,6,4}; x.records[0].destination={-2,-1,7,5}; x.pixelChecks=15;
      for(uint32_t row:{40u,76u,112u}) for(uint32_t p=0;p<5;++p) x.Expected(row+4*p,0x89abcdef,4);
      // Distinct uncovered source bytes make a wrong clipping origin visible.
      x.source.fill(0); for(uint32_t row:{27u,39u,51u}) for(unsigned p=0;p<5;++p) x.source[row+p]=255;
      x.Queries(); f.push_back(x); }
    { Fixture x("fresh-record-origins"); x.count=2; x.pixelChecks=2; x.source.fill(0); x.source[0]=255;
      // First record acquires source origin (1,1), reading zero at 13. The
      // next record must restart at (0,0), reading 255, not retain that origin.
      x.records[0].destination={-1,-1,1,1}; x.records[1]=x.records[0]; x.records[1].destination={1,0,2,1};
      x.Expected(4,0x89abcdef,4); x.Queries(2); f.push_back(x); }
    for(uint32_t depth:{16u,24u,32u}) {
        Fixture x("odd-pitch"); x.target.depth=depth; x.target.pitch=depth==16?11:19;
        x.source.fill(255); x.records[0].destination={0,0,1,2}; x.pixelChecks=2;
        x.Expected(0,0x89abcdef,depth==16?2:4); x.Expected(depth==16?10:16,0x89abcdef,depth==16?2:4);
        x.Queries(); f.push_back(x);
    }
    for(uint32_t mode:{1u,3u}) for(unsigned kind=0;kind<4;++kind) {
        Fixture x("empty-or-unsupported"); x.mode=mode; x.pixelChecks=0;
        if(kind==0) x.records[0].destination={2,0,2,1};
        if(kind==1) x.records[0].destination={3,0,1,1};
        if(kind==2) x.records[0].destination={4,0,6,1};
        if(kind==3) x.target.depth=8;
        x.Queries(1,false); f.push_back(x);
    }
    { Fixture x("zero-count-null"); x.count=0; x.nullTarget=true; x.pixelChecks=0; f.push_back(x); }
    { Fixture x("control-character"); x.records[0].character=0x1f; x.nullTarget=true; x.pixelChecks=0; f.push_back(x); }
    for(uint32_t op:{1u,2u}) for(uint32_t mode:{1u,2u,3u,4u}) for(uint8_t al:{uint8_t(0),uint8_t(1)}) {
        Fixture x("lock-forward"); x.operation=op; x.mode=mode; x.count=0; x.pixelChecks=0;
        x.target.lock_al=al; x.target.unlock_al=al; x.expectedResult=al;
        x.callbacks.push_back({op==1?0x18u:0x1cu,0x8080,al}); f.push_back(x);
    }
    return f;
}
inline uint64_t Hash(const void* bytes,size_t len,uint64_t h=14695981039346656037ull) {
    auto* p=static_cast<const uint8_t*>(bytes);
    for(size_t i=0;i<len;++i) { h^=p[i]; h*=1099511628211ull; } return h;
}
struct Run {
    Fixture input;
    std::vector<uint8_t> pixels;
    RawTarget host;
    std::array<fc4_fr_record,2> records{};
    explicit Run(const Fixture& f):input(f),pixels(f.initial) {
        host.raw=input.target; host.raw.pixels=pixels.data()+16;
        for(unsigned i=0;i<2;++i) {
            input.records[i].source=input.source.data();
            auto& r=input.records[i];
            records[i]={r.destination,input.source.data(),128,r.pitch,r.width,r.height,r.character};
        }
    }
    uint64_t ImmutableHash() const {
        auto h=Hash(input.source.data(),input.source.size());
        h=Hash(input.colours.data(),sizeof(input.colours),h);
        h=Hash(input.records.data(),sizeof(input.records),h);
        h=Hash(records.data(),sizeof(records),h);
        return Hash(&input.clip,sizeof(input.clip),h);
    }
    uint32_t Rust(uint8_t& result) {
        std::array<fc4_fr_trace,128> trace{}; uint32_t count=0;
        uint32_t status=fc4_font_renderer_verify(input.operation,input.nullTarget?nullptr:&host.raw,
            input.count?records.data():nullptr,input.count,input.mode,
            reinterpret_cast<const uint8_t*>(input.colours.data()),sizeof(input.colours),
            input.clipped?&input.clip:nullptr,trace.data(),uint32_t(trace.size()),&count,&result);
        if(count<=trace.size()) host.trace.assign(trace.begin(),trace.begin()+count);
        return status;
    }
};
} // namespace fc4::font_verify
