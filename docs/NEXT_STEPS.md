# Next steps

The one pick-up document for the next coordinator session. It states what we do next and nothing else. Every write
replaces the whole file: no history, no logs, no "previously". Work that is done and verified is deleted from it.
Where another document disagrees with this one, this one wins.

State as of 2026-10-28 evening: **the native port starts.** Lanes of the old replace-mode runs are stopped and stay
stopped. The engine logic in `engine/core` is converted, class by class, into drop-in replacements for the exe's own
functions, and each converted function is proven identical to the original before it counts.

## 1. The goal and the proof

**Goal:** a standalone game in Rust that behaves exactly like SimCity 4, byte for byte. Players supply their own data.

**Proof, in two layers. Equal or different is binary; there is no tolerance.**
1. **Per function.** `tools/prove` captures the real game at a function's entry and at its return. It then replays
   the original and our function in an emulator from the same captured state, on the captured inputs and on generated
   variants. The verdict is **EXACT** or **DIFFERENT** with the first difference.
2. **Whole game.** The converted functions of an owner are switched on in the game, and every reference save of the
   corpus that enters them must stay identical under the fixed schedule (section 5).

**Credit (Exact-replaced):** a function counts when `prove` says EXACT on all its captures and variants, from at
least three scenarios, and its owner, switched on, keeps every reference that enters it identical.

## 2. The native port

- **Crate:** `engine/native` (`fc4-native`): Rust, `no_std` outside tests, target `i686-pc-windows-msvc`. It builds
  `fc4native.dll` with no imports and no C runtime.
- **One function, one replacement.** Each converted function replaces exactly one function of the exe. It keeps:
  - the same calling convention and arguments;
  - the same effects on memory: return value, callee-saved registers, x87 state, and every byte outside the dead
    stack;
  - the same calls into other functions of the game, in the same order, with the same arguments.
- **It works on the game's own objects.** It reads and writes fields at the original offsets (`docs/abi/layouts.md`).
  - It allocates through the game's allocator.
  - It writes the game's own vtable addresses into objects it constructs.
  - It calls other game functions at their original address, through `game_fn!`, so a later replacement of the
    callee is picked up.
  - No second copy of state, and no bridging to `engine/core`.
- **Arithmetic matches the game's x87 at control word 0x007F:**
  - `f32` operations;
  - float-to-integer conversion with the semantics of the game's `__ftol` helper (truncate to 64 bits, keep the low
    32 bits). Never a bare Rust `as`, which saturates;
  - transcendentals on the x87 itself.
- **Exports:** one export per function, `fc4p_<address>` (8 lowercase hex digits of the original entry).
- **In the game:** `FreeCity4.dll` loads `fc4native.dll` from its own folder at startup. For each function enabled in
  the ini (`[native] functions = <owner>|<address>, ...`, or `all`), it writes a jump at the original entry, in
  memory only, after the 1.1.641 version check.
- **Tests move with the code.** A converted module takes its tests from `engine/core`. They build objects in memory
  at the original layout. In `cfg(test)`, a `game_fn!` callee resolves to our own port, or to a fake when none exists.
  The tests are kept for later refactoring and optimisation. The proof is `prove`.
- **Deleting the old code.** Once an owner is proven and switched on, its `engine/core` module (its tests moved), its
  `engine/capi` wrappers and its `engine/plugin` class and shadow are deleted.

## 3. The prove tool (`tools/prove`)

- `python tools/prove.py capture --functions <owner|address,...> --scenario <name> [--hits N]` runs the original
  game in a test slot (9-12 only) under the pinned fixed-schedule DLL, with no native functions on. It stores each
  hit in `build-runs/prove/captures/` (ignored by git):
  - the pages that the original's replay touches, plus the stack;
  - the exit pages that changed.

  Each capture is validated: replaying the original in the emulator must reproduce the real exit exactly.
- `python tools/prove.py check <owner|address,...> [--dll engine/native build]` runs the original and ours from every
  capture and from generated variants: arguments and the object fields the original reads, typed sweeps with
  boundaries, NaN and out-of-range values. It compares:
  - the return value (EAX; EDX for 64-bit returns; ST0 for float returns);
  - the callee-saved registers and ESP;
  - the x87 stack and control word;
  - every byte outside the dead stack;
  - the sequence of calls into the game, compiler helpers excluded.

  The verdict is EXACT or DIFFERENT. A variant that leaves the captured pages is reported as skipped, never as
  passed. `check` writes `docs/proven.tsv`, which `tools/coverage.py` reads.
- Emulator facts already established, with one city and one capture per function:
  - the emulator matched the real CPU on 13 of 13 functions (161,612 changed bytes);
  - x87 transcendentals run on the host CPU, because the emulator's are only double-precision accurate;
  - clock reads (`rdtsc`, `rdtscp`) are fixed inputs;
  - heap growth (NtAllocateVirtualMemory) is serviced as zeroed pages.

## 4. Milestone 1: the land value simulator

`cSC4LandValueSimulator`: 27 functions, 6,711 bytes. Convert from `engine/core/src/landvalue.rs` and its spec
`docs/specs/landvalue.md`.

```
006bd4b0 GetGZCLSID           006bd8a0 Release                 006bddc0 (vtable thunk)
006bd4c0 QueryInterface       006bd8d0 Write                   006bddd0 (vtable thunk)
006bd540 TractIsZoned         006bd950 Read                    006bded0 (vtable)
006bd5c0 TractHasLand         006bd9f0 (helper)                006bdf70 Shutdown
006bd640 (helper)             006bda30 (helper, 716 bytes)     006be110 ReadSimulatorTunableValues
006bd6b0 DoEndOfSimulationCycle 006bdd10 (helper)              006be3b0 Init
006bd700 GetLandValueType     006bdd90 GetAverageLandValue     006be7b0 DoIdle (2,281 bytes)
006bd740 GetLandValue         006bdda0 GetModeAltitude         006bf0d0 DoMessage
006bd750 SetupAltitudeEffect  006bddb0 (vtable thunk)
006bd810 LookupAltitudeEffect
```

Done when:
- every function is EXACT in `prove`, from at least three scenarios;
- with `[native] functions = landvalue`, every reference that enters land value stays identical;
- the old land value code in core, capi and plugin is deleted.

## 5. Kept from before: the fixed schedule and the references

`docs/specs/simulator.md` §5.8, applied to the original by `engine/plugin/FC4Schedule.cpp` (in-memory hooks only).
The reference store (`tools/golden.py`, `build-runs/golden`) keys each reference on the schedule code, the pinned DLL
and the run settings. Pinned now (r6): sha ee75f849, schedule code c7ac4ea4290c77db. Captures use the same pinned
DLL. The whole-game check runs `tools/check.py` with the owner's native functions on.

## 6. After milestone 1

- Scale with Codex lanes, one owner per lane, in order of reach (the code the corpus executes).
- A lane's credit is functions proven EXACT and switched on.
- Anything done five times by hand becomes a tool first.
- The standalone track follows once enough owners are native: the save round trip, then a headless host, then
  rendering and UI.

## 7. Stopped (do not restart)

- Owner-level replace mode with a second copy of state as a credit path.
- Shadows as proof.
- Record/replay at the `fc4.h` seam.
- House-city verified.
- Single-method credit lanes.
- An operator layer between the coordinator and the lanes.
