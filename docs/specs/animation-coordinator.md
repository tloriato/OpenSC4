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
[budget.md](budget.md) (`0x6A5FD3FD` BudgetForOccupantChanged).

## 0. Summary

Animated buildings and props (moving parts) cost a tick each. The coordinator decides which of them actually
animate: only those in **blocks of 16 × 16 cells** that overlap the visible area, at zoom 2 or closer, that are
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
registered occupants, enabled := the global `0x00B43CD8`'s byte `+0xF16` ≥ 2 *[inferred: an animation detail
setting]*, refreshing every block when it changes. `Shutdown` unsubscribes, releases the services and empties the
grid.

### 1.2 Filters

- **Animatable** (`0x00445FA0`): the occupant's models have one of the two model classes `0xEA123BE1` /
  `0xAA123BF9` and its exemplar lacks `0x4A70D491`.
- **Shown animatable** (`0x00446000`): the same, for occupants with flag 4 set.

## 2. Registering (`SetOccupantAnimation(occupant, on)`, `0x00446890`) *[compat]*

For an animatable occupant: its block is its position ÷ 256 m in x and z, clamped to the grid (`0x00446070`); `on`
adds it to the block's set, off removes it; the registered count follows. It **should animate** when all hold:

1. animation is enabled and `on`;
2. the view zoom is at least the minimum zoom (2);
3. its block lies in the visible block rectangle;
4. it may animate (`0x00446290`): an exemplar with `0xEA5393ED` needs the power simulator's powered test (slot 31)
   to pass; an exemplar with `0x8A5F88E3` is checked against its budget item *[inferred: funding]*.

When that differs from the occupant's flag `0x20` (animating), the animation is started or stopped
(`0x00446150`): the occupant's message target (IID `0x452294AA`) is added to the animation tick manager (slot 5)
and flag `0x20` set, or removed (slot 6, with `0xC0`) and the flag cleared.

## 3. Updates (`DoMessage`, `0x00446EB0`) *[compat]*

| Message | Handling |
|---|---|
| `0x8A07AE00` | with registered occupants: the blocks under the changed rectangle (`0x00445F10`) are re-evaluated (`0x00446510`) |
| `0x6A5FD3FD` | the occupant (IID `0x26BB767E`) is re-evaluated (`0x004465D0`) |
| `0x231BBF91` | every block is re-evaluated (`0x00446760`) |
| `0xA6B79602` | zoom := the new zoom; with registered occupants, when the rotation changed at a zoom of 2 or closer on both sides, or the zoom crossed 2, the visible block rectangle is recomputed and the blocks re-evaluated (`0x004465A0`) |
| `0xA6B79612` | the visible block rectangle moves: the blocks that left it and those that entered it (`0x004469B0`) are re-evaluated (`0x00446D20`) |

## 4. Quirks

1. Animation follows 256 m blocks, not the exact visible area: an occupant just off screen in a visible block keeps
   animating (§2).
2. The grid is square, sized from the city's x cell count (§1.1).

## 5. Open questions

See [questions/animation-coordinator.md](questions/animation-coordinator.md) AC1–AC2: what message `0x231BBF91` is and
what the enabling byte `+0xF16` of `0x00B43CD8` holds; the exact budget test of property `0x8A5F88E3`.

## 6. Conformance tests

1. **Blocks:** an occupant at (300, 700) m is in block (1, 2).
2. **Zoom:** at zoom 1 nothing animates; at zoom 2 the occupants of visible blocks do.
3. **Scroll:** moving the view by one block starts the entering block's occupants and stops the leaving one's.
4. **Power:** an occupant whose exemplar has `0xEA5393ED` stops when its cell loses power (`0x8A07AE00`).
5. **Flag:** a started occupant has flag `0x20` set and is on the animation tick manager.
