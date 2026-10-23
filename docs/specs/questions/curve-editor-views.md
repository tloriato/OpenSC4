# Curve entry-view questions

These are remaining portions of the initial research questions, not reopened
prior-owner questions. Proven subsets now live in the named spec sections;
none of the original broad questions is claimed completely answered.

## Remaining Init exceptional boundaries (ordinary contracts proved)

- **CEV-1 — Happiness Init exceptional/reentrant boundary.** §12 now proves the entire ordinary Init path through 0x0066CDA3: raw creation IDs, geometry, acquire/attach/release, graph configuration, refresh then base Init, final AL and ordinary returned-failure prefixes. There is no endpoint enforcement before first refresh; empty live-map refresh remains unsupported. Still open: invalidating/reentrant primitive callbacks and nonreturn/exception prefixes beyond this normal-domain contract; no rollback or safe null recovery is implied. Packed import/readback and interpolation remain §4.1/§6.
- **CEV-2 — GeneralResponse Init exceptional/reentrant boundary.** §13 now proves the entire ordinary Init through 0x0066DCDA: raw control creation/geometry, direct graph interfaces, local extrema overwritten before graph use, early base-Init AL ignored, no owner SetCurrentPoint during Init, final refresh then unconditional AL=1. Ordinary failed-creation/query prefixes are specified without rollback. Still open: invalidating/reentrant callbacks during creation/configuration/refresh and nonreturn/exception prefixes; early base effects can survive such prefixes, and repeated entry follows the observed flag rather than an assumed successful first completion. Import/readback, point controller and display remain §4.2/§5.2/§7.

## Remaining work package 2 — primitive aliasing, reentry and numeric environment

- **CEV-3 — Shared assignment and callbacks.** Curve wrappers' acquire/output-write, destructor order and raw stores are in §8.2. Still unqualified: foreign assignment 0x009161DB on same/null variant input, caller-held output aliasing during GetValue's output setter, and callback mutations that invalidate GeneralResponse's saved erase iterator or Happiness's node pointers. Establish only actual reachable caller obligations; do not invent rollback, stable snapshots, successful null assignment or interface-query failure recovery.
- **CEV-4 — Numeric PC/RC and exceptional values.** Exact schedules now appear in §2, §4, §5.2, §6, §7.2 and §8.1. Derive environment-qualified literal ties for Happiness bias extraction (including signed values outside add's accepted range), exact epsilon-border subtraction cases under relevant PC, NaN/sNaN payload/store changes, signed-zero tie association in extrema and masked/unmasked exception prefixes. Existing finite traces are explicitly nearest-mode/normal-domain derivations; do not infer a process-wide x87 control word or a universal portable exception result.
