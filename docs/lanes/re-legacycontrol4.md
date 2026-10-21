| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| re-legacycontrol4: docs/specs/legacy-control.md for owner cGZLegacyControl (45 of 57 fns specified) | Claude / .claude/worktrees/re-legacycontrol4 | ready (docs only) | LC4: 12 interpreters (Btn 2261, OptGrp 2629, TextEdit 1649, Spinner 1422, LineInput 1344, ListBox 1332, SC3WinGen 1260, Combo 1002, TextTicker 799, Text 690, Scrollbar 523, BMP 492); LC8 per-type fragment writers; LC5 |

## Report
- Round 1 (e2534f7b, 788a031c, 5d4b976b): layout, slots, lifecycle, vocabulary, SetParameters, common + Slider (38/57).
- Round 2 (28e58dc6, 0fda97d2, b81957ea): TreeView 9.2, FileBrowser 9.3, image-key helper 9.4 (LC6), Grid 9.5 (41/57 claimed, Grid partial).
- Round 3 (final):
  - 0dc73dce: section 3 control-area vtable 0x00AD5344 (8 slots read from the image: QI/AddRef/Release this-4 thunks,
    two 1-arg return-true stubs, SetArea/GetArea forwarders, one 2-arg return-true stub) - LC1 answered DIFFERS.
    Section 7.6 script fragment format of serialiser 0x00965F73: `<LEGACY ` + clsid/iid resolution (table 0x00B16FA8,
    21 rows) + common words (id, area, fillcolor, caption, captionres, comments, 13 yes/no flags, exact printf forms)
    + per-type writer dispatch (19 IIDs -> writer addresses) + `>
`; always succeeds - LC3 answered DIFFERS.
    Quirks Q8/Q9. New LC8: word sets of the 19 per-type writers (non-owner functions).
  - ee57bca5: Grid wingridcell branch nesting byte-checked (0x0095AA02-0x0095AC59), cell-data record offsets,
    Q10 (stale style-word scratch gates the key scan) - LC7 answered DIFFERS; Grid now complete.
  - 4975c9ca Folders 9.6 (Q11), d335e9f6 FlatRect 9.7, a5d8e2ef Outline 9.8 (Q12).
- Measured: 45 of 57 owner functions specified (honest; round 2 counted partial Grid, now complete). FileBrowser has
  no second pass, so it is whole. 12 interpreters open.
- Questions: answered LC1, LC3, LC7; filed LC8; open LC4 (12 interpreters), LC5, LC8.
- Exact remaining work: LC4 interpreters Btn 0x0094F757, OptGrp 0x00956C46, TextEdit 0x0094E7C8, Spinner 0x00950AF6,
  LineInput 0x009505B6, ListBox 0x0094EE39, SC3WinGen 0x009513BC, Combo 0x0094F36D, TextTicker 0x0094CE28,
  Text 0x0094E516, Scrollbar 0x009503AB, BMP 0x0095002C; LC8 writers (table in 7.6, ~38 KB code); LC5 (WinMgr slot 3).
- REA not used (export + decomp/ + raw image bytes via build-runs/peek.py, tbl.py). No ABI rows, no engine/, no game runs.
