# Spec: Graphics Rules (`cSC4ConfigRuleManager`, `cSC4RuleParser`)

Status: **behaviour spec**, clean-room input. This document describes *behaviour*, not code. It was written by the
analysis team from the Windows 1.1.641 executable (read in the instruction bytes and in a Ghidra 12.1.4 project of
the Steam build, whose code equals the GOG build's; evidence in `docs/lanes/re-dates.md`). Implementers must work
from this spec, the public LGPL gzcom-dll SDK headers and general knowledge only. They must **not** consult decompiled
or disassembled code (see [CONTRIBUTING.md](../../CONTRIBUTING.md)).

Normative words: **must**, **should**, **may**. Tagged notes: *[compat]* is required for compatibility with the
original game and its plugins; *[choice]* is a behaviour we chose where the original was arbitrary or undefined;
*[inferred]* is consistent with all evidence but not proven; *[quirk]* is surprising original behaviour that is
kept on purpose; *[compat-limit]* depends on something FreeCity4 cannot reproduce, so only the rule is matched.

Related specs: [render-properties.md](render-properties.md) (the properties the rules set),
[command-line.md](command-line.md) (the `-writeLog` switch and the callers of §6), [INVENTORY-M6.md](INVENTORY-M6.md)
(F1, "Graphics rules").

## 0. Summary

At start-up the config rule manager measures the machine (CPU, memory, screen, video card and driver, sound), reads
`Graphics Rules.sgr` (which includes `Video Cards.sgr`) and an optional user `Graphics Options.sgr`, and evaluates
the rules in them: matching rules set render properties and recommend an option for each **option group** (the
graphics settings of the options dialog, such as Shadows or Texture Quality). Choosing an option in the dialog
applies that option's property set. Everything is logged to `<computer name>-config-log.txt`.

## 1. The manager *[compat]*

| Item | Value |
|---|---|
| COM class | `0x0A1D8285` (`0x007CE130`); `GZCOM_CreatecSC4ConfigRuleManager` (`0x007D1030`); constructor `0x007D0CC0` |
| `QueryInterface` (`0x007CE140`) | `0x0A1D8284`, else the `cRZUnknown` base |
| Vtable `0x00ABB34C` | 3 `Init`, 4 `SetOptionsFromConfiguration`, 5 `LogConfigurationToFile`, 6 `ApplyOptionGroupSetting`, 7 `GetOptionGroupAutoSetting`; the `+4` interface's slot 3 (`0x007D0DB0`) is the deleting destructor |
| `+0x0C` | the rules (0x7C bytes each, in file order) |
| `+0x18` | the option groups (16 bytes each: the recommended option, then the options, each a property set of 0x24 bytes) |
| `+0x24`, `+0x28`, `+0x34` | the flags, the string variables (0x14 bytes each), the numeric variables (floats) |
| `+0x40` | the render properties (counted) |
| `+0x6C` | the log file |

A **property set** is three lists: (boolean index, value), (integer index, value), (float index, value). Applying it
(`0x007CE1F0`) calls the render properties' `SetBool`, `SetInt` and `SetFloat` for each, in that order.

## 2. Variables (`GatherInformation`, `0x007CF780`) *[compat]*

**Flags** (bit = index), named for `has` / `hasNo`:

| Bit | Name | Source |
|---|---|---|
| 0 | `hardwareDriver` | the driver is a hardware driver and `kForceSoftwareDriver` is false |
| 1 | `DXT` | the driver supports compressed textures *[inferred]* |
| 2 | `fullscreen` | full-screen mode *[inferred]* |
| 3 | `MMX` | CPU feature bit 23 |
| 4 | `FPU` | CPU feature bit 0 |
| 5 | `backingStore` | a backing store exists (global `0x00B4E1E4` non-null, `0x007C2790`) |
| 6 | `forcedFSAA` | `0x008D2320` on the driver *[inferred: anti-aliasing forced by the driver]* |

**Numbers** (`atLeast` / `atMost`), as floats: 0 `cpuSpeed` (MHz, `0x00891552`), 1 `memory`, 2 `freeMemory`,
3 `screenWidth`, 4 `screenHeight`, 5 `screenBPP`, 6 `videoMemory` and 7 `textureMemory` (MB: the graphic system's
memory figures shifted right by 20), 8 `textureStages`, 9 `bogoFPS`, 10 `bogoMPPS` *[inferred: 9 and 10 are not
measured and stay 0]*.

**Strings** (`stringMatch` / `stringNotMatch`): 0 `userName`, 1 `appName`, 2 `version`, 3 `osVersion`,
4 `computerName`, 5 `CPU`, 6 `sglDriverName`, 7 `sglDriverVersion`, 8 `driverName`, 9 `driverVersion`,
10 `cardName`, 11 `cardVersion`, 12 `cardIdentity`, 13 `soundDriverName`, 14 `soundCardName`, 15 `buildType`.

- 6–11 are the successive tokens of the driver's description string.
- 13 and 14 come from the sound system (service `0xE9D27715`), or `Unknown` without one.
- 15 is `Release`.

## 3. Reading the rules (`ParseConfigFile`, `0x007CFE10`) *[compat]*

1. A rule parser (`cSC4RuleParser`) is made. When `cardVersion` is longer than 26 characters, its 4 characters at
   offset 8 and at offset 22, read as hexadecimal, are the card's vendor and device IDs.
2. The files `Graphics Rules.sgr` (the data directory) and `Graphics Options.sgr` (the user data directory) are
   given to the parser, with the symbol `prefsFirstTime` set to true or false from an application flag
   (`+0xF0C` of application slot 38) *[inferred: the first run]*. `cardIdentity` starts as a copy of `cardName`.
3. On success, the rules and option groups move into the manager, and when the card was found in a `vendor` block
   (§4.4), `cardIdentity` := `"<vendor>, <card>"`. The result is the parse's success.

`Init(properties)` (slot 3, `0x007D1890`) holds the render properties, gathers the variables and parses; when the
parse fails it installs the fallback rules (§7).

## 4. The language *[compat]*

The files use the game's command parser (`nSCRes`): one command per line with its arguments, `#` comments, `set
name value` symbols used as `$name`, blocks closed by `end`, and `sinclude "file"` (the shipped rules end with
`sinclude "Video Cards.sgr"`) *[inferred: these generic commands belong to the shared parser]*. Arguments are
words, numbers or quoted strings; a switch is a `-word` argument. Names of flags and variables are matched against
the tables of §2 (`nSCRes::ParseEnum`); a property name is resolved with the render properties' find, booleans first,
then integers, then floats (`AddOptionToSet`, `0x007E4050`); an unknown one is the parse error
`Unknown render property`.

### 4.1 Option groups

```
optionGroup <group number>
   option <option number>
      property <name> <value>
      ...
   ...
end
```

Group and option numbers grow the tables as needed; each option collects a property set.

### 4.2 Rules

```
rule ["name"] [-partial] [-any]        or        partialRule ["name"] [-any]
   has <flag>              hasNo <flag>
   atLeast <number> <v>    atMost <number> <v>
   stringMatch <string> <pattern>      stringNotMatch <string> <pattern>
   property <name> <value>
   option <group> <option>
   <nested rules>
end
```

- `partialRule` is `rule -partial`. An unnamed rule takes the previous rule's name followed by `>`.
- Each rule records its **depth** (how many rules enclose it).
- `has f` requires flag f set, `hasNo f` requires it clear (both set f in the rule's mask); `atLeast n v` requires
  number n ≥ v, `atMost n v` requires n ≤ v; `stringMatch s p` requires string s to match the wildcard pattern p
  (`WildcardMatch`, case-insensitive) *[inferred: the case flag]*, `stringNotMatch` the opposite.
- Without `-any` **all** the conditions must hold (`0x007CE900`); with `-any` **one** is enough (`0x007CEA80`).
  A rule without conditions always matches, but a `-any` rule without conditions never does *[quirk]*.

### 4.3 Values

Booleans are parsed by the parser's boolean reader (`true` / `false`, as the shipped file writes them), integers
and floats by its number readers.

### 4.4 Video cards

```
vendor "name" <id> [<id> ...]
   card <device id> "card name"
end
```

A `vendor` block whose IDs include the detected vendor ID enables its `card` lines; a `card` whose device ID equals
the detected one marks the card found and records its name (`0x007E4730`, `0x007E4800`).

## 5. Evaluation (`ApplyRules`, `0x007D10B0`) *[compat]*

The rules are walked in file order with a stack of depths:

1. When the stack is empty or the rule is deeper than the top, it is tested. If it fails, it and every following
   deeper rule (its children) are skipped. If it matches:
   - log `applying <name>`;
   - apply its property set;
   - for each `option g o`, the recommended option of group g := o (the table grows as needed), logging
     `  option g = o`;
   - if the render property `kForceSoftwareDriver` is now true, clear the `hardwareDriver` flag;
   - unless the rule is partial, push its depth.
2. Otherwise (the rule is not deeper than the top), every following rule at least as deep as the top is skipped and
   the top is popped.

So among siblings the first matching non-partial rule wins and stops the rest (an if / else-if chain); partial
rules never stop their siblings; children are tested only under a matching parent.

## 6. Applying and logging

- `SetOptionsFromConfiguration` (slot 4, `0x007D18E0`): logging is on unless the command line has
  `-writeLog:off` (case-insensitive). With logging, the log file `<app directory><computerName>-config-log.txt` is
  opened and the variables are written (`LogConfigInfo` `0x007CEF40`: `name = value` with `%s`, `%d` or `%g`); then
  `ApplyRules` (log headings `Rules`, `-----`); then, with logging, the option summary (`LogOptions` `0x007CE260`,
  lines such as `Texture Quality      current: %3d recommended: %3d`) and the file is closed.
- `ApplyOptionGroupSetting(g, o)` (slot 6, `0x007CE870`): applies option o's property set of group g; out-of-range
  numbers do nothing.
- `GetOptionGroupAutoSetting(g)` (slot 7, `0x007CE8D0`): the recommended option of group g, 0 when g is out of
  range.

## 7. Fallback rules (`SetFallbackRules`, `0x007D13C0`)

When the files cannot be parsed, `Using fallback rules.` is logged and three rules are built in:

| Rule | Properties |
|---|---|
| `standard software` | `RenderCloudsAndFog`, `RenderShadows`, `RenderDecals`, `UseSecondStage` false |
| `standard hardware` | `RenderCloudsAndFog`, `RenderShadows`, `RenderDecals` true, `UseSecondStage` false |
| `two stages` | nested, `atLeast textureStages 2` *[inferred: the value]*: `UseSecondStage` true |

*[inferred: the software and hardware rules test the `hardwareDriver` flag]*.

## 8. Users

`cSC4App::PostFrameWorkInit` and `DoPreferencesChanged` (command-line.md) call `SetOptionsFromConfiguration`; the
graphics options dialog uses slots 6 and 7.

## 9. Quirks

1. Partial rules apply and never stop their siblings; a plain rule stops its later siblings even if they would match
   better (§5).
2. Card IDs are read at fixed offsets of `cardVersion` (§3).
3. An unnamed rule's name is its predecessor's name with `>` appended, whatever the nesting (§4.2).
4. A `-any` rule with no conditions never matches (§4.2).

## 10. Open questions

See [questions/graphics-rules.md](questions/graphics-rules.md) GR1–GR3: the exact sources of `DXT`, `fullscreen`,
`forcedFSAA` and the `bogo` numbers, the case rule of `stringMatch`, and the conditions of the fallback rules.

## 11. Conformance tests

1. **First match:** siblings `rule "a"` (`atLeast memory 512`) and `rule "b"` (no conditions) with memory 1024:
   only "a" is applied; with memory 256 only "b".
2. **Partial:** two sibling `partialRule`s without conditions are both applied.
3. **Any:** `rule -any` with `atLeast memory 4096` and `has MMX` matches on an MMX machine with 512 MB.
4. **Children:** a child of a failing rule is never tested; `rule -any` with only a `property` line is never
   applied.
5. **Options:** `option 2 3` in a matching rule makes `GetOptionGroupAutoSetting(2)` = 3; group 99 gives 0.
6. **Unknown property:** `property noSuchThing 1` is a parse error.
7. **Software:** a matching rule that sets `forceSoftwareDriver true` clears `hardwareDriver` for the rules after it.
8. **Log name:** on a machine named `ALPHA` the log is `ALPHA-config-log.txt` in the application directory.
