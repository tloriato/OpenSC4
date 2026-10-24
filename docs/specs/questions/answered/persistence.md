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
- ~~**PR2:** (§6) does any shipped package or save contain entries of type `0x62B9DA24` (buffer resources, forms
  v0/v1/v2), and which forms occur? A DBPF index scan of the install answers it. [expected: few or none; UI images
  are PNG (`0x856DDBAC`) and load through the persist-buffer / gimex factories, not here]~~
  **ANSWERED 2026-10-24 (re-persistence-buffer-corpus):** persistence.md §6.5 records a complete scan of
  GOG installer 25621: 857 DBPF package paths (440 distinct contents), 281,399 indexed entries
  (232,303 after deduplicating contents), zero target entries and no scan errors. All 417 shipped city
  saves and their identical support copies were included, along with root, locale, SKU and configuration
  packages. Thus none of the buffer forms occurs under this type in the enumerated corpus. The manifest
  records hashes and counts; no claim is made about other editions, user saves, plugins or runtime callers.
