// SPDX-License-Identifier: GPL-3.0-or-later
#pragma once
#include <cstdint>
namespace fc4 {
void SimGridsShadowFixtures();
void SimGridsShadowTrack(const char* owner, uint64_t token);
void SimGridsShadowValue(uint64_t token, int32_t x, int32_t z, const void* value, uint32_t size);
void SimGridsShadowBulkRead(uint64_t token, const void* values, uint32_t count, uint32_t size);
void SimGridsShadowSet(uint64_t token, int32_t x, int32_t z, const void* value, uint32_t size);
void SimGridsShadowFill(uint64_t token, double value);
void SimGridsShadowBulkWrite(uint64_t token, const uint32_t* indices, const void* values, uint32_t count, uint32_t size);
void SimGridsShadowShutdown(uint64_t token, bool result);
void SimGridsShadowRelease(uint64_t token);
}
