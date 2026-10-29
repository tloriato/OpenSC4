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

#include "FC4Schedule.h"

#include "FC4Config.h"
#include "FC4Log.h"
#include "FC4Seed.h"
#include "fc4.h"

#include "cIGZFrameWork.h"
#include "cRZCOMDllDirector.h"

#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#include <windows.h>
#include <intrin.h>
#include <tlhelp32.h>

#include <algorithm>
#include <cctype>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <cwchar>
#include <functional>
#include <map>
#include <queue>
#include <string>
#include <unordered_map>
#include <vector>

#pragma intrinsic(_ReturnAddress)
#pragma intrinsic(_AddressOfReturnAddress)

extern "C" IMAGE_DOS_HEADER __ImageBase;  // this DLL's load address

#ifndef FC4_SCHEDULE_CODE
#define FC4_SCHEDULE_CODE "unknown"
#endif

namespace {

// A hash of this file and FC4Schedule.h (engine/CMakeLists.txt); tools/golden.py reads it from the DLL file.
const char kScheduleCode[] = "FC4_SCHEDULE_CODE=" FC4_SCHEDULE_CODE;

// ---- addresses in the 1.1.641 executable (an interop table; checked by Supported) ----------------
constexpr uintptr_t kLimitTimerStart = 0x0068BF50;    // small-owners-b9.md §4
constexpr uintptr_t kLimitTimerExpired = 0x0068BF80;  // small-owners-b9.md §4
constexpr uintptr_t kGrowExpiredReturn = 0x0069B149;  // the call at 0x0069B144 in Grow (building-development.md §4.4)
constexpr uintptr_t kRemainingMs = 0x0070E2B0;        // traffic: a timer's remaining ms, for the path finder's budget
constexpr uintptr_t kCrtTime = 0x009EFB59;            // time(time_t*), 32-bit time_t
constexpr uintptr_t kCrtClock = 0x009F361F;           // clock()
constexpr uintptr_t kCrtSrand = 0x009EFB92;           // srand(unsigned), the calling thread's state (foundations §12)
constexpr uintptr_t kCrtGetPtd = 0x009F7604;          // _getptd(): the thread's CRT data; rand()'s state at +0x14
constexpr uintptr_t kRandomFirst = 0x00913600;        // cRZRandom / cRZRandomSuper code (foundations §6.1)
constexpr uintptr_t kRandomEnd = 0x00913C00;
constexpr uintptr_t kRandomCtor = 0x00913A5C;         // cRZRandom::cRZRandom(seed)
constexpr uintptr_t kRandomSuperCtor = 0x00913B74;    // cRZRandomSuper::cRZRandomSuper(seed)
constexpr uintptr_t kWriteRef = 0x005D4810;           // the serializer's reference writer (foundations §2.7)
constexpr uintptr_t kRegisterObject = 0x005D43F0;     // its registration (object, id, class)
constexpr uintptr_t kQpcSlot = 0x00A802F0;            // the import slot of QueryPerformanceCounter (cRZTimer, timers.md)
constexpr uintptr_t kSleepSlot = 0x00A800C8;          // Sleep
constexpr uintptr_t kTempPathASlot = 0x00A8029C;      // the import slot of GetTempPathA
constexpr uintptr_t kControllerWriteSlot = 0x00A9602C;  // the automata controller manager's Write (0x005174D0), a vtable slot
constexpr uintptr_t kTrafficMapWriteSlot = 0x00A8FEAC;  // the traffic network map's Write (0x004B1CE0), a vtable slot
constexpr uintptr_t kWatercraftWriteSlot = 0x00A9DF1C;  // the watercraft manager's Write (0x0056FA70), a vtable slot
constexpr uintptr_t kAdjacentCitiesSlot = 0x00AB1F30;  // the neighbours simulator's GetAdjacentCities (0x006D01E0), a vtable slot
constexpr uintptr_t kTempPathW = 0x00B62904;          // cRZFile's GetTempPathW, from GetProcAddress at start-up
constexpr uintptr_t kForeground = 0x005F85F0;         // "the game is the foreground window", asked by the save's thumbnail
const uint8_t kForegroundBytes[] = {0x56, 0xFF, 0x15, 0x0C, 0x05, 0xA8, 0x00};  // push esi; call [GetForegroundWindow]
constexpr uintptr_t kSleepExSlot = 0x00A802F8;        // SleepEx (cRZThread::SleepCurrentThread)
constexpr uintptr_t kTimerOverhead = 0x00B54358;      // cRZTimer's calibrated call overhead o, u64 (timers.md §1.5)
constexpr uintptr_t kMallocEntry = 0x009F066E;          // the C runtime heap (statically linked)
constexpr uintptr_t kFreeEntry = 0x009F015B;
constexpr uintptr_t kReallocEntry = 0x009F2329;
constexpr uintptr_t kCallocEntry = 0x009FE846;
constexpr uintptr_t kMsizeEntry = 0x009F6CAB;
constexpr uintptr_t kFastAllocate = 0x005B9FC0;       // the framework allocator's Allocate(n) (framework-misc2.md §2.1)
constexpr uintptr_t kProcessHeapAlloc = 0x0090CF72;   // an allocator back-end: HeapAlloc(GetProcessHeap(), 0, n)
constexpr uintptr_t kProcessHeapFree = 0x0090CF86;    // its HeapFree
constexpr uintptr_t kChunkAlloc = 0x0090D089;         // a back-end: VirtualAlloc of a 512 KB chunk
constexpr uintptr_t kChunkFree = 0x0090D0A0;          // its VirtualFree(second argument)
constexpr uintptr_t kBlockAlloc = 0x0090D8E0;         // the object pools' page source: 2^shift bytes, so aligned
constexpr uintptr_t kBlockFree = 0x0090D91C;          // its free(p, shift)

// Site inventory for the 1.1.641 executable: {site, containing function entry}.
// Includes entry hooks, interior patches, seed stack keys and return-address keys.
// Data patches have no containing function (0). Keep this complete when adding hooks.
const fc4::ScheduleSite kScheduleSites[] = {
    {0x0041AA02, 0x0041A990},  // kSeedSites
    {0x00429A68, 0x00429890},  // kSeedSites
    {0x004489A4, 0x004487F0},  // kWalkSites
    {0x004489CD, 0x004487F0},  // kTickStep
    {0x00458A78, 0x00458A60},  // kPoolTakes
    {0x00458F58, 0x00458F40},  // kPoolTakes
    {0x0045DA58, 0x0045D9D0},  // kSeedSites
    {0x0046820F, 0x004681A0},  // kInlineWalks
    {0x00468337, 0x004681A0},  // kInlineWalks
    {0x00468912, 0x00468410},  // kInlineWalks
    {0x00468DF8, 0x00468410},  // kInlineWalks
    {0x00472978, 0x00472960},  // kPoolTakes
    {0x00472C48, 0x00472C30},  // kPoolTakes
    {0x00477338, 0x00477320},  // kPoolTakes
    {0x004776C8, 0x004776B0},  // kPoolTakes
    {0x004B0390, 0x004B0390},  // kHashBegin
    {0x004B434D, 0x004B4300},  // kIntersectionInsert
    {0x004B459B, 0x004B44F0},  // kJunctionInsert
    {0x004B478B, 0x004B4710},  // kPoolTakes
    {0x004B4980, 0x004B48B0},  // kWalkSites
    {0x004B49A0, 0x004B48B0},  // kWalkSites
    {0x004B4EE4, 0x004B4DE0},  // kPoolTakes
    {0x004F6D2B, 0x004F6C80},  // kPoolTakes
    {0x004F747A, 0x004F7300},  // kPoolTakes
    {0x004F79D0, 0x004F7850},  // kPoolTakes
    {0x004F9F40, 0x004F9D70},  // kPoolTakes
    {0x00518961, 0x00518950},  // kWalkSites
    {0x005189AB, 0x00518950},  // kWalkSites
    {0x00518C75, 0x00518B80},  // kWalkSites
    {0x00518D8C, 0x00518B80},  // kWalkSites
    {0x0052A02F, 0x00529FD0},  // kSeedSites
    {0x0052A22D, 0x0052A1E0},  // kWalkSites
    {0x0052A2D7, 0x0052A1E0},  // kWalkSites
    {0x0052A31B, 0x0052A1E0},  // kWalkExits
    {0x0052A4A9, 0x0052A380},  // kWalkSites
    {0x0052A4D0, 0x0052A380},  // kWalkExits
    {0x0052A560, 0x0052A380},  // kWalkSites
    {0x0054092B, 0x00540910},  // kPoolTakes
    {0x00540AE8, 0x00540AC0},  // kPoolTakes
    {0x00541B03, 0x00541AE0},  // kPoolTakes
    {0x00542828, 0x00542800},  // kPoolTakes
    {0x005448B3, 0x00544890},  // kPoolTakes
    {0x005469BC, 0x00546990},  // kPoolTakes
    {0x005572B3, 0x00557290},  // kPoolTakes
    {0x0055B993, 0x0055B970},  // kPoolTakes
    {0x00562C3C, 0x00562C10},  // kPoolTakes
    {0x00573FFE, 0x005736A0},  // kInlineWalks
    {0x005740D9, 0x005736A0},  // kInlineWalks
    {0x00574120, 0x005736A0},  // kInlineWalks
    {0x005741C6, 0x005736A0},  // kInlineWalks
    {0x0057ADD9, 0x0057AD90},  // kSeedSites
    {0x0057D072, 0x0057D040},  // kSeedSites
    {0x005B9FC0, 0x005B9FC0},  // kFastAllocate
    {0x005CA8B8, 0x005CA8A0},  // kPoolTakes
    {0x005D0BD8, 0x005D0BC0},  // kPoolTakes
    {0x005D4810, 0x005D4810},  // kWriteRef
    {0x005D5518, 0x005D5500},  // kPoolTakes
    {0x005D6238, 0x005D6220},  // kPoolTakes
    {0x005D64D8, 0x005D64C0},  // kPoolTakes
    {0x005DE9C2, 0x005DDEC0},  // kThumbnailWaitReturn
    {0x005DED08, 0x005DECF0},  // kPoolTakes
    {0x005E00CD, 0x005E0050},  // kPoolTakes
    {0x005E0838, 0x005E0820},  // kPoolTakes
    {0x005E14A8, 0x005E1490},  // kPoolTakes
    {0x005E2A68, 0x005E2A50},  // kPoolTakes
    {0x005E3348, 0x005E3330},  // kPoolTakes
    {0x005EBE08, 0x005EBDF0},  // kPoolTakes
    {0x005ECE78, 0x005ECE60},  // kPoolTakes
    {0x005ED6F8, 0x005ED6E0},  // kPoolTakes
    {0x005ED9B8, 0x005ED9A0},  // kPoolTakes
    {0x005EDAF8, 0x005EDAE0},  // kPoolTakes
    {0x005EDD98, 0x005EDD80},  // kPoolTakes
    {0x005EEF68, 0x005EEF50},  // kPoolTakes
    {0x005EF198, 0x005EF180},  // kPoolTakes
    {0x005EFC98, 0x005EFC80},  // kPoolTakes
    {0x005F04A8, 0x005F0490},  // kPoolTakes
    {0x005F0578, 0x005F0560},  // kPoolTakes
    {0x005F5068, 0x005F5050},  // kPoolTakes
    {0x005F5CF8, 0x005F5CE0},  // kPoolTakes
    {0x005F7D98, 0x005F7D80},  // kPoolTakes
    {0x005F84E8, 0x005F84D0},  // kPoolTakes
    {0x005F85F0, 0x005F85F0},  // kForeground
    {0x005FD858, 0x005FD840},  // kPoolTakes
    {0x005FD908, 0x005FD8F0},  // kPoolTakes
    {0x005FDF18, 0x005FDEF0},  // kPoolTakes
    {0x005FDFF0, 0x005FDFD0},  // kPoolTakes
    {0x005FE244, 0x005FE210},  // kPoolTakes
    {0x005FE2D4, 0x005FE2A0},  // kPoolTakes
    {0x005FE364, 0x005FE330},  // kPoolTakes
    {0x005FE3F4, 0x005FE3C0},  // kPoolTakes
    {0x005FE484, 0x005FE450},  // kPoolTakes
    {0x005FE524, 0x005FE4F0},  // kPoolTakes
    {0x005FE5B4, 0x005FE580},  // kPoolTakes
    {0x005FE644, 0x005FE610},  // kPoolTakes
    {0x005FE6E4, 0x005FE6B0},  // kPoolTakes
    {0x005FE784, 0x005FE750},  // kPoolTakes
    {0x005FE824, 0x005FE7F0},  // kPoolTakes
    {0x005FF304, 0x005FF280},  // kPoolTakes
    {0x005FF7E9, 0x005FF770},  // kPoolTakes
    {0x005FF8DC, 0x005FF860},  // kPoolTakes
    {0x005FF9CC, 0x005FF950},  // kPoolTakes
    {0x005FFAC1, 0x005FFA40},  // kPoolTakes
    {0x00605058, 0x00605040},  // kPoolTakes
    {0x00605468, 0x00605450},  // kPoolTakes
    {0x00619EA8, 0x00619E90},  // kPoolTakes
    {0x0061C498, 0x0061C480},  // kPoolTakes
    {0x0061D218, 0x0061D200},  // kPoolTakes
    {0x0061E838, 0x0061E820},  // kPoolTakes
    {0x00647248, 0x00647230},  // kPoolTakes
    {0x00647808, 0x006477F0},  // kPoolTakes
    {0x006487E8, 0x006487D0},  // kPoolTakes
    {0x006489C8, 0x006489B0},  // kPoolTakes
    {0x006499E8, 0x006499D0},  // kPoolTakes
    {0x0064A2F8, 0x0064A2E0},  // kPoolTakes
    {0x0064AB88, 0x0064AB70},  // kPoolTakes
    {0x0064E358, 0x0064E340},  // kPoolTakes
    {0x00652FF8, 0x00652FE0},  // kPoolTakes
    {0x00653578, 0x00653560},  // kPoolTakes
    {0x0068BF50, 0x0068BF50},  // kLimitTimerStart
    {0x0068BF80, 0x0068BF80},  // kLimitTimerExpired
    {0x0068D2B8, 0x0068D280},  // kSeedSites
    {0x0069B149, 0x0069B0B0},  // kGrowExpiredReturn
    {0x0069E02E, 0x0069E010},  // kSeedSites
    {0x006ADA65, 0x006ADA50},  // kSeedSites
    {0x006AF19A, 0x006AF150},  // kReseedSites
    {0x006B193A, 0x006B1880},  // kSeedSites
    {0x006BF448, 0x006BF430},  // kPoolTakes
    {0x006C2722, 0x006C26D0},  // kPoolTakes
    {0x006C2FD8, 0x006C2FC0},  // kPoolTakes
    {0x006C3434, 0x006C3100},  // kPoolTakes
    {0x006D0628, 0x006D0520},  // kSeedSites
    {0x006D585A, 0x006D57D0},  // kSeedSites
    {0x006D9131, 0x006D90F0},  // kSwitchInsert
    {0x006DF11B, 0x006DF0D0},  // kSeedSites
    {0x006E1F95, 0x006E1F80},  // kSeedSites
    {0x006F34B6, 0x006F3410},  // kSeedSites
    {0x007005D6, 0x00700510},  // kSeedSites
    {0x0070E2B0, 0x0070E2B0},  // kRemainingMs
    {0x0070FBA9, 0x0070FB70},  // kPoolTakes
    {0x0070FC34, 0x0070FB70},  // kPoolTakes
    {0x0071F1A5, 0x0071E7E0},  // kSeedSites
    {0x00728E58, 0x00728C40},  // kSeedSites
    {0x00732F87, 0x00732BF0},  // kWalkSites
    {0x007331C1, 0x00732BF0},  // kWalkSites
    {0x007C5412, 0x007C5370},  // kPoolTakes
    {0x007CA292, 0x007CA200},  // kPoolTakes
    {0x007FB663, 0x007FB640},  // kPoolTakes
    {0x007FC463, 0x007FC440},  // kPoolTakes
    {0x007FDF08, 0x007FDCBA},  // kPoolTakes
    {0x007FED68, 0x007FED50},  // kPoolTakes
    {0x00801B23, 0x00801B00},  // kPoolTakes
    {0x008065E8, 0x008065C0},  // kPoolTakes
    {0x00890198, 0x00890198},  // kStopwatchReads
    {0x00890279, 0x00890279},  // kStopwatchReads
    {0x00890377, 0x00890377},  // kStopwatchReads
    {0x008905F0, 0x008905F0},  // kStopwatchReads
    {0x0090CF72, 0x0090CF72},  // kProcessHeapAlloc
    {0x0090CF86, 0x0090CF86},  // kProcessHeapFree
    {0x0090D089, 0x0090D089},  // kChunkAlloc
    {0x0090D0A0, 0x0090D0A0},  // kChunkFree
    {0x0090D8E0, 0x0090D8E0},  // kBlockAlloc
    {0x0090D91C, 0x0090D91C},  // kBlockFree
    {0x00913683, 0x00913667},  // clock return in Seed
    {0x00913667, 0x00913667},  // kRandomSeed
    {0x0091373B, 0x0091372E},
    {0x00913740, 0x0091372E},  // clock return in RandomDoubleUniform  // kZeroStateClock
    {0x00913A72, 0x00913A72},
    {0x00913A86, 0x00913A72},  // clock return in RandomSuper::Seed  // kRandomSuperSeed
    {0x009205B6, 0x009205A0},  // kSleepCurrentThreadReturn
    {0x0093384F, 0x009337F9},  // kAddTimerNow
    {0x0093388F, 0x009337F9},  // kAddTimerNow
    {0x00933900, 0x009338C2},  // kWorkerNow
    {0x00933905, 0x009338C2},  // worker return
    {0x009339AA, 0x009338C2},  // kWorkerNow
    {0x009339AF, 0x009338C2},  // worker return
    {0x009EFB59, 0x009EFB59},  // kCrtTime
    {0x009F015B, 0x009F015B},  // kFreeEntry
    {0x009F066E, 0x009F066E},  // kMallocEntry
    {0x009F2329, 0x009F2329},  // kReallocEntry
    {0x009F361F, 0x009F361F},  // kCrtClock
    {0x009F6CAB, 0x009F6CAB},  // kMsizeEntry
    {0x009FE846, 0x009FE846},  // kCallocEntry
    {0x00A800C8, 0x00000000},  // kSleepSlot
    {0x00A8029C, 0x00000000},  // kTempPathASlot
    {0x00A802F0, 0x00000000},  // kQpcSlot
    {0x00A802F8, 0x00000000},  // kSleepExSlot
    {0x00A8FEAC, 0x00000000},  // kTrafficMapWriteSlot
    {0x00A9602C, 0x00000000},  // kControllerWriteSlot
    {0x00A9DF1C, 0x00000000},  // kWatercraftWriteSlot
    {0x00AB1F30, 0x00000000},  // kAdjacentCitiesSlot
    {0x00B54358, 0x00000000},  // kTimerOverhead
    {0x00B62904, 0x00000000},  // kTempPathW
};
constexpr uint32_t kReturnKeys[] = {
    0x0041AA02,
    0x00429A68,
    0x0045DA58,
    0x0052A02F,
    0x0057ADD9,
    0x0057D072,
    0x005DE9C2,
    0x0068D2B8,
    0x0069B149,
    0x0069E02E,
    0x006ADA65,
    0x006AF19A,
    0x006B193A,
    0x006D0628,
    0x006D585A,
    0x006DF11B,
    0x006E1F95,
    0x006F34B6,
    0x007005D6,
    0x0071F1A5,
    0x00728E58,
    0x00913683,
    0x00913740,
    0x00913A86,
    0x009205B6,
    0x00933905,
    0x009339AF,
};
uintptr_t g_nativeReturns[std::size(kScheduleSites)] = {};

uintptr_t OriginalReturn(uintptr_t site) {
    for (size_t i = 0; i < std::size(kScheduleSites); ++i) {
        if (g_nativeReturns[i] != 0 && site == g_nativeReturns[i]) return kScheduleSites[i].site;
    }
    return site;
}

// The bytes each hooked entry must hold before we write it.
const uint8_t kStartBytes[] = {0x53, 0x56, 0x57, 0x8B, 0xF1, 0x0F, 0x31};
const uint8_t kExpiredBytes[] = {0x56, 0x0F, 0x31, 0x8B, 0x31};
const uint8_t kRemainingBytes[] = {0x83, 0xEC, 0x08, 0x56, 0x0F, 0x31};
const uint8_t kTimeBytes[] = {0x55, 0x8B, 0xEC, 0x51, 0x51};
const uint8_t kClockBytes[] = {0x55, 0x8B, 0xEC, 0x51, 0x51};
const uint8_t kWriteRefBytes[] = {0x8B, 0x41, 0x2C, 0x85, 0xC0};
// The heap entries are copied whole into trampolines (no relative operands).
const uint8_t kMallocBytes[] = {0xFF, 0x35, 0x40, 0xD9, 0xBA, 0x00};
const uint8_t kFreeBytes[] = {0x6A, 0x0C, 0x68, 0x60, 0x63, 0xAE, 0x00};
const uint8_t kReallocBytes[] = {0x6A, 0x14, 0x68, 0x80, 0x64, 0xAE, 0x00};
const uint8_t kCallocBytes[] = {0x6A, 0x10, 0x68, 0x88, 0xBA, 0xAE, 0x00};
const uint8_t kMsizeBytes[] = {0x6A, 0x10, 0x68, 0xA0, 0x68, 0xAE, 0x00};
const uint8_t kFastAllocateBytes[] = {0x56, 0x57, 0x8B, 0x7C, 0x24, 0x0C};
const uint8_t kProcessHeapBytes[] = {0xFF, 0x74, 0x24, 0x04, 0x6A, 0x00};
const uint8_t kChunkAllocBytes[] = {0x6A, 0x04, 0x68, 0x00, 0x10, 0x00, 0x00};
const uint8_t kChunkFreeBytes[] = {0x68, 0x00, 0x80, 0x00, 0x00};
const uint8_t kBlockAllocBytes[] = {0x56, 0x57, 0xBF, 0xFC, 0x25, 0xB6, 0x00};
const uint8_t kBlockFreeBytes[] = {0x55, 0x8B, 0xEC, 0x56, 0xBE, 0xFC, 0x25, 0xB6, 0x00};

// [choice] The Grow timer expires at its 16th read: every qualifying developer
// (at most 13 with the shipped exemplars) is picked first, and 16 reads keep the
// adaptive step at 1024 (5 <= n <= 20, building-development.md §4.4).
constexpr uint32_t kGrowReads = 16;
// [choice] Any other timer expires only after 2^24 reads (none should get there:
// it would be a spin like Grow's; logged).
constexpr uint32_t kOtherReads = 1u << 24;
uint32_t g_otherReads = kOtherReads;  // `schedule limit reads` (the N and 2N check)
// A timer Start set holds this in its high dword. The others are deadlines built
// inline from rdtsc + 0x7FFFFFFFFFFFFFFF (traffic DoPostCityInit): they never expire.
constexpr uint32_t kStarted = 0x5C4ED000;
// [choice] Traffic's remaining-time query; only cSC4PathFinder::FindPath's Start
// uses the result, and Start ignores it under the schedule.
constexpr uint32_t kRemaining = 1;
// [choice] rand()'s state at the city load: derive(S, K).
constexpr uint32_t kLoadRandK = 0x4C0AD000;
// [choice] The wall clock seen by the original under the schedule.
constexpr uint32_t kTime = 1767225600;  // 2026-01-01 00:00:00 UTC
constexpr int32_t kClock = 1000;        // ms since process start
// [choice] The main thread's performance counter: +1 tick per read, and at least
// kFrameMs per frame of the simulator (cFC4Simulator::OnTick). Other threads keep the real one.
constexpr uint32_t kFrameMs = 33;
// A main-thread sleep advances that counter by its length. It really sleeps as long,
// except the save's thumbnail wait (up to 10 s for the model loader in 500 ms sleeps,
// SC4WriteCityRegionViewThumbnail), which really sleeps 1 ms.
constexpr DWORD kRealSleepMs = 1;
constexpr uintptr_t kSleepCurrentThreadReturn = 0x009205B6;  // cRZThread::SleepCurrentThread's SleepEx call
constexpr uintptr_t kThumbnailWaitReturn = 0x005DE9C2;       // its call in the thumbnail wait
// [choice] The main thread's heap from the city load on: one region at a fixed address,
// so the addresses the game orders and hashes objects by (the class store, the automata
// controller map, every pointer-keyed set) repeat. The first base that can be reserved; only
// the first repeats the hash orders (a later one is a warning). It lies below this DLL's fixed
// base (CMakeLists.txt, /BASE:0x5C000000), as a randomised DLL base moved it run to run.
constexpr uintptr_t kDllBase = 0x5C000000;
constexpr uintptr_t kArenaBases[] = {0x20000000, 0x28000000, 0x30000000, 0x38000000, 0x40000000, 0x48000000};
constexpr size_t kArenaSizes[] = {0x30000000, 0x20000000, 0x18000000};
constexpr size_t kArenaCommit = 0x100000;    // committed in 1 MB steps
constexpr size_t kSmallLimit = 4096;         // blocks up to this (with the header) in 16-byte classes
constexpr size_t kChunkAlign = 0x10000;      // VirtualAlloc's granularity, for the 512 KB chunks
constexpr uint32_t kArenaMagic = 0xFC4A110C;

struct Generator {
    uintptr_t address;
    uint32_t k;
    bool super;
};

// foundations.md §6.3 (all 34 checked against the static initialisers' "clock XOR K" in this executable).
const Generator kGenerators[] = {
    {0x00B485F0, 0x0C09C0B8, false}, {0x00B49080, 0xAC09C0E8, false}, {0x00B4AFD8, 0x4C09C113, false},
    {0x00B49AF0, 0x6C09C0F4, false}, {0x00B4B398, 0x8C09C68A, false}, {0x00B22FB0, 0x00000000, false},
    {0x00B23078, 0x8C09C2D5, false}, {0x00B230C8, 0x0C09C347, false}, {0x00B21648, 0xEC09C147, false},
    {0x00B22AE8, 0x6C09C1AB, false}, {0x00B22B60, 0xEC09C1D1, false}, {0x00B22B80, 0x4C09C187, false},
    {0x00B217C0, 0xAC357AF9, false}, {0x00B22BE8, 0xAC09C1EF, false}, {0x00B22C10, 0xAC09C20E, false},
    {0x00B22C38, 0xAC09C22B, false}, {0x00B22DF8, 0xEC09C261, false}, {0x00B22E58, 0x2C09C27B, false},
    {0x00B4BC78, 0x4C09C543, false}, {0x00B4D530, 0x8C09C57A, false}, {0x00B228A0, 0x6C09BFC3, false},
    {0x00B22E28, 0xCC09C24E, false}, {0x00B4B3D0, 0x6C09C5C8, false}, {0x00B4C7C8, 0xAC09C558, false},
    {0x00B4D5D0, 0xAC09C596, false},
    {0x00B49120, 0x0C09C461, true},  {0x00B47B78, 0x6C09C3D7, true},  {0x00B48630, 0x6C09C3F6, true},
    {0x00B49BD0, 0x4C09C4D3, true},  {0x00B4A5C0, 0x4C09C51F, true},  {0x00B44870, 0x8C09C379, true},
    {0x00B43E88, 0x6C09C07B, true},  {0x00B231B0, 0xAC09C2EC, true},  {0x00B21D78, 0x4C09C170, true},
};

// ---- state (hooks run on the game's threads; the counters are diagnostics) ----------------------
bool g_wanted = false;
bool g_fixed = false;
uint32_t g_seed = 0;
uint8_t* g_clockTrampoline = nullptr;
using QpcFn = BOOL(WINAPI*)(LARGE_INTEGER*);
QpcFn g_realQpc = nullptr;
using SleepFn = void(WINAPI*)(DWORD);
using SleepExFn = DWORD(WINAPI*)(DWORD, BOOL);
SleepFn g_realSleep = nullptr;
SleepExFn g_realSleepEx = nullptr;
int64_t g_qpcPerMs = 0;
DWORD g_mainThread = 0;
int64_t g_qpc = 0;          // the main thread's virtual counter
bool g_virtual = false;     // from the city load on (the start-up screens keep real time)
int64_t g_frameTicks = 0;   // kFrameMs in counter ticks
using ClockFn = int32_t(__cdecl*)();
using SrandFn = void(__cdecl*)(unsigned);
volatile LONG g_starts = 0, g_reads = 0, g_unstartedReads = 0, g_growExpiries = 0, g_otherExpiries = 0, g_timeCalls = 0,
              g_clockVirtual = 0, g_clockReal = 0, g_refs = 0, g_saves = 0, g_srands = 0, g_frames = 0, g_qpcVirtual = 0,
              g_qpcReal = 0, g_sleeps = 0, g_sleptMs = 0;
volatile LONG g_otherLogged = 0;
int32_t g_day = 0;
uint32_t g_dayDelivery = 0;

uint32_t Derive(uint32_t k) {
    uint32_t v = g_seed ^ k;
    if (v == 0 || v == 0xFFFFFFFFu) v ^= 1u;  // foundations §6.3
    return v;
}

bool Supported() {
    const auto base = reinterpret_cast<uintptr_t>(GetModuleHandleW(nullptr));
    if (base != 0x400000) return false;
    const auto* dos = reinterpret_cast<const IMAGE_DOS_HEADER*>(base);
    if (dos->e_magic != IMAGE_DOS_SIGNATURE || dos->e_lfanew <= 0 || dos->e_lfanew > 0x1000) return false;
    const auto* nt = reinterpret_cast<const IMAGE_NT_HEADERS32*>(base + dos->e_lfanew);
    return nt->Signature == IMAGE_NT_SIGNATURE && nt->FileHeader.TimeDateStamp == 0x4C12BFB7 &&
           nt->OptionalHeader.SizeOfImage == 0x7C8000;
}

// Other threads are suspended while an entry is rewritten, and none may be
// inside the bytes being replaced (as FC4Inplace.cpp does).
class PausedThreads {
public:
    PausedThreads(uintptr_t entry, unsigned bytes) {
        const HANDLE snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0);
        if (snapshot == INVALID_HANDLE_VALUE) {
            ok_ = false;
            return;
        }
        THREADENTRY32 t{};
        t.dwSize = sizeof(t);
        for (BOOL more = Thread32First(snapshot, &t); more; more = Thread32Next(snapshot, &t)) {
            if (t.th32OwnerProcessID != GetCurrentProcessId() || t.th32ThreadID == GetCurrentThreadId()) continue;
            if (count_ == 256) {
                ok_ = false;
                break;
            }
            const HANDLE h = OpenThread(THREAD_SUSPEND_RESUME | THREAD_GET_CONTEXT, FALSE, t.th32ThreadID);
            if (h == nullptr) continue;  // a thread that has just exited
            if (SuspendThread(h) == static_cast<DWORD>(-1)) {
                CloseHandle(h);
                continue;
            }
            handles_[count_++] = h;
            CONTEXT c{};
            c.ContextFlags = CONTEXT_CONTROL;
            if (!GetThreadContext(h, &c) || (c.Eip >= entry && c.Eip < entry + bytes)) {
                ok_ = false;
                break;
            }
        }
        CloseHandle(snapshot);
    }
    ~PausedThreads() {
        for (unsigned i = count_; i > 0; --i) {
            ResumeThread(handles_[i - 1]);
            CloseHandle(handles_[i - 1]);
        }
    }
    bool ok() const { return ok_; }

private:
    HANDLE handles_[256] = {};
    unsigned count_ = 0;
    bool ok_ = true;
};

