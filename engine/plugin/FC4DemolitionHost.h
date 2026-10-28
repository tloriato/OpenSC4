// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the Free
// Software Foundation, either version 3 of the License, or (at your option)
// any later version. This program is distributed WITHOUT ANY WARRANTY; see
// the GNU General Public License for more details. You should have received a
// copy of the license along with this program (see LICENSE at the repository
// root); if not, see <https://www.gnu.org/licenses/>.

// The demolition utility's host table (fc4_demolition_host, fc4.h;
// docs/specs/demolition.md) on the city's managers: the occupant manager's
// box and cell-rect queries, the occupant slots, the building / flora /
// network / pole prices, the two shared occupant filters, the lot manager and
// lot developer, the zone manager, the terrain, the network manager, the 3D
// view and the effects.
//
// This is what glue17 left out: without it a replaced utility prices utility
// demolition at 0 and demolishes nothing, so `enable = demolition` stayed
// unwired. Every one of the 86 callbacks is filled here; a missing one reads
// as 0 / false / none in the core, which is exactly the silent mis-price this
// file exists to prevent.
//
// Tokens are object addresses (the core's documented token order: "the
// plugin's tokens are the objects' addresses", core/src/demolition/mod.rs):
//   - an occupant token is its cISC4Occupant*;
//   - a lot token is its cISC4Lot*;
//   - a filter token is its cISC4OccupantFilter*;
//   - a container token is the caller's own container pointer (the `orphans`
//     SC4List<cISC4Occupant*>* and the `demolished` std::set of occupants).
// Tokens the host hands out are BORROWED for the duration of the call; the
// core AddRefs only the view filter (GetCurrentViewFilterInfo) and the lot of
// a deferred lot demolition, and calls `release` for those.
//
// References (HANDOFF §5.1b): the occupant queries return one reference per
// occupant and the lists are held until the outermost core call returns;
// `list_append` takes the orphan's reference (DM11: the list's owner releases
// it); `set_insert` AddRefs only a new insertion (the game's
// std::set<cRZAutoRefCount<cISC4Occupant>>).
//
// Verified slot order. The SDK declares some overloads in reverse order (the
// terrain's three SetAltitude overloads are the ones this host needs), so the
// classes whose slots this file calls are declared here, below, through
// no-overload `cFC4...` shapes in docs/abi/interfaces.tsv order — never by
// subclassing the SDK interface (HANDOFF "General rules for changing glue").
#pragma once

#include "fc4.h"

#include "SC4List.h"

#include <cstdint>
#include <memory>
#include <string>
#include <vector>

class cIGZUnknown;
class cISC4City;
class cISC4Occupant;
class cISC4OccupantFilter;
class cISC4OccupantManager;
class cISC4Lot;
class cISCPropertyHolder;

