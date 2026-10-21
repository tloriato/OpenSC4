| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| glue-terrain3 (enable = terrain glue over the TR21 3D view contract, terrain.md §15.5) | `glue-terrain3` / (engine lane) | wip (run4 round 3): stall cause found (no TerrainReconciliation -> popups never disabled -> modal dialog); fix not yet coded | implement §12.1 DoAnimationTick + 0x2A612643 + slot 89 (foundations §11.6), step 7 part 2, TR37, Save |

## Handoff (round 3)
- Goal: enable = terrain clean to 2032 (turbo), ref_warnings=0; then normal speed; full gate.
- Done (all diagnostic, no behaviour change): 1e78a7ca..333b30c7 bounded terrain trace in cFC4Terrain.cpp
  (namespace trace: ring of locked-slot enter/leave, DoMessage, HostPost words, RedisplayTerrainRect caller) and a
  stall monitor thread (no SimNewDay for 2.5 s: ring + every thread's stack, kernel-object probe, twice).
- ROOT CAUSE (runs gt3-r3-d1..d8): not a lock. Terrain lock free, no thread in terrain code. Main thread sleeps
  (NtDelayExecution) in a game modal-dialog loop (frames 0x99E3DE/0x99E425 = inside cGZWin::ChildToFront,
  0x9DA900 next to cGZWinMgrW32::GZWinMsgPost, docs/abi). gt3-r3-d8 (close_dialogs = 1, diagnostic only):
  clean to 2032, dialogs_closed=9 (all 0x4A9DB60C, first 2027-02-01), ref_warnings=1 (not yet looked at).
  Why: the harness disables popups only at ConnectionsReady 0x6AC284F3; control gt3-r2-ctl-1 logs "popup dialogs:
  disabled ... at ConnectionsReady"; gt3-r3-d7 logs "WARNING no ConnectionsReady within 30 game day(s)".
  ConnectionsReady follows TerrainReconciliation 0xCA63D517, which only the terrain posts (foundations.md
  §11.6, lines 1384-1458; terrain.md §12.1 lines 1303-1323). Our DoMessage has `case kMsgAnimationTick: break;`
  and `case kMsgReconcile: break;` and slot 89 ReconcileEdgesWithNeighboringCities is `{}` (line ~961).
- Next (exact): in cFC4Terrain.cpp DoMessage: kMsgAnimationTick -> if f_.view && f_.reconcileTick < 2 and
  ++f_.reconcileTick == 2: preference (cISC4App::GetPreferences() +0x4EC, default 1; width unspecified -> file
  TR question) == 0 ? post 0xCA63D517 (all 0, MessagePost queued) : GeneralMessagePostToTarget(0x2A612643,
  Target()). kMsgReconcile -> winmgr = GetSystemService(0xA417445E, IID 0x5A4) (FC4WinInterfaces.h
  cFC4IGZWinMgr, slot 42 IsModal); none -> stop; modal -> re-post 0x2A612643; else slot 89 (0,1,1).
  Slot 89: return if !cISC4App GetPopupDialogsEnabled (slot 23; see how FC4Popups.cpp calls it); edge compare
  ComputeEdgeVerticesToReconcile is unspecified -> file a numbered TR question, literal reading "no mismatch";
  then post 0xCA63D517 data1=a, data2=0, data3=0, data4=0. Then run a new unique label (gt3-r3-f1, no
  close_dialogs), then normal speed, then qbuild all; then step 7 part 2 / TR37 / Save if budget allows.
- Facts: labels used gt3-r3-d1..d8 (d2 ran the d1 DLL). Symbolize our frames with VS llvm-symbolizer
  (VC/Tools/MSVC/14.37.32822/bin/Hostx64/x64) --obj build-x86/FreeCity4.dll, address 0x10000000+off-1.
  Remove or keep the trace monitor before ready (it is diagnostic; keep it cheap). Gate not re-run this round.

## Round 2 report (run4 round 2, 2026-10-21)

Goal: `enable = terrain` clean to 2032 under turbo with ref_warnings=0. **Not reached; not ready.**

Integration: main merged in (c5e6b495). questions/terrain.md is main's copy; all of TR33-TR41 are answered.

Code (ed29ba77, engine/plugin/cFC4Terrain.cpp and compat/FC4Compat.*; no core or ABI changes):
- TR38, the cause found: HostPost called SetVoid1(sender). void1 is data1 (+0x10), so the view read the
  sender's address as the rectangle. Posts now carry only the type and data1-4, in a fresh message each time.
  A heights or water rectangle outside the grid is refused and logged (§15.5.11 item 19).
- TR40: Init calls cSTETerrain::StaticInit (0x0074FE40, a new Direct entry) before the view exists.
- TR39: Read checks version <= 2, then calls SC4GetIGZDBSegment (0x0045BA70, Direct) and then slot 96 Load,
  and returns Load's result. Init keeps a grid that Load installed when its size matches the city.
- TR35: Shutdown calls the view's Shutdown (0x007560F0, Direct), Releases the view and clears view-ready.
- TR34: step 6 calls the weather simulator's slot 5 (DoCompleteSimulation; the SDK names it DoPostCityInit).
- TR33/TR41: step 7 no longer runs RedisplayTerrain. Part 1 is in place (normals plus MarkCliff, §15.5.13.2
  and §15.5.13.6; T = the core's shipped cliff_slope). Part 2 is **not implemented**: the texture-layer update
  of §15.5.13.3 and §15.5.13.6, so record +0x16 stays 0.

Game evidence:
- gt3-r2-1 and gt3-r2-2 (ed29ba77, enable = terrain, turbo): the 128x128 saved grid loads in Read; StaticInit
  returns 1; view Init returns 1; no fault; ref_warnings=0. Both runs end `stalled`, at 2026-06-03 and
  2026-06-09. Both stall with the same stack: fc4_sim_on_tick -> FreeCity4.dll+0x81BAE0 -> game 0x52E0F5 ->
  0x36361F -> 0x5DA900 -> ntdll. The game date stops advancing several seconds before the watchdog fires.
- gt3-r2-ctl-1 (the same build without the replacement): clean to 2027-01-15. **So the stall is caused by the
  terrain replacement.**

Gate (ed29ba77): qbuild all PASS. cargo 3578 passed, 0 failed; clippy 0; ctest 62/62.

Questions: none filed this round.

Next steps:
1. Find what the sim agent at game 0x52E0F5 waits on with the replaced terrain. Log the terrain slots that are
   called around the stall, for example a slot that returns a value the caller loops on.
2. Step 7 part 2 (§15.5.13.3 steps 1-8 and §15.5.13.6). It reads the StaticInit statics in place, so it needs
   compat data addresses, and calls the view's 0x00755ED0, 0x007574A0 and 0x007571D0.
3. The TR37 call SetTriangleIndicesForCell (0x00752AF0, args (z, x), ECX = view base) in slots 93/94, the
   hide slots and SetTerrainCellFlags, behind the +0xEC guard.
4. Save (slot 97) is still not wired.
