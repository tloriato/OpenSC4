| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| verify-persist-keylist4: published KeyList owner and independent fixture | Codex / verify-persist-keylist4 | **WIP**: core/fixture complete; gate Rust 5332/0, clippy clean, CTest 68/68; native and literal40 each 235/238 with PR14; literal40 ref_warnings=4 fails required control | Resolve PR14 and the four literal40 shutdown reference warnings before qualification; no automatic extra round |

## Report

**Bounded WIP, first of at most three owner phases; no automatic continuation or ready promotion.** Published production algorithms, independent literal tests and the original-owner fixture are implemented and committed. The final build gate passes. The required literal40 control fails the zero-reference-warning condition, and absent-key Erase returns conflict with the current spec (PR14). These failures remain visible; no reroll, expectation fitting, resynchronization or other-owner fix was attempted.

Only the authoritative `persistence_manager::KeyList` gained algorithms: append another list in enumeration order, erase the first exact three-word match, clear, presence, valid-index access and copied ordered enumeration. Existing new/insert/keys/size/reference semantics and manager consumers are preserved. Three literal conformance tests were committed first in `7f9459ed`; production/transport/fixture followed in `eb96fcca`, ownership accounting in `32460411`, and the final return-channel review in `36d90ebf`.

The public SDK supplies the complete typed interface, CLSID 656B8F2D and IID 656B8F12; `cIGZCOM.h` documents GetClassObject as class instantiation. The plugin registration audit found no replacement KeyList registration. No missing-binding assumption or private layout was introduced. The new `keylist_verify` opt-in defaults off and runs synchronously at PostAppInit, before city simulation. C ABI public exports, tables, version and defaults are unchanged; the new transport functions are private static-link symbols with local C++ declarations.

## Validation and immutable binaries

- Baseline `all`: Rust **5329/0**, clippy zero diagnostics, CMake **216/216**, CTest **67/68**, qbuild exit 1. The failure was ret_channel_check on five newly added fixture boolean uses; source changed during this baseline. Preserved under `build-runs/baseline/`.
- Focused `quick`: **5171 passed / 0 failed / 0 ignored** across 14 result lines for capi/conformance/core; formats dependents compiled with no-run; CMake **7/7**. PASS, with a retained tree-changed flag while the local return-channel comments were completed. Preserved under `build-runs/quick/`.
- Scoped absolute-path fmt: PASS for the three touched Rust source/test files. Initial relative-path invocation failed to find the files; intermediate formatting differences were corrected without broad formatting.
- Final `all`, once on committed engine source: Rust **5332/0**, including all three new literal tests; clippy **exit 0, zero diagnostics**; CMake **4/4**; full CTest **68/68**, 72.39s; qbuild **exit 0 / PASS**. Preserved under `build-runs/final/`.
- Retain the final fc4verify **tree-changed** flag: only the lane documentation changed during the gate. `git diff -- engine` was empty and engine tree **b959b2e0ee25c3f9cfff8fb111c307a96d2dc333** remained identical through all controls. No extra all was run for documentation-only changes.

The baseline, quick and final checkers each stalled in their own stats-only `sccache --show-stats` child after build completion. OS process metadata identified ownership, and only those children (34812, 38200, 18580) were terminated to let the existing checker proceed. No duplicate checker, shared daemon restart, tool edit or global-state change was used. q.py/Bash access and Win32_Process command-line access were denied; direct local PowerShell and OS process metadata supplied the needed local operations.

One DLL/PDB pair was frozen once at HEAD `56b5ab4f`, with final engine code `36d90ebf` and the engine tree above. All requests use `build-runs/keylist4-frozen/FreeCity4.dll`; hashes were rechecked after the controls:

| Artifact | SHA256 |
|---|---|
| FreeCity4.dll | 34A7848B1588263FC818F657D30689B0439262F636B05FA5ADB473926E0041C5 |
| FreeCity4.pdb | 0FA615A407DA8FC2EC5EB2949B455B5ADAEA043140C54E32000072E141C2FC3A |

## Game controls

All three unique requests used Konradshohe, turbo=1, end_date=2032-01-01, seconds=600, timeout=800, popup_dialogs=0, refaudit=1, autotest_end=quit and the same full-save script at 2031-12-01. There was at most one request outstanding. Each was waited in one blocking local helper call without qwait; completed verdicts were retained. No requests remain outstanding.