// A jump at `entry` to `target`, if the entry holds `expected`.
const char* PatchJump(uintptr_t entry, const uint8_t* expected, size_t n, const void* target, uint8_t opcode = 0xE9) {
    auto* p = reinterpret_cast<uint8_t*>(entry);
    if (std::memcmp(p, expected, n) != 0) return "unexpected bytes";
    uint8_t jump[5] = {opcode, 0, 0, 0, 0};
    const int32_t rel = static_cast<int32_t>(reinterpret_cast<uintptr_t>(target) - (entry + 5));
    std::memcpy(jump + 1, &rel, 4);
    PausedThreads paused(entry, n);
    if (!paused.ok()) return "a thread is inside the entry";
    DWORD before = 0;
    if (!VirtualProtect(p, n, PAGE_EXECUTE_READWRITE, &before)) return "VirtualProtect failed";
    std::memcpy(p, jump, 5);
    if (n > 5) std::memset(p + 5, 0x90, n - 5);
    DWORD ignored = 0;
    VirtualProtect(p, n, before, &ignored);
    FlushInstructionCache(GetCurrentProcess(), p, n);
    return nullptr;
}

// ---- time limits --------------------------------------------------------------------------------
// LimitTimer is one u64; under the schedule it holds the reads since Start and kStarted.
void __fastcall LimitStart(uint32_t* self, void*, int32_t) {
    self[0] = 0;
    self[1] = kStarted;
    InterlockedIncrement(&g_starts);
}

uint32_t __fastcall LimitExpired(uint32_t* self) {
    if (self[1] != kStarted) {
        InterlockedIncrement(&g_unstartedReads);
        return 0;
    }
    const uint32_t reads = ++self[0];
    InterlockedIncrement(&g_reads);
    const uintptr_t site = OriginalReturn(reinterpret_cast<uintptr_t>(_ReturnAddress()));
    if (site == kGrowExpiredReturn) {
        if (reads < kGrowReads) return 0;
        InterlockedIncrement(&g_growExpiries);
        return 1;
    }
    if (reads < g_otherReads) return 0;
    InterlockedIncrement(&g_otherExpiries);
    if (InterlockedIncrement(&g_otherLogged) <= 8) {
        fc4::Log("schedule: WARNING a time limit called from 0x%08X reached %u reads (a spin, or work of that "
                 "size); it expires",
                 static_cast<unsigned>(site), reads);
    }
    return 1;
}

uint32_t __fastcall RemainingMs(uint32_t*) { return kRemaining; }

// ---- timer callbacks -----------------------------------------------------------------------------
// [choice] From the city load on, the timer schedulers' callbacks (cRZCallbackTimer: the animation tick
// manager's auto-step and the effects tick it posts, tooltips, the news ticker, ...; timers.md §2.5) fire
// on the main thread at the start of each simulator frame, in each scheduler's list order, by a clock of
// kFrameMs a frame that starts at 0 with every entry due one period later. The schedulers' worker
// thread reads a clock of 0, so nothing is ever due there: no callback posts a message at a moment
// the machine chooses.
constexpr uintptr_t kTimerElapsed = 0x00890198;                  // cRZTimer::GetElapsedTime (thiscall)
constexpr uintptr_t kAddTimerNow[] = {0x0093384F, 0x0093388F};   // AddTimer's clock reads (call rel32)
constexpr uintptr_t kWorkerNow[] = {0x00933900, 0x009339AA};     // the worker loop's
constexpr uintptr_t kDefaultScheduler = 0x00B62AB0;
constexpr uint32_t kSchedulerService = 0xA76C78E7, kSchedulerServiceIid = 0x276C784B;
// A scheduler: +0x10/+0x14 entries {ID, fn, arg, one-shot (u8), period, due, fixed-rate (u8)}, +0x44 its
// lock (cRZCriticalSection: slot 1 Lock, slot 2 Unlock).
constexpr size_t kEntryWords = 7;
uint8_t* g_schedulers[2] = {};
volatile LONG g_timerOn = 0;
volatile uint32_t g_timerMs = 0;
long g_timerFires = 0;

uint32_t __fastcall SchedulerNow(void* timer, void*) {
    if (!g_timerOn) return reinterpret_cast<uint32_t(__fastcall*)(void*, void*)>(kTimerElapsed)(timer, nullptr);
    const uintptr_t site = OriginalReturn(reinterpret_cast<uintptr_t>(_ReturnAddress())) - 5;
    return site == kWorkerNow[0] || site == kWorkerNow[1] ? 0 : g_timerMs;
}

// The call at `site` to `expected` now calls `target`.
const char* PatchCall(uintptr_t site, uintptr_t expected, const void* target) {
    auto* p = reinterpret_cast<uint8_t*>(site);
    int32_t rel = 0;
    std::memcpy(&rel, p + 1, 4);
    if (p[0] != 0xE8 || site + 5 + rel != expected) return "unexpected bytes";
    rel = static_cast<int32_t>(reinterpret_cast<uintptr_t>(target) - (site + 5));
    PausedThreads paused(site, 5);
    if (!paused.ok()) return "a thread is inside the call";
    DWORD before = 0;
    if (!VirtualProtect(p, 5, PAGE_EXECUTE_READWRITE, &before)) return "VirtualProtect failed";
    std::memcpy(p + 1, &rel, 4);
    DWORD ignored = 0;
    VirtualProtect(p, 5, before, &ignored);
    FlushInstructionCache(GetCurrentProcess(), p, 5);
    return nullptr;
}

void SchedulerLock(uint8_t* scheduler, bool lock) {
    void* section = scheduler + 0x44;
    const auto* vt = *static_cast<const uintptr_t* const*>(section);
    reinterpret_cast<void(__fastcall*)(void*, void*)>(vt[lock ? 1 : 2])(section, nullptr);
}

uint32_t*& EntriesBegin(uint8_t* scheduler) { return *reinterpret_cast<uint32_t**>(scheduler + 0x10); }
uint32_t*& EntriesEnd(uint8_t* scheduler) { return *reinterpret_cast<uint32_t**>(scheduler + 0x14); }

// At the city load: the framework's scheduler and the default one, every entry due one period from 0.
bool TimersStart() {
    cIGZFrameWork* fw = RZGetFrameWork();
    void* service = nullptr;
    if (fw == nullptr || !fw->GetSystemService(kSchedulerService, kSchedulerServiceIid, &service) || service == nullptr) {
        return false;
    }
    const auto* vt = *static_cast<const uintptr_t* const*>(service);
    g_schedulers[0] = reinterpret_cast<uint8_t*(__fastcall*)(void*, void*)>(vt[3])(service, nullptr);
    reinterpret_cast<uint32_t(__fastcall*)(void*, void*)>(vt[2])(service, nullptr);  // Release
    if (g_schedulers[0] == nullptr) return false;
    if (g_schedulers[0] != reinterpret_cast<uint8_t*>(kDefaultScheduler)) {
        g_schedulers[1] = reinterpret_cast<uint8_t*>(kDefaultScheduler);
    }
    for (uint8_t* sch : g_schedulers) {
        if (sch != nullptr) SchedulerLock(sch, true);
    }
    g_timerMs = 0;
    for (uint8_t* sch : g_schedulers) {
        if (sch == nullptr) continue;
        for (uint32_t* e = EntriesBegin(sch); e != EntriesEnd(sch); e += kEntryWords) e[5] = e[4];
    }
    InterlockedExchange(&g_timerOn, 1);
    for (int i = 1; i >= 0; --i) {
        if (g_schedulers[i] != nullptr) SchedulerLock(g_schedulers[i], false);
    }
    return true;
}

// One frame: the worker loop's step (timers.md §2.5, steps 1-2) on the main thread by the frame clock.
void TimersFrame() {
    if (!g_timerOn) return;
    g_timerMs += kFrameMs;
    const uint32_t now = g_timerMs;
    for (uint8_t* sch : g_schedulers) {
        if (sch == nullptr) continue;
        struct Fire {
            uint32_t fn, arg;
        } fire[64];
        unsigned n = 0;
        SchedulerLock(sch, true);
        for (uint32_t* e = EntriesBegin(sch); e != EntriesEnd(sch);) {
            if (now < e[5]) {
                e += kEntryWords;
                continue;
            }
            if (n < 64) fire[n++] = {e[1], e[2]};
            if ((e[3] & 0xFF) != 0) {  // one-shot: erased
                uint32_t*& end = EntriesEnd(sch);
                std::memmove(e, e + kEntryWords, (end - (e + kEntryWords)) * sizeof(uint32_t));
                end -= kEntryWords;
                continue;
            }
            e[5] = (e[6] & 0xFF) != 0 ? e[5] + e[4] : now + e[4];
            if (e[5] < now) e[5] = 0xFFFFFFFFu;
            e += kEntryWords;
        }
        SchedulerLock(sch, false);
        for (unsigned i = 0; i < n; ++i) reinterpret_cast<void(__cdecl*)(uint32_t)>(fire[i].fn)(fire[i].arg);
        g_timerFires += static_cast<long>(n);
    }
}

