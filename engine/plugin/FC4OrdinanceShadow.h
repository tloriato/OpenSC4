// SPDX-License-Identifier: GPL-3.0-or-later
#pragma once
#include <cstdint>
class cIGZMessage2;
class cIGZMessageTarget2;
namespace fc4 {
void OrdinanceShadowStartup(bool replaced);
bool OrdinanceShadowDeliver(cIGZMessageTarget2*, const char*, cIGZMessage2*, uint32_t, bool*);
void OrdinanceShadowCityShutdown();
}
