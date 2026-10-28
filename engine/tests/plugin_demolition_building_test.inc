// SPDX-License-Identifier: GPL-3.0-or-later
// Regression for the Darris, Indereast and West Weburg demolition exits.
// Lot slot 48 returns cISC4BuildingOccupant*, whose occupant is a distinct
// interface (SDK cISC4BuildingOccupant::AsOccupant, slot 3).
namespace demolition_building_test {
struct Interface {
    void** vtbl;
    void* result;
    unsigned calls = 0;
};
void* __fastcall GetResult(Interface* self, void*) {
    ++self->calls;
    return self->result;
}
struct Occupant {
    void** vtbl;
    uint32_t refs = 1, acquisitions = 0, releases = 0;
};
bool __fastcall Query(Occupant*, void*, uint32_t, void** out) {
    if (out) *out = nullptr;
    return false;
}
uint32_t __fastcall AddRef(Occupant* self, void*) {
    ++self->acquisitions;
    return ++self->refs;
}
uint32_t __fastcall Release(Occupant* self, void*) {
    ++self->releases;
    return --self->refs;
}
struct Allocator {
    static bool Available() { return true; }
    static void* Allocate(uint32_t size) { return std::malloc(size); }
    static void Free(void* p) { std::free(p); }
};

struct FilterOccupant {
    void** vtbl;
    FakeHolder* holder;
    uint32_t type = 0x278128A0, group = 0, iid = 0;
    Occupant* queried = nullptr;
};
bool __fastcall FilterQuery(FilterOccupant* self, void*, uint32_t iid, void** out) {
    *out = nullptr;
    if (iid != self->iid || !self->queried) return false;
    *out = self->queried;
    AddRef(self->queried, nullptr);
    return true;
}
void* __fastcall Holder(FilterOccupant* self, void*) { return self->holder->AsSdk(); }
uint32_t __fastcall Type(FilterOccupant* self, void*) { return self->type; }
bool __fastcall Group(FilterOccupant* self, void*, uint32_t group) { return self->group == group; }
bool __fastcall Position(FilterOccupant*, void*, float* out) {
    out[0] = 32; out[1] = 0; out[2] = 48;
    return true;
}
void __fastcall Cell(void*, void*, float x, float z, int& cx, int& cz) {
    CHECK(x == 32 && z == 48);
    cx = 2; cz = 3;
}
void* __fastcall GetLot(Interface* self, void*, int x, int z, bool create) {
    CHECK(x == 2 && z == 3 && !create);
    return self->result;
}
uint32_t __fastcall State(Interface* self, void*) { return static_cast<uint32_t>(reinterpret_cast<uintptr_t>(self->result)); }

struct MessageServer {
    void** vtbl;
    Occupant* occupant;
    unsigned sends = 0, posts = 0;
};
bool __fastcall Send(MessageServer* self, void*, cIGZMessage2Standard* msg) {
    ++self->sends;
    CHECK(msg->GetType() == 0xAA7A014D);
    CHECK(msg->GetVoid1() == self->occupant);
    CHECK(msg->GetIGZUnknown() == reinterpret_cast<cIGZUnknown*>(self->occupant));
    CHECK(self->occupant->refs == 2); // the payload keeps the pole alive during delivery
    return true;
}
bool __fastcall Post(MessageServer* self, void*, cIGZMessage2*, bool) {
    ++self->posts;
    return true;
}
} // namespace demolition_building_test

