# Lanes

Per-lane stop state, as the merge train's "Per-lane files" step wants it: one file per lane holding today's
lane-table row (`docs/lanes/INDEX.md`, the HANDOFF §0 table until 2026-10-20), so the table is regenerated
from these instead of every lane editing the same lines.

Format: a markdown table with the row `Lane | Worktree | State at stop | Next steps`.

| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| abi-hygiene | `C:\Users\tlori\FreeCity4\.claude\worktrees\abi-hygiene` (branch `abi-hygiene`, tip `46f3b7cb`) | **ready.** No ABI change: `FC4_ABI_VERSION` stays **68**. (1) `tools/bump_abi.py`: the merge train's once-per-batch bump (`--batch`, `--lanes`, `--dry-run`, `--force`, `--record`, `--self-test`); increments the number, appends one `fc4.h` history line naming the batch and its lanes, rewrites the mirrored Rust const in `engine/capi/src/lib.rs`, refuses a dirty `fc4.h` and a repeated `--batch` (`build-runs/abi-batches.json`, gitignored). `--self-test` 0 failures. (2) `engine/tests/abi_entries.inc` is generated at build time and left git: CMake target `fc4_abi_entries` runs `gen_abi_entries.py` on every build, `fc4_abi_conformance_test` depends on it, no Python 3 skips the test (55 of 57 ctest entries), a generation failure fails the build. Standalone `python tests/gen_abi_entries.py` from `engine/` still works. (3) Held rows `re-answers25` applied to `docs/abi/interfaces.tsv` (`cISC4NetworkOccupant` 67 `GetPassageCeiling(float&)`, 68 `SetPassageCeiling(float)`; the rows already existed, these add the method names and notes). Conformance **4594 slots compared, 0 mismatches in required interfaces**, 39 waived, 60 in reported-only ones. Gate: plugin build + ctest 57/57 in the worktree and again in a fresh detached worktree at the tip with `abi_entries.inc` absent. No Rust touched, so cargo/clippy/rustfmt not applicable. No game runs. | (1) **Merge train:** expect a one-time conflict on `engine/tests/abi_entries.inc` when merging this branch into lanes that still track it — resolve by **deleting the file** on both sides and letting the build regenerate it (see below). (2) Run `python tools/bump_abi.py --batch <batch-id> --lanes <lanes in the batch>` **once per batch**, never per lane and never by hand; leave `FC4_ABI_VERSION` at 68 until then. (3) Optional: teach the train to call the script from `tools/merge-train.ps1`, and drop the now-dead "rerun gen_abi_entries.py / revert if only line endings change" step from the merge procedure. |

## The one-time `abi_entries.inc` deletion conflict

Every other lane still has `engine/tests/abi_entries.inc` tracked. This branch deletes it from git. The first
merge that brings `abi-hygiene` into a lane (or into main) will conflict on that path, because one side
modified a file the other deleted.

The merge train should resolve it **once**, the same way every time:

- keep the deletion (`git rm engine/tests/abi_entries.inc`, or `git add` the deleted side);
- do **not** restore, regenerate or commit the file;
- make sure `.gitignore` keeps the `engine/tests/abi_entries.inc` line that this branch adds;
- the build regenerates it via the `fc4_abi_entries` CMake target before compiling the conformance test, so
  nothing needs the file to be present.

After that one resolution the file never conflicts again: it is out of git in every branch that has taken
this change. Lanes that merged before this lands still carry it and will each need that single resolution
when their branch merges.
