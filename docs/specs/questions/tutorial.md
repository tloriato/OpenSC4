# tutorial: open questions
<!-- Questions about [tutorial.md](../tutorial.md). Append new entries at the END; when one is answered, move it to answered/tutorial.md. Rules: README.md. -->
<!-- from: glue-tutorial4 (2026-10-21) -->
- **TU6 - sections 4.2 / 4.3 and TU1 (registry calls as direct calls):** `cSC4TutorialRegistry` has no vtable (TU1),
  so a replacement `cSC4TutorialSystem` can make `startTutorial`'s `SetActive(GUID, 1)`, `endTutorial`'s
  `SetActive(GUID, 0)` and the city-init handler's entry lookup only by calling `0x004B5D80` and `0x004B5D10`
  itself, in the docs/abi/direct-calls.md format, but neither is there. For each: calling convention (thiscall with
  ECX = `cISC4App` slot 49's result, `&app+0x1C0`?), the stack arguments (count, types: is the GUID a `long` passed
  by value, is `active` a bool byte or a dword?), the bytes the callee pops and the return channel (AL / EAX; for
  `0x004B5D10` a `cItem*` into the list node or null?). Is calling `SetActive` from a DLL safe while the
  preferences block may be saved (slot 17) on the main thread only? [no call is made: SetActive is not made and
  the entry lookup answers "no entry", so a loaded tutorial city keeps is-tutorial set without a window, quirk 2's
  state]
<!-- from: glue-tutorial4 (2026-10-21) -->
- **TU7 - section 4.3 (the debug cheat's subcommands):** the cheat `0x6A382BAF` "Tutorial" handler (`0x0043F870`,
  message `0x230E27AC`) has subcommands reset, save, step, next, quit. Where in the message is the subcommand
  (the cheat string, data field 1/2, a `cIGZString`?), how is it parsed, and what does each one do (which slots /
  fields / registry and window calls, in order)? Only reachable when `cISC4App` slot 21 is true. [the handler is
  registered as specified, logs the cheat and ignores it]
<!-- from: glue-tutorial4 (2026-10-21) -->
- **TU8 - section 4.4 slot 13 (`FilterMessage`'s argument):** "a message of kind 3 with ID `0x287259F6` /
  `0x287259F7` ... data field 2 ... `0x49D993D4`, its field 3": what is the argument's type (a `cGZMessage*`, a
  `cIGZMessage2*`, a window message struct), where are "kind", "ID" and the data fields (offsets, or which
  interface slots), and what are their types? [not decoded; the answer is true, which is the without-window answer]
<!-- from: glue-tutorial4 (2026-10-21) -->
- **TU9 - sections 4.2 / 4.4 (the tutorial window as direct calls):** `cSC4TutorialUIWinProc` (100 bytes,
  constructor `0x00442920`, close `0x00442C80`, `OnTutorialEvent` `0x00443970`) has no interface or vtable rows in
  docs/abi. To keep the original window a replacement needs: the allocator for the 100 bytes (docs/abi/allocators.md),
  the constructor's arguments and the reference count it leaves; which pointer `startTutorial` stores at +0x40 and
  hands to `Init` (and `Init`'s arguments, e.g. the tutorial system, the GUID, the task number, the Lua context); the
  window's vtable with slots 6-11 (`FilterMessage` field, `ShowUIHintsForUsers`, `HandleCatalogMenu`, current task,
  `FilterLeftMouseClick`, `ResetTutorialNumAndIndex`): address, pop, arguments and return channel; and the close
  call's convention. Does the window call back into the tutorial system (which slots) during Init or a task step?
  [no window is created: every slot answers its without-window column]
