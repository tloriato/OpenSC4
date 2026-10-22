# Working conventions

Moved from HANDOFF §8 on 2026-10-20. Who reads it: everyone; engine and tooling lanes before their first
build or commit.

- **Bootstrap the harness with the engine** (the user, 2026-10-14): as more of the game is ours, use it to make the
  feedback loops short and automated (a turbo clock, plugin-side scripted actions instead of screen clicks, end
  dates instead of timeouts). Prefer a tooling switch in the plugin (off by default, logged) over UI automation.
- **Long commands, one wait:** give builds, cargo, ctest, `fc4verify`/`qbuild`, regress, game runs and
  `qwait.py`/`lanes.py wait` a tool-call wait of at least 60 s (Codex: `yield_time_ms >= 60000`); never spend a call
  only to check whether a command has finished. Measured 2026-10-21/22 on 196 Codex sessions: 22% of model calls
  were such checks, after 2,072 commands started with a 1 s yield.
- **Write files with the edit tool** (Edit/Write for Claude, `apply_patch` for Codex), never through the shell
  (here-strings, `Set-Content`, `WriteAllText`, redirects: 989 such writes in the same logs); a multi-line script is a
  file in the worktree's `build-runs/`, run by path.
- **Run cargo from `engine/`**, never the worktree root: there `cargo fmt --check` prints help and exits 0.
- **rustfmt: check only the files you touched**, `python <MAIN>/.claude/lane-bin/qbuild.py --wt <WT> fmt <files>`;
  add no diff and never mass-reformat. `cargo fmt --check` on main is not clean (18 files differ, measured
  2026-10-22), so its exit code says nothing about your change. **Clippy is clean** (`-D warnings`); a new warning
  is a regression.
- **Read results, not exit codes alone:** sum every `test result:` line from a captured file (never a `tail`); read
  ctest's verdict line (`0 tests failed` matches a grep for "failed").
- **Git:** prefer `git -C <path>` over `cd` (a stray `cd` once committed into another lane's worktree). Before deleting
  any branch, prove nothing on it is unique. Worktrees made by an elevated session need
  `safe.directory = C:/Users/tlori/FreeCity4/.claude/worktrees/*` (in the global git config).
- **PowerShell 5.1** mangles `"` in native arguments: write commit messages to a file, UTF-8 without BOM,
  `git commit -F`. PowerShell variable names are case-insensitive (`$a` clobbers `$A`).
- **`build-runs/` is gitignored:** gate logs, run logs, DLL copies and lane prompts live there; record the numbers in
  docs, not the files.
- **Build infra:** build through `qbuild.py` / `fc4verify.py`, never a bare cmake: they set a private
  `_MSPDBSRV_ENDPOINT_` (the shared 32-bit `mspdbsrv` runs out of address space with many lanes, LNK1318) and the
  sccache server per worktree ([coordinating.md](coordinating.md) "Build cache, lane checks and the gate").
  Both share one fail-fast OS lease per canonical worktree, including alternate log directories and verdict
  publication. A busy check prints its owner's PID, command, source and start: let it finish, then retry;
  it has not run a check or published a new verdict. `fmt` checks remain independent. The lease guardian
  waits for launched commands even if a wrapper exits unexpectedly; only its verified delegated child can join.
  Never remove `build-runs/.build-check.lock`. If the guardian itself crashes, ownership fails closed:
  inspect `build-runs/.build-check-owner.json` and surviving commands; only after confirming all have ended,
  manually archive that metadata file and retry (a live OS lock still cannot be bypassed).
- **Coordinator only, status reports for the user:** plain language, short, what changed and what is next.
- **Coordinator only, memory:** a Claude coordinator's notes are in `C:\Users\tlori\.claude\projects\C--Users-tlori-FreeCity4\memory\`.
