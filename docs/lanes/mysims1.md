# mysims1 — the mysims shadow (a same-process differential of the MySims)

| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| mysims1 (the mysims shadow: FC4MySimsShadow + regress shadows list) | `mysims1` / `.claude/worktrees/mysims1` (branch `mysims1`, engine lane) | merged by ancestry in train19 via mysims1fix3; MySims exact 73/73, run 223, gate/regress PASS | See mysims1fix3 report; future MS3/MS5/MS7 implementation |

## Handoff

- Round 6 (2026-10-21): FC4MySimsShadow.cpp written and committed (6ef1db2e), retfix re-audited + committed (78592388).
  qbuild all PASS (cargo 3576, clippy 0, fc4verify 62/62). DLL copied to C:/Users/tlori/FreeCity4/build-runs/dlls/FreeCity4-mysims1.dll.
  mysims1-sh-1 queued (req written, in game-queue). Next: read the verdict, grep the log for mysims lines,
  diagnose mismatches (MS11+ if a spec gap), then mysims1-sh-2 (shadows string, ref_warnings=0), report + merge queue.

- Goal: add the mysims shadow (same-process differential like the other shadows), wire it
  into the regress shadows list (tools/regress.ps1 `$runs.shadows`, additive); exact to
  2032 on the test city; in-game runs mysims1-sh-1 (shadows on, original classes) and
  mysims1-sh-2 clean with ref_warnings=0.
- Done: worktree created (branch `mysims1` from main `7e042544`); submodule
  engine/third_party/gzcom-dll repointed to this worktree and populated; lane file
  committed `93c9ca3e`; baseline `qbuild all` PASS (build-runs/baseline.out).
- Design researched (facts a successor needs):
  - The plugin has NO MySim glue: nothing in engine/plugin/ references fc4_advisors.
    The game's own `cSC4MySimAgentSimulator` is reached via `cISC4City` slot 71
    `GetMySimAgentSimulator()` (SDK header engine/third_party/gzcom-dll/gzcom-dll/include/cISC4City.h:188;
    cISC4MySimAgentSimulator.h, cISC4MySim.h, cISC4Advisor.h, cISC4AdvisorSystem.h all in that SDK dir).
  - Original per-MySim state readable read-only: each MySim's cIGZSerializable record
    (version 17, mysims.md §6.2: rows 1-31 + advisor part §11.2 advisors.md) captured via
    WriteCapture (FC4WriteCapture.h); getters (GetSlot, GetResidence, GetJob, GetAge,
    GetSign, IsMale, GetWealth, IsActive, GetVehicleKey, GetLocation); agent getters
    GetActiveMySims/MySimLivesHere/GetActiveUIPanelSim. The advisor system's own record
    (§11.1 advisors.md) refs every advisor incl. MySims.
  - Shadow-core read path problem: CaptureIStream returns refs as HOST tokens, so
    `Advisor::read_record`'s advice refs would NOT resolve to core Advices (skipped), and
    a byte-exact record round trip of the MySim record is infeasible as-is. Working
    pattern from advisors_tests.rs:717-744 (subject_round_trip_and_core_handles): write a
    custom istream whose get_ref returns `core_handle` from fc4_advisors_create_object
    (class store creates core Advice/Subject/MySim objects) so refs resolve as core
    objects; then compare field-by-field (decode BOTH records with CaptureReader in C++
    and diff the decoded fields) instead of raw byte equality.
  - Shadow core = fc4_advisors handle created with a read-only fc4_advisor_host +
    fc4_mysim_host (all mutating callbacks NULL; resolve_ref maps capture object id →
    token), plus MySimAgent init; the shadow drives its own daily/monthly via
    fc4_advisors_advisor_simulate on the MySim ids after mirroring the original's steps
    (compare per mysims.md §2.2 saved fields + GetActiveMySims count; EQ/HQ recomputed
    equal only if the host feeds the same grid values).
  - Wiring points: FC4Config.cpp kOtherKeys end (add `mysims_shadow`); cFC4Director.cpp
    PostAppInit `MySimsShadowStart()` + PreAppShutdown Stop; FC4Observer.cpp PostCityInit
    `MySimsShadowCityLoaded(CurrentCity())` and kMsgSimNewMonth case `MySimsShadowMonth`;
    CMakeLists.txt plugin list (additive); tools/regress.ps1 line 170 shadows string
    (additive at end).
