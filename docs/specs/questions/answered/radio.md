# radio: answered questions

<!-- from: re-radio (2026-10-22) -->
- ~~**RA1 - section 2.1:** what event `0x4AB186E9` does to the playing sound (its slot 22 with the position from slot
  20 and the event's value: a seek, a cross-fade, or a restart). [a fade toward the new station]~~
  **ANSWERED 2026-10-27 (re-q-audio):** MATCHES — The event asynchronously fades current volume to data1 over data2 milliseconds, then marks the pending station transition (§2.1).

<!-- from: re-radio (2026-10-22) -->
- ~~**RA2 - section 2.4 step 3:** which radio field's bit 0 sets the stations' shuffle strategy (1 = sort by file
  time). [an option saved in the settings record]~~
  **ANSWERED 2026-10-27 (re-q-audio):** MATCHES — Settings word s[7], at object +0x2C, supplies bit 0: clear selects random shuffle, set selects file-time sorting when stations are built (§2.4).

<!-- from: re-radio (2026-10-22) -->
- ~~**RA3 - section 2.6:** who sets the suspend byte `+0x54` and the "keep playing" flag `+0x6D` the idle tick and
  station selection test. [the SC4 audio event handler]~~
  **ANSWERED 2026-10-27 (re-q-audio):** DIFFERS — SC4 preview messages set suspension through radio slot 30; the call-sign selector itself latches keep-playing before its fade event and does not restore it (§2.6).

