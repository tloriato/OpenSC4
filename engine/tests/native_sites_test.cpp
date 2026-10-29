// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

#include "FC4Schedule.h"
#include <cstdio>
#include <iterator>

int main() {
    const fc4::ScheduleSite keys[] = {
        {0x0069B149, 0x0069B0B0}, {0x0093384F, 0x009337F9}, {0x0093388F, 0x009337F9},
        {0x0070E2B0, 0x0070E2B0}, {0x00A802F0, 0},
    };
    const auto missing = [&](uint32_t function, const uint32_t* sites, size_t count) {
        return fc4::MissingScheduleSite(keys, std::size(keys), function, sites, count);
    };
    int failures = 0;
    const auto check = [&](bool ok) { if (!ok) ++failures; };
    // Old DLLs have no site table: unaffected ports work, all affected entries are refused.
    check(missing(0x006BD540, nullptr, 0) == 0);
    check(missing(0x0069B0B0, nullptr, 0) == 0x0069B149);
    check(missing(0x0070E2B0, nullptr, 0) == 0x0070E2B0);
    check(missing(0, nullptr, 0) == 0); // global data is not a function
    const uint32_t grow[] = {0x0069B149};
    check(missing(0x0069B0B0, grow, std::size(grow)) == 0);
    check(missing(0x009337F9, grow, std::size(grow)) == 0x0093384F);
    const uint32_t partial[] = {0x0093384F};
    check(missing(0x009337F9, partial, std::size(partial)) == 0x0093388F);
    const uint32_t complete[] = {0x0093384F, 0x0093388F};
    check(missing(0x009337F9, complete, std::size(complete)) == 0);
    std::printf("native site policy: %d failures\n", failures);
    return failures != 0;
}
