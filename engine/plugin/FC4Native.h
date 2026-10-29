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

// fc4native.dll (engine/native): drop-in replacements ("ports") for the game's own functions.
//
// FreeCity4.ini:
//
//   [native]
//   functions = 006bd540, 006bd810    ; or owner names (landvalue), or all
//   dll = C:\path\to\fc4native.dll     ; optional; default: next to FreeCity4.dll
//
// (the flat keys `native functions` / `native dll` work too; environment FC4_NATIVE_FUNCTIONS /
// FC4_NATIVE_DLL when the ini has neither). Only these two keys belong to the section: any other key is
// global wherever it stands, as before. List items are separated by commas, semicolons or spaces; an item
// is a port's 8-hex-digit original address, an owner name (every port of that owner), or `all`.
//
// When `functions` is set, NativeStartup (at game start, after the executable is identified as the
// 1.1.641 build the ports are written for) loads the DLL, reads its port table (`fc4native_ports`),
// checks each enabled port's export (`fc4p_<address>`), and writes a 5-byte `jmp` to it at the original's
// entry in memory (the executable on disk is never changed). One log line per jump, with the entry bytes
// it replaced, and a summary. The optional `fc4native_sites` table (FC4S, version 1, entries of
// {function, site}, two uint32_t words) declares the schedule sites a port honours. The loader refuses a
// function containing an undeclared site; a return key also needs its `fc4r_<site>` export. The original
// `fc4native_ports` v1 layout is unchanged, and older DLLs without a site table remain supported.
// When `functions` is absent or empty, nothing is loaded, nothing is written
// and nothing is logged.
#pragma once

namespace fc4 {

// See above. Called once from the subsystem resolve at game start.
void NativeStartup();

}  // namespace fc4