void DemolitionBuildingInterfaceChecks(FC4TestSeamFn seam) {
    using namespace demolition_building_test;
    FakeVtable lotVt, buildingVt, occupantVt;
    lotVt.slots[48] = reinterpret_cast<void*>(&GetResult);
    buildingVt.slots[3] = reinterpret_cast<void*>(&GetResult);
    FakeObject occupant{occupantVt.slots.data()};
    Interface building{buildingVt.slots.data(), &occupant};
    Interface lot{lotVt.slots.data(), &building};
    const auto token = seam(kFC4SeamDemolitionLotBuilding, &lot, nullptr);
    CHECK(token == reinterpret_cast<intptr_t>(&occupant));
    CHECK(token != reinterpret_cast<intptr_t>(&building));
    CHECK(lot.calls == 1 && building.calls == 1);
    // Empty lots and a missing lot stay null, without calling a building.
    lot.result = nullptr;
    CHECK(seam(kFC4SeamDemolitionLotBuilding, &lot, nullptr) == 0);
    CHECK(seam(kFC4SeamDemolitionLotBuilding, nullptr, nullptr) == 0);
    CHECK(lot.calls == 2 && building.calls == 1);
    // Conversion may itself return null; it is still a borrowed interface.
    lot.result = &building;
    building.result = nullptr;
    CHECK(seam(kFC4SeamDemolitionLotBuilding, &lot, nullptr) == 0);
    CHECK(lot.calls == 3 && building.calls == 2);
    // A query-supplied reference is registered before release, without an
    // extra AddRef or a false over-release warning in the audit.
    occupantVt.slots[0] = reinterpret_cast<void*>(&Query);
    occupantVt.slots[1] = reinterpret_cast<void*>(&AddRef);
    occupantVt.slots[2] = reinterpret_cast<void*>(&Release);
    Occupant queried{occupantVt.slots.data()};
    {
        // SDK-compatible layout; this test allocator needs no game framework.
        // Its destructor, like SC4List's, releases non-null pointer values.
        fc4::GameList<cISC4Occupant*, Allocator> list;
        AddRef(&queried, nullptr); // one owned reference supplied by a query
        CHECK(list.push_back(reinterpret_cast<cISC4Occupant*>(&queried)));
        CHECK(list.push_back(nullptr));
        CHECK(seam(kFC4SeamDemolitionLotOccupants, &list, nullptr) == 0);
        for (auto* value : list) CHECK(value == nullptr);
    }
    CHECK(queried.refs == 1 && queried.acquisitions == 1 && queried.releases == 1);
}

void DemolitionPreDemolishChecks(FC4TestSeamFn seam) {
    using namespace demolition_building_test;
    FakeVtable occupantVt, serverVt;
    occupantVt.slots[1] = reinterpret_cast<void*>(&AddRef);
    occupantVt.slots[2] = reinterpret_cast<void*>(&Release);
    serverVt.slots[3] = reinterpret_cast<void*>(&Send);
    serverVt.slots[4] = reinterpret_cast<void*>(&Post);
    Occupant occupant{occupantVt.slots.data()};
    MessageServer server{serverVt.slots.data(), &occupant};
    seam(kFC4SeamDemolitionPreDemolish, &occupant, &server);
    CHECK(server.sends == 1 && server.posts == 0);
    CHECK(occupant.refs == 1 && occupant.acquisitions == 1 && occupant.releases == 1);
    seam(kFC4SeamDemolitionPreDemolish, nullptr, &server);
    seam(kFC4SeamDemolitionPreDemolish, &occupant, nullptr);
    CHECK(server.sends == 1 && server.posts == 0 && occupant.refs == 1);
}

void DemolitionRegionMaskChecks(FC4TestSeamFn seam) {
    struct CellMap {
        virtual ~CellMap() = default;
        uint32_t width = 2, height = 35, words = 2;
        uint32_t** rows = nullptr;
    };
    struct Region {
        int32_t x0 = -3, z0 = 20, x1 = -2, z1 = 54;
        CellMap map;
    } region;
    static_assert(sizeof(Region) == 0x24, "SDK native cell region including vptr");
    uint32_t row0[] = {0x80000001, 5}, row1[] = {2, 4};
    uint32_t* rows[] = {row0, row1};
    region.map.rows = rows;
    uint8_t mask[32]{};
    CHECK(seam(kFC4SeamDemolitionRegionMask, &region, mask) == 9);
    const uint8_t expected[] = {1, 0, 0, 0x80, 0x15, 0, 0, 0, 0x20};
    CHECK(std::memcmp(mask, expected, sizeof(expected)) == 0);
    // Invalid dimensions/strides/pointers fail before indexing the bitmap.
    region.map.words = 1;
    CHECK(seam(kFC4SeamDemolitionRegionMask, &region, mask) == -1);
    region.map.words = 2;
    region.map.height = 34;
    CHECK(seam(kFC4SeamDemolitionRegionMask, &region, mask) == -1);
    region.map.height = 35;
    rows[1] = nullptr;
    CHECK(seam(kFC4SeamDemolitionRegionMask, &region, mask) == -1);
    CHECK(seam(kFC4SeamDemolitionRegionMask, nullptr, mask) == -1);
}

