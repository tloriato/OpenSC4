// SPDX-License-Identifier: GPL-3.0-or-later
// Included inside FC4FloraShadow.cpp's anonymous namespace, after CaptureStream.
// Public GZCOM/SDK calls only; no static-record getter or native field access.
class OccupantInputStream final : public cFC4ISC4DBSegmentIStream {
public:
    OccupantInputStream(const uint8_t* p, size_t n) : data(p), size(n) {}
    bool QueryInterface(uint32_t id, void** out) override {
        if (!out) return false;
        *out = nullptr;
        if (id != GZIID_cIGZUnknown && id != kFC4_IID_cISC4DBSegmentIStream) return false;
        *out = static_cast<cFC4ISC4DBSegmentIStream*>(this); AddRef(); return true;
    }
    uint32_t AddRef() override { return ++refs; }
    uint32_t Release() override { return --refs; } // stack-owned test stream
    bool Skip(uint32_t n) override {
        if (n > size - offset) { error = 1; return false; }
        offset += n; return error == 0;
    }
    bool GetVoid(void* p, uint32_t n) override {
        if (n > size - offset || (n && !p)) { error = 1; return false; }
        if (n) memcpy(p, data + offset, n);
        offset += n; return error == 0;
    }
    bool GetSint8(int8_t& v) override { return GetVoid(&v, sizeof(v)); }
    bool GetUint8(uint8_t& v) override { return GetVoid(&v, sizeof(v)); }
    bool GetSint16(int16_t& v) override { return GetVoid(&v, sizeof(v)); }
    bool GetUint16(uint16_t& v) override { return GetVoid(&v, sizeof(v)); }
    bool GetSint32(int32_t& v) override { return GetVoid(&v, sizeof(v)); }
    bool GetUint32(uint32_t& v) override { return GetVoid(&v, sizeof(v)); }
    bool GetSint64(int64_t& v) override { return GetVoid(&v, sizeof(v)); }
    bool GetUint64(uint64_t& v) override { return GetVoid(&v, sizeof(v)); }
    bool GetFloat32(float& v) override { return GetVoid(&v, sizeof(v)); }
    bool GetFloat64(double& v) override { return GetVoid(&v, sizeof(v)); }
    bool GetRZCharStr(char*, uint32_t) override { error = 1; return false; }
    bool GetGZStr(cIGZString&) override { error = 1; return false; }
    bool GetGZSerializable(cIGZSerializable& s) override { return s.Read(*this); }
    int32_t GetError() override { return error; }
    int32_t SetUserData(cIGZVariant*) override { return 0; }
    int32_t GetUserData() override { return 0; }
    bool Open(cISC4DBSegment*, const cGZPersistResourceKey&, bool) override { return false; }
    bool Close() override { return true; }
    bool IsOpen() override { return true; }
    int32_t GetRecord() override { return 0; }
    int32_t GetSegment() override { return 0; }
    bool ReadGZSerializable(cIGZSerializable**, bool) override { error = 1; return false; }
    bool ReadResKey(cGZPersistResourceKey& k) override {
        return GetUint32(k.type) && GetUint32(k.group) && GetUint32(k.instance);
    }
    bool ReadVariant(cIGZVariant&) override { error = 1; return false; }
private:
    const uint8_t* data;
    size_t size, offset = 0;
    uint32_t refs = 1;
    int32_t error = 0;
};

struct OccupantProbeCounts {
    size_t compared = 0, different = 0, refused = 0, fallback = 0, undefined = 0;
    void Check(bool equal) { ++compared; if (!equal) ++different; }
};

