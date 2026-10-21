| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| re-exstore-contracts4 | re | data-formats EX41-EX45: resource-manager factory contract (§8.1), Sku_Data file set (§3.1) | ready |

## Report
- What changed (docs only): docs/specs/data-formats.md gained §8.1 "Resource-manager factory contract" and two
  §3.1 bullets; the §8 replaceability paragraph now points at §8.1. EX41-EX45 are in
  docs/specs/questions/answered/data-formats.md; QUESTIONS.md is regenerated. Spec commit 07bb02cb.
- EX41 DIFFERS: RegisterObjectFactory is resource-manager slot 19 `0x00974407` (class ID, type ID, factory or null;
  null means GZCOM creates the class with IID 0xA56B8F17). Both tables are hash multimaps, so a duplicate is
  added: it neither replaces the first nor is refused, and precedence is undefined once a rehash reverses equal
  keys (`0x009978AF`). Unregister is slot 20 `0x00973FD6`, by class ID only (type entries stay). The game never
  calls it. Replacement rule: unregister the game's class ID, then register under the same class ID.
- EX42 DIFFERS: slot 9 CreateInstance(type, IID, out) takes no record. The record path is base slot 3 `0x0097554F`;
  its two trailing u32 values are ignored by these factories. An unknown type or IID gives null and false. Each
  level releases its own reference, and the caller of GetResource owns one.
- EX43 MATCHES: a fresh record at position 0, already QFS-decompressed by the segment (`0x009701FD`, `0x0096DA30`).
- EX44 MATCHES: the manager closes the record (CloseRecord on success, AbortRecord on failure); the class is never
  checked. The full reader audit stays [inferred].
- EX45 MATCHES: the multi segment's own filter is `*.dat` (ctor `0x0097EB52`), searched through FindFirstFileA.
  Files that fail to open are skipped silently.
- Tools: REA returned target_unavailable (no binary open). All evidence comes from re_query.py asm/xrefs, from
  decomp/ and from raw vtable reads (build-runs/rd.py).
- No engine/ABI/game/gate. Open questions for data-formats: none left.
- Next: an implementer can add a plug-in-mode factory test that follows §8.1 (unregister plus same-class re-register).
