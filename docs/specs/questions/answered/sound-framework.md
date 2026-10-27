# sound-framework: answered questions

<!-- from: re-sndsys (2026-10-22) -->
- ~~**SY1 - section 2.2:** what the callback timer's thread call (its slot 12, then slot 5 with 4) sets: a thread
  priority or a time resolution. [thread priority]~~
  **ANSWERED 2026-10-27 (re-q-audio):** MATCHES — The scheduler forwards priority 4 to its worker thread, mapping to Windows above-normal priority +1; it is not timer resolution (§2.2).

<!-- from: re-sndsys (2026-10-22) -->
- ~~**SY2 - section 2.4:** which of the three listener factors pushed to renderer slots 8, 10 and 12 is distance,
  roll-off and Doppler. [in that order]~~
  **ANSWERED 2026-10-27 (re-q-audio):** DIFFERS — Extended-listener slots 8/10/12 set Doppler/roll-off/distance, respectively (§2.4).

