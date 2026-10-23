| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| re-model-instance-numerics4 — MI11–13 primary numeric research | Codex / re-model-instance-numerics4 | in progress | Prove MI13, reconcile MI12, audit MI11 instruction stores and caller control words. |

## Handoff
- Goal: resolve the published MI11–13 gaps using original Windows 1.1.641 instructions and data only.
- Done: MI13 instruction proof and four literal resource/write vectors in §6.1.1; MI12 staged vector 15 reconciled. Both moved to answered; questions index/check passed.
- Next: audit MI11 point/pick/length instruction store schedule and native control-word callers.
- Evidence: ReadAnim 00808660, SetModel 00801C40, frame copy 00806E41–59, material helpers 00802240/00802070.
- MI13: interior-zero succeeds with scales (.5,1,1,.5,1); initial zoom4 zero succeeds without writing scale4. I=0 fails without rollback.
- Export identity: SimCity 4.exe SHA256 3bc5c7fe807fe5aa24a3abb06e40f13486670bc1150612c09baf4ecedafe1a4b, image base 00400000.
- Main source: C:/Users/tlori/FreeCity4; worktree is this lane directory.
- Manifest: C:/Users/tlori/FreeCity4/build-runs/run4-re-model-instance-numerics4-r1-manifest.json.
- q.py cannot start Bash (E_ACCESSDENIED); use capped native PowerShell/local Python commands.
- No engine/tests, live target repair, subagents, builds, game runs, refs, queue or shared state writes authorized.