// ---- stopwatches started before the load ---------------------------------------------------------
// [choice] A cRZTimer (timers.md §1) still running from the start-up screens reads, on the main thread,
// as started at the city load with nothing accumulated: its first read or stop there moves its start
// stamp (a real counter value, below the virtual counter's start) to the virtual counter's start.
// Otherwise the app's idle, autosave and cache timers and the animation tick's first step carry the
// start-up's real length into the run.
constexpr uintptr_t kStopwatchReads[] = {0x00890198, 0x00890279, 0x00890377, 0x008905F0};  // GetElapsedTime x3, Stop
const uint8_t kElapsedBytes[] = {0x55, 0x8B, 0xEC, 0x83, 0xEC, 0x14};
const uint8_t kStopBytes[] = {0x55, 0x8B, 0xEC, 0x51, 0x51};
int64_t g_loadQpc = 0;  // the virtual counter's start
uint8_t* g_stopwatchReal[4] = {};
long g_stopwatchesRebased = 0;

void __fastcall StopwatchRebase(uint8_t* timer) {
    if (!g_virtual || GetCurrentThreadId() != g_mainThread || *reinterpret_cast<int32_t*>(timer) == 0) return;
    auto* start = reinterpret_cast<int64_t*>(timer + 0x08);
    if (*start == 0 || *start >= g_loadQpc) return;
    *start = g_loadQpc;
    *reinterpret_cast<int64_t*>(timer + 0x10) = 0;
    ++g_stopwatchesRebased;
}

// The hooked entries (thiscall): rebase `this`, then the original.
#define FC4_STOPWATCH_THUNK(i)                     \
    __declspec(naked) void StopwatchThunk##i() {   \
        __asm push ecx                             \
        __asm call StopwatchRebase                 \
        __asm pop ecx                              \
        __asm jmp dword ptr [g_stopwatchReal + 4 * i] \
    }
FC4_STOPWATCH_THUNK(0)
FC4_STOPWATCH_THUNK(1)
FC4_STOPWATCH_THUNK(2)
FC4_STOPWATCH_THUNK(3)
#undef FC4_STOPWATCH_THUNK
void (*const kStopwatchThunks[])() = {StopwatchThunk0, StopwatchThunk1, StopwatchThunk2, StopwatchThunk3};

// ---- uninitialised values ------------------------------------------------------------------------
// [choice] A transit switch inserted for an unknown ID is all zero (traffic-network.md §4.7.3), as in
// our game: the original copies a stack temporary whose fields are whatever the stack held (pointers
// into memory outside the fixed heap among them), into its pathing costs and the save.
constexpr uintptr_t kSwitchInsert = 0x006D9131;  // the miss path of the switch table's insert-if-missing (0x006D90F0)
const uint8_t kSwitchInsertBytes[] = {0x89, 0x4C, 0x24, 0x44, 0x8D, 0x4C, 0x24, 0x10};  // mov [esp+44h],ecx; lea ecx,[esp+10h]
long g_switchesZeroed = 0;

__declspec(naked) void SwitchInsertThunk() {
    __asm {
        push eax
        push ecx
        push edi
        lea edi, [esp + 0x1C]  // the 0x34-byte temporary at [esp+10h] before the three pushes
        mov ecx, 0x0D
        xor eax, eax
        rep stosd
        pop edi
        pop ecx
        pop eax
        lock inc g_switchesZeroed
        mov dword ptr [esp + 0x44], ecx
        lea ecx, [esp + 0x10]
        push 0x006D9139  // kSwitchInsertResume
        ret
    }
}

// [choice] A new intersection's four sides and flag start at zero: the original's add-intersection
// (0x004B4300) sets only the temporary's vector and list before copying it into the network map's
// intersection table, and the sides that stay unused carry stack contents into the save.
constexpr uintptr_t kIntersectionInsert = 0x004B434D;  // push 0Ch; mov dword ptr [esp+8Ch], 0
const uint8_t kIntersectionInsertBytes[] = {0x6A, 0x0C, 0xC7, 0x84, 0x24, 0x8C, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00};
long g_intersectionsZeroed = 0;

__declspec(naked) void IntersectionInsertThunk() {
    __asm {
        push eax
        push ecx
        push edi
        lea edi, [esp + 0x40]  // the temporary at [esp+34h] before the three pushes: sides and flag, 54h bytes
        mov ecx, 0x15
        xor eax, eax
        rep stosd
        pop edi
        pop ecx
        pop eax
        lock inc g_intersectionsZeroed
        push 0x0C
        mov dword ptr [esp + 0x8C], 0
        push 0x004B435A  // after the two replaced instructions
        ret
    }
}

// [choice] A new train junction's four junction indices start at zero: the original's add-junction (0x004B44F0)
// sets a side's u16 index only when that side has a path, and the indices of the sides without one carry stack
// contents into the network map's last table and its save (0x6A0F82B2, traffic-network.md §6.2.4). The game
// never reads them (an empty side has no path to pick).
constexpr uintptr_t kJunctionInsert = 0x004B459B;  // mov [esp+5Ch],ebx (ebx = 0); xor bl,bl
const uint8_t kJunctionInsertBytes[] = {0x89, 0x5C, 0x24, 0x5C, 0x32, 0xDB};
long g_junctionsZeroed = 0;

__declspec(naked) void JunctionInsertThunk() {
    __asm {
        mov dword ptr [esp + 0x5C], ebx
        mov dword ptr [esp + 0x30], ebx  // the temporary's four sides (16 bytes each from [esp+24h]): index words
        mov dword ptr [esp + 0x40], ebx
        mov dword ptr [esp + 0x50], ebx
        mov dword ptr [esp + 0x60], ebx
        lock inc g_junctionsZeroed
        xor bl, bl
        push 0x004B45A1  // after the two replaced instructions
        ret
    }
}

// ---- wall clock ---------------------------------------------------------------------------------
uint32_t __cdecl CrtTime(uint32_t* out) {
    InterlockedIncrement(&g_timeCalls);
    if (out != nullptr) *out = kTime;
    return kTime;
}

int32_t __cdecl CrtClock() {
    const uintptr_t site = OriginalReturn(reinterpret_cast<uintptr_t>(_ReturnAddress()));
    if (site >= kRandomFirst && site < kRandomEnd) {
        InterlockedIncrement(&g_clockVirtual);
        return kClock;
    }
    // [choice] Window fades (cGZWin) and the other users keep the real clock.
    InterlockedIncrement(&g_clockReal);
    return reinterpret_cast<ClockFn>(g_clockTrampoline)();
}

BOOL WINAPI VirtualQpc(LARGE_INTEGER* out) {
    if (!g_virtual || GetCurrentThreadId() != g_mainThread) {
        InterlockedIncrement(&g_qpcReal);
        return g_realQpc(out);
    }
    InterlockedIncrement(&g_qpcVirtual);
    out->QuadPart = ++g_qpc;
    return TRUE;
}

bool MainSleep(DWORD ms) {
    if (!g_virtual || GetCurrentThreadId() != g_mainThread || ms == INFINITE) return false;
    InterlockedIncrement(&g_sleeps);
    InterlockedExchangeAdd(&g_sleptMs, static_cast<LONG>(ms));
    g_qpc += static_cast<int64_t>(ms) * g_qpcPerMs;
    return true;
}

void WINAPI VirtualSleep(DWORD ms) {
    MainSleep(ms);
    g_realSleep(ms);
}

DWORD WINAPI VirtualSleepEx(DWORD ms, BOOL alertable) {
    // [ret][ms][alertable][SleepCurrentThread's return]: the thumbnail wait sleeps 1 ms.
    const auto* frame = static_cast<const uintptr_t*>(_AddressOfReturnAddress());
    const bool thumbnail = OriginalReturn(frame[0]) == kSleepCurrentThreadReturn && OriginalReturn(frame[3]) == kThumbnailWaitReturn;
    return g_realSleepEx(MainSleep(ms) && thumbnail && ms > kRealSleepMs ? kRealSleepMs : ms, alertable);
}

// ---- temporary files ----------------------------------------------------------------------------
// [original only] cRZFile::MakeTempPathName (0x0091A497) names a file from the temp directory and time(),
// counting down while the name exists. With time() fixed, games running side by side race for the same names
// in one directory, and the retries reach the main thread's heap. Each process has a directory of its own,
// emptied at start-up (a process ID can be reused).
wchar_t g_tempW[MAX_PATH] = {};
char g_tempA[MAX_PATH] = {};
void* g_realTempPathA = nullptr;

DWORD WINAPI TempPathA(DWORD n, LPSTR out) {
    const DWORD len = static_cast<DWORD>(std::strlen(g_tempA));
    if (out == nullptr || n <= len) return len + 1;
    std::memcpy(out, g_tempA, len + 1);
    return len;
}

DWORD WINAPI TempPathW(DWORD n, LPWSTR out) {
    const DWORD len = static_cast<DWORD>(wcslen(g_tempW));
    if (out == nullptr || n <= len) return len + 1;
    std::memcpy(out, g_tempW, (len + 1) * sizeof(wchar_t));
    return len;
}

const char* TempStart() {
    wchar_t base[MAX_PATH];
    const DWORD n = GetTempPathW(MAX_PATH, base);
    if (n == 0 || n > MAX_PATH - 16) return "no temp directory";
    swprintf(g_tempW, MAX_PATH, L"%sFC4-%08X\\", base, static_cast<unsigned>(GetCurrentProcessId()));
    if (!CreateDirectoryW(g_tempW, nullptr) && GetLastError() != ERROR_ALREADY_EXISTS) return "cannot create its directory";
    WIN32_FIND_DATAW found{};
    const HANDLE h = FindFirstFileW((std::wstring(g_tempW) + L"*").c_str(), &found);
    if (h != INVALID_HANDLE_VALUE) {
        do {
            if ((found.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY) == 0) DeleteFileW((std::wstring(g_tempW) + found.cFileName).c_str());
        } while (FindNextFileW(h, &found));
        FindClose(h);
    }
    if (WideCharToMultiByte(CP_ACP, 0, g_tempW, -1, g_tempA, MAX_PATH, nullptr, nullptr) == 0) return "path not convertible";
    void** wide = reinterpret_cast<void**>(kTempPathW);
    if (*wide == nullptr) return "cRZFile's GetTempPathW is not resolved yet";
    *wide = reinterpret_cast<void*>(&TempPathW);
    return nullptr;
}

// ---- the window -----------------------------------------------------------------------------------
// [original only] A save renders the region view's thumbnail only while the game is the foreground window
// (0x005DDEC0), which with games side by side is chance; the rendering reaches the main thread's heap. The game
// is never in the foreground: every save writes the default thumbnail, after its wait (virtual sleeps).
long g_backgrounds = 0;

bool __cdecl NeverForeground() {
    InterlockedIncrement(&g_backgrounds);
    return false;
}

// [original only] A shown window is painted when other windows uncover it (games side by side minimizing at
// real-time moments), and the paint creates the screen's draw context on the main thread's heap. The game's
// windows are minimized at the city load, before that heap starts; one shown again (the game restores itself at the
// end of a load) is minimized again at the next frame and counted.
HWND g_windows[8] = {};
long g_windowCount = 0, g_minimized = 0, g_reminimized = 0;

BOOL CALLBACK MinimizeOwn(HWND h, LPARAM) {
    DWORD pid = 0;
    GetWindowThreadProcessId(h, &pid);
    if (pid == GetCurrentProcessId() && IsWindowVisible(h) && g_windowCount < 8) g_windows[g_windowCount++] = h;
    return TRUE;
}

void MinimizeWindows() {
    if (g_windowCount == 0) EnumWindows(&MinimizeOwn, 0);
    for (long k = 0; k < g_windowCount; ++k) {
        const HWND h = g_windows[k];
        if (IsWindow(h) && IsWindowVisible(h) && !IsIconic(h)) {
            ShowWindow(h, SW_MINIMIZE);
            ++(g_frames == 0 ? g_minimized : g_reminimized);
        }
    }
}

// ---- generators ---------------------------------------------------------------------------------
// The calling thread's rand() state (foundations §12.1); our cores draw from it too (fc4_crt_set_host).
uint32_t* CrtState() {
    using GetPtdFn = uint8_t*(__cdecl*)();
    return reinterpret_cast<uint32_t*>(reinterpret_cast<GetPtdFn>(kCrtGetPtd)() + 0x14);
}

void ReseedGenerators() {
    using CtorFn = void*(__fastcall*)(void* self, void*, uint32_t seed);
    for (const Generator& g : kGenerators) {
        const auto ctor = reinterpret_cast<CtorFn>(g.super ? kRandomSuperCtor : kRandomCtor);
        ctor(reinterpret_cast<void*>(g.address), nullptr, Derive(g.k));
    }
}

// The generators the original seeds from the clock (cRZRandom::Seed or cRZRandomSuper::Seed of 0xFFFFFFFF, reading
// time() and clock()) get derive(K) of foundations §6.3, as our cores seed theirs (engine/core/src/seeding.rs).
// The owner is the first return site in the table among the 16 dwords above Seed's return address (a generator's
// constructor calls Seed; the protection simulators' common constructor is called by the fire and police ones).
enum SeedKind : uint8_t {
    kClockSeed,    // K, for a seed of 0xFFFFFFFF
    kCountedSeed,  // K XOR n, n the earlier seeds at this site since the load, for a seed of 0xFFFFFFFF
    kAnySeed,      // K, whatever the seed (the traffic simulator's Init seeds from a cRZTime)
    kTimes28Seed,  // derive(K) x 28, whatever the seed (the prop developer: cRZTime x 28)
};

struct SeedSite {
    uintptr_t site;
    uint32_t k;
    SeedKind kind;
};

const SeedSite kSeedSites[] = {
    {0x0041AA02, 0x4A0DD616, kClockSeed},    // advisor system
    {0x00429A68, 0x4A1DBBBF, kCountedSeed},  // each MySim (n = its creation index)
    {0x0045DA58, 0xEAD4BA36, kCountedSeed},  // city detail manager, increase pass
    {0x0052A02F, 0x0BB14381, kClockSeed},    // city situation manager
    {0x0057ADD9, 0x69C1718F, kClockSeed},    // disaster fire instance
    {0x0057D072, 0x4A132FDF, kCountedSeed},  // pipe burst, AttemptDisasterStart
    {0x0068D2B8, 0x0990BDD8, kClockSeed},    // aura simulator
    {0x0069E02E, 0x0A413481, kCountedSeed},  // civic building simulator, monthly
    {0x006ADA65, 0x2990C05A, kClockSeed},    // fire protection simulator
    {0x006E1F95, 0x6990C04F, kClockSeed},    // police simulator
    {0x006B193A, 0xC990C034, kClockSeed},    // flora simulator
    {0x006D0628, 0xC990C0A4, kClockSeed},    // neighbours' deal propositions (process-wide)
    {0x006D585A, 0xAA3FE6BC, kClockSeed},    // park manager
    {0x006DF11B, 0x0990C075, kClockSeed},    // plumbing simulator (Init)
    {0x006F34B6, 0x89C48F47, kTimes28Seed},  // prop developer
    {0x007005D6, 0x4990C013, kClockSeed},    // residential simulator
    {0x0071F1A5, 0x6990C1AA, kAnySeed},      // traffic simulator (Init)
    {0x00728E58, 0x2990C1BC, kClockSeed},    // weather simulator
};
constexpr size_t kSeedSiteCount = sizeof(kSeedSites) / sizeof(kSeedSites[0]);
uint32_t g_seedCounts[kSeedSiteCount] = {};  // seeds at each site since the load
long g_seedsDerived = 0, g_seedsClock = 0;
uintptr_t g_clockSeedSites[16] = {};          // the sites of clock seeds not in the table, logged once each

uint32_t __cdecl SeedFor(const uint32_t* stack) {
    const uint32_t seed = stack[1];
    for (int i = 0; i < 16; ++i) {
        for (size_t j = 0; j < kSeedSiteCount; ++j) {
            const SeedSite& e = kSeedSites[j];
            if (OriginalReturn(stack[i]) != e.site) continue;
            if (seed != 0xFFFFFFFFu && e.kind != kAnySeed && e.kind != kTimes28Seed) return seed;
            InterlockedIncrement(&g_seedsDerived);
            const uint32_t n = g_seedCounts[j]++;
            if (e.kind == kCountedSeed) return Derive(e.k ^ n);
            if (e.kind == kTimes28Seed) return Derive(e.k) * 28u;
            return Derive(e.k);
        }
    }
    if (seed == 0xFFFFFFFFu) {
        InterlockedIncrement(&g_seedsClock);
        uintptr_t site = 0;  // the first return site outside the generators' code
        for (int i = 0; i < 16 && site == 0; ++i) {
            if (stack[i] >= 0x00401000 && stack[i] < 0x00A70000 && (stack[i] < kRandomFirst || stack[i] >= kRandomEnd)) site = stack[i];
        }
        for (uintptr_t& logged : g_clockSeedSites) {
            if (logged == site) break;
            if (logged == 0) {
                logged = site;
                fc4::Log("schedule: a clock seed at 0x%08X keeps time() and clock() (not in the seed table)", static_cast<unsigned>(site));
                break;
            }
        }
    }
    return seed;
}

constexpr uintptr_t kRandomSeed = 0x00913667;  // cRZRandom::Seed
const uint8_t kRandomSeedBytes[] = {0x56, 0x8B, 0xF1, 0x8B, 0x4C, 0x24, 0x08};  // push esi; mov esi, ecx; mov ecx, [esp+8]
constexpr uintptr_t kRandomSuperSeed = 0x00913A72;  // cRZRandomSuper::Seed
const uint8_t kRandomSuperSeedBytes[] = {0x8B, 0x44, 0x24, 0x04, 0x83, 0xF8, 0xFF};  // mov eax, [esp+4]; cmp eax, -1

__declspec(naked) void RandomSeedThunk() {
    __asm {
        push ecx
        lea eax, [esp + 4]  // Seed's return address, then its seed
        push eax
        call SeedFor
        add esp, 4
        pop ecx
        mov [esp + 4], eax
        push esi
        mov esi, ecx
        mov ecx, [esp + 8]
        push 0x0091366E  // after the three replaced instructions
        ret
    }
}

__declspec(naked) void RandomSuperSeedThunk() {
    __asm {
        push ecx
        lea eax, [esp + 4]
        push eax
        call SeedFor
        add esp, 4
        pop ecx
        mov [esp + 4], eax
        mov eax, [esp + 4]
        cmp eax, -1
        push 0x00913A79  // after the two replaced instructions (push and ret keep the flags)
        ret
    }
}

// cRZRandom::RandomDoubleUniform reseeds a zero multiplicative state from clock() (foundations §6.1). The flammability
// simulator's generator is seeded 0 and keeps it, so its first fire roll (flammability.md §6.8) reseeds: that reseed
// gets derive(GZCLSID) of foundations §6.3, as our core's. The owner is the first return site in the table among the
// 16 dwords above the clock() call's return address; other zero states keep clock().
constexpr uintptr_t kZeroStateClock = 0x0091373B;  // call clock() in cRZRandom::RandomDoubleUniform

const SeedSite kReseedSites[] = {
    {0x006AF19A, 0xE990C045, kClockSeed},  // flammability simulator, the fire roll's RandomDoubleRangeUniform
};

int32_t __cdecl ZeroStateClock() {
    const auto* stack = static_cast<const uintptr_t*>(_AddressOfReturnAddress());
    for (int i = 0; i < 16; ++i) {
        for (const SeedSite& e : kReseedSites) {
            if (OriginalReturn(stack[i]) != e.site) continue;
            InterlockedIncrement(&g_seedsDerived);
            return static_cast<int32_t>(Derive(e.k));
        }
    }
    InterlockedIncrement(&g_clockVirtual);  // the generators' clock(), as CrtClock gives their code
    return kClock;
}

// ---- object IDs ---------------------------------------------------------------------------------
struct SaveIds {
    std::unordered_map<uintptr_t, uint32_t> ids;
    uint32_t next = 1;
};
std::unordered_map<uintptr_t, SaveIds> g_saveIds;  // by serializer; main thread (saves)

