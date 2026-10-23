| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| re-curve-editor-views4 — first curve entry-view behavior | Codex; re-curve-editor-views4 (detached HEAD) | WIP — substantive behavior committed; Init UI remains partial | Complete two remaining work packages in questions/curve-editor-views.md. |

## Report

Published the first dedicated [curve entry-view specification](../specs/curve-editor-views.md).
The historical approximately 8000-byte identity estimate is not behavior coverage.
No earlier dedicated owner publication was found in the permitted canonical material;
resource-exemplar-editor §13/14 expressly retained identity only. Its changes here
are narrow links and qualification of that historical table, leaving other owners alone.

Milestones:

- `2eeeabe8`: distinct construction/layout, interface adjustment/acquire, Happiness signed-key and x87 value gates/store/refresh, command selectors and first literal traces.
- `96a66a58`: native import/readback, Happiness's owned 256-sample interpolation, GeneralResponse epsilon insertion/replacement/deletion/wrapped navigation, text-set prefix effects and further traces.
- `82954017`: GeneralResponse graph bounds and call order, both formatters, reference thunks/destructors, detailed per-body ledger, precise question scope and canonical links.

The spec separates packed Happiness records and float-pair GeneralResponse records,
import from live points, entry SetValue from the misleadingly named key-erase method,
AL booleans from full-word/pointer tests, and owner arithmetic from raw graph primitives.
It documents unsigned low-word formatting versus signed low-word import, missing-Y
GeneralResponse add reusing the X text, duplicate/near-match association, empty/odd-count
boundaries, and mutation-before-callback/partial-failure prefixes. Independent literal
ledgers are explicitly PROPOSED / UNEXECUTED; none came from replacement code or execution.

Evidence: primary native export/re_query instructions and direct callers; decomp used
for navigation only. PE and export SHA-256 both
`3bc5c7fe807fe5aa24a3abb06e40f13486670bc1150612c09baf4ecedafe1a4b`, image base
0x00400000, PE timestamp 0x4C12BFB7. No REA query/preflight was attempted or required.
The q.py Bash wrapper failed with `Bash/Service/CreateInstance/E_ACCESSDENIED`;
PowerShell queries succeeded. Optional Python `pefile` import failed as unavailable;
an ignored read-only standard-library PE reader supplied the actual constant bytes.
Git repeatedly reported packed-refs.lock permission denied but successfully created
the detached-HEAD commits, verified by log and clean tracked status; shared refs/config
were not modified or repaired.

Checks: `git diff --check main -- <four changed documentation paths>` passed.
Ignored `build-runs/check-curve-docs.py` passed UTF-8/no-listings checks, two local
link targets and four tables with consistent columns. Tracked changes are four
documentation files only. Cargo/clippy/ctest and native game/runtime checks: n/a,
not authorized for this RE documentation lane. C ABI unchanged; no ABI proposals;
game runs `[]`, shadow n/a. No question was marked fully answered, so no answered
entry or generated QUESTIONS/index file was changed.

Questions: CEV-1 through CEV-4 in
[questions/curve-editor-views.md](../specs/questions/curve-editor-views.md).
Two precise remaining work packages keep this **WIP**, not DOCS READY:

1. Full native Init creation/geometry/configuration/final-result and failure prefixes:
   Happiness 0x0066C68C–0x0066CDA3 and GeneralResponse 0x0066D395–0x0066DCDA.
   Use paged native instructions; `asm --full` did not lift the 56-line page cap in
   this environment. Already inspected prefixes and isolated suffix pages do not
   qualify the whole Init. GeneralResponse graph constructor at 0x0066DAA1 targets
   foreign cGZScatterGraph 0x009BA033, not proof of its algorithm.
2. Shared variant same/null/output-alias primitive obligations, mutation during callbacks,
   and environment-qualified PC/RC/tie/NaN/sNaN/payload/exception-prefix literal cases.
   Do not turn absent safety guards into rollback, safe empty Happiness sampling,
   inherited reset or successful malformed-input behavior.

No engine/tools/ABI/map/names/generated coverage edits, builds, tests or game actions.
Manifest: C:/Users/tlori/FreeCity4/build-runs/run4-re-curve-editor-views4-r1-manifest.json.
