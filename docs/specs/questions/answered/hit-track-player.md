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

**HT3-owned — Notifications and spatial conversion.**

**ANSWERED 2026-10-22 (re-hittrack4):** DIFFERS — Spec §5.3 contracts every
branch of notification body 0x0093BD06 (745 bytes), including float re-reads,
target clear/search/QI ordering and the sole propagated false result. Spatial
0x0093A9BC (82 bytes) / 0x0093AA0E (97 bytes) enter at P+8, use three signed
integer coordinate properties, truncate binary32 inputs via int64 `_ftol2`,
keep only low 32 bits and issue an additional invalidation write. There is no
world-distance scaling. Cases 19–21 discriminate these behaviors. The 80-word
external register translation table remains public HT3 work; these three owned
bodies total 924 bytes and do not answer that table.

**HT4 — Pitch and completion-time arithmetic.**

**ANSWERED 2026-10-22 (re-hittrack4):** DIFFERS — Spec §5.4 contracts the
49-byte signed-exponent helper 0x0093AA7E, 454-byte UpdatePitch 0x0093B508 and
212-byte CompletionTime 0x0093ACD6, total 715 bytes. Pitch computes two
independent endpoints from original signed frequency, spills each to binary64,
converts each to low-word int64 truncation, and calls Random(B,A); there is no
clamp or endpoint sort. Zero random bound avoids generator advance. Completion
time is an unsigned provider ratio spilled to binary32 before reference release;
missing/zero-denominator providers return positive zero. Exact operation order,
failure/null edges and boundary cases 22–24 are published in the spec.