uint32_t __fastcall WriteRef(uint8_t* iface, void*, void* stream, void* object) {
    using U32Fn = uint32_t(__fastcall*)(void*, void*);
    using SetU32Fn = uint8_t(__fastcall*)(void*, void*, uint32_t);
    using ErrorFn = uint8_t(__fastcall*)(void*, void*);
    using RegisterFn = uint8_t(__fastcall*)(void*, void*, void* object, uint32_t id, uint32_t clsid);
    // As the original (0x005D4810): refuse when the serializer is not writing.
    if (*reinterpret_cast<uint32_t*>(iface + 0x2C) == 0) return 0;
    uint8_t* base = iface - 8;
    if (*reinterpret_cast<uint32_t*>(base + 0x28) == 0) return 0;
    void** svt = *reinterpret_cast<void***>(stream);
    uint32_t clsid = 0, id = 0;
    if (object != nullptr) {
        void** ovt = *reinterpret_cast<void***>(object);
        clsid = reinterpret_cast<U32Fn>(ovt[5])(object, nullptr);  // GetGZCLSID
        // A serializer that has registered nothing yet starts a new numbering
        // (its +0x2C counts the objects it registered; a reused address is reset).
        SaveIds& s = g_saveIds[reinterpret_cast<uintptr_t>(base)];
        if (*reinterpret_cast<uint32_t*>(base + 0x2C) == 0 && s.next != 1) {
            s.ids.clear();
            s.next = 1;
        }
        if (s.next == 1 && s.ids.empty()) InterlockedIncrement(&g_saves);
        const auto it = s.ids.find(reinterpret_cast<uintptr_t>(object));
        if (it != s.ids.end()) {
            id = it->second;
        } else {
            id = s.next++;
            s.ids.emplace(reinterpret_cast<uintptr_t>(object), id);
        }
        InterlockedIncrement(&g_refs);
    }
    reinterpret_cast<SetU32Fn>(svt[9])(stream, nullptr, id);  // SetUint32
    if (object != nullptr) {
        reinterpret_cast<SetU32Fn>(svt[9])(stream, nullptr, clsid);
        if (!reinterpret_cast<RegisterFn>(kRegisterObject)(base, nullptr, object, id, clsid)) return 0;
    }
    if (reinterpret_cast<ErrorFn>(svt[18])(stream, nullptr)) return 0;  // GetError
    return 1;
}

// ---- heap ---------------------------------------------------------------------------------------
// From the city load on, every allocation the main thread makes through the C runtime,
// the framework allocator or its chunk back-ends comes from one region at a fixed base,
// with free lists that only the main thread touches; so the same sequence of requests
// gets the same addresses. A free block is reused lowest address first, so the order
// in which a batch of blocks is freed (a container walked in hash order) does not
// matter. Other threads keep the original heaps. A region block freed on another
// thread is never reused (its timing is not the main thread's).
struct BlockHeader {   // 16 bytes before each block (the chunks have none)
    uint32_t size;     // requested
    uint32_t span;     // header + block, rounded
    uint32_t magic;
    uint32_t stamp;    // the block's allocation number from the city load: its object's creation order (address order)
};

struct Arena {
    uint8_t* base = nullptr;
    size_t size = 0;
    size_t top = 0;
    size_t committed = 0;
    using Free = std::priority_queue<uint32_t, std::vector<uint32_t>, std::greater<uint32_t>>;  // offsets, lowest first
    Free small[kSmallLimit / 16];
    std::map<size_t, Free> large;    // by span
    Free chunks;                     // 512 KB, 64 KB aligned
    Free blocks[24];                 // the pools' pages, by shift
    std::map<uint8_t*, size_t> chunkSizes;
    uint32_t hash = 2166136261u;
    uint32_t ops = 0;
    uint32_t stamps = 0;             // blocks allocated
};
Arena g_arena;
bool g_heapReady = false;  // the region is reserved and every heap entry hooked
bool g_heap = false;       // from the city load on
volatile LONG g_heapLeaks = 0, g_heapFallbacks = 0, g_heapForeign = 0;

struct HeapTrampolines {
    uint8_t* malloc = nullptr;
    uint8_t* free = nullptr;
    uint8_t* realloc = nullptr;
    uint8_t* calloc = nullptr;
    uint8_t* msize = nullptr;
    uint8_t* fastAllocate = nullptr;
    uint8_t* processHeapAlloc = nullptr;
    uint8_t* processHeapFree = nullptr;
    uint8_t* chunkAlloc = nullptr;
    uint8_t* chunkFree = nullptr;
    uint8_t* blockAlloc = nullptr;
    uint8_t* blockFree = nullptr;
} g_heapReal;

using MallocFn = void*(__cdecl*)(size_t);
using FreeFn = void(__cdecl*)(void*);
using ReallocFn = void*(__cdecl*)(void*, size_t);
using CallocFn = void*(__cdecl*)(size_t, size_t);
using MsizeFn = size_t(__cdecl*)(void*);
using FastAllocateFn = void*(__fastcall*)(void*, void*, uint32_t);
using HeapFreeFn = BOOL(__cdecl*)(void*);
using ChunkAllocFn = void*(__stdcall*)(uint32_t);
using ChunkFreeFn = BOOL(__stdcall*)(uint32_t, void*);
using BlockAllocFn = void*(__cdecl*)(uint32_t);
using BlockFreeFn = void(__cdecl*)(void*, uint32_t);

bool InArena(const void* p) {
    const auto* b = static_cast<const uint8_t*>(p);
    return g_arena.base != nullptr && b >= g_arena.base && b < g_arena.base + g_arena.size;
}

bool OnArena() { return g_heap && GetCurrentThreadId() == g_mainThread; }

// Diagnostics: `heap trace = <directory>` writes every region operation of the main
// thread (op, size, offset, three callers) to heap-<pid>.bin, up to `heap trace max` operations
// (environment SC4_HEAP_TRACE / SC4_HEAP_TRACE_MAX: the golden harness drops FC4_*).
HANDLE g_traceFile = INVALID_HANDLE_VALUE;
uint32_t g_traceMax = 0;
std::vector<uint32_t> g_dumpAt;  // `heap dump at = N, M, ...` (ascending): the region's bytes at those operations
std::string g_traceDir;
constexpr uint32_t kTraceWords = 6;  // op, size, offset, three return addresses in the executable
uint32_t g_traceBuffer[kTraceWords * 16384];
uint32_t g_traceUsed = 0;
const uintptr_t* g_frame = nullptr;  // the hooked entry's return-address slot (main thread)

bool ReturnSite(uintptr_t v) {
    if (v < 0x00401005 || v >= 0x00A80000) return false;
    const auto* b = reinterpret_cast<const uint8_t*>(v);
    return b[-5] == 0xE8 || (b[-6] == 0xFF && ((b[-5] >> 3) & 7) == 2) || (b[-2] == 0xFF && ((b[-1] >> 3) & 7) == 2) ||
           (b[-3] == 0xFF && ((b[-2] >> 3) & 7) == 2) || (b[-7] == 0xFF && ((b[-6] >> 3) & 7) == 2);
}

void TraceFlush() {
    if (g_traceFile == INVALID_HANDLE_VALUE || g_traceUsed == 0) return;
    DWORD written = 0;
    WriteFile(g_traceFile, g_traceBuffer, g_traceUsed * 4, &written, nullptr);
    g_traceUsed = 0;
}

void ArenaDump() {
    const std::string path = g_traceDir + "/heap-" + std::to_string(GetCurrentProcessId()) + "-" +
                             std::to_string(g_arena.ops) + ".dmp";
    const HANDLE f = CreateFileA(path.c_str(), GENERIC_WRITE, FILE_SHARE_READ, nullptr, CREATE_ALWAYS, 0, nullptr);
    if (f == INVALID_HANDLE_VALUE) return;
    for (size_t at = 0; at < g_arena.top;) {
        const DWORD n = static_cast<DWORD>(g_arena.top - at < 0x1000000 ? g_arena.top - at : 0x1000000);
        DWORD written = 0;
        WriteFile(f, g_arena.base + at, n, &written, nullptr);
        at += n;
    }
    CloseHandle(f);
}

// Diagnostics: `heap watch = N:OFF[,OFF...]` (offsets in hex) arms the main thread's debug registers at operation N
// on up to four region words and logs each write to them: the operation count, the new value, the writer and its
// callers.
uint32_t g_watchAt = 0;
uint32_t g_watchOffsets[4] = {};
unsigned g_watchCount = 0, g_watchHits = 0;
constexpr DWORD kWatchArm = 0xE0FC4057;

LONG CALLBACK WatchHandler(EXCEPTION_POINTERS* e) {
    CONTEXT* c = e->ContextRecord;
    if (e->ExceptionRecord->ExceptionCode == kWatchArm) {
        DWORD* dr[4] = {&c->Dr0, &c->Dr1, &c->Dr2, &c->Dr3};
        DWORD dr7 = 0;
        for (unsigned i = 0; i < g_watchCount; ++i) {
            *dr[i] = static_cast<DWORD>(reinterpret_cast<uintptr_t>(g_arena.base + g_watchOffsets[i]));
            dr7 |= (1u << (2 * i)) | (1u << (16 + 4 * i)) | (3u << (18 + 4 * i));  // local; on write; 4 bytes
        }
        c->Dr7 = dr7;
        c->ContextFlags |= CONTEXT_DEBUG_REGISTERS;
        fc4::Log("schedule: watch armed at operation %u (%u words)", g_arena.ops, g_watchCount);
        return EXCEPTION_CONTINUE_EXECUTION;
    }
    if (e->ExceptionRecord->ExceptionCode != EXCEPTION_SINGLE_STEP || (c->Dr6 & 0xF) == 0) return EXCEPTION_CONTINUE_SEARCH;
    for (unsigned i = 0; i < g_watchCount; ++i) {
        if ((c->Dr6 & (1u << i)) == 0 || g_watchHits >= 20000) continue;
        ++g_watchHits;
        char callers[128] = "";
        int n = 0;
        const auto* sp = reinterpret_cast<const uintptr_t*>(c->Esp);
        for (int j = 0, k = 0; j < 64 && k < 8; ++j) {
            if (ReturnSite(sp[j])) {
                n += std::snprintf(callers + n, sizeof(callers) - n, " %08X", static_cast<unsigned>(sp[j]));
                ++k;
            }
        }
        fc4::Log("schedule: watch %u at operation %u: 0x%08X by 0x%08X;%s", i, g_arena.ops,
            *reinterpret_cast<const uint32_t*>(g_arena.base + g_watchOffsets[i]), static_cast<unsigned>(c->Eip), callers);
    }
    c->Dr6 = 0;
    c->ContextFlags |= CONTEXT_DEBUG_REGISTERS;
    return EXCEPTION_CONTINUE_EXECUTION;
}

void ArenaNote(uint32_t op, uint32_t a, uint32_t b) {
    if (g_watchAt != 0 && g_arena.ops == g_watchAt) RaiseException(kWatchArm, 0, 0, nullptr);
    if (!g_dumpAt.empty() && g_arena.ops == g_dumpAt.front()) {
        ArenaDump();
        g_dumpAt.erase(g_dumpAt.begin());
    }
    if (g_traceFile != INVALID_HANDLE_VALUE && g_arena.ops < g_traceMax) {
        uint32_t* r = g_traceBuffer + g_traceUsed;
        r[0] = op;
        r[1] = a;
        r[2] = b;
        r[3] = r[4] = r[5] = 0;
        for (int i = 0, k = 3; g_frame != nullptr && i < 48 && k < 6; ++i) {
            if (ReturnSite(g_frame[i])) r[k++] = static_cast<uint32_t>(g_frame[i]);
        }
        if ((g_traceUsed += kTraceWords) == sizeof(g_traceBuffer) / 4) TraceFlush();
    }
    if (op == 'F' || op == 'c' || op == 'd') {  // frees change no address by themselves (lowest first)
        ++g_arena.ops;
        return;
    }
    uint32_t h = g_arena.hash;
    for (const uint32_t v : {op, a, b}) {
        for (int i = 0; i < 4; ++i) h = (h ^ ((v >> (8 * i)) & 0xFF)) * 16777619u;
    }
    g_arena.hash = h;
    ++g_arena.ops;
}

// A heap trace row that is not a heap operation (a walk in creation order: 'W' {key count, 0, its caller}, then
// 'K' {stamp} per key, in the walk's order); it changes neither the operation count nor the digest.
void TraceNote(uint32_t op, uint32_t a, uint32_t site) {
    if (g_traceFile == INVALID_HANDLE_VALUE || g_arena.ops >= g_traceMax) return;
    uint32_t* r = g_traceBuffer + g_traceUsed;
    r[0] = op;
    r[1] = a;
    r[2] = 0;
    r[3] = site;
    r[4] = r[5] = 0;
    if ((g_traceUsed += kTraceWords) == sizeof(g_traceBuffer) / 4) TraceFlush();
}

uint8_t* ArenaBump(size_t span, size_t align) {
    const size_t at = (g_arena.top + align - 1) & ~(align - 1);
    if (at + span > g_arena.size) return nullptr;
    if (at + span > g_arena.committed) {
        const size_t want = (at + span - g_arena.committed + kArenaCommit - 1) & ~(kArenaCommit - 1);
        const size_t grow = want < g_arena.size - g_arena.committed ? want : g_arena.size - g_arena.committed;
        if (VirtualAlloc(g_arena.base + g_arena.committed, grow, MEM_COMMIT, PAGE_READWRITE) == nullptr) return nullptr;
        g_arena.committed += grow;
    }
    g_arena.top = at + span;
    return g_arena.base + at;
}

void* ArenaAlloc(size_t n) {
    if (n > g_arena.size) return nullptr;
    size_t span = (n + sizeof(BlockHeader) + 15) & ~size_t{15};
    uint8_t* block = nullptr;
    Arena::Free* reuse = nullptr;
    if (span <= kSmallLimit) {
        reuse = &g_arena.small[span / 16 - 1];
    } else {
        span = (span + 4095) & ~size_t{4095};
        const auto it = g_arena.large.find(span);
        if (it != g_arena.large.end()) reuse = &it->second;
    }
    if (reuse != nullptr && !reuse->empty()) {
        block = g_arena.base + reuse->top();
        reuse->pop();
        // [choice] Memory the original reads before writing reads 0 (simulator.md §5.8): a reused block is
        // cleared, as the region's fresh memory is.
        std::memset(block + sizeof(BlockHeader), 0, span - sizeof(BlockHeader));
    }
    if (block == nullptr && (block = ArenaBump(span, 16)) == nullptr) {
        InterlockedIncrement(&g_heapFallbacks);
        return nullptr;
    }
    auto* h = reinterpret_cast<BlockHeader*>(block);
    h->size = static_cast<uint32_t>(n);
    h->span = static_cast<uint32_t>(span);
    h->magic = kArenaMagic;
    h->stamp = ++g_arena.stamps;
    ArenaNote('A', static_cast<uint32_t>(n), static_cast<uint32_t>(block - g_arena.base));
    return block + sizeof(BlockHeader);
}

BlockHeader* ArenaHeader(void* p) {
    auto* h = reinterpret_cast<BlockHeader*>(static_cast<uint8_t*>(p) - sizeof(BlockHeader));
    return h->magic == kArenaMagic ? h : nullptr;
}

// Main thread only.
void ArenaFree(void* p) {
    BlockHeader* h = ArenaHeader(p);
    if (h == nullptr) {
        InterlockedIncrement(&g_heapForeign);  // not a block start (a chunk is freed by ChunkFree)
        return;
    }
    auto* block = reinterpret_cast<uint8_t*>(h);
    ArenaNote('F', h->size, static_cast<uint32_t>(block - g_arena.base));
    const auto offset = static_cast<uint32_t>(block - g_arena.base);
    (h->span <= kSmallLimit ? g_arena.small[h->span / 16 - 1] : g_arena.large[h->span]).push(offset);
}

void* __cdecl CrtMalloc(size_t n) {
    if (OnArena()) {
        g_frame = static_cast<const uintptr_t*>(_AddressOfReturnAddress());
        if (void* p = ArenaAlloc(n)) return p;
    }
    return reinterpret_cast<MallocFn>(g_heapReal.malloc)(n);
}

void __cdecl CrtFree(void* p) {
    if (p == nullptr) return;
    if (OnArena()) g_frame = static_cast<const uintptr_t*>(_AddressOfReturnAddress());
    if (!InArena(p)) {
        reinterpret_cast<FreeFn>(g_heapReal.free)(p);
    } else if (GetCurrentThreadId() == g_mainThread) {
        ArenaFree(p);
    } else {
        InterlockedIncrement(&g_heapLeaks);
    }
}

size_t __cdecl CrtMsize(void* p) {
    if (p != nullptr && InArena(p)) {
        const BlockHeader* h = ArenaHeader(p);
        return h != nullptr ? h->size : 0;
    }
    return reinterpret_cast<MsizeFn>(g_heapReal.msize)(p);
}

void* __cdecl CrtRealloc(void* p, size_t n) {
    if (p == nullptr) return CrtMalloc(n);
    if (n == 0) {  // the C runtime frees and returns null
        CrtFree(p);
        return nullptr;
    }
    const bool main = OnArena();
    if (main) g_frame = static_cast<const uintptr_t*>(_AddressOfReturnAddress());
    if (!InArena(p) && !main) return reinterpret_cast<ReallocFn>(g_heapReal.realloc)(p, n);
    const size_t old = CrtMsize(p);
    if (InArena(p) && main) {
        BlockHeader* h = ArenaHeader(p);
        if (h != nullptr && n + sizeof(BlockHeader) <= h->span && n + sizeof(BlockHeader) + 16 > h->span) {
            ArenaNote('R', static_cast<uint32_t>(n), static_cast<uint32_t>(reinterpret_cast<uint8_t*>(h) - g_arena.base));
            h->size = static_cast<uint32_t>(n);
            return p;
        }
    }
    void* q = main ? ArenaAlloc(n) : nullptr;
    if (q == nullptr) q = reinterpret_cast<MallocFn>(g_heapReal.malloc)(n);
    if (q == nullptr) return nullptr;
    std::memcpy(q, p, old < n ? old : n);
    CrtFree(p);
    return q;
}

void* __cdecl CrtCalloc(size_t count, size_t n) {
    if (OnArena() && (count == 0 || n <= 0xFFFFFFE0u / count)) {
        g_frame = static_cast<const uintptr_t*>(_AddressOfReturnAddress());
        if (void* p = ArenaAlloc(count * n)) {
            std::memset(p, 0, count * n);
            return p;
        }
    }
    return reinterpret_cast<CallocFn>(g_heapReal.calloc)(count, n);
}

// The framework allocator: a region block carries the header of the allocator's own
// large blocks, (n + 4) | 0x80000000, so its Deallocate and Reallocate send it to free().
void* __fastcall FastAllocate(void* self, void*, uint32_t n) {
    if (OnArena()) {
        g_frame = static_cast<const uintptr_t*>(_AddressOfReturnAddress());
        if (auto* q = static_cast<uint32_t*>(ArenaAlloc(n + 4))) {
            q[0] = (n + 4) | 0x80000000u;
            return q + 1;
        }
    }
    return reinterpret_cast<FastAllocateFn>(g_heapReal.fastAllocate)(self, nullptr, n);
}

void* __cdecl ProcessHeapAlloc(size_t n) {
    if (OnArena()) {
        g_frame = static_cast<const uintptr_t*>(_AddressOfReturnAddress());
        if (void* p = ArenaAlloc(n)) return p;
    }
    return reinterpret_cast<MallocFn>(g_heapReal.processHeapAlloc)(n);
}

BOOL __cdecl ProcessHeapFree(void* p) {
    if (p == nullptr || !InArena(p)) return reinterpret_cast<HeapFreeFn>(g_heapReal.processHeapFree)(p);
    CrtFree(p);
    return TRUE;
}

constexpr size_t kChunkSize = 0x80000;

void* __stdcall ChunkAlloc(uint32_t unused) {
    if (OnArena()) {
        g_frame = static_cast<const uintptr_t*>(_AddressOfReturnAddress());
        uint8_t* chunk = nullptr;
        if (!g_arena.chunks.empty()) {
            chunk = g_arena.base + g_arena.chunks.top();
            g_arena.chunks.pop();
            std::memset(chunk, 0, kChunkSize);  // as fresh VirtualAlloc memory
        } else {
            chunk = ArenaBump(kChunkSize, kChunkAlign);
        }
        if (chunk != nullptr) {
            g_arena.chunkSizes[chunk] = kChunkSize;
            ArenaNote('C', kChunkSize, static_cast<uint32_t>(chunk - g_arena.base));
            return chunk;
        }
        InterlockedIncrement(&g_heapFallbacks);
    }
    return reinterpret_cast<ChunkAllocFn>(g_heapReal.chunkAlloc)(unused);
}

BOOL __stdcall ChunkFree(uint32_t first, void* p) {
    if (p == nullptr || !InArena(p)) return reinterpret_cast<ChunkFreeFn>(g_heapReal.chunkFree)(first, p);
    if (GetCurrentThreadId() != g_mainThread) {
        InterlockedIncrement(&g_heapLeaks);
        return TRUE;
    }
    auto* chunk = static_cast<uint8_t*>(p);
    const auto it = g_arena.chunkSizes.find(chunk);
    if (it == g_arena.chunkSizes.end()) {
        InterlockedIncrement(&g_heapForeign);
        return TRUE;
    }
    ArenaNote('c', static_cast<uint32_t>(it->second), static_cast<uint32_t>(chunk - g_arena.base));
    g_arena.chunks.push(static_cast<uint32_t>(chunk - g_arena.base));
    g_arena.chunkSizes.erase(it);
    return TRUE;
}

