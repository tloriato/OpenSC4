# Scenario UI drivers

The DLL accepts these opt-in dated `.script` actions in the test slot. UI
actions run at framework ticks, in file order, separated by at least 150 ms.
`uiwait` uses wall time, so a paused modal window can still receive input.
Actions following a UI action wait behind it. Modal loops can pump input,
but nested commands and simulator/save actions wait until the command returns.

| Verb | Arguments | Delivery |
|---|---|---|
| `uicommand` | `0x12345678 [0\|1]` | Registered ID through command-server slot 20, with separate input/output sets; optional boolean parameter 0. |
| `uimouse` | `x y move\|down\|up\|click` | Client pixel coordinates; posted mouse messages. |
| `uicell` | `x z height move\|down\|up\|click` | Cell centre `(16*x+8,height,16*z+8)` projected by view utilities, then converted to main-window pixels. Height is metres above datum. |
| `uicontrol` | `0x12345678 [native]` | Find a visible enabled child and click its centre; `native` dispatches canvas mouse records through the window manager's hit testing and capture. |
| `uikey` | `1..255` | Win32 virtual-key down/up, including scan and transition bits. |
| `uitext` | `printable ASCII text` | Up to 256 WM_CHAR characters, preserving internal spaces. No implicit Enter. |
| `uiwait` | `0..60000` | Delay subsequent actions by milliseconds; never sleeps the main thread. |

Mouse input targets the current process's `cIGZFrameWorkW32::GetMainHWND()`; it neither moves
the desktop pointer nor changes another slot's focus. Points outside the client
are refused. Use separate down/move/up actions for road drags. Projection is
restricted to the identified supported game image and a live city view.
Unknown command IDs are refused. `status=0` and `posted` prove delivery only:
qualification requires target-method reach and a clean verdict with no reference
warnings. `ui_command_catalog=1` logs registered IDs/names/parameters on the first
command for diagnosis; it does not execute the catalogue.

An unfounded god-mode city has no new-day messages. Put `establish <city name>`
first, dated at the city's saved date, before funded construction or UI input.
This opt-in action calls the native city's `SetEstablished(true)`, which posts
the establishment message and enables the simulator's established-city agents.
The leading action and its same-date setup can run on a framework tick; later
dates still require actual new days. `funds <amount>` then supplies construction
money through the budget interface. Neither a queued script nor a load-only
verdict proves that the city was founded or that placements succeeded.

For a dated-save region driver, save before `uicommand 0x6a935e39 1`
(`QuitCity`, boolean parameter 0 = do not ask). An empty parameter set asks for
confirmation; automatic dialog closing can cancel it while delivery still
returns status 0. Qualify the transition with region-screen method entries.

`budgetwindow` delivers the city-view advice link. Its expanded-tab button is
posted to the window manager, so the action logs delivery, not immediate panel
visibility. Wait before selecting a visible row, and qualify sub-budget entry
from reach. `uicontrol ... native` dispatches the canvas mouse records directly
and preserves the native window manager's hit testing and button notifications.

`uicontrol` refuses a control if it or any parent is hidden or disabled. A
resource can contain overlapping hidden and visible buttons with different IDs.
The budget panel must first be expanded with `aa3ac50e`; its visible education,
utilities and ordinance rows are `aa3ac507`, `aa3ac508` and `aa3ac501`
(budget-window.md section 2.1). Clicking the hidden `aa3ac4xx` counterparts does
not exercise a sub-budget. Closing an already hidden panel is a refused action.

The `full,ui29` suite rows contain the house-city, West Weburg and Bevopolis
censuses plus the volcano observation. Final-DLL runs `reach-ui29-p18`, `p22`,
`p20` and `p23` respectively completed 50, 22, 27 and 3 actions with clean verdicts,
zero reference warnings and 40/40 exact framework shadow samples. Their target
entry evidence is:

