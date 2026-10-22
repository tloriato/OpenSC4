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

// The layout-compatible grid family (plugin/FC4SimGridLogic.h) against
// docs/specs/simgrid.md §3-§6, with a counting malloc allocator and a fake city.
#include "FC4SimGridLogic.h"
#include "FC4SimGridsMethods.h"

#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <set>
#include <string>
#include <vector>

namespace {

int g_fail = 0;
#define CHECK(c)                                                         \
    do {                                                                 \
        if (!(c)) {                                                      \
            std::printf("FAIL %s:%d: %s\n", __FILE__, __LINE__, #c);     \
            ++g_fail;                                                    \
        }                                                                \
    } while (0)

std::set<void*> g_live;  // blocks from the test allocator
int g_failAfter = -1;    // allocations left before Allocate returns nullptr
struct TestAlloc {
    static void* Allocate(uint32_t n) {
        if (g_failAfter == 0) return nullptr;
        if (g_failAfter > 0) --g_failAfter;
        void* p = std::malloc(n ? n : 1);
        g_live.insert(p);
        return p;
    }
    static void Free(void* p) {
        CHECK(g_live.erase(p) == 1);  // only blocks this allocator handed out
        std::free(p);
    }
};

bool g_haveCity = true;
fc4::SimGridCity g_city{64, 64, 16.0f, 16.0f};
std::vector<std::string> g_trace;
struct TestHost {
    static bool Get(fc4::SimGridCity& c) {
        if (g_haveCity) c = g_city;
        return g_haveCity;
    }
    static void Trace(const char* what, uint32_t, uint32_t, int32_t, int32_t, int32_t, uint32_t, int changed, uint64_t,
                      uint32_t) {
        g_trace.push_back(std::string(what) + ":" + std::to_string(changed));
    }
};

template <class T>
using Grid = fc4::cFC4SimGridT<T, TestAlloc, TestHost>;

class OStream final : public cIGZOStream {
public:
    std::vector<uint8_t> bytes;
    bool QueryInterface(uint32_t, void**) override { return false; }
    uint32_t AddRef() override { return 1; }
    uint32_t Release() override { return 1; }
    void Flush() override {}
    bool SetSint8(int8_t v) override { return Put(&v, 1); }
    bool SetUint8(uint8_t v) override { return Put(&v, 1); }
    bool SetSint16(int16_t v) override { return Put(&v, 2); }
    bool SetUint16(uint16_t v) override { return Put(&v, 2); }
    bool SetSint32(int32_t v) override { return Put(&v, 4); }
    bool SetUint32(uint32_t v) override { return Put(&v, 4); }
    bool SetSint64(int64_t v) override { return Put(&v, 8); }
    bool SetUint64(uint64_t v) override { return Put(&v, 8); }
    bool SetFloat32(float v) override { return Put(&v, 4); }
    bool SetFloat64(double v) override { return Put(&v, 8); }
    bool SetRZCharStr(char const*) override { return false; }
    bool SetGZStr(cIGZString const&) override { return false; }
    bool SetGZSerializable(cIGZSerializable const&) override { return false; }
    bool SetVoid(void const* p, uint32_t n) override { return Put(p, n); }
    int32_t GetError() override { return 0; }
    int32_t SetUserData(cIGZVariant*) override { return 0; }
    int32_t GetUserData() override { return 0; }

private:
    bool Put(const void* p, size_t n) {
        const uint8_t* b = static_cast<const uint8_t*>(p);
        bytes.insert(bytes.end(), b, b + n);
        return true;
    }
};

class IStream final : public cIGZIStream {
public:
    explicit IStream(std::vector<uint8_t> b) : bytes(std::move(b)) {}
    std::vector<uint8_t> bytes;
    size_t at = 0;
    bool QueryInterface(uint32_t, void**) override { return false; }
    uint32_t AddRef() override { return 1; }
    uint32_t Release() override { return 1; }
    bool Skip(uint32_t n) override { return Get(nullptr, n); }
    bool GetSint8(int8_t& v) override { return Get(&v, 1); }
    bool GetUint8(uint8_t& v) override { return Get(&v, 1); }
    bool GetSint16(int16_t& v) override { return Get(&v, 2); }
    bool GetUint16(uint16_t& v) override { return Get(&v, 2); }
    bool GetSint32(int32_t& v) override { return Get(&v, 4); }
    bool GetUint32(uint32_t& v) override { return Get(&v, 4); }
    bool GetSint64(int64_t& v) override { return Get(&v, 8); }
    bool GetUint64(uint64_t& v) override { return Get(&v, 8); }
    bool GetFloat32(float& v) override { return Get(&v, 4); }
    bool GetFloat64(double& v) override { return Get(&v, 8); }
    bool GetRZCharStr(char*, uint32_t) override { return false; }
    bool GetGZStr(cIGZString&) override { return false; }
    bool GetGZSerializable(cIGZSerializable&) override { return false; }
    bool GetVoid(void* p, uint32_t n) override { return Get(p, n); }
    int32_t GetError() override { return 0; }
    int32_t SetUserData(cIGZVariant*) override { return 0; }
    int32_t GetUserData() override { return 0; }

private:
    bool Get(void* p, size_t n) {
        if (bytes.size() - at < n) return false;
        if (p != nullptr && n != 0) std::memcpy(p, bytes.data() + at, n);
        at += n;
        return true;
    }
};

// §6.3: the raw vector<vector<T>> through GetGridData.
template <class T>
const fc4::SimGridVec<fc4::SimGridVec<T>>* Raw(Grid<T>* g) {
    return reinterpret_cast<const fc4::SimGridVec<fc4::SimGridVec<T>>*>(g->GetGridData());
}

void TestGeometryAndLayout() {
    CHECK(fc4::SimGridShift(1) == 0 && fc4::SimGridShift(2) == 1 && fc4::SimGridShift(3) == 1);
    CHECK(fc4::SimGridShift(4) == 2 && fc4::SimGridShift(5) == 2 && fc4::SimGridShift(8) == 3);
    CHECK(fc4::SimGridShift(0) == 0 && fc4::SimGridShift(-4) == 0);

    auto* g = new Grid<uint8_t>();
    CHECK(g->RefCount() == 0);  // §6.1
    CHECK(g->GetTractSize() == 0 && g->GetTractCountX() == 0 && Raw(g)->begin == nullptr);  // §6.4.1
    CHECK(g->GetGZCLSID() == 0x49B9E602);
    g->Init();
    g->SetTractSize(4);
    g->SetInstanceID(0x1234);
    CHECK(g->GetInstanceID() == 0x1234);
    CHECK(g->GetTractShift() == 2 && g->GetTractCountX() == 16 && g->GetTractCountZ() == 16);
    CHECK(g->GetTractWidthX() == 64.0f && g->GetOneOverTractWidthZ() == 1.0f / 64.0f);
    const auto* raw = Raw(g);
    CHECK(raw->end - raw->begin == 16 && raw->capacityEnd == raw->end);
    for (const auto* r = raw->begin; r != raw->end; ++r) CHECK(r->end - r->begin == 16);
    // Raw writes are visible through the interface and vice versa (x-major rows).
    raw->begin[3].begin[5] = 77;
    CHECK(g->GetTractValue(3, 5) == 77 && g->GetCellValue(13, 22) == 77);
    g->SetTractValue(7, 1, 9);
    CHECK(raw->begin[7].begin[1] == 9);
    CHECK(g->TractIsInBounds(15, 15) && !g->TractIsInBounds(16, 0) && !g->TractIsInBounds(0xFFFFFFFFu, 0));
    int32_t tx = 0, tz = 0;
    CHECK(g->PositionToTract(130.0f, 63.9f, tx, tz) && tx == 2 && tz == 0);
    float px = 0, pz = 0;
    CHECK(g->TractCornerToPosition(2, 3, px, pz) && px == 128.0f && pz == 192.0f);
    CHECK(g->TractCenterToPosition(2, 3, px, pz) && px == 160.0f && pz == 224.0f);
    // [choice] out of range: 0, writes ignored.
    CHECK(g->GetTractValue(-1, 0) == 0 && g->GetTractValue(16, 0) == 0);
    g->SetTractValue(16, 0, 1);

    // §4.2: same S does nothing; a new S keeps values where indices exist.
    g->SetTractSize(4);
    CHECK(Raw(g) == raw && raw->begin[3].begin[5] == 77);
    g->SetTractSize(8);  // 8x8
    CHECK(g->GetTractCountX() == 8 && g->GetTractValue(3, 5) == 77 && g->GetTractValue(7, 1) == 9);
    g->SetTractSize(2);  // 32x32, new elements 0
    CHECK(g->GetTractCountX() == 32 && g->GetTractValue(3, 5) == 77 && g->GetTractValue(20, 20) == 0);
    g->SetTractValues(4);
    CHECK(g->GetTractValue(31, 31) == 4 && g->GetTractValue(3, 5) == 4);

    // No city: unchanged.
    g_haveCity = false;
    g->SetTractSize(1);
    CHECK(g->GetTractSize() == 2);
    g_haveCity = true;

    // §4 slots 0-2: one shared count; Release destroys at count <= 1.
    void* p = nullptr;
    CHECK(g->QueryInterface(0xC9BD939A, &p) && p == static_cast<cISC4SimGrid<uint8_t>*>(g) && g->RefCount() == 1);
    CHECK(g->QueryInterface(GZIID_cIGZSerializable, &p) && p == static_cast<cIGZSerializable*>(g));
    CHECK(g->RefCount() == 2);
    CHECK(!g->QueryInterface(0x12345678, &p));
    CHECK(g->Release() == 1);
    CHECK(g->Release() == 0);  // destroyed
    CHECK(g_live.empty());
}

void TestAverages() {
    auto* g = new Grid<int16_t>();
    g->SetTractSize(2);  // shift 1, 32x32
    g->SetTractValue(0, 0, 10);
    g->SetTractValue(1, 0, -3);
    g->SetTractValue(0, 1, 4);
    // One tract: the value unchanged.
    CHECK(g->GetAverageValueInCellRect(0, 0, 2, 2) == 10);
    // Cells (0..3) x (0..1): tract 0 twice x2 cells... sum = 10*4 + (-3)*4 = 28 over 8 -> 3 (truncated).
    CHECK(g->GetAverageValueInCellRect(0, 0, 4, 2) == 3);
    // Tracts: (10 - 3 + 4 + 0) / 4 = 2.75 -> 2.
    CHECK(g->GetAverageValueInTractRect(0, 0, 2, 2) == 2);
    g->SetTractValue(0, 0, -10);
    CHECK(g->GetAverageValueInTractRect(0, 0, 2, 2) == -2);  // -9/4 = -2.25 toward zero
    CHECK(g->GetAverageValueInTractRect(0, 0, 0, 2) == 0);   // [choice] empty: 0
    g->Release();

    auto* f = new Grid<float>();
    f->SetTractSize(1);
    f->SetTractValue(0, 0, 1.9f);
    f->SetTractValue(1, 0, 2.9f);
    // Values truncated before summing (1 + 2), quotient not truncated: 1.5.
    CHECK(f->GetAverageValueInTractRect(0, 0, 2, 1) == 1.5f);
    CHECK(f->GetAverageValueInCellRect(0, 0, 2, 1) == 1.5f);
    f->Release();

    auto* u = new Grid<uint64_t>();
    u->SetTractSize(1);
    u->SetTractValue(0, 0, 6);
    u->SetTractValue(1, 0, 3);
    CHECK(u->GetAverageValueInTractRect(0, 0, 2, 1) == 4);
    u->Release();
    CHECK(g_live.empty());
}

template <class T>
void TestRecordRoundTrip(T a, T b, size_t elemBytes, uint32_t clsid) {
    auto* g = new Grid<T>();
    g->Init();
    g->SetTractSize(16);  // 4x4
    g->SetInstanceID(0xABCD);
    g->SetTractValue(1, 2, a);
    g->SetTractValue(3, 0, b);
    OStream o;
    CHECK(g->Write(o));
    CHECK(o.bytes.size() == 2 + 1 + 4 * 10 + 16 * elemBytes);
    {  // tooling: the record digest is FNV-1a 64 of exactly the written bytes
        uint32_t nonzero = 0;
        const uint64_t d = g->RecordDigest(&nonzero);
        CHECK(d == fc4::SimGridFnv(fc4::kSimGridFnvBasis, o.bytes.data(), o.bytes.size()));
        CHECK(nonzero == (a != T{} ? 1u : 0u) + (b != T{} ? 1u : 0u));
    }
    uint16_t ver = 0;
    std::memcpy(&ver, o.bytes.data(), 2);
    uint32_t cls = 0;
    std::memcpy(&cls, o.bytes.data() + 3, 4);
    CHECK(ver == 1 && o.bytes[2] == 1 && cls == clsid);
    // Value (1,2) at index 1*4+2 (x outer, z inner).
    T v{};
    std::memcpy(&v, o.bytes.data() + 43 + 6 * elemBytes, elemBytes);
    CHECK(v == a);

    auto* r = new Grid<T>();
    IStream in(o.bytes);
    CHECK(r->Read(in) && in.at == o.bytes.size());
    CHECK(r->InitFlag() && r->GetGZCLSID() == clsid && r->GetInstanceID() == 0xABCD);
    CHECK(r->GetTractSize() == 16 && r->GetTractShift() == 4 && r->GetTractCountX() == 4);
    CHECK(r->GetTractWidthX() == 256.0f && r->GetTractValue(1, 2) == a && r->GetTractValue(3, 0) == b);
    CHECK(r->Checksum() == g->Checksum());
    g->Release();
    r->Release();
}

void TestRecord() {
    TestRecordRoundTrip<uint8_t>(200, 7, 1, 0x49B9E602);
    TestRecordRoundTrip<int8_t>(-5, 7, 1, 0x49B9E603);
    TestRecordRoundTrip<uint16_t>(60000, 7, 2, 0x49B9E604);
    TestRecordRoundTrip<int16_t>(-300, 7, 2, 0x49B9E605);
    TestRecordRoundTrip<uint32_t>(0xDEADBEEF, 7, 4, 0x49B9E606);
    TestRecordRoundTrip<int32_t>(-70000, 7, 4, 0x49B9E607);
    TestRecordRoundTrip<uint64_t>(0x1122334455667788ull, 7, 8, 0x49B9E608);
    TestRecordRoundTrip<int64_t>(-5, 7, 8, 0x49B9E609);
    TestRecordRoundTrip<float>(2.5f, -1.0f, 4, 0x49B9E60A);
    TestRecordRoundTrip<double>(1e300, -1.0, 8, 0x49B9E60B);

    // The stored CLSID overwrites the object's own (§5): a "bool" factory's
    // uint8 grid reports what it read.
    auto* g = new Grid<uint8_t>();
    g->SetClsid(0x49B9E601);
    CHECK(g->GetGZCLSID() == 0x49B9E601);
    OStream o;
    g->Write(o);  // empty grid: header only
    CHECK(o.bytes.size() == 43);
    o.bytes[3] = 0x02;
    auto* r = new Grid<uint8_t>();
    IStream in(o.bytes);
    CHECK(r->Read(in) && r->GetGZCLSID() == 0x49B9E602 && r->GetTractCountX() == 0);
    // Wrong version, short stream, hostile counts: false.
    std::vector<uint8_t> bad = o.bytes;
    bad[0] = 2;
    IStream b1(bad);
    CHECK(!r->Read(b1));
    IStream b2(std::vector<uint8_t>(o.bytes.begin(), o.bytes.begin() + 20));
    CHECK(!r->Read(b2));
    bad = o.bytes;
    int32_t huge = 100000;
    std::memcpy(bad.data() + 19, &huge, 4);  // countX
    IStream b3(bad);
    CHECK(!r->Read(b3));
    g->Release();
    r->Release();
    CHECK(g_live.empty());
}

void TestTraceAndRawChange() {
    auto* g = new Grid<uint8_t>();
    g->SetTractSize(8);
    g->SetTractValue(1, 1, 5);
    OStream o;
    g_trace.clear();
    g->Write(o);
    CHECK(g_trace.size() == 1 && g_trace[0] == "write:-1");
    auto* r = new Grid<uint8_t>();
    IStream in(o.bytes);
    r->Read(in);
    CHECK(g_trace.back() == "read:-1");
    OStream o2;
    r->Write(o2);
    CHECK(g_trace.back() == "write:0");
    const uint32_t before = fc4::GetSimGridStats().rawChanged.load();
    Raw(r)->begin[2].begin[2] = 9;  // what original code does through GetGridData
    OStream o3;
    r->Write(o3);
    CHECK(g_trace.back() == "write:1" && fc4::GetSimGridStats().rawChanged.load() == before + 1);
    g->Release();
    r->Release();
    CHECK(!g_trace.empty() && g_trace.back() == "all-released:-1");
}

void TestAllocFailure() {
    auto* g = new Grid<uint32_t>();
    g->SetTractSize(4);
    g->SetTractValue(1, 1, 3);
    g_failAfter = 5;  // the outer array and 4 rows, then nothing
    g->SetTractSize(2);
    g_failAfter = -1;
    // [choice] left empty, never counts larger than the data.
    CHECK(g->GetTractCountX() == 0 && Raw(g)->begin == nullptr);
    CHECK(g->GetTractValue(1, 1) == 0);
    g->Release();
    CHECK(g_live.empty());
}

template<class T> void TestPersistentMethodTape(T high) {
    auto* native=new Grid<T>(); auto* owned=new Grid<T>();
    native->Init(); native->SetTractSize(8);
    OStream bootstrap; CHECK(native->Write(bootstrap));
    IStream input(bootstrap.bytes); CHECK(owned->Read(input));
    fc4::SimGridsMethods<T> tape(owned);
    std::vector<uint8_t> out;
    int32_t a[4]={1,1,0,0};
    native->SetTractValue(1,1,high);
    CHECK(tape.Run(5,a,reinterpret_cast<const uint8_t*>(&high),sizeof(T),out));
    for(int i=0;i<100;++i) {
        CHECK(tape.Run(1,a,nullptr,0,out));
        const T v=native->GetTractValue(1,1);
        CHECK(fc4::SimGridsBuffersEqual(&v,sizeof(v),out));
    }
    CHECK(tape.scalarGet==100 && tape.scalarSet==1 && tape.calls==101);
    // Mutation after bootstrap must be detected and never installed in owned.
    const T corrupt=static_cast<T>(11); native->SetTractValue(1,1,corrupt);
    CHECK(tape.Run(1,a,nullptr,0,out));
    CHECK(!fc4::SimGridsBuffersEqual(&corrupt,sizeof(corrupt),out));
    CHECK(owned->GetTractValue(1,1)==high);
    native->SetTractValue(1,1,high); // restoration only in this negative-control fixture
    for(uint32_t op : {2u,3u,4u}) {
        int32_t rect[4]={0,0,2,2};
        CHECK(tape.Run(op,rect,nullptr,0,out));
        T v=op==2?native->GetCellValue(0,0):(op==3?native->GetAverageValueInCellRect(0,0,2,2):native->GetAverageValueInTractRect(0,0,2,2));
        CHECK(fc4::SimGridsBuffersEqual(&v,sizeof(v),out));
    }
    const auto before=tape.calls;
    int32_t bad[4]={-1,0,0,1};
    CHECK(!tape.Run(1,bad,nullptr,0,out));
    CHECK(!tape.Run(3,bad,nullptr,0,out));
    CHECK(!tape.Run(5,a,nullptr,0,out));
    CHECK(tape.calls==before);
    OStream expected,actual; CHECK(owned->Write(expected)); CHECK(native->Write(actual));
    CHECK(fc4::SimGridsBuffersEqual(actual.bytes.data(),actual.bytes.size(),expected.bytes));
    actual.bytes.back()^=1;
    CHECK(!fc4::SimGridsBuffersEqual(actual.bytes.data(),actual.bytes.size(),expected.bytes));
    CHECK(!fc4::SimGridsBuffersEqual(expected.bytes.data(),expected.bytes.size()-1,expected.bytes));
    CHECK(tape.Run(14,a,nullptr,0,out));
    CHECK(out.size()==64*sizeof(T) && tape.bulkGet==64);
    native->Release(); owned->Release(); CHECK(g_live.empty());
}

}  // namespace

int main() {
    TestGeometryAndLayout();
    TestAverages();
    TestRecord();
    TestTraceAndRawChange();
    TestAllocFailure();
    TestPersistentMethodTape<uint8_t>(201); TestPersistentMethodTape<int8_t>(-3);
    TestPersistentMethodTape<uint16_t>(60001); TestPersistentMethodTape<int16_t>(-300);
    TestPersistentMethodTape<uint32_t>(0xdeadbeef); TestPersistentMethodTape<int32_t>(-70001);
    TestPersistentMethodTape<uint64_t>(0x123456789abcdef0ull); TestPersistentMethodTape<int64_t>(-5);
    TestPersistentMethodTape<float>(1.9f); TestPersistentMethodTape<double>(-3.25);
    if (g_fail != 0) {
        std::printf("simgrid_test: %d failure(s)\n", g_fail);
        return 1;
    }
    std::printf("simgrid_test: OK\n");
    return 0;
}
