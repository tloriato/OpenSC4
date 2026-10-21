# Lane log

| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| core-commands4 (engine: command system core, `cGZCommandParameterSet` / `RZParseDelimitedText` / `cGZStringCommandGenerator` / `cGZMessage2CommandDispatcher` + message) | `core-commands4` / (engine lane) | **ready** (core only, no plugin/CAPI/CMake/ABI change). New `engine/core/src/commands` (all slots of the four owners from command-system.md §1-§4; command server, message server 2 and converting getters as typed callback traits), `fc4_formats::command_values::PropertyConvert` (exact data-formats §5.1 getters; fc4-formats now depends on fc4-core). 21 new tests (spec §9 vectors 1-16). Gate: cargo 3823 passed, clippy 0, ctest 63/63. No game run. Questions GC5-GC7 | (1) Plugin glue: GZCOM wrappers over these owners (shadow of the game's parameter sets / replace the dispatcher) once a command-server spec exists. (2) Specify `cGZCommandServer` (§5 lists only the used slots). (3) Answers to GC5-GC7 |

## Report

**What changed**
- `engine/core/src/commands/`: `params.rs` (`CommandParameterSet`, 16 slots: QI/AddRef/Release with the §1.2 count rule, count, `SetParameterCount` grow/shrink-highest-first, `operator[]` with the shared writable fallback as `get_or_fallback(_mut)`, Get/Set/Append (highest+1, wrapping), Remove, status, `CompareParameterString` incl. ID −2, `ValidateParameter` kind 0xFF quirk, first/next ID quirks; `make_lower` per graphics-formats §3.11 choice), `split.rs` (`parse_delimited_text`, `line_of` for the length −1 rule), `generator.rs` (`StringCommandGenerator`, 12 slots, capacity loop 1…256 with silent drop, quoting §3.5, command/output to text, ExecuteCommand; `CommandServer` trait for server slots 11/12/20), `dispatcher.rs` (`Message2CommandDispatcher`, 7 slots, Init/Shutdown member rules, ExecuteCommand §4.4 with default-set clearing and status-0 insertion; `CommandMessage` with shared count, counted set references, QI faces and `Create`; `CommandMessageServer` trait for MessageSend).
- `engine/formats/src/command_values.rs`: `PropertyConvert` maps core variants onto exemplar `Property` values for `GetValString` / `GetValSint32`.
- Command server itself not implemented (not specified); no execution is fabricated.

**Measured**: baseline cargo 3803 / clippy 0 / ctest 63/63; final cargo 3823 / clippy 0 / ctest 63/63 (`qbuild all` PASS).

**Choices / questions**: GC5 (−2 walk visits the map, a parameter under 0xFFFFFFFE compared directly; §3.6 walks stop before 0xFFFFFFFF), GC6 (empty remainder after a trailing delimiter adds no word), GC7 (VoidPtr scalar has no text). Spec *[choice]*s kept: SetParameterCount stops when an append does not grow; null output set in ConvertCommandOutputToString is empty; object values have no text (data-formats).

**Commits**: d0ae2867 lane file, 50849566 core module, 5ef1414e converter + questions, then clippy rename and this report.
