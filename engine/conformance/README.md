<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# Conformance vectors

Language-neutral test scenarios for the simulator (`docs/specs/simulator.md`), the shared foundations
(`docs/specs/foundations.md`) and the budget (`docs/specs/budget.md`). Each file in
[`vectors/`](vectors/) is a JSON document that describes a starting configuration, a script of operations, and the
expected observable results: agent deliveries, broadcast posts, reference-count calls, return values, and state
snapshots.

The vectors are the **migration contract**. Any implementation of these subsystems, in any language, must pass every
vector unchanged. The Rust runner in this crate (`cargo test -p fc4-conformance`) is one implementation of this
document. When the runner and this README disagree, fix the runner.

Vectors are derived from the spec, never from the original program. Each vector names the spec sections it covers
in `spec`. Behaviours that the spec leaves open and that a vector pins down are marked `[choice]` in the vector's
`description`.

## File layout

```json
{
  "format": 1,
  "name": "slow_day_agent",
  "spec": ["5.2", "10.3"],
  "description": "free text",
  "clock": {"start_ms": 0, "auto_step_ms": 0},
  "agents": {"D": {"finish_after": 3}},
  "steps": [ {"op": "add_agent", "agent": "D", "level": 4}, ... ]
}
```

| Key | Meaning |
|---|---|
| `format` | Must be `1`. Runners reject other values. |
| `name`, `spec`, `description` | Documentation only. |
| `clock` | Fake millisecond clock. `start_ms` (default 0). `auto_step_ms` (default 0) is added after **every** clock read. |
| `agents` | Scripted agent behaviours, by label (see below). An agent used without an entry gets the defaults. |
| `steps` | Operations, executed in order. A failing step fails the vector. |

Unknown keys inside an operation are ignored, so `note` can be used for comments. Unknown `op` names, unknown state
fields and unknown event or filter keys are errors.

### Clock rules

The simulator reads time only through the injected clock. With `auto_step_ms: 0` time moves only through
`advance`/`set_clock` and through agent `cost_ms`. Vectors that check exact timings use a frozen clock plus agent
costs, so their results do not depend on how often an implementation reads the clock. Vectors with
`auto_step_ms > 0` check bounds only.

A frozen clock combined with `on_tick` needs some agent with `cost_ms > 0`. Otherwise the frame loop never reaches
its budget. Runners should abort such runaway loops. The Rust runner stops after 50 million clock reads in one step.

The slice timer is restarted only by `on_tick`. Before the first tick it counts from clock value 0, which is also
the clock value when the simulator is created in every current vector (`start_ms` 0). So, outside `on_tick` and
with the clock 15 ms or more past the last slice start, every scheduler step is followed by "budget exhausted". In
practice, a direct `simulate_agents` call then makes exactly one delivery. With the clock still at the slice start,
the budget is never exhausted and `simulate_agents` walks every level. (§3 lists the timers but not their initial
value; this is a [choice].)

### Agents

An agent is identified by a **label** (a string). Each label maps to one opaque handle. Registering the same label
at several levels registers the same target several times.

| Behaviour key | Default | Meaning |
|---|---|---|
| `finish_after` | 1 | Calls needed per interval. The agent returns "finished" on every N-th consecutive call, and the counter resets after it finishes. `0` = never finishes. |
| `cost_ms` | 0 | Added to the fake clock on every call, after the `deliver` event is recorded. |
| `on_call` | `{}` | `{"<n>": [op, ...]}`: operations executed **inside** the n-th call (1-based, counted over the agent's lifetime), before it returns. Used to test re-entrancy. |

### Events

The runner records, in order:

| `event` | Fields |
|---|---|
| `deliver` | `agent`, `type`, `data1`, `data2`, `data3`, `t` (clock when the call started) |
| `post` | `type`, `data1`, `data2`, `data3`, `t` |
| `add_ref` | `agent` |
| `release` | `agent` |
| `publish` | `cycles_to_ms`, `ms_to_cycles` (limit-timer coefficients, §5.5; compared with relative tolerance 1e-6) |

`type` may be written as a number, as a `"0x…"` string, or as one of these names:

| Name | ID | Name | ID |
|---|---|---|---|
| `SimBegin` | 0x66956810 | `SimSpecificDay` | 0x66956818 |
| `SimEmergency` | 0x66956811 | `SimSpecificTime` | 0x66956819 |
| `SimNewMinute` | 0x66956812 | `SimIdle` | 0x6695681A |
| `SimNewHour` | 0x66956813 | `PauseChanged` | 0xAA7FB7E0 |
| `SimNewDay` | 0x66956814 | `HiddenPauseChanged` | 0x4A7FB7E2 |
| `SimNewWeek` | 0x66956815 | `EmergencyPauseChanged` | 0x4A7FB807 |
| `SimNewMonth` | 0x66956816 | `SpeedChanged` | 0x4A7FB7E9 |
| `SimNewYear` | 0x66956817 | | |

A **filter** is an object with any of `event`, `agent`, `type`, or any other event key (as in `expect_events`). An event matches when every given key is equal.

### Operations

Every operation that calls a simulator method accepts an optional `"expect": <bool>`, which is compared against
the method's return value.

**Lifecycle and frame**

| op | Arguments | Calls |
|---|---|---|
| `init` | `tuning?`: `{"ms_per_day": [..], "animation_dilation": [..]}` | `Init` (core part: §9 steps 1, 2, 5–7) |
| `shutdown` | | `Shutdown` (core part: §9 steps 2, 3, 7) |
| `tick` | `count` (default 1) | `OnTick` |
| `simulate_time_passage` / `simulate_agents` / `simulate_idle_time` | | §5.2 / §5.3 / §5.4 |
| `begin_new_interval` | `level` | §5.3 step 1 for one level |
| `repeat` | `count`, `steps` | runs `steps` `count` times |

**Clock**

| op | Arguments | Effect |
|---|---|---|
| `advance` | `ms` | clock += ms |
| `set_clock` | `now_ms?`, `auto_step_ms?` | sets the clock and/or its auto-step |

**Speed, pause, misc** (§6)

`pause`, `resume`, `hidden_pause`, `hidden_resume`, `emergency_pause`, `emergency_resume`; `set_speed {speed}`;
`set_sim_time {time}`; `set_max_ms_per_tick {ms}`; `set_city_established {value}`.

**Limit-timer calibration** (§5.5): the runner provides a fake wall clock (`qpc`, ticks) and a fake TSC (`tsc`,
cycles). Both start at 0 and move only through these ops. Every publish is recorded as a `publish` event.

| op | Arguments | Effect |
|---|---|---|
| `set_timer_source` | `qpc_frequency`, `initial_ms_per_cycle` | installs the fake source/sink (`expect` = return value) |
| `set_timers` / `advance_timers` | `qpc?`, `tsc?` | set, or add to, the fake counters. Negative deltas wrap. |
| `calibrate_tick` | | one calibration step, without the rest of OnTick |
| `expect_calibration` | `none?`, `cycles_to_ms?`, `ms_to_cycles?`, `samples?` | the current coefficients (relative tolerance 1e-6) and the number of samples folded in |

**Notifications** (§9): `post_city_init` delivers notification `0x26D31EC1` (PostCityInit), which calls
`HiddenResume`. Unsubscribing is the host's job and is not modelled.

**Agents** (§4)

| op | Arguments |
|---|---|
| `add_agent` | `agent`, `level`, `name?` (default: the label), `flags?` (default 3) |
| `remove_agent` | `agent`, `level?`. Without `level` this is `RemoveAgent(target)` across all levels. |
| `remove_all_agents` | `level?` |
| `set_agent_enabled` | `name`, `enabled` |

**Cheats** (§9): `cheat {id, text}`. `id` is a number, a hex string, `"FPS"` or `"SimDate"`. `text` is the whole
typed line, for example `"SimDate 3 15 2001"`.

**Persistence** (§8)

| op | Arguments | Effect |
|---|---|---|
| `save` | `slot` | stores the bytes of `Write` under a name |
| `load` | one of `slot`, `hex`, or `record`; `truncate?` (n bytes); `expect?`: `"ok"` (default), `"unsupported_version"` or `"truncated"` | `Read` |
| `expect_save` | one of `slot`, `hex`, or `record` | the bytes `Write` produces now must be equal |

A **record object** describes a save record field by field. It is encoded by the runner independently of the
implementation under test. Missing fields default to the values shown:
`{"version": 4, "in_tick": false, "city_established": false, "pause_count": 0, "sim_speed": 3, "sim_time": 0,
"frame_budget_ms": 15, "level_counters": [0 × 11], "level_fired": [false × 11], "julian": 2451545}`. Use
`"date": {"year", "month", "day"}` instead of `julian` to give a calendar date. Per-level arrays can also be
written as objects keyed by level, such as `{"4": 125}`. Unlisted levels are then 0.

**Expectations**

| op | Arguments | Checks |
|---|---|---|
| `expect_events` | `events`: list of partial event objects; `filter?` | The events recorded since the last `expect_events`/`clear_events` (after filtering) match the list one-to-one, in order. Each expected object checks only the keys it gives. **Clears all recorded events.** |
| `expect_event_count` | `filter?`, `count?`, `min?`, `max?` | number of matching events (does not clear) |
| `expect_spacing` | `filter?`, `min_ms?`, `max_ms?` | `t` difference between consecutive matching events (does not clear) |
| `expect_sequence` | `filter?`, `field` (`data1`/`data2`/`data3`), `start`, `step` (default 1) | the i-th matching event has `field == start + i·step` (does not clear) |
| `clear_events` | | forget recorded events |
| `expect_state` | `state`: partial snapshot | see below |
| `expect_date` | any of `julian`, `year`, `month`, `day`, `day_of_year`, `weekday` | the current simulation date |
| `expect_julian` | `year`, `month`, `day`, and any of `julian`, `day_of_year`, `weekday`, `leap_year`, `days_in_february` | the pure date functions (§2.1), including the round trip |
| `expect_agent_enabled` | `name`, `value` | `GetAgentEnabled` |
| `expect_agent_names` | `names` | `EnumerateAgentsByName` (in order) |
| `expect_dilation` | `value` | `GetAnimationTimeDilation` (±1e-6) |
| `expect_paused` | any of `paused`, `hidden`, `emergency`, `any` | `IsPaused` / `IsHiddenPaused` / `IsEmergencyPaused` / `IsAnyPaused` |
| `expect_debug` | `debug`: any of `ready`, `one_shot_or_emergency_pending`, `hours_complete`, `day_level_done`, `tomorrow_starts_week/month/year`, `blocked_by_week/month/year`, `day_gate_open`, `interval_elapsed_ms`, `ms_per_day` | the day-barrier diagnostics view (§5.2 step 8): why the next day can or cannot open |

`expect_state` fields: `initialized`, `in_tick`, `city_established`, `pause_count`, `hidden_pause_count`,
`emergency_pause_count`, `sim_speed`, `sim_time`, `speed_change_enabled`, `frame_budget_ms`, `gate_level`,
`ms_per_day`, `target_fps`, `julian`, `ms_per_day_table` (array), `animation_dilation` (array, ±1e-6),
`level_counters`, `level_fired`, `agent_counts`, `cursors` (per-level, as arrays or `{"level": value}` objects), and
`idle_cursor`. Cursor values are the index of the agent under the cursor in registration order, or `null` (or -1)
for "at end". Booleans may be written as `true`/`false` or `1`/`0`.

## Foundations ops

Pure functions of `docs/specs/foundations.md`; they need no simulator state. Vector files are named `f7_…` (and `f6_…` for §6 items outside §7, `df5_…` for the data-formats §5 conversions).

| op | Arguments | Checks |
|---|---|---|
| `stream_write` | `values`: list of one-key objects (`u8`, `i8`, `u16`, `i16`, `u32`, `i32`, `u64`, `i64`, `f32`, `f64`, `bool`, `str`, `bytes` (hex), `res_key` (`[type, group, instance]`), `null_ref`); `hex` | the bytes written (§2.2) equal `hex` |
| `stream_read` | `hex`; `reads`: list of `{get, expect?, position?}` with `get` one of `u8`, `u16`, `u32`, `i32`, `u64`, `i64`, `f32`, `f64`, `str`, `rz_char_str` (+ `max_bytes`, result as hex), `skip` (+ `n`); `error?` | each read's value; `expect: null` means the read must fail without consuming; `position` after the read; the sticky error flag at the end |
| `crc` | `ascii` or `hex`; `expect` | CRC-32/MPEG-2 (§3.4) |
| `store_entry` | `class_id`, `payload_hex`, `entry_hex`, `index_hex?` | the class-object store entry (and index) written for one object with ID 1 whose `Write` emits the payload (§3.3) |
| `curve` | `points` (flat x, y list), `absent?`, `eval`: `[[x, y], ...]` (x and y may be `"0x…"` bit patterns; an expected NaN matches any NaN), `tolerance?` (relative, default 1e-6) | response-curve construction and evaluation (§4) |
| `rng` | `generator` (`RzRandom` or `RzRandomSuper`), `seed`, `wall_clock_seed?`, `process_clock?`, `calls`: `{method, args?, weights?, expect?, lcg?, mul?}`. Methods: `RzRandom`: `next_raw`, `uniform_u32`, `range_i32`, `uniform_f64`, `range_f64`, `gaussian`, `range_gaussian`, `range_gaussian_fast`, `range_i32_gaussian_fast`, and the pure mappings `uniform_of` (`[i]`, one signed draw) and `gaussian_to_range` (`[g, a, b]`); `RzRandomSuper`: `next_u32`, `uniform_u32`, `uniform_f64`, `range_f64`, `weighted_choice` | outputs and internal states, bit-exact (§6) |
| `derive_seed` | `cases`: `{session, k or generator, k_expect?, expect, next_raw?}`; `generator` names a file-local generator of the §6.3 table (`building_development`, `growth_developer`, `tract_developer`, `lot_developer`, `zone_developer`, `lot`, `budget`, `crime`, `pollution`, `wealth_requester`, `network_lot_manager`, `network_construction_crew`, `construction_occupant`, `base_texture_occupant`, `retaining_wall_occupant`, `prop_occupant`, `fire_occupant`, `earthquake`, `pipe_burst`, `riot`, `automata`, `region_creation`, `advice`) | the derived seed (§6.3), the generator's `K`, and the first `NextRaw` words of a `cRZRandom` with that seed |

| `variant_write` | `variant`, `hex`, `ok?` (default true), `round_trip?` (default true), `position?`, `error?` | `WriteVariant` (§9.2) gives `hex`; unless `ok` is false or `round_trip` is off, the bytes read back to the same variant |
| `variant_read` | `hex`, `preset?` (the destination's value before the read), `expect` (a variant, or `null` = the read fails), `unchanged?`, `position?`, `error?` | `ReadVariant` (§9.2) |
| `holder_write` | `props`: `[{id, value}]` in the order they are added; `hex` | the plain holder record (§10.2) |
| `holder_read` | `hex`, `preset?` (props), `ok?`, `expect?` (props in holder order), `rewrite?`, `position?`, `error?` | reading the plain holder; `rewrite` writes it back and compares with `hex` |
| `exemplar_holder` | `exemplars?` (a stub resource manager: `[{key, props}]`), `preset_parent?` (key), `hex`, `ok?`, `own?`, `parent?` (key or `null`), `lookup?` (`[{id, value or null}]`), `rewrite?`, `position?` | reading the exemplar holder (§10.3); keys are `{group, type, instance}` |
| `crt_rand` | `seed?` (default 1), `srand?`, `expect?` (outputs), `shuffle?` (`{input, expect, calls?}`) | the C runtime `rand()` and `random_shuffle` (§12) |
| `variant_convert` | `variant` (a scalar), `cases`: `[{to, expect}]` with `to` one of `u8`, `i8`, `u16`, `i16`, `u32`, `i32`, `u64`, `i64`, `bool`, `f32`, `f64` | a scalar element converted as a property reader does (`docs/specs/data-formats.md` §5.1): integer targets keep the low bits of the element's int64, Bool is int64 ≠ 0, float targets go through its double (a Float32 read as Float32 as is); integer expectations may be `"0x…"` bit patterns |
| `variant_get_u32` | `variant` (any variant), `expect`: a number or `false` | holder slot 8, `GetProperty(id, uint32&)` (`docs/specs/data-formats.md` §5.1, `docs/specs/neighbors.md` §15.4 NG44): scalars by number, arrays, strings, the void array and Empty through the text round trip (`fc4-formats`); pointers and objects fail *[choice]* |

Floats in `stream_write`, `stream_read` and `curve` may be numbers or `"0x…"` bit patterns (for NaN and −0.0).

A **variant** in a vector is `{"type": T, "value": v}` for a scalar (`T` one of `bool`, `u8`, `i8`, `u16`, `i16`,
`u32`, `i32`, `u64`, `i64`, `f32`, `f64`, `char`, `unichar`, `rzchar`, `voidptr`), `{"type": "T[]", "values": [..]}`
for an array (also `object[]`), `{"type": "string", "value": "…"}`, `{"type": "bytes", "hex": "…"}` (the void
array), `{"type": "empty"}`, and `{"type": "object", "id"?, "class"?}` (no `id` = a null reference; written with
that save ID, read back as the host object whose token is the ID).

## Budget ops

The budget vectors (`b15_…`) drive the budget simulator with a fake host: property holders (named by label), a lot
list, ordinance and neighbour stubs, LTEXT strings, the history warehouse, the disaster layer and the save store.
Every op name starts with `budget_`.

### Environment and objects

| op | Arguments | Effect |
|---|---|---|
| `budget_env` | any of `difficulty`, `year`, `month`, `established`, `city_size` (`[x, z]`), `ordinance_income`, `ordinance_expense`, `neighbor_income`, `neighbor_expense`, `rng_seed`, `advance_month` (true: next month), `disasters` (IDs that exist), `ltext` (`[{group, instance, text}]`) | sets the fake world. Defaults: hard, 2000-01, established, seed 1, everything else 0. Missing LTEXT reads as `LT:gggggggg:iiiiiiii`, building-type names as `BT:iiiiiiii`. |
| `budget_create` | | a new budget simulator (label `budget`); it replaces the current one |
| `budget_init` | `tuning?` (`{"0xPROPID": value or [values]}`, the exemplar of §9; without it, the code defaults), `fix_cs3?` (§13.1 fix), `expect?` | `Init` (core part) |
| `budget_shutdown` | `expect?` | `Shutdown` (core part) |
| `budget_message` | `type` (name or ID), `data1?`, `expect?` | `DoMessage` (§4.3) |
| `budget_cheat` | `id` (`RiskyMoney`, `WeaknessPays`, `Moolah` or an ID), `text?`, `expect?` | the cheat notification |
| `budget_holder` | `holder` (label), `props?` (`{"0xPROPID": {"u32"\|"i64"\|"f32": [...]}}`, replaces all properties), `occupant?` | creates or changes a property holder |
| `budget_lots` | `lots`: `[{purpose, building_wealth, lot_wealth, value, land_value, size_x, size_z, has_building, livable, repeat}]` (defaults 1, 1, 1, 0, 0, 1, 1, true, true, 1) | the lot list `ComputeTax` iterates |

### Calls

`budget_call` (`method`, `args?`) calls the budget simulator, `budget_dept` (`dept`, `method`, `args?`) a department
and `budget_line` (`dept`, `line`, `method`, `args?`) a line item. Methods are the snake-case names of the spec
methods (`deposit_funds`, `issue_bond`, `set_funding_percentage`, `set_local_funding_percent`, `full_expenses`, …;
see `src/budget.rs` for the list) plus `settle_month` (§5.1), `simulate_day` (§5.7), `compute_tax` (§5.2) and
`tamper_funds` (writes `funds` only, for the shadow check). Holders are passed by label. Results are checked with
`expect` (booleans, integers, floats with relative tolerance 1e-6, strings, lists, `null` for a null pointer) and,
for integers, `expect_min` / `expect_max`. Departments and line items return their IDs; holders their labels.

### Checks

| op | Arguments | Checks |
|---|---|---|
| `budget_expect` | `state`: any of `initialized`, `loaded_from_save`, `agriculture_rate_fix`, `panel_flag`, `funds`, `shadow_funds`, `total_borrowed`, `last_tax_income`, `ytd_income`, `ytd_expense`, `ytd_tax_income`, `ytd_reserved`, `ytd_ordinance_income`, `ytd_ordinance_expense`, `ordinance_income_by_month`, `ordinance_expense_by_month`, `ytd_bond_payments`, `last_month_income`, `last_month_expense`, `yearly_income`, `yearly_expense`, `bond_count`, `risky_money_counter`, `department_ids`, `tax` (`{"i": {rate, max_rate, weighted_assessed, monthly_tax, reserved, history}}`), `bonds` (list of partial bonds) | the budget state (§3.1). Arrays may be given as `{"index": value}` objects. |
| `budget_expect_items` | `holder`, `purpose?`, `items` (list of partial `{department, line, purpose, cost, index, local_funding}`, or `null` for none) | `GetBudgetItemInfo`, or `GetBudgetItemForPurpose` when `purpose` is given |
| `budget_expect_loans` | `loans` (partial `{months_remaining, years_remaining, total_loan_amount, remaining_payment, monthly_payment}`) | `GetAllLoans` |
| `budget_expect_groups` | `groups` (partial `{id, name, total_expense, total_income, departments}`) | `GetAllGroups` plus each group's department set |
| `budget_expect_tuning` | `tuning`: any of the §9 values by snake-case name; `land_value_curve` as `[[x, y], ...]` evaluations | the tuning in effect |
| `budget_expect_holder` | `holder`, `prop`, `values` (`{"u32"\|"i64"\|"f32": [...]}`) or `absent: true` | a property of a holder (floats relative 1e-6) |
| `budget_expect_refs` | `refs`: `{"label": count}` | the reference count of a holder or object (1 = only the world's own) |

### Persistence

| op | Arguments | Effect |
|---|---|---|
| `budget_save` | `slot` | saves the budget through the class-object store (foundations §3.5) |
| `budget_load` | `slot`, `expect?` (default true: the store reports a clean load), `read_ok?` (whether the budget record was accepted) | loads a new budget from the slot and makes it current. Holders come back as the same objects, with one reference each. Record `Read` results are ignored by the store, so a rejected record leaves its object in the initial state. |
| `budget_expect_save` | `slot` | saving now gives byte-identical records |
| `budget_patch` | `slot`, `class` (`budget`, `department`, `line` or an ID), `entry?` (default 0), `offset` (into the payload), `hex` | overwrites bytes of a saved entry and fixes its CRC |

Budget events: `post` events carry `void1` and `unknown` as object labels (`""` for none); `history` (`series`,
`value`), `history_queue` (`series`, `length`), `show_budget_window` (`panel`), `disaster` (`id`, `x`, `z`),
`add_ref` / `release` (`agent` = label). Message names: `FundsChanged`, `BondIssued`, `BondFinished`,
`EndGameForBudgetFailure`, `BudgetGroupChanged`, `FundingPercentageChanged`, `IdealMonthlyFundingChanged`,
`BudgetForOccupantChanged`, `CityEstablished`, `ToggleBudgetDialog`.

## Scalar simulator ops (shared)

The demand, ordinance and weather vectors share a few conventions (`src/scalar.rs`):

- Each family keeps its own fake world for the whole vector, created by its first op.
- Numbers may be written as integers, floats or `"0x…"` strings. Results are compared with a relative tolerance
  of 1e-5 (demand spec §14), exactly when both sides are integers (an integer result against an expected `"0x…"`
  or decimal string is exact too, so an ID never matches its neighbour); objects are compared key by key, with
  keys compared as integers when both parse.
- Events go to one log. `*_expect_events` (`events`, `filter?`) checks the events since the last check, one to one
  and in order, each given as a partial object (only the keys written are checked), after keeping only the events
  that match `filter`; it then clears the log. `*_expect_event_count` (`filter?`, `count` / `min` / `max`) counts
  without clearing; `*_clear_events` clears.
- Property maps are written `{"0xPROPID": {"u32": 5}}` (a scalar), `{"f32": [1, 2]}` (an array) or
  `{"str": "text"}`; types `u8`, `u16`, `u32`, `i32`, `i64`, `f32`, `bool`.
- Saves go through the class-object store (foundations §3.5) with a root record that references the simulator.
  `*_patch` (`slot`, `class`, `entry?`, `offset`, `hex`, `insert?`, `delete?`) overwrites, inserts or removes
  payload bytes of one entry and fixes its size and CRC.

## Demand ops

The demand vectors (`d16_…`) drive the demand simulator (`fc4_core::demand`) over a scripted city. Every op name
starts with `demand_`.

### Environment

| op | Arguments | Effect |
|---|---|---|
| `demand_env` | any of `cells` (`[x, z]`, read at `Init`), `today` (Julian day; default 2000-01-01), `date` (`{year, month, day}`), `advance_days`, `clock_step` (ms the fake clock advances per reading; default 0), `air`, `sea` (port trips), `history_max` (`{"0xSERIES": max}`; absent = no data in the window), `ordinance` (`{"0xPROP": factor}`; default 1.0), `nation` (`{"0xDEMANDID": modifier}`), `nation_default`, `dev_types` (`[{purpose, wealth, dev}]`) | sets the fake world. `GetDeveloperType` defaults to R `0x1000`, CS `0x3100`, CO `0x3300`, Tourism `0x3200` (+ `0x10`·wealth), I `0x4100`/`0x4200`/`0x4300`/`0x4400`. |
| `demand_exemplar` | `instance`, `group?` (default `0xC7BB4816`), `exemplar_type?` (default 3), `key?` (0x21), `min?`, `max?`, `supply?`, `demand?` (initial values), `drives?`, `satisfies?`, `contributes?` (`[[target, percent], …]`), `caps?`, `sums?` (key lists), `region_use?`, `props?` (extra raw properties) | adds a demand exemplar; enumeration order is the order of these ops, and a later exemplar with the same key overrides an earlier one when loaded singly |
| `demand_tuning` | `props` (the Demand Simulator exemplar `0x4A1F3CE6` as a property map) or `null` | replaces or removes the tuning exemplar |
| `demand_building` | `type`, `purpose?`, `capacities?` (`[[[devType, capacity], …], …]`), `creates?`, `satisfies?` (pairs), `creates_amounts?`, `satisfies_amounts?` (float lists), `props?` | a building type: its profile (BDS) and exemplar |
| `demand_cells` | `cells`: `[{x, z, w?, h?, building_type, wealth, has_capacity, desirability, capacity, eq, workforce}]` (defaults 1, 1, true, 100, 0, 0, 0), `replace?` (default true) | census cells; `w`/`h` repeat an entry over a rectangle |
| `demand_lot` | `lot` (ID), `building_type?`, `excluded?` (`0x27812844`) | a lot object for StateChanged |
| `demand_connections` | `connections`: `[{type, city, traffic}]` | the neighbour connections and their freight trips |
| `demand_cities` | `cities?` (`[{id, tax, allowable, population, extrapolated, workforce}]`, maps keyed by type; replaces all), `all_connected?`, `connected?`, `self?` | region cities and the two neighbour lists |
| `demand_developers` | `developers`: `[{sub_type, kind, source?: [supply, demand], last_build?}]` | growth developers (`source` absent = no source node) |

### Calls and checks

| op | Arguments | Effect |
|---|---|---|
| `demand_create` | | a new demand simulator |
| `demand_init` / `demand_shutdown` | `expect?` | `Init` / `Shutdown` (core parts) |
| `demand_message` | `type` (`SimBegin`, `SimNewDay`, `SimNewMonth`, `SimIdle`, `PreSave`, `ConnectionCreated`, `ConnectionDeleted`, `IngredChangesMade` or an ID), `data1?`, `data2?`, `data3?`, `expect?` | `DoMessage` |
| `demand_state_changed` | `lot`, `old`, `new`, `building_type`, `wealth` | kMessageStateChanged (§7) |
| `demand_call` | `method`, `args?`, `expect?`, `expect_bits?` (the float32 bits of a float result, `"0x…"`; for an array of floats such as `jobs_per_unit`, an array of them) | a simulator method: `get_demand`, `simulator_type`, `update_occupant_effects`, `jobs_per_unit`, `jobs_by_census`, `neutral_tax_rate`, `local_population_summary`, `do_idle`, `propagate` (DoDemandIndexLinks), `simulator_begin`, `daily`, `monthly`, `update_modifiers`, `evaluate_connection_utilization`, `export_demand`, `reset_regional_demands`, `perform_census`, `apply_census`, `record_census_history`, `add_agent_effect`, `update_capacity`, `set_economy_bonuses`, `reload_tuning` |
| `demand_new_node` | `node` (label) | a free-standing node (class factory) |
| `demand_node` | `node` (label) or `key`, `method`, `args?`, `expect?` | a node method by snake-case name (`add_to_supply`, `end_of_cycle`, `set_active_max`, `calculate_active_demand` (`args: [smooth]`), …) |
| `demand_expect_node` | `node` or `key`, `state` (partial: `id`, `supply`, `demand`, `new_supply`, `new_demand`, `active_demand`, `active_min`, `active_max`, `economy_modifier`, `tax_modifier`, `demand_cap`, `region_use`, `flag`, `loaded_from_save`, `initialized`) | the node state |
| `demand_expect` | `state` (partial: `initialized`, `loaded_from_save`, `census_state`, `cursor`, `co_economy_bonus`, `i_economy_bonus`, `commercial_relief`, `industrial_relief`, `current_census`, `previous_census`, `node_keys`, `link_count`, `effect_building_types`, `export_cities`) | the simulator state |
| `demand_expect_refs` | `refs` (`{"0xKEY": count}`) | host reference counts of the simulator's nodes |
| `demand_save` / `demand_load` / `demand_expect_save` | `slot`; load: `expect?` (clean store load) | the class-object store; a load makes the loaded simulator current |
| `demand_patch` | see the shared conventions; `class` is `simulator`, `node` or an ID | |
| `demand_expect_payload` | `slot`, `class`, `entry?`, `hex` | the payload bytes of one saved entry |
| `demand_node_record` | `node`/`key`, `hex`, `read?`, `expect?` | with `read`, reads the bytes into the node (`expect` = the result); otherwise the node's own record must equal `hex` |

Demand events: `post` (`key`, `data2`: ActiveDemandChanged), `history` (`series`, `value`), `history_queue`
(`series`, `length`), `extrapolate` (`city`, `type`, `amount`: `cRegionalCity::ExtrapolateGrowth`), `warn` (`text`).

## Ordinance ops

The ordinance vectors (`o13_…`) drive the ordinance simulator and the standard ordinance classes
(`fc4_core::ordinance`) over a scripted city. Every op name starts with `ordinance_`. Ordinance ops also work
inside scheduler agents' `on_call` (see `o13_10_month_order`).

### Environment

| op | Arguments | Effect |
|---|---|---|
| `ordinance_env` | any of `year` (sim year; `null` = no simulator; default 2000), `population`, `budget` (budget simulator present; default true), `departments` (`{"0xDEPT": totalExpenses}`), `line_items` (`{"0xDEPT": {"0xLINE": fullExpenses}}`), `landfill_capacity`, `power_plants` (type-3 count), `police`, `recycling` (recycling-center effect), `lots` (`[{lot, building_type}]`), `view_control` (current input control ID), `ltext` (`[{type?, group?, instance, text}]`, default type `0x2026960B`, group `0x6A231EAA`), `classes` (registered ordinance classes; `null` = all 21 standard classes) | sets the fake world |
| `ordinance_exemplar` | `instance`, `group?` (default `0xA9C2C209`), `exemplar_type?` (default 14), `id?` (default the instance; `null` omits it), `enact?`, `repeal?`, `constant?` (s32), `factor?`, `chance?` (f32), `year?`, `advisor?`, `income?`, `name?`, `description?` (string properties), `name_ltext?` / `description_ltext?` (`true` = the shipped LTEXT key for the instance, or `[type, group, instance]`), `props?`, `replace?`, `remove?` | adds (or replaces / removes) an ordinance exemplar; enumeration order is the order of these ops |

### Calls and checks

| op | Arguments | Effect |
|---|---|---|
| `ordinance_create` | | a new ordinance simulator |
| `ordinance_init` / `ordinance_shutdown` | `expect?` | `Init` / `Shutdown` (core parts) |
| `ordinance_message` | `type` (`SimNewMonth`, `SimBegin` or an ID), `expect?` | `DoMessage` |
| `ordinance_call` | `method`, `args?`, `expect?` | a simulator method: `add_ordinance` / `remove_ordinance` (`args: [label]`), `is_ordinance_available`, `is_ordinance_on`, `is_ordinance_available_but_off`, `set_ordinance_available`, `set_ordinance_on`, `available_count`, `available_array`, `on_count`, `on_array`, `id_array` (`args: [capacity]`, result `{ids, total}`), `is_ordinance_enabled`, `set_ordinance_enabled`, `ordinance_id_by_name`, `ordinance_by_id` / `ordinance_by_name` (result: the ID or `null`), `monthly_expense`, `monthly_income`, `cumulative_factor`, `cumulative_offset` (`args: [prop, index]`), `do_monthly_simulation`, `simulation_begin` |
| `ordinance_new` | `ordinance` (label), `class`, `instance?`, `group?` | a free-standing standard ordinance (class factory), optionally `SetKey` |
| `ordinance_foreign` | `ordinance` (label), `id`, `name?`, `enact?`, `repeal?`, `monthly?`, `conditions?`, `available?`, `on?`, `props?` | a scripted plugin `cISC4Ordinance` |
| `ordinance_object` | `ordinance` (label) or `id`, `method`, `args?`, `expect?` | an ordinance method by snake-case name (`init`, `current_monthly_income`, `check_conditions`, `simulate`, `set_on`, `force_available`, `has_misc_property`, …; `set_key` / `from_data` take `[instance]` and `group?`) |
| `ordinance_expect_object` | `ordinance` or `id`, `state` (partial: `class`, `plain`, `initialized`, `key` (`[type, group, instance]` or `null`), `id`, `name`, `description`, `year_first_available`, `chance_availability`, `enactment_income`, `retracment_income`, `monthly_constant_income`, `monthly_income_factor`, `advisor_id`, `is_income_ordinance`, `available`, `on` (the raw flag), `enabled`, `adjusted`, `misc_parent` (the parent exemplar's instance or `null`)) | a standard ordinance's state |
| `ordinance_expect` | `state` (partial: `initialized`, `loaded_from_save`, `keys`, `monthly_expense`, `monthly_income`, `pending`) | the simulator state |
| `ordinance_expect_refs` | `refs` (`{"label" or "0xID": count}`) | host reference counts |
| `ordinance_save` / `ordinance_load` / `ordinance_expect_save` | `slot`; load: `expect?` (clean store load) | the class-object store; a load makes the loaded simulator current |
| `ordinance_patch` / `ordinance_expect_payload` | see the shared conventions; `class` is `simulator` or a class ID | |
| `ordinance_record` | `ordinance`/`id`, `hex`, `read?`, `expect?` | with `read`, reads the bytes into the ordinance (`expect` = the result); otherwise its record must equal `hex` |

Ordinance events: `post` (`type`: `OrdinanceUpdated`, `AvailabilityChanged`, `MonthlyDone`; `id` = data2, `value` =
data3), `demolish` (`lots`), `hide` (`building_type`), `remove_control` (`id`).

## Weather ops

The weather vectors (`w14_…`) drive the weather simulator (`fc4_core::weather`) over a scripted city and terrain.
Every op name starts with `weather_`. Cell coordinates are terrain vertex / cell indices; world units are 16 per
cell. Grids are x-major (`rows[x][z]`).

### Environment

| op | Arguments | Effect |
|---|---|---|
| `weather_env` | any of `size` (`[cellsX, cellsZ]`; the city is 16 × that in world units; default 16 × 16), `cell_width` (`[x, z]`, city cell width, default 16), `hemisphere` (default 1), `month` (the simulator's calendar month, default 1), `water_prefs` (`[lo, hi]` or `null` = no region), `terrain` (default true), `flat` (the height of every vertex; clears `heights`), `heights` (`[{x, z, w?, d?, h}]`), `water` (`[{x, z, w?, d?}]` cells with `CellHasWater`, or `null`), `normals` (`[{x, z, w?, d?, n: [x, y, z]}]`, default (0, 1, 0)), `sea_level` (default −1) | sets the fake world. `GetAltitude(worldX, worldZ)` reads the vertex `floor(world / 16)` |
| `weather_tuning` | `shipped` (the §9 values), `props` (a property map merged over them), or `none` (no exemplar) | the tuning exemplar the next `Init` reads |
| `weather_create` | `seed?` (the wall-clock seed), `rng?`: `{seed}` (a real generator with that seed) or `{stub: {uniform, gaussian, int}}` (queued values; each queue repeats its last value, defaults 0.5 / 0 / 0) | a new simulator (the default generator is wall-clock seeded; a stub maps its uniform u to a range with the 24-bit `r24(r24(u × r24(b − a)) + a)` of foundations §6.1) |
| `weather_rng` | `seed` or `stub` as in `weather_create` | replaces the simulator's generator |

### Calls and checks

| op | Arguments | Effect |
|---|---|---|
| `weather_init` / `weather_shutdown` | `expect?` | `Init` / `Shutdown` (core parts) |
| `weather_message` | `type` (`SimNewDay`, `SimNewWeek`, `SimNewMonth`, `PostCityInit` or an ID), `date?` (`{year, month, day}`, the day message's data1), `data1?`, `data3?` (the month), `expect?` | `DoMessage` |
| `weather_call` | `method`, `args?`, `expect?` | a simulator method: `do_post_city_init`, `do_complete_simulation`, `do_monthly_simulation`, `do_weekly_simulation`, `do_daily` (`[julian]`), `new_current_ambient_values` (`[seasonMonth]`), `onshore_direction` (result `[x, z]`), `simulate_cell_temperature`, `simulate_moisture` (`[x0, z0, x1, z1]` cells), `update_moisture` (world units), `generate_moisture_variation`, `rebuild_water`, `set_manual_moisture_addition`, `cell_width_x/z`, `cell_count_x/z`, `position_to_cell` (`[result, cx, cz]`), `cell_to_position`, `cell_to_standard_city_cell`, `standard_city_cell_to_cell`, `current_season`, `current_season_month`, `moisture_value`, `moisture_type`, `moisture_map` (the grid), `humidity`, `ambient_wind` / `wind_at_cell` (`{speed, v}`) |
| `weather_set` | `state` (scalar fields as in `weather_expect`; `*_fill` fills a grid; `sea_mask_cells`, `moisture_cells`, `moisture_base_cells`, `temperature_cells`: `[{x, z, w?, d?, v?}]`) | pokes state (test setup) |
| `weather_expect` | `state` (any §3 field in snake case, plus `water_rects`, `iterator` (`state`: `running`/`exhausted`, `mode`, `rect`, `count`, `index`, `current`, `step`) and `rng` (`lcg`, `mul`, `has_cached`, `cached`); `iterator` and `rng` match partially), `exact?` (true: the other numbers must be bit-equal, for precision tests) | the simulator state |
| `weather_expect_grid` | `grid` (`temperature`, `moisture`, `moisture_base`, `sea_mask`) and any of `size`, `rows`, `all`, `cells` (`[{x, z, w?, d?, v}]`), `rect` (`[x0, z0, x1, z1)`) with `inside` / `outside`, `range` (`[lo, hi]`), `min`, `max` | grid contents |
| `weather_iterator` | `rect` (`[x0, z0, x1, z1]`, inclusive), `shuffle?` (default true), `crt_seed?` (default 1), `rand_calls?`, `step?`, `count?`, `current?`, `first?` (`[[x, z], …]`), `full_pass?` | a free-standing prime-step iterator (§6.7) |
| `weather_save` / `weather_load` / `weather_expect_save` | `slot`; load: `rng?` (the loaded object's generator before Read), `expect?` | the class-object store; a load makes the loaded simulator current |
| `weather_patch` / `weather_expect_payload` | see the shared conventions (the class is always the weather simulator) | |

Weather events: `post` (`type`: `SeasonChanged`, `WeatherCycleCompleted`, `MoistureMapUpdated` or `other`;
`data2`), and with a stub generator `rng` (`kind`: `uniform`, `gaussian`, `int` (with `lo`, `hi`); `value`).

## Simgrid ops

The simgrid vectors (`g10_…`) drive the core grid class and map utilities of `fc4_core::grid` (`src/simgrid.rs`
here). Grids are named; `type` is one of `u8`, `i8`, `u16`, `i16`, `u32`, `i32`, `f32`, `f64`.

| op | Arguments | Effect / check |
|---|---|---|
| `grid_city` | `cells` (`[x, z]`), `cell_width?` | the city the grids are sized from |
| `grid_new` | `grid`, `type`, `size` (tract size S) | a new grid, `Init` included |
| `grid_set_tract_size` | `grid`, `size` | `SetTractSize` (§3.2) |
| `grid_expect_geometry` | `grid`, any of `size`, `shift`, `count_x`, `count_z`, … | the derived geometry |
| `grid_set` / `grid_expect` | `grid`, `all?`, `values` (`[[x, z, v]]`) | write / check tract values |
| `grid_call` | `grid`, `method` (`cell_value`, `tract_value`, `average_cell_rect`, `average_tract_rect`, `tract_in_bounds`, `position_to_tract`, `tract_center`, `tract_corner`, `clsid`, `init`, `set_instance_id`), `args`, `expect` | one method |
| `grid_write` / `grid_read` | `grid`, `hex`, `zero_tail?`, `position?`, `expect?` | the save record (§5) |
| `grid_radius_effect` | `grid`, `center`, `radius`, `center_value`, `outer_value` | the radial effect (§4.6) |
| `grid_distance_effect` | `grid`, `mask?` (default the target), `mask_value`, `radius`, `near`, `far` | the distance effect (§4.7) |
| `grid_two_grids` | `a` (u8), `b` (i16), `expect` (`[[x, z, v]]` in visit order) | the paired walk |
| `iter_scan` | `rect` (`[x0, x1, z0, z1]`), `step` (below 1 counts as 1), `reset?`, `expect`, `rests?` (`[x, z]` after the end) | the scan iterator |

## Land value ops

The land value vectors (`lv16_…`) drive the land value simulator with a fake host: terrain (land vertices,
altitudes, allowed heights), zones, developers with their desirability maps, and grids created through the host
(the plugin creates the game's own grid classes). Every op name starts with `lv_`.

| op | Arguments | Effect / check |
|---|---|---|
| `lv_world` | any of `cells`, `cell_width`, `land`, `water`, `land_vertices`, `clear_land`, `altitude`, `altitudes`, `heights`, `zoned`, `clear_zones`, `developers` (`{"0x1030": {size, cells?, all, values} \| null}`; `cells: [x, z]` makes the map cover a city of that size) | the fake world |
| `lv_create` / `lv_init` / `lv_shutdown` | `tuning?` for init (`"shipped"` or an object of §9 values; `null` = absent) | lifecycle (§4) |
| `lv_step` | `budget`, `count?`, `expect_state?` | SimIdle steps (§6) |
| `lv_run` | `budget`, `max`, `expect_calls?` | steps until a pass completes |
| `lv_message` | `type`, `data3?`, `expect?` | `DoMessage` (§5) |
| `lv_set` | any of `dirty`, `state`, `average`, `mean_alt`, `min_alt`, `max_alt`, `mode_alt` | debug overrides |
| `lv_expect` | state fields, `histogram`, `mean_alt_bits`, tuning fields, `has_grids`, `developer_weights` | the simulator state |
| `lv_set_grid` / `lv_expect_grid` | `grid` (`intrinsic`, `total`, `slope`, `scratch`), `all?`, `values?`, `rows?` (`rows[z][x]`), `tract_size?` | grid contents |
| `lv_query` | `method` (`land_value`, `land_value_type`, `average_land_value`, `mode_altitude`), `args`, `expect` | the interface queries (§11) |
| `lv_altitude` | `setup` (`[mode, mean, min, max]`), `max_allowed`, `curve?`, `expect?`, `effects?` | the altitude effect (§7.4) |
| `lv_desirability` | `weight`, `curve?`, `cases` | the desirability step (§7.5) |
| `lv_save` / `lv_load` | `ids` / `hex`, `position?`, `expect?` | the save record (§10) |
| `lv_expect_events` / `lv_clear_events` | `events` | host calls: `create`, `set_tract_size`, `grid_shutdown`, `release`, `history_length`, `history`, `post`, `load` |

## Tract grid ops

The tract grid vectors (`tg10_…`) drive the trait-based grid module `fc4_core::grid` (the storage-agnostic
`TractGrid` used by the M3 grid simulators; `docs/specs/simgrid.md`). Every op name starts with `tg_`. Grids are named; values are JSON numbers (integers for
integer grids). Tract coordinates are `[x, z]`; `rows` are x-major (`rows[x][z]`).

| op | Arguments | Effect |
|---|---|---|
| `tg_city` | `cells?` (`[countX, countZ]`, default 256 × 256), `width?` (`[x, z]` cell width, default 16) | the city later `SetTractSize` calls read |
| `tg_create` | `name`, `type` (`u8`, `i8`, `u16`, `i16`, `u32`, `i32`, `u64`, `i64`, `f32`, `f64`) or `clsid` (a §2 factory ID; `expect: false` if none), `expect_type?`, `init?`, `tract_size?`, `instance?` | a new core grid |
| `tg_call` | `grid`, `method`, `args?`, `expect?` | a slot: `init`, `shutdown`, `is_init`, `instance_id`, `set_instance_id`, `clsid`, `set_tract_size`, `tract_size`, `shift`, `count_x/z`, `width_x/z`, `inv_width_x/z`, `geometry` (an object of all of these), `tract_in_bounds`, `position_to_tract` (`[tx, tz]`), `tract_corner`, `tract_center`, `get_cell_value`, `get_tract_value`, `set_tract_value`, `set_tract_values`, `average_in_cell_rect`, `average_in_tract_rect` |
| `tg_set` | `grid`, any of `fill` (first), `rows`, `cells` (`[[x, z, v], …]`) | writes values |
| `tg_expect` | `grid`, any of `counts`, `cells` (`[[x, z, v], …]`), `others` (every unlisted tract), `rows` | checks values |
| `tg_record` / `tg_read` | `grid`, `hex`, `zeros?` (trailing zero bytes); read: `expect?` | the §5 record written / read |
| `tg_store_round_trip` | `grid`, `class?` (the record class it must be saved under), `expect_type?` | saves a reference through the class-object store, loads it through the §2 factories, compares records |
| `tg_radius_effect` | `grid` (`u8`, `i8`, `i16`), `center`, `r`, `c`, `o`, `counts?` (default the grid's) | `AddRadiusEffect` |
| `tg_table_distance` | `dx`, `dz`, `expect` | the §7.1 distance table |
| `tg_distance_effect` | `target`, `mask` (`u8` grids, may be equal), `value`, `radius`, `near`, `far` | `AddDistanceEffect` |
| `tg_two_grids` | `a`, `b`, `add?` (the callback adds B to A), `calls?` (`[[x, z, b], …]` in order) | `ProcessTwoGridsWithCallback` |
| `tg_scan` | `rect`, `step?`, `resets` (`[{at: [x, z], expect: [[x, z], …]}]`, every cell until false) | the scanning iterator |
| `tg_random_iterator` | `mode` (`table`, `prime`, `constructed`), `rect`, `crt_seed?`, `first_next?`, `resets` (`[{rect?, shuffle?, rand_calls?, count?, table?, first?, full_pass?}]`) | the random cell iterator |

## Flammability ops

The flammability vectors (`fl16_…`) drive the flammability simulator (`fc4_core::flammability`) with a fake host:
occupants with their flammability property, lots, plumbing, zones, the ordinance factor, the season, the fire
disaster manager, and the two u8 grids it creates through the host (`GridProvider<u8>`; tract size 1). Every op
name starts with `flam_`. Tract coordinates are `[x, z]`; grid cells are `[[x, z, v], …]`.

### Environment

| op | Arguments | Effect |
|---|---|---|
| `flam_env` | any of `cells` (`[countX, countZ]`, default 64 × 64), `occupants` (`[{id, type (building, prop, flora or an ID), flam (an integer (a Uint32 scalar), a variant object (the shared types plus `i8`, `i16`, `u64`, `f64`, `char`), or null = no property), preview?, lot?, cell or rect ([x0, z0, x1, z1]), pos? ([x, y, z])}]`, added to the listing), `unlist` (ids no longer listed in their cells; still known by id), `forget` (ids removed completely), `lots` (`[{id, condition, building?}]`), `random_lot` (id or null), `factor` (the ordinance factor 0x28F42AA0, null = none; default 1), `season` (null = no weather; default 0), `unwatered` / `unwatered_all`, `zoned` / `zoned_all`, `tutorial`, `fire_manager` (default true), `plumbing` / `zone_manager` (false = the service is missing; default true) | the fake world |
| `flam_tuning` | `shipped` (the §8 exemplar), `props` (a property map; merged over the shipped values with `shipped`, else alone), `remove` (property IDs removed afterwards), or `none` (no exemplar) | the tuning exemplar the next `Init` reads |
| `flam_create` | `rng?` (`{r: [...]}` queued `RzRandom` doubles, repeating 50 when empty, or `{process_clock}` the real generator with that seed; default the queue), `crt_seed?` | a new simulator |
| `flam_rng` | as `rng` in `flam_create` | replaces the simulator's generator |

### Calls and checks

| op | Arguments | Effect / check |
|---|---|---|
| `flam_init` / `flam_shutdown` | `expect?` | `Init` / `Shutdown` (portable parts) |
| `flam_message` | `type` (`SimNewDay`, `SimBegin`, `InsertOccupant`, `RemoveOccupant`, `ChangeOccupant`, `SeasonChange`, `OrdinanceUpdated`, `ConditionChanged`, `ExemplarChanged` or an ID), `occupant?`, `lot?`, `expect?` | `DoMessage` |
| `flam_days` | `count`, `ends?` (the days, 1-based in this op, on which SimCycleComplete was posted) | `count` SimNewDay messages |
| `flam_call` | `method`, `args?`, `expect?` | pure functions: `occupant_raw_value` (`[p, m]`), `daily_value` (`[r, g, watered, noWater]`), `global_multiplier` (`[factor, season, summer]`, null = missing), `compute_hotspots` (`[candidates, radius, minWeight]`), `query_interface` (`[iid]`), `notifications` (the subscribed message types, in order); simulator methods: `occupant_raw`, `tract_raw`, `occupant_update` (`[id, add]`), `compute_global_multiplier`, `is_cell_flammable`, `hotspot` (`[v, x, z]` or false), `average`, `flammability_grid` (its instance ID or null), `daily`, `end_of_cycle`, `compute_hotspot_list` (the new list), `fire_roll`, `curve` (`[x]`, null without a curve), `add_ref`, `release` |
| `flam_set` | any of `raw`, `adjusted` (cells), `raw_fill`, `adjusted_fill`, `state` (`global_multiplier`, `average`, `zoned_sum`, `zoned_count`, `tracts_per_day`, `candidates`, `hotspots` (`[[x, z, weight]]`), tuning fields, `curve` (flat pairs or null)) | pokes state (test setup) |
| `flam_expect` | `state`: any of `initialized`, `ref_count`, `destroyed`, `count_x/z`, `average`, `global_multiplier`, `zoned_sum`, `zoned_count`, `candidates`, `hotspots`, `tracts_per_day`, the tuning fields, `has_curve`, `has_adjusted`, `has_raw`, `iterator` (`state`, `mode`, `dirty`, `rect`, `count`, `index`, `current`, `step`; matches partially) | the simulator state |
| `flam_expect_grid` | `grid` (`raw`, `adjusted`) and any of `counts`, `tract_size`, `instance`, `cells`, `others` / `all` (every unlisted tract) | grid contents |
| `flam_expect_rand` | `calls` | the number of `rand()` calls on the subsystem's CRT generator since the last check |
| `flam_expect_visits` | any of `all`, `cells` (`[[x, z, n]]`), `total` | daily visits (one zone-type query each) since the last check |
| `flam_expect_events` / `flam_expect_event_count` / `flam_clear_events` | see the shared conventions | host calls |
| `flam_save` / `flam_load` / `flam_expect_save` | `slot`; load: `rng?`, `expect?` (the store-level result; the simulator's Read result is ignored, foundations §3.5), `into_current?` (read the record into the current simulator instead of a new one) | the class-object store; loaded grids are registered with the fake host |
| `flam_patch` / `flam_expect_payload` | see the shared conventions; `flam_patch` takes `class?` (default the simulator) to patch a grid record | |

Flammability events: `create_grid` (`instance`, `size`), `grid_shutdown` (`instance`), `random_lot` (`lot`),
`history` (`type`, `value`), `ignite` (`x`, `z`, `use_probability`, `teleport`, `fire_iid`), `cause` (`cause`),
`post` (`type`: `SimCycleComplete`), and with a queued generator `rng` (`value`, `a`, `b`).

## Pollution ops

The pollution vectors (`p11_…`) drive the pollution simulator (`fc4_core::pollution`) over a scripted city. Every
op name starts with `pollution_`. The city defaults to 128 × 128 cells of 16 m (64 × 64 tracts), the shipped tuning,
ordinance factors 1.0, zone 0 everywhere, no traffic, no deals, no landfill, funding 100 %, a budget department and
working visual effects. Grid coordinates are tracts unless an argument says cells; the garbage value grid has
1-cell tracts.

### Environment

| op | Arguments | Effect |
|---|---|---|
| `pollution_env` | any of `cells` (`[x, z]`), `cell_width` (`[x, z]`), `altitude` (terrain height everywhere), `zone_default`, `zones` (`[{x, z, w?, d?, zone}]` cells), `traffic` (`[{x, z, w?, d?, v}]` air-polluting traffic per cell), `factors` (`[{id, index?, value}]` cumulative ordinance factors; no `index` = indices 0..15), `deals` (`[{deal, type: import/export/ID, amount, used?}]`, replaces all; `null` = none), `population`, `landfill` (`[usage, capacity]`), `landfill_tiles`, `funding` (the department's funding fraction), `department` (whether `CreateDepartmentBudget` succeeds), `effects` (whether `CreateVisualEffect` succeeds) | sets the fake world |
| `pollution_tuning` | `shipped?` (default true), `props?` (merged over it), `remove?` (property IDs), or `none` | the tuning exemplar read at Init / TuningChanged |
| `pollution_occupant` | `occupant` (label), `props?` (property map), `tract?` (`[tx, tz]`: position at the tract centre), `position?` (`[x, y, z]`), `cells?` (`[x0, z0, x1, z1]` inclusive footprint → bounding box; default the position as a point), `bbox?` (`[[x, y, z], [x, y, z]]`), `powered?` (default true), `budget?` (`{line, cost?, index?, purpose?}`, the item `GetBudgetItemForPurpose` returns) | defines or redefines an occupant (the test holds one reference) |

### Calls and checks

| op | Arguments | Effect |
|---|---|---|
| `pollution_create` | `fx_seed?` (the effect generator's seed, default 1) | a new simulator (its `rand()` seeded 1) |
| `pollution_init` / `pollution_shutdown` | `expect?` | `Init` / `Shutdown` |
| `pollution_message` | `type` (`SimBegin`, `SimNewDay`, `SimNewMonth`, `OccupantInserted`, `OccupantRemoved`, `PostCityInit`, `TuningChanged`, `PreViewLevelChange` or an ID), `occupant?`, `view_level?` (PreViewLevelChange), `count?` (repeat), `expect?` | `DoMessage` |
| `pollution_call` | `method`, `args?` (occupants by label), `expect?` | a simulator method: `simulate_day`, `simulate_month`, `end_of_cycle`, `update_visual_effects`, `reload_tuning`, `add_radiation_to_city` (`[x, y, z, strength, radius]`), `dispose` (`[g]`, `remaining?: {wtp, inc, wte}`; result `{taken, remaining}`), `pollution_grid` (result the grid name or `null`), `pollution_value`, `air_value`, `water_value`, `garbage_value`, `average_*_value`, `is_air_polluted`, `is_water_polluted`, `is_garbage_accumulated`, `is_radioactive`, `worst_*_tract` (`[x, z, value]`), `garbage_supply`, `landfill_tile_capacity`, `garbage_capacity`, `is_garbage_uncollected`, `total_garbage_uncollected`, `set_is_radioactive`, `set_air_value`, `set_water_value`, `set_garbage_value`, `add_to_garbage_supply`, `set_garbage_capacity`, `monthly_incinerator_capacity`, `monthly_wte_capacity`, `recycling_center_effect`, `garbage_abducted_prev_month`, `garbage_{produced,imported,exported,recycled,incinerated,wte,landfilled}_last_month`, the four counts, `garbage_buildings` (labels), `building_garbage_capacity`, `garbage_scaling_factor`, `is_active_landfill`, `set_is_active_landfill`, `garbage_put_in_landfill`, `max_capacity` / `current_capacity` (`[prop, occupant]`, result `[found, value]`), `recycling_center_max_capacity`, `recycling_center_current_capacity`, `recycling_center_current_capacity_bits` (`[found, float32 bits]`), `current_occupant_condition`, `current_occupant_efficiency`, `incinerator_used_capacity`, `wte_used_capacity`, `incinerator_current_capacity`, `wte_current_capacity`, `water_treatment_{max,current,used}_capacity`, `calc_occupant_maintenance_cost`, `max_level_for_ui`, `requested_share` (`[x0, z0, x1, z1]` cells), `requester_satisfaction_id`, `uses_custom_prop_selector`, `select_prop`, `set_occupant_list_used_capacity` (`[list (0 water treatment, 1 incinerators, 2 waste-to-energy), total, used]`, §4.9.3) |
| `pollution_set` | `state` (scalar fields as in `pollution_expect`) | pokes state (test setup) |
| `pollution_expect` | `state` (partial: `initialized`, `loaded`, `cell_x`, `cell_z`, `tracts_per_day`, `iterator` (`mode`, `rect`, `count`, `index`, `table_len`, `exhausted`; partial), the §3.1 scalars in snake case (and `efficiency_bits`, the float32 bits), this month's counters `produced` … `landfilled` and last month's `last_produced` … `last_landfilled`, `worst_air` / `worst_water` / `worst_garbage` (`[x, z, value]`), `department` (label or `null`), the lists `wtp`, `incinerators`, `wte_plants`, `recycling` (labels), `blocks_x`, `blocks_z`, `grids` (eight booleans)) | the simulator state |
| `pollution_set_grid` | `grid` (`air_src`, `water_src`, `garbage_src`, `rad_src`, `air_val`, `water_val`, `garbage_val`, `rad_val`), `fill?`, `tracts?` / `cells?` (`[{x, z, w?, d?, v}]`; `v` may be a raw `"0x8064"`) | writes grid tracts directly (no dirty flags) |
| `pollution_expect_grid` | `grid`, `tracts?` / `cells?`, `others?` (every tract not listed), `nonzero?` (count), `size?` (tract counts) | grid contents |
| `pollution_expect_occupant` | `occupant`, `props?` (`{"0xID": value or null for absent}`), `writes?` (`{"0xID": count}`: how often the simulator wrote the property) | the occupant's properties |
| `pollution_expect_refs` | `refs` (`{"label": count}`; occupants start at 1, the department at 0) | host reference counts |
| `pollution_expect_effects` | `water?`, `air?`, `radiation?`, `garbage?` (effects the simulator holds), `live?` (effect objects not yet released) | visual effects |
| `pollution_crt` | `seed?`, `mark?`, `rand_calls?` (the simulator's `rand()` calls since the last mark) | the iterators' generator |
| `pollution_expect_fx` | `seed?` (default 1), `draws` | the effect generator equals a fresh one after `draws` draws |
| `pollution_save` / `pollution_load` / `pollution_expect_save` | `slot`; load: `expect?` (clean store load), `fx_seed?` | the class-object store (grids are saved as their own objects; occupants and the department are host objects that come back with a reference); a load makes the loaded simulator current |
| `pollution_read` | `slot`, `expect`, `into?` (`current` (default) or `new`) | `Read` of the slot's payload outside the store (the store ignores `Read`'s result) by the current simulator or a new one (made current); occupants and the department resolve to their saved tokens, each resolution handing over a reference |
| `pollution_patch` / `pollution_expect_payload` | see the shared conventions (the class is always the pollution simulator); `expect_payload` takes `hex?`, `at?` (`[{offset, hex}]`), `length?` | |

Pollution events: `add_agent` (`level`, `name`, `flags`), `remove_agent`, `subscribe` / `unsubscribe` (`types`),
`add_prop_requester`, `remove_prop_requester`, `post` (`type`: `RadiationChanged`, `AirChanged`, `WaterChanged`,
`GarbageChanged`, `DealStatus` or the ID; `data1`), `history` (`series`, `value`), `history_queue` (`series`,
`length`), `deal_used` (`deal`, `used`), `mark_deals_dirty`, `lot_condition` (`x`, `z`, `condition`),
`create_department` (`id`, `group`), `department_name` (`group`, `instance`), `max_funding` (`value`), `line_item`
(`line`, `local`, `group`, `instance`), `full_expenses` (`line`, `value`), `budget_add` / `budget_remove`
(`occupant`, `index`), `budget_cost` (`occupant`, `index`, `cost`), `line_expenses` (`line`, `delta`),
`effect_create` (`effect`, `id`, `kind`), `effect_position` (`effect`, `position`), `effect_param` (`effect`,
`param`, `value`), `effect_start` / `effect_stop` (`effect`, `t`), `effect_release` (`effect`).

## Crime ops

The crime vectors (`c17_…`) drive the crime simulator (`fc4_core::crime`) over a scripted city. Every op name
starts with `crime_`. The city is 64 × 64 cells (16 m) unless `crime_env` changes it; the tuning defaults to the
shipped exemplar and the type list to the 14 shipped types. Grids are addressed by tract `[x, z]`; lots, occupants
and props by integer tokens. The fake lot class (for saves) is `0x12345678`.

### Environment

| op | Arguments | Effect |
|---|---|---|
| `crime_env` | any of `cells` (`[x, z]`), `clear_zones`, `zones` (`[{x, z, w?, d?, v}]` cell zone types), `demand` (`{"0x4100": [value, max, min] \| null}`), `jail` (`GetJailPopulation`), `released` (`ResetUnjailedCriminals`), `ordinance` (the cumulative factor, `null` = no ordinance simulator), `disaster_layer` (default true), `police` (`{size?, fill?, cells?: [[x, z, v]]}` or `null`; default none), `ltext` (`[{key: [t, g, i], text, detokenized?}]`: what the host's §7 lookup finds; `detokenized: false` = the game's detokenizer returns false on it) | the fake world |
| `crime_residential` | `wealth` (0..2), `size?` (default 2), `pop` / `eq` (`[[x, z, v]]`), or `none` | that wealth's population (u16) and EQ (f32) grids |
| `crime_lots` | `lots` (`[{id, rect: [x1, z1, x2, z2], location? ([x, z], default the rect corner; `null` = GetLocation fails), condition?, building? ({token?, purpose, wealth}; default none), center? (default the rect middle)}]`), `random` (the `GetRandomLot` results in order, ids or `null`) | lots and the random-lot queue (an exhausted queue gives no lot) |
| `crime_occupants` | `occupants` (`[{id, preview?, effect? (u8 array for 0xCA5B9306, or a typed value such as `{"u32": [100, 8]}` handed over as its raw little-endian storage; a scalar is no bytes), pos}]`) | occupants for the inserted / removed messages |
| `crime_nearby` | `occupants` (`[{token, type, group?, incident_property?, pos, iid? (default true), state?}]`) | what the occupant manager's box query sees (it applies the x/z box) |
| `crime_props` | `props` (`{"0xPROP": crimeIncidentID}`) | prop profiles for `SelectProp` |
| `crime_types` | `shipped?` (default true), `types` (`[{id, props?}]`, property maps appended in order) | the crime type exemplars the next load reads |
| `crime_tuning` | `shipped`, `props` (merged over the shipped values when `shipped`), or `none` | the tuning exemplar |
| `crime_create` | `rng?`: `{seed}` (a real `cRZRandomSuper`) or `{stub: {uniform, range, uint}}`; `new_process?` | a new simulator; the process state (type list, generator) is shared with earlier simulators of the vector unless `new_process`; `rng` replaces the generator |
| `crime_rng` | `seed` or `stub` | replaces the shared generator |

The stub generator logs `rng` events: `uniform` (`value`), `range` (`lo`, `hi`, `value` = `lo + u(hi − lo)`),
`uint` (`n`, `value`). Each queue repeats its last value (defaults 0.5, 0.5, 0); `uint` values are returned as
they are.

### Calls and checks

| op | Arguments | Effect |
|---|---|---|
| `crime_init` / `crime_shutdown` | `expect?` | `Init` / `Shutdown` |
| `crime_message` | `type` (`SimNewDay`, `SimNewMonth`, `SimIdle`, `OccupantInserted`, `OccupantRemoved`, `OrdinanceUpdated`, `ExemplarChanged` or an ID), `data1?`, `data2?`, `occupant?` (void1), `count?`, `expect?` | `DoMessage` |
| `crime_call` | `method`, `args?`, `expect?` | `do_idle`, `do_census`, `calculate_economic_factors`, `compute_criminal_base`, `compute_crimes_to_commit`, `decay_crime_grid`, `compute_average_crime`, `load_types`, `read_tuning`, `add_crimes` (`[n]`), `add_crime_event` (`[lot]`, the severity), `lot_bits` (`[lot]` → `{zone, wealth, purpose, condition}`), `average_crime`, `monthly_crime_count`, `crime_hotspot` (`[v, x, z]` or false), `nearest_sim` / `nearest_prop` (`[[x, y, z], r]` → token or null), `incident_array` (`[initial length]` → `{result, events: [{cell, type, visuals, icon}]}`; pre-filled elements have type 0xFFFFFFFF), `set_visuals_created`, `visuals_created`, `simulator_type`, `requested_share` (`[x1, z1, x2, z2]`), `requester_satisfaction_id`, `uses_custom_prop_selector`, `select_prop` (`[[props], cx, cz]`), `curve` (`[name, x, wealth?]`, names `eq`, `jobless`, `crimes`, `attenuation`, `prop_request`) |
| `crime_eval` | `fn`, `args`, `expect` | pure functions: `bucket` (`[eq]`, null outside 0..3), `jobless` (`[a, b, c]`), `decay` (`[v, decay]`), `mask` (`[bytes \| null]`), `lot_bits` (`[zone, [purpose, wealth] \| null, condition]`), `choose` (`[weights, r]`), `rhe` (`[x]`, x a number or a hex string of float32 bits) |
| `crime_set` | `state`: any of `hotspot` (`[x, z, v]`), `average_crime`, `ordinance_factor`, `monthly_crime_count`, `state`, `criminal_base`, `crimes_to_commit`, `loaded_from_save`, `census`, `jobless`, `incidents` (`[{lot (token or null), type, name?, icon?, visuals?}]`, no AddRef) | pokes state |
| `crime_expect` | `state`: `initialized`, `loaded_from_save`, `has_message_server`, `has_crime_grid`, `has_effect_grid`, `state`, `criminal_base`, `crimes_to_commit`, `census`, `jobless`, `average_crime`, `average_crime_bits` (hex float32 bits), `ordinance_factor`, `monthly_crime_count`, `hotspot`, and, matched partially, `incidents`, `iterator` (`state`, `mode`, `dirty`, `rect`, `count`, `table`, `index`, `current`, `step`), `tuning` (`tract_size`, `decay`, `reoffend`, `prop_request_empty`), `types` (`id`, `name`, `icon`, `zone`, `wealth`, `purpose`, `condition`, `severity`, `arson`, `weight`; null before the first load) | the simulator state |
| `crime_grid_set` / `crime_expect_grid` | `grid` (`crime`, `effect`); set: `fill?`, `cells?`; expect: any of `counts`, `size`, `instance`, `cells`, `others` (every unlisted tract) | grid contents |
| `crime_save` / `crime_load` / `crime_expect_save` | `slot`; load: `expect?` (clean store load) | the class-object store; lots come back as the tokens they had; a load makes the loaded simulator current |
| `crime_read` | `slot`, `expect`, `position?`, `into?` (`new` (default) or `current`) | `Read` of the slot's payload outside the store (the store ignores `Read`'s result) by a new simulator (made current) or by the current one |
| `crime_patch` / `crime_expect_payload` | see the shared conventions (the class is always the crime simulator) | |

Crime events: `load_types`, `agent` (`name`, `levels`, `flags`), `remove_agent`, `subscribe` / `unsubscribe`
(`types`), `acquire_services`, `release_services`, `add_prop_requester`, `create` (`instance`, `size`),
`grid_shutdown` (`instance`), `jail_population`, `reset_unjailed`, `crime_stats`, `add_ref` / `release` (`lot`),
`ignite` (`building`, `cause`), `post` (`type`: `CrimeCommitted` with `crime`, `lot`, `name`, or
`CrimeMapUpdated`), and with a stub generator `rng`.

## Aura ops

The aura vectors (`a17_…`) drive the aura simulator (`fc4_core::aura`) with a scripted city implementing
`AuraHost`: lots (rectangles, zone, building occupant, wealth, populations), occupants and their Sint32-array
properties, budget funding, tax rates, the ordinance offset, the land value type, the grids of other simulators,
the limit timer and the registry. Every op name starts with `aura_`. The default city is 64 × 64 cells of 16 m
(16 × 16 tracts at S = 4; the work grid has one tract per cell). Grid cells are `[[x, z, v], …]`.

### Environment

| op | Arguments | Effect |
|---|---|---|
| `aura_env` | any of `cells` (`[x, z]`), `cell_width` (`[x, z]` m), `date` (default 1000), `tuning` (`"shipped"` (default), `"empty"`, or null = no exemplar), `tuning_props` (a property map merged into the tuning), `tuning_remove` (property IDs), `funding` (`{"0xID": f, "nan" or null}`; Parks and Recreation is `0x8A4260A6`), `tax` (`{"index": rate}`, default 0), `ordinance_offset`, `land_value_type` (default 1), `timer_expired` (default false), `timer_reads_before_expiry` (n or null: the clock answers "not expired" to the first n reads after each timer start, then "expired"; overrides `timer_expired`), `log_timer_reads` (log every clock read), `registry_ok` (default true), `lots` (`[{rect: [x1, z1, x2, z2], zone? (1), building? (occupant ID, 0 by default, null = none), wealth? (1), pops? ([[type, count]])}]`, or null), `occupants` (`{"id": {type?, pos: [x, y, z], props: {"0xID": [ints]}, dept?} or null}`), `grids` (`{name: {size? (4), fill?, cells?} or null}` for `air`, `water`, `garbage`, `radiation`, `school`, `hospital`, `crime`, `trip`, `congestion`, `pop1`..`pop3`) | the scripted city |
| `aura_create` | `rng?`: `{seed}` (a real `cRZRandom`) or `{stub: {uniform: [u…], int: [k…]}}` (a double draw returns `a + u (b − a)`, an integer draw returns `k`; the last value repeats); default a clock-seeded generator. Draws are logged | a new simulator |
| `aura_rng` | as `rng` above | replaces the generator |

### Calls and checks

| op | Arguments | Effect / check |
|---|---|---|
| `aura_init` / `aura_shutdown` | `expect?` | `Init` / `Shutdown` |
| `aura_message` | `type` (`SimBegin`, `SimNewDay`, `SimNewMonth`, `SimIdle`, `PostCityInit`, `OccupantInserted`, `OccupantRemoved`, `OrdinanceUpdated`, `ExemplarChanged` or an ID), `data1..3?`, `occupant?`, `expect?` (the `DoMessage` result) | `DoMessage` |
| `aura_idle` | `ms?` (5), `count?` (1), `expect_state?` | `DoIdle` calls |
| `aura_run` | `ms?`, `max?`, `expect_calls?` | `DoIdle` until state 0 |
| `aura_call` | `method`, `args?`, `expect?` | `add_transient_effect` (`[x, z, radius, magnitude]`), `add_transient_effect_type` (`[x, z, type]`), `aura_value` (`[x, z]` cells), `mayor_rating`, `max_mayor_rating` / `min_mayor_rating` (`[rating, x, z]`), `bust_strike` (`[type]`), `registry` (`command`, `args`: floats are Float32 variants, integers Sint64), `occupant_changed` (`[id, add]`), `centre_cell` (`[x, z]` m), `lot_base_aura` (`[lot]` as in `lots`), `tax_change`, `read_tuning`, `query_interface` (`[iid]` → `"primary"`, `"message_target"`, `"serializable"` or false), `class_id`, `slot_pops` (`[slot]`) |
| `aura_set` | `state`: any of `state`, `cycle_start_day`, `department_cursor` (null = none), `mayor_rating`, `unused_rating`, `global_effect`, `effective_base_aura`, `base_aura`, `hotspot`, `long_decay`, `short_decay`, `max_rating`, `min_rating`, `max_location`, `min_location`, `low_tracts`, `hotspots` (`[[x, z, w]]`), `re_add_landmarks`, `previous_tax_rate`, `slice_step` (`[step, misses]`) | pokes state (test setup) |
| `aura_expect` | `state`: the `aura_set` fields plus `initialized`, `riot_roll`, `transient_magnitude`, `transient_radius`, `curves` (partial `{park, air, water, garbage, radiation, school, hospital, crime, trip, congestion, tax_change, value_wealth, riot}` booleans: whether the curve exists), `departments` (IDs, ascending), `lot_iterator` | the simulator state |
| `aura_set_grid` | `grid` (`aura`, `base`, `transient`, `work`, `building_effect`, `park_map`, `raw_park`, `landmark_map`, `landmark_work`, `dept:0xID`), `fill?`, `cells?` | writes a grid; a missing department grid is first created through a landmark occupant with no cone (§7.2) |
| `aura_expect_grid` | `grid` and any of `exists`, `tract_size`, `counts`, `cells`, `others` (every unlisted tract) | grid contents |
| `aura_expect_events` / `aura_expect_event_count` / `aura_clear_events` | see the shared conventions | host calls |
| `aura_save` / `aura_load` / `aura_expect_save` | `slot`; load: `rng?`, `expect?` (the store-level result; the simulator's Read result is ignored, foundations §3.5), `into_current?` (read the record into the current simulator instead of a new one) | the class-object store |
| `aura_patch` / `aura_expect_payload` | see the shared conventions (the simulator's record) | |

Aura events: `create_grid` (`type` i8/i16, `instance`, `size`), `base_aura` (`values`), `lot_iterator`, `history`
(`series`, `value`), `disaster` (`manager`, `x`, `z`), `strike` (`service`: police, fire, health, school), `post`
(`type`), `timer_start` (`ms`), `timer_read` (`lots_taken`: lots taken from the current iterator; only with `log_timer_reads`), `subscribe` / `unsubscribe` (`types`), `add_agent` (`level`, `name`, `flags`),
`remove_agent`, `registry_add`, `registry_remove`, and `rng` (`kind` double with `value`, or int with `n` and
`value`).

## Power ops

The power vectors (`pw15_…`) drive the power simulator (`fc4_core::power`) with a scripted city implementing
`PowerHost`: occupants (buildings, poles, lines, network pieces) with their properties, footprints and budget
items; lots; zones; the budget's departments and line items; neighbour deals and PowerPole connections; the
ordinance factor; the 3D view's input control. Every op name starts with `power_`. The default city is 256 × 256
cells of 16 m with the shipped "Utilities" tuning and factor 1.0. Host objects are numbers (occupant, lot, deal and
connection IDs as given); departments and line items get tokens from `0x10001` on (the simulator itself takes one
token per `power_create`). IDs in events are exact `"0x%08X"` strings (numbers compare with a relative tolerance,
which would let neighbouring IDs match).

### Environment

| op | Arguments | Effect |
|---|---|---|
| `power_env` | any of `cells` (`[x, z]`), `cell_width` (`[x, z]` m), `factor` (the ordinance factor), `sim_time`, `department_ok` (whether `CreateDepartmentBudget` succeeds), `view_control` (ID or null), `zones` (`[{x, z, w?, d?, zone}]`, zone 0 clears), `occupants` (`[{id, type ("building", "pole", "line", "network" or an ID), cell? ([x, z], poles/lines/network pieces), cells? ([x1, z1, x2, z2], default the cell), bbox? ([[x, y, z], [x, y, z]], default the cells × 16), pos? (default the bbox centre), props? (a property map), lot?, building_type? (null = not a building occupant; default 0x1234 for buildings), qi? (true; the type's QueryInterface), ignored? (pole slot 13), preview? (line), flags? (network flags), poles? ([a, b], a line's connected poles), query_rect? ([x1, z1, x2, z2]: the rect the occupant-manager query matches it by, default its cells), in_manager? (true), distressed?, budget? ({department? (0x8910BC8A), line? (0), cost? (0), index? (0), funding? (1.0)})}]`), `pole_set` (`{first, count, cost? (1), origin? ([x, z])}`: `count` poles with consecutive IDs at origin + (i mod 50, i div 50)), `occupant_props` (`{"id": {"0xID": value or null}}`), `occupant_state` (`{"id": {in_manager?, distressed?, ignored?, budget? (or null)}}`), `lots` (`[{id, rect, state? (0), needs_power?, has_power?}]`), `departments` (`[{id, funding? (1.0), budget_funding? (0), lines? ([line or {line, secondary?}…])}]`; an existing department is updated, an existing line item is kept and only its `secondary` is set), `deals` (`[{id, type ("buy", "sell" or an ID), city, amount, used? (0)}]` or null; replaces the list), `connections` (`[{id, city, cell, visited?, active?, other? (the connection on the other side)}]` or null; replaces the list) | the scripted city |
| `power_tuning` | `none: true` (no exemplar), or `shipped?` (true; false starts empty), `props?` (merged property map), `remove?` (IDs) | the tuning exemplar |
| `power_create` | `crt_seed?` (C `rand()` seed, default 1) | a new simulator (the old one is dropped without `Shutdown`) |

### Calls and checks

| op | Arguments | Effect / check |
|---|---|---|
| `power_init` / `power_shutdown` | `expect?` | `Init` / `Shutdown` |
| `power_message` | `type` (`SimNewMonth`, `SimNewDay`, `SimIdle`, `OccupantInserted`, `OccupantRemoved`, `ZoningChanged`, `TuningReload`, `TerrainChanged`, `OrdinanceUpdated`, `StateChanged`, `DealStatus`, `ConnectionsReady`, `PostCityInit` or an ID), `data1..4?`, `occupant?` / `void1?` (the IGZUnknown), `occupants?` (`[id…]` or `{first, count}`: the message once per occupant), `rect?` (zoning), `count?`, `expect?` (the `DoMessage` result) | `DoMessage` |
| `power_idle` | `count?` (1), `expect_state?` | `DoIdle` calls |
| `power_cycle` | `recompute?` (request one first), `count?` (repeat), `max?` (16), `expect_calls?` | `DoIdle` until the state is back at 4 after leaving it |
| `power_call` | `method`, `args?`, `expect?` | slots: `position_to_cell` (`[x, z]` m), `cell_to_position`, `cell_to_standard_city_cell`, `annual_consumption`, `annual_consumption_by_type`, `monthly_requirement`, `monthly_production`, `monthly_import`, `monthly_export`, `monthly_demand`, `monthly_consumption_at` (`[x, z]`), `monthly_consumption`, `supply_value` / `is_powered`, `is_wired`, `unpowered_consumer_count`, `powered_consumer_count`, `power_plant_count_of_type`, `power_plant_count`, `power_plants` (IDs; each AddRef'ed), `power_pole_count`, `overworked_plant_count`, `aged_plant_count`, `distressed_plant_count`, `find_power_plants` (`{lowest: [id, funding] or null, worn: [id, pct] or null}`), `is_occupant_powered`, `occupant_power_consumed` / `occupant_max_power_produced` (value or false), `occupant_power_produced` (`[produced, used]` or false), `is_on_strike`, `begin_strike`, `end_strike`, `chance_of_strike`, `strike_start_date`, `strike_length`, `current_plant_efficiency`, `current_plant_condition`, `current_power_produced`, `strike_efficiency`, `funding_percent` (null without a department), `monthly_generation_cost`, `plant_funding_percentage` (false without a budget item), `set_plant_funding_percentage` (`[id, fraction]`); internals: `update_power_grid` (`[x, z]`), `flood` (`[x, z]` → `[rowMin, rowMax]`), `age_plant`, `apply_pole_funding` (`[fraction]`), `monthly`, `do_idle`, `occupant_inserted`, `occupant_removed`, `cell_changed` (`[rect]`), `expand` (`[rect]`), `read_tuning`, `curve` (`[name, x]`, name `funding_efficiency`, `funding_decay`, `usage_decay`, `pole_decay`, `strike_response`); identity: `query_interface` (`[iid]` → `"primary"`, `"message_target"`, `"serializable"` or false), `class_id`, `slot_pops` (`[slot]`) |
| `power_set` | `mark` / `wire` / `powered` / `next` (cells `[x, z]` or rects `[x1, z1, x2, z2]` to set), `consumption` (`[[x, z, v]]`), `state`: any of `enabled`, `initialized`, `running`, `recompute`, `age_plants`, `wire_dirty`, `state`, `ring_index`, `production`, `consumption`, `import`, `export`, `demand`, `consumers`, `powered`, `unpowered`, `requirement`, `cost_base`, `deficit`, `broken_fraction`, `broken_count`, `pole_count`, `factor`, `strike_start`, `strike_flag`, `unused_flag`, `radius`, `annual`, `ring_row` (the current row), `per_type` | pokes state (test setup) |
| `power_expect` | `state`: the fields above plus `department` (token or null), the `last_*` values, `plants` (`[[visited, id]]`), `plant_count`, `poles`, `dirty_rect`, `changed`, `overworked`, `aged`, `distressed`, the tuning scalars (`max_funding`, `strike_efficiency`, `max_production_ratio`, `min_acceptable_funding`, `strike_duration`, `min_funding_end_strike`), their exact f32 bits as `"0x%08X"` strings (`max_funding_bits`, `strike_efficiency_bits`, `max_production_ratio_bits`, `min_acceptable_funding_bits`, `min_funding_end_strike_bits`) and `deficit_bits`, `map_size` (`[rows, columns]`), `bitmap_words`, `has_grid` | the simulator state |
| `power_expect_bitmap` | `bitmap` (`mark`, `wire`, `powered`, `next`), `cells?` (set), `rects?` (all set), `unset?`, `count?` (set bits), `exact?` (no other bit set) | a bitmap |
| `power_expect_grid` | any of `exists`, `instance`, `tract_size`, `counts`, `cells` (`[[x, z, v]]`), `rects` (`[[x1, z1, x2, z2, v]]`), `others` (every unlisted cell) | the powered grid |
| `power_expect_occupant` | `occupant`, `props?` (`{"0xID": value or null}`), `distressed?` | an occupant's properties as the simulator left them |
| `power_expect_world` | any of `lots` (`{"id": {has_power}}`), `deals` (`{"id": {used, flags}}`), `connections` (`{"id": {visited, active}}`), `line_items` (`[{department, line, exists?, expenses?, secondary?}]`), `distressed` (count of distressed poles), `departments` (IDs), `department_lookups` (`{"0xDEPT": n}`: `GetDepartmentBudget` calls since the last `power_clear_events`) | host objects |
| `power_expect_rand` | `calls` | C `rand()` calls since the last check (or `power_create` / `power_load`) |
| `power_expect_refs` | `refs` (`{"token": count}`, counts start at 0) | host reference counts |
| `power_expect_events` / `power_expect_event_count` / `power_clear_events` | see the shared conventions | host calls |
| `power_save` / `power_load` / `power_expect_save` | `slot`; load: `expect?` (the store-level result), `crt_seed?` | the class-object store; host objects in the save are an occupant class `0x0CC00001` and a department class `0x0CC00002` |
| `power_patch` / `power_expect_payload` | see the shared conventions (the simulator's record) | |
| `power_read` | `slot` (the saved record) or `hex`, `expect?` | calls `Read` of the current simulator directly (the store ignores its result) |

Power events: `acquire_services`, `release_services`, `factor` (`id`, `index`), `add_agent` (`level`, `name`, `flags`),
`remove_agent`, `subscribe` / `unsubscribe` (`types`), `post` (`type`, `data`, `unknown`), `history` (`series`,
`value`), `history_queue` (`series`, `length`), `create_grid` (`instance`, `size`), `set_property` (`occupant`, `id`,
`value`), `break_connections` (`pole`, `flag`), `distressed` (`pole`, `on`), `remove_connection` (`pole`, `other`: `pole.RemoveConnection(other, 0)`),
`occupants_in_cells` (`rect`), `remove_occupant`, `tool_zone_change` (`rect`, `data3`), `lot_has_power` (`lot`, `on`),
`lot_condition` (`lot`, `condition`), `demolish` (`lots`, `flags`), `radiation` (`position`, `strength`, `radius`),
`remove_view_control`, `deal_used` (`deal`, `used`), `deal_flags` (`deal`, `flags`, `on`), `mark_deals_dirty`,
`conn_active` (`conn`, `on`), `create_department` (`id`, `group`), `department_name`, `max_funding` (`department`, `value`, `bits`),
`create_line_item` (`department`, `line`, `local`), `create_line_item_for_type`, `line_item_name`, `display_flag`,
`secondary_info` and `full_expenses` (`item` = `[department, line]`, `value`), `budget_line`, `budget_cost`,
`add_funded` / `remove_funded`, `local_funding`, `local_full_funding`.

## Plumbing ops

The plumbing vectors (`pl14_…`) drive the plumbing (water) simulator (`fc4_core::plumbing`) over a scripted city
implementing `PlumbingHost`: cells, terrain water, water pollution values, the water department's funding and the
ordinance factor; building and pipe occupants (properties, lots, network flags, `ConnectsToNeighbor` directions,
budget items); neighbour deals and connections; and the budget (departments, line items). Every op name starts
with `plumbing_`. The default city is 64 × 64 cells with the shipped tuning, funding 1.0, ordinance factor 1.0 and
wall-clock seed 1. Objects are named by label; host objects in a save use the test-only classes `0x0CC00011`
(occupant) and `0x0CC00012` (department).

### Environment and objects

| op | Arguments | Effect |
|---|---|---|
| `plumbing_env` | any of `cells` (`[x, z]`), `funding`, `factor` (null = no ordinance simulator), `water` (`[[x, z]]`), `water_rects` (`[{x, z, w, d}]`), `clear_water`, `water_values` (`[{x, z, v}]`), `no_terrain`, `no_disaster`, `no_department`, `no_grids`, `log_water` (log every `CellHasWater` call as a `has_water` event), `wall_seed`, `departments` (`[{label, id}]`) | the scripted city |
| `plumbing_tuning` | `none` (no exemplar), `code` (an empty exemplar), or the shipped exemplar plus `props` (`{"0xID": value}`; `{"remove": true}` removes one); `reload?` sends the tuning message | the tuning exemplar |
| `plumbing_building` | `label`, `x`, `z`, `w?`, `d?`, `anchor?`, `props?`, `lot?` (default true: a lot `<label>_lot` over the rectangle), `lot_rect?`, `needs_power?`, `has_power?`, `building_type?`, `budget?` (`{department, line, cost, index?}`), `local_funding?`, `insert?` (default true) | a building occupant, inserted through the message |
| `plumbing_pipe` / `plumbing_pipe_row` / `plumbing_pipes` | pipe: `label`, `x`, `z` (or a rectangle), `dirs?`, `flags?`, `insert?`; row: `prefix`, `from`, `to` (connected to each other); pipes: `prefix`, `count`, `width?`, `x0?`, `z0?` (unconnected) | pipe occupants |
| `plumbing_occupant` | `occupant`, `props?`, `net_flags?`, `budget?` (null removes it), `local_funding?` | changes an occupant |
| `plumbing_lot` | `lot`, `needs_power?`, `has_power?`; or a new lot with `label` and a rectangle | lots |
| `plumbing_deal` / `plumbing_connection` | deal: `label` or `deal`, `type` (`import`, `export` or a number), `amount`, `used?`, `city`, `flags?`; connection: `label`, `x`, `z`, `city`, `active?`, `visited?`, `other?` | neighbour deals and connections |

### Calls and checks

| op | Arguments | Effect / check |
|---|---|---|
| `plumbing_create` / `plumbing_init` / `plumbing_shutdown` | `expect?` | a new simulator, `Init`, `Shutdown` |
| `plumbing_rng` / `plumbing_gaussian` | rng: `seed`; gaussian: `seed`, `calls` (`[{lo, hi, expect}]`), `raws?` | the generator; `RandomSint32RangeGaussianFast` on a fresh one |
| `plumbing_slice` | `cells` per call in states 6 and 7 (null = the whole map) | the time slice ([choice]) |
| `plumbing_message` | `type` (`SimNewMonth`, `SimNewDay`, `SimIdle`, `ConnectionsReady`, `InsertOccupant`, `RemoveOccupant`, `TuningReload`, `PipeBurst`, `DealStatus` or an ID), `data1..3?`, `occupant?`, `deal?`, `expect?` | `DoMessage` |
| `plumbing_insert` / `plumbing_remove` | `occupant` | the occupant messages |
| `plumbing_idle` | `count?` (1), `ms?` (5), `expect?` | `DoIdle` calls |
| `plumbing_pass` | `calls?` | starts a pass and idles until state 9 |
| `plumbing_month` | `expect?` | the month message |
| `plumbing_call` | `method`, `args?`, `expect?` | the §10 slots (`query_interface`, `add_ref`, `release`, `supply_value`, `supply_value_map_ptr`, `are_watered_buildings_in_cell`, `is_building_watered`, `water_production`, `is_cell_watered`, `num_water_sources`, `num_polluted_water_sources`, `water_production_capacity`, `water_consumed`, `water_imported`, `water_exported` (EAX), `num_pipes`, `num_distressed_pipes`, `num_burst_pipes`, `num_watered_buildings`, `num_unwatered_buildings`, `water_demand` (EAX), `water_demand_64`, `water_producer_info`, `water_producer_condition_percent`, `occupant_monthly_upkeep_cost`, `find_water_buildings`, `water_producers`), `slot_return` (`[slot]`), and the steps `nearest_water` (`[x, z, type, reach]`), `nearest_water_ring`, `update_water_production`, `age_water_producers`, `building_consumption`, `is_connected_to_current_network`, `pipe_cost`, `trunc`, `eax_of` |
| `plumbing_set` | `state` (any of `state`, `recompute`, `ageing`, `enabled`, `pipe_condition`, `num_pipes`, `num_distressed`, `num_burst`, `demand`, `pass_counter`, `total_water_cost`, `last_produced`, `last_consumed`, `last_imported`, `last_exported`), `flags?` (`[{x, z, v}]`), `production?` (`[{occupant, production}]`), `crt?` | pokes state (test setup) |
| `plumbing_expect` | `state`: a partial object (nested objects match partially) of the state fields (`state`, `recompute`, `initialized`, the counters and totals, `flag_map_size`, `flag_map_nonzero`, `producers`, `current`, `pipes`, `leaked`, `iterator` (`state`, `x`, `z`, `step`, `rect`), the grids, `rng_lcg`, `crt_state`, …) | the simulator state |
| `plumbing_expect_tuning` | the scalar fields (and `max_funding_bits`, the hex float32 bits), `eval?` (`[{curve, x, y}]`), `empty?` | the tuning |
| `plumbing_expect_flags` / `plumbing_expect_grid` | flags: `cells`; grid: `grid` (`published` or `working`), `instance?`, `cells?`, `rects?`, `nonzero?` | the flag map and the u8 grids |
| `plumbing_expect_occupant` / `_deal` / `_connection` / `_line_item` / `_refs` | occupant: `props?` (null = absent), `net_flags?`, `budget_cost?`; deal: `used?`, `flags?`; connection: `active?`, `visited?`; line item: `line`, `expenses?`, `secondary?`, `display?`, `exists?`; refs: `{label: count}` | host objects |
| `plumbing_expect_events` / `plumbing_expect_event_count` / `plumbing_clear_events` | see the shared conventions | host calls |
| `plumbing_save` / `plumbing_load` / `plumbing_expect_save` | `slot`; load: `expect?` (the store-level result; the simulator's Read result is ignored, foundations §3.5) | the class-object store |
| `plumbing_patch` / `plumbing_expect_payload` | see the shared conventions (the simulator's record) | |

Plumbing events: `agent`, `remove_agent`, `subscribe`, `unsubscribe`, `post` (`type`, `data1`), `history` (`id`,
`value`), `create_department`, `department_name`, `max_funding` (`value`, `bits`), `create_line_item`, `line_item_name`,
`remove_line_item` (`line`), `create_building_line_item`, `display_flag`, `add_expenses` (`line`, `delta`),
`set_expenses`, `set_secondary`, `change_budget_line`, `change_budget_cost`, `pipe_watered` (`pipe`, `watered`),
`redisplay` (`min`, `max`), `burst` (`x`, `z`, `manager`), `lot_condition` (`lot`, `condition`), `set_used`,
`deal_flags`, `deals_dirty`, `create_grid` (`instance`, `size`), `grid_shutdown` (`grid`) and, with `log_water`,
`has_water` (`x`, `z`).

## Protection ops

The protection vectors (`pr18_…`) drive the fire and police simulators (`fc4_core::protection`) with a scripted
city implementing `ProtectionHost`: occupants (type, position, prop flag, lot road access, station effect
properties, a property map, budget items), lots, zoned cells, dispatches, departments and their funding, the
police coverage percentage, sea level and altitude, the disaster flag and the date. Every op name starts with
`protection_`. The default city is 64 × 64 cells of 16 m (32 × 32 tracts at S = 2). Grid cells are
`[[x, z, v], …]`; IDs may be written as `"0x…"` strings.

### Environment

| op | Arguments | Effect |
|---|---|---|
| `protection_env` | any of `cells` (`[x, z]`), `day` (default 1000), `police_pct` (the police simulator's percent coverage, or null), `sea_level` (250), `altitude` (300), `disaster`, `departments_fail`, `dept_funding` (`{"0xID": f}`, default 1.0), `tuning` (`"shipped"` (default), `"empty"`, or null = no exemplar), `tuning_props` (a property map merged into the tuning), `zones` (`[{x, z, w?, d?, v?}]` cells), `lots` (`{"id": [x, z] or null}`, the lot's cell), `dispatches` (`[{id, station, call, active, pos: [x, z]}]`; the station's dispatch effect draws the cone, protection §7.2) | the scripted city |
| `protection_occupants` | `{"id": {type?, pos?: [x, y, z], prop?, road? (true/false = a lot with/without road access, null = no lot), station?: [kind, centre, outer, radius, units] (writes the kind's effect, unit count and dispatch properties), props?: {"0xID": {"u32" / "i32" / "f32": value or array} or null}, items?: {"0xPURPOSE": {local, funded? (true), cost?, dept? (true; false = no department; or a department ID)} or null}} or null}` (merged into existing occupants) | occupants |
| `protection_create` | `kind` (`fire` or `police`), `rng?`: `{stub: {uniform: [u…], range: [u…]}}` (a range draw returns `a + u (b − a)`; the last value repeats) or `{seed}` | a new simulator; the shipped tuning when none is set |

### Calls and checks

| op | Arguments | Effect / check |
|---|---|---|
| `protection_grids` | `size` (the grids' tract size) | replaces both coverage grids of the current simulator with grids of that tract size on the same city (to show that the simulator uses its own S = 2) |
| `protection_init` / `protection_shutdown` | `expect?` | `Init` / `Shutdown` |
| `protection_message` | `type` (`SimBegin`, `SimNewDay`, `SimNewMonth`, `OccupantInserted`, `OccupantRemoved`, `OccupantChanged`, `FundingChanged`, `ReloadTuning`, `DispatchBegin`, `DispatchActive`, `DispatchInactive`, `DispatchEnd`, `Crime` or an ID), `data1?`, `data2?`, `occupant?`, `unknown?` (the IGZUnknown: an occupant or lot ID), `count?` (repeat), `expect?` | `DoMessage` |
| `protection_call` | `method`, `args?`, `expect?`, `bits?` | a simulator method by its snake-case name (for example `station_efficiency [occ]`, `station_effect [occ, record?]` (`record` `{x, z, radius, centre, outer, original_centre}` is the caller's record, default zeroed), `funding_of [occ, purpose]`, `jail_funding [occ]`, `jailed_percentage_bits`, `station_stats [occ]` (padded to 4), `max_possible_dispatch_count`, `dispatch_from_station [occ, id]`, `occupant_coverage [occ, p, cover_before?, station_before?]`, `striking_stations [list_len]`, `jailbreak_buildings [list_len]`, `dispatches_at_station [occ, flags?, base?]`), plus `add_ref`, `release`, `slot [n]` (name, pops, return convention), `slot_count`, `query_interface [iid]`, `class_id`, `curve [funding/strike/jail/jailbreak, x]`, `tuning` and `read_tuning` |
| `protection_eval` | `fn`, `args`, `expect` | the pure helpers: `adjusted_value`, `scale_effect`, `occupant_coverage`, `combine_strike_chance`, `unit_mask`, `dispatch_mask`, `revoke_mask`, `base_call_number`, `rehabilitation`, `reset_unjailed`, `jailed_percentage` (NaN is `"nan"`), `jailed_percentage_bits`, `pct` (x × 0.01f), `jailbreak_ends` (`[percentage, threshold]`), `jailbreak_starts` (`[chance, draw]` → `[draw taken, starts]`), `ratio_share`, `escapees`, `jail_capacity`; float arguments of `jailbreak_ends` / `jailbreak_starts` may be `"0x…"` float32 bit patterns |
| `protection_set` | `state`: any of `tracts_per_day`, `factor`, `counters` (`[zoned, sum, nonZero]`), `avg`, `pct`, `strike_flag`, `strike_date`, `next_call`, `capacity`, `population`, `jailed`, `arrests_last`, `arrests_this`, `unarrested_this`, `crimes_last`, `crimes_this` | pokes state (test setup) |
| `protection_expect` | `state`: the `protection_set` fields plus `initialized`, `ref_count`, `destroyed`, `loaded`, `department`, `jail_department`, `purpose`, `stations`, `air_source`, `jails`, `funded_jails`, `jailbreaks`, `crt` (the rand() state) and `iterator` (partial `{state, mode, rect, count, index, current, step}`) | the simulator state |
| `protection_expect_occupant` | `id`, `props` (`{"0xID": value or array}`) | occupant properties |
| `protection_expect_refs` | `all_zero?`, `refs?` (`{"id": count}`) | outstanding host references |
| `protection_grid_set` / `protection_expect_grid` | `grid` (`adjusted` or `unadjusted`); set: `cells`; expect: any of `exists`, `size`, `counts`, `instance`, `cells`, `others` | the coverage grids |
| `protection_expect_events` / `protection_expect_event_count` / `protection_clear_events` | see the shared conventions | host calls |
| `protection_save` / `protection_load` / `protection_expect_save` | `slot` | the class-object store (load recreates the simulator, its grids and the host objects) |
| `protection_read` | `slot` or `hex`, `expect`, `position?`, `reuse?` | `Read` into a new simulator, or with `reuse` into the current one (a re-read); `position` is the bytes consumed |
| `protection_patch` / `protection_expect_payload` | see the shared conventions (the simulator's record) | |

With `"bits": true` on a `protection_call` or `protection_eval`, every float in the result becomes its float32 bit
pattern (`"0x3DCCCCCC"`), compared exactly; other floats compare with the usual relative tolerance.

Protection events: `acquire_services` / `release_services` (`kind`), `create_grid` (`type` u8/i16, `instance`,
`size`), `set_instance`, `shutdown_grid`, `add_agent` (`level`, `name`, `flags`), `remove_agent`, `subscribe` /
`unsubscribe` (`types`), `register_dispatch` / `unregister_dispatch`, `department` (`id`, `group`, `name`, `max`),
`add_funded` / `remove_funded` (`dept`, `occupant`, `purpose`), `set_funding` (also `p`), `add_controllers` /
`remove_controllers` (`group`: `striking_fire`, `striking_police`, `jailbreak`; `occupant`), `history`
(`series`, `value`), `post` (`type`, `subject?`), `air_create`, `air_init` / `air_shutdown` (`source`), and `rng`
(`kind` uniform or range with `lo`, `hi`, `value`).

## Flora ops

The flora vectors (`fo16_…`) drive the flora simulator (`fc4_core::flora`) with a scripted city implementing
`FloraHost`. Every op name starts with `flora_`. The default city is 1024 m with 64 × 64 cells (cell =
floor(position / 16)), sea level 250 m, flat ground 260 m (normal.y 1), moisture type 8, no lots, the terrain
initialized, an occupant manager that accepts inserts, the shipped "Flora Tuning Parameters" (burn rate 1, removal
step 1, period 24, wild seeds 0, elevation scale 800) and a process-wide registry shared by every simulator of the
vector (`flora_registry reset` starts a new process). Occupants are numbers; the simulator's new occupants get IDs
from 1000 on. A flora occupant's bounding box is its position ± its record's half width (1) in x and z and
[y, y + height (3)], unless `bbox` is given. The occupant manager lists only inserted occupants (flag 1); its box
query is the original's (§6.4.2): an occupant is returned when the 64 m index cells it is listed in (its box,
inset by 0.1 on x and z where wider than 1 m) meet the query's cells (clamped to 0..191) and its box overlaps the
query box strictly on x, y and z; a cell query (§6.10.4) is the box query of the cells' world box (inset 0.1 m,
y unbounded). Host references are counted per occupant (an inserted occupant holds one). The scripted generator returns queued
integers (`range_i32`, `uniform_u32`; empty → the lower bound / 0), queued uniforms `u` (`uniform`; `range_f64`
returns `r24(r24(u × r24(b − a)) + a)`; empty → 0.5) and queued Gaussians (empty → 0); every draw is an `rng` event.
`rng: {seed, process_clock?}` uses the real generator (`FloraRng`) instead.

### Environment

| op | Arguments | Effect |
|---|---|---|
| `flora_env` | any of `exists`, `size` (`[x, z]` m), `cells` (`[x, z]`), `established`, `date` (the sim date number), `terrain_initialized`, `sea`, `ground`, `grounds` (`[[x0, z0, x1, z1, h]]`, last match wins), `normal_y`, `normals` (`[[x0, z0, x1, z1, y]]`), `lots` (`[[x0, z0, x1, z1]]`), `moisture`, `moistures` (`[[x0, z0, x1, z1, m]]`), `manager` (an occupant manager exists), `insert_ok`, `segment` (the new-city specification's obliterated segment, or null), `segment_records` (`[{segment, key: [t, g, i], hex (the record's bytes) or null (removed)}]`), `segment_error_keys` (keys whose streams report `GetError`), `occupants` (`[{id, type ("flora", "building", "prop", "landfill", "network" or an ID), pos?, bbox? ([[x, y, z], [x, y, z]]), in_manager? (true), preview?, network? (flags), flora_type?, birth?, last_seed?, state?, orientation?, record? (type or null; default the flora type)}]`), `forget` (IDs to drop) | the scripted city |
| `flora_types` | `types` (`[{type, exemplar_type? (0x0F), next?, water?, floats? (values of 0x083DC35B) or the named floats maturity, seeding_age, interval, seeds, spread, lifespan, max_age, slope, min_score, probability, half_width, unused, clearance, height, cluster_radius?, cluster_height?, table? (one value for all 256 cells), table_cells? ([[e, m, v]]; without `table` the rest is 0.6 where both indices are in 5..12, else 0), never_auto?, flag_switch?, families?, plop_cost?, props? (extra property map)}]`), `append?` | the flora exemplars, in enumeration order |
| `flora_tuning` | `none: true` (no exemplar), or `shipped?` (true), `props?`, `remove?` | the tuning exemplar |
| `flora_registry` | `reset?` (a new process registry), `default_flags?` (the key-0 record's flags, [choice] default 7), `app_shutdown?` | the process-wide registry |
| `flora_create` | `rng?` (`{ints?, doubles?, gauss?}` or `{seed, process_clock?}`), `crt_seed?` | a new simulator (the old one is dropped without `Shutdown`) |
| `flora_rng` | as `rng` above | replaces the simulator's generator |

### Calls and checks

| op | Arguments | Effect / check |
|---|---|---|
| `flora_init` / `flora_shutdown` | `expect?` | `Init` / `Shutdown` |
| `flora_message` | `type` (`SimNewDay`, `SimNewMonth`, `InsertOccupant`, `RemoveOccupant`, `TerrainHeightsChanged`, `ExemplarChanged`, `CityEstablished`, `CityObliterated`, `PlaceFlora`, `Cheat` or an ID), `data1..3?` (`data3` may be an occupant type name), `occupant?` (void1), `text?` (the cheat line), `params?` (`[[as_u32, as_f32]]`), `expect?` | `DoMessage` |
| `flora_days` | `count?` (1) | SimNewDay messages |
| `flora_call` | `method`, `args?`, `expect?` | pure: `elevation_index` (`[alt, sea, water, scale]`), `table_byte`, `blocks_per_month` (`[cx, cz, period]`), `scale_wild_seeds` (`[w, sx, sz]`), `blocks_flora` (`[type, flags or null]`), `query_interface` (`"primary"`, `"message_target"`, `"serializable"` or false), `advance_cluster_type`; simulator: `elevation_index_at` (`[x, z, water]`), `score` (`[type, x, z]`), `best_type` (→ `[type, score]`), `location_ok` (`[type, x, z, exclude?]`), `add_new` (`[type, [x, y, z], force]` → `{occupant, pos}`), `add_flora_occupant`, `drop_seed`, `simulate_existing` (`[occupant]`), `monthly`, `wild_seeds`, `daily`, `kill`, `occupant_inserted` (`[occupant, type]`), `terrain_change` (`[[x0, z0, x1, z1]]`), `demolish`, `demolish_cells`, `demolish_bbox` (`[min, max]`), `cost`, `enable_flora`, `blast` (`[n, m, s]`), `static_data` (→ type or null), `all_types`, `all_types_into` (`[buffer length or null, count]` → `{ret, count, copied}`), `family_types`, `is_valid`, `is_family_valid`, `types_by_filter` (`[accepted types or null]` → `{ret, set}`), `types_by_family`, `add_static` (`[type spec or null]`), `save_obliterated` / `restore_obliterated` (`[segment]`), `occupants_in_cells` (`[rect, type]`: the scripted manager's cell query → IDs); pure (§6.4.2, §6.10.4): `manager_cell` (`[p]`), `manager_listing` (`[min, max]` → `[x0, z0, x1, z1]`), `manager_query` (`[occupant min, occupant max, query min, query max]` → returned), `cell_query_box` (`[rect or null, [cells x, cells z]]` → `{cells, min, max}`), `read_tuning`, `exemplar_changed` (`[inst]`), `add_ref`, `release`, `draw_uniform` |
| `flora_set` | `iterator_table?` (`[[bx, bz]]`: a running table-mode iterator that visits exactly these blocks, in order), `state`: any of `enabled`, `propagation`, `propagation_forced`, `preserve`, `size`, `cells`, `today`, `period`, `blocks_per_month`, `wild_seeds_per_month`, `keep_iterator_flag`, `elevation_scale`, `burn_accumulator`, `burnt_removal_accumulator`, `burn_rate`, `burnt_removal_step` | pokes state (test setup) |
| `flora_expect` | `state`: the fields above plus `initialized`, `ref_count`, `destroyed`, `burnt` (occupant IDs), `simulator_held`, `rng` (`[lcg, mul, flag, cached]`), `iterator` (partial: `state`, `mode`, `dirty`, `rect`, `count`, `table_len`, `index`, `current`, `step`) | the simulator state |
| `flora_expect_occupant` | `occupant`, any of `exists`, `in_manager`, `type`, `flora_type`, `birth`, `last_seed`, `state`, `orientation`, `record`, `pos` | an occupant |
| `flora_expect_record` | `type`, any of `exists`, `next`, `seed_type`, `water`, `flags`, the named floats, `cluster_radius`, `cluster_height`, `unnamed`, `families`, `plop_cost`, `table` (`[[e, m, byte]]`) | a static flora record |
| `flora_expect_registry` | any of `types` (the type array), `keys` (map keys, ascending), `families`, `family` (`{id: [types]}`), `loaded`, `default_flags` | the registry |
| `flora_expect_rand` | `calls` | C `rand()` calls (block shuffles) since the last check (or `flora_create` / `flora_load`) |
| `flora_expect_refs` | `refs` (`{"id": count}`) | host reference counts |
| `flora_expect_segment` | `segment`, `count?`, `records?` (`[{key: [t, g, i], hex? (the whole record) or prefix? (its first bytes)}]`), `absent?` (keys) | an obliteration segment's records |
| `flora_expect_events` / `flora_expect_event_count` / `flora_clear_events` | see the shared conventions | host calls |
| `flora_save` / `flora_load` / `flora_expect_save` | `slot`; load: `rng?`, `expect?`, `crt_seed?` | the class-object store; burnt-list occupants are host objects of class `0x0CC00001` |
| `flora_patch` / `flora_expect_payload` | see the shared conventions (the simulator's record) | |
| `flora_read` | `slot` or `hex`, `expect?`, `consumed?` | calls `Read` of the current simulator directly |

Flora events: `app_callback`, `post` (`type`, `data2`), `subscribe` / `unsubscribe` (`types`), `register_command` /
`unregister_command` (`id`, `name`), `add_agent` (`level`, `name`, `flags`), `remove_agent`, `acquire_services`,
`release_services`, `register_cheat` / `unregister_cheat` (`id`, `name`), `rng` (`method`, arguments, `value`), and
per occupant (`occupant`): `create`, `set_type` / `set_birth` / `set_last_seed` / `set_state` / `set_orientation`
(`value`), `set_record` (`record`), `set_position` (`pos`), `set_flag` (`on`), `change` (0x99EF1145), `init`,
`insert`, `remove`, `move`, `write_occupant` (`in_manager`), `read_occupant` (`key`); `seg_open` (`segment`, `key`,
`write`, `stream` or null), `seg_close` (`stream`). Segment streams count from 1; a write always opens, a read only
a stored record; a written record is stored when its stream closes without an error. An occupant's own record is a
stand-in for lot-records.md §11: `u16 3`, flora type, birth date, last seeding date (`u32` each), state (`i8`),
orientation (`u8`), position (3 × `f32`), flag 1 (`u8`), little-endian, 29 bytes.

## Development helper ops

`development_eval`: `fn`, `args`, `expect` for the shared growth helpers of `fc4_core::development`
(building-development §4.7.3): `developer_type [p, w]`, `wealth [devType]`, `building_map_key [p, w]`,
`building_fits_in_zone [bz, cz]`, `growth_stage_fits_in_zone [stage, zone]`.

## Building development ops

The building development vectors (`bd11_…`) drive `fc4_core::development_simulator` with a scripted city
implementing `DevelopmentHost` and `GrowthDevelopers` (the growth developers, one to one with
`fc4_core::growth_developer`). Every op name starts with `bds_`. The default city: day 1000, difficulty 1,
64 × 64 cells of 16 m (cell = floor(position / 16), centre = (c + 0.5) × 16), terrain altitude 300, area altitude
250, no tuning exemplar, no census, funds 100000, demolition cost 0, an occupant manager that accepts inserts.
Host objects are numbers handed out from 256 (`bds_world next_token` moves the counter): the BDS itself at
`bds_create`, then developers, the building map and the construction node as `Init` creates them, then created
occupants. Growth developers are scripted per developer type (the type is the exemplar's property `0x21`, read at
the developer's `Init`): active demand (default 0, the value the developer's source node answers
`QueryActiveDemandValue` with; the fake node is named by the developer's number; `no_node` = no source node), purpose (default from the type),
sub-type (default the type), lists valid, growth and decline lists (`[[x, z, stage]]`), and how many times
`IsMapUpdateFinished` answers false after each `StartMapUpdate`. The tract developer returns queued values
(`grow`, `decline`; then the defaults, 0). The process statics (catalogue, tried-tract marks, generator) are one
`BdsProcess` per `bds_process` (seed default 0); every generator draw is a `draw` event (`n`, `value`, `stepped`)
logged at the moment it happens, so the events show how draws interleave with the tract developer's calls.
Saves write references as `objectID, classID` (foundations §2.7): the map `0x49B9E602`, the node `0x8990C12D`,
developers `0x0990C138`, occupants `0x0CC00001`; `bds_load` resolves an ID to the object saved under it, else to
the host object with that number (which the world may not know: unresolved).

### City

| op | Arguments | Effect |
|---|---|---|
| `bds_process` | `seed?` (0) or `session?` | new process statics (empty catalogue and marks, the generator seeded with `seed`, or `BdsProcess::from_session`: `derive_seed(session, 0x0C09C0B8)`, foundations §6.3) |
| `bds_world` | any of `day`, `next_token`, `difficulty`, `cell_widths` (`[x, z]`), `cell_counts`, `altitude`, `area_altitude`, `census` (`{id: supply}`), `tuning` (property map, `"shipped"` = {8192, 20480} and {1.2, 1.1, 1.0}, or null), `funds`, `demolish_cost`, `occupant_manager`, `insert_ok` | the scripted city |
| `bds_exemplars` | `list`: `{building, purpose?, wealth?, capacity? (for its own developer type), families?, size? ([x, y, z]), name?, localized?, group?, props?}`, `{developer: type}` (0: no property `0x21`), `{foundation}`, `{wall}` or raw `{instance, group?, props}`; `append?` | the exemplars, in enumeration order |
| `bds_lot_configs` | `list`: `{size: [w, h], zones: [...], ids: [...], orientation? (0), pos? ([x, z]; default the lot centre), no_object?}` | lot configurations for the density table |
| `bds_developers` | `devs`: `{"type": {ad?, no_node?, purpose?, sub_type?, valid?, growth?, decline?, finish_after?}}` (merged) | scripted growth developers |
| `bds_tract` | `grow?`, `decline?` (queues), `grow_default?`, `decline_default?` | tract developer returns |
| `bds_lots` | `lots`: `[{id, building_type?, rect? ([x1, z1, x2, z2]), stage? (or null), building? (or null), state?, center?, slope?, dev_slope?, demolishable?}]` (merged), `cells`: `[{x, z, zone?, powered?, lv_type?, lv?, lot? (or null)}]`, `cell_default?` | lots and cells |
| `bds_occupants` | `list`: `[{id, groups?, preview?, pos?, bbox? ([min, max]), cost?, load_fails?}]` (occupant 0 is the template for created occupants), `building_cells?` (`[[x, z]]`) | occupants |

### Calls and checks

| op | Arguments | Effect / check |
|---|---|---|
| `bds_create` | `grow_budget?` (u32; default u32::MAX) | a new BDS (reference count 1; the old one is dropped without `Shutdown`) with that daily Grow attempt budget ([choice]: at most N tract-developer Grow calls per `Grow`; vectors that run `Grow` set it explicitly) |
| `bds_grow_budget` | `value` | sets the Grow attempt budget of the current BDS |
| `bds_init` / `bds_shutdown` | `expect?` | `Init` / `Shutdown` |
| `bds_add_ref` / `bds_release` | `expect?` (the count) | |
| `bds_message` | `type` (`SimBegin`, `SimNewDay`, `SimNewMonth`, `SimIdle`, `Cheat`, `PlaceBuilding`, `TerrainChanged`, `OccupantInserted`, `OccupantRemoved`, `StateChanged` or an ID), `data1..4?`, `occupant?`, `lot?`, `params?` (`[[as_u32, as_f32]]`), `expect?` | `DoMessage` |
| `bds_day` | `count?` (1) | SimNewDay, then day + 1 |
| `bds_call` | `method`, `args?`, `expect?` | `grow`, `plan_decline`, `execute_decline_plan`, `do_idle`, `monthly`, `daily`, `read_tuning`; `total_resources`, `free_resources`, `claim` / `release` (`[n]`); catalogue: `is_building_type_valid`, `is_building_family_type_valid`, `is_building_in_family`, `types_by_family` (`[f, vector?]` → `{count, vector}`), `types_by_family_set` (`{count, set}`), `types_by_filter` (`[accepted or null, set?]`), `profile` / `profile_ptr` (→ `{purpose, wealth, capacity, size}` or null), `building_key` / `foundation_key` / `wall_key` (→ `[t, g, i]` or null), `exemplar_name`, `building_name`, `all_building_types` / `all_family_types` / `all_foundation_types` / `all_wall_types` (`[vector?]`), `all_building_types_array` (`[n, length or null]` → `{ret, n, array}`), `density` (the table, `[[zone, key, value]]`), `max_capacity` (`[zone, p, w]`), `app_shutdown`; `developer_type`, `wealth`, `developer`, `developer_by` (`[p, w]`), `developers`, `map_key`, `map_key_of_type`, `day_of_last_build`, `stage_multiplier`, `cheats`; `suitable` (`[profile]` → `{result, reason}`, reason starts at -1), `location_ok` (`[type, x, z]`), `add_occupant` (`[type, [x, y, z], orientation]`), `add_occupant_at_cell` (`[type, cx, cz, orientation]`), `plop` (`[profile]`), `insert_occupant`, `demolish_foundation`, `demolition_cost`, `foundation_cost`, `building_changed` (`[lot, wealth, built]`), `terrain_changed` (`[rect, kind]`), `unpack_terrain_rect` (`[data1, data2, cx, cz]`), `query_interface` (→ `"primary"`, `"message_target"`, `"message_target2"`, `"serializable"` or null), `class_id`, `special_buildings`. A profile is `{zone, wealth, purpose, lv_min, lv_max, type, x, z}` |
| `bds_set` | `state`: any of `supply`, `demand`, `map_id`, `node_id`, `decline_plan` (`[[day, x, z, developer, stage?]]`), `state`, `ignore_plumbing`, `ignore_power`, `ignore_caps`, `coffee_bar`, `bias`, `loaded_from_save`, `special_buildings`, `highest_stage` (`{type: stage}`, merged) | pokes state (test setup) |
| `bds_expect` | `state`: any of `initialized`, `loaded_from_save`, `destroyed`, `ref_count`, `state`, `cursor`, `developers`, `highest_stage`, `last_build` (`{type: value}`), `special_buildings`, `decline_plan`, `planned_growth` (count), `grow_budget`, `cr_min`, `cr_max`, `cr_per_resident`, `bias`, `stage_multiplier`, the four stored cheats, `cell_counts`, `has_map`, `map_id`, `node_id`, `supply`, `demand`, `marks` (`{type: [[x, z]]}`), `steps` (generator steps), `rng_state`, `catalogue_loaded`, `building_count`, `funds`, `day` | the BDS, process and city state |
| `bds_expect_map` | `cells`: `[[x, z, value]]` | building map cells |
| `bds_expect_refs` | `developers?`, `occupants?` (`{"id": count}`) | host reference counts |
| `bds_expect_events` / `bds_expect_event_count` / `bds_clear_events` | see the shared conventions | host calls and draws |
| `bds_save` | `slot`, `expect?` | `Write` |
| `bds_expect_record` | `slot`, `hex?`, `length?`, `at?` (`[{offset, hex}]`) | the record bytes |
| `bds_patch` | `slot`, `offset` + `hex`, or `truncate` | edits a saved record |
| `bds_load` | `slot` or `hex`, `expect?`, `consumed?` | `Read` into the current BDS |

Building development events: `draw` (`n`, `value`, `stepped`), `static_cleanup`, `post` (`type`, `data2`),
`subscribe` / `unsubscribe` (`types`), `register_command` (`id`, `name`), `unregister_command` (`id`), `add_agent`
(`level`, `name`, `flags`), `remove_agent`, `register_cheat` (`id`, `name`), `unregister_cheat` (`id`),
`acquire_services`, `release_services`, `create_grid` (`instance`, `tract_size`, `id`), `create_node` (`id`),
`active_demand_changed` (`node`, `key`, `data2`), `create_developer`, `developer_init` (`dev`, `type`),
`developer_shutdown`, `developer_release`, `start_map_update`, `map_update_finished` (`dev`, `result`),
`tract_grow` (`dev`, `x`, `z`, `stage`, `type`, `outcomes_in`, `flag_in`, `built`), `tract_decline` (`dev`, `x`,
`z`, `stage`, `type`, `result`), `update_lot_elements` (`lot`, `had_foundation`: the low byte of the terrain-change
kind ≠ 0, [inferred]), `area_altitude` (`rect`), `dezone` (`rect`,
`flag`), `demolish_lots` (`lots`, `cost`), `set_funds` (`value`), `insert_occupant` (`occ`, `result`),
`remove_occupant`, `create_occupant`, `set_building_type` (`occ`, `type`), `load_occupant` (`occ`, `type`,
`result`), `set_bbox` (`occ`, `min`, `max`), `set_position` (`occ`, `pos`), `set_orientation` (`occ`,
`orientation`), `init_occupant`, `add_ref`, `release` (`occ`), `advisor_event` (`advisor`, `id`, `subject` (null),
`discarded`: `{x, z, lot, data1}`).

## Growth developer ops

The growth developer vectors (`gd14_…`, extras `gd00_…`) drive `fc4_core::growth_developer` over a scripted world
implementing `GrowthDeveloperHost`: the services, message server and simulator of `Init`, the date, source maps,
demand nodes (`GetDemand` / `QuerySupplyValue` / `AddToDemandValue`) and neighbour counts, the BDS's answers
(`GetMaxCapacity` by zone type, `GetBuildingProfile` purposes, `GetSpecialBuildings`, `GetStagePopulationMultiplier`,
`GetTractDeveloper`), the tract developer, zone types, lots, special-building occupants and the time-slice deadline.
Every op name starts with `gdev_`. All developers of a vector share one `GrowthStatics` (the file-local generator,
seed 0 until `gdev_statics`, and the slice step). The default city is 64 × 64 cells, date 0, wall-clock seed 1,
multiplier 1.0, no neighbours, no deadline, and a tract developer named `tract_developer`. Objects are named by
label, grids by their instance ID (`"0x02001010"`). The shared conventions of "Scalar simulator ops" apply (events,
property maps, the store).

### Environment

| op | Arguments | Effect |
|---|---|---|
| `gdev_env` | any of `cells` (`[x, z]`), `date`, `wall_seed`, `missing_services` (`["city", "bds", "occupant_manager", "zone_manager", "land_value", "traffic"]`), `no_message_server`, `no_simulator`, `no_grids`, `log_population` (log `UpdatePopulation`), `log_zone` (log `GetZoneType`), `log_supply` (default true: log demand and neighbour queries), `tract_developer` (bool), `multiplier`, `connected` (`[n1, n2]`), `demand` (`[{id, index?, supply?, active?, label?}]`; `active` answers `QueryActiveDemandValue`), `clear_demand`, `timer_checks` (the deadline answers "not yet" this many times per call; null = never), `max_capacity` (`{"zoneType": value}`), `purposes` (`{"0xType": purpose}`), `zones` (`[{x, z, w?, d?, zone}]`) | the scripted world |
| `gdev_source` | `map` (`intrinsic_land_value`, `total_land_value`, `slope`, `r_proximity` + `index` 1..3, `co_proximity` + `index` `0x3320` / `0x3330`, `park`, `pollution` + `index` 0..3, `school`, `hospital`, `crime`, `trip_length`, `commercial_traffic`, `landmark`, `transient`), `size` (tract size), `fill?`, `tracts?` (`[[x, z, v]]`), `none?` (removes it) | a source map with the element type of §4.3 |
| `gdev_statics` | `seed?` (reseeds the shared generator), `session?` (with `fresh`: `GrowthStatics::from_session`, `derive_seed(session, 0xAC09C0E8)`, foundations §6.3), `slice_step?`, `misses?`, `fresh?` (a new shared object for developers created later) | the process-wide state |
| `gdev_occupant` | `label`, `props?` (`{"0xID": value}`; `{"remove": true}` removes), `position?` (`[x, y, z]`) or `cell?` (`[x, z]`: its centre) | an occupant; `PositionToCell` is `floor(p / 16)` |
| `gdev_specials` | `list` (occupant labels) | the BDS's special-building list |
| `gdev_lot` | `label`, `building_type?`, `capacity?`, `size?` (`[w, d]`), `rect?` (`[x0, z0, x1, z1]`), `location?` (`[x, z]`) | a lot |

### Calls and checks

| op | Arguments | Effect / check |
|---|---|---|
| `gdev_create` | `label?` (`dev`), `add_ref?` (true: `GrowthDeveloper::create`, the constructor + `AddRef` of the BDS's `CreateDeveloper`; false: the bare constructor, count 0) | a constructed developer; it becomes the current one (other ops take `dev?`) |
| `gdev_init` | `exemplar?` (a shipped developer type; default an empty exemplar), `props?` (added or removed on top), `null?`, `expect?` | `Init` |
| `gdev_shutdown` | `expect?` | `Shutdown` |
| `gdev_idle` | `count?` (1), `ms?` (5), `expect?` | SimIdle messages (`DoIdle(ms)`) |
| `gdev_update` | `ms?`, `calls?` | `StartMapUpdate`, then `DoIdle` until the update finishes; `calls` checks the number of calls |
| `gdev_message` | `type` (`SimIdle`, `SimNewDay`, `ZoningChange`, `StateChanged` or an ID), `data1..4?`, `rect?`, `lot?`, `expect?` | `DoMessage` |
| `gdev_call` | `method`, `args?`, `expect?`, `expect_bits?` (the float32 bits of a float result) | the §10.1 slots (`add_ref`, `release`, `target_zone`, `developer_type` / `sub_type`, `purpose`, `wealth`, `construction_info` (`[initial zone code]`), `lists_valid`, `growth_list`, `decline_list` (`[[x, z, maxStage]]`), `growth_items`, `decline_items` (owned copies), `desirability_map`, `capacity_map`, `population_map`, `max_capacity_map` (instance IDs), `total_capacity`, `source_demand`, `source_active_demand` (`GetSourceDemand().QueryActiveDemandValue()`, null without a node), `caps`, `desirability_threshold`, `abandonment_threshold`, `monopoly_use`, `style_percentages`, `start_map_update`, `is_map_update_finished`, `estimate_desirability`, `subjective_thresholds`), the internal steps `new_day_finished`, `city_size`, `update_max_capacity` (`[rect]`), `update_population_count` (`[lot, type, add]`), `do_idle`, the pure helpers `population_of`, `desirability_fraction`, `add_curve`, `commute_add`, `phantom_term`, `round_capacity`, `split_capacity`, `decline_key_seed`, `decline_key` (with the world's wall seed), `style_fraction`, and `query_interface`, `slot` (`[n]` → `[name, pop, return]`), `draw_salt` |
| `gdev_set` | `state?` (`update_state`, `start_day`, `last_phantom_sum`, `total_capacity`, `ref_count`, `growth`, `decline`), `grid?` (`zoned`, `working`, `desirability`, `population`, `capacity`, `max_capacity`) with `fill?` / `tracts?` | pokes state (test setup) |
| `gdev_expect` | `state?`: a partial object of `initialized`, `ref_count`, `destroyed`, `holder`, `services`, `message_server`, `update_state`, `start_day`, `iterator` (`x`, `z`, `rect`, `state`; partial), `specials`, `special_cursor`, `growth`, `decline`, `last_phantom_sum`, `total_capacity`, `source_demand`, `last_salt`, `city_size`, `city_size_bits`, `grids` (partial), `rng_lcg`, `draws`, `slice_step`, `slice_misses`, and the tuning (`developer_type`, `target_zone`, `purpose`, `wealth`, `land_value_min`, `land_value_max`, `use_intrinsic`, `tract_size`, `baseline`, `growth_threshold`, `decline_threshold`, `phantom_demand`, `caps`, `monopoly_use`, `stage_count`, `curves` (the present curves in §4.6 order), `styles`, `abandonment`, `subjective_min`, `subjective_max`); `curve_eval?` (`[{curve, x, y}]`), `stage_eval?` (`[{stage, x, y, bits?}]`) | the developer and the shared statics |
| `gdev_expect_grid` | `grid`, `exists?`, `instance?`, `size?`, `counts?`, `all?`, `tracts?` (`[[x, z, v]]`), `nonzero?` | one of the six grids |
| `gdev_expect_salts` | `devs` (labels), `salts` | the developers' last salts, as a multiset, are among `salts` (each listed salt used once): which industrial developer gets which salt is a [compat-limit] and is not pinned |
| `gdev_expect_refs` / `gdev_expect_demand` | refs: `{label: count}` (references held through `AddRef` / `Release`); demand: `totals` (`{label: sum of AddToDemandValue}`) | host objects |
| `gdev_expect_events` / `gdev_expect_event_count` / `gdev_clear_events` | see the shared conventions | host calls |
| `gdev_sort` | `keys`, `order?` (`desc` / `asc`), `expect` (original indices in sorted order) | the game's `std::sort` on its own |
| `gdev_save` / `gdev_load` / `gdev_expect_save` | `slot`, `dev?`; load: `label?` (`dev`), `expect?` (the store-level result) | the class-object store; the loaded developer becomes current |
| `gdev_patch` / `gdev_expect_payload` | see the shared conventions (the developer's record) | |
| `gdev_read` | `hex` (a record with null references), `expect?` | `Read` on the current developer |

Growth developer events: `acquire_service` (`service`, `ok`), `release_service`, `acquire_message_server` (`ok`),
`release_message_server`, `subscribe` / `unsubscribe` (`types`), `agent` (`level`, `name`, `flags`), `remove_agent`,
`create_grid` (`instance`, `size`), `set_tract_size` (`which`, `grid`, `size`), `grid_shutdown` (`which`, `grid`),
`add_ref` / `release` (`object`), `get_demand` (`id`, `index`, `node`), `query_supply` (`node`), `add_to_demand`
(`node`, `value`, `bits`), `active_demand` (`node`), `connected_cities`, `all_connected_cities`, `stage_multiplier`, `tract_developer`,
`special_buildings`, `max_capacity` (`zone`, `purpose`, `wealth`), `building_purpose` (`building_type`),
`lot_building_type` (`lot`, `flag`), `lot_capacity` (`lot`, `developer_type`, `flag`), `lot_size`, `lot_rect`,
`lot_location` (`lot`), `timer_start` (`ms`), `wall_clock_seed`, and with their flags `update_population`
(`tract_developer`, `developer`, `x`, `z`, `stage`) and `zone_type` (`x`, `z`).

## Tract developer ops

The tract developer vectors (`td11_…`, extras `td00_…`) drive `fc4_core::tract_developer` with a scripted world
implementing `TractDeveloperHost`. Every op name starts with `tdev_`. Objects are tokens: developers, lots and
configurations take the `id` the vector gives; lots and filters the host creates get tokens from 65536 on. The
default world: a 64 × 64-cell city, construction resources 10⁹ (none used), no cheats, no anchor needed, every grid
at its default (zone 1, land value 0, powered 2, watered 1, fire 1, park 1, road access 1), no networks, no
surrounding lots, the shipped tuning exemplar (§4.21), services, simulator and message server present. A lot's
rectangle is computed from its location, size and facing (§4.10.1 table). `StartConstruction` puts the lot in
state 2 with the building; `AbandonBuilding` sets state 4 (synchronous, `lot-developer.md` §4.3);
`ReoccupyBuilding` sets state 3; `DemolishLots` clears the building; `CreateLotIfPossible` makes a state-1 lot whose
location is the inverse of the §4.10.1 table. With facing −1 on a free rectangle it first draws `U(4)` on the zone
developer's generator (R9, zones.md §7.7 / §9), records it in the tract developer's draw trace and faces the new lot
that way (the harness has no road model). Draws are traced by kind: R1–R8 by the core (§4.22), R9 by the host.

### Setup

| op | Arguments | Effect |
|---|---|---|
| `tdev_world` | any of `city` (`[x, z]` cells), `supply`, `used`, `cheats` (`{ignore_plumbing, ignore_power, ignore_caps, debug_growth}`), `anchor` (NeedsAnchorBuilding's answer), `construction_sets_state` (true), `no_services`, `no_simulator`, `no_message_server`, `create_fails` (CreateLotIfPossible gives no lot and no draw; the event still shows `lot: null`), `zone_seed` (the zone developer generator's state; default `0x8C09C68A`, its seed at clock 0) | the world |
| `tdev_developer` | `id`; any of `target_zone` (1), `subtype` (`0x1010`), `purpose` (1), `wealth` (1), `zone_code` (1), `land_value` (`[min, max]`, `[0, 0]`), `tract_size` (4), `shift` (2), `desirability` / `capacity` (`[[tx, tz, v]]`), `demand` (number, `"nan"` or null = no source demand; 1), `caps` (`[power, water, fire, park]`, 255 each), `monopoly_use` (−1), `styles` (`[[group, fraction]]`, ascending keys) | a growth developer (fields not given keep their value) |
| `tdev_lot` | `id`; any of `at` (location `[x, z]`), `size` (`[w, h]`, 1 × 1), `facing` (0), `slope`, `zone` (1), `state` (1), `livable`, `has_capacity` (default: a building stands), `days`, `wealth`, `historical`, `building`, `capacity`, `available` (default: the capacity), `road`, `organic` (true), `cfg_stage` (number or null), `can_create` (7); `remove: true` | a lot |
| `tdev_cfg` | `id` (token); any of `cfg_id` (the configuration ID, for `GetLotConfiguration`), `size`, `slope` (`[min, max]`, ±100), `stage` (1), `capacity` (`[min, max]`), `zones`, `wealths`, `purposes`, `styles` (absent = compatible with all), `roads` (required-road bits), `primary` (family IDs; null = no primary object) | a lot configuration (all are visible to the configuration manager, filtered by `IsLotConfigurationIncluded`) |
| `tdev_building` | `id` (type); any of `purpose` (1), `wealth` (1), `capacity` (1), `groups` (absent = every occupant group), `monopoly`, `fields` (`0x29B55F73` IDs) | a building type |
| `tdev_family` | `id`, `members` (BDS insertion order) | a building family |
| `tdev_cells` | `grid` (`zone`, `land_value`, `powered`, `watered`, `fire`, `park`, `road_access`), `default?`, `cells?` (`[[x, z, v]]`), `rects?` (`[[x1, z1, x2, z2, v]]`) | grid values |
| `tdev_networks` | `cells` (`[[x, z, [types]]]`) | network tiles and their `IsOfType` answers |
| `tdev_surrounding` | `rect?` (the lots `GetLotsSurroundingRect` appends for flag 1), `lots?` (`{"lot": [lots]}` for `GetLotsSurroundingLot`, flag 1); other flags append nothing | neighbour lists |
| `tdev_tuning` | `none: true`, or `shipped?` (true) with `props?` (a property map merged over it) and `remove?` (IDs) | the "Building Development" exemplar |
| `tdev_create` | `seed?` (the file-local generator's seed exactly; default `0x4C09C113`, the original's seed at clock 0) or `session?` (`TractDeveloperStatics::from_session`, `derive_seed(session, 0x4C09C113)`, foundations §6.3), `crt_seed?` (1), `trace?` (true), `histogram?` (true), `shared?` (reuse the previous instance's statics) | a new tract developer |
| `tdev_rng` | `seed`, or `lcg` and `mul`; `crt?` | sets the shared generator / `rand()` state |
| `tdev_set` | any of `styles` (assigned as is), `style_index`, `years_since`, `years_between`, `single_style` | pokes the style state |

### Calls and checks

| op | Arguments | Effect / check |
|---|---|---|
| `tdev_init` / `tdev_shutdown` | `expect?` | `Init` / `Shutdown` |
| `tdev_message` | `msg`, `expect?` | `DoMessage` |
| `tdev_new_year` | `count?` (1) | `NewYear` |
| `tdev_set_active_styles` | `styles` | `SetActiveStyles` |
| `tdev_use_all_styles` | `value` | `UseAllStylesAtOnce` |
| `tdev_grow` | `dev`, `tract` (`[x, z]`), `stage`, `append?` (keep the previous outcome list), `out_flag_in?`; checks `expect` (built), `outcomes` (the whole list), `out_flag` | `Grow` |
| `tdev_decline` | `dev`, `tract`, `expect?` (the sum) | `Decline` |
| `tdev_update_population` | `dev`, `tract` | `UpdatePopulation` |
| `tdev_save` | `expect?` (hex), `ok?` | `Write` |
| `tdev_load` | `hex`, `expect?` | `Read` |
| `tdev_expect` | any of `style_index`, `years_since`, `years_between`, `single_style`, `all_styles`, `styles`, `current_style`, `desired` (`[desired, desiredMax]` of the last Grow), `chosen_cfg`, `chosen_building`, `lcg`, `mul`, `crt`, `initialised`, `configured`, `filters` (cached count), `tuning` (partial: `overbuild`, `reoccupy`, `redevelop_size_threshold`, `demolition`, `new_lot`, `occupancy_variation`, `min_vacancy`, `kick_out`, `available_styles`), `used` (resources), `lots` (existing tokens) | state |
| `tdev_expect_lot` | `lot`; any of `exists`, `state`, `facing`, `building`, `population` (`{devType: n}`), `rect` | a lot |
| `tdev_expect_hist` | `row`, `counts` (`{code: n}`; every other counter of the row is 0) | the debug histogram of the last Grow (§4.1) |
| `tdev_expect_draws` | `draws`: each `"Rn"` (kind only) or `["Rn", arg, value]` (exact); `keep?` | the draws since the last check |
| `tdev_expect_events` / `tdev_expect_event_count` / `tdev_clear_events` | see the shared conventions | host calls |
| `tdev_eval` | `fn`, `args`, `expect`: `outcome_codes`, `best [codes]`, `desired [demand, overbuild]`, `capacity_window [candidate, desiredMax, built]`, `rect [x, z, w, h, facing]`, `corner [bits, candidate]`, `roads_check [bits, candidate, requireRoads]` (21 or `[27, corner]`), `style_points [fraction]`, `style_group [[[key, fraction]], r]`, `score [candidate, zone, tuning {demolition, new_lot}, seed]` (`{score, bits, mul}`), `hash_order [values]`, `stl_sort [scores]` (sorted indices), `query_interface [iid]`, `zone_family [zone]`; a candidate is `{kind, existing, abandoned, n, facing, lots, street, road, avenue, rail}` | pure functions |

Tract developer events: `acquire_services`, `add_agent` (`level`, `message`), `acquire_message_server`,
`release_message_server`, `remove_agent`, `release_services`, `register_filter` (`wealth`, `purpose`, `filter`),
`unregister_filter` (`filter`), `claim` (`amount`, `ok`), `release` (`amount`), `needs_anchor` (`rect`, `purpose`),
`start_construction` (`lot`, `cfg`, `building`, `flag`), `abandon` (`lot`, `reason`), `reoccupy` (`lot`,
`dev_type`), `demolish` (`lot`), `delete_lot` (`lot`), `zone_draw` (`value`, R9), `create_lot` (`x`, `z`, `w`, `h`,
`facing`, `lot`), `redisplay` (`rect`), `set_facing` (`lot`, `facing`), `set_state` (`lot`, `state`),
`set_population` (`lot`, `dev_type`, `population`).

## Lot developer ops

The lot developer vectors (`ld16_…` for the §16 conformance tests, extras `ld00_…`) drive
`fc4_core::lot_developer` over a scripted world implementing `LotDeveloperHost`: the services, message server and
simulator of `Init`; the city, simulator, terrain and 24-hour clock; the lot manager and its safe lot iterator
(the lot-list order the daily pass walks); lots and their per-lot properties (the counters and the zot of §0's
table); lot configurations and their object arrays; buildings and other occupants (bounding boxes, city cells,
groups, properties); the occupant and prop managers; the BDS (profiles, developers, cheats, families,
`AddNewBuildingOccupant`); growth developers; power, plumbing, traffic and demand; flora; demolition; business
units; and the geometry the spec leaves to a later terrain / lot-content spec (§12.2 Q4). Every op name starts with
`ldev_`. Objects are tokens (`u64`): the vector gives the `id` of each lot, configuration, occupant, developer and
unit it creates; the host hands out further tokens from 1 for props and buildings it makes. The default world is a
64 × 64-cell city (cell width 16), a 64 × 64 terrain, date 0, time mode 0, no cheats, services / simulator /
message server present, and the shipped tuning exemplar (§3.5). The file-local generator (§10.1) is one
`LotDeveloperStatics` shared by the instance; `ldev_create` seeds it explicitly (default seed 12345) or from the
session seed (§6.3), and `rand()` starts at state 1. Draws are traced by kind R1–R15 (§10.2).

### Setup

| op | Arguments | Effect |
|---|---|---|
| `ldev_world` | any of `city` (`[x, z]` cells), `cell_widths` (`[x, z]`), `terrain_cells` (`[x, z]`), `terrain_width`, `sim_date`, `time_mode` (24-hour clock), `view_rotation`, `jobs_demand` (number, `"nan"` or null = no node), `cheats` (`{ignore_plumbing, ignore_power, coffee_bar}`), `no_services`, `no_simulator`, `no_message_server`, `claim_resources`, `add_building_fails`, `foundation_fails` (BDS slot 23 fails), `base_texture_fails` (`GetLotBaseTextureOccupant` fails), `zone_color` (`GetZoneDragColor`, default `0xFFB23300`), `zone_alpha` (default `0xAA`), `trace_terrain` (log every terrain read as `alt_vertex` / `water_vertex` (`index`) and `alt_at` / `water_at` (`x`, `z`) events) | the world |
| `ldev_lot` | `id?` (assigned when absent); any of `x`, `z`, `w`, `h`, `zone` (1), `state` (1), `livable`, `condition`, `days`, `building_type`, `building` / `no_building`, `cfg` / `no_cfg`, `wealth`, `occupancy`, `needs_power`, `has_power`, `road`, `job`, `organic`, `flags`, `historical`, `initialized`, `available`, `travel_jobs` (`[w0..w3]`), `designate` / `no_designate`, `unit` / `no_unit`, `props` (`{"0xID": value}`), `counter` (`{low_desirability, no_power, no_water, travel, zot}`), `altitude` (lot slot 16), `facing` (lot slot 23), `locked` (`GetVerticesAreLocked`), `order` (append to the lot-list order; default true for a new lot) | a lot |
| `ldev_cfg` | `id` (token); any of `cfg_id` (its own ID, for `GetLotConfiguration`), `w`, `h`, `stage`, `water`, `max_slope`, `objects` (`[{type, flags, ids, tag, orientation, byte3, offset [x, y, z], footprint_min [x, z], footprint_max [x, z], foundation}]`, the §17.1 record), `primary_building` / `primary_foundation` (null = none, `[ids]`, or an object record), `water_objects` | a lot configuration |
| `ldev_occ` | `id` (token); any of `is_building`, `bbox` (`[[min], [max]]`), `cells` (`[x1,z1,x2,z2]`), `group`, `lit`, `position`, `orientation`, `night_light`, `lot` (its owning lot, for `GetLotProps` and `GetLotBaseTextureOccupant`), `props` (`{"0xID": value}`), `type` (occupant type; default foundation / building / prop), `foundation_kind` (makes it a foundation, §17.5), `foundation_type`, `inserted` (`IsFlagSet(1)`, default true), `immovable` (blocks `ModifyTerrainHeight`, §17.3) | an occupant |
| `ldev_profile` | `building` (type); `purpose`, `wealth`, `capacity` | a building profile |
| `ldev_family` | `family`, `members` (`[ids]`) | a building / flora family |
| `ldev_valid` | `building` (`[ids]`), `flora` (`[ids]`) | IDs `IsBuildingType` / `IsFloraTypeValid` accept |
| `ldev_dev` | `id` (token); `key?` (the developer type `(purpose<<8)|wealth`; default from `purpose`/`wealth`, an omitted one keeping the value already in the key), `purpose`, `desirability`, `water_cap` | a growth developer |
| `ldev_unit` | `id` (token); `members` (`[lots]`, also sets each lot's unit) | a business unit |
| `ldev_cells` | `powered` / `watered` (`[[x, z, v]]`, v≠0 sets), `trip` (`[[x, z, v]]`) | cell grids |
| `ldev_terrain` | `reset` (back to flat, no overrides), `flat` (the default altitude), `vertex` (`[[x, z, v]]`), `alt` (`[[index, value]]`), `water` (`[[index, value]]`; index = `x + z · vertsX`), `pos` / `water_pos` (`[[x, z, v]]`: `GetAltitude` / `GetWaterLevel` at an exact position; elsewhere the vertex grid), `water_vertices` / `water_cells` (`[[x, z]]`) | vertex altitudes (default 100) and water levels (default 0) |
| `ldev_resource` | `instances` (`[ids]`), `replace?` (clear the set first) | base-texture resources that exist (§6.6) |
| `ldev_surrounding` | `lot` (`{"token": [neighbours]}` for `GetLotsSurroundingLot`), `rect` (`[lots]` for `GetLotsSurroundingRect`), `in_rect` (`[lots]` for `GetLotsInCellRect`) | neighbour / rectangle lists |
| `ldev_tuning` | `none: true`, `shipped: true`, or explicit `stay_new`, `thresholds` (`[abandon, distress, renew]`), `night_acc`, `night_batch`, `max_slope_flatness`, `cost_curve` (`[[x, y], …]`); `from_shipped: true` with any of `auto_level_delta`, `max_area_slope`, `default_lot_foundation` overrides the shipped values | the "Building Development" exemplar |
| `ldev_create` | `seed?` (the file-local generator's seed exactly; default 12345) or `session?` (`from_session`, `derive_seed(session, 0x6C09C0F4)`, §6.3), `crt_seed?` (1), `trace?` (true) | a new lot developer |
| `ldev_rng` | `seed`, or `lcg` and `mul`; `crt?` | sets the shared generator / `rand()` state |
| `ldev_make_lots` | `count` | `count` empty state-1 lots along `(x, 0)`, in lot-list order (each visit logs one `reset_travel_pop`) |
| `ldev_make_buildings` | `count` | `count` building occupants (tokens 1..count) each on its own lot at `(i, 0)`, delivered as an `0x99EF1142` insert so they join the night deque |
| `ldev_clear_lots` / `ldev_clear_buildings` | | empties the lots (and order) / the night deque and scripted buildings |
| `ldev_set` | any of `daily_state`, `visited`, `net_changed`, `unit_changed`, `no_zot_until_day`, `can_check_commute`, `night_mode`, `night_acc`, `night_batch`, `night_cursor`, `lit_flag`, `force_empty` (direct, no walk), `daily_work_budget` | pokes state |

### Calls and checks

| op | Arguments | Effect / check |
|---|---|---|
| `ldev_init` / `ldev_shutdown` | `expect?` | `Init` (§1.3) / `Shutdown` (§1.4) |
| `ldev_read_tuning` | | `ReadSimulatorTunableValues` (§3.5) without `Init` |
| `ldev_message` | `msg`, `data1..4?`, `void1?`, `lot?`, `expect?` | `DoMessage` (§3.2) |
| `ldev_daily` | `expect?` | `DoDailySimulation` (§4.1); `per_day` reads the quota |
| `ldev_per_day` | `expect?` | the quota `n / 28 + 1` |
| `ldev_update_lot` | `lot`, `expect?` | `UpdateLot` (§4.2), returns "state changed" |
| `ldev_test_desirability` / `ldev_test_power` / `ldev_test_water` / `ldev_test_condition` | `lot`, `at` (`[x, z]`), `expect?` | the tests of §5.1–§5.5 |
| `ldev_test_travel` | `lot`, `at`, `purpose?`, `wealth?`, `expect?` | `TestTravel` (§5.4) with a temporary profile |
| `ldev_coffee_bar` | `lot` | `TestCoffeeBar` (§5.8, always draws R3 first) |
| `ldev_add_zot` / `ldev_remove_zot` | `lot`, `prop`, `zot` / `lot`, `zot?` (0 = slot 11), `expect?` | the zot helpers (§5.6) |
| `ldev_pick_unit` | `lot` | `PickUnitToJoin` (§5.7, draws R2) |
| `ldev_update_designates` | `unit?`, `expect?` (the resulting reference, null when dissolved) | slot 34 `UpdateTravelDesignates` (§5.7) |
| `ldev_update_populations` | `unit` | slot 35 `UpdateTravelPopulations` (§5.7) |
| `ldev_remove_from_unit` | `lot`, `expect?` | slot 33 `RemoveLotFromItsBusinessUnit` (§5.7) |
| `ldev_needs_anchor` | `rect`, `purpose`, `expect?` | `NeedsAnchorBuilding` (§5.7) |
| `ldev_start_construction` / `ldev_construct_default` | `lot`, `cfg?`, `type`, `animate?`, `expect?` | `StartConstruction` (§6.1) / `ConstructDefaultLot` (§6.4) |
| `ldev_abandon` / `ldev_reoccupy` / `ldev_change_building` | `lot`, `reason` / `lot`, `dev` / `lot`, `type`, `expect?` | slots 8 / 9 / 10 (§6.4) |
| `ldev_update_lot_elements` | `lot`, `kind` (the **whole 32-bit** terrain-change kind; only `kind != 5` is tested, §6.4), `expect?` | slot 37 |
| `ldev_area_flat` | `rect` (cells) or `lot`, `water?` (default true), `expect` | slot 25 `IsAreaFlat` / slot 26 `IsLotFlat` (§17.2, LD29: every vertex against the first, read with water) |
| `ldev_area_range` | `rect`, `water?` (default true), `expect?`, `expect_bits?` | slot 27 `GetAreaAltitudeRange` (§17.2) |
| `ldev_start_demolish` / `ldev_end_demolish` / `ldev_clear_lot` | `lot`, `expect?` | slots 12 / 13 and `ClearLot` (§7) |
| `ldev_explode_lot` | `lot`, `burnt?`, `expect?` | slot 14 (§7.2) |
| `ldev_add_rubble` | `lot`, `min` (`[x,y,z]`), `max`, `burnt?` | `AddLotRubbleProps` (§7.3, draws R9–R14) |
| `ldev_power_changed` | `rect` | `DoPowerChanged` (§9) |
| `ldev_set_normal_texture` | `lot`, `cfg?`, `flag?` | slot 44 (§6.6, draws R4) |
| `ldev_set_force_empty` | `value` | slot 47 (§6.6, walks every lot on a change) |
| `ldev_anim_tick` | `ms` | `DoAnimationTick` (§8.3) |
| `ldev_light` / `ldev_update_all` | `building`, `night?` / `mode` | `Light` (§8.4) / `UpdateAll` (§8.2) |
| `ldev_area_slope` / `ldev_dev_cost` / `ldev_altitude` | `rect`, `water?` / `rect` / `at`, `water?`, `expect?` | slots 29 / 32 / 15 (§11.4) |
| `ldev_area_developed` | `rect` or `lot`, `expect?` | slots 23 / 24 (always false) |
| `ldev_area_altitude` | `rect` (int32 cells), `water?`, `lowest?` (slot 17) or `average: true` (slot 18), `expect?`, `expect_bits?` | §17.2 |
| `ldev_altitude_f` | `rect` (float metres), `lowest`, `water`, `expect?` | slot 16 (§17.2) |
| `ldev_level_internal` | `rect`, `target`, `skip_water`, `force`, `lot`, `expect` (the call count) | slot 19 (§17.4) |
| `ldev_level_bordering` | `rect`, `h`, `lot`, `expect` | slot 20 (§17.4) |
| `ldev_attempt_levelling` | `rect`, `water`, `per_vertex`, `max_slope` (number, `"flt_max"`, `"nan"`), `lot`, `expect` | slot 21 (§17.4) |
| `ldev_object_position` | `lot`, `cfg` (or null), `object` (a §17.1 record), `expect {pos, orient}` | slot 39 (§17.6) |
| `ldev_position_for_building` | `lot`, `expect` (`{pos, orient}` or null) | slot 40 (§17.6) |
| `ldev_boundary_texture` | `sides` (`[s0, s1, s2, s3]`), `expect` (`{texture, rotation, flip}`, null before `Init`) | slot 42 (§17.7) |
| `ldev_set_empty_texture` | `lot` | slot 43 (§17.7) |
| `ldev_foundation` | `occupant` (slot 48), `lot` (slot 51), `cell [x, z]` + `kind` (slot 49) or `position [x, z]` + `kind` (slot 50); `expect` (token or null) | §17.5.1 |
| `ldev_remove_foundation` | `position` + `kind` (slot 52), `cell` (slot 53) or `occupant` (slot 60); `expect` | §17.5.2 |
| `ldev_add_lot_foundation` | `lot`, `type` (slot 55) or `cfg` (slot 54, null allowed), `use_lot_altitude?`, `level?`, `walls?`, `expect` | §17.5.4 |
| `ldev_update_lot_foundation` | `lot`, `expect` | slot 61 (§17.5.4) |
| `ldev_foundation_required` | `occupant` (slot 56) or `min`, `max` (slot 57); `expect` | §17.5.5 |
| `ldev_add_building_foundation` | `building`, `cfg` (slot 58) or `type` (slot 59); `expect` | §17.5.5 |
| `ldev_expect_terrain` | `vertex` (`[[x, z, v]]`) | vertex altitudes |
| `ldev_expect_occ` | `id`; any of `exists`, `position`, `bbox`, `cells`, `orientation`, `foundation_kind`, `foundation_type` | an occupant |
| `ldev_save` / `ldev_load` | `expect?` (hex), `ok?` / `hex`, `expect?` | `Write` / `Read` (§13) |
| `ldev_expect` | any of `initialised`, `daily_state`, `visited`, `net_changed`, `unit_changed`, `net_snap`, `unit_snap`, `zots_allowed`, `no_zot_until_day`, `can_check_commute`, `force_empty`, `night_mode`, `night_acc`, `night_batch`, `night_cursor`, `lit_flag`, `verts_x`, `extent_x`, `extent_z`, `night` (the deque's tokens, `null` for a null slot), `night_len`, `per_day`, `cell_counts`, `lcg`, `mul`, `crt`, `tuning` (partial) | state |
| `ldev_expect_lot` | `lot`; any of `exists`, `state`, `condition`, `zone`, `building_type`, `wealth`, `flags`, `altitude`, `facing`, `historical`, `has_power`, `cfg`, `building`, `unit`, `designate`, `rect`, `prop` (`{name or 0xID: value}`) | a lot |
| `ldev_expect_draws` | `draws`: each `"Rn"` (kind only) or `["Rn", arg, value]` (exact); `keep?` | the draws since the last check |
| `ldev_expect_events` / `ldev_expect_event_count` / `ldev_clear_events` | see the shared conventions | host calls |
| `ldev_eval` | `fn`, `args`, `expect`: `is_empty_lot_suitable_for_zot [zone, x, z]`, `desirability_tolerance [x, z]`, `answers_iid [iid]`, `rubble_count [minx, miny, minz, maxx, maxy, maxz]` (`{count, small, large}`), `reason_const [name]` | pure functions |

Lot developer events: `acquire_services`, `release_services`, `add_agent` (`level`, `message`), `remove_agent`,
`acquire_message_server`, `release_message_server`, `subscribe` / `unsubscribe` (`message`), `register_tick` /
`unregister_tick`, `post` (`message`, `data1`, `lot`), `delete_lot`,
`set_state` (`lot`, `state`, `wealth`, `reason`), `set_condition` (`lot`, `condition`), `set_population`,
`update_road_access`, `reset_travel_pop` / `add_travel_pop`, `vertices_locked`, `update_slope`, `set_texture`
(`texture`, `x`, `z`, `orientation`, `byte3`), `occupant_changed`, `insert_occupant`, `occ_post` (`occ`, `message`),
`set_lit`, `create_prop` (`occ`, `type`), `prop_add` (`occ`, `type`, `pos`, `orientation`), `add_lot_prop` (`lot`,
`occ`, `orientation`, `pos`), `prop_remove`, `remove_lot_props`, `remove_building_props`, `flora_add`,
`flora_demolish`, `demolish_occupant`, `demolish_lot`, `add_building` (`occ`, `type`), `claim` (`amount`, `ok`),
`add_lot_foundation` (`lot`, `type`), `add_building_foundation`, `remove_building_foundation`,
`update_lot_foundation`, `update_retaining_walls`, `lot_occupants` (`lot`; lot slot 49 of `UpdateLotObjectAlitudes`,
§6.5, LD17), `occupant_moved` (`occ`, `y`: occupant manager slot 24 with 1 after the new y), `update_base_texture_occupant`
(`lot`, `had_foundation`), `set_empty_texture`, `remove_retaining_walls`, `remove_lot_foundation`,
`update_surrounding_foundations`, `update_surrounding_walls` (`lot`), the §17 terrain writer `modify_terrain`
(`per_vertex`, `rect`, `h`, `exempt`) with `set_altitude` (`x`, `z`, `h`), `set_altitude_rect` (`rect`, `h`, `reason`) and
`redisplay` (`rect`, `reason`), BDS slot 23 `add_foundation` (`type`, `kind`, `pos`, `dx`, `dz`, `orient`, `bottom`,
`occ`, `top`, `size`, `cells`), `retaining_wall` (`lot`, `flag`), `set_textures` (`occ`, `n`) followed by one
`set_texture` per record (`texture`, `x`, `z`, `orientation`, `byte3`, `extra` (five bytes), `frame`),
`remove_occupant` (`occ`, `a`, `flags`), `shutdown_occupant` (`occ`), `add_network_nodes` (`lot`, `nodes`: one
`[type, x, z, b, c, d, orientation]` per connection node, §6.5, LD17), `remove_network_nodes`,
`construction_occupant` (`lot`), `level_internal`, `attempt_levelling`, `rubble_decal` (`instance`, `size`),
`unit_create`, `unit_add` (`unit`, `lot`).

## Zones ops

The zones vectors (`zn15_…`) drive both classes of `fc4_core::zones` (the zone manager and the zone developer / lot
manager, `docs/specs/zones.md`) over one scripted world implementing `ZonesHost`. Every op name starts with `zone_`.
Objects are tokens: lots the world creates start at 4096, so the first is `0x1000`; configurations, developers,
families, buildings and occupants take the `id` / `token` the vector gives. The default world is a 64 × 64-cell city,
dry and flat, funds 10⁹, every service present, the shipped "Zone Manager" tuning (§4), no networks, lots or
occupants. The two classes reach each other through the world, as they do through the global pointers (§13.1), so a
committed `PlaceZone` runs the whole parcellization synchronously.

Networks are straight runs of one kind (network type: 0 road, 3 street, 6 avenue, 10 one-way, 11 dirt road); each cell
records its kind and the four network edges it connects, so `ExistsConnectionOfType` and `ConnectsToNeighbor` (§7.13)
answer from the run's own geometry. The street tool is the stub of §15 tests 19–29: `Place(a, b)` logs the call, fails
when the segment contains a `zone_street_fail` cell, and otherwise puts a street on every cell of `a..b` that has no
network — joining it to the same-kind network it abuts, as an automatic street does — while the cells keep their zone
(the posted dezone is not run). With realMode false (highlight mode, test 33) it lays no street: each cell of `a..b`
with no network becomes a **street draft** (flags `0x8`, a nonzero mask byte per edge towards a neighbouring cell of
the segment; a later draft on the cell adds its bytes), and the call is logged as `P(…) draft`. The street and
dirt-road tool stubs answer `OccupantExists` (§8.8) from their drafts and log each call (`occupant_exists`);
`Reset` (from `ClearHighlight`) drops the street tool's drafts. A region argument `{rect, cells?, rects?}` takes
either the listed cells or the union of the listed rects (default: every cell of `rect`).

### Setup

| op | Arguments | Effect |
|---|---|---|
| `zone_env` | any of `cells` (`[x, z]`), `funds`, `landfill_cost` (what the landfill developer's `GetZoningCost` returns for a type-14 `PlaceZone`; default 0), `services_ok` (false = the developer's `Init` fails), `tuning` (`"shipped"`, a property map, or null) | the world; `cells` is read at `Init` |
| `zone_create` | `session?` (`derive_seed(session, 0x8C09C68A)`, foundations §6.3; default session 0) or `seed?` (an exact seed), `trace?` (true), `manager?` (true), `developer?` (true) | the shared statics and the two classes |
| `zone_rng` | `seed`, or `lcg` and `mul` | the file-local generator's state (§9) |
| `zone_network` | `runs` (`[{kind, rect \| cells}]`), `add?` (keep the existing networks) | network runs with their edge connectivity |
| `zone_zones` | `rect` with `v`, or `cells` (`[[x, z, v]]`) | writes the zone grid directly, without `PlaceZone` (test setup) |
| `zone_water` | `cells` (`[[x, z]]`), `clear?` | `CellHasWater` cells |
| `zone_lot` | `rect`, `facing?` (0); any of `zone`, `state`, `flags`, `cfg`, `building`, `business_unit`, `altitude` | a lot in the cell map and the master list |
| `zone_cfg` | `id` (the token and the configuration ID), `w`, `h`, `water?`, `zone_ok?` (true), `purposes?` / `wealths?` (the compatible types; absent = all), `stage?` (1, `GetGrowthStage`), `object?` (the primary building object's ID list; absent or null = no object) | a lot configuration; the configuration manager walks them in `zone_cfg` order (§7.2.1 step 5) |
| `zone_dev` | `token`, `type` (the developer type the BDS returns it for), `purpose`, `wealth`, `shift?` (0), `demand?`, `estimate?` (`[[x, z, v]]`, `EstimateDesirability`; 0 elsewhere), `growth?` (`[[x, z, maxStage]]`; absent = the growth/decline lists are not valid), `desir_value?` (`[[tx, tz, v]]`) | a growth developer (§7.2.1); a repeated `token` replaces it |
| `zone_family` | `id`, `types` | a building family (BDS slot 19; a building ID answers itself, anything else nothing) |
| `zone_building` | `id`, `purpose?`, `wealth?`, `capacity?` | a building profile (BDS slot 39) |
| `zone_occupant` | `token`, `type`, `pos` (`[x, y, z]`), `bounding?` (`[x1, z1, x2, z2]`), `net_flags?`, `can_remove?` (true) | an occupant (building, obstacle, landfill, network) |
| `zone_options` | `alternate?` (false), `streets?` (true), `custom?` (false) | `SetOptions` (slot 7) |
| `zone_lot_size` | `width?` (2), `depth?` (2), `min_depth?` (1), `max_depth?` (3), `block?` (6) | forces the lot-size parameters and **bypasses** `DetermineLotSize` (§7.2.1), as the tests 19–29 common setup does |
| `zone_street_fail` | `cells` (`[[x, z]]`) | the cells that make the street stub fail a segment containing them |
| `zone_draft` | `tool` (3 street, 11 dirt road), `cell?`, `mask` (bytes for edges −x, −z, +x, +z), `flags?` (`0x8` / `0x800`), `clear?` | a draft record of a network tool stub (§8.8, test 33d); `clear` drops the tool's drafts first |

### Calls and checks

| op | Arguments | Effect / check |
|---|---|---|
| `zone_init` / `zone_shutdown` | `expect?` | both classes' `Init` / `Shutdown` (the developer's without the 3D view) |
| `zone_message` | `id` (or a name), `target?` (`"manager"` (default) or `"developer"`), `occupant?`, `data1?`, `data3?`, `data4?`, `value?`, `expect?` | `DoMessage` (§5.1, §5.2) |
| `zone_place_zone` | `region` (`{rect, cells?}`; without `cells` every cell of the rect is set), `type`, `commit?` (true), `skip_funds?` (false), `check_slope?` (false), `skip_size?` (false), `check_lot_inside?` (true), `expect?` (`{ok, cost, error, region_cells}`) | slot 11 (§6); `region_cells` are the cells still set in the caller's region afterwards |
| `zone_do_parcellization` | `region`, `type`, `flag?` (false), `keep_log?` | slot 6 (§7.1); clears the parcel log first |
| `zone_draw_street` | `region`, `x1`, `z1`, `x2`, `z2`, `per_cell?` (false), `keep_log?` | `DrawStreet` (§7.10) with `region` as the restriction |
| `zone_highlight_parcels` | `region`, `type`, `point?` (`[x, z]` or null), `out?` (true: pass an out pointer), `initial_out?` (a region, or null), `expect_out?` (null or partial `{rect, cells, count}`), `keep_log?` | slot 5 `HighlightParcels` (§7.1, test 33); clears the parcel log first |
| `zone_lay_in_jog_streets` | `region`, `kind`, `highlight?` (false), `keep_log?` | `LayInJogStreets` (§7.12) on its own with `region` as the restriction; clears the parcel log first |
| `zone_examine_boundaries` | `region`, `type`, `expect` (`{flag, d, q, scores, conditions}`) | `ExamineBoundaries` (§7.3) on its own |
| `zone_determine_lot_size` | `region` (the cropped region), `type`, `expect` (`{width, depth, min_depth, max_depth, block}`) | `DetermineLotSize` (§7.2.1) on its own (test 30) |
| `zone_simple_parcellize` | `region` (the restriction region), `type?` (8), `d`, `q`, `n`, `keep_log?` | `SimpleParcellize(n)` (§7.5) with D and Q set directly (test 32); clears the parcel log first |
| `zone_parcellizable_region` | `region`, `type`, `highlight?` (`{virtual: [[x, z]], deleted: [[x, z]]}`: highlight mode over those virtual-lot and virtually deleted cells), `expect` (`{cells, virtual_deleted}`, each sorted) | `CreateParcellizableRegion` (§7.4) on its own |
| `zone_call` | `target?` (`"developer"` (default) or `"manager"`), `method`, `args?`, `expect?` | manager: `zone_count`, `zone_type`, `is_rci_zone`, `development_failure_count`, `undeveloped_total`, `abandoned_tile_count`, `min_zone_size`, `max_zone_size`, `zone_display_alpha`, `default_zone_display_alpha`, `zone_drag_color`, `texture_for_zone`, `is_loaded`; developer: `can_create_lot`, `create_lot_if_possible` (`{ok, lot}`), `get_lot_count`, `master_lots`, `get_random_lot`, `get_lots_in_cell_rect`, `get_lots_surrounding_rect`, `exists_network_of_type` (a fourth argument true runs it in highlight mode), `exists_connection_of_type` (`[x, z, flags, side, type, highlight?]`), `is_cell_water`, `get_occupant_lot`, `lot_savvy_terrain_altitude` (`{alt, ok}`), `neighbor_cells_at_edge_of_lot` (`{out, al}`), `delete_lots`, `is_loaded` |
| `zone_eval` | `fn`, `args`, `expect` | pure functions: `depth_rule [rem, front, far, min, max, ld, ps]`, `depth_rule_row [front, far, min, max, ld, ps]` (rem 1..12), `middle_row [o0, so, b1, b2, lw]`, `zone_density [zone]`, `block_depth [density, depth, residential]`, `capacity_value [k, n, avg]` (§7.2.1 step 6), `adjust_depth_range [min, max]` (→ `[min, max]`, §7.2.1 step 7), `occupant_cell [p]`, `savvy_cell [p]`, `derive_seed [session]` |
| `zone_expect` | `state` (partial: `manager_initialized`, `manager_loaded`, `developer_initialized`, `developer_loaded`, `lot_count`, `real_mode`, `orientation_flag`, `orientation_d`, `orientation_q`, `scores`, `conditions`, `lot_size`, `deferred`, `funds`, `place_count`, `failure_map1_count`, `lcg`, `next_raw` (a non-consuming peek), `draw_count`, `cfg_refs` (`[[id, addRefs, releases]]` per configuration touched since the last `zone_clear_events`, by ID), `current_zone`, `highlight_point`, `virtual_lots` / `virtually_deleted` (`[[x, z, x1, z1, facing]]` by cell), `drafts` (`[[tool, x, z, flags, [mask]]]`), `tool_resets` (tool kinds reset since the last clear), `levelling_count` (`AttemptAreaLevelling` calls since the last clear), `locked_lots`), `zones` (`[[x, z, v]]`), `zoning_changes` (partial `{zone, check_slope, rect, cells}` per posted message) | the world and both classes |
| `zone_expect_parcel_log` | `log` (strings), `keep?` | the ordered log since the last clear: lot creations as `(x1,z1)-(x2,z2) fN`, `Place` calls as `P(x1,z1,x2,z2)` |
| `zone_expect_draws` | `draws`: each `"Zn"` (kind only) or `["Zn", arg, value]` (exact); `keep?` | the draws since the last check (§9) |
| `zone_expect_log` / `zone_expect_event_count` / `zone_clear_events` | see the shared conventions (`clear_events` also clears the parcel log, the posted messages, the demolition list and `cfg_refs`) | host calls |
| `zone_save` / `zone_load` | `slot`; load: `expect?` (true = a clean store load) | the class-object store; a load replaces both classes and rebuilds the master list in first-cell order (§10.2) |
| `zone_record` | `target?` (`"manager"` (default) or `"developer"`) and one of `hex`, `hex_prefix`, `byte_length`, or `read` (+ `expect?`) | one class's record bytes (§10) |

Zones events: `manager_acquire_services`, `manager_release_services`, `developer_acquire_services`,
`developer_release_services`, `subscribe` / `unsubscribe` (`who`, `messages`), `shutdown_grid` (`class`),
`acquire_network_tools`, `release_network_tools`, `zoning_change` (`zone`, `check_slope`), `demolish_lots` (`commit`,
`count`, `flags`), `demolish_region` (`count`), `remove_occupant`, `street_joining_helper` (`data4`),
`place_zone_result` (`ok`, `cost`, `error`), `zone_lot` (`lot`), `bds_developer` (`type`), `estimate_desirability`
(`dev`, `x`, `z`), `lot_configurations` (the configuration manager's walk), `occupant_exists` (`tool`, `x`, `z`, `flag`).

## Traffic network ops

The traffic network vectors (`tn11_…`) drive `fc4_core::traffic_network::NetworkModel` (`docs/specs/traffic-network.md`)
over one scripted world implementing `NetworkHost`. Every op name starts with `tn_`. The default world is a 4 × 4-cell
city of 16 m cells with the shipped tuning (§4.5) and no speeds, occupants, lots, water or neighbour connections, as
in the §11 preamble. Cells map to world positions at their centre (`16x + 8`, `16z + 8`). Occupants answer
`PathConnection` only for the listed (path type, entry, exit) triples and the turn-around test only for the listed
entries; `GetPathInfo` always succeeds. A lot's anchor (`GetLot(x, z, true)`, `GetLocation`) is its NW cell.

Floats are compared bit-exactly as `"0x3C23D70A"` strings (upper case, 8 hex digits); routes are lower-case hex byte
strings (`"0000004005aa02"`).

### Setup

| op | Arguments | Effect |
|---|---|---|
| `tn_env` | any of `cells` (`[x, z]`), `tuning` (`"shipped"` or null = the constructor's), `speeds` (`[[t, type, v]]`, raw "Max speed by Network" values), `cell_width` (16) | the world; read at `tn_init` |
| `tn_init` | — | `NetworkModel::new`, the tuning and the inverse speed table (§4.8 step 5), then `Init` (§1) |
| `tn_occupant` | `id`, `cell` or `pos` (`[x, y, z]`), `kind?` (`network` (default), `subway`, `prop`, `building`), `types?`, `conns?` (`[[entry, exit]]` for `path_type?` (1), or `[[pt, entry, exit]]`), `turns?` (entries), `flags?`, `tunnel?` (the paired portal's id, or true), `props?` (property map), `prop_type?`, `bounding?` (`[x1, z1, x2, z2]`), `paths?` (`[[entry, exit, vertex]]`), `near?` (`[[vertex, exit, [entries]]]`) | adds or replaces an occupant; `building` occupants answer no rectangle query |
| `tn_remove_occupant` | `id` | takes the occupant out of the rectangle queries; it stays readable, as during its removal message |
| `tn_lot` | `id`, `rect`, `facing?` (0), `building?`, `holder?` (id + `0x10000`), `pops?` (`[[[type, count]]]`, one list per map), `trip_caps?` (4 values, `GetMaxTripCapacity`), `props?` (the holder's initial properties) | a lot |
| `tn_water` | `cells` or `rect`, `depth`, `body?` (1), `under_water?` (true) | navigable cells with a depth and a water-body ID |
| `tn_damaged` | `cells` | the pothole cells (`IsRoadDamaged`) |
| `tn_connection` | `id`, `type`, `cell`, `pool?` (holder token), `pool_cells?` (the cells `GetNeighborPoolPropertyHolder` answers at; default `cell`), `trip_caps?` | a neighbour connection |
| `tn_curve` | `points` (`[[x, y]]`), `multiplier?` (1.0) | the congestion curve and transit multiplier (§4.7.3) |
| `tn_switch` | `id`, any of `load`, `capacity`, `time_factor`, `building`, `flags`, `to_mask`, `has_access_point`, `access` (`[x, z]`), `rules` (`[{kind, side, from, to}]`), `cells` (`[[x, z]]`: writes this ID into the switch ID grid and `mode_mask?` (0) into the mode grid there) | a switch table entry, inserted if missing (test setup: §11 test 10, and a switch as a load leaves it for the DoPostCityInit re-read, §4.7.2) |
| `tn_set_cell` | `cell` or `cells`, `prior_car` | pokes cell connection records (test setup, §11 test 6) |
| `tn_rng` | `seed` | a `cRZRandom` the model never sees (§11 test 12) |

### Calls and checks

| op | Arguments | Effect / check |
|---|---|---|
| `tn_call` | `method`, `args?`, `expect?` | `occupant_changed [id, removed?]`, `do_post_city_init`, `post_city_init_rebuild`, `post_city_init_finish`, `do_connections_changed [x1, z1, x2, z2]`, `set_connection [x, z, e, x', t]`, `update_intersection_data`, `get_capacity [n, x, z]` (bits), `get_connection_count [n, x, z]`, `are_lots_connected [a, b]`, `get_connected_occupant_count [lot, type]`, `get_connected_destination_count [lot, k]`, `get_subnetworks_for_lot [lot]` (`{ids, ok}`), `get_subnetworks_in_rectangle [x1, z1, x2, z2]`, `get_subnetworks_in_region [x1, z1, x2, z2, [[x, z]]]`, `get_occupant_count_for_all_subnetworks [type]`, `transit_switch_entry_cost [id]` (bits), `lot_activated [lot]` (whether a switch was created), `lot_activated_outcome [lot]` (`{outcome, switch_created, continues}`, outcome `no_building` / `returned` / `done`; `continues` = the simulator's step 7 follows, §4.7.1), `lot_deactivated [lot]`, `get_transit_switches`, `transit_switch_query_target [x, z]` (`{switch}` / `{ferry}` / null), `tunnel_changed [id, removed]`, `find_water_path [sx, sz, tx, tz]` (`{cells, g, forward, backward}` or null), `get_water_route [x1, z1, x2, z2, initial hex]` (`{ok, route}`), `water_terrain_changed [data1, data2]`, `water_update`, `has_map_exit [x, z]`, `neighbor_connection_inner_cell [x, z]` (`[x, z, side]` or null), `set_neighbour_edge [x, z, mask, pool, cost]`, `begin_ferry_pass`, `work_on_ferry_routes [max]`, `ferry_terminal_count [type]`, `all_ferry_routes`, `ferry_route_between_tiles [x1, z1, x2, z2]`, `ferry_routes_in_use`, `clear_subnetworks`, `update_subnetworks`, `fill [x, z, e]`, `begin_subnetwork_info`, `work_on_subnetwork_info [max]`, `update_subnetwork_info` |
| `tn_eval` | `fn`, `args`, `expect` (objects partial) | pure functions: `depth_byte [navigable, w]`, `step_cost [depth, g]`, `heuristic [dx, dz]`, `route_cost [cells]`, `encode_route [[[x, z]]]`, `side_of [ax, az, bx, bz]`, `tunnel_distance [dx, dy, dz]`, `inverse_max_speed [v, cell width]`, `read_rules [facing, [entries]]` (`{rules, to_mask, flags, mode_mask}`), `rule_bytes [property]` (the raw bytes of a property value such as `{"u8": [..]}`, §4.7.2), `read_rule_property [facing, property]` (as `read_rules` over those bytes, plus `count`) |
| `tn_expect_cell` | `cell` or `cells`, `expect` (partial: `conn` [9], `switch` [9], `prior_car` [4], `wormhole`, `switch_id`, `switch_mode`, `records` (each `{net, conn_count, type, inv_cap, extra_cost, flags, subnet}`, partial), `car` (the first car record, or null), `water_depth`) | the cell connection record, the network-info list and the grids |
| `tn_expect_state` | `state` (partial: `count`, `subnet_counter`, `working_len`, `published_len`, `published` (`[{population, capacity}]`), `intersection_dirty`, `intersection_rect`, `subnetworks_stale`, `network_changed`, `occupants_changed`, `next_switch_id`, `switch_ids`, `ferry_counts`, `terminal_count`, `ferry_pass_running`, `ferry_pass_terminal`, `water_dirty`, `water_dirty_rect` (`[x1, z1, x2, z2]`), `edge_density_keys`, `inv_cap_table`, `intersection_values`, `damaged_road_cost`, `posted`, `refs`) | the model state and the world's posted messages and references |
| `tn_expect_tunnel` | `cell`, `record` (null, or partial `{other, mask, type, cost}` with `cost` keyed by travel type) | the tunnel table |
| `tn_expect_lot` | `lot`, `subnetworks` (null = property `0x2A4F9C74` absent) | the lot's list |
| `tn_expect_switch` | `id`, `switch` (null, or partial `{building, load, time_factor, capacity, flags, to_mask, mode_mask, has_access_point, access, simulator_value, counts_as_intersection, rules}`) | the switch table (read without inserting) |
| `tn_expect_ferry` | `index`, `terminal` (null, or partial `{departure, arrival, water, lot, type, mask, reroute, links, neighbour_links}`) | the ferry terminal list |
| `tn_expect_rng` | `expect?` | the generator's next `RandomUint32Uniform(100)` equals a fresh generator's with the same seed |

## Pathfinder ops

The pathfinder vectors (`pf11_…`) drive `fc4_core::pathfinder::Pathfinder` (`docs/specs/pathfinder.md`) over one
scripted traffic simulator and city implementing `PathfinderHost`, with the network model's `NetworkState` set up cell
by cell (no occupants: cell records, network info, switches, tunnels and ferry terminals are written directly). Every
op name starts with `pf_`. The default world is the §11 preamble: an 8 × 8 city, `invMaxSpeed[Car][Street] = 1.5`
(every other entry `FLT_MAX`), `Curve(0.0) = 1.0`, `affectedByTraffic[Car] = canReachDestination[Car] =
generatesTraffic[Car] = true` (everything else false or 0, including the default starting costs and `incomePerTile`),
NearestDestinationAttractiveness 1.0, MaxDistanceOriginToNetwork 1, no strike (state 0), trip scale 1.0, no carpool,
all volumes and destination bytes 0, free counts 0. The timer never expires unless `deadline_reached` is set. The
work-check statics (§4.11) are fresh for each vector ("fresh process") and shared by its pathfinders. Pathfinders are
numbered by `pf?` (default 0).

Floats are given and compared as exact bit patterns `"0x3C23D70A"` (upper case, 8 hex digits; a plain number is also
accepted as input and converted to float32); the income accumulator (a double) as 16 hex digits; routes as lower-case
hex byte strings; node keys and masks as `"0x…"` (8 hex digits).

### Setup

| op | Arguments | Effect |
|---|---|---|
| `pf_env` | any of `cells` (`[x, z]`: replaces the network with an empty one of that size, with the preamble speed and curve), `can_reach` / `generates` / `affected` (lists of travel types, replacing the table), `income_per_tile` (`[[t, v]]`), `default_start_cost` (9 values), `inv_max_speed` (`[[t, type, v]]`), `curve` (`[[x, y]]`), `transit_multiplier`, `attractiveness`, `max_distance`, `strike` (1 = strike), `trip_scale`, `carpool`, `free` (4 counts), `income` (the accumulator), `deadline_reached` | the simulator's tables and inputs |
| `pf_cell` | `cell`, `cells` or `rect`, any of `conn` / `switch` (`[[t, word]]`), `prior_car` (4 words), `wormhole`, `info` (`[[network, type, inv_cap, extra_cost]]`, appended) | pokes cell connection records and network info |
| `pf_street` | `cell`, `cells` or `rect` | the §11 preamble street cell: road info `{0.0625, Street, 0.0}`, `conn[Car] = 0x0104`, `switch[Car] = 0xFFFF`, `priorCar = 0xFFFF`, wormhole 0 |
| `pf_lot` | `id`, `rect`, `holder?` (id + `0x10000`), `max?` / `used?` (the holder's 4 trip capacities), `dest?` (destination bits written on every cell of the lot) | a lot (`GetLot`, its holder, its bounding rect) |
| `pf_capacity` | `holder`, `max?`, `used?` | `GetMaxTripCapacity` and the used capacity of a holder |
| `pf_dest` | `cell`, `cells` or `rect`, `bits` | destination bitmask bytes (off-map cells allowed) |
| `pf_volume` | `cell`, `cells` or `rect`, `n?` (1), `value` | volume grid values |
| `pf_neighbor_cost` | `cell` (an off-map edge cell), `cost`, `mask?` (0), `pool?` (0) | the network's neighbour edge record at that cell (traffic-network.md §2.7.1), read by `GetTrafficNeighborConnection` (§4.13 step 3) |
| `pf_pool` | `cell`, `cells` or `rect`, `holder`, `max?`, `used?` | the neighbour pool's property holder at off-map cells (§4.10.1) |
| `pf_ferry` | `cell`, `mask`, `arrival?` (`[x, z]`, (0, 0)), `links?` (`[[x, z, cost]]`), `repeat?` (1: the link list is repeated), `neighbour_links?` (`[[off x, off z, edge x, edge z, cost]]`) | a ferry terminal departing at `cell`: wormhole bit 4, the terminal list and the departure table; §4.6's off-map fallback runs over its neighbour links and starts at `arrival` |
| `pf_tunnel` | `cell`, `other`, `mask?` (`0x1FE`), `cost?` (`[[t, v]]`) | a tunnel portal: wormhole bit 2 and the tunnel table |
| `pf_switch` | `id`, any of `load`, `capacity`, `time_factor`, and `cell`, `cells` or `rect` | a transit switch table entry and the switch ID grid |
| `pf_create` / `pf_new` | `pf?` | `CreatePathFinder` (constructor, AddRef, Init) / the bare constructor |

### Calls and checks

| op | Arguments | Effect / check |
|---|---|---|
| `pf_call` | `pf?`, `method`, `args?`, `expect?` | the slots: `query_interface [iid]`, `add_ref`, `release`, `init`, `shutdown`, `set_flag [bits]`, `clear_flag [bits]`, `clear_all_flags`, `set_route_info [bool]` (a fresh deque or none), `set_trip_destination_filter [f]`, `set_destination_flag [bits]`, `clear_destination_flag [bits]`, `clear_all_destination_flags`, `set_travel_type_flag [mask, also starting]`, `clear_travel_type_flag [mask]`, `clear_all_travel_type_flags`, `set_starting_cost_array [9 floats or null]`, `set_max_trip_length [v]`, `set_traffic_density [n]`, `set_start_rect [x1, z1, x2, z2]`, `set_end_rect [x1, z1, x2, z2]`, `set_max_work_time [ms]`, `reset`, `find_path`, `get_traffic_arrived`, `get_trip_length` (bits), `get_trip_length_scaled_for_display` (bits), `get_end_cell` (`[x, z]`), `get_last_travel_type`, `get_compressed_route [initial hex?]` (`{bytes, mask}`), `was_trip_reversible`, `apply_traffic [style]`, `release_path_nodes`; and the internal `heuristic [x, z]` and `score [x, z, g]` (bits) |
| `pf_expect` | `pf?`, `state` (partial: `ref_count`, `destroyed`, `initialised`, `city_size`, `flags`, `filter`, `destination_flags`, `allowed`, `starting`, `start_costs`, `trip_length`, `density`, `arrived`, `start_rect`, `end_rect`, `search_rect`, `excluded_rect`, `route` (`[[x, z, t]]` or null), `max_work_time`, `suspended`, `max_cost` (9), `weight`, `over_budget` (9), `disabled`, `node_count`, `nodes` (allocation order, each partial `{index, key, cell, t, side, g, f, parent, open, closed}`; the list length must match), `open_set` (node indices in heap order), `result` (node index or null), `end_cell`, `last_travel_type`, `destination_type_reached`, `park_and_ride` (`[x, z, side]`), `work_counter`) | the object's state |
| `pf_work_check` | `set?` (`[m, a]`), `expect?` (`[m, a]`) | the shared work-check multiplier and adaptation counter |
| `pf_expect_world` | `state` (partial: `volumes` (`[[n, x, z, bits]]`, every value written, sorted), `dest` (`[[x, z, bits]]`, every byte written, sorted), `free`, `income`, `edge_density_keys` (sorted), `switch_loads` (`[[id, bits]]`)) | the simulator side |
| `pf_expect_edge` | `cell`, `t?` (1), `evening?` (false), `counts` (`[[entry, exit, value]]`) | the record's `current` counters: the listed ones, every other counter 0 (a missing record reads as zeros) |
| `pf_eval` | `fn`, `args`, `expect` | pure functions: `heap_order [[f]]` (the pop order of pushes, §4.7), `boundary_cells [rect]`, `start_ring_cells [start, ring]`, `grow_clamp [rect, d]` (clamped to the world's city), `encode_route [[[x, z, t]]]` (`{bytes, mask}`, over the world's network), `encode_unit_steps [x0, z0, t, code, count]`, `distance_to_rect [x, z, rect]` (bits), `node_key [prefix, t, x, z]`, `bucket [key]` |

## Residential ops

The residential vectors (`rs15_…` for the tests of `docs/specs/residential.md` §15, `reh11_…` for the tests of
`docs/specs/residential-education-health.md` §11, `rs00_…` for the rest) drive the residential simulator
(`fc4_core::residential`, with its education and health half) over one scripted world implementing
`ResidentialHost` and `EducationHealthHost`: the three growth developers' population maps (`int16`, tract size
`dev_size`), the BDS building map, the pollution source values (one value per kind, at every cell), the month and
day number, the terrain altitude, the effects manager, occupants (their groups, properties, lots, positions,
building types and budget items), the budget simulator's departments (the simulator's own are tokens 900, 901, …
in creation order; the ones a vector scripts with `departments` have the tokens it gives), department funding,
the locally funded set, line items (tokens 700, 701, … in creation order), the ordinances that are on, and the
automata controllers. Every op name starts with `residential_`. The default world is a 16 × 16-cell city,
`dev_size` 4, the shipped tuning (`shipped_tuning()`), month 6, day 1000, altitude 0, every service present, effect
token 500, no pollution, no building map, no occupants, no scripted departments (`GetDepartmentBudget` fails) and
no ordinance on.

Objects are tokens and grids are core `SimGrid`s, so a record read back shares the writer's grids.

### Environment

| op | Arguments | Effect |
|---|---|---|
| `residential_env` | any of `cells` (`[x, z]`), `dev_size`, `month`, `today`, `tuning` (`"shipped"`, `"empty"` or null = no exemplar), `tuning_props` (`{"0xID": value or null}`, merged into the tuning; null removes), `pollution` (`[air, water, garbage]`), `altitude`, `effect` (the effect `CreateVisualEffect` returns, or null), `building_map` (`{size?, cells: [[x, z, key]]}` or null), `demand_node`, `message_server`, `simulator`, `served` (`[s, h]`: the fixture below), `groups` (`{"occupant": ["0xGROUP", …] or null}`; a listed occupant is a building occupant), `props` (`{"occupant": {"0xID": value or null} or null}`: the occupant's properties, values as in `tuning_props`, for example `{"u32": 1000}`, `{"f32": 0.5}` or `{"f32": [x0, y0, x1, y1]}` for a curve), `lots` / `positions` (`{"occupant": [x, z] or null}`: the lot centre, else the occupant's position, default (0, 0)), `building_types` / `slot11` (`{"occupant": value}`), `departments` (`{"0xID": {token, funding?} or null}`: `GetDepartmentBudget` and the department's funding, default 1.0), `items` (`{"occupant": {purpose: {department, line?, index?, cost?, local?} or null}}`: the budget items; a purpose is `school_staff`, `school_vehicle`, `hospital_staff`, `hospital_vehicle` or an ID), `locally_funded` (`[[department token, occupant, purpose]]`, replaces the set), `line_items` (`[[department token, line, token, count]]`), `ordinances` (`{"0xID": {misc properties} or null}`: the ordinances that are on) | the world; `cells` and `dev_size` recreate the developers' maps; grids are sized at `Init` |
| `residential_create` | `rng?` (`{draws: [v, …]}`: each `RandomDoubleRangeUniform(a, b)` returns the next value verbatim, or `b` when none is left; or `{session}`: `derive_seed(session, 0x4990C013)`), `proximity_size?`, `work_budget?` | a new constructed simulator, which replaces the current one |
| `residential_tract` | `x`, `z`, any of `pop` (current populations), `new` (the developers' maps), `steady` (both), `eq`, `hq` (floats or `"0x…"` bit patterns), `age` | one population tract |
| `residential_grid_set` | `grid`, `cells` (`[[x, z, v]]`) and / or `all` | writes a grid: `proximity1`–`3`, `population1`–`3`, `eq1`–`3`, `hq1`–`3`, `age`, `school_grade`, `hospital_grade`, `school_coverage0`–`6`, `hospital_coverage0`–`2`, `temporary`, `developer1`–`3`, `building_map` |
| `residential_set` | `work_budget?`; `state`: any of `state`, `dirty`, `population`, `running_population`, `le_sum`, `age_count` / `age_eq_sum` (an array or `{"bucket": v}`), `school_covered`, `hospital_covered`, `proximity_size`, `unused` (`[f1, f2]`), `health` / `education` (block fields), `lists` (`pending_school`, `pending_hospital`, `other`, `school_groups`, `hospital_groups`, `health_affecting`; null entries allowed), `school_auto_budget`, `hospital_auto_budget`, `iterator` (`{step?, advance?}`) | pokes state (test setup) |

The `served: [s, h]` fixture is applied after the next successful `residential_init`: `s` schools (occupants
9001, …) and `h` hospitals (9101, …) of capacity 100 (units 0 with the shipped chunks) and radius 10 m at lot centre
(32, 32), each in its service's first group list with one reference, and population (1, 0, 0) on tract (0, 0).
Each building counts that tract once in its coverage state (REH §4.4 [quirk]), so states 5 and 7 serve `s` and
`h`.

### Calls and checks

| op | Arguments | Effect / check |
|---|---|---|
| `residential_init` / `residential_shutdown` | `expect?` | `Init` (then the `served` fixture) / `Shutdown` |
| `residential_message` | `type` (`SimBegin`, `SimNewDay`, `SimNewMonth`, `SimIdle`, `OccupantInserted`, `OccupantRemoved` or an ID), `occupant?` (void1), `data3?` (default the building type `0x278128A0`), `count?`, `expect?` | `DoMessage`, each result checked |
| `residential_run_to` | `state`, `steps?` | NewDay until the state is reached (every step must finish); `steps` checks how many were taken |
| `residential_call` | `method`, `args?`, `expect?`, `bits?`, `partial?` (objects compared key by key) | a method: `do_population_change [x, z]`, `hq_loss [x, z]`, `tract_quotient [eq/hq, x, z]`, `get_cell_eq` / `get_cell_hq [x, z]`, `get_cell_eq_by_wealth` / `get_cell_hq_by_wealth [x, z, w]`, `get_global_eq`, `get_global_hq`, `get_cell_life_expectancy [x, z]`, `get_global_le`, `get_cell_workforce_percent [x, z]`, `get_global_workforce_percent`, `get_average_new_age_by_wealth [w]`, `get_population`, `max_quotient`, `total_city_upkeep_cost`, `average_grid [eq/hq]` (`{min, max, values (x outer, z inner), instance, size, refs}`), `average_grid_is_new [eq/hq]`, `quotient_min_max [eq/hq]` (`[minX, minZ, maxX, maxZ, min, max]` or null), `grid_triple [eq/hq/population]` (`{result, instances, added}`: the reference count each grid gained), the strike slots (`school_is_on_strike`, `end_school_strike`, `chance_of_school_strike`, and the health ones), `school_building_count`, `school_buildings [groups or null]`, `hospital_building_count`, `hospital_buildings`, `health_affecting_buildings`, the auto-budget slots (`global_auto_budget_schools`, `set_global_auto_budget_schools [v]`, the hospital ones, `get_auto_budget [occ]`, `set_auto_budget [occ, v]`), `set_occupant_funding [occ, staff, vehicle, school]` (slot 19), `get_occupant_funding [occ, school, a?, b?]` (slot 20 with the caller's a, b, default 1.0: `[a, b]`), `efficiency_ratings [occ, school, ignore_strike]` (`[e1, e2]`), `base_staff_efficiency [occ, school]`, `do_building_changed [occ, added]` (REH §4.1), `occupant_service [occ]` (`"school"`, `"hospital"` or null), `ordinance_decay_products` (`[edu, health]`), `ordinance_modifiers [school]` (`[m1, m2, m3]` of the current effect list), `school_system_rating` / `health_system_rating` (slots 12 / 13), `school_system_totals` / `hospital_system_totals [[groups]]` (slot 14: `{count, totals}`), `school_query_data` / `hospital_query_data [occ]` (slots 26 / 27: `{capacity, effective_capacity, capacity_per_staff, effective_per_staff, served, grade, staff_funding, staff_cost, vehicle_funding, vehicle_cost}` or null), `estimate_occupant_capacity [occ]` (slot 28: `[capacity, staff]`), `occupant_coverage [occ, level]` (slot 45: `[dist, radius]` or null), `estimate_ideal_funding [occ]` (slot 58: the funding or null), `get_proximity [w, x, z]` (the value, or null without a map), `add_ref`, `release`, `class_id`, `tuning` (the population tuning); and the pure `age_bucket [age]`, `pct [x]`, `rhe [x]` (round half to even), `tracts_in_radius [px, pz, r, w, countX, countZ]` (`[[x, z], …]`, REH §4.4), `slot [n]` (`[name, pops, return]` with return `AL`, `EAX`, `ST0`, `hidden` or `none`), `slot_count`, `query_interface [iid]` |
| `residential_expect` | `state` (partial; the `residential_set` fields plus `initialized`, `city_started`, `ref_count`, `destroyed`, `message_server_held`, `population_size`, `eq_size`, `tuning_loaded`, `temporary` (exists), `departments` (`[health1, health2, education1, education2]`), `iterator` (`{state: fresh/running/done, x, z, step, rect}`), `school_grade_total` / `hospital_grade_total` (RS13), `school_effects` / `health_effects` (the number of ordinances in each effect list)); `bits?` | the simulator state; objects compare key by key |
| `residential_expect_props` | `occupant` with `props` (`{"0xID": value or null}`, null = absent; a u32 compares as an integer, an f32 as a float), `locally_funded` (the whole set, `[[department, occupant, purpose ID]]`), `line_counts` (`{"token": count or null}`); `bits?` | the world's occupant properties and budget state |
| `residential_expect_tract` | `x`, `z`, any of `pop`, `eq`, `hq`, `age`; `bits?` | one tract |
| `residential_expect_grid` | `grid` (as for `grid_set`); any of `exists`, `size`, `counts`, `instance`, `cells`, `all`, `others` (every tract not in `cells`); `bits?` | a grid |
| `residential_expect_refs` | `refs` (`{"id": count}`) | references the simulator holds on world objects (AddRef − Release) |
| `residential_expect_events` / `residential_expect_event_count` / `residential_clear_events` | see the shared conventions | host calls |
| `residential_write` | `slot`, `expect?`, `length?` | `Write`; the bytes and the registry entries are kept in `slot` |
| `residential_read` | `slot` or `hex`, `version?` (rewrites the first two bytes), `length?` (truncates), `patch?` (`[{offset, hex}]`), `reuse?`, `rng?`, `expect?` | `Read` into a new constructed simulator (the class store's path), or with `reuse` into the current one |
| `residential_expect_record` | `slot`, any of `length`, `at` (`[{offset, hex}]`), `same_as` (another slot) | record bytes |

With `"bits": true` every float in the result becomes its float32 bit pattern (`"0x3F4CCCCC"`), compared exactly.

Residential events:
- lifecycle: `acquire_services`, `acquire_developers`, `demand_node`, `message_server`, `budget_simulator`, `simulator`, `add_agent` (`level`, `name`, `flags`), `remove_agent`, `subscribe` / `unsubscribe` (`type`), `release_message_server_and_budget`, `release_services`, `queue_length` (`series`, `length`);
- departments and grids: `department` (`id`, `name`, `max`, `init`, `token`), `release_department` (`token`), `shutdown_grid` (`instance`; the temporary grid is 0);
- references: `add_ref` / `release` (`id`);
- the month pass: `get_month`, `effect` (`name`), `transform` (`effect`, `pos`), `start_effect` (`effect`), `history` (`series`, `value`), `rng` (`lo`, `hi`, `value`);
- strikes and ordinances: `strike` (`type`, `occupant` (null for the city-level message), `data2`, `data3`),
  `controllers` (`name`, `occupant`, `added`), `ordinance_effects` (the list of ordinances that are on was read);
- the budget (REH §4.1, §4.2): `change_line` (`occupant`, `index`, `line`), `create_line` (`department`, `line`,
  `local`, `token`), `display_flag` (`line_item`, `flag`, `on`), `remove_line` (`department`, `line`), `line_count`
  (`line_item`, `count`), `add_local` / `remove_local` (`department`, `occupant`, `purpose`), `set_local`
  (`department`, `occupant`, `fraction`, `purpose`).

## Landfill ops

The landfill vectors (`lf11_…`) drive `fc4_core::landfill` (`docs/specs/landfill.md`) over one scripted city
implementing `LandfillHost`. Every op name starts with `landfill_`. The default world is the one §11 states: a
64 × 64-cell city, flat at 270 m, sea level 250, the shipped tuning (§4.1), `tileCap` 30000, sanitation funding
100 %, every cell of zone type 14, no roads, not connected, supply 0. Cell centres are at `16·x + 8`. Landfill tiles
are core objects the world keeps per cell; the zot is a prop whose position the world records.

The lot developer's `AttemptAreaLevelling` (slot 21) is the core function `attempt_area_levelling` (§4.5.3) over the
world's stubbed slots, as test 22 asks: `GetAverageAreaAltitude` returns the stubbed `avg` (or, when none is set, the
mean of the rect's cell altitudes), `GetAreaSlope` the stubbed `slope` (0), the lot developer's tuning
`maxAreaSlopeAllowedForLevelling` the stubbed `tuning` (40); `ModifyTerrainHeight` is logged and, with `apply` (true),
sets the cells of `[x1, x2) × [z1, z2)` to the target and posts `0x278E6003` with `data4 = 5`. `PlaceZone` rezones the
cells and **posts** the zoning message (§4.4.8); posted messages wait until `landfill_deliver`.

### Setup

| op | Arguments | Effect |
|---|---|---|
| `landfill_env` | any of `cells` (`[w, h]`), `sea`, `altitude` (the flat ground), `tuning` (`"shipped"`, null = the code defaults, or a property map), `supply`, `tile_capacity`, `uncollected`, `funding`, `current` (line item 30's current expenses), `department` (bool), `connected`, `networks` (`[[x, z]]`), `zone_type` (the default zone type), `view_level`, `levelling` (`{avg, slope, tuning, apply}`; `avg` null = the cell mean; `"FLT_MAX"` / `"NaN"` allowed) | the world |
| `landfill_altitude` | `cells` (`[[x, z, a]]`) and/or `rect` with `value` | cell altitudes |
| `landfill_zone_types` | `cells` (`[[x, z, type]]`) | the zone manager's map |
| `landfill_create` | `post_city_init?` (true), `expect?` (Init's result) | a developer and its `Init` |
| `landfill_heaps` | `zone?` (0) and `heaps` (`[[hx, hz, valid, active, capacity, contained]]`, replaces the vector), or `index` with `heap`, or `contained` (every heap) | test setup |
| `landfill_set` | `zone?`, any of `level_altitude`, `connected`, `clear_cells` (`[[x, z]]`) | test setup |

### Calls and checks

| op | Arguments | Effect / check |
|---|---|---|
| `landfill_zone` | `rect`, `type?` (14), `expect?` | the zoning message `0x07699455` over the whole rect (§3.2, §4.4.1) |
| `landfill_message` | `type`, `data1?` … `data4?`, `rect?` (the region), `expect?` | `DoMessage` |
| `landfill_month` | `count?` (1) | SimNewMonth (§4.7.1) |
| `landfill_terrain` | `rect`, `reason?` (1), `altitude?` (sets the rect's cells first) | `0x278E6003` over the rect with `data4 = reason` (§3.2) |
| `landfill_deliver` | `count?` | delivers the posted messages in order |
| `landfill_shutdown` | | the developer's `Shutdown` |
| `landfill_dezone_cell` | `zone?`, `cell`, `change_zone?` (false), `remove_texture?` (false) | `DezoneCell` (§4.4.8) |
| `landfill_decay` / `landfill_redistribute` | `zone?`, `index` (decay) | `DecayGarbage` (§4.7.3, the developer's tuning) / `RedistributeGarbage` (§4.7.4) |
| `landfill_update_tiles` | `zone?`, `index`, `seed?` (1), `draws?` | `UpdateSurroundingTiles` (§4.8) with a CRT `rand()` at `seed`; `draws` = the expected number of `rand()` calls |
| `landfill_levelling` | `args` (`[rect, f, withWater, waterFlag, maxAreaSlope]`), `expect` | `AttemptAreaLevelling` (§4.5.3) over the stubbed slots |
| `landfill_call` | `method`, `args?`, `zone?`, `expect?` | `get_zoning_cost [rect]` (`[low, high]`), `get_landfill_usage` (`[usage, capacity]`), `get_landfill_tile_count`, `get_maintenance_costs`, `get_current_budget`, `get_efficiency`, `update_capacity_values`, `get_landfill_zone_from_cell [[x, z]]` (the zone's index or null), `recalculate_capacity`, `get_capacity` (`[available, contained, capacity]`), `is_zone_connected` |
| `landfill_expect` | `state` (partial: `zones`, `maintenance`, `capacity_total`, `used_total`, `loaded`, `department`, `crt_state`, `supply`, `tiles` (`[[x, z, tile, orientation, y]]` by z then x), `active_landfill_cells`, `zots`, `posted`), `zone?` with `zone_state` (partial: `rect`, `cells`, `mask` (x outer, z inner), `w`, `h`, `heaps`, `valid_heaps`, `connected`, `zot` (`[x, y, z]` or null), `level_altitude`, `completely_dezoned`), `zone_types` (`[[x, z, type]]`) | the developer, a zone and the world |
| `landfill_record` | `hex` or `hex_prefix` | the developer's record (§6.1) |
| `landfill_zone_record` | `read` (a §6.2 record), `expect?`, `rewrite?` (the write reproduces the bytes), `init?`, `zone_state?` | a fresh zone read from bytes |
| `landfill_reload` | `expect?` (true), `post_city_init?` (false) | writes the developer's record and reads it into a new developer (zones with a zot cannot be reloaded: the record holds a reference) |
| `landfill_expect_events` / `landfill_expect_event_count` / `landfill_clear_events` | see the shared conventions | host calls |

Landfill events: `add_agent` (`name`, `level`, `flags`), `remove_agent`, `subscribe` / `unsubscribe` (`type`),
`supply` (`delta`), `put_in` (`value`), `garbage_capacity` (`value`), `full_expenses` (`line`, `value`), `release`
(`token`), `place_zone` (`rect`, `type`), `set_altitude` (`rect`, `altitude`), `redisplay` (`rect`), `tile` (`x`, `z`,
`tile`, `orientation`, `y`), `tile_remove` (`x`, `z`), `tile_move` (`x`, `z`, `y`), `zot_add` (`pos`), `zot_remove`,
`levelling` (`rect`, `f`, `with_water`, `water_flag`, `max`, `result`), `modify_terrain` (`first`, `second`, `rect`,
`altitude`, `lot`, `reason`, `cost_out`), `truck` (`enabled`), `seagull` (`on`), `history` (`series`, `value`).

## Neighbours ops

The neighbours vectors (`nb18_…`) drive `fc4_core::neighbors` (`docs/specs/neighbors.md`) over one scripted region
implementing `NeighborsHost`. Every op name starts with `neighbors_`. The default world: a 64 × 64-cell city, today
1000 (Julian day), no regional cities and no current regional city (vectors add them), the shipped tuning (§12),
session seed `0x12345678`, terrain altitude 270 (`GetAltitude`, the marker height at creation) and surface 280
(`GetMaxSurfaceLevel`), every utility figure 0 and every department missing, landfill usage and capacity 0. Cell
centres are at `16·x + 8`; marker props get tokens 256, 257, … and live in the cell of their position.

Regional cities are numbered by the vector. `FindConnectionOnOtherSide` asks the neighbour's own connection list
(`conns`); `GetAllConnections` of another city is its `links`. `SetLocalDeals` reproduces the foundations §11.3 quirk:
deals with a missing region city on either side leave the simulator's list. Status reports the simulator **posts**
are logged and queued (`neighbors_deliver` delivers them as `0xEA418D8F` with `data1` = the status); reports
`SetAmount` **sends** are logged and delivered at once (unless `deliver_sent` is false), as the message server does.

Deals are named by integers: a vector names its own (`neighbors_deal`, `id`); a deal the core created (clones,
proposals, Init's rebuilt list) gets 1000, 1001, … in the order the runner first sees it (an event or a result).
Deal types may be written `"E0"` … `"E5"` (§3), connection types by their SDK names in lower case (`"road"`,
`"power_pole"`, `"ground_highway"`, …) or numbers.

### Setup

| op | Arguments | Effect |
|---|---|---|
| `neighbors_env` | any of `cells` (`[w, h]`), `today`, `current` (a city or null), `session`, `tuning` (`"shipped"`, null, or a property map), `tuning_remove` (`[IDs]`), `figures` (fields of `UtilityFigures`: `total_funds`, `monthly_income`, `power_production`, `power_requirement`, `water_capacity`, `water_demand`, `garbage_recycled`, `waste_to_energy_capacity`, `incinerator_capacity`, `garbage_abducted`, `garbage_produced`, `waste_to_energy_expenses`, and the departments' `power_department_expenses`, `water_department_expenses`, `sanitation_department_expenses`, null = missing), `landfill` (`[usage, capacity]`), `altitude`, `altitude_at` (`[[px, pz, y]]`: `GetAltitude` at those world positions, §6.3), `surface`, `deliver_sent`, `city_object` (whether the current city object exists, default true; §8.2) | the world |
| `neighbors_city` | `id`, any of `pos`, `size` (`[1, 1]`), `established` (true), `budget`, `income`, `stats` (`{name: [power, water, garbage]}` for `exported`, `imported`, `produced`, `demanded`, `cost_per_unit`; merged), `conns` (`[[type, x, z, active]]`), `links` (`[[type, rx, rz]]`), `local_deals` (`[deal spec]`) | adds or updates a regional city |
| `neighbors_occupant` | `id`, `kind` (`"network"`, `"power_pole"` or a number), `networks?`, `cell?`, `pole_flag?` | an occupant for §6.5 / §11.1 |
| `neighbors_create` | `init?` (true; `expect` = its result), `ready?` (false: ConnectionsReady) | a new simulator (replaces the old one) |
| `neighbors_deal` | `id`, a deal spec, `adopt?` (true: appended to the simulator's list, no push) | a deal |
| `neighbors_deal_set` | `deal`, a deal spec | overwrites the deal's fields (no reports) |
| `neighbors_connection_set` | `connection` (`[type, x, z]`), `active?`, `visited?` | one of our connections |

A deal spec: `type`, `other` (a city: its position) or `other_side` (`[x, z]`), `city` or `city_side` (default: the
current city's position), `amount`, `inc_amount`, `inc_amount2`, `last_amount`, `last_inc_amount`, `amount_used`,
`cost`, `flags` (default `0x81`), `start`, `expiration`, `current`, `suspension`, `properties`.

### Calls and checks

| op | Arguments | Effect / check |
|---|---|---|
| `neighbors_message` | `type`, `data1?` … `data3?`, `occupant?`, `expect?` | `DoMessage` |
| `neighbors_month` | `today?`, `count?` (1) | SimNewMonth |
| `neighbors_deliver` | `max?` (1000), `count?` | delivers the posted status reports in order, including the ones posted meanwhile |
| `neighbors_call` | `method`, `args?`, `expect?` | `init`, `shutdown`, `create_connection [t, x, z, flag]` (true = created), `request_create_connection [t, x, z]` (`"exists"` or `{confirm: city or null}`), `delete_connection`, `delete_occupant_connection [occ]`, `delete_all_connections_of_type [t]`, `has_connection`, `find_city_on_other_side_of_connection [x, z]`, `find_connection_on_other_side [t, x, z]` (`{token, active}` or null), `neighbour_cell [x, z]` (`[city, x, z]`), `connections`, `connections_of_type [t]`, `connections_to_city [c]`, `connections_of_type_to_city [t, c]`, `find_unvisited_connection [t, c]`, `find_connection_suitable_for_deal [type, c]` (connections as `[type, x, z]`), `adjacent_cities [flag]`, `connected_cities [flag]`, `all_connected_cities [flag]`, `get_deal_cost [type, amount, c]`, `get_deal_cost_trunc` (the same, truncated), `get_deal_max_amount [type, c]`, `update_commodity_stats [c]` (`c` is a city object, NB12: `"city"` = the current one; a number, a regional city's ID included, is another object; null none), `total_monthly_expense`, `total_monthly_income`, `propose_deal [type, c]`, `propose_deals [add]` (deal IDs), `add_deal [deal]`, `set_ui_disable_flag [b]`, `mark_deals_dirty`, `find_deals_all`, `find_deals [type]`, `find_deals_with [type, c]`, `clone_deal_data [spec, silent]`, `simulate_deals`, `set_two_curves [b]` |
| `neighbors_deal_call` | `deal`, `method`, `args?`, `expect?` | `set_amount [a]`, `set_flags [f, on]`, `cancel`, `activate`, `update_cost`, `update_state`, `clone [silent]` (the clone's ID), `check_flags_and [f]`, `check_flags_or [f]`, `other_city` |
| `neighbors_geometry` | `fn`, `args`, `expect` | the static helpers of §2: `side [x, z, countX, countZ]` (`"west"`, `"north"`, `"east"`, `"south"`, `"interior"`), `neighbouring_region_cell [side, x, z, pos, size]`, `neighbour_connection_cell [side, x, z, regionCell, npos, nsize]` (`[0, 0]` for interior, NB6), `is_valid_connection_cell [x, z, size]`, `connection_inner_cell [x, z, countX, countZ]` (null inside or at a corner), `inner_cell` |
| `neighbors_price` | `buyer`, `seller` (statistics), `inputs` (`base`, `scale`, `min_markup`, `max_markup` as f32 values widened, `existing`), `curve` (`"scarcity"`, `"garbage"` or a flat array), `amount?`, `expect?`, `expect_unit?`, `expect_trunc?` | §8.1 as pure functions: with `amount` `GetDealCost`, without `GetDealMaxAmount` |
| `neighbors_expect` | `state` (partial: `initialized`, `ready`, `deals_dirty`, `stats_dirty`, `ui_disabled`, `busy`, `connections`, `deals`, `tuning`, `rng` (`[lcg, mul]` or null), `posted`, `props`, `today`), `deal` with `deal_state` (partial: `type`, `other_side`, `city_side`, the amounts, `cost`, `flags`, `start`, `expiration`, `current`, `suspension`, `in_list`, `other_city`), `connection` with `connection_state` (partial: `type`, `cell`, `other_region_cell`, `active`, `visited`, `flags`, `needs_traffic_sync`), `city` with `city_state` (partial: `budget`, `income`, `exported`, `imported`, `produced`, `demanded`, `cost_per_unit`), `deals` (the list's IDs in order), `props` (marker positions in token order) | the simulator, a deal, a connection, a city |
| `neighbors_record` | `hex`, or `read` with `expect?` and `connections?` | the simulator's record (§13; references numbered 1, 2, … in list order), or a fresh simulator reading bytes (each reference a fresh connection) |
| `neighbors_deal_record` | `deal` with `hex`, or `read` with `expect?`, `rewrite?`, `remaining?`, `id?`, `deal_state?` | the deal record (§5.6) |
| `neighbors_connection_record` | `connection` with `hex`, or `read` with `expect?`, `rewrite?`, `connection_state?` | the connection record (§4.3) |
| `neighbors_save` / `neighbors_load` / `neighbors_expect_save` / `neighbors_patch` | `slot`; `expect?` (load: the store loaded cleanly); patch: `class` (`"simulator"`, `"connection"` or a CLSID) and the shared patch arguments | the class-object store round trip; the store ignores `Read`'s result (foundations §3.5) |
| `neighbors_expect_events` / `neighbors_expect_event_count` / `neighbors_clear_events` | see the shared conventions | host calls |

Neighbours events: `add_agent` (`level`, `name`, `flags`), `remove_agents`, `subscribe` / `unsubscribe` (`msg`),
`history` (`series`, `value`), `symmetric` (`city`, `type`, `x`, `z`, `add`), `reload_city`, `update_local_deals`,
`update_import_export` (`city`), `set_local_deals` (`city`, `deals`: the IDs after the region's pruning), `status`
(`deal`: an ID or null, `status`, `flags` at the time, `sent`), `connection_created` / `connection_deleted` (`type`,
`x`, `z`), `prop_add` (`prop`, `pos`, `orientation`, `props`), `prop_remove` (`prop`), `prop_move` (`prop`, `pos`).

## Traffic simulator ops

The traffic simulator vectors (`ts11_…`) drive `fc4_core::traffic_simulator::TrafficSimulator`
(`docs/specs/traffic-simulator.md`), with its own network model and pathfinders, over one scripted game implementing
`TrafficSimHost`. Every op name starts with `ts_`. The default world: 8 × 8 cells (tract size 1), the shipped tuning
(`SimTuning::spec_exemplar`, §4.14), day of month 5, date number 1, clock 0, workforce 1.0, carpool off, no view,
every department funded at 1.0, no lots and no connections. Departments get tokens 900, 901, … in creation order
(Init creates roads, then mass transit). A lot's holder defaults to `id + 0x10000` and its building to
`id + 0x20000`; a connection's holder to `id + 0x10000`. `ts_init` replaces the simulator and keeps the world.

Floats are written as numbers or as their bit pattern (`"0x3FC00000"`). Float results are compared as bit patterns,
so write float expectations as `"0x…"` strings (or 0); doubles are 16-digit bit strings. A trip is `{count, type,
strategy, flags, status, dest: [x, z], length, mask, route, return_route}` (routes in hex; omitted fields 0 or
empty; checks compare only the fields given). A source is `["lot", id]` or `["connection", id]`. A `holder` (in
`ts_capacity`, `ts_prop`, `ts_expect_prop` and the capacity calls) is a number or `{"pool": id}`, the simulator's
neighbour pool (0: the default pool).

### Setup

| op | Arguments | Effect |
|---|---|---|
| `ts_env` | any of `cells` (`[w, h]`), `tuning` (`"shipped"`, or `"code"`: no exemplar), `day`, `date`, `clock`, `workforce`, `carpool`, `view` (`[x1, z1, x2, z2]` or null), `roads_funding`, `transit_funding`, `dev_pop` (`[[developer type, x, z, population]]`), `dept_base`, `budget_items`, `cities` (`[[x, z, E0, E1, E2]]`: the established connected cities, traffic-network.md §2.7.4), `regional_demand` (`[J0, J1, J2]`, null: no node), `drives_on_right` (true), `query_items` (`[[building, department ID, cost]]`: the `0xCA550301` budget items of §4.16) | the world |
| `ts_lot` | `id`, `rect`, `holder?`, `building?` (null: none), `zone?` (0), `livable?`, `road_access?` (true), `link?`, `pops?` (`[[developer type, count]]`), `strategy_byte?`, `capacity?` (the u16[12] property `0x8A0B4100`: max 0–3, arrived this cycle 4–7, arrived last cycle 8–11), `trips?` | adds or replaces a lot |
| `ts_connection` | `id`, `cell`, `holder?`, `dirty?` (true: the volume-dirty flag), `type?` (the connection type; absent: `FindConnection` matches any type), `other_side?` (`FindConnectionOnOtherSide`), `city?` (`[x, z]` of the established city on the other side), `targets?` (`[[row, [9 counts]]]`, property `0xAA9EA81D + row`), `trips?` | a neighbour connection |
| `ts_trips` | `source`, `trips` | the host's trip list |
| `ts_capacity` | `holder`, `values` | the trip capacity property (u16) |
| `ts_prop` | `holder`, `id`, `value` (a variant) | any holder property |
| `ts_init` | `session_seed?` (0), `expect?` | a new simulator and its Init |
| `ts_street` | `cell`, `cells` or `rect` | §11 test 10's road info (network 1, `invCapacity 0.0625`, Street, 3 connections) |
| `ts_info` | `cell`, `cells` or `rect`; `info` (`[[network, type, inv_cap, conn_count, flags?]]`) | network-info records |
| `ts_state` | `state`: any of `cycle_state` (set without the entry action), `pass_total`, `to_reroute`, `to_search`, `units_per_slice`, `pct` (`[[wealth, [3 percentages]]]`), `budget_total`, `strike_state`, `strike_months`, `transit_income`, `toll_income`, `rail_damage_accident_factor`, `commute_max_time`, `strategy_bucket`, `buckets` (`[[bucket, [[x, z]]]]`), `dest` (`[[x, z, bits]]`), `free` (the free-destination counts), `accident_check` (`[[x, z, p]]`), `rng_seed` (the generator reseeded, wall and process clocks = the value), `pass_counters` (the 10 counts), `processed`, `day_index`, `income_acc` and `fee_acc` (doubles), `ferry_fare`, `income_per_tile` (9 floats), `cells_per_type` (13 counts), `potholes` (the published pothole count), `phase_positions` (`[x, z]`: the saved source cell, §3.5 step 11), `subnetworks_stale`, `network_changed` | the state |
| `ts_damage` | `kind` (`road`, `rail`), `intact?`, `damaged?` (`[[x, z]]`), `rate?`, `cell_count?`, `damaged_count?` (set as given, after the cells) | damage cells |
| `ts_occupant` | `id`, `flags` (bit `1 << t` per network type, bit 31 the subway skip), `cell` (the position: the middle of the cell), `own?` (the network occupant's own cell, slot 59; default `cell`) | a network occupant; `ts_call do_message` with data3 `0xC772BF98` and object `id` sends it (§3.6) |
| `ts_edge` | `usage` (`[[x, z, t, evening, value]]`) | edge-density usages (§4.8) |
| `ts_net` | any of `conn`, `switch` (`[[x, z, t, word]]`), `wormhole` (`[[x, z, byte]]`), `switch_ids` (`[[x, z, id]]`), `switches` (`[[id, toll]]` or `[[id, {toll?, building?, capacity?, prev_usage?, ratio?}]]`: inserted, the toll's f32 bits in `simulator_value`, `prev_usage` and `ratio` the two counters of §4.9 step 4), `edges` (`[[x, z, mask]]`: neighbour edge records, traffic-network.md §2.7.1), `terminals` (`[{dep, arr, lot?, riders_last?, links?: [[x, z]] (keyed by that departure cell), neighbour_links?: [[off_x, off_z, edge_x, edge_z]]}]`, appended with both keys registered) | the network model's cell records, switches and ferry terminals for ConfirmRoute and ApplyRoute (§4.3, §4.5) |
| `ts_grid` | `grid`, `n?`, `values` (`[[x, z, v]]`) | writes a grid |

Grids: `air`, `congestion`, `trip_length`, `commercial`, `commercial_work` (bytes); `peak_volume`, `peak_ratio`,
`volume` (with `n`, default 1) (floats).

### Calls and checks

| op | Arguments | Effect / check |
|---|---|---|
| `ts_call` | `method`, `args?`, `expect?` (objects compared on the keys given) | `init`, `shutdown`, `read_tunable_values`, `work_on_sim_cycle`, `do_daily_simulation`, `do_monthly_simulation`, `do_post_city_init`, `do_message [type, data1, data2, data3, object]`, `enter_cycle_state [s]` (with the entry action), `cycle_state`, `cycle_states [stale?]` (the states visited until 0 comes back, at most `max?` 64 calls; `stale` is set before each call; fails when a phase does not finish in one call), `apply_route [route, count, source rect, [x, z], mode, type]`, `confirm_route [route]` (with the strike state), `update_traffic_volume [source]`, `confirm_source [source]`, `complete_trip_source_calculations [source]`, `capacity_still_available [trip]`, `apply_traffic_to_destination [count, x, z, last type, trip type]`, `setup_trip [trip, source]` (the first pathfinder: `{density, filter, destination_flags, flags, allowed, starting, start_rect, end_rect, start_costs, max_work_time}`), `setup_path_finder_for_lot [lot, {density?, max_work_time?}?]` (`{result, pf}`, the pathfinder preset with the given settings first), `return_leg_rects [[x, z], result type, pf start rect]` (`{start (null: not written), end}`), `transit_switch_query_data [x, z]` (null, or `{available, max_capacity, load_fraction, usage, cost}`), `background_traffic [x, z]`, `travel_time_ratio [x, z, t]`, `check_rail_accident [x, z]`, `is_road_damaged [x, z]`, `apply_pothole_changes [kind, delta]` (kind 1 road, 2 rail), `travel_strategy_percentages [wealth]`, `trip_scale`, `trip_scale_for_display`, `freight_scaling_factor`, `last_monthly_income`, `query_interface [iid]`, `simulator_type`, `desired_lot_insertion_point`, `max_trip_capacity [holder, k]`, `set_max_trip_capacity [holder, k, v]`, `traffic_arrived [holder, k]`, `traffic_arrived_this_cycle [holder, k]`, `destination_byte [x, z]`, `free_destinations`, `neighbour_record [x, z]` (`{mask, pool, cost}`, traffic-network.md §2.7), `pool_ids`, `set_connection [x, z, e, exit, t]` (traffic-network.md §4.3), `rng_uniform [n, count]`, `rng_next [count]`, `rng_double [count]` (as float32 bits) |
| `ts_eval` | `fn`, `args`, `expect` | pure functions: `congestion_byte [peak volume, peak ratio]` and `congestion_curve [x]` (the shipped curve after the inversion), `shipped_congestion` (`{points, scale}`), `invert_curve [[[x, y]]]`, `air_polluting_byte [v]`, `trip_length_byte [average, commuteMaxTime]`, `strategy_percentages [[3 pct]]`, `read_tuning [base, props, fields]` (`ReadSimulatorTunableValues` and the strategy re-read over `"shipped"` or `"code"` with a property map `{"0xID": value}`; fields `max_speed`, `tile_costs`, `income_per_tile`, `generates_traffic`, `start_neutral`, `start_car`, `background`, `roads_funding`, `transit_funding`, `pct`), `residential_split [commuters, [3 fractions], strategy byte]`, `residential_split_pct [commuters, [3 pct], strategy byte]`, `residential_commuters [population, scale]`, `industrial_trips [workers, scale]`, `background_value [R, C, I]`, `damage_target [funding, rate, cells]` (`[rate, potholes]`), `reconcile_lot [trips, [3 targets], type]`, `reconcile_connection [trips, [[9 counts] × 3]]`, `decode_route [hex]` (`[[x, z, t, jump]]`), `find_highway_return_tile [x, z, back, drives on right, [X, Z]]` (`[x, z]`), `read_route [hex]` (the reader's records `[[x, z, t, t', entry, exit, next is a 0x80 jump]]`, sides null when undefined, §4.3), `legacy_density [v]`, `write_trip [trip]` (hex), `read_trip [hex, prior trip?]` (the trip, or null on failure) |
| `ts_save` | `slot`, `prefix?` (hex), `length?` | `Write` into a slot, with its reference registry |
| `ts_load` | `slot`, `version?` (the leading u16 patched), `truncate?` (bytes dropped from the end), `expect?` | `Read` |
| `ts_expect_same_save` | `a`, `b` | the two records are byte-identical |
| `ts_expect_state` | `state` (partial: `initialised`, `cycle_state`, `total`, `days`, `day_index`, `processed`, `pass_total`, `to_reroute`, `to_search`, `source_pos`, `source_count` (the connections of the iterator's snapshot), `phase_positions` (`[x, z]`), `held_source` (a source or null), `finish_resume` (a lot or null), `pass_counters`, `strategy_bucket`, `strategy_bucket_counter`, `buckets`, `budget_total`, `strike_state`, `strike_months`, `strike_flag`, `strike_probability`, `attempted`, `routed`, `job_capacity`, `car_len_sum`, `car_len_count`, `freight_len_sum`, `freight_count`, `freight_byte`, `avg_car_commute`, `transit_income`, `toll_income`, `last_monthly_income`, `income_acc`, `fee_acc`, `free`, `type_count_building`, `type_count_published`, `ferry_total`, `cells_per_type`, `stats_records` (the published u16 × 13), `stats_rows` (14 rows `{sum, congested, max, max_cell}`, slot 13 the bridge), `stats_work_rows` (the working set's 14 rows, same form), `stat_max_cmp` (the 14 comparison values of §4.9 step 3, f32 bits), `potholes`, `next_accident_check`, `accident_check`, `roads`, `mass_transit`, `commute_max_time`, `congestion_scale`, `congestion_points`, `units_per_slice`, `last_day`, `cycle_start_date`, `subnetworks_stale`, `network_changed`, `transit_multiplier`) | the simulator state |
| `ts_expect_grid` | `grid`, `n?`, `values` (`[[x, z, v]]`) | grid values |
| `ts_expect_edge` | `cell`, `t?` (1), `evening?` (false: key bit 24), `counts?` and `last?` (`[[entry, exit, value]]`, every other counter 0), `usage?` | `GetTrafficEdgeDensity` |
| `ts_expect_world` | `state?` (partial: `posted` (`[[type, data1, data2, data3]]`), `posted_types`, `history` (`[[series, value]]`), `incomes` (`[[department, item, amount]]`), `accidents` (`[[x, z, ms]]`), `agents` (levels), `subscriptions`, `departments` (`[[id, max funding]]`), `released_departments`, `refs` (non-zero reference counts), `network_costs` (`[[mass transit, [13 cell counts]]]`, one per budget update)); `posted_includes?` (`[[type]]` or `[[type, data1, data2, data3]]`); `history_includes?` (`[[series, value]]`) | the host's log |
| `ts_expect_prop` | `holder`, `id`, `expect` (null: absent; else the elements), `elem?` (`"u16"` or `"u32"`) | a holder property |
| `ts_expect_trips` | `source`, `trips` (the whole list; each trip partial) | the host's trip list |
| `ts_expect_damage` | `kind`, `state` (partial: `cell_count`, `damaged_count`, `rate`, `intact` and `damaged` (set sizes), `damaged_cells`) | the damage block |
| `ts_expect_network` | `cell`, `n?` (1), `record` (partial `{extra_cost, inv_cap, type, conn_count}`, or null) | a network-info record |
| `ts_expect_net` | `switches` (`[[id, null or partial {load, toll}]]`, read without inserting), `terminals` (`[[index, partial {riders_current, riders_last, links, neighbour_links}]]`, the links as their 4 counters) | switch usage and ferry counters (§4.5) |

## History warehouse ops

The history vectors (`hw12_…`) drive `fc4_core::history::HistoryWarehouse` (`docs/specs/history.md`) over one
scripted world implementing `HistoryHost`. Every op name starts with `hw_`. The world: the simulator date (a Julian
day number, default 2000-01-01), a millisecond timer that returns `now` and then moves by `now_step` on every
reading (both 0 by default), and a message server: `RecordHistoryData`'s send is logged and delivered to the
warehouse at once when the warehouse has subscribed the message (as `MessageSend` does). Dates are written as numbers
or as `"YYYY-MM-DD"`; keys and values as numbers or `"0x…"` strings. A point is `[date, value]`.

### Setup

| op | Arguments | Effect |
|---|---|---|
| `hw_env` | any of `date`, `now`, `now_step`, `deliver` (false: sends are logged, not delivered) | the world |
| `hw_create` | | a new warehouse (the subscriptions forgotten) |
| `hw_series` | `key` or `keys` (`[first, count]`: that many consecutive keys), `points?` (`[]`), `max_count?` (0) | sets series directly (test support) |
| `hw_pass` | `state`, `cursor?` (a key; absent: none), `counter?` (0) | the thinning pass state (test support) |
| `hw_days` | `from`, `to`, `record?` (`[key, value]`, or `[key, "index"]`: the day's index from `from`) | runs the days as the simulator would: a year change delivers SimNewYear, then SimNewDay until it returns true, then the record message |

### Calls and checks

| op | Arguments | Effect / check |
|---|---|---|
| `hw_call` | `method`, `args?`, `expect?` (false results are `null`), `precision?` (24 or 53; default 24), `expect_bits?` (the float results' bit patterns) | `init`, `shutdown`, `message [type, data1?, data2?]`, `new_day`, `new_year`, `record [key, value]` (`RecordHistoryData`: the send only), `query_interface [iid]`, `store [key, value]`, `set_size [key, n]` / `set_queue_length`, `coalesce [key, from, to, granularity]`, `dispose [key]`, `dispose_range [key, from, to]`, `thinning_step`, `variable_coalesce [key]`, `get_count [key, from, to]`, `get_trend [key, from, to]` (`[slope, intercept]`), `get_point [key, date]`, `get_min_max_average [key, from, to]` (`[min, max, avg]`) |
| `hw_expect` | `state?` (partial: `initialized`, `pass_state`, `cursor`, `pass_counter`, `series_count`, `total_points`, `keys`); `series?` (a key) with `missing?`, `points?`, `dates?`, `values?`, `count?`, `max_count?` | the warehouse |
| `hw_record` | `hex` (compares `Write`), or `read` (hex, read into the current warehouse) with `expect?` and `remaining?` | the record (§7) |
| `hw_save` | `slot` | `Write` into a slot |
| `hw_load` | `slot`, `version?` (the leading u16 patched), `truncate?` (bytes dropped from the end), `expect?` | a new warehouse reads the slot |
| `hw_expect_save` | `slot` with `hex`, `same_as` (another slot) or `length` | a saved record |
| `hw_expect_events` / `hw_expect_event_count` / `hw_clear_events` | as the other cores | events: `subscribe` / `unsubscribe` `{msg}`, `add_agent {level, name, flags}`, `remove_agent`, `send {msg, data1, data2, delivered}` |

## 24-hour clock ops

The clock vectors (`ck12_…`) drive `fc4_core::clock24::Clock24` (`docs/specs/clock24.md`) over one scripted world
implementing `ClockHost`. Every op name starts with `ck_`. The world: a microsecond clock (`now`, default 1,000,000)
that only moves when a vector moves it, a cheat code manager (present by default) and the "Main Simulator" exemplar
(default: the shipped values, magnification 135 and night hours 20 / 1). Times of day are written as seconds or as
`"HH:MM:SS"`.

### Setup

| op | Arguments | Effect |
|---|---|---|
| `ck_env` | any of `now`, `cheat_manager`, `tuning` (`"shipped"`, null: no exemplar, or `{magnification?, night_start?, night_end?}`: absent keys are missing properties) | the world |
| `ck_create` | | a new clock (the constructor's values) |
| `ck_set` | any of `seconds`, `visual_seconds`, `night` (`[start, end]`) | sets the state without messages (test support) |
| `ck_idle` | `advance?` (µs added to the clock first), `elapsed?` (moves the clock until the running timer shows that many µs), `count?` (1: repeats, `advance` before each), `expect?` | `OnIdle` |

### Calls and checks

| op | Arguments | Effect / check |
|---|---|---|
| `ck_call` | `method`, `args?`, `expect?`, `expect_bits?` (a float result's bit pattern) | `init`, `shutdown`, `on_idle`, `query_interface [iid]`, `set_visual_time_mode [mode]`, `visual_time_mode`, `time_percentage`, `visual_time_percentage`, `set_time_percentage [p]`, `hour`, `set_hour [h]`, `visual_hour`, `minute`, `set_minute [m]` (the new seconds), `visual_minute`, `second`, `set_seconds [s]` (`GetMinute`), `visual_second`, `event_hour [i]`, `time_magnification`, `set_time_magnification [m]`, `time_magnification_min_max` (`[min, max]`), `set_time_magnification_min_max [min, max]`, `pause [on]`, `is_paused`, `hidden_pause [on]`, `is_hidden_paused`, `is_night`, `is_night_hour [h]`, `night_hours` (the night hours among 0–23), `enable_notifications [on]`, `change_visual_time [t]`, `cheat [cheat, text]` (the cheat message 0x230E27AC; the cheat is `"WhatTimeIzIt"`, `"Stopwatch"` or an ID), `message [type, data1, text or null]` |
| `ck_expect` | `state` (partial: `initialized`, `pause_count`, `hidden_pause_count`, `magnification`, `magnification_bits`, `min_magnification`, `min_magnification_bits`, `max_magnification`, `seconds`, `visual_seconds`, `night_start`, `night_end`, `notifications`, `visual_mode`, `timer_running`, `timer_elapsed` (µs)) | the clock |
| `ck_record` | `hex` (compares `Write`), or `read` (hex, read into the current clock) with `expect?` and `remaining?` | the 26-byte record (§7) |
| `ck_expect_events` / `ck_expect_event_count` / `ck_clear_events` | as the other cores | events: `post {msg, name, data2, data3}` (names `HourChanged`, `TimeTick`, `VisualHourChanged`, `VisualTimeTick`, `DayNightChanged`, `PauseChanged`, `HiddenPauseChanged`), `add_system_service`, `remove_system_service`, `add_to_on_idle`, `remove_from_on_idle`, `register_cheat {id, name}`, `add_cheat_notification {id}`, `remove_cheat_notification {id}`, `unregister_cheat {id}` |

## Commercial and industrial ops

The commercial and industrial vectors (`ci13_…`) drive `fc4_core::commercial` (`docs/specs/commercial-industrial.md`):
the commercial simulator (`CommercialSim`) over one scripted city implementing `CommercialHost`, and the industrial
stub (`IndustrialSim`, which has no host). Commercial op names start with `commercial_`, industrial ones with
`industrial_`. The default world: an 8 × 8-cell city with a demand simulator answering both nodes `0x3000` and
`0x3800` (a node's token is its ID), no building map, building-map keys `(purpose << 2) + wealth + 1`, and no
occupants. The two proximity grids are core `SimGrid`s the world makes; a record read back shares the writer's
grids.

### Setup

| op | Arguments | Effect |
|---|---|---|
| `commercial_env` | any of `cells` (the square city's side), `nodes` (the node IDs the demand simulator answers), `demand` (whether the city has a demand simulator), `map` (`{key, cells: [[x, z]]}` or `{fill, cells: [{x, z, v}]}`; null = no building map), `keys` (`[{purpose, wealth, key}]`), `purposes` (`{"occupant": purpose}`: the building occupants) | the world |
| `commercial_create` / `industrial_create` | | a new constructed object, which replaces the current one |
| `commercial_set` | `dirty?`, `state?` | pokes the state (test setup) |
| `commercial_set_grid` | `grid` (`CO2` = CO$$, `CO3` = CO$$$), `fill?`, `cells?` (`[{x, z, v}]`) | writes a proximity grid |

### Calls and checks

| op | Arguments | Effect / check |
|---|---|---|
| `commercial_init` / `commercial_shutdown`, `industrial_init` / `industrial_shutdown` | `expect?` | `Init` / `Shutdown` |
| `*_add_ref` / `*_release` | `expect?` | `AddRef` / `Release` (the count returned) |
| `*_identity` | `iids` (`{"0xIID": bool}`), `class` | `QueryInterface` and `GetGZCLSID` |
| `commercial_message` / `industrial_message` | `type` (`SimNewMonth`, `OccupantInserted`, `OccupantRemoved`, … or an ID), `data3?` (default the building type `0x278128A0` for the occupant messages), `occupant?` (void1), `expect?` | `DoMessage` |
| `commercial_idle` | `count?` (1) | `DoIdle` steps |
| `commercial_expect` | `state` (partial: `initialized`, `dirty`, `state`, `flag` (the saved byte), `ref_count`, `destroyed`, `city_held`, `bds_held`, `message_server_held`, `nodes` (tokens or null), `co2` / `co3` (the grids' instances, null: none), `tract_size`) | the simulator state |
| `industrial_expect` | `state` (partial: `initialized`, `ref_count`, `destroyed`) | the stub's state |
| `commercial_expect_grid` | `grid`; `rows` (`rows[z][x]`, every cell), or `cells` (`[{x, z, v}]`) with `rest?` (every other cell), or `count` (`{"value": cells}`) | a proximity grid |
| `commercial_proximity_map` | `developer`, `expect` (`"CO2"`, `"CO3"` or null) | `GetProximityMap` |
| `*_write` | `slot?`, `expect?`, `length?`, `hex?` | `Write`; the bytes and the registry entries are kept in `slot` |
| `*_read` | `slot` or `hex`, `version?` (rewrites the first two bytes), `reuse?`, `expect?` | `Read` into a new constructed object, or with `reuse` into the current one |
| `commercial_expect_record` | `slot`, any of `same_as`, `length`, `at` (`[{offset, hex}]`) | record bytes |
| `commercial_expect_events` / `commercial_expect_event_count` / `commercial_clear_events` | see the shared conventions | host calls |

Commercial events: `agent` (`level`, `name`, `flags`), `remove_agent`, `acquire_message_server`,
`release_message_server`, `subscribe` / `unsubscribe` (`type`), `acquire_city` / `release_city`, `acquire_bds` /
`release_bds`, `demand_node` (`id`), `release_demand_node` (`node`), `shutdown_grid` (`instance`).

## Civic building ops

The civic building vectors (`cb12_…`) drive `fc4_core::civic::CivicSim` (`docs/specs/civic-buildings.md`) over one
scripted city implementing `CivicHost`. Every op name starts with `civic_`. The default world: no departments (the
simulator's own are created at `Init`; a department's token is its ID), no funding (unreadable), no occupants or
lots, an automata controller manager, and the stub generator (session seed `0x12345678`). A line item's token is
`(department << 32) | line`. The stub generator logs `seed` (`seed`) when the simulator makes a temporary generator,
then `double` and `u32` (`n`) for each draw; doubles come from the `doubles` queue (then 1.0), indices from `u32s`
(then n − 1). With `random: "default"` the core's own generator (`RzRandomSuper`, foundations §6.3) is used.

References are counted per token (AddRef − Release, plus the one a resolved save reference hands over); a reference
that does not resolve (not a known department or occupant) reads as null and its count is given back.

### Setup

| op | Arguments | Effect |
|---|---|---|
| `civic_env` | any of `departments` (IDs that exist already), `funding` (`{"0xID": fraction or "nan"}`), `automata`, `random` (`"stub"` or `"default"`), `doubles`, `u32s` (appended to the queues) | the world |
| `civic_building` | `occupant`, `occupant_type?` (`0x278128A0`), `building?` (true: the building interface answers), `type?` (the building type), `key?` (the building key's instance), `conditional?` (property `0xEA3209F8`), `exclusion?` (property `0xEA2E078B`), `groups?`, `lot?` (default occupant + 1; null: none), `condition?` (0) | an occupant and its lot |
| `civic_item` | `occupant`, `purpose` (`"maintenance"`, `"income"` or an ID), `department`, `line?` (0), `cost`, `index?` (0); or `remove: true` | a budget item |
| `civic_lot` | `lot`, `building` (null: none), `condition?` | a lot |
| `civic_set_line` | `department`, `line`, `income?`, `type?` | an existing line item |
| `civic_create` | `session_seed?` | a new constructed simulator, which replaces the current one |
| `civic_set` | `disease_centers?`, `maintained?` (token lists, null entries allowed, no reference taken), `exclusions?` (IDs inserted in order) | pokes the state (test setup) |

### Calls and checks

| op | Arguments | Effect / check |
|---|---|---|
| `civic_init` / `civic_shutdown` / `civic_add_ref` / `civic_release` | `expect?` | the lifecycle slots |
| `civic_identity` | `iids` (`{"0xIID": bool}`), `class` | `QueryInterface` and `GetGZCLSID` |
| `civic_message` | `type` (`SimNewMonth`, `OccupantInserted`, `OccupantRemoved`, `LotStateChanged` or an ID), `occupant?` (void1), `lot?` (the IGZUnknown), `data2?`, `expect?` | `DoMessage` |
| `civic_call` | `method`, `args?`, `expect?` | `is_building_excluded [id]`, `get_status [key]` (`{status, count, tooltip}` or null), `update_status [key, status, count, tooltip]`, `or_status [key, bits]` (a write through the pointer `get_status` returns; false when there is no entry), `landmark_count`, `monthly`, `lot_activated [lot]`, `building_removed [occupant]` |
| `civic_expect` | `state` (partial: `initialized`, `destroyed`, `ref_count`, `landmarks`, `maintained`, `disease_centers`, `exclusions` and `status_keys` (in hash order), `temporaries` (generators made), `landmark_count`) | the simulator state |
| `civic_expect_line` | `department`, `line`, `state` (partial: `expenses`, `income`, `type`) or `absent: true` | a line item |
| `civic_expect_item` | `occupant`, `purpose`, `state` (partial: `line`, `department`, `cost`, `index`) or `absent: true` | a budget item |
| `civic_expect_conditions` | `conditions` (`{"lot": condition}`) | lot conditions |
| `civic_expect_refs` | `refs` (`{"token": count}`) | references held |
| `civic_write` | `slot?`, `expect?`, `length?`, `hex?` | `Write`; the bytes and the registry entries are kept in `slot` |
| `civic_read` | `slot` or `hex` (references resolve to their object IDs), `version?`, `length?` (truncates), `reuse?`, `expect?` | `Read` into a new constructed simulator, or with `reuse` into the current one |
| `civic_expect_record` | `slot`, any of `same_as`, `length`, `at` | record bytes |
| `civic_expect_events` / `civic_expect_event_count` / `civic_clear_events` | see the shared conventions | host calls |

Civic events: `agent` (`level`, `name`, `flags`), `remove_agent`, `subscribe` / `unsubscribe` (`type`),
`register_lua` / `unregister_lua`, `add_ref` / `release` (`token`), `create_department` (`id`, `group`),
`department_name` (`department`, `group`, `instance`), `max_funding` (`department`, `max`), `change_line`
(`occupant`, `index`, `line`), `remove_item` (`occupant`, `index`), `create_line` (`department`, `line`, `local`),
`remove_line` (`department`, `line`), `set_line_type` (`line`, `type`), `set_condition` (`lot`, `condition`),
`controllers` (`name`, `occupant`; only with an automata controller manager), and the stub's `seed`, `double`, `u32`.

## Park ops

The park vectors (`pk12_…`) drive `fc4_core::parks::ParkManager` (`docs/specs/parks.md`) over one scripted city
implementing `ParkHost`. Every op name starts with `park_`. The default world: every service present (city, budget,
lot manager, message server), no departments (the manager creates Parks and Recreation at `Init`; a department's
token is its ID), creation succeeds, no funding (unreadable), no tuning exemplar, no occupants, an automata
controller manager, and the stub generator (the same queues and events as the civic one, without `seed`). The
manager's own token is `0xA000`. A line item's token is `(department << 32) | line`. References and save references
are counted as for the civic ops.

### Setup

| op | Arguments | Effect |
|---|---|---|
| `park_env` | any of `services` (`{city, budget, lot_manager, message_server: bool}`), `departments`, `create_departments`, `create_lines` (whether creation succeeds), `funding` (`{"0xID": fraction, "nan" or null}`; null: unreadable), `tuning` (`"shipped"`, `"no_curve"` (the shipped threshold only), null (no exemplar) or `{props: {"0xID": value}}`), `automata`, `random` (`"stub"` or `"default"`), `doubles`, `u32s` | the world |
| `park_building` | `occupant`, `occupant_type?` (`0x278128A0`), `building?` (true: `QueryInterface(0x87DDA3A9)` answers), `type?`, `groups?` (`"park"`, `"zoo"` or IDs), `preview?` (property `0x029244DB` present), `lot?` (the lot at its position; default occupant + 1, null: none), `condition?` | an occupant and its lot |
| `park_item` | `occupant`, `department`, `line?` (0), `cost`, `index?` (0), `purpose?` (`0xCA639989`); or `remove: true` | a budget item |
| `park_line` | `department`, `line`, `expenses?`, `secondary?` | creates or updates a line item |
| `park_create` | `session_seed?` | a new constructed manager, which replaces the current one |
| `park_set` | `parks?`, `zoos?` (token lists, null entries allowed, no reference taken), `underfunded?` (the latch) | pokes the state (test setup) |

### Calls and checks

| op | Arguments | Effect / check |
|---|---|---|
| `park_init` / `park_shutdown` / `park_add_ref` / `park_release` | `expect?` | the lifecycle slots |
| `park_identity` | `iids`, `class` | `QueryInterface` and `GetGZCLSID` |
| `park_message` | `type` (`SimNewMonth`, `OccupantInserted`, `OccupantRemoved` or an ID), `occupant?` (void1), `expect?` | `DoMessage` |
| `park_call` | `method`, `args?`, `expect?`, `expect_bits?` (the double's bit pattern, `"0x…"`) | `monthly`, `zoo_escape`, `escape_probability [f]` (`p` of §4.4 step 5 with the manager's curve) |
| `park_expect` | `state` (partial: `initialized`, `destroyed`, `ref_count`, `tuning_read`, `underfunded`, `parks`, `zoos`, `city_held`, `budget_held`, `message_server_held`, `lot_manager_held`, `department`, `threshold`, `curve_points`, `curve` (`[[x, y]]`)) | the manager state |
| `park_expect_line` | `department`, `line`, `state` (partial: `expenses`, `secondary`, `display` (the display flags set)) or `absent: true` | a line item |
| `park_expect_item` | `occupant`, `purpose?`, `state` (partial: `line`, `department`, `cost`, `index`) | a budget item |
| `park_expect_conditions` / `park_expect_refs` | as for the civic ops | lot conditions / references held |
| `park_write` / `park_read` / `park_expect_record` | as for the civic ops | the record |
| `park_expect_events` / `park_expect_event_count` / `park_clear_events` | see the shared conventions | host calls |

Park events: `tuning` (the exemplar was asked for), `acquire` (`service`, `ok`), `release_service` (`service`),
`agent` (`level`, `name`, `flags`), `remove_agent`, `subscribe` / `unsubscribe` (`type`), `add_ref` / `release`
(`token`), `create_department` (`id`, `group`), `department_name` (`department`, `group`, `instance`),
`max_funding` (`department`, `max`), `change_line` (`occupant`, `index`, `line`), `create_line` (`department`,
`line`, `local`), `display_flag` (`line`, `flag`, `on`), `set_condition` (`lot`, `condition`), `post` (`type`,
`void1`, `unknown`: the posted ZooEscape), `controllers` (`name`, `occupant`; only with an automata controller
manager), and the stub's `double`, `u32` (`n`).

## Network manager ops

The network manager vectors (`nm16_…`, `nm00_…`) drive `fc4_core::network_manager` (`docs/specs/network-manager.md`):
the network manager (`NetworkManager`, component 53), the network lot manager (`NetworkLotManager`, component 11) and
the riot's removal lock (`RiotLock`), over one scripted 64 × 64 city implementing `NetworkManagerHost`,
`NetworkLotHost` and `RiotHost`. Every op name starts with `net_`. Ops that address one manager take `of`:
`"manager"` (the default) or `"lots"`. The default world: day, no UK paths, flat terrain at altitude 0, area slope
0, a traffic network map, crews that unregister themselves at their slot 5, no occupants, zones, lots or
configurations, and the stub generator (queued `u32s`, then 0; every draw is a `draw` event with `n`). The managers'
own tokens are `0xA000` and `0xB000`. Tool tokens are `0x7000 + n` in creation order (`Init` creates types 0..12,
so the cached tool of type `t` is `0x7000 + t`); street props, flora and effect props get `0x5000`, `0x5001`, … in
creation order. Cell (x, z) has its corner at `(16x, 16z)` and its centre at `(16x + 8, 16z + 8)`; an occupant's
altitude is the terrain's plus its `height`. A save reference to an occupant or an effect prop resolves with one
reference; any other token does not resolve. References held are counted per token (`net_expect_refs`); `add_ref` /
`release` are not events.

### Setup

| op | Arguments | Effect |
|---|---|---|
| `net_env` | any of `night`, `uk_paths`, `terrain` (the flat altitude), `slope` (the lot developer's area slope; `"nan"` allowed), `traffic_map`, `crews_unregister`, `random` (`"stub"` or `"default"`), `session_seed`, `u32s` | the world |
| `net_occupant` | `occupant`, `flags?` (an integer or a list of names: `road`, `rail`, `highway`, `street`, `pipe`, `power_line`, `avenue`, `subway`, `light_rail`, `monorail`, `one_way_road`, `dirt_road`, `ground_highway`, `translucent`, `pipe_distressed`, `pipe_burst`, `flag_40000`, `flag_80000`, `intersection`, `connection_node`, `usable`, `flag_80000000`, or integers), `cell?`, `in_city?` (true: occupant flag 1), `managed?` (globally managed), `lit?` (CanBeLit), `height?` (`"nan"` allowed), `creation?` (the token), `retrofit?` (CheckForRetrofit), `flag4?` (occupant flag 4), `removable?` (true), `piece?`, `rotation?`, `flip?`, `intersection?`, `edges6?` (GetEdgeTypesFlag(6)), `active_edges?`; or `remove: true` | creates or updates an occupant (fields left out keep their value); a network occupant is one with a type flag (`0x1FFF`) |
| `net_zone` | `cells` (`[[x, z, zone]]`), `clear?` | the zone grid |
| `net_lot` | `lot`, `rect` (`[x1, z1, x2, z2]`) or `cell`, `wealth?` (0), `livable?`, `building_type?`; `profiles?` (`{"0xTYPE": wealth}`); `clear?` | a lot over its cells, building profiles |
| `net_config` | `id`, `piece`, `token?` (the ID), `zones?` / `wealths?` (allowed values; absent: all), `mask?` (4 bytes), `period?`, `min_slope?`, `max_slope?`, `rotations?`, `flip_rule?`, `objects?` (`[{kind: "prop" \| "flora" \| n, flags?, orientation?, offset?: [x, y, z], field_24?, ids?: [...]}]`, `ids` being the object's candidate list); `clear?` | a network lot configuration, loaded at the lot manager's `Init` |
| `net_create` | `of?` (`"manager"`, `"lots"` or `"both"`, the default) | new constructed managers |

### Calls and checks

| op | Arguments | Effect / check |
|---|---|---|
| `net_init` / `net_shutdown` / `net_add_ref` / `net_release` | `of?`, `expect?` | the lifecycle slots |
| `net_identity` | `of?`, `iids` (`{"0xIID": bool}`), `class` | `QueryInterface` and `GetGZCLSID` |
| `net_message` | `of?`, `type` (`ViewChanged`, `TerrainChanged`, `LongTerrainBegin`, `LongTerrainEnd`, `WaterDisplayChanged`, `PreCityShutdown`, `PostCityInit`, `OccupantInserted`, `OccupantRemoved`, `Dawn`, `Dusk`, `UsableChanged`, `CreateLotObjects`, `ZoningChanged`, `LotStateChanged`, `RoadDamageChanged` or an ID), `data1`..`data4`, `occupant` (void1), `unknown` (the IGZUnknown), `view` (`[old, new]`), `rect` (the zoning rectangle), `expect?` | `DoMessage` |
| `net_call` | `method`, `args`, `expect?` | manager: `get_network_tool [type, unique]` (`{tool, type}` or null), `destroy_network_occupants [[occ…]]`, `queue_removals [on]`, `remove_or_replace_occupant [tool, occ \| null]` (`[result, replaced]`), `register_construction_crew [c]`, `unregister_construction_crew [c]`, `add_to_retrofit_list [n]`, `terrain_rect [data1, data2]` (`[x1, z1, x2, z2]`), `corner_mismatch [d]`, `set_network_flag [flags, bits, in_city]` / `clear_network_flag [flags, bits]` (`{flags, usable_post, translucency}`); lot manager: `zone_type_for_network [x, z]`, `wealth_for_network [x, z]`, `is_lot_compatible [config, x, z, zone, wealth, rotation, flip]`, `object_position [[x, z], rotation, flipped, o, [ox, oy, oz]]` (`{pos, orientation}`, the altitude being the flat terrain's), `update_roads_in_cell [x, z]`, `update_road_style [x, z, n]`, `add_pending [x, z]`, `add_border_to_pending [[x1, z1, x2, z2]]`, `process_update_list`, `set_lot_configuration_for_next_piece [config \| null]`, `redevelop_lot [x, z]`, `do_road_damage_change [x, z, damaged]`, `get_network_lot [x, z]`, `configuration_by_id [id]`, `pothole_offset [e]`, `road_texture [zone, wealth]`, `road_variation [zone, wealth]` |
| `net_riot` | `action?` (`add_instance`, `remove_instance`, `shutdown`, or `mark` with `cell`), `can_remove?` (`{"0xOCC": [level, bool]}`, checked after the action) | the riot lock over the world's removal counters |
| `net_expect` | `of?`, `state`: manager (partial: `initialized`, `destroyed`, `ref_count`, `uk_paths`, `long_terrain_op`, `dirty` (`[x1, z1, x2, z2]`, null when empty), `managed`, `pending`, `queueing`, `retrofit`, `crews`, `tools` (each cached tool's type or null), `riot_locked`, `riot_instances`); lots (partial: `initialized`, `destroyed`, `ref_count`, `lots` (`[{cell, occupant, config, config_id, locked}]` in map order), `lot_count`, `potholes` (`[{cell, prop}]`), `pending` (cells in key order), `pending_count`, `forced`, `configs` (IDs, sorted)) | the state |
| `net_expect_lot` | `cell`, `state` (partial: `occupant`, `config`, `config_id`, `locked`) or `absent: true` | a network lot |
| `net_expect_refs` | `refs` (`{"token": count}`) | references held |
| `net_write` | `of?`, `slot?`, `expect?`, `length?`, `hex?`, `registered?` (the tokens in save-registry order) | the record |
| `net_read` | `of?`, `slot` or `hex` (object IDs resolve to themselves), `length?` (cut), `reuse?` (the current manager, else a new one), `error?` (the stream starts in error), `position?` (bytes consumed), `expect?` | the record |
| `net_expect_record` | `slot`, `same_as?`, `length?` | a saved record |
| `net_expect_events` / `net_expect_event_count` / `net_clear_events` | see the shared conventions | host calls |

Network manager events: `subscribe` / `unsubscribe` (`type`), `create_tool` (`type`, `tool`), `tool_init` /
`tool_shutdown` (`type`), `destroy` (`type`, `occupants`), `underground` (`type`, `network_type`, `cells`,
`occupants`), `cell_info` (`type`, `packed`), `reinsert` (`type`, `info`), `set_lit` (`occupant`, `lit`), `retrofit`
(`occupant`), `remove_occupant` (`occupant`), `query` (`rect`, `mask`: the occupant manager's rectangle query),
`refresh` (`occupant`: the posted `0x99EF1145`), `view_rotation`, `crew_shutdown` (`crew`). Network lot manager
events: `texture` (`occupant`, `texture`), `variation` (`occupant`, `variation`), `set_flag` (`occupant`, `bits`),
`intersection` (`occupant`), `remove_props` / `demolish_flora` (`cell`), `prop` (`id`, `pos`, `orientation`,
`token`), `prop28` / `prop29` (`prop`, `value`), `flora` (`id`, `pos`, `token`), `flora28` (`flora`, `index`),
`effect` (`token`), `effect_name` (`prop`, `name`), `effect_start` / `effect_stop` (`prop`), `position` (`prop`,
`pos`), `insert` / `remove` (`occupant`), and the stub's `draw` (`n`). Riot events: `unset_removal` /
`set_removal` (`occupant`, `mask`).

## Network rules ops

The network rules vectors (`nr15_…`) drive `fc4_core::network_rules` (`docs/specs/network-rules.md`): a rule
database (`NetworkRules`) loaded from vector text, a scratch tile evaluator, a 64 × 64 cell grid of cell infos and
the solve state (the set `S`, the solved list, the potential-intersection list `+0x08`, the valid count). Every op
name starts with `nr_`. Rule text (`text`) is a string or a list of lines joined with `\n`. Cells are `[x, z]`;
sets given as cell lists are built in the given order (193 buckets) and reported in iteration order. Network types
are names (`Road`, `Rail`, …, `GroundHighway`) or numbers 0-12. The intersection rules' `Test` runs on a dry city
that accepts every demolition region.

| op | Arguments | Effect / check |
|---|---|---|
| `nr_reset` | `width?`, `height?` (64) | a new database, scratch evaluator, grid and solve state |
| `nr_edge_token` / `nr_mirror_byte` / `nr_mirror_edges` | `value` / `byte` / `edges`, `expect` | §3.3 edge token; §4.2 mirror of a byte or an edge word |
| `nr_rotate_meta` / `nr_rotate_orientation` / `nr_rotate_edges` | `m` / `orientation` / `edges`, `turns`, `expect` | the §4.1 rotation tables |
| `nr_tiles` | `text`, `type?` (append the file to that type's evaluator list), `append?` (add to the scratch evaluator instead of replacing it), `expect?` (partial: `status` (`missing`, `complete`, `rejected`, `bad_centre`, `truncated`), `transmogrify`, `source_rules`, `rules`, `dropped`) | reads a tile rule file |
| `nr_expect_rules` | `rules` (`[{key, transform, conditions: [[m, value, mask]], outputs: [[id, m, orientation]]}]`; fields left out are not compared) | the scratch evaluator's rules in map order |
| `nr_load` | `resource` (`multi` (RUL1), `overrides` (RUL2), `intersections` (RUL0), `network` (the network INI) or a tile instance), `text`, `expect?` (partial: `entries`, `ignored`, `sections`, `rules`, …) | `LoadResource` into the database |
| `nr_cell` | `x`, `z`, `edges` (`{name: word}`, each setting its type bit), `flags?` (ORed), `in_run?`, `step?`, `fixed?`, `node?`, `intersection?`, `one_way?`, `marked?`, `occupant?` (`{piece, orientation, network_flags, node_edges: [a, b], anchor: [x, z], paired_portal: [x, z]}`) | puts a cell info |
| `nr_evaluate` | `x`, `z`, `type`, `expect` (null or `[[id, m, orientation]]`) | `Evaluate` with the scratch evaluator |
| `nr_set_s` / `nr_set_valid_count` | `cells` / `count` | the solve state |
| `nr_append_solved` | `id`, `orientation`, `x`, `z` | appends a solved record (sets the cell's `+0xB8`) |
| `nr_apply` | `p`, `type`, `expect_p?`, `expect_w?` | `ApplyNetworkRules` with the scratch evaluator and an empty `W` |
| `nr_multi` | `type` (the tool's), `expect` (the output cells) | `SolveMultiTypeIntersections` |
| `nr_subtypes` | `p`, `reset?` (true), `expect` (`R`), `expect_p?` | `AssignEdgeSubtypes` |
| `nr_patch` | `a`, `b` (`[id, orientation]`), `side`, `expect`, `expect_a?`, `expect_b?` | `PatchTilePair` |
| `nr_adjust` / `nr_verify` | `resolve?` (false) / -, `expect` | `AdjustTileSubsets` / `VerifyTileSubsets` |
| `nr_solve` | `type`, `resolve?`, `expect` | the rule stages of `SolveNetwork` |
| `nr_int_rule` | `id`, `rotate?`, `transpose?` (applied to a copy), `state` (`occupied`, `new_cells`, `tagged` (`[[x, z, tag]]`), `bounds` (`[minX, maxX, minZ, maxZ]`), `one_way_dir`, `type_mask`, `ring`, `add` (`[prev, next]`), `edges` (`{tag: [edges1, mask1, edges2, mask2]}`)) | an intersection rule |
| `nr_int_test` | `id`, `rotate?`, `transpose?`, `x`, `z`, `expect` (0, 1 or 2) | `cIntRule::Test` |
| `nr_identify` | `steps` (`[[[x, z], …], …]`), `v`, `type`, `expect`, `expect_intersections?` (`[[rule, x, z, start, a, b]]`) | `IdentifyHighwayIntersections` |
| `nr_one_way` | `steps` (`[[[x, z], …], …]`) | `ResolveOneWayConflicts` (§6.5) over the dragged run |
| `nr_separation` | `x`, `z`, `type`, `expect` (null or `[id, rot, flip]`) | `SolveMultiTypeSeparation` (§8.3) over the database's evaluators |
| `nr_tile_info` | `id`, `rotate?`, `transpose?`, `offset` (`[dx, dz]`), `expect` (null or `[tile, orientation, new_cell]`) | `GetTileInfo` (§10.9) |
| `nr_pieces` | `id`, `rotate?`, `transpose?`, `x`, `z`, `expect` (`[[ox, oz, piece_id, orientation, path, ax, az, new_cell]]`) | the pieces an intersection places (§10.9) |
| `nr_mark_pieces` | `on`, `valid_count` | `MarkHighwayIntersectionPieces` (§10.6) over the recorded intersections |
| `nr_remove_pieces` | `valid_count`, `bridge?`, `bridge_cells?` (`[[end_step, [key, …]], …]`) | the §10.6 remove, or the bridge one of network-bridges.md §8.3.1 when `bridge` |
| `nr_expect_solved` / `nr_expect_s` / `nr_expect_potential` | `records` (`[[id, orientation, x, z]]`) / `cells` / `entries` (`[[x, z, t]]`) | the solve state |
| `nr_expect_cell` | `x`, `z`, `state` (`flags`, `union`, `solved`, `intersection`, `one_way`, `marked`, `in_run`, `edges` (every non-zero word)) | a cell info |

## City detail ops

The city detail vectors (`cd8_…`) drive `fc4_core::city_detail::CityDetailManager` (`docs/specs/city-detail.md`)
over one scripted city implementing `CityDetailHost`. Every op name starts with `cd_`. The default world: preference
3, no lots, no network lots, creation succeeds, the stub generator.

| op | Arguments | Effect / check |
|---|---|---|
| `cd_env` | any of `setting` (the preference), `session_seed` (a new session), `random` (`"stub"` or `"session"`), `u32s` (the stub's queue), `network` (`[[x, z, lot]]`), `create` (whether AddLotProp / AddNewFloraOccupant answer an occupant) | the world |
| `cd_lot` | `lot`, `objects` (`[{type: "prop" \| "flora" \| n, flags?, tag, ids?}]`, `ids` defaulting to `[tag]`; null: no configuration) | appends a lot |
| `cd_occupant` | `occupant`, `lot`, `type` (`"prop"`, `"flora"`, `"building"` or an ID), `tag?` | an occupant |
| `cd_ids` | `lot`, `index`, `ids` | the configuration manager's ID array of an object (families expanded) |
| `cd_create` | | a new manager |
| `cd_init` / `cd_shutdown` / `cd_add_ref` / `cd_release` / `cd_get_level` | `expect?` | the slots |
| `cd_set` | `level` | the stored level (test setup) |
| `cd_identity` | `iids` (`{"0xIID": bool}`), `class?` | `QueryInterface`, `GetGZCLSID` |
| `cd_expect` | `state` (partial: `level`, `ref_count`, `destroyed`, `last_pass`, `removed`, `created`, `draws`, `network_lots`, `session_passes`: the increase passes by this manager object, n of §3.5, CD5) | the manager |
| `cd_expect_occupants` | `lot`, `occupants` (`[[occupant, kind or type, tag or null]]`, token order) | a lot's occupants |
| `cd_write` / `cd_read` | `hex?`, `slot?`, `fresh?`, `expect?` | the record |
| `cd_expect_events` / `cd_expect_event_count` / `cd_clear_events` | see the shared conventions | host calls |

City detail events: `demolish_flora` / `remove_prop` (`occupant`), `placement` (`lot`, `index`), `add_prop` /
`add_flora` (`id`, the placement, `occupant`), `set_tag` (`occupant`, `tag`), `redevelop` (`x`, `z`, `lot`), `draw`.

## Port developer ops

The port developer vectors (`pd12_…`) drive `fc4_core::port_developer::PortDeveloper`
(`docs/specs/port-developers.md`) over one scripted city implementing `PortHost`: the shipped airport and seaport
exemplars, lots and building occupants, the budget line, office counts per subnetwork, freight arrivals and the
terrain's water. Every op name starts with `pd_`. A lot configuration's token is its ID.

| op | Arguments | Effect / check |
|---|---|---|
| `pd_env` | any of `kind`, `line` (`[full, current]` or null), `follow` (current expenses follow SetFullExpenses), `department` (a token or null), `offices` (`[[CO$$ per subnetwork], [CO$$$ per subnetwork]]`), `terrain` (`[width, depth]`), `water` (`[[x, z]]`, replacing the set), `create_lot`, `construct` (whether CreateLot and ConstructDefaultLot succeed), `missing_configs` (configuration IDs the manager does not have; ConstructDefaultLot with the null configuration then leaves the lot in state 2 with condition 1 and answers true, PD5), `exemplars` (`{"0xINSTANCE": {properties} \| null}`) | the world |
| `pd_lot` | `lot`, any of `config` (`[id, growth stage]` or null), `x`, `z`, `state`, `condition`, `days`, `rect` (`[x0, z0, x1, z1]`), `facing`, `compatible`, `subnetworks`, `arrived` | creates or updates a lot |
| `pd_occupant` | `occupant`, `lot`, `type?` (`"building"` or an ID), `group?`, `preview?` | a building occupant |
| `pd_create` | `kind?` (`"airport"` or `"seaport"`) | a new developer |
| `pd_init` / `pd_shutdown` / `pd_add_ref` / `pd_release` / `pd_month` | `expect?` | the slots; the month tick |
| `pd_message` | `msg`, `data?`, `old?`, `new?`, `expect?` | `DoMessage` |
| `pd_set_port` | `index`, any of `stage`, `trips`, `efficiency`, `usage` | pokes a port (test setup) |
| `pd_identity` | `kind`, `iids` (`{"0xIID": offset \| false}`) | `QueryInterface` |
| `pd_query` | `query` (`num_ports`, `num_airports`, `num_seaports`, `num_trips`, `avg_efficiency`, `least_efficient`, `all_port_lots`, `usage_values`, `maintenance_cost`, `city_has_sea`, `cell_sea_access`, `funding_ratio`, `required_budget`), `args?`, `expect` | the queries |
| `pd_expect` | `state` (partial: `tuning_loaded`, `initialized`, `tuning_instance`, `type_count`, `types`, `zone_type`, `ports`, `ref_count`, `destroyed`, `recheck_access`, `city_has_sea`, `department`, `full`, `current`, `month_ran`, `history`, `event`, `skipped`) | the developer |
| `pd_expect_port` | `index`, `state` (partial: `stage`, `type`, `x`, `z`, `efficiency`, `usage`, `trips`, `lot`, `needs_lookup`, `swapping`, `access`) | a port |
| `pd_expect_map` | `cells` (row-major), `size?` | the sea-access map |
| `pd_write` / `pd_read` | `hex?`, `slot?`, `fresh?`, `expect?` | the record |
| `pd_expect_events` / `pd_expect_event_count` / `pd_clear_events` | see the shared conventions | host calls |

Port developer events: `create_department`, `add_agent`, `remove_agent`, `full_expenses`, `withdraw`, `condition`,
`start_demolish`, `end_demolish`, `create_lot`, `set_configuration`, `construct`, `dezone`, `effects`, `history`,
`advisor`, `trip_capacity`, `props_state`, `altitude`.

## Prop developer ops

The prop developer vectors (`pv12_…`) drive `fc4_core::prop_developer::PropDeveloper`
(`docs/specs/prop-developer.md`) over one scripted city implementing `PropHost`. Every op name starts with `pv_`.
The default world: a 64 × 64 city, cell width 16, no lots, no props, every cell powered, no 24-hour clock, visual
mode 0, day, a 3D view with every trigger off, the managers and wealth requester token of the corpus. A network
occupant's token names its first cell (`0xXXXXZZZZ`). A prop removed through the prop manager keeps its type and
properties for its removal message, as the live occupant does (§4.7). Records are written and read with each reference as
`u32 token, u32 class`, so the spec's corpus records round-trip byte for byte. Every generator draw is logged as a
`draw` event (`gen` `"member"` or `"wealth"`; `call` `"RS"` with `lo`, `hi`, or `"U"` with `n`; `value`).

| op | Arguments | Effect / check |
|---|---|---|
| `pv_env` | any of `cells`, `clock` (`[hour, minute]` or null), `visual_mode`, `night` (the lighting manager), `view`, `triggers_on`, `date`, `heights` (`[terrain, water]`), `unpowered` (`[[x, z]]`, replacing the set), `zones` (`[[x, z, type]]`), `profiles` (`[[id, satisfaction, wealth]]`), `families` (`{"0xFAMILY": [ids]}`), `exemplars` (`{"0xID": {properties}}`, the properties of a created prop of that type), `wealth_token` | the world |
| `pv_lot` | `lot`, any of `x`, `z`, `size`, `livable`, `building`, `occupancy`, `wealth`, `state`, `objects` (`[{tag, ids?, type? (1), flags? (1)}]`; null: no configuration) | creates or updates a lot |
| `pv_network_lot` | `token`, `cell`, `objects?`, `wealth?` | a network lot |
| `pv_prop` | `prop`, any of `lot` (null: a street prop), `cell`, `tag`, `type`, `class`, `properties`, `state`, `pos` | creates or updates a prop |
| `pv_requester` | `token`, any of `share`, `satisfaction`, `custom`, `select` (what SelectProp answers; null: the first ID) | a host requester |
| `pv_script` | `member?`, `wealth?` (results to append) | the scripted generators |
| `pv_create` | `seed?` (the session seed: real generators, the wealth one shared per seed; none: scripted) | a new developer |
| `pv_init` / `pv_shutdown` / `pv_add_ref` / `pv_release` | `expect?` | the slots |
| `pv_add_requester` | `token`, `class` (`0x89C49986` is a wealth requester) | `AddPropRequester` |
| `pv_remove_requester` | `token`, `expect?` | `RemovePropRequester` |
| `pv_message` | `msg`, `object?`, `data1`…`data4?`, `expect?` | `DoMessage` |
| `pv_step` | `expect` (`[bx, bz]` or null) | one block step |
| `pv_update_block` | `block` (`[bx, bz]`) | one block update |
| `pv_update_all` | | `UpdateAllOptionalProps` |
| `pv_signs` | `show?`, `toggle?`, `expect?` (visible afterwards) | slots 8–10 |
| `pv_wealth_select` | `ids`, `x`, `z`, `expect` | the wealth requester's `SelectProp` |
| `pv_identity` | `iids?`, `wealth_iids?` (`{"0xIID": offset \| false}`), `member_seed?` (`[S, seed]`) | `QueryInterface`, the member seed |
| `pv_expect` | `state` (partial: `initialized`, `scan_flag`, `scanner` (`"fresh"`, `"running"`, `"done"`), `scan_byte` (the state byte written while running, PV13), `scan_at`, `clip`, `wealth_added`, `night_cursor`, `night_mode`, `night_batch`, `night_acc`, `is_night`, `tod_last`, `signs_visible`, `ref_count`, `destroyed`, `crt_state`, and the list lengths `requesters`, `night`, `triggers`, `time_of_day`, `dated`, `power`, `signs`) | the developer |
| `pv_expect_block` | `state` (partial: `block`, `lots`, `occ_sum`, `slots`, `kept_all`, `target`, `want`, `kept`, `removed`, `created`, `member_draws`, `stopped`) | the last block update |
| `pv_expect_lists` | any of `requesters` (`[[token, class]]`), `night`, `time_of_day`, `dated`, `power`, `signs` (tokens), `triggers` (`[[trigger, [tokens]]]`), `classes` (`{"0xTOKEN": class}`) | the lists |
| `pv_expect_prop` | `prop`, `state` (partial: `exists`, `state`, `type`, `lot`, `tag`, `requester`, `pos`) | a prop in the world |
| `pv_write` / `pv_read` | `hex?`, `length?`, `slot?`, `fresh?`, `seed?`, `expect?` | the record |
| `pv_expect_events` / `pv_expect_event_count` / `pv_clear_events` | see the shared conventions | host calls |

Prop developer events: `create_wealth_requester`, `add_agent` (`level`, `name`, `flags`), `remove_agents`,
`subscribe` (`msg`), `unsubscribe_all`, `animation_tick` (`priority`), `unregister_animation_tick`,
`post_lot_state_props` (`x`, `z`, `entered`), `hold` / `release` (`token`), `share` (`requester`, `rect`), `select`
(`requester`, `ids`, `x`, `z`), `remove_prop`, `create_prop` (`lot`, `index`, `id`, `prop`), `create_network_prop`,
`prop_state`, `timer` (`prop`, `mask`, `on`), `chance`, `time_of_day_range`, `date_range`, `tick_date`, `position`,
`flag`, `draw`.

## Demolition ops

The demolition vectors (`dm12_…`, `dm00_…`) drive `fc4_core::demolition::DemolitionUtility`
(`docs/specs/demolition.md`) over one scripted city implementing `DemolitionHost`. Every op name starts with
`demolition_`. The default world: a 64 × 64 city, the building development simulator, the flora simulator and the
prop manager present, no view filter, view level 1, the shipped thresholds and building development exemplars, no
occupants, no lots, zone 0 and altitude 0 everywhere, no water. Cells are 16 m; an occupant placed with `cell` is
centred in that cell with an 8 m box. `CanRemove(level)` answers false for level bit 1 with the system lock and
for level bit 2 with the player lock. Removing an occupant clears its flag 1 (in the occupant manager); a second
removal fails. `EndDemolishLot` leaves the lot in state 1 without a building; `CreateLotIfPossible` creates a 1 × 1
lot with the zone of its cell (tokens from `0x9000`). The utility's own token is `0xD0`.

### Setup

| op | Arguments | Effect |
|---|---|---|
| `demolition_env` | any of `cells` (`[x, z]`), `building_development_simulator`, `flora_simulator`, `prop_manager` (bools), `view_filter` (token or null), `view_level` (int or null), `rejects` (`{"filter": [occupants]}`), `rejects_buildings` (filters refusing the building type), `thresholds` / `building_development` (`"shipped"`, null or `{props}`), `water` (`[[x, z]]` vertices), `altitudes` (`[[x, z, h]]`), `zones` (`[[x1, z1, x2, z2, zone]]`) | the world |
| `demolition_occupant` | `occupant`, `type` (`foundation`, `building`, `pole`, `construction`, `prop`, `network`, `flora` or an ID), `cell?`, `bounds?` (`[x0, z0, x1, z1]` m), `flags?` (1), `system_lock?`, `player_lock?`, `foundation?` (slot 9's answer; absent: no foundation interface), `network_flags?` (absent: no network interface), `network_types?`, `pole?` / `network?` (the interfaces; default by type), `cost?`, `connections?`, `default_ok?` / `realistic_ok?` (true), `props?` (`{"0xID": bool}`), `groups?`, `extent?` (screen extent, 10; null: below the cutoff) | creates an occupant, or updates the given fields |
| `demolition_lot` | `lot`, `rect` (`[x1, z1, x2, z2]`, inclusive), `state?` (1), `building?`, `occupants?`, `demolishable?` (true), `zone?` (also written to the zone map), `water?` (false; null: no configuration), `locked?`, `initialized?` (true), `facing?`, `condition?` | creates a lot, or updates the given fields |
| `demolition_create` | | a new constructed utility, which replaces the current one |

### Calls and checks

| op | Arguments | Effect / check |
|---|---|---|
| `demolition_init` / `demolition_shutdown` / `demolition_add_ref` / `demolition_release` | `expect?` | the lifecycle slots |
| `demolition_identity` | `iids` (`{"0xIID": offset or null}`), `class` | `QueryInterface` and `GetGZCLSID` |
| `demolition_message` | `type` (`ExemplarChanged`, `PostCityInit`, `DeferredLotDemolition` or an ID), `data1?`, `data2?`, `lot?`, `dezone?`, `expect?` | `DoMessage` |
| `demolition_lots` | `do_it`, `lots` (tokens or null), `privilege?` (1), `flags?` (1), `dezone?`, `caller_filter?`, `demolished?`, `expect?` (`{ok, cost}`; cost null: left unchanged) | `DemolishLots` |
| `demolition_region` | `do_it`, `rect` (a full region), `privilege?`, `flags?`, `dezone?`, `caller_filter?`, `demolished?`, `clicked?`, `sx?`, `sy?`, `expect?` | `DemolishRegion` |
| `demolition_demolish_occupant` | `do_it`, `occupant`, `privilege?`, `flags?`, `skip_network_update?`, `orphans?` (a list token), `expect?` | `DemolishOccupant` |
| `demolition_terrain` | `do_it`, `per_vertex`, `rect`, `altitude`, `exempt?`, `reason?` (5), `expect?` (the count) | `ModifyTerrainHeight` |
| `demolition_expect` | `state` (partial: `initialized`, `destroyed`, `ref_count`, `tuning` (`[small/medium model, medium/large model, small/medium group, medium/large group, destroyed tile cost]`), `cells`) | the utility state |
| `demolition_expect_lot` | `lot`, `state` (partial: `state`, `building`, `zone`, `rect`) or `absent: true` | a lot |
| `demolition_expect_zones` | `cells?` (`[[x, z, zone]]`), `lot_count?` | the zone map and the lot count |
| `demolition_write` / `demolition_read` | `expect?`, `hex?` | the record (read into the current utility; without `hex`, the last written record) |
| `demolition_expect_events` / `demolition_expect_event_count` / `demolition_clear_events` | see the shared conventions | host calls |

Demolition events: `subscribe` / `unsubscribe` (`type`), `acquire` / `release_service` (`service`: `city`,
`occupant_manager`, `lot_developer`, `terrain`), `release` (`token`), `shutdown` / `pre_demolish` / `remove_prop` /
`delete_connection` / `sound` (`occupant`), `remove` (`occupant`, `flag`), `break` (`pole`), `append` (`list`,
`occupant`), `insert` (`set`, `occupant`), `queue` (`on`), `destroy` (`occupants`), `explode` (`lot`, `flag`),
`start` / `end` / `delete` (`lot`), `create` (`x`, `z`, `facing`), `dezone` (`rect`), `set_rect` (`rect`, `height`,
`reason`), `set_vertex` (`x`, `z`, `height`), `redisplay` (`rect`, `reason`), `effect` (`occupant`, `keys`, `ray`
(`[x, y]` or null), `grey`), `group` (`instance`, `x`, `z`).

## Dispatch ops

The dispatch vectors (`dp12_…`, `dp00_…`) drive `fc4_core::dispatch::DispatchManager` (`docs/specs/dispatch.md`)
over scripted dispatch sources implementing `DispatchHost`. Every op name starts with `dispatch_`. A source has
dispatch types, `CanDispatch` / all-busy answers and stations with a unit mask (bit `i` = unit `base + i` free);
`DispatchFromNearestStation` takes the first free unit of the first station that has one. New host objects
(vehicles, pylons) get tokens from `0x1000` up, in creation order, each with one reference. The terrain altitude
at (x, z) is (x + z) / 100. The vehicle view is `0x7E`; the manager's token is `0xD15`. References read from a
record resolve to their object IDs (the manager takes one reference each).

### Setup

| op | Arguments | Effect |
|---|---|---|
| `dispatch_env` | `vehicles_fail?` (BeginVehicle fails), `vehicle_group?` (the occupant group of new vehicles; null: `0x4202` for type 1, `0x4303` for type 5, else `0x4101`), `park_fails?` (the return's unit-state call fails: nothing placed) | the world |
| `dispatch_source` | `source`, `types?`, `can?` (true), `all_busy?` (false), `nearest?` (true), `stations?` (`[{station, base? (1), count, mask? (all free)}]`), `register?` (true: `RegisterDispatchSource` on creation) | creates a source, or updates the given fields |
| `dispatch_create` | | a new constructed manager |

### Calls and checks

| op | Arguments | Effect / check |
|---|---|---|
| `dispatch_init` / `dispatch_shutdown` / `dispatch_add_ref` / `dispatch_release` | `expect?` | the lifecycle slots |
| `dispatch_identity` | `iids`, `class` | `QueryInterface` and `GetGZCLSID` |
| `dispatch_message` | `type` (`RemoveOccupant`, `DisasterPeriodEnded`, `ViewChanged`, `AutomatonEvent` or an ID), `code?` (data2), `state?` (data3), `object?` or `automaton_of?` (a dispatch ID: its record's automaton), `expect?` | `DoMessage` |
| `dispatch_call` | `method`, `args`, `expect?` (a record is compared on the fields given) | `dispatch (type, x, z)`, `can_dispatch (type, x, z)`, `get_dispatch_source (type)`, `get_dispatch_info (id)`, `get_dispatch_count (type)`, `get_dispatch_id_array (type, n)`, `get_active_pylon`, `get_automaton_from_pylon (pylon)`, `get_available_pylon_count (type)`, `get_dispatch_vehicle_view`, `update_dispatch_state (id, state)`, `revoke_dispatch_by_id (id)`, `revoke_dispatch_by_pylon (pylon)`, `revoke_all_dispatches (type)`, `begin_dispatch_tool (type)`, `end_dispatch_tool (type)`, `set_active_pylon (pylon or null)`, `register_dispatch_source` / `unregister_dispatch_source (source)` |
| `dispatch_expect` | `state` (partial: `initialized`, `destroyed`, `ref_count`, `loaded`, `next_id`, `tool_type`, `record_count`, `records` (`[{type, id, call, view_handle, active, target, station, automaton, pylon}]`), `active_pylon`, `available_pylons`, `available_calls` (their call numbers), `sources`, `vehicle_view`) | the manager state |
| `dispatch_expect_units` | `source`, `masks` (one per station) | the unit masks |
| `dispatch_expect_refs` | `{"token": references}` | references held |
| `dispatch_write` | `expect?`, `hex?`, `length?` | the record |
| `dispatch_read` | `hex?` (or the last written record), `reuse?`, `expect?` | `Read` into a new constructed manager (or the current one) |
| `dispatch_expect_events` / `dispatch_expect_event_count` / `dispatch_clear_events` | see the shared conventions | host calls |

Dispatch events: `acquire` / `release_service` (`service`: `message_server`, `city`), `subscribe` / `unsubscribe`
(`type`), `post` (`type`, `dispatch_type`, `id`), `create_view`, `destroy_view` / `refresh_view` (`view`), `track`
(`automaton`, `kind`, `handle`), `untrack` (`handle`), `nearest` (`station`, `call`), `from_station` /
`revoke_from_station` (`station`, `call`), `vehicle` (`automaton`, `id`, `call`), `vehicle_failed` (`id`),
`despawn` / `go_home` (`automaton`), `create_pylon` / `pylon_shutdown` (`pylon`), `move` (`pylon`, `x`, `y`, `z`),
`place` (`pylon`, `station`, `index`, `count`), `park` / `park_failed` (`pylon`, `station`).

## City situation ops

The city situation vectors (`cs12_…`, `cs00_…`) drive `fc4_core::city_situation::CitySituationManager`
(`docs/specs/city-situation.md`) over a scripted game implementing `CitySituationHost`. Every op name starts with
`citysit_`. The default world: the debug flag clear, every service present, the four automata managers `0x43`,
`0x45`, `0x44`, `0x46`, an empty (all-zero) reward list, the host's steps 2–4 of `CheckForSuccess` answering 3 (the
core runs the mission clock, CS4), the popup closing with button 0, funds 100,000 (a withdrawal that would leave them
below zero is refused, budget.md §6), coverage cells that build, no advices, no automata, no groups, no created
automaton, no MySim window. The manager's token is `0xC5`; its generator is seeded with
`derive_seed(0x12345678, 0x0BB14381)`.

### Setup

| op | Arguments | Effect |
|---|---|---|
| `citysit_env` | any of `debug`, `rewards` (the reward list from its start), `status` (the answer of the host's steps 2–4 of `CheckForSuccess`), `button`, `funds`, `candidates` (`{"advice": [automata]}`), `groups` (`{"group": [automata]}`), `created` (the automaton `CreateAutomatonForPlayerDrive` hands over, held; null: none), `coverage_ok` (default true), `mysim_window` (the window of advice flag `0x20`, held; null: none) | the world |
| `citysit_advice` | `advice`, `guid?`, `flags?`, `mission_type?` (2), `end_delay?`, `success_money?`, `failure_money?`, `success_aura?` / `failure_aura?` (`[radius, magnitude]`), `target?` (the first target; absent: none), `target_count?`, `time_limit?`, `accepts?` (the automata advice slot 53 accepts; logs an `advice_accepts` event per question), `accepts_groups?` (the groups advice slot 54 accepts), `groups?` (advice slot 12), `priority?` (`cISC4Advice` slot 7, default 0), `next_targets?` (what choosing the next target hands out, in order; then none) | a city-situation advice of advisor 29 |
| `citysit_automaton` | `automaton`, `flags?` (2), `type?` (1), `position?` (`[x, z]`), `preview?`; or `remove: true` | an automaton |
| `citysit_create` | `session_seed?` | a new constructed manager |
| `citysit_set` | `success_counts?` (`{"0xGUID": count}`), `loaded_from_save?` | pokes the state (test setup) |
| `citysit_mark` | | remembers the state and the generator's next word |

### Calls and checks

| op | Arguments | Effect / check |
|---|---|---|
| `citysit_init` / `citysit_shutdown` / `citysit_add_ref` / `citysit_release` | `expect?` | the lifecycle slots |
| `citysit_identity` | `iids` (`{"0xIID": offset or null}`), `class` | `QueryInterface` and `GetGZCLSID` |
| `citysit_message` | `type` (`OccupantInserted`, `OccupantRemoved`, `AdviceAdded`, `AdviceRemoved`, `AdviceRemovedAll`, `PopupClosed`, `ShowPopup`, `Window`, `InitiateByGuid`, `PostCityInit`, `AnimationTick`, `AutomatonNotify`, `PlayerLeftVehicle`, `Cheat`, `PopupNoAdvice` or an ID), `data1?`, `data2?`, `data3?` or `ms?` (the tick's float bits), `object?`, `void1?`, `text?`, `expect?` | `DoMessage` |
| `citysit_ticks` | `count`, `ms` | that many animation ticks |
| `citysit_call` | `method`, `args`, `expect?` | `initiate_situation (advice (null allowed), vehicle, briefing)`, `initiate_situation_by_guid (guid, briefing, group)`, `initiate_situation_for_occupant (occupant, advice?)`, `initiate_situation_by_automata_group (group, briefing, search)`, `start_joyride (automaton, b, target)`, `treat_automaton_as_group (automaton (null allowed), group)`, `check_for_success (t_ms)`, `activate_situation`, `deactivate_situation`, `end_situation`, `handle_status_changed (s)`, `get_mission_success_count (guid)`, `lua_activate_situation (guid, briefing)`, the getters of §8 (`get_active_situation`, `get_current_status`, `get_active_auto`, `get_current_target`, `get_current_target_index`, `get_num_targets`, `get_num_targets_remaining`, `get_current_time_limit_seconds`, `get_current_situation_time`, `get_current_time_remaining_seconds`, `get_current_target_name`, `get_situation_aura`, `get_situation_money`, `is_situation_available`, `get_max_csi`, `get_tracked_automata_count`, `get_csi_visible`, `countdown_is_in_progress`), `set_max_csi`, `set_csi_visible`, `handle_tuning_parameters_changed`, `reset_tutorials`, `initiate_player_drive_mode`, `random_uniform_u32 (n)` |
| `citysit_expect` | `state` (partial: `initialized`, `destroyed`, `ref_count`, `indicators_visible`, `loaded_from_save`, `active_situation`, `automata_managers`, `track_list` (`[{advice, automaton, handle}]`), `tracked`, `success_counts` (`{"0xGUID": count}`), `status`, `active_automaton`, `current_target`, `num_targets`, `target_index`, `end_delay`, `elapsed`, `countdown`, `time_limit`, `just_activated`, `warning_given`, `max_csi`, `window`) | the manager state |
| `citysit_expect_unchanged` | | the state and the generator are as at `citysit_mark` |
| `citysit_expect_world` | `state` (partial: `funds`, `rewards` (up to the last non-zero entry), `retired`) | the game |
| `citysit_expect_refs` | `{"token": references}` | references held |
| `citysit_write` | `expect?`, `hex?` | the record |
| `citysit_read` | `hex?` (or the last written record), `reuse?`, `expect?`, `consumed?` | `Read` into a new constructed manager (or the current one) |
| `citysit_expect_events` / `citysit_expect_event_count` / `citysit_clear_events` | see the shared conventions | host calls |

City situation events: `register_cheat`, `unregister_cheat`, `subscribe` / `unsubscribe` (`type`), `acquire` /
`release_service` (`service`: `indicator_view`, `lot_manager`, `mysim`, `lua`), `register_tick`,
`unregister_tick`, `acquire_automata_managers` (`managers`), `create_lua_table`, `remove_lua_table`,
`release_mission_resources`, `show_all_indicators` (`kind`, `visible`), `save_preferences`, `retire` (`advice`),
`script` (`advice`, `script`: `start`, `success`, `failure`), `bind` (`advice`, `vehicle`), `coverage` (`advice`,
`built`), `next_target` (`advice`, `target`), `create_automaton` (`advice`, `group`, `automaton`),
`remove_automaton` (`automaton`), `set_flag` (`automaton`, `flag`), `set_group` (`automaton`, `group`),
`track` (`automaton`, `handle`), `untrack` (`handle`), `listen` / `unlisten` (`automaton`), `freeze` (`manager`,
`on`), `sound` (`automaton`, `sound`), `countdown_start`, `countdown_end`, `outcome_effect` (`advice`, `success`,
`x`, `z`), `drive` (`vehicle`, `on`), `joyride` (`vehicle`), `drive_view` (`automaton`, `target`),
`clear_mission_state` (`advice`), `release_situation_effect`, `aura` (`x`, `z`, `radius`, `magnitude`), `deposit`
(`amount`, `accepted`), `send` (`type`, `advice` (null: a joyride), `data1`), `post` (`type` and `advice`,
`vehicle`, `data1`, `data2` as the message has them), `hide_mission_status_panel`, `end_mysim_dialog`, `popup`
(`popup`: `briefing`, `success`, `failure`; `advice`, `vehicle`).

## Automata ops

The automata vectors (`am14_…`) drive `fc4_core::automata` (`docs/specs/automata.md`). Every op name starts with
`automata_`. One world per vector holds a script set (empty), a controller manager whose host logs `log <text>`,
`add_ref 0x…` and `release 0x…` (the low 32 bits of the token), an automata manager, a scripted city for the riot
and fire hooks, and the riot leaders and fire hook. Records are read with references resolved to host tokens
`class << 32 | id` and written back as ID and class. An occupant given by number is a building of class
`0xA9BD882D`. Controllers the manager creates get tokens from `0x1000` upward; controllers read by `cm_read` are core
controllers whose token is their object ID.

### Records

| op | Arguments | Check |
|---|---|---|
| `automata_record` | `class` (any class of the layer: components 40, 41, 43–46 or a referenced object), `hex`, `expect?` (false: Read fails), `left?` (bytes left after the read; default 0), `fields?` (partial: `version`, `base_version`, `clock`, `time_a`, `time_b` (`[hour, day]`), `entries`, `sets`, `loaded`, `max_count`, `active`, `time_scaling`, `automata`, `group_counts` (`[[group, count]]`), `tag`, `cursor`, `incidents`, `old_version`, `pairs`, `spots`, `lots`, `occupants`, `ferries`, `occupant` (`[id, class]`), `template`, `position`, `flags`, `curve`, `period`, `second_period`, `day_stamp`, `counter`), `written?` (the expected re-encoding; default the input) | Read, the fields, Write |

### Controller manager

| op | Arguments | Effect / check |
|---|---|---|
| `automata_scripts` | `templates?` (`[{name, kind? ("generator" or "attractor"), flags?}]`), `groups?` (`[{id, name, templates}]`) | replaces the script set |
| `automata_cm_new` | | a new controller manager (not loaded) |
| `automata_cm_read` | `hex`, `templates?` (`{"<controller id>": name}`: the template name each read controller holds), `expect?` | Read |
| `automata_cm_add` | `occupant`, `template`, `expect?` (whether a controller was made) | slot 5 `AddControllerFromTemplateName` |
| `automata_cm_add_group` | `occupant`, `id` or `name`, `no_scripts?`, `expect?` | slots 9 and 10 |
| `automata_cm_remove` | `occupant`, `expect?` (the count removed) | slot 14 `RemoveControllers` |
| `automata_cm_remove_group` | `occupant`, `id` or `name`, `expect?` | slots 12 and 13 |
| `automata_cm_post_city_init` | `expect?` (the dropped template names, in order) | `PostCityInit` |
| `automata_cm_expect` | partial: `occupants` (`{"<occupant>": [template names]}`, every occupant), `loaded`, `bound` (per controller in order: template bound), `shut_down` (the read controller IDs shut down) | the manager state |
| `automata_cm_write` | `hex` | Write |
| `automata_expect_log` / `automata_clear_log` | `log` | the host log |

### Managers and hooks

| op | Arguments | Effect / check |
|---|---|---|
| `automata_mgr_new` | `kind` (`vehicle`, `pedestrian`, `aircraft`, `watercraft`), `automata?` (a count of placeholder automata) | a new manager |
| `automata_mgr_max_count` | `n`, `state?` (partial: `max`, `hard_limit`, `removed`, `count`) | `SetMaxCount` |
| `automata_mgr_driving_mode` | `view`, `expect` | `IsDrivingMode` |
| `automata_automaton_controller` | `flags`, `null` | an automaton whose controller has `flags` writes a null controller reference or not |
| `automata_city` | any of `road`, `cap` (the leader cap is reached), `create_null`, `buildings` (occupants whose type is a building) | the scripted city |
| `automata_riot_add` | `x`, `z`, `expect` (the leader index or null) | `AddRiotLeader` |
| `automata_riot_stage` | `leader?`, `stage`, `expect` | the stage update |
| `automata_riot_end` | `leader?`, `expect` | the end |
| `automata_fire_target` / `automata_fire_shutdown` | `target` / none, `expect` | the fire occupant's hooks |
| `automata_expect_calls` / `automata_clear_calls` | `calls` | the city calls, as text |

City calls: `FindRoad x z`, `LeaderCap n`, `CreateAutomaton group (x, y, z) mode (dx, dy, dz) a b c`, `AddRef`,
`SetState automaton type a b c d e f`, `SetVisible automaton bool`, `RegisterLocation`, `AddControllerFromTemplateName
occupant name extra`, `RemoveControllers`, `UnregisterLocation`, `Release`, `AddControllersFromOccupantGroup name
occupant extra`. A road tile at `(x, z)` is at `(16x + 8, 0, 16z + 8)`; the created automaton is `0x500`, `0x501`, …;
its occupant is the automaton `| 0x10000`.

## Disaster ops

The disaster vectors (`ds18_…` for the §18 tests, `ds00_…` for the rest) drive `fc4_core::disasters::Disasters`
(`docs/specs/disasters.md`) over one scripted city implementing `DisasterHost`. Every op name starts with `dis_`.
The world outlives `dis_create`, so a vector can build a new layer over the same city. The default world: a 64 × 64
city of 16 m cells, established, not paused, wall clock at 0 ms, follow-disasters off, view level 1, the shipped
"Flame" and "Riot" exemplars (no others: the short-form disasters use their defaults), no occupants, cells or
automata, session seed `0x12345678` and the stub generator for all five generators (fire, riot, earthquake,
pipe burst and the pipe burst's start). The stub returns queued `u32s` (else 1 for a raw draw and n − 1 for
`RandomUint32Uniform(n)`) and `doubles` (else 0.99; a uniform range maps a queued `u` to `a + u (b − a)`, default
0; a Gaussian range the same, default 0.5); every draw is an event. Tokens: fire occupants `0xF000 + n`, riot
leaders `0xA000 + n`, toxic occupants `0xC000 + n`, in creation order over the world's life. The world's emergency
pause and save-disable counters follow the layer's calls; the simulation counts as paused while either the manual
pause or the emergency pause is on.

### Setup

| op | Arguments | Effect |
|---|---|---|
| `dis_env` | any of `cells` [x, z], `cell_width`, `paused`, `emergency` (the counter), `established`, `follow`, `no_burn_down` (the tutorial flag), `demolish_removes` (a demolition removes the occupant and sends `OccupantRemoved` to the layer inside the call), `wind`, `view_level`, `tuning` (`{flame, riot, earthquake, pipe_burst, volcano: "shipped" (flame and riot), null or {props}}`), `random` (`"stub"` or `"default"`, read by `dis_create`), `u32s`, `doubles` | the world |
| `dis_occupant` | `occupant`, `type?` (`building`, `flora`, `prop`, `network` or an ID; building), `cell` [x, z], `rect?` [x0, z0, x1, z1], `position?`, `in_manager?` (true), `lot?` (occupant + `0x100000`; null: none), `flags?`, `props?` (`{"0xID": number, bool or null}`; the flammability `0x29244DB5` defaults to 100), `coverage?` (fire coverage, 0), `areas?` [top, side] | adds or updates an occupant |
| `dis_cell` | `cell`, `cells` or `rect` (inclusive), with any of `flammability`, `police`, `road`, `network` (the network occupant's token or null), `network_base` (each cell its own network occupant: base + the cell's index), `pipe` (a token or null), `pipe_neighbours` (the connected cells of that pipe) | cell data |
| `dis_automaton` | `automaton`, `cell` (null: gone) | an automaton's cell |
| `dis_create` | `session_seed?` | a new layer (not initialised), which replaces the current one |
| `dis_clock` | `set?`, `advance?` (ms) | the wall clock |

### Calls and checks

| op | Arguments | Effect / check |
|---|---|---|
| `dis_init` / `dis_shutdown` | `expect?` | the layer's `Init` and `Shutdown` |
| `dis_call` | `method`, `args?`, `expect?` | `enable_disasters`, `disasters_enabled`, `has_manager`, `disaster_has_begun`, `disaster_has_ended`, `is_disaster_ongoing`, `register_location [instance, x, z, occupant, zoom, focus]`, `unregister_locations`, `unregister_location_by_occupant`, `most_recent_location` ([x, z] or null), `location_count`, `go_to_next_disaster`, `disaster_cost`, `reset_disaster_cost`, `attempt_start [manager, x, z]` and `attempt_start_any [manager]` (the instance or null), `active_count`, `instance_shutdown`, `fire_instance`, `ignite_occupant`, `ignite [occupant, force, fire_id, focus]`, `ignite_cell` and `manager_ignite_cell [x, z, use_probability, teleport, fire_id]`, `extinguish_cell [x, z, amount]`, `quick_burn_cell`, `ignite_cell_radius [x, z, r0, r1]`, `extinguish`, `burn_down`, `set_cause`, `cause`, `fire_count`, `fire_count_in_rect [x0, z0, x1, z1]`, `fire_update` (the fire's tick), `fire_set_hosed [fire occupant, delta]`, `spill_toxic_waste [x, z, amount]`, `toxic_set_hosed`, `volcano_request_end`, `start_disaster_effect [effect, x, z]`. Managers by name (`volcano`, `fire`, `earthquake`, `pipe_burst`, `riot`, `effect`, `toxic`) or ID |
| `dis_message` | `target` (`"layer"`, `{"manager": …}` or `{"instance": id}`), `type` (`OccupantRemoved`, `OccupantInserted`, `DispatchBegin`, `DispatchActive`, `DispatchInactive`, `DispatchEnd`, `ToxicSpill`, `PreViewLevelChange`, `EffectFinished`, `EffectMoved` or an ID), `data1..3?`, `void1?`, `x?`, `z?`, `expect?` | `DoMessage` |
| `dis_tick` | `instance` (an ID or `"fire"`), `us?` (16000), `count?` (1), `advance_ms?` (added to the clock before each tick) | animation ticks |
| `dis_seconds` | `count`, `instance?` (`"fire"`) | `count` times: one second of wall clock, then the fire's update (or a tick of the instance) |
| `dis_flame_tuning` | `tuning` (`"shipped"`, null or `{props}`), `state?` (partial: `d3`, `d3cum`, `d4`, `d5`, `d6`, `zoom`, `d9_empty`), lookups `stages`, `d9`, `resistance`, `da` (`[[input, output]]`) | `FireTuning` |
| `dis_riot_tuning` | `tuning`, `state?` (partial: `max_riots`, `max_leaders`, `stages`, `seconds_per_stage`, `leaders_spawned`, `zoom`), lookups `police`, `stage_of` | `RiotTuning` |
| `dis_expect` | `state` (partial: `initialised`, `enabled`, `disable_counter`, `ongoing`, `assessing`, `building_count`, `total_cost`, `kinds`, `special_count`, `fire_timers` / `police_timers` ([[key, seconds]]), `overall_s`, `fire_effectiveness`, `police_effectiveness`, `locations` ([{instance, x, z, zoom, occupant}]), `managers`, `emergency`, `save_disabled`, `fire_count`, `reentrant_refused`, `riot_locked` (the network occupants the riots hold locked); with a fire instance `fire_phase`, `fires`, `fire_ids`, `toast`, `cause`, `elapsed_ms`, `next_ms`, `toasted`, `fire_tick`, `d3cum`, `zoom`; `toxic_instance` (null without one) and `toxic_amounts`) | the layer and the world |
| `dis_expect_fire` | `index?` (0), `state` (partial: `occupant`, `target`, `counter`, `stage`, `max_stage`, `fire_id`, `hosed`, `extinguish_at`, `top_flames`, `side_flames`) or `absent: true` | one fire of the list |
| `dis_expect_riot` | `instance`, `state` (partial: `phase`, `leader_count`, `leaders` ([{automaton, counter, stage, max_stage, cell}])) or `absent: true` | a riot |
| `dis_expect_instance` | `instance`, `state` (partial: `type`, `class`, `phase`, `location`, `tick`) or `absent: true` | any instance |
| `dis_expect_refs` | `refs` (`{"token": count}`) | references the core holds |
| `dis_expect_events` / `dis_expect_event_count` / `dis_clear_events` | see the shared conventions | host calls |

Disaster events: `emergency_pause`, `emergency_resume`, `disable_save`, `enable_save`, `post` (`type`, `data1`,
`data2`, `data3`, `data4`, `void1`), `subscribe` / `unsubscribe` (`target`, `type`), `register_tick` (`instance`,
`priority`), `unregister_tick` (`instance`), `history` (`series`, `value`), `tuning` (`instance`), `go_to` (`x`,
`z`, `zoom`), `fire_effectiveness` / `police_effectiveness` (`value`), `remove_occupant`, `condition` (`lot`,
`condition`), `create_fire` (`instance`, `fire`, `target`, `fire_id`, `top`, `side`), `remove_fire` (`fire`,
`target`), `aura`, `effect` (`name`, `occupant`), `effect_at` (`name`, `x`, `z`), `burn_flora`, `demolish`
(`occupant`, `lot`, `flags`), `radiation` (`occupant`, `amount`, `radius`), `leader` (`automaton`, `x`, `z`),
`unset_removal` / `set_removal` (`occupant`, `flags`: the riot's removal lock), `riot_effect` / `riot_message` (`automaton`, `stage`), `end_leader`, `leader_tick`, `toxic`
(`instance`, `occupant`, `x`, `z`, `amount`), `toxic_amount`, `remove_toxic`, `brush` (`brush`, `x`, `z`,
`strength`, `width`), `redraw` (the rect), `end_long_terrain`, `stop_textures`, `repulsor` (`x`, `z`),
`leak_effect` (`pipe`, `level`, `shown`), `demolish_pipe` (`pipe`, `flags`), `volcano_deformation`,
`volcano_end_deformation`, `lava` (`instance`, `radius`), `lava_end`, and the stub's `raw`, `u32` (`n`),
`double`, `range` and `gaussian` (`a`, `b`), each with its `generator`.

## Prop and lot configuration manager ops

The `plt_…` vectors (prop-lotconfig.md §T) drive `fc4_core::prop_manager::PropManager` (Part A, ops `pm_…`) and
`fc4_core::lot_config_manager::LotConfigManager` (Part B, ops `lcm_…`), each over its own scripted city that
outlives `*_create`. Tokens are integers (`"0x301"`); props, configurations and decals the core has the world
create get tokens from `0x1000` in creation order. The prop world: the occupant manager `0x50`, the lot manager
`0x51`, the resource manager `0x52`, the view filter `0x53`, 16 m cells, exemplars in group `0x1234`; the occupant
manager's box query returns the inserted occupants of the filter's usage inside the box (bounds included), its
cell-rectangle query those within one cell of the rectangle, each with a reference for the caller. The lot world:
detail 3, the building development simulator `0x50`, the flora simulator `0x51`, the prop manager `0x52`;
configurations load from `lcm_lot_exemplar` with the core's object reader (`lot_object_array`).

| op | Arguments | Effect / check |
|---|---|---|
| `pm_env` | any of `no_resource_manager`, `no_sink` (props have no exemplar interface), `no_services`, `gzcom_classes` (classes GZCOM can create) | the world |
| `pm_exemplar` | `instance`, `families?`, `size?` (OccupantSize floats), `class?`, `exemplar_type?` (`0x1E`), `group?`, `properties?` (`{props}`) | a prop exemplar |
| `pm_occupant` | `occupant`, `pos?` [x, y, z], `usage?`, `inserted?`, `shown?`, `refs?` (its starting count) | an occupant |
| `pm_lot` | `lot`, `rect` [x1, z1, x2, z2] (cells, inclusive) | a lot |
| `pm_building` | `building`, `cell` [x, z], `props` (null: no property holder; `"absent"`; or the property's prop list) | a building |
| `pm_create` | `same_catalogue?` (keep the process catalogue; else a new one) | a new prop manager |
| `pm_init` / `pm_shutdown` | `expect?` | `Init` / `Shutdown` |
| `pm_call` | `method`, `args?`, `list?` (the caller's list before the call), `vec?` (slot 9's vector), `expect?` | `is_prop_type_valid`, `is_prop_family_type_valid`, `is_prop_in_family [type, family]`, `prop_types_by_family` (`{result, vec}`), `all_prop_types` and `all_prop_family_types` (sorted), `prop_key_from_type` ([type, group, instance] or null), `prop_name`, `prop_profile` and `prop_profile_ptr` (`{class, size, satisfaction, wealth, crime}` or null), `create_prop` (the token or null), `add_city_prop`, `add_lot_prop [lot, occupant]`, `add_building_prop [building, prop]`, `get_lot_props [lot]`, `get_lot_props_at [x, z]`, `get_street_props_at [x, z]`, `get_building_props [building]`, `get_building_props_at [x, z]` (each `{result, list}`), `remove_building_prop [building, prop]`, `remove_building_props`, `remove_lot_props`, `remove_prop`, `remove_street_props_from_tile [x, z]` |
| `pm_expect` | `state` (partial: `initialized`, `occupant_manager`, `resource_manager`, `lot_manager`, `view_filter`, `filter`, `types`, `families`, `profiles` (the catalogue's map sizes)) | the manager |
| `pm_expect_occupant` | `occupant`, `state` (partial: `prop_type`, `bbox` ([min, max] or null), `builtin` (`plain`, `terrain_decal`, `visual_effect` or null), `class`, `usage`, `inserted`, `shown`, `inits`, `shutdowns`, `pos`) | an occupant |
| `pm_expect_building` | `building`, `props` | the building-prop property |
| `pm_expect_refs` | `refs` (`{"token": count}`) | reference counts |
| `pm_record` / `pm_read` | `hex`; `pm_read` also `objects?` (`{"n": token}`, default `0x4F + n`) and `expect?` | the component 5 record |
| `lcm_env` | any of `detail`, `no_resource_manager`, `valid_walls` | the world |
| `lcm_lot_exemplar` | `id`, `families?`, `size?` [x, z] (1 × 1), `version?` (2; null: absent), `objects` (each the lot object's u32 values) | a lot configuration exemplar of group `0xA8FBD372` |
| `lcm_remove_exemplar` | `id` | removes it |
| `lcm_config` | `label`, `id`, `families?`, `size?`, `walls?`, `objects?` (`[{kind, ids, rect? [min_x, min_z, max_x, max_z], centre? [x, z], orientation?}]`) | a configuration not from an exemplar (one reference) |
| `lcm_set_objects` | `configuration`, `objects` | replaces its objects |
| `lcm_building_type` | `type`, `capacity?`, `footprint?` [w, d], `exemplar?` (`{props}`) | a building type |
| `lcm_prop_type` / `lcm_flora_type` | `type`, `size` [x, z] / `radius` | a prop or flora type |
| `lcm_building_family` / `lcm_prop_family` / `lcm_flora_family` | `type` (the family), `types` | a family |
| `lcm_filter` | `filter`, `ids` (the configuration IDs it accepts) | a lot configuration filter |
| `lcm_objects` | `objects`, `version?`, `detail`, `expect` ([[type, v1], …]) | the object reader alone |
| `lcm_lod` | `v1`, `expect` | `LOD = (v1 >> 4) & 3` |
| `lcm_create` | `same_catalogue?` | a new manager |
| `lcm_init` / `lcm_shutdown` / `lcm_unload` | `expect?` | `Init`, `Shutdown`, the process unload callback |
| `lcm_remember` / `lcm_expect_same` | `id`, `as` (a label); `lcm_expect_same` also `same` | slot 8's object kept under a label / compared |
| `lcm_call` | `method`, `args?`, `as?`, `expect?` | `configuration_count`, `object_count` (summed over the map), `configuration_ids` (sorted), `family_id_set` (sorted), `configuration [id]` (its ID or null), `add_configuration [configuration, add]`, `remove_configuration [id]`, `configurations_by_family`, `configurations_by_size [x, z]`, `configurations_by_filter [filter or null]` (IDs in the core's order), `ids_by_family`, `ids_by_size`, `ids_by_filter`, `register_filter`, `unregister_filter`, `flush`, `has_building_family` and `has_building_type [configuration, id]` (booleans), `object_id_array [type, ids]`, `footprint_props`, `footprint_buildings`, `footprint_flora [ids]` ([w, d, 0, 0]), `post_city_init`, `fix [configuration]` (the code), `create_generic [type]` (`{size, objects}` or null; `as` labels it) |
| `lcm_expect` | `state` (partial: `initialized`, `building_development`, `flora`, `prop_manager`, `filters`, `loaded`, `detail`) | the manager and its catalogue |
| `lcm_expect_config` | `configuration`, `state` (partial: `id`, `size`, `objects`, `object_count`, `info` (`{min_capacity, max_capacity, groups, non_monopoly}` or null), `walls`, `generic_for`) | a configuration |
| `lcm_expect_refs` | `refs` (tokens or labels) | reference counts |
| `lcm_record` / `lcm_read` | as `pm_record` / `pm_read` | the component 9 record |

Configuration arguments are labels, tokens or null. Events (`*_expect_events`, `*_expect_event_count`,
`*_clear_events`): the prop world's `shutdown_callback`, `post` (`manager`, `data2`), `command` / `uncommand` (`id`),
`subscribe` / `unsubscribe` (`type`), `hide`, `remove` (`occupant`), `add_property` / `remove_property`
(`building`); the lot world's `register_callback`, `unregister_callback`, `subscribe`, `unsubscribe`.

## Tutorial ops

The `tu8_…` vectors (tutorial.md §8) and `tu00_…` drive `fc4_core::tutorial::TutorialSystem` over a scripted
registry, window (token `0x40`, task none) and 3D view. Every op name starts with `tut_`.

| op | Arguments | Effect / check |
|---|---|---|
| `tut_env` | any of `cheats` (the cheat manager exists and the app allows cheats), `registry` (`{"guid": entry byte +0x10}`), `registry_result` (what `SetActive` leaves in AL), `task` (the window's current task or null), `task_edge` | the world |
| `tut_create` | | a new system (constructed state) |
| `tut_read` / `tut_record` | `hex`, `expect?` | `Read` / `Write` of the component 48 record |
| `tut_message` | `type` (`CityInitComplete`, `EndTutorial`, `PreSave` or an ID), `data?`, `expect?` | `DoMessage` |
| `tut_call` | `method`, `args?`, `expect?` | `init`, `shutdown`, `start_tutorial`, `end_tutorial [bool]`, `is_tutorial`, `task_number`, `guid`, `set_task_number`, `increment_task_number`, `is_edge_scrolling_allowed`, `current_task`, `scripting_context`, `filter_message [kind, id, field2, field3]`, `show_ui_hints`, `handle_catalog_menu`, `filter_left_mouse_click [x, z, n]`, `reset_num_and_index [a, b]` (the window answers `b`), `on_tutorial_event` |
| `tut_expect` | `state` (partial: `is_tutorial` (the byte), `task_number`, `guid`, `field_28`, `end_pending`, `start_tried`, `window`, `window_refs`, `registry_active` (`{"0xGUID": bool}`)) | the system |

Tutorial events: `subscribe` / `unsubscribe` (`type`), `register_cheat`, `unregister_cheat`, `create_window`,
`init_window`, `close_window`, `window_filter` (`value`), `window_hints`, `window_menu` (`menu`), `window_reset`
(`a`, `b`), `window_event` (`event`), `set_active` (`guid`, `active`), `view_restore`.

## Occupant manager ops

The occupant manager vectors (`om18_…`, `om14_…`) drive `fc4_core::occupant_manager::OccupantManager`
(`docs/specs/occupant-manager.md`) over one scripted world implementing `OccupantHost`. Every op name starts with
`om_`. Occupants are tokens with a bounding box, a type, flags, a stored manager box and the base occupant behaviour
of §5.6: the first `Init` posts Create (unless `quiet`, bit 1 of `+0x2E` as a loaded occupant's own `Read` sets it: then an `occupant_slot17` event), `Shutdown` of an
initialised occupant posts Destroy, and `PostOccupantMessage` records a `post` event (a Move of a `sync` occupant is
marked `sync: true`). References are counted per token (net). Query results are released by the runner after the
checks (unless `keep: true`), so reference counts show only the manager's own references.

### Setup

| op | Arguments | Effect |
|---|---|---|
| `om_create` | | a new constructed manager (not initialised, no rows), which replaces the current one |
| `om_occupant` | `occupant`, any of `type` (default `0x100`), `min`, `max` (`[x, y, z]`), `holder` (default true), `flags`, `om_box` (`[x0, z0, x1, z1]` or null = the "no box" mark), `initialized`, `quiet`, `sync` | creates or updates an occupant |
| `om_filter` | `filter` (a token), `exclude` (tokens) | a caller filter that includes everything else |
| `om_trace` | `locks?`, `refs?` | also record `lock` / `unlock` (`depth`) and `add_ref` / `release` (`occupant`) events |

### Calls and checks

| op | Arguments | Effect / check |
|---|---|---|
| `om_call` | `method`, its arguments, `expect?` | one slot (below) |
| `om_expect` | `state` (partial: `initialized`, `was_read`, `ref_count`, `destroyed`, `size`, `counts` (the four), `filter_type` (null: no shared filter), `rows`, `holders`, `nodes`) | the manager |
| `om_expect_cell` | `cell` (`[x, z]`, world cells), `occupants` (front first) and / or `single` (bit 0 per node), or `null: true` (no holder) | a cell list |
| `om_expect_occupant` | `occupant`, `state` (partial: `flags`, `om_box`, `initialized`) | an occupant |
| `om_expect_refs` | `{"token": references}` | net references taken |
| `om_write` / `om_read` / `om_expect_record` | as for the park ops; `om_expect_record` also takes `u32s` (`[{offset, values}]`, little-endian) | the record (§13) |
| `om_expect_events` / `om_expect_event_count` / `om_clear_events` | see the shared conventions | host calls |

`om_call` methods (filters: `filter` null = all, a token = a caller filter, `{"type": n}` = the shared one-type
filter; the typed overloads take `type`):

- conversions (`args`): `position_to_world_cell`, `position_to_manager_cell`, `position_to_standard_cell` (two
  floats); `world_to_manager_cell`, `manager_to_world_cell`, `standard_to_world_cell`, `world_cell_to_position`,
  `manager_cell_to_position`, `standard_cell_to_position`, `bounding_cells` (two ints); `world_to_standard_cell`
  (x, z, the old z output); `query_interface` (IID → view offset 0 / 4 / 8 or null); `shrink_bits` (min, max
  floats: §5.1 step 5 on one axis, the two bounds as `float` bit patterns);
- lifecycle: `set_city_size` (`args`), `init`, `shutdown`, `add_ref`, `release`, `class_id`, `cell_sizes`,
  `world_cell_count`, `cell_count`, `cell_bounds`;
- `insert` (`occupant`, `data4`), `remove` (`occupant`, `shutdown`, `data4`), `remove_occupants` (`filter`,
  `shutdown`, `data4`), `remove_occupants_typed` (`type`, …), `move` (`occupant`, `post`, default true);
- `is_cell_empty` (`args`), `find_occupant` (`occupant`, `search_all`, `xz` (the outputs' values before the call),
  `expect_xz`);
- first-occupant queries (result: token or null): `first_by_position` (`args` floats), `first_in_cell`,
  `first_by_standard_cell` (`args` ints), `first_by_standard_cells` (`min?`, `max?`), each with `_typed`;
- list queries (`expect_list`, `list` = elements already in the list, `max_count` default −1): `by_bbox` (`min?`,
  `max?` 3-vectors), `by_cells`, `by_standard_cells` (`min?`, `max?` pairs), each with `_typed`;
- iteration (each call records `callback` (`occupant`, `depth`); `stop_at` = occupants for which the callback
  returns false; `actions` = `{"occupant": [om_call bodies]}` run inside that callback, failures recorded as
  `action_error`): `iterate_by_bbox` (`min`, `max`), `iterate` (`min?`, `max?` 3-int vectors),
  `iterate_by_standard_cells` (`min?`, `max?`), each with `_typed`.

Occupant manager events: `post` (`type`, `occupant`, `data2` (flag 4), `data3` (the type), `data4`, `sync`,
`inserted` (flag 1 when posted)), `occupant_slot17`, `callback`, `action_error`, and with `om_trace`: `lock` /
`unlock` (`depth`), `add_ref` / `release` (`occupant`).

## Terrain ops

The terrain vectors (`tr18_…` for the §18 tests, `tr00_…` for the rest) drive `fc4_core::terrain`
(`docs/specs/terrain.md`): the height field `Terrain`, the brushes, the surface-water queries and bodies of water
(`BodiesOfWater`, `NavigationHost`) and the volcano lava (`Lava`, `LavaHost`). Every op name starts with `ter_`.
The terrain's posts are drained into the event log after each op. The default terrain: 2 × 2 cells of 16 m, flat at
300 m, sea level 250 m (each vertex's depth starts at sea − altitude), the shipped tuning, view-ready (as if
`DoPostCityInit` had run). The default world: no lots, bridges or power poles, every cell flammable, and the
random draw returns 0 unless queued.

### Setup

| op | Arguments | Effect |
|---|---|---|
| `ter_create` | `cells?` [x, z], `altitudes?` (row-major, z outer, `(x+1)(z+1)` values; `"nan"` allowed) or `flat?`, `sea?`, `tuning?` (`"shipped"` or a partial `TerrainTuning`: `max_height`, `min_height`, `new_sea_level`, `new_flat_height`, `image_height_scale`, `sea_level_min`, `sea_level_max`, `cliff_slope`, `saved_78`), `view_ready?` (true), `new?` (false; true: `Create` from the tuning instead of the altitude array) | a new terrain, replacing the current one |
| `ter_env` | any of `lots` ([[x, z, lot]]), `bridges` ([[x, z, deck or null]]: null is a bridge whose deck is unknown), `poles` ([[x, z]]), `not_flammable` ([[x, z]]), `burn_cells` (the queue `next_burn_cell` returns), `random` (queued draws) | the world |
| `ter_set` | `vertex` [x, z] or `vertices` [x0, z0, x1, z1] (inclusive) with any of `altitude`, `depth`, `flags`; or `cell` [x, z] with `depths` (the corners (x, z), (x+1, z), (x, z+1), (x+1, z+1)); or `cached` [min, max] | raw state, no messages, no depth bookkeeping |
| `ter_add_brush` | `brush` (ID) and either `bitmap` (`{width, height, fill}` or `{width, height, pixels}`; `brush` one of the shipped `0x11`, `0x12`, `0x14`) or `components` (`[{kind, radius, strength, oriented?, neutral?, bitmap?}]`, `radius` / `strength` one number or five per-level values, `oriented` false, `neutral` 0) with `step?` (the random-walk step in radians, 0); `walk_angle?` (`angle?` is the same), `stroke_angle?`, `expect?` | registers a description: a shipped one (one oriented kind-0 component, the 0.5° default walk step, so every in-bounds `ApplyBrush` logs one `random` n = 5) or a custom one |

### Calls and checks

| op | Arguments | Effect / check |
|---|---|---|
| `ter_call` | `method`, `args?`, `expect?`, `expect_bits?` (the f32's bits) | the queries `altitude`, `altitude_at_index`, `altitude_at_vertex`, `altitude_at_nearest_grid`, `altitude_checksum`, `cell_corner_altitudes`, `location_is_in_bounds`, `face_normal` ([x, y, z] or null when untouched), `min_height`, `max_height`, `min_allowed_height`, `max_allowed_height`, `vert_index`, `sea_level`, `water_level`, `water_depth_at_vertex`, `water_depth_at_index`, `max_surface_level`, `cell_has_water`, `cell_has_land`, `is_land_vertex`, `is_water_vertex`, `is_sea_vertex`, `water_depth_for_cell`, `has_draft_clearance`, `water_type`, `is_navigable` ([x, z, r, lot]), `is_triangulation_flipped`, `cell_flags`, `terrain_type`; the setters `raise_vertex`, `lower_vertex`, `set_altitude_at_vertex`, `set_altitude`, `set_altitude_rect`, `lock_altitude` ([x0, z0, x1, z1, lock]), `set_sea_level`, `set_terrain_type`; the messages `redisplay` ([terrain, water, x0, z0, x1, z1, data4]), `redisplay_cells` (slot 69, the same arguments as cell corners), `begin_long_terrain_operation`, `end_long_terrain_operation`, `post_city_init`; the bodies of water `update_bodies`, `body` ([x, z] → the ID, 0 for none), `bodies_map`; round 2: `lerped_water_level` ([x, z] metres), `flooding_altitude` ([x, z]), `water_velocity` ([x, z] → [3]), `sediment_at` ([x, z] metres), `water_display_state`, `set_water_display_state` ([state] → whether it posted), `default_brush_level`, `neighbor_mask` ([x, z]), `talus_angle`, `set_talus_angle` ([radians]), `brush_angles` ([id] → [walk, stroke] or null), `set_brush_angles` ([id, walk, stroke]), `brush_corners` ([cx, cz, w, h, cos, sin] → four [x, z]), `spans` (the four corners as [x0, z0, … x3, z3] → `ComputeSpansForRectangle`'s [[z, start, end]]) |
| `ter_apply_brush` | `brush`, `from` [x, z], `to?`, `level?`, `redisplay?`, `strength?` (1), `radius?` (1), `data4?`, `target?`, `expect?`, `out?` ([x0, z0, x1, z1] or null: empty) | `ApplyBrush` with a `BrushParams` |
| `ter_expect` | `state` (partial: `cells`, `sea_level`, `min_height`, `max_height` (the cache), `view_ready`, `terrain_type`, `saved_78`, `posts_pending`) | the terrain object |
| `ter_expect_vertices` | `vertex` [x, z] with any of `altitude`, `depth`, `flags`; or `rect?` (the whole terrain) with any of `min`, `max`, `baseline` + `changed_within` (the only vertices that differ from `baseline`) and `changed` (how many) | the height field |
| `ter_lava_start` | `at` [x, z] (metres), `radius?` (20), `expect?` (`{source, active, burn}` rectangles) | a `Lava` over the terrain |
| `ter_lava_set` | `vertex` [x, z] or `around` [x, z] (the eight neighbours) with any of `level`, `production`, `solidification` | lava state |
| `ter_lava_call` | `method`, `args?`, `expect?` | `flow_amount` and `transfer` ([x, z, dt] → `{amount, to, mode}`; `mode` is `none`, `ignite` or `burn`), `level` ([x, z]), `flow` ([dt], one `DoLavaFlow`), `burn`, `rects`, `lowest_neighbour` ([x, z] → [x, z] or null), `set_things_on_fire` ([x, z, old, amount] → the mode) |
| `ter_record` | `sizes?` (`{misc, altitudes, properties, edges: [4]}`), `round_trip?` (write, load into a fresh terrain, compare every field), `sea_after_load?` (write, load, the loaded sea level), `edge?` (`{side, count, values?}`) | the §13.1 records |
| `ter_expect_events` / `ter_expect_event_count` / `ter_clear_events` | see the shared conventions | host calls |

Terrain events: `post` (`msg`, `data1`..`data4`), `ignite` (`x`, `z`), `quick_burn` (`x`, `z`), `cause`
(`cause`) and `random` (`n`). Floats are numbers or `"nan"`.

### Nation

| op | Arguments | Check |
|---|---|---|
| `nat_economy` | `difficulty?` (three modifiers), `curves` (`{"id": [x0, y0, x1, y1, …]}`), `checks` ([[demand ID, months, city difficulty or null (no city), expected]]) | `GetEconomyModifier` (`fc4_core::demand::NationEconomy`, nation §5) |

## Advisor ops

The advisor vectors (`ad17_…`, `ad04_…`, `ad06_…`) drive `fc4_core::advisors::AdvisorSystem` (`docs/specs/advisors.md`)
over the core's scripted host (`fc4_core::advisors::scripted::ScriptedHost`). Every op name starts with `adv_`. Lua is
replaced by literal trigger texts: a trigger evaluates its text through `adv_trigger`, else as a literal (`true`,
`false`, `nil`, a number, `0/0`, `"str"`, `error`). The default world: a city that exists and is established,
difficulty 0, 64 × 64 cells, day 0, start year 2000, not paused, no debug, an empty `advisors` table, no advices, no
sources (every read fails), tags (1, 2). Detokenizing prefixes `detok:`. A host object reference has class
`0x12345678` and its token as the object.

Object specs (`object`, `advisor`, `advice`, `target`, …): `"system"`, `{"advisor": id}`, `{"advice": guid,
"advisor"?: id}` (the first advisor holding it), `{"mysim": index}` (with the MySim ops) or a name given by
`adv_subject`, `adv_create_object`, `mysim_read` or a record read (`ref<id>`).
## Effects ops

The effects vectors (`ef17_…` for the §17 tests) drive `fc4_core::effects` (`docs/specs/effects.md`): the effects
manager (`EffectsManager`), the brush effect, the game effect and the effect directory. Every op name starts with
`ef_`. The host logs every call it receives. The default host: a simulator with dilation 1, a city of 64 × 64 cells
(16 m), render components inactive.

### Setup

| op | Arguments | Effect |
|---|---|---|
| `adv_env` | any of `has_city`, `established`, `difficulty` (null: unreadable), `cells` ([x, z]), `day` or `date` ([y, m, d]), `start_year`, `paused`, `now_ms`, `debug`, `popup` (`{low, medium, high}` or null), `tutorial`, `advisors_table` (false: not a table), `tags` | the world |
| `adv_advisor` | `class` (name or ID), `id`, `caption?` (`Planning\|tail`), `name?` (`Bob`), `advice_type?` (1) | appends an `advisors` entry |
| `adv_advice` | `guid`, `class?` (`Advice`), `type?` (1), `priority?` (50), `mood?`, `frequency?` (35), `timeout?` (45), `trigger?` (text, default `true`; null: none), `once`, `news_only`, `event`, `command`, `persist`, `no_timeout`, `effects`, `nd_type`, `replace?` (empties the table first) | appends an `advices` entry |
| `adv_trigger` | `text`, `result` (`true`/`false`, a number or `"nan"`, `"nil"`, `"error"`, `{"str": s}`) or `bad: true` (does not compile) | a trigger's result |
| `adv_condition` | `index`, `result` | a reward condition's result |
| `adv_value` | `src` (a `SrcKind` name), `a?`, `b?`, `object?`, `value` (`{i, f, cell, object, object2, n, m}`; null: unreadable) | a subsystem value |
| `adv_list` | `kind` (`hospitals`, `schools`, `fire_stations`, `police_stations`, `port_lots`, `connections`, `deal_proposals` or a code), `a?`, `groups?`, `items` (`[{object, f0, f1, x, z, flags, kind}]`; null: unreadable) | a list read |
| `adv_occupant` | `occupant`, `type?` (building), `conditional?`, `key_instance?`, `groups?` | an occupant |
| `adv_reward_subject` / `adv_status` / `adv_deal` / `adv_history` | `guid` + subject fields / `guid`, `status`, `count?`, `tooltip?` / `object`, `type` / `type`, `value`, `slope`, `count` | reward subjects, conditional-building statuses, deals, history |
| `adv_create` | `session_seed?` | a new system over the same host (not initialised) |
| `adv_subject` | `name`, `clone_of?`, `location?` (through `SetLocation`), `object?` (null: none), `data1?`, `data2?` | creates or changes a named subject |
| `adv_set` | `object` + advisor fields (`month_counter`, `current_day`, `cursor`, `dirty`, `pending`, `building_count`, `park_count`, `recreation_count`, `landmarks`, `name`, `queue: []`, `info: {guid, next_day?, expiry?, last_month?}`), advice fields (`nd_type`, `guid`, `advisor`) or `reload_pending` (system) | pokes the state |

### Calls and checks

| op | Arguments | Effect / check |
|---|---|---|
| `adv_init` / `adv_shutdown` / `adv_reload_data` / `adv_ticker` | `expect?` | the system's slots; the ticker handler |
| `adv_message` | `target?` (system), `type` (a name or ID), `data1`–`data4`, `void1`, `unknown`, `expect?` | `DoMessage` |
| `adv_day` | `day` or `date`, `advisor?` | sets the date, then the advisor's `Simulate(0)` |
| `adv_simulate` / `adv_update_variables` | `advisor`, `kind` / `advisor` | `Simulate(kind)` / `UpdateVariables` |
| `adv_push_event` | `event`, `subject?`, `advisor?` (its queue; default: the system) | `PushEvent` |
| `adv_flags` / `adv_get_trigger` / `adv_retire` / `adv_command_would_trigger` | `advice`, `mask`, `on` / `advice` / `advisor`, `advice?` / `advisor`, `command`; `expect?` | the advice and advisor slots |
| `adv_context` | `push` (an advice) or `pop` (`expect`: the GUID or null) | the context-advice stack |
| `adv_lua` | `fn`, `args` (numbers, `"0x…"`, null, booleans, `{"str"}`, `{"subject": name}`), `expect` (the results; null: no such function), `expect_bits?` | a Lua function |
| `adv_rng` | `mul?`, `lcg?`, `expect_mul?`, `expect_range?` (`range_f64(0, 100)` as float32) | the system generator |
| `adv_batch_size` / `adv_popup` | `n`, `expect` / `init?`, `expect` | the §6.2 batch size / the popup threshold |
| `adv_add_advisor` / `adv_advisor_init` / `adv_create_object` | `object` / `object`, `table_index` / `class`, `as`; `expect?` | the system's factory and advisor slots |
| `adv_write` / `adv_read` / `adv_expect_record` | `object`, `name` / `object`, `hex` or `record` / `name`, `hex` | one record (references numbered from 1; read references: core classes from the factory, others host objects with token = ID) |
| `adv_save` / `adv_load` / `adv_expect_entry` | `name` / `name` (into the current system) / `save`, `class`, `entry?`, `hex` | the class-object store round trip |
| `adv_expect` | `object`, `state` (partial; see below) or `absent: true` | an object's state |
| `adv_expect_globals` / `adv_expect_local` | `values` (`{name: value or null}`) / `name`, `value`, `subject` (partial) or `absent` | the Lua variables (a local reads the host log since the last event check) |
| `adv_expect_status` / `adv_expect_refs` / `adv_expect_history_days` | `guid`, `state` / `refs` / `days` | statuses, host references, history reads |
| `adv_expect_events` / `adv_expect_event_count` / `adv_clear_events` | see the shared conventions | host calls (both hosts when the MySim ops run) |

State fields. System: `advisors`, `initialized`, `loaded_from_save`, `reload_pending`, `thresholds`, `tags`,
`last_day`, `idle_start`, `context`, `dolly_llama`, `popup_threshold`. Advisor: `id`, `class`, `kind`, `caption`,
`name`, `advice_type`, `table_index`, `month_counter`, `current_day`, `cursor`, `advices`, `active` (GUIDs), `events`
(`{"0xEVENT": [indices]}`), `queue` (`[[event, subject]]`), `dirty`, `pending`, the subscription flags, the
CityPlanning counters and `landmarks`, the Safety flags. Advice: `guid`, `class`, `kind`, `flags`, `active`,
`advisor`, `table_index`, `priority`, `frequency`, `timeout`, `mood`, `event`, `command`, `effects`,
`trigger_source`, `has_trigger`, `loaded`, `nd_type`, `subject`, and from its advisor `next_day`, `expiry`,
`last_month`. Subject: `location`, `object`, `data1`, `data2`, `has_location`, `x`, `z`.

Advisor events: `post` / `send` (`type`, `data1`, `object`: `none`, `advice:0xGUID`, `advisor:ID` or `host:0x…`),
`update_self` (`advisor`, `data1`, `data2`), `subscribe` / `unsubscribe` (`target`, `type`), `global` / `local`
(`name`, `value`, `subject`), `global_string`, `effect` (`id`, `assert`), `rebuild` (`guid`), `history`, `status`,
`cheat` (`id`, `on`), `window` (`start`), `ticker`, `collect_garbage`, `lua` (`call`), `cancel_deal`,
`set_ordinance`.

## MySim ops

The MySim vectors (`ms12_…`) drive `fc4_core::mysims` (`docs/specs/mysims.md`): the agent simulator and the MySims,
which are advisors of the advisor world above (so the `adv_` ops work on them too). Every op name starts with
`mysim_`. `mysim_create` builds the shipped advisors table (a plain advisor 23, MySims 1–5, the My Street sim 30),
a new system with the MySim classes registered, `Init`, and the agent's `Init`, over the core's scripted MySim host
(`fc4_core::mysims::scripted::ScriptedMySimHost`): 64 × 64 cells, cell centres at 16 × c + 8 m, every read failing
until set. `sim` is a vector index (0–5) or a name from `mysim_read`.

### Setup

| op | Arguments | Effect |
|---|---|---|
| `mysim_create` | `table?` (`"shipped"` or `[{class, id}]`), `session_seed?`, `init?` (true) | the world (logs cleared) |
| `mysim_env` | any of `cells`, `ui_blocked`, `disaster_ongoing`, `no_lua`, `no_query`, `change_job`, `no_trip_simulators`, `eq`, `hq`, `travel_ratio`, `commute`, `life_expectancy`, `new_age` (null: unreadable), `disaster_locations` (`[[type, [x, z]]]`), `tuning`, `average_age` (`[[x, z, age]]`), `grids` (`[[grid, x, z, v]]`; grids `crime`, `air`, `water`, `garbage`, `radiation`, `aura`, `average_eq`, `average_hq`, `congestion`, `desirability:0x…`), `grids_at`, `coverage` (`{police, fire}`), `powered` / `watered` (`{token: bool}`), `stations` (`{police, fire: [[token, funding]]}`), `find_job` (queued answers), `job_destination`, `neighbor_names` (`[[x, z, name]]`), `special` / `schools` / `hospitals` (tokens), `grades` / `landmark_funding` (`{token: f}`) | the host's answers |
| `mysim_building` | `token`, `type?` (0x10), `name?` (`House`), `groups?` ([0x1000]), `completion?` (1), `purpose?` (1), `position`, `lot?` (`{cell, rect?, condition? 1, road_access? true, wealth? 2, building?}`) or `remove` | a building and its lot |
| `mysim_position` / `mysim_rect_lot` / `mysim_dispatch` | `occupant`, `position` / a lot / `id` + dispatch fields | a fire's position / a lot of the trip square / a MySim's dispatch |
| `mysim_set` | `sim` + MySim fields (state, slot, wealth, age, keys, rings, pylon, references, …), `month_counter`, `queue: []` | pokes the state |
| `mysim_reseed` | `sim`, `seed` | the MySim's generator restarts (clock seeds 1) |
| `ef_create` | `directory?` (`"volcano"`, the default: `shipped::volcano_directory()`; or inline `{games?, effects: [{name, priority? (1), flags?, components: [{type, index?, name?, flags?, lod? [min, max], group?, weight?}]}]}`, with the shipped brush records and, unless `games` is given, the shipped game records), `seed?` (the session seed), `max_priority?`, `finish_mode?` (`hook` (the default) or `draw_pass`), `nested_update?` (true) | a new manager, replacing the current one; labels are cleared |
| `ef_env` | any of `dilation` (a number, or null: no simulator), `cells` [x, z], `render_active` | the host |
| `ef_effect` | `name`, `as?` (a label), `expect?` (whether an effect was created), `expect_index?` (the description chosen; -1: none qualifies) | `CreateVisualEffect` |

Component `type`: `particles`, `decal`, `meta`, `brush`, `attractor`, `game`, `sound` or a number. Game record
fields (inline): `flags`, `demolish`, `demolish_flags`, `min_size`, `max_size`, `burn`, `message`, `message_data`,
`duration`, `toxic`, `extinguish`.

### Calls and checks

| op | Arguments | Effect / check |
|---|---|---|
| `mysim_call` | `sim`, `method` (`move_in`, `set_residence`, `change_residence`, `find_new_residence`, `test_for_residence`, `change_job`, `find_new_job`, `job_found`, `check_commute`, `fail_commute`, `quit_job`, `neighbor_connection_removed`, `move_out`, `leave_city`, `delete_from_game`, `announce_move_in`, `update_variables`, `update_quick_variables`, `update_local_buildings`, `do_script_command`, `distance_to_closest`, `distance_to_closest_special`, `local_grade`, `trip_area`, `update_trip_variables`, `disaster`, `building_added`, `building_removed`, `fire_added`, `pipe_burst`, `tornado`, `command_would_trigger`, `init`) with named arguments, `expect?` | a MySim slot |
| `mysim_simulate` | `sim`, `kind`, `day?`, `month_counter?` (before the step) | `Simulate(kind)` |
| `mysim_message` | `sim?` (default: the agent), the message fields | `DoMessage` |
| `mysim_agent` | `method` (`init`, `shutdown`, `count`, `active`, `move_in`, `lives_here`, `set_active_ui_panel_sim`, `get_active_ui_panel_sim`, `write`, `read`), `expect?` | the agent's slots |
| `mysim_expect` / `mysim_expect_subject` / `mysim_expect_fields` / `mysim_expect_agent` | `sim`, `state` / `sim`, `event`, `state` / `sim`, `fields` / `state` | the MySim state (with `id`, `class`, `personal_name`, `month_counter`, `events` (queued IDs), `lcg`), a queued subject, the Lua fields, the agent |
| `mysim_write` / `mysim_read` / `mysim_compose` / `mysim_expect_record` | `sim`, `name`, `advisor_part?` / `sim` or `new` + `as`, `hex` or `record`, `patch?` ([offset, hex]), `token_base?` (0x1000) / `name`, `parts` / `name`, `hex`, `then?`, `prefix?` | records (read references resolve to host tokens base + ID) |
| `mysim_expect_refs` / `mysim_run` | `refs` / `months`, `days?` (28), `start_day` | references held / day and month passes of MySims 0–4 |

MySim events: `create_table`, `activate` (`sim`), `agent_lua` (`on`), `mysim_post` (`type`, `data1`, `data2`,
`void1`, `sim`), `dispatch` (`sim`, `what`), `trigger` (`name`, `position`), `mysim_subscribe` (`type`, `on`).
## Network geometry ops

The network geometry vectors (`ng18_…`, the §18 tests of `docs/specs/network-geometry.md`) drive
`fc4_core::network_geometry`: a `NetworkGeometry` (the run, the passes, the cell infos, the two satisfiers) over a
`fc4_core::terrain::Terrain` and a scripted host. Every op name starts with `ng_`. The terrain's posts are drained into
the event log after each op. The default city: 64 × 64 cells, flat at 300 m, sea level 250 m, view-ready, the shipped
placement parameters (§2.4). The default host: no occupants, no existing pieces, no connection nodes, no bridge
types.

| op | Arguments | Effect / check |
|---|---|---|
| `ng_city` | `cells?` [x, z] (64, 64), `altitude?` (300), `sea?` (250), `columns?` [[x0, x1, alt]] (vertex columns), `slope_x?` (altitude := `altitude` + `slope_x` × x), `vertices?` [[x, z, alt]], `view_ready?` (true) | a new terrain |
| `ng_types` | `type`, `params` (partial: `max_slope`, `max_slope_change`, `max_raise`, `max_cut`, `resolve_tolerance`, `tunnel_trigger_slope`, `tunnel_end_slope`, `new_cell_tolerance`, `support_threshold`, `max_support_height`, `tunnel_clearance`) | one type's placement parameters over the shipped ones |
| `ng_host` | any of `landfill` [[x, z]], `immovable` [[x, z, node, type]] (immovable network occupants), `nodes` [[x, z]] (connection-node cell infos), `existing` [{`cell`, `corners` [4], `immovable?`, `flags?` (1), `supports?`}], `extend` [[x, z]] (§3.3 step 3), `bridge_types` (count), `bridge_min_span`, `highlight_cut` (slot 59's new count) | the host |
| `ng_segments` | `start`, `end`, `snap?`, `straight_only?`, `widths?` [w1, w2]; `state` (partial: `end`, `dragged_count`, `straight_count`, `axis`, `pen_direction`, `end_direction`) | `BreakIntoStraightAndDiagSegments` |
| `ng_draw` | `type?` (0), `start`, `end`, `snap?`, `expect?` | `Reset` then `DrawNetworkLine` |
| `ng_validate` | `expect?` (the valid count) | the passes of §4 |
| `ng_cell_infos` | | the cell infos of the valid steps |
| `ng_smoothen` | `level`, `expect?` | `SmoothenNetwork(level)` |
| `ng_support` | `expect?` | slot 56 on the last smoothing |
| `ng_place` | `expect?` (the valid count) | the passes, the cell infos and the smoothing loop |
| `ng_expect` | `state` (partial: `valid`, `failure`, `level` (null = none), `end`, `dragged_count`, `straight_count`, `pen_direction`, `end_direction`, `cells`, `kinds`, `tunnels` [[start, end]], `bridges` [[start, end]], `tunnel_searches`, `heights` / `terrain_heights` (`{"vertex": h}`, null = unsolved), `smoothing_cells`, `boundary`) | the geometry |
| `ng_type_info` | `type`; `state` (partial: `slope_rise`, `diagonal_slope_rise`, `smoothness`, `diagonal_smoothness`, `tunnel_trigger_rise`, `widths`, `straight_only`) | the shipped type info (§2.3) |
| `ng_sat` | `sat?` (`network` or `terrain`), `call`, `args?`, `expect?` | the satisfier: `init` [lo, hi] (`"max"` / `"-max"` for ±FLT_MAX), `shutdown`, `add` [v, h] (→ new), `fixed` [v, h], `range` [v, a, b], `equality` [a, b], `slope` [a, b, s], `smoothness` [a, b, c, s], `preferred` [v, h], `preferred_range` [v, a, b], `solve` (→ bool), `height` [v], `range_of` [v], `solution` (→ [[v, h]] in solution order) |
| `ng_terrain_vertex` | `n`, `t`, `supports`, `fixed?`, `type?`; `expect` `{fix}`, `{range, preferred}` or `"unsupported"` | §9.6 step 3 |
| `ng_support_level` | `corners`, `terrain`, `threshold`; `expect` `{level, flags}` | §10.2 |
| `ng_occupant_flags` | `flags`, `real`, `tunnel?`; `expect` | §10.1 |
| `ng_cost` | `demolition?`; `expect` | `GetCostOfSolution` of the placed run |
| `ng_power_line_cost` | `poles`, `wire`, `per_pole?` (15), `per_wire?` (2.0); `expect` | §11 |
| `ng_destroy` | `destroyed`, `pieces`, `replaced` ([[x, z]] each); `expect` `{removals, resolve}` | §13 |
| `ng_grade` | `expect?` `{changed, rect}` | slot 57 on the terrain satisfier |
| `ng_expect_events`, `ng_clear_events` | as elsewhere | the terrain's posts |

## Effects ops

The effects vectors (`ef17_…` for the §17 tests) drive `fc4_core::effects` (`docs/specs/effects.md`): the effects
manager (`EffectsManager`), the brush effect, the game effect and the effect directory. Every op name starts with
`ef_`. The host logs every call it receives. The default host: a simulator with dilation 1, a city of 64 × 64 cells
(16 m), render components inactive.

### Setup

| op | Arguments | Effect |
|---|---|---|
| `ef_create` | `directory?` (`"volcano"`, the default: `shipped::volcano_directory()`; or inline `{games?, effects: [{name, priority? (1), flags?, components: [{type, index?, name?, flags?, lod? [min, max], group?, weight?}]}]}`, with the shipped brush records and, unless `games` is given, the shipped game records), `seed?` (the session seed), `max_priority?`, `finish_mode?` (`hook` (the default) or `draw_pass`), `nested_update?` (true) | a new manager, replacing the current one; labels are cleared |
| `ef_env` | any of `dilation` (a number, or null: no simulator), `cells` [x, z], `render_active` | the host |
| `ef_effect` | `name`, `as?` (a label), `expect?` (whether an effect was created), `expect_index?` (the description chosen; -1: none qualifies) | `CreateVisualEffect` |

Component `type`: `particles`, `decal`, `meta`, `brush`, `attractor`, `game`, `sound` or a number. Game record
fields (inline): `flags`, `demolish`, `demolish_flags`, `min_size`, `max_size`, `burn`, `message`, `message_data`,
`duration`, `toxic`, `extinguish`.

### Calls and checks

| op | Arguments | Effect / check |
|---|---|---|
| `ef_call` | `method`, `effect?` (a label), `args?`, `repeat?`, `expect?` (the last result) | the manager: `tick` ([us]), `effects_tick` (DoEffectsTick §6.1: the activation of the new list; returns how many were activated), `frame` ([us]: the tick, then the finishing hook, then the draw pass), `finish_hook`, `draw_pass` (whether the sweep ran), `shutdown`, `set_zoom` ([zoom, offset]), `set_max_priority`, `has` ([name]); an effect: `start` / `stop` ([mode?], 0 by default), `set_position` ([x, y, z]), `set_end_message` ([type, data1]), `forward_parameter` ([index, value]), `add_ref`, `release`, `is_started`, `is_stopping` |
| `ef_run_stroke` | `effect`, `us`, `max?` (10000), `ticks?`, `applications?`, `first?` (the brush calls of the first ticks), `every?` (the brush calls of every tick) | ticks until the effect's brush is inactive |
| `ef_child` | `effect`, `n?` (0), `name?`, `as?` | the child of the effect's `n`-th built meta component |
| `ef_expect` | `effect`, `state` (partial: `exists`, `name`, `index`, `refs`, `started`, `stopping`, `components` (built), `active`) | an effect |
| `ef_expect_brush` | `effect`, `n?`, `state` (partial: `brush`, `rate`, `length`, `one_shot`, `strength`, `radius`, `active`, `long_op`, `acc`, `t`, `prev`, `cur`, `applications`) | the effect's `n`-th brush component |
| `ef_expect_game` | `effect`, `n?`, `state` (partial: `started`, `pending`, `timer`, `count`, `cell`, `data`, `applies`) | the effect's `n`-th game component |
| `ef_expect_manager` | `state` (partial: `select_draws`, `end_posts`, `recycled`, `effect_count`, `active_count`, `total_us`, `lod`, `max_priority`) | the manager |
| `ef_select` | `first_word?`, `draws?`, `weights` + `members?` (1-based) | a fresh select generator (seed `0x1105`): its first word, its first `RandomUint32Uniform(0x10000)` draws and the members they pick |
| `ef_dilation` | `paused?`, `emergency_paused?`, `driving?`, `hidden_paused?`, `speed?` (0), `expect` (the dilation), `us?` + `expect_us?` (the dilated step) | `effects::dilation`: the shipped table (§6, test 17) |
| `ef_particles` | `flags?` (0: duration, 2: loop budget, 4: plain), `emit_loop_count?` (1), `loop_interval?` (1.0), `per_loop?` (1), `life?` (1.0), `duration?` (1.0), `dt?` (1/30), `max?` (10000), `stop_after?` + `stop_mode?` (0) (a Stop before that update number + 1), `expect_updates` (the update on which it went inactive, or null) | `effects::particles::ParticleSimulator` from `Start` (§7.1, test 16) |
| `ef_expect_events` / `ef_expect_event_count` / `ef_clear_events` | see the shared conventions | host calls |

Effects events: `brush` (`brush`, `from`, `to`, `level`, `strength`, `radius`, `redisplay`, `data4`, `target`),
`begin_long`, `end_long`, `ignite` (`x`, `z`, `r0`, `r1`), `cause` (`cause`), `toxic` (`x`, `z`), `demolish`
(`rect` [x0, z0, x1, z1], `flags`, `dezone`, `no_networks`, `no_flora`, `min_size`, `max_size`), `extinguish`
(`x`, `z`, `amount`), `post` (`type`, `data1`, `data2`, `data3`), `pause`, `resume`, `hidden_pause`,
`hidden_resume` and `clock` (`on`).

## UI window ops

The UI window vectors (`ui11_…`) drive `fc4_core::ui_windows` (`docs/specs/ui-windows.md`): the window object,
`cSC4View3DWin`, its catalog menus and the dialog close path. Every op name starts with `uiw_`. One vector per
conformance test of §11, named `ui11_NN_<name>` where `NN` is the test number.

`uiw_create` builds the view at a fixed object address `0x2000`, so its `cIGZWin` sub-object's token is always
`0x200C` — `AsIGZWin()` is `this + 0x0C` and that *is* the token. Pre-seeded catalog menus use the fixed tokens
`0x20` (secondary) and `0x30` (tertiary); a window the script factory creates is `0x9001`, `0x9002`, … in call order.

### Setup and state

| op | Arguments | Effect |
|---|---|---|
| `uiw_create` | | the view, its window store and the hidden-pause counter (logs cleared) |
| `uiw_env` | any of `create_ok` (the script factory answers), `catalog_iface_ok` (`QueryInterface(0x8A2CAD8B)` answers), `slot17`, `slot9` | the host's answers |
| `uiw_seed_secondary` / `uiw_seed_tertiary` | `kind` | a catalog menu already open at `+0x1F8` / `+0x1FC` |
| `uiw_set_catalog_state` | `present?` (true), `field0?`, `field1?` | sets or clears `+0x208` |
| `uiw_set_tool` | `tool` (0 clears) | the current view input control |
| `uiw_expect` | `state` (partial) | `this_addr`, `win`, `as_igzwin`, `win_offset`, `secondary_win`, `tertiary_win`, `secondary_kind`, `tertiary_kind`, `catalog_state_ptr`, `catalog_field0`, `catalog_field1`, `view_manager`, `input_control`, `refs`, `hidden_pause_count`, `hidden_paused`, `paused`, `painted` |
| `uiw_layout` | `state` (partial) | `as_igzwin_null`, `as_igzwin`, `win_offset`, `vtable_primary`, `vtable_window`, `slot_count`, `catalog_state_offset`, and `slot<N>_ret` / `slot<N>_produces_value` for every `cISC4View3DWin` slot |

### Calls

| op | Arguments | Effect |
|---|---|---|
| `uiw_close_secondary` / `uiw_close_tertiary` | | slots 59 / 60. Secondary cascades to slot 60 first |
| `uiw_invoke_secondary` | `instance`, `kind`, `caller_data?`, `target_win_id?` (1), `expect?` (`toggled`, `replaced_then_failed`, `opened`) | `InvokeSecondaryMenu` (4 arguments) |
| `uiw_invoke_tertiary` | the same + `margin?` | `InvokeTertiaryMenu` (5 arguments; `margin` is arg5, the −25 / −75 the secondary form lacks) |
| `uiw_deliver` | `msg`, `data1?`, `data2?`, `data3?`, `expect?` (the stage list) | the §6.5 chain, returning the stages reached |
| `uiw_popup` | `advice_flags?` (2), `msg`, `data1?`, `data2?`, `data3?`, `expect?` | the advisor popup's win-proc decision |
| `uiw_popup_instance` | `mood?`, `expect?` | which UI script instance an advisor popup is built from |
| `uiw_pause_enter` / `uiw_pause_leave` | `is_dialog?` (true) | one level of the hidden pause |
| `uiw_paint` / `uiw_sweep_effects` | | the view paints / the effects sweep runs |
| `uiw_save` / `uiw_load` | `state` / the record fields, `expect?` | `DoSave` / `DoLoad` of the View3D record |

`msg` is one of `command`, `link`, `attach`, `detach`, `modal_begin`, `modal_end`, `modal_end_posted`, `key_down`,
`key_up`. For `command`, `data1` is forced to `0x287259F6` and `data2` is the button id.

### Checks

| op | Arguments | Effect / check |
|---|---|---|
| `uiw_expect_ops` | `ops`, `only?` (an event-kind filter) | the window ops taken since `uiw_clear_ops`, one to one and **in order** |
| `uiw_expect_ops_contain` / `uiw_expect_ops_absent` | `ops`, `only?` | every listed op present / none present |
| `uiw_expect_last_align` | `state` | the one `AlignWindowToWindow` call |
| `uiw_expect_events` / `uiw_expect_event_count` / `uiw_clear_events` | | the host callbacks taken |

`uiw_expect_ops`'s events are `child_add`, `child_remove`, `child_delete`, `destroy_window`, `hide_window`,
`show_window`, `pull_to_front`, `add_message_filter`, `remove_message_filter`, `remove_all_view_input_controls`,
`create_window`, `window_created`, `align_window_to_window`, `set_win_gen_flag`, `query_interface`, `query_win_gen`,
`release`, `do_message`, `do_win_proc_message`, `gz_on_command`, `is_window_valid`, `catalog_state_write`, `get_id`.
Ordering is the point of several tests: `only` restricts the comparison to the named kinds so a vector can pin an
order across kinds without listing every intermediate step.


## Network bridge ops

The network bridge vectors (`nbr17_...`, the section 17 tests of `docs/specs/network-bridges.md`)
drive `fc4_core::network_bridge`: a `BridgeTool` (the bridge types, the water and land passes,
the elevations, the two models) over a flat city and a scripted bridge host. Every op name
starts with `nbr_`. The default city: 64 x 64 cells, flat at 300 m, sea level 250 m, no wet
cells. The default host: nothing in the way. The default tool: the shipped road bridge types
(section 2.6).

| op | Arguments | Effect / check |
|---|---|---|
| `nbr_city` | `cells?` [x, z], `altitude?` (300), `sea?` (250), `wet_columns?` [[x0, x1, z]] (wet cells), `heights?` [[x, z, h]] (a per-cell height) | a new city |
| `nbr_host` | `clear?`, `blocked?` [{`rect`: [[x, z], [x, z]], `mask?`}], `max_slope?` (0.6109, radians), `preferred?`, `override?` | the host and the tool's preferred bridge and altitude override |
| `nbr_types` | `type` | the shipped bridge types of a network type (2.6), replacing the tool |
| `nbr_drag` | `steps`, `z?` (10), `pen?` (4) | a straight one-cell-wide x-drag |
| `nbr_water` | `expect?` (the number of bridges recorded) | `IdentifyBridgeCells` (4) |
| `nbr_land_scan` | `max_height?` (55), `bank_tolerance?` (20) | `IdentifyLandBridgeCells` (7.2) |
| `nbr_adjust` | `start`, `end`; `expect` {`start`, `end`, `clearance`, `reference`} | `AdjustBridgeSpan` (5.2) |
| `nbr_pad` | `start`, `end`, `min_len`; `expect` {`ok`, `start`, `end`} | `PadStepsToMakeMinBridgeSpan` (6.2) |
| `nbr_best` | `start`, `end`, `water?` (true), `h?` (0); `expect` {`index`, `start`, `end`, `candidates`} | `FindBestBridgeType` (6.1) |
| `nbr_model` | `bridge_type`, `start`, `end`, `deck?` (270), `pen?` (4), `z?` (10), `specialized?` | the parametric (10) or specialised (9) model |
| `nbr_pins` | `valid?`; `expect` [[start, end, e, e] ...] | the height-solve constraint entries (8.3) |
| `nbr_cost` | `per_step?` (100); `expect` | `GetCostOfSolution`'s bridge term (12) |
| `nbr_getter` | `slot` (`major`, `altitude`, `reference`, `land`, `preferred`, `override`, `bitmap`, `candidates`), `index?` | the tool's bridge slots (1.1) |
| `nbr_edge_mask` | `pen`, `index`, `n`; `expect` | the allowed edge mask (4.3) |
| `nbr_orient` | `pen`; `expect` {`r`, `t`, `t_mirror`, `forward`} | the orientation (9.1) |
| `nbr_ramp_heights` | `pairs` [[a, b] ...]; `expect` {`rise`, `drops`, `offsets`, `last_column`} | the ramp heights (2.4) |
| `nbr_parse` | `text`, `keys`, `min_span?`; `expect` {`ok`, `lengths`} | the specialisation grammar (3.1) |
| `nbr_expect` | `expect` (partial: `valid`, `records`, `candidates`, `kinds`, `piece_kinds`, `columns`) | the tool and the run |

## Adding a vector

1. Name it after the spec section it mainly covers, for example `s5_3_….json` (simulator), `f7_…` (foundations
   §7) or `b15_…` (budget §15), `d16_…` (demand §16), `o13_…` (ordinance §13), `w14_…` (weather §14),
   `g10_…` and `tg10_…` (simgrid §10, `fc4_core::grid`),
   `fl16_…` (flammability §16), `p11_…` (pollution §11), `c17_…` (crime §17), `a17_…` (aura §17),
   `pw15_…` (power §15), `pl14_…` (plumbing §14),
   `pr18_…` (protection §18), `fo16_…` (flora §16), `bd11_…` (building development §11),
   `gd14_…` (growth developer §14), `td11_…` (tract developer §11), `ld16_…` (lot developer §16),
   `zn15_…` (zones §15), `lf11_…` (landfill §11), `nb18_…` (neighbours §18), `tn11_…` (traffic network §11),
   `pf11_…` (pathfinder §11), `rs15_…` (residential §15), `rs00_…` (other residential behaviour), `ts11_…`
   (traffic simulator §11), `hw12_…` (history §12), `ck12_…` (clock24 §12), `nm16_…` (network manager §16;
   `nm00_…` for other network manager behaviour), `cd8_…` (city detail §8), `pd12_…` (port developers §12),
   `pv12_…` (prop developer §12), `dm12_…` / `dm00_…` (demolition §12 / other demolition behaviour), `dp12_…` /
   `dp00_…` (dispatch), `cs12_…` / `cs00_…` (city situation), `ds18_…` and `ds00_…` (disasters §18 and the rest of
   disasters), `plt_…` (prop-lotconfig §T), `tu8_…` and `tu00_…` (tutorial §8 and the rest), `tr18_…` and
   `tr00_…` (terrain §18 and the rest of terrain), `nt05_…` (nation §5), `am14_…`
   (automata §14), `om18_…` / `om14_…` (occupant manager §18 / §14), `ad17_…` (advisors §17; `ad04_…` and
   `ad06_…` for other advisor behaviour) and `ms12_…` (MySims §12).
   (automata §14), `om18_…` / `om14_…` (occupant manager §18 / §14) and `nr15_…` (network rules §15).
2. Prefer a frozen clock and exact expectations. Derive every expected value from the spec, by hand, and say how in
   `description` or `note`.
3. Run `cargo test -p fc4-conformance`. The Rust runner generates one test per file.
4. If the core fails the vector, decide whether the spec, the vector or the core is wrong. Never edit a vector to
   match an implementation without a spec reason.

## Coverage (spec §10)

| §10 item | Vectors |
|---|---|
| 1 date conversions | `s10_1_date_conversions` |
| 2 day cadence at speed 2 | `s10_2_day_cadence_ticks`, `s10_2_day_cadence_free_running_clock` |
| 3 slow day agent | `s10_3_slow_day_agent` |
| 4 week/month/year | `s10_4_week_month_year` |
| 5 one-shot agents | `s10_5_one_shot_agents`, `s10_5_emergency_agents_ticks` |
| 6 pause / emergency | `s10_6_pause_and_emergency` |
| 7 enabled bits | `s10_7_enabled_bits`, `s10_7_disabled_agents_skipped` |
| 8 save round trip | `s10_8_save_load` |
| 9 limit-timer calibration | `s10_9_limit_timer_calibration` |
| other sections | `s4_reentrancy_and_cursors`, `s4_month_registration_order`, `s5_1_on_tick_guards`, `s5_2_minute_and_hour_levels`, `s5_3_begin_new_interval_flush`, `s5_4_idle_time`, `s6_speed_and_dilation`, `s6_invalid_speed_from_save`, `s9_cheats`, `s9_post_city_init` |

## Coverage (foundations §7)

| §7 item | Covered by |
|---|---|
| 1–4 primitives, sticky error, strings, resource key | `f7_01_04_stream_primitives` |
| 5 references | `fc4-core` unit tests (`stream`, `store`): null/object bytes, shared instances, cycles, unknown class |
| 6–7 CRC and class record | `f7_06_07_crc_and_class_record` |
| 8–9 store save order and load | `fc4-core` unit tests (`store`); `b15_16_save` (three real classes, renumbered IDs) |
| 10 city references | not yet (no city object) |
| 11 response curves | `f7_11_response_curves` |
| 12 messages | `engine/tests/message_test.cpp` (the GZCOM message class) |
| 13–15 random generators, 16 uniform rounding and clamped Gaussian | `f7_13_15_random_generators` |
| §6.3 seed derivation (session seed → each generator's seed) | `f6_03_seed_derivation` |
| §14 16–17 variants | `f14_16_17_variants` |
| §14 18–19 property holders | `f14_18_19_property_holders` |
| data-formats §5.1 scalar conversions (EX2–EX4) | `df5_01_scalar_conversions`; holder slot 8 `GetProperty(id, uint32&)` on every kind of value (NG44) in `df5_02_get_property_u32`; the text round trip, the exemplar reader and data-formats §10.4–§10.7 are `fc4-formats` unit tests |
| §14 20–22 regional-city record, deal claiming, import/export | not yet (neighbours milestone) |
| §14 23 `rand()` | `f14_23_crt_rand`; the prime-step iterator that uses it: `w14_05_iterator` and the `fc4-core` unit tests (`weather::iterator`) |
| §14 24 month order | `s4_month_registration_order` (registration order within a level), `o13_10_month_order` (the ordinance-lag scenario with the real ordinance simulator and a stub budget, in both orders) |

## Coverage (budget §15)

| §15 item | Vector |
|---|---|
| 1 funds | `b15_01_funds` |
| 2 tax | `b15_02_tax` |
| 2 tax: the total is a float32 running sum in lot order (BQ3, the §5.2 worked example: 6567 in list order, 6566 reversed, lines 3848 + 2718) | `b15_02b_tax_lot_order` |
| 3 settlement | `b15_03_settlement` |
| 4 year end | `b15_04_year_end` |
| 5 bankruptcy | `b15_05_bankruptcy` |
| 6 bonds | `b15_06_bonds` |
| 7 department percentage | `b15_07_department_pct` |
| 8 redistribution | `b15_08_redistribution` |
| 9 per-building funding | `b15_09_local_funding` |
| 10 re-synchronisation | `b15_10_resync` |
| 11 budget items | `b15_11_budget_items` |
| 12 groups | `b15_12_groups` |
| 13 tuning | `b15_13_tuning` |
| 14 daily | `b15_14_daily` |
| 15 estimates | `b15_15_estimates` |
| 16 save | `b15_16_save` |
| other sections (4.1, 4.3: messages, cheats, new-city history) | `b4_3_messages_and_cheats` |

## Coverage (demand §16)

| §16 item | Vector |
|---|---|
| 1 node arithmetic | `d16_01_node_arithmetic` |
| 2 active demand formula | `d16_02_active_demand` |
| 3 min/max ordering | `d16_03_min_max` |
| 4 propagation | `d16_04_propagation` |
| 5 shipped graph | `d16_05_shipped_graph_excerpt` (the shipped data cannot be vendored: the vector rebuilds the links of §4.5 that test 5 exercises) |
| 6 census | `d16_06_census` |
| 7 occupancy messages | `d16_07_occupancy` |
| 8 tax and economy | `d16_08_tax_economy` (the nation contract of §8.2 is a `fc4-core` unit test) |
| 9 cap relief | `d16_09_cap_relief` |
| 10 region | `d16_10_region` |
| 11 history | `d16_11_history` |
| 12 save round trip | `d16_12_save` |
| §5 `GetNeutralTaxRate` sum order (DQ9, the §5 worked example) | `d16_13_neutral_rate_sum_order` |
| §5 `CalculateJobsPerUnitOfDemand` scale, the §4.1 link factor (33 → `0x3EA8F5C2`; DQ12: two roundings from 2^31 up, 3242996796 → `0x4BF76BC7`) and `GetJobsBySensus` (DQ10: `(jobs[3] + jobs[2]) + jobs[1]`, 1/3/96 % × 1000 → 999) | `d16_14_jobs_per_unit_scale` |

## Coverage (ordinance §13)

| §13 item | Vector |
|---|---|
| 1 loading | `o13_01_loading` |
| 2 monthly amount | `o13_02_monthly_amount` |
| 3 availability | `o13_03_availability` |
| 4 enact/repeal accounting | `o13_04_accounting` |
| 5 messages | `o13_05_messages` |
| 6 quirks | `o13_06_quirks` |
| 7 cumulative queries | `o13_07_cumulative` |
| 8 array queries | `o13_08_arrays` |
| 9 save | `o13_09_save` |
| foundations §14 item 24 | `o13_10_month_order` |

## Coverage (weather §14)

| §14 item | Vector |
|---|---|
| 1 ambient values | `w14_01_ambient` |
| 2 season | `w14_02_season` |
| 3 wind | `w14_03_wind` |
| 4 temperature (and the §6.6 water scan and sea mask) | `w14_04_temperature` |
| 5 iterator | `w14_05_iterator` |
| 6 moisture | `w14_06_moisture` |
| 7 moisture variation (and §6.4.1 SetManualMoistureAddition) | `w14_07_variation` |
| 8 accessors | `w14_08_accessors` |
| 9 save | `w14_09_save` |
| 10 ambient and wind precision (§6.1, §6.2) | `w14_10_ambient_precision` |
| 11 table-mode iterator (§6.7) | `tg10_19_random_iterator` |
| 12 pass-2 smoothing reads pass-1 values (§6.5) | unit test `weather::simulator::tests::pass_two_reads_pass_one` |

## Coverage (simgrid §10)

| §10 item | Vector |
|---|---|
| 1 geometry | `g10_01_geometry` |
| 2–3 positions and bounds | `g10_02_03_positions_bounds` |
| 4–8 values, averages, tract size changes | `g10_04_08_values_averages` |
| 9–10 persistence | `g10_09_10_persistence` |
| 11–14 radius effect, rounding, clamping | `g10_11_14_radius_effect` |
| §7.1 huge radii and centres near the int32 limits (regression) | `g10_11b_radius_overflow` |
| 15–16 distance effect | `g10_15_16_distance_effect` |
| §7.2 radius above 255 rounds (regression) | `g10_15b_distance_rounds_cap` |
| 17 two grids with callback | `g10_17_two_grids` |
| 18 scanning iterator | `g10_18_scan_iterator` |

## Coverage (land value §16)

| §16 item | Vector |
|---|---|
| 1 land value type | `lv16_01_land_value_type` |
| 2 code defaults, lifecycle | `lv16_02_defaults_lifecycle` |
| 3–4 water bonus | `lv16_03_04_water_bonus` |
| 5 altitude statistics | `lv16_05_altitude_statistics` |
| 6 mode tie | `lv16_06_mode_tie` |
| 7 zoning fallback | `lv16_07_zoning_fallback` |
| 8–9 altitude effect, topIndex | `lv16_08_09_altitude_effect` |
| 10 desirability | `lv16_10_desirability` |
| 11 average | `lv16_11_average` |
| 12–13 save and load | `lv16_12_13_save_load` |
| 14 messages | `lv16_14_messages` |
| 15 pipeline call count | `lv16_15_pipeline` |
| 16 missing altitude curve | `lv16_16_missing_altitude_curve` |
| 17 desirability mapping (scratch 2, desirability 4) | `lv16_17_desirability_mapping` |
| 18 desirability map smaller than the scratch area (skipped tracts; equal and coarser [choice]s) | `lv16_18_desirability_coverage` |

## Coverage (simgrid §10, `fc4_core::grid`)

| §10 item | Vector |
|---|---|
| 1 geometry | `tg10_01_geometry` |
| 2 positions | `tg10_02_positions` |
| 3 bounds | `tg10_03_in_bounds` |
| 4 cell value | `tg10_04_cell_value` |
| 5 averages | `tg10_05_averages` |
| 6 float average | `tg10_06_float_average` |
| 7 signed average | `tg10_07_signed_average` |
| 8 resize | `tg10_08_resize` |
| 9 record (and store save/load) | `tg10_09_record` |
| 10 read (and the §2 factories) | `tg10_10_read` |
| 11 radius effect | `tg10_11_radius_effect` |
| 12 radius rounding | `tg10_12_radius_rounding` |
| 13 round half even | `tg10_13_round_half_even` |
| 14 clamping and edges | `tg10_14_radius_clamp` |
| 15 distance effect | `tg10_15_distance_effect` |
| 16 land value distance | `tg10_16_distance_landvalue` |
| 17 two grids | `tg10_17_two_grids` |
| 18 scanning iterator | `tg10_18_scan_iterator` |
| §7.4 random cell iterator (table, prime-step, constructed) | `tg10_19_random_iterator` |

## Coverage (flammability §16)

| §16 item | Vector |
|---|---|
| Init, tuning (code defaults, shipped values, curve "delete" pattern) | `fl16_00_init_tuning` |
| 1 occupant raw value | `fl16_01_occupant_raw` |
| 2 tract priority | `fl16_02_tract_priority` |
| 3–4 daily value | `fl16_03_04_daily_value` |
| 5 cadence | `fl16_05_cadence` |
| 6 average | `fl16_06_average` |
| 7 hotspots | `fl16_07_hotspots` |
| 8 hotspot slot | `fl16_08_hotspot_slot` |
| 9 global multiplier | `fl16_09_global_multiplier` |
| 10–11 fire roll | `fl16_10_11_fire_roll` |
| 12–13 occupant update | `fl16_12_13_occupant_update` |
| 14 persistence (Read appends; failed reference) | `fl16_14_persistence` |
| 15 interface slots (portable part) | `fl16_15_slots` |
| 16 property conversion (`GetValSint32`) | `fl16_16_property_conversion` |
| 17 single-value curve | `fl16_17_single_value_curve` |
| 18 missing services | `fl16_18_missing_services` |
| 19 subscriptions, `0xC53D10AA` ignored | `fl16_19_subscriptions` |

## Coverage (pollution §11)

| §11 item | Vector |
|---|---|
| 1 cone | `p11_01_03_cones` |
| 2 default radii | `p11_01_03_cones` |
| 3 garbage rectangle | `p11_01_03_cones` |
| 4 skip (and the lists and budget of §4.2.3–4.2.4) | `p11_04_skip_lists_budget` |
| 5 relaxation | `p11_05_08_tract_visit` |
| 6 water stop at 1 | `p11_05_08_tract_visit` |
| 6 the special case always writes: v = 1 kept, `dirtyWater` set (§4.4.4 step 10, PG14) | `p11_28_water_special_case_dirty` |
| 7 traffic | `p11_05_08_tract_visit` |
| 8 zone averaging | `p11_05_08_tract_visit` |
| 9 garbage production | `p11_09_12_garbage` |
| 10 Dispose waste-to-energy quirk (and export, landfill) | `p11_09_12_garbage` |
| 11 valve | `p11_09_12_garbage` |
| 12 valve closed in a cell | `p11_09_12_garbage` |
| 13 water treatment | `p11_13_15_capacities` |
| 14 efficiency | `p11_13_15_capacities` |
| 15 current capacity (and maintenance cost, §4.9.4) | `p11_13_15_capacities` |
| 16 monthly cost (and aging, usage roll, waste-to-energy output) | `p11_16_monthly` |
| 17 cadence | `p11_17_18_cycle` |
| 18 end of cycle | `p11_17_18_cycle` |
| 19 slots | `p11_19_20_slots` |
| 20 prop share | `p11_19_20_slots` |
| 21 effects | `p11_21_effects` |
| 22 persistence | `p11_22_persistence` |
| 23 deal rounding | `p11_23_deal_rounding` |
| 24 usage distribution | `p11_24_usage_distribution` |
| 25 Read with a stream error | `p11_25_read_error` |
| `_ftol` truncation (foundations §18.1) and curve patterns (foundations §4.1, §4.3) | `p11_26_trunc_curves` |
| other sections (lifecycle §4.1, disaster radiation §4.3, tuning §7.1, SimBegin) | `p11_00_lifecycle` |

## Coverage (crime §17)

| §17 item | Vector |
|---|---|
| 1 decay | `c17_01_decay` |
| 2 census buckets | `c17_02_census` |
| 3 jobless factor | `c17_03_jobless` |
| 4 criminal base | `c17_04_criminal_base` |
| 5 crimes to commit | `c17_05_crimes_to_commit` |
| 6 ordinance rounding | `c17_06_ordinance_rounding` |
| 7 attenuation (u = 1.0 commits) | `c17_07_attenuation` |
| 8 stamp and hotspot | `c17_08_stamp_hotspot` |
| 9 type selection (and §6.8 severity 0) | `c17_09_type_selection` |
| 10 mask parsing (and the §7 type list) | `c17_10_masks` |
| 11 crime-effect cone (and the §6.10 raw-byte read, CQ13) | `c17_11_crime_effect`, `c17_25_crime_effect_raw` |
| 12 average crime | `c17_12_average` |
| 13 pipeline | `c17_13_pipeline` |
| 14 RNG draw count (and the §6.1 24-bit sequences of a seeded generator) | `c17_14_rng_draws`, `c17_23_rng_seeded` |
| 15 incident array (core part; the 28-byte layout is the plugin's) | `c17_15_incident_array` |
| 16 prop requester | `c17_16_prop_requester` |
| 17 save | `c17_17_save` |
| 18 lifecycle | `c17_18_lifecycle` |
| 19 `rhe` beyond 2²² | `c17_22_rhe` (also `c17_16_prop_requester`) |
| 20 Read with a stream error | `c17_17_save` |
| 21 SelectProp, no lot | `c17_16_prop_requester` |
| 22 nearest crime sim distance | `c17_20_nearest` |
| §8 tuning | `c17_19_tuning` |
| §11.3 nearest crime sim / prop | `c17_20_nearest` |
| §6.8 arson | `c17_21_arson` |
| §7 type names (CQ8: LTEXT key, U+2022, fallback) | `c17_24_type_names` |
| §7 name and icon keys of other value types: the first 12 data bytes as three u32 (CQ23) | `c17_26_type_key_raw_bytes` |

## Coverage (aura §17)

| §17 item | Vector |
|---|---|
| Init, Shutdown, tuning (defaults, "delete" curve pattern, exemplar reload) | `a17_00_lifecycle_tuning` |
| 1 ABI (slots 6 / 7, getters, class, QueryInterface) | `a17_01_abi` |
| 2 park funding (int16 wrap, NaN), 19 no Parks department | `a17_02_park_funding` |
| 3 landmark funding (int16 wrap) | `a17_03_landmark_funding` |
| 4 tax effect | `a17_04_tax_effect` |
| 5 lot base aura | `a17_05_lot_base_aura` |
| 6 work stamping (and the time slice with an expiring clock) | `a17_06_work_stamping` |
| 7 curves on cells | `a17_07_curves_on_cells` |
| 8 long decay | `a17_08_long_decay` |
| 9 global effect | `a17_09_global_effect` |
| 10 aura and rating | `a17_10_aura_rating` |
| 11 transient | `a17_11_transient` |
| 12–13 hotspots and riot | `a17_12_13_hotspots_riot` |
| 14 cadence | `a17_14_cadence` |
| 15 occupant cones | `a17_15_occupant_cones` |
| 16 persistence (and a failed reference, foundations §2.7) | `a17_16_persistence` |
| 17 BustStrike | `a17_17_bust_strike` |
| 18 registry | `a17_18_registry` |
| 17 desirability mapping (scratch 2, desirability 4) | `lv16_17_desirability_mapping` |
| 20 empty curves | `a17_20_empty_curves` |
| 21 population mapping | `a17_21_population_mapping` |
| 22 time-slice step | `a17_22_time_slice` |
| §6 integer stores (`_ftol`, low bits) | `a17_23_integer_stores` |

## Coverage (power §15)

| §15 item | Vector |
|---|---|
| tuning (code defaults, "default" curve pattern, reload, ordinance factor) | `pw15_00_tuning` |
| budget hookup and removal, FindPowerPlants, GetPowerPlants, lots, deal status (§5, §5.1, §5.2) | `pw15_00_budget_lots_deals` |
| terrain changes, PostCityInit, pole and network removal, Shutdown (§5.2, §5.4, §5.6, §4.2) | `pw15_00_terrain_postinit` |
| int64 line-item secondary info (§5.1, §5.2) | `pw15_00_secondary_int64` |
| 1 output, 2 slot 45/46 ABI (and slots 32–34, 49–51) | `pw15_01_02_output_abi` |
| 3 single network, 4 shortfall, 5 conservation | `pw15_03_05_network_supply` |
| 6 import, 7 export | `pw15_06_07_import_export` |
| 8 flood fill, 9 wire radius, 10 centre cell | `pw15_08_10_flood_wire_centre` |
| 11 grid values (and dirty wiring, §5.3) | `pw15_11_grid_values` |
| 12 positions, 19 strikes (and identity, §1) | `pw15_12_19_positions_strikes` |
| 13 maintenance | `pw15_13_maintenance` |
| 14 ageing | `pw15_14_ageing` |
| 15 pole decay | `pw15_15_pole_decay` |
| 16 deficit, 17 unpowered % | `pw15_16_17_deficit_unpowered` |
| 18 ring | `pw15_18_ring` |
| 20 save | `pw15_20_save` |
| 21 lifecycle | `pw15_21_lifecycle` |
| 22 Nuclear Free Zone | `pw15_22_nuclear_free_zone` |
| 23 maintenance with departments, 24 FindPowerPlants with departments | `pw15_23_24_budget_departments` |
| 25 counters below zero, 26 pole removal | `pw15_25_26_counters_poles` |
| 27 terrain passes | `pw15_27_terrain` |
| 28 NaN ratio | `pw15_28_nan_ratio` |
| 29 Init after load, 30 failed Read | `pw15_29_30_init_load` |

## Coverage (plumbing §14)

| §14 item | Vector |
|---|---|
| Init, Shutdown, references, the agent and the department | `pl14_00_lifecycle` |
| Buildings, budget items, maintenance, the consumer-removal and lotless-producer quirks | `pl14_00_buildings_budget` |
| Shared seed cells (§6.2) | `pl14_00_seeding` |
| 1 defaults, 2 shipped tuning | `pl14_01_02_tuning` |
| 3 pipe flags, 4 pipe cost, 16 burst bookkeeping | `pl14_03_04_16_pipes` |
| 5 pump capacity, 6 pollution shutdown, 7 power, 8 condition curve | `pl14_05_08_production` |
| 9 basic flood, 10 shortage | `pl14_09_10_flood` |
| 11 ring geometry | `pl14_11_ring_geometry` |
| 12 export | `pl14_12_export` |
| 12 export flags per network, 0x28 (PL7) | `pl14_12_export_flags` |
| 13 import | `pl14_13_import` |
| 14 pipe funding, 15 GaussianFast | `pl14_14_15_pipe_funding` |
| 17 deal message, 18 disabled | `pl14_17_18_messages` |
| 19 grid values, 20 grid swap (and the time slice) | `pl14_19_20_grids` |
| 21 NearestWater | `pl14_21_nearest_water` |
| 22 ageing | `pl14_22_ageing` |
| 23 persistence | `pl14_23_persistence` |
| 24 SDK return channels | `pl14_24_slots` |
| 25 iterator after a pass (PL4) | `pl14_25_iterator` |
| 26 current producer in a save (PL3) | `pl14_26_current_producer` |
| 27 re-Init after Shutdown (PL2) | `pl14_27_reinit` |

## Coverage (protection §18)

| §18 item | Vector |
|---|---|
| Init, Shutdown, Release, tuning (code defaults, shipped exemplar, "default" curve pattern, reload filter) | `pr18_00_lifecycle` |
| 1 fire efficiency curve | `pr18_01_fire_efficiency_curve` |
| 2 road and strike modifiers, fundingOf | `pr18_02_efficiency_modifiers` |
| 3 jail efficiency | `pr18_03_jail_efficiency_curve` |
| 4 strike curve and the single draw | `pr18_04_strike_curve` |
| 5 chance of strike | `pr18_05_chance_of_strike` |
| 6 strike end (duration, funding, unsigned days) | `pr18_06_strike_end` |
| 7 monthly strike pass (posts, unadjusted rebuild only) | `pr18_07_monthly_strike_pass` |
| 8 jailbreak curve | `pr18_08_jailbreak_curve` |
| 9 rehabilitation | `pr18_09_rehabilitation` |
| 10 ResetUnjailedCriminals | `pr18_10_reset_unjailed` |
| 11 Recalculate and the jail lists | `pr18_11_recalculate` |
| 12 escapees, jailbreak begin and end | `pr18_12_escapees` |
| 13 crime events and ProcessMonthlyCrimeStats | `pr18_13_crime_event` |
| 14 occupant coverage | `pr18_14_occupant_coverage` |
| 15 effect scaling | `pr18_15_effect_scaling` |
| 16 dispatch masks, nearest station, dispatch cones | `pr18_16_dispatch` |
| 17 call numbers | `pr18_17_call_numbers` |
| 18 cadence | `pr18_18_cadence` |
| 19 cycle averages and the daily step | `pr18_19_cycle_averages` |
| 20 adjusted copy and funding change | `pr18_20_adjusted_copy` |
| 21 persistence (layout, versions, store round trip) | `pr18_21_persistence` |
| 22 slot ABI, QueryInterface, crime-compatible slots | `pr18_22_slot_abi` |
| 23 stuck jailbreak (capacity 0, NaN percentage) | `pr18_23_stuck_jailbreak` |
| 24 jailed percentage and the end and start tests | `pr18_24_jailed_percentage` |
| 25 percentages (float32 0.01f) | `pr18_25_percentages` |
| 26 GetMaxPossibleDispatchCount | `pr18_26_max_possible_dispatch` |
| 27 station effect partial reads | `pr18_27_effect_partial_reads` |
| 28 call-number search | `pr18_28_call_number_search` |
| 29 station statistics | `pr18_29_station_statistics` |
| 30 jail capacity without an item or property | `pr18_30_jail_capacity` |
| 31 locally-funded-jail list | `pr18_31_funded_jail_list` |

## Coverage (flora §16)

| §16 item | Vector |
|---|---|
| constructor, Init, Shutdown, tuning (shipped and code defaults), QueryInterface, reference count (§1, §3.1, §4, §7) | `fo16_00_init_tuning` |
| 1 stage, 2 negative age | `fo16_01_02_stage` |
| 3 elevation index, 4 score | `fo16_03_04_elevation_score` |
| 5 exemplar table conversion (and the record's other fields) | `fo16_05_table_conversion` |
| 6 best type (and the key-0 record) | `fo16_06_best_type` |
| 7 blocksPerMonth (and the monthly iterator), 8 wild seed scaling (and wild seeding) | `fo16_07_08_blocks_wild` |
| 9 daily, 10 daily with rate > 1 | `fo16_09_10_daily` |
| 11 DropSeed | `fo16_11_drop_seed` |
| 12 location bounds, 13 slope, 14 under water, 15 blocking | `fo16_12_15_location` |
| 16 planting altitude, 17 cluster merge | `fo16_16_17_planting` |
| 18 PlaceFlora | `fo16_18_place_flora` |
| 19 seeding, 20 seeding interval | `fo16_19_20_seeding` |
| 21 low score | `fo16_21_low_score` |
| 22 kill (and RemoveOccupant) | `fo16_22_kill` |
| 23 building insert (and networks) | `fo16_23_building_insert` |
| 24 terrain change | `fo16_24_terrain` |
| 25 cheat (and blast, exemplar-changed routing), 26 established | `fo16_25_26_cheat_established` |
| 27 persistence, 28 Init after load | `fo16_27_28_persistence` |
| 29 GetAllFloraTypes (and the other type queries) | `fo16_29_all_types` |
| 30 obliteration | `fo16_30_obliteration` |
| 31 AdvanceClusterType, 32 seed types | `fo16_31_32_clusters_seed_types` |

## Coverage (building development §11)

| §11 item | Vector |
|---|---|
| 1 developer types, 2 map keys, 3 zone fits | `bd11_01_03_types_keys_zones` |
| 4 monthly supply | `bd11_04_monthly_supply` |
| 5 claim / release | `bd11_05_construction_resources` |
| 6 grow order (seeds 12345 and 1); the multi-day draw sequence | `bd11_06_grow_order`, `bd11_06_grow_days` |
| 7 tried-tract marks (and their save / Shutdown) | `bd11_07_tried_tract_marks` |
| 8 decline plan (seed 99), 9 decline execution | `bd11_08_09_decline` |
| 10 max density | `bd11_10_max_density` |
| 11 suitability | `bd11_11_suitability` |
| 12 building map, 13 highest stage | `bd11_12_13_building_map_stage` |
| 14 record | `bd11_14_record` |
| 15 stage multiplier (and the tuning reads) | `bd11_15_stage_multiplier` |
| 16 catalogue queries | `bd11_16_catalogue` |
| Init, Shutdown, Release, QueryInterface (§1) | `bd11_00_lifecycle` |
| The generator's seed from the session seed (§4.14, foundations §6.3) | `bd11_00_session_seed` |
| DoIdle / monthly state machine (§3.1, §4.1, §4.2) | `bd11_00_idle_monthly` |
| messages: cheats, special buildings, terrain unpacking, PlaceBuilding (§3.2, §4.12) | `bd11_00_messages` |
| DoTerrainChanged (§4.11) | `bd11_00_terrain_changed` |
| placing buildings (§4.9) | `bd11_00_placing` |
| Grow and decline edges (§4.3–§4.5) | `bd11_00_grow_edges` |
| the daily Grow attempt budget ([choice] replacing the 3 ms timer, §4.4) | `bd11_06_grow_budget` |
| persistence beyond test 14 (§6) | `bd11_00_persistence` |
| catalogue loading, names, keys (§4.7.1–§4.7.3); hash order and resize | `bd11_00_catalogue_extras`, `bd11_00_catalogue_hash_order` |

## Coverage (growth developer §14)

| §14 item | Vector |
|---|---|
| Creation, Init, Shutdown, references, QueryInterface, slot table | `gd00_lifecycle` |
| Getters, construction info, state machine bounds | `gd00_getters` |
| Every desirability layer and EstimateDesirability against a model | `gd00_layers` |
| Special buildings (states 14 → 16) | `gd00_special_buildings` |
| Time slicing of state 18 (shared step) | `gd00_slicing` |
| The game's `std::sort` and its tie order | `gd00_sort_ties` |
| Shipped exemplars (§4.9) | `gd00_shipped_tuning` |
| Draw sequence over several developers (§7): one draw per industrial update, the shared generator's state, salts as a multiset ([compat-limit]) | `gd00_draw_sequence` |
| The shared generator's seed from the session seed (§7, foundations §6.3) | `gd00_session_seed` |
| The BDS's developer operations, one method each (§12, BDS §1.5) | `gd00_replacement_unit` |
| 1 population | `gd14_01_population` |
| 2 max capacity | `gd14_02_max_capacity` |
| 3 built capacity | `gd14_03_built_capacity` |
| 4 stage assignment | `gd14_04_stage_assignment` |
| 5 AddCurve wrap | `gd14_05_wrap` |
| 6 commute | `gd14_06_commute` |
| 7 EstimateDesirability vs update | `gd14_07_estimate` |
| 8 lists | `gd14_08_lists` |
| 9 industrial decline order (seeded RNG) | `gd14_09_industrial_decline_rng` |
| 10 NewDay rule | `gd14_10_newday` |
| 11 phantom demand (by the rules: 39, see the vector) | `gd14_11_phantom` |
| 12 style map | `gd14_12_style_map` |
| 13 tuning defaults | `gd14_13_tuning_defaults` |
| 14 save round trip | `gd14_14_save` |
| 15 subjective thresholds | `gd14_15_subjective` |
| 16 StateChanged | `gd14_16_state_changed` |

## Coverage (tract developer §11)

| §11 item | Vector |
|---|---|
| 1 outcome codes and `best` | `td11_01_02_codes_desired` |
| 2 desired / desiredMax | `td11_01_02_codes_desired` |
| 3 pass budget (18 passes; a failed relaxed pass ends the call) | `td11_03_pass_budget` |
| 4 strict vs relaxed (rule 7) | `td11_04_strict_relaxed` |
| 5 takeover thresholds | `td11_05_takeover` |
| 6 score, residential type 2 | `td11_06_09_score` |
| 7 score, commercial | `td11_06_09_score` |
| 8 score, aggregation and subdivision lot costs | `td11_06_09_score` |
| 9 industrial random term | `td11_06_09_score` |
| 10 bounding rectangle | `td11_10_15_geometry_window_styles` |
| 11 corner test | `td11_10_15_geometry_window_styles` |
| 12 capacity window | `td11_10_15_geometry_window_styles` |
| 13 style percentages → group | `td11_10_15_geometry_window_styles` |
| 14 seeded PickBuilding draws (R3–R5), inside a full Grow | `td11_14_pick_building_draws` |
| 15 hash order tie | `td11_10_15_geometry_window_styles`, `td00_hash_growth_sort` |
| 16 seeded UpdatePopulation (R7), incl. the shipped variation 0 and tract (0, 0) (seed 1, [choice]) | `td11_16_17_update_population` |
| 17 population clamps | `td11_16_17_update_population` |
| 18 Decline (R8, multi-cell lot counted once) | `td11_18_decline` |
| 19 save record (v3 bytes, v1 / v2 loads, v4 rejected, truncated records) | `td11_19_save_record` |
| 20 style rotation | `td11_20_21_styles` |
| 21 SetActiveStyles | `td11_20_21_styles` |
| 22 subdivision geometry (facings 2 and 0, inside full Grows) | `td11_22_subdivision` |
| 23 farm minimum (14 → 0, 15 → Claim(45), Release(30); R6) | `td11_23_farm` |
| 24 IsLotSuitable order | `td11_24_lot_suitable_order` |
| extra: STLport hash_set growth and `std::sort` (introsort, ties) | `td00_hash_growth_sort` |
| extra: per-pass draw order of a full Grow with several scored candidates (R1, R2 × 3, R3, R4, R5) and a scored takeover | `td00_industrial_grow_draws` |
| extra: lifecycle, messages, QueryInterface, filters, tuning reader | `td00_lifecycle_tuning` |
| extra: the file-local generator's seed from the session seed (§4.22, foundations §6.3) | `td00_session_seed` |
| extra: aggregation, Build without a lot [quirk], resource failure | `td00_aggregation_build` |
| extra: R9 (CreateLotIfPossible with facing −1) in the per-pass order | `td00_aggregation_r9` |
| extra: kick-out, caps (building developer's power, own fire), style checks [quirk] | `td00_kickout_caps_styles` |

## Coverage (lot developer §16)

| §16 item | Vector |
|---|---|
| 1 desirability tolerance (per-location seeds) | `ld16_01_desirability_tolerance` |
| 2 desirability threshold (29 / 30 low, 31 not) | `ld16_02_09_desirability_power` |
| 3 daily quota (n/28 + 1) | `ld16_03_04_18_daily_pass` |
| 4 wrap day (29 lots, perDay 2) | `ld16_03_04_18_daily_pass` |
| 5 power grace (1, 1, 0; reset on power) | `ld16_05_07_09_power_travel_zot_condition` |
| 6 travel (commute 7th visit, no-road 3rd) | `ld16_05_07_09_power_travel_zot_condition` |
| 7 zot priority (lower wins, road notification first) | `ld16_05_07_09_power_travel_zot_condition` |
| 8 empty-lot zot pattern | `ld16_01_desirability_tolerance` |
| 9 condition (wealth mismatch, occupancy, abandoned steps) | `ld16_05_07_09_power_travel_zot_condition` |
| 10 development cost (slope cascade, curve) | `ld16_10_11_geometry_cost_rubble_count` |
| 11 rubble count (small / large / interior) | `ld16_10_11_geometry_cost_rubble_count`, `ld00_rubble_interior` |
| 12 seeded rubble corners | `ld16_12_13_14_seeded_draws` |
| 13 seeded completion content | `ld16_12_13_14_seeded_draws` |
| 14 coffee-bar draw first | `ld16_12_13_14_seeded_draws` |
| 15 night batch (hard-coded 200 threshold) | `ld16_15_16_night_lights` |
| 16 shuffle (`rand()` swaps) | `ld16_15_16_night_lights` |
| 17 save record (24 bytes, version rejected) | `ld16_17_persistence` |
| 18 abandonment order (last failing test) | `ld16_03_04_18_daily_pass` |
| 19 interface degenerates (slots 23, 24, 32) | `ld16_19_57_interface_business_units` |
| 20–25 construction occupant (§6.7) | out of scope: a separate subsystem (`cSC4ConstructionOccupant`), not the lot developer core |
| 26 StateChanged data4 (§3.2) | out of scope: `cSC4Lot::SetState`, owned by the lot core |
| 27 UpdateLotElements kind (full 32-bit `kind != 5`) | `ld16_27_update_lot_elements_kind` |
| 42 IsAreaFlat against the first vertex, with water (LD29) | `ld16_42_is_area_flat` |
| extra: AddNetworkConnectionNodes and UpdateLotObjectAlitudes (§6.5, LD17) | `ld6_5_network_nodes_object_altitudes` |
| 28 slot 16 float rectangle (sample order, clamp, interior vertices) | `ld16_28_float_rect_altitude` |
| 29 slot 21 worked example 2 (int32 slot 18, per-vertex and area mode, slope cut-off) | `ld16_29_30_levelling` |
| 30 slot 19 worked example 1 (negative target, force, narrow rectangles; LD-Q1) | `ld16_29_30_levelling` |
| 31 ModifyTerrainHeight per-vertex rules (0.05, water, immovable, locked lots) | `ld16_31_modify_terrain_per_vertex` |
| 32 lot foundation worked example 3 (facings, useLotAltitude, 3 × 3 levelling) | `ld16_32_33_lot_foundation` |
| 33 R8 (foundation type draw) | `ld16_32_33_lot_foundation` |
| 34 foundation lookup by position (vertical line), removal | `ld16_34_foundation_lookup` |
| 35 object positions worked example 4 | `ld16_35_object_positions` |
| 36 boundary rules | `ld16_36_37_boundary_textures` |
| 37 empty-lot textures worked example 5 | `ld16_36_37_boundary_textures` |
| 38 interface returns (the parts the core decides; channels and pops in the plugin ctest) | `ld16_38_interface_returns` |
| extra: lifecycle, messages, QueryInterface, tuning | `ld00_lifecycle_tuning` |
| extra: UpdateLot flow (grown, civic, empty, unit step) | `ld16_42_update_lot` |
| extra: PickUnitToJoin, NeedsAnchorBuilding | `ld16_19_57_interface_business_units` |
| extra: UpdateTravelDesignates (anchor and dissolution), UpdateTravelPopulations, RemoveLotFromItsBusinessUnit | `ld16_57_business_units` |
| extra: StartConstruction, ConstructDefaultLot, Abandon, Reoccupy, ChangeBuilding | `ld00_construction` |
| extra: base textures and their wealth / decay variants | `ld00_base_texture` |
| extra: demolition and explosion | `ld16_07_demolition_explosion` |
| extra: the file-local generator's seed from the session seed (§10.1, foundations §6.3) | `ld00_session_seed` |

## Coverage (zones §15)

| §15 item | Vector |
|---|---|
| 1 zone types (`IsRCIZone`, `GetZoneType`'s sign-extended int8) | `zn15_01_zone_types` |
| 2 size errors (too large, too small; a type ≥ 16 dezones, ZG17) | `zn15_02_size_errors` |
| 3 cost (60 / 66 / 0, funds, counts, insufficient funds) | `zn15_03_cost` |
| 4 water (reject for type 1, accept for 12, error 2 for 14) | `zn15_04_water` |
| 5 ZoningChange (one queued message, void2 rect and cells) | `zn15_05_zoning_change` |
| 6 dezone under a network (flags `0x80200090`, `checkLotInside`) | `zn15_06_dezone_under_network` |
| 7 failure count (the zero-entry insert quirk) | `zn15_07_failure_count` |
| 8 `GetOccupantLot` cells | `zn15_08_09_cell_mapping` |
| 9 `GetLotSavvyTerrainAltitude` cells (the 0.999 round-up quirk) | `zn15_08_09_cell_mapping` |
| 10 `CanCreateLot` codes 3, 7, 8, 2, 1, 0 | `zn15_10_can_create_lot` |
| 11 `GetLotsInCellRect` (both `originOnly` modes) | `zn15_11_get_lots_in_cell_rect` |
| 12 `GetLotsSurroundingRect` (side bits `0xF`, 2, 8) | `zn15_12_get_lots_surrounding_rect` |
| 13 `ExamineBoundaries` (S, D, Q, flag; the zone-1 swap; `alternateOrientation`) | `zn15_13_examine_boundaries` |
| 14 seeded parcellization (session 0 → `0x8C09C68A`; four `U(4)` = 3, 0, 3, 0) | `zn15_14_seeded_parcellization` |
| 15 `GetRandomLot` (four draws, then the lot) | `zn15_15_get_random_lot` |
| 16 remnant draws (one per cell for zones 7–9, none for 1–6) | `zn15_16_remnant_draws` |
| 17 `CreateLotIfPossible` with facing −1 (exactly one `U(4)`) | `zn15_17_create_lot_facing_minus_one` |
| 18 save round trip (both records byte-exact; the master-list order quirk) | `zn15_18_save_round_trip` |
| 19 6 × 4 low-density residential along a road (§7.14 worked example) | `zn15_19_parcellize_6x4_residential` |
| 20 zone 4: back street and forced middle cross street, remnant fallbacks | `zn15_20_parcellize_zone4_streets` |
| 21 no automatic streets (farRoad, six 2 × 2 lots, no Place) | `zn15_21_no_automatic_streets` |
| 22 L-shaped region (the clipped back street, no cross street) | `zn15_22_l_shaped_region` |
| 23 jog street (`LayInJogStreets` dead end, no parallel connection) | `zn15_23_jog_street` |
| 24 extended street (`ExtendAsStreets`, two components in first-cell order) | `zn15_24_extended_street` |
| 25 large block (back streets at x 12, 17, 21; the cross street; 25 lots) | `zn15_25_large_block` |
| 26 map edge (the `p0` clamp to 0, two remnants; the control needs no clamp) | `zn15_26_map_edge_start_clamp` |
| 27 `DepthRule` (all three cases, four parameter sets, ps 0 and 1) | `zn15_27_depth_rule` |
| 28 `DrawStreet` (runs, per-cell mode, the backwards line, the fallback chain) | `zn15_28_draw_street` |
| 29 middle row (`o0 = 10`, step +2, four row ranges) | `zn15_29_middle_row` |
| 30 `DetermineLotSize` (developer choice, stage walk, filter, scores, widening, blocks; the ascending-ID tie; zone 7; the pure functions) | `zn15_32_determine_lot_size` |
| 31 `ExamineBoundaries` at the city border (both low edges, both high edges, the zone-1 swap, placeStreets off) | `zn15_33_examine_boundaries_city_border` |
| 32 `SimpleParcellize` street lines (all eight perpendicular (Q, D) pairs, the edge rule) | `zn15_34_simple_parcellize_street_table` |
| extra: `SimpleParcellize`'s street lines (§7.5 step 1), Q = 3 | `zn15_30_simple_parcellize_streets` |
| extra: `SimpleParcellize`'s Q = 2 x-line rule (§7.5, the `D = 3` branch) | `zn15_31_simple_parcellize_q2_x_lines` |
| extra: `CreateParcellizableRegion`'s `IsCellOccupied` (§7.4, ZQ2): the five highlight-mode rows, the real-mode control, the zone-0 network cell | `zn15_35_parcellizable_region_highlight` |
| 33a highlight mode: test 19 as a preview (virtual lots, the draft Place, `*out`, the state after; with and without the zone; again) | `zn15_36_highlight_preview` |
| 33b highlight mode: a virtually deleted lot | `zn15_37_highlight_virtually_deleted_lot` |
| 33c highlight mode: the point filter (inside, adjacent, neither, outside `*out`'s rect) | `zn15_38_highlight_point_filter` |
| 33d the draft queries of `ExistsNetworkOfType` and `ExistsConnectionOfType` (quirks 16, 17), real-mode control | `zn15_39_draft_queries` |
| 33e highlight mode: test 23 as a preview (the drafts seen by the region and the scores) | `zn15_40_highlight_jog_street` |
| extra: `DetermineLotSize`'s int32 score (ZQ9), `sx = 0` (ZQ10), missing candidates (ZQ7) | `zn15_41_determine_lot_size_int32_score` |
| extra: `LayInJogStreets`' parallel test has a draft branch (§7.12, ZQ4) | `zn15_42_jog_parallel_draft` |
| extra: a failed size check is an ABORT (§6, ZG17): the type-14 landfill cost of the whole region, skipFunds, error 1 replacing 3 or 4 | `zn15_43_size_abort_landfill_cost` |
| extra: `Init` copies the alpha to the default alpha whenever the exemplar is found, also without Zone Display Alpha (§10.1, ZG18); no exemplar leaves both | `zn15_44_default_alpha_without_property` |
| extra: the dezone under a network clips a rectangle that crosses the city edge and dezones the inside cells (§5.1, ZG21) | `zn15_45_dezone_across_city_edge` |
| extra: `Shutdown` deletes every lot through its own `DeleteLot`, x outer, z inner, before the network tools (§1.2, ZG32); with `trace_calls` the world also logs `remove_building_zot_prop`, `lot_shutdown` and `release_lot` (`lot`, `refs`) | `zn1_2_shutdown_deletes_lots` |

The two extras took the numbers 30 and 31 before §15 gained tests 30–32, so tests 30, 31 and 32 are the vectors
numbered 32, 33 and 34.

Not covered by a vector, and why:

- **`DetermineLotSize` (§7.2.1)** is bypassed in tests 19–29 (the spec's own common setup forces width 2, depth 2,
  minDepth 1, maxDepth 3, block 6) and is covered on its own by test 30 (`zn15_32`).
- **`UpdateHighlightTextures`** and the terrain view's slot 9 in `ClearHighlight` (§7.1) are UI and left out of the
  core; highlight mode itself is covered by test 33 (`zn15_36`–`zn15_40`).
- **`CanCreatePloppedLot` (slot 21)**, the highlight textures, the cheats "PlopAllLots" / "PlopAllBuildings" and
  `InitBoundaryRules` are left out of the core (UI or debug, §1.2 step 7, §14.3 item 2).
- **The untraced helpers** `0x00731F60` / `0x00732450` (§5.2 step 3) are one host call and `0x0072B180` (§7.2 step 7)
  is left out, so no vector pins them.

## Coverage (landfill §11)

Every test also has a unit test in `fc4-core` (`landfill::tests::test_NN_…`).

| §11 item | Vector |
|---|---|
| 1 heap capacity (4×4: 9 × 53333 = 479997; 2×2: 120000; 1×5: no heaps) | `lf11_01_heap_capacity` |
| 2 zoning cost, flat (0, EDX 0) | `lf11_02_zoning_cost_flat` |
| 3 zoning cost, slope (variance 20, mean 278, 112) | `lf11_03_zoning_cost_slope` |
| 4 zoning cost, extension (`levelAltitude` 275, 40; no tolerance test) | `lf11_04_zoning_cost_extension` |
| 5 sea-level clamp (mean 256, raw-altitude cost 124) | `lf11_05_sea_level_clamp` |
| 6 first intake (the full host-call sequence, four corner tiles, no draw) | `lf11_06_first_intake` |
| 7 decay (9300, 4, 38500) | `lf11_07_decay` |
| 8 underfunding (5000) | `lf11_08_underfunding` |
| 9 per-tile cap (20000, supply 30000) | `lf11_09_per_tile_cap` |
| 10 global cap (code default 40000) | `lf11_10_global_cap` |
| 11 disconnected (no intake, decay, the zot at centre + 3.0, excluded totals, removal on connect; §4.7.5's two spiral examples: (12, 21) past a dezoned centre, (14, 22) on the clipped ring 2) | `lf11_11_disconnected` |
| 12 random draws (one `rand()` → interior or. 2; none from heap (0,0); one heap per month in play) | `lf11_12_random_draws` |
| 13 redistribution (100, 50, 0 both ways) | `lf11_13_redistribution` |
| 14 spill on dezone (80000, 67500 back to the supply, §4.4.2 order) | `lf11_14_spill_on_dezone` |
| 15 narrow remainder (6 posted type-0 `PlaceZone`s, zone removed, no re-entrancy) | `lf11_15_narrow_remainder` |
| 16 adjacency merge (union rect, 8 cells, heaps (0,0) and (2,2)) | `lf11_16_adjacency_merge` |
| 17 save round trip (§6.3 bytes, `levelAltitude` from the centre, the last-column quirk) | `lf11_17_save_round_trip` |
| 18 usage clamp (100, 100) | `lf11_18_usage_clamp` |
| 19 `GetLandfillZoneFromCell` (half-open rect) | `lf11_19_zone_from_cell` |
| 20 efficiency (1.0, 1.2) | `lf11_20_efficiency` |
| 21 terrain move (271; 0.2 unchanged; empty cell dezoned; sea-level floor) | `lf11_21_terrain_move` |
| 22 levelling target and refusal (vertex average, inert `f`, slope 41, NaN `maxAreaSlope`, ≥ 1000, finite limit; through the zone) | `lf11_22_levelling` |
| 23 reason 5 ignored (also 0 and 4; reasons 1, 2, 3 dezone the empty cells) | `lf11_23_reason_5_ignored` |

The zot's ring order is §4.7.5's (LF4): ring r starts at `(cx + r, cz − r + 1)` and runs +z, −x, −z, +x. The second
half of `lf11_11` was regenerated from the spec's two worked examples (it was built on a top-left-corner order).

## Coverage (neighbours §18)

Every test also has at least one unit test in `fc4-core` (`neighbors::tests::test_NN_…`).

| Test (§18) | Vector |
|---|---|
| 1 geometry (sides, region cells, neighbour connection cells, corners invalid; an interior cell gives our own city and (0, 0), NB6; through the region) | `nb18_01_geometry` |
| 2 connections (one object, one `0x8A0D15DA`, one marker, orientations for both flags, none for Ferry, pipe / subway / elevated markers; road + rail occupant deletes Rail; corner deletes two; records, store round trip, version 3 rejected) | `nb18_02_connections` |
| 3 price (2.6314286 / 263 scarcity, 2.2150418 / 221 garbage; through the simulator) | `nb18_03_price` |
| 4 max amount (950, 1050, 100) | `nb18_04_max_amount` |
| 5 curve bug (221 after `UpdateCommodityStats` with a sanitation department; 263 without it or the property; only the current city object counts, NB12) | `nb18_05_curve_bug` |
| 6 life cycle (activation posting status 0, 0, 1 through `SetFlags`, NB2; first strike, Grace ignores failures, Probation at 90 days, removal with status 3, Probation clears at 180 days, cancelled status 2, expiry 6, renewal 7 or CannotAfford) | `nb18_06_life_cycle` |
| 7 garbage capacity (60 + 60 against exported 100 and capacity 50) | `nb18_07_garbage_capacity` |
| 8 totals (flags `0x01 0x04 0x08 0x10 0x20` exclude; `0x40 0x80 0x100 0x200` do not) | `nb18_08_totals` |
| 9 re-entrancy (retry 8 during a run; 0 / 8 → statsDirty, others dealsDirty) | `nb18_09_reentrancy` |
| 10 proposals (term today + 365, amount 0, cost 1; none with an existing deal, budget ≤ 5000 or ≤ 60 × cost) | `nb18_10_proposals` |
| 11 clone (swap with E0 ↔ E1, Cancelled on a missing city, Silent follows the argument) | `nb18_11_clone` |
| 12 deal record (version 5 byte-identical, second `amountUsed` kept, version 4 without the suspension date) | `nb18_12_deal_record` |
| extra: the marker height at the inner cell's centre (§6.3, NB3) | `nb18_13_marker_height_inner_cell` |

*[choice]* items the vectors depend on: `CreateConnection`'s result (`nb18_02`); unknown types and a null city
price at 0 (`nb18_03`); the two-curve switch (`nb18_05`, last part). Test 7 puts both deals in Probation so that
NoLandfill shows as a removal (status 5); in Grace or before the first strike the flag is cleared again in the same
run (§7.3 step 3).

## Coverage (traffic network §11)

| §11 item | Vector |
|---|---|
| 1 three-cell street (`conn` words, records, `GetCapacity` 100.0, `GetConnectionCount` 2) | `tn11_01_three_cell_street` |
| 2 subnetworks of test 1 (labels, counter 2, vector sizes, the empty seed at A) | `tn11_02_subnetworks` |
| 2 the last column is never a seed (C alone) | `tn11_02b_last_column_never_seed` |
| 3 lot lists and queries (L1, L2, the lot without a list) | `tn11_03_lot_lists` |
| 4 intersection reduction: Street, Road, Highway never reduced | `tn11_04a_street_intersection`, `tn11_04b_road_intersection`, `tn11_04c_highway_not_reduced` |
| 5 capacity table (`invCapTable`, Road / Highway / OneWay / Dirt capacities) | `tn11_05_capacity_table` |
| 6 turn-around via `SetConnection` (`0x4444`, the off-map west neighbour) | `tn11_06_set_connection` |
| 7 tunnel cost (mask `0x1FE`, Car / Walk / PassengerTrain costs) | `tn11_07_tunnel_cost` |
| 8 water route (depth bytes, cells 6, g 5.0, link cost, both route byte strings, different bodies) | `tn11_08_water_route` |
| 9 ferry cost encoding | `tn11_09_ferry_cost` |
| 10 switch entry cost (q = 0, q = 6, 0/0, unknown ID) | `tn11_10_switch_entry_cost` |
| 11 rule rotation (facing 1, `0x01018000`) | `tn11_11_rule_rotation` |
| 12 no random draws | `tn11_12_no_random_draws` |
| extra: transit-switch lot activation, the §4.7.4 phase, deactivation | `tn11_13_switch_lot` |
| extra: ferry terminals (activation, routing, the ferry phase, removal, the periodic pass) | `tn11_14_ferry_terminals` |
| extra: lane continuity (the last flagged occupant decides) | `tn11_15_lane_continuity` |
| extra: neighbour capacity counted per labelled entry side | `tn11_16_neighbour_capacity` |
| extra: `GetNeighborConnectionInnerCell` and `HasMapExit` (§2.7) | `tn11_17_map_edge` |
| extra: a partial rule returns from DoLotActivated; the raw-byte count of u8, u32 and scalar properties (§4.7.1 step 1, §4.7.2, TN1) | `tn11_18_switch_partial_rule` |
| extra: switch flags from all rules, the station worked example (`0x4C`), its access point (§4.7.2, TN19) | `tn11_19_switch_flags_all_rules` |
| extra: the DoPostCityInit re-read (rules replaced, mask and flags) and the access points after it (§1, §4.7.2, TN3, TN4) | `tn11_20_post_load_switches` |
| extra: the water cell is the neighbour N and an earlier one stays; type 2 never activates; counts of types ≥ 2 are 0 (§4.9.1, §4.9.5, TN10, TN18) | `tn11_21_ferry_water_cell_and_type` |
| extra: a network occupant dirties its bounding cells when the NW cell is under water, not clamped (§4.10.1, TN15) | `tn11_22_water_occupant_bounds` |
| 3 the ring in boundary order (TN7); `GetConnectedDestinationCount` with k < 0 is 0 (TN17) | `tn11_03_lot_lists` |
| 8 the float32 step constants: byte 47 → `0x4081EB85` (TN14) | `tn11_08_water_route` |

Not covered by a vector, and why:

- **The second half of test 12** (DoLotActivated's one draw on the simulator's generator, §12) belongs to
  traffic-simulator.md: the network model has no generator.
- **`GetDesiredLotInsertionPoint` (§4.14)**, the persistence record (§6) and the simulator's message bookkeeping (§3)
  are traffic-simulator.md's and not part of `fc4_core::traffic_network`.
- **`GetFerryRoutesInUse` (§4.9.5)** needs rider counters, which only the simulator's ferry trips write; the vector
  world has no way to set them, so only the empty result is reachable (unit tests cover the list shape).

## Coverage (pathfinder §11)

| §11 item | Vector |
|---|---|
| 1 open-set tie order (d, e, b, c, a) | `pf11_01_open_set_tie_order` |
| 2 straight street (ring cells, start nodes, node keys and order, the pushed-back goal, results, route, reversible) | `pf11_02_straight_street` |
| 3 compressed route of test 2 | `pf11_03_compressed_route` |
| 4 ApplyTraffic(0), style 2, carpool | `pf11_04a_apply_traffic`, `pf11_04b_apply_traffic_style2`, `pf11_04c_apply_traffic_carpool` |
| 5 starting cost `0.4f` (the bits of every g, trip length) | `pf11_05_starting_cost` |
| 6 budget (over-budget count 2, the max kept), the flag-2 fallback, the shared trip length as the next budget | `pf11_06a_budget`, `pf11_06b_nearest_on_failure`, `pf11_06c_shared_trip_length` |
| 7 the 65th rejection disables the type; 64 do not | `pf11_07a_over_budget_disable`, `pf11_07b_over_budget_64` |
| 8 boundary order | `pf11_08_boundary_order` |
| 9 standard destination order | `pf11_09_standard_destination_order` |
| 10 heuristic | `pf11_10_heuristic` |
| 11 compressed route with jumps | `pf11_11_compressed_route_jumps` |
| 12 run length limit | `pf11_12_run_length_limit` |
| 13 check-interval adaptation | `pf11_13_check_interval_adaptation` |
| 14 lifetime | `pf11_14_lifetime` |
| extra: constructor values, QueryInterface, Init / Shutdown, configuration slots (§1, §2, §4.2) | `pf11_15_constructor_and_slots` |
| extra: exit node (cost r24(f − g), key, goal off-map), neighbour connection cost for filter 3 (§10 Q1) | `pf11_16_exit_node_connection_cost` |
| extra: exit cost outside the search rect (the heuristic term) | `pf11_17_exit_cost_heuristic` |
| extra: off-map start fallback over the ferry terminals' neighbour links, at the arrival cell (§4.6, PF5) | `pf11_18_off_map_start` |
| extra: switch move, park-and-ride cell, runs split by type, reversibility through a switch | `pf11_19_park_and_ride` |
| extra: transit-switch entry cost, goal test skipped at switch cells, ApplyTraffic switch usage, per-type edges | `pf11_20_transit_switch` |
| extra: the mass-transit strike masks switch transitions (§4.4 step 6) | `pf11_21_strike` |
| extra: filter 0 in city (capacity, arrived), a stale destination cleared, the neighbour pool off-map | `pf11_22_standard_destination`, `pf11_23_stale_destination`, `pf11_24_neighbour_pool` |
| extra: tunnel move and the jump record | `pf11_25_tunnel` |
| extra: flag 4 (continue) keeps the nodes and the open set | `pf11_26_continue` |
| extra: flag 8 (park-and-ride return, §4.8.4) | `pf11_27_park_and_ride_return` |
| extra: ApplyTraffic income with the uncarpooled count and the 24-bit accumulator add | `pf11_28_income` |
| extra: off-map start fallback, two matching links: the last wins and the node is in the heap twice (§4.6, PF5) | `pf11_29_off_map_start_last_match` |

Not covered by a vector, and why:

- **Suspension (§4.11)** is wall-clock dependent; the spec says conformance runs use max work time 0 (test 13 uses a
  timer that never expires). A unit test drives the scripted timer through a suspend and resume.
- **Filter 2 (§4.10.3)** has no setter in the interface (§10 Q2); the vector world answers no occupants.
- **Subway-station transfers (§4.8.3 step 3)** are unreachable in 1.1.641 (no code sets wormhole bit 1).

## Coverage (residential §15)

| §15 item | Vector |
|---|---|
| 1 proximity, one source (RG23: the seed fills only the z = 0 edge, so every map is 1 there and 0 elsewhere; RS5) | `rs15_01_proximity_one_source` |
| 1 a key on the z = 0 edge (R$ (3, 0) = 0, R$$ unchanged; RG23) | `rs15_01c_proximity_edge_key` |
| 1 the §5.3 worked example (4 × 4, R$$ and R$$$ reuse the grid; RG23) | `rs15_01b_proximity_worked_example` |
| 2 proximity cap (200 at x = 200, 255 at x = 280) | `rs15_02_proximity_cap` |
| 3 proximity, coarser target (RG23: tract (1, 1) overwrites (1, 0)'s 0; (0, 0) = (1, 0) = 1, (0, 1) = (1, 1) = 0) | `rs15_03_proximity_coarser_target` |
| 4 decay (10 → 9.8, 0.1 → 0) | `rs15_04_decay` |
| 5 newcomers' age (35) | `rs15_05_06_newcomers_age` |
| 6 age quirk (71) | `rs15_05_06_newcomers_age` |
| 7 December (41, 255 → 0, total 0 → 0; RS2) | `rs15_07_december` |
| 8 generational reset (RS1: `0x421FFFFF`, `0x423FFFFF`, ageEqSum 7800; RS7: the shipped LE curve, LE 65) | `rs15_08_generational_reset` |
| 9 buckets | `rs15_09_buckets` |
| 10 age groups (float32 rounding of n) | `rs15_10_age_groups` |
| 11 average LE (71) | `rs15_11_average_life_expectancy` |
| 12 per mille (half to even, the series order) | `rs15_12_per_mille` |
| 13 quotient wrap (156.810) | `rs15_13_quotient_wrap` |
| 14 pollution byte (44 → 0.2085938; RS3) | `rs15_14_pollution_byte` |
| 15 life expectancy (75, 55) | `rs15_15_life_expectancy` |
| 16 workforce (0.5 through the hidden pointer, ≈ 0.444) | `rs15_16_workforce` |
| 17 new age (35, 25 without the exemplar) | `rs15_17_new_age` |
| 18 NewMonth | `rs15_18_new_month` |
| 19 NewDay unfinished (work budget, resume, same result) | `rs15_19_new_day_unfinished` |
| 20 `Read` versions (2, 3, 4–6, 7, any version, truncation) | `rs15_20_read_versions` |
| 21 `Init` after load (strike flag cleared) | `rs15_21_init_after_load` |
| 22 slot ABI (slots 10, 21, 23–25) | `rs15_22_slot_abi` |
| 23 buzzards draws (1.5 → one draw, 2.0 → none, pop 0 → none) | `rs15_23_buzzards` |
| 24 percentage getter (RS1) | `rs15_24_percentage_getter` |
| extra: `Init` / `Shutdown` call order, new-city sizes, grid instances | `rs00_lifecycle` |
| extra: `Init` failures, the held message server, `AddRef` / `Release`, `QueryInterface`, class ID | `rs00_identity_and_init_failures` |
| extra: the state order (§5.1), state 10 and unknown states wait | `rs00_state_order` |
| extra: state-0 list distribution, slots 46–49 and 61 (REH §4.1) | `rs00_state0_lists` |
| extra: occupant messages (§4.3) | `rs00_occupant_messages` |
| extra: slots 5, 17/18, 19, 37/38, 50–57 | `rs00_small_slots` |
| extra: a month end to end | `rs00_month_end_to_end` |
| extra: RS2 (the month read per tract) | `rs00_rs2_month_per_tract` |
| extra: the record layout (452 bytes, the state at offset 230, the dirty byte, list sizes) and a byte-exact round trip | `rs00_record` |
| extra: version < 5 / < 6 block conversions (RS1); `Read` releases what it replaces | `rs00_record_old_versions` |
| REH §11 | see the next section |

Not covered by a vector, and why:

- **The 5 ms CPU-cycle slicer (§5.2)** is replaced by a work budget ([choice]); test 19 pins the budget's resume
  behaviour, not the clock.
- **RS5** has no vector of its own: a byte-wide running distance would wrap 255 + 1 to 0 and put zeros on
  `rs15_01`'s z = 0 edge, where the maps hold 1; the single-row example of RS5 is a `fc4-core` unit test (`rs5_distance_does_not_wrap`).

## Coverage (residential education and health §11)

Every REH §11 test is also a `fc4-core` unit test (`residential::tests::reh01_…` … `reh24_…`).

| §11 test | Vector |
|---|---|
| 1 coverage units (3, 6; saturation 255; the low-16-bit sum 0x10000 → 0 [quirk]) | `reh11_01_coverage_units` |
| 2 units 0 still serve, once per building [quirk]; a tract a lower group covers is not counted | `reh11_02_units_zero_still_serve` |
| 3 the tract list (x-major, the lower clamp, the unsigned upper compare) | `reh11_03_tract_list` |
| 4 staff ratio and grade (0.8; 1.0 at served 0 and 500) | `reh11_04_staff_ratio_and_grade` |
| 5 the grade-grid blend (0.5; 2 × 0.5 + 0.5 clamped to 1.0 [quirk]) | `reh11_05_grade_grid_blend` |
| 6 served rounded half to even (2, 4, 2) | `reh11_06_served_rounding` |
| 7 the strike roll (49.9 < 50 starts, 50.0 does not; one draw per eligible building) | `reh11_07_strike_roll` |
| 8 the strike end (501 > 500 days; staff ratio 0.9 ≥ min grade [quirk]; 0.89 continues) | `reh11_08_strike_end`, `rs00_reh08_09_strikes` |
| 9 chance of strike (0.75; 1.0 on strike; the reused c and g [quirk]: 0.9375, 0.5) | `reh11_09_chance_of_strike`, `rs00_reh08_09_strikes` |
| 10 system rating (0.75, 1.0, 0; the stale query record [quirk]: 0.5) | `reh11_10_system_rating` |
| 11 per mille (247; population 0 → the hospital series first) | `reh11_11_history_per_mille`, `rs15_12_per_mille` |
| 12 tract quotient (`0x42855555`, average grid 66) | `reh11_12_tract_quotient`, `rs00_reh12_22_quotients` |
| 13 life expectancy (57, 75, 50) | `reh11_13_life_expectancy` |
| 14 EQ decay (`0x411CCCCD`, 0) | `reh11_14_eq_decay` |
| 15 the HQ pollution byte (0.2171875, 0.2 [quirk]) | `reh11_15_hq_pollution_byte` |
| 16 the newcomer blend (45, `0x40D55555`) | `reh11_16_newcomer_blend`, `rs00_reh16_17_newcomers_and_reset` |
| 17 the generational reset (RS1: `0x421FFFFF`, `0x41BFFFFF`) | `reh11_17_generational_reset`, `rs00_reh16_17_newcomers_and_reset` |
| 18 ideal funding (0.5; the max `0x3F999999`; not graded / no data → null) | `reh11_18_ideal_funding` |
| 19 the ordinance m3 (unchanged until state 0, then 1.1 = `0x3F8CCCCD`) | `reh11_19_ordinance` |
| 20 the capacity estimate ((1000, 34); the school divisor for hospitals [quirk]: (1000, 40); (500, 20)) | `reh11_20_capacity_estimate` |
| 21 query data (per staff 33 / 34 school ceil, 33 hospital floor [quirk]; slot 14 totals) | `reh11_21_query_data` |
| 22 min / max coordinates ((10, 14) and (22, 6) on 32 × 32) | `reh11_22_min_max_coordinates`, `rs00_reh12_22_quotients` |
| 23 death's door (draws, height 60, one effect per tract) | `reh11_23_deaths_door`, `rs00_reh23_deaths_door` |
| 24 slot rows (34, 10–14, 56, 57) | `reh11_24_slot_abi`, `rs15_22_slot_abi` |
| extra: §4.1 building registration (the line change, the line item and its count, locally funded purposes, references; an unknown department) | `rs00_reh41_building_registration` |
| extra: §4.2 funding percentages and efficiency (slots 19 / 20; the strike efficiency; question 1) | `rs00_reh42_funding_and_efficiency` |
| extra: §4.3 the effect lists (an element per service and ordinance with any of its six properties; a curve alone counts; Init and state 0), m1 / m2 / m3, the decay products in the population tuning, an ordinance age curve and m2 in grading | `rs00_reh43_ordinance_effects` |
| extra: §4.4 / §4.5 a building without a coverage radius (skipped; served, effective capacity and grade 0; the staff ratio kept) | `rs00_reh45_missing_radius` |
| extra: §4.6 the EQ boost through one grading (the distance curve at the 0..1 fraction; the z centre an empty tract does not advance [quirk]; GetPopulationServed's tracts-over-metres distance [quirk]) | `rs00_reh46_radius_effect` |
| extra: §4.10 the auto-budget step (before the grading; the building's or the global flag; not graded; the max; a hospital without a vehicle item) | `rs00_reh410_auto_budget` |
| extra: §4.11 slot 45's coverage geometry (the distance curve walk [quirk]; question 7) | `rs00_reh411_occupant_coverage` |
| extra: state-0 list distribution and slots 46–49, 61 (§4.1) | `rs00_state0_lists` |
| RS7 (the shipped LE curve) | `rs15_08_generational_reset` |
| RS8 (the health-affecting list) | `rs00_rs8_health_affecting`, `rs00_state0_lists` |
| RS9, RS11, RS14, RS15 (a damaged record: Read false, the simulator as constructed, references released) | `rs00_rs11_14_15_damaged_records`, `rs15_20_read_versions` |
| RS10 (replaced references released after the commit) | `rs15_20_read_versions`, `rs00_record_old_versions` |
| RS12 (null list entries dropped by Read) | `rs00_record` |
| RS13 (grade totals reset by state 0, never saved) | `rs00_rs13_grade_totals` |
| RS16 (the unsigned 64-bit LE division, the `≠ 0` guard) | `rs00_rs16_negative_population` |
| RS17 (version 3 keeps the running population; residential 9.3's example) | `rs00_rs17_version3_running_population` |

Not covered by a vector, and why:

- **Test 24's calling-convention details** (slot 34 leaves the x87 stack unchanged; slots 10–13 return the hidden
  pointer) are properties of the exported thunks, which this crate does not build; the vectors check the slot rows
  (name, bytes popped, return channel) that the export must match.

## Coverage (traffic simulator §11)

| §11 item | Vector |
|---|---|
| 1 cycle order, with and without the subnetwork rebuild; the days of `DoDailySimulation` | `ts11_01_cycle_order` |
| 2 day pacing (N = 3, N = 1, `toReRoute` 17) | `ts11_02a_day_pacing_slices_of_3`, `ts11_02b_day_pacing_slices_of_1`, `ts11_02c_reroute_days` |
| 3 congestion scale (the inverted points, `0x42DA924A`) | `ts11_03_congestion_scale` |
| 4 congestion map | `ts11_04_congestion_map` |
| 5 air-polluting map | `ts11_05_air_polluting_map` |
| 6 trip-length map, a lot with a failed trip | `ts11_06_trip_length_map` |
| 7 strategy percentages, the small-lot thresholds | `ts11_07_strategy_percentages` |
| 8 large lot split | `ts11_08_large_lot_split` |
| 9 carpool | `ts11_09_carpool` |
| 10 hand-checkable network: mode 0, edge densities, arrived, income, CreateMaps, CreateFinalMap, CompleteTripSourceCalculations, FinalCalculations, the history; mode 2 | `ts11_10a_hand_checkable_network`, `ts11_10b_hand_checkable_network_mode_2` |
| 11 background traffic | `ts11_11_background_traffic` |
| 12 seeded draws: the re-draw, the continuation, lot activation, seed 12345, `RandomDoubleUniform`, `CheckRailAccident` | `ts11_12_seeded_draws` |
| 13 capacity | `ts11_13_capacity` |
| 14 neighbour reconcile and SetupTrip | `ts11_14_neighbour_reconcile` |
| 15 save round trip, the header, the trip record | `ts11_15_save_round_trip` |
| extra: version-1 trip record, the long vector, a short record (§6.3) | `ts11_16_trip_record_version_1` |
| extra: `Read` version gate, truncation, unchanged on failure (§6.4) | `ts11_17_read_gates` |
| extra: legacy edge densities (§6.4) | `ts11_18_legacy_edge_density` |
| extra: lot reconcile, the probe, freight and commuter counts (§4.1.1) | `ts11_19_lot_reconcile` |
| extra: lot activation stores the destination bits (TN1), free destinations, the bucket; deactivation; the no-building stop (§4.1.3) | `ts11_20_lot_activation` |
| extra: funding damage target and potholes (§4.12 step 1) | `ts11_21_funding_damage_and_potholes` |
| extra: strike transitions and messages (§4.12 step 2) | `ts11_22_strike_transitions` |
| extra: the accident check needs a view (§4.13) | `ts11_23_accident_check` |
| extra: day-1 incomes (§3.3) | `ts11_24_day_one_income` |
| extra: Init (agents, subscriptions, departments), identity slots, Shutdown (§1, §7) | `ts11_25_init_and_identity` |
| extra: DumpEdgeDensities, ReduceEdgeDensities, `GetTravelTimeRatio` (§4.8) | `ts11_26_edge_dump_and_reduce` |
| extra: short and missing tuning arrays (TS28), the funding percents (TS29), the strategy bytes (§4.14, §4.1.2) | `ts11_27_tuning_reads` |
| extra: entering 18 keeps pass counter 1 (TS35, §3.3) | `ts11_47_pass_counter_1` |
| extra: the s16 phase positions: zeroed on entering 16–18, the taken source's cell, the 16-bit resume, FinishTripMap's lot location (TS44, TS45) | `ts11_48_phase_positions` |
| extra: the state-2 statistics reset, the −1.0 comparison values, a NaN record (TS40, §4.9) | `ts11_49_statistics_reset` |
| extra: the station cost rounded once above 2^24 (TS46, §4.16) | `ts11_50_station_cost_rounds_once` |
| extra: the ConfirmTrips merge (a routed trip goes back to 0 only when it absorbed one) and pass counters 3, 4, 5, 7, 8 (TS55, TS59, §4.3) | `ts11_51_confirm_trips_merge_counters` |
| extra: the re-route context cell written with the phase positions (TS60, §6.1 row 14) | `ts11_52_context_cell` |
| extra: entering 13 resets the scanning iterator; done at (x0, z1 + 1) after it (TS64, traffic-network §4.12) | `ts11_53_subnetwork_info_scan` |

Not covered by a vector, and why:

- **Path searches over a real network (states 17 and 18)** need network occupants with path info; the vector world
  has none (the pathfinder's own vectors pin the search). Test 10 drives `ApplyRoute` with the spec's route bytes.
- **Transit switches, tunnels and ferries in the record** round-trip only as empty lists here; Read's switch mask and
  flags (`read_mask_and_flags`) are a `fc4-core` unit test.
- **The work budget (§3.4)** replaces the original's CPU-time slicing *[choice]*; test 2 pins its pacing, not a clock.

## Coverage (history §12)

| §12 item | Vector |
|---|---|
| 1 Init and Shutdown: the subscriptions, the day and year agents, QueryInterface | `hw12_01_init` |
| 2 RecordHistoryData and Store: the example of §4.8, an earlier day, a new key, a full queue, no delivery before Init | `hw12_02_store` |
| 3 SetSize and the queue-length message | `hw12_03_set_size` |
| 4 Coalesce: the four granularities, the month-only grouping, the failures | `hw12_04_coalesce` |
| 5 the thinning pass: the order, `perPass`, the timer, the state machine | `hw12_05_thinning_pass` |
| 5 VariableCoalesce: the five ranges | `hw12_05b_variable_coalesce` |
| 5 26 yearly passes over a daily series (the shape of the Konradshohe series, §4.8) | `hw12_05c_yearly_passes` |
| 6 Dispose (the kept last point) | `hw12_06_dispose` |
| 7 GetPoint and GetCount: the example of §4.8, the edges, interpolation | `hw12_07_get_point` |
| 8 GetTrend at 24 and 53 bits (bit patterns) | `hw12_08_get_trend` |
| 9 GetMinMaxAverage: the example of §4.8, the end points | `hw12_09_get_min_max_average` |
| 10 the record: the round trip, version 1, version 3, `0xCDCDCDCD`, duplicates, truncation | `hw12_10_save` |

Not covered by a vector, and why:

- **The 421 corpus saves of test 10** are not in the repository; `hw12_10_save` and the `fc4-core` unit test
  `hw12_10_corpus_shaped_v1_record` use records of the corpus's shape instead.

## Coverage (clock24 §12)

| §12 item | Vector |
|---|---|
| 1 construction, QueryInterface, Init and Shutdown | `ck12_01_construction` |
| 2 the tuning (§6) | `ck12_02_tuning` |
| 3 the step threshold, the clamp, the pauses, the cadence of §4.9 | `ck12_03_step_threshold` |
| 4 an hour step, the evening of §4.9, the wrap | `ck12_04_hour_step` |
| 5 the 5-minute tick | `ck12_05_five_minute_tick` |
| 6 the night boundary, notifications off, the night hours | `ck12_06_night_boundary` |
| 7 the visual time modes | `ck12_07_modes` |
| 8 the pause counters and the timer | `ck12_08_pause_counters` |
| 9 the setters | `ck12_09_setters` |
| 10 EnableDayNightTransitionNotifications | `ck12_10_notifications` |
| 11 the cheats | `ck12_11_cheats` |
| 12 the 26-byte record, version 1, version 3, truncation | `ck12_12_save` |
| 13 Read before Init, the first step, a saved pause | `ck12_13_load_order` |

Not covered by a vector, and why:

- **The 421 corpus records of test 12** are not in the repository; the `fc4-core` unit tests read records of the
  corpus's shape instead.
- **`GetTimePercentage`** uses `1.1574074e-05f` (`0x37422E45`, 1/86400), not the spec's `0x37C22E45` (1/43200,
  which gives 1.0 at noon) *[choice]*; `ck12_01_construction` pins 0.5 at noon.

## Coverage (commercial and industrial §13)

| §13 item | Vector |
|---|---|
| 1 identity of both classes | `ci13_01_identity` |
| 2 commercial Init: a missing node, both nodes (the grids, agents and subscriptions); Shutdown | `ci13_02_init` |
| 3 dirty marking (purpose 3, purpose 1, a network, a preview occupant) | `ci13_03_dirty_marking` |
| 4 state machine (0, 2, 3; the maps change on the second and third calls; clean state 3) | `ci13_04_state_machine` |
| 5 map content (the edge-only seed, RG23) | `ci13_05_map_content` |
| 6 loaded grids (the rebuild runs in place) | `ci13_06_loaded_grids` |
| 7 `GetProximityMap` | `ci13_07_proximity_map` |
| 8 save (the flag, version 2 rejected, the rebuild after a flag-1 read; the industrial record) | `ci13_08_save` |
| 9 corpus: synthetic records of the shapes §8.3 reports | `ci13_09_corpus_shapes` |

Not covered by a vector: the 421-save corpus itself (item 9) is not available to this suite; `ci13_09` replays the
record shapes the spec reports.

## Coverage (civic buildings §12)

| §12 item | Vector |
|---|---|
| 1 Init (the three departments, Landmarks held, subscriptions, the agent, Lua), a loaded Landmarks reused, Shutdown, Release | `cb12_01_init` |
| 2 exclusion (insert, remove, a non-building occupant; no preview test; one of two equal IDs removed) | `cb12_02_exclusion` |
| 3 counts (insert, remove, the 16-bit wrap; no entry; `0xEA3209F8 = false`; no key) | `cb12_03_counts` |
| 4 activation (data2 2 and 3, the line, +30 / +60, a lot without a building, a missing department) | `cb12_04_activation` |
| 5 income (the line keyed by the building type, type 1, removal deletes it; 500 → 200; an existing line keeps its type) | `cb12_05_income` |
| 6 monthly condition (0.49, 0.5, NaN; condition 1 stays) | `cb12_06_monthly_condition` |
| 7 ape escape (the temporary generator's seed, the float32 threshold, the index draw, no controller manager, a null entry, no centre) | `cb12_07_ape_escape` |
| 8 status API (update, get, unknown key, a write through the pointer) | `cb12_08_status_api` |
| 9 save (the `mod 193` order, byte-identical, version 5 rejected, a cut record, a read over a running simulator) | `cb12_09_save` |
| 10 corpus: synthetic records of the §7.1 shapes, colliding keys, the list-count limit | `cb12_10_corpus_shapes` |

Not covered by a vector: the 421-save corpus itself (item 10) is not available to this suite; `cb12_10` replays the
shapes of the §7.1 table.

## Coverage (parks §12)

| §12 item | Vector |
|---|---|
| 1 Init (the department only when missing, a department that cannot be created, missing services), Shutdown, Release | `pk12_01_init` |
| 2 tuning (shipped, without the curve, none, custom, read once per object) | `pk12_02_tuning` |
| 3 insert (the line, display flag 2, 10 / 1, 20 / 2, a preview occupant; zoos; the failed building interface; a missing department; a line that cannot be created) | `pk12_03_insert` |
| 4 remove (15 / 3 → 0 / 0, the line stays, a duplicate removed with one release; zoos) | `pk12_04_remove` |
| 5 distress latch (0.25, 0.2, 0.3, NaN; no zoo: no draw; no park: nothing; unreadable funding) | `pk12_05_distress_latch` |
| 6 shipped threshold (PK1: run down at the first pass, never restored below 2500 %, again after a reload) | `pk12_06_shipped_threshold` |
| 7 escape draw (`p` bit for bit, 0.07 escapes, 0.0700001 does not, the message and `zoomanji`, the index draw, a null entry) | `pk12_07_escape_draw` |
| 8 save (byte-identical, versions 1 and 3 rejected, a cut record, unresolvable references, the list-count limit; the corpus shapes: empty, 1, 3, 6 and 9 parks) | `pk12_08_save` |

Not covered by a vector: the 421-save corpus itself (part of item 8) is not available to this suite; `pk12_08`
replays the record shapes §7 reports.

## Coverage (network manager §16)

| §16 item | Vector |
|---|---|
| 1 component 53 empty (version 2 rejected after 2 bytes, a stream in error, a null reference (NM6), an unresolvable one) | `nm16_01_component_53_empty` |
| 2 component 11 empty (version 1 rejected, nothing else read) | `nm16_02_component_11_empty` |
| 3 managed list order (swap-and-pop, the record C then B, byte-identical round trip, Read appends) | `nm16_03_managed_list_order` |
| 4 duplicate insert | `nm16_04_duplicate_insert` |
| 5 not managed (a plain road, a non-network occupant, no occupant) | `nm16_05_not_managed` |
| 6 lighting (dusk, dawn, PostCityInit by day and at night) | `nm16_06_lighting` |
| 7 GetNetworkTool (Init's subscriptions and tools, type 13, cached, unique, PreCityShutdown) | `nm16_07_get_network_tool` |
| 8 QueueRemovals dedupe (creation order, the references kept) | `nm16_08_queue_removals_dedupe` |
| 9 groups (road, pipe, subway in that order; power lines ignored; null tools skipped) | `nm16_09_groups` |
| 10 terrain rect (clamping, the long-operation merge, LongTerrainEnd's refit) | `nm16_10_terrain_rect` |
| 11 corner rule (0.005, 0.02, −0.001, kind 4, NaN, a pipe with kinds 1 and 3, kind 5, kind 6) | `nm16_11_corner_rule` |
| 12 riot lock (the set, two riots, the riot manager's Shutdown) | `nm16_12_riot_lock` |
| 13 riot edge (x < 3, z < 3, the (3, 3) boundary) | `nm16_13_riot_edge` |
| 14 zone for network (priorities, the diagonals, a zoned cell, ties, the city edge) | `nm16_14_zone_for_network` |
| 15 wealth for network (tie up; 1, 1, 2; a diagonal; none; out of range; the building profile) | `nm16_15_wealth_for_network` |
| 16 compatibility pattern (rows and columns, no zone list, slope, rotation, flip rule, wealth) | `nm16_16_compatibility_pattern` |
| 17 object position (rotations 0–3 and 4, flipped) | `nm16_17_object_position` |
| 18 choose and keep (one lot draw, the object draws, keeping, rezoning, usable off) | `nm16_18_choose_and_keep` |
| 19 forced configuration (locked, never replaced; connection nodes and underground pieces) | `nm16_19_forced_configuration` |
| 20 road style (street, rail, agriculture, skipped pieces, the tables) | `nm16_20_road_style` |
| 21 pothole (the W dead end, repeat, rail, no network, repair, nothing to repair) | `nm16_21_pothole` |
| 22 usable message (in and out of the city; translucency) | `nm16_22_usable_message` |
| 23 lot state ring (the 14 border cells, processed in key order, the city edge) | `nm16_23_lot_state_ring` |
| (§3.2, §4.2, §8.6, §17) the component 11 record with a lot and a pothole, Init resolving and dropping lots, RedevelopLot's draws, Shutdown | `nm00_01_lot_record_and_lifecycle` |

Not covered by a vector: the presentation slots 13–15, `CreateConnectionNode`, `AutoconnectLotNodes` and
`GetAllIntersectionPieces`, which are not in the core; the default generator's values (`random: "default"` is
available, but no vector pins its draws).

## Coverage (network rules §15)

| §15 item | Vector |
|---|---|
| 1 edge tokens | `nr15_01_edge_tokens` |
| 2 one rule | `nr15_02_one_rule` |
| 3 rejected header (the evaluator still joins the type's list) | `nr15_03_rejected_header` |
| 4 bad centre (rules read before it stay) | `nr15_04_bad_centre` |
| 5 Transmogrify, straight (6 variants dropped) | `nr15_05_transmogrify_straight` |
| 6 Transmogrify, diagonal (`noflip`, `notrans`) | `nr15_06_transmogrify_diagonal` |
| 7 mirror of a subtype | `nr15_07_mirror_of_a_subtype` |
| 8 rotation tables | `nr15_08_rotation_tables` |
| 9 conditions and wildcards | `nr15_09_conditions_and_wildcards` |
| 10 valid positions | `nr15_10_valid_positions` |
| 11 claims (bucket order, skipped outputs) | `nr15_11_claims` |
| 12 two types crossing (with and without the RUL1 entry, through `SolveNetwork`) | `nr15_12_two_types_crossing` |
| 13 covered, with override | `nr15_13_covered_with_override` |
| 14 three types | `nr15_14_three_types` |
| 15 scheme 2 (a fixed cell fails) | `nr15_15_scheme_2` |
| 16 scheme 1 | `nr15_16_scheme_1` |
| 17 PatchTilePair (hard conflict, resolve mode, quirk 5) | `nr15_17_patch_tile_pair` |
| 18 rotate bounds (quirk 8) | `nr15_18_rotate_bounds` |
| 19 layout and centre (the +1 shift) | `nr15_19_layout_and_centre` |
| 20 Test | `nr15_20_test` |
| 21 bridge cell as two types (quirk 4) | `nr15_21_bridge_cell_as_two_types` |
| 22 OneWayDir | `nr15_22_one_way_dir` |
| 23 IdentifyHighwayIntersections (returns `v`) | `nr15_23_identify_highway_intersections` |
| 24 TileVerifiers | `nr15_24_tile_verifiers` |
| the shipped rule files | not a vector: `cargo test -p fc4-formats --release --test network_rules -- --nocapture` |

The core's unit tests (`fc4_core::network_rules`) repeat the 24 items and also cover the INI reader, the `sscanf`
subset, Transmogrify moving conditions and outputs, connection-node qualification (quirk 3), the padded set, the
unsolved-cell result, the check-type grammar, CopyFrom / Transpose / Translate, the ordering rings, DoAutoComplete
and the reload order; `fc4_capi`'s `network_rules_tests` cover the C ABI.

## Coverage (city detail §8)

| §8 item | Vector |
|---|---|
| 1 new city record (default preference, preference 1) | `cd8_01_new_city` |
| 2 version 1 rejected | `cd8_02_record` |
| 3 same level: nothing changes | `cd8_03_same_level` |
| 4 reduce (unlisted tags removed, untagged and listed stay) | `cd8_04_reduce` |
| 5 increase (one prop per missing required object, at the developer's placement; optional skipped; one draw each) | `cd8_05_increase`, `cd8_06_session_generator` |
| 6 network lots redeveloped once in both directions, none in a new city | `cd8_01_new_city`, `cd8_04_reduce`, `cd8_05_increase` |
| CD4: the presence test counts flora and prop occupants by tag alone; step 4 draws for any object type | `cd8_08_presence_and_object_types` |
| CD5: the pass count n is per manager object and starts at 0 | `cd8_06_session_generator`, `cd8_07_pass_count_per_manager` |

## Coverage (port developers §12)

| §12 item | Vector |
|---|---|
| 1 empty records, versions 1 and 3 rejected | `pd12_01_empty_records` |
| 2 round trip of the §7 example, the counts | `pd12_02_round_trip` |
| 3 adoption (phase-2 lot and preview ignored) | `pd12_03_adoption` |
| 4 month with one small airport | `pd12_04_month` |
| 5 two airports in one subnetwork | `pd12_05_two_airports` |
| 6 the advisor event and condition 2 | `pd12_06_advice` |
| 7 growth to the next stage | `pd12_07_growth`, `pd12_07b_growth_failures` |
| 8 a destroyed port lot | `pd12_08_removal` |
| 9 the seaport map | `pd12_09_seaport_map`, `pd12_11_seaport_tick` |
| 10 no ports | `pd12_10_no_ports` |
| 11 funding scales only the public query (PD8) | `pd12_16_funding_month` |
| 12 a missing next-stage configuration (PD5) | `pd12_17_missing_configuration` |
| 13 `Init` guarded, `Shutdown` empties the ports and types (PD10) | `pd12_18_init_shutdown` |
| other sections | `pd12_12_funding`, `pd12_13_queries`, `pd12_14_tuning`, `pd12_15_reference_count`, `pd12_19_terrain_grid` (§4.8 grid, PD4, PD6) |

## Coverage (prop developer §12)

| §12 item | Vector |
|---|---|
| 1 empty record (127 bytes), Init, identity | `pv12_01_empty_record` |
| 2 template read (batch `0xBAADF00D` kept, written as version 5; version 3 rejected) | `pv12_02_template_read` |
| 3 round trip (the §7 example; every list filled) | `pv12_03_round_trip` |
| 4 block order | `pv12_04_block_order` |
| 5, 6 worked examples A, B, C (and E) | `pv12_05_example_a_b_c`, `pv12_06_example_e` |
| 7 split (example D) | `pv12_07_split` |
| 8 no lots | `pv12_08_no_lots` |
| 9 wealth selection | `pv12_09_wealth_selection` |
| 10 night sweep (and switch all) | `pv12_10_night_sweep`, `pv12_11_switch_all` |
| 11 time of day | `pv12_12_time_of_day` |
| 12 registration (and triggers, power, unregistering) | `pv12_13_registration` |
| 13 signs | `pv12_14_signs` |
| 14 lot state | `pv12_15_lot_state` |
| other sections | `pv12_16_cell_walk_quirks`, `pv12_17_requesters`, `pv12_18_scanner_state_byte` (§7, PV13), `pv12_19_unregister_tests` (§4.7, PV12) |

Not covered by a vector: the 421-save corpus itself (item 3) is not available to this suite; `pv12_02` and `pv12_03`
replay the records §7 quotes.

## Coverage (demolition §12)

| §12 item | Vector |
|---|---|
| 1 record (`01 00`; version 2 and a cut record rejected) | `dm12_01_record` |
| 2 empty list (and lists of null lots, §12 item 15): true, cost untouched | `dm12_02_empty_list` |
| 3 building price 1234, nothing removed; no building development simulator | `dm12_03_building_price` |
| 4 destroyed 3 × 2 lot: 12, dry run and committed | `dm12_04_destroyed_lot` |
| 5 not demolishable: true, 0, untouched | `dm12_05_not_demolishable` |
| 6 blocked lot (player lock, privilege 1), the other lot still demolished; the system privilege; a system-locked prop | `dm12_06_blocked_lot` |
| 7 re-parcel into four 1 × 1 lots; `0x801` keeps the lot in state 1; §4.10 example 4 | `dm12_07_reparcel` |
| 8 dezone, and zones 11, 12, 15 | `dm12_08_dezone` |
| 9 explode `0x303` / `0x1303`; no water configuration; water-compatible | `dm12_09_explode` |
| 10 power pole: `0xAA7A014D` once before the removal, the orphan in the same call; dry run; a caller list; no pole interface | `dm12_10_power_pole` |
| 11 three networks in one `DestroyNetworkOccupants` inside the `QueueRemovals` bracket; directly; a network-type occupant without the network interface is still queued (DM9) | `dm12_11_networks` |
| 12 per-vertex terrain (building box, locked lot, water, tolerance, exempt lot, movable building, the redisplay rectangle); area mode | `dm12_12_terrain` |
| 13 effects: none with cause 0, none in a dry run; the occupant and group effects | `dm12_13_effects` |
| 14 the `float` tolerance: exactly 0.05f not counted in area mode, the next `f32` above counted and set, the one below skipped (DM6) | `dm12_14_tolerance` |
| extra: identity, Init / Shutdown, Release, the tuning and ExemplarChanged | `dm00_lifecycle` |
| extra: the deferred lot demolition message | `dm00_deferred` |

## Coverage (dispatch §12)

| §12 item | Vector |
|---|---|
| 1 empty record (22 bytes) | `dp12_01_empty_record` |
| 2 the corpus forms round-trip (toolType 4, 5, 7, 8); Init sets toolType 8 | `dp12_02_corpus_round_trip` |
| 3 versions 1 and 3 fail with `loaded` set; a cut record | `dp12_03_wrong_versions` |
| 4 Dispatch: no source, a free unit (the record, begin once), CanDispatch false, all busy, a failed vehicle | `dp12_04_dispatch` |
| 5 a failed InitDispatch advances nextID | `dp12_05_failed_init_dispatch` |
| 6 automaton events 7 (5, 5, 3, 0) and 3 | `dp12_06_state_events` |
| 7 RevokeDispatchByID of an active record: end only, the unit back; no source | `dp12_07_revoke_active` |
| 8 RevokeAllDispatches(1) removes type-0 records | `dp12_08_revoke_all` |
| 9 RemoveOccupant of a station with two records (the vehicles released, DP7) | `dp12_09_station_removed` |
| 10 the tool with mask `0b101` | `dp12_10_tool` |
| extra: identity, lifecycle, sources | `dp00_lifecycle` |
| extra: the §4.7 walk-through and its record | `dp00_worked_example` |
| extra: DisasterPeriodEnded, ViewChanged, re-targeting a dragged pylon, dropping it on its station | `dp00_messages` |
| extra: new vehicles tracked by occupant group, a re-target not tracked again (DP9); a failed return's unit state, `RevokeDispatchByPylon` without a source (DP8); a removed station's active available pylon (DP7) | `dp00_answers15` |

## Coverage (city situation §12)

| §12 item | Vector |
|---|---|
| 1 record round trip (the three corpus payloads; version 3; version 1) | `cs12_01_record` |
| 2 duplicate GUID keeps the first count | `cs12_02_duplicate_guid` |
| 3 profile rewards overwrite a loaded count with 1 | `cs12_03_profile_rewards` |
| 4 idle tick: 10,000 ticks, nothing posted, drawn or changed | `cs12_04_idle_tick` |
| 5 success payout (deposit, aura, count, one popup post) | `cs12_05_success_payout` |
| 6 failure penalty refused by the budget, the popup still posted | `cs12_06_failure_no_money` |
| 7 no city end | `cs12_07_no_city_end` |
| 9 time limit: 0.5 s never fails by time; 40 s: the warning on the first tick whose pre-increment `elapsed` is ≥ 10.0f, status 2 when `elapsed` reaches 40.0f (CS4) | `cs12_09_time_limit` |
| 10 `DeactivateSituation` vs `EndSituation` during the countdown (CS6) | `cs12_10_deactivate_vs_end` |
| extra: identity, Init / Shutdown, Release | `cs00_lifecycle` |
| extra: initiate, activate, the countdown, the 30-second warning, the time limit, deactivate | `cs00_mission_clock` |
| extra: triggered / untriggered situations, max CSI, `HandleInsertAutomaton` (CS13), removed and disabled vehicles, PostCityInit | `cs00_tracking` |
| extra: the popup and its buttons, could-not-start, by GUID, the cheat, Lua, ResetTutorials, `EndSituation` | `cs00_popup` |
| extra: the reward list, saturation, the §8 getters | `cs00_reward_and_getters` |
| extra: slot 22 `InitiateSituationForOccupant(occupant, advice)` (§8, CS2) | `cs00_slot22` |
| extra: `InitiateSituation`'s results (CS5); the outcome positions (CS9); disabled automata and targets (CS10); the track list by priority (CS7); slots 21, 23–25 (CS11) | `cs00_answers15` |

Not covered by a vector: item 8 (slots 13–15 return in ST0) is an ABI property of the C++ shim; the C ABI's
`fc4_citysit_*` float getters return `float` and are checked by the capi tests.

## Coverage (disasters §18)

| §18 item | Vector |
|---|---|
| 1 coverage threshold (D9 50, 51, 0; a forced ignition skips the test) | `ds18_01_coverage_threshold` |
| 2 stage lookup (the shipped thresholds, the defaults, a short D3) | `ds18_02_stage_lookup` |
| 3 unattended burn-down (29, 30, 55 updates, the demolition, a forced ignition at the 25th, maxStage below 2) | `ds18_03_unattended_burn_down` |
| 4 hosed fire (the draws, the extinguish delay, no demolition) | `ds18_04_hosed_fire` |
| 5 episode pause (begin once, end once with the history) | `ds18_05_episode_pause` |
| 6 nested enable counter | `ds18_06_nested_enable_counter` |
| 7 riot timing (30, 90 and 180 s; coverage 255) | `ds18_07_riot_timing` |
| 8 riot leader limit (112 and 113; three riots; the road search) | `ds18_08_riot_leader_limit` |
| 9 toxic spill (unattended 120 updates, one hose 30) | `ds18_09_toxic_spill` |
| 10 dispatch effectiveness | `ds18_10_dispatch_effectiveness` |
| other sections | `ds00_layer_and_locations`, `ds00_ignite_cell`, `ds00_short_form`, `ds00_riot_lock` (network-manager §10) |

The core's unit tests (`fc4_core::disasters`) also cover the lot conditions, the spread, the toast list, the damage
assessment, re-entrant occupant removal and the default generators.

## Coverage (prop-lotconfig §T)

| §T item | Vector |
|---|---|
| 1 profile defaults (the default box, OccupantSize) | `plt_01_profile_defaults` |
| 2 unknown profile (slot 13 inserts, quirk 2) | `plt_02_unknown_profile` |
| 3 families (load order, no duplicates, quirk 1) | `plt_03_families` |
| 4 GetLotProps boundary (quirk 7) | `plt_04_get_lot_props_boundary` |
| 5 street props (quirk 4) | `plt_05_street_props` |
| 6 building props (the property vector) | `plt_06_building_props` |
| 7 detail filter (CD1) | `plt_07_detail_filter` |
| 8 rebuild on a detail change (a small catalogue in place of the shipped 1908) | `plt_08_rebuild_on_detail_change` |
| 9 GetObjectIDArray (quirk 13) | `plt_09_object_id_array` |
| 10 has building family / type | `plt_10_has_building_type` |
| 11 prop footprint | `plt_11_prop_footprint` |
| 12 records (components 5 and 9) | `plt_12_records` |

The cores' unit tests (`fc4_core::prop_manager`, `fc4_core::lot_config_manager`) also cover `Init` / `Shutdown`,
the catalogue message, CreateProp's classes, insertion by usage, the view filter and CreateTextDecal messages, the
add/remove table, set queries, filter caches, building information, CreateGeneric and FixLotConfiguration.

## Coverage (tutorial §8)

| §8 item | Vector |
|---|---|
| 1 plain record | `tu8_01_plain_record` |
| 2 tutorial record (the consumers through their own vectors) | `tu8_02_tutorial_record` |
| 3 GUID mapping (quirk 1) | `tu8_03_guid_mapping` |
| 4 version | `tu8_04_version` |
| 5 end without registry, then save | `tu8_05_end_without_registry_then_save` |
| 6 task counter | `tu8_06_task_counter` |
| other sections (§4.3 messages, quirks 2 and 4) | `tu00_messages` |

## Coverage (occupant manager §18)

| §18 item | Vectors |
|---|---|
| 1 counts | `om18_01_counts` |
| 2 position to world cell | `om18_02_position_to_world_cell` |
| 3 standard cell floor | `om18_03_standard_cell_floor` |
| 4 slot 12 bug | `om18_04_slot12_bug` |
| 5 insert cells | `om18_05_insert_cells` |
| 6 insert and its messages | `om18_06_insert_messages` |
| 7 insert failure | `om18_07_insert_failure` |
| 8 newest first | `om18_08_newest_first` |
| 9 dedup and order across cells | `om18_09_dedup_and_order` |
| 10 strict box test | `om18_10_strict_box` |
| 11 null max quirk | `om18_11_null_max_quirk` |
| 12 typed type 0 | `om18_12_typed_type_zero` |
| 13 remove order | `om18_13_remove_order` |
| 14 RemoveOccupants order | `om18_14_remove_occupants_order` |
| 15 move | `om18_15_move` |
| 16 FindOccupant | `om18_16_find_occupant` |
| 17 GetBoundingCells | `om18_17_bounding_cells` |
| 18 iteration | `om18_18_iteration` |
| 19 save round-trip | `om18_19_save_round_trip` |
| 20 shutdown | `om18_20_shutdown` |
| 21 slot 34 clamp (OM5) | `om18_21_slot34_clamp` |
| 22 wrapped box (OM6) | `om18_22_wrapped_box` |
| 23 slot 44 batches (OM7) | `om18_23_slot44_batches` |
| 24 shrink precision (OM4) | `om18_24_shrink_precision` |
| §14 lock regions and re-entrant callbacks | `om14_lock_regions`, `om14_reentrant_callbacks` |

## Coverage (terrain §18)

| §18 item | Vector |
|---|---|
| 1 `GetAltitude` on the worked cell (both triangles, the corners, out of bounds, the ignored flip flag) | `tr18_01_get_altitude` |
| 2 the floor at cell boundaries and the last row and column | `tr18_02_floor_at_cell_boundaries` |
| 3 `GetFaceNormal` (both triangles; the untouched output at the upper ends and below 0) | `tr18_03_face_normal` |
| 4 `CellHasWater` and `CellHasLand` (shore, dry, NaN, out of bounds) | `tr18_04_cell_has_water_and_land` |
| 5 `GetWaterDepthForCell` and the draft clearance | `tr18_05_water_depth_for_cell` |
| 6 `GetWaterType` (land, sea, fresh) | `tr18_06_water_type` |
| 7 `SetAltitudeAtVertex` depth bookkeeping (quirk 4), no clamp, the lock | `tr18_07_set_altitude_at_vertex` |
| 8 `SetAltitude(x, z, h)` clamps, the lock, the unchecked index | `tr18_08_set_altitude_xz` |
| 9 `SetSeaLevel` range and message order | `tr18_09_set_sea_level` |
| 10 `RedisplayTerrain` packing, the view-ready gate, slot 69, the long-operation messages | `tr18_10_redisplay_packing` |
| 11 the pipe-burst sinkhole (kind 0 lower; a neutral bitmap; the walk draw; the texture message's words) | `tr18_11_pipe_burst_sinkhole` |
| 12 an unknown brush | `tr18_12_unknown_brush` |
| 13 the lava flow rule and `SetThingsOnFire` | `tr18_13_lava_flow_rule` |
| 14 the record sizes and round trip | `tr18_14_records` |
| 15 the lava neighbour order and the `GetNeighbors` mask | `tr18_15_lava_neighbour_order` |
| 16 `SetThingsOnFire` (x outer, z inner; one flammable cell) | `tr18_16_set_things_on_fire` |
| 17 `GetWaterType` with a NaN depth, `AltitudeAtWhichFloodingWillOccur`, `GetLerpedWaterLevel`, `GetWaterVelocity`, `GetSedimentValue(x, z)` | `tr18_17_water_type_nan_and_flooding` |
| 18 `ComputeSpansForRectangle` (the worked example, 45°, the clamps) | `tr18_18_spans` |
| 19 the walk and stroke angles | `tr18_19_brush_angles` |
| 20 the default brush level | `tr18_20_default_brush_level` |
| 21 `SetWaterSurfaceDisplayState` posts only on a change | `tr18_21_water_display_state` |
| 22 kind 5 (level toward the target) | `tr18_22_kind5_level` |
| 23 the sea level after a load | `tr18_23_sea_level_after_load` |
| other sections | `tr00_cached_min_max_stale` (§4.6), `tr00_navigability_and_bodies` (§10.3, §10.4), `nt05_economy_modifier` (nation §5) |

The core's unit tests (`fc4_core::terrain`) also cover the other brush kinds (2 talus smoothing, 7 the mark), the lava start and burn rectangles,
the record versions and truncation, the tuning exemplar and the bodies-of-water flood fill; `fc4_capi`'s `terrain_tests` cover the C ABI.

## Coverage (automata §14)

| §14 item | Vector |
|---|---|
| 1 script system (`01 00`; `02 00` fails) | `am14_01_script_system` |
| 2 controller manager, empty (30 bytes; a cut record) | `am14_02_controller_manager_empty` |
| 3 controller manager times | `am14_03_controller_times` |
| 4 vehicle and pedestrian managers, empty (versions 1 and 4, base 7, a cut record) | `am14_04_traffic_managers_empty` |
| 5 attractor | `am14_05_attractor` |
| 6 generator (version 3 fails; the base version is kept) | `am14_06_generator` |
| 7 watercraft, version 6 (version 5 bare, version 7 fails) | `am14_07_watercraft_v6` |
| 8 riot order (plus the stage rule and the road, cap and null-automaton stops) | `am14_08_riot_order` |
| 9 fire | `am14_09_fire` |
| 10 flag 8 | `am14_10_flag_8` |
| 11 unknown template (and the skipped pass when not loaded) | `am14_11_unknown_template` |
| 12 corpus round trip | not a vector: `cargo test -p fc4-formats --release --test automata_records -- --nocapture` |
| other: version 3 traffic managers with an incident, the aircraft lists and versions, a ferry entry, an automaton set | `am14_12_versions_and_lists` |
| other: `SetMaxCount`, `IsDrivingMode` | `am14_13_manager_slots` |
| other: the template and occupant-group slots | `am14_14_controller_groups` |

The automaton and state records (§6) are covered by the core unit tests and the corpus sweep, not by a vector.

## Coverage (advisors §17)

| §17 item | Vector |
|---|---|
| 1 batch size | `ad17_01_batch_size` |
| 2 evaluation gate | `ad17_02_evaluation_gate` |
| 3 once | `ad17_03_once` |
| 4 persist / no_timeout | `ad17_04_persist_no_timeout` |
| 5 trigger values (also a trigger that does not compile, a missing trigger) | `ad17_05_trigger_values` |
| 6 event advice | `ad17_06_event_advice` |
| 7 activation messages | `ad17_07_activation_messages` |
| 8 ticker | `ad17_08_ticker` |
| 9 paused | `ad17_09_paused` |
| 10 mayor rating | `ad17_10_mayor_rating` |
| 11 subject clamp (and the object references) | `ad17_11_subject_clamp` |
| 12 reward condition | `ad17_12_reward_condition` |
| 13 persistence round trip | `ad17_13_persistence` |
| 14 advice record | `ad17_14_advice_record` |
| 15 subject record | `ad17_15_subject_record` |
| 16 system record | `ad17_16_system_record` |
| 17 CityPlanning record | `ad17_17_city_planning_record` |
| 18 random_chance | `ad17_18_random_chance` |
| 19 utility counts | `ad17_19_utility_counts` |
| 20 HQ empty lists | `ad17_20_hq_empty_lists` |
| 21 popup thresholds | `ad17_21_popup_thresholds` |
| 22 Lua functions | `ad17_22_lua_functions` |

Other behaviour: system Init and Shutdown (`ad04_01_system_init`), advisor re-Init against a changed table
(`ad04_02_advisor_reinit`), RetireAdvice and CommandWouldTrigger (`ad06_01_retire_command`). The unit tests of
`fc4_core::advisors` cover the subclass variables (§5.5, §7) in more depth.

## Coverage (MySims §12)

| §12 item | Vector |
|---|---|
| 1 ownership (the vector, get_count, the agent record; more than five MySims) | `ms12_01_ownership` |
| 2 move-in age | `ms12_02_move_in_age` |
| 3 move-in EQ ring | `ms12_03_eq_ring` |
| 4 EQ ≤ 0 ignored | `ms12_04_eq_not_positive` |
| 5 ageing (and the skip conditions) | `ms12_05_ageing` |
| 6 job search | `ms12_06_job_search` |
| 7 home destroyed (no disaster) | `ms12_07_home_destroyed` |
| 8 home destroyed by disaster | `ms12_08_home_destroyed_by_disaster` |
| 9 job destroyed | `ms12_09_job_destroyed` |
| 10 script commands | `ms12_10_script_commands` |
| 11 spawn | `ms12_11_spawn` |
| 12 building added (and a fire) | `ms12_12_building_added` |
| 13 riot subject (and pipe burst, tornado) | `ms12_13_riot_subject` |
| 14 trip percentages (landmark and disasters: `ms12_14b_trip_landmark`) | `ms12_14_trip_percentages` |
| 15 local distances (and the closest special building) | `ms12_15_local_distances` |
| 16 school grade | `ms12_16_school_grade` |
| 17 GetActiveUIPanelSim | `ms12_17_active_ui_panel_sim` |
| 18 persistence round trip | `ms12_18_record` |
| 19 old version (14) | `ms12_19_version_14` |
| 20 no feedback | `ms12_20_no_feedback` |

Item 3: the `delta_eq` field is written by `UpdateVariables` (§4.3 step 3) before the ring moves (step 4), so the
field after step k shows the delta of step k − 1; the vector checks both. Item 20 in its full-game form (every
simulator record byte-identical over 24 months with and without MySims) needs the whole simulator stack; the vector
checks that the MySims make no simulator-facing host call.

## Prop occupant ops

The prop occupant vectors (`po11_…`, `po00_…`) drive `fc4_core::prop_occupant::PropOccupant` and
`VisualEffectProp` (`docs/specs/prop-occupant.md`) behind a fake game implementing `PropOccupantHost` and
`EffectHost`. Every op name starts with `po_`. The default world: no prop, an empty process-wide prop type table, a
fake game that is not in the city and has no model-state property. `po_game` configures the fake game; `po_expect`
reads the prop, the effect prop and the game together. The record ops take and emit hex, so a corpus record is pinned
byte for byte.

| op | Arguments | Effect / check |
|---|---|---|
| `po_create` | `loaded?` | a fresh prop with the constructor's defaults (§2.2) |
| `po_effect_create` | `no_effect?` | a fresh `0x2977AA49` prop (§6.2) |
| `po_call` | `method`, `args?`, `expect?` | one method of the prop (below) |
| `po_expect` | `state` (partial) | the prop, the effect prop and the game |
| `po_game` | any of `in_city`, `model_state_property` (bool or null = no exemplar), `draws` (`[r, …]`, scripted `RandomUint32Uniform(100)`), `flag2`, `clear` (reset the logs) | the fake game |
| `po_const` | `name`, `expect` | a constant of §1 and §2.2 |
| `po_record_create` | | a fresh default record, replacing whatever was there |
| `po_record_from_prop` | | the record a save would write for the live prop (§7) |
| `po_read` | `hex`, `apply?`, `expect` (`"ok"`, `"base-failed"`, `"truncated"`) | `Read` (§7, `lr` §10.1) |
| `po_write` | `hex?`, `expect` (the hex) | `Write` (§7) |
| `po_record_field` | `name`, `expect` | one field of the current record |
| `po_effect_call` | `method`, `args?`, `expect?` | one method of the visual-effect prop |
| `po_effect_read` | `hex`, `expect` (`["ok", nameHex, trailer, byte0, byte1]` or `"failed"`) | `lr` §10.2 |
| `po_effect_write` | `hex?`, `expect` (the hex) | `lr` §10.2, with the record quirk applied when the effect is live |
| `po_config` | `ids` (array or null), `unknown?` | a lot's configuration object IDs, for the PV2 ops |
| `po_tag` | `args` (`[tag]`), `expect` (`["matched", index]`, `"config-lacks-tag"`, `"no-configuration"`, `"unknown-configuration"`, or null) | classify one tag (§5.3) |
| `po_block` | `method` (`reset`, `keep`, `kept`, `unmatched`, `slots_with`) | the `pd` §4.3 step 2.3.2 slot accounting |
| `po_tally` | `method` (`tagged`, `matched`, `config_lacks_tag`, `no_configuration`, `unknown_configuration`, `unmatched`, `unmatched_percent`) | the PV2 corpus categories |
| `po_store` | `method` (`write`), `args` (`[count]`), `expect` (`[written, entries, crcs_ok, read, index_ok]`) | the class-object store framing (§7) |

### Prop occupant methods (`po_call`)

`query_interface` / `query_interface_base`, `as_occupant_offset` / `as_occupant_offset_null`, `add_ref`, `release`,
`refcount`, `set_bounding_box`, `bounding_box`, `set_bounding_box_from_size`, `slot5_stub`, `slot6_noop`,
`get_prop_type`, `set_prop_type`, `prop_type_index`, `type_table_len`, `get_orientation`, `set_orientation`,
`get_state`, `set_state`, `restore_state`, `set_time_of_day_range`, `get_time_of_day_range`, `time_of_day_bytes`,
`set_simulator_date_range`, `simulator_date_range`, `tick_simulator_date`, `set_random_chance`,
`get_random_chance`, `get_prop_timer_bitfield`, `get_prop_timer_state`, `set_prop_timer_state`, `get_prop_usage`,
`set_prop_usage`, `add_usage`, `lot_tag`, `has_lot_tag`, `set_lot_tag`, `init_after_load`, `shutdown`, `set_loaded`,
and the slot table `vtable_len`, `slot_channel`, `slot_method`, `slot_address`, `slot_offset`, `slot_args`,
`vtable_addr`, `vtable_next_addr`.

### Visual-effect methods (`po_effect_call`)

`query_interface`, `as_occupant_offset`, `as_prop_occupant_offset`, `object_size`, `class_id`,
`set_effect_type_id`, `set_effect_type_name`, `start_effect`, `stop_effect`, `effect_handle`, `effect_id`,
`effect_name`, `is_started`, `is_live`, `should_drop_name_on_write`.

### `po_expect` state fields

`state`, `chance`, `tod`, `tod_bytes`, `timer_bits`, `lot_tag`, `has_lot_tag`, `prop_type`, `type_index`,
`orientation`, `usage`, `bbox`, `refcount`, `loaded`, `has_date_range`, `date_range`, `occupant_type`,
`object_size`, `effect_id`, `effect_name`, `effect_started`, `effect_live`, `effect_ctor_flag`, `effect_scratch`,
`messages`, `clock`, `draws`, `removed`, `model_states`, `effect_calls`.

## Coverage (prop-occupant §11)

The 25 conformance tests of `docs/specs/prop-occupant.md` §11, plus one cross-section vector for the slot table.
`po00_01_vtable_channels` pins the measured return channels of §4.1 that `docs/abi/interfaces.tsv` labels
`eax-or-void`.

| §11 item | Vector |
|---|---|
| 1 identity (§1) | `po11_01_identity` |
| 2 constructor defaults (§2.2) | `po11_02_constructor_defaults` |
| 3 object size (§1) | `po11_03_object_size` |
| 4 AsOccupant, the vtable extent and the refcount (§2.2, §2.3, §4.2, quirk 10) | `po11_04_as_occupant_vtable_refcount` |
| 5 GetPropType round trip (§5.1) | `po11_05_prop_type_round_trip` |
| 6 unknown type is interned (§5.1) | `po11_06_unknown_type_interned` |
| 7 GetState is a clean EAX return (§4.1) | `po11_07_get_state_eax` |
| 8 GetRandomChance is an AL return (§4.1, quirk 3) | `po11_08_get_random_chance_al` |
| 9 bounded box (§5.2) | `po11_09_bounding_box` |
| 10 orientation (§5.2) | `po11_10_orientation` |
| 11 time of day (§4.1, §8 item 3) | `po11_11_time_of_day` |
| 12 timer bits (§4.1) | `po11_12_timer_bits` |
| 13 timer transition draw (quirk 7) | `po11_13_timer_transition_draw` |
| 14 lot tag default (§5.3) | `po11_14_lot_tag` |
| 15 usage (§3) | `po11_15_usage` |
| 16 save / load round trip (§7) | `po11_16_save_load_round_trip` |
| 17 version ≤ 5 read (quirk 5) | `po11_17_version_five_read` |
| 18 unknown type survives a round trip (§5.1, quirk 2) | `po11_18_unknown_type_round_trip` |
| 19 class-object store framing (§7) | `po11_19_store_framing` |
| 20 loaded props do not announce themselves (§3) | `po11_20_loaded_no_announce` |
| 21 visual-effect attach (§6.2) | `po11_21_effect_attach` |
| 22 visual-effect record quirk (§6.2) | `po11_22_effect_record_quirk` |
| 23 state and the model (quirk 6) | `po11_23_state_and_model` |
| 24 PV2 tag matching (§5.3) | `po11_24_pv2_tag_matching` |
| 25 removal (§3) | `po11_25_removal` |
| — the 24-slot table and its return channels (§4.1, §4.2, §8 item 2) | `po00_01_vtable_channels` |

Item 24 reproduces the PV2 rule literally: a tagged prop whose tag names no configuration object lowers the block's
`k` by one (`pd` §4.3 step 2.3.2 step 2), which is what leaves the block's slot count one lower. The vector also
pins the corpus split of §5.3 (40,830 of 147,162 tagged lot props unmatched, 27.7 %: 40,794 + 36 + 0). Item 13 sets
the prop to state 2 first so the `SetState(0)` call is observable — `SetState` is a no-op when the value already
equals `+0x63` (`nt` §13.1 step 1).
## Coverage (network geometry §18)

| §18 item | Vector |
|---|---|
| 1 straight drag | `ng18_01_straight_drag` |
| 2 near-straight drag (pre-adjust 1.2) | `ng18_02_near_straight_drag` |
| 3 diagonal snap, width 1 | `ng18_03_diagonal_snap_width_1` |
| 4 diagonal snap, width 2 | `ng18_04_diagonal_snap_width_2` |
| 5 street is straight only | `ng18_05_street_straight_only` |
| 6 slope limits | `ng18_06_slope_limits` |
| 7 satisfier, slope | `ng18_07_satisfier_slope` |
| 8 satisfier, smoothness and preference | `ng18_08_satisfier_smoothness_preference` |
| 9 tunnel trigger | `ng18_09_tunnel_trigger` |
| 10 no tunnel code | `ng18_10_no_tunnel_code` |
| 11 a bridge needs a straight drag | `ng18_11_bridge_needs_straight_drag` |
| 12 starting in water | `ng18_12_starting_in_water` |
| 13 reserved landfill | `ng18_13_reserved_landfill` |
| 14 support level | `ng18_14_support_level` |
| 15 terrain support | `ng18_15_terrain_support` |
| 16 draft flag | `ng18_16_draft_flag` |
| 17 cell cost | `ng18_17_cell_cost` |
| 18 power-line cost | `ng18_18_power_line_cost` |
| 19 destroy test set | `ng18_19_destroy_test_set` |
| 20 grading threshold | `ng18_20_grading_threshold` |
| 21 smoothing codes | `ng18_21_smoothing_codes` |

The core's unit tests (`fc4_core::network_geometry`) also cover the pre-adjust rules, the city-edge refusal of wide
types, the satisfier's collapse rule, slope merging, preferred ranges and pinned propagation, the smoothing and
boundary sets, the water-surface floor of the cell infos, the flat-cell and cross-section rules, the offset tables and
the diagonal-end extension; `fc4_capi`'s `network_geometry_tests` cover the C ABI.
## Coverage (effects §17)

| §17 item | Vector |
|---|---|
| 1 the brush stroke count (record 2 at 33333 us) | `ef17_01_brush_stroke` |
| 2 the dilation (1.5; 100000 us) and the truncated time | `ef17_02_dilation` |
| 3 the select draw (seed `0x1105`) and the volcano creators | `ef17_03_select_draw` |
| 4 the one-shot crater | `ef17_04_one_shot_crater` |
| 5 out of bounds | `ef17_05_out_of_bounds` |
| 6 the game record's order and the clipped rectangle | `ef17_06_game_record_order` |
| 7 moving to a new cell | `ef17_07_move_to_new_cell` |
| 8 the pause record | `ef17_08_pause_record` |
| 9 the priority filter and the index -1 no-op | `ef17_09_priority_filter` |
| 10 no end without the draw pass (both finish modes; EF2, EF8) | `ef17_10_no_end_without_the_draw_pass` |

The core's unit tests (`fc4_core::effects`) also cover the directory parser and writer, the LOD ranges, the
nested update (EF9), the game record's other actions, the shipped tables and shutdown; the install sweep
(`cargo test -p fc4-formats --test effects_directory -- --nocapture`) parses the installed directory; `fc4_capi`'s
`effects_tests` cover the C ABI.
## Coverage (ui-windows §11)

| §11 item | Vector |
|---|---|
| 1 AsIGZWin offset (`this + 0x0C`, null for a null `this`) | `ui11_01_as_igzwin_offset` |
| 2 two vtables, one allocation | `ui11_02_two_vtables_one_allocation` |
| 3 slots 59 and 60 return nothing | `ui11_03_slots_return_nothing` |
| 4 secondary close is a no-op when nothing is open | `ui11_04_secondary_close_is_a_noop` |
| 5 tertiary close is a no-op when nothing is open | `ui11_05_tertiary_close_is_a_noop` |
| 6 secondary close cascades | `ui11_06_secondary_close_cascades` |
| 7 teardown order (Hide, Release, ChildDelete) | `ui11_07_teardown_order` |
| 8 both fields cleared | `ui11_08_both_fields_cleared` |
| 9 secondary does not write catalog state | `ui11_09_secondary_does_not_write_catalog_state` |
| 10 tertiary writes two dwords, slot 17 then slot 9 | `ui11_10_tertiary_writes_two_dwords` |
| 11 tertiary skips the save on a failed query | `ui11_11_tertiary_skips_the_save_on_a_failed_query` |
| 12 invoke toggles (secondary and tertiary) | `ui11_12_invoke_toggles` |
| 13 invoke replaces | `ui11_13_invoke_replaces` |
| 14 invoke drops the tool first | `ui11_14_invoke_drops_the_tool_first` |
| 15 parent is the view's window | `ui11_15_parent_is_the_views_window` |
| 16 script group | `ui11_16_script_group` |
| 17 null create is safe | `ui11_17_null_create_is_safe` |
| 18 generator flags | `ui11_18_generator_flags` |
| 19 alignment and show order | `ui11_19_alignment_and_show_order` |
| 20 advisor popup close path (`[game]` observation; replacement side pinned) | `ui11_20_advisor_popup_close_path` |
| 21 `DoWinProcMessage` gets the window as arg1 | `ui11_21_do_win_proc_message_takes_the_window` |
| 22 message delivery order | `ui11_22_message_delivery_order` |
| 23 the hidden pause is a nesting count (`[game]`; replacement pinned) | `ui11_23_hidden_pause_is_a_nesting_count` |
| 24 effects end messages need a paint (`[game]`; window-side consequence) | `ui11_24_effects_end_messages_need_a_paint` |
| 25 only the View3D record is saved | `ui11_25_only_the_view3d_record_is_saved` |

Tests 20, 23 and 24 are marked **[game]** in ui-windows.md §11: they need the original. Their vectors pin the
replacement-side behaviour that makes the observation true — the popup's flag table (including that a key message is
never looked at, which is why Esc and Enter do nothing), the hidden pause as a nesting count that a camera move raises
with no dialog open, and the window-side consequence of effects.md EF2 (a view that is never painted never reaches the
effects draw pass).

## Dates and times ops

`docs/specs/dates.md`. Day numbers and instants are plain integers (`"0x..."` strings allowed). `"tz"` is
`{"timezone": seconds west of UTC, "daylight": 0|1}` (the CRT's `_timezone` / `_daylight`), default UTC with no
daylight time.

| Op | Keys | Effect |
|---|---|---|
| `dt_fn` | `fn`, `args`, `tz?`, `expect` | one pure function: `jday` (m, d, y), `mdy` (n, returns `[m, d, y]`), `weekday`, `day_of_year`, `first_day_of_month` (n, m), `previous` (n, w), `leap_year`, `days_in_year`, `day_within_month`, `month_from_name` (string), `date_string` (n, format), `set_day_of_year` (doy, y), `begin_dst` / `end_dst` (y, the instant), `begin_dst_day` / `end_dst_day` (y, `[m, d, y]` of the UTC day), `is_dst` (t), `time_from_local` (day number, h, m, s), `time_local_secs`, `time_hour`, `time_minute`, `time_date` (t) |
| `dt_new` | `obj`, `day?` or `mdy?` | a labelled `cGZDate`, count 0 |
| `dt_call` | `obj`, `method`, `args`, `out?`, `tz?`, `expect` | one `cIGZDate` slot (names as in `fc4_core::dates::GzDate`; `eq` / `ne` / `add_assign` / `sub_assign` / `copy` / `clone`, `post_inc` / `post_dec` for the own vtable). Date arguments are labels or day numbers. Slots 37-40 return `"this"`. `out` names the out date of `max_date` / `min_date` / `previous` and the new date of `clone`. A `release` that frees removes the label |
| `dt_expect` | `obj`, `state` | fields `exists`, `day`, `refs` |

## Coverage (dates §8)

| §8 item | Vector |
|---|---|
| 1-5 day numbers, invalid dates, round trip and fields, leap years, FirstDayOfMonth | `dt8_01_day_numbers` |
| 6-9 Previous, Set(dayOfYear, y), month names, text | `dt8_02_setters_text` |
| 10 the COM object, and every slot of §4 | `dt8_03_com_object` |
| 11 the daylight rule, and §2.1 / §2.3 (crime.md §9's worked example) | `dt8_04_daylight_time` |

Not covered by a vector, and why: `cRZDate()` / `cRZTime()` "now" read the host clock *[compat-limit]*; the
`fc4-core` unit tests drive them through a fixed `WallClock`. Names shorter than three characters (DT2) and the
`Clone` / `DateString` results (DT3) are pinned as *[choice]*.
