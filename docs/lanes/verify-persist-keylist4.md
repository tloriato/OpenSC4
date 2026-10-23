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
- Next: scoped fmt and serial quick after baseline resolves, then commit frozen engine source before one final stable all; freeze DLL/PDB and run OFF first. No requests yet.
- Constraints: no RE sources, no duplicated list algorithms, no public ABI table/version changes; original provenance required for both lists in both ON controls.
- Native controls required in sequence: OFF through 2032 with save/shutdown, native ON, literal40 ON using one frozen DLL/PDB. No requests outstanding.
