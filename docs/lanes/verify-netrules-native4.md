| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| verify-netrules-native4 (published native rule/solver endpoint audit) | `verify-netrules-native4` / engine lane, detached HEAD | blocked; unsupported independent endpoint (NR16) | Publish the typed independent lifecycle/solver contract before resuming; no further qualification round or alias. |

## Endpoint qualification

The requested independent original rule/solver comparison cannot be implemented from the published contracts.
`docs/specs/network-rules.md` §2.4 explicitly says the rule family has no outside entry point, factory, class ID
or IID. Section 2.4.3 offers resource overrides and network tool/manager replacement hosts; none supplies an
independently owned original rule/evaluator/local solver fixture. The object shapes and direct-call addresses
in §2.4.1 do not establish the required typed independent lifecycle.

A capped search of `docs/abi/`, `engine/third_party/gzcom-dll` and `engine/plugin` for `cSC4TileRule`,
`cSC4TileEvaluator`, `LoadEvaluator`, `00622A90` and `006220D0` found no binding. Qualification stopped after
five recovery shell calls, within the eight remaining calls of the original 15-call allowance. No expanded
implementation or additional endpoint investigation followed.

## Changes and exclusions

- `232fa2d2` records the qualification and appends **NR16** to `docs/specs/questions/network-rules.md`.
- NR16 requests typed construction, loading, input/output cell ownership, primitive service dependencies,
  allocator/destructor pairing and failure cleanup for an independent original rule/solver fixture.
- No engine/plugin, switch/default, ABI table, tools or map changes; no ABI exports changed.
- No private layout/constructor guesses, original-output seeding, verifier clones, global-owner promotion,
  live network mutation or source fitting. NR6/NR7 and Pipe/Subway/platform exclusions remain unresolved.
- No game request was issued; no game verdict or solver parity is claimed. Transport 27/27 is not a solver comparison.
- No fourth qualification round, alias or external communication. The question index was not regenerated:
  running `tools/` scripts and editing that generated index are forbidden to this lane.

## Validation

`git diff --check HEAD~1 HEAD` passed for the question/qualification commit. The interrupted first-round
baseline sessions 94019/82654 have no established completion. A fresh quiet `qbuild.py --wt <WT> all` was
started on unchanged engine source in recovery round 2 (session 96879). At the last observation cargo was
compiling the workspace; cargo, clippy and CTest completion counts are not yet available. Full logs are in
this worktree's `build-runs/qbuild-*.log` and `build-runs/fc4verify-*.log`. No gate pass is claimed.

## Next steps

Park the queue entry as **wip**. A spec/ABI owner must resolve NR16 before any original solver comparison can
be attempted. The coordinator can collect the outstanding full gate's final logs and regenerate the question
index. There are no outstanding game requests. The worktree remains detached as required; no shared refs
were changed. Recovery manifest: `C:/Users/tlori/FreeCity4/build-runs/run4-verify-netrules-native4-r2-manifest.json`.
