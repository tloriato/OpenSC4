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
    size_t algorithms = 0, qi = 0, references = 0, records = 0, bytes = 0;
    size_t reads = 0, writes = 0, owned = 0, outUnchanged = 0, outCleared = 0, outOther = 0;
    void Check(bool equal) {
        ++compared;
        if (!equal) { ++different; Log("flora occupant difference: event=%zu", compared); }
    }
    void Query(bool equal) { ++algorithms; Check(equal); }
    void QI(bool equal) { ++qi; Check(equal); }
    void Ref(bool equal) { ++references; Check(equal); }
    void Read(bool equal) { ++reads; Check(equal); }
    void Write(bool equal) { ++writes; Check(equal); }
    void Bytes(CaptureStream& out, const uint8_t* expected, size_t n) {
        ++records; bytes += n;
        Check(out.Size() == n && memcmp(out.Data(), expected, n) == 0);
    }
};

void CheckOccupant(cISC4Occupant* o, cISC4FloraOccupant* f,
                   const fc4_flora_occupant_input& expected, OccupantProbeCounts& c) {
    c.QI(f->AsOccupant() == o); // borrowed; do not release
    const auto query = [&](uint32_t op, uint32_t arg = 0) {
        return fc4_flora_occupant_query(&expected, op, arg);
    };
    c.Query(static_cast<uint32_t>(o->GetType()) == query(0));
    c.Query(f->GetFloraType() == query(1));
    c.Query(f->GetBirthDate() == query(2));
    c.Query(f->GetLastSeedingDate() == query(3));
    c.Query(static_cast<uint32_t>(f->GetState()) == query(4));
    c.Query(f->GetOrientation() == query(5));
    c.Query(o->GetHighlight() == query(6));
    c.Query(o->GetFlags() == query(7));
    for (uint32_t bit : {1u, 2u, 4u, 8u, 16u, 32u, 64u, 128u, 256u})
        c.Query(o->IsFlagSet(bit) == (query(8, bit) != 0));
    for (uint32_t mask : {0u, 1u, 2u, 3u, 4u})
        c.Query(o->CanRemove(mask) == (query(9, mask) != 0));
    cS3DVector3 p;
    o->GetPosition(&p); ++c.undefined; // result channel unspecified for flora override
    c.Check(memcmp(&p, expected.position, sizeof(expected.position)) == 0);
    // FQ17: no specified flora bounds/default/static projection; no call or credit.
    ++c.refused;
}

