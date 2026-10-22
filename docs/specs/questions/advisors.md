# advisors: open questions
<!-- Questions about [advisors.md](../advisors.md). Append new entries at the END; when one is answered, move it to answered/advisors.md. Rules: README.md. -->


<!-- from: re-advisor-api4 (2026-10-21) -->
- **AD11 - section 4.2:** what path does `cISC4App` slot 55 `GetSkuSpecificDirectory` (`0x00449E20`, the string object at app `+0xC4`) hold at run time, and who fills it? `PreFrameWorkInit` fills the data, CD, Plugins, Sku_Data and user-data strings but not this one; a runtime read of the getter would settle it. [unknown; the replacement should call the getter, not build the path]
<!-- from: re-advisor-final-gaps4 (2026-10-22) -->
- **AD16 - sections 4.5 and 6.9.1 (AD12 edge follow-up):** what does the property/string provider supply for absent or non-string `title` / `message`, both during Init's property-map copy (`0x00409123`–`0x004091B2`) and refresh's Lua `ToString(-1)` plus string assignment (`0x0040938D`–`0x004093D7`)? Determine empty/default, numeric coercion and null-string handling before specifying those malformed-field cases. String-valued fields, including empty strings, are specified in AD12; do not assume every missing/non-string value safely becomes empty. [unknown; no native malformed-field result claimed]

<!-- from: verify-advice-subject4 (2026-10-22) -->
- **AD17 - sections 1.1 and 1.2, public factory spelling:** these sections call the factory `GZCOM()->CreateInstance`, but public SDK `cIGZCOM` exposes only `GetClassObject(uint32_t clsid, uint32_t iid, void** out)` for class instantiation (its documentation explicitly says it creates the class instance and returns the requested interface). Is this the intended public operation for the independently owned subject endpoint? [verifier uses only the public SDK class-instantiation operation with the published subject CLSID/IID; no private constructor, fallback or advisor factory]
