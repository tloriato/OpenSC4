# foundations: open questions
<!-- Questions about [foundations.md](../foundations.md). Append new entries at the END; when one is answered, move it to answered/foundations.md. Rules: README.md. -->

<!-- from: From the return-channel audit (2026-10-16) -->
- **HG2:** cISC4App (`vt:cSC4App::vftable_3`) return channels are in no spec: slot 10 RequestQuit (the autotest falls
  back to AutotestFinish when it reads false), slot 13 LoadCity, slots 15/16 SaveCity, slot 21
  GetDebugFunctionalityEnabled (it gates the cheat-code registration in `cFC4Budget.cpp` and `cFC4Simulator.cpp`).
  Does each set AL/EAX, and with what meaning? [SDK bool trusted; LoadCity and SaveCity only logged; a garbage
  RequestQuit true is covered by the 90 s quit grace]

<!-- from: From the return-channel audit (2026-10-16) -->
- **HG3:** framework slots read as bool with no spec: `vt:cGZCOM` slot 3 GetClassObject (every site also tests the out
  pointer), cIGZPersistResourceManager slots 4 GetPrivateResource and 14, cIGZDate slots 4 SetDayNumber and 6
  SetMonthDayYear (passed through by cFC4Date / cFC4Simulator), cIGZVariant slot 54 CreateValString, cIGZWin slot 36
  GetChildAs. Is each a real bool? [SDK bool trusted]

<!-- from: glue-rng4 native RNG contract audit (2026-10-22) -->
- **RN4 - sections 6.1, 6.4 and 8.6, native-original fixture contract:** Can either exact owner,
  `cRZRandom` or `cRZRandomSuper`, be exercised independently inside the supported GOG 1.1.641 process through
  a published SDK/service/factory route or a fully typed supported-image native entrypoint? The SDK declares
  only an opaque `intptr_t cISC4AdvisorSystem::GetRandom()`; it declares neither generator's operations, and
  the reconciled ABI tables/direct-call contracts contain neither exact owner. Section 6 publishes behavior
  and some addresses, but not a complete calling, object acquisition/construction, storage, lifetime or
  disposal contract. Please publish the permitted route, complete argument/return/calling conventions and
  supported-image validation contract, and how an isolated native instance can be given known seed/clock
  inputs without disturbing the game's streams. Include raw/bounded/signed/double/Gaussian/weighted methods
  where present, zero-bound draw behavior, seed sentinel/zero fallback and cache/reseed behavior. If no such
  route exists, explicitly state that native sequence verification is blocked. [Do not call behavior-spec
  addresses, dereference the opaque advisor handle, invent object layouts, or count Rust/spec/SDK-generated
  sequences as an independent game-native reference. Native-original versus Rust remains unverified.]

<!-- from: glue-rng4 native RNG contract audit (2026-10-22) -->
- **RN5 - sections 6.2-6.4, real native insertion contract:** Is there a supported published way to install
  an opt-in Rust-backed replacement for either exact owner `cRZRandom` or `cRZRandomSuper` so actual game RNG
  calls reach it? Please identify the SDK service/factory registration and acquisition path, or publish a
  complete typed native insertion contract, including caller coverage, ownership/lifetime, installation and
  removal timing, and how independent member/temporary and process-wide streams retain their seed ownership
  and draw order across city loads. The current SDK, reconciled ABI and foundations random/seeding sections
  expose no such route; a list of native static addresses and the opaque advisor `GetRandom()` return are not
  an insertion contract. If neither owner is replaceable through a supported route, please say so explicitly.
  [No `rng` enable key, invented registration, vtable/constructor hook, private-field patch or native-operation
  forwarding replacement is installed. Existing defaults, maps/tools and RNG-owning subsystem behavior stay
  unchanged; same-name simulation/renderer generators are outside this request.]

<!-- from: verify-rng-native4 independent native verification (2026-10-22) -->
- **RN6 - sections 6.1-6.3 and 18.3, independent read-only native draw capture:**
  If the owned typed native construction/method/lifetime route requested in RN4 cannot be published,
  can a supported read-only capture be published instead, with independently proven initial seed/state
  inputs for each exact `cRZRandom` / `cRZRandomSuper` instance? It must identify the generator and its
  lifetime, starting LCG/multiplicative state or all MT words/index, Gaussian cache state, actual clock
  inputs for sentinel/zero fallback, ordered method arguments and draws (including intervening consumers),
  and main-thread x87 precision at each float-sensitive operation. Inputs must be established independently
  of captured outputs; no live reseeding, private constructor/vtable/layout, or inverse inference of a seed
  from draws. Section 6.3's derived session seeds are FreeCity4 choices, not proof of native initial state;
  section 6.2 says RNG state is not saved. Neither the opaque SDK advisor handle nor algorithm addresses
  establish such a capture. [Blocked together with RN4: zero native comparisons; no native/shadow/default/
  replacement credit. Preserve Gaussian retry/log compatibility choices and all future mismatches/refusals
  separately; do not substitute a verifier algorithm or original draws for production-computed answers.]
