// SPDX-License-Identifier: GPL-3.0-or-later
#include "FC4FontRendererFixtures.h"
#include <cstdio>
using namespace fc4::font_verify;
int main() {
    unsigned checks=0,failed=0;
    auto check=[&](bool ok) { ++checks; if(!ok) ++failed; };
    unsigned mocks=0,owners=0;
    check(!WhenEnabled(false,[&] { ++mocks; ++owners; })); check(mocks==0 && owners==0);
    check(OriginalIdentity(0xac55b4,0x400000,0x400000));
    check(!OriginalIdentity(0xac14bc,0x400000,0x400000)); // same IID, wrong holder
    check(!OriginalIdentity(0xac55b4,0x500000,0x400000));
    check(!OriginalIdentity(0xac55b4,0x400000,0));
    unsigned operations=0,pixels=0,callbacks=0;
    for(const auto& f:Fixtures()) {
        Run r(f); auto before=r.ImmutableHash(); uint8_t result=0xff;
        uint32_t status=r.Rust(result); check(status==0); check(before==r.ImmutableHash());
        bool literal=Matches(r.pixels,r.host.trace,f.expected,f.callbacks,result,f.expectedResult);
        check(literal);
        if(!literal) std::printf("literal FAIL %s mode=%u depth=%u format=%u status=%u\n",f.name,f.mode,f.target.depth,f.target.format,status);
        auto mutated=r.pixels; mutated[16]^=1;
        check(!Matches(mutated,r.host.trace,f.expected,f.callbacks,result,f.expectedResult));
        auto trace=r.host.trace; trace.push_back({0xffffffff,0,0});
        check(!Matches(r.pixels,trace,f.expected,f.callbacks,result,f.expectedResult));
        // Exercise the actual typed C++ target entries independently of original execution.
        RawTarget host; host.raw=r.host.raw;
        RawTarget* v=&host;
        check(v->Width()==f.target.width); check(v->Height()==f.target.height);
        check(v->Depth()==f.target.depth); Format fmt{}; v->GetFormat(&fmt);
        check(fmt.format==f.target.format && fmt.depth==f.target.format_depth);
        check(v->Pitch()==f.target.pitch); check(v->Pixels()==r.host.raw.pixels);
        check(v->Lock(0x8080)==(f.target.lock_al!=0)); check(v->Unlock(0x8080)==(f.target.unlock_al!=0));
        ++operations; pixels+=f.pixelChecks; callbacks+=unsigned(f.callbacks.size());
    }
    std::printf("font renderer bridge operations=%u pixels=%u callbacks=%u checks=%u failed=%u native-original=0\n",operations,pixels,callbacks,checks,failed);
    return failed?1:0;
}
