# Spec: animation coordinator (`cSC4AnimationCoordinator`)

Status: **behaviour spec**, clean-room input. This document describes *behaviour*, not code. It was written by the
analysis team from the Windows 1.1.641 executable (read in the instruction bytes and in a Ghidra 12.1.4 project of
the Steam build, whose code equals the GOG build's; evidence in `docs/lanes/re-dates.md`). Implementers must work
from this spec, the public LGPL gzcom-dll SDK headers and general knowledge only. They must **not** consult decompiled
or disassembled code (see [CONTRIBUTING.md](../../CONTRIBUTING.md)).

Normative words: **must**, **should**, **may**. Tagged notes: *[compat]* is required for compatibility with the
original game and its plugins; *[choice]* is a behaviour we chose where the original was arbitrary or undefined;
*[inferred]* is consistent with all evidence but not proven; *[quirk]* is surprising original behaviour that is
kept on purpose; *[compat-limit]* depends on something FreeCity4 cannot reproduce, so only the rule is matched.

Related specs: [model-maker.md](model-maker.md) (§3.1 creates the coordinator, §2 holds it; the maker also calls it when
an occupant is shown), [camera.md](camera.md) (§2.7, the view messages), [power.md](power.md) (powered tests),
[budget.md](budget.md) (`0x6A5FD3FD` BudgetForOccupantChanged; §5.3 the budget items),
[city-detail.md](city-detail.md) (§2, the Graphic Detail byte).

## 0. Summary

Animated buildings and props (moving parts) cost a tick each. The coordinator decides which of them actually
animate: only those in **blocks of 16 × 16 cells** that overlap the visible area, at zoom 2 or closer and at medium or high Graphic Detail, that are
powered when they need power and funded when they depend on a budget. It starts and stops each occupant's animation
on the animation tick manager as the view moves and as power and budgets change.

## 1. The object *[compat]*

- 0x5C bytes, created by the model maker (`0x00447750`); vtable `0x00A86690`: slot 3 `DoMessage` (`0x00446EB0`),
  slot 4 the deleting destructor (`0x00447B00`, body `0x00447A00`).
- State: initialised (`+0x08`); the animation tick manager, the occupant manager, the power simulator
  (`0x00B43DA0`), the budget simulator (`0x00B43D64`) and the lot manager; two occupant filters; the view zoom
  (`+0x28`, initially 4) and the minimum zoom (`+0x2C`, 2); the visible block rectangle (`+0x30`); the block grid
  (`+0x40`, rows of blocks, each a set of occupants and a count); blocks per side (`+0x4C`); the number of
  registered occupants (`+0x50`); animation enabled (`+0x54`, initially true).

### 1.1 `Init` (`0x00447B20`) and `Shutdown` (`0x00447900`)

`Init` (once) builds the filters, takes the services above, subscribes to `0x8A07AE00` (power changed in a
rectangle), `0x6A5FD3FD` (BudgetForOccupantChanged), `0x231BBF91`, `0xA6B79612` (ViewTChange3) and `0xA6B79602`
(ViewZRChange3), reads the view zoom, sizes the grid to (city cells x ÷ 16)² blocks *[inferred: square cities]*,
and computes the visible block rectangle from `SC4ViewUtilities`' visible world rectangle (world ÷ 256 m). With
registered occupants (count not 0), enabled is re-read from the preferences object (global `0x00B43CD8`, §1.3) as in
§1.3. `Shutdown` unsubscribes, releases the services and empties the grid.

### 1.2 Filters

- **Animatable** (`0x00445FA0`): the occupant's models have one of the two model classes `0xEA123BE1` /
  `0xAA123BF9` and its exemplar lacks `0x4A70D491`.
- **Shown animatable** (`0x00446000`): the same, for occupants with flag 4 set.

### 1.3 The animation switch (`0x00B43CD8`, byte `+0xF16`) *[compat]*

The global `0x00B43CD8` is the preferences object and its byte `+0xF16` is the **Graphic Detail** option
(city-detail.md §2): 1 low, 2 medium, 3 high. The coordinator's enabled flag (`+0x54`, initially true) is derived
from it only when there are registered occupants (`+0x50` is not 0):

- enabled := the preferences object exists and the byte is above 1, so animation runs at medium and high detail and
  is off at low detail (a missing preferences object counts as off);
- when that differs from the stored flag the new value is stored and, if the coordinator is initialised, **every
  block** is re-evaluated (§2) over the whole grid; if it does not differ nothing else happens.

This is evaluated in `Init` (`0x00447DC6`–`0x00447E15`) and on the message `0x231BBF91` (§3). During the first
`Init` the coordinator is not yet initialised, so the flag is stored without a refresh. With no registered occupants
the flag is left as it was (it stays stale until the first re-evaluation after an occupant registers).

## 2. Registering (`SetOccupantAnimation(occupant, on)`, `0x00446890`) *[compat]*

For an animatable occupant: its block is its position ÷ 256 m in x and z, clamped to the grid (`0x00446070`); `on`
adds it to the block's set, off removes it; the registered count follows. It **should animate** when all hold:

1. animation is enabled (§1.3) and `on`;
2. the view zoom is at least the minimum zoom (2);
3. its block lies in the visible block rectangle;
4. it may animate (`0x00446290`): an exemplar with `0xEA5393ED` needs the power simulator's powered test (slot 31)
   to pass; an exemplar with `0x8A5F88E3` needs its funding test to pass (§2.1). Both tests are applied in this
   order; an occupant lacking both properties always passes.

### 2.1 The funding test (property `0x8A5F88E3`) *[compat]*

The property needs at least two values: the first is a budget **purpose ID** `P`, the second an unsigned
**funding threshold** `T` in percent. With fewer than two values (or no property) nothing is tested and the occupant
passes.

1. Funding `f` (`0x00446210`): the lot manager's slot 14 `GetOccupantLot` gives the lot of the occupant, the lot's
   slot 48 its building, and the budget simulator's slot 51 `GetBudgetItemForPurpose(holder, P, &item)` (budget.md
   §5.3: the first budget item of that **building's** property holder whose purpose is `P`) its item. `f` := the
   item's local funding fraction (1.0 = fully funded). With no lot, no building or no item, `f` := 0.
2. The occupant passes unless `T` is greater than `100 × f`: the product and `T` (converted from unsigned) are
   compared at full x87 precision, equality passes and an unordered result (a NaN fraction) also passes.

So an unfunded or unregistered building (`f` = 0) fails every test with `T` above 0 and passes only `T` = 0. The test
looks at the funding of the building on the occupant's lot, not at an item of the occupant's own exemplar.

When the result differs from the occupant's flag `0x20` (animating), the animation is started or stopped
(`0x00446150`): the occupant's message target (IID `0x452294AA`) is added to the animation tick manager (slot 5)
and flag `0x20` set, or removed (slot 6, with `0xC0`) and the flag cleared.

## 3. Updates (`DoMessage`, `0x00446EB0`) *[compat]*

| Message | Handling |
|---|---|
| `0x8A07AE00` | with registered occupants: the blocks under the changed rectangle (`0x00445F10`) are re-evaluated (`0x00446510`) |
| `0x6A5FD3FD` | the occupant (IID `0x26BB767E`) is re-evaluated (`0x004465D0`) |
| `0x231BBF91` (PreferencesChanged, posted when an options dialog is accepted: application.md §13.3) | with registered occupants the enabled flag is re-read from the Graphic Detail byte (§1.3); only when it changed, every block is re-evaluated (`0x00446760`, `0x004465A0`) |
| `0xA6B79602` | zoom := the new zoom; with registered occupants, when the rotation changed at a zoom of 2 or closer on both sides, or the zoom crossed 2, the visible block rectangle is recomputed and the blocks re-evaluated (`0x004465A0`) |
| `0xA6B79612` | the visible block rectangle moves: the blocks that left it and those that entered it (`0x004469B0`) are re-evaluated (`0x00446D20`) |

## 4. Quirks

1. Animation follows 256 m blocks, not the exact visible area: an occupant just off screen in a visible block keeps
   animating (§2).
2. The grid is square, sized from the city's x cell count (§1.1).

## 5. Open questions

AC1 (what message `0x231BBF91` and the byte `+0xF16` of `0x00B43CD8` are) and AC2 (the exact test of property
`0x8A5F88E3`) are answered in §1.3 and §2.1 and archived in
[questions/answered/animation-coordinator.md](questions/answered/animation-coordinator.md). The remaining open
questions are in [questions/animation-coordinator.md](questions/animation-coordinator.md).

## 6. Conformance tests

1. **Blocks:** an occupant at (300, 700) m is in block (1, 2).
2. **Zoom:** at zoom 1 nothing animates; at zoom 2 the occupants of visible blocks do.
3. **Scroll:** moving the view by one block starts the entering block's occupants and stops the leaving one's.
4. **Power:** an occupant whose exemplar has `0xEA5393ED` stops when its cell loses power (`0x8A07AE00`).
5. **Flag:** a started occupant has flag `0x20` set and is on the animation tick manager.
6. **Detail switch:** with occupants registered, changing Graphic Detail from 3 to 1 and delivering `0x231BBF91`
   stops every animating occupant; from 1 to 2 starts those in visible blocks that pass §2. Delivering the message
   again without a change re-evaluates nothing.
7. **Funding:** an occupant whose property `0x8A5F88E3` is {P, 50} and whose lot's building has a budget item for
   P at 60 % funding animates; at 40 % it does not; at exactly 50 % it does; with no item for P it does not.
   With {P, 0} it always does.
