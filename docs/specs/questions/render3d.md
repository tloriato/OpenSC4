# render3d: open questions
<!-- Questions about [render3d.md](../render3d.md). Append new entries at the END; when one is answered, move it to answered/render3d.md. Rules: README.md. -->

- **RND1 - section 2:** the name of cIGZOStream/cIGZIStream slot 8 (`+0x20`) used for the viewport fields and the
  camera mode in `cSC43DRender::Write`/`Read` (`0x007C310B`, `0x007C31AD`): `PutSint32`/`GetSint32` is inferred from
  slot 9 being `PutUint32`. Settle from the stream vtable row in docs/abi/interfaces.tsv.
- **RND2 - section 1.3:** byte `+0x60` defaults to 1 and, per view3d.md section 4.1 step 12, suppresses `ClearViewport`
  in `Draw`; `GetBackgroundPicture` returns it as the "enabled" out flag. Which writer clears it (candidate:
  `SetBackgroundPicture` `0x007C2610`/`0x007C26D0`)? Until settled a replacement keeps default 1.
- **RND4 - section 10.1:** the `StartScreenShake` overload (slot 43, `0x007C6BC0`) never writes `P+0x04`, the
  fade-out length copied to node `+0x20` (stack slot frame+4 is not stored between `0x007C6BC0` and the slot-42 call at
  `0x007C6D4C`), so `StopScreenShake` on such a shake sees stack garbage. The spec has a replacement store 0.0
  (stop at once). Confirm by a game run whether stopping an overload-started shake ends it immediately or fades it.

<!-- from: core-scene-renderer4 (2026-10-23) -->
- **RND5 - sections 1,4,6,9:** which allocation failure, refcount/area/deadline overflow and reentry domains have defined owner results? Is any recovery defined when existing portable view utilities cannot rebuild a frustum? [Checked portable errors can retain the published prefix effects; no clean false or native-equivalence claim. Last Release stops after never-initialized Shutdown at the unspecified destructor; no initialized lifecycle is fabricated.]

<!-- from: core-scene-renderer4 (2026-10-23) -->
- **RND6 - section 10.3:** what are the exact spill/precision points of curve sampling, positional weight, phase multiplication/addition and accumulated sx/sy? [Use only bounded arithmetic with exact representability; arbitrary table interpolation, nonzero repeated scales beyond one factor and nonzero sine entries remain Unsupported. Seed-88 random entries reuse the existing RzRandom; overload fade remains the RND4 0.0 choice.]

<!-- from: core-scene-renderer4 (2026-10-23) -->
- **RND7 - section 5:** when exactly is queued bit 0x10 cleared relative to each kind's dirty requests, and are outside-box tolerance subtractions stored as f32? Which +5 byte is supplied by ChangeModel? [Clear 0x10 after completed dirty requests; pure post-move box-policy helper uses literal f32 subtraction with 0.0001f32. ChangeModel unused byte is typed None; never-read stack words are typed UnspecifiedNeverRead. Full MoveGridElement and UpdateGridElementObjectFlags remain Unsupported.]
