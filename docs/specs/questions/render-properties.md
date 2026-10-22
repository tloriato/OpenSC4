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

<!-- from: glue-render-properties4 (2026-10-22) -->
- **RP8 - §3 registered defaults versus configured application values:** Which caller/configuration writes transform the registered defaults into the live application values, and at what supported acquisition point can an independent owner receive those same inputs without copying original outputs? Same-process run `grprops4-r1-native-48237e0d` (immutable DLL SHA256 `5053463be47b1b96f77fbd6e57bef8aed62a2d1e7fff74ba1d8e09632faa4346`) is clean2032ref0; all77 names and 5500 lookups match, but 21 live values differ from §3 defaults at PostAppInit and later. Native/Rust pairs: bool22=0/1, bool31=0/1; int5=400/10000, int6=2/0, int7=1/5, int12=0/2, int13=5/3, int14=5/4, int15=3/1, int18=2/4, int21=0/1, int22=2/0; float2=8/2, float3=0.25/1, float5=2/1, float6=0.2/0.0001 (bits 3e4ccccd/38d1b717), float7=40/12, float8=24/12, float9=12/6, float12=0/1, float13=0/1. [Keep constructor defaults literal; record these as live-configuration differences, not a default correction. No seeding/resynchronization from native getter outputs; no full-value shadow or replacement claim. Need published caller input/order contracts.]

  **RP6 empirical addendum (same run, uncontrolled active profile):** Direct original-CRT probes on the game thread returned `(input,isupper,tolower,low-byte)` = `(73,1,105,105)`, `(105,0,105,105)`, `(-128,0,128,128)`, `(-48,1,240,240)`, `(-1,0,255,255)`. This is evidence against assuming the pure C profile in this process. Signed-byte original/Rust Find probes, including every high byte alone and after RS, match the independent scalar oracle. No accepted CRT locale name/codepage was measured and no locale mutation was attempted; RP6 remains open, including before/after-construction controls (RP7 native factory also absent).

  **RP8 controls 2026-10-22:** `grprops4-r1-union39-48237e0d` is clean2032ref0 and `grprops4-r1-normal-48237e0d` is clean2026-04-01ref0 (request turbo0). All13 checkpoints across three processes retain exactly the same21 pairs above, including before turbo could start. Both owners' setter/restoration probes are exact, with zero resync. These controls establish persistence of the observation, not the missing configuration provenance or a full-value shadow.