OFF and native ON: **enable empty, disable=budget**, with the default simulator retained. Actual subsystem logs show exactly simulator enabled and budget disabled. Their only control change was keylist_verify=0 versus 1. Literal40 ON enables exactly:

`landvalue,weather,ordinance,demand,power,plumbing,flammability,pollution,protection,crime,aura,flora,landfill,residential,neighbors,zones,development,tract,history,clock24,commercial,industrial,civic,parks,lotdev,zonedev,traffic,automata,netmgr,netlot,nettool,demolition,occmgr,propmgr,lotcfg,citysit,propdev,simgrids,citydetail,ports`

Literal40 keeps disable=budget and keylist_verify=1; actual logs show exactly those 40 enabled plus the default simulator, budget disabled, and no experimental extra opt-ins. Harness logs for every run identify staged DLL hash prefix **34a7848b**. All logs show PostAppShutdown and PostSystemServiceShutdown; foreign exceptions, watchdog stalls, RTC failures and overruns are zero.

| Label | Harness verdict / qualification | Final date / run seconds | Full SaveCity | New KeyList comparisons |
|---|---|---|---|---|
| vpkl4-r1-off-20261023-a | clean, ref_warnings=0; OFF qualified | 2032-01-01 / 42.000 | ok=1, 11047ms, executed 2031-12-13 | 0 as intended, no exactness claim |
| vpkl4-r1-native-on-20261023-a | clean, ref_warnings=0 | 2032-01-10 / 49.031 | ok=1, 11016ms, executed 2031-12-03 | 235 exact / 3 diff / 0 refusal of 238; PR14 |
| vpkl4-r1-literal40-on-20261023-a | clean, **ref_warnings=4; required control unqualified** | 2032-01-01 / 120.500 | ok=1, 11079ms, executed 2031-12-01 | 235 exact / 3 diff / 0 refusal of 238; PR14 |

Both ON controls independently created two distinct fresh empty ORIGINAL interfaces and logged their provenance before scoring: vtable **00AADA64**, module **SimCity 4.exe**, image base **00400000**, all **11/11 method addresses ORIGINAL** for each list. Native pointers were 03026814/03026894 in native ON and 02FE6814/02FE6894 in literal40 ON. No factory inference was borrowed from another DLL/control. Per control: 205 positive and 33 negative assertions; method mask FF; factory-owned/acquired/created-released **2/2/2**; extra AddRef/Release **2/2**; natural count **0**. Cleanup completed before city simulation.

Per control, assertion categories are Size16, EnumKeys-count16, EnumKeys-copy-address16, IsPresent11, Erase-first5, EraseAll4, Insert-key14, GetKey-valid52, EnumKeys-ordered-copy52 and EnumKeys-retained-copy52. Insert(list) effects are included in literal snapshots; its unspecified SDK bool is discarded. Every assertion logs exact/diff and every refusal would log explicitly. The three differences in each ON control are assertions **8/76/199**, absent Erase returns **native=1 / Rust=0 / literal=0**. Tested sequence effects match throughout. No turbo-only cause is established: the same differences occur in the synchronous pre-city fixture in both profiles. No normal-speed or reload qualification is claimed.

The four literal40 warnings are actual last-reference warnings at shutdown, all class **0xE990BFFC**, vtable **SimCity 4.exe+0x6B0EB0**: DropObject:144 twice (airport/seaport shutdown context), DropObject:126 (parks shutdown context), ReleaseToken:465 (landfill shutdown context). These are retained as a failed requirement, not excused using another DLL's control or assumed to be pre-existing. The KeyList fixture's own created and extra reference holds balance. Other-owner investigation/changes are outside this lane.

## Question and next steps

**PR14**, appended to `docs/specs/questions/persistence.md`, asks for the precise absent-key Erase return guarantee. Production and literal false-on-absence remain unchanged pending a published answer. PR1-13 were not reopened. The coordinator must regenerate the questions index; this engine lane did not run tools/ or edit the generated index.

Before ready qualification: resolve PR14 through the published spec and separately resolve the four literal40 reference warnings, then schedule any further bounded phase explicitly. This round stops with committed source/report and a WIP queue entry. No full-owner/native-exact promotion, reroll, extra label or additional round is claimed.

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
