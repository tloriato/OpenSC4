// SPDX-License-Identifier: GPL-3.0-or-later
#pragma once

#include <cstdint>

class cISC4City;
class cIGZMessage2;
class cIGZMessageTarget2;

namespace fc4 {
// A deliberately incomplete original-authoritative shadow. Only the two
// calendar getters have a captured-input contract today (weather question WY1).
// No simulation, initialization, serialization or RNG equality is claimed.
void WeatherShadowStartup(bool simulatorReplaced, bool weatherReplaced);
void WeatherShadowCityLoaded(cISC4City* city);
void WeatherShadowPreSave();
void WeatherShadowCityShutdown();
bool WeatherShadowDeliver(cIGZMessageTarget2* target, const char* name, cIGZMessage2* message,
                          uint32_t type, int64_t data1, bool* finished);
}  // namespace fc4
