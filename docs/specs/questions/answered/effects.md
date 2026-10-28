# effects: answered questions (archive)

- **EF26 - Typed renderer record codec dependencies for the replacement resource (replace-effectsres28):**
  EF25 settles the resource primary/persistence interface and the complete outer
  directory sequence. The replacement now has that physical interface layout,
  shared lifetime, collection publication and the empty-directory stream path.
  Full nonempty resource Read/Write still needs a clean interface to the separate
  renderer value codecs. Section 18.9.5 names particle writer `005AFFF0`, decal
  writer `005A3E00`, dynamic writer `00583620`, visual writer `005AB590` and
  camera writer `005AB340`; sections 3 and particles section 2 name associated
  readers. `docs/abi/functions.tsv` supplies calling convention/argument counts,
  but several entries have unnamed arguments, and e.g. particle reader A is
  thiscall while reader B is cdecl. Please specify each helper's ordered typed
  parameters, receiver (if any), return channel, section-version input (if any),
  and borrowed/owned argument contract. Alternatively, specify the complete
  binary record order/version masks for decal and dynamic descriptions, plus
  the native tractor/wiggle reconstruction from their two serialized vec3s.
  We can then retain the existing description classes as independent renderer
  dependencies without calling native cSC4EffectsResource Read/Write or guessing
  how to call a non-interface helper. Nonempty payload is currently refused;
  no game acceptance or replacement credit is claimed.

  **ANSWERED 2026-10-26 (re-ef30):** DIFFERS — effects.md §18.10 specifies all
  twelve ordered typed codec contracts, receivers, stack cleanup, version
  selection and borrowed lifetimes. Both particle A and old visual Read use
  description receivers; the other ten use stream then value arguments.
  No helper takes an explicit section version or returns a success Boolean.
  Decal Write and old visual Read return borrowed stream pointers in EAX;
  other results are not semantic return values. Native constructed renderer
  values own their decoded members; the resource can use these independent
  dependencies, including particle nested reconstruction. Decal/dynamic wire
  tables and masks are supplied as well. Corrects the optional camera to a
  counted float curve and the visual selector to version exactly 1 versus
  all other values. EF26-C1 gives an 86-byte decal decode, publication and
  normalized write-back; C2–C7 cover version/ABI/lifetime cases. These are
  static predictions, not game acceptance or replacement credit.

- **EF25 - Effects-resource creation and complete primary interface (replace-effectsres28):**

The requested `effectsres` factory would replace CLSID `2A5118AD` while leaving
the original manager usable. Sections 18.7.3 and 18.8.1 specify resource IID
`CA51189B`, collection IID `C97CF5CE` at resource+4, publication at primary slot 5,
and stream Read at primary slot 7. The SDK supplies the class ID but no resource
interface declaration. `docs/abi/vtables.tsv` supplies the collection vtable
`00AA0A60`, but not the primary vtable `00AA0AC0`.

Please specify the complete primary slot declarations (argument/return types,
stream interfaces and calling conventions), all QI identities/sub-object offsets
used by the existing resource factory/serializer, and reference-count/destruction
contracts. In particular, what are primary slots 3, 4, 6 and the slots after 7;
does QI expose `cIGZSerializable` and/or `cIGZPersistResource`, and at what offsets?
Also specify the writer's supported root versions, output/result contract and
failure-state behavior for a partially read resource. The existing Rust directory
writer explicitly emits only its simulation sections, so its output alone cannot
establish full-resource serialization compatibility.

The lane stopped native resource registration rather than invent these ABI facts.
It separately fixed the specified rejection of root versions outside 3..4
(section 18.8.1), which both Rust directory entry points previously accepted.

**ANSWERED 2026-10-26 (re-ef29):** DIFFERS — effects.md §18.9 specifies all
nine primary slots, their argument/return channels and thiscall cleanup; QI
exposes primary R, collection R+4, and persist/canonical-unknown R+11C, but no
`cIGZSerializable`. All share one zero-initialized atomic reference count with
final destruction through persist slot 12. The existing resource factory and
serializer require the persist identity and the documented input/output stream
IIDs. Read accepts roots 3..4; Write always emits root 4 and every renderer and
simulation section. Normal Read/Write completion returns true despite primitive
failure flags; accepted reads mutate incrementally without rollback. Worked ABI,
identity, lifetime and bridge cases, plus EF25-W1..W5 and EF25-R1..R7, make those
rules testable. Section 3's former standalone-length terminator wording is also
corrected: the visual map ends on the exact `(end,FFFFFFFF)` name/index pair.

<!-- from: glue-effects-whole9 (2026-10-24) -->
- **EF22 - section 18.8 safe native interceptor installation:** EF21 specifies observation sites and calling conventions, but not the complete instruction spans, relocation rules, expected bytes/version guards or adjusted thunk entries needed to install those interceptors without reading/disassembling executable code. Please specify a safe attachment/removal contract before natural Init: the version-validated patch spans and continuation/exception behavior for Init/bootstrap, resource requests/Read/publication/releases, Begin/End, parse and outer reload/tick, including exact this adjustments at virtual entries and how every possible writer is excluded while copying. Section 18.1 identifies the +8 message-target vtable but does not give the callable adjustor thunk entry value for a validated slot replacement. Is there an existing documented interception mechanism that avoids code patching and still observes these natural operations? [No guessed hooks or private executable inspection. Current plugin measures only SDK name membership at load/month; retained visual/built-in payload and historical owner comparison stay unavailable until thread/depth/writer conditions are established. The core implements bounded EF21 payload copying and observation bookkeeping for a future authorized host attachment.]

  **ANSWERED 2026-10-24 (re-effects-ef24):** DIFFERS — effects.md §§18.1,18.8.6–18.8.7 give executable fingerprint, complete expected-byte intervals, continuations/relative-tail-jump rule, request/parse call-site ABIs, original vtable cells and receiver adjustments. Message slot 3 directly contains 00594120 and expects M+8, without an additional thunk adjustment. No native registration facility, universal writer lock or exception-safe detour installer exists in these interfaces; vtable-only attachment misses direct calls and early Read. Host exclusion, lifetime, unwind transparency and safe installation/removal are mandatory; absent proof means refusal, not invented history. Twelve proposed conformance vectors are unexecuted; no native interceptor was installed or whole-owner credit awarded.

<!-- from: glue-effects-whole9 (2026-10-24) -->
- **EF23 - section 18.8.3 sequence payload extent:** EF21 lists sequence extent 0x18 and only the vector at +0x0C, while the existing sequence stream contract retains a flags word after the element vector. Is that word part of the native description payload, and if so what is its offset/extent and mask? [Copy only the EF21-listed vector; sequence flags and complete sequence-owner credit unavailable, rather than guessing an out-of-extent field.]

  **ANSWERED 2026-10-24 (re-effects-ef24):** DIFFERS — effects.md §18.8.4: extent is 0x1C, including the u32 flags at +0x18. Fresh flags are zero; script switches OR bits 1,2,4. Stream Read masks with 7, Write emits the full stored word, and capture must retain the full memory value, including anonymous/empty-vector sequences. Five proposed conformance vectors are unexecuted.

