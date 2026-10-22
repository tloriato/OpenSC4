# lua-host: open questions
<!-- Append-only questions from clean-room implementer lanes. -->

No open entries. LH1/LH2 are in [answered/lua-host.md](answered/lua-host.md).

<!-- from: glue-lua-native4 (2026-10-22) -->
- **LH3 - section 6.3.6 native slot 18 payload ABI:** the SDK forward-declares `cIGZLuaScriptServer::BreakpointEntry` but does not define it. What is the exact x86 native payload layout (field types/offsets, string ownership and construction/destruction), and output-list append/copy contract for `GetBreakpointEntryList`? The Rust-owned breakpoint payload is not evidence of C++ layout. [Native output-list binding withheld; no guessed payload or writes.]

<!-- from: glue-lua-native4 (2026-10-22) -->
- **LH4 - section 1.4.1 plugin installation timing:** LH1 explicitly leaves the duplicate-class selection policy and a public plugin callback preceding the application's factory request unproved. Is there a supported public installation sequence that selects an external ScriptServer factory before service `0x4BD10133` is registered and references are retained? Specify the callback/order and selection contract, or confirm no supported route. [No duplicate-class interception, service removal/replacement, private-field patch or cached-reference retargeting; opt-in public acquisition tracing only.]

<!-- from: glue-lua-native4 correction (2026-10-22) -->
**LH3 withdrawn as a spec blocker:** section 6.3.6 already publishes the payload size `0x34`, ID/name/line/enabled/condition offsets `0/4/0x18/0x1C/0x20`, clear-before-lookup and ordered deep-copy behavior. The SDK forward declaration alone is not a missing specification. Implementing the concrete string/list adapter remains implementation work; no additional layout is guessed in this round. Retain LH3 for provenance, with no unanswered-contract claim.

<!-- from: glue-lua-native4 (2026-10-22) -->
- **LH5 - section 6.3.1 primary destruction-dispatch slot 25 typed ABI:** the spec states the slot's Shutdown-then-+8 deleting-destructor behavior, but the SDK's public primary interface ends at slot 24. What is slot 25's exact native argument list/callee-pop count and return channel (void/AL/EAX/pointer)? The +8 slot 3 flags/pointer-return contract is distinct. [No primary slot-25 declaration or call invented; the bounded wrapper implements normal Release through the Rust owner's specified shutdown/destruction order and is not installed.]
