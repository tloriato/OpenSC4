# command-line: answered questions

<!-- from: core-cmdline4 (2026-10-21) -->
- ~~**CL2 - §4.3, §3 slot 9:** slot 11 (`GetIndexOfSwitch(name, start)`) with an **empty** `name`: §4.1 says slot 8
  returns false for an empty name, §4.3 has no such rule. Read literally, an empty name matches a lone prefix
  argument (`-`, `/` or `\`, length 1), so `IsSwitchPresent("")` (slot 9) is true on `[-]`. Is that the original
  behaviour, or does slot 11 also return −1 for an empty name? (FreeCity4 core implements the literal reading.)~~

**ANSWERED 2026-10-27 (re-q-framework):** DIFFERS — Slot 11 requires at least two argument characters, so an empty name always gives −1 and slot 9 gives false, even for a lone prefix (§4.3).
