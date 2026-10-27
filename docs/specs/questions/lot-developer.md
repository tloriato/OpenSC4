# lot-developer: open questions
<!-- Questions about [lot-developer.md](../lot-developer.md). Append new entries at the END; when one is answered, move it to answered/lot-developer.md. Rules: README.md. -->

<!-- from: core-construction5b (2026-10-23) -->
- **LD39 - §6.7.2/§6.7.15 missing texture-scale properties:** the code-default column is “—” for
  sfPlankTextureScale, sfScaffoldTextureScale and sfGirderTextureScale, while missing properties keep
  the code default. What values survive when these properties are absent? [Core keeps the existing
  `None` tuning values and propagates an explicit NaN into dependent UV/texture-transform fields,
  rather than choosing a numerical scale. Supplied scales, including all shipped values, are implemented.]
