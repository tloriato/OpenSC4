# network-manager: open questions
<!-- Questions about [network-manager.md](../network-manager.md). Append new entries at the END; when one is answered, move it to answered/network-manager.md. Rules: README.md. -->


<!-- from: verify-netmgrfix4 (2026-10-21) -->
- **NM12 - section 8.5 step 3.3 and section 17 (the configuration draw as an external input):** the netmgr shadow
  must replay the original's draws to compare component 11 without resyncing, but the spec gives no public point at
  which that input can be captured before our step. The draw is `RandomUint32Uniform(count)` on the file-local
  `cRZRandomSuper` `0x00B44870`, seeded at static initialisation (before any plugin loads) from `cRZTime()` XOR
  `0x8C09C379` (foundations section 6.3), and its stream is advanced in between by the section 8.6 prop and flora
  object draws. No cISC4NetworkLotManager slot (section 11.2), message or other interface exposes the generator's
  seed, state or draw count, and foundations section 6.1 gives no in-memory layout of `cRZRandomSuper` (offset of
  the 624 state words and of the index) that a plugin may read. Questions: (a) is there a documented capture point
  (an interface, a message, or a specified object layout at `0x00B44870` plus a guarantee that reading it from the
  plugin at a given boundary, e.g. before the `0x8A9C4C6D`/lot-state message that triggers `ProcessUpdateList`, is
  sanctioned) from which the shadow can take the generator's state and seed our `NetworkLotRandom` with it? (b) If
  not, is the configuration chosen among `count >= 2` compatible candidates officially outside exact comparison,
  so that the shadow's verdict is "exact except section 17 draws" and a resync after a draw-only difference is the
  specified behaviour? [current: no capture; the shadow counts such cells as draws and restarts the lot core from
  the original's record (4 resyncs in vnmf4-turbo2032-a, 2 in vnmf4-normal-a), so it is not zero-resync exact]
