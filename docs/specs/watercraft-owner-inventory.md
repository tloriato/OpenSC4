# Watercraft: ownership census (2026-10-22)

Companion to [watercraft.md](watercraft.md), WC3. This is address/size/name metadata, not recovered source.

The task's 79 functions / 38,556 bytes could not be reproduced. The attribution algorithm at lane base
`d1fbb50a` assigns **75 functions / 38,073 bytes** using the Windows export, committed names, vtables, interfaces,
library/range rules and noncode-reference exclusions. The discrepancy is four functions / 483 bytes. No four
addresses are invented to fill it, and no coverage-map or attribution input was changed. WC3 stays open.

The census was established before tracing the new behavior paths. It includes reached helpers and inherited
interface thunks; attribution does not prove a distinct cSC4Watercraft method/signature or full specification.
`name-high` comes from committed names; `reach`, `vtable` and `neighbour` are attribution rules. An unnamed row
retains its address as its identifier. Sizes are exported body-byte counts, not subtraction of adjacent addresses.

| Address | Body bytes | Export/committed name | Attribution |
|---|---|---|---|
| `0x004FAF30` | 8 | unnamed | vtable |
| `0x004FAF40` | 8 | unnamed | vtable |
| `0x00508740` | 675 | unnamed | reach |
| `0x00566D60` | 62 | unnamed | reach |
| `0x00566E00` | 35 | unnamed | reach |
| `0x00566EE0` | 30 | `cSC4Watercraft::Write` | name-high |
| `0x00566F00` | 48 | `cSC4Watercraft::Read` | name-high |
| `0x00566F30` | 36 | `cSC4Watercraft::QueryInterface` | name-high |
| `0x00566F60` | 113 | `cSC4Watercraft::Assist_PickNewDir` | name-high |
| `0x00567000` | 94 | unnamed | reach |
| `0x00567080` | 329 | unnamed | reach |
| `0x005671D0` | 312 | unnamed | reach |
| `0x00567310` | 1845 | `cSC4Watercraft::LoadDriveParameters` | name-high |
| `0x00567A50` | 85 | unnamed | reach |
| `0x00567AB0` | 86 | unnamed | reach |
| `0x00567B10` | 113 | unnamed | reach |
| `0x00567DF0` | 131 | `cSC4Watercraft::OnLand` | name-high |
| `0x00567E80` | 74 | `cSC4Watercraft::EndTowing` | name-high |
| `0x00567ED0` | 241 | unnamed | reach |
| `0x00567FD0` | 478 | unnamed | reach |
| `0x005681B0` | 118 | `cSC4Watercraft::IsOutsideCity` | name-high |
| `0x00568230` | 185 | unnamed | reach |
| `0x005682F0` | 464 | unnamed | reach |
| `0x00568570` | 582 | `cSC4Watercraft::Crash_DetermineCase` | name-high |
| `0x005687C0` | 735 | `cSC4Watercraft::Crash_InanimateCollisionDetection` | name-high |
| `0x005690D0` | 333 | unnamed | reach |
| `0x00569230` | 630 | `cSC4Watercraft::DoMessage` | name-high |
| `0x005694B0` | 1072 | `cSC4Watercraft::DoFollowingTick` | name-high |
| `0x005698E0` | 88 | `cSC4Watercraft::Engine_MovementPossible` | name-high |
| `0x00569940` | 1314 | unnamed | reach |
| `0x00569E70` | 1172 | unnamed | reach |
| `0x0056A310` | 299 | unnamed | reach |
| `0x0056A440` | 2883 | unnamed | reach |
| `0x0056AF90` | 3506 | unnamed | reach |
| `0x0056BD50` | 47 | unnamed | reach |
| `0x0056BD80` | 778 | `cSC4Watercraft::DoUndockingTick` | name-high |
| `0x0056C090` | 914 | `cSC4Watercraft::Engine_DetectCollision` | name-high |
| `0x0056C430` | 1051 | `cSC4Watercraft::Movement_PositionNew` | name-high |
| `0x0056D0E0` | 556 | `cSC4Watercraft::Shutdown` | name-high |
| `0x0056D310` | 440 | `cSC4Watercraft::DoPlayerDriveTick` | name-high |
| `0x0056D4D0` | 86 | `cSC4Watercraft::EndDocking` | name-high |
| `0x0056D530` | 189 | `cSC4Watercraft::Ferry_ClearDockRequests` | name-high |
| `0x0056D9A0` | 699 | `cSC4Watercraft::cSC4Watercraft` | name-high |
| `0x0056DC60` | 6 | `cSC4Watercraft::GetGZCLSID` | name-high |
| `0x0056DC70` | 8 | unnamed | vtable |
| `0x0056DC80` | 8 | unnamed | vtable |
| `0x0056DC90` | 8 | unnamed | vtable |
| `0x0056DCA0` | 8 | unnamed | vtable |
| `0x0056DCB0` | 8 | unnamed | vtable |
| `0x0056DCC0` | 8 | unnamed | vtable |
| `0x0056DCD0` | 8 | unnamed | neighbour |
| `0x0056DCE0` | 8 | unnamed | vtable |
| `0x0056DCF0` | 8 | `__ZThn4_N14cSC4Watercraft8ShutdownEv` | name-high |
| `0x0056DD00` | 30 | `cSC4Watercraft::~cSC4Watercraft` | name-high |
| `0x0056DD20` | 286 | unnamed | reach |
| `0x0056DE40` | 1274 | `cSC4Watercraft::Init` | name-high |
| `0x0056E340` | 526 | `cSC4Watercraft::HandleEnterState` | name-high |
| `0x0056E5C0` | 2577 | `cSC4Watercraft::TranslatePlayerDriveCommands` | name-high |
| `0x0056EFE0` | 508 | unnamed | reach |
| `0x0056F1E0` | 514 | unnamed | reach |
| `0x0056F3F0` | 141 | `cSC4Watercraft::Ferry_InitializeCurrentRoute` | name-high |
| `0x0056F480` | 130 | `cSC4Watercraft::Ferry_OtherStationInformation` | name-high |
| `0x0056F510` | 344 | `cSC4Watercraft::SetNbrRouteData` | name-high |
| `0x0056F670` | 136 | `cSC4Watercraft::Ferry_SetStationInformation` | name-high |
| `0x0056F700` | 52 | `cSC4Watercraft::Ferry_SwitchActiveStation` | name-high |
| `0x0056F7B0` | 8 | `__ZThn4_N14cSC4Watercraft4InitEv` | name-high |
| `0x00570140` | 220 | `cSC4Watercraft::HandleExitState` | name-high |
| `0x00570270` | 598 | `cSC4Watercraft::DoCrashSequenceTick` | name-high |
| `0x005704D0` | 322 | `cSC4Watercraft::Ferry_DockRequests` | name-high |
| `0x00570620` | 1212 | `cSC4Watercraft::Engine_AIPickLocation` | name-high |
| `0x00570AE0` | 353 | `cSC4Watercraft::GetDebugExtraText` | name-high |
| `0x00571880` | 2779 | `cSC4Watercraft::Engine_AIBoatMotion` | name-high |
| `0x00572770` | 1406 | `cSC4Watercraft::DoDockingTick` | name-high |
| `0x00572CF0` | 1342 | `cSC4Watercraft::DoAnimationTick` | name-high |
| `0x00573340` | 293 | `cSC4Watercraft::DoStateTick` | name-high |

## Contract boundaries

Traced signatures: Init() returns boolean; QueryInterface(iid,out) returns boolean;
Read(stream)/Write(stream) delegate the base boolean as described in watercraft §3.1; GetGZCLSID() returns
`0x096E75F5`; DoAnimationTick(float A,float B) returns boolean; Movement_PositionNew(float* dt) publishes pose;
DoFollowingTick(float dt) returns a boolean. Address 0x00568230 takes float dt by value plus transform and
candidate-position outputs. Address 0x00567080 takes a pointer to the signed command and returns on the x87 stack.
These are behavioral call shapes, not declarations for an ABI change. Untraced signatures are not guessed.

Direct-call evidence corrects the old helper grouping: Movement_PositionNew calls 0x00569940, 0x00568230 and
0x00569E70. DoCrashSequenceTick calls 0x0056A440 for state 71 and 0x0056AF90 for state 70. The latter two are
crash-motion bodies, not missing direct movement callees. Their complete equations remain WC5 residuals.

Constructor/destructor closure, all inherited step internals, full helper signatures and the 79-versus-75 census
discrepancy remain open. No complete-owner implementation/specification/verification/replacement credit follows
from this table.
