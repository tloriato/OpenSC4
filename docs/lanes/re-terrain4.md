| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| re-terrain4 (run4 terrain unblocker: TR38/TR39/TR40 crash blockers, TR35) | `re-terrain4` / (docs lane) worktree `.claude/worktrees/re-terrain4` | **ready** (docs only): round 3 TR41 answered (terrain.md §15.5.13.6); round 1 TR38/TR39/TR40/TR35; round 2 TR33/TR34/TR36/TR37 answered in terrain.md §15.5.13 (+ load path §15.5.13.5); TR41 filed | glue-terrain3: weather DoCompleteSimulation at step 6, step-7 normals + texture update, SetTriangleIndicesForCell(z,x) in flip/hide slots; glue-terrain3: implement step 7 per §15.5.13.6 (read StaticInit statics in place) |



## Report (re-terrain4, 2026-10-21, run4 round 3)

Sources: re_query.py export (instruction bytes), exe data constants and decomp/ for structure; REA not needed.
Ghidra's pseudocode inverts two compare polarities in MarkCliff; every compare was read from the bytes.
Commits 1bc85dba (MarkCliff, proximity, base pass), 678a66ae (brush pass, table origins, TR41 moved).

- **TR41, DIFFERS** (terrain.md §15.5.13.6). MarkCliff 0x00742FC0 flips the cell (slot 93 (x, z, 1), then flag
  0x800) only when both triangles are steep and the other diagonal is not steep on both sides. It then marks the
  steep alternative triangles. Otherwise it marks the corners of the steep original triangles with 0x8.
  ComputeWaterProximityValues 0x007440C0: a dirty-rectangle area gate of 100 or more, then a 4-neighbour distance
  to water capped at P+1 (P = 4). Base pass 0x007482C0: cliff id B414[type] weight 50; water 0x14 below depth 15,
  else 0x13; land B420[type][31-floor(30t)][moisture type]; low dry ground B410 weight 1. Each id is splatted by
  its hash node: response curve, radius and reach per zoom. Family variant ids: (variant<<12)+id. Brush pass
  0x00748950: id 0xE0 splatted around flag-0x400 vertices of each +0x1B0 entry, faded by (1 - age/10000000).
- **Correction:** §15.5.13.3 had 0x13/0x14 swapped; shallow water (< 15) is 0x14.
- Not traced: which reader fills 0x00B4B420 and the hash nodes. The replacement reads those statics in place
  after StaticInit (already required by §15.5.12.2), so step 7 needs no guess.

Next: glue-terrain3 implements step 7 from §15.5.13.2/§15.5.13.3/§15.5.13.6.

## Report (re-terrain4, 2026-10-21, run4 round 2)

Sources: re_query.py export and decomp/ only. REA batch_decompile was denied (dont-ask mode), so nothing came
from REA. Numeric facts and conventions are from the instruction bytes. The new spec text is terrain.md §15.5.13,
plus edits to §2.2 (record layers), §2.3 (`+0x100`) and the §15.5.1 table rows 6 and 7 and its load-path note.

- **TR34, DIFFERS.** `+0x100` is the city's weather simulator. `Init` sets it from city slot 70 and `AddRef`s it.
  Step 6 calls its slot 5, `DoCompleteSimulation` (0x007296F0), with no null test. That function runs once and
  settles the weather before step 7. The replacement must make the call.
- **TR33, DIFFERS.** `0x00743B60` recomputes the face and vertex normals, flags 0x8 and 0x800, and the MarkCliff
  slot-93 flips. The face-normal formulas and the normalisation are taken from the bytes. Quirk: it never
  recomputes the last vertex row `z = cellsZ`. `0x007490B0` does seven things in order: water proximity,
  PadRectangle in place (per-zoom pad), weather UpdateMoisture, clearing the layer ids, marking the view rows dirty
  (`0x00755ED0(zLo, zHi)`, ECX = view), the base-texture pass, the brush-texture pass, then normalisation. Neither
  function posts a message, so a whole-city RedisplayTerrain is not an equivalent.
- **TR36, DIFFERS.** Record `+0x16..+0x21` holds three `{u16 id, u16 weight}` layers. The add rule
  (`trunc(w*255+1)`, max on the same id, replace the lightest) and the normalisation (to 250) are exact.
