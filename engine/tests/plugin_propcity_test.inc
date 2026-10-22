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

// The glue12b1 lane's plugin tests (included by plugin_load_test.cpp after
// plugin_cmglue_test.inc, whose helpers it uses): the prop developer and its
// wealth requester (prop-developer.md) and the city detail manager
// (city-detail.md), without a city: the QI tables (§1), the records' round
// trips (a version-4 prop record written back as version 5), Init / Shutdown
// without services, the block step's scan flag, and the shadows' INACTIVE
// lines with ours enabled.

#include "cISC4PropDeveloper.h"
#include "FC4PropDevEvidence.h"

namespace propcity_test {

constexpr uint32_t kCLSID_PropDev = 0x89C48F47;
constexpr uint32_t kIID_PropDev = 0xE9A1F4B6;
constexpr uint32_t kCLSID_Wealth = 0x89C49986;
constexpr uint32_t kIID_PropRequester = 0x89C498BE;
constexpr uint32_t kCLSID_CityDetail = 0xEAD4BA36;
constexpr uint32_t kIID_CityDetail = 0x6AD4B9F1;

// cISC4PropRequester (no SDK header; prop-developer.md §4.1).
class TestPropRequester : public cIGZUnknown {
public:
    virtual int32_t GetRequestedShare(SC4Rect<int32_t> const& cells) = 0;
    virtual uint32_t GetRequesterSatisfactionId() = 0;
    virtual bool UsesCustomPropSelector() = 0;
    virtual uint32_t SelectProp(void const* candidates, int32_t x, int32_t z) = 0;
};

// cISC4CityDetailManager (city-detail.md §6).
class TestCityDetail : public cIGZUnknown {
public:
    virtual bool Init() = 0;
    virtual bool Shutdown() = 0;
    virtual uint32_t GetCityDetail() = 0;
};

// An object whose serializable is at +4 (the wealth requester, the city
// detail manager): 1 and `iid` give the primary, 0xE4FDA3D4 +4.
void CheckQiPlus4(cIGZUnknown* primary, uint32_t iid, uint32_t clsid) {
    void* p = nullptr;
    CHECK(primary->QueryInterface(GZIID_cIGZUnknown, &p) && p == static_cast<void*>(primary));
    if (p != nullptr) static_cast<cIGZUnknown*>(p)->Release();
    p = nullptr;
    CHECK(primary->QueryInterface(iid, &p) && p == static_cast<void*>(primary));
    if (p != nullptr) static_cast<cIGZUnknown*>(p)->Release();
    cIGZSerializable* ser = nullptr;
    CHECK(primary->QueryInterface(GZIID_cIGZSerializable, reinterpret_cast<void**>(&ser)) && ser != nullptr);
    CHECK(reinterpret_cast<uintptr_t>(ser) == reinterpret_cast<uintptr_t>(primary) + 4);
    if (ser != nullptr) {
        CHECK(ser->GetGZCLSID() == clsid);
        ser->Release();
    }
    p = reinterpret_cast<void*>(0x1234);
    CHECK(!primary->QueryInterface(cmglue_test::kIID_MessageTarget2, &p) && p == reinterpret_cast<void*>(0x1234));
}

void PutU32(std::vector<uint8_t>& b, uint32_t v) {
    for (int i = 0; i < 4; ++i) b.push_back(static_cast<uint8_t>(v >> (8 * i)));
}

}  // namespace propcity_test

