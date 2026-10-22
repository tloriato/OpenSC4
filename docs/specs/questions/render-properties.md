# render-properties: open questions
<!-- Questions about [render-properties.md](../render-properties.md). Append new entries at the END; when one is answered, move it to answered/render-properties.md. Rules: README.md. -->

<!-- from: re-renderprops (2026-10-19) -->
- **RP1 - §1.2, §3:** who reads the two extra values of an integer or float entry (`+0x20`, `+0x24`; for example
  1–10 for `kParticleEffectsLOD`, 0–5 for the backing-store zooms)? None of the 18 slots does, and set ignores them.

<!-- from: re-renderprops (2026-10-19) -->
- **RP2 - §1.2:** what the two extra bytes of a boolean entry (`+0x1D`, `+0x1E`, always 0 at registration) are for.

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

<!-- from: glue-render-properties4 (2026-10-22) -->
- **RP7 - §1.1/§1.4 application/view installation and native construction:** What public acquisition/factory and installation contracts permit creating independent native owners and supplying a replacement to the application and view? The SDK GetRenderProperties route is borrowed and has no setter; §1.1 identifies constructors but does not publish a callable constructor ABI, factory ID, or application/view ownership-transfer boundary. Please specify full P/U interface installation and lifetime, including view construction, failure and shutdown. [Implement only native-text Rust adapter and independent comparisons against the acquired original; withhold replacement and native fresh-owner/deleting-destructor/locale-construction experiments. Do not invent object layout writes, class registration, constructor calls or a forwarding proxy.]