- **TR37, DIFFERS.** The call is thiscall with ECX = the view base and stack arguments `(z, x)`; the callee pops
  them with `ret 8`. It runs synchronously in the slot when `+0xEC` is set. The flip slots call it only on a
  change; the hide slots call it always.
- **Read vs §15.5.1.** On a load, `Read` stores the view that the serialization system rebuilt; QueryInterface
  0xC9B84E10 returns the base with an AddRef. DoPostCityInit then skips creation and calls `Init` on that object.
  The terrain holds exactly one reference on every path. Lifecycle otherwise unchanged, so no new question was
  needed. A replacement may ignore the saved object *[choice]*: its only saved state is the static grid flag,
  which the view's own Read restores.
- **New question TR41:** the exact rules of MarkCliff and of the two texture passes (0x007482C0, 0x00748950).

Next: glue-terrain3 implements §15.5.13. RE: TR41.

## Report (re-terrain4, 2026-10-21, run4 round 1)

Sources: re_query.py export and decomp/ only (REA not needed). Numeric and calling-convention facts are from the
instruction bytes. Commits: a65deae2 (TR38, TR40), 7c33c5a3 (TR39), a9e0b498 (TR35), then this report.

Questions: TR33-TR40 copied from the glue-terrain3 question file (build-runs/evidence-run4/terrain.md) into
docs/specs/questions/terrain.md. TR35, TR38, TR39 and TR40 moved to answered/terrain.md; QUESTIONS.md regenerated.

- **TR38 (gt3-repl-4 fault at 0x00755C2B), DIFFERS.** The fault is not a missing precondition. The heights copy
  `FUN_007569F0` loops over the delivered rectangle and clamps nothing. The fault registers show `x = 0x4102`,
  `z = 0`, `side 2` and `n = 129`, so the view's statics were correct. The loop ran past `x1 = 128`, which means
  the message the view received had `data1 & 0xFFFF >= 0x4102`. `Init` already copies the whole city itself. The
  rule is in §15.5.12.1: one fresh `cRZMessage2Standard` per post, set up the same way as helper `0x00743380`
  (new 0x2C, AddRef, SetType/Data1..4, MessagePost(msg,0), Release). Never reuse or rewrite a message after
  posting it. Rectangles must lie inside the grid. The array layouts and formulas of `FUN_00755B60` are written
  out there.
- **TR40 (gt3-repl-5 fault at 0x007540A5), DIFFERS.** This is not edge-layer state and not slot 67.
  `DrawSideCliffs` reads texture id 0x11 from the per-detail-level table `0x00B4C670[level]`. That table has
  `0x00B4BC44` entries and is filled from `0x00B4BCD8[terrainType]`. Both statics are written only by
  `cSTETerrain::StaticInit` `0x0074FE40`, which `cSTETerrain::Init` calls. The rule (§15.5.12.2, item 18): run it,
  or reproduce those statics, before the first view Init. §15.5.12.3 lists the view's own edge-layer setup.
  §15.5.12.4 describes slot 67 `DoLighting`. Two §15.5.9 rows were corrected: the tables are per level, and
  BC44 is the texture table length.
- **TR39, DIFFERS.** Nothing outside the terrain calls slot 96. The terrain's own `Read` (`0x00745380`) reads the
  version and the view reference into `+0xE8`, then calls `Load(SC4GetIGZDBSegment(stream))`. So the records
  load during Read (§13.2). Note: `Read` writes the saved view reference into `+0xE8`, and §15.5.1 should be
  checked against that on the load path.
- **TR35, DIFFERS.** The terrain's slot-4 `Shutdown` calls view Shutdown with `ECX` = the view base, then Release,
  and clears `+0xE8`/`+0xEC`. The view destructor also calls Shutdown (item 15a).

Next: implementer (glue-terrain3) fixes posting, runs 0x0074FE40 and calls Load from Read, then sends
gt3-repl-6. RE: TR33 (0x00743B60 = CalculateNormalsAndAssignCliffTextures, likely also the +0x16 writer for
TR36), TR37 (call at 0x007411E1), TR34, and the Read-time view reference vs §15.5.1 creation.
