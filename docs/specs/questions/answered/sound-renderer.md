# sound-renderer: answered questions

<!-- from: re-sndrender (2026-10-22) -->
- ~~**SRN1 - section 4.5:** what the 1–6 level at `+0xB8` of a 2D sound controls (a streaming chunk count, a quality
  level or a voice priority). [voice priority]~~
  **ANSWERED 2026-10-27 (re-q-audio):** DIFFERS — The level is stored at object +0xC0 through the +8 interface and scales streaming allocation in groups of six chunks; it is not voice priority (§4.5).

<!-- from: re-sndrender (2026-10-22) -->
- ~~**SRN3 - section 4.5:** the play-position formula of `0x00876255` (it multiplies the sample count by 8000 before
  dividing by the rate, then by 1000 when stopped). [an ms value with a unit slip]~~
  **ANSWERED 2026-10-27 (re-q-audio):** DIFFERS — The operation calculates remaining time using bytes divided by bits/sample, omits channels, returns all-ones for playing streams, and adds a stopped-state factor of 1000 (§4.5).

