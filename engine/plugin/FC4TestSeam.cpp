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

#include "FC4TestSeam.h"
#include "cFC4LotConfiguration.h"
#include "FC4AutomataShadow.h"
#include "FC4AdvisorsShadow.h"
#include "FC4OrdinanceShadow.h"
#include "cFC4PowerPoleOccupant.h"
#include "cFC4ConstructionRecord.h"
#include "cFC4NetworkLotManager.h"
#include "cFC4NetworkManager.h"

#include "FC4AuraShadow.h"
#include "FC4BudgetShadow.h"
#include "FC4CrimeShadow.h"
#include "FC4DemandShadow.h"
#include "FC4MySimsShadow.h"  // the mysims1 lane: mysims_shadow
#include "FC4DevelopmentShadow.h"
#include "FC4Dialogs.h"
#include "FC4FlammabilityShadow.h"
#include "FC4FloraShadow.h"
#include "FC4GrowthProbe.h"
#include "FC4HistClock.h"
#include "FC4LandValueShadow.h"
#include "FC4LandfillShadow.h"
#include "FC4Fpu.h"
#include "FC4Log.h"
#include "FC4Observer.h"
#include "FC4HostGrid.h"
#include "FC4Streams.h"
#include "FC4PlumbingShadow.h"
#include "FC4PollutionShadow.h"
#include "FC4PowerShadow.h"
#include "FC4ProtectionShadow.h"
#include "FC4ResidentialShadow.h"
#include "FC4Script.h"
#include "FC4TrafficShadow.h"
#include "FC4TerrainShadow.h"  // the glue16 lane
#include "FC4CitySitShadow.h"     // the glue17 lane
#include "FC4DemolitionShadow.h"  // the glue17 lane
#include "FC4DemolitionHost.h"
#include "cFC4Demolition.h"
#include "FC4DispatchShadow.h"    // the glue17 lane
#include "cFC4Terrain.h"        // the glue18 lane
#include "FC4Popups.h"
#include "FC4Properties.h"
#include "FC4RefAudit.h"
#include "cFC4Crime.h"
#include "cFC4Demand.h"
#include "cFC4Flammability.h"
#include "cFC4Landfill.h"
#include "cFC4Neighbors.h"
#include "cFC4Plumbing.h"
#include "cFC4Power.h"
#include "cFC4TrafficSimulator.h"
#include "cFC4Automata.h"  // the verify-automatarefs4 lane (AutomataCmTeardownTest)
#include "cFC4ZoneManager.h"
#include "cFC4TractDeveloper.h"
#include "cFC4LotDeveloper.h"
#include "cFC4Parks.h"  // the cmglue lane
#include "cFC4ZoneDeveloper.h"
#include "cFC4Protection.h"
#include "cFC4Variant.h"
#include "fc4.h"
#include "FC4NetToolHost.h"

#include <cstring>
#include <vector>
#include "FC4GZFramework.h"
#include "FC4DemolitionHost.h"