| Path | Entered methods |
|---|---|
| Query, text, budgets (p18) | Query OnMouseDownL/DescribeBuilding, TextEdit insertion, SubBudget lifecycle. |
| Map layer (p18) | WinMapView ShowWindow, DisplayStandardUint8Grid and DisplayStandardSint16Grid. |
| Road drag (p22) | NetworkTool OnMouseDownL/OnMouseUpL plus PlaceNetwork/PlaceNetworkOccupants; no saved-network assertion. |
| News expansion (p20) | DoWinProcMessage entered; PurgeInactive (`00772ee0`) first entered in window 7 (2257-08), containing the news button at 2257-08-03; news-ticker.md section 3 assigns the purge to expansion. |
| Volcano (p23) | 26 of 28 traced cSTEVolcanoLava methods, 7,306 entered-function bytes. |

Periodic NewsTickerWinProc display also entered in p18; p20 adds the separate
expansion evidence above. The final Bevopolis script allows startup UI to settle
and sends Escape before selecting query. Its cell 63,63 at height 0 posts at
client 478,354 without a query mouse-handler entry, as did the earlier height-250
candidate at 478,198. Cell projection therefore still has delivery evidence only;
Bevopolis qualifies news input, not a query click at a cell or an accurate terrain
intersection. No cell-targeting coverage gain is claimed.
The final West Weburg run uses the suite's `[ini] close_dialogs = 1`: it closed
two dialogs and completed. Earlier p19 stalled before its script without that
setting; p21 was rejected before launch because the setting was outside `[ini]`.
The registered `ViewWebBrowser` command refuses an empty parameter set (status
2); no web coverage follows from its registration.

The censuses pause simulation before UI actions and resume afterwards. Modal
dialogs and the harness's initial unpause can resume the clock. Both end-date
exit modes therefore drain queued/active UI actions and the final input delay
before exiting; the wall deadline, watchdog and external timeout remain active.
The house census also tries the installed data-view tab `99887755` and its
air-pollution layer `0000500e`. A refused control remains a failed action.

`disaster volcano x z` extends the existing disaster verb with explicit cells.
It asks disaster manager `C4C90997` to start at world metres `(16*x+8,16*z+8)`
(disasters.md section 10). It neither selects an interactive tool nor owns the
returned instance. Unlike fire/riot, volcano does not accept `auto`.
The volcano candidate waits 55 seconds of wall time because lava lasts 50
animation seconds; turbo game dates cannot substitute for that interval.

The attempted import-template driver was removed after corrupt run
`reach-ui29-p7`; it supplies no qualified tree coverage. Questions UI29-TREE
and UI29-HTTP in the corresponding spec question files block tree-template
and public WWW LoadDocument drivers. No browser-launch command is qualified
as web/HTTP coverage.

Final reconciliation of UI29-TREE: ui-script-service.md section 3 specifies a
control-set element ID as slot 4's third argument, with lookup after executing
the resource. A null result alone does not establish factory failure. Safe
cleanup and import-tree initialization remain open; the removed candidate is
not reinstated. UI29-HTTP still lacks an ordered public call signature.

The installed accelerator resource `{A2E3D533,6A231EAA,6A9362EF}` supplies query
`6a935cf3`, road `6a935dc0`, MySim `6a935ca8`, mayor `6a935ca4`, play/graphics/audio
options `6a935e2a/6a935e2d/6a935e34`, and close `6a935cf1`.
The general accelerator resource with instance `6A9362F1` supplies cheat dialog
`2a9496c9`. Installed UI resources contain budget button `00000041` and news
button `ca2aedcc`. Resource contents and extracted game data stay uncommitted.

`reach_startup=1` arms tracing immediately after the director's entry snapshot,
before subsystem registration and application init. Its sentinel date is
1999-01-01; `reach_lifecycle=1` opens the 2000-01-01 load window, followed by the
saved city's real date. This cannot capture anything before the DLL director
is constructed. Use a plain run to distinguish natural startup from setup probes.
