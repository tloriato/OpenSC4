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

// Unit tests of plugin/FC4HarnessLogic.h (tooling: sim_turbo,
// autotest_end_date, script): dates and rates.

#include "FC4HarnessLogic.h"

#include <cstdio>
#include <string>

namespace {

int g_failures = 0;

#define CHECK(cond)                                                          \
    do {                                                                     \
        if (!(cond)) {                                                       \
            std::printf("FAILED: %s (line %d)\n", #cond, __LINE__);          \
            ++g_failures;                                                    \
        }                                                                    \
    } while (0)

void DateTests() {
    using namespace fc4::harness;
    Date d;
    CHECK(ParseIsoDate("2000-01-01", &d) && d.year == 2000 && d.month == 1 && d.day == 1);
    CHECK(d.Key() == 20000101);
    CHECK(ParseIsoDate("2000-02-29", &d) && d.day == 29);   // leap (divisible by 400)
    CHECK(!ParseIsoDate("1900-02-29", &d));                  // not leap (divisible by 100)
    CHECK(ParseIsoDate("2004-02-29", nullptr));
    CHECK(!ParseIsoDate("2001-02-29", nullptr));
    CHECK(ParseIsoDate("2001-12-31", nullptr));
    CHECK(!ParseIsoDate("2001-13-01", nullptr));
    CHECK(!ParseIsoDate("2001-00-10", nullptr));
    CHECK(!ParseIsoDate("2001-04-31", nullptr));
    CHECK(!ParseIsoDate("2001-04-00", nullptr));
    CHECK(!ParseIsoDate("0000-01-01", nullptr));
    CHECK(!ParseIsoDate("2001-1-01", nullptr));
    CHECK(!ParseIsoDate("2001/01/01", nullptr));
    CHECK(!ParseIsoDate("2001-01-01 ", nullptr));
    CHECK(!ParseIsoDate("+001-01-01", nullptr));
    CHECK(!ParseIsoDate("", nullptr));
    CHECK(FormatDate(Date{2000, 3, 1}) == "2000-03-01");
    CHECK((Date{2000, 12, 31}).Key() < (Date{2001, 1, 1}).Key());
    CHECK((Date{2000, 2, 1}).Key() > (Date{2000, 1, 31}).Key());
}

void RateTests() {
    using namespace fc4::harness;
    CHECK(RatePer100(31, 2480) == 1250);  // 12.50 days/s
    CHECK(FormatPer100(1250) == "12.50");
    CHECK(FormatPer100(RatePer100(10, 2480)) == "4.03");
    CHECK(RatePer100(5, 0) == 0);
    CHECK(FormatPer100(7) == "0.07");
    CHECK(FormatPer100(0) == "0.00");
}

void JulianTests() {
    using namespace fc4::harness;
    CHECK(JulianFromCivil(2000, 1, 1) == 2451545);
    CHECK(JulianFromCivil(2000, 2, 1) == 2451576);
    CHECK(JulianFromCivil(2000, 2, 29) == 2451604);
    CHECK(JulianFromCivil(2000, 3, 1) == 2451605);
    CHECK(JulianFromCivil(1970, 1, 1) == 2440588);
    CHECK(JulianFromCivil(2001, 1, 1) == 2451911);
    const Date d = CivilFromJulian(2451600);
    CHECK(d.year == 2000 && d.month == 2 && d.day == 25);
    // Round trip over 1899..2101 (leap rules of 100 and 400 included), and
    // consecutive julians are consecutive dates.
    int bad = 0;
    Date prev = CivilFromJulian(JulianFromCivil(1899, 1, 1) - 1);
    for (int32_t j = JulianFromCivil(1899, 1, 1); j <= JulianFromCivil(2101, 12, 31); ++j) {
        const Date c = CivilFromJulian(j);
        if (JulianFromCivil(c.year, c.month, c.day) != j) ++bad;
        if (!ParseIsoDate(FormatDate(c), nullptr)) ++bad;
        if (c.Key() <= prev.Key()) ++bad;
        prev = c;
    }
    CHECK(bad == 0);
}

void EndDateTests() {
    using namespace fc4::harness;
    CHECK(!EndDateDue(0, 2451605, true));               // no end date
    CHECK(!EndDateDue(2451605, 2451604, true));         // the day before
    CHECK(EndDateDue(2451605, 2451605, true));          // the day itself
    CHECK(EndDateDue(2451605, 2451700, true));          // already past (a city loaded after it)
    CHECK(!EndDateDue(2451605, 2451700, false));        // the autotest clock has not started

    RunFigures f;
    FormatRunFigures(2451605, true, 2480, 31, &f);
    CHECK(std::string(f.date) == "2000-03-01");
    CHECK(std::string(f.realSeconds) == "2.480");
    CHECK(std::string(f.daysPerSecond) == "12.50");
    FormatRunFigures(2451545, true, 12, 0, &f);
    CHECK(std::string(f.realSeconds) == "0.012");
    CHECK(std::string(f.daysPerSecond) == "0.00");
    FormatRunFigures(2451545, true, 0, 3, &f);  // no time yet: 0, not a division by zero
    CHECK(std::string(f.daysPerSecond) == "0.00");
    FormatRunFigures(0, false, 999, 5, &f);
    CHECK(std::string(f.date) == "-");
    CHECK(std::string(f.realSeconds) == "-");
    CHECK(std::string(f.daysPerSecond) == "-");
}

// One line through ParseScript: its action (or its error).
fc4::harness::ParsedScript One(const std::string& line) { return fc4::harness::ParseScript(line); }

bool ErrorIs(const std::string& line, const std::string& message) {
    const fc4::harness::ParsedScript p = One(line);
    const bool ok = p.actions.empty() && p.errors.size() == 1 && p.errors[0].line == 1 && p.errors[0].message == message;
    if (!ok) {
        std::printf("  line '%s': %zu action(s), %zu error(s)%s%s\n", line.c_str(), p.actions.size(), p.errors.size(),
                    p.errors.empty() ? "" : ", first: ", p.errors.empty() ? "" : p.errors[0].message.c_str());
    }
    return ok;
}

void ScriptParseTests() {
    using namespace fc4::harness;
    // Good lines.
    {
        const ParsedScript p = One("2000-01-05 zone 10 11 12 13 1");
        CHECK(p.errors.empty() && p.actions.size() == 1);
        const ScriptAction& a = p.actions[0];
        CHECK(a.line == 1 && a.op == ScriptOp::Zone && a.date.Key() == 20000105 && a.julian == 2451549);
        CHECK(a.x0 == 10 && a.z0 == 11 && a.x1 == 12 && a.z1 == 13 && a.type == 1);
        CHECK(a.text == "10 11 12 13 1");
    }
    {
        const ParsedScript p = One("  2000-01-05\tpricezone 20 10 22 12 4   # a commercial block");
        CHECK(p.errors.empty() && p.actions.size() == 1);
        CHECK(p.actions[0].op == ScriptOp::PriceZone && p.actions[0].type == 4 && p.actions[0].x1 == 22);
        CHECK(p.actions[0].text == "20 10 22 12 4");
    }
    {
        const ParsedScript p = One("2000-01-06 DEZONE 10 10 11 12");  // names are case-insensitive
        CHECK(p.errors.empty() && p.actions.size() == 1);
        CHECK(p.actions[0].op == ScriptOp::Dezone && p.actions[0].type == 0 && p.actions[0].z1 == 12);
    }
    {
        const ParsedScript p = One("2000-01-06 zone 0 0 4095 4095 15");  // the limits
        CHECK(p.errors.empty() && p.actions.size() == 1 && p.actions[0].type == 15 && p.actions[0].x1 == 4095);
    }
    {
        const ParsedScript p = One("2000-01-06 zone 7 7 7 7 0");  // one cell, type 0
        CHECK(p.errors.empty() && p.actions.size() == 1 && p.actions[0].op == ScriptOp::Zone && p.actions[0].type == 0);
    }
    CHECK(One("2000-01-07 speed 0").actions.at(0).speed == 0);
    CHECK(One("2000-01-07 speed 2").actions.at(0).speed == 2);
    CHECK(One("2000-01-07 pause").actions.at(0).op == ScriptOp::Pause);
    CHECK(One("2000-01-07 resume").actions.at(0).op == ScriptOp::Resume);
    CHECK(One("2000-01-07 digest # now").actions.at(0).op == ScriptOp::Digest);
    CHECK(One("2000-01-07 turbo on").actions.at(0).on);
    CHECK(!One("2000-01-07 turbo OFF").actions.at(0).on);
    {
        const ParsedScript p = One("2000-01-08 mark  hello   world  # not part of it");
        CHECK(p.errors.empty() && p.actions.size() == 1 && p.actions[0].op == ScriptOp::Mark);
        CHECK(p.actions[0].text == "hello   world");
    }
    {
        const ParsedScript p = One("2000-01-09 save   # in place, a full save");
        CHECK(p.errors.empty() && p.actions.size() == 1 && p.actions[0].op == ScriptOp::Save);
        CHECK(!p.actions[0].fast && p.actions[0].text.empty());
    }
    {
        const ParsedScript p = One("2000-01-09 SAVE Fast");
        CHECK(p.errors.empty() && p.actions.size() == 1 && p.actions[0].op == ScriptOp::Save && p.actions[0].fast);
        CHECK(p.actions[0].text == "Fast");
    }
    {
        const ParsedScript p = One("2000-01-10 network 30 12 20 12 0  # a road, ends in either order");
        CHECK(p.errors.empty() && p.actions.size() == 1 && p.actions[0].op == ScriptOp::Network);
        const ScriptAction& a = p.actions[0];
        CHECK(a.x0 == 30 && a.z0 == 12 && a.x1 == 20 && a.z1 == 12 && a.type == 0 && a.text == "30 12 20 12 0");
    }
    CHECK(One("2000-01-10 NETWORK 0 0 4095 0 12").actions.at(0).type == 12);
    CHECK(ScriptOpName(ScriptOp::Network) == std::string("network"));
    {  // the glue16 lane: terrain x0 z0 x1 z1 height [kind] (a vertex rectangle)
        const ParsedScript p = One("2000-01-10 terrain 3 4 6 8 312.75");
        CHECK(p.errors.empty() && p.actions.size() == 1 && p.actions[0].op == ScriptOp::Terrain);
        CHECK(p.actions[0].x0 == 3 && p.actions[0].z0 == 4 && p.actions[0].x1 == 6 && p.actions[0].z1 == 8);
        CHECK(p.actions[0].height == 312.75f && p.actions[0].kind == 1);
        const ParsedScript q = One("2000-01-10 TERRAIN 0 0 0 0 300 5");
        CHECK(q.errors.empty() && q.actions.size() == 1 && q.actions[0].height == 300.0f && q.actions[0].kind == 5);
        CHECK(ErrorIs("2000-01-10 terrain 1 2 3 4", "terrain takes 5 or 6 argument(s) (x0 z0 x1 z1 height [kind]), "
                                                   "not 4"));
        CHECK(ErrorIs("2000-01-10 terrain 1 2 3 4 -5", "terrain height '-5' is not a decimal 0..5000"));
        CHECK(ErrorIs("2000-01-10 terrain 1 2 3 4 5001", "terrain height '5001' is not a decimal 0..5000"));
        CHECK(ErrorIs("2000-01-10 terrain 1 2 3 4 1.2.3", "terrain height '1.2.3' is not a decimal 0..5000"));
        CHECK(ErrorIs("2000-01-10 terrain 1 2 3 4 300 7", "terrain kind '7' is not 0..6"));
        CHECK(ErrorIs("2000-01-10 terrain 4 2 3 4 300", "the rectangle 4,2..3,4 is reversed (x0 <= x1 and z0 <= z1 "
                                                       "needed)"));
        CHECK(ScriptOpName(ScriptOp::Terrain) == std::string("terrain"));
    }
    CHECK(ScriptOpName(ScriptOp::PriceZone) == std::string("pricezone"));
    CHECK(ScriptOpName(ScriptOp::Save) == std::string("save"));
    CHECK(ScriptOpName(ScriptOp::Disaster) == std::string("disaster"));
    {
        ScriptAction a;
        bool has = false;
        std::string error;
        CHECK(ParseScriptLine("2026-02-01 Disaster FIRE 12 34", 7, &a, &has, &error) && has);
        CHECK(a.op == ScriptOp::Disaster && a.disaster == kDisasterFire && a.x0 == 12 && a.z0 == 34);
        CHECK(ParseScriptLine("2026-02-01 disaster riot 0 4095 # c", 8, &a, &has, &error) && has);
        CHECK(a.disaster == kDisasterRiot && a.x0 == 0 && a.z0 == 4095 && a.text == "riot 0 4095");
        CHECK(ParseScriptLine("2026-02-01 disaster survey", 9, &a, &has, &error) && has);
        CHECK(a.disaster == kDisasterSurvey);
        CHECK(ParseScriptLine("2026-02-01 disaster fire AUTO", 10, &a, &has, &error) && has);
        CHECK(a.disaster == kDisasterFire && a.x0 == -1 && a.z0 == -1);
        // The glue-portdev4 lane: port airport|seaport x z [facing [type]].
        CHECK(ParseScriptLine("2026-02-01 port airport 10 20", 11, &a, &has, &error) && has);
        CHECK(a.op == ScriptOp::Port && a.kind == 0 && a.x0 == 10 && a.z0 == 20 && a.facing == 0 && a.type == 0);
        CHECK(ParseScriptLine("2026-02-01 port Seaport 1 2 3 1", 12, &a, &has, &error) && has);
        CHECK(a.kind == 1 && a.x0 == 1 && a.z0 == 2 && a.facing == 3 && a.type == 1);
        CHECK(ErrorIs("2026-02-01 port heliport 1 2", "port 'heliport' is not airport or seaport"));
        CHECK(ErrorIs("2026-02-01 port airport 1", "port takes 3 to 5 argument(s) (airport|seaport x z [facing [type]])"));
        CHECK(ErrorIs("2026-02-01 port airport 1 2 4", "port facing '4' is not 0..3"));
        // The glue-tutorial4 lane: tutorial query|task n|next|end 0/1|event e|reset a b.
        CHECK(ParseScriptLine("2026-02-01 tutorial query", 13, &a, &has, &error) && has);
        CHECK(a.op == ScriptOp::Tutorial && a.kind == kTutorialQuery);
        CHECK(ParseScriptLine("2026-02-01 tutorial Task 7", 14, &a, &has, &error) && has);
        CHECK(a.kind == kTutorialTask && a.x0 == 7);
        CHECK(ParseScriptLine("2026-02-01 tutorial reset 3 4", 15, &a, &has, &error) && has);
        CHECK(a.kind == kTutorialReset && a.x0 == 3 && a.z0 == 4);
        CHECK(ParseScriptLine("2026-02-01 tutorial end 1", 16, &a, &has, &error) && has);
        CHECK(a.kind == kTutorialEnd && a.x0 == 1);
        CHECK(ParseScriptLine("2026-02-01 tutorial start", 17, &a, &has, &error) && a.kind == kTutorialStart);
        CHECK(ParseScriptLine("2026-02-01 tutorial cheat step 1e", 18, &a, &has, &error) &&
              a.kind == kTutorialCheat && a.text == "step 1e");
        CHECK(ParseScriptLine("2026-02-01 tutorial cheat NEXT 1", 19, &a, &has, &error) &&
              a.kind == kTutorialCheat && a.text == "NEXT 1");
        CHECK(ErrorIs("2026-02-01 tutorial next 1", "tutorial next takes 0 argument(s)"));
        CHECK(ErrorIs("2026-02-01 tutorial end 2", "tutorial end argument '2' is not 0 or 1"));
        CHECK(ParseScriptLine("2015-09-18 tutorial initdone", 20, &a, &has, &error) && a.kind == kTutorialInitDone);
        CHECK(ParseScriptLine("2015-09-18 tutorial hints", 21, &a, &has, &error) && a.kind == kTutorialHints);
        CHECK(ParseScriptLine("2015-09-18 tutorial click 10 20 2", 22, &a, &has, &error) &&
              a.kind == kTutorialClick && a.x0 == 10 && a.z0 == 20 && a.x1 == 2);
        CHECK(ParseScriptLine("2015-09-18 tutorial filter 3 3 11 22", 23, &a, &has, &error) &&
              a.kind == kTutorialFilter && a.x0 == 3 && a.z0 == 3 && a.x1 == 11 && a.z1 == 22);
        CHECK(ErrorIs("2015-09-18 tutorial filter 3 4 0 0", "tutorial filter ID selector is not 0..3"));
        CHECK(ErrorIs("2015-09-18 tutorial click 1 2", "tutorial click takes 3 argument(s)"));
        CHECK(ErrorIs("2026-02-01 port airport 4096 2", "x '4096' is not a cell coordinate 0..4095"));
    }

    // Every error, with the message the log shows.
    CHECK(ErrorIs("2000-13-01 zone 1 1 2 2 1", "'2000-13-01' is not a date YYYY-MM-DD"));
    CHECK(ErrorIs("2001-02-29 pause", "'2001-02-29' is not a date YYYY-MM-DD"));
    CHECK(ErrorIs("zone 1 1 2 2 1", "'zone' is not a date YYYY-MM-DD"));
    CHECK(ErrorIs("2000-01-01", "a date with no action"));
    CHECK(ErrorIs("2000-01-01   # just a date", "a date with no action"));
    CHECK(ErrorIs("2000-01-01 plop 1 2",
                  "unknown action 'plop' (zone, pricezone, dezone, network, speed, pause, resume, turbo, digest, mark, "
                  "save, terrain, disaster, port, tutorial)"));
    CHECK(ErrorIs("2000-01-01 zone 1 2 3 4", "zone takes 5 argument(s) (x0 z0 x1 z1 type), not 4"));
    CHECK(ErrorIs("2000-01-01 zone 1 2 3 4 1 9", "zone takes 5 argument(s) (x0 z0 x1 z1 type), not 6"));
    CHECK(ErrorIs("2000-01-01 pricezone 1 2 3", "pricezone takes 5 argument(s) (x0 z0 x1 z1 type), not 3"));
    CHECK(ErrorIs("2000-01-01 zone a 2 3 4 1", "x0 'a' is not a cell coordinate 0..4095"));
    CHECK(ErrorIs("2000-01-01 zone 1 -2 3 4 1", "z0 '-2' is not a cell coordinate 0..4095"));
    CHECK(ErrorIs("2000-01-01 zone 1 2 4096 4 1", "x1 '4096' is not a cell coordinate 0..4095"));
    CHECK(ErrorIs("2000-01-01 zone 1 2 3 4.5 1", "z1 '4.5' is not a cell coordinate 0..4095"));
    CHECK(ErrorIs("2000-01-01 zone 5 2 3 4 1", "the rectangle 5,2..3,4 is reversed (x0 <= x1 and z0 <= z1 needed)"));
    CHECK(ErrorIs("2000-01-01 dezone 1 9 3 4", "the rectangle 1,9..3,4 is reversed (x0 <= x1 and z0 <= z1 needed)"));
    CHECK(ErrorIs("2000-01-01 zone 1 2 3 4 16", "zone type '16' is not 0..15"));
    CHECK(ErrorIs("2000-01-01 pricezone 1 2 3 4 R", "zone type 'R' is not 0..15"));
    CHECK(ErrorIs("2000-01-01 dezone 1 2 3", "dezone takes 4 argument(s) (x0 z0 x1 z1), not 3"));
    CHECK(ErrorIs("2000-01-01 dezone 1 2 3 4 0", "dezone takes 4 argument(s) (x0 z0 x1 z1), not 5"));
    CHECK(ErrorIs("2000-01-01 network 1 2 3 4", "network takes 5 argument(s) (ax az bx bz type), not 4"));
    CHECK(ErrorIs("2000-01-01 network 1 2 x 4 0", "bx 'x' is not a cell coordinate 0..4095"));
    CHECK(ErrorIs("2000-01-01 network 1 2 3 4 13", "network type '13' is not 0..12"));
    CHECK(ErrorIs("2000-01-01 speed", "speed takes 1 argument(s) (0, 1 or 2), not 0"));
    CHECK(ErrorIs("2000-01-01 speed 3", "speed '3' is not 0, 1 or 2"));
    CHECK(ErrorIs("2000-01-01 pause now", "pause takes 0 argument(s) (none), not 1"));
    CHECK(ErrorIs("2000-01-01 resume 1", "resume takes 0 argument(s) (none), not 1"));
    CHECK(ErrorIs("2000-01-01 digest all", "digest takes 0 argument(s) (none), not 1"));
    CHECK(ErrorIs("2000-01-01 turbo", "turbo takes 1 argument(s) (on or off), not 0"));
    CHECK(ErrorIs("2000-01-01 turbo maybe", "turbo 'maybe' is not on or off"));
    CHECK(ErrorIs("2000-01-01 mark", "mark takes a text"));
    CHECK(ErrorIs("2000-01-01 mark # only a comment", "mark takes a text"));
    CHECK(ErrorIs("2000-01-01 save slow", "save 'slow' is not fast (save, or save fast)"));
    CHECK(ErrorIs("2000-01-01 save fast now", "save takes 0 or 1 argument(s) (fast), not 2"));
    CHECK(ErrorIs("2000-01-01 disaster", "disaster '' is not fire, riot or survey"));
    CHECK(ErrorIs("2000-01-01 disaster flood 1 2", "disaster 'flood' is not fire, riot or survey"));
    CHECK(ErrorIs("2000-01-01 disaster fire 1",
                  "disaster takes 3 argument(s) (fire|riot x z, or fire|riot auto), not 2"));
    CHECK(ErrorIs("2000-01-01 disaster riot 1 2 3",
                  "disaster takes 3 argument(s) (fire|riot x z, or fire|riot auto), not 4"));
    CHECK(ErrorIs("2000-01-01 disaster riot x 2", "x 'x' is not a cell coordinate 0..4095"));
    CHECK(ErrorIs("2000-01-01 disaster fire 1 4096", "z '4096' is not a cell coordinate 0..4095"));
    CHECK(ErrorIs("2000-01-01 disaster survey now", "disaster takes 1 argument(s) (survey), not 2"));
    CHECK(ErrorIs("2000-01-01 save_as test1",
                  "save_as is not available (an open question: what saving a city under another path does to the "
                  "city and its region is not documented); save saves in place"));

    // A whole file: a byte order mark, CRLF, comments and blank lines; every
    // error is reported with its line, and the good lines are still read.
    const std::string file =
        "\xEF\xBB\xBF# a script\r\n"
        "\r\n"
        "2000-01-05 zone 10 10 12 12 1\r\n"
        "2000-01-05 bogus\r\n"
        "   # indented comment\r\n"
        "2000-02-30 pause\r\n"
        "2000-01-05 dezone 10 10 11 12\r\n"
        "2000-01-06 mark done";
    const ParsedScript p = ParseScript(file);
    CHECK(p.actions.size() == 3);
    CHECK(p.errors.size() == 2);
    if (p.errors.size() == 2) {
        CHECK(p.errors[0].line == 4 && p.errors[0].message.find("unknown action 'bogus'") == 0);
        CHECK(p.errors[1].line == 6 && p.errors[1].message == "'2000-02-30' is not a date YYYY-MM-DD");
    }
    if (p.actions.size() == 3) {
        CHECK(p.actions[0].line == 3 && p.actions[1].line == 7 && p.actions[2].line == 8);
        CHECK(p.actions[2].text == "done");
    }
    CHECK(ParseScript("").actions.empty() && ParseScript("").errors.empty());
    CHECK(ParseScript("\n\n# nothing\n").actions.empty() && ParseScript("\n\n# nothing\n").errors.empty());
    // A BOM is only skipped on line 1.
    CHECK(ParseScript("2000-01-01 pause\n\xEF\xBB\xBF" "2000-01-01 pause").errors.size() == 1);
}

void ScriptScheduleTests() {
    using namespace fc4::harness;
    const ParsedScript p = ParseScript(
        "2000-01-05 mark A\n"    // line 1
        "2000-01-03 mark B\n"    // line 2
        "2000-01-05 mark C\n"    // line 3
        "2000-01-03 mark D\n"    // line 4
        "2000-01-10 mark E\n");  // line 5
    CHECK(p.errors.empty() && p.actions.size() == 5);
    ScriptSchedule s;
    s.Load(p.actions);
    CHECK(s.Size() == 5 && s.Remaining() == 5);
    CHECK(s.First() != nullptr && s.First()->text == "B");
    CHECK(s.Last() != nullptr && s.Last()->text == "E");
    auto texts = [](const std::vector<const ScriptAction*>& v) {
        std::string out;
        for (const ScriptAction* a : v) out += a->text;
        return out;
    };
    CHECK(texts(s.Due(JulianFromCivil(2000, 1, 2))).empty());
    CHECK(texts(s.Due(JulianFromCivil(2000, 1, 3))) == "BD");  // one date: file order
    CHECK(texts(s.Due(JulianFromCivil(2000, 1, 3))).empty());  // once only
    CHECK(texts(s.Due(JulianFromCivil(2000, 1, 4))).empty());
    CHECK(texts(s.Due(JulianFromCivil(2000, 1, 7))) == "AC");  // the 5th was not a new day: the first one after it
    CHECK(s.Remaining() == 1);
    CHECK(texts(s.Due(JulianFromCivil(2000, 1, 10))) == "E");
    CHECK(s.Remaining() == 0 && texts(s.Due(JulianFromCivil(2001, 1, 1))).empty());

    // Every date already past when the first new day comes (a city loaded
    // later): all at once, by date, then file order.
    ScriptSchedule late;
    late.Load(p.actions);
    CHECK(texts(late.Due(JulianFromCivil(2005, 6, 1))) == "BDACE");
    CHECK(late.Remaining() == 0);

    // A new Load starts again.
    late.Load(p.actions);
    CHECK(late.Remaining() == 5);
    ScriptSchedule empty;
    empty.Load({});
    CHECK(empty.First() == nullptr && empty.Due(2451545).empty());
}

// disasters.md §5.4 step 4 (the fire coverage average): the clamped inclusive
// rectangle goes to grid slot 8 as x, z, width, height (simgrid.md §4.1).
void CellRectArgsTests() {
    using namespace fc4::harness;
    const GridCellRectArgs a = CellRectArgs(56, 86, 56, 87);  // the logged 1x2 spread target
    CHECK(!a.empty && a.x == 56 && a.z == 86 && a.w == 1 && a.h == 2);
    const GridCellRectArgs b = CellRectArgs(0, 0, 0, 0);  // one cell
    CHECK(!b.empty && b.w == 1 && b.h == 1);
    const GridCellRectArgs c = CellRectArgs(10, 20, 13, 21);
    CHECK(!c.empty && c.w == 4 && c.h == 2);
    CHECK(CellRectArgs(0, 0, -1, -1).empty);  // no overlap with the city
    CHECK(CellRectArgs(5, 5, 5, 4).empty && CellRectArgs(5, 5, 5, 4).h == 0);
}

}  // namespace

int main() {
    DateTests();
    RateTests();
    JulianTests();
    EndDateTests();
    ScriptParseTests();
    ScriptScheduleTests();
    CellRectArgsTests();
    if (g_failures == 0) std::printf("harness logic test: all checks passed\n");
    return g_failures == 0 ? 0 : 1;
}