// A pool object's creation stamp and size, by its region offset (PoolTaken; main thread).
struct PoolStamp {
    uint32_t stamp, stride;
};
std::map<uint32_t, PoolStamp> g_poolStamps;

// The object pools' pages (2^shift bytes; a pool finds an object's page by that alignment).
void* __cdecl BlockAlloc(uint32_t shift) {
    if (OnArena() && shift < 24) {
        g_frame = static_cast<const uintptr_t*>(_AddressOfReturnAddress());
        const size_t n = size_t{1} << shift;
        Arena::Free& reuse = g_arena.blocks[shift];
        uint8_t* block = nullptr;
        if (!reuse.empty()) {
            block = g_arena.base + reuse.top();
            reuse.pop();
            std::memset(block, 0, n);  // as fresh memory (ArenaAlloc)
        } else {
            block = ArenaBump(n, n);
        }
        if (block != nullptr) {
            ArenaNote('D', static_cast<uint32_t>(n), static_cast<uint32_t>(block - g_arena.base));
            return block;
        }
        InterlockedIncrement(&g_heapFallbacks);
    }
    return reinterpret_cast<BlockAllocFn>(g_heapReal.blockAlloc)(shift);
}

void __cdecl BlockFree(void* p, uint32_t shift) {
    if (p == nullptr || !InArena(p)) {
        reinterpret_cast<BlockFreeFn>(g_heapReal.blockFree)(p, shift);
    } else if (GetCurrentThreadId() != g_mainThread || shift >= 24) {
        InterlockedIncrement(&g_heapLeaks);
    } else {
        g_frame = static_cast<const uintptr_t*>(_AddressOfReturnAddress());
        const auto offset = static_cast<uint32_t>(static_cast<uint8_t*>(p) - g_arena.base);
        ArenaNote('d', 1u << shift, offset);
        g_poolStamps.erase(g_poolStamps.lower_bound(offset), g_poolStamps.lower_bound(offset + (1u << shift)));
        g_arena.blocks[shift].push(offset);
    }
}

// The static object pools (an instance {vtable, current page, full pages}; slot 2 makes a page;
// a page is {next, prev, free list, count}, count + 0x40000000 while full). At the city load
// every page made before it is retired (count + kRetired: never current again, never moved
// between the rings, never released), and each pool starts from one new page of the region,
// so the city's objects come from the region whatever the start-up screens left in the pools.
struct PoolSite {
    uintptr_t pool;
    uintptr_t vtable;  // 0: set by its constructor at run time (checked against the range only)
    uint32_t stride;   // its objects' size in a page (its page maker, vtable slot 2)
};
constexpr PoolSite kPools[] = {
    {0x00B07F1C, 0x00A87370, 0x5C}, {0x00B07FC8, 0x00A88644, 0x5C}, {0x00B09428, 0x00A8CE88, 0x68},
    {0x00B09588, 0x00A8FE7C, 0x58}, {0x00B098A8, 0x00A942D8, 0x4C}, {0x00B098B4, 0x00A942E8, 0x10},
    {0x00B098D0, 0x00A943E8, 0x60}, {0x00B09FEC, 0x00A99E24, 0x3C}, {0x00B0A018, 0x00A9A428, 0x270},
    {0x00B0A09C, 0x00A9C7F4, 0x2C8}, {0x00B0A0F8, 0x00A9D194, 0x2B8}, {0x00B0A808, 0x00AA20A0, 0x1A0},
    {0x00B0A824, 0x00AA2870, 0x278}, {0x00B0A830, 0x00AA2A08, 0x5C}, {0x00B0A86C, 0x00AA2CDC, 0x60},
    {0x00B0A884, 0x00AA2CEC, 0xF0}, {0x00B0A898, 0x00AA2E94, 0xB0}, {0x00B0A8AC, 0x00AA30B4, 0x6C},
    {0x00B0A8C8, 0x00AA4458, 0x30}, {0x00B0A8E4, 0x00AA4960, 0x68}, {0x00B0A8F0, 0x00AA4970, 0xA0},
    {0x00B0A8FC, 0x00AA4980, 0x94}, {0x00B0A908, 0x00AA4990, 0xD8}, {0x00B0A914, 0x00AA49A0, 0x88},
    {0x00B0A96C, 0x00AA5288, 0x590}, {0x00B0A9B8, 0x00AA54E0, 0xBC}, {0x00B0A9E8, 0x00AA6CB4, 0x1C},
    {0x00B0A9F4, 0x00AA6CC4, 0x38}, {0x00B0B2D0, 0x00AA7180, 0x1F0}, {0x00B0B394, 0x00AA7F00, 0x180},
    {0x00B0B3A8, 0x00AA8224, 0x1D8}, {0x00B0B570, 0x00AA9458, 0x1E0}, {0x00B0BEC0, 0x00AA9978, 0x280},
    {0x00B0C0C8, 0x00AA9D04, 0x124}, {0x00B0C258, 0x00AA9D14, 0x58}, {0x00B0CDB8, 0x00AAA590, 0x2D8},
    {0x00B0D008, 0x00AAABC0, 0x278}, {0x00B0D3AC, 0x00AB184C, 0x8C}, {0x00B0D484, 0x00AB3380, 0x18},
    {0x00B0DC30, 0x00000000, 0x34}, {0x00B0E330, 0x00ABD28C, 0xD0}, {0x00B0E33C, 0x00ABD29C, 0x2C},
    {0x00B0E348, 0x00ABD2AC, 0x174}, {0x00B0E368, 0x00ABD4D0, 0xA4}, {0x00B0E388, 0x00ABD5F8, 0x7C},
};
constexpr uint32_t kRetired = 0x20000000;
using PageMakeFn = uint32_t*(__fastcall*)(void*);
uint32_t g_poolsRetired = 0, g_pagesRetired = 0;

// Main thread, with the region on (the new pages come from BlockAlloc).
void PoolsRetire() {
    for (const PoolSite& site : kPools) {
        auto* pool = reinterpret_cast<uint32_t*>(site.pool);
        const uint32_t vtable = pool[0];
        if ((site.vtable != 0 && vtable != site.vtable) || vtable < 0x00A80000 || vtable >= 0x00B00000 || pool[1] == 0) {
            fc4::Log("schedule: WARNING object pool at 0x%08X not restarted (vtable 0x%08X)",
                     static_cast<unsigned>(site.pool), vtable);
            continue;
        }
        for (const uint32_t head : {pool[1], pool[2]}) {
            auto* page = reinterpret_cast<uint32_t*>(head);
            while (page != nullptr) {
                page[3] += kRetired;
                ++g_pagesRetired;
                page = reinterpret_cast<uint32_t*>(page[0]);
                if (page == reinterpret_cast<uint32_t*>(head)) break;
            }
        }
        uint32_t* page = reinterpret_cast<PageMakeFn>(reinterpret_cast<const uint32_t*>(vtable)[2])(pool);
        page[0] = page[1] = static_cast<uint32_t>(reinterpret_cast<uintptr_t>(page));
        pool[1] = static_cast<uint32_t>(reinterpret_cast<uintptr_t>(page));
        pool[2] = 0;
        ++g_poolsRetired;
    }
}

// [choice] A pool object's creation stamp (simulator.md §5.8). Pool pages have no block headers, so each allocation
// site of the pools (kPoolTakes) is hooked where it reads the pool's current page after the pool's slow path: the
// page's free-list head is then still the object being taken. The object's stamp continues the blocks' count.
struct PoolTake {
    uintptr_t at;
    uint8_t n;
    uint8_t bytes[6];  // mov reg, [pool + 4]
    uintptr_t pool;
};
const PoolTake kPoolTakes[] = {
    {0x00458A78, 6, {0x8B,0x15,0x20,0x7F,0xB0,0x00}, 0x00B07F1C},
    {0x00458F58, 6, {0x8B,0x15,0x20,0x7F,0xB0,0x00}, 0x00B07F1C},
    {0x00472978, 6, {0x8B,0x15,0xCC,0x7F,0xB0,0x00}, 0x00B07FC8},
    {0x00472C48, 6, {0x8B,0x15,0xCC,0x7F,0xB0,0x00}, 0x00B07FC8},
    {0x00477338, 6, {0x8B,0x15,0x2C,0x94,0xB0,0x00}, 0x00B09428},
    {0x004776C8, 6, {0x8B,0x15,0x2C,0x94,0xB0,0x00}, 0x00B09428},
    {0x004B478B, 6, {0x8B,0x0D,0x8C,0x95,0xB0,0x00}, 0x00B09588},
    {0x004B4EE4, 5, {0xA1,0x8C,0x95,0xB0,0x00}, 0x00B09588},
    {0x004F6D2B, 6, {0x8B,0x15,0xAC,0x98,0xB0,0x00}, 0x00B098A8},
    {0x004F747A, 6, {0x8B,0x0D,0xAC,0x98,0xB0,0x00}, 0x00B098A8},
    {0x004F79D0, 6, {0x8B,0x0D,0xAC,0x98,0xB0,0x00}, 0x00B098A8},
    {0x004F9F40, 6, {0x8B,0x0D,0xD4,0x98,0xB0,0x00}, 0x00B098D0},
    {0x0054092B, 6, {0x8B,0x15,0xF0,0x9F,0xB0,0x00}, 0x00B09FEC},
    {0x00540AE8, 6, {0x8B,0x15,0xF0,0x9F,0xB0,0x00}, 0x00B09FEC},
    {0x00541B03, 6, {0x8B,0x15,0xF0,0x9F,0xB0,0x00}, 0x00B09FEC},
    {0x00542828, 6, {0x8B,0x15,0xF0,0x9F,0xB0,0x00}, 0x00B09FEC},
    {0x005448B3, 6, {0x8B,0x15,0x1C,0xA0,0xB0,0x00}, 0x00B0A018},
    {0x005469BC, 6, {0x8B,0x0D,0x1C,0xA0,0xB0,0x00}, 0x00B0A018},
    {0x005572B3, 6, {0x8B,0x15,0xA0,0xA0,0xB0,0x00}, 0x00B0A09C},
    {0x0055B993, 6, {0x8B,0x15,0xFC,0xA0,0xB0,0x00}, 0x00B0A0F8},
    {0x00562C3C, 6, {0x8B,0x0D,0xFC,0xA0,0xB0,0x00}, 0x00B0A0F8},
    {0x005CA8B8, 6, {0x8B,0x15,0x0C,0xA8,0xB0,0x00}, 0x00B0A808},
    {0x005D0BD8, 6, {0x8B,0x15,0x0C,0xA8,0xB0,0x00}, 0x00B0A808},
    {0x005D5518, 6, {0x8B,0x15,0x28,0xA8,0xB0,0x00}, 0x00B0A824},
    {0x005D6238, 6, {0x8B,0x15,0x34,0xA8,0xB0,0x00}, 0x00B0A830},
    {0x005D64D8, 6, {0x8B,0x15,0x34,0xA8,0xB0,0x00}, 0x00B0A830},
    {0x005DED08, 6, {0x8B,0x15,0x70,0xA8,0xB0,0x00}, 0x00B0A86C},
    {0x005E00CD, 6, {0x8B,0x15,0x88,0xA8,0xB0,0x00}, 0x00B0A884},
    {0x005E0838, 6, {0x8B,0x15,0x70,0xA8,0xB0,0x00}, 0x00B0A86C},
    {0x005E14A8, 6, {0x8B,0x15,0x9C,0xA8,0xB0,0x00}, 0x00B0A898},
    {0x005E2A68, 6, {0x8B,0x15,0xB0,0xA8,0xB0,0x00}, 0x00B0A8AC},
    {0x005E3348, 6, {0x8B,0x15,0xB0,0xA8,0xB0,0x00}, 0x00B0A8AC},
    {0x005EBE08, 6, {0x8B,0x15,0xCC,0xA8,0xB0,0x00}, 0x00B0A8C8},
    {0x005ECE78, 6, {0x8B,0x15,0xE8,0xA8,0xB0,0x00}, 0x00B0A8E4},
    {0x005ED6F8, 6, {0x8B,0x15,0xF4,0xA8,0xB0,0x00}, 0x00B0A8F0},
    {0x005ED9B8, 6, {0x8B,0x15,0x00,0xA9,0xB0,0x00}, 0x00B0A8FC},
    {0x005EDAF8, 6, {0x8B,0x15,0x0C,0xA9,0xB0,0x00}, 0x00B0A908},
    {0x005EDD98, 6, {0x8B,0x15,0x18,0xA9,0xB0,0x00}, 0x00B0A914},
    {0x005EEF68, 6, {0x8B,0x15,0xE8,0xA8,0xB0,0x00}, 0x00B0A8E4},
    {0x005EF198, 6, {0x8B,0x15,0xF4,0xA8,0xB0,0x00}, 0x00B0A8F0},
    {0x005EFC98, 6, {0x8B,0x15,0x0C,0xA9,0xB0,0x00}, 0x00B0A908},
    {0x005F04A8, 6, {0x8B,0x15,0x18,0xA9,0xB0,0x00}, 0x00B0A914},
    {0x005F0578, 6, {0x8B,0x15,0x00,0xA9,0xB0,0x00}, 0x00B0A8FC},
    {0x005F5068, 6, {0x8B,0x15,0x70,0xA9,0xB0,0x00}, 0x00B0A96C},
    {0x005F5CF8, 6, {0x8B,0x15,0x70,0xA9,0xB0,0x00}, 0x00B0A96C},
    {0x005F7D98, 6, {0x8B,0x15,0xBC,0xA9,0xB0,0x00}, 0x00B0A9B8},
    {0x005F84E8, 6, {0x8B,0x15,0xBC,0xA9,0xB0,0x00}, 0x00B0A9B8},
    {0x005FD858, 6, {0x8B,0x15,0xEC,0xA9,0xB0,0x00}, 0x00B0A9E8},
    {0x005FD908, 6, {0x8B,0x15,0xF8,0xA9,0xB0,0x00}, 0x00B0A9F4},
    {0x005FDF18, 6, {0x8B,0x15,0xEC,0xA9,0xB0,0x00}, 0x00B0A9E8},
    {0x005FDFF0, 6, {0x8B,0x15,0xEC,0xA9,0xB0,0x00}, 0x00B0A9E8},
    {0x005FE244, 5, {0xA1,0xEC,0xA9,0xB0,0x00}, 0x00B0A9E8},
    {0x005FE2D4, 5, {0xA1,0xEC,0xA9,0xB0,0x00}, 0x00B0A9E8},
    {0x005FE364, 5, {0xA1,0xEC,0xA9,0xB0,0x00}, 0x00B0A9E8},
    {0x005FE3F4, 5, {0xA1,0xEC,0xA9,0xB0,0x00}, 0x00B0A9E8},
    {0x005FE484, 5, {0xA1,0xEC,0xA9,0xB0,0x00}, 0x00B0A9E8},
    {0x005FE524, 5, {0xA1,0xEC,0xA9,0xB0,0x00}, 0x00B0A9E8},
    {0x005FE5B4, 5, {0xA1,0xEC,0xA9,0xB0,0x00}, 0x00B0A9E8},
    {0x005FE644, 5, {0xA1,0xEC,0xA9,0xB0,0x00}, 0x00B0A9E8},
    {0x005FE6E4, 5, {0xA1,0xEC,0xA9,0xB0,0x00}, 0x00B0A9E8},
    {0x005FE784, 5, {0xA1,0xEC,0xA9,0xB0,0x00}, 0x00B0A9E8},
    {0x005FE824, 5, {0xA1,0xEC,0xA9,0xB0,0x00}, 0x00B0A9E8},
    {0x005FF304, 5, {0xA1,0xEC,0xA9,0xB0,0x00}, 0x00B0A9E8},
    {0x005FF7E9, 6, {0x8B,0x0D,0xEC,0xA9,0xB0,0x00}, 0x00B0A9E8},
    {0x005FF8DC, 6, {0x8B,0x0D,0xEC,0xA9,0xB0,0x00}, 0x00B0A9E8},
    {0x005FF9CC, 6, {0x8B,0x0D,0xEC,0xA9,0xB0,0x00}, 0x00B0A9E8},
    {0x005FFAC1, 6, {0x8B,0x0D,0xEC,0xA9,0xB0,0x00}, 0x00B0A9E8},
    {0x00605058, 6, {0x8B,0x15,0xD4,0xB2,0xB0,0x00}, 0x00B0B2D0},
    {0x00605468, 6, {0x8B,0x15,0xD4,0xB2,0xB0,0x00}, 0x00B0B2D0},
    {0x00619EA8, 6, {0x8B,0x15,0x98,0xB3,0xB0,0x00}, 0x00B0B394},
    {0x0061C498, 6, {0x8B,0x15,0x98,0xB3,0xB0,0x00}, 0x00B0B394},
    {0x0061D218, 6, {0x8B,0x15,0xAC,0xB3,0xB0,0x00}, 0x00B0B3A8},
    {0x0061E838, 6, {0x8B,0x15,0xAC,0xB3,0xB0,0x00}, 0x00B0B3A8},
    {0x00647248, 6, {0x8B,0x15,0x74,0xB5,0xB0,0x00}, 0x00B0B570},
    {0x00647808, 6, {0x8B,0x15,0x74,0xB5,0xB0,0x00}, 0x00B0B570},
    {0x006487E8, 6, {0x8B,0x15,0xC4,0xBE,0xB0,0x00}, 0x00B0BEC0},
    {0x006489C8, 6, {0x8B,0x15,0xC4,0xBE,0xB0,0x00}, 0x00B0BEC0},
    {0x006499E8, 6, {0x8B,0x15,0xCC,0xC0,0xB0,0x00}, 0x00B0C0C8},
    {0x0064A2F8, 6, {0x8B,0x15,0x5C,0xC2,0xB0,0x00}, 0x00B0C258},
    {0x0064AB88, 6, {0x8B,0x15,0x5C,0xC2,0xB0,0x00}, 0x00B0C258},
    {0x0064E358, 6, {0x8B,0x15,0xCC,0xC0,0xB0,0x00}, 0x00B0C0C8},
    {0x00652FF8, 6, {0x8B,0x15,0xBC,0xCD,0xB0,0x00}, 0x00B0CDB8},
    {0x00653578, 6, {0x8B,0x15,0xBC,0xCD,0xB0,0x00}, 0x00B0CDB8},
    {0x006BF448, 6, {0x8B,0x15,0xB0,0xD3,0xB0,0x00}, 0x00B0D3AC},
    {0x006C2722, 6, {0x8B,0x15,0xB0,0xD3,0xB0,0x00}, 0x00B0D3AC},
    {0x006C2FD8, 6, {0x8B,0x15,0xB0,0xD3,0xB0,0x00}, 0x00B0D3AC},
    {0x006C3434, 6, {0x8B,0x15,0xB0,0xD3,0xB0,0x00}, 0x00B0D3AC},
    {0x0070FBA9, 6, {0x8B,0x15,0x88,0xD4,0xB0,0x00}, 0x00B0D484},
    {0x0070FC34, 6, {0x8B,0x3D,0x88,0xD4,0xB0,0x00}, 0x00B0D484},
    {0x007C5412, 6, {0x8B,0x15,0x34,0xDC,0xB0,0x00}, 0x00B0DC30},
    {0x007CA292, 6, {0x8B,0x0D,0x34,0xDC,0xB0,0x00}, 0x00B0DC30},
    {0x007FB663, 6, {0x8B,0x15,0x34,0xE3,0xB0,0x00}, 0x00B0E330},
    {0x007FC463, 6, {0x8B,0x15,0x4C,0xE3,0xB0,0x00}, 0x00B0E348},
    {0x007FDF08, 6, {0x8B,0x15,0x40,0xE3,0xB0,0x00}, 0x00B0E33C},
    {0x007FED68, 6, {0x8B,0x15,0x4C,0xE3,0xB0,0x00}, 0x00B0E348},
    {0x00801B23, 6, {0x8B,0x15,0x6C,0xE3,0xB0,0x00}, 0x00B0E368},
    {0x008065E8, 6, {0x8B,0x15,0x8C,0xE3,0xB0,0x00}, 0x00B0E388},
};

void __cdecl PoolTaken(const uint32_t* pool, uint32_t stride) {
    if (!OnArena()) return;
    auto* object = reinterpret_cast<uint8_t*>(reinterpret_cast<const uint32_t*>(pool[1])[2]);
    if (!InArena(object)) return;
    g_poolStamps[static_cast<uint32_t>(object - g_arena.base)] = {++g_arena.stamps, stride};
}

