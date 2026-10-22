// SPDX-License-Identifier: GPL-3.0-or-later
#pragma once
namespace fc4 {
// Opt-in verification only: no application/view installation contract (RP7).
void RenderPropertiesStart();
void RenderPropertiesCheck(const char* phase, bool setters);
void RenderPropertiesStop();
}