<!-- from: glue-effects-shadow7 (2026-10-24); copied from its reserved worktree -->
- **EF20 - sections 2, 6.1/6.2 and 18.6 native record-only capture:** Please specify the collection getter return types, exact argument types/ownership and description enumeration/count access, the resource collection-reader IID/signature, and a read-only queue/bootstrap state capture protocol. The slot tables identify methods but their ABI rows say eax-or-void, and section 2 does not define list/description container framing. How can a shadow capture all published descriptions, bindings, triggers, camera state and retained file queue without mutating the native manager or guessing signatures/layouts? [Compare only section 3 directory name decoding against native section 4 HasVisualEffect, through the public SDK raw record and manager interfaces. No native collection getters, private container reads, queue hooks, publication/placement equivalence or full-owner coverage claimed until this contract is provided.]

  **ANSWERED 2026-10-24 (re-effects-ef20):** DIFFERS — effects.md §18.7:
  named getters return bool in AL; six copy into constructed caller-owned value
  objects, while the component getter returns a retained description pointer.
  No count/enumeration interface exists, and even null-output getters mutate
  scratch state. Native vector/hash framing supplies retained indices, names,
  bindings, triggers, camera copies and queue records. Resource IID CA51189B
  slot 5 is void publication to a collection, with a storage-swap fast path;
  it must not be called for inspection. Capture at a quiescent owner boundary,
  preserving typed values and refusing incomplete samples. Bootstrap history
  requires observing natural operations; it cannot be reconstructed from final
  state. Ten proposed conformance vectors, unexecuted; no coverage expansion
  or payload equivalence is inferred from names/counts alone.

- **EF21 — §18.7 native observation attachment and remaining payloads:** EF20 specifies retained memory framing but not an attachment contract for tracing the naturally executed bootstrap/publication calls. How does a default-off plugin observer obtain a live resource primary identity and the natural bootstrap/publication inputs/results without requesting a new resource or changing owner behavior? Which owner-thread city-load/month callback boundaries exclude concurrent retained collection/queue writes, including reload? Supply complete typed native payload memory layouts for particle/dynamic/decal/shake/light and component types 3–15 (including anonymous descriptions), or explicit cross-references to their complete contracts, to support payload-exact capture. [Compare only the visual records and name/index multiset for now; other families, camera/binding/trigger publication and queue/bootstrap history remain uncredited. No resource publication/getter/parse calls or guessed native hooks are introduced.]

  **ANSWERED 2026-10-24 (re-effects-ef21):** DIFFERS — effects.md §18.8 specifies natural bootstrap/resource-service/publication attachment, borrowed resource identity/lifetime, parse result widths, reload boundaries, and complete built-in particle/dynamic/decal/shake/light and type-3..8 payload layouts. Types 9–15 are reserved directories with no built-in private payload contract; a foreign object there is explicitly unsupported rather than misread. City-load/month labels supply no lock; copy only in a verified owner-thread interval outside the outermost Init/bootstrap/reload, or refuse. A late attachment cannot reconstruct history, and full manager/resource coverage additionally requires live lifecycle and stream/reference-event measurements. Thirteen proposed conformance vectors are unexecuted. Question copied from glue-effects-capture8 because it was not yet on main.