// Each take calls a thunk: the hooked read, then PoolTaken(pool, stride), preserving registers and flags.
// A callable thunk also lets a native allocator follow the live rel32 at the site.
const char* HookPoolTakes(unsigned* hooked) {
    constexpr size_t kBytes = 48 * std::size(kPoolTakes);  // a thunk is at most 6 + 27 bytes, aligned to 16
    auto* code = static_cast<uint8_t*>(VirtualAlloc(nullptr, kBytes, MEM_COMMIT | MEM_RESERVE, PAGE_EXECUTE_READWRITE));
    if (code == nullptr) return "no thunk page";
    uint8_t* c = code;
    const auto put32 = [&c](uint32_t v) { std::memcpy(c, &v, 4); c += 4; };
    const auto rel32 = [&c, &put32](uintptr_t target) { put32(static_cast<uint32_t>(target - (reinterpret_cast<uintptr_t>(c) + 4))); };
    for (const PoolTake& t : kPoolTakes) {
        uint32_t stride = 0;
        for (const PoolSite& site : kPools) {
            if (site.pool == t.pool) stride = site.stride;
        }
        if (stride == 0) return "a take without its pool";
        uint8_t* thunk = c;
        std::memcpy(c, t.bytes, t.n);
        c += t.n;
        *c++ = 0x9C;  // pushfd
        *c++ = 0x60;  // pushad
        *c++ = 0x68;  // push stride
        put32(stride);
        *c++ = 0x68;  // push pool
        put32(static_cast<uint32_t>(t.pool));
        *c++ = 0xE8;  // call PoolTaken
        rel32(reinterpret_cast<uintptr_t>(&PoolTaken));
        *c++ = 0x83;  // add esp, 8
        *c++ = 0xC4;
        *c++ = 0x08;
        *c++ = 0x61;  // popad
        *c++ = 0x9D;  // popfd
        *c++ = 0xC3;  // ret to the exe or a native port
        c = code + ((c - code + 15) & ~15);
        FlushInstructionCache(GetCurrentProcess(), thunk, c - thunk);
        if (const char* err = PatchJump(t.at, t.bytes, t.n, thunk, 0xE8)) return err;
        ++*hooked;
    }
    return nullptr;
}

// ---- address order ------------------------------------------------------------------------------
// [choice] Where the original walks a container keyed by object address, the walk is in the objects' creation
// order (simulator.md §5.8). An object's creation stamp is the allocation number of the region block that holds
// it: the key is the object (malloc), 4 bytes into the block (the framework allocator's blocks, FastAllocate, which
// the game's operator new uses) or an interface further inside it (a class with several bases). Blocks never split
// (a free block is reused only at its own span) and are cleared when reused, so the nearest header before the key
// whose block holds it is the block's own. A pool object's stamp is its take's (PoolTaken). An object outside the
// region (created before the city load) has none and sorts after all others, by address (counted: such a walk
// does not repeat).
volatile LONG g_unstamped = 0, g_ordered = 0;
std::map<std::pair<bool, uint32_t>, uint32_t> g_unstampedClasses;
constexpr size_t kStampScan = 0x2000;  // how far before the key its block's header may be

uint32_t StampOf(void* p) {
    auto* b = static_cast<uint8_t*>(p);
    if (InArena(b) && !g_poolStamps.empty() && GetCurrentThreadId() == g_mainThread) {
        const auto key = static_cast<uint32_t>(b - g_arena.base);
        auto it = g_poolStamps.upper_bound(key);
        if (it != g_poolStamps.begin() && key - (--it)->first < it->second.stride) return it->second.stamp;
    }
    if (b != nullptr && InArena(b - 4 - sizeof(BlockHeader))) {
        if (const BlockHeader* h = ArenaHeader(b)) return h->stamp;
        const BlockHeader* h = ArenaHeader(b - 4);
        if (h != nullptr && *reinterpret_cast<const uint32_t*>(b - 4) == ((h->size) | 0x80000000u)) return h->stamp;
        const uintptr_t key = reinterpret_cast<uintptr_t>(b);
        const uintptr_t low = reinterpret_cast<uintptr_t>(g_arena.base);
        for (uintptr_t at = (key - sizeof(BlockHeader)) & ~uintptr_t{15}; at >= low && key - at <= kStampScan; at -= 16) {
            const auto* c = reinterpret_cast<const BlockHeader*>(at);
            if (c->magic == kArenaMagic && c->span % 16 == 0 && c->size + sizeof(BlockHeader) <= c->span) {
                if (key < at + sizeof(BlockHeader) + c->size) return c->stamp;
                break;  // the block before the key ends before it: the key is in a pool page
            }
        }
    }
    return UINT32_MAX;
}

uint32_t CreationStamp(void* p) {
    const uint32_t stamp = StampOf(p);
    if (stamp != UINT32_MAX) return stamp;
    auto* b = static_cast<uint8_t*>(p);
    InterlockedIncrement(&g_unstamped);
    if (b != nullptr && GetCurrentThreadId() == g_mainThread) {
        // [diagnostic] by the key's first word (its vtable), in the region (a pool page) or not
        const uint32_t vtable = *reinterpret_cast<const uint32_t*>(b);
        ++g_unstampedClasses[{InArena(b), vtable}];
    }
    return UINT32_MAX;
}

// An STLport hash table of object pointers (nodes {next, key, ...}; `vector` points at its bucket vector
// {begin, end}), put in creation order for one call that walks it without changing it: every node is chained in
// bucket 0, sorted, and the table is restored after. The original's walk ends with bucket 0's chain: at a chain's
// end it looks for the next bucket from the last key, and all the others are empty.
class CreationOrder {
  public:
    explicit CreationOrder(uint8_t* vector)
        : begin_(reinterpret_cast<void***>(vector)[0]), end_(reinterpret_cast<void***>(vector)[1]) {
        for (void** b = begin_; b != end_; ++b) {
            buckets_.push_back(*b);
            for (auto* node = static_cast<void**>(*b); node != nullptr; node = static_cast<void**>(node[0])) {
                nodes_.push_back({node, node[0], CreationStamp(node[1])});
            }
        }
        if (nodes_.empty()) return;
        std::stable_sort(nodes_.begin(), nodes_.end(), [](const Node& a, const Node& b) {
            if (a.stamp != b.stamp) return a.stamp < b.stamp;
            return reinterpret_cast<uintptr_t>(a.node[1]) < reinterpret_cast<uintptr_t>(b.node[1]);
        });
        TraceNote('W', static_cast<uint32_t>(nodes_.size()), 0);
        for (const Node& n : nodes_) TraceNote('K', n.stamp, 0);
        for (size_t i = 0; i < nodes_.size(); ++i) nodes_[i].node[0] = i + 1 < nodes_.size() ? nodes_[i + 1].node : nullptr;
        std::fill(begin_, end_, nullptr);
        *begin_ = nodes_[0].node;
        InterlockedExchangeAdd(&g_ordered, static_cast<LONG>(nodes_.size()));
    }
    ~CreationOrder() {
        for (const Node& n : nodes_) n.node[0] = n.next;
        std::copy(buckets_.begin(), buckets_.end(), begin_);
    }
    CreationOrder(const CreationOrder&) = delete;
    CreationOrder& operator=(const CreationOrder&) = delete;

  private:
    struct Node {
        void** node;
        void* next;
        uint32_t stamp;
    };
    void** begin_;
    void** end_;
    std::vector<void*> buckets_;
    std::vector<Node> nodes_;
};

// The automata controller manager's save (Write 0x005174D0, automata.md §3.1): its occupant entries and queues
// are its two hash maps of occupant pointers (buckets at +0x50 and +0x64).
using SerializeFn = bool(__fastcall*)(void*, void*, void*);
void* g_realControllerWrite = nullptr;

bool __fastcall ControllerWrite(uint8_t* self, void*, void* stream) {
    CreationOrder entries(self + 0x50), queues(self + 0x64);
    return reinterpret_cast<SerializeFn>(g_realControllerWrite)(self, nullptr, stream);
}

// The traffic network map's save (Write 0x004B1CE0; `self` is the map + 0x0C): its intersections, a hash map of
// network occupant pointers (the map's +0x28, buckets at self + 0x20), and its last table, a hash map of network
// occupant pointers to four (u32 list, u16) pairs (buckets at self + 0x38).
void* g_realTrafficMapWrite = nullptr;

bool __fastcall TrafficMapWrite(uint8_t* self, void*, void* stream) {
    CreationOrder intersections(self + 0x20), occupants(self + 0x38);
    return reinterpret_cast<SerializeFn>(g_realTrafficMapWrite)(self, nullptr, stream);
}

// The watercraft manager's save (Write 0x0056FA70, automata.md §5.3; `self` is the manager + 8): its (lot, occupant)
// pairs, its occupants and its ferry entries are three hash maps of object pointers (buckets at self + 0xA9C,
// + 0xAB0 and + 0xA88).
void* g_realWatercraftWrite = nullptr;

bool __fastcall WatercraftWrite(uint8_t* self, void*, void* stream) {
    CreationOrder pairs(self + 0xA9C), occupants(self + 0xAB0), ferries(self + 0xA88);
    return reinterpret_cast<SerializeFn>(g_realWatercraftWrite)(self, nullptr, stream);
}

// The neighbours simulator's GetAdjacentCities (0x006D01E0, neighbors.md §9.1; thiscall (list, includeUnestablished)):
// the original collects the cities into a set of pointers, so the list it appends to is in address order. The cities
// it appends are put in region position order (x, then z: the regional city's slot 5, GetPosition(x&, z&)), §9.1's
// order, which Init's reloads and clones (§14.1 steps 5-7) and the proposals and pushes follow. The list is an STLport
// list of city references: its first word is the sentinel node {next, prev}; a node is {next, prev, city}.
using AdjacentCitiesFn = uint32_t(__fastcall*)(void*, void*, void**, uint32_t);
void* g_realAdjacentCities = nullptr;

uint32_t __fastcall AdjacentCities(void* self, void*, void** list, uint32_t includeUnestablished) {
    auto* sentinel = static_cast<void**>(list[0]);
    auto* last = static_cast<void**>(sentinel[1]);  // the last node before the call (the sentinel when empty)
    const uint32_t r = reinterpret_cast<AdjacentCitiesFn>(g_realAdjacentCities)(self, nullptr, list, includeUnestablished);
    struct Entry {
        void** node;
        int32_t x, z;
    };
    std::vector<Entry> added;
    for (auto* n = static_cast<void**>(last[0]); n != sentinel; n = static_cast<void**>(n[0])) {
        Entry e{n, 0, 0};
        if (void* city = n[2]) {
            using GetPositionFn = void(__thiscall*)(void*, int32_t*, int32_t*);
            reinterpret_cast<GetPositionFn>((*static_cast<void***>(city))[5])(city, &e.x, &e.z);
        }
        added.push_back(e);
    }
    std::stable_sort(added.begin(), added.end(),
                     [](const Entry& a, const Entry& b) { return a.x != b.x ? a.x < b.x : a.z < b.z; });
    void** prev = last;
    for (const Entry& e : added) {
        prev[0] = e.node;
        e.node[1] = prev;
        prev = e.node;
    }
    prev[0] = sentinel;
    sentinel[1] = prev;
    InterlockedExchangeAdd(&g_ordered, static_cast<LONG>(added.size()));
    return r;
}

// A walk of an STLport hash table of object pointers that may change the table: the walk's begin call
// (hashtable::begin, 0x004B0390: thiscall, the table {?, buckets begin, buckets end}, out {node, table}) goes to
// OrderedBegin and its step to the next node to OrderedNext. The walk visits the keys present at its start in
// creation order; a key removed before its turn is skipped, one added during the walk is not visited. Main thread;
// walks nest (innermost last). A table's bucket is (key >> shift) % bucket count, shift 0 or (a set of pointers) 2.
constexpr uintptr_t kHashBegin = 0x004B0390;
constexpr uintptr_t kTickStep = 0x004489CD;          // the animation tick manager's call of its iterator's ++
constexpr uintptr_t kTickIteratorNext = 0x00448230;  // that ++
struct OrderedWalk {
    uint8_t* table;
    unsigned shift;
    std::vector<uint32_t> keys;
    size_t next;
};
std::vector<OrderedWalk> g_walks;
volatile LONG g_gone = 0;  // keys removed before their turn

void** HashFind(uint8_t* table, unsigned shift, uint32_t key) {
    void** begin = *reinterpret_cast<void***>(table + 4);
    void** end = *reinterpret_cast<void***>(table + 8);
    if (begin == end) return nullptr;
    for (auto* node = static_cast<void**>(begin[(key >> shift) % static_cast<uint32_t>(end - begin)]); node != nullptr;
         node = static_cast<void**>(node[0])) {
        if (static_cast<uint32_t>(reinterpret_cast<uintptr_t>(node[1])) == key) return node;
    }
    return nullptr;
}

void* OrderedStep() {
    OrderedWalk& w = g_walks.back();
    while (w.next < w.keys.size()) {
        if (void** node = HashFind(w.table, w.shift, w.keys[w.next++])) return node;
        InterlockedIncrement(&g_gone);
    }
    g_walks.pop_back();
    return nullptr;
}

uint32_t* OrderedBeginShift(uint8_t* table, unsigned shift, uint32_t* out, uint32_t site,
                            uint32_t (*rank)(void*) = &CreationStamp) {
    std::vector<std::pair<uint32_t, uint32_t>> order;  // rank, key
    void** begin = *reinterpret_cast<void***>(table + 4);
    void** end = *reinterpret_cast<void***>(table + 8);
    for (void** b = begin; b != end; ++b) {
        for (auto* node = static_cast<void**>(*b); node != nullptr; node = static_cast<void**>(node[0])) {
            order.emplace_back(rank(node[1]), static_cast<uint32_t>(reinterpret_cast<uintptr_t>(node[1])));
        }
    }
    std::sort(order.begin(), order.end());
    TraceNote('W', static_cast<uint32_t>(order.size()), site);
    for (const auto& o : order) TraceNote('K', o.first, 0);
    g_walks.push_back({table, shift, {}, 0});
    for (const auto& o : order) g_walks.back().keys.push_back(o.second);
    InterlockedExchangeAdd(&g_ordered, static_cast<LONG>(order.size()));
    out[0] = static_cast<uint32_t>(reinterpret_cast<uintptr_t>(OrderedStep()));
    out[1] = static_cast<uint32_t>(reinterpret_cast<uintptr_t>(table));
    return out;
}
uint32_t* __fastcall OrderedBegin(uint8_t* table, void*, uint32_t* out) {
    return OrderedBeginShift(table, 0, out, static_cast<uint32_t>(OriginalReturn(reinterpret_cast<uintptr_t>(_ReturnAddress()))));
}
uint32_t* __fastcall OrderedBeginPointers(uint8_t* table, void*, uint32_t* out) {
    return OrderedBeginShift(table, 2, out, static_cast<uint32_t>(OriginalReturn(reinterpret_cast<uintptr_t>(_ReturnAddress()))));
}
// A set of lot configurations (pointers) in ascending configuration ID (slot 3), zones.md §7.2.1's order.
uint32_t LotConfigurationId(void* cfg) {
    using GetIdFn = uint32_t(__thiscall*)(void*);
    return reinterpret_cast<GetIdFn>((*static_cast<void***>(cfg))[3])(cfg);
}
uint32_t* __fastcall OrderedBeginById(uint8_t* table, void*, uint32_t* out) {
    return OrderedBeginShift(table, 2, out, static_cast<uint32_t>(OriginalReturn(reinterpret_cast<uintptr_t>(_ReturnAddress()))),
                             &LotConfigurationId);
}

void* __cdecl OrderedNext() { return g_walks.empty() ? nullptr : OrderedStep(); }

// A walk whose step is a call of the iterator's ++ (ecx = the iterator {node, table}; returns it).
uint32_t* __fastcall OrderedAdvance(uint32_t* iterator) {
    iterator[0] = static_cast<uint32_t>(reinterpret_cast<uintptr_t>(OrderedNext()));
    return iterator;
}

// A walk that leaves early (a search that stops at its first match) drops the keys it has not visited.
volatile LONG g_abandonedEmpty = 0;
void __cdecl OrderedAbandon() {
    if (!g_walks.empty()) {
        g_walks.pop_back();
    } else {
        InterlockedIncrement(&g_abandonedEmpty);
    }
}

// A walk of an STLport red-black tree of object pointers (a set; nodes {color, parent, left, right, key}, null
// leaves; its header {color, root, leftmost, rightmost}) whose begin (the leftmost) and step (the successor) are
// inline: the begin goes to TreeBegin and the step to TreeNext. As for a hash table, the walk visits the keys
// present at its start in creation order and skips a key removed before its turn; its end is the header. A walk
// is found by its header, and a begin drops an earlier walk of the same header (one that left early).
struct TreeWalk {
    void** header;
    std::vector<uint32_t> keys;
    size_t next;
};
std::vector<TreeWalk> g_treeWalks;
volatile LONG g_treeLost = 0;  // steps of a tree walk that was not begun (ended at once)

void** TreeFind(void** header, uint32_t key) {
    for (auto* node = static_cast<void**>(header[1]); node != nullptr;) {
        const auto k = static_cast<uint32_t>(reinterpret_cast<uintptr_t>(node[4]));
        if (k == key) return node;
        node = static_cast<void**>(key < k ? node[2] : node[3]);
    }
    return nullptr;
}

void* __cdecl TreeNext(void** header) {
    for (size_t i = g_treeWalks.size(); i-- > 0;) {
        TreeWalk& w = g_treeWalks[i];
        if (w.header != header) continue;
        while (w.next < w.keys.size()) {
            if (void** node = TreeFind(header, w.keys[w.next++])) return node;
            InterlockedIncrement(&g_gone);
        }
        g_treeWalks.erase(g_treeWalks.begin() + static_cast<ptrdiff_t>(i));
        return header;
    }
    InterlockedIncrement(&g_treeLost);
    return header;
}

void* __cdecl TreeBegin(void** header, uint32_t site) {
    g_treeWalks.erase(std::remove_if(g_treeWalks.begin(), g_treeWalks.end(),
                                     [header](const TreeWalk& w) { return w.header == header; }),
                      g_treeWalks.end());
    std::vector<std::pair<uint32_t, uint32_t>> order;  // stamp, key
    std::vector<void**> pending;
    if (header[1] != nullptr) pending.push_back(static_cast<void**>(header[1]));
    while (!pending.empty()) {
        void** node = pending.back();
        pending.pop_back();
        order.emplace_back(CreationStamp(node[4]), static_cast<uint32_t>(reinterpret_cast<uintptr_t>(node[4])));
        if (node[2] != nullptr) pending.push_back(static_cast<void**>(node[2]));
        if (node[3] != nullptr) pending.push_back(static_cast<void**>(node[3]));
    }
    std::sort(order.begin(), order.end());
    TraceNote('W', static_cast<uint32_t>(order.size()), site);
    for (const auto& o : order) TraceNote('K', o.first, 0);
    g_treeWalks.push_back({header, {}, 0});
    for (const auto& o : order) g_treeWalks.back().keys.push_back(o.second);
    InterlockedExchangeAdd(&g_ordered, static_cast<LONG>(order.size()));
    return TreeNext(header);
}

// [diagnostic] `walk census = on`: hashtable::begin (kHashBegin) is replaced by an equal function that counts, per
// caller, its walks of tables with at least two keys that are objects in the region, with a digest of their
// orders (as creation stamps: a run with other addresses differs where a walk's order follows the addresses);
// listed with each month.
struct CensusSite {
    uint32_t walks;
    uint32_t byAddress;
    uint32_t keys;
    uint32_t order;  // a digest of those walks' orders, as their keys' creation stamps (0 for none)
};
std::map<uint32_t, CensusSite> g_census;
bool g_censusOn = false;
uint32_t* __fastcall HashBeginCensus(uint8_t* table, void*, uint32_t* out) {
    void** begin = *reinterpret_cast<void***>(table + 4);
    void** end = *reinterpret_cast<void***>(table + 8);
    void* first = nullptr;
    const bool count = OnArena();
    uint32_t region = 0, order = 2166136261u;
    for (void** b = begin; b != end; ++b) {
        if (*b == nullptr) continue;
        if (first == nullptr) first = *b;
        if (!count) break;
        for (auto* node = static_cast<void**>(*b); node != nullptr; node = static_cast<void**>(node[0])) {
            // an object: in the used region, aligned, its first word a vtable of the executable
            const auto* key = static_cast<const uint8_t*>(node[1]);
            if (InArena(key) && key < g_arena.base + g_arena.top && (reinterpret_cast<uintptr_t>(key) & 3) == 0) {
                const uint32_t first = *reinterpret_cast<const uint32_t*>(key);
                if (first >= 0x00A80000 && first < 0x00B00000) {
                    ++region;
                    const uint32_t stamp = StampOf(node[1]);
                    for (int i = 0; i < 4; ++i) order = (order ^ ((stamp >> (8 * i)) & 0xFF)) * 16777619u;
                }
            }
        }
    }
    if (count) {
        CensusSite& site = g_census[static_cast<uint32_t>(OriginalReturn(reinterpret_cast<uintptr_t>(_ReturnAddress())))];
        ++site.walks;
        if (region >= 2) {
            ++site.byAddress;
            site.keys += region;
            for (int i = 0; i < 4; ++i) site.order = (site.order ^ ((order >> (8 * i)) & 0xFF)) * 16777619u;
        }
    }
    out[0] = static_cast<uint32_t>(reinterpret_cast<uintptr_t>(first));
    out[1] = static_cast<uint32_t>(reinterpret_cast<uintptr_t>(table));
    return out;
}

