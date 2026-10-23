# Curve entry-view questions

- **CEV-1 — Happiness variant and container lifecycle.** Qualify +0xD8 slot +0x18/+0x1C (0x0066B180/0x0066BD70), helper 0x0066C570 duplicate/key order, helper 0x0066B710 erase/count effects, display 0x0066BB50 and Init 0x0066C5F0. Which exact field order and numeric conversions are owned here?
- **CEV-2 — GeneralResponse value/controller/events.** Qualify +0xD8 slot +0x1C 0x0066B220, Init 0x0066D240, UpdateCurveDisplay 0x0066C180, DoButtonPress 0x0066CE10 and SetCurrentPoint 0x0066C060: copies, point order, control result gates and writeback/notification order remain unclaimed.
- **CEV-3 — Lifecycle and failure.** Qualify adjusted interface thunks, owner destructors, held references and same/null/replacement behavior. Do not infer rollback, allocation-failure recovery or reentry safety from names.
- **CEV-4 — Numeric environment.** For 0x0066CDB0 and later conversion bodies, establish exact x87 operation/store schedules and the dependence on PC/RC, NaN payloads, exceptional operands and tie rounding. Current finite add-point gates are known; masked/unmasked exception outcomes are not.
