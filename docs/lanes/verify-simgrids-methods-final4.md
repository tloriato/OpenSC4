| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| verify-simgrids-methods-final4 (actual production method verification) | `verify-simgrids-methods-final4` / engine lane, detached worktree | wip: code270a49a2+d81d9a29; cargo4951/0, clippy0, ctest68/68; normal39-b/current40-d CLEAN ref0; native39-a/b CLEAN ref1; native39-2032c exited; SG3 fixture differences31/run; required save/reload chain incomplete; ABI unchanged | 1. Review adverse controls and SG3. 2. No fourth lane round or promotion. |

## Final report

**Wip, no promotion.** Actual shipped production methods now execute against independently owned state.
The required zero-warning native/current40/native save/reload chain did not pass. Two longer native controls
have reference warnings; another control exited before its save. These failures remain unreconciled.
This final logical round is exhausted; no fourth round or restart is requested. No requests remain outstanding.

### Implementation and ownership

- `270a49a2` replaces the active plugin's stateless Rust record transport with direct shipped factories
  `CreateFC4SimGridU8/S8/U16/S16/U32/S32/U64/S64/F32/F64` and a typed operation-tape adapter.
  `d81d9a29` adds complete scalar-bit diagnostics and SG3. Neither changes production grid algorithms.
- Each natural lifetime creates one independent object, reads its single initial supplied record through
  `cIGZSerializable::Read`, then retains that object. Subsequent native captures only compare with production
  state. Captured scalar/bulk setters, fill, geometry, averages and shutdown replay through public typed methods.
  Bulk comparisons obtain all elements through production getters; complete records compare every byte.
  No scalar query decodes/clones an entire grid. No native fallback, post-bootstrap native resync, or host observer recursion.
- Factory count0 is explicitly AddRef'd; typed and serializable QI references are audited, the temporary
  factory reference is released, and the two owned references are released at lifetime end. All completed
  native/fixture lifetimes log serial_release1/final_release0. Owned objects never enter native reference inputs.
  The existing observer mutex protects map lookup, calls, captures and destruction across getter threads;
  fixtures are private to their startup thread. No shared Rust Rc/RefCell state or unsafe Send implementation.
- `backend=shipped-cpp` counters count dispatched semantic methods, with scalar_get/set and bulk_get separated.
  Guard geometry getter calls and reference management are excluded. `method_read/write` are separate and
  include bootstrap and diagnostic serialization; bootstrap gets no equality credit. Legacy `changed` counts
  serialized Update outputs only (natural0 does not mean no state changes); scalar setter counts are actual calls.
- The unused internal Rust bridge remains explicitly labeled historical retained-record transport. No public
  ABI export/version, table, map, shared configuration, default, subsystem registration or generated ABI file changed.

### Historical evidence correction

The parent report now carries an explicit correction: its historical8,621,314/8,621,314 total includes
8,620,930 scalar queries that indexed retained serialized bytes; its1,010,081 scalar setters patched those bytes.
They are retained-record probe evidence, **not production-method verification**. Counts and earlier adverse
runs remain preserved, with no new coverage credit. New measurements below are separate shipped-C++ evidence.

### Gates

- First required all: cargo4951/0, clippy0, ctest68/68; tree changed while it ran, so not the stable final gate.
- Focused: quick cargo530/0, C++ build PASS; all67 C++ tests PASS, including ten-format persistent method-tape
  tests with independently read initial state, repeated get/set calls, corruption and truncation negative controls.
- Stable all at270a49a2: cargo4951/0, clippy0, build2/2, ctest68/68 PASS72.8s.
  Final stable all atd81d9a29: cargo4951/0, clippy0, build2/2, ctest68/68 PASS72.6s. Later edits are documentation only.
- Rust comment file fmt check PASS with absolute path. Initial relative fmt path failed; corrected immediately.
  q.py could not start Bash (E_ACCESSDENIED); focused tests used the permitted quiet fc4verify cxxtest wrapper.
  qbuild rejected cxxtest as an unknown step after its quick step passed. No mass reformat or unrelated test changes.

### Complete-run results

All names below begin `sgm4-r1-`. Native figures use release reports only; cumulative shutdown reports are
retained in the audit but never added again. Full lifetimes pair by token and release, including reused tokens.

| Label suffix | Verdict / date / refs | Native exact/compared | Tape method_calls | Scalar get / set | Bulk getter calls | Native Read / Write | Cleanup |
|---|---|---|---|---|---|---|---|
| native39-20261022a | CLEAN2032-01-01 / ref1 | 23,030,495/23,030,495 | 27,774,902 | 23,029,595 / 2,652,243 | 2,093,056 | 300 / 900 | 310 |
| native39-20261022b | CLEAN2033-01-01 / ref1 | 4,312,882/4,312,882 | 5,219,544 | 4,312,690 / 505,438 | 401,408 | 64 / 192 | 74 |
| normal39-20261022b | CLEAN2032-03-01 / ref0 | 1,080,802/1,080,802 | 1,311,866 | 1,080,730 / 116,440 | 114,688 | 24 / 72 | 34 |

- Completed native totals:388 lifetimes;28,424,179/28,424,179 exact, differences0/refused0/capture failures0/
  resync0/fallback0. Tape methods34,306,312: scalar gets28,423,015, scalar sets3,274,121, bulk getters2,609,152,
  shutdown24. Read388/Write1164. Observed adapter calls31,697,524: get28,423,015 + set3,274,121 + bulk_read364
  + shutdown24; natural bulk_write0/fill0. Completed runs are measurements, not a clean-gate claim for ref1 runs.
