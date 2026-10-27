# audio-scape: answered questions

<!-- from: re-scape7 (2026-10-23) -->
- ~~**SCP4 - section 3.4:** the sound definition's `extra`, `flag` and `f` words are stored and copied but never read
  by this owner; does the sound manager read them through the instance pointer it receives? [no: the instance is
  only an id]~~
  **ANSWERED 2026-10-27 (re-q-audio):** MATCHES — The copy address is an opaque instance key; the receiver does not read extra/flag/f through it (§3.4, §3.5).

