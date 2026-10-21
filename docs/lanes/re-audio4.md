| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| re-audio4 (RE spec: audio ownership reconcile + ambience manager) | `re-audio4` / (RE lane) | **ready** (spec only, no engine/ABI change): new `docs/specs/audio-ambience.md` specifies `cSC4AudioAmbienceManager` (11,298 B); proposed exact audio owner rows below; questions AA1-AA4 | AA1 (ambience layer internals, 14,417 B) next; then cSC4AudioScapeManager / cSC4AudioListener |

## Report

What changed
- `docs/specs/audio-ambience.md` (new): `cSC4AudioAmbienceManager` object layout (0x120 B), `Init` (4 services,
  12 message subscriptions incl. `0xC53D10AA`), city init/shutdown, occupant sound rules per property
  (`0x6A36AFAC`, `0xC9B93A56`, alarm props `0x6A36AFAB`/`0x0A36AFA2`/`0x2A53BE13`, condition props), automata and
  railway-crossing sounds, the alarm clock (whole-hour quirk: minutes/60 is an integer division at `0x004E8EDB`),
  tuning (default margin 70.0), the attention rectangle, the `OnTick` stage machine (999 us budget per stage),
  `DoEvent` -> `cFreshPlayer` slot table, `DoMessage` table, the ambience-layer vtable as called.
- `docs/specs/questions/audio-ambience.md`: AA1-AA4; `docs/specs/QUESTIONS.md` re-indexed.
- REA preflight: NO-GO ("No app is open"); all evidence from `tools/re_query.py` (export + asm) and decomp/.

Proposed coverage-map rows (coordinator applies; place them BEFORE the `cSC4Audio*` / `cGZSnd*` wildcard rows,
first match wins). Owner bytes measured with `tools/coverage.py --owners` (audio total 225,440 B):

| pattern | spec | bytes | basis |
|---|---|---|---|
| `cSC4AudioAmbienceManager` | `audio-ambience.md` | 11,298 | this lane, whole class |
| `cSC4AudioEventHandler` | `audio.md` | 8,877 | audio.md §7.1-7.3 (layout, Init, DoMessage, HandleAutomataMessages) |
| `cSC4AudioSoundManager` | `audio.md` | 6,909 | audio.md §8.1-8.5, §8.7 |
| `cGZSndEventStd` | `audio.md` | 478 | audio.md §3.1, §4 |
| `cSC4SoundDescription` (existing row: set spec) | `audio.md` | 1,112 | audio.md §10.9 |

Total 28,674 B (12.7% of audio). NOT claimed (only partly specified): `cSC4AudioAmbienceLayer` (interface only,
AA1), `cSC4AudioAEMSEventSystem` (§5), `cGZSndMgr` (§8.6), `cSC4AudioEventHandlerDisasterFire` (§7.4),
`cSC4AudioAEMSSoundBank` (§10.7), `cSC4AudioCOMDirector` (§11 registrations only), and every Hit/Fresh/Radio/
GZSndSys class. Wildcard rows keep spec `-`.

Next
1. AA1: spec `cSC4AudioAmbienceLayer` (14,417 B; `_UpdateAmbienceScores` `0x004E5920`, `_UpdateAmbienceLoops`
   `0x004E5D50`, `_ReadTuningValues` `0x004E7A90`) into audio-ambience.md §8 -> then claim it.
2. AA2 `_UpdateLotOccupantsForConditionChange` `0x004E9DA0`.
3. `cSC4AudioListener` (2,799 B) and `cSC4AudioScapeManager` (9,811 B) are the next SC4-side owners.
