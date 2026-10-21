# Analysis export recovery, 2026-10-21

After train42 landed, coverage regeneration found ghidra/export/windows missing. It was rebuilt from the existing FreeCity4 Ghidra project with -noanalysis -readOnly and FC4ExportTables.java. coverage.py verified the GOG 1.1.641 executable hash. The source of the missing directory has not yet been established.

The saved project has newer function boundaries than the missing export: game functions 29,470 -> 29,545 and game bytes 5,586,376 -> 5,585,702. The four rounded coverage percentages remain 44.3/34.3/16.1/17.6. This analysis input refresh is not source implementation or replacement progress; byte/function deltas spanning it need that qualification.

| Input | Previous SHA256 prefix | Restored SHA256 prefix |
|---|---|---|
| functions.tsv | 6c7bf06c03862295 | 49cb20c46bf1108e |
| function-details.tsv | 5a03b2d280f407d0 | 64e295b03826034a |
| calls.tsv | b9b0e3b2157d0412 | 3975d7030d997e48 |
| data-references.tsv | 6b5ec13c66dd96de | 056c6ca265d810c6 |
| instructions.asm | cbc5795d08d336cb | 946a3559f8f4c7a1 |
