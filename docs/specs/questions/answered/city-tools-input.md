# city-tools-input: answered questions

- **CT17 — §3.14 / §3.2 step 6, preview models, constraints, feedback colours and views.**
  **ANSWERED 2026-10-23 (re-citytools-ct5):** DIFFERS — §3.14 specifies preview creation/filtering,
  transforms, coverage categories, independent visibility/validity, effect reuse and constraint cell
  rectangles. Model highlight selectors are 3 valid / 4 invalid with white RGB and alpha 0.4;
  constraints are blue for water/type 5, red for land/type 6 irrespective of validity. Views are
  Water (`A`) and subway-building placement (`F2`), not subway-network placement (`F1`). CT17-a–h
  provide conformance vectors.

- **CT16 — §3.13, toll-booth side codes, cells and compatible-network mask.**
  **ANSWERED 2026-10-23 (re-citytools-ct5):** DIFFERS — §3.13 specifies the four byte codes,
  alignment bit, six footprint/position cases and configuration IDs. Search checks depth 0 through 2
  inclusive and rejects any flag in `((~(1 << type)) & 0x1FFF) | 0x400000`; the latter flag rejects
  even a same-type occupant. Missing branches pass. CT16-a–e are conformance vectors.

- **CT2 — §3.7, WM slot 19 after mouse movement: which operation?**
  **ANSWERED 2026-10-23 (re-citytools-ct5):** DIFFERS — input-message queue flush,
  identified by the identical call in SC4FlushInputMessageQueue; conditional on held WM,
  unconditional with respect to pick/feedback changes. §3.7 and §6 give the rule/vectors.
- **CT22 — §3.4, body of DoPlopRingEffectFinished.**
  **ANSWERED 2026-10-23 (re-citytools-ct5):** DIFFERS — clear the overlay flag, then
  recalculate coverage if a 3D view is held; no ring release or placeability change. §3.4/§6.
- **CT23 — §3.12, explicit zoning flags and clearing failures.**
  **ANSWERED 2026-10-23 (re-citytools-ct5):** DIFFERS — slope check and skip-size both
  true, checkLotInside false, commit true on both zoning calls; dezoning skipFunds = zero
  tool cost, zoning skipFunds true. Every clearing stage gates the next; no rollback. §3.12/§6.
- **CT24 — §3.11 versus §4, equality at a four-point frontage improvement.**
  **ANSWERED 2026-10-23 (re-citytools-ct5):** MATCHES — detailed formula governs:
  own 3 / best 7 keeps player rotation; own 3 / best 8 changes it. Corrected quirk 10. §3.11/§6.

<!-- re-query-ct5: resolved portions only; residual questions retain their IDs in the open file. -->
- **CT14 (resolved portion) — neighbour kinds/resources and writer ordering.**
  **ANSWERED 2026-10-23 (re-query-ct5):** DIFFERS — §8.16.6 specifies last-successful network-kind priority
  and the one/two-record scripts for every kind; kinds 4/5 count connection records rather than perform a
  separate window lookup. §8.16.7 distinguishes the building pick from later cell-writer ordering. Row fillers,
  MySim branches and complete writer bodies remain open under CT14.
- **CT18 (resolved portion) — occupant null entries and funding related IDs.**
  **ANSWERED 2026-10-23 (re-query-ct5):** DIFFERS — §8.16.1 confirms null occupant entries `0x1026`–`0x1028`
  and gives all six funding lists. Hospital staff stores four IDs: `0x1023`, `0x1024`, `4`, `0x3F800000`;
  truncating this to two changes the original contract. The empty-cell sound remains open under CT18.
- **CT19 (resolved portion) — selection, geometry, commute, curve evaluation and lost-luggage resource.**
  **ANSWERED 2026-10-23 (re-query-ct5):** DIFFERS — §8.16.2–§8.16.5 define ordered developer ranking with
  truncated-demand ties and a `(-1000,-1000)` sentinel; right-aligned wealth geometry using `0x3EAAAAAB`;
  unsigned commute sampling with zero mapped to 255; endpoint-clamped ordered-curve evaluation; and one
  fixed `(type,group,instance)` airport exemplar key. The remaining numerical domains, noise and curve
  construction stay open under CT19.
- **CT20 (resolved portion) — debug enable source and writer phase distinction.**
  **ANSWERED 2026-10-23 (re-query-ct5):** MATCHES — §8.16.7 requires application debug functionality plus
  Shift/Ctrl/Alt, and separates building-pick query-data bookkeeping from the later building, population,
  demand, EQ/HQ and cell writer sequence. Full traffic/debug helper contracts remain open under CT20.