// The walks redirected: each step to the next node is replaced whole by a thunk that puts OrderedNext's node in
// the walk's node register and continues at the loop's test of it. A walk that can leave early has its exit
// patched too (kWalkExits).
struct WalkSite {
    const char* name;
    uintptr_t begin;  // its call of kHashBegin
    uintptr_t step;   // its step to the next node
    uint8_t bytes[5];
    const void* thunk;
    const void* order = nullptr;  // its begin, if not OrderedBegin (OrderedBeginPointers for a step call)
};
#define FC4_WALK_STEP(name, reg, test)      __declspec(naked) void name() {             __asm call OrderedNext                  __asm mov reg, eax                      __asm push test                         __asm ret                           }
FC4_WALK_STEP(UpdateControllersStep, ebx, 0x005189E5)
FC4_WALK_STEP(AnimationTickStep, eax, 0x00518D9B)  // the test also stores eax in the iterator
FC4_WALK_STEP(TrafficMapRebuildStep, esi, 0x004B49DF)
FC4_WALK_STEP(SituationPickStep, ebx, 0x0052A311)
FC4_WALK_STEP(SituationMatchStep, eax, 0x0052A573)  // the test also stores eax in the iterator
FC4_WALK_STEP(LotSizeConfigurationStep, eax, 0x00733201)  // the test also stores eax in the iterator
#undef FC4_WALK_STEP
const WalkSite kWalkSites[] = {
    // The animation tick manager's tick (0x004487F0) of each priority class's set of targets (animation-tick.md §4),
    // a set of pointers (its bucket step 0x0047C1A0 shifts the key by 2); its step is a call of the iterator's ++
    // (kTickStep), which goes to OrderedAdvance.
    {"the animation tick manager's tick (0x004487F0)", 0x004489A4, 0, {}, nullptr},
    // The automata controller manager's per-frame walks of its occupant map (+0x58; automata.md §3.1).
    // mov eax, [ebx]; test eax, eax; je ...
    {"the automata controller manager's UpdateControllers (0x00518950)", 0x00518961, 0x005189AB,
     {0x8B, 0x03, 0x85, 0xC0, 0x74}, reinterpret_cast<const void*>(&UpdateControllersStep)},
    // mov eax, [esi]; test eax, eax; jne ...
    {"the automata controller manager's DoAnimationTick (0x00518B80)", 0x00518C75, 0x00518D8C,
     {0x8B, 0x06, 0x85, 0xC0, 0x75}, reinterpret_cast<const void*>(&AnimationTickStep)},
    // The traffic network map's rebuild (0x004B48B0) of its intersections (+0x28). mov eax, [esi]; test eax, eax; je ...
    {"the traffic network map's intersection rebuild (0x004B48B0)", 0x004B4980, 0x004B49A0,
     {0x8B, 0x06, 0x85, 0xC0, 0x74}, reinterpret_cast<const void*>(&TrafficMapRebuildStep)},
    // The city situation manager's pick (0x0052A1E0) of the first eligible automaton among the occupants of an
    // area (a set it fills, 0x00525640) that it does not already track. mov eax, [ebx]; test eax, eax; je ...
    {"the city situation manager's pick of an automaton (0x0052A1E0)", 0x0052A22D, 0x0052A2D7,
     {0x8B, 0x03, 0x85, 0xC0, 0x74}, reinterpret_cast<const void*>(&SituationPickStep)},
    // Its match (0x0052A380) of a situation's automaton with its tracked automata (+0xA08), up to the first match.
    // mov eax, [esp + 0x28]; mov eax, [eax]; ...
    {"the city situation manager's match of its tracked automata (0x0052A380)", 0x0052A4A9, 0x0052A560,
     {0x8B, 0x44, 0x24, 0x28, 0x8B}, reinterpret_cast<const void*>(&SituationMatchStep)},
    // The zone developer's DetermineLotSize (0x00732BF0) scoring of the lot configurations of a set of pointers it
    // fills (zones.md §7.2.1 step 6), the first of equal scores winning. mov ecx, [esp + 0x48]; mov eax, [ecx]; ...
    {"the zone developer's lot size choice (0x00732BF0)", 0x00732F87, 0x007331C1,
     {0x8B, 0x4C, 0x24, 0x48, 0x8B}, reinterpret_cast<const void*>(&LotSizeConfigurationStep),
     reinterpret_cast<const void*>(&OrderedBeginById)},
};

// The early exits of those walks: each drops the walk (OrderedAbandon), then does what it replaced.
__declspec(naked) void SituationPickFound() {  // the pick's AddRef of the automaton it found, then its exit
    __asm {
        call OrderedAbandon
        mov edx, dword ptr [esi]
        mov ecx, esi
        call dword ptr [edx + 4]
        push 0x0052A322
        ret
    }
}
__declspec(naked) void SituationMatchTest() {  // the match's loop test of its found flag
    __asm {
        mov al, byte ptr [esp + 0x13]
        test al, al
        jne found
        push 0x0052A4DC
        ret
    found:
        call OrderedAbandon
        push 0x0052A646
        ret
    }
}
struct WalkExit {
    const char* name;
    uintptr_t at;
    uint8_t bytes[5];
    const void* thunk;
};
const WalkExit kWalkExits[] = {
    // mov edx, [esi]; mov ecx, esi; call [edx + 4]
    {"the situation pick's exit at its find (0x0052A31B)", 0x0052A31B, {0x8B, 0x16, 0x8B, 0xCE, 0xFF},
     reinterpret_cast<const void*>(&SituationPickFound)},
    // mov al, [esp + 0x13]; test al, al
    {"the situation match's exit at its match (0x0052A4D0)", 0x0052A4D0, {0x8A, 0x44, 0x24, 0x13, 0x84},
     reinterpret_cast<const void*>(&SituationMatchTest)},
};
constexpr uint8_t kHashBeginBytes[] = {0x8B, 0x51, 0x08, 0x56, 0x8B};

// The walks whose begin is inline too, each begin and step replaced by a thunk.
// Red-black trees (TreeBegin, TreeNext): demolition's set of the occupants it removes (demolition.md §4.4 step 5,
// §4.6). Each begin is `mov reg, [header + 8]; cmp reg, header`, replaced by a thunk that sets reg to TreeBegin's
// node and the flags as that compare did; each step ends at the loop's compare of its node (eax) with the header,
// where its thunk continues.
__declspec(naked) void DemolishOccupantsBegin() {  // DemolishRegion's price and remove (0x00468410): edx, header ecx
    __asm {
        push eax
        push ecx
        push 0x00468912
        push ecx
        call TreeBegin
        add esp, 8
        mov edx, eax
        pop ecx
        pop eax
        cmp edx, ecx
        push 0x00468917
        ret
    }
}
__declspec(naked) void DemolishOccupantsStep() {  // its header at [esp + 0x44]
    __asm {
        push edx
        push dword ptr [esp + 0x48]
        call TreeNext
        add esp, 4
        pop edx
        push 0x00468E34
        ret
    }
}
__declspec(naked) void UpdateNetworksBegin() {  // UpdateNetworks (0x004681A0): ebp, header ecx
    __asm {
        push eax
        push ecx
        push edx
        push 0x0046820F
        push ecx
        call TreeBegin
        add esp, 8
        mov ebp, eax
        pop edx
        pop ecx
        pop eax
        cmp ebp, ecx
        push 0x00468214
        ret
    }
}
__declspec(naked) void UpdateNetworksStep() {  // its header at [esp + 0x14]
    __asm {
        push edx
        push dword ptr [esp + 0x18]
        call TreeNext
        add esp, 4
        pop edx
        push 0x00468374
        ret
    }
}
// Hash tables (OrderedBegin, OrderedNext): the watercraft manager's DoAnimationTick (0x005736A0) creating the
// missing ferries of its ferry entries (the map at +0xA8C) and the missing marina boats of its marina map (+0xAB4),
// each walk's begin an inline search for the first bucket (automata.md §5.3). Each begin thunk sets the walk's node
// register (and the table register) and continues at the loop's test of the node.
__declspec(naked) void FerriesBegin() {  // ebx the manager; the node in ebp, the table in edi
    __asm {
        lea edi, [ebx + 0xA8C]
        sub esp, 8
        mov edx, esp
        push edx
        mov ecx, edi
        call OrderedBegin
        mov ebp, dword ptr [esp]
        add esp, 8
        push 0x0057403D
        ret
    }
}
__declspec(naked) void FerriesStep() {
    __asm {
        call OrderedNext
        mov ebp, eax
        push 0x00574114
        ret
    }
}
__declspec(naked) void MarinaBoatsBegin() {  // ebx the manager; the node in esi, the table in eax (then edi)
    __asm {
        lea eax, [ebx + 0xAB4]
        push eax
        sub esp, 8
        mov edx, esp
        push edx
        mov ecx, eax
        call OrderedBegin
        mov esi, dword ptr [esp]
        add esp, 8
        pop eax
        push 0x00574158
        ret
    }
}
__declspec(naked) void MarinaBoatsStep() {
    __asm {
        call OrderedNext
        mov esi, eax
        push 0x005741FF
        ret
    }
}
struct InlineWalkSite {
    const char* name;
    uintptr_t begin;
    uint8_t beginBytes[5];
    const void* beginThunk;
    uintptr_t step;
    uint8_t stepBytes[5];
    const void* stepThunk;
};
const InlineWalkSite kInlineWalks[] = {
    // mov edx, [ecx + 8]; cmp edx, ecx / mov eax, [esp + 0x10]; mov ecx, ...
    {"demolition's removal of a region's occupants (0x00468410)", 0x00468912, {0x8B, 0x51, 0x08, 0x3B, 0xD1},
     reinterpret_cast<const void*>(&DemolishOccupantsBegin), 0x00468DF8, {0x8B, 0x44, 0x24, 0x10, 0x8B},
     reinterpret_cast<const void*>(&DemolishOccupantsStep)},
    // mov ebp, [ecx + 8]; cmp ebp, ecx / mov ecx, [ebp + 0xC]; test ecx, ecx
    {"demolition's network update (0x004681A0)", 0x0046820F, {0x8B, 0x69, 0x08, 0x3B, 0xE9},
     reinterpret_cast<const void*>(&UpdateNetworksBegin), 0x00468337, {0x8B, 0x4D, 0x0C, 0x85, 0xC9},
     reinterpret_cast<const void*>(&UpdateNetworksStep)},
    // mov ecx, [ebx + 0xA94] / mov eax, [ebp]; test eax, eax
    {"the watercraft manager's ferry creation (0x005736A0)", 0x00573FFE, {0x8B, 0x8B, 0x94, 0x0A, 0x00},
     reinterpret_cast<const void*>(&FerriesBegin), 0x005740D9, {0x8B, 0x45, 0x00, 0x85, 0xC0},
     reinterpret_cast<const void*>(&FerriesStep)},
    // mov edx, [ebx + 0xABC] / mov eax, [esi]; test eax, eax; je ...
    {"the watercraft manager's marina boat creation (0x005736A0)", 0x00574120, {0x8B, 0x93, 0xBC, 0x0A, 0x00},
     reinterpret_cast<const void*>(&MarinaBoatsBegin), 0x005741C6, {0x8B, 0x06, 0x85, 0xC0, 0x74},
     reinterpret_cast<const void*>(&MarinaBoatsStep)},
};

// A trampoline: the entry's first n bytes (whole instructions, no relative operands), then a jump back.
uint8_t* Trampoline(uint8_t*& page, size_t& used, uintptr_t entry, size_t n) {
    if (page == nullptr) {
        page = static_cast<uint8_t*>(VirtualAlloc(nullptr, 4096, MEM_COMMIT | MEM_RESERVE, PAGE_EXECUTE_READWRITE));
        if (page == nullptr) return nullptr;
    }
    if (used + n + 5 > 4096) return nullptr;
    uint8_t* t = page + used;
    std::memcpy(t, reinterpret_cast<const void*>(entry), n);
    t[n] = 0xE9;
    const int32_t rel = static_cast<int32_t>((entry + n) - (reinterpret_cast<uintptr_t>(t) + n + 5));
    std::memcpy(t + n + 1, &rel, 4);
    used += (n + 5 + 15) & ~size_t{15};
    return t;
}

// Reserves the region and hooks the heap entries; false (and the original heaps) if any step fails.
bool HeapInstall(const char** failure) {
    for (const size_t size : kArenaSizes) {
        for (const uintptr_t base : kArenaBases) {
            void* p = VirtualAlloc(reinterpret_cast<void*>(base), size, MEM_RESERVE, PAGE_NOACCESS);
            if (p == reinterpret_cast<void*>(base)) {
                g_arena.base = static_cast<uint8_t*>(p);
                g_arena.size = size;
                break;
            }
            if (p != nullptr) VirtualFree(p, 0, MEM_RELEASE);
        }
        if (g_arena.base != nullptr) break;
    }
    if (g_arena.base == nullptr) {
        *failure = "no fixed base could be reserved";
        return false;
    }
    struct Entry {
        uintptr_t entry;
        const uint8_t* bytes;
        size_t n;
        const void* hook;
        uint8_t** real;
    };
    const Entry entries[] = {
        {kMallocEntry, kMallocBytes, sizeof(kMallocBytes), reinterpret_cast<const void*>(&CrtMalloc), &g_heapReal.malloc},
        {kFreeEntry, kFreeBytes, sizeof(kFreeBytes), reinterpret_cast<const void*>(&CrtFree), &g_heapReal.free},
        {kReallocEntry, kReallocBytes, sizeof(kReallocBytes), reinterpret_cast<const void*>(&CrtRealloc), &g_heapReal.realloc},
        {kCallocEntry, kCallocBytes, sizeof(kCallocBytes), reinterpret_cast<const void*>(&CrtCalloc), &g_heapReal.calloc},
        {kMsizeEntry, kMsizeBytes, sizeof(kMsizeBytes), reinterpret_cast<const void*>(&CrtMsize), &g_heapReal.msize},
        {kFastAllocate, kFastAllocateBytes, sizeof(kFastAllocateBytes), reinterpret_cast<const void*>(&FastAllocate),
         &g_heapReal.fastAllocate},
        {kProcessHeapAlloc, kProcessHeapBytes, sizeof(kProcessHeapBytes), reinterpret_cast<const void*>(&ProcessHeapAlloc),
         &g_heapReal.processHeapAlloc},
        {kProcessHeapFree, kProcessHeapBytes, sizeof(kProcessHeapBytes), reinterpret_cast<const void*>(&ProcessHeapFree),
         &g_heapReal.processHeapFree},
        {kChunkAlloc, kChunkAllocBytes, sizeof(kChunkAllocBytes), reinterpret_cast<const void*>(&ChunkAlloc),
         &g_heapReal.chunkAlloc},
        {kChunkFree, kChunkFreeBytes, sizeof(kChunkFreeBytes), reinterpret_cast<const void*>(&ChunkFree), &g_heapReal.chunkFree},
        {kBlockAlloc, kBlockAllocBytes, sizeof(kBlockAllocBytes), reinterpret_cast<const void*>(&BlockAlloc),
         &g_heapReal.blockAlloc},
        {kBlockFree, kBlockFreeBytes, sizeof(kBlockFreeBytes), reinterpret_cast<const void*>(&BlockFree), &g_heapReal.blockFree},
    };
    // Every entry is checked and every trampoline built before the first jump is written.
    uint8_t* page = nullptr;
    size_t used = 0;
    for (const Entry& e : entries) {
        if (std::memcmp(reinterpret_cast<const void*>(e.entry), e.bytes, e.n) != 0) {
            *failure = "unexpected entry bytes";
            return false;
        }
        if ((*e.real = Trampoline(page, used, e.entry, e.n)) == nullptr) {
            *failure = "no trampoline page";
            return false;
        }
    }
    FlushInstructionCache(GetCurrentProcess(), page, 4096);
    for (const Entry& e : entries) {
        if (const char* err = PatchJump(e.entry, e.bytes, e.n, e.hook)) {
            *failure = err;  // the entries patched so far still call the original through their trampolines
            return false;
        }
    }
    return true;
}

std::string Lower(std::string s) {
    for (char& c : s) c = static_cast<char>(std::tolower(static_cast<unsigned char>(c)));
    const size_t b = s.find_first_not_of(" \t"), e = s.find_last_not_of(" \t");
    return b == std::string::npos ? std::string() : s.substr(b, e - b + 1);
}

}  // namespace

