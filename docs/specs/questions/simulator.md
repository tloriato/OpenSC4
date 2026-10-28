# Simulator questions

## SR33-1 — Legacy timer byte count (save-roundtrip33, 2026-10-28)

Simulator §8 and save-records §7.6 call the legacy timer 11 bytes, but their
named fields (`u8 units`, `u64 elapsed`, `u8 running`) total 10 bytes, matching
stream-helpers §2.1. Please confirm that 10 is the intended byte count.
The reader currently follows the explicit field widths and discards 10 bytes.