void CheckOccupant(cISC4Occupant* o, cISC4FloraOccupant* f,
                   const fc4_flora_occupant_input& expected, OccupantProbeCounts& c) {
    c.Check(f->AsOccupant() == o); // borrowed; do not release
    c.Check(static_cast<uint32_t>(o->GetType()) == kTypeFlora);
    c.Check(f->GetFloraType() == expected.flora_type);
    c.Check(f->GetBirthDate() == expected.birth);
    c.Check(f->GetLastSeedingDate() == expected.seeding);
    c.Check(f->GetState() == expected.state);
    c.Check(f->GetOrientation() == expected.orientation);
    c.Check(o->GetHighlight() == expected.highlight);
    c.Check(o->GetFlags() == expected.flags);
    for (uint32_t bit : {1u, 2u, 4u, 8u, 32u, 128u})
        c.Check(o->IsFlagSet(bit) == ((expected.flags & bit) != 0));
    c.Check(o->CanRemove(1) == ((expected.removal >> 8) == 0));
    c.Check(o->CanRemove(2) == ((expected.removal & 255) == 0));
    cS3DVector3 p;
    o->GetPosition(&p); ++c.undefined; // result channel unspecified for flora override
    c.Check(memcmp(&p, expected.position, sizeof(expected.position)) == 0);
    // FQ17: no specified flora bounds/default/static projection; no call or credit.
    ++c.refused;
}

