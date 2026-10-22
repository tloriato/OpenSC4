# watercraft: open questions
<!-- Questions about [watercraft.md](../watercraft.md). Append new entries at the END; when one is answered, move it to answered/watercraft.md. Rules: README.md. -->

<!-- from: re-boatsped (2026-10-21) -->
- **WC1 - §1, §5.1:** who sets the byte `+0x2CC` (count the idle budget in every animation tick); no watercraft
  function writes it except `Init`, which clears it. [as read]

<!-- from: re-boatsped (2026-10-21) -->
- **WC2 - §5.1:** what the caller of the boat's animation tick does when it returns false (a boat still for more
  than 5 s outside states 7, 16, 75–78), and the meaning of drive-parameter entries 4, 11–14, 18, 22–24, 26, 30, 31
  and 34. [as read]
