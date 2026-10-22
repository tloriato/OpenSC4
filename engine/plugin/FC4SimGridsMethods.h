// SPDX-License-Identifier: GPL-3.0-or-later
// Operation tape adapter: calls the shipped typed interface, never computes
// grid results or reads serialized value offsets. Pointers are borrowed from
// the owning shadow; all access, including destruction, is externally locked.
#pragma once
#include "cISC4SimGrid.h"
#include <cstdint>
#include <cstring>
#include <limits>
#include <vector>

namespace fc4 {
inline bool SimGridsBuffersEqual(const void* p,size_t n,const std::vector<uint8_t>& expected) {
    return n==expected.size() && (n==0 || std::memcmp(p,expected.data(),n)==0);
}
template<class T> class SimGridsMethods {
public:
    explicit SimGridsMethods(cISC4SimGrid<T>* grid) : g(grid) {}
    uint64_t calls=0, scalarGet=0, scalarSet=0, bulkGet=0;
    bool Run(uint32_t op, const int32_t* a, const uint8_t* data, uint32_t n, std::vector<uint8_t>& out) {
        out.clear();
        const int32_t x=a[0], z=a[1], w=a[2], h=a[3];
        auto in=[&](int64_t tx,int64_t tz) { return tx>=0 && tz>=0 && tx<g->GetTractCountX() && tz<g->GetTractCountZ(); };
        auto value=[&](T& v) { if(n!=sizeof(T) || !data) return false; std::memcpy(&v,data,sizeof(T)); return true; };
        if(op>=1 && op<=5) {
            int64_t tx=x,tz=z,ex=x,ez=z;
            if(op==2 || op==3) {
                const int shift=g->GetTractShift();
                if(shift<0 || shift>30) return false;
                tx=x>>shift; tz=z>>shift;
                if(op==3) { ex=(int64_t(x)+w-1)>>shift; ez=(int64_t(z)+h-1)>>shift; }
                else { ex=tx; ez=tz; }
            } else if(op==4) { ex=int64_t(x)+w-1; ez=int64_t(z)+h-1; }
            if(!in(tx,tz) || !in(ex,ez) || ((op==3 || op==4) && (w<=0 || h<=0 || int64_t(x)+w>INT32_MAX || int64_t(z)+h>INT32_MAX))) return false;
        }
        switch(op) {
        case 1: Put(out,g->GetTractValue(x,z)); ++calls; ++scalarGet; return true;
        case 2: Put(out,g->GetCellValue(x,z)); ++calls; return true;
        case 3: Put(out,g->GetAverageValueInCellRect(x,z,w,h)); ++calls; return true;
        case 4: Put(out,g->GetAverageValueInTractRect(x,z,w,h)); ++calls; return true;
        case 5: { T v{}; if(!value(v)) return false; g->SetTractValue(x,z,v); ++calls; ++scalarSet; return true; }
        case 6: { T v{}; if(!value(v)) return false; g->SetTractValues(v); ++calls; return true; }
        case 7: g->SetTractSize(x); ++calls; return true; // public city getters inside production method
        case 8: Put(out,uint8_t(g->Init())); ++calls; return true;
        case 9: Put(out,uint8_t(g->Shutdown())); ++calls; return true;
        case 10: g->SetInstanceID(uint32_t(x)); ++calls; return true;
        case 11: {
            if(n%(4+sizeof(T)) || (n && !data)) return false;
            const uint64_t nz=uint32_t(g->GetTractCountZ()), nx=uint32_t(g->GetTractCountX());
            for(uint32_t i=0;i<n;i+=4+sizeof(T)) { uint32_t index; std::memcpy(&index,data+i,4); if(!nz || index>=nx*nz) return false; }
            for(uint32_t i=0;i<n;i+=4+sizeof(T)) { uint32_t index; T v; std::memcpy(&index,data+i,4); std::memcpy(&v,data+i+4,sizeof(T)); g->SetTractValue(int32_t(index/nz),int32_t(index%nz),v); ++calls; ++scalarSet; }
            return true;
        }
        case 12:
            Put(out,g->GetTractSize()); Put(out,g->GetTractShift()); Put(out,g->GetTractCountX()); Put(out,g->GetTractCountZ());
            Put(out,g->GetTractWidthX()); Put(out,g->GetTractWidthZ()); Put(out,g->GetOneOverTractWidthX()); Put(out,g->GetOneOverTractWidthZ()); calls+=8; return true;
        case 13: {
            int32_t tx=0,tz=0; float px=0,pz=0;
            g->PositionToTract(100.0f,-10.0f,tx,tz); Put(out,tx); Put(out,tz);
            g->TractCornerToPosition(3,4,px,pz); Put(out,px); Put(out,pz);
            g->TractCenterToPosition(3,4,px,pz); Put(out,px); Put(out,pz);
            Put(out,uint8_t(g->TractIsInBounds(uint32_t(-1),0)));
            Put(out,uint8_t(g->TractIsInBounds(g->GetTractCountX()-1,g->GetTractCountZ()-1)));
            Put(out,uint8_t(g->TractIsInBounds(g->GetTractCountX(),0))); Put(out,g->GetInstanceID()); calls+=7; return true;
        }
        case 14:
            for(int32_t tx=0;tx<g->GetTractCountX();++tx) for(int32_t tz=0;tz<g->GetTractCountZ();++tz) { Put(out,g->GetTractValue(tx,tz)); ++calls; ++bulkGet; }
            return true;
        default: return false;
        }
    }
private:
    cISC4SimGrid<T>* g;
    template<class V> static void Put(std::vector<uint8_t>& out,V v) { const auto* p=reinterpret_cast<const uint8_t*>(&v); out.insert(out.end(),p,p+sizeof(V)); }
};
} // namespace fc4