- Denominator: scalar28,423,015 + bulk-data364 + bulk-complete-Write364 + shutdown-return24 + shutdown-Write24
  + release-Write388 =28,424,179. Natural owner=pollution, CLSID49B9E605/int16 only. No power/other-owner credit.
- In each of these three native runs: ten constructed formats,569/600 checks, differences31/refused0/failures0/
  resync0/fallback0; tape methods1200 (get50/set100), Read40/Write340, ten negative controls, ten final releases0.
  The600 include30 native reference-count sanity controls, separate from independent grid-method comparisons
  (539/570). Read includes ten bootstrap reads plus explicit identical fixture Read inputs, including invalid version.
  Negative controls alter complete buffers and lengths and do not enter equality totals. Across these runs:
  fixture1707/1800, differences93, negative controls30; owned cleanup418/418 returns1,0 including388 natural objects.
- **SG3:** u32/u64/i64 each have ten average differences; f64 has one cell-average difference atS3. Full scalar
  bits/op/size/rectangle are logged for every difference in the diagnostic version. Examples: u32 native40000000
  versus shipped40000003; i64 native0048D15A00000000 versus shipped0048D159E26AF380; f64 native4014E38E40000000
  versus shipped4014E38E38E38E39. The normal-speed run repeats all31 differences. No average exactness claim or fix.
- `current40-control-20261022d`: independent parent-fixture control, CLEAN2033-01-01/ref0. All64 observed natural
  replacement lifetimes refused and paired with releases;4,818,289 unverified adapter calls, exact0/0, no owned
  method calls or comparisons. Ten fixture refusals, comparisons0. No allocation/capture failures, resync or fallback
  observed. These are refusal controls, not equality successes. No save script, so no saved-city roundtrip claim.

### Adverse controls and incomplete save/reload requirement

- native39-a and native39-b each report one warning naming the **native land-value object** at development's
  `Unhold:254`. Both end cleanly otherwise (foreign exceptions0, watchdog stalls0, rtc0, overruns0). This is a call-site
  observation, not a causal diagnosis or an ownership fix. Their guards stopped save copying and downstream requests.
  Full scripted saves/digests occurred in the game, but no immutable output copy or subsequent reload is claimed.
- `native39-2032c`: exited(code-1) without a verdict; plugin first-chance access violation. Last progress remains
  2031-12-02 paused/emergency; scripted2032-02-01 save and target2032-03-01 never reached. Final natural denominator
  unknown; grant no equality/control credit. The guard stopped the chain. No dump, map or executable investigation;
  no causal attribution to the owned grid or any unrelated subsystem. No downstream c requests were submitted.
- Normal-speed March/May/July2032 was attempted as a separately bounded chain after the longer controls failed;
  it did not pass its first native stage. No current40/native roundtrip save chain completed, no saves were copied,
  and no prior adverse verdict was overwritten or discarded. No retries or further requests remain outstanding.

### Frozen artifacts, provenance and bounds

- A binary: `build-runs/dll-sgm4-r1-20261022a`, DLL EDEF6F3BF70719CC40A0C509645C26DEDC3281686CAF1E6E13BF88B602BA6B60.
- Final b binary: `build-runs/dll-sgm4-r1-20261022b`, DLL068B26EC28F230712E40053BF5D796D900026F7915751BDB4A777BD46F074CE3,
  PDB9A30DAB9D18B6E5FB0FF40F83F544B7BDBA7FD8AE36E747B72D7902F4CA4386C. Rehashed after the last run;
  b/normal/c/current40 use this exact DLL/PDB. Requests and receipts retain the harness's binary identity.
- Parent fixture `build-runs/fixtures/sgv4-r1-native39-20261022c.sc4` under main was rehashed as
  F03DA3420CD8FABE50BD887890310D2D5577E5C5C109F76E3906EF84E255FB7A before guarded submission. A uses stock city;
  all later requests use this unchanged fixture. No game files committed.
- Explicit current40 profile (native39 removes only simgrids): budget,weather,ordinance,demand,power,plumbing,
  flammability,pollution,protection,crime,aura,flora,landfill,residential,zones,neighbors,development,traffic,tract,
  lotdev,commercial,industrial,civic,parks,zonedev,history,clock24,dispatch,demolition,automata,netmgr,netlot,nettool,
  ports,citysit,propdev,propmgr,lotcfg,occmgr,simgrids. No configuration or default changes.
- Local evidence: `build-runs/audit-sgm4.py`, `audit-sgm4.json`, `audit-<label>-simgrids.txt`, complete game logs/verdicts,
  immutable binaries/hashes and unique requests. The audit verifies full counts, empty outstanding lifetime maps,
  single natural Read per lifetime, positive actual get/set counters, expected negative controls and cleanup1,0.
  C's exited verdict/log are preserved separately, excluded from complete-run aggregates. C log SHA256
  684DFABF75E86557A6E30D6E49EE2FF04607B25A0650EE69BCA80D2127456165; verdict5697349EEE1010DE1053638D5F940E7323CECBF802770C9735E19C6FE2ACD41C.
- **SG1/SG2 remain open**: raw structure/lifetime/out-of-range dependencies and unsized factory serialization.
  Bool, unobserved owners/formats, natural averages/geometry/fill/bulk-write, map utilities/iterators and allocation
  failure paths remain outside natural evidence. Constructed coverage is separate and bounded. No whole-family,
  replacement-owner, defaults or project coverage promotion.

### Disposition

Question **SG3** appended to `docs/specs/questions/simgrid.md`; no spec behavior changed. The generated question
index still needs coordinator regeneration (tools were forbidden and only this question file could be edited).
Coordinator review must retain all adverse controls and determine a separate disposition for the reference warning,
incomplete/crashing save control and SG3 before publication/promotion. No fourth lane round, restart, merge or push.