int PropDevEnableTest(const std::string& dll, const std::string& ini, const std::string& log) {
    // Negative controls: matching returns with missing inputs, failed replay,
    // and mismatches all remain in the full native denominator. No zero-call
    // equality or creation/read/write count can turn them into exact credit.
    fc4::PropDevEvidence evidence;
    CHECK(evidence.nativeCalls == 0 && evidence.equal == 0 && evidence.compared == 0);
    evidence.ObserveCall(true, true, true, false);
    evidence.ObserveCall(false, true, true, false);
    evidence.ObserveCall(true, false, true, false);
    CHECK(evidence.nativeCalls == 3 && evidence.refused == 3);
    CHECK(evidence.compared == 0 && evidence.equal == 0);
    CHECK(evidence.returnCompared == 2 && evidence.returnEqual == 1);
    CHECK(evidence.diagnosticMismatches == 1);
    using namespace cmglue_test;
    using namespace propcity_test;
    // Off by default; `enable = propdev` registers the developer and the
    // wealth requester. The shadow is INACTIVE with ours.
    SetEnvironmentVariableA("FC4_DISABLE", nullptr);
    SetEnvironmentVariableA("FC4_ENABLE", nullptr);
    WriteTextFile(ini, "[subsystems]\nenable = propdev\npropdev_seed = 9\npropdev_shadow = 1\n");
    HMODULE hd = LoadLibraryA(dll.c_str());
    CHECK(hd != nullptr);
    if (hd == nullptr) return 1;
    auto getDirector = reinterpret_cast<cIGZCOMDirector* (*)()>(GetProcAddress(hd, "GZDllGetGZCOMDirector"));
    cIGZCOMDirector* director = getDirector ? getDirector() : nullptr;
    CHECK(director != nullptr);
    SetGamePrecision();
    if (director != nullptr) {
        std::vector<uint32_t> classes;
        director->EnumClassObjects(&CollectClass, &classes);
        CHECK(classes.size() == 6);  // simulator (1) + budget (3) + propdev (2)
        CHECK(std::count(classes.begin(), classes.end(), kCLSID_PropDev) == 1);
        CHECK(std::count(classes.begin(), classes.end(), kCLSID_Wealth) == 1);

        // §7: a new object's record, version 5 (91 bytes, null manager refs).
        std::vector<uint8_t> fresh;
        if (auto* p = Create<cISC4PropDeveloper>(director, kCLSID_PropDev, kIID_PropDev)) {
            CheckQiTable(p, kIID_PropDev, kCLSID_PropDev);
            bool ok = false;
            fresh = RecordOf(p, &ok);
            CHECK(ok);
            CHECK(fresh.size() == 91);
            if (fresh.size() == 91) {
                CHECK(fresh[0] == 5 && fresh[1] == 0);  // version
                CHECK(fresh[2] == 0);                   // scan flag
                CHECK(fresh[32] == 0);                  // wealthAdded
                CHECK(fresh[86] == 1);                  // signsVisible
            }
            CHECK(p->AreSignsAndLabelsVisible());
            // §3.1 without a city: the wealth requester, scan flag 1.
            CHECK(p->Init());
            if (cIGZMessageTarget2* mt = TargetOf(p)) {
                ResidentialDayMessage idle(0x6695681A, 0);
                CHECK(mt->DoMessage(&idle));  // scan flag 1 -> 2
                CHECK(mt->DoMessage(&idle));  // block (0, 0): no lots
                ResidentialDayMessage tick(0x86AD10EE, 0);
                CHECK(!mt->DoMessage(&tick));  // §3.3: an animation tick in night mode 0
                ResidentialDayMessage other(0x12345678, 0);
                CHECK(mt->DoMessage(&other));
                mt->Release();
            }
            p->ToggleSignsAndLabels();
            CHECK(!p->AreSignsAndLabelsVisible());
            p->ShowSignsAndLabels(true);
            CHECK(p->AreSignsAndLabelsVisible());
            CHECK(p->Shutdown());  // §3.2: the requesters released, wealthAdded 0
            std::vector<uint8_t> after = RecordOf(p, &ok);
            CHECK(ok && after.size() == 91);
            if (after.size() == 91) {
                CHECK(after[2] == 0 && after[32] == 0);
                CHECK(after[33] == 0 && after[34] == 0);  // no requesters
            }
            CHECK(p->Release() == 0);
        }
        // Read before Init, written back byte for byte.
        std::vector<uint8_t> again;
        CHECK(ReadInto<cISC4PropDeveloper>(director, kCLSID_PropDev, kIID_PropDev, fresh, &again));
        CHECK(again == fresh);
        // §7: version 4 (no signs) reads; it is written back as version 5.
        if (fresh.size() == 91) {
            std::vector<uint8_t> v4(fresh.begin(), fresh.end() - 5);
            v4[0] = 4;
            CHECK(ReadInto<cISC4PropDeveloper>(director, kCLSID_PropDev, kIID_PropDev, v4, &again));
            CHECK(again == fresh);
            std::vector<uint8_t> v3 = v4;
            v3[0] = 3;
            CHECK(!ReadInto<cISC4PropDeveloper>(director, kCLSID_PropDev, kIID_PropDev, v3, nullptr));
            std::vector<uint8_t> shortRec(fresh.begin(), fresh.begin() + 40);
            CHECK(!ReadInto<cISC4PropDeveloper>(director, kCLSID_PropDev, kIID_PropDev, shortRec, nullptr));
        }

        // The wealth requester (§8.3).
        void* obj = nullptr;
        CHECK(director->GetClassObject(kCLSID_Wealth, kIID_PropRequester, &obj));
        if (auto* w = static_cast<TestPropRequester*>(obj)) {
            CheckQiPlus4(w, kIID_PropRequester, kCLSID_Wealth);
            SC4Rect<int32_t> r{};
            CHECK(w->GetRequestedShare(r) == 10);
            CHECK(w->GetRequesterSatisfactionId() == 0x49A1E995);
            CHECK(w->UsesCustomPropSelector());
            cIGZSerializable* s = nullptr;
            CHECK(w->QueryInterface(GZIID_cIGZSerializable, reinterpret_cast<void**>(&s)) && s != nullptr);
            if (s != nullptr) {
                ResidentialSegOStream out;
                CHECK(s->Write(out) && out.bytes.empty());
                const std::vector<uint8_t> none;
                ResidentialSegIStream in(none);
                CHECK(s->Read(in));
                s->Release();
            }
            CHECK(w->Release() == 0);
        }
        CheckCw("propdev");
    }
    FreeLibrary(hd);
    DeleteFileA(ini.c_str());
    const std::string text = ReadFile(log);
    const auto npos = std::string::npos;
    CHECK(text.find("subsystem 'propdev': enabled (2 classes replaced) by FreeCity4.ini (enable=)") != npos);
    CHECK(text.find("session seed 9") != npos);
    CHECK(text.find("propdev Init: ok=1 scan=1") != npos);
    CHECK(text.find("propdev Shutdown: ok=1 scan=0") != npos);
    CHECK(CountOf(text, "propdev Read: ok=1") == 2);
    CHECK(CountOf(text, "propdev Read: ok=0") == 2);
    CHECK(text.find("propdev shadow: requested (propdev_shadow from FreeCity4.ini) but INACTIVE") != npos);
    CHECK(text.find("propdev shadow: ON") == npos);
    CHECK(text.find("structured exception") == npos);
    CHECK(text.find("config: WARNING unknown key") == npos);
    CHECK(text.find("WARNING: released last reference") == npos);
    CHECK(text.find("WARNING: over-release") == npos);
    CHECK(text.find("streams: WARNING") == npos);
    if (g_failures == 0) std::printf("plugin propdev enable test: all checks passed\n");
    return g_failures == 0 ? 0 : 1;
}