- **CT11 - §8.7, the tip display tail of `DoMessage`:** the exact callout calls (slots 4 and 14), the `SetTip`
  (`0x004C5BA0`) contract and which of 3D view slots 40/41 run on the shown and empty paths. [next RE round]
  **ANSWERED 2026-10-21 (re-querytool4):** MATCHES - §8.11.3: a non-zero icon with a callout shows the callout (slot 4 image, slot 14 secondary image or (null, 0, 0), `SetTip`, then 3D view slot 41 clears the cursor text); otherwise the callout is hidden and 3D view slot 40 sets the cursor text (key `0x2A456CB1`, 0, title, body, 0).

<!-- from: the re-citytools4 lane (2026-10-21) -->
- ~~**CT8 - §8.6, the click modifier bits `0x01` and `0x02`:** which keys are they (Shift / Ctrl, or the mouse
  button state)? [unknown]~~
  **ANSWERED 2026-10-21 (re-query):** MATCHES — Shift (`0x01`) and Ctrl (`0x02`): the window's key-modifier word
  (budget-window.md §4.0, BW5: `0x1` Shift, `0x2` Ctrl, `0x4` Alt). `IsDebugQueryEnabled` (`0x004C4700`) reads the
  three keys itself (`0x10`, `0x11`, `0x12`) (city-tools-input.md §8.6).

<!-- from: the re-citytools4 lane (2026-10-21) -->
- ~~**CT10 - §8.6 step 3:** what is residential simulator slot 59 called with the quarter-resolution cell (a debug
  commute or travel test)? [unknown]~~
  **ANSWERED 2026-10-21 (re-query):** MATCHES — slot 59 is `ToggleTractTracking(int, int)` (SDK order), called with the cell
  divided by 4; its body in this build is a shared empty one, so the Ctrl-click does nothing (§8.6).

<!-- from: the re-citytools4 lane (2026-10-21) -->
- ~~**CT12 - §8.10, the query tool remainder:** 183 functions, 60,518 bytes after round 3 (zone and neighbour
  dialogs, description writers, 87 filler bodies of the §8.12.3 table and their typed workers, route code, rating
  helpers). The lane parked after its third round; needed before the owner can be claimed. [next RE lane]~~
  **ANSWERED 2026-10-21 (re-query):** DIFFERS — the remainder is now §8.12.4 (every filler of the table, by what it shows),
  §8.13 (`PlayOccupantQuerySound`, the zone-based and neighbour dialogs, the dialog's win proc), §8.14 (the building
  values and the subjective ratings) and §8.15 (route mode, outline); §7 claims the owner. The neighbour dialog's
  details and the debug description writers stay at summary level (CT14).

<!-- from: the re-citytools4 lane (2026-10-21) -->
- ~~**CT3 - §3.2 step 5, the two dwords `+0x128`/`+0x12C`:** read from the building-development service's record
  (offsets `+0x30`/`+0x34`) for the chosen building; constructor defaults `0xFFFE7960` (-100000) and -1. What are
  they and who reads them (`CanPlaceLot` or `PlaceLot`)? [unknown]~~
  **ANSWERED 2026-10-22 (re-plop):** MATCHES — the int64 plop cost of the chosen building (`BuildingProfile` `+0x30`,
  `GetBuildingProfilePtr` = service slot 40; exemplar property `0x49CAC341`, building-development.md). `CanPlaceLot`
  reads it: the cost is 0 when it is 0, otherwise `GetLotDevelopmentCost(rect)` plus it (§3.10); `PlaceLot` charges
  that cost (§3.12). The defaults are the int64 −100000 (unknown).

<!-- from: the re-citytools4 lane (2026-10-21) -->
- ~~**CT5 - §3.2 step 4 / §3.7, the bodies of `PlaceLot` (`0x004C1740`), `CanPlaceLot` (`0x004C1110`), the facing
  helpers (`0x004BE750`, `0x004BEB70`, `0x004BFB30`), `SetVisualUserFeedback` (`0x004C2660`),
  `SetConstraintFeedback` (`0x004C2090`) and `GetBuildingPreviewModel` (`0x004C0680`):** unspecified; needed
  before the place-lot tool can be claimed. [next RE round]~~
  **ANSWERED 2026-10-22 (re-plop):** DIFFERS — the bodies are §3.9 (`GetLotCenterAndCellCoverage`), §3.10 (`CanPlaceLot`,
  `CanDemolishLot`), §3.11 (`GetAutomaticFacingAndCellCoverage`, the street count) and §3.12 (`PlaceLot`); §7 claims
  the tool. The toll-booth picker (§3.13) and the preview and feedback drawing (§3.14) stay at summary level (CT16,
  CT17).

