# Runs: glue21fix2 (the PL17 CreateLotObjects fix)

All runs: city Konradshohe, `turbo = 1`, `end_date = 2032-01-01`; the game queue. "checked" = the
`-DFC4_CHECKED=ON` build (RTC failures logged with call sites).

| Run | INI / DLL | Verdict | Result |
|---|---|---|---|
| glue21fix2-gate-1 | `enable = propmgr, lotcfg`, three shadows; debug | exited -1 | The PL17 AV at `SimCity 4.exe+0x60D4B5` is GONE (slot 27 fills the caller's `SC4Vector`); a new in-dll int3 (CRT `failwithmessage`) right after the civic `AddAgent` burst. |
| glue21fix2-rtc-1 | same; checked | corrupt | RTC ERROR #0 named: "ESP not preserved" at `cFC4PropManager.cpp:416/417` — `SetPosition` called through the prop-occupant primary vtable whose slot 9 is `GetOrientation` (pop 0). Chain: game `CreateProp` (`exe+0x20CFE8`) → `fc4_prop_manager_add_prop_new` → `add_street_prop` → `set_position`. Then an AV writing `0x00AA48F8` (the desync's downstream damage). |
| glue21fix2-rtc-2 | checked after the QI normalization | exited -1 | 100 `over-release` warnings at `CreatePropInstance` (the factory reference released before it was audited) — fixed by `FC4_ACQUIRED(prop, "GetClassObject")` first. Then the lotcfg vector<bool> AV at SimNewDay. |
| glue21fix2-rtc-3 | checked, audit fixed | exited -1 | ref_warnings = 0, rtc = 0; the remaining fault is the lotcfg configuration-lifetime AV (independent of propmgr). |
| glue21fix2-iso-lotcfg-1/2 | `enable = lotcfg` only; debug (2 = with in-dll AV stack capture) | exited -1 | Same AV at SimNewDay 29 with propmgr OFF — the lotcfg lifetime issue is independent. Stack: core `simulate_agents` → deliver → game growth dev (`exe+0x304C1E`…`0x30975F`) → `cFC4LotConfiguration::IsCompatibleWithPurposeType:191` → `std::vector<bool>` reading 0 (a deleted wrapper; `edi` = the dead configuration). |
| glue21fix2-iso-lotcfg-3/4/5, isoref-1..4 | `enable = lotcfg`, per-op ref tracing | exited -1 | The ref ledger measured: creation +1 (catalogue, via core `add_configuration`), +1 raw per slot-20 map rebuild (`InsertConfig`), +1 per set-query insert (§B5.2 — was missing, added; duplicates kept per quirk 12). The game's growth pass then releases 4 references per configuration (release site `exe` ILT `0x00709798`) and the wrapper deletes under the catalogue. Slot 12 (release every element once + empty the set) implemented per §B7. Filed as PL18. |

## Final gate at `1b840d70`

qbuild all: cargo test 2860 passed / 0 failed, clippy clean (0 diagnostics), build 3/3 edges,
ctest 100% (58/58). Not yet game-clean: the PL18 configuration-lifetime fault stands between the
run and a `clean` verdict; the merge-queue entry is `wip` until PL18 is answered and the run ladder
reaches `clean` with the three shadows exact.

## Round 4 (2026-10-21, the double-add measured)

The candidate fix was already committed (`1b840d70`): the plugin loop's `FC4_ADDREF` per matched
configuration over the core's `with_refs` add = 2 per matched config per slot-9 query (the isoref
runs predate that commit and measured one add per query). Runs (Konradshohe, turbo to 2032,
`enable = lotcfg`):

| Run | Build | Verdict | Result |
|---|---|---|---|
| glue21fix2-ref2-1 | double-add | exited -1 | Day 29's wave no longer deletes (was ~23 wrappers); day 30 deletes 3 wrappers (k=1 query, no lot ref), then the same `vector<bool>` AV (`edi` = the dead wrapper). |
| glue21fix2-ref2-2 | + per-op ref trace restored | exited -1 | Full ledger measured: base after load 2 (catalogue + raw map add), game bulk wave +1 (exe `0x0069C385`), per-candidate game adds exe `0x006BF53D` (1..66 per config, 736 configs), query +2. The wave (exe `0x00709798`) releases -1 per config per pass; a k=1 no-lot config (count 4) drains to 0 and dies; a k=1 with-lot one (count 5) keeps 1. |
| glue21fix2-ref2-3 | + 40-ref cushion | exited -1 | DECISIVE: the wave drains a config to ZERO regardless of the references: five count-44 configs released 43 logged + 1 deleting times; two count-44 configs released once and stopped; in ref2-2 a count-4 died and a count-5 kept 1. The release count follows the game's candidate/pass structure (a config the game never AddRef'd was released 44 times), so no reference hand-out balances it. |

Conclusion: reference-count compensation cannot fix PL18. Filed **PL19** (the wave's container and
the original's behaviour at `0x00709798`) in docs/specs/questions/prop-lotconfig.md; the double-add
is kept (strict improvement), the per-op trace stays in `cFC4LotConfiguration.cpp` and the slot-9
query markers in `cFC4LotConfigManager.cpp` (diagnostics for the RE answer to be verifiable).
qbuild all at `f65bf327`+1: cargo 2860/0, clippy 0, build+ctest green; the game runs keep
ref_warnings = 0 and the shadows never ran to compare (the AV precedes them).
