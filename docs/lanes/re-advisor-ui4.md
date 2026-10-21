| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| re-advisor-ui4: advisors AD4 (window direct-call ABI), AD5 (9 UI Lua entries), AD6 (camera_jump / camera_zoom) | `re-advisor-ui4` / (RE lane) | ready, docs only, no ABI change; AD4 MATCHES, AD5 and AD6 DIFFERS, all answered and archived | 1. glue lane: use advisors.md §12.1.1 "Direct-call ABI", §8.3.1 and §8.3.2 for the replacement's window route and the 11 UI/camera Lua entries. |

## Report

**Round 1 (AD4, AD5; commits 197b97a4..9876c28d).** advisors.md §12.1.1 direct-call ABI of the five window bodies;
§8.3 UI row corrected to the real 9 entries; §8.3.1 written; §14.2 reader list; AD6 filed.

**Round 2 (AD6).**
- advisors.md §8.3.2 (new): `camera_jump` `0x40AF90` and `camera_zoom` `0x40B1F0` are records 5 and 6 of the
  simulation collector `0x40B8D0` (isGlobal false, `game.*`); §8.3 Camera row now points there.
- Both: cdecl lua_CFunction, return 0, no pushes, read index -1 only; host = 4 bytes before the state, QI
  `0xEA10C4A2`; unconditional `0x00B207C0` slot 20 call (discarded, no null test).
- camera_jump: GetTop>0, IsUserData, ToUserData, system slot 22 tag == Tag(-1), renderer `0x00B43DD0` and its
  slot 16 camera control non-null; subject (ToUserTag, not null-tested) with location -> (16*x, GetAltitude, 16*z)
  via terrain `0x00B43CF4` slots 6/4/5/13; else its object QI'd as occupant `0x26BB767E` -> GetPosition; then
  `0x7CD810` (rejects NaN or |c| > 100000). Failures: gAdviceMessageBox `0x4179A0` with fixed text/caption.
- camera_zoom: GetTop>0, IsNumber, renderer+camera control -> SetZoom `0x7CDC50`(f32(ToNumber)); no range check in
  the callback (quirk: the "valid range" box only shows for missing/non-numeric args or no view).
- Under a replacement: original addresses need a system with slots 20 and 22 in `0x00B207C0`.
- AD6 moved to answered/advisors.md (DIFFERS); QUESTIONS.md regenerated.

**Evidence.** re_query.py asm/func (export), `bin/SimCity4.exe` constants and vtable `0x00A81090` read with
build-runs/rd.py (not committed). REA not used.

**Open.** None filed. The meaning of window-manager slot 44 args `(text, caption, 0, 1, 0)` is *[inferred]*.
