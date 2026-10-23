| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| verify-persist-keylist4: published KeyList owner and independent fixture | Codex / verify-persist-keylist4 | in progress | Baseline, contract and factory audit, literal tests, production operations, native controls |

## Handoff
- Goal: finish authoritative persistence_manager::KeyList section 5 operations and independently verify original KeyList through the public typed factory.
- Done: initial status clean; operating rules read. Authored three independent literal conformance tests before missing-method implementation.
- Contract audit: persistence.md section 5; SDK cIGZPersistResourceKeyList.h publishes all eight operations and CLSID 656B8F2D/IID 656B8F12; cIGZCOM.h documents GetClassObject as class instantiation. No KeyList registration in plugin director/subsystems. No factory binding gap inferred.
- Baseline all started once, exec session 37809; test phase completed, clippy phase started. No duplicate checker. Process enumeration works; Win32_Process command-line access denied.
- Production milestone: insert_list, erase first, erase_all, is_present, valid get_key, enum_keys(copy) extend existing owner only. New private static-link transport retains two real production owners; config keylist_verify defaults off; pre-city synchronous fixture is wired.
- Fixture: validates vtable and all 11 published interface method module owners on both created lists before scoring; compares literals, native and Rust; Insert(list) bool observed only; balanced AddRef/Release values unscored. No natural/full-owner promotion.
- Baseline all session 37809 remains live after build completion; no CTest log/result yet, do not duplicate checker. Initial fmt invocation with relative paths failed; use absolute paths. New transport formatting corrections in progress.
- Checker diagnosis: OS process command lines identified own qbuild PID 5912 -> fc4verify 2396 -> sccache stats-only child 34812 hung after build. Terminated only that stats child; CTest then started (PID 32232), presently progressing. No shared daemon/tool changes.
- Scoped absolute-path fmt now passes (3 touched Rust files); engine source milestone eb96fcca; tests-first milestone 7f9459ed. Factory-owned/refaudit/extra-reference counters now separated for refusal accounting.
- Baseline complete: Rust 5329/0, clippy clean, build success, CTest 67/68 (ret_channel_check failed on five newly written fixture uses; source changed during baseline). Logs preserved under build-runs/baseline. Local [abi-ret] comments now cite published scalar bool contracts; Insert(list) bool discarded entirely. No ABI table edits.
- Quick started once: session 18211, qbuild PID 33712 / fc4verify PID 27796; Rust complete, CMake linked new fixture. Do not duplicate. Final all has not yet started.
- Quick checker finished after terminating only its own hung stats child 38200; CMake build succeeded. Final all started on committed source 36d90ebf, engine tree b959b2e0ee25c3f9cfff8fb111c307a96d2dc333; session 7147, PID 13944. No further engine changes permitted for this gate/control set.
- Pending controls (not submitted): vpkl4-r1-off-20261023-a, vpkl4-r1-native-on-20261023-a, vpkl4-r1-literal40-on-20261023-a. Exact baseline enable empty / disable budget / default simulator retained; literal40 is the task's exact 40-enable list plus infrastructure defaults. Same full-save script at 2031-12-01, all end 2032-01-01.
- Final gate PASS: Rust 5332/0 (three new literal tests executed), clippy exit 0/zero diagnostics, CMake 4/4, CTest 68/68 in 72.39s. qbuild exit 0. Retain fc4verify's tree-changed flag: only lane documentation changed during this gate; git diff -- engine empty and engine tree b959b2e0ee25c3f9cfff8fb111c307a96d2dc333 unchanged. No extra all.
- Frozen once: build-runs/keylist4-frozen/FreeCity4.dll SHA256 34A7848B1588263FC818F657D30689B0439262F636B05FA5ADB473926E0041C5; PDB SHA256 0FA615A407DA8FC2EC5EB2949B455B5ADAEA043140C54E32000072E141C2FC3A. Freeze HEAD 56b5ab4f; final engine code 36d90ebf.
- OFF vpkl4-r1-off-20261023-a complete: clean, ref_warnings=0, foreign_exceptions/watchdog/rtc/overruns=0, verdict date 2032-01-01 (42.000s simulated run, 56s wall). Full save returned 1 in 11047ms at 2031-12-13, PostAppShutdown logged, frozen DLL hash prefix 34a7848b. Actual subsystem log: simulator enabled, budget disabled, every experimental subsystem off by default. Zero KeyList assertions as intended. Full verdict/log retained in build-runs/game-results.
- Next: submit native ON vpkl4-r1-native-on-20261023-a, same frozen DLL/PDB and profile except keylist_verify=1; wait without qwait. No native comparisons credited yet. No reload fixture or reload claim.

## Method and body limits

The owner attribution is **22 attributed functions / 1,071 bytes**. It is a denominator from persistence.md, not 22 unique algorithms, not a measurement of all native bodies, and not full-owner credit. This is the first constructed-original KeyList owner phase; the manager and runtime residuals remain unchanged.

| Published method/body | Production and literal coverage | Native scope and limit |
|---|---|---|
| Insert(key) | Existing append/true preserved; deliberately unsorted inputs and repeats | Fixed literal effects and true channel only |
| Insert(list) | New method delegates source enumeration to target insert | Two independent owners; append order, source unchanged, clear/reuse independence, empty source; bool discarded/unverified |
| Erase | New first exact three-word match removal | Duplicate first removal, second removal, absence, surviving type/group/instance near matches |
| EraseAll | New clear/true | Empty/nonempty clear, reuse, source independence |
| IsPresent | New linear exact equality | Present and absent before/after erase and clear, zero and FFFFFFFF words |
| Size | Existing semantics preserved | Both owners and all literal snapshot lengths |
| GetKey | New copied value at valid index | Insertion order at valid indices only; invalid index expressly unverified |
| EnumKeys | New ordered copied values | Empty/nonempty callback count/order, callback reference distinct from owner key addresses, retained copy edits leave owner unchanged; no mutating callback |
| AddRef / Release | Existing logical 32-bit reference methods untouched | Only balanced extra AddRef/Release observations and created-reference cleanup; no numeric constructor-count comparison, overflow or after-free calls |
| Constructor / destructor / QI / aliases | Rust logical constructor 1 remains existing choice | Native constructor count, QI, overflow, reentrancy, self-insert, invalid GetKey and unknown body aliases remain unverified; no invented per-body attribution |

Private transport holds two actual production `KeyList` instances. It has no list algorithm, cloned verifier owner, native-output seed or resynchronization. The C++ fixture uses public SDK CLSID 656B8F2D/IID 656B8F12 and `GZCOM()->GetClassObject`; both fresh lists must have independently logged vtable storage and all 11 method addresses owned by the identified supported game image before any native assertion can score. Factory/null/wrong-shape/module/alias failures refuse credit. All fixture objects are cleaned before city simulation.

Existing resource-manager verification (23 commands / 66 assertions, and the previously accepted resource/current40 23/23 helper result) is separate inherited evidence, not new KeyList coverage. No manager, cache, factory, record, resource, runtime or natural-owner promotion is claimed. There is no reload claim: these controls start from the ordinary harness city, without a staged reload fixture.
- Next: scoped fmt and serial quick after baseline resolves, then commit frozen engine source before one final stable all; freeze DLL/PDB and run OFF first. No requests yet.
- Constraints: no RE sources, no duplicated list algorithms, no public ABI table/version changes; original provenance required for both lists in both ON controls.
- Native controls required in sequence: OFF through 2032 with save/shutdown, native ON, literal40 ON using one frozen DLL/PDB. No requests outstanding.
