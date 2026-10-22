# output-streams: open questions
<!-- Questions about [output-streams.md](../output-streams.md). Append new entries at the END; when one is answered, move it to answered/output-streams.md. Rules: README.md. -->

<!-- from: re-ostreams (2026-10-19) -->
- **OS1 - §4.3:** what does the GZ log write to its stream (`SC4CreateGZLog` `0x005C6E80`, the stream handed to the
  framework log)? Only strings, or also numbers, which would come out as raw big-endian bytes? And which file name
  does `cSC4App::PreFrameWorkInit` pass?

<!-- from: re-ostreams (2026-10-19) -->
- **OS2 - §2.2, §4.1:** only for a text-mode file stream, which the game never creates: does the STLport file buffer
  translate `\n` to CR LF in text mode, and which decimal point is in effect when such a stream writes a float (the C
  runtime locale after `0x00910014` and `SetCurrentLanguage`)?

<!-- from: re-stream-core-answers4 (2026-10-21) -->
- **OS7 - §2.2:** is there a double whose `%g` text differs from the model (17 correctly rounded digits, then
  half-up at the 7th) because the runtime's 12-byte power-of-ten scaling (`___multtenpow12`, `___ld12mul`) moves
  its 18th digit across 5? Needs an emulation of that arithmetic or a runtime sweep.

<!-- from: verify-streams-native4 (2026-10-22) -->
- **OS8 - §§1.2, 3, 5: published native construction and owned destruction route:** can the ABI/spec authors publish a complete callable contract for constructing and destroying an owned original binary stream for independent native comparison? The spec gives object sizes, constructor addresses (`cRZOStream` `0x0092BBDE`, `cRZNullOStream` `0x0092BC4A`, `cRZIStream` `0x0092BC14`, `cRZNullIStream` `0x0092BC80`) and destructor addresses, but no stream construction/nonvirtual destruction entries occur in `docs/abi/direct-calls.md` or the public SDK headers. Please explicitly state each usable constructor's convention, exact `this` pointer, argument widths/pop count and return contract; and a callable nondeleting destruction contract (base destructors `0x0092B62D` / `0x0092B79F`, or concrete deleting-destructor-with-flags-zero route with its typed ABI and guaranteed no deallocation). In particular §5 does not give the input constructor's pop count or concrete null-input destructor's arguments/nondeleting semantics. The original global heap allocate/free pair is already published in `docs/abi/allocators.md` (`0x005E55E0` / `0x005E5620`); please confirm its applicability to the stated 16/20-byte stream objects and pair it with the destruction route, or publish a supported factory/lifetime route. For a controlled sink/source, also confirm whether a constructed base may use host-owned virtual sink/source callbacks while directly executing the documented original inherited binary methods, and how its lifetime must end. [Blocked before native calls: constructor addresses and virtual-method ABI alone are not promoted to construction/destruction contracts; no guessed convention, storage lifetime or destructor. No native comparison credit.]