namespace fc4 {

// ---- verified-order game interfaces (docs/abi/interfaces.tsv) ------------------------------------
// Only the slots this host calls are named; the rest are numbered so the
// vtable keeps the game's order. `[abi-ret]` marks a return the TSV does not
// resolve to one register read (eax-or-void / via-call / unknown / forward);
// the behaviour source for each is cited in FC4DemolitionHost.cpp.

// cISC4OccupantFilter (7 slots; the two shared filters and the caller's).
class cFC4OccupantFilter : public cIGZUnknown {
public:
    virtual bool IsOccupantIncluded(cISC4Occupant* o) = 0;          // 3  [abi-ret] al
    virtual bool IsOccupantTypeIncluded(uint32_t type) = 0;         // 4  [abi-ret] al
    virtual bool IsPropertyHolderIncluded(cISCPropertyHolder* p) = 0;  // 5  [abi-ret] al
};

// cISC4FoundationOccupant (13 slots): the kind the realism test reads.
class cFC4FoundationOccupant : public cIGZUnknown {
public:
    virtual cISC4Occupant* AsOccupant() = 0;                        // 3
    virtual uint32_t GetFoundationType() = 0;                       // 4
    virtual uint32_t SetFoundationType(uint32_t) = 0;               // 5
    virtual float GetFloat3C() = 0;                                 // 6  x87
    virtual uint32_t SetFloat3C(float) = 0;                         // 7
    virtual bool SetSize(const float*) = 0;                         // 8  al
    virtual uint32_t GetKind() = 0;                                 // 9  eax (demolition §4.2 step 5)
    virtual uint32_t SetKind(int8_t) = 0;                           // 10
    virtual uint32_t GetOrientation() = 0;                          // 11
    virtual bool SetOrientation(int32_t) = 0;                       // 12 al
};

// cISC4NetworkOccupant (81 slots): the flag test and the demolition price.
class cFC4NetworkOccupant : public cIGZUnknown {
public:
    virtual uint32_t NetSlot03(uint32_t) = 0;                       // 3
    virtual uint32_t Shutdown() = 0;                                // 4
    virtual bool IsManagedGlobally() = 0;                           // 5  al
    virtual bool CheckForRetrofit() = 0;                            // 6  al
    virtual void SetRetrofitFlag(bool) = 0;                         // 7  void
    virtual uint32_t HasAllNetworkFlags(uint32_t) = 0;              // 8  eax
    virtual uint32_t HasAnyNetworkFlag(uint32_t mask) = 0;          // 9  [abi-ret] eax-or-void (demolition §4.2 step 7)
    // 10..62 are not called here.
    virtual uint32_t NetSlot10(uint32_t) = 0;
    virtual uint32_t NetSlot11(uint32_t) = 0;
    virtual uint32_t NetSlot12() = 0;
    virtual uint32_t NetSlot13(uint32_t, uint32_t) = 0;
    virtual uint32_t NetSlot14(uint32_t, uint32_t) = 0;
    virtual uint32_t NetSlot15() = 0;
    virtual uint32_t NetSlot16(uint32_t, uint32_t) = 0;
    virtual bool IsImmovable() = 0;
    virtual void SetImmovable(bool) = 0;
    virtual bool IsUsable() = 0;
    virtual void SetUsable(bool) = 0;
    virtual uint32_t IsPlaced() = 0;
    virtual uint32_t NetSlot22() = 0;
    virtual uint32_t PieceId() = 0;
    virtual uint32_t NetSlot24() = 0;
    virtual uint32_t NetSlot25() = 0;
    virtual uint32_t GetOrientation() = 0;
    virtual uint32_t NetSlot27(uint32_t) = 0;
    virtual uint32_t NetSlot28() = 0;
    virtual uint32_t NetSlot29(uint32_t) = 0;
    virtual uint32_t SetQuadDraw(uint32_t, uint32_t) = 0;
    virtual uint32_t NetSlot31(uint32_t, uint32_t, uint32_t, uint32_t) = 0;
    virtual uint32_t NetSlot32() = 0;
    virtual uint32_t GetModelInstances(uint32_t, uint32_t, uint32_t, uint32_t, uint32_t) = 0;
    virtual uint32_t NetSlot34(uint32_t) = 0;
    virtual float NetSlot35(uint32_t) = 0;
    virtual uint32_t NetSlot36(uint32_t) = 0;
    virtual uint32_t NetSlot37(uint32_t, uint32_t) = 0;
    virtual uint32_t GetEdgeMaskForType(uint32_t) = 0;
    virtual uint32_t NetSlot39(uint32_t, uint32_t, uint32_t) = 0;
    virtual uint32_t NetSlot40() = 0;
    virtual uint32_t NetSlot41(uint32_t) = 0;
    virtual uint32_t NetSlot42(uint32_t) = 0;
    virtual uint32_t NetSlot43() = 0;
    virtual uint32_t NetSlot44(uint32_t) = 0;
    virtual uint32_t NetSlot45() = 0;
    virtual uint32_t NetSlot46() = 0;
    virtual uint32_t NetSlot47(uint32_t, uint32_t) = 0;
    virtual uint32_t NetSlot48() = 0;
    virtual uint32_t GetPathInfo() = 0;
    virtual void ReleasePathInfo() = 0;
    virtual uint32_t NetSlot51(uint32_t) = 0;
    virtual uint32_t GetBuildingDevelopmentSimulator() = 0;
    virtual uint32_t NetSlot53(uint32_t, uint32_t, uint32_t) = 0;
    virtual uint32_t NetSlot54(uint32_t, uint32_t, uint32_t) = 0;
    virtual uint32_t NetSlot55() = 0;
    virtual float NetSlot56(uint32_t, uint32_t) = 0;
    virtual uint32_t NetSlot57(uint32_t, uint32_t, uint32_t) = 0;
    virtual uint32_t NetSlot58(uint32_t, uint32_t) = 0;
    virtual uint32_t NetSlot59(uint32_t, uint32_t) = 0;
    virtual uint32_t NetSlot60() = 0;
    virtual uint32_t SetLit(uint32_t) = 0;
    virtual bool CanBeLit() = 0;
    virtual int64_t NetSlot63() = 0;                                // 63 edx:eax: the demolition price (demolition §4.3)
};

// cISTETerrain (98 slots). The SDK lists the three SetAltitude overloads in
// the reverse of the game's order (49 rect, 50 vertex, 51 x/z/h), so this
// shape is the verified order of interfaces.tsv.
class cFC4Terrain : public cIGZUnknown {
public:
    virtual uint32_t TerrainSlot03() = 0;
    virtual uint32_t TerrainSlot04() = 0;
    virtual bool IsInitialized() = 0;                               // 5  al
    virtual float CellWidth() = 0;                                  // 6  x87
    virtual uint32_t GetCellCountX() = 0;                           // 7  eax
    virtual uint32_t GetCellCountZ() = 0;                           // 8  eax
    // 9..13
    virtual uint32_t TerrainSlot09(uint32_t, uint32_t) = 0;
    virtual uint32_t TerrainSlot10(uint32_t, uint32_t, uint32_t) = 0;
    virtual uint32_t TerrainSlot11(uint32_t, uint32_t) = 0;
    virtual float GetAltitude(float) = 0;                           // 12 x87
    virtual float GetAltitude(float, float) = 0;                    // 13 x87
    virtual float GetAltitudeAtVertex(int32_t x, int32_t z) = 0;    // 14 x87 (demolition §4.7)
    // 15..36
    virtual uint32_t TerrainSlot15(uint32_t, uint32_t, uint32_t, uint32_t) = 0;
    virtual uint32_t TerrainSlot16(uint32_t, uint32_t, uint32_t, uint32_t) = 0;
    virtual uint32_t TerrainSlot17(uint32_t, uint32_t, uint32_t) = 0;
    virtual float TerrainSlot18(uint32_t, uint32_t) = 0;
    virtual float TerrainSlot19(uint32_t, uint32_t) = 0;
    virtual float TerrainSlot20(uint32_t) = 0;
    virtual float MinHeight() = 0;
    virtual float MaxHeight() = 0;
    virtual float MinAllowedHeight() = 0;
    virtual float MaxAllowedHeight() = 0;
    virtual uint32_t TerrainSlot25(uint32_t, uint32_t) = 0;
    virtual uint32_t TerrainSlot26(uint32_t, uint32_t, uint32_t) = 0;
    virtual uint32_t TerrainSlot27(uint32_t, uint32_t, uint32_t) = 0;
    virtual uint32_t TerrainSlot28(uint32_t, uint32_t, uint32_t) = 0;
    virtual float GetSeaLevel() = 0;
    virtual float TerrainSlot30(float, float) = 0;
    virtual float TerrainSlot31(uint32_t) = 0;
    virtual float TerrainSlot32(float, float) = 0;
    virtual float TerrainSlot33(uint32_t, uint32_t) = 0;
    virtual bool CellHasWater(uint32_t, uint32_t) = 0;
    virtual bool CellHasLand(uint32_t, uint32_t) = 0;
    virtual bool IsLandVertex(uint32_t, uint32_t) = 0;
    virtual bool IsWaterVertex(int32_t x, int32_t z) = 0;           // 37 al (demolition §4.7)
    // 38..48
    virtual bool IsSeaVertex(uint32_t, uint32_t) = 0;
    virtual uint32_t AreSeaVertices(uint32_t) = 0;
    virtual uint32_t DoPostCityInit() = 0;
    virtual uint32_t SetTerrainType(uint32_t) = 0;
    virtual uint32_t GetTerrainType() = 0;
    virtual uint32_t CreateFlatAreas(uint32_t) = 0;
    virtual uint32_t CreateShoreCliffs(uint32_t) = 0;
    virtual uint32_t SetSeaLevel(float) = 0;
    virtual uint32_t ChangeSeaLevel(uint32_t) = 0;
    virtual float GetDefaultTerrainRaiseLowerAmount() = 0;
    virtual uint32_t ChangeTerrainLevel(uint32_t, uint32_t) = 0;
    // The three SetAltitude overloads in the GAME's order (interfaces.tsv
    // 49 / 50 / 51); the SDK lists them reversed.
    virtual void SetAltitudeRect(fc4_cell_rect const& r, float h, bool, uint32_t reason) = 0;  // 49 [abi-ret] eax-or-void
    virtual void SetAltitudeAtVertex(int32_t x, int32_t z, float h) = 0;                       // 50 [abi-ret] eax-or-void
    virtual void SetAltitudeXYZ(int32_t x, int32_t z, float h) = 0;                            // 51 void (demolition §4.7)
    // 52..67
    virtual uint32_t LockAltitude(uint32_t, uint32_t) = 0;
    virtual uint32_t NumberOfWaterVertices(uint32_t) = 0;
    virtual uint32_t TerrainSlot54() = 0;
    virtual float TerrainSlot55(uint32_t, uint32_t) = 0;
    virtual uint32_t StartFlood() = 0;
    virtual bool ApplyBrush(uint32_t, uint32_t) = 0;
    virtual uint32_t GetBrushExemplarID(uint32_t, uint32_t) = 0;
    virtual uint32_t BeginBrushScrub(uint32_t) = 0;
    virtual uint32_t EndBrushScrub(uint32_t) = 0;
    virtual float TerrainSlot61(uint32_t) = 0;
    virtual int64_t TerrainSlot62(uint32_t) = 0;
    virtual uint32_t TerrainSlot63(uint32_t, uint32_t) = 0;
    virtual uint32_t UpdateWaveProducers() = 0;
    virtual uint32_t TerrainSlot65(uint32_t, uint32_t, uint32_t) = 0;
    virtual uint32_t GetView() = 0;
    virtual uint32_t DoLighting(uint32_t) = 0;
    virtual void RedisplayTerrain(bool, bool, fc4_cell_rect const& r, uint32_t reason) = 0;    // 68 void (demolition §4.7)
};

// cISC4FloraSimulator (26 slots): the flora demolition price.
class cFC4FloraSim : public cIGZUnknown {
public:
    virtual uint32_t FloraSlot03() = 0;
    virtual uint32_t FloraSlot04() = 0;
    virtual uint32_t FloraSlot05(uint32_t, uint32_t) = 0;
    virtual uint32_t FloraSlot06(uint32_t) = 0;
    virtual uint32_t FloraSlot07(uint32_t) = 0;
    virtual uint32_t FloraSlot08(uint32_t) = 0;
    virtual uint32_t FloraSlot09(uint32_t, uint32_t) = 0;
    virtual uint32_t FloraSlot10(uint32_t) = 0;
    virtual uint32_t FloraSlot11(uint32_t) = 0;
    virtual uint32_t FloraSlot12(uint32_t) = 0;
    virtual uint32_t FloraSlot13(uint32_t, uint32_t) = 0;
    virtual uint32_t FloraSlot14(uint32_t, uint32_t) = 0;
    virtual uint32_t FloraSlot15(uint32_t, uint32_t, uint32_t, uint32_t) = 0;
    virtual uint32_t FloraSlot16(uint32_t, uint32_t, uint32_t) = 0;
    virtual uint32_t FloraSlot17(uint32_t, uint32_t, uint32_t) = 0;
    virtual uint32_t FloraSlot18(uint32_t) = 0;
    virtual uint32_t FloraSlot19(uint32_t, uint32_t) = 0;
    virtual uint32_t FloraSlot20(uint32_t, uint32_t) = 0;
    virtual uint32_t FloraSlot21(uint32_t, uint32_t, uint32_t) = 0;
    virtual int64_t GetCostOfOccupantDemolition(cISC4Occupant* o, int32_t flags) = 0;  // 22 edx:eax
};

// cISC4NetworkManager (25 slots): both are void in the TSV.
class cFC4NetworkManager : public cIGZUnknown {
public:
    virtual uint32_t NetMgrSlot03() = 0;
    virtual uint32_t NetMgrSlot04() = 0;
    virtual uint32_t GetNetworkTool(uint32_t, bool) = 0;            // 5  eax
    virtual void DestroyNetworkOccupants(SC4List<cISC4Occupant*>& list) = 0;  // 6  void (demolition §4.6)
    virtual void QueueRemovals(bool on) = 0;                        // 7  void (demolition §4.4 step 7)
};

// cISC4View3DWin (61 slots): the view filter and level of §4.4 step 2 / §4.2.
class cFC4View3D : public cIGZUnknown {
public:
    virtual uint32_t ViewSlot03() = 0;
    virtual uint32_t GetRenderer() = 0;
    virtual uint32_t ViewSlot05() = 0;
    virtual uint32_t ViewSlot06() = 0;
    virtual uint32_t ViewSlot07(uint32_t, uint32_t, uint32_t) = 0;
    virtual uint32_t ViewSlot08(uint32_t) = 0;
    virtual uint32_t ViewSlot09() = 0;
    virtual uint32_t ViewSlot10() = 0;
    virtual uint32_t ViewSlot11(uint32_t, uint32_t) = 0;
    virtual bool ViewSlot12() = 0;
    virtual bool ViewSlot13() = 0;
    virtual uint32_t ViewSlot14(uint32_t) = 0;
    virtual uint32_t ViewSlot15() = 0;
    virtual uint32_t ViewSlot16() = 0;
    virtual uint32_t ViewSlot17(uint32_t, uint32_t) = 0;
    virtual void GetCurrentViewFilterInfo(void* out) = 0;           // 18 [abi-ret] via-call (demolition §4.4 step 2)
    virtual uint32_t ViewSlot19(uint32_t) = 0;
    virtual uint32_t ViewSlot20(uint32_t) = 0;
    virtual uint32_t ViewSlot21(uint32_t) = 0;
    virtual uint32_t GetCurrentViewLevel() = 0;                     // 22 [abi-ret] eax-or-void (demolition §4.2 step 7)
};

// cISC4PropManager (41 slots): slot 29 is RemoveProp(occ) (the spec's
// `RemovePropA`).
class cFC4PropManager2 : public cIGZUnknown {
public:
    virtual bool PropMgr03() = 0;
    virtual bool PropMgr04() = 0;
    virtual uint32_t PropMgr05(uint32_t) = 0;
    virtual uint32_t PropMgr06(uint32_t) = 0;
    virtual bool PropMgr07(uint32_t, uint32_t) = 0;
    virtual bool PropMgr08(uint32_t) = 0;
    virtual uint32_t PropMgr09(uint32_t, uint32_t) = 0;
    virtual bool PropMgr10(uint32_t, uint32_t) = 0;
    virtual bool PropMgr11(uint32_t) = 0;
    virtual uint32_t PropMgr12(uint32_t) = 0;
    virtual uint32_t PropMgr13(uint32_t) = 0;
    virtual uint32_t PropMgr14(uint32_t) = 0;
    virtual bool PropMgr15(uint32_t, uint32_t) = 0;
    virtual bool PropMgr16(uint32_t, uint32_t) = 0;
    virtual bool PropMgr17(uint32_t, uint32_t) = 0;
    virtual bool PropMgr18(uint32_t, uint32_t) = 0;
    virtual bool PropMgr19(uint32_t) = 0;
    virtual uint32_t PropMgr20(uint32_t, uint32_t, uint32_t) = 0;
    virtual bool PropMgr21(uint32_t) = 0;
    virtual bool PropMgr22(uint32_t) = 0;
    virtual bool PropMgr23(uint32_t, uint32_t, uint32_t) = 0;
    virtual bool PropMgr24(uint32_t, uint32_t) = 0;
    virtual bool PropMgr25(uint32_t, uint32_t) = 0;
    virtual uint32_t PropMgr26(uint32_t, uint32_t, uint32_t) = 0;
    virtual bool PropMgr27(uint32_t, uint32_t) = 0;
    virtual bool PropMgr28(uint32_t) = 0;
    virtual bool RemoveProp(cISC4Occupant* o) = 0;                  // 29 al (demolition §4.3)
};

// ---- the host context -----------------------------------------------------------------------------

struct DemolitionHostContext {
    cISC4City* city = nullptr;       // borrowed for the city session
    void* messageTarget = nullptr;   // the GZCOM class's cIGZMessageTarget2*