namespace fc4 {

const ScheduleSite* ScheduleSites(size_t* count) {
    *count = std::size(kScheduleSites);
    return kScheduleSites;
}

void ScheduleNativeModule(void* module) {
    for (size_t i = 0; i < std::size(kScheduleSites); ++i) {
        char name[16];
        std::snprintf(name, sizeof(name), "fc4r_%08x", kScheduleSites[i].site);
        g_nativeReturns[i] = reinterpret_cast<uintptr_t>(GetProcAddress(static_cast<HMODULE>(module), name));
    }
}

uint32_t ScheduleMissingSite(uint32_t function, const uint32_t* sites, size_t count) {
    const uint32_t missing = MissingScheduleSite(kScheduleSites, std::size(kScheduleSites), function, sites, count);
    if (missing != 0) return missing;
    // A declaration of a return key is usable only with its actual exported return instruction.
    for (size_t i = 0; i < std::size(kScheduleSites); ++i) {
        if (kScheduleSites[i].function != function) continue;
        for (uint32_t key : kReturnKeys) {
            if (kScheduleSites[i].site == key && g_nativeReturns[i] == 0) return key;
        }
    }
    return 0;
}

bool ScheduleFixed() { return g_fixed; }

void ScheduleStampsRestart() { g_arena.stamps = 0; }

void ScheduleStartup() {
    const char* source = "";
    const std::string v = Lower(Setting("schedule", "FC4_SCHEDULE", &source));
    g_wanted = v == "fixed";
    if (!g_wanted) {
        if (!v.empty() && v != "original") Log("schedule: WARNING schedule = '%s' is neither fixed nor original", v.c_str());
        return;
    }
    if (!Supported()) {
        Log("schedule: WARNING schedule = fixed needs the 1.1.641 executable; nothing is changed");
        return;
    }
    g_seed = SessionSeed(nullptr);
    const std::string reads = Setting("schedule limit reads", "SC4_SCHEDULE_LIMIT_READS");
    if (!reads.empty()) g_otherReads = static_cast<uint32_t>(std::strtoul(reads.c_str(), nullptr, 10));
    // The clock trampoline: clock()'s first five bytes (no relative operands), then back.
    g_clockTrampoline = static_cast<uint8_t*>(VirtualAlloc(nullptr, 16, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE));
    if (g_clockTrampoline != nullptr) {
        std::memcpy(g_clockTrampoline, kClockBytes, 5);
        g_clockTrampoline[5] = 0xE9;
        const int32_t rel = static_cast<int32_t>((kCrtClock + 5) - (reinterpret_cast<uintptr_t>(g_clockTrampoline) + 10));
        std::memcpy(g_clockTrampoline + 6, &rel, 4);
        DWORD before = 0;
        VirtualProtect(g_clockTrampoline, 16, PAGE_EXECUTE_READ, &before);
        FlushInstructionCache(GetCurrentProcess(), g_clockTrampoline, 16);
    }
    struct Hook {
        const char* name;
        uintptr_t entry;
        const uint8_t* bytes;
        size_t n;
        const void* target;
    };
    const Hook hooks[] = {
        {"LimitTimer::Start", kLimitTimerStart, kStartBytes, sizeof(kStartBytes), reinterpret_cast<const void*>(&LimitStart)},
        {"LimitTimer::Expired", kLimitTimerExpired, kExpiredBytes, sizeof(kExpiredBytes),
         reinterpret_cast<const void*>(&LimitExpired)},
        {"traffic remaining ms", kRemainingMs, kRemainingBytes, sizeof(kRemainingBytes),
         reinterpret_cast<const void*>(&RemainingMs)},
        {"time()", kCrtTime, kTimeBytes, sizeof(kTimeBytes), reinterpret_cast<const void*>(&CrtTime)},
        {"clock()", kCrtClock, kClockBytes, sizeof(kClockBytes), reinterpret_cast<const void*>(&CrtClock)},
        {"reference writer", kWriteRef, kWriteRefBytes, sizeof(kWriteRefBytes), reinterpret_cast<const void*>(&WriteRef)},
    };
    bool all = g_clockTrampoline != nullptr;
    for (const Hook& h : hooks) {
        const char* err = PatchJump(h.entry, h.bytes, h.n, h.target);
        if (err != nullptr) {
            all = false;
            Log("schedule: WARNING %s at 0x%08X not hooked: %s", h.name, static_cast<unsigned>(h.entry), err);
        }
    }
    // The performance counter: the import slot, so every cRZTimer reader on the
    // main thread sees the virtual counter; o = 0 (it was measured on the real one).
    g_mainThread = GetCurrentThreadId();
    LARGE_INTEGER f{};
    QueryPerformanceFrequency(&f);
    g_frameTicks = f.QuadPart * kFrameMs / 1000;
    g_qpcPerMs = f.QuadPart / 1000;
    const struct {
        const char* name;
        uintptr_t slot;
        void* hook;
        void** real;
    } slots[] = {
        {"QueryPerformanceCounter", kQpcSlot, reinterpret_cast<void*>(&VirtualQpc), reinterpret_cast<void**>(&g_realQpc)},
        {"Sleep", kSleepSlot, reinterpret_cast<void*>(&VirtualSleep), reinterpret_cast<void**>(&g_realSleep)},
        {"SleepEx", kSleepExSlot, reinterpret_cast<void*>(&VirtualSleepEx), reinterpret_cast<void**>(&g_realSleepEx)},
        {"GetTempPathA", kTempPathASlot, reinterpret_cast<void*>(&TempPathA), &g_realTempPathA},
        {"the automata controller manager's Write", kControllerWriteSlot, reinterpret_cast<void*>(&ControllerWrite),
         &g_realControllerWrite},
        {"the traffic network map's Write", kTrafficMapWriteSlot, reinterpret_cast<void*>(&TrafficMapWrite),
         &g_realTrafficMapWrite},
        {"the watercraft manager's Write", kWatercraftWriteSlot, reinterpret_cast<void*>(&WatercraftWrite),
         &g_realWatercraftWrite},
        {"the neighbours simulator's GetAdjacentCities", kAdjacentCitiesSlot, reinterpret_cast<void*>(&AdjacentCities),
         &g_realAdjacentCities},
    };
    if (const char* err = TempStart()) {
        all = false;
        Log("schedule: WARNING no temp directory of its own: %s", err);
    }
    for (const auto& e : slots) {
        auto* slot = reinterpret_cast<void**>(e.slot);
        DWORD before = 0;
        if (VirtualProtect(slot, 4, PAGE_READWRITE, &before)) {
            *e.real = *slot;
            *slot = e.hook;
            DWORD ignored = 0;
            VirtualProtect(slot, 4, before, &ignored);
        } else {
            all = false;
            Log("schedule: WARNING the %s slot is not writable", e.name);
        }
    }
    *reinterpret_cast<uint64_t*>(kTimerOverhead) = 0;
    const char* heapFailure = nullptr;
    g_heapReady = HeapInstall(&heapFailure);
    if (!g_heapReady) {
        all = false;
        Log("schedule: WARNING the heap is not fixed: %s", heapFailure);
    } else if (g_arena.base != reinterpret_cast<uint8_t*>(kArenaBases[0]) || g_arena.size != kArenaSizes[0]) {
        all = false;
        Log("schedule: WARNING the heap is at 0x%08X (%u MB), not 0x%08X (%u MB): its hash orders are not the other runs'",
            static_cast<unsigned>(reinterpret_cast<uintptr_t>(g_arena.base)), static_cast<unsigned>(g_arena.size >> 20),
            static_cast<unsigned>(kArenaBases[0]), static_cast<unsigned>(kArenaSizes[0] >> 20));
    }
    if (reinterpret_cast<uintptr_t>(&__ImageBase) != kDllBase) {
        all = false;
        Log("schedule: WARNING FreeCity4.dll is at 0x%08X, not its fixed base 0x%08X: the game's pointer-keyed sets "
            "holding its objects order differently", static_cast<unsigned>(reinterpret_cast<uintptr_t>(&__ImageBase)),
            static_cast<unsigned>(kDllBase));
    }
    for (const uintptr_t site : {kAddTimerNow[0], kAddTimerNow[1], kWorkerNow[0], kWorkerNow[1]}) {
        if (const char* err = PatchCall(site, kTimerElapsed, reinterpret_cast<const void*>(&SchedulerNow))) {
            all = false;
            Log("schedule: WARNING the timer scheduler's clock read at 0x%08X not hooked: %s", static_cast<unsigned>(site), err);
        }
    }
    uint8_t* page = nullptr;
    size_t used = 0;
    for (int i = 0; i < 4; ++i) {
        const uint8_t* bytes = i < 3 ? kElapsedBytes : kStopBytes;
        const size_t n = i < 3 ? sizeof(kElapsedBytes) : sizeof(kStopBytes);
        const char* err = std::memcmp(reinterpret_cast<const void*>(kStopwatchReads[i]), bytes, n) != 0 ? "unexpected bytes"
                          : (g_stopwatchReal[i] = Trampoline(page, used, kStopwatchReads[i], n)) == nullptr ? "no trampoline"
                                                                                                         : nullptr;
        if (err == nullptr) err = PatchJump(kStopwatchReads[i], bytes, n, reinterpret_cast<const void*>(kStopwatchThunks[i]));
        if (err != nullptr) {
            all = false;
            Log("schedule: WARNING the stopwatch read at 0x%08X not hooked: %s", static_cast<unsigned>(kStopwatchReads[i]), err);
        }
    }
    if (page != nullptr) FlushInstructionCache(GetCurrentProcess(), page, 4096);
    if (const char* err = PatchJump(kSwitchInsert, kSwitchInsertBytes, sizeof(kSwitchInsertBytes),
                                    reinterpret_cast<const void*>(&SwitchInsertThunk))) {
        all = false;
        Log("schedule: WARNING the transit switch insert at 0x%08X not hooked: %s", static_cast<unsigned>(kSwitchInsert), err);
    }
    if (const char* err = PatchJump(kIntersectionInsert, kIntersectionInsertBytes, sizeof(kIntersectionInsertBytes),
                                    reinterpret_cast<const void*>(&IntersectionInsertThunk))) {
        all = false;
        Log("schedule: WARNING the intersection insert at 0x%08X not hooked: %s", static_cast<unsigned>(kIntersectionInsert), err);
    }
    if (const char* err = PatchJump(kJunctionInsert, kJunctionInsertBytes, sizeof(kJunctionInsertBytes),
                                    reinterpret_cast<const void*>(&JunctionInsertThunk))) {
        all = false;
        Log("schedule: WARNING the train junction insert at 0x%08X not hooked: %s", static_cast<unsigned>(kJunctionInsert), err);
    }
    for (const WalkSite& w : kWalkSites) {
        const void* order = w.order != nullptr ? w.order
                            : w.thunk != nullptr ? reinterpret_cast<const void*>(&OrderedBegin)
                                                 : reinterpret_cast<const void*>(&OrderedBeginPointers);
        const char* err = PatchCall(w.begin, kHashBegin, order);
        if (err == nullptr) {
            err = w.thunk != nullptr ? PatchJump(w.step, w.bytes, sizeof(w.bytes), w.thunk)
                                     : PatchCall(kTickStep, kTickIteratorNext, reinterpret_cast<const void*>(&OrderedAdvance));
        }
        if (err != nullptr) {
            all = false;
            Log("schedule: WARNING %s not reordered: %s", w.name, err);
        }
    }
    for (const InlineWalkSite& w : kInlineWalks) {
        const char* err = std::memcmp(reinterpret_cast<const void*>(w.step), w.stepBytes, sizeof(w.stepBytes)) != 0
                              ? "unexpected bytes at its step"
                              : PatchJump(w.begin, w.beginBytes, sizeof(w.beginBytes), w.beginThunk);
        if (err == nullptr) err = PatchJump(w.step, w.stepBytes, sizeof(w.stepBytes), w.stepThunk);
        if (err != nullptr) {
            all = false;
            Log("schedule: WARNING %s not reordered: %s", w.name, err);
        }
    }
    unsigned takes = 0;
    if (const char* err = HookPoolTakes(&takes)) {
        all = false;
        Log("schedule: WARNING %u of %u pool takes hooked: %s", takes, static_cast<unsigned>(std::size(kPoolTakes)), err);
    }
    for (const WalkExit& x : kWalkExits) {
        if (const char* err = PatchJump(x.at, x.bytes, sizeof(x.bytes), x.thunk)) {
            all = false;
            Log("schedule: WARNING %s not hooked: %s", x.name, err);
        }
    }
    if (!Setting("walk census", "SC4_WALK_CENSUS").empty()) {
        const char* err = PatchJump(kHashBegin, kHashBeginBytes, sizeof(kHashBeginBytes),
                                    reinterpret_cast<const void*>(&HashBeginCensus));
        g_censusOn = err == nullptr;
        Log("schedule: walk census %s", err == nullptr ? "on" : err);
    }
    if (const char* err = PatchJump(kForeground, kForegroundBytes, sizeof(kForegroundBytes),
                                    reinterpret_cast<const void*>(&NeverForeground))) {
        all = false;
        Log("schedule: WARNING the foreground test at 0x%08X not hooked: %s", static_cast<unsigned>(kForeground), err);
    }
    if (const char* err = PatchJump(kRandomSeed, kRandomSeedBytes, sizeof(kRandomSeedBytes),
                                    reinterpret_cast<const void*>(&RandomSeedThunk))) {
        all = false;
        Log("schedule: WARNING cRZRandom::Seed at 0x%08X not hooked: %s", static_cast<unsigned>(kRandomSeed), err);
    }
    if (const char* err = PatchJump(kRandomSuperSeed, kRandomSuperSeedBytes, sizeof(kRandomSuperSeedBytes),
                                    reinterpret_cast<const void*>(&RandomSuperSeedThunk))) {
        all = false;
        Log("schedule: WARNING cRZRandomSuper::Seed at 0x%08X not hooked: %s", static_cast<unsigned>(kRandomSuperSeed), err);
    }
    if (const char* err = PatchCall(kZeroStateClock, kCrtClock, reinterpret_cast<const void*>(&ZeroStateClock))) {
        all = false;
        Log("schedule: WARNING the zero-state reseed at 0x%08X not hooked: %s", static_cast<unsigned>(kZeroStateClock), err);
    }
    ReseedGenerators();
    fc4_crt_set_host(&CrtState);
    g_fixed = true;
    Log("schedule: fixed (code %s, %s; docs/specs/simulator.md 5.8): time limits count reads (Grow expires at read %u, others "
        "at %u), time() = %u, the generators' clock() = %d, %u file-local generators re-seeded with derive(S = "
        "0x%08X, K), rand() set before every agent delivery and shared with our cores, save object IDs 1, 2, 3, ..., the controller manager's save, update and tick the traffic network map's save and rebuild, and the animation tick's targets in creation order, the main thread's "
        "performance counter virtual (+%u ms a frame), temp directory %s, the main thread's heap at 0x%08X (%u MB) from "
        "the city load%s",
        kScheduleCode + 18, source, kGrowReads, g_otherReads, kTime, kClock, static_cast<unsigned>(sizeof(kGenerators) / sizeof(kGenerators[0])),
        g_seed, kFrameMs, g_tempA, static_cast<unsigned>(reinterpret_cast<uintptr_t>(g_arena.base)),
        static_cast<unsigned>(g_arena.size >> 20), all ? "" : " WARNING: some hooks failed (above)");
}

void ScheduleBeforeCityLoad() {
    if (!g_fixed) return;
    MinimizeWindows();
    std::memset(g_seedCounts, 0, sizeof(g_seedCounts));
    ReseedGenerators();
    // rand() as the load (subsystem Init shuffles: flora, weather) finds it.
    reinterpret_cast<SrandFn>(kCrtSrand)(Derive(kLoadRandK));
    // The main thread's virtual counter starts here, at the real value, so intervals
    // that began on the start-up screens stay positive.
    LARGE_INTEGER now{};
    g_realQpc(&now);
    g_qpc = g_loadQpc = now.QuadPart;
    g_virtual = true;
    g_heap = g_heapReady;
    if (g_heap) PoolsRetire();
    if (!TimersStart()) Log("schedule: WARNING the timer scheduler was not found: its callbacks stay on the worker thread");
    const std::string trace = Setting("heap trace", "SC4_HEAP_TRACE");
    if (g_heap && !trace.empty()) {
        const std::string path = trace + "/heap-" + std::to_string(GetCurrentProcessId()) + ".bin";
        g_traceFile = CreateFileA(path.c_str(), GENERIC_WRITE, FILE_SHARE_READ, nullptr, CREATE_ALWAYS, 0, nullptr);
        const std::string max = Setting("heap trace max", "SC4_HEAP_TRACE_MAX");
        g_traceMax = max.empty() ? 8000000u : static_cast<uint32_t>(std::strtoul(max.c_str(), nullptr, 10));
        g_traceDir = trace;
        const std::string dumps = Setting("heap dump at", "SC4_HEAP_DUMP_AT");
        for (const char* at = dumps.c_str(); *at != '\0';) {
            char* end = nullptr;
            const unsigned long n = std::strtoul(at, &end, 10);
            if (end == at) break;
            if (n != 0) g_dumpAt.push_back(static_cast<uint32_t>(n));
            at = *end == ',' ? end + 1 : end;
        }
        const std::string watch = Setting("heap watch", "SC4_HEAP_WATCH");
        if (!watch.empty()) {
            char* end = nullptr;
            g_watchAt = static_cast<uint32_t>(std::strtoul(watch.c_str(), &end, 10));
            for (const char* at = *end == ':' ? end + 1 : end; *at != '\0' && g_watchCount < 4;) {
                const unsigned long off = std::strtoul(at, &end, 16);
                if (end == at) break;
                g_watchOffsets[g_watchCount++] = static_cast<uint32_t>(off);
                at = *end == ',' ? end + 1 : end;
            }
            if (g_watchCount != 0) AddVectoredExceptionHandler(1, WatchHandler);
        }
        Log("schedule: heap trace to %s (%s, up to %u operations)", path.c_str(),
            g_traceFile != INVALID_HANDLE_VALUE ? "open" : "FAILED", g_traceMax);
    }
    Log("schedule: file-local generators re-seeded, rand() set, the virtual performance counter and the fixed heap "
        "(%s; %u object pools restarted, %u earlier pages retired) started before the city load", g_heap ? "on" : "off",
        g_poolsRetired, g_pagesRetired);
    // A clock seed on the start-up screens is state the load does not reset (in every run so far, the region
    // screen's clouds, 0x007A9B66, with their allocations on the main thread).
    if (g_clockSeedSites[0] != 0) {
        Log("schedule: WARNING a clock seed at 0x%08X came before the city load: this run does not repeat",
            static_cast<unsigned>(g_clockSeedSites[0]));
    }
}

void ScheduleDayOpened(int32_t julian) {
    g_day = julian;
    g_dayDelivery = 0;
}

void ScheduleFrame() {
    if (!g_fixed || !g_virtual) return;
    InterlockedIncrement(&g_frames);
    g_qpc += g_frameTicks;
    MinimizeWindows();
    TimersFrame();
}

int64_t ScheduleQpc() {
    LARGE_INTEGER now{};
    VirtualQpc(&now);
    return now.QuadPart;
}

uint32_t ScheduleClockMs() {
    if (!g_fixed || !g_virtual || g_qpcPerMs == 0) return 0;
    return static_cast<uint32_t>((g_qpc - g_loadQpc) / g_qpcPerMs);
}

void ScheduleBeforeDelivery(const void* target, uint32_t msgType) {
    if (!g_fixed) return;
    {
        // FNV-1a over every generator's state (cRZRandom 12 bytes, cRZRandomSuper 0x9C8).
        uint32_t h = 2166136261u;
        for (const Generator& g : kGenerators) {
            const auto* b = reinterpret_cast<const uint8_t*>(g.address);
            for (size_t i = 0, n = g.super ? 0x9C8 : 12; i < n; ++i) h = (h ^ b[i]) * 16777619u;
        }
        const uint32_t randState = *CrtState();
        Log("schedule: digest %d.%u rand 0x%08X generators 0x%08X heap %u 0x%08X target vt 0x%08X msg 0x%08X", g_day,
            g_dayDelivery, randState, h, g_arena.ops, g_arena.hash, *reinterpret_cast<const uint32_t*>(target), msgType);
    }
    // [choice] rand() state for the n-th delivery of day d: S XOR (d * 0x9E3779B1) XOR (n * 0x85EBCA77).
    const uint32_t state = g_seed ^ (static_cast<uint32_t>(g_day) * 0x9E3779B1u) ^ (g_dayDelivery * 0x85EBCA77u);
    ++g_dayDelivery;
    reinterpret_cast<SrandFn>(kCrtSrand)(state);
    InterlockedIncrement(&g_srands);
}

void ScheduleMonthLine(int32_t julian) {
    if (!g_fixed) return;
    TraceFlush();
    Log("schedule: month at julian %d: time limits started %ld, read %ld (+%ld unstarted, never expire), Grow "
        "expiries %ld, other expiries %ld; "
        "time() calls %ld; clock() virtual %ld, real %ld; performance counter virtual %ld, real (other threads) %ld; "
        "frames %ld, sleeps %ld (%ld ms); timer callbacks %ld on the main thread; stopwatches rebased %ld; transit switches zeroed %ld; intersections zeroed %ld; train junctions zeroed %ld; foreground tests %ld (all false); windows minimized %ld, again %ld; clock seeds derived %ld, kept %ld; rand() set %ld times; save references %ld in %ld save(s); address-ordered nodes walked in creation order %ld (%ld keys without a stamp, %ld gone before their turn, %ld tree steps without a walk); heap %u operations "
        "(0x%08X), %u MB used, %u MB committed, %ld blocks freed on other threads, %ld fallbacks, %ld foreign frees",
        julian, g_starts, g_reads, g_unstartedReads, g_growExpiries, g_otherExpiries, g_timeCalls, g_clockVirtual, g_clockReal, g_qpcVirtual, g_qpcReal, g_frames, g_sleeps, g_sleptMs, g_timerFires, g_stopwatchesRebased, g_switchesZeroed, g_intersectionsZeroed, g_junctionsZeroed, g_backgrounds, g_minimized, g_reminimized, g_seedsDerived, g_seedsClock, g_srands,
        g_refs, g_saves, g_ordered, g_unstamped, g_gone, g_treeLost, g_arena.ops, g_arena.hash, static_cast<unsigned>(g_arena.top >> 20),
        static_cast<unsigned>(g_arena.committed >> 20), g_heapLeaks, g_heapFallbacks, g_heapForeign);
    if (!g_unstampedClasses.empty()) {
        std::string line;
        for (const auto& [k, n] : g_unstampedClasses) {
            char one[48];
            std::snprintf(one, sizeof(one), " %s 0x%08X x %u", k.first ? "pool" : "outside", k.second, n);
            line += one;
        }
        Log("schedule: keys without a stamp by first word:%s", line.c_str());
    }
    if (g_censusOn) {
        std::vector<std::pair<uint32_t, uint32_t>> sites;  // walks by address, caller
        for (const auto& [at, site] : g_census) {
            if (site.byAddress != 0) sites.emplace_back(site.byAddress, at);
        }
        std::sort(sites.rbegin(), sites.rend());
        std::string line;
        for (const auto& [n, at] : sites) {
            const CensusSite& site = g_census[at];
            char one[80];
            std::snprintf(one, sizeof(one), " 0x%08X %u/%u (%u keys, order %08X)", at, site.byAddress, site.walks, site.keys,
                          site.order);
            line += one;
        }
        Log("schedule: walk census (caller, walks by address/walks, keys): %u callers;%s", static_cast<unsigned>(g_census.size()),
            line.c_str());
    }
}

}  // namespace fc4
