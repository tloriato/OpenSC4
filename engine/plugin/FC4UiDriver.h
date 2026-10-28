// SPDX-License-Identifier: GPL-3.0-or-later
#pragma once
// Opt-in scenario actions. Game interfaces own routing, focus and tool state.
#include "FC4HarnessLogic.h"
#include "FC4WinInterfaces.h"
#include "FC4RefAudit.h"
#include "FC4Config.h"
#include "FC4Log.h"
#include "compat/FC4Compat.h"
#include "cIGZApp.h"
#include "cIGZFrameWork.h"
#include "cIGZFrameWorkW32.h"
#include "cIGZCommandServer.h"
#include "cIGZCommandParameterSet.h"
#include "cIGZVariant.h"
#include "cGZMessage.h"
#include "cISC4App.h"
#include "cISC4View3DWin.h"
#include "cISC43DRender.h"
#include "cRZBaseString.h"
#include "cRZCOMDllDirector.h"
#include "SC4Vector.h"
#include <windows.h>
#include <cmath>
#include <cstring>

namespace fc4 { namespace ui_driver {
inline bool IsAction(harness::ScriptOp op) {
    using harness::ScriptOp;
    return op == ScriptOp::UiCommand || op == ScriptOp::UiMouse || op == ScriptOp::UiCell ||
        op == ScriptOp::UiKey || op == ScriptOp::UiText || op == ScriptOp::UiWait || op == ScriptOp::UiControl;
}

// The framework supplies its actual host window; another slot may have the
// same title, and the host need not have a particular Win32 class name.
inline HWND GameWindow() {
    auto* fw = RZGetFrameWork();
    cIGZFrameWorkW32* win32 = nullptr;
    if (!fw || !fw->QueryInterface(GZIID_cIGZFrameWorkW32,
            reinterpret_cast<void**>(&win32)) || !win32) return nullptr;
    FC4_ACQUIRED(win32, "UI driver Win32 framework");
    // [abi-ret] public SDK cIGZFrameWorkW32::GetMainHWND returns HWND.
    HWND window = win32->GetMainHWND();
    FC4_RELEASE(win32);
    DWORD pid = 0;
    GetWindowThreadProcessId(window, &pid);
    return pid == GetCurrentProcessId() ? window : nullptr;
}

inline bool InputClientRect(HWND window, RECT& client) {
    if (!IsIconic(window)) return GetClientRect(window, &client) != 0;
    // The fixed schedule deliberately minimizes the window (simulator.md 5.8).
    // Its current client can be empty, but posted input still uses the normal
    // client coordinates. Recover that extent without restoring/painting it.
    WINDOWPLACEMENT placement{};
    placement.length = sizeof(placement);
    RECT frame{};
    if (!GetWindowPlacement(window, &placement) ||
        !AdjustWindowRectEx(&frame, static_cast<DWORD>(GetWindowLongA(window, GWL_STYLE)),
                            GetMenu(window) != nullptr,
                            static_cast<DWORD>(GetWindowLongA(window, GWL_EXSTYLE)))) return false;
    const auto& normal = placement.rcNormalPosition;
    client = RECT{0, 0, normal.right - normal.left - (frame.right - frame.left),
                       normal.bottom - normal.top - (frame.bottom - frame.top)};
    return client.right > 0 && client.bottom > 0;
}

inline std::string Command(uint32_t id, int32_t parameter = -1) {
    auto* fw = RZGetFrameWork();
    cIGZCommandServer* server = nullptr;
    if (!fw || !fw->GetSystemService(0xEB903A32, 0xCB90253A,
            reinterpret_cast<void**>(&server)) || !server) return "SKIPPED: no command server";
    FC4_ACQUIRED(server, "UI driver command server");
    static bool catalogued = false;
    if (!catalogued && SettingInt("ui_command_catalog", nullptr, 0) == 1) {
        catalogued = true;
        SC4Vector<uint32_t> ids;
        // SPEC-GAP: enumeration's return is unspecified; use only the filled IDs.
        server->EnumerateCommands(ids);
        for (uint32_t command : ids) {
            cRZBaseString commandName, parameters;
            // [abi-ret] command-system.md section 5: false for unknown ID, true otherwise.
            if (server->GetCommandInfo(command, commandName, parameters))
                Log("ui driver catalog id=%08x name=%s args=%s", command, commandName.ToChar(), parameters.ToChar());
        }
    }
    cRZBaseString name, info;
    // [abi-ret] command-system.md section 5 specifies this boolean lookup result.
    const bool known = server->GetCommandInfo(id, name, info);
    cIGZCommandParameterSet* in = nullptr;
    cIGZCommandParameterSet* out = nullptr;
    if (known) {
        server->CreateCommandParameterSet(&in);
        server->CreateCommandParameterSet(&out);
    }
    if (in) FC4_ACQUIRED(in, "UI driver input set");
    if (out) FC4_ACQUIRED(out, "UI driver output set");
    std::string result = "SKIPPED: unknown command or unavailable parameter sets";
    if (in && out) {
        // command-system.md section 1.3: a fresh set numbers its first value 0.
        // QuitCity reads this boolean as "do not ask" (view3d-window section 6).
        if (parameter >= 0) {
            in->SetParameterCount(1);
            auto* value = in->GetParameter(0); // borrowed from the set
            if (value) value->SetValBool(parameter != 0);
        }
        // command-system.md section 5: SDK overload order differs; ID is slot 20.
        using Execute = int32_t (__thiscall*)(void*, uint32_t, cIGZCommandParameterSet*, cIGZCommandParameterSet*);
        auto execute = reinterpret_cast<Execute>((*reinterpret_cast<void***>(server))[20]);
        const int32_t status = execute(server, id, in, out);
        result = "status=" + std::to_string(status) + " name=" + name.ToChar();
        if (parameter >= 0) result += " bool0=" + std::to_string(parameter);
        // Status zero is only delivery, never a reach/semantic qualification.
    }
    if (out) FC4_RELEASE_CREATED(out);
    if (in) FC4_RELEASE_CREATED(in);
    FC4_RELEASE(server);
    return result;
}

inline bool CellPixel(const harness::ScriptAction& a, int& x, int& y) {
    if (!compat::IdentifiedVersion()) return false;
    auto* fw = RZGetFrameWork();
    auto* app = fw ? fw->Application() : nullptr; // borrowed
    cISC4App* sc4 = nullptr;
    if (!app || !app->QueryInterface(GZIID_cISC4App, reinterpret_cast<void**>(&sc4)) || !sc4) return false;
    FC4_ACQUIRED(sc4, "UI driver app");
    auto* main = reinterpret_cast<cFC4IGZWin*>(sc4->GetMainWindow()); // borrowed
    auto* shell = main ? main->GetChildWindowFromID(0x6104489A) : nullptr;
    cISC4View3DWin* view = nullptr;
    bool ok = false;
    if (shell && shell->GetChildAs(0x9A47B417, 0xFA47B3F9, reinterpret_cast<void**>(&view)) && view) {
        FC4_ACQUIRED(view, "UI driver city view");
        auto* renderer = view->GetRenderer(); // borrowed
        void* utilities = renderer ? reinterpret_cast<void*>(renderer->GetViewUtilities()) : nullptr;
        if (utilities) {
            // view-utilities.md sections 4 and 13.1: mutable float32 triple,
            // thiscall, pop 4, void. Explicit height is metres above datum.
            float point[3] = {a.x0 * 16.0f + 8.0f, a.height, a.z0 * 16.0f + 8.0f};
            using Project = void (__thiscall*)(void*, float*);
            reinterpret_cast<Project>(0x007F7850)(utilities, point);
            if (std::isfinite(point[0]) && std::isfinite(point[1]) &&
                point[0] >= 0 && point[0] <= 32767 && point[1] >= 0 && point[1] <= 32767) {
                x = static_cast<int>(point[0]); y = static_cast<int>(point[1]);
                auto* win = reinterpret_cast<cFC4IGZWin*>(view->AsIGZWin());
                if (win) {
                    // widget-toolkit.md section 5.1: in/out coordinates, no return needed.
                    win->WindowToScreenCoordinates(x, y);
                    ok = true;
                }
            }
        }
        FC4_RELEASE(view);
    }
    FC4_RELEASE(sc4);
    return ok;
}

inline bool ControlPixel(uint32_t id, int& x, int& y) {
    auto* fw = RZGetFrameWork();
    auto* app = fw ? fw->Application() : nullptr;
    cISC4App* sc4 = nullptr;
    if (!app || !app->QueryInterface(GZIID_cISC4App, reinterpret_cast<void**>(&sc4)) || !sc4) return false;
    FC4_ACQUIRED(sc4, "UI driver app");
    auto* main = reinterpret_cast<cFC4IGZWin*>(sc4->GetMainWindow());
    auto* child = main ? main->GetChildWindowFromIDRecursive(id) : nullptr; // borrowed
    if (!child) Log("UI control %08X unavailable: child not found (main=%p)", id, static_cast<void*>(main));
    bool ok = false;
    bool visible = child != nullptr;
    for (auto* ancestor = child; ancestor; ancestor = ancestor->GetParentWin()) {
        // [abi-ret] widget-toolkit.md section 3: GetFlag returns (flags & mask) != 0.
        if (!ancestor->GetFlag(1) || !ancestor->GetFlag(2)) {
            Log("UI control %08X unavailable: ancestor %08X visible=%u enabled=%u",
                id, ancestor->GetID(), ancestor->GetFlag(1) ? 1u : 0u, ancestor->GetFlag(2) ? 1u : 0u);
            visible = false;
            break;
        }
    }
    if (visible && child->GetW() > 0 && child->GetH() > 0) {
        x = child->GetW() / 2; y = child->GetH() / 2;
        child->WindowToScreenCoordinates(x, y); // consume coordinates, not a leftover AL
        ok = true;
    } else if (visible) {
        Log("UI control %08X unavailable: area %dx%d", id, child->GetW(), child->GetH());
    }
    FC4_RELEASE(sc4);
    return ok;
}

inline std::string Run(const harness::ScriptAction& a) {
    using harness::ScriptOp;
    if (a.op == ScriptOp::UiWait) return "wait-ms=" + std::to_string(a.type);
    if (a.op == ScriptOp::UiCommand) return Command(a.command, a.commandBool);
    if (a.op == ScriptOp::UiControl && a.nativeControl) {
        int x = 0, y = 0;
        if (!ControlPixel(a.command, x, y)) return "SKIPPED: control unavailable";
        auto* fw = RZGetFrameWork();
        cFC4IGZWinMgr* manager = nullptr;
        if (!fw || !fw->GetSystemService(kFC4_SRVID_cIGZWinMgr, kFC4_IID_cIGZWinMgr,
                reinterpret_cast<void**>(&manager)) || !manager) return "SKIPPED: no window manager";
        FC4_ACQUIRED(manager, "UI driver native mouse queue");
        // widget-winmgr.md section 6.1: normal hit testing, capture and focus.
        // Dispatch the canvas's mouse record, not the Win32 conversion in
        // GZWinMsgPostMouse (section 5); modifiers occupy its high 16 bits.
        const auto dispatch = [&](uint32_t type, uint32_t buttons) {
            // ProcessCursorMessage converts the record's coordinates in place
            // (widget-toolkit.md section 15.1). Each event needs screen coords.
            cGZMessage mouse(type);
            mouse.dwData1 = static_cast<uint32_t>(x);
            mouse.dwData2 = static_cast<uint32_t>(y);
            mouse.dwData3 = buttons << 16;
            manager->ProcessMouseMessage(mouse);
        };
        dispatch(13, 0);
        dispatch(7, 0x10);
        dispatch(10, 0);
        FC4_RELEASE(manager);
        return "delivered native mouse (qualify with reach) client=" + std::to_string(x) + "," + std::to_string(y);
    }
    HWND window = GameWindow();
    if (!window) return "SKIPPED: no process-local framework window";
    bool ok = true;
    if (a.op == ScriptOp::UiKey) {
        const LPARAM scan = static_cast<LPARAM>(MapVirtualKeyA(a.type, MAPVK_VK_TO_VSC)) << 16;
        ok = PostMessageA(window, WM_KEYDOWN, a.type, scan | 1) != 0;
        ok = (PostMessageA(window, WM_KEYUP, a.type, scan | 0xC0000001u) != 0) && ok;
    } else if (a.op == ScriptOp::UiText) {
        for (unsigned char c : a.text) ok = (PostMessageA(window, WM_CHAR, c, 1) != 0) && ok;
    } else {
        int x = a.x0, y = a.z0;
        if (a.op == ScriptOp::UiCell && !CellPixel(a, x, y)) return "SKIPPED: cell projection unavailable";
        if (a.op == ScriptOp::UiControl && !ControlPixel(a.command, x, y)) return "SKIPPED: control unavailable";
        RECT client{};
        if (!InputClientRect(window, client) || !PtInRect(&client, POINT{x, y}))
            return "SKIPPED: point outside game client";
        // SPEC-GAP: scenario mouse phases use posted Win32 client input; no
        // focus stealing or host cursor movement is needed for isolated slots.
        static bool down = false;
        const LPARAM point = MAKELPARAM(x, y);
        ok = PostMessageA(window, WM_MOUSEMOVE, down ? MK_LBUTTON : 0, point) != 0;
        if (a.kind == 1 || a.kind == 3 || a.op == ScriptOp::UiControl) {
            ok = (PostMessageA(window, WM_LBUTTONDOWN, MK_LBUTTON, point) != 0) && ok;
            down = true;
        }
        if (a.kind == 2 || a.kind == 3 || a.op == ScriptOp::UiControl) {
            ok = (PostMessageA(window, WM_LBUTTONUP, 0, point) != 0) && ok;
            down = false;
        }
        return std::string(ok ? "posted" : "FAILED: PostMessage") + " client=" + std::to_string(x) + "," + std::to_string(y);
    }
    return ok ? "posted (delivery only; qualify with reach)" : "FAILED: PostMessage";
}
} } // namespace fc4::ui_driver