<!-- from: the re-citytools4 lane (2026-10-21) -->
- ~~**CT1 - §2.3, what the 3D view does with a handler's result:** the handlers return a bool, read as "handled".
  How does `cSC4View3DWin` walk its control stack and persistent controls on an input event (top first? stop at
  the first true? does `ShouldStack` false change the push)? Owner: the 3D view's input forwarding, not specified
  anywhere. [top control first, stop on true *[inferred]*]~~
  **ANSWERED 2026-10-27 (re-q-city):** DIFFERS — "top control first, stop on true" holds only for key events and part of the rest. The view walks both its lists from the last element to the first: key down/up go to the stack (first taker) and then to every persistent control; mouse presses and releases to the persistent controls (first taker) and then the stack (first taker); mouse move to every persistent control, then the stack first taker; the wheel to the stack first taker, then (if none took it) the persistent controls and the zoom; mouse exit to the top stack control and every persistent control. `ShouldStack` is never consulted: the push accepts any control whose `Init` succeeds (§2.3, view3d-window.md §6c).

- ~~**CT6 - §8.2, the query tool's `+0x98`:** the constructor stores 3; no function of §8 reads it. Who reads it
  (the dialog or route code)? [unknown]~~
  **ANSWERED 2026-10-27 (re-q-city):** DIFFERS — it is read: `+0x98` is the route-step filter used by `DoesRouteCrossCell` (1: every step except subway steps, 2: only subway steps, 3: every step). The path and traffic-info builders set it (1, or 3 for transport buildings in group `0x1003`; 2 for subway network occupants, else 1); the constructor value 3 means "every step" (§8.2, §8.15).

- ~~**CT7 - §8.4 step 5, the legend dialog:** the exact script key passed to `SC4CreateWindowFromScript` and the
  placement arithmetic (window slots 41, 44, 45, 56) of the two legend panes; `SetLegendMinimized`
  (`0x004C4630`) swaps their visibility and moves the minimised pane. [next RE round]~~
  **ANSWERED 2026-10-27 (re-q-city):** MATCHES — the script key is `{0, 0x96A006B0, 0xABB0120F}` with child ID `0x6BB92BCB`; the full pane is moved to `(viewWidth − paneWidth − 2, 2)` and shown, the small pane to `(F.right − M.width, F.top)` and hidden, and the shell is removed from the view window. `SetLegendMinimized` hides the full pane, shows the small one and re-places it; the reverse just swaps visibility (§8.4 step 5).

- ~~**CT9 - §8.6 step 2, the automaton and stoplight hits:** what do the vehicle manager's slot 34 and the traffic
  network's slot 37 do with the picked automaton and stoplight prop (select or track it, toggle the light)? (City
  slot 43 is `GetVehicleManager` and slot 40 `GetTrafficNetwork` in the SDK; the automaton filter accepts vehicles
  `0xC7CC75F1` and pedestrians `0x496E5783`, `0x004C3AB0`; re-query 2026-10-21.) [unknown]~~
  **ANSWERED 2026-10-27 (re-q-city):** MATCHES — both are Shift clicks. The automaton goes to the vehicle manager's slot 34, `SetCameraFollow` (the camera tracks it, zooming to step 4 from below 3 when the auto-zoom render property is on); the stoplight prop goes to the traffic network map's slot 37, `SwitchStoplightStates` (flips the signal phase of the intersection at the prop's cell and reloads the countdowns) (§8.6, §8.6.1).

<!-- from: the re-querytool4 lane (2026-10-21, round 2) -->
- ~~**CT13 - §8.11.3/§8.11.4, the tip gauge data:** the meaning of callout slot 4's values (12, 10) and slot 14's
  value 5 with the fraction (an icon size and a gauge bar?). Round 3 named the services in §8.11.4 (police
  `0x00B43D98`, fire `0x00B43D78`, pollution `0x00B43D9C`, slots per protection.md / pollution.md), so only the
  callout slots remain. On the callout's primary vtable `0x00AB69D0`, slot 4 (`0x007982C0`) takes one held pointer
  (`+0x130`) and slot 14 (`0x007984D0`) a held pointer (`+0x140`) and a float clamped to [0, 1]; the integer
  arguments (12, 10) and (5) of §8.11.3 do not fit those shapes, so the interface the tip display calls through
  must be checked again. [unknown; callout slots only]~~
  **ANSWERED 2026-10-27 (re-q-city):** DIFFERS — the values are not an icon size and gauge: callout slot 4 is `(skin, padX, padY)`, so 12 and 10 are the horizontal and vertical padding around the text, and the "icon" of the tip tables is the callout's skin bitmap; slot 14 is `(meter, count, fill)`, so 5 is the meter's repeat count and the fraction is its fill (clamped to 0–1). The interface was the right one: callout-box.md §2 already gives these shapes (§8.11.3).