void FloraOccupantProbe(cISC4City* city, int64_t day) {
    static cISC4City* lastCity = nullptr;
    static int64_t lastDay = 0;
    if (!city || (city == lastCity && day >= lastDay && day - lastDay < 300)) return;
    lastCity = city; lastDay = day;
    static_assert(sizeof(fc4_flora_occupant_input) == 44, "typed flora input ABI");
    OccupantProbeCounts c;
    cIGZCOM* com = RZGetCOMDllDirector() ? GZCOM() : nullptr;
    if (!com) { Log("flora occupant probe: REFUSED factory unavailable (FQ17)"); return; }
    for (uint32_t index = 0; index < 5; ++index) {
        void* raw = nullptr;
        if (!com->GetClassObject(0xA9C05C85, GZIID_cISC4Occupant, &raw) || !raw) {
            ++c.refused; continue;
        }
        auto* o = static_cast<cISC4Occupant*>(raw);
        FC4_ACQUIRED(o, "GetClassObject");
        void* fq = nullptr; void* sq = nullptr;
        bool fqOK = o->QueryInterface(GZIID_cISC4FloraOccupant, &fq) && fq;
        bool sqOK = o->QueryInterface(GZIID_cIGZSerializable, &sq) && sq;
        if (fqOK) FC4_ACQUIRED(static_cast<cIGZUnknown*>(fq), "QueryInterface");
        if (sqOK) FC4_ACQUIRED(static_cast<cIGZUnknown*>(sq), "QueryInterface");
        c.Check(fqOK); c.Check(sqOK);
        if (fqOK && sqOK) {
            auto* f = static_cast<cISC4FloraOccupant*>(fq);
            auto* s = static_cast<cIGZSerializable*>(sq);
            c.Check(s->GetGZCLSID() == 0xA9C05C85);
            fc4_flora_occupant_input input{};
            input.flora_type = 0x12340000u + index;
            input.birth = index ? 0xFFFFFFFFu : 2451545;
            input.seeding = 2451545 + index;
            input.state = static_cast<int32_t>(index * 128) - 129;
            input.orientation = index * 255;
            input.lot_tag = index * 0x12345678u;
            input.position[0] = -32.5f + index;
            input.position[1] = 100.25f;
            input.position[2] = 64.0f;
            input.flags = 0xCD; input.highlight = 7;
            input.removal = static_cast<uint16_t>(index * 257);
            input.cells[0] = 0xFF;
            uint8_t bytes[1024]{};
            const size_t n = fc4_flora_occupant_encode(&input, bytes, sizeof(bytes));
            fc4_flora_occupant_input expected{};
            c.Check(n != 0);
            c.Check(fc4_flora_occupant_decode(bytes, n, &expected));
            OccupantInputStream in(bytes, n);
            c.Check(s->Read(in)); c.Check(in.GetError() == 0);
            CheckOccupant(o, f, expected, c);
            CaptureStream written;
            c.Check(s->Write(written));
            c.Check(written.Size() == n && memcmp(written.Data(), bytes, n) == 0);
            // Same typed mutations on the owned original and independent codec.
            input.flora_type ^= 0xFFFF; input.birth ^= 0x76543210;
            input.seeding = 0; input.state = 255; input.orientation = 257;
            f->SetFloraType(input.flora_type); f->SetBirthDate(input.birth);
            f->SetLastSeedingDate(input.seeding); f->SetState(input.state);
            f->SetOrientation(input.orientation); c.undefined += 5; // flora §11.4
            const size_t m = fc4_flora_occupant_encode(&input, bytes, sizeof(bytes));
            fc4_flora_occupant_decode(bytes, m, &expected);
            CheckOccupant(o, f, expected, c);
            written.Reset(); c.Check(s->Write(written));
            c.Check(written.Size() == m && memcmp(written.Data(), bytes, m) == 0);
            // Read versions, every strict truncation, and the base-reject quirk.
            for (size_t shortSize = 0; shortSize < m; ++shortSize) {
                OccupantInputStream shortIn(bytes, shortSize);
                fc4_flora_occupant_input ignored{};
                const bool rust = fc4_flora_occupant_decode(bytes, shortSize, &ignored);
                c.Check(s->Read(shortIn) == rust);
                ++c.refused; // FQ17: no partial-state equivalence claimed
            }
            const uint8_t badVersion[] = {2, 0};
            const uint8_t badBase[] = {3, 0, 5, 0};
            for (const auto& sample : {std::make_pair(badVersion, size_t(2)), std::make_pair(badBase, size_t(4))}) {
                OccupantInputStream bad(sample.first, sample.second);
                fc4_flora_occupant_input ignored{};
                c.Check(s->Read(bad) == fc4_flora_occupant_decode(sample.first, sample.second, &ignored));
            }
            const uint32_t added = FC4_ADDREF(f);
            c.Check(FC4_RELEASE(f) + 1 == added);
        }
        if (fqOK) FC4_RELEASE(static_cast<cIGZUnknown*>(fq));
        if (sqOK) FC4_RELEASE(static_cast<cIGZUnknown*>(sq));
        c.Check(FC4_RELEASE_CREATED(o) == 0);
    }
    Log("flora occupant probe: day=%lld owned=5 comparisons=%zu differences=%zu refusals=%zu fallbacks=%zu undefined_returns=%zu static_calls=0 FQ17",
        static_cast<long long>(day), c.compared, c.different, c.refused, c.fallback, c.undefined);
    // Natural occupants: read-only public snapshots, separately accounted; no
    // captured native value is an expected computed result for the owned corpus.
    size_t natural = 0, snapshotQueries = 0, invalid = 0;
    cISC4OccupantManager* om = city->GetOccupantManager(); // borrowed
    if (om) {
        SC4List<cISC4Occupant*> list;
        om->GetOccupantsByStandardCityCells(list, nullptr, nullptr, kTypeFlora, 0xFFFFFFFFu);
        for (cISC4Occupant* o : list) {
            if (!o) continue;
            void* q = nullptr;
            if (o->QueryInterface(GZIID_cISC4FloraOccupant, &q) && q) {
                auto* f = static_cast<cISC4FloraOccupant*>(q);
                FC4_ACQUIRED(f, "QueryInterface");
                const uint32_t t = f->GetFloraType();
                const uint32_t b = f->GetBirthDate(), seed = f->GetLastSeedingDate();
                const int32_t state = f->GetState();
                const uint32_t orientation = f->GetOrientation();
                if (state < -128 || state > 127 || orientation > 255 || f->AsOccupant() != o) ++invalid;
                if (natural == 0)
                    Log("flora occupant natural sample: type=%08x birth=%u seeding=%u state=%d orientation=%u", t, b, seed, state, orientation);
                snapshotQueries += 6; ++natural;
                FC4_RELEASE(f);
            } else ++invalid;
        }
        om->ReleaseOccupantList(list); // published manager ownership, not ours
    }
    Log("flora occupant natural: day=%lld occupants=%zu readonly_queries=%zu invalid=%zu computed_credit=0 mutations=0 static_calls=0",
        static_cast<long long>(day), natural, snapshotQueries, invalid);
}
