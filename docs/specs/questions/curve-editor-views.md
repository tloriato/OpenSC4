# Curve entry-view questions

These are remaining portions of the initial research questions, not reopened
prior-owner questions. Proven subsets now live in the named spec sections;
none of the original broad questions is claimed completely answered.

## Remaining work package 1 — Init UI and observable prefixes

- **CEV-1 — Happiness Init exceptional/reentrant boundary.** §12 now proves the entire ordinary Init path through 0x0066CDA3: raw creation IDs, geometry, acquire/attach/release, graph configuration, refresh then base Init, final AL and ordinary returned-failure prefixes. There is no endpoint enforcement before first refresh; empty live-map refresh remains unsupported. Still open: invalidating/reentrant primitive callbacks and nonreturn/exception prefixes beyond this normal-domain contract; no rollback or safe null recovery is implied. Packed import/readback and interpolation remain §4.1/§6.
- **CEV-2 — GeneralResponse Init suffix, 0x0066D395–0x0066DCDA.** Complete control creation IDs/geometry, graph type/interfaces, initial SetCurrentPoint and refresh ordering, local bounds setup and final Init result channel. §4.2/§5.2/§7 now prove import/readback, point controller/events and display, but not this creation suffix. Distinguish an early base-Init success flag from subsequent creation/query failure prefixes; no rollback is established.

## Remaining work package 2 — primitive aliasing, reentry and numeric environment

- **CEV-3 — Shared assignment and callbacks.** Curve wrappers' acquire/output-write, destructor order and raw stores are in §8.2. Still unqualified: foreign assignment 0x009161DB on same/null variant input, caller-held output aliasing during GetValue's output setter, and callback mutations that invalidate GeneralResponse's saved erase iterator or Happiness's node pointers. Establish only actual reachable caller obligations; do not invent rollback, stable snapshots, successful null assignment or interface-query failure recovery.
- **CEV-4 — Numeric PC/RC and exceptional values.** Exact schedules now appear in §2, §4, §5.2, §6, §7.2 and §8.1. Derive environment-qualified literal ties for Happiness bias extraction (including signed values outside add's accepted range), exact epsilon-border subtraction cases under relevant PC, NaN/sNaN payload/store changes, signed-zero tie association in extrema and masked/unmasked exception prefixes. Existing finite traces are explicitly nearest-mode/normal-domain derivations; do not infer a process-wide x87 control word or a universal portable exception result.
