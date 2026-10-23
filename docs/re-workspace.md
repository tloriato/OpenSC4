# Reverse-engineering workspace

This file covers the **reverse-engineering workspace** that produces the specs in [specs/](specs/). It is for spec
writers only (the in-game harness entry under Tooling is for the coordinator); implementers must not use it (see the
clean-room rules in [CONTRIBUTING.md](../CONTRIBUTING.md)). It contains no game binaries or assets, and decompiled
output is never committed.

## Early RE notes (spec writers only; superseded by docs/specs where they overlap)

| Topic | Doc |
|---|---|
| The engine core: agent scheduler, clock, message protocol | [docs/sim-engine.md](sim-engine.md) |
| Sim speed, tunables, save format, budget, neighbours, weather | [docs/subsystems/clock-budget-neighbors.md](subsystems/clock-budget-neighbors.md) |
| R/C/I demand graph, population census, residential EQ/HQ | [docs/subsystems/demand-rci.md](subsystems/demand-rci.md) |
| Growth: desirability, tract selection, lot scoring, abandonment | [docs/subsystems/development.md](subsystems/development.md) |
| Traffic: trip generation, A* routing, congestion feedback | [docs/subsystems/traffic.md](subsystems/traffic.md) |
| Grids: SimGrid, land value, pollution, power, water, crime, aura, fire/police, flora | [docs/subsystems/grid-sims.md](subsystems/grid-sims.md) |

Each subsystem doc labels its claims **[confirmed]** (read in code) or **[inferred]**, and gives Windows
addresses alongside the matching named Mac Rev A function.

## Data

