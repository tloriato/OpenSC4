| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| glue-lua-native4 (bounded native Lua ScriptServer boundary) | Codex / `glue-lua-native4` (engine, detached) | wip: activation withheld; cargo 4964/0, clippy 0 diagnostics, ctest 68/68 on cf8d3ec1. Game lua-native4-r1-final-2032-20261022a clean 2032-01-13/ref0/full save/shutdown. Native execution callbacks/comparisons 0/0; LH4/LH5 open, LH3 withdrawn. | 1. Resolve LH4/LH5. 2. Complete native execution/list/notification/reentry transport. 3. Prove public insertion and independent native lifecycle comparisons, then actual current40 regression. |

## Report

This guarded round implements a bounded native boundary, not an activated replacement. The opt-in
`enable=luascripts` key has zero registered classes and performs public-interface diagnostics. The
incomplete factory cannot become the city's scheduler. The merged `glue-lua-scriptserver4` branch was
not restarted. No defaults, ABI tables, coverage maps, tools, regression lists or ABI version were edited.

### Implemented portions

- Corrected production-core identities using LH1: CLSID `CBD101A9`, service ID `4BD10133`, priority
  `-999999`, IID `4BD10129`. The old constants confused base constructor service-ID/priority arguments
  with class/service IDs.
- Added private CAPI allocation/destruction, lifecycle/refcount and scalar-getter transport over the
  existing production `lua_script_server::ScriptServer`. **Three new C exports**; ABI version unchanged.
- Added primary, service and +8 destruction views, shared QI/refcount identity, atomic base-service
  getters and normal Release cleanup into the Rust owner. Actual x86 view offsets +4/+8 measured correctly.
- Added typed LH2 foreign adapters: public COM VM/generator creation, CommandServer acquisition
  `(EB903A32,CB90253A)`, separate Lua-generator three-argument Init, immediate temporary cleanup,
  borrowed generic-generator sibling Shutdown, AddToTick/RemoveFromTick and reference cleanup.
  No original ScriptServer result computes a Rust result; no fixed-address calls or private-field patches.
- Native transport failures and unfinished methods explicitly refuse. A CAPI mutex prevents overlapping
  mutable Rust references; busy/reentrant calls refuse rather than implementing immediate native reentry.

### Exact remaining gaps

1. **LH4 insertion:** the spec establishes no plugin callback/duplicate-class dispatch recipe before
   application service creation. The original is already present at this plugin's OnStart. Removing it
   or rewriting retained pointers is not a supported substitution.
2. **LH5 primary slot 25:** effects are specified, but the complete typed native argument/return ABI is
   absent from the public SDK/spec. No extra primary vtable signature is guessed. Normal Release uses
   the Rust owner's shutdown/destruction order; the +8 deleting-destructor contract is separate.
3. **Implementation work, not spec blockers:** global-argument stack/closure binding (Init refuses at
   its first unbound MainThread primitive), VM execution/debug hooks, parser/file primitives, native
   notifications, full control/breakpoint methods, string/list/vector outputs and immediate callback
   reentry remain unbound. Complete these before publishing any factory. These are not forwarded to
   the original owner or disguised as successful outputs.
4. No native original-versus-production-core execution comparison, replacement load/tick/save/reload
   callback chain or whole-owner coverage is claimed. Construction remains **0/0**. The actual current40
   replacement/shadow union was **not run**; no older key list was copied or inferred.

### Validation and retained failures

- Baseline `qbuild all`: cargo **4961/0**, clippy **0 diagnostics**, build **210/210**, CTest **68/68**,
  qbuild PASS. The tree changed during this baseline.
- Focused `qbuild quick` compiled the bounded CAPI/native implementation. Three new CAPI tests in the
  final gate cover failed-init latch/Release-at-zero; cleanup following an explicitly refused primitive
  with borrowed-sibling ownership; busy-owner refusal without touching output. They supply foreign
  primitives, not a second owner algorithm or original outputs.
- First final gate: cargo **4964/0**, clippy **0**, build **4/4**, CTest **67/68**. Ret-channel check
  flagged two new uses. Fixed by citing Open's published AL contract and removing IsServiceRunning
  from diagnostics. That failed gate is retained.
- Stable final `qbuild all` on **cf8d3ec19a5765c45ebcc981d24fbcec046c2431**: cargo **4964/0**,
  clippy **0 diagnostics**, build **4/4**, CTest **68/68** in **81.4 s**, fc4verify PASS **85.0 s**,
  qbuild PASS. No source changes during this gate; subsequent tracked edits document results/questions.
- Touched-file `qbuild fmt` passed with absolute paths. Initial relative-path invocation failed due to
  wrapper working directory. q.py formatting invocation failed with Bash access denied; direct rustfmt
  touched only the new Rust file, followed by successful required fmt checks.
- Final logs: `build-runs/lua-native4-final-{cargo,clippy,build,ctest}.log`.

### Immutable game evidence

- Unique request **lua-native4-r1-final-2032-20261022a**, slot **1**, gated head **cf8d3ec1**;
  SHA256 **5832A58D9E83CB09E035D077EBA9AE0DE43296CC64995D2D79B52F84D7C0A112**.
  Snapshot/head/hash record: `build-runs/lua-native4-artifact.json`; hash rechecked unchanged after run.
- **clean**, date **2032-01-13**, `ref_warnings=0`, exceptions/stalls/RTC/overruns **0**;
  **65.406 s** simulation / **81 s** wall. No failed-start/harness-error/resend. No request outstanding.
- Full save **ok=1**, **11000 ms**, run on **2031-12-12**. Quit reached PreAppShutdown and
  PostSystemServiceShutdown. These are game/original-owner observations, not native replacement
  callbacks. Supported native reload was not exercised.
- **6/6** public traces found the same original primary **03084714** and service **03084718**,
  ID **4BD10133**, priority **-999999**, at OnStart/PreFrameWorkInit/PreAppInit/PostAppInit/
  PreAppShutdown/PostSystemServiceShutdown (framework states **1/1/3/6/10/12**). Six pairs of
  acquired references were released. Running status deliberately unobserved.
- One insertion-route gap retained at all **six** trace points; **zero** insertion attempts. Bounded
  construction: layout **1**, current/last **-1/-1**, native execution callbacks **0**, comparisons **0**,
  game-path refusals **0**, resync **0**, installed **0**. No shadow exactness or coverage credit.
  No shadow comparisons/differences exist to confirm at normal speed.
- Raw evidence: `build-runs/game-results/<label>.verdict` and `.log`; save action file and DLL snapshot
  remain in ignored worktree `build-runs/`. No game files committed.

### Questions and commits

**LH4/LH5** appended to `docs/specs/questions/lua-host.md`. **LH3 withdrawn**: section 6.3.6 already
publishes payload offsets and copy behavior. Coordinator question-index regeneration remains necessary;
this lane did not run tools scripts or hand-edit generated indexes.

Implementation **bf59c831** (substantive bounded native code before call 30); tests/LH3 correction
**7b71c6b4**; return fix and gated implementation **cf8d3ec1**; gate/request record **71891197**.
This final report records **wip**, with no promotion or automatic restart authorization.