- **EF15 — Effects-script command owner contracts missing from effects.md:**
  The requested owner family is `(anonymous_namespace)::c*Command`, including
  effect-description parser commands. Which exact classes belong to that family,
  and which spec sections define each one's token inputs, state transitions,
  defaults, validation, block enter/exit, foreign calls and conformance cases?
  `effects.md` §§0–18 contains no command-class inventory or command algorithms;
  its §17 exercises runtime effects, not script commands. The separate
  `effects-parser.md` §6.1 supplies 21 root literal/descriptor/hook associations,
  expressly excludes their execution algorithms, and §6.2 supplies only the
  `sequenceEffect` normal handler, expressly excluding other nested entries.
  Please publish the class-to-descriptor inventory, all remaining root/nested
  handler contracts and command-specific conformance cases (including any
  commands absent from that bounded dictionary). Distinguish these owned
  algorithms from foreign renderer, sound, resource and generic-reader services.
  [Retain existing registration and sequenceEffect implementation; do not infer
  handler behavior or class names from command literals or addresses. Whole
  command-family closure remains blocked on this publication; EFP1–EFP7 also
  continue to bound the existing parser's supported domain.]
  **ANSWERED 2026-10-23 (re-effects-ef5):** DIFFERS — effects.md §19 now
  publishes the descriptor inventory and execution of all registered root and
  nested families. §§19.53–19.56 close the residual visual-effect children,
  aliases and select/particleSequence hooks with EC277–EC304. Class names
  remain limited to confirmed export identities. Foreign primitive boundaries
  and the numeric qualification of effects-parser §14 remain explicit.

- **EF16 — §19.52 / EC275 conflicts with effects-parser §6.1 for instance:**
  effects §19.52 assigns descriptor `00A9FA08` direct handler `00596980` and
  EC275 says this is the same handler as root instance. The root inventory in
  effects-parser §6.1 instead assigns that descriptor `0059A730`. Which is the
  correct root and nested association? [Retain both literal registration
  ledgers, use the settled §19.4 effect-ID algorithm for instance, and leave
  only the claimed equal-handler part of EC275 open; no inferred correction.]
  **ANSWERED 2026-10-23 (re-effects-ef5):** DIFFERS — §19.52's address
  was wrong: both registrations use descriptor `00A9FA08`, whose +0C word
  is `0059A730`. `00596980` is inside effectsResource's exit hook.
  EC275's equal-handler assertion and §19.4's instance behavior stand.

- ~~**EFP2 — Rotation instruction/store semantics:** In effects-parser.md §2/§8,
  establish x87 operations and binary32 store order for all five rotation branches,
  including key order versus text order.~~
  **ANSWERED 2026-10-23 (re-effects-commands5):** DIFFERS — effects-parser.md §13:
  turns multiply binary32 2π without an intervening angle store; cosine stores
  binary32, positive sine initially stays x87. X/Z store negative sine before
  their second column; Y retains positive sine. Row-dependent product stores
  are specified explicitly. XYZ executes X/Y/Z; ZXY executes Z/X/Y on successive
  selected values. Each completed axis increments the byte count and sets bit 04;
  earlier axes survive a later conversion exception. Six conformance rows.
  Process PC/RC and broader numeric qualification remain EFP7.
<!-- Archive for [effects.md](../../effects.md). Lanes do not read this file. Open questions are in ../effects.md. -->

<!-- from: From the automata state behaviours and the effect-directory measurements (2026-10-16) -->
- ~~**EF2:** confirm at runtime that a volcano never reaches state 7 when the city view does not draw (effects.md
  §15 quirk 1). [needs a runtime check]~~ **CLOSED 2026-10-16 (re-integrate1), statically.** `0x005BFD70` has
  exactly one reference in the whole decomp, in `cSC4EffectsManager::DeactivateUnusedVisualEffects` `0x005922A0`
  (`cSC4EffectsManager.c:1987`), which in turn is referenced only from
  `cSC4EffectsManager::Update(SC4DrawContext*)` `0x00593120` (`:2993`) - no direct call sites, dispatched from
  the vtable slot at `0x00A9F25C`. The end-message post exists in exactly one place. So no draw pass means no
  finish test, no effect ever finishes and no end message is ever posted. The volcano stuck in state 5/7 and the
  effect-disaster emergency pause never lift.

<!-- from: From the automata state behaviours and the effect-directory measurements (2026-10-16) -->
- ~~**EF3:** the path by which particle systems spawn the "slave" effects, and whether the parameter 13 forwarded
  by the effect-disaster instance reaches them. [untraced]~~ **ANSWERED 2026-10-16 (re-integrate1).** There is no
  spawn API: each particle class concatenates its base name with a fixed suffix (`FUN_0041E270` `0x0041E270`) and
  calls `CreateVisualEffect` (slot 7). Suffixes and callers: `_slave` (`0x005AEF20` per particle, `0x00585190`
  once at Start), `_death` (`0x005AF390`), `_collide` (`0x00584890`), `_terrain`/`_water` (`FUN_005B8290` on
  impact). Parameter 13 reaches **game components only** - particle `SetVoidParams` `0x005B2200` accepts only
  index 7, decals only index 6, meta recurses (`0x005C0540`). Shipped census (1148 names): `_slave` 143,
  `_terrain` 33, `_water` 20, `_death` 21, `_collide` 3, `_control` 6. effects.md §10.2. The one sub-question
  that survives is EF6 below.

<!-- from: From the enable=/spec-contradiction wave (filed by the coordinator from the implementer lanes' reports, 2026-10-16; answered by re-answers31) -->
- ~~**glue22 question 9** (`enable = effects`): `ACCESS_VIOLATION` writing `0x4D` at a wild `eip` about 100 ms
  after `Init`, no method of ours entered; the game calls through a vtable pointer read past our object. A `+4`
  draw-pass sub-object did not help. What are all the interface sub-objects of the original
  `cSC4EffectsManager`, at which offsets?~~ **ANSWERED 2026-10-16 (re-answers31): DIFFERS.** The manager is
  **six** sub-objects with vptrs at `+0x00`, `+0x04`, `+0x08`, `+0x0C`, `+0x10`, `+0x94` (destructor
  `0x0058FA60`, confirmed by the Mac `__ZThnN_` this-adjustments). `QueryInterface` (`0x0058D9E0`) returns
  `this + 8` for IID `0x452294AA` (`cIGZMessageTarget2`) and `this + 0x94` for `0xC97CF5CE`. The message
  dispatch fetches the vptr at `+0x08`, which is past a single-interface object. Written into
  [effects.md](../../effects.md) **§18**.

<!-- from: From the automata state behaviours and the effect-directory measurements (2026-10-16); answered by re-answers34 -->
- ~~**EF1 (narrowed):** the exact finish time of `volcano_control`. Only particle components could hold it:
  brush ends at `t > length` (2.0-4.0 dilated effect-seconds), decals self-stop at fade 1.0 with mode byte
  `+0x15 == 2` (`0x00582920`), game/attractor/camera carry the ignore bit - yet all 1921 shipped particle
  components have `flags = 0`. Do `cSC4ParticlesEffect` components ever report inactive, and by what path?~~
  **ANSWERED 2026-10-20 (re-answers34): MATCHES - statically, the runtime trace is not needed.** The emitter
  self-stops. Three facts from the instruction bytes (effects.md 7.1):
  (1) `cSCParticleSimulatorBase::Stop` `0x005B9E70` takes two mode bytes; mode1 == 0 is an **immediate** stop
  (`started := 0`, all particles destroyed), mode1 != 0 is **delayed** (`+0x0C` set, particles play out).
  `cSC4ParticlesEffect::Stop` `0x005B21E0` calls the base with `(mode == 0, 0)`, so the component's `Stop(0)`
  is the delayed one and `Stop(1)` destroys at once.
  (2) `CheckForStop` `0x005B9D40` (end of every draw-time update) converts a delayed stop to an immediate one
  when the live count `+0x14` reaches 0 - that is when `IsActive` goes false.
  (3) The emitter calls `Stop(0)` on itself when its budget is exhausted: `cSC4ParticlesEffect` at `0x00AA6D4`
  / `0x00AA6EB` (interval countdown `+0x190` seeded in Start from desc `+0x20..+0x2C`); ModelBased via its
  `+4` sub-object slot 15 `0x005B96E0`, which dispatches on the particle description's flag bits: bit 2
  (`0x04`) -> `0x005B3CF0` (plain rate emitter, no self-stop), bit 1 (`0x02`) -> `0x005B8B50` (loop-budget
  emitter: `+0x128` counts loops vs desc `emit_loop_count` at `+0x18`, `Stop(0)` at `0x005B8D67`), else
  `0x005B87B0` (duration-budget emitter, `Stop(0)` at `0x005B8B29`).
  So every shipped volcano particle component (`emit_loop_count = 1`) ends on its own and `volcano_control`
  finishes ~50 effect-seconds (the crater decal's `life`) after its creator starts, plus one draw-pass sweep
  for the meta parent. Written into effects.md **7.1** and the EF1 entry.

- ~~**EF4:** the meaning of visual effect flag bits 0, 3, 6 and 8, and when an effect created during a
  draw-pass sweep is activated in a session that only ticks.~~ **ANSWERED 2026-10-20 (re-answers34): MATCHES
  (bits) / settled (activation).**
  *Bits.* The 9-bit description flags land verbatim in the effect's `+0x28` (reader `0x005ABF50` masks
  `& 0x1FF`; `TestFlag` slot 17 `0x005BFC90` reads them). A byte-level scan of the effects module
  (`0x0058D000`-`0x005C5000`) finds tests of exactly five bits on `+0x28`: bit 1 (finish test `0x005BFD89`),
  bit 2 (hard stop `0x005BEBB6`), bit 4 (slot 13 no-forward `0x005BFC26`/`0x005C05E6`), bit 5 (pick ray
  `0x0058E6A7`), bit 7 (finish test `0x005BFDD2`). **Bits 0, 3, 6, 8 have no consumer anywhere in the module**;
  they are stored and readable via `TestFlag` but change nothing in the engine - a replacement may accept and
  ignore them.
  *Activation.* `ActivateNewVisualEffects` `0x005921B0` is called from both `CreateVisualEffect` overloads
  (`0x00593AB0`, `0x00593B68`), from the draw pass (`0x00592A01`) and from `UpdateActiveVisualEffects`
  `0x00593B90`. The last runs on `0x697D01ED` (`DoMessage` `0x00594476`), which `EffectsTickCallback`
  `0x0058D9A0` posts once per animation tick (registered in `Init` `0x00594EEE` via
  `EnableAutoStep(1, 0x0058D9A0)` on the animation-tick manager at `+0xCD8`). So an effect created during a
  draw-pass sweep is activated on the **next animation tick** in a ticking session, not only on the next draw
  pass. effects.md EF4 and 6.1.

- ~~**EF5:** whether a hidden pause counts as "paused" for the time dilation, which would make the 5 s and 15 s
  pauses of effects.md 9.4 real-time seconds.~~ **ANSWERED 2026-10-20 (re-answers34): DIFFERS from the earlier
  dilation reading.** `GetAnimationTimeDilation` `0x007026F0` consults, in order: `IsEmergencyPaused` (slot 17)
  -> the paused value; the object cached at global `0x00B43D44` (city slot 43, the vehicle/automata manager
  family) whose slot 13 is `cSC4AutomataManager::IsDrivingMode` `0x00506940` (true while the U-Drive-It view
  `0x2B709676` is up) -> the paused value; `IsPaused` (slot 15) -> **0.0**; otherwise by `GetSimSpeed`
  `0x008265C0`: speed 0 -> 1.0, speed 1 -> 1.5, speed 2 -> 2.0. The table lives at the simulator's `+0x108`
  pointer: `[0.0, 1.0, 1.5, 2.0, 1.0]`, written by `ReadSimulatorTunableValues` `0x007025B0`
  (`0x00702686`-`0x007026B1`). **`IsHiddenPaused` (slot 16) is never called**, so a hidden pause does not
  change the dilation: the 5 s / 15 s hidden pauses run in effect-seconds dilated at the current speed, not
  real-time seconds. The earlier "1 while paused, 3 at speed 2" was wrong (now 0.0 and 2.0). effects.md 6,
  quirk 3, test 17.

- ~~**EF6:** what `data1` is on a `kSC4MessageEffectLocation` (`0x2A7B582E`) posted from a game record inside a
  particle-spawned slave.~~ **ANSWERED 2026-10-20 (re-answers34): DIFFERS - the handler matches strictly on
  `data1 == instance`, so particle-spawned slaves never update the instance's location.**
  `cSC4DisasterInstanceEffect`'s message handler `0x00576E70` (registered at vtable `0x00A9E254`) does, for
  `0x2A7B582D` (`EffectDone`) and for `0x2A7B582E` (the `SUB EAX,0x2A7B582D; DEC EAX` path at `0x00576E86`)
  alike: read data1 (message slot 9, `+0x24`) and require `EAX == this - 0xC` - the instance pointer
  (`LEA ECX,[EDI-0xC]; CMP EAX,ECX`, `0x00576E93`-`0x00576EBE`). Only then does it take data2/data3
  (`0x00576E9B` slot 14, `0x00576EA5` slot 19) into the instance's `+0x4`/`+0x8` and call the location setter
  at `[this-4] vt+0xC`. `cSC4Scrubber::Apply` `0x005BD3C0` posts `data1 = data (+0x58)`; for a
  particle-spawned slave that value is the particle's `+0x26C`, never the instance, so the update is dropped.
  The 10.2 damage table is unaffected (the damages do not depend on the location messages); only the
  *tracking* of tornado/meteor/robot-walk slaves by the instance does not happen. effects.md 10.2, 12.

- ~~**EF7:** the exact semantics of `cSC4ModelBasedParticlesEffect +0x26B / +0x26C` and `+0x26F / +0x270`.
  Ghidra splits them into five byte fields and that class has no `SetVoidParams` to populate them from a
  parent's parameter 13.~~ **ANSWERED 2026-10-20 (re-answers34): MATCHES the five-byte split, with one
  correction - there *is* a `SetVoidParams`.** From the bytes (`0x005AC510` ctor, `0x005AEC90`
  `SetDescription`, `0x005AC040`):
  `+0x26B` = the forward-parameter-13 gate: read at `0x005AF02A` (`CreateParticle`) and `0x005AF8B7`
  (`DestroyParticle`), both before `ForwardParameter(13, &this+0x26C, 1)` to a spawned slave. **Nothing in the
  effects module writes it** (full ModRM/SIB scan for disp32 `0x26B` writes: none; the ctor initialises
  `+0x26C..+0x270` but not `+0x26B`) - effectively uninitialised, a quirk; a replacement should make it an
  explicit default-off switch.
  `+0x26C` = render latch: `SetDescription` stores `cS3DModelProcessor::SetModel`'s return (`0x005AEDEB`);
  read only in `RenderActiveParticles` `0x005AE4A1`, gating `LightModel`/`DeformModel`.
  `+0x26D` / `+0x26E` = "a `<base>_slave`" / "a `<base>_death` effect exists" (`%s_slave` `0x00AA0CD0`,
  `%s_death` `0x00AA0CC4`; `0x00AEE79`, `0x00AEEF7`).
  `+0x26F`/`+0x270` = the parameter-13 channel for the chain hand-off: `SetVoidParams` **does exist** -
  `0x005AC040`, slot 6 of the component vtable `0x00AA0C58` - index 13 sets `+0x26F = 1` and `+0x270 = *value`,
  others fall through to the shared `0x005B2200` (index 7 only). `HandOffParticleToChainSystem` `0x005AF060`
  reads `+0x26F` at `0x005AF27C` and forwards `13, &this+0x270`; `DestroyParticle` forwards `13, &this+0x26C`.
  So `+0x26C` is the per-particle channel's value and `+0x270` the chain hand-off's value, `+0x26B`/`+0x26F`
  their enable bytes. The "accepts only index 7" claim in EF3 refers to the shared handler only. effects.md
  10.2.

- ~~**EF8:** who posts and who consumes `kSC4MessageTornadoDead` `0x0A36F1E8` and
  `kSC4MessageMissionEffectStarted` `0x5F900DCC`.~~ **ANSWERED 2026-10-20 (re-answers34).**
  `kSC4MessageTornadoDead` `0x0A36F1E8`: the constant occurs exactly once in the image - its name-table entry
  at `0x00B08430`. Nothing posts it and nothing dispatches on it: a dead ID in this build (the live sibling
  `kSC4MessageTornadoLocation` `0x0A36F1E9` is posted by `0x0042E5DF`/`0x0042EE8C` and consumed by
  `cSC4MySimAgentSimulator::DoMessage` `0x0042EFDC`). A replacement need not implement it.
  `kSC4MessageMissionEffectStarted` is `0xCC0D905F` as stored in the table at `0x00B08420` (the question's
  `0x5F900DCC` was a transposed reading): **nobody posts it** either. The only references are
  `cSC4CitySituationManager::Init` `0x0052DD20` (subscribe, server slot 6 `+0x14`), `DoMessage` `0x0052EFEC`
  -> handler `0x0052F096` and `Shutdown` `0x00527C58` (unsubscribe, `+0x18`). The handler checks
  `[this+0x9F0]`, calls its slot 15 (`+0x3C`) and stores the message's sender (`[vt+0x80]`) into the
  `cRZAutoRefCount<cIGZWin>` at `+0xA98` (`0x0052F0B9`-`0x0052F0C0`) - a latent bind-the-sender-window hook
  that nothing in this build triggers. Both are latent hooks, not live traffic. effects.md 12.

- ~~**EF9:** where the message names for `0x86AD10EE` and `0x697D01ED` live - they are not in the `0x00B08360`
  table (115 pairs).~~ **ANSWERED 2026-10-20 (re-answers34): DIFFERS from the "115 pairs" reading.** There is
  one message name table and it is larger than thought: it spans `0x00B07FD8`-`0x00B08FE0`, **513**
  `{u32 id, const char*}` pairs, every one resolving to a printable name. `0x00B08360` is simply a position
  inside it (`kSC4MessageMouseScrollBegin` sits there). `kSC4MessageAnimationTick` `0x86AD10EE` is entry
  `0x00B08250` (name at `0x00A8C06C`; sibling `0x86AD10EF` "kSC4MessageAnimationTickPriority" at
  `0x00B08258`). `0x697D01ED` has **no name anywhere**: the ID appears only at `0x0058D9CA` (the posting
  callback), `0x0059446C` (dispatch) and `0x00594DA5` (subscribe) - an unnamed internal message. effects.md 12.

- ~~**EF10:** whether "every component that reaches a brush or game record, directly or through meta, is
  `[1, 6)`" holds per effect.~~ **ANSWERED 2026-10-20 (re-answers34): MATCHES - measured on the shipped
  directory.** Ran `engine/formats/tests/effects_directory.rs` (`effects_directory_sweep`) against the
  installed game (`FC4_GAME_DIR` = the GOG install; `SimCity_1.dat`, 1,094,484 bytes, root version 4):
  `effects_directory: 72 effects reach a brush or game record, all through [1, 6) components; 887 other
  components have another range`. The narrower claim holds for all 72 effects whose reach set contains a
  brush or game record; the 887 non-`[1, 6)` components are all outside those reach sets (render-only).
  effects.md 5.2.

- ~~**EF11 - §6 dilation table:** which dilation entry `GetSimSpeed` values outside 0-2 use.~~
  **ANSWERED 2026-10-21 (re-effects-contracts4): MATCHES - `[1]` = 1.0 for every speed other than 1 and 2.**
  `GetSimSpeed` `0x008265C0` returns the raw `u32` at simulator `+0x3C`; the getter `0x007026F0` tests only
  `speed - 1 == 0` -> `[2]` and `speed - 2 == 0` -> `[3]` (`0x00702727`-`0x0070272B`); everything else, 0
  included, loads `[1]` (`0x0070272D`). No clamp or arithmetic. effects.md 6 step 1.

- ~~**EF12 - §6.1 / §5.2 / §5.3:** whether `DoEffectsTick` rebuilds every active effect each tick or only
  activates the new list, and how an unchanged-L rebuild avoids a fresh select-group draw.~~
  **ANSWERED 2026-10-21 (re-effects-contracts4): DIFFERS - `DoEffectsTick` does neither on an ordinary tick.**
  `DoEffectsTick` `0x00593D00` calls `UpdateActiveVisualEffects` only when the `+0xC78` object's slot 14
  reports queued effect files (then `ParseQueuedFiles` and a reload with `+0xB8C` = 1); otherwise it only does
  the snow-line step. Activation of new effects happens in `CreateVisualEffect` (`0x00593AB0`, `0x00593B68`),
  at the end of `DeactivateUnusedVisualEffects` (`0x00592A01`) and in `UpdateActiveVisualEffects`, which itself
  is a no-op until `kSC4MessagePostAppServicesInitComplete` sets `+0x1AD` (`0x0059458A`). No per-effect
  "built" flag: in `UpdateVisualEffect` `0x005919D0` a description takes part in a select draw only if it is
  in range at `L` and was not at `L_prev` (`+0xC84` / `+0xC88`), or the build is new, or a reload is running
  (`0x00591C08`-`0x00591C32`); existing components survive pass 1 unless reloading. effects.md 5.3 (rebuild
  test), 6.1 (correction), 14.

- ~~**EF13 - §7.1 emitter details** (a)-(e).~~ **ANSWERED 2026-10-21 (re-effects-contracts4): PARTLY.**
  (a) DIFFERS - `Start` `0x005B3000` seeds the countdown `+0x190` from the start-delay range `D+0x20..+0x24`
  if `D+0x20 >= 0`, else the interval range `D+0x28..+0x2C`, else 0; the first burst comes on the first
  draw-time update after the countdown runs out (immediately only when both ranges are negative).
  (b) loop-budget emitter: one burst of `N = ceil(curve[0])` (times the detail scales when `> 20`) per loop,
  each particle initialised with the burst age; not a caller-given count. (c) DIFFERS - the rate emitter has no
  seconds budget: it counts loops of `D+0x14` seconds in `+0x128` against `D+0x18`, like the loop-budget one;
  with flag A `0x08` it holds at the last loop instead of stopping. (d) MATCHES - the dispatch `0x005B96E0`
  returns at once while a delayed stop is pending (`+0x10` = base `+0x0C`). (e) MATCHES - live particles are
  updated (slot 20) first, then the emitter runs (`0x005B870A`). effects.md 7.1.1, 14.
- ~~**EF14 - §7.1.1 emitter gaps** (core-effects-fix4, 2026-10-21): (a) the plain emitter `0x005B3CF0` (flag A `0x04`)
  has no body in the spec; the core uses the rate emitter's integral over the whole step without loop counting,
  gaps or stop - what does it do (sub-steps, phase wrap, ages, does it touch `w`)? (b) the rate scale `+0xC0` for
  flags A bits 19/20/21: which wins when several are set, and is `m'` scaled at all when none is? (core: lowest
  bit wins; none -> not applied). (c) `SetDescription`'s `1e-6` floor for `D+0x14`: is it written back into the
  description (so the burst age and the gap use it too) or used for `ρ` only? (core: `ρ` only). (d) the
  particle init/update (main slot 20): when does init with age `a` fail, and when does an update return false?
  (core: a caller-given life; init fails when `a >= life`, a particle dies when its remaining life reaches 0).
  (e) the curve interpolation when `P − n` is outside `[0, 1]` and for an empty curve (core: clamped, 0).
  [all marked `[choice, EF14]` / `[choice]` in engine/core/src/effects/particles.rs]~~ **ANSWERED 2026-10-21 (re-effects14-4):** DIFFERS - (a) DIFFERS: the plain emitter `0x005B3CF0` only tops the live count up to `ceil(curve[0])` (no `m` or `+0xC0` scale) once `P > n`; `n`, `w` and `c` are never touched, nothing is sub-stepped, and new particles start at age 0 with no slot-20 call. (b) MATCHES: bits 19, 20 and 21 are tested in that order and the first set bit gives `k`, `k²` or `k³`; with none set there is no factor. (c) MATCHES: the `1e-6` is a local value and `D+0x14` is never written; only `ρ` is floored. (d) DIFFERS in where life comes from: `InitEffectsParticle` draws life uniformly from `[D+0x0C, D+0x10]` per particle; slot 20 does `age += t` and returns false when `age ≥ life`, so init fails when `a ≥ life`; when flags A `& 0x18000D00`, water/terrain kills can also fail it. (e) DIFFERS: the original does not clamp (reads past the end when `P − n > 1`, reachable when `ρ > 15`) and has no empty-curve guard; clamping and treating an empty curve as 0 stay `[choice]`. effects.md 7.1.1.


<!-- from: core-effects-parser4; questions originally in effects-parser.md -->
- ~~**EFP1 — Foreign switch-reader language and consumption**
  In effects-parser.md §2/§4, what exact contract does 0067A7D0 expose for
  dash/case/delimiters, aliases, duplicates, missing/empty values, unknown options,
  terminators and token consumption? Supply individual lookup outcomes and scratch
  indices, not a host-produced final parser result. Present owner rules do not prove
  these generic reader behaviors.~~
  **ANSWERED 2026-10-23 (re-effects-commands5):** DIFFERS — effects-parser.md §9
  establishes exact case-sensitive single-dash matching, first duplicate per lookup,
  in-place prefix marking, retained records, minimum arity, -/+ alphabetic
  terminators, positional-prefix collection and later unknown-switch validation.
  Eight independent token/index vectors; effects.md §19.1 links the service.
- ~~**EFP4 — Foreign registration identity and ownership**
  In effects-parser.md §5.1/§5.2, establish P+90 slots +44 and +48's matching
  interface contracts: named validation, duplicate/collision/replacement behavior,
  null-name identity/index selection and reference transfer of the supplied object.
  The owned bodies append locally; neither generates an anonymous name or counter.
  Do not claim a foreign counter boundary from the raw returned-index ledgers.~~
  **ANSWERED 2026-10-23 (re-effects-commands5):** MATCHES — effects-parser.md §11.1:
  lowercase copied keys in separate type buckets; null names append without named
  identities; returned indices are per-type vector positions. Named replacement
  preserves index, retains new before releasing old; identical pointers are no-ops.
  Lookup with null out only validates; failed lookup leaves out unchanged.
- ~~**EFP5 — Final lifetime and shared reference operations**
  In effects-parser.md §5.5/§8, positively identify destructor/refcount-zero cleanup
  and held-interface release order. Linked callback 0059EBE0 is not yet proved a
  destructor; shared AddRef/Release delegation cannot certify parser-exclusive
  lifetime behavior. Distinguish this class's factory from the separate file queue.~~
  **ANSWERED 2026-10-23 (re-effects-commands5):** DIFFERS — effects-parser.md §11.2:
  actual AddRef/Release stubs are 0059F3C0/0059F3D0, not 0059F3E0/0059F3F0.
  Final Release invokes deleting destructor 005A27F0, member cleanup 0059F4E0.
  Held component P+490 releases first, then description/container cleanup;
  interfaces P+A8, P+94, P+90, P+8C release in that order, followed by base
  destruction and allocation free. Callback 0059EBE0 is distinct.

- ~~**EFP3 — Error transport and lifetime outside normal domain**
  In effects-parser.md §3, identify the actual error transport of 009EF84F and
  cleanup/unwind behavior of the select/LOD/prob errors. Do not assume false-return,
  transaction rollback or resumed parsing from the misleading export name.~~
  **ANSWERED 2026-10-23 (re-effects-commands5):** DIFFERS — effects-parser.md §10:
  009EF84F raises a noncontinuable MSVC C++ exception through RaiseException,
  code E06D7363, flags 1, three information words. Owner frames install no local
  catch/unwind cleanup. Earlier token marking and description writes remain;
  rejected-value conversion, append and normal temporary cleanup are not reached.
  No implicit end hook or rollback is performed. File-reader recovery is a
  separate foreign scope, with no continuation guarantee from these owner bodies.
- ~~**EFP6 — Command-library dispatch policy**
  In effects-parser.md §6, establish foreign root/nested registrar duplicate,
  lookup/case, descriptor ownership and block-hook dispatch policy. Current proof
  is the owner's ordered literal-to-descriptor registration, plus the concrete
  sequenceEffect normal handler, not all root command semantics.~~
  **ANSWERED 2026-10-23 (re-effects-commands5):** DIFFERS — effects-parser.md §12:
  exact byte/case lookup; last registration wins. Root duplicates append owned
  descriptors; nested duplicates overwrite without destroying the old descriptor.
  Registration invokes +08 initialization, not AddRef. Search is innermost block
  outward, then root. Wrappers forward simple +0C, block enter +14 and exit +18;
  StartBlock pushes, end invokes the exit before popping. Only nested-selected
  commands get the post-handler unexpected-switch check. Six independent vectors.

<!-- from: core-effects-close5 (2026-10-24) -->
- ~~**EF18 - §6.1 EffectsParser queue wrapper, EffectsBootstrap, collection lifecycle:** what are the queue/file processing and bootstrap lifecycle contracts for the separate `EffectsParser` and `EffectsBootstrap` owners, and any `cSC4EffectsCollection` behavior beyond the component directory in effects-parser §11.1? §6.1 only names `ParseQueuedFiles` in the manager's reload sequence; effects-parser §1 expressly excludes that wrapper, and effects.md supplies no bootstrap algorithm. [Queue processing/bootstrap return explicit Unsupported; the settled component directory remains implemented. EF17 is reserved by the coordinating brief and is not duplicated here.]~~

  **ANSWERED 2026-10-24 (re-effects-ef18):** DIFFERS — effects.md §§6.2, 18.6:
  queue/bootstrap are supported manager operations, not independent extra COM
  owners. Retained inputs parse synchronously; success clears UI error and sends
  C9F775BB, failure reports the diagnostic. Bootstrap orders base resource,
  optional packed main.fx, extra resources, then unpacked files when selected.
  Collection Begin retains descriptions but clears bindings/triggers; publication
  copies and fixes references without rejecting unresolved names; End rebuilds
  ID and trigger indices. Twelve proposed conformance vectors, including optional
  file exception suppression (flag bit 0 clear).

<!-- from: core-effects-close5 (2026-10-24) -->
- ~~**EF19 - §§3, 8.3, 9.2 component transform decoding and numeric order:** what is the mapping from the 56-byte `S3DTransformLegacyLoad` placement to rotation/scale/translation, and the intermediate operation/store order for component world positions? [Implement the literal semantic parent translation + parent rotation/scale of local offset using binary32 products and left-to-right sums, with direct parent translation for a zero local offset. Do not decode the opaque placement bytes or claim native numeric equivalence. Manager legacy placement decoding remains outside this component API.]~~

  Clarification: effects-parser §13 already settles the parser's in-memory matrix/offset/scale storage and rotation stores, without requiring a renderer row/column convention. EF19 concerns the legacy resource-load mapping and component world-position convention/order, not those settled parser stores. An identity parent returns local translation directly, including its original binary32 bits.

  **ANSWERED 2026-10-24 (re-effects-ef18):** DIFFERS — effects.md §3.1:
  legacy placement is nine row-ordered float words, translation xyz, uniform
  scale, and a revision word whose low byte is retained. Loading sets rotation
  and translation flags even for numeric identity. Component shortcuts test
  flag bytes, not zero offsets. Dot products sum z,x,y with caller x87 precision;
  brush and game have the separately documented binary32 spill boundaries.
  Six proposed conformance vectors include cancellation and signed-zero cases.

<!-- from: replace-effectsres30 (2026-10-26); imported by re-ef31 -->
- ~~**EF27 - Typed visual-name dictionary insertion boundary (replace-effectsres30):**~~

~~EF26 is implemented for the renderer codecs. Nonempty resource Read now reaches
the visual name/index map, whose retained layout and traversal are specified in
effects.md 18.7.2 and whose insertion/terminator behavior is in 18.9.6. The current
adapter refuses the first nonterminator pair instead of guessing a native call;
this prevents replacement acceptance for the shipped directory.
Reproduced with the native manager in `build-runs/game-results/effectsres30-codecs-on1.log`
in the main checkout, line 166: all description sections were traversed before
the explicit EF27 refusal; the short run's clean verdict is not payload credit.~~

~~Please specify the independent typed native dictionary-insertion dependency
used here (not native resource Read): address, receiver, ordered arguments,
hidden result storage if any, stack cleanup, return channel, key/value layout,
copy/allocator ownership, duplicate-key behavior, and any spelling normalization.
The ABI inventory has a string-keyed insert_unique at 0051FDC0 with two stack
words and void return, but does not identify those words or establish that this
is the resource's operation. The published string-key lookup cannot establish
insertion's typed arguments. Alternatively specify the complete string hash,
bucket growth and chain insertion contract for a compatible independent insert.~~

~~Worked case requested: retain an old entry, read two entries with the same mixed
case spelling and different indices, then the exact (end,FFFFFFFF) terminator;
state the resulting nodes/indices/spelling and writer traversal. No helper call
or hash algorithm has been guessed. effectsres remains default-off and not ready.~~

**ANSWERED 2026-10-26 (re-ef31):** DIFFERS — effects.md §18.9.7: the resource
calls bucket growth `00587010` then duplicate-preserving insertion `00589E80`,
not unique insertion `0051FDC0`. Both use dictionary H = C+4 = resource R+8 in
ECX. Growth takes count+1 by value, callee pop 4; insertion takes an 8-byte
caller result pointer then the borrowed 16-byte string/index pair pointer,
callee pop 8, with EAX equal to that result pointer. Result words are new node
and H. Nodes/key bytes are independently allocated/copied; the numeric index
is unowned. Keys remain exact counted bytes without normalization. New equal
keys link immediately after the first equal node; old nodes/indices survive.
Signed-byte multiply-by-5 hash, unsigned modulus, full prime growth table and
rehash order are also specified. EF27-C1 gives the requested retained-old-entry,
two mixed-case duplicates, terminator and writer traversal; C2–C5 cover typed
ABI/copy ownership, third duplicate, byte-string hash and growth reversal.
All are static predictions; no runtime observation remains required for this
boundary, and no replacement acceptance or game payload credit is claimed.

## EF28 — full-house startup fails before any effects-resource factory call

Lane `replace-effectsres30`, 2026-10-27. Acceptance (a) remains unqualified:
the train288 startup failure reproduces, but an unchanged-source repeat passes.
Please specify the lifecycle/first-use contract involved before an engine fix
is attempted; the resource implementation cannot explain this difference from
its current spec.

- After merging main (merge `bca6ee6ca`), full house list plus `effectsres`,
  `effectsres30-r3-house-on1.log` lines 10988–11009: first native audio tick
  faults at `004E2C06`, reading null, ECX=0. Stack: `004E51AD`, `004E537B`,
  `004E957F`, `004E9592`, `004EA216`; no plugin return address. Verdict corrupt,
  2026-01-27, real_seconds=0.031, foreign_exceptions=1, ref_warnings=0.
- `audio-filters.md` section 2 identifies `004E2C00` as the view-level global
  filter, using the 3D view at `00B43DC4` without a null check; `audio-ambience.md`
  section 8.4 specifies the enclosing CanHandle/HandleCell calls. `view3d-window.md`
  section 2 says its constructor publishes the view through SL::OnView3DCreate.
- This failed log has **zero** GetClassObject calls for CLSID `2A5118AD` and
  zero `cSC4EffectsResource` factory entries. Only registration/enumeration
  occurs. The resource's constructor, bindings and Read/Write are not entered
  through our creator before the fault. The effects manager is replaced too.
- Unchanged source/DLL, full house plus key: `effectsres30-r3-house-on2.log`
  and `.verdict` are clean through 2032-01-01, foreign_exceptions/ref_warnings=0.
  Same-source key-off control `effectsres30-r3-house-off1` is clean to that date;
  key-off `house-off2` and `house-off3` are clean to 2026-04-01. Failed and passing
  candidate DLL SHA256: `5ffd9d89b6428de369ebe045b1a4ef7a6ce589cebb545faf9ba066eeec768ac6`.
- The failed candidate constructs a **second** replacement effects manager
  (factory call 841, log lines 9552–9559) after PostCityInit; passing on2 and
  off1 each construct only one. This is an observation, not a causal claim.
- All logs/verdicts above live in `C:/Users/tlori/FreeCity4/build-runs/game-results/`;
  exact requests are in its sibling `game-queue/done/`. The original failure is
  `regress-train288-replace.log` (same native stack). Prior key-off
  `effectsres30-r2-s1off1.log` also has the same native fault, on earlier code.

Which specified path creates/destroys the city-screen view and the second
effects manager during this load, and which game interface/lifecycle boundary
guarantees that native audio sees a live view before its first tick? Is a
view-less load valid for that service? Please also identify any resource-class
registration side effect, before its first factory call, that the replacement
must preserve. No native audio/global patch or speculative timing fix was made.

**ANSWERED 2026-10-27 (re-ef32):** DIFFERS — [effects.md](../../effects.md)
§18.11, [audio-ambience.md](../../audio-ambience.md) §2.5 and
[view3d-window.md](../../view3d-window.md) §2.1 specify the missing boundary.
The app creates/publishes its city-screen view before city init and ambience
tick-list enrollment on PostCityInit; synchronous PreCityShutdown disables
and removes ambience before screen destruction clears the view. Native view
filters have no view-less active-city behavior. City and region screens own
separate effects managers; region initialization requests another instance at
`007AD129` and publishes it at `00B43D1C`, without creating a city-screen view.
Resource class registration only stores its creator/identity and enumerates
the class with zero as callback argument 2; it has no hidden audio/view setup.
EF28-C1–C3 give worked cases. The failed run's actual second caller and reason
for a missing view cannot be identified statically from the supplied log:
§18.11.3 requires paired caller/lifetime traces, every write to `00B43DC4`,
and nested city-message/tick-list boundaries. That execution-specific part is
narrowed to an explicit runtime observation, not asserted as a resource fix
or qualified replacement acceptance. The corrupt run and clean repeat remain
separate evidence.

## DSEX33-EF01 — effects height-map service used by native aircraft

Lane `disasters-exact33`, 2026-10-28. The coordinator's effects-only Pyxis
comparison `compare-20261027-230157-13af6338/result/game.log`, line 1903,
faults reading null at `00537188`, inside the ABI-identified helicopter
`AI_ControlLiftOverBuildings`. The replacement's manager slot 9 returned
null. Aircraft spec section 4.2.4 requires a service obtained once through
operation +0x24, then queried through +0x18 with `(1, X, Z)`.
The SDK names these `GetEffectMaps` and `MapLerpedHeight`.

Effects sections 2 and 11/18 do not specify map contents or their lifecycle.
Please specify the map dimensions/count, coordinates and interpolation,
map-1 population/update rules (including building/terrain changes), outside
bounds behavior, and ownership/QI contract. The lane exposes an owner-lived
empty service with zero heights as a marked SPEC-GAP to remove the null
call; this does not claim compatibility for map values.

**ANSWERED 2026-10-28 (re-blockers37b):** DIFFERS — effects.md §18.12 specifies counted map IID 0A5EAA1A, borrowed manager getter, terrain/overlay dimensions, unchecked bounds, shipped interpolation order and building maintenance.

## DSEX33-EF02 — effects tick registration and fixed-clock consumption

Lane `disasters-exact33`, 2026-10-28. Effects section 6.1 says Init calls
`EnableAutoStep(1, EffectsTickCallback)` on the animation tick manager, with
the callback posting `0x697D01ED`. However animation-tick-manager.md section 2
defines slot 14 as `EnableAutoStep(bool, u32 intervalUs)`, not a callback
registration. Please reconcile these contracts and specify the effects
manager's animation-target registration, priority, callback registration and
shutdown ownership. The adapter currently subscribes through the message
server; no guessed callback-as-integer call has been added.

Please also specify any clock/stopwatch reads made by the effects manager's
Init, animation tick, ordinary DoEffectsTick and draw pass, including their
conditions/order. Simulator section 5.8 advances the fixed counter per read,
so omitting a read can change another owner's animation delta. This is a
possible explanation to investigate, not an established cause.

Evidence: final effects-only qcheck-28.log is clean/ref_warnings=0 but exact
0/4. Pennoli result `C:/Users/tlori/FreeCity4/build-runs/golden-runs/compare-20261028-072900-5d057a8e/result`
has two different records, with no shared-rand or DIGEST difference. In
`896E75AF/299B2D1B/0`, the only payload difference is the opacity tween's
elapsed float at record offset 276 (record starts at byte 86621, size 335,
object ID 318): reference `0x3D07325A` = 0.0330070033669 seconds; candidate
`0x3D07314D` = 0.0330060012639 seconds. Delta is 1.00210309029 microseconds;
the other four different bytes are that record's checksum. Carjacked vehicle
record `CBB32B0A/299B2D1B/0` has small position/destination changes in three
324-byte records. The same differences occur in qcheck-25.log. This does not
justify adding dummy clock reads or changing FC4Schedule.

### DSEX33-EF02 follow-up: direct tick delivery and embedded timer (round 11)

Animation-tick-manager.md sections 4/5 specify direct delivery to registered
targets, rather than message-server broadcasts. The replacement now registers
its +8 target in the every-tick band (192, exact priority remains a SPEC-GAP)
and removes it at Shutdown. Effects-only qcheck-34.log remains 0/4 exact,
with the same first records/counts as qcheck-33.log; all four runs are clean
with ref_warnings=0. Registration alone has not removed these differences.

Animation-tick-manager.md section 7 also identifies the effects Init call as
slot +24 on the embedded +CD8 object, with arguments 0058D9A0 and zero,
rather than EnableAutoStep. Timers.md section 2.2 specifies that shape as
SetCallback(fn,arg) on cRZCallbackTimer. Please specify whether this is that
timer, its construction/start/stop order, period, and callback ABI. This
narrows the earlier contradictory contract; no callback-as-interval call,
guessed timer period, or compensating clock read has been added.

### DSEX33-EF02 follow-up: reach replay has no getter/creation calls (round 12)

Final-source effects-only qcheck-39.log remains exact 0/4, with the same
first differences as qcheck-38.log. Reach result:
`C:/Users/tlori/FreeCity4/build-runs/golden-runs/compare-20261028-092012-9e689352/result`.
There is no shared-rand or DIGEST difference; three save records differ.
An earlier identical first difference in qcheck-38.log was decoded using
landvalue section 10: intrinsic/slope records match, total land value differs,
and the simulator's saved average is reference 74 versus candidate 87;
its dirty byte is zero on both sides and its mean altitude is 290 on both.

Separate fixed-schedule replay `disasters-exact33-r12-query-reach-2` is clean
with ref_warnings=0. The new opt-in trace confirms one primary effects QI
(`C97CF5CD`) and its enabled marker; no other QI, HasVisualEffect, creation,
GetEffectMaps or GetTallestBuildingAltitude entries occur through April 9.
This replay uses the same city/action script, but is not a qcheck proof.
Native-only `disasters-exact33-r12-native-names-2` is also clean: all 3,465
Boolean membership queries match at load and each of three month samples.
That observation covers neither creation nor lifecycle equivalence.

Please specify the effects manager's Init and ordinary-message/timer
lifecycle, including collaborator writes and clock reads. These observations
do not establish the cause of the land-value difference, but do not identify
a returned getter value to change. No schedule change or clock compensation
has been made.

**ANSWERED 2026-10-28 (re-blockers37b):** DIFFERS — effects.md §§6.1–6.1.1 specifies direct target +8 at priority 192, embedded callback timer started at PostCityInit with period 1000 ms, shutdown, collaborator writes and clock ownership.
