# mysims: open questions
<!-- Questions about [mysims.md](../mysims.md). Append new entries at the END; when one is answered, move it to answered/mysims.md. Rules: README.md. -->

<!-- from: the mysims core audit (cores26, 2026-10-20): mysims.md §11.2's open points, first filed -->
<!-- from: the cores28 lane (2026-10-20): implementing the cSC4MySimDispatch and cSC4MySimPool cores -->
- **MS8 (§1.1, §4.7):** the job-search pathfinder's exact setup — mysims.md says "destination mask `0x1CF`" but
  `0x1CF` is a travel-type mask (Walk/Car/Bus/PassengerTrain/Subway/ElevatedTrain/Monorail) in traffic-simulator.md,
  not a destination bitmask; the core uses it as the travel-type mask with filter 3, flags 1|2, density 0. Which is
  it? [inferred: 0x1CF is the travel-type mask]
- **MS9 (§4.7):** does the dispatch re-run the pathfinder once per candidate lot until one succeeds, or a single
  FindPath with filter 0 (nearest standard destination) that picks the lot itself? [inferred: single run, filter 3]
- **MS10 (§1.1, cSC4MySimPool):** the Sim-template record layout — where templates come from (installation files?
  which paths?), their fields (name/sign/bio/portrait?) and what `Import`/`FindTheSimsInstallation` actually read.
  Core models only name/portrait/bio fields. [inferred]
