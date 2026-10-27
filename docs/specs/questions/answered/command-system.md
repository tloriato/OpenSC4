# command-system: answered questions

<!-- from: re-commands (2026-10-19) -->
- ~~**GC4 - §1.5:** which status values do the handlers use besides 2? A survey of the `SetStatusParameterValue` calls
  in the handlers would give the vocabulary.~~

**ANSWERED 2026-10-27 (re-q-framework):** MATCHES — Verified framework handlers write 0 for success, 1 for an unsatisfied window poll or registered message wait, and 2 for failure; these uses do not restrict plugin status integers (§1.5).

<!-- from: core-commands4 (2026-10-21) -->
- ~~**GC5 - §1.4 `CompareParameterString` with ID −2:** how does the "any parameter" walk visit the parameters: the
  map itself (so a parameter under `0xFFFFFFFF` is compared) or slots 13/14 (which stop before it)? And a parameter
  stored under `0xFFFFFFFE` itself would recurse through slot 11 forever; does the original guard it? Same question
  for the "each parameter in ID order" walks of §3.6. [the −2 walk uses the map, every ID, and compares a parameter
  under `0xFFFFFFFE` directly; the §3.6 walks stop before `0xFFFFFFFF` as §1.4 describes]~~

**ANSWERED 2026-10-27 (re-q-framework):** DIFFERS — The −2 search walks the map, including 0xFFFFFFFF when reached, but recursively restarts on a stored −2 without a guard. The §3.6 serialization walks use slots 13/14 and omit 0xFFFFFFFF (§§1.4, 3.6).
