# render-properties: open questions
<!-- Questions about [render-properties.md](../render-properties.md). Append new entries at the END; when one is answered, move it to answered/render-properties.md. Rules: README.md. -->

<!-- from: re-render-properties-contract4 (2026-10-22) -->
- **RP6 - §1.3 non-C text profiles:** Which LC_CTYPE/codepage profiles occur at render-properties construction and later Find calls in each shipped language, and what are the resulting comparisons for ASCII I/i and bytes `0x80–0xFF` before NUL (including a locale change between construction and lookup)? [The owner demonstrably calls active-locale CRT tolower on sign-extended bytes, not Unicode folding; the language manager can change the locale. §1.3 and §7.2 establish the C-locale ASCII core domain only. Need locale/codepage-controlled native discriminators before extending that claim; no universal non-ASCII miss or fixed Windows encoding is asserted.]

  **Binding evidence 2026-10-22 (re-render-properties-binding4):** §1.4 now specifies
  the two private executable-CRT `int32_t __cdecl(int32_t)` primitives, supported-image
  RVAs, signed-byte inputs, low-byte mapping result, borrowed function/name ownership,
  SDK application acquisition, and explicit adapter-unavailable behavior. §7.3 adds
  independent native recipes. There is no exported CRT/ctype service to discover with
  GetProcAddress, no locale snapshot captured by the owner, and no need to invent CRT
  record layouts. **RP6 stays open for its original per-language empirical question**:
  accepted locale/codepage and startup construction order, observed I/i/high-byte results,
  and effects of locale changes. Native language-manager success can mask a failed
  setlocale; requested language alone is insufficient evidence. No profile run was made.

<!-- RP7 (filed in glue-renderprops-shadow9) is answered in render-properties.md §1.5/§7.4 and archived in answered/render-properties.md. -->

<!-- from: glue-renderprops-shadow9 (2026-10-24) -->
- **RP8 - §1.3 isolated constructor ABI:** Please confirm the private constructor at 0x007E3650 accepts only the complete-object receiver (x86 __thiscall, no stack arguments), with caller-provided aligned 0x30-byte storage, and that U slot 3 flags 0/2 can reclaim its tables while retaining that outer storage. [Literal receiver-only construction in isolated native fixtures; never allocate/free the borrowed application or embedded S. Freeing destructor flags 1/3 and Release at 0/1 remain unmeasured without a specified game-heap allocation contract. RP7 live-trace and complete-view bridges remain unavailable, explicitly reported rather than invented.]
