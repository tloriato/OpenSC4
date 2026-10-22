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

<!-- from: verify-save-records-native4 (2026-10-22) -->
- **SV2 - sections 2.1, 2.7 and 3.3-3.5; save-records sections 5 and 10, isolated native record verification:**
  What published supported route acquires an independently owned `cISC4DBSegment` with its original
  `cISC4COMSerializer`, opens an immutable copied city fixture read-only, and feeds independently supplied
  raw record bytes and raw resource/class/object identifiers to the original SC4 record readers? The public
  SDK declares `cISC4DBSegment::OpenIStream`/`CloseIStream`, `GetCOMSerializer` (opaque forward declaration)
  and `cISC4DBSegmentIStream::Open`, but supplies no SC4 segment class factory ID or acquisition service,
  no typed COM-serializer interface, and no complete isolated-instance lifecycle. Section 2.7 describes
  the private constructor and serializer lifetime, not a supported construction/calling contract; the
  reconciled vtable rows describe calls on an existing object, not how to obtain one. Please specify the
  factory/service or fully typed supported-image endpoint, required initialization and read-only open
  sequence, raw-byte/identifier injection, serializer registration/resolution prerequisites, ownership
  and exact close/release order on success, missing record, bad version, corrupt length/checksum,
  truncation and refused open. Include an owned output-record route if native roundtrip serialization
  is supported, without writing the live city or immutable source fixture. Explicitly state if no such
  route is supported. [Native verification blocked: do not guess private construction/storage, obtain
  answers from a live city's serializer, equate a generic packed segment or DB-record file adapter with
  the SC4 serializer or `cRZFile`, seed Rust from native parsed values, or count portable byte fixtures
  as an original-native shadow. No native endpoint, fixture comparisons or game-control runs claimed.]