int CityDetailEnableTest(const std::string& dll, const std::string& ini, const std::string& log) {
    using namespace cmglue_test;
    using namespace propcity_test;
    // Off by default; `enable = citydetail` registers it. No application: the
    // setting reads as the default 3 (§2).
    SetEnvironmentVariableA("FC4_DISABLE", nullptr);
    SetEnvironmentVariableA("FC4_ENABLE", nullptr);
    WriteTextFile(ini, "[subsystems]\nenable = citydetail\ncitydetail_seed = 7\ncitydetail_shadow = 1\n");
    HMODULE hd = LoadLibraryA(dll.c_str());
    CHECK(hd != nullptr);
    if (hd == nullptr) return 1;
    auto getDirector = reinterpret_cast<cIGZCOMDirector* (*)()>(GetProcAddress(hd, "GZDllGetGZCOMDirector"));
    cIGZCOMDirector* director = getDirector ? getDirector() : nullptr;
    CHECK(director != nullptr);
    SetGamePrecision();
    std::vector<uint8_t> none, high, medium, v1;
    PutU32(none, 0);
    PutU32(none, 0xFFFFFFFFu);
    PutU32(high, 0);
    PutU32(high, 3);
    PutU32(medium, 0);
    PutU32(medium, 2);
    PutU32(v1, 1);
    PutU32(v1, 3);
    if (director != nullptr) {
        std::vector<uint32_t> classes;
        director->EnumClassObjects(&CollectClass, &classes);
        CHECK(classes.size() == 5);  // simulator (1) + budget (3) + citydetail (1)
        CHECK(std::count(classes.begin(), classes.end(), kCLSID_CityDetail) == 1);
        if (auto* c = Create<TestCityDetail>(director, kCLSID_CityDetail, kIID_CityDetail)) {
            CheckQiPlus4(c, kIID_CityDetail, kCLSID_CityDetail);
            bool ok = false;
            CHECK(RecordOf(c, &ok) == none && ok);  // §2: none
            CHECK(c->GetCityDetail() == 0xFFFFFFFFu);
            CHECK(c->Init());  // §3.1: a new city: nothing, then the setting
            CHECK(c->GetCityDetail() == 3);
            CHECK(RecordOf(c, &ok) == high && ok);  // §8 test 1
            CHECK(c->Shutdown());
            CHECK(c->Release() == 0);
        }
        std::vector<uint8_t> again;
        CHECK(ReadInto<TestCityDetail>(director, kCLSID_CityDetail, kIID_CityDetail, high, &again));
        CHECK(again == high);
        CHECK(ReadInto<TestCityDetail>(director, kCLSID_CityDetail, kIID_CityDetail, medium, &again));
        CHECK(again == medium);
        CHECK(!ReadInto<TestCityDetail>(director, kCLSID_CityDetail, kIID_CityDetail, v1, nullptr));  // §8 test 2
        // Saved 2, current 3: the increase pass over no lots.
        void* obj = nullptr;
        CHECK(director->GetClassObject(kCLSID_CityDetail, kIID_CityDetail, &obj));
        if (auto* c = static_cast<TestCityDetail*>(obj)) {
            cIGZSerializable* s = nullptr;
            CHECK(c->QueryInterface(GZIID_cIGZSerializable, reinterpret_cast<void**>(&s)) && s != nullptr);
            if (s != nullptr) {
                ResidentialSegIStream in(medium);
                CHECK(s->Read(in));
                s->Release();
            }
            CHECK(c->Init());
            CHECK(c->GetCityDetail() == 3);
            CHECK(c->Release() == 0);
        }
        CheckCw("citydetail");
    }
    FreeLibrary(hd);
    DeleteFileA(ini.c_str());
    const std::string text = ReadFile(log);
    const auto npos = std::string::npos;
    CHECK(text.find("subsystem 'citydetail': enabled (1 class replaced) by FreeCity4.ini (enable=)") != npos);
    CHECK(text.find("session seed 7") != npos);
    CHECK(text.find("citydetail Init: setting=3 level -1->3 pass=none") != npos);
    CHECK(text.find("citydetail Init: setting=3 level 2->3 pass=increase") != npos);
    CHECK(CountOf(text, "citydetail Read -> 1") == 3);
    CHECK(CountOf(text, "citydetail Read -> 0") == 1);
    CHECK(text.find("citydetail shadow: requested (citydetail_shadow from FreeCity4.ini) but INACTIVE") != npos);
    CHECK(text.find("structured exception") == npos);
    CHECK(text.find("config: WARNING unknown key") == npos);
    CHECK(text.find("WARNING: released last reference") == npos);
    CHECK(text.find("WARNING: over-release") == npos);
    if (g_failures == 0) std::printf("plugin citydetail enable test: all checks passed\n");
    return g_failures == 0 ? 0 : 1;
}
