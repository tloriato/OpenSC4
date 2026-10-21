# Game runs: glue21 (the prop manager / lot configuration manager replacements)

One row per queue run of this lane's `FreeCity4.dll` in the real game (docs/harness.md). The verdict word is the
plugin's; `ref_warnings` must be 0 and every shadow line exact, or every difference explained as a numbered
question. Shadow columns are `<lines>, <lines with every mismatch count 0>`.

City is Konradshohe throughout, `turbo = 1`, `end_date = 2032-01-01`. The three shadows are on in every run
(`propmgr_shadow = 1`, `propmgr_shadow_cells = 32`, `lotcfg_shadow = 1`, `tutorial_shadow = 1`).

| Run label | Date / end | Verdict | ref_warnings | propmgr shadow | lotcfg shadow | tutorial shadow | Notes |
|---|---|---|---|---|---|---|---|
| `glue21-base-1` | 2032-01-08, turbo to 2032-01-01 | **`clean`** | 0 | 220, 220 | 293, 293 | 146, 146 | **the shadows over the originals** (no `enable =`): all three exact. This is the lane's clean turbo-to-2032 run |
| `glue21-repl-1` | - | `corrupt` | 0 | - | - | - | `enable = propmgr, lotcfg` and the three shadows: `foreign_exceptions=1`, no date reached (`SimCity 4.exe+0x1CF0DF`, read `0x38`) |
| `glue21-prop-1` | - | `corrupt` | 0 | - | - | - | `enable = propmgr` alone: same fault as `glue21-repl-1` |
| `glue21-lot-1` | - | `corrupt` | 0 | - | - | - | `enable = lotcfg` alone: `SimCity 4.exe+0x5BF49`, read `0x8C` |
| `glue21-prop-2` | - | `corrupt` | 0 | - | - | - | after the `GetPropName` and refaudit fixes: `SimCity 4.exe+0x1447C7`, read `0x5` |
| `glue21-prop-3` | - | `corrupt` | 0 | - | - | - | with `subscribe` / `acquire_service` tracing (no `propmgr:` line before the fault) |
| `glue21-prop-4` | 2026-01-27 | `corrupt` | 0 | - | - | - | after the `Read` before `Init` fix: **the city now loads and ticks to SimNewDay 2026-01-28**, then `SimCity 4.exe+0x2F44EF`, read `0x7C` |
| `glue21-lot-2` | - | `corrupt` | 0 | - | - | - | `enable = lotcfg` after the same fix: `SimCity 4.exe+0x5BF49`, read `0x8C` (unchanged; GLUE21-1) |
| `glue21-gate-1` | - | `corrupt` | 0 | - | - | - | the requested gate run (`enable = propmgr, lotcfg` and the three shadows): `foreign_exceptions=1`, no date reached |

`ref_warnings` is 0 in every run, including the corrupt ones.

## What `glue21-base-1` shows

The three shadows are exact over the **original** `cSC4PropManager`, `cSC4LotConfigurationManager` and
`cSC4TutorialSystem` on a full turbo run to 2032-01-08 (31.6 s real, 68.7 days a second): 220 / 220, 293 / 293 and
146 / 146 lines with every mismatch count 0. Those are the same comparisons glue15 recorded at 76 / 101 / 50 over a
run to 2028-01-01; the longer run simply produces more month lines. Nothing in this lane changed what the shadows
compare.

## Why the replacement runs are `corrupt`

Bisecting (`glue21-base-1` against `glue21-prop-1` and `glue21-lot-1`) puts the fault in the two replacement
classes, not in the shadows and not in the game. Three bugs were found and fixed on the way (see
`docs/lanes/glue21.md`); after the last of them the city loads and ticks for a game day before a first-chance
`ACCESS_VIOLATION` outside `FreeCity4.dll` (reading `0x7C` from a null object, `esi = 0`, in the prop developer /
construction-occupant path that calls prop manager slot 15). The remaining fault is filed as GLUE21-2.

`glue21-lot-2` faults earlier and at an address that did not move across the fixes: `enable = lotcfg` cannot build
a configuration catalogue at all (GLUE21-1, `cSC4LotConfiguration` is not GZCOM-creatable), so the game receives
`GetLotConfiguration -> null` where it expects a configuration. That run is expected to stay `corrupt` until
GLUE21-1 is answered.
