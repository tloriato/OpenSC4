// SPDX-License-Identifier: GPL-3.0-or-later
#include "fc4.h"
#include <cstdint>
#include <cstring>
#include <cstdio>
#include <vector>

static std::vector<int32_t> calls;
static int32_t Upper(int32_t x) { return x >= 65 && x <= 90 ? 0x100 : 0; }
static int32_t Lower(int32_t x) {
    calls.push_back(x);
    return (x >= 65 && x <= 90 ? x + 32 : x) | 0x10000;
}
#define REQUIRE(x) do { if (!(x)) { std::fprintf(stderr, "line %d: %s\n", __LINE__, #x); return 1; } } while (0)
int main() {
    REQUIRE(fc4_renderprops_create(nullptr, Lower) == nullptr);
    REQUIRE(fc4_renderprops_create(Upper, nullptr) == nullptr);
    fc4_renderprops* a = fc4_renderprops_create(Upper, Lower);
    fc4_renderprops* b = fc4_renderprops_create(Upper, Lower);
    REQUIRE(a && b);
    const int counts[] = {38, 24, 15};
    for (uint32_t k = 0; k < 3; ++k) REQUIRE(fc4_renderprops_count(a, k) == counts[k]);
    uint8_t bytes[] = {'R', 'S', 0, 0x80, 0xff};
    calls.clear();
    REQUIRE(fc4_renderprops_find(a, 0, bytes, sizeof(bytes)) == 9);
    REQUIRE((calls == std::vector<int32_t>{82, 83}));
    const uint8_t high[] = {0x49, 0x80, 0xd0, 0xff, 0};
    calls.clear();
    REQUIRE(fc4_renderprops_find(a, 0, high, sizeof(high)) == -1);
    REQUIRE((calls == std::vector<int32_t>{73, -128, -48, -1}));
    uint8_t name[32];
    std::memset(name, 0x5a, sizeof(name));
    REQUIRE(fc4_renderprops_name(a, 0, 9, name, sizeof(name)) == 13);
    REQUIRE(std::memcmp(name, "rendershadowsZ", 14) == 0);
    REQUIRE(fc4_renderprops_name(a, 0, 9, name, 3) == -2);
    REQUIRE(fc4_renderprops_name(a, 0, 9, nullptr, 0) == 13);
    uint32_t out = 0xdeadbeef;
    REQUIRE(fc4_renderprops_get(a, 0, 38, &out) == 0 && out == 0xdeadbeef);
    REQUIRE(fc4_renderprops_set(a, 0, 38, 1) == 0);
    REQUIRE(fc4_renderprops_set(a, 0, 0, 2) == 0);
    for (uint32_t input : {0x80000000u, 0x7fc01234u, 0x7f800000u, 0xc2150000u}) {
        REQUIRE(fc4_renderprops_set(a, 2, 3, input) == 1);
        REQUIRE(fc4_renderprops_get(a, 2, 3, &out) == 1 && out == input);
    }
    REQUIRE(fc4_renderprops_get(b, 2, 3, &out) == 1 && out == 0x3f800000);
    fc4_renderprops_destroy(a);
    REQUIRE(fc4_renderprops_get(b, 2, 3, &out) == 1 && out == 0x3f800000);
    fc4_renderprops_destroy(b);
    fc4_renderprops_destroy(nullptr);
    std::puts("render properties C ABI: PASS");
    return 0;
}
