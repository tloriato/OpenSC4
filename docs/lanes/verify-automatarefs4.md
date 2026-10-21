| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| verify-automatarefs4: the 238 ref warnings of the saved-city control run (controller manager teardown) | engine implementer, `.claude/worktrees/verify-automatarefs4` | ready: vam4-c1 (exact sg4-c1 copy, with save) clean ref_warnings=0 to 2032-01-01; vam4-n1 (no save) clean 0; gate PASS | merge; rerun the simgrids joint (sg4-j1) on top; answer AT21 |

## Report

**Cause.** sg4-c1 (glue-simgrids4, 36 subsystems without simgrids, script `2031-06-01 save`) logged
238 "released last reference" warnings, all at `HostRelease` (cFC4Automata.cpp:53) during city teardown. The
refaudit summary pins them: the controller manager core holds 214 generator (`0x4A1628C5`, vtable +0x695F00) and
25 attractor (`0x6A1628BA`, +0x695E48) references acquired at load (ReadRef's audited QueryInterface, net +214 / +25
at the Shutdown summary) and releases them all in `fc4_automata_cm_destroy` from
`~cFC4AutomataControllerManager`: 214 + 24 of those releases are the last.

**Lifetime contract (public specs).** automata.md §3.4: the manager keeps the reference its Read returned; §7.1
(AM7 step 3): the manager shuts down and releases the controllers it drops. foundations.md §2.7 SV1: the load's
object table holds +1 on each loaded object only until the loaded segment closes, and a save closes it
(building-development.md table, "loaded, then saved at least once": the holder's release is the last). So in a
saved city the manager's reference is legitimately the last one; in an unsaved city it is not.

**Fix (7893f2db).** The controller manager's host gets `user = this` and its own `CmHostRelease`: while the
destructor runs (`destroying_`), a game object's release is `FC4_RELEASE_AT_DESTRUCTION` (the audit still
requires our sites to have acquired the reference, so nothing unacquired is hidden); every other cm release (Read
replacing the record, `set_clock`, `RemoveControllers`) stays plain `FC4_RELEASE`. No class list, no leaked
reference, no skipped release, no C ABI change. Test seam op 69 `kFC4SeamAutomataCmTeardown` +
plugin_load_test: the destructor's last release gives 0 warnings and leaves the count 0; a replaced clock reference
released outside destruction still warns (1); with an extra holder the count stays 1 and nothing warns.

**Measured.**
- Gate: baseline `qbuild all` PASS (cargo 3756 passed, clippy 0, ctest 63/63); after the fix the same.
- vam4-c1 (byte copy of sg4-c1.req, label changed; script build-runs/sg4-save-j1.txt): clean, ref_warnings 0,
  2032-01-01; save ok=1 at 2031-06-01; refaudit: generator acquired 214 held to Shutdown, then 214 releases, 214
  last, all declared (temporaries 214); attractor 25 / 24 likewise. Same counts as sg4-c1, now declared.
- vam4-n1 (the same without the save): clean, ref_warnings 0; no last-reference releases of either class (the
  load table still holds them, as SV1 says).

**Question.** AT21 (docs/specs/questions/automata.md): where the original controller manager drops its
controller / occupant / queue / clock references at city close, and whether controllers are shut down first.

**Next.** Merge; rerun the joint 36+simgrids save run (sg4-j1) on top of this branch; when AT21 is answered,
move the release to the point it names (and shut controllers down first if required).
