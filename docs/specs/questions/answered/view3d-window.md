# view3d-window: answered questions
<!-- Answered entries moved from ../view3d-window.md. -->

<!-- from: the cSC4View3DWin window lane (re-view3dwin4, 2026-10-21) -->
- **VW1 - §3 step 3:** what each of the thirteen cheats does in `ProcessCheatCode` (`0x007F3510`, 244 decomp lines).
  The Ghidra output types the cheat ID as a float, so branch targets must be read on the instructions. Known so
  far: `You don't deserve it` (`0x4A82253E`) sets byte `+0x446` and returns `0x4A822501`; `WatchMeMove`
  (`0x4A972F37`) sets a byte flag and returns `0x4A972F01`; one branch sets the camera control's value to `64.0`
  or `0.9f` (`0x3F666666`); two branches build a `cSC4RealisticOccupantFilter` at `+0x320`. What settles it: a
  byte-level walk of `0x007F3510` mapping each `CMP` against the IDs of §3 to its effect.
  **ANSWERED 2026-10-21 (re-view3dwin4):** DIFFERS — view3d-window.md §3.1: the earlier notes were wrong on two points: both byte-flag cheats return true (not an ID), and the 0.9/64 values are SizeOf's clamp of its argument; TastyZots/Zoneria toggle one realistic-filter flag each and return false.
