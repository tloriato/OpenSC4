| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| verify-streams-native4: original binary streams verification | `verify-streams-native4` / (engine lane, detached HEAD) | blocked: OS8; no engine or ABI changes; cargo/clippy/ctest n/a; no game runs; constructed 0, natural 0, compared 0 (no credit) | 1. Publish the OS8 construction/destruction ABI and lifetime contract. 2. Implement and run independent native comparisons and required game controls. |

## Report

Stopped at the mandatory native acquisition audit, within the first 15 tool calls. Filed **OS8** in `docs/specs/questions/output-streams.md`. This lane does not establish verification coverage and is not ready for engine integration.

### Published facts and missing contract

- Read output-streams §§1, 3, 5 and the specifically requested answered OS3–OS6. OS3/OS4 concern excluded text/file behavior. OS5 makes failed length-byte state unspecified natively and failed first-chunk unwritten bytes unspecified; these cannot become deterministic native expectations. OS6 specifies no Skip(0) and null-input byte order false.
- Output base: 16 bytes; constructor `0x0092BBDE` with bool and pop 4; nonvirtual destructor `0x0092B62D`; deleting destructor `0x0092BBF8` with pop 4 and flag-bit-0 deallocation. Null output: 16 bytes; no-argument constructor `0x0092BC4A`; deleting destructor `0x0092BC5E`.
- Input base: 16 bytes; constructor `0x0092BC14`; nonvirtual destructor `0x0092B79F`; deleting destructor `0x0092BC2E`. Null input: 20 bytes; constructor `0x0092BC80`; deleting destructor `0x0092BC97`. Section 5 does not specify the input constructor's pop count or null-input destructor's flag argument/pop/no-free behavior.
- `docs/abi/allocators.md` publishes original global-heap allocation/free as cdecl `void*(uint32_t)` at `0x005E55E0` / `void(void*)` at `0x005E5620`. The allocator itself is not missing.
- Exact class/address searches of `docs/abi/direct-calls.md`, `docs/abi/layouts.md` and public SDK headers found no stream constructor/nonvirtual-destructor callable contracts. SDK stream headers publish the abstract virtual interfaces only. The general thiscall rule in direct-calls applies to functions documented there; it is not an explicit stream-constructor entry. Published virtual method tables do not establish a constructor's or a nonvirtual destructor's typed ABI.
- OS8 asks for the missing convention, this-pointer, argument/pop and destruction contracts, plus confirmation of the owned allocator/lifetime pairing and controlled host-callback route. No constructor address was treated as sufficient permission to invent these facts.

### Results and limits

- Production code, C ABI, ABI tables, shared maps, tools and defaults unchanged. No private stream replica, second-Rust oracle, copied native output inputs, periodic resync or undefined failed-byte comparisons introduced.
- Native objects constructed: **0**. Natural original objects acquired: **0**. Native comparisons: **0/0**, explicitly no credit. Four candidate class construction routes were withheld at source review; this is not a runtime refusal measurement.
- Runtime differences/refusals/fallbacks/resyncs: no runtime execution, so unmeasured; none discarded. No compatibility limit was converted into a match.
- No plugin built and no game request submitted; no outstanding label, current-40 control, 2032 run, ref-warning measurement, save/shutdown or reload result. Those requirements remain outstanding for a future implementation after OS8 is answered.
- Gate: n/a, documentation-only blocker. The task expressly forbids an unchanged full baseline; no source changes existed to justify focused or final build checks. `q.py` failed to launch Bash (`Bash/Service/CreateInstance/E_ACCESSDENIED`); subsequent allowed reads used capped PowerShell.
- Question index regeneration was not run: this lane may only append its question file and may not run `tools/` scripts or edit the generated index. OS8 was the published next free OS ID at filing.

### Next steps

1. Publish the OS8 ABI/lifetime contract in allowed sources; no original bytes or analysis output should be supplied to the implementer.
2. In a separately authorized round, implement comparisons of production core/CAPI operations with original native owner methods using independent identical inputs and explicit undefined-state exclusions.
3. Complete focused checks, one stable final full gate, and uniquely labelled required game runs with owned/native/current-40 controls, end date 2032, clean verdict/ref_warnings=0 and full save/shutdown; retain every difference/refusal/fallback/resync and report constructed/natural counts separately.
