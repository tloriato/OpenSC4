# persistence: answered questions

<!-- from: re-persist-manager4 (2026-10-22) -->
- **PR4:** ([persistence-manager.md](../../persistence-manager.md) §9, service update return) Which shipped
  callers observe the return byte of the manager service's slot 11 (`0x0097356D`), and what deterministic
  return should the clean-room implementation expose? Native instructions OR every cache update result
  into an uninitialized stack byte `[EBP-1]` (`0x0097359C`) and return it (`0x009735AF`); the empty-cache
  path is uninitialized too. Callback order/arguments are specified, but an exact boolean baseline cannot
  be inferred from this body. [expected: callers ignore it; verify call sites before choosing an initialized
  OR reduction or constant true; neither choice is established by this audit]
  **ANSWERED 2026-10-22 (re-persist-update-policy4):** DIFFERS — Native AL is the pre-existing stack byte
  OR every callback AL, without normalization; empty/all-false returns retain that unknown byte. The
  shipped framework service dispatcher calls this slot at `0x0087A8C1` and ignores its return, advancing
  the iterator instead. Manager registration into that dispatch list is traced from `0x0097530F` through
  framework registration `0x00879D21`. No observing caller was found in the traced registration, dispatch
  and explicit service-ID lookup paths. §9.2 explicitly chooses initialized logical OR with canonical
  false on empty/all-false, true otherwise, preserving every callback; this is a deterministic clean-room
  policy, not a recovered native baseline. §10.1 separates callback traces, native constraints and chosen
  outputs in six independent vectors. The audit does not claim behavior for arbitrary external plugins.

<!-- from: re-persistence4 (2026-10-21) -->
- ~~**PR1:** (§3.1) which native-order flag do the DBPF packed-file records (`cGZDBRecordPackedFile` constructor
  `0x0096D859`, called from `0x0096D8D0`, `0x0096D9D1`, `0x0096DA30` and the two write-record constructors) receive
  from their segment, and can any segment in the shipped game be configured big-endian? [expected: always 1 for
  the shipped packages and saves, which are little-endian]~~
  **ANSWERED 2026-10-27 (re-x-fw):** DIFFERS — persistence.md section 3.1 corrected: the packed-file records never take the order flag from their segment. All five construction sites (0x00970291, 0x009702C5, 0x009703A0, 0x00970C71, 0x00970E87, via the open and write routines 0x009701FD, 0x00971178, 0x009712D7) pass the constant 1, and nothing in the segment can change it, so no big-endian packed-file record exists. New section 3.1.1 lists the kinds and construction quirks.
