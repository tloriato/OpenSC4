// SPDX-License-Identifier: GPL-3.0-or-later
#pragma once
#include <cstdint>
struct FC4LuaOwner;
struct FC4LuaCall { uint32_t op; uint64_t handle; uint64_t other; uint32_t number; };
using FC4LuaCallback = bool (*)(void*, const FC4LuaCall*, uint64_t*);
extern "C" {
FC4LuaOwner* fc4_luascripts_new();
void fc4_luascripts_free(FC4LuaOwner*);
bool fc4_luascripts_call(FC4LuaOwner*, FC4LuaCallback, void*, uint32_t, int32_t, uint32_t, int64_t*);
}
namespace fc4 {
// Public-interface trace only. Does not replace an existing service or publish
// the incomplete owner factory. All entry points require enable=luascripts.
void LuaScriptsTrace(const char* phase);
}