void DemolitionFilterChecks(FC4TestSeamFn seam) {
    using namespace demolition_building_test;
    FakeVtable occupantVt, queriedVt, cityVt, managerVt, lotVt;
    occupantVt.slots[0] = reinterpret_cast<void*>(&FilterQuery);
    occupantVt.slots[6] = reinterpret_cast<void*>(&Holder);
    occupantVt.slots[7] = reinterpret_cast<void*>(&Type);
    occupantVt.slots[8] = reinterpret_cast<void*>(&Position);
    occupantVt.slots[22] = reinterpret_cast<void*>(&Group);
    queriedVt.slots[1] = reinterpret_cast<void*>(&AddRef);
    queriedVt.slots[2] = reinterpret_cast<void*>(&Release);
    FakeHolder holder;
    Occupant queried{queriedVt.slots.data()};
    FilterOccupant occupant{occupantVt.slots.data(), &holder};
    const auto inclusion = [&] { return seam(kFC4SeamDemolitionFilters, &occupant, nullptr); };
    CHECK(seam(kFC4SeamDemolitionFilters, nullptr, nullptr) == 0);
    CHECK(inclusion() == 3);
    // Presence excludes even a zero value; the third property tests its bool value.
    for (uint32_t id : {0x49512A09u, 0x8A15F1BDu}) {
        holder.Set(id, uint32_t{0});
        CHECK(inclusion() == 0);
        holder.RemoveProperty(id, false);
    }
    for (bool value : {false, true}) {
        cRZBaseVariant flag;
        flag.SetValBool(value);
        holder.AddPropertyVariant(0xCAA45670, fc4::AsGame(static_cast<cIGZVariant const*>(&flag)), false);
        CHECK(inclusion() == (value ? 0 : 3));
    }
    holder.RemoveProperty(0xCAA45670, false);
    for (uint32_t group : {0x4217u, 0x4208u, 0x421Du, 0x5001u}) {
        occupant.group = group;
        CHECK(inclusion() == (group == 0x5001 ? 1 : 0));
    }
    occupant.group = 0;
    occupant.queried = &queried;
    for (uint32_t iid : {0x4A10AFC2u, 0x8A139184u}) {
        occupant.iid = iid;
        CHECK(inclusion() == 0);
    }
    // Sign/decal interfaces exclude props only. Release the queried interface,
    // which differs from the occupant pointer, exactly once per successful QI.
    for (uint32_t iid : {0x4B44FBE2u, 0x2B3B7D86u}) {
        occupant.iid = iid;
        occupant.type = 0xA823821E;
        CHECK(inclusion() == 1);
        occupant.type = 0x278128A0;
        CHECK(inclusion() == 3);
    }
    cityVt.slots[33] = reinterpret_cast<void*>(&GetResult);
    cityVt.slots[95] = reinterpret_cast<void*>(&Cell);
    managerVt.slots[6] = reinterpret_cast<void*>(&GetLot);
    lotVt.slots[33] = reinterpret_cast<void*>(&State);
    Interface lot{lotVt.slots.data(), reinterpret_cast<void*>(1)};
    Interface manager{managerVt.slots.data(), &lot};
    Interface city{cityVt.slots.data(), &manager};
    occupant.type = 0x497F6D9D;
    occupant.iid = 0x097F6D98;
    CHECK(seam(kFC4SeamDemolitionFilters, &occupant, &city) == 1);
    lot.result = reinterpret_cast<void*>(6);
    CHECK(seam(kFC4SeamDemolitionFilters, &occupant, &city) == 3);
    manager.result = nullptr;
    CHECK(seam(kFC4SeamDemolitionFilters, &occupant, &city) == 3);
    CHECK(queried.refs == 1 && queried.acquisitions > 0 && queried.acquisitions == queried.releases);
}
