# Ordinance: open questions

- **OR-SE34-1 — availability chance conversion on exemplar refresh (2026-10-28, services-exact34):**
  Sections 3.1/3.2 say the stored chance is a fraction read from property
  28EBBC36; section 3.7 lists the shipped property value as 0.005. Section 9.2
  explicitly scales only pre-v4 saved chances, then refreshes exemplar fields.
  Should FromData also multiply that property's value by 0.01f? The earlier
  ordinance-only reach29-growth result differs in all 20 ordinance class records,
  with equal sizes, deliveries and DIGESTs. For CPR Training (00D0723D), the only
  payload difference is offsets 164..167: reference 0x3851B717 (0.00005f), ours
  0x3BA3D70A (0.005f). The four other differing bytes are the entry checksum.
  Evidence: main build-runs/golden-runs/compare-20261028-025502-a68b2905/result;
  reference key e12df03f46d0d85608a36c89e11b52ad74b4ba3ce8eca978f4c307af1eb2aaae.
  Please specify the conversion and whether it applies to every exemplar load
  and post-Read refresh. This is historical-pin diagnostic evidence; qcheck-1/2
  in this worktree report missing references while the new pin is recorded.