- Round 2 (2026-10-21, context OVER before code; design above re-verified, keep it):
  - main is now 37283fba and HAS cores28: engine/core/src/mysims/dispatch.rs + pool.rs (+
    tests), sim.rs +6 lines, MySimHost gains only the optional-default `dispatch_check_flags`;
    mysims.md itself unchanged; questions MS8-MS10 exist on main but NOT in this worktree.
    This branch cut at 7e042544 = 2 commits behind. First step of round 3:
    `git -C <WT> merge main` (the shadow touches no mysims core file; merge is clean).
  - The C++ side of the design holds: cISC4City.h:188 slot 71, MySim QI 0x0A1DA42F,
    AsAdvisor() = cISC4MySim slot 3 → cISC4Advisor slot 14 Simulate(kind)/slot 5 GetID; the
    agent's 6 slot ids come from the city's advisor system (slot 81 GetAdvisorSystem,
    slot 8 GetAdvisors map, id 30 = My Street at index 5). GetActiveMySims takes an
    SC4Vector<cISC4MySim*> (the game appends; use a local SC4Vector, plugin/FC4GameContainers.h
    PushTrivial for pointers).
  - Feeding the shadow core over the C ABI (fc4.h): `fc4_advisors_create(&h,&m,S)` with
    read-only hosts (all mutating callbacks NULL, resolve_ref returns the token,
    object_created returns 0), `fc4_mysims_agent_init`, then mirror §3.2 messages via
    `fc4_mysims_agent_do_message` (Target pattern, FC4HistoryShadow.cpp lines 655-700) and
    the day/month steps via `fc4_mysims_advisor_simulate(p, id, 0|1)` AFTER the original's
    own pass (ageing needs the shadow advisor's month_counter to advance — Simulate(1) does
    that itself, advisor.rs:436). `ui_blocked` callback NULL (reads false; the original's
    blocking-UI state is not readable read-only, question MS2).
  - Session seed: `fc4::SessionSeed()` (plugin/FC4Seed.h). Each core MySim's RNG derives
    seed = SessionSeed ^ (clsid::MYSIM ^ index) (sim.rs:105, rng.rs derive_seed), so with the
    same S the shadow core is draw-exact; the original's MySim RNG is wall-clock-seeded and
    unreadable — FindNewResidence's start position is the only uncomparable draw (only
    reached on home destruction): log it, do not compare.
  - Session-seed mismatch NOTE: the shadow core's per-MySim index is its creation order in
    the class store during Read, which may differ from the original's; if a start-position
    draw ever matters the diff will show it — name it and move on.
  - Compare DECODED fields, not bytes: original per-MySim record (WriteCapture on each
    MySim's Serializable, version 17 §6.2) decoded in C++ with CaptureReader; shadow state
    read with fc4_mysims_agent_get_state + fc4_mysim_get_state + fc4_mysim_text. Also compare
    GetActiveMySims count/ids and the getters (GetSlot/GetAge/GetSign/IsMale/GetWealth/
    IsActive/GetResidence/GetJob) against the shadow's state — getters are the surface the
    UI reads. EQ/HQ: the shadow's host must feed the same grid values for eq/hq to match:
    wire the mysim host's eq_by_wealth/hq_by_wealth/life_expectancy/average_age to the game
    (residential simulator, mysims.md §9.2) READ-ONLY, or expect those fields to differ and
    name them. Simpler first cut: wire grid+eq hosts to the game read-only; the shadow is
    then exact when the simulators are.
  - Wiring points with VERIFIED line numbers (this worktree): FC4Config.cpp kOtherKeys ends
    line 320 ("ports_shadow"}}; add `mysims_shadow` before the close); cFC4Director.cpp
    PostAppInit 341-346 (add MySimsShadowStart() after TerrainShadowStart()), PreAppShutdown
    354-364 (add Stop); FC4Observer.cpp PostCityInit case 574-601 (add CityLoaded after
    AutomataShadowCityLoaded line 601), kMsgSimNewMonth case 664-707 (add Month after
    PropDevShadowMonth ~line 701), PreCityShutdown 630-647 (add CityShutdown after
    AutomataShadowCityShutdown 647); FC4Subsystems.cpp Resolve() end (~line 629, after
    AutomataShadowStartup(enabledKey("automata"))): `MySimsShadowStartup(!SimulatorReplaced())`
    (no subsystem named mysims; the INACTIVE case is `enable` making the agent ours — there
    is no enable key for it, so the bool is always true and the check stays for symmetry);
    engine/CMakeLists.txt plugin list after FC4AutomataShadow (~line 470); tools/regress.ps1
    line 170 shadows string (append `mysims_shadow = 1` at the end); engine/README.md a
    "MySims shadow" bullet after the Automata shadow bullet (~line 1774, additive).
