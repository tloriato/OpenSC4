| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| re-view3dwin4: cSC4View3DWin window spec (view3d-window.md) | `re-view3dwin4` / (RE spec lane) | ready (round 3, final; owner still partial): §6c input handlers (keys, character, all mouse slots), §6b AddGUI tail and +0x190/+0x194, VW6 narrowed, tests VW-T7..T9; docs-only, no gate, no game runs | 1. AddGUI per-panel extras 0x007EE39B-0x007EE610. 2. VW6 rest (+0x2EC/+0x2F0 creator, 0x00B43C9C identity). 3. VW2-VW5. 4. 8 ABI rows stay proposals (gated review) |

## Round 3 report (final)

What changed (docs/specs/view3d-window.md):

- New §6c, all byte-checked on the instructions: the state the handlers use (modifiers `+0x11C`, control stack
  `+0x134`, persistent controls `+0x140`, key-scroll pair `+0x348/+0x34C`, edge directions `+0x350/+0x354`,
  right-press point `+0x358/+0x35C`, drag flag `+0x360`); a dispatch table for ProcessKeyDown/Up, GZOnCharacter,
  MouseDownL/DownR/UpL/UpR/Move/Wheel/Exit (order stack vs persistent, first-taker vs broadcast, capture, return
  values); tutorial filter modes -1/1/0; mouse-scroll start (cSC4ViewInputControlCityScroll, ownership); left-release
  scroll re-check; right-release click-to-centre (Pick / PickTerrain, camera jump 0x007CD810, discarded terrain
  reads quirk, 3-pixel drag threshold, mods & 7); edge scrolling (pref byte +0xF09 of 0x00B43CD8, x<=1 / x>=W-1);
  GZOnCharacter bounds quirk; key results include persistent controls (the decomp misses 0x007E67A6).
- §6 key-binding helper offset corrected to `+0x120` (0x007F3F39 is relative to this+0xC).
- §6b: main panel cIGZWinGen into +0x190, hidden button +0x194 (class 0x22ECFC47), and the AddGUI tail
  0x007EED41-0x007EF1B6 (callout box styles/image, cGZWinBMP + surface +0x3D0 128x32, +0x31C, unstored overlay
  window 0xCA5D3294 ID 0x6A5E44B6, winproc for window 0xEA8CAD14, not-established step 7). Corrected round 2's
  claim that +0x1F4 is an object (AddGUI only clears it).
- VW6 narrowed, still open; QUESTIONS.md reindexed. Tests VW-T7..T9.
- Commits: 9f656f88 (§6c), 445ba2a6 (§6b tail, VW6), this one (report).

REA not used (export + decomp + exe bytes; helpers build-runs/casm.sh, arange.sh, vt.py).

Owner coverage: still partial. Remaining exact functions not specified: AddGUI per-panel extras
0x007EE39B-0x007EE610 (cases for windows 0xE9889775 / 0xABB26B0E, lists +0x198/+0x1B0/+0x1BC, 0x00442E40,
0x007EBD30, 0x0078F3C0, 0x0078E610; MySim winproc 0x0076F160 at +0x2E4); RecalcScrollVelocity 0x007E6C60;
UpdateCursorTextPositions 0x007E9090; Set/RemoveCurrentViewInputControl 0x007EBD70/0x007E9390 and
SendViewInputControlChangeMessage 0x007E8090; SetKeyAccelerator 0x007E5700; GZOnCommand 0x007E5BA0; message-filter
branches (VW4); VW2, VW3, VW5. ABI proposals (build-runs/abi-rows-re-view3dwin4.tsv) left for gated review.

## Round 2 report

What changed (docs/specs/view3d-window.md):

- §3.1 `ProcessCheatCode` (`0x007F3510`), walked on the instructions: call path (window message `0x230E27AC` →
  `cRZCmdLine` from data2 → `this − 0xC`; result = message consumed), per-cheat table for all 13 IDs with argument
  rules and return values; the two filter cheats (`TastyZots` toggles realistic-filter `+0xD`, `Zoneria` toggles `+0xC`
  and clears `+0xE`, both return false *[quirk]*); `SizeOf` clamp [0.9, 64] and the camera control store at
  `0x007CD6E0` (bounds test the old value); `RenderProp`/`RP` argument forms and the usage text through an empty slot
  45 stub; `Recorder` = `AddWinAnimationRecorder` (child `0xA802B4EB`, `<UserData>RecordedAnimations\`).
- VW1 answered DIFFERS (round-1 notes said the byte-flag cheats return IDs; they return true; 0.9/64 are SizeOf's
  clamp). Moved to questions/answered/view3d-window.md.
- §6b `AddGUI`/`RemoveGUI`: caller pairing (DoPreCityInit end / Shutdown), main panel, the five tab panels with
  script instances and window IDs, per-panel lists and generator slots, text fields, news ticker, four more script
  windows and their winprocs, and the RemoveGUI teardown order. VW6 filed for the AddGUI tail and the fields RemoveGUI
  releases but AddGUI's first part does not create.
- Tests VW-T5 (SizeOf), VW-T6 (filter cheats, TerrainQuery).
- Commits: 01f83a9c (§3.1, VW1), ee4cf05a (§6b, VW6), this one (tests, report).

REA not used: all reads from tools/re_query.py, decomp/ and exe data bytes (build-runs/peread.py).
Not covered: input handlers (ProcessKeyDown/Up, mouse), VW2-VW6. ABI rows not applied (task rule).

## Round 1 report

What changed:

- New spec `docs/specs/view3d-window.md` for owner `cSC4View3DWin` (decomp file: 149 functions). It covers what
  ui-windows.md §3-4 and view3d.md §5 did not: §1 the four interface sub-objects (`+0` cISC4View3DWin 0x00ABCE40,
  `+4` cIGZMessageTarget2 0x00ABCE2C, `+8` window message filter IID 0xC6AE7085 0x00ABCE1C, `+0xC` cIGZWin
  0x00ABCBC0, 151 slots with the 17 overridden slots listed), QueryInterface's exact five-IID set, one shared refcount;
  §2 constructor order and defaults, the 33-type notification set at 0x00ABC070; §3 Init (command-name table
  0x00B0DD98, 13 cheat registrations with ID/text pairs read on the instruction order, three conditional), §4
  Shutdown (unregisters all 13 cheats unconditionally, quirk), §5 notification routing table (command messages to
  ProcessCommand first, then 25 types), §6 window DoMessage routing, focus-loss scroll stop, link-result quirk,
  the filter; §6a DoPreCityInit/DoPostCityInit (camera control at +0xF0 with start focus (512,0,512), renderer at
  +0xF4, helper at +0xEC, model maker at +0xF8). Four conformance tests VW-T1..T4.
- Offsets are absolute and checked on the instructions: the decomp's `this` is obj+4 in 0x007F53C0 and obj+0xC
  in 0x007F3E50, so its field names there are shifted.
- Questions: docs/specs/questions/view3d-window.md VW1-VW5; QUESTIONS.md reindexed.
- Proposed ABI rows: build-runs/abi-rows-re-view3dwin4.tsv (vtables.tsv format, 8 rows: 0x00ABCE2C and 0x00ABCE1C,
  not in docs/abi).
- REA was unavailable ("No app is open"); all reads came from tools/re_query.py, decomp/ and raw image bytes.

Not covered: per-cheat effects (VW1), AddGUI/RemoveGUI, the catalog item/building list builders, the input
handlers' detail (city-tools-input.md has part), cost indicator and cursor text.
