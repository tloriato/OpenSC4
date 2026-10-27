# mysim-tools: answered questions (archive)

<!-- from: re-mysimtools7 (2026-10-23) -->
- ~~**MST1 - §3.1, §3.2:** what are `cISC4BuildingOccupant` slots 9 and 11, whose pointed-to values the tools test
  (a float ≥ 1.0; a value equal to, or not equal to, 1)? The spec reads them as "fully built" and "residential
  purpose". [inferred]~~
  **ANSWERED 2026-10-27 (re-q-city):** MATCHES — slot 9 (`0x00458810`) hands out the address of the building's completion float (field `+0x58`, which slot 10 sets and the lot developer raises to 1.0 when construction ends), so "≥ 1.0" is "construction finished". Slot 11 (`0x00459250`) is `GetBuildingProfile`: it returns the building type's shared profile, whose first field is the purpose; purpose 1 is residence, so "= 1" is "a residential building" and the job tool's "not 1" is "not residential" (§3.1, §3.2).

<!-- from: re-mysimtools7 (2026-10-23) -->
- ~~**MST2 - §3.1, §3.2:** the lot slots the tools read: 41 (a 1–3 value used as the wealth), 33 (must be 3), 35
  (must be below 4) and 63 (called with the Sim's wealth, must be above 0). Which `cISC4Lot` methods are they?
  [inferred]~~
  **ANSWERED 2026-10-27 (re-q-city):** MATCHES — lot slot 41 is `GetOccupantWealth` (byte `+0x82`, 0–3), slot 33 `GetState` (byte `+0x7F`; 3 is "occupied", a refinement of the inferred "built"), slot 35 `GetCondition` (byte `+0x80`; 4 is the last step of an abandoned lot's decline, so "below 4" excludes it) and slot 63 `GetTravelJobs(wealth)` (the lot's job supply for wealth 1, 2 or 3 as a float, 0.0 for any other wealth) (§3.1, §3.2).

<!-- from: re-mysimtools7 (2026-10-23) -->
- ~~**MST3 - §5.3:** pylon slots 7 and 20: the filter needs slot 7 ≠ 0, and slot 20's result must QI to `0x0A1DA42F`
  (read as the owning Sim). Are they a UI-state getter and `GetPylonOwner`? [inferred]~~
  **ANSWERED 2026-10-27 (re-q-city):** MATCHES — pylon slot 7 is `GetUIState` (field `+0x6C`; 0 also makes the pylon unpickable, 1 normal, 2 active, 3 hovered) and slot 20 is `GetPylonOwner` (field `+0x1C0`, set by slot 19); the filter needs the owner non-null and answering `0x0A1DA42F` (§5.3).
