# framework-rz-files: answered questions (archive)
<!-- Open questions are in ../framework-rz-files.md. Preserve question IDs and provenance. -->

<!-- from: core-rzfiles4 (2026-10-21) -->
- ~~**RZ6 - §3.5 ReadNumber overflow:** "follows the C runtime": wrap, clamp to LONG_MAX/LONG_MIN, or other?
  The core wraps modulo 2^32. [choice]~~
  **ANSWERED 2026-10-24 (re-rz-ini-number-overflow):** MATCHES — framework-rz-files.md §3.5.1
  establishes modulo-2^32 conversion followed by signed interpretation, without saturation or an
  overflow-based failure. Any accepted digits still yield one scanner assignment and ReadNumber
  returns true; no-digit failures leave the numeric output unchanged. Static analysis of the matching
  executable and 32 isolated native-scanner probes agree, including beyond-64-bit and 1,024-digit
  inputs. The existing provisional wrap policy is a compatibility requirement for this domain.
  The probe executes the embedded scanner; ReadEntry and the running game are outside its scope,
  and the ReadNumber boolean follows the separately verified wrapper check.
