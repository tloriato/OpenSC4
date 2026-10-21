| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| re-spanbridge4444-4 | workflow / `re-spanbridge4444-4` | **ready** (spec only): round 2 adds §1.2 (type-7 producer; SP4 narrowed, still open). Round 1: SP3 answered in `docs/specs/span-render-4444.md` §1.1 (native call boundary of `cSpanRender4444`); SP4 filed (type-7 producer) | SP4 needs a runtime breakpoint on `0x008307A8`; reconcile the proposed vtable rows |

## Report round 2 (2026-10-21)

**What changed**: `docs/specs/span-render-4444.md` gains §1.2, "Where colour type 7 comes from". The only shipped
type-7 producer is `cGZGraphicSystemW32::OnCanvasInit` (`0x0098B3CC`). It gives the screen buffer
(`cGZScreenBuffer::Init` `0x00991EC0`) and its 4x4 auxiliary buffer type 7 at screen depth <= 16 and type 9 otherwise.
The screen buffer is claimed by `cGZDrawContextRenderToSimGLFactory`: draw-system slot 3 puts each new factory at
the front of the list, and the SimGL factory accepts only IID `0xE3094DC8`, which `cGZScreenBuffer` answers. So the
screen buffer never reaches `RenderToBuffer::Init`. Colour type 1 stays 1 and is rejected by Init (types outside
2..9 fail). The Gimex (literal targets 1/2/9), buffer-resource (type from the file header) and lot-editor (8/9)
sources are listed. The only route to a type-7 RenderToBuffer target is undocumented and goes through the game's own
COM classes: `cGZBuffer` CLSID `0xC470D325`, Init type 7, then `cIGZDrawSystem` slot 4.

**Questions**: SP4 is narrowed in `docs/specs/questions/span-render-4444.md` and stays open. Ruling out data-driven
type-7 buffers that reach draw-system slot 4 needs a runtime hit count on `0x008307A8`. `QUESTIONS.md` was reindexed.
REA was unavailable (no app open), so the work used bytes via `re_query.py` and `build-runs/scan.py`.

**Next**: a hit count on `0x008307A8` in a 16-bit scripted session. If it is 0, SP4 closes with "no shipped type-7
RenderToBuffer target", and pixel validation must use the COM-built buffer route.

## Report (2026-10-21)

**What changed**
- `docs/specs/span-render-4444.md` new §1.1 "Native call boundary": 4-byte per-context object (GlobalHeapAlloc,
  vptr only) pointing at one shared static vtable `0x00AC185C` (immediate occurs once; slot entries 1-5 each occur
  once in the image), so patching entries 1-5 covers every A4R4G4B4 instance. Slots 1-5 are MSVC thiscall, `this`
  ignored, callee pops 16/16/20/16/24 bytes (4/4/5/4/6 dwords), no return value, EBX/EBP/ESI/EDI preserved; slot 0
  thiscall(flags) `RET 4`, frees via GlobalHeapFree `0x005E5620`. Lifetime: Shutdown `0x0082FF20` / Release
  `0x008306C0`. Only native caller: `DrawPreclippedSpanList` `0x008302E0` (slot selection table by context bytes
  `+0x74`/`+0x41`, argument sources `+0x78`, `+0x44`, `+0xB4`, `+0xC4`; blit sites `0x008304D1`, `0x0083053B`,
  `0x00830591`). No interface exposes `+0xB0`; the context is created only by factory `0x0082B910` (vtable
  `0x00AC162C`, registered by `cGZDrawSystem::Init`, reached via `cIGZDrawSystem` `0x00AC1668` slot 4 `0x0082BAB0`);
  `0xAB300B20` is not on that path. §7 and §11 cross-reference it.
- Questions: SP3 archived to `docs/specs/questions/answered/span-render-4444.md` (MATCHES); SP4 filed (which shipped
  path builds a type-7 RenderToBuffer target; static sweeps found no literal 7 at the 34 colour-pair constructor
  calls or at draw-system CreateBuffer calls). `docs/specs/QUESTIONS.md` regenerated.
- Proposed ABI rows (not reconciled into docs/abi): `build-runs/abi-rows-re-spanbridge4444-4.tsv` (6 rows,
  `docs/abi/vtables.tsv` format, vtable `0x00AC185C`).

**Method**: all from bytes (`re_query.py asm --bytes`) and a raw image scanner `build-runs/scan.py` (immediate,
call-target, disp32 and push-imm sweeps). REA was not needed.

**Next**: SP4 by a runtime breakpoint on `0x008307A8` (Init, type-7 branch) in a scripted UI session; then a shadow
could forward the five slots by patching `0x00AC1860`-`0x00AC1870`.
