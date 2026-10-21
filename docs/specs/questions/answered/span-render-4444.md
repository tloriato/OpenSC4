# span-render-4444: answered questions (archive)
<!-- Archive for [span-render-4444.md](../../span-render-4444.md). Lanes do not read this file. Open questions are in ../span-render-4444.md. -->

<!-- from: glue-span4444-4 (2026-10-21) -->
- ~~**SP3 - section 1 / section 7 (interop boundary for a shadow or replacement):** the spec gives the vtable
  `0x00AC185C` and the slot addresses but no call boundary a plugin may use. To wire the landed core
  (`engine/core/src/span_render4444/`) into the game without inventing anything, the glue needs: (a) the calling
  convention, stack cleanup (bytes popped) and return value of slots 1-5 (thiscall with `this` in ECX? the exact
  dword count of each argument list, e.g. slot 5 `(p1, p2, rectA, rectB, src, mode)` = 6 dwords?); (b) a
  reconciled `docs/abi/vtables.tsv` row for `nGZDrawSoftRender::cSpanRender4444` (none exists; compat entries must
  come from docs/specs or docs/abi), and whether the vtable is shared by every instance (so one slot forwarder
  covers all contexts) or copied per context; (c) whether any GZCOM path reaches the renderer instead (the SDK has
  `kcGZDrawContextAsBuffer` `0x0AB300B20` but no interface for it; is the context `+0xB0` pointer settable or
  readable through any interface?); (d) which shipped code paths create a colour-type-7 (A4R4G4B4)
  `cGZDrawContextRenderToBuffer` in the GOG 1.1.641 build under the default driver, and which scripted in-city
  action (harness `script =`) makes them call slots 1-5, so a shadow can see real native inputs.
  [glue-span4444-4: no plugin wiring; the core stays unit-tested only, no detour or vtable patch installed]~~
  **ANSWERED 2026-10-21 (re-spanbridge4444-4):** MATCHES - span-render-4444.md section 1.1: slots 1-5 are MSVC thiscall, `this` in ECX ignored, args right to left, callee pops 16/16/20/16/24 bytes (4/4/5/4/6 dwords), no return value, EBX/EBP/ESI/EDI preserved; slot 0 thiscall(flags) `RET 4`. One static vtable `0x00AC185C` shared by all 4-byte per-context objects (GlobalHeapAlloc), so patching its entries 1-5 covers every instance; proposed vtables.tsv rows in the lane's build-runs/abi-rows-re-spanbridge4444-4.tsv. Only native caller: DrawPreclippedSpanList `0x008302E0` (selection by context bytes +0x74/+0x41). No interface or GZCOM class (incl. `0xAB300B20`) exposes or sets `+0xB0`; the context comes only from factory `0x0082B910` via cIGZDrawSystem slot 4. Part (d) (shipped type-7 producer and script action) not found statically: filed as SP4.