extern "C" __declspec(dllexport) intptr_t __cdecl FC4TestSeam(uint32_t op, void* a, void* b) {
    switch (op) {
        case kFC4SeamBudgetPostSink:
            fc4::Log("test seam: budget post sink %s", a ? "installed" : "removed");
            fc4::SetBudgetPostSink(reinterpret_cast<FC4PostSinkFn>(a), b);
            return 1;
        case kFC4SeamObserverCity:
            fc4::ObserverTestSetCity(static_cast<cISC4City*>(a));
            return 1;
        case kFC4SeamObserverTick:
            fc4::ObserverTestTick();
            return 1;
        case kFC4SeamObserverMessage:
            fc4::ObserverTestMessage(static_cast<uint32_t>(reinterpret_cast<uintptr_t>(a)), reinterpret_cast<intptr_t>(b));
            return 1;
        case kFC4SeamObserverApp:
            fc4::ObserverTestSetApp(static_cast<cISC4App*>(a));
            fc4::PopupDialogsTestSetApp(static_cast<cISC4App*>(a));
            return 1;
        case kFC4SeamPowerBudgetPath:
            return PowerTestBudgetPath(static_cast<cISC4BudgetSimulator*>(a));
        case kFC4SeamFlammabilityFire:
            return FlammabilityTestFirePath(static_cast<cISC4City*>(a));
        case kFC4SeamBudgetShadow:
            return BudgetShadowTestMonth(static_cast<const FC4ShadowTestInput*>(a));
        case kFC4SeamDemandShadow:
            return DemandShadowTestDeliver(static_cast<const FC4DemandShadowTestInput*>(a));
        case kFC4SeamCrimeShadow:
            return CrimeShadowTestRun(static_cast<const FC4CrimeShadowTestInput*>(a));
        case kFC4SeamAuraShadow:
            return AuraShadowTestCityLoad(static_cast<const FC4AuraShadowDeliverInput*>(a));
        case kFC4SeamDevelopmentShadow:
            return DevelopmentShadowTest(static_cast<const FC4DevelopmentShadowTestInput*>(a));
        case kFC4SeamAuraShadowDeliver:
            return AuraShadowTestDeliver(static_cast<const FC4AuraShadowDeliverInput*>(a));
        case kFC4SeamRefAuditRelease: {
            cIGZUnknown* o = static_cast<cIGZUnknown*>(a);
            const intptr_t mode = reinterpret_cast<intptr_t>(b);
            const long before = fc4::RefWarningCount();
            if ((mode & 1) != 0) FC4_ACQUIRED(o, "test seam");
            if ((mode & 2) != 0) {
                FC4_RELEASE_CREATED(o);
            } else if ((mode & 4) != 0) {
                FC4_RELEASE_AT_DESTRUCTION(o);
            } else {
                FC4_RELEASE(o);
            }
            return fc4::RefWarningCount() - before;
        }
        case kFC4SeamPollutionShadow:
            return PollutionShadowTestDeliver(static_cast<const FC4PollutionShadowTestInput*>(a));
        case kFC4SeamFloraShadow:
            return FloraShadowTestDeliver(static_cast<const FC4FloraShadowTestInput*>(a), static_cast<intptr_t*>(b));
        case kFC4SeamLandfillShadow:
            return LandfillShadowTestDeliver(static_cast<const FC4LandfillShadowTestInput*>(a));
        case kFC4SeamTrafficShadow:
            return TrafficShadowTestDeliver(static_cast<const FC4TrafficShadowTestInput*>(a));
        case kFC4SeamPowerShadow:
            return PowerShadowTestDeliver(static_cast<const FC4PowerShadowTestInput*>(a));
        case kFC4SeamPlumbingShadow:
            return PlumbingShadowTestRun(static_cast<const FC4PlumbingShadowTestInput*>(a));
        case kFC4SeamDemandCapacity: {
            const auto* in = static_cast<const FC4DemandCapacityTestInput*>(a);
            if (in == nullptr) return -1;
            return DemandReportCapacities(nullptr, in->buildingType, in->profile, in->visit, in->ctx);
        }
        case kFC4SeamLandValueShadow:
            return LandValueShadowTest(static_cast<const FC4LandValueShadowTestInput*>(a));
        case kFC4SeamFlammabilityShadow:
            return FlammabilityShadowTest(static_cast<const FC4FlammabilityShadowTestInput*>(a));
        case kFC4SeamResidentialShadow:
            return ResidentialShadowTestDeliver(static_cast<const FC4ResidentialShadowTestInput*>(a));
        case kFC4SeamRawArrayBytes: {
            auto* io = static_cast<FC4RawArrayBytesTestIo*>(b);
            if (a == nullptr || io == nullptr) return -1;
            std::vector<uint8_t> bytes;
            if (!fc4::props::GetRawArrayBytes(static_cast<const cISCPropertyHolder*>(a), io->id, bytes)) return 0;
            io->len = static_cast<uint32_t>(bytes.size());
            if (!bytes.empty())
                std::memcpy(io->bytes, bytes.data(), bytes.size() < sizeof(io->bytes) ? bytes.size() : sizeof(io->bytes));
            return 1;
        }
        case kFC4SeamLotCenter:
            return fc4::CrimeTestLotCenter(a, static_cast<float*>(b));
        case kFC4SeamCellCenter: {
            auto* io = static_cast<FC4CellCenterTestIo*>(b);
            if (a == nullptr || io == nullptr) return -1;
            fc4::ProtectionContext pc;
            pc.readOnly = true;
            pc.city = static_cast<cISC4City*>(a);
            const fc4_protection_host ph = fc4::MakeProtectionReadOnlyHost(&pc);
            ph.cell_center_to_position(ph.user, io->cx, io->cz, &io->protectionX, &io->protectionZ);
            fc4::LandfillContext lc;
            lc.readOnly = true;
            lc.city = static_cast<cISC4City*>(a);
            const fc4_landfill_host lh = fc4::MakeLandfillReadOnlyHost(&lc);
            lh.cell_center_to_position(lh.user, io->cx, io->cz, &io->landfillX, &io->landfillZ);
            return 1;
        }
        case kFC4SeamPipeBurst: {
            const int32_t* xz = static_cast<const int32_t*>(b);
            if (a == nullptr || xz == nullptr) return -1;
            return fc4::PlumbingTestPipeBurst(static_cast<cISC4City*>(a), xz[0], xz[1]);
        }
        case kFC4SeamVariant: {
            if (a == nullptr) return -1;
            const cFC4Variant proto;
            cFC4IGZVariant* copy = nullptr;
            if (!proto.CreateCopy(&copy)) return -1;
            *static_cast<cFC4IGZVariant**>(a) = copy;
            return 1;
        }
        case kFC4SeamX87Check: {
            static fc4::X87Site site = {"test seam", 0, 0, 0, nullptr};
            return fc4::X87Check(site);
        }
        case kFC4SeamRustPanic:
            fc4_debug_panic();
            return 1;  // only if the build unwinds
        case kFC4SeamProtectionShadowVtables:
            fc4::ProtectionShadowTestOriginalVtables(a, b);
            return 1;
        case kFC4SeamZones:
            return a == nullptr ? -1 : ZonesTestSeam(static_cast<const FC4ZonesTestInput*>(a));
        case kFC4SeamTraffic:
            return a == nullptr ? -1 : TrafficTestSeam(static_cast<const FC4TrafficTestInput*>(a));
        case kFC4SeamTract:
            return a == nullptr ? -1 : TractTestSeam(static_cast<const FC4TractTestInput*>(a), b);
        case kFC4SeamLotDev:
            return a == nullptr ? -1 : LotDevTestSeam(static_cast<const FC4LotDevTestInput*>(a), b);
        case kFC4SeamZoneDev:
            return a == nullptr ? -1 : ZoneDevTestSeam(static_cast<const FC4ZoneDevTestInput*>(a), b);
        case kFC4SeamGrowthProbe:
            return GrowthProbeTest(static_cast<const FC4GrowthProbeTestInput*>(a));
        case kFC4SeamScript:  // the harness lane (FC4Script.h)
            return fc4::ScriptLoad(static_cast<const char*>(a), "test seam");
        case kFC4SeamDialogs:  // the harness lane (FC4Dialogs.h)
            return DialogsTestSeam(static_cast<const FC4DialogsTestInput*>(a));
        case kFC4SeamHistClock:  // the history and clock lane (FC4HistClock.h)
            return HistClockTestSeam(static_cast<const FC4HistClockTestInput*>(a));
        case kFC4SeamTerrainShadow:  // the glue16 lane (FC4TerrainShadow.h)
            return TerrainShadowTest(static_cast<FC4TerrainShadowTestInput*>(a));
        case kFC4SeamCitySitShadow:  // the glue17 lane (FC4CitySitShadow.h)
            return CitySitShadowTest(static_cast<FC4CitySitShadowTestInput*>(a));
        case kFC4SeamDemolitionShadow:  // the glue17 lane (FC4DemolitionShadow.h)
            return DemolitionShadowTest(static_cast<FC4DemolitionShadowTestInput*>(a));
        case kFC4SeamDispatchShadow:  // the glue17 lane (FC4DispatchShadow.h)
            return DispatchShadowTest(static_cast<FC4DispatchShadowTestInput*>(a));
        case kFC4SeamTerrainGlue:  // the glue18 lane (cFC4Terrain.h)
            return TerrainGlueTest(static_cast<FC4TerrainGlueTestInput*>(a));
        case kFC4SeamMySimsShadow:  // the mysims1 lane (FC4MySimsShadow.h)
            return fc4::MySimsShadowTest(static_cast<fc4::FC4MySimsShadowTestInput*>(a));
        case kFC4SeamParks:  // the cmglue lane (cFC4Parks.h)
            return a == nullptr ? -1 : fc4::ParksTestZooEscape(static_cast<const FC4ParksTestInput*>(a));
        case kFC4SeamNeighborsConfirm:  // the apply12 lane (NG35)
            fc4::NeighborsTestSetConfirm(reinterpret_cast<fc4::NeighborsConfirmFn>(a));
            return 1;
        case kFC4SeamNeighborsApp:  // the neighbours apply lane (NG14, NG4)
            if (b != nullptr) {
                return fc4::NeighborsTestCopyDeal(static_cast<cISC4NeighborDeal*>(a),
                                                  static_cast<cISC4NeighborDeal**>(b))
                           ? 1
                           : 0;
            }
            fc4::NeighborsTestSetApp(static_cast<cISC4App*>(a));
            return 1;
        case kFC4SeamUnreadRefs: {  // glue-exemplars4 (FC4Streams.h NoteUnreadRef)
            // `a` is a grid token that must never be dereferenced (the test
            // passes an unmapped address). Bits: 1 noted, 2 empty geometry,
            // 4 Get 0 without a call, 8 PreCityInit clears it, 16 PostLoad too.
            static const fc4::HostGridContext ctx{"seam", fc4::Sw::AuraGridWrites};
            const fc4_grid_api api = fc4::MakeGridApi(&ctx);
            const uint64_t t = static_cast<uint64_t>(reinterpret_cast<uintptr_t>(a));
            intptr_t bits = 0;
            fc4::NoteUnreadRef(t);
            if (fc4::IsUnreadRef(t)) bits |= 1;
            fc4_grid_geometry geo{};
            geo.count_x = 7;
            api.geometry(api.user, t, &geo);
            if (geo.count_x == 0 && geo.count_z == 0 && geo.tract_size == 0) bits |= 2;
            api.set(api.user, t, 0x49B9E603u, 1, 1, 5.0);  // dropped
            api.fill(api.user, t, 0x49B9E603u, 5.0);       // dropped
            if (api.get(api.user, t, 0x49B9E603u, 1, 1) == 0.0) bits |= 4;
            fc4::ObserverTestMessage(0x26D31EC0u, 0);  // kSC4MessagePreCityInit
            if (!fc4::IsUnreadRef(t)) bits |= 8;
            fc4::NoteUnreadRef(t);
            fc4::ObserverTestMessage(0x26C63342u, 0);  // kSC4MessagePostLoad
            if (!fc4::IsUnreadRef(t)) bits |= 16;
            return bits;
        }
        case kFC4SeamFpuReference:
            if (a == nullptr) {
                fc4::FpuSetReference("test seam");
            } else {
                fc4::FpuTick();  // the observer's per-tick check, without the rest of OnTick
            }
            return 1;
        case kFC4SeamAutomataCmTeardown:  // the verify-automatarefs4 lane (cFC4Automata.h)
            return a == nullptr ? -1
                                : AutomataCmTeardownTest(static_cast<cIGZUnknown*>(a), static_cast<cIGZUnknown*>(b));
        case kFC4SeamAdvisorsShadow:
            return fc4::AdvisorsShadowTest(static_cast<cIGZUnknown*>(a));
        case kFC4SeamOrdinanceShadow:
            return fc4::OrdinanceShadowTest(static_cast<cISC4OrdinanceSimulator*>(a));
        case kFC4SeamAutomataCapture:
            return fc4::AutomataCaptureTest(static_cast<cIGZUnknown*>(a),
                static_cast<uint32_t>(reinterpret_cast<uintptr_t>(b)));
        case kFC4SeamConstructionCapture:
            return fc4::ConstructionCaptureTest(static_cast<cIGZUnknown*>(a));
        case kFC4SeamConstructionRecordFactory:
            return fc4::ConstructionRecordFactoryTest(static_cast<cIGZUnknown*>(a), b);
        case kFC4SeamLotConfigurationCreate:
            return reinterpret_cast<intptr_t>(CreateFC4LotConfiguration());
        case kFC4SeamNetOccPoleFactory: {
            if (a || b) return 0;
            auto* pole = CreateFC4PowerPoleOccupant();
            if (pole) pole->AddRef();
            return reinterpret_cast<intptr_t>(pole);
}
        case kFC4SeamNetworkLotOwnership:
            return NetworkLotOwnershipTest(static_cast<cIGZUnknown*>(a), static_cast<cIGZUnknown*>(b));
        case kFC4SeamNetworkCrewOwnership:
            return NetworkCrewOwnershipTest();
        case kFC4SeamNetOccPoleLink: {
            if (!a || b) return 0;
            const auto& t = *static_cast<FC4NetOccPoleLinkTest*>(a);
            switch (t.operation) {
            case 0: return FC4NetOccEnablePoleConnection(t.first, t.second, t.value != 0);
            case 1: return FC4NetOccAddPoleConnection(t.first, t.second, t.buffer);
            case 2: return FC4NetOccRebuildPoleWires(t.first, t.second);
            case 3: return FC4NetOccPoleAttachmentPoints(t.first, t.value, static_cast<float*>(t.buffer));
            case 4: return FC4NetOccRemovePoleConnection(t.first, t.second, t.value != 0);
            default: return 0;
            }
        }
        case kFC4SeamDemolitionLotBuilding: {
            const auto host = fc4::MakeDemolitionHost(nullptr);
            return static_cast<intptr_t>(host.lot_building(nullptr, reinterpret_cast<uintptr_t>(a)));
        }
        case kFC4SeamDemolitionLotOccupants: {
            const long before = fc4::RefWarningCount();
            fc4::DemolitionVisitOccupants(*static_cast<SC4List<cISC4Occupant*>*>(a),
                                         [](void*, uint64_t) {}, nullptr);
            return fc4::RefWarningCount() - before;
        }
        case kFC4SeamMySimsActiveIds: {
            if (!a || !b) return -1;
            const long before = fc4::RefWarningCount();
            const auto ids = fc4::MySimsActiveIds(*static_cast<SC4Vector<cISC4MySim*>*>(a));
            for (size_t i = 0; i < ids.size() && i < 5; ++i) static_cast<int32_t*>(b)[i] = ids[i];
            return fc4::RefWarningCount() - before;
        }
        case kFC4SeamMySimsUiBlocked:
            return fc4::MySimsUiBlocked(static_cast<cIGZWinMgr*>(a));
        case kFC4SeamMySimsSavedReference:
            if (!b) return -1;
            return static_cast<intptr_t>(fc4::MySimsSavedReferenceTest(
                static_cast<cIGZUnknown*>(a), *static_cast<uint32_t*>(b)));
        case kFC4SeamGZFrameworkCreate: {
            if (b) return reinterpret_cast<intptr_t>(fc4::GZCommandServerTestCreate(static_cast<cIGZUnknown*>(a)));
            auto* buffer = fc4::CreateGZBuffer();
            if (buffer) buffer->AddRef();
            return reinterpret_cast<intptr_t>(buffer);
        }
        case kFC4SeamNetworkToolOccupantViews:
            return fc4::NetToolOccupantViewsTest(static_cast<cIGZUnknown*>(a),
                                               static_cast<cIGZMessageServer2*>(b));
        case kFC4SeamPowerLineConnections:
            return PowerTestLineConnections(static_cast<cIGZUnknown*>(a), static_cast<uint64_t*>(b));
        case kFC4SeamDemolitionFilters:
            return (fc4::DemolitionDefaultFilterIncludes(static_cast<cISC4Occupant*>(a)) ? 1 : 0) |
                   (fc4::DemolitionRealisticFilterIncludes(static_cast<cISC4Occupant*>(a),
                                                           static_cast<cISC4City*>(b)) ? 2 : 0);
        case kFC4SeamDemolitionPreDemolish:
            fc4::DemolitionSendPreDemolish(static_cast<cIGZMessageServer2*>(b), static_cast<cISC4Occupant*>(a));
            return 0;
        case kFC4SeamDemolitionRegionMask: {
            if (!a || !b) return -1;
            fc4_zone_region region{};
            std::vector<uint8_t> mask;
            if (!fc4::DemolitionRegionToC(*static_cast<SC4CellRegion<int32_t>*>(a), region, mask) ||
                mask.size() > 256) return -1;
            std::memcpy(b, mask.data(), mask.size());
            return static_cast<intptr_t>(mask.size());
        }
        case kFC4SeamDemolitionQueryLifetime: {
            if (!a || !b) return -1;
            auto* occupant = static_cast<cIGZUnknown*>(a);
            auto* counts = static_cast<uint32_t*>(b);
            fc4::DemolitionHostContext ctx;
            auto hold = [&ctx, occupant] {
                FC4_ADDREF(occupant);
                ctx.held.emplace_back(occupant, [](void* p) {
                    FC4_RELEASE(static_cast<cIGZUnknown*>(p));
                });
            };
            {
                fc4::DemolitionHostCall outer(ctx);
                // Reenter once while releasing the first query batch.
                FC4_ADDREF(occupant);
                ctx.held.emplace_back(occupant, [&](void* p) {
                    fc4::DemolitionHostCall reentrant(ctx);
                    hold();
                    FC4_RELEASE(static_cast<cIGZUnknown*>(p));
                });
                {
                    fc4::DemolitionHostCall inner(ctx);
                    hold();
                }
                counts[0] = static_cast<uint32_t>(ctx.held.size());
                counts[1] = ctx.callDepth;
            }
            counts[2] = static_cast<uint32_t>(ctx.held.size());
            counts[3] = ctx.callDepth;
            return 0;
        }
        case kFC4SeamPowerCreate: {
            cIGZUnknown* power = CreatePowerSimulator();
            power->AddRef();
            return reinterpret_cast<intptr_t>(power);
        }
        default:
            return 0;
    }
}
