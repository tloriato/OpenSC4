# cHitTrackPlayer — answered questions

**HT2-owned — Playback/definition lifecycle and child visitors.**

**ANSWERED 2026-10-22 (re-hittrack4):** DIFFERS — Spec §6.4–6.7 now defines all
seven owned bodies (1939 bytes): PlayPause's gate/resume/fresh activation and
signed wrapped fade-rate division; UpdatePatch selection and reference changes;
NoteOn's ignored patch result and unused argument; dependent retention and
provider-zero copy; visitor query/action/release order; SetTrackDef's direct,
unchecked provider-slot writes and retained stale trailing providers. The
visitor's 0x456B8F1D is an interface query through GetTrackDef, not the property
lookup suggested in the original question. Exact addresses and independent
discriminating cases are in the named spec sections and §10 cases 13–18.

Moved owned entries: 0x0093AB4B (137), 0x0093ABD4 (137), 0x0093B791 (541),
0x0093B9AE (321), 0x0093BFEF (82), 0x0093C0EC (317), 0x0093EAA1 (404).
HT2's non-owned cleanup boundary 0x0093E60A remains in the open question file;
this partial closure does not answer that external contract.