// Typed SDK COM identity/refcounts, separately accounted from Rust algorithms.
void CheckOccupantQI(cISC4Occupant* o, cISC4FloraOccupant* f,
                     cIGZSerializable* s, void** refs, OccupantProbeCounts& c) {
    void* canonical = nullptr;
    const bool ok = o->QueryInterface(GZIID_cIGZUnknown, &canonical);
    c.QI(ok && canonical);
    if (!ok || !canonical) return;
    auto* identity = static_cast<cIGZUnknown*>(canonical);
    FC4_ACQUIRED(identity, "QueryInterface");
    fc4_flora_occupant_reference(refs, 1);
    for (cIGZUnknown* view : {static_cast<cIGZUnknown*>(o), static_cast<cIGZUnknown*>(f),
                              static_cast<cIGZUnknown*>(s), identity}) {
        for (uint32_t iid : {GZIID_cIGZUnknown, GZIID_cISC4Occupant,
                             GZIID_cISC4FloraOccupant, GZIID_cIGZSerializable}) {
            void* output = nullptr;
            const bool acquired = view->QueryInterface(iid, &output);
            c.QI(acquired && output);
            if (!acquired || !output) continue;
            auto* q = static_cast<cIGZUnknown*>(output);
            FC4_ACQUIRED(q, "QueryInterface");
            fc4_flora_occupant_reference(refs, 1);
            void* expected = iid == GZIID_cIGZUnknown ? canonical :
                iid == GZIID_cISC4Occupant ? static_cast<void*>(o) :
                iid == GZIID_cISC4FloraOccupant ? static_cast<void*>(f) : static_cast<void*>(s);
            c.QI(output == expected);
            c.Ref(FC4_ADDREF(q) == fc4_flora_occupant_reference(refs, 1));
            c.Ref(FC4_RELEASE(q) == fc4_flora_occupant_reference(refs, 2));
            c.Ref(FC4_RELEASE(q) == fc4_flora_occupant_reference(refs, 2));
        }
        for (uint32_t iid : {0u, 0x7f4cffffu}) {
            for (void* sentinel : {static_cast<void*>(nullptr), static_cast<void*>(o)}) {
                void* output = sentinel;
                const bool acquired = view->QueryInterface(iid, &output);
                c.QI(!acquired);
                if (acquired && output) {
                    auto* q = static_cast<cIGZUnknown*>(output);
                    FC4_ACQUIRED(q, "QueryInterface");
                    fc4_flora_occupant_reference(refs, 1);
                    c.Ref(FC4_RELEASE(q) == fc4_flora_occupant_reference(refs, 2));
                }
                // FQ18: failed-QI output behavior is observed, not fabricated.
                if (output == sentinel) ++c.outUnchanged;
                else if (!output) ++c.outCleared;
                else ++c.outOther;
                ++c.refused;
                c.Ref(FC4_ADDREF(view) == fc4_flora_occupant_reference(refs, 1));
                c.Ref(FC4_RELEASE(view) == fc4_flora_occupant_reference(refs, 2));
            }
        }
    }
    ++c.refused; // FQ18: null output-address calls have no safety contract.
    c.Ref(FC4_RELEASE(identity) == fc4_flora_occupant_reference(refs, 2));
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
    // Five preserved narrowing fixtures plus all normal states x orientations.
    for (uint32_t index = 0; index < 25; ++index) {
        void* raw = nullptr;
        if (!com->GetClassObject(0xA9C05C85, GZIID_cISC4Occupant, &raw) || !raw) {
            ++c.refused; continue;
        }
        auto* o = static_cast<cISC4Occupant*>(raw);
        ++c.owned;
        FC4_ACQUIRED(o, "GetClassObject");
        void* rustRefs = nullptr;
        c.Check(fc4_flora_occupant_reference(&rustRefs, 0) == 1);
        void* fq = nullptr; void* sq = nullptr;
        bool fqOK = o->QueryInterface(GZIID_cISC4FloraOccupant, &fq) && fq;
        bool sqOK = o->QueryInterface(GZIID_cIGZSerializable, &sq) && sq;
        if (fqOK) FC4_ACQUIRED(static_cast<cIGZUnknown*>(fq), "QueryInterface");
        if (sqOK) FC4_ACQUIRED(static_cast<cIGZUnknown*>(sq), "QueryInterface");
        if (fqOK) fc4_flora_occupant_reference(&rustRefs, 1);
        if (sqOK) fc4_flora_occupant_reference(&rustRefs, 1);
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
            if (index >= 5) {
                input.state = (index - 5) / 4;
                input.orientation = (index - 5) % 4;
            }
            input.lot_tag = index * 0x12345678u;
            input.position[0] = -32.5f + index;
            input.position[1] = 100.25f;
            input.position[2] = 64.0f;
            input.flags = 0xCD; input.highlight = 7;
            input.removal = static_cast<uint16_t>(index * 257);
            if (index >= 5) {
                const uint8_t flags[] = {0, 4, 255, 128, 1};
                const uint16_t removal[] = {0, 1, 256, 257, 65535};
                input.flags = flags[(index - 5) / 4];
                input.highlight = static_cast<uint8_t>((index - 5) % 16);
                input.removal = removal[(index - 5) / 4];
            }
            input.cells[0] = 0xFF;
            uint8_t bytes[1024]{};
            const size_t n = fc4_flora_occupant_encode(&input, bytes, sizeof(bytes));
            fc4_flora_occupant_input expected{};
            c.Check(n != 0);
            c.Check(fc4_flora_occupant_decode(bytes, n, &expected));
            OccupantInputStream in(bytes, n);
            c.Read(s->Read(in)); c.Check(in.GetError() == 0);
            CheckOccupant(o, f, expected, c);
            CheckOccupantQI(o, f, s, &rustRefs, c);
            CaptureStream written;
            c.Write(s->Write(written));
            c.Bytes(written, bytes, n);
            // Same typed mutations on the owned original and independent codec.
            input = expected; // Rust-decoded normalization, never native output
            input.flora_type ^= 0xFFFF; input.birth ^= 0x76543210;
            input.seeding = 0; input.state = 255; input.orientation = 257;
            if (index >= 5) {
                input.state = ((index - 5) / 4 + 1) % 5;
                input.orientation = ((index - 5) % 4 + 1) % 4;
            }
            f->SetFloraType(input.flora_type); f->SetBirthDate(input.birth);
            f->SetLastSeedingDate(input.seeding); f->SetState(input.state);
            f->SetOrientation(input.orientation); c.undefined += 5; // flora §11.4
            cS3DVector3 moved;
            input.position[0] = moved.fX = 23.5f;
            input.position[1] = moved.fY = -4.0f;
            input.position[2] = moved.fZ = 999.0f;
            o->SetPosition(&moved); ++c.undefined;
            for (const auto& mutation : {std::make_pair(0u, 0xCDu), std::make_pair(1u, 0x80u),
                    std::make_pair(2u, 8u), std::make_pair(3u, 30u),
                    std::make_pair(4u, 3u), std::make_pair(5u, 1u),
                    std::make_pair(7u, 0u), std::make_pair(7u, 1u),
                    std::make_pair(0u, 0x100u), std::make_pair(1u, 0x181u),
                    std::make_pair(2u, 0x180u), std::make_pair(3u, 0x100u),
                    std::make_pair(5u, 3u), std::make_pair(4u, 2u)}) {
                const uint32_t op = mutation.first, arg = mutation.second;
                fc4_flora_occupant_mutate(&input, op, arg);
                switch (op) {
                    case 0: o->SetAllFlags(arg); break;
                    case 1: o->SetFlag(arg); break;
                    case 2: o->ClearFlag(arg); break;
                    case 3: o->SetHighlight(arg, false); break;
                    case 4: o->SetRemovalFlags(arg); break;
                    case 5: o->UnsetRemovalFlags(arg); break;
                    case 7: o->SetVisibility(arg != 0, false); break;
                }
                ++c.undefined;
                c.Query(o->GetFlags() == fc4_flora_occupant_query(&input, 7, 0));
                c.Query(o->GetHighlight() == fc4_flora_occupant_query(&input, 6, 0));
                for (uint32_t bit : {1u, 2u, 4u, 8u, 16u, 32u, 64u, 128u, 256u})
                    c.Query(o->IsFlagSet(bit) == (fc4_flora_occupant_query(&input, 8, bit) != 0));
                for (uint32_t mask : {0u, 1u, 2u, 3u})
                    c.Query(o->CanRemove(mask) == (fc4_flora_occupant_query(&input, 9, mask) != 0));
            }
            const size_t m = fc4_flora_occupant_encode(&input, bytes, sizeof(bytes));
            fc4_flora_occupant_decode(bytes, m, &expected);
            CheckOccupant(o, f, expected, c);
            written.Reset(); c.Write(s->Write(written));
            c.Bytes(written, bytes, m);
            CaptureStream failedNative, failedRust;
            failedNative.Fail(); failedRust.Fail();
            OStreamAdapter errorAdapter(failedRust);
            c.Write(s->Write(failedNative) == fc4_flora_occupant_write(&input, errorAdapter.Get()));
            c.Check(failedNative.GetError() != 0 && failedRust.GetError() != 0);
            // Read versions, every strict truncation, and the base-reject quirk.
            for (size_t shortSize = 0; shortSize < m; ++shortSize) {
                OccupantInputStream shortIn(bytes, shortSize);
                fc4_flora_occupant_input ignored{};
                const bool rust = fc4_flora_occupant_decode(bytes, shortSize, &ignored);
                c.Read(s->Read(shortIn) == rust);
                ++c.refused; // FQ17: no partial-state equivalence claimed
            }
            const uint8_t badVersion[] = {2, 0};
            const uint8_t badBase[] = {3, 0, 5, 0};
            for (const auto& sample : {std::make_pair(badVersion, size_t(2)), std::make_pair(badBase, size_t(4))}) {
                OccupantInputStream bad(sample.first, sample.second);
                fc4_flora_occupant_input ignored{};
                c.Read(s->Read(bad) == fc4_flora_occupant_decode(sample.first, sample.second, &ignored));
            }
            c.Ref(FC4_ADDREF(f) == fc4_flora_occupant_reference(&rustRefs, 1));
            c.Ref(FC4_RELEASE(f) == fc4_flora_occupant_reference(&rustRefs, 2));
            void* unknown = nullptr;
            c.Check(!f->QueryInterface(0x7f4cffff, &unknown));
            c.Check(unknown == nullptr);
        }
        if (fqOK) c.Ref(FC4_RELEASE(static_cast<cIGZUnknown*>(fq)) == fc4_flora_occupant_reference(&rustRefs, 2));
        if (sqOK) c.Ref(FC4_RELEASE(static_cast<cIGZUnknown*>(sq)) == fc4_flora_occupant_reference(&rustRefs, 2));
        c.Ref(FC4_RELEASE_CREATED(o) == fc4_flora_occupant_reference(&rustRefs, 2));
        c.Check(rustRefs == nullptr);
    }
    Log("flora occupant probe: day=%lld owned=%zu comparisons=%zu differences=%zu refusals=%zu fallbacks=%zu undefined_returns=%zu static_calls=0 FQ17,FQ18",
        static_cast<long long>(day), c.owned, c.compared, c.different, c.refused, c.fallback, c.undefined);
    Log("flora occupant accounting: unique_inputs=25 normal_cross=20 narrowing=5 algorithms=%zu qi_assertions=%zu reference_results=%zu records=%zu bytes=%zu read_results=%zu write_results=%zu failed_qi_out_unchanged=%zu cleared=%zu other=%zu",
        c.algorithms, c.qi, c.references, c.records, c.bytes, c.reads, c.writes, c.outUnchanged, c.outCleared, c.outOther);
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
