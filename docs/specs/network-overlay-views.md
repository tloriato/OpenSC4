# Spec: network overlay views (`SC4PathInfoView`, `SC4TrainJunctionView`)

Status: **behaviour spec**, clean-room input. This document describes *behaviour*, not code. It was written by the
analysis team from the Windows 1.1.641 executable (read in the instruction bytes and in a Ghidra 12.1.4 project of
the Steam build, whose code equals the GOG build's; evidence in `docs/lanes/re-dates.md`). Implementers must work
from this spec, the public LGPL gzcom-dll SDK headers and general knowledge only. They must **not** consult decompiled
or disassembled code (see [CONTRIBUTING.md](../../CONTRIBUTING.md)).

Normative words: **must**, **should**, **may**. Tagged notes: *[compat]* is required for compatibility with the
original game and its plugins; *[choice]* is a behaviour we chose where the original was arbitrary or undefined;
*[inferred]* is consistent with all evidence but not proven; *[quirk]* is surprising original behaviour that is
kept on purpose; *[compat-limit]* depends on something FreeCity4 cannot reproduce, so only the rule is matched.

Related specs: [render3d.md](render3d.md) (§11 view objects), [traffic-network.md](traffic-network.md) (§7.3
the path info, network occupant slot 49 `GetPathInfo`), [automata.md](automata.md) (§9, the cheat handler and
U-Drive-It), [draw-context.md](draw-context.md), [overlay-views.md](overlay-views.md).

## 0. Summary

Two view objects draw over the network. The **path-info view**, a debugging aid that the automata manager's cheat
handler turns on, draws every network path of the visible cells as coloured lines with arrowheads. The
**train-junction view**, made by the U-Drive-It train control, shows at each rail junction ahead the routes the
player can choose, as arrow pictures, with the chosen one highlighted.

## 1. The path-info view (`SC4PathInfoView`) *[compat]*

- Made and initialised by the automata manager's cheat handler (`0x0050DD30`, automata.md §9). `Init`
  (`0x00542EE0`): an untextured blended state (source alpha, one minus source alpha, alpha test "greater" than 0),
  without depth write; with the depth test less-or-equal unless the view was told to ignore depth.
- `Update(context)` (`0x00542FC0`), each draw:
  1. A pulse byte advances by 5 per draw (wrapping), starting at 0.
  2. Over the network manager's visible cell rectangle, each network occupant on each cell gives its path info
     (occupant slot 49), and each of its paths is drawn when the view's path-type filter (`+0x18`, −1 = all) is −1
     or equals the path key's bits 16–23.
  3. A path whose key has any of bits 28–31 set is drawn as a red cross, 4 m each way, at its first point.
  4. Otherwise its points (at most 25) form one line strip, each point 0.5 m above the path's height (or the
     ground under it when the path has no heights), in a colour (alpha 200) chosen by the key's bits 24–27: 1 light
     green, 2 blue, 4 yellow, 6 and 7 magenta, others light red; a path with flag bit 1 instead pulses from red to
     white.
  5. An arrowhead of two 3 m lines at ±45° marks the path's end, along its last segment longer than 1 mm.
  6. When the key's byte 1 is `0xFF`, a bar 6 m across, perpendicular to the first segment, marks the start; when
     its byte 0 is `0xFF`, the same marks the end.

## 2. The train-junction view (`SC4TrainJunctionView`) *[compat]*

- Made by the U-Drive-It train control (`cSC4ViewInputControlPlayerDrive::Init`, `0x00565A10`). `Init`
  (`0x005633C0`): the draw state, a table of the unit directions between the four edge midpoints of a 16 m cell
  (west (0, 8), north (8, 0), east (16, 8), south (8, 16)), six arrow pictures (FSH group `0x1ABE787D`, instances
  `0x6BE09921`–`0x6BE09926`: three turns, each plain and selected) and two marker models (type
  `0x5AD0E817`, group `0xBADB57F1`, instances `0x8BB70000` and `0x8BB80000`).
- `Update(context)` (`0x005657A0`): over the visible cell rectangle, for each network occupant and each of its four
  entry edges, the network manager (slot 41) gives the routes leaving that edge and the one chosen; a junction with
  routes is drawn (`DrawTrainJunction`, `0x00564BE0`).
- **Drawing a junction:** its transparency is 0.8 at the junction the player's train is at or heads for, else 0.5.
  Each route but the chosen one gets its plain picture (`GetPathBitmap`, `0x00564A60`): with d the unit direction
  of the route's last segment (taken over its last three points, or the whole route when that is shorter than
  0.25 m) and e the entry direction, c = d.x e.z − d.z e.x chooses the picture pair `0x6BE09923`/`24` below −0.25,
  `0x6BE09925`/`26` up to 0.25 (no turn) and `0x6BE09921`/`22` above (plain, selected) *[inferred: the two turn
  directions]*. The picture is a quad at the entry edge, 12 m into the cell (`GetArrowBitmapQuad`, `0x005637E0`), at the
  ground height (3 m higher on elevated rail, track type 7), each one a little above the last (+0.01 per quad). The
  chosen route is drawn last in its selected picture, then the marker model is placed there 0.2 m up (3 m more on
  elevated rail) and drawn.

## 3. Users

The automata manager (cheat) and the U-Drive-It player train control.

## 4. Quirks

1. Only the first 25 points of a path are drawn by the path-info view (§1).
2. Junction arrows are stacked by small depth steps rather than sorted (§2).

## 5. Open questions

See [questions/network-overlay-views.md](questions/network-overlay-views.md) NOV1: which cheat turns the path-info
view on, and what the path key's bytes and type nibble denote in traffic-network.md's terms.

## 6. Conformance tests

1. **Colour:** a path of type nibble 4 is drawn yellow with alpha 200.
2. **Limit:** a path of 40 points draws a strip of 25 points.
3. **Arrow choice:** entry direction e = (1, 0) and a route ending along d = (0, −1) give c = 1: the pair
   `0x6BE09921`/`0x6BE09922`; a route ending along e gives c = 0: the pair `0x6BE09925`/`0x6BE09926`.
4. **Transparency:** a junction away from the player's train draws at 0.5.
