# advisors: open questions
<!-- Questions about [advisors.md](../advisors.md). Append new entries at the END; when one is answered, move it to answered/advisors.md. Rules: README.md. -->
<!-- from: glue-advisors-impl4 (2026-10-21) -->
- **AD4 - section 12.1.1 (window route as direct calls):** a replacement system can keep the original window only by
  calling `0x41DB50`, `0x41E320`, `0x41D650`, `0x41D710` and `0x41D830` itself (FC4Compat, docs/abi/direct-calls.md
  format), but the spec gives only their addresses and bodies. For each, what are the calling convention, the stack
  arguments (count, types, any `this`-adjusted sub-object pointer), the bytes the callee pops and the return channel
  (AL / EAX / none)? In particular: (a) does the constructor `0x41DB50` take arguments besides `this` (the 0x70 block
  from `0x5E55E0`), and what reference count does it leave (0 or 1) before slot 24's AddRef? (b) is the pointer
  stored at system `+0x5C` the block base or an interface sub-object, and which pointer do Init / Show / Hide /
  teardown expect in ECX? (c) does `0x41E320` return a value slot 24 ignores, and does the win proc (Init or later)
  keep a reference to the system or the city that the teardown releases, so the replacement's Shutdown order matters?
  (d) does anything in the win proc read the system, advisor or advice objects other than through `cISC4AdvisorSystem`
  slot 7, `cISC4Advisor` slots 10/11 and interface slots of the listed advices (which ones)? [no direct call is made:
  the replacement system is not enabled until this is documented; null / no-op is ruled out by AD1]
<!-- from: glue-advisors-impl4 (2026-10-21) -->
- **AD5 - sections 8.2 / 8.3 (the 9 UI Lua entries under a replacement system):** a replacement's `register_api`
  host callback must register all 47 entries of the two collectors, but §8.3 gives the UI group (`camera_jump`,
  `camera_zoom`, `tool_plop_*`, `tool_button`, `window_*`, `window_query`) only as "UI only". (a) What are the exact 9
  names, which are global and which go into `game`, and their arguments / results / behaviour (enough to reimplement
  them through public interfaces)? (b) Or, if the shim is to register the original C functions instead: their
  addresses, that each is a plain `lua_CFunction` (cdecl, one `lua_State*`, int result), and whether any of them
  reads the private global `0x00B207C0` or a system / advisor / advice member (so it would break with the original
  system absent). (c) Does anything other than the system's `register_api` (e.g. the app, the tutorial, MySim)
  register any of the 47 entries into the app's Lua context? [the replacement is not enabled until known; no entry
  is guessed or registered as a no-op]
