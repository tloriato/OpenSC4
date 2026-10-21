# output-streams: open questions
<!-- Questions about [output-streams.md](../output-streams.md). Append new entries at the END; when one is answered, move it to answered/output-streams.md. Rules: README.md. -->

<!-- from: re-ostreams (2026-10-19) -->
- **OS1 - §4.3:** what does the GZ log write to its stream (`SC4CreateGZLog` `0x005C6E80`, the stream handed to the
  framework log)? Only strings, or also numbers, which would come out as raw big-endian bytes? And which file name
  does `cSC4App::PreFrameWorkInit` pass?

<!-- from: re-ostreams (2026-10-19) -->
- **OS2 - §2.2, §4.1:** only for a text-mode file stream, which the game never creates: does the STLport file buffer
  translate `\n` to CR LF in text mode, and which decimal point is in effect when such a stream writes a float (the C
  runtime locale after `0x00910014` and `SetCurrentLanguage`)?
