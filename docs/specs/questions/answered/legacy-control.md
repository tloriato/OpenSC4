# legacy-control: answered questions

<!-- from: re-legacycontrol4 (2026-10-21) -->
- **LC6 - section 9.1:** contract of helper 0x0094C1DA (SetGZWinImage, 4 stack arguments) called after an image is set. [unknown]
  **ANSWERED 2026-10-21 (re-legacycontrol4):** DIFFERS — it stores {group, instance} as an 8-byte type-0x8000 variant in control parameter 0xDAA3B860 + n (Slider n = 0); see section 9.4.
<!-- from: re-legacycontrol4 (2026-10-21) -->
- **LC1 - section 1:** the +0x04 sub-object vtable 0x00AD5344 (IID 0x5A356159) has no slot targets in the export; what are its slots? [unknown; implement QI/AddRef/Release thunks only]
  **ANSWERED 2026-10-21 (re-legacycontrol4):** DIFFERS — 8 slots read from the image: QI/AddRef/Release this-4 thunks, two 1-arg return-true stubs, SetArea/GetArea forwarders to the control (false when no control), one 2-arg return-true stub; see section 3.
<!-- from: re-legacycontrol4 (2026-10-21) -->
- **LC3 - section 7.4:** what text does serialiser 0x00965F73 (called by GetScriptFragment) write for a control? [unknown; return false until specified]
  **ANSWERED 2026-10-21 (re-legacycontrol4):** DIFFERS — it writes `<LEGACY clsid=.. iid=.. ` + common words (id, area, fillcolor, caption, captionres, comments, 13 yes/no flags) + per-type words + `>
` and always succeeds; see section 7.6 (per-type word sets open as LC8).
<!-- from: re-legacycontrol4 (2026-10-21) -->
- **LC7 - section 9.5:** exact branch nesting of the GZWinGrid `wingridcell` flags (0x40000/0x2000/0x10000/0x1000) and the meaning of cell fields 7-11 (parser 0x00956464). [inferred from pseudocode; see 9.5 step 11]
  **ANSWERED 2026-10-21 (re-legacycontrol4):** DIFFERS — 0x40000 text branch excludes image/string-resource; 0x2000 image else 0x10000 string resource; 0x1000 record independent; the key scan tests the stale style-word scratch (Q10); record offsets in section 9.5 step 11.
