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

#include "FC4HostGrid.h"
#include "FC4SimGridsShadow.h"

#include "FC4Log.h"
#include "FC4Perf.h"
#include "FC4RefAudit.h"
#include "FC4Streams.h"

#include "cIGZCOM.h"
#include "cIGZSerializable.h"
#include "cISC4SimGrid.h"
#include "cRZCOMDllDirector.h"

#include <atomic>
#include <cmath>

namespace fc4 {
namespace {

constexpr uint32_t kIID_cISC4SimGrid = 0xC9BD939A;
constexpr uint32_t kClsBool = 0x49B9E601;
constexpr uint32_t kClsU8 = 0x49B9E602;
constexpr uint32_t kClsI8 = 0x49B9E603;
constexpr uint32_t kClsU16 = 0x49B9E604;
constexpr uint32_t kClsI16 = 0x49B9E605;
constexpr uint32_t kClsU32 = 0x49B9E606;
constexpr uint32_t kClsF32 = 0x49B9E60A;

const HostGridContext* Ctx(void* u) { return static_cast<const HostGridContext*>(u); }

template <class T>
cISC4SimGrid<T>* As(uint64_t t) {
    return reinterpret_cast<cISC4SimGrid<T>*>(FromToken(t));
}

// A grid the class-object store has not read yet (FC4Streams.h NoteUnreadRef;
// foundations §2.7: a Read may only keep the pointer). Such a grid is in no
// specified state, so it reads as empty and writes to it are dropped.
bool Unread(uint64_t t) {
    if (!IsUnreadRef(t)) return false;
    static std::atomic<uint32_t> logged{0};
    if (logged.fetch_add(1) < 4)
        Log("host grid: grid %p accessed before the store load read it (foundations 2.7); treated as empty",
            static_cast<void*>(FromToken(t)));
    return true;
}

bool Supported(uint32_t kind) {
    return kind == kClsU8 || kind == kClsBool || kind == kClsI8 || kind == kClsU16 || kind == kClsI16 ||
           kind == kClsU32 || kind == kClsF32;
}

template <class T>
void Setup(void* p, int32_t tractSize, uint32_t instance) {
    cISC4SimGrid<T>* g = static_cast<cISC4SimGrid<T>*>(p);
    g->Init();
    g->SetTractSize(tractSize);
    g->SetInstanceID(instance);
}

uint64_t Create(void* u, uint32_t kind, uint32_t instance, int32_t tractSize) {
    return CreateGameGrid(Ctx(u)->tag, kind, instance, tractSize);
}

uint32_t ClassOf(void*, uint64_t t) {
    if (t == 0) return 0;
    cIGZSerializable* s = nullptr;
    if (!FromToken(t)->QueryInterface(GZIID_cIGZSerializable, reinterpret_cast<void**>(&s)) || s == nullptr) return 0;
    FC4_ACQUIRED(s, "QueryInterface");
    const uint32_t c = s->GetGZCLSID();
    FC4_RELEASE(s);
    return c;
}

// Slots 10-17 do not depend on the element type.
void Geometry(void* u, uint64_t t, fc4_grid_geometry* out) {
    cISC4SimGrid<uint8_t>* g = As<uint8_t>(t);
    if (g == nullptr || out == nullptr) return;
    if (Unread(t)) {
        *out = fc4_grid_geometry{};
        return;
    }
    out->tract_size = g->GetTractSize();
    out->shift = g->GetTractShift();
    out->count_x = g->GetTractCountX();
    out->count_z = g->GetTractCountZ();
    out->width_x = g->GetTractWidthX();
    out->width_z = g->GetTractWidthZ();
    out->inv_width_x = g->GetOneOverTractWidthX();
    out->inv_width_z = g->GetOneOverTractWidthZ();
    SimGridsShadowTrack(Ctx(u)->tag,t);
}

template<class T> T ObservedGet(uint64_t t,int32_t x,int32_t z) {
    T v=As<T>(t)->GetTractValue(x,z);
    SimGridsShadowValue(t,x,z,&v,sizeof(v)); return v;
}
double Get(void* u, uint64_t t, uint32_t kind, int32_t x, int32_t z) {
    if (t == 0 || Unread(t)) return 0.0;
    if (perf::On()) perf::Count(perf::kGridGet);  // tooling: perf_profile (FC4Perf.h)
    SimGridsShadowTrack(Ctx(u)->tag,t);
    switch (kind) {
        case kClsI16: return ObservedGet<int16_t>(t,x,z);
        case kClsI8: return ObservedGet<int8_t>(t,x,z);
        case kClsU16: return ObservedGet<uint16_t>(t,x,z);
        case kClsU32: return ObservedGet<uint32_t>(t,x,z);
        case kClsF32: return ObservedGet<float>(t,x,z);
        case kClsU8:
        case kClsBool: return ObservedGet<uint8_t>(t,x,z);
        default: return 0.0;
    }
}

// The bulk reader (fc4_grid_set_bulk_reader, a performance hook): the same
// GetTractValue calls as Get, in one host call per grid, x outer.
template <class T>
uint32_t ReadAll(uint64_t t, void* out, uint32_t count) {
    const cISC4SimGrid<T>* g = As<T>(t);
    const int32_t nx = g->GetTractCountX();
    const int32_t nz = g->GetTractCountZ();
    if (nx <= 0 || nz <= 0 || static_cast<uint64_t>(nx) * static_cast<uint64_t>(nz) != count) return 0;
    T* o = static_cast<T*>(out);
    for (int32_t x = 0; x < nx; ++x) {
        for (int32_t z = 0; z < nz; ++z) *o++ = g->GetTractValue(x, z);
    }
    SimGridsShadowBulkRead(t,out,count,sizeof(T));
    return count;
}

uint32_t BulkRead(uint64_t t, uint32_t kind, void* out, uint32_t count) {
    if (t == 0 || out == nullptr || count == 0 || Unread(t)) return 0;
    if (perf::On()) perf::Count(perf::kGridBulk);  // tooling: perf_profile (FC4Perf.h)
    switch (kind) {
        case kClsI16: return ReadAll<int16_t>(t, out, count);
        case kClsI8: return ReadAll<int8_t>(t, out, count);
        case kClsU16: return ReadAll<uint16_t>(t, out, count);
        case kClsU32: return ReadAll<uint32_t>(t, out, count);
        case kClsF32: return ReadAll<float>(t, out, count);
        case kClsU8:
        case kClsBool: return ReadAll<uint8_t>(t, out, count);
        default: return 0;
    }
}

// The core converts with its element type, so the double is exact.
template <class T>
T Narrow(double v) {
    return static_cast<T>(v);
}

bool WritesOn(void* u, const char* what) {
    if (On(Ctx(u)->writes)) return true;
    NoteSkipped(Ctx(u)->writes, what);
    return false;
}

// The bulk writer (fc4_grid_set_bulk_writer, a performance hook): the same
// SetTractValue calls as Set, in order, in one host call.
template <class T>
uint32_t WriteAll(uint64_t t, const uint32_t* index, const void* values, uint32_t n) {
    cISC4SimGrid<T>* g = As<T>(t);
    const int32_t nz = g->GetTractCountZ();
    if (nz <= 0) return 0;
    const T* v = static_cast<const T*>(values);
    for (uint32_t i = 0; i < n; ++i) {
        g->SetTractValue(static_cast<int32_t>(index[i] / static_cast<uint32_t>(nz)),
                         static_cast<int32_t>(index[i] % static_cast<uint32_t>(nz)), v[i]);
    }
    SimGridsShadowBulkWrite(t,index,values,n,sizeof(T));
    return n;
}

uint32_t BulkWrite(void* u, uint64_t t, uint32_t kind, const uint32_t* index, const void* values, uint32_t n) {
    // Writes switched off: none here (the core then sets tract by tract, and Set notes each skip).
    if (t == 0 || index == nullptr || values == nullptr || n == 0 || !On(Ctx(u)->writes) || Unread(t)) return 0;
    if (perf::On()) perf::Count(perf::kGridBulk);  // tooling: perf_profile (FC4Perf.h)
    switch (kind) {
        case kClsI16: return WriteAll<int16_t>(t, index, values, n);
        case kClsI8: return WriteAll<int8_t>(t, index, values, n);
        case kClsU16: return WriteAll<uint16_t>(t, index, values, n);
        case kClsU32: return WriteAll<uint32_t>(t, index, values, n);
        case kClsF32: return WriteAll<float>(t, index, values, n);
        case kClsU8:
        case kClsBool: return WriteAll<uint8_t>(t, index, values, n);
        default: return 0;
    }
}

template<class T> void ObservedSet(uint64_t t,int32_t x,int32_t z,double v) {
    T value=Narrow<T>(v); As<T>(t)->SetTractValue(x,z,value);
    SimGridsShadowSet(t,x,z,&value,sizeof(value));
}
void Set(void* u, uint64_t t, uint32_t kind, int32_t x, int32_t z, double v) {
    if (t == 0 || Unread(t) || !WritesOn(u, "grid SetTractValue")) return;
    if (perf::On()) perf::Count(perf::kGridSet);  // tooling: perf_profile (FC4Perf.h)
    SimGridsShadowTrack(Ctx(u)->tag,t);
    switch (kind) {
        case kClsI16: ObservedSet<int16_t>(t,x,z,v); break;
        case kClsI8: ObservedSet<int8_t>(t,x,z,v); break;
        case kClsU16: ObservedSet<uint16_t>(t,x,z,v); break;
        case kClsU32: ObservedSet<uint32_t>(t,x,z,v); break;
        case kClsF32: ObservedSet<float>(t,x,z,v); break;
        case kClsU8:
        case kClsBool: ObservedSet<uint8_t>(t,x,z,v); break;
        default: break;
    }
}

void Fill(void* u, uint64_t t, uint32_t kind, double v) {
    if (t == 0 || Unread(t) || !WritesOn(u, "grid SetTractValues")) return;
    SimGridsShadowTrack(Ctx(u)->tag,t);
    switch (kind) {
        case kClsI16: As<int16_t>(t)->SetTractValues(Narrow<int16_t>(v)); break;
        case kClsI8: As<int8_t>(t)->SetTractValues(Narrow<int8_t>(v)); break;
        case kClsU16: As<uint16_t>(t)->SetTractValues(Narrow<uint16_t>(v)); break;
        case kClsU32: As<uint32_t>(t)->SetTractValues(Narrow<uint32_t>(v)); break;
        case kClsF32: As<float>(t)->SetTractValues(Narrow<float>(v)); break;
        case kClsU8:
        case kClsBool: As<uint8_t>(t)->SetTractValues(Narrow<uint8_t>(v)); break;
        default: break;
    }
    SimGridsShadowFill(t,v);
}

void Shutdown(void* u, uint64_t t) {
    if (t == 0) return;
    FC4_LOGV("%s: grid %p shut down", Ctx(u)->tag, static_cast<void*>(FromToken(t)));
    const bool result=As<uint8_t>(t)->Shutdown();
    SimGridsShadowShutdown(t,result);
}

void Release(void*, uint64_t t) { ReleaseGameGrid(t); }

}  // namespace

uint64_t CreateGameGrid(const char* tag, uint32_t kind, uint32_t instance, int32_t tractSize) {
    if (!Supported(kind)) {
        Log("%s: WARNING grid class 0x%08X not supported; grid 0x%08X not created [choice]", tag, kind, instance);
        return 0;
    }
    cIGZCOM* com = RZGetCOMDllDirector() ? GZCOM() : nullptr;
    void* p = nullptr;
    if (com == nullptr || !com->GetClassObject(kind, kIID_cISC4SimGrid, &p) || p == nullptr) {
        Log("%s: WARNING could not create grid 0x%08X (class 0x%08X) through GZCOM", tag, instance, kind);
        return 0;
    }
    FC4_ACQUIRED(static_cast<cIGZUnknown*>(p), "GetClassObject");
    if (kind == kClsI16) {
        Setup<int16_t>(p, tractSize, instance);
    } else if (kind == kClsI8) {
        Setup<int8_t>(p, tractSize, instance);
    } else if (kind == kClsU16) {
        Setup<uint16_t>(p, tractSize, instance);
    } else if (kind == kClsU32) {
        Setup<uint32_t>(p, tractSize, instance);
    } else if (kind == kClsF32) {
        Setup<float>(p, tractSize, instance);
    } else {
        Setup<uint8_t>(p, tractSize, instance);
    }
    FC4_LOGV("%s: grid 0x%08X created (class 0x%08X, tract size %d) at %p", tag, instance, kind, tractSize, p);
    SimGridsShadowTrack(tag,TokenOf(p));
    return TokenOf(p);
}

uint32_t ReleaseGameGrid(uint64_t token) {
    SimGridsShadowRelease(token);
    ForgetUnreadRef(token);
    // The token's reference is ours whichever way it came: GetClassObject in
    // CreateGameGrid, or a save's ReadGZSerializable (then QueryInterface'd to
    // cISC4SimGrid) in Read; both noted with FC4_ACQUIRED. So this may be the
    // last reference, and that free is expected: a grid we created has no
    // other owner, and a loaded grid has none after a save. SV1 (answered,
    // foundations §2.7 "References the load keeps"; the bracket matches): the
    // city segment's serializer table keeps one reference on each object the
    // load created until the next save, or, in a city never saved, until city
    // close after Shutdown; so in an unsaved city this release is not the
    // last, and after a save it is.
    cIGZUnknown* g = FromToken(token);
    return g != nullptr ? FC4_RELEASE_CREATED(g) : 0;
}

fc4_grid_api MakeGridApi(const HostGridContext* ctx) {
    fc4_grid_api a{};
    a.struct_size = sizeof(a);
    a.user = const_cast<HostGridContext*>(ctx);
    a.create = &Create;
    a.class_of = &ClassOf;
    a.geometry = &Geometry;
    a.get = &Get;
    a.set = &Set;
    a.fill = &Fill;
    a.shutdown = &Shutdown;
    a.release = &Release;
    fc4_grid_set_bulk_reader(&Get, &BulkRead);    // devperf: one host call per grid read
    fc4_grid_set_bulk_writer(&Set, &BulkWrite);  // devperf: one host call per walk's writes
    return a;
}

}  // namespace fc4
