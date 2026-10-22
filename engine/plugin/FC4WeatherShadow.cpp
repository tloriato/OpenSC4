// SPDX-License-Identifier: GPL-3.0-or-later
#include "FC4WeatherShadow.h"

#include "FC4Config.h"
#include "FC4Interfaces.h"
#include "FC4Log.h"
#include "FC4RefAudit.h"
#include "fc4.h"
#include "cIGZMessageTarget2.h"
#include "cISC4City.h"

#include <cstring>
#include <windows.h>

namespace fc4 {
namespace {
constexpr uint32_t kDay = 0x66956814;
constexpr uint32_t kWeek = 0x66956815;
constexpr uint32_t kMonth = 0x66956816;
constexpr uint32_t kWeatherIID = 0xC6C66245;
bool g_requested = false, g_active = false;
cISC4City* g_city = nullptr;  // borrowed only between PostCityInit and PreCityShutdown
cFC4ISC4WeatherSimulator* g_original = nullptr;
fc4_weather* g_core = nullptr;
int32_t g_month = 1, g_hemisphere = 1;
uint64_t g_days = 0, g_weeks = 0, g_months = 0, g_saves = 0;
uint64_t g_compared = 0, g_mismatches = 0, g_refused = 0, g_failures = 0;
uint64_t g_ownerRefused = 0, g_originalCalls = 0;
uint64_t g_seen = 0;

int32_t Month(void*) { return g_month; }
int32_t Hemisphere(void*) { return g_hemisphere; }

void Report(const char* operation, int64_t julian) {
    Log("weather shadow %s julian=%lld: INCOMPLETE WY1; original=%p natural_calls=%llu "
        "days=%llu weeks=%llu months=%llu presaves=%llu calendar_equal=%llu calendar_compared=%llu "
        "calendar_mismatches=%llu simulation_compared=0 simulation_refused=%llu failures=%llu "
        "owner_refused=%llu natural_seen=%llu fixtures=0 fallback=0 resync=0 native_seed_captured=0 rng_draws_captured=0 "
        "init_compared=0 read_compared=0 write_compared=0 ambient_compared=0 wind_compared=0 "
        "grid_compared=0 shutdown_compared=0 full_exact=0",
        operation, static_cast<long long>(julian), g_original,
        static_cast<unsigned long long>(g_originalCalls), static_cast<unsigned long long>(g_days),
        static_cast<unsigned long long>(g_weeks), static_cast<unsigned long long>(g_months),
        static_cast<unsigned long long>(g_saves), static_cast<unsigned long long>(g_compared - g_mismatches),
        static_cast<unsigned long long>(g_compared), static_cast<unsigned long long>(g_mismatches),
        static_cast<unsigned long long>(g_refused), static_cast<unsigned long long>(g_failures),
        static_cast<unsigned long long>(g_ownerRefused), static_cast<unsigned long long>(g_seen));
}

void CompareCalendar() {
    // Capture host inputs, never native output-derived state. The core stays
    // uninitialized: its calendar getters require neither Init nor RNG draws.
    // Literal scope of weather section 4; WY1 parks all stateful operations.
    auto* simulator = g_city ? AsGame(g_city->GetSimulator()) : nullptr;
    if (!simulator || !g_core || !g_original) {
        ++g_failures;
        return;
    }
    uint32_t year = 0, month = 0, day = 0, doy = 0, weekday = 0;
    simulator->GetSimDateParts(&year, &month, &day, &doy, &weekday);
    if (month < 1 || month > 12) {
        ++g_failures;
        return;
    }
    g_month = static_cast<int32_t>(month);
    g_hemisphere = static_cast<int32_t>(g_city->GetWorldHemisphere());
    const int32_t nativeMonth = g_original->GetCurrentSeasonMonth();
    const int32_t nativeSeason = g_original->GetCurrentSeason();
    const int32_t rustMonth = fc4_weather_season_month(g_core);
    const int32_t rustSeason = fc4_weather_season(g_core);
    g_compared += 2;
    g_mismatches += (nativeMonth != rustMonth) + (nativeSeason != rustSeason);
    if (nativeMonth != rustMonth || nativeSeason != rustSeason) {
        Log("weather shadow calendar DIFFERENCE: input_month=%d hemisphere=%d native=%d/%d rust=%d/%d",
            g_month, g_hemisphere, nativeMonth, nativeSeason, rustMonth, rustSeason);
    }
}
}  // namespace

void WeatherShadowStartup(bool simulatorReplaced, bool weatherReplaced) {
    g_requested = SettingInt("weather_shadow", "FC4_WEATHER_SHADOW", 0) != 0;
    g_active = g_requested && simulatorReplaced && !weatherReplaced;
    if (!g_requested) return;
    Log("weather shadow: requested active=%d simulator_replaced=%d weather_replaced=%d; "
        "INCOMPLETE WY1: calendar getters only; original remains authoritative; "
        "no native seed/CRT-rand capture contract; stateful replay REFUSED",
        g_active ? 1 : 0, simulatorReplaced ? 1 : 0, weatherReplaced ? 1 : 0);
}

void WeatherShadowCityLoaded(cISC4City* city) {
    if (!g_active) return;
    // Never replace an existing core with newly sampled native state.
    if (g_city || g_core || g_original) {
        ++g_failures;
        Report("duplicate-load-refused", 0);
        return;
    }
    // Totals are process cumulative, including deliveries before PostCityInit.
    // A load must not erase refusals or failures from an earlier city/boundary.
    g_city = city;
    auto* weather = city ? AsGame(city->GetWeatherSimulator()) : nullptr;
    if (!weather) {
        ++g_failures;
        Report("load-no-owner", 0);
        return;
    }
    // An explicitly configured original is still checked at runtime: a provider
    // implemented by this plugin must never earn original comparison credit.
    // Inspect only the published QueryInterface vtable entry, never fields or
    // executable bytes. Only the process executable can earn native credit.
    MEMORY_BASIC_INFORMATION info{};
    const void* query = (*reinterpret_cast<void***>(weather))[0];
    if (!VirtualQuery(query, &info, sizeof(info)) || info.AllocationBase != GetModuleHandleW(nullptr)) {
        ++g_ownerRefused;
        Log("weather shadow: native owner REFUSED weather=%p query=%p module=%p expected=%p",
            weather, query, info.AllocationBase, GetModuleHandleW(nullptr));
        Report("load-owner-refused", 0);
        return;
    }
    Log("weather shadow: native owner weather=%p query=%p module=%p", weather, query, info.AllocationBase);
    g_original = weather;
    FC4_ADDREF(g_original);
    fc4_weather_host host{};
    host.struct_size = sizeof(host);
    host.sim_month = &Month;
    host.world_hemisphere = &Hemisphere;
    // This control seed is NEVER claimed to match the native seed; no RNG-using
    // methods are called. No host post callback, agents or other side effects.
    g_core = fc4_weather_create(&host, 0);
    if (!g_core) ++g_failures;
    Report("load", 0);
}

bool WeatherShadowDeliver(cIGZMessageTarget2* target, const char* name, cIGZMessage2* message,
                          uint32_t type, int64_t data1, bool* finished) {
    if (!g_active || !target || !message || !finished || !name ||
        std::strcmp(name, "cSC4WeatherSimulator") != 0 ||
        (type != kDay && type != kWeek && type != kMonth)) return false;
    ++g_seen;
    cFC4ISC4WeatherSimulator* owner = nullptr;
    if (!target->QueryInterface(kWeatherIID, reinterpret_cast<void**>(&owner)) || !owner) {
        ++g_ownerRefused;
        return false;
    }
    FC4_ACQUIRED(owner, "weather shadow delivery owner QI");
    const bool same = owner == g_original;
    FC4_RELEASE(owner);
    if (!same) {
        ++g_ownerRefused;
        return false;
    }
    // Exactly one authoritative natural delivery. Never invoke the Rust
    // simulator with fabricated seeds or duplicate its host notifications.
    *finished = target->DoMessage(message);
    ++g_originalCalls;
    if (type == kDay) ++g_days;
    if (type == kWeek) ++g_weeks;
    if (type == kMonth) ++g_months;
    ++g_refused;
    CompareCalendar();
    if (type == kMonth) Report("month", data1);
    return true;
}

void WeatherShadowPreSave() {
    if (!g_active) return;
    ++g_saves;
    // This is a notification, not evidence that original Write ran/succeeded.
    Report("presave-not-write", 0);
}

void WeatherShadowCityShutdown() {
    if (!g_active) return;
    Report("precityshutdown-not-Shutdown", 0);
    fc4_weather_destroy(g_core);
    g_core = nullptr;
    FC4_RELEASE(g_original);
    g_city = nullptr;
}
}  // namespace fc4