    // The game's two shared occupant filters (demolition.md §4.2 step 1:
    // cSC4DefaultOccupantFilter and cSC4RealisticOccupantFilter). Both are
    // created here once and answered through the shared helper below; the
    // game exposes no getter for its own instances.
    uint64_t defaultFilter = 0;    // token of our cFC4OccupantFilter, 0 = none
    uint64_t realisticFilter = 0;  // token of our cFC4OccupantFilter, 0 = none

    // Lists from the occupant queries, each holding one reference per
    // occupant, kept until DemolitionHostReleaseHeld.
    std::vector<std::shared_ptr<void>> held;

    // Callbacks whose game call is not made (logged once each): the effects
    // are presentation only and the spec lets a replaced utility do nothing
    // there (§4.8 [choice]).
    std::vector<std::string> notWired;
};

// The host table over `ctx` (which must outlive the core).
fc4_demolition_host MakeDemolitionHost(DemolitionHostContext* ctx);

// Releases every held occupant query list (city shutdown, after the core is
// destroyed) and destroys the two shared filters.
void DemolitionHostReleaseHeld(DemolitionHostContext* ctx);
void DemolitionHostReleaseAll(DemolitionHostContext* ctx);

// The two shared occupant filters, created on first use and destroyed by
// DemolitionHostReleaseAll. `DemolitionDefaultFilterIncludes` and
// `DemolitionRealisticFilterIncludes` are the §4.2 step 1 verdicts.
bool DemolitionDefaultFilterIncludes(cISC4Occupant* o);
bool DemolitionRealisticFilterIncludes(cISC4Occupant* o, cISC4City* city = nullptr);

// Visits query-owned references, then releases and clears each list value so
// the SDK list destructor only frees nodes (one release per query reference).
void DemolitionVisitOccupants(SC4List<cISC4Occupant*>& list, fc4_token_callback cb, void* ctx);

// Every occupant the manager lists in the world box / the standard-city-cell
// rectangle, one reference each until the list goes (`holdIn`: kept in the
// context until DemolitionHostReleaseHeld). The box query is occupant manager
// slot 36 and the cell-rect query slot 40 (slot 41 is its typed thunk).
std::shared_ptr<void> DemolitionOccupantsInBox(cISC4City* city, float x0, float z0, float x1, float z1,
                                              DemolitionHostContext* holdIn);
std::shared_ptr<void> DemolitionOccupantsInRect(cISC4City* city, const fc4_cell_rect* r,
                                               DemolitionHostContext* holdIn);

// The city's managers this host uses (borrowed; nullptr without a city).
cISC4OccupantManager* DemolitionOccupantManager(cISC4City* city);
cFC4Terrain* DemolitionTerrain(cISC4City* city);

}  // namespace fc4
