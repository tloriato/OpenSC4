# disasters: open questions
<!-- Questions about [disasters.md](../disasters.md). Append new entries at the END; when one is answered, move it to answered/disasters.md. Rules: README.md. -->

## From the disaster layer glue (filed by the coordinator from the `glue13` lane's report, 2026-10-16; merged `e7386b0`)

glue13 wired `fc4_disasters_host` to the game (`FC4DisasterHost`) and added `disasters_shadow = 1` (`FC4DisasterShadow`):
the original layer runs and a mirror core follows it (ABI 67, taken as 61 on the lane). The lane's final run
dis-final-11 was clean: fire 211/263 lines exact, riot 2/354, quiet 0/1, and every other difference is tagged with one
of the questions below in the log. The lane's Q36 (the riot's AttemptDisasterStart) is answered by disasters.md §6.2
(it always returns null; the shadow takes the instance from the first riot RegisterDisasterLocation). The lane's own
numbers (its Q26-Q42) are given in parentheses; the current choice is in brackets.

<!-- from: From the disaster layer glue (2026-10-16) -->
- **DS28:** (Q29) toxic spill, terrain, pipe burst and volcano are not mirrored: no start path was exercised. [fire and
  riot only]

<!-- from: verify-disasters4 (2026-10-21) -->
- **DS41:** §5.4 step 4 says an occupant rectangle with no overlap with the city "gives an empty rectangle", then
  averages it with `GetAverageValueInCellRect(x, z, w, h)` (`sum / (w × h)`). With w × h = 0 the division (and
  simgrid §4.1 step 1's one-tract test with `x + w − 1 < x`) is not specified: what coverage byte does the original's
  step 4 (and §5.6's spread) read for such an occupant, and does it refuse? [the plugin reads 0 coverage, so it does
  not refuse at step 4; unreachable for an occupant inside the city]

<!-- from: glue-disasters-house11 (2026-10-24) -->
- **DS47 - sections 7-10, extending the exact same-process shadow:** what supported observation contract supplies the original pipe leak's sampled burst time/effect-created result, toxic occupant amount/hosed state, and quake/volcano deformation completion to a read-only mirror? Section 7 explicitly permits a different fault line, section 8 leaves effect creation without a view unresolved, section 9 describes amounts without a typed getter contract, and section 10 delegates lava to terrain. Please specify typed interfaces/slots and return/ownership contracts (or explicitly define which independent bookkeeping outputs constitute exactness for these short-form disasters). [The opt-in house schedule exercises fire and riot only; no guessed signatures, forced completion, native state copied into the core, or verification claim for these other types. DS28 remains open.]

## DSEX33-DS01 — period-start instance identity and native safety consumer

Lane `disasters-exact33`, 2026-10-28. Sections 2.4 and 3.2 specify the period-start
message's counted instance and generic slots, while advisors.md 7.6 obtains the
fire cause through the instance. Please specify the exact interface pointer
identity stored in the message, any QI/adjustment made by the native safety
advisor, and the supported call contract by which it reads the cause. Does this
consumer require an additional instance IID or a concrete sub-object layout?

Evidence: disasters-only `paradis-dreyse`, qcheck-15.log, result
`C:/Users/tlori/FreeCity4/build-runs/golden-runs/compare-20261028-041512-83113dc4/result`.
The run is clean/ref_warnings=0 but not exact. Class 6A26D049 has 22 reference
subjects versus 21 candidate subjects. Payload-multiset differences are reference
locations (167,6) and (9,8), both data words zero, versus candidate (67,218),
data1=0 and data2=22937454 hex; all three have null object references. These
subjects have not been attributed to a particular event. Copying locations into
the public layer before forwarding core posts leaves this difference unchanged.
The adapter currently stores its generic instance view in the counted unknown;
no guessed IID, native layout, or consumer adjustment has been added.

### DSEX33-DS01 follow-up: generic +0x38 experiment (round 5)

An isolated experiment reserved a cause word at counted generic-instance
pointer +0x38 and synchronized it after every core call, including SetCause.
The adapter tests passed, but disasters-only Dreyse qcheck-19.log still has
17 different records and its first shifted ID is still 22/class 6A3848DB.
Result: `C:/Users/tlori/FreeCity4/build-runs/golden-runs/compare-20261028-044515-bef8b707/result`.
The speculative storage and its tests were removed. This does not settle the
native consumer's actual pointer origin or prove that the cause query caused
the missing advice subject. The script reached fire (165,5) and riot (9,8);
both managers reported one active instance.

### DSEX33-DS01 follow-up: original period-start fields (round 6)

Original-only `disasters-exact33-r5-shadow-dreyse-2` completed cleanly with
`ref_warnings=0`. Its period-start messages contain count/cell/type
`1/(167,6)/621CDA33` and `2/(9,8)/CA380B06` (log lines 4783-4784).
Both shadow summaries report zero mismatches. This confirms that the two
reference subject cells match original period-start messages; it does not yet
attribute the missing candidate subject to a cause query or pointer adjustment.
The replacement now logs instance QI requests/results and GetCause callers to
distinguish those possibilities without implementing an unspecified layout.

### DSEX33-DS01 follow-up: rejected advisor interface (round 7)

After merging the authorized reference pin, disasters-only Dreyse still has
17 different records, with the first object-ID shift at 22/class 6A3848DB.
Result: `C:/Users/tlori/FreeCity4/build-runs/golden-runs/compare-20261028-061122-cfe7bdf6/result`.
The run is clean with `ref_warnings=0`. Its game.log lines 2915-2947 show
the specified period-start count/cell/type for fire and riot. Lines 2973-2974
then show the native consumer at caller 00417163 requesting IID **22223BA4**
from both counted instances; both requests are rejected. There are no native
GetID/GetCause calls in this trace. The only logged GetID callers belong to
our period-post construction/logging.

No contract for this IID was found in disasters.md, advisors.md, foundations.md,
docs/abi, or the SDK headers. Please specify its identity, supported instance
types, returned pointer adjustment and slots, including the fire-cause read's
pointer origin. The trace now localizes the missing interface; the adapter
still rejects it rather than guessing a native layout or vtable.

## DSEX33-DS02 - volcano lava creation through a game interface

Lane `disasters-exact33`, 2026-10-28. Disasters section 10 and terrain section
11 specify the flow, rates, priority and shutdown, but not a game-interface
operation by which a replacement disaster instance obtains the terrain lava
object. The public `cISTETerrain` header exposes terrain operations, including
the volcano brush, but no lava creation operation. ABI function rows identify
concrete `cSTEVolcanoLava` methods; they do not identify a service/factory
interface for creating and owning one.

Please specify the game-interface creation/query contract, any required
initialization, ownership, tick registration/removal, and the flow/rate/end
operations available to a disaster instance. If no such interface exists,
please state the required replacement boundary rather than requiring a
concrete native-object allocation or direct constructor call.

Evidence: qcheck-27.log, volcano result
`C:/Users/tlori/FreeCity4/build-runs/golden-runs/compare-20261028-072418-751489ef/result`.
Delivery order matches all 31,531 logged deliveries, but the first shared-rand
difference is 2026-01-28 delivery 1, prop-developer target `00AB29E8`, message
`6695681A`: reference `8A369225`, replacement `389BE6C5`. Its game.log reports
`volcano_start_lava`, `volcano_lava_rates` and `volcano_end_lava` unwired. The
deformation effect is now created and retained through the effects interface.
This identifies an unimplemented contract, not proof that lava causes the
first rand difference; the missing advice IID in DSEX33-DS01 remains separate.