- [re/symbols_windows.tsv](../re/symbols_windows.tsv) and [re/symbols/*.tsv](../re/symbols/): hand-verified
  Windows address → name mappings, each with its evidence.
- `re/names_windows.tsv` (generated): every name applied to the Windows exe, with source and confidence.
- [re/class_id_table.tsv](../re/class_id_table.tsv): the exe's built-in ID→name table (648 class, message,
  service and command IDs).
- [re/tuning/](../re/tuning/): simulator tuning exemplars and the Demand exemplar graph, extracted from the game data.
- [re/prior_art.md](../re/prior_art.md): community knowledge (hard-coded addresses, interface IDs, vtable orders).

## Tooling

- `tools/symport/`: matches Windows functions to the debug-symbol Mac build.
  - Stage A: rare 32-bit constants and strings.
  - Stage B: call-graph propagation.
  - Stage C: vtable slot alignment, including the GCC→MSVC/ICC destructor and overload rules.
  - Stage D: ordered callee alignment.
  - It also detects identical-code-folded functions.
- `ghidra/scripts/`: headless scripts that apply names (with evidence as plate comments) and export
  per-namespace decompilation.
- `tools/setup_ghidra.ps1`: builds the Ghidra projects from your own executables and exports the analysis tables.
- `tools/build.ps1`: the full pipeline (match → merge → apply → export to `decomp/`).
- `tools/dbpf.py`, `tools/extract_tuning.py`: read-only DBPF/exemplar reader and tuning dumper.
- `tools/re_query.py`: read-only queries over the Ghidra export (function, asm, callers, callees, xrefs, strings,
  names, vtables); see [Querying in parallel lanes](#querying-in-parallel-lanes).
- `tools/x86dis.py`, `tools/class_vtables.py`: quick disassembly and vtable discovery from a CLSID.
- In-game harness (coordinator): `tools/ingame_run.ps1`, `tools/regress.ps1`, `tools/run_summary.ps1`, `tools/sc4ctl.ps1`;
  see [docs/ops/running-the-game.md](ops/running-the-game.md). Implementers work from the coordinator's run results and do not open `tools/`.

## Querying in parallel lanes

Use `tools/re_query.py` first. It reads the Ghidra export (`ghidra/export/windows`, from the main checkout when a
worktree has none) plus `re/names_windows.tsv` and `docs/abi/vtables.tsv`, prints one line per item, and needs no
server or lock. Each table is parsed once into `build-runs/re-query-cache/` of the main checkout (about 1 s cold,
written to a temp file and `os.replace`d) and rebuilt when a source's size or mtime changes. A query takes about
0.15 s. 8 concurrent invocations work, with a cold cache (0.7 s wall) and a warm one (0.25 s wall).

```bash
python tools/re_query.py func 0x5B9FEB cSC4PathFinder::cSC4PathFinder   # containing function, size, name, owner
python tools/re_query.py asm 00407bc0 --max 60      # instructions, call targets and strings named
python tools/re_query.py callers 009457c6           # also: callees, xrefs [--from], vtable <class|addr>
python tools/re_query.py --limit 20 strings 'demand'  # also: search <name-regex> [--all]
```

An address can be hex (with or without `0x`), `FUN_xxxxxxxx`, `SimCity 4.exe+0x...`, or a name (exact, else a
unique regex match). `vtable <function>` lists every vtable slot that points at that function. `asm` on bytes that
Ghidra did not decode falls back to capstone (`tools/x86dis.py`).

The export does not have pseudocode, types or Ghidra's live analysis. For those, use REA (the Ghidra-backed MCP
tools). These are the facts measured on 2026-10-20 (rea-agents 6.4.0, Ghidra 12.1.4):

- **One session per Claude Code process.** Each Claude Code process starts one `rea.mjs mcp` server. That server
  holds one active target, and the tools take no session id. Every subagent or lane of that process shares the one
  target and the one Ghidra JVM (about 2 GB).
  - When two lanes called `open_binary` on the same exe, they joined the same session. A second `open_binary` on an
    exe that is already open returns at once.
  - The first query after an open imports and auto-analyses the exe, which takes about 3 minutes. Other lanes'
    calls wait for that import rather than failing.
  - `close_binary` waits for in-flight requests and then closes the session for every lane. A lane's next call then
    fails with `target_unavailable`. `open_binary` on another file replaces the target for everyone.
- **Separate processes are independent.** A second process (`node .../rea-agents/scripts/rea.mjs xrefs <exe> <addr>`)
  ran its own JVM next to the MCP session's JVM. Both answered. The second process also re-analysed the exe for 3
  minutes.
- **"Ghidra client is already started".** The error comes from `GhidraClient.#start` in rea-agents. It fires when a
  start begins while the previous run's private runtime directory or socket is still recorded, meaning a startup that
  failed was not cleaned up. Two things make a startup fail:
  - The caller cancelled it, for example a lane that was stopped or timed out mid-call.
  - It ran past `REA_GHIDRA_STARTUP_TIMEOUT_MS`, which defaults to 330 s. That limit is close to the 3-minute import
    when several JVMs or a game run share the machine.

  `%TEMP%\rea-ghidra-*` directories left over from 13, 14 and 16 October are the trace of such cleanups. From then on,
  every call through that server fails until a `close_binary` completes the cleanup.

These observations apply only to clients sharing the **same** MCP server process.
A keeper opened by the coordinator cannot initialize a later independent worker.
Reserved workers must use [same-client target admission](ops/rea-admission.md):
the coordinator reviews the exact read-only target/API contract, admission retains
the actual procedure-capable client, and the worker attaches to that client with
only the two research tools. Claude RE workflow attachment is currently unsupported.
No lane may open a different target, close/reset another session, or retry a round
to hide a startup failure. Failed startup preserves its cause and cleans only
owned processes; a tool list or server count must never produce target-ready GO.

## Rebuilding locally

This section rebuilds the RE workspace. To build the engine and the plugin DLL, see
[engine/README.md](../engine/README.md#build-and-test).

Everything is built from your own copy of the game. Nothing decompiled is downloaded or committed.

1. Install Ghidra 12.1.x (`C:\tools\ghidra_12.1.4_PUBLIC`), JDK 21+, Python with `pefile` and `capstone`.
2. Copy `SimCity 4.exe` to `bin\SimCity4.exe`.
3. Clone [0xC0000054/sc4-ghidra-symbols](https://github.com/0xC0000054/sc4-ghidra-symbols) to
   `external\sc4-ghidra-symbols` (type archives and Function ID databases).
4. Run `.\tools\setup_ghidra.ps1 -Windows`. It imports the exe into `ghidra\project\FreeCity4`, runs
   auto-analysis with the Function ID databases attached, and exports the analysis tables to `ghidra\export\windows`.
5. Run `.\tools\build.ps1 -SkipMatch`. It applies the committed names (`re\names_windows.tsv`), labels and types,
   then exports the decompilation to `decomp\`.

To verify a project built elsewhere without touching `ghidra\project` or `decomp\` (for example a scratch project
from `setup_ghidra.ps1 -ProjectRoot`):

```bash
.\tools\build.ps1 -SkipMatch -ProjectRoot <projectDir> -DecompOut <outDir>
```

Expected output of step 5 on a fresh `-Windows` project (verified 2026-10-13, Ghidra 12.1.4):
`renamed=15859 created=8 missing=76 failed=191 thiscallConverted=10145`, `labels=856 vtablesTyped=853 failed=0`,
`classes=1023 methods=9258`, `fieldsRenamed=235 filled=105`, and 1815 decompilation files. The counts are benign:
`missing` are auto/library names whose address has no function in this project; the 191 `failed` are all
"a symbol named `_lua_…` already exists at this address" — the Function ID databases already applied the identical
name, so none is a real conflict (0 of the 191 differ from the name we tried to set). All 906 hand/community names
in `re\names_windows.tsv` apply cleanly.

Optional, only to re-run the symbol port or to read the named Mac code: get the Mac "Rev A" Intel executable
(Aspyr's Mac port, which kept its debug symbols) from the
[SC4 Maxis files](https://community.simtropolis.com/sc4-maxis-files/) archive on Simtropolis, run
`.\tools\setup_ghidra.ps1 -Mac <path>`, then `.\tools\build.ps1` (with matching).

## Caveats

- Decompiled output is Ghidra pseudocode, not original source.
- The Windows build used the **Intel C++ Compiler 6.0** (`-Qvc6`, VC6 runtime). Class layouts differ from the Mac/GCC
  build, so Mac struct layouts are never applied to Windows `this` pointers.
- Automatic names are reported with a confidence level. Bodies of 12 bytes or less are usually folded getters and are marked `low`.
