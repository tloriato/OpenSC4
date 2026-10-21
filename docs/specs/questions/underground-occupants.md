# underground-occupants: open questions
<!-- Questions about [underground-occupants.md](../underground-occupants.md). Append new entries at the END; when one is answered, move it to answered/underground-occupants.md. Rules: README.md. -->

<!-- from: re-underground (2026-10-19) -->
- **UG1 - §0, §2.3:** the two base classes, `cSC4UndergroundOccupant` (`QueryInterface` `0x0065A080`, `Write`
  `0x0065B140`, `Read` `0x0065E510`, `Draw` `0x0065B290`) and `cSC4UndergroundConnectionOccupant` (`0x00654030` …):
  their save records, drawing and model building, and how they use the static model descriptors and the diagonal
  ratio tables. The pipe and subway records cannot be written without them.

<!-- from: re-underground (2026-10-19) -->
- **UG2 - §2.2:** what network flag bits `0x02000000` and `0x04000000` (`+0x13C`) mean; either one makes an
  occupant's sort value its piece id instead of its class ID.

<!-- from: re-underground (2026-10-19) -->
- **UG3 - §3.3:** which subway pieces are `0x300` and `0x4B00` (their tunnel floor comes from the second vertex
  list), and what the zipper piece type (main slot 99) being 0 means for them.

<!-- from: re-underground (2026-10-19) -->
- **UG4 - §4:** the values `0x2A183167` (pipe connection) and `0x8A13918A` (subway connection) that `Init` stores at
  `+0x24` before the base `Init`.
