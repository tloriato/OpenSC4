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

// Pure logic of the test-harness switches (sim_turbo, autotest_end_date,
// script): dates, rates, the end-date rule, the script parser and its
// schedule. Header-only and free of Windows and game calls, so
// tests/harness_logic_test.cpp checks it directly. Tooling only; no spec
// behaviour.
#pragma once

#include <algorithm>
#include <cstdint>
#include <cstdio>
#include <string>
#include <utility>
#include <vector>

namespace fc4 {
namespace harness {

// A civil date (Gregorian, as the core's julian.rs).
struct Date {
    int32_t year = 0;
    int32_t month = 0;
    int32_t day = 0;
    // yyyymmdd: orders like the dates themselves.
    int32_t Key() const { return year * 10000 + month * 100 + day; }
};

inline bool IsLeapYear(int32_t y) { return (y % 4 == 0 && y % 100 != 0) || y % 400 == 0; }

inline int32_t DaysInMonth(int32_t y, int32_t m) {
    static const int32_t kDays[12] = {31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31};
    if (m < 1 || m > 12) return 0;
    return (m == 2 && IsLeapYear(y)) ? 29 : kDays[m - 1];
}

// Exactly "YYYY-MM-DD" (four, two and two digits), a real day, year >= 1.
inline bool ParseIsoDate(const std::string& s, Date* out) {
    if (s.size() != 10 || s[4] != '-' || s[7] != '-') return false;
    int32_t v[3] = {0, 0, 0};
    const size_t start[3] = {0, 5, 8};
    const size_t len[3] = {4, 2, 2};
    for (int f = 0; f < 3; ++f) {
        for (size_t i = 0; i < len[f]; ++i) {
            const char c = s[start[f] + i];
            if (c < '0' || c > '9') return false;
            v[f] = v[f] * 10 + (c - '0');
        }
    }
    if (v[0] < 1 || v[1] < 1 || v[1] > 12 || v[2] < 1 || v[2] > DaysInMonth(v[0], v[1])) return false;
    if (out != nullptr) {
        out->year = v[0];
        out->month = v[1];
        out->day = v[2];
    }
    return true;
}

inline std::string FormatDate(const Date& d) {
    char b[16];
    std::snprintf(b, sizeof(b), "%04d-%02d-%02d", d.year, d.month, d.day);
    return b;
}

// `count` per second over `ms` milliseconds, times 100 (two decimals in
// integer arithmetic; 0 for ms = 0).
inline uint64_t RatePer100(uint64_t count, uint64_t ms) { return ms == 0 ? 0 : count * 100000ull / ms; }

// "12.34" for a RatePer100 value, into a caller's buffer (no allocation).
inline void FormatPer100To(char* out, size_t cap, uint64_t v) {
    std::snprintf(out, cap, "%llu.%02llu", static_cast<unsigned long long>(v / 100),
                  static_cast<unsigned long long>(v % 100));
}

inline std::string FormatPer100(uint64_t v) {
    char b[32];
    FormatPer100To(b, sizeof(b), v);
    return b;
}

// The Julian day number of a civil date and back (the same numbering as the
// core's julian.rs: 2451545 = 2000-01-01). Pure arithmetic, no allocation, so
// the autotest verdict can use it from any thread.
inline int32_t JulianFromCivil(int32_t year, int32_t month, int32_t day) {
    const int64_t a = (14 - month) / 12;
    const int64_t y = int64_t{year} + 4800 - a;
    const int64_t m = int64_t{month} + 12 * a - 3;
    return static_cast<int32_t>(day + (153 * m + 2) / 5 + 365 * y + y / 4 - y / 100 + y / 400 - 32045);
}

inline Date CivilFromJulian(int32_t julian) {
    int64_t l = int64_t{julian} + 68569;
    const int64_t n = 4 * l / 146097;
    l = l - (146097 * n + 3) / 4;
    const int64_t i = 4000 * (l + 1) / 1461001;
    l = l - 1461 * i / 4 + 31;
    const int64_t j = 80 * l / 2447;
    Date d;
    d.day = static_cast<int32_t>(l - 2447 * j / 80);
    const int64_t k = j / 11;
    d.month = static_cast<int32_t>(j + 2 - 12 * k);
    d.year = static_cast<int32_t>(100 * (n - 49) + i + k);
    return d;
}

// ---- autotest_end_date: the verdict's run figures ----------------------------------------------------

// The end date is due at a new day on or after it, once the autotest clock runs.
inline bool EndDateDue(int32_t endJulian, int32_t julian, bool clockStarted) {
    return endJulian != 0 && clockStarted && julian >= endJulian;
}

// The verdict's `date=…, real_seconds=…, days_per_second=…` values, in fixed
// buffers (the verdict may be written from an exception handler):
//   date            the verdict's julian as YYYY-MM-DD ("-" for julian <= 0);
//   real_seconds    since the autotest clock started (the first tick after the
//                   city load), "S.mmm" ("-" if it never started);
//   days_per_second the new days seen since the clock started over those
//                   seconds, "X.YY" ("-" if it never started). [choice] A
//                   count of SimNewDay messages, not julian differences.
struct RunFigures {
    char date[16];
    char realSeconds[24];
    char daysPerSecond[32];
};

inline void FormatRunFigures(int32_t julian, bool started, uint32_t elapsedMs, uint32_t days, RunFigures* out) {
    if (julian > 0) {
        const Date d = CivilFromJulian(julian);
        std::snprintf(out->date, sizeof(out->date), "%04d-%02d-%02d", d.year, d.month, d.day);
    } else {
        std::snprintf(out->date, sizeof(out->date), "-");
    }
    if (started) {
        std::snprintf(out->realSeconds, sizeof(out->realSeconds), "%u.%03u", elapsedMs / 1000, elapsedMs % 1000);
        FormatPer100To(out->daysPerSecond, sizeof(out->daysPerSecond), RatePer100(days, elapsedMs));
    } else {
        std::snprintf(out->realSeconds, sizeof(out->realSeconds), "-");
        std::snprintf(out->daysPerSecond, sizeof(out->daysPerSecond), "-");
    }
}

// ---- script = <path>: the parser and the schedule ----------------------------------------------------
//
// One action per line, `YYYY-MM-DD <action> <args>`; `#` starts a comment
// anywhere on a line ([choice] also inside a mark's text); blank lines and
// comment lines are skipped; tokens are separated by spaces or tabs; action
// names are case-insensitive ([choice]). Actions:
//   zone x0 z0 x1 z1 type     PlaceZone, commit = 1 (type 0..15, zones.md §2)
//   pricezone x0 z0 x1 z1 type  the same, commit = 0 (a dry run)
//   dezone x0 z0 x1 z1        PlaceZone type 0, commit = 1
//   network ax az bx bz type  a straight network run from cell a to cell b in
//                             real mode (network-manager.md §9,
//                             network-tools.md §3: type 0..12, free, no
//                             regional connection; a, b in either order)
//   speed 0|1|2               SetSimSpeed (turtle, llama, cheetah)
//   pause | resume            one Pause / Resume call
//   turbo on|off              our simulator's turbo clock (FC4Turbo.h)
//   digest                    the observer's DIGEST line now
//   mark <text>               a log line with the text
//   save [fast]               cISC4App::SaveCity(fast) in place, at the next
//                             framework tick (FC4Script.h)
//   port airport|seaport x z [facing [type]]  (tooling, cFC4PortDeveloper.h
//                             PortScriptBuild): the stage-0 lot of port type
//                             `type` (default 0) built at (x, z), facing 0..3
//   terrain x0 z0 x1 z1 h [kind]  cISTETerrain::SetAltitude(rect, h, true, kind)
//                             (terrain.md §6 slot 49): the inclusive VERTEX
//                             rectangle set to the height h (metres, a decimal
//                             0..5000), the water of the rectangle reset and
//                             RedisplayTerrain(1, 1, rect, kind) (kind 0..6, §8.2;
//                             [choice] default 1, the god-mode tool's)
//   disaster fire|riot x z    the disaster layer's manager for fire or riot
//   disaster fire|riot auto   (auto: the survey's first candidate cell):
//                             AttemptDisasterStart(x, z) (manager slot 6; for
//                             the fire IgniteCell(x, z, false, false, 0),
//                             disasters.md §5.5; for the riot §6.2), cells
//   disaster survey           a log of candidate cells for the two (no call
//                             into the layer; FC4DisasterShadow.h)
// Coordinates are whole numbers 0..kScriptMaxCell with x0 <= x1 and z0 <= z1
// ([choice]: a reversed rectangle is an error, not swapped; a network run's
// two ends may come in either order: the tool clamps and draws them as given).

enum class ScriptOp {
    Zone,
    PriceZone,
    Dezone,
    Speed,
    Pause,
    Resume,
    Turbo,
    Digest,
    Mark,
    Save,
    Terrain,
    Disaster,
    Network,
    Port,
    Tutorial,
    Dispatch,
    PortProps,
    Ordinance
};

constexpr int32_t kScriptMaxCell = 4095;  // [choice] cities are at most 256 cells a side
constexpr int32_t kScriptMaxNetworkType = 12;  // network-manager.md §3.1: types 0..12

// The `tutorial` action's operations (glue-tutorial4; `kind`).
constexpr int32_t kTutorialQuery = 0;  // tutorial query
constexpr int32_t kTutorialTask = 1;   // tutorial task n: slot 19 SetTutorialTaskNumber(n)
constexpr int32_t kTutorialNext = 2;   // tutorial next: slot 20 IncrementTutorialTaskNumber
constexpr int32_t kTutorialEnd = 3;    // tutorial end 0|1: slot 6 endTutorial(bool)
constexpr int32_t kTutorialEvent = 4;  // tutorial event e: slot 18 OnTutorialEvent(e)
constexpr int32_t kTutorialReset = 5;  // tutorial reset a b: slot 17 ResetTutorialNumAndIndex(a, b)
constexpr int32_t kScriptMaxTutorialArg = 999999;

// The `disaster` action's kinds.
constexpr int32_t kDisasterFire = 0;
constexpr int32_t kDisasterRiot = 1;
constexpr int32_t kDisasterSurvey = 2;

inline const char* ScriptOpName(ScriptOp op) {
    switch (op) {
        case ScriptOp::Zone: return "zone";
        case ScriptOp::PriceZone: return "pricezone";
        case ScriptOp::Dezone: return "dezone";
        case ScriptOp::Speed: return "speed";
        case ScriptOp::Pause: return "pause";
        case ScriptOp::Resume: return "resume";
        case ScriptOp::Turbo: return "turbo";
        case ScriptOp::Digest: return "digest";
        case ScriptOp::Mark: return "mark";
        case ScriptOp::Save: return "save";
        case ScriptOp::Network: return "network";
        case ScriptOp::Terrain: return "terrain";
        case ScriptOp::Disaster: return "disaster";
        case ScriptOp::Port: return "port";
        case ScriptOp::Tutorial: return "tutorial";
        case ScriptOp::Dispatch: return "dispatch";
        case ScriptOp::PortProps: return "portprops";
        case ScriptOp::Ordinance: return "ordinance";
    }
    return "?";
}

struct ScriptAction {
    int32_t line = 0;    // 1-based line in the file
    Date date;
    int32_t julian = 0;  // of `date`
    ScriptOp op = ScriptOp::Mark;
    int32_t x0 = 0, z0 = 0, x1 = 0, z1 = 0;  // zone, pricezone, dezone; network: the run's ends
    int32_t type = 0;                        // zone, pricezone (dezone: 0); network: the network type
    int32_t speed = 0;                       // speed
    bool on = false;                         // turbo
    bool fast = false;                       // save
    float height = 0.0f;                     // terrain (metres)
    int32_t kind = 1;                        // terrain (RedisplayTerrain's data4)
    int32_t disaster = 0;                    // disaster: kDisasterFire, kDisasterRiot, kDisasterSurvey
    int32_t facing = 0;                      // port: the lot's facing 0..3 (kind: 0 airport, 1 seaport; type)
    std::string text;                        // mark; the arguments as written for the others
    uint32_t ordinanceID = 0;                // ordinance: hexadecimal public ID
};

struct ScriptError {
    int32_t line = 0;
    std::string message;
};

struct ParsedScript {
    std::vector<ScriptAction> actions;  // file order
    std::vector<ScriptError> errors;    // file order; any error refuses the script
};

namespace detail {

inline bool IsBlank(char c) { return c == ' ' || c == '\t'; }

inline std::string TrimBlank(const std::string& s) {
    size_t a = 0, b = s.size();
    while (a < b && (IsBlank(s[a]) || s[a] == '\r')) ++a;
    while (b > a && (IsBlank(s[b - 1]) || s[b - 1] == '\r')) --b;
    return s.substr(a, b - a);
}

inline std::string Lower(std::string s) {
    for (char& c : s) {
        if (c >= 'A' && c <= 'Z') c = static_cast<char>(c - 'A' + 'a');
    }
    return s;
}

// A whole number 0..max, decimal digits only.
inline bool ParseCount(const std::string& s, int32_t max, int32_t* out) {
    if (s.empty() || s.size() > 9) return false;
    int32_t v = 0;
    for (char c : s) {
        if (c < '0' || c > '9') return false;
        v = v * 10 + (c - '0');
    }
    if (v > max) return false;
    *out = v;
    return true;
}

// A height in metres: digits, optionally a point and more digits, 0..max.
inline bool ParseHeight(const std::string& s, double max, float* out) {
    if (s.empty() || s.size() > 12) return false;
    double v = 0.0, scale = 0.0;
    bool digits = false;
    for (char c : s) {
        if (c == '.') {
            if (scale != 0.0) return false;
            scale = 1.0;
            continue;
        }
        if (c < '0' || c > '9') return false;
        digits = true;
        if (scale == 0.0) {
            v = v * 10.0 + (c - '0');
        } else {
            scale *= 0.1;
            v += (c - '0') * scale;
        }
    }
    if (!digits || v > max) return false;
    *out = static_cast<float>(v);
    return true;
}

}  // namespace detail

// Parses one line (no line end). Returns false with `*error` set for a bad
// line; true with `*has` = false for a blank or comment line.
inline bool ParseScriptLine(const std::string& raw, int32_t lineNo, ScriptAction* out, bool* has, std::string* error) {
    *has = false;
    std::string line = raw;
    const size_t hash = line.find('#');
    if (hash != std::string::npos) line.erase(hash);
    line = detail::TrimBlank(line);
    if (line.empty()) return true;
    std::vector<std::string> tok;
    std::vector<size_t> at;  // where each token starts (a mark keeps its text as written)
    for (size_t i = 0; i < line.size();) {
        while (i < line.size() && detail::IsBlank(line[i])) ++i;
        if (i >= line.size()) break;
        const size_t s = i;
        while (i < line.size() && !detail::IsBlank(line[i])) ++i;
        tok.push_back(line.substr(s, i - s));
        at.push_back(s);
    }
    ScriptAction a;
    a.line = lineNo;
    if (!ParseIsoDate(tok[0], &a.date)) {
        *error = "'" + tok[0] + "' is not a date YYYY-MM-DD";
        return false;
    }
    a.julian = JulianFromCivil(a.date.year, a.date.month, a.date.day);
    if (tok.size() < 2) {
        *error = "a date with no action";
        return false;
    }
    const std::string name = detail::Lower(tok[1]);
    const size_t nargs = tok.size() - 2;
    a.text = nargs > 0 ? detail::TrimBlank(line.substr(at[2])) : std::string();
    auto wantArgs = [&](size_t n, const char* usage) {
        if (nargs == n) return true;
        char b[160];
        std::snprintf(b, sizeof(b), "%s takes %u argument(s) (%s), not %u", name.c_str(), static_cast<unsigned>(n),
                      usage, static_cast<unsigned>(nargs));
        *error = b;
        return false;
    };
    auto cells = [&](const char* const* names) {
        int32_t* const dst[4] = {&a.x0, &a.z0, &a.x1, &a.z1};
        for (int i = 0; i < 4; ++i) {
            if (!detail::ParseCount(tok[2 + i], kScriptMaxCell, dst[i])) {
                char b[160];
                std::snprintf(b, sizeof(b), "%s '%s' is not a cell coordinate 0..%d", names[i], tok[2 + i].c_str(),
                              kScriptMaxCell);
                *error = b;
                return false;
            }
        }
        return true;
    };
    auto rect = [&]() {
        static const char* const kNames[4] = {"x0", "z0", "x1", "z1"};
        if (!cells(kNames)) return false;
        if (a.x0 > a.x1 || a.z0 > a.z1) {
            char b[160];
            std::snprintf(b, sizeof(b), "the rectangle %d,%d..%d,%d is reversed (x0 <= x1 and z0 <= z1 needed)", a.x0,
                          a.z0, a.x1, a.z1);
            *error = b;
            return false;
        }
        return true;
    };
    if (name == "zone" || name == "pricezone") {
        a.op = name == "zone" ? ScriptOp::Zone : ScriptOp::PriceZone;
        if (!wantArgs(5, "x0 z0 x1 z1 type") || !rect()) return false;
        if (!detail::ParseCount(tok[6], 15, &a.type)) {
            *error = "zone type '" + tok[6] + "' is not 0..15";
            return false;
        }
    } else if (name == "dezone") {
        a.op = ScriptOp::Dezone;
        if (!wantArgs(4, "x0 z0 x1 z1") || !rect()) return false;
        a.type = 0;
    } else if (name == "network") {
        static const char* const kEnds[4] = {"ax", "az", "bx", "bz"};
        a.op = ScriptOp::Network;
        if (!wantArgs(5, "ax az bx bz type") || !cells(kEnds)) return false;
        if (!detail::ParseCount(tok[6], kScriptMaxNetworkType, &a.type)) {
            *error = "network type '" + tok[6] + "' is not 0..12";
            return false;
        }
    } else if (name == "speed") {
        a.op = ScriptOp::Speed;
        if (!wantArgs(1, "0, 1 or 2")) return false;
        if (!detail::ParseCount(tok[2], 2, &a.speed)) {
            *error = "speed '" + tok[2] + "' is not 0, 1 or 2";
            return false;
        }
    } else if (name == "pause" || name == "resume" || name == "digest") {
        a.op = name == "pause" ? ScriptOp::Pause : name == "resume" ? ScriptOp::Resume : ScriptOp::Digest;
        if (!wantArgs(0, "none")) return false;
    } else if (name == "turbo") {
        a.op = ScriptOp::Turbo;
        if (!wantArgs(1, "on or off")) return false;
        const std::string v = detail::Lower(tok[2]);
        if (v != "on" && v != "off") {
            *error = "turbo '" + tok[2] + "' is not on or off";
            return false;
        }
        a.on = v == "on";
    } else if (name == "mark") {
        a.op = ScriptOp::Mark;
        if (nargs == 0) {
            *error = "mark takes a text";
            return false;
        }
    } else if (name == "save") {
        a.op = ScriptOp::Save;
        if (nargs > 1) {
            char b[160];
            std::snprintf(b, sizeof(b), "save takes 0 or 1 argument(s) (fast), not %u", static_cast<unsigned>(nargs));
            *error = b;
            return false;
        }
        if (nargs == 1) {
            if (detail::Lower(tok[2]) != "fast") {
                *error = "save '" + tok[2] + "' is not fast (save, or save fast)";
                return false;
            }
            a.fast = true;
        }
    } else if (name == "terrain") {
        a.op = ScriptOp::Terrain;
        if (nargs != 5 && nargs != 6) {
            char b[160];
            std::snprintf(b, sizeof(b), "terrain takes 5 or 6 argument(s) (x0 z0 x1 z1 height [kind]), not %u",
                          static_cast<unsigned>(nargs));
            *error = b;
            return false;
        }
        if (!rect()) return false;
        if (!detail::ParseHeight(tok[6], 5000.0, &a.height)) {
            *error = "terrain height '" + tok[6] + "' is not a decimal 0..5000";
            return false;
        }
        a.kind = 1;
        if (nargs == 6 && !detail::ParseCount(tok[7], 6, &a.kind)) {
            *error = "terrain kind '" + tok[7] + "' is not 0..6";
            return false;
        }
    } else if (name == "disaster") {
        a.op = ScriptOp::Disaster;
        const std::string kind = nargs > 0 ? detail::Lower(tok[2]) : std::string();
        if (kind == "survey") {
            a.disaster = kDisasterSurvey;
            if (!wantArgs(1, "survey")) return false;
        } else if (kind == "fire" || kind == "riot") {
            a.disaster = kind == "fire" ? kDisasterFire : kDisasterRiot;
            if (nargs == 2 && detail::Lower(tok[3]) == "auto") {
                a.x0 = a.z0 = a.x1 = a.z1 = -1;  // the survey's first candidate
                *out = a;
                *has = true;
                return true;
            }
            if (!wantArgs(3, "fire|riot x z, or fire|riot auto")) return false;
            static const char* const kNames[2] = {"x", "z"};
            int32_t* const dst[2] = {&a.x0, &a.z0};
            for (int i = 0; i < 2; ++i) {
                if (!detail::ParseCount(tok[3 + i], kScriptMaxCell, dst[i])) {
                    char b[160];
                    std::snprintf(b, sizeof(b), "%s '%s' is not a cell coordinate 0..%d", kNames[i],
                                  tok[3 + i].c_str(), kScriptMaxCell);
                    *error = b;
                    return false;
                }
            }
            a.x1 = a.x0;
            a.z1 = a.z0;
        } else {
            *error = "disaster '" + (nargs > 0 ? tok[2] : std::string()) + "' is not fire, riot or survey";
            return false;
        }
    } else if (name == "ordinance") {
        a.op = ScriptOp::Ordinance;
        if (!wantArgs(3, "hex-ID on|available|enabled 0|1")) return false;
        std::string id = detail::Lower(tok[2]);
        if (id.size() >= 2 && id.substr(0, 2) == "0x") id.erase(0, 2);
        if (id.empty() || id.size() > 8) { *error = "ordinance ID requires 1..8 hex digits"; return false; }
        for (char c : id) {
            const uint32_t digit = c >= '0' && c <= '9' ? uint32_t(c - '0') :
                c >= 'a' && c <= 'f' ? uint32_t(c - 'a' + 10) : 16;
            if (digit == 16) { *error = "invalid ordinance hex ID"; return false; }
            a.ordinanceID = a.ordinanceID * 16 + digit;
        }
        const std::string mode = detail::Lower(tok[3]);
        if (mode != "available" && mode != "on" && mode != "enabled") {
            *error = "ordinance mode must be available, on or enabled"; return false;
        }
        a.kind = mode == "available" ? 0 : mode == "on" ? 1 : 2;
        if (tok[4] != "0" && tok[4] != "1") { *error = "ordinance state must be 0 or 1"; return false; }
        a.on = tok[4] == "1";
    } else if (name == "portprops") {
        // Explicit diagnostic action: native PD12 switch on an existing lot.
        a.op = ScriptOp::PortProps;
        if (!wantArgs(3, "x z state")) return false;
        if (!detail::ParseCount(tok[2], kScriptMaxCell, &a.x0) ||
            !detail::ParseCount(tok[3], kScriptMaxCell, &a.z0) ||
            !detail::ParseCount(tok[4], 1, &a.type)) {
            *error = "portprops requires cells 0..4095 and state 0..1";
            return false;
        }
    } else if (name == "port") {
        // Tooling (glue-portdev4): port airport|seaport x z [facing [type]].
        a.op = ScriptOp::Port;
        const std::string kind = nargs > 0 ? detail::Lower(tok[2]) : std::string();
        if (kind != "airport" && kind != "seaport") {
            *error = "port '" + (nargs > 0 ? tok[2] : std::string()) + "' is not airport or seaport";
            return false;
        }
        a.kind = kind == "airport" ? 0 : 1;
        if (nargs < 3 || nargs > 5) {
            *error = "port takes 3 to 5 argument(s) (airport|seaport x z [facing [type]])";
            return false;
        }
        static const char* const kNames[2] = {"x", "z"};
        int32_t* const dst[2] = {&a.x0, &a.z0};
        for (int i = 0; i < 2; ++i) {
            if (!detail::ParseCount(tok[3 + i], kScriptMaxCell, dst[i])) {
                *error = std::string(kNames[i]) + " '" + tok[3 + i] + "' is not a cell coordinate 0..4095";
                return false;
            }
        }
        a.x1 = a.x0;
        a.z1 = a.z0;
        a.facing = 0;
        a.type = 0;
        if (nargs >= 4 && !detail::ParseCount(tok[5], 3, &a.facing)) {
            *error = "port facing '" + tok[5] + "' is not 0..3";
            return false;
        }
        if (nargs == 5 && !detail::ParseCount(tok[6], 15, &a.type)) {
            *error = "port type '" + tok[6] + "' is not 0..15";
            return false;
        }
    } else if (name == "tutorial") {
        // Tooling (glue-tutorial4): the city's tutorial system (cISC4City slot
        // 82) through its interface. No `start`: the original's startTutorial
        // opens its tutorial window (tutorial.md 4.2).
        a.op = ScriptOp::Tutorial;
        const std::string what = nargs > 0 ? detail::Lower(tok[2]) : std::string();
        size_t want = 0;
        if (what == "query") {
            a.kind = kTutorialQuery;
        } else if (what == "task") {
            a.kind = kTutorialTask, want = 1;
        } else if (what == "next") {
            a.kind = kTutorialNext;
        } else if (what == "end") {
            a.kind = kTutorialEnd, want = 1;
        } else if (what == "event") {
            a.kind = kTutorialEvent, want = 1;
        } else if (what == "reset") {
            a.kind = kTutorialReset, want = 2;
        } else {
            *error = "tutorial '" + what + "' is not query, task, next, end, event or reset";
            return false;
        }
        if (nargs != want + 1) {
            *error = "tutorial " + what + " takes " + std::to_string(want) + " argument(s)";
            return false;
        }
        int32_t* const dst[2] = {&a.x0, &a.z0};
        for (size_t i = 0; i < want; ++i) {
            if (!detail::ParseCount(tok[3 + i], what == "end" ? 1 : kScriptMaxTutorialArg, dst[i])) {
                *error = "tutorial " + what + " argument '" + tok[3 + i] + "' is not " +
                         (what == "end" ? "0 or 1" : "0..999999");
                return false;
            }
        }
    } else if (name == "dispatch") {
        a.op = ScriptOp::Dispatch;
        if (!wantArgs(1, "type")) return false;
        if (!detail::ParseCount(tok[2], 8, &a.type)) {
            *error = "dispatch type must be 0..8";
            return false;
        }
    } else if (name == "save_as") {
        // [question] Not implemented: what cISC4App::SaveCity(path, fast)
        // (slot 15) leaves as the city's save path, and what the region does
        // with a second save of the same city in its folder, are not
        // documented (engine/README.md, the script table).
        *error = "save_as is not available (an open question: what saving a city under another path does to the city "
                 "and its region is not documented); save saves in place";
        return false;
    } else {
        *error = "unknown action '" + tok[1] +
                 "' (zone, pricezone, dezone, network, speed, pause, resume, turbo, digest, mark, save, terrain, "
                 "disaster, port, tutorial, dispatch)";
        return false;
    }
    *out = a;
    *has = true;
    return true;
}

// Parses a whole file: every line, every error (with its line number).
inline ParsedScript ParseScript(const std::string& text) {
    ParsedScript p;
    int32_t lineNo = 0;
    size_t from = 0;
    while (from <= text.size()) {
        size_t end = text.find('\n', from);
        if (end == std::string::npos) end = text.size();
        ++lineNo;
        std::string line = text.substr(from, end - from);
        if (lineNo == 1 && line.size() >= 3 && static_cast<unsigned char>(line[0]) == 0xEF &&
            static_cast<unsigned char>(line[1]) == 0xBB && static_cast<unsigned char>(line[2]) == 0xBF) {
            line.erase(0, 3);  // a UTF-8 byte order mark
        }
        ScriptAction a;
        bool has = false;
        std::string error;
        if (!ParseScriptLine(line, lineNo, &a, &has, &error)) {
            p.errors.push_back(ScriptError{lineNo, error});
        } else if (has) {
            p.actions.push_back(a);
        }
        if (end == text.size()) break;
        from = end + 1;
    }
    return p;
}

// The actions in run order: by date, then file order ([choice] also for
// actions of different past dates that fall due at the same new day). Each
// runs once, at the first new day on or after its date.
class ScriptSchedule {
public:
    void Load(std::vector<ScriptAction> actions) {
        std::stable_sort(actions.begin(), actions.end(),
                         [](const ScriptAction& a, const ScriptAction& b) { return a.julian < b.julian; });
        actions_ = std::move(actions);
        next_ = 0;
    }
    // The actions due at the new day `julian` (not run before), in run
    // order; they count as run from here.
    std::vector<const ScriptAction*> Due(int32_t julian) {
        std::vector<const ScriptAction*> due;
        while (next_ < actions_.size() && actions_[next_].julian <= julian) due.push_back(&actions_[next_++]);
        return due;
    }
    size_t Size() const { return actions_.size(); }
    size_t Remaining() const { return actions_.size() - next_; }
    const ScriptAction* First() const { return actions_.empty() ? nullptr : &actions_.front(); }
    const ScriptAction* Last() const { return actions_.empty() ? nullptr : &actions_.back(); }

private:
    std::vector<ScriptAction> actions_;
    size_t next_ = 0;
};

// The arguments of cISC4SimGrid slot 8 `GetAverageValueInCellRect` for an
// inclusive cell rectangle x0..x1, z0..z1: simgrid.md §4.1 / §8 item 2, the last
// two parameters are a width and a height (not the SDK's bottom-right corner).
// `empty` when the rectangle has no cell (then w or h is 0).
struct GridCellRectArgs {
    int32_t x = 0, z = 0, w = 0, h = 0;
    bool empty = true;
};
inline GridCellRectArgs CellRectArgs(int32_t x0, int32_t z0, int32_t x1, int32_t z1) {
    GridCellRectArgs a;
    a.x = x0;
    a.z = z0;
    a.empty = x1 < x0 || z1 < z0;
    a.w = a.empty ? 0 : x1 - x0 + 1;
    a.h = a.empty ? 0 : z1 - z0 + 1;
    return a;
}

}  // namespace harness
}  // namespace fc4