- Round 3 (2026-10-21, further research, NO code yet):
  - main MERGED clean (tip `a1a6d6d3`); questions MS8-MS10 now in this worktree
    (docs/specs/questions/mysims.md, next free MS11).
  - Line-format contract (run_summary.ps1:134-151): a shadow line counts as a "shadow line"
    only if it matches `\] ([a-z]+) shadow [^:]*\d{4}-\d\d-\d\d[^:]*: .*(mismatch(es|ed)[= ]\d|: clean\b)`
    — i.e. the log line must read `<name> shadow YYYY-MM-DD <event>: ... mismatches=N`. The
    LAST `; mismatches=N` on the line decides clean/differs. So the shadow's load/month lines
    must carry a date and end with `; mismatches=N` (civic/citysit shadows are the template).
  - Compare DECODED vs DECODED (final decision): capture each original MySim's record
    (WriteCapture), decode it in C++ with CaptureReader (version-17 layout mysims.md §6.2),
    and diff against the shadow core's `fc4_mysim_get_state` + `fc4_mysims_agent_get_state`
    + `fc4_mysim_text(bio=0/path=1)`. Plus agent getters via
    `fc4_mysims_active/lives_here/get_active_ui_panel_sim` against the game's
    GetActiveMySims/MySimLivesHere/GetActiveUIPanelSim (SC4Vector via plugin FC4GameContainers
    PushTrivial; std::map GetAdvisors avoided — use GetAdvisor(id) after reading the map via
    per-id probe is NOT possible: instead enumerate ids 1-5 + 30 by QI-ing each candidate
    `as->GetAdvisor(id)` for cISC4MySim 0x0A1DA42F).
  - ORIGINAL message mirroring (exact source): the agent's own cIGZMessageTarget2 (city
    component, obtained slot 71) receives the SAME §3.2 messages the game delivers to it —
    the shadow core must get copies. Route them from the shadow's own message-subscriptions:
    register the shadow target for the SAME 11 msg ids (order in agent.rs AGENT_SUBSCRIPTIONS:
    0x99EF1142, 0x99EF1145, 0x99EF1143, 0xA6B79602, 0xA6B79612, 0x44778230, 0x8A09FA6C,
    0xCA1B1E64, 0xEA1F0DCD, 0x0A36F1E9, 0xEA0D163E) and forward each into
    `fc4_mysims_agent_do_message` with fc4_adv_message{msg_type,data1..4,void1=token of the
    occupant,unknown=advisor id for 0x8A09FA6C}. Occupant token = canonical cISC4Occupant
    TokenOf (civic shadow's Canonical() helper pattern, cFC4Civic.cpp:157).
  - Daily/monthly drive: the ADVISOR TICKER (0xCA5501F2, advisors.md §5.1) is what calls
    Simulate(kind) on every advisor incl. MySims (monthly on day 28 BEFORE daily). The shadow
    core's ticker must be driven identically: subscribe the shadow target to 0xCA5501F2 +
    PostCityInit 0x26D31EC1 + CityEstablished 0x26D31EC4 and forward into
    `fc4_advisors_do_message` — the core's on_ticker implements §5.1 itself (system.rs:431)
    and calls Simulate on all advisors, so NO manual per-advisor simulate calls needed. The
    host `sim_date` callback must return the real game date (fc4_adv_date{day_number,year,
    month,day}) from cISC4Simulator slot 8 GetSimDateNumber + fc4_date_from_julian.
  - Lua/host wiring for the SHADOW core (all read-only, mapped to game): create_table=false,
    set_field=collector (fields go to a buffer, optionally diffed later), activate_lua=true
    (no-op), tuning=NULL (code defaults — the original reads the same adv_const defaults at
    Init), ui_blocked=false (MS2), register_agent_lua=no-op, cell_counts=city slots 93/94,
    cell_center=city slot 97, disaster_ongoing=disaster layer slot 10, disaster_locations=
    slots 16/17 + instance GetID() (manager type = instance id, disasters.md §1: GetID
    returns the MANAGER's type), building=occupant QI 0x87DDA3A9: GetBuildingType(4)/
    GetCompletionPercent(9, SC4Percentage* → float)/GetBuildingProfile(11: purpose,wealth)/
    GetPosition(occ slot 8)/name=GetBuildingName(occ 123)/groups=GetOccupantGroups(occ 68,
    std::set<uint32_t>), lot=lot mgr GetOccupantLot(14)+GetLocation(21)/HasRoadAccess(28)/
    GetCondition(35)/GetOccupantWealth(41)/lot rect from GetLot bounds, occupant_position=
    occ slot 8, buildings_in_box=occupant mgr slot 40 w/ filter... FIRST CUT: implement
    only what the daily/monthly path + saved fields need (eq/hq/average_age/life_expectancy,
    grids for UpdateVariables), leave trip/dispatch callbacks NULL and NAME the gap in the
    startup log — trip fields only diverge on dispatch activity, which the test city's idle
    MySims don't generate.
  - EQ/HQ feed: residential simulator (city slot 68): GetCellEQByWealth(37)/GetCellHQByWealth(38)
    take (x,z,WealthType), GetAverageAgeMap(41) → cISC4SimGrid<int32_t> GetCellValue,
    GetCellLifeExpectancy(29) returns int32_t AL; grids for UpdateVariables: crime sim
    (slot 33?) GetCrimeGrid, pollution GetPollutionGrid(0..3), aura GetAuraGrid (int8),
    congestion GetCongestionMap, desirability via bds GetDeveloper(0x1010/0x1020/0x1030)
    → GetDesirabilityMap; coverage: police/fire sim GetPolicePowerGrid/GetFireProtectionGrid
    + GetAverageValueInCellRect; stations: police GetPoliceStations(63)/fire GetFireStations
    (SC4List, ilist for fire) + GetStationFunding(occ, int32&, SC4Percentage&); powered
    (power sim IsOccupantPowered 31), watered (plumbing IsBuildingWatered 9).
  - Also verified: SDK headers exist for every sim used (cISC4SimGrid template in SDK dir);
    `SC4Percentage` = struct{float percentage} (SC4Percentage.h:24); cISC4MySim SDK header
    compiled-order caveat is IRRELEVANT for the shadow (we only CALL slots ≤ 20 which match);
    cISC4AdvisorSystem::GetAdvisors returns std::map (avoid; use GetAdvisor(id) probes).
  - Next: (1) write engine/plugin/FC4MySimsShadow.{h,cpp} (decode-and-diff pattern,
    hosts above); (2) wire: FC4Config.cpp kOtherKeys end (~line 320, add `mysims_shadow`),
    cFC4Director.cpp PostAppInit/PreAppShutdown, FC4Observer.cpp PostCityInit + kMsgSimNewMonth
    + PreCityShutdown, FC4Subsystems.cpp after AutomataShadowStartup (line 637), engine/
    CMakeLists.txt plugin list after FC4AutomataShadow (line 447-448), tools/regress.ps1
    line 170 shadows string append, engine/README.md automata bullet area (additive);
    (3) qbuild quick then all; (4) game runs mysims1-sh-1 (shadows) + mysims1-sh-2 clean
    ref_warnings=0; (5) report + build-runs/merge-queue/mysims1.json (abi=false).
4c8d19c4 mysims1: handoff — design researched, baseline gate PASS
93c9ca3e mysims1: lane file, handoff at the baseline
- Round 4 (2026-10-21, main re-merged + design finished; next agent WRITES THE CODE):
  - main merged AGAIN (tip `c09e4226`, glue29 landed; `qbuild quick` PASS; tree clean).
    Wiring line numbers from THIS tree: FC4Subsystems.cpp Resolve() ends after the
    NetToolRegistry block (~line 666, before `ScriptReload();` - insert
    `MySimsShadowStartup();` there); FC4Observer.cpp PreCityShutdown case after line 651
    (`NetToolRegistry::CityShutdown();`) add `MySimsShadowCityShutdown();`;
    PostCityInit case (~line 602, after AutomataDriverCityLoaded) add
    `MySimsShadowCityLoaded(CurrentCity());`; kMsgSimNewMonth case (~line 706, after
    PropDevShadowMonth) add `MySimsShadowMonth(CurrentCity(), julian);`; PostAppInit
    (cFC4Director.cpp ~line 346 after TerrainShadowStart()) add `MySimsShadowStart();`;
    PreAppShutdown (~line 364) add `MySimsShadowStop();`. FC4Config.cpp kOtherKeys ends
    line 320 `"ports_shadow"}};` - add `"mysims_shadow",` before the close with a
    `// the mysims1 lane` comment. engine/CMakeLists.txt: after the nettool block
    (lines ~403-409) add `plugin/FC4MySimsShadow.cpp/.h` with a comment. tools/regress.ps1
    line 170 shadows string: append the mysims entry inside the quotes (additive at end).
  - ABI call map VERIFIED (engine/capi/include/fc4.h): create
    `fc4_advisors_create(&advisor_host, &mysim_host, SessionSeed())`,
    `fc4_mysims_agent_init(p)`; messages `fc4_advisors_do_message(p, &fc4_adv_message)`
    (msg_type/data1..4/void1/unknown) and `fc4_mysims_agent_do_message(p, &m)`;
    PostCityInit + CityEstablished + TICKER all through `fc4_advisors_do_message`
    (the core implements advisors.md 5.1 itself, system.rs on_ticker;
    SYSTEM_SUBSCRIPTIONS = 0x26D31EC1 / 0x26D31EC4 / 0xCA5501F2). Read back:
    `fc4_mysims_agent_get_state`, `fc4_mysims_active(p, out, cap)` (ids),
    `fc4_mysim_get_state(p, id, out)`, `fc4_mysim_text(p, id, which 0=bio 1=path, buf,
    cap)` (full-len return), `fc4_advisors_advisor_state(p, id, out)`
    (month_counter/current_day/cursor/advice_count),
    `fc4_advisors_advisor_text(p, id, 1, buf, cap)` (personal name),
    `fc4_advisors_get_state(p, out)` (last_day, has_mysims). Destroy
    `fc4_advisors_destroy(p)`. fc4_mysim_state field order (for the compare): state
    slot wealth sign is_male age birth_month retired neighbor_job commute_check
    lua_table unnamed_flag eq_index hq_index generation home_cell_x/z neighbor_cell[2]
    commute_cell[2] jobless_days bitmap_id last_commute_time last_local_disaster
    home_position[3] eq hq eq_delta hq_delta eq_ring[6] hq_ring[6] vehicle_key
    car_key pedestrian_key unnamed_pair[2] last_player_trip_day last_trip_update_day
    local_area_size pylon... local_group_count residence job dispatch dispatch_tool.
  - fc4_advisor_host fields the shadow host NEEDS (rest NULL): advisor_count,
    advisor_entry (hardcode the 7 shipped entries: 23 plain, MySims 1-5 class
    0x4A1DBBBF, 30 class 0x0BF606A5; captions/names copied from the original's
    GetAdvisor(id)->GetName()/GetPersonalName() at first city load), advice_count ->
    from the captured advice list (see below), sim_date (city simulator slot 8 +
    fc4_date_from_julian), start_year (simulator GetSimStartDate - CHECK the exact
    SDK slot), has_city true, city_established (city +0x58 raw read with the vtable
    guard, as the other raw reads), is_any_paused (simulator slot 19 IsAnyPaused),
    now_ms 0, subscribe/unsubscribe (record; our target registers at Start()), post
    no-op, post_ticker (push a TICKER message to our own forwarder target),
    set_global no-op, register_api no-op, create_tags (1,2), compile_trigger -> None,
    call_trigger -> OTHER, reload_scripts no-op, collect_garbage no-op, close_lua
    no-op, advisor_window no-op, popup_priority -> None, debug_enabled false,
    register_cheat no-op, object_created -> keep every handle in a std::vector (the
    host owns them) and return an incrementing token; destroy all at
    fc4_advisors_destroy time. fc4_mysim_host: create_table false, set_field = a
    no-op counter, activate_lua false, tuning -> None (code defaults; mysims.md 4.11
    matches), ui_blocked false (MS2), register_agent_lua no-op, cell_counts (city
    slots 93/94), cell_center (slot 97), disaster_ongoing (layer slot 10),
    disaster_locations (layer slots 16/17 + instance GetID), building (occupant QI
    0x87DDA3A9: GetBuildingType slot 4 / GetCompletionPercent 9 (SC4Percentage*) /
    GetBuildingProfile 11 (purpose, wealth) / GetPosition (occ slot 8) / name
    GetBuildingName (slot 123 per cISC4BuildingOccupant.h) / groups
    GetOccupantGroups (occ slot 68, std::set)), lot (lot mgr GetOccupantLot 14 +
    GetLocation 21 / HasRoadAccess 28 / GetCondition 35 / GetOccupantWealth 41; rect
    from the lot manager's GetLot(x,z) bounds - CHECK cISC4Lot for a bounds getter,
    else the single-cell rect), occupant_position (occ 8), buildings_in_box /
    all_buildings (occupant mgr: GetOccupantsByStandardCityCells with type
    0x278128A0, GetOccupantsByBBox with min/max as FC4DemolitionHost.cpp:228 does;
    one reference per element - ReleaseOccupantList slot 48 or hold until the
    outermost call returns, cFC4Flora Hold pattern), lots_in_rect (lot mgr
    GetLotsInCellRect slot 7, the GameList pattern FC4BudgetHost ScanLotList),
    special / education buildings (BDS slot 44 GetSpecialBuildings; residential
    slots 47/49 GetSchoolBuildings/GetHospitalBuildings), grade (residential
    GetSchoolQueryData/GetHospitalQueryData slots 26/27 - CHECK the SDK slot numbers
    before coding), job_title (building GetBuildingName), landmark_funding (budget
    slot 51 GetBudgetItemForPurpose), average_age (residential GetAverageAgeMap
    slot 41 -> grid GetCellValue), eq/hq_by_wealth (residential 37/38),
    life_expectancy (29), new_age_by_wealth (42), grid/grid_at/coverage (crime slot
    5, pollution 5 + arg, aura 12, congestion traffic 10, desirability via BDS
    GetDeveloper(purpose, wealth) -> cISC4GrowthDeveloper GetDesirabilityMap;
    coverage police GetPolicePowerGrid / fire GetFireProtectionGrid +
    GetAverageValueInCellRect), powered (power IsOccupantPowered), watered (plumbing
    IsBuildingWatered), stations (police GetPoliceStations SC4List / fire
    GetFireStations ilist + GetStationFunding), travel_time_ratio (traffic 24),
    trip_simulators true, dispatch -> None (no dispatch core in the shadow: trip
    state unkept), find_new_job false FIRST CUT (if the original finds jobs the
    `job`/`commute_cell` fields will mismatch and the line names them; a follow-up
    can wire cores28's dispatch core), commute_time -> None, dispatch_command no-op,
    post no-op, automata_trigger no-op, subscribe no-op, add_ref/release real
    game refs (guarded, IsOurObject), object_ref Host{token}, resolve_ref identity
    on the capture-token map.
  - NO-LUA decision (final): the shadow core runs WITHOUT Lua (create_table false
    => lua_table false). Saved fields are Lua-independent: bio/age/birth_month from
    the record, eq/hq rings from grids, tuning from code defaults. VERIFY at code
    time: update_trip_variables must no-op with lua_table false (read sim.rs
    654-770 first; if it does not guard, guard it - a core change is in scope for
    this lane and covered by the mysims tests).
  - ADVICE LIST (the shadow's advice_count must match): at first city load walk the
    original MySim's cISC4Advisor GetCurrentAdvices() (slot 10, SC4List of
    {int32, cRZAutoRefCount<cISC4Advice>}; the StationList pattern
    FC4ProtectionShadow.cpp:73; the list belongs to the advisor - borrow) and give
    the shadow host advice_entry(i): guid (advice slot 6 GetGUID), class_id (the
    advice's GetGZCLSID via cIGZSerializable QI), priority/frequency/timeout/mood
    (advice slots per advisors.md 12.3 - CHECK the exact slot order there before
    coding), event 0, command 0, trigger absent. The shadow's advices are then
    structurally identical (all inactive, no triggers): advice_count matches, the
    day pass advances the cursor over the same count, nothing activates
    (compile_trigger None => GetTrigger false). nextDay/expiry/active NOT compared.
  - DECODE-AND-DIFF (final): original per-MySim record (WriteCapture on its
    cIGZSerializable) decoded in C++ with CaptureReader (version-17 layout
    mysims.md 6.2; state==0 short-circuits to the advisor part) vs the shadow's
    fc4_mysim_get_state + fc4_mysim_text + fc4_advisors_advisor_state. Refs compare
    through the capture-id -> token map (WriteCapture::Object(id) -> TokenOf, no
    references held; the original keeps its buildings alive while we compare inside
    one month callback): the shadow's residence/job tokens come from resolve_ref
    returning that token, so fc4_mysim_get_state's residence/job are DIRECTLY
    comparable with the original's getters. The shadow core's add_ref/release take
    real game references (guarded, IsOurObject) - released by the core through
    release(); balance verified by ref_warnings=0 in mysims1-sh-2.
  - INJECT at first city load: for each original MySim (ids 1-5, 30, from
    GetAdvisorSystem -> GetAdvisor(id) QI 0x0A1DA42F non-null): capture its record,
    decode, fc4_advisors_create_object(class from the record), fc4_adv_obj_read with
    the CUSTOM istream whose get_ref (a) advice refs -> create a plain shadow advice
    (fc4_advisors_create_object(0x8A09F5F4)) per orig object id (cached), (b)
    subject refs -> fc4_advisors_new_subject per orig id (cached), (c) building refs
    -> the captured token, (d) dispatch refs -> the game token (or null). Then
    fc4_advisors_add_advisor(p, obj) (map[id], id from the decoded advisor part).
    After all five + 30: fc4_mysims_agent_init(p) (slots rebuild from the map,
    ascending id, 30 -> index 5 - same as the original). Then hand-deliver
    POST_CITY_INIT (+ CITY_ESTABLISHED if established) to fc4_advisors_do_message
    ONCE (flag): the shadow target's own subscription is recorded but NOT
    registered with the game server for these two types, so the hand-delivery is
    the only path (TICKER though IS registered and flows live).
  - LATER MONTHS: no resync; the ticker keeps both sides in step (same day
    counters); a mismatch does not rebuild the core (the next month line shows
    whether the diff persists).
  - COMPARED (final list): per MySim (ids 1-5 + 30): state byte, slot,
    residence/job token (0 = null both sides), home cell, neighbor/commute cells,
    jobless days, vehicle/car/ped keys, wealth, generation, sign, is_male, bio,
    age, birth_month, bitmap_id, portrait path, retired, last_local_disaster,
    home_position[3] (f32 bits), eq/hq/eq_delta/hq_delta/eq_index/hq_index/
    eq_ring[6]/hq_ring[6] (f32 bits), last_player_trip_day, dispatch non-null-ness;
    advisor part: personal name, month_counter, current_day, cursor, advice_count.
    Agent: slots[6], last_disaster_type, active_ui_index. GetActiveMySims (ids +
    order), MySimLivesHere per residence, GetActiveUIPanelSim index. NOT compared
    (named on the line): last_commute_time (pathfinder timing), pylon /
    dispatch-tool state (UI), unnamed pair/flag (MS1), per-advice
    nextDay/expiry/active (no Lua in the shadow), the event queue.
  - SEAM: kFC4SeamMySimsShadow = 68 (next free after 67) with events: 0 = one month
    compare (returns the mismatch count, -1 not taken), 1 = state dump (out struct:
    created, had_mysims, months, clean_months, last_mismatches, agent slots), 2 =
    test city pointer install (like FC4ObserverTestSetCity). Plugin test mode
    `--ini-mysims-shadow` (RUN_SERIAL): ON line, month line mismatches=0 with no
    active MySims (ids 1-5 + 30 exist, state 0), agent slots = [1,2,3,4,5,30].
  - GAME RUNS: mysims1-sh-1 (ini `mysims_shadow = 1`, turbo 1, end_date 2032-01-01):
    ON line + a month line per month mismatches=0 (test city: no MySims placed).
    mysims1-sh-2 (the full regress shadows string + mysims): no interference,
    ref_warnings=0. Diagnose any diff to the field BEFORE further runs.
  - QUESTIONS: none filed this round; MS8-MS10 stay open (not shadow-blocking);
    MS11+ only if a shadow mismatch reveals a spec gap.
  - NEXT: write engine/plugin/FC4MySimsShadow.{h,cpp} (~700 lines: hosts, target,
    capture/decode, inject, ticker, compare, seam); wire the 7 call sites +
    kOtherKeys + CMakeLists + regress.ps1 + engine/README.md (additive bullet after
    the automata shadow bullet); `qbuild quick` then `all`; the 2 game runs; report
    + build-runs/merge-queue/mysims1.json (abi=false).
- Round 5 (2026-10-21, header WRITTEN, impl next; context OVER):
  - engine/plugin/FC4MySimsShadow.h written (uncommitted at stop): public API
    MySimsShadowStartup/Start/Stop/CityLoaded/Month/CityShutdown + seam struct
    FC4MySimsShadowTestInput (op 68, events 0 = one month compare, 1 = state dump with
    slots[6]). Line contract: `mysims shadow YYYY-MM-DD load|month: ... mismatches=N`.
  - Design VERIFIED this round (all facts re-checked in this tree, trust these):
    * Core drive: fc4_advisors_do_message(POST_CITY_INIT 0x26D31EC1 / CITY_ESTABLISHED
      0x26D31EC4 / TICKER 0xCA5501F2) -> system.rs on_ticker (line 431) runs daily+monthly
      itself (MONTHLY_DAY = 28, advisors/mod.rs:398; monthly BEFORE daily on day 28:
      monthly_step line 441, daily_step 457). NO manual simulate calls needed.
    * Agent: fc4_mysims_agent_init AFTER all advisors added (agent.rs init line 82
      rebuilds slots from sys.advisors(), MY_STREET_ID = 30 -> index 5, others ascending
      0-4).
    * Agent messages: forward the 11 AGENT_SUBSCRIPTIONS (mysims/mod.rs:94) via
      fc4_mysims_agent_do_message (capi 2279); void1 = occupant token (CanonicalOccupant
      pattern cFC4Civic.cpp:167), unknown = advisor id for ADVICE_COMMAND (0x8A09FA6C).
      MySim's own 6 (MYSIM_SUBSCRIPTIONS mod.rs:116) + the base's 5 (BASE_SUBSCRIPTIONS
      advisors/mod.rs:240) + the system's 3 (SYSTEM_SUBSCRIPTIONS mod.rs:246) arrive as
      advisor-host subscribe(target_kind, advisor_id, token, msg_type) callbacks: the
      shadow target registers the UNION of subscribed ids at Start() and forwards agent
      ids to fc4_mysims_agent_do_message, system ids to fc4_advisors_do_message, per-
      advisor ids to fc4_advisors_advisor_do_message(p, id, m).
    * fc4_advisor_host (fc4.h 10611-10670) REQUIRED: advisor_count/advisor_entry (7
      entries: plain 23 + MySims 1-5 class 0x4A1DBBBF + 30 class 0x0BF606A5, advice_type
      1; captions/names copied from the original GetAdvisor(id)->GetName()/
      GetPersonalName() at first load), advice_count/advice_entry (inject the original's
      current advices per MySim: cISC4Advisor slot 10 GetCurrentAdvices SC4List of
      pair<int32, cRZAutoRefCount<cISC4Advice>>; entry fields from cISC4Advice slots:
      mood 6, priority 7, GUID 8, frequency 16 GetFrequencyInDays, timeout 17
      GetTimeOutInDays, event 26 GetEventID, command 27 GetCommandID; class_id via
      GetGZCLSID; trigger: compile_trigger -> None), sim_date (cFC4ISC4Simulator
      GetSimDateNumber FC4Interfaces.h:84 + fc4_date_from_julian), start_year
      (GetSimStartDate slot 5, cFC4IGZDate Year slot 13), has_city true,
      city_established (city +0x58 raw read with the vtable guard as other shadows),
      is_any_paused (simulator slot 18, FC4Interfaces.h:94), now_ms 0,
      subscribe/unsubscribe record only, post no-op, post_ticker no-op (the game's own
      ticker reaches the shadow target), set_global no-op, create_tags (1,2),
      compile_trigger None, call_trigger OTHER, reload_scripts/register_api no-op,
      popup_priority None, debug_enabled false, register_cheat no-op, object_created
      (handles kept in a vector; return ++token), add_ref/release real game refs guarded
      by IsOurObject, resolve_ref identity on the token, difficulty None, occupant_* via
      cISC4Occupant, cell_counts city slots 93/94.
    * fc4_mysim_host (fc4.h 10751-10813): create_table FALSE (no Lua), set_field
      counter, activate_lua false, tuning None (code defaults), ui_blocked false (MS2),
      register_agent_lua no-op, cell_counts/cell_center (city 93/94/97), disaster_ongoing
      (layer slot 10), disaster_locations (layer 16/17 + instance GetID), building
      (occupant QI 0x87DDA3A9), lot (lot mgr GetOccupantLot 14), occupant_position
      (occ slot 8), buildings_in_box/all_buildings (occupant mgr GetOccupantsByBBox
      pattern FC4DemolitionHost.cpp:228, hold until the outermost call returns),
      lots_in_rect (lot mgr 7), job_title (building name), landmark_funding (budget 51),
      average_age (residential GetAverageAgeMap -> grid GetCellValue),
      eq/hq_by_wealth (residential GetCellEQByWealth/GetCellHQByWealth(x, z, wealth)),
      life_expectancy (GetCellLifeExpectancy(x, z) int32), new_age_by_wealth
      (GetAverageNewAgeByWealth), grid/grid_at (crime GetCrimeGrid, pollution
      GetPollutionGrid(0..3), aura GetAuraGrid, congestion traffic GetCongestionMap,
      desirability BDS GetDeveloper(0x1010/0x1020/0x1030)), coverage (police
      GetPolicePowerGrid / fire GetFireProtectionGrid + GetAverageValueInCellRect),
      powered (power IsOccupantPowered), watered (plumbing IsBuildingWatered),
      stations (police GetPoliceStations SC4List / fire GetFireStations ilist),
      trip_simulators true; dispatch/commute_time/job_destination/find_new_job NULL
      FIRST CUT (idle city: no dispatch activity, no job search; name the gap on the
      line; follow-up wires cores28's dispatch core), grade/special/education/
      travel_time_ratio NULL first cut (trip fields are not compared), dispatch_command
      no-op, post/automata_trigger no-op, add_ref/release/resolve_ref as the advisor
      host.
    * INJECT (load): per original MySim id (probe city GetAdvisorSystem->GetAdvisor(id)
      QI 0x0A1DA42F non-null for 1-5, 30): WriteCapture on its cIGZSerializable,
      fc4_advisors_create_object(class 0x4A1DBBBF or 0x0BF606A5), fc4_adv_obj_read with
      a CUSTOM istream (CaptureIStream-like but get_ref: advice refs (class
      0x8A09F5F4) -> create a plain shadow advice per captured object id
      (fc4_advisors_create_object(0x8A09F5F4), cached by id) and return core_handle;
      subject refs (0x6A26D050) -> fc4_advisors_new_subject cached; building/dispatch/
      tool refs (0x87DDA3A9/0xEB8B6F9A/0xC9B6D7D6) -> the capture object's token
      (cap.Object(id) -> TokenOf)); then fc4_advisors_add_advisor(p, obj). After all:
      fc4_mysims_agent_init, then ONE fc4_advisors_do_message(POST_CITY_INIT) and
      (CITY_ESTABLISHED when established) - flag-guarded once.
    * COMPARE (load + month): per id 1-5, 30: decode the original's record
      (CaptureReader, version-17 layout mysims.md 6.2 rows 1-31; state 0 short-circuits;
      refs through cap.Object(id)->TokenOf) vs fc4_mysim_get_state + fc4_mysim_text
      (bio 0, path 1). Advisor part after row 31 (advisors.md 11.2: u16 3, table_index,
      caption, name, month_counter, current_day, id, cursor, count, per advice: ref +
      4 i32 + u8 + i32; event count + pairs): compare personal name, month_counter,
      current_day, cursor, advice_count only. Agent: fc4_mysims_agent_get_state (slots,
      last_disaster_type, active_ui_index). Getters: GetActiveMySims (SC4Vector via
      fc4::PushTrivial, compare ids+order), MySimLivesHere per residence,
      GetActiveUIPanelSim. NOT compared (named on the line): last_commute_time, pylon,
      dispatch tool, unnamed pair/flag (MS1), per-advice nextDay/expiry/active, trip
      state, dispatch refs (null-ness only).
    * GETTER SLOTS verified (cISC4City.h lines 140+): GetLotManager/GetOccupantManager/
      GetAuraSimulator/GetBuildingDevelopmentSimulator/GetCrimeSimulator/
      GetFireProtectionSimulator/GetPlumbingSimulator/GetPoliceSimulator/
      GetPollutionSimulator/GetPowerSimulator/GetResidentialSimulator/
      GetTrafficSimulator/GetWeatherSimulator/GetMySimAgentSimulator(line 188)/
      GetDisasterLayer(line 190) all on the SDK header; cFC4ISC4Simulator (FC4Interfaces.h:76)
      GetSimDateNumber 8 / GetSimStartDate 5 / IsAnyPaused 18.
  - NEXT (round 6, first steps): write engine/plugin/FC4MySimsShadow.cpp per the map
    above (~700 lines); wire: FC4Config.cpp kOtherKeys end line 320 (add "mysims_shadow"
    with comment), cFC4Director.cpp PostAppInit ~346 MySimsShadowStart() +
    PreAppShutdown ~364 MySimsShadowStop(), FC4Observer.cpp PostCityInit ~602
    CityLoaded + kMsgSimNewMonth ~706 Month + PreCityShutdown ~651 CityShutdown,
    FC4Subsystems.cpp ~666 MySimsShadowStartup() before ScriptReload() (+ include near
    line 52), FC4TestSeam.h op 68 enum + FC4TestSeam.cpp case, engine/CMakeLists.txt
    plugin list after nettool block (~403-409), tools/regress.ps1 line 170 shadows
    append `mysims_shadow = 1`, engine/README.md bullet after the automata shadow
    bullet. Then `qbuild quick`, `qbuild all`, game runs mysims1-sh-1 (mysims_shadow = 1,
    turbo, end 2032-01-01) + mysims1-sh-2 (shadows string, ref_warnings = 0), report +
    build-runs/merge-queue/mysims1.json (abi=false).
