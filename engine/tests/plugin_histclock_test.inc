// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.
//
// The history warehouse and 24-hour clock replacements and their shadows
// (docs/specs/history.md, clock24.md), included by plugin_load_test.cpp. The
// DLL runs without a game framework: test seam op 54 (FC4HistClock.h)
// answers the game calls instead.

namespace {

constexpr uint32_t kCLSID_History = 0x89EFA536;
constexpr uint32_t kIID_cISC4HistoryWarehouse = 0x69F4A803;
constexpr uint32_t kHcMsgRecord = 0x89EFA536;
constexpr uint32_t kHcMsgQueueLength = 0x89EFA537;
constexpr uint32_t kHcMsgSimNewDay = 0x66956814;
constexpr uint32_t kHcMsgSimNewYear = 0x66956817;
constexpr uint32_t kHcIIDMessageTarget2 = 0x452294AA;
constexpr uint32_t kHcDay0 = 2451545;  // 2000-01-01

// A cIGZDate seen through the compiled slots: only DayNumber (slot 26) is read.
class HcDate final : public cFC4IGZDate {
public:
    explicit HcDate(uint32_t d) : day(d) {}
    uint32_t day;
    bool QueryInterface(uint32_t, void**) override { return false; }
    uint32_t AddRef() override { return 1; }
    uint32_t Release() override { return 1; }
    uint32_t SetMonthNameDayYear(char const*, uint32_t, uint32_t) override { return 0; }
    uint32_t SetDayNumber(uint32_t d) override {
        day = d;
        return true;
    }
    uint32_t SetTime(cRZTime const&) override { return 0; }
    uint32_t SetMonthDayYear(uint32_t, uint32_t, uint32_t) override { return 0; }
    uint32_t SetDayOfYear(uint32_t, uint32_t) override { return 0; }
    uint32_t Copy(cFC4IGZDate const&) override { return 0; }
    uint32_t Clone(cFC4IGZDate**) override { return 0; }
    bool Between(cFC4IGZDate const&, cFC4IGZDate const&) override { return false; }
    int32_t CompareTo(cFC4IGZDate const&) override { return 0; }
    uint32_t WeekDay() override { return 0; }
    uint32_t Year() override { return 0; }
    uint32_t DayOfYear() override { return 0; }
    uint32_t DayOfMonth() override { return 0; }
    uint32_t FirstDayOfMonthOf(uint32_t) override { return 0; }
    uint32_t FirstDayOfMonth() override { return 0; }
    uint32_t Hash() override { return day; }
    bool IsValid() override { return true; }
    bool Leap() override { return false; }
    uint32_t MaxDate(cFC4IGZDate const&, cFC4IGZDate&) override { return 0; }
    uint32_t MinDate(cFC4IGZDate const&, cFC4IGZDate&) override { return 0; }
    uint32_t Month() override { return 0; }
    uint32_t Previous(uint32_t, cFC4IGZDate&) override { return 0; }
    uint32_t YearLastTwoDigits() override { return 0; }
    uint32_t DayNumber() override { return day; }
    bool IsLess(cFC4IGZDate const&) override { return false; }
    bool IsLessOrEqual(cFC4IGZDate const&) override { return false; }
    bool IsGreater(cFC4IGZDate const&) override { return false; }
    bool IsGreaterOrEqual(cFC4IGZDate const&) override { return false; }
    bool IsEqual(cFC4IGZDate const&) override { return false; }
    bool IsNotEqual(cFC4IGZDate const&) override { return false; }
    uint32_t MinusDays(int32_t) override { return 0; }
    uint32_t MinusDate(cFC4IGZDate const&) override { return 0; }
    uint32_t PlusDays(int32_t) override { return 0; }
    uint32_t PlusDate(cFC4IGZDate const&) override { return 0; }
    cFC4IGZDate& Increment() override { return *this; }
    cFC4IGZDate& Decrement() override { return *this; }
    cFC4IGZDate& AddDays(int32_t) override { return *this; }
    cFC4IGZDate& SubtractDays(int32_t) override { return *this; }
    uint32_t DateString(cIGZString&, uint32_t) override { return 0; }
    bool DayWithinMonth(uint32_t, uint32_t, uint32_t) override { return false; }
    uint32_t DaysInYear(uint32_t) override { return 365; }
    uint32_t Jday(uint32_t, uint32_t, uint32_t) override { return 0; }
    bool LeapYear(uint32_t) override { return false; }
};

// A standard message: type, data1, data2 and void2 (the cheat text).
class HcMessage final : public cIGZMessage2Standard {
public:
    HcMessage(uint32_t t, intptr_t d1, intptr_t d2 = 0, void* v2 = nullptr) : type(t), data1(d1), data2(d2), void2(v2) {}
    uint32_t type;
    intptr_t data1, data2;
    void* void2;
    bool QueryInterface(uint32_t riid, void** p) override {
        if (p == nullptr || riid != GZIID_cIGZMessage2Standard) return false;
        *p = static_cast<cIGZMessage2Standard*>(this);
        return true;
    }
    uint32_t AddRef() override { return 1; }
    uint32_t Release() override { return 1; }
    bool Create(uint32_t, void**) const override { return false; }
    uint32_t GetType() const override { return type; }
    void SetType(uint32_t t) override { type = t; }
    bool operator==(cIGZMessage2 const&) const override { return false; }
    bool operator<(cIGZMessage2 const&) const override { return false; }
    bool GetHasData1() const override { return true; }
    intptr_t GetData1() const override { return data1; }
    void* GetVoid1() const override { return nullptr; }
    void SetData1(intptr_t v) override { data1 = v; }
    void SetVoid1(void*) override {}
    bool GetHasData2() const override { return true; }
    intptr_t GetData2() const override { return data2; }
    void* GetVoid2() const override { return void2; }
    void SetData2(intptr_t v) override { data2 = v; }
    void SetVoid2(void* v) override { void2 = v; }
    bool GetHasData3() const override { return false; }
    intptr_t GetData3() const override { return 0; }
    void* GetVoid3() const override { return nullptr; }
    void SetData3(intptr_t) override {}
    void SetVoid3(void*) override {}
    bool GetHasData4() const override { return false; }
    intptr_t GetData4() const override { return 0; }
    void* GetVoid4() const override { return nullptr; }
    void SetData4(intptr_t) override {}
    void SetVoid4(void*) override {}
    bool GetHasString() const override { return false; }
    bool GetString(uint32_t, void**) const override { return false; }
    void SetString(cIGZString*) override {}
    bool GetHasIGZUnknown() const override { return false; }
    cIGZUnknown* GetIGZUnknown() const override { return nullptr; }
    void SetIGZUnknown(cIGZUnknown*) override {}
    bool GetHasExtra() const override { return false; }
    uint32_t GetExtra(uint32_t, void**) const override { return 0; }
    void SetExtra(cIGZSerializable*) override {}
};

// What the hooks saw.
struct HcNote {
    cIGZMessageTarget2* target;
    uint32_t type;
    bool add;
};
struct HcAgentCall {
    cIGZMessageTarget2* target;
    uint32_t level;
    std::string name;
    uint32_t flags;
    bool add;
};
struct HcWorld {
    uint32_t date = kHcDay0;
    std::vector<HcNote> notes;
    std::vector<HcAgentCall> agents;
    std::vector<std::array<uint32_t, 3>> sent;  // type, data1, data2 of each MessageSend
    cIGZMessageTarget2* deliverTo = nullptr;    // the server's subscriber of the record
};

uint32_t HcSimDate(void* c) { return static_cast<HcWorld*>(c)->date; }
void HcNotification(void* c, cIGZMessageTarget2* t, uint32_t type, bool add) {
    static_cast<HcWorld*>(c)->notes.push_back({t, type, add});
}
void HcAgent(void* c, cIGZMessageTarget2* t, uint32_t level, const char* name, uint32_t flags, bool add) {
    static_cast<HcWorld*>(c)->agents.push_back({t, level, name ? name : "", flags, add});
}
void HcSend(void* c, cIGZMessage2* m) {
    HcWorld* w = static_cast<HcWorld*>(c);
    cIGZMessage2Standard* s = nullptr;
    if (m != nullptr && m->QueryInterface(GZIID_cIGZMessage2Standard, reinterpret_cast<void**>(&s)) && s != nullptr) {
        w->sent.push_back({m->GetType(), static_cast<uint32_t>(s->GetData1()), static_cast<uint32_t>(s->GetData2())});
        s->Release();
    }
    if (w->deliverTo != nullptr) w->deliverTo->DoMessage(m);  // as the message server would
}

FC4HistClockHooks HcHistoryHooks(HcWorld* w) {
    FC4HistClockHooks h{};
    h.struct_size = sizeof(h);
    h.ctx = w;
    h.sim_date = &HcSimDate;
    h.notification = &HcNotification;
    h.agent = &HcAgent;
    h.send = &HcSend;
    return h;
}

intptr_t HcInstall(FC4TestSeamFn seam, const FC4HistClockHooks* hooks) {
    FC4HistClockTestInput in{};
    in.op = 0;
    in.hooks = hooks;
    return seam(kFC4SeamHistClock, &in, nullptr);
}

uint32_t HcBits(float f) {
    uint32_t b = 0;
    std::memcpy(&b, &f, 4);
    return b;
}

// The §4.8 recording example through the record message, then the queries.
void HcHistoryRecordAndQuery(cFC4ISC4HistoryWarehouse* h, HcWorld& w) {
    const uint32_t K = 0x0A1100E8;
    const uint32_t d100 = kHcDay0 + 100;
    CHECK(h->SetHistoryDataQueueLength(K, 2));  // slot 13
    const struct {
        uint32_t day;
        int32_t value;
    } recs[] = {{d100, 5}, {d100 + 1, 6}, {d100 + 1, 7}, {d100 + 2, 8}};
    const size_t sentBefore = w.sent.size();
    for (const auto& r : recs) {
        w.date = r.day;
        CHECK(!h->RecordHistoryData(K, r.value));  // slot 12: MessageSend, delivered back; false (HW12)
    }
    CHECK(w.sent.size() == sentBefore + 4);
    if (w.sent.size() == sentBefore + 4) {
        CHECK(w.sent[sentBefore][0] == kHcMsgRecord && w.sent[sentBefore][1] == K && w.sent[sentBefore][2] == 5);
        CHECK(w.sent[sentBefore + 3][2] == 8);
    }
    // (101, 7), (102, 8): count, the exact ends, after the last point.
    float n = -1.0f;
    CHECK(h->GetCount(K, HcDate(d100), HcDate(d100 + 2), n) && n == 2.0f);  // slot 5
    float v = -1.0f;
    CHECK(h->GetPoint(K, HcDate(d100 + 2), v) && v == 8.0f);  // slot 8
    CHECK(h->GetPoint(K, HcDate(d100 + 50), v) && v == 8.0f);
    CHECK(h->GetPoint(K, HcDate(d100), v) && v == 7.0f);  // before the first point: the first value
    float mn = 0, mx = 0, avg = 0;
    CHECK(h->GetMinMaxAverage(K, HcDate(d100 + 1), HcDate(d100 + 2), mn, mx, avg));  // slot 7
    CHECK(mn == 7.0f && mx == 8.0f && avg == 7.5f);
    float slope = -9, icpt = -9;
    CHECK(h->GetTrend(K, HcDate(d100 + 1), HcDate(d100 + 2), slope, icpt));  // slot 6
    CHECK(slope == 1.0f && icpt == 7.0f);
    // A missing key: false, outputs untouched (§4.7, §9).
    n = -3.0f;
    mn = mx = avg = -4.0f;
    CHECK(!h->GetCount(0x12345678, HcDate(d100), HcDate(d100 + 2), n) && n == -3.0f);
    CHECK(!h->GetMinMaxAverage(0x12345678, HcDate(d100), HcDate(d100 + 2), mn, mx, avg));
    CHECK(mn == -4.0f && mx == -4.0f && avg == -4.0f);
    // The queue-length message (0x89EFA537) through the message target.
    if (w.deliverTo != nullptr) {
        HcMessage q(kHcMsgQueueLength, K, 1);
        CHECK(w.deliverTo->DoMessage(&q));
        CHECK(h->GetCount(K, HcDate(d100), HcDate(d100 + 2), n) && n == 1.0f);
        HcMessage rec(kHcMsgRecord, 0x2A11CB48, static_cast<intptr_t>(-17));
        w.date = d100 + 3;
        CHECK(w.deliverTo->DoMessage(&rec));
        CHECK(h->GetPoint(0x2A11CB48, HcDate(d100 + 3), v) && v == -17.0f);
    }
    // Slots 9-11: Coalesce and the two Disposes (the compiled order).
    const uint32_t K2 = 0x6A51178B;
    for (uint32_t i = 0; i < 4; ++i) {
        w.date = d100 + 10 + i;
        CHECK(!h->RecordHistoryData(K2, static_cast<int32_t>(i)));
    }
    CHECK(h->GetCount(K2, HcDate(d100), HcDate(d100 + 20), n) && n == 4.0f);
    CHECK(!h->Coalesce(0x12345678, HcDate(d100), HcDate(d100 + 20), 0));
    CHECK(h->Coalesce(K2, HcDate(d100), HcDate(d100 + 20), 0));  // one month: the last group stays open
    CHECK(h->DisposeRange(K2, HcDate(d100 + 10), HcDate(d100 + 13)));  // keeps the last point of the range
    CHECK(h->GetCount(K2, HcDate(d100), HcDate(d100 + 20), n) && n == 1.0f);
    CHECK(h->DisposeAll(K2));
    CHECK(!h->DisposeAll(0x12345678));
    // The yearly pass through the agent messages: SimNewYear arms it; each
    // SimNewDay step answers true or false (call again).
    if (w.deliverTo != nullptr) {
        HcMessage year(kHcMsgSimNewYear, 0);
        CHECK(w.deliverTo->DoMessage(&year));
        for (int i = 0; i < 6; ++i) {
            HcMessage day(kHcMsgSimNewDay, 0);
            w.deliverTo->DoMessage(&day);
        }
    }
}

// The two-thread checks below interleave a reader thread with the main
// thread's steps. On a busy machine a freshly created thread may not run at
// all before the main thread's loop ends (the 2000/3000 steps take only
// milliseconds), so the main thread never assumes it ran: the reader signals
// HcAnswerGate every time the plugin answers it, and the main thread blocks on
// that signal with a bounded deadline -- never a sleep, so it wakes the moment
// the answer lands. Every `kHcThreadCheckpoint` steps it waits until the
// reader has answered again since the previous checkpoint, which also proves
// the getters answer between the steps throughout (stronger than the single
// end-of-loop check, kept below). A wait that runs out of time makes the
// check fail rather than hang.
constexpr int kHcThreadCheckpoint = 500;
constexpr std::chrono::seconds kHcThreadDeadline{60};

class HcAnswerGate {
public:
    // Called by the reader thread once the plugin has answered it.
    void Arrive() {
        {
            std::lock_guard<std::mutex> hold(m_);
            ++count_;
        }
        cv_.notify_all();
    }
    // Blocks until the reader has answered again since `seen`, then updates
    // `seen`. False once `deadline` passes first. The increment happens under
    // m_ and the waiter holds m_ across its check and its wait, so an answer
    // cannot slip between them and be missed.
    bool WaitForAnswer(int& seen, std::chrono::steady_clock::time_point deadline) {
        std::unique_lock<std::mutex> hold(m_);
        while (count_ <= seen) {
            if (cv_.wait_until(hold, deadline) == std::cv_status::timeout && count_ <= seen) return false;
        }
        seen = count_;
        return true;
    }
    int Answers() const {
        std::lock_guard<std::mutex> hold(m_);
        return count_;
    }

private:
    mutable std::mutex m_;
    std::condition_variable cv_;
    int count_ = 0;
};

// Queries from another thread while the main thread records (the lock).
void HcHistoryThreads(cFC4ISC4HistoryWarehouse* h, HcWorld& w) {
    std::atomic<bool> stop{false};
    std::atomic<bool> started{false};
    std::atomic<int> answers{0};
    HcAnswerGate gate;
    std::thread t([&] {
        float n = 0;
        started = true;
        while (!stop.load()) {
            if (h->GetCount(0x0A1100E8, HcDate(kHcDay0), HcDate(kHcDay0 + 100000), n)) gate.Arrive();
        }
    });
    // The querying thread runs before the records start (otherwise a fast
    // main thread can finish all 2000 before the thread's first query).
    while (!started.load()) std::this_thread::yield();
    const auto deadline = std::chrono::steady_clock::now() + kHcThreadDeadline;
    int seen = 0, checkpoints = 0;
    bool answering = true;
    for (int i = 0; i < 2000; ++i) {
        w.date = kHcDay0 + 200 + static_cast<uint32_t>(i);
        h->RecordHistoryData(0x0A1100E8, i);
        // From the first record on the key exists, so every query answers.
        if (answering && i % kHcThreadCheckpoint == 0) {
            answering = gate.WaitForAnswer(seen, deadline);
            if (answering) ++checkpoints;
        }
    }
    stop = true;
    t.join();
    CHECK(answering && checkpoints == 2000 / kHcThreadCheckpoint);  // answers between the records, throughout
    CHECK(gate.Answers() > 0);  // the reader thread got an answer from the plugin
}

std::vector<uint8_t> HcWrite(cIGZSerializable* s) {
    LandfillOStream out;
    CHECK(s->Write(out));
    return out.bytes;
}

int HistoryEnableTest(const std::string& dll, const std::string& ini, const std::string& log) {
    SetEnvironmentVariableA("FC4_DISABLE", nullptr);
    SetEnvironmentVariableA("FC4_ENABLE", nullptr);
    WriteTextFile(ini, "[subsystems]\nenable = history\n");
    HMODULE hd = LoadLibraryA(dll.c_str());
    CHECK(hd != nullptr);
    if (hd == nullptr) return 1;
    auto getDirector = reinterpret_cast<cIGZCOMDirector* (*)()>(GetProcAddress(hd, "GZDllGetGZCOMDirector"));
    auto seam = reinterpret_cast<FC4TestSeamFn>(GetProcAddress(hd, "FC4TestSeam"));
    cIGZCOMDirector* director = getDirector ? getDirector() : nullptr;
    CHECK(director != nullptr && seam != nullptr);
    SetGamePrecision();
    if (director != nullptr && seam != nullptr) {
        std::vector<uint32_t> classes;
        director->EnumClassObjects(&CollectClass, &classes);
        CHECK(classes.size() == 5);  // simulator (1) + budget (3) + history (1)
        CHECK(std::count(classes.begin(), classes.end(), kCLSID_History) == 1);

        static HcWorld w;
        const FC4HistClockHooks hooks = HcHistoryHooks(&w);
        CHECK(HcInstall(seam, &hooks) == 1);
        std::vector<uint8_t> record;
        void* obj = nullptr;
        CHECK(director->GetClassObject(kCLSID_History, kIID_cISC4HistoryWarehouse, &obj));
        auto* h = static_cast<cFC4ISC4HistoryWarehouse*>(obj);
        if (h != nullptr) {
            // §1: 0x69F4A803 and 1 at +0, cIGZSerializable at +8, nothing else -
            // not cIGZMessageTarget2 although the +4 sub-object exists [quirk].
            void* p = nullptr;
            CHECK(h->QueryInterface(GZIID_cIGZUnknown, &p) && p == static_cast<void*>(h));
            if (p != nullptr) static_cast<cIGZUnknown*>(p)->Release();
            p = nullptr;
            CHECK(h->QueryInterface(kIID_cISC4HistoryWarehouse, &p) && p == static_cast<void*>(h));
            if (p != nullptr) static_cast<cIGZUnknown*>(p)->Release();
            cIGZSerializable* ser = nullptr;
            CHECK(h->QueryInterface(GZIID_cIGZSerializable, reinterpret_cast<void**>(&ser)) && ser != nullptr);
            CHECK(reinterpret_cast<char*>(ser) == reinterpret_cast<char*>(h) + 8);
            if (ser != nullptr) CHECK(ser->GetGZCLSID() == kCLSID_History);
            p = reinterpret_cast<void*>(0x1234);
            CHECK(!h->QueryInterface(kHcIIDMessageTarget2, &p) && p == reinterpret_cast<void*>(0x1234));
            CHECK(!h->QueryInterface(0x12345678, &p) && p == reinterpret_cast<void*>(0x1234));
            CHECK(!h->QueryInterface(0x287FB697, &p) && p == reinterpret_cast<void*>(0x1234));
            // §3 Init: the +4 target subscribes to the record, then the queue
            // length; agents day (4) then year (7), flags 3.
            CHECK(h->Init());
            CHECK(w.notes.size() == 2 && w.agents.size() == 2);
            cIGZMessageTarget2* target = w.notes.empty() ? nullptr : w.notes[0].target;
            CHECK(reinterpret_cast<char*>(target) == reinterpret_cast<char*>(h) + 4);
            if (w.notes.size() == 2) {
                CHECK(w.notes[0].type == kHcMsgRecord && w.notes[0].add);
                CHECK(w.notes[1].type == kHcMsgQueueLength && w.notes[1].add && w.notes[1].target == target);
            }
            if (w.agents.size() == 2) {
                CHECK(w.agents[0].level == 4 && w.agents[0].flags == 3 && w.agents[0].add);
                CHECK(w.agents[1].level == 7 && w.agents[1].flags == 3 && w.agents[1].add);
                CHECK(w.agents[0].name == "cSC4HistoryWarehouse" && w.agents[1].name == "cSC4HistoryWarehouse");
                CHECK(w.agents[0].target == target);
            }
            w.deliverTo = target;
            HcHistoryRecordAndQuery(h, w);
            HcHistoryThreads(h, w);
            // §7: version 2, then the series in ascending key order.
            if (ser != nullptr) {
                record = HcWrite(ser);
                CHECK(record.size() > 6 && record[0] == 2 && record[1] == 0);
            }
            // §3 Shutdown: both unsubscribed, the agent removed; the series stay.
            CHECK(h->Shutdown());
            CHECK(w.notes.size() == 4 && w.agents.size() == 3);
            if (w.notes.size() == 4) CHECK(!w.notes[2].add && !w.notes[3].add);
            if (w.agents.size() == 3) CHECK(!w.agents[2].add && w.agents[2].target == target);
            float n = 0;
            CHECK(h->GetCount(0x0A1100E8, HcDate(kHcDay0), HcDate(kHcDay0 + 100000), n) && n == 1.0f);
            if (ser != nullptr) {
                CHECK(HcWrite(ser) == record);
                ser->Release();
            }
            w.deliverTo = nullptr;
            CHECK(h->Release() == 0);
        }
        // Read before Init (§7): the record loads into a new warehouse and
        // writes back byte for byte; version 3 is refused; a version-1
        // maxCount of 100 reads as 0.
        for (uint16_t version : {uint16_t{2}, uint16_t{3}}) {
            obj = nullptr;
            CHECK(director->GetClassObject(kCLSID_History, kIID_cISC4HistoryWarehouse, &obj));
            auto* fresh = static_cast<cFC4ISC4HistoryWarehouse*>(obj);
            if (fresh == nullptr) continue;
            cIGZSerializable* s = nullptr;
            CHECK(fresh->QueryInterface(GZIID_cIGZSerializable, reinterpret_cast<void**>(&s)) && s != nullptr);
            if (s != nullptr && !record.empty()) {
                std::vector<uint8_t> bytes = record;
                bytes[0] = static_cast<uint8_t>(version);
                LandfillIStream in(bytes);
                CHECK(s->Read(in) == (version == 2));
                if (version == 2) CHECK(HcWrite(s) == record);
            }
            if (s != nullptr) s->Release();
            CHECK(fresh->Release() == 0);  // never initialised: no unsubscription
        }
        {
            // v1: one series, key 7, maxCount 100, points (d, 1), (d, 2), (d + 1, 3).
            const uint32_t d = kHcDay0;
            std::vector<uint8_t> v1 = {1, 0};
            auto u32 = [&v1](uint32_t x) {
                for (int i = 0; i < 4; ++i) v1.push_back(static_cast<uint8_t>(x >> (8 * i)));
            };
            u32(1);
            u32(7);
            u32(100);
            u32(3);
            u32(d);
            u32(1);
            u32(d);
            u32(2);
            u32(d + 1);
            u32(3);
            obj = nullptr;
            CHECK(director->GetClassObject(kCLSID_History, kIID_cISC4HistoryWarehouse, &obj));
            auto* fresh = static_cast<cFC4ISC4HistoryWarehouse*>(obj);
            cIGZSerializable* s = nullptr;
            if (fresh != nullptr && fresh->QueryInterface(GZIID_cIGZSerializable, reinterpret_cast<void**>(&s))) {
                LandfillIStream in(v1);
                CHECK(s->Read(in));
                std::vector<uint8_t> want = {2, 0};
                auto put = [&want](uint32_t x) {
                    for (int i = 0; i < 4; ++i) want.push_back(static_cast<uint8_t>(x >> (8 * i)));
                };
                put(1);
                put(7);
                put(0);  // 100 -> 0 in a version-1 record
                put(2);
                put(d);
                put(2);  // the second 2000-01-01 entry overwrote the first
                put(d + 1);
                put(3);
                CHECK(HcWrite(s) == want);
                s->Release();
            }
            if (fresh != nullptr) CHECK(fresh->Release() == 0);
        }
        CHECK(HcInstall(seam, nullptr) == 1);
    }
    FreeLibrary(hd);
    const std::string text = ReadFile(log);
    const auto npos = std::string::npos;
    CHECK(text.find("subsystem 'history': enabled (1 class replaced) by FreeCity4.ini (enable=)") != npos);
    CHECK(text.find("history Init: ok=1") != npos);
    CHECK(text.find("off-main calls") != npos);
    CHECK(text.find("read back and rewritten identical)") != npos);
    CHECK(text.find("WARNING: released last reference") == npos && text.find("WARNING: over-release") == npos);
    if (g_failures == 0) std::printf("plugin history replacement test: all checks passed\n");
    return g_failures == 0 ? 0 : 1;
}

// ---- the history shadow (FC4HistoryShadow.h, seam op 54 op 1) --------------------------------------------

intptr_t HcShadow(FC4TestSeamFn seam, FC4HistoryShadowTestInput& in) {
    in.struct_size = sizeof(in);
    FC4HistClockTestInput t{};
    t.op = 1;
    t.arg = &in;
    return seam(kFC4SeamHistClock, &t, nullptr);
}

FC4HistoryShadowTestInput HcEvent(cIGZUnknown* orig, uint32_t event, uint32_t type = 0, uint32_t d1 = 0,
                                  uint32_t d2 = 0, int32_t julian = 0) {
    FC4HistoryShadowTestInput in{};
    in.event = event;
    in.original = orig;
    in.type = type;
    in.data1 = d1;
    in.data2 = d2;
    in.julian = julian;
    return in;
}

int HistoryShadowIniTest(const std::string& dll, const std::string& ini, const std::string& log) {
    SetEnvironmentVariableA("FC4_DISABLE", nullptr);
    SetEnvironmentVariableA("FC4_ENABLE", nullptr);
    // With ours enabled the shadow is inactive in the game; the seam still
    // runs it, with FreeCity4's warehouse standing in for the original.
    WriteTextFile(ini, "[subsystems]\nenable = history\n[tools]\nhistory_shadow = 1\n");
    HMODULE hd = LoadLibraryA(dll.c_str());
    CHECK(hd != nullptr);
    if (hd == nullptr) return 1;
    auto getDirector = reinterpret_cast<cIGZCOMDirector* (*)()>(GetProcAddress(hd, "GZDllGetGZCOMDirector"));
    auto seam = reinterpret_cast<FC4TestSeamFn>(GetProcAddress(hd, "FC4TestSeam"));
    cIGZCOMDirector* director = getDirector ? getDirector() : nullptr;
    CHECK(director != nullptr && seam != nullptr);
    SetGamePrecision();
    if (director != nullptr && seam != nullptr) {
        static HcWorld w;
        const FC4HistClockHooks hooks = HcHistoryHooks(&w);
        CHECK(HcInstall(seam, &hooks) == 1);
        void* obj = nullptr;
        CHECK(director->GetClassObject(kCLSID_History, kIID_cISC4HistoryWarehouse, &obj));
        auto* h = static_cast<cFC4ISC4HistoryWarehouse*>(obj);
        if (h != nullptr) {
            cIGZUnknown* orig = h;
            CHECK(h->Init());
            w.deliverTo = w.notes.empty() ? nullptr : w.notes[0].target;
            // Before the load: no shadow core, nothing taken.
            FC4HistoryShadowTestInput in = HcEvent(orig, 3);
            CHECK(HcShadow(seam, in) == -1);
            const uint32_t K = 0x0A1100E8, K2 = 0xAA11F837;
            CHECK(h->SetHistoryDataQueueLength(K, 3));
            for (uint32_t i = 0; i < 40; ++i) {
                w.date = kHcDay0 + 10 * i;
                h->RecordHistoryData(K, static_cast<int32_t>(i * i));
                h->RecordHistoryData(K2, static_cast<int32_t>(1000 - 7 * i));
            }
            // City load: the original's record read and written back, then
            // the queries on it.
            in = HcEvent(orig, 0, 0, 0, 0, static_cast<int32_t>(w.date));
            CHECK(HcShadow(seam, in) == 0);
            CHECK(in.compared > 2);
            // Messages reach both (as the message server delivers them).
            for (uint32_t i = 40; i < 80; ++i) {
                w.date = kHcDay0 + 10 * i;
                h->RecordHistoryData(K2, static_cast<int32_t>(3 * i));
                in = HcEvent(orig, 1, kHcMsgRecord, K2, 3 * i);
                CHECK(HcShadow(seam, in) == 1);
            }
            HcMessage q(kHcMsgQueueLength, 0x6A51178B, 2);
            w.deliverTo->DoMessage(&q);
            in = HcEvent(orig, 1, kHcMsgQueueLength, 0x6A51178B, 2);
            CHECK(HcShadow(seam, in) == 1);
            // The yearly pass through the agent deliveries: the shadow steps
            // its core after the original's step.
            w.date = kHcDay0 + 3650;  // 2009-12-29
            in = HcEvent(orig, 2, kHcMsgSimNewYear);
            CHECK(HcShadow(seam, in) == 1 && in.finished == 1);
            for (int i = 0; i < 4; ++i) {
                in = HcEvent(orig, 2, kHcMsgSimNewDay);
                CHECK(HcShadow(seam, in) == 1);
            }
            // The month comparison: exact.
            in = HcEvent(orig, 3, 0, 0, 0, static_cast<int32_t>(w.date));
            CHECK(HcShadow(seam, in) == 0);
            CHECK(in.compared > 100);
            // A record the shadow did not see: found, then the core is rebuilt.
            w.date += 1;
            h->RecordHistoryData(K2, 77);
            in = HcEvent(orig, 3, 0, 0, 0, static_cast<int32_t>(w.date));
            CHECK(HcShadow(seam, in) > 0);
            in = HcEvent(orig, 3, 0, 0, 0, static_cast<int32_t>(w.date));
            CHECK(HcShadow(seam, in) == 0);
            // City shutdown: the core dropped.
            in = HcEvent(orig, 4);
            CHECK(HcShadow(seam, in) == 0);
            in = HcEvent(orig, 3);
            CHECK(HcShadow(seam, in) == -1);
            CHECK(h->Shutdown());
            w.deliverTo = nullptr;
            CHECK(h->Release() == 0);
        }
        CHECK(HcInstall(seam, nullptr) == 1);
    }
    FreeLibrary(hd);
    const std::string text = ReadFile(log);
    const auto npos = std::string::npos;
    CHECK(text.find("history shadow: requested (history_shadow from FreeCity4.ini) but INACTIVE") != npos);
    CHECK(text.find("load: the original's record") != npos && text.find("written back identical; mismatches=0") != npos);
    CHECK(text.find("month: series=3 ") != npos);
    CHECK(text.find("record=") != npos && text.find("DIFFERENT") != npos);
    CHECK(text.find("history shadow (cont.) series 0xAA11F837") != npos);
    CHECK(text.find("rebuilt from the original's record") != npos);
    CHECK(text.find("years=1") != npos);
    CHECK(text.find("WARNING: released last reference") == npos && text.find("WARNING: over-release") == npos);
    if (g_failures == 0) std::printf("plugin history shadow test: all checks passed\n");
    return g_failures == 0 ? 0 : 1;
}

// ---- the 24-hour clock (clock24.md, seam op 54 op 0) -----------------------------------------------------------

constexpr uint32_t kCLSID_Clock24 = 0xA7E7F929;
constexpr uint32_t kIID_cISC424HourClock = 0x681BD53A;
constexpr uint32_t kHcMsgCheat = 0x230E27AC;
constexpr uint32_t kHcCheatWhatTime = 0x281F7858;
constexpr uint32_t kHcCheatStopwatch = 0xC9B73FBB;
constexpr uint32_t kHcHour = 0x44778230, kHcTick = 0x44778231, kHcVHour = 0x44778232, kHcVTick = 0x44778233,
                   kHcDayNight = 0x44778234, kHcPause = 0x44778235, kHcHidden = 0x44778236;

struct HcPost {
    uint32_t type, d2, d3;
    void* void1;
    cIGZUnknown* unk;
};
struct HcClockWorld {
    uint64_t now = 1000000;
    uint32_t clockReads = 0;
    std::vector<HcPost> posts;
    std::vector<std::pair<void*, uint32_t>> framework;  // service, op
    std::vector<std::array<uint32_t, 2>> cheats;        // op, id
    std::vector<std::string> cheatNames;
    bool manager = true;
    bool tuning = true;
};

uint64_t HcNow(void* c) {
    auto* w = static_cast<HcClockWorld*>(c);
    ++w->clockReads;
    return w->now;
}
void HcPostHook(void* c, cIGZMessage2* m) {
    cIGZMessage2Standard* s = nullptr;
    if (m != nullptr && m->QueryInterface(GZIID_cIGZMessage2Standard, reinterpret_cast<void**>(&s)) && s != nullptr) {
        static_cast<HcClockWorld*>(c)->posts.push_back({m->GetType(), static_cast<uint32_t>(s->GetData2()),
                                                        static_cast<uint32_t>(s->GetData3()), s->GetVoid1(),
                                                        s->GetIGZUnknown()});
        s->Release();
    }
}
void HcFramework(void* c, void* service, uint32_t op) {
    static_cast<HcClockWorld*>(c)->framework.push_back({service, op});
}
bool HcCheat(void* c, uint32_t op, uint32_t id, const char* name) {
    HcClockWorld* w = static_cast<HcClockWorld*>(c);
    if (op == 0) return w->manager;
    w->cheats.push_back({op, id});
    if (op == 1) w->cheatNames.push_back(name ? name : "");
    return true;
}
bool HcTuning(void* c, fc4_clock24_tuning* t) {
    if (!static_cast<HcClockWorld*>(c)->tuning) return false;
    t->has_magnification = t->has_night_start = t->has_night_end = 1;
    t->magnification = 135.0f;  // the shipped "Main Simulator" values (§6)
    t->night_start = 20;
    t->night_end = 1;
    return true;
}

FC4HistClockHooks HcClockHooks(HcClockWorld* w) {
    FC4HistClockHooks h{};
    h.struct_size = sizeof(h);
    h.ctx = w;
    h.post = &HcPostHook;
    h.framework = &HcFramework;
    h.cheat = &HcCheat;
    h.now_micros = &HcNow;
    h.clock_tuning = &HcTuning;
    return h;
}

std::vector<uint8_t> HcClockRecord(uint16_t version, int32_t pause, float mag, float max, uint32_t seconds,
                                   uint32_t nightStart, uint32_t nightEnd) {
    std::vector<uint8_t> b;
    auto put = [&b](const void* p, size_t n) {
        const uint8_t* q = static_cast<const uint8_t*>(p);
        b.insert(b.end(), q, q + n);
    };
    put(&version, 2);
    put(&pause, 4);
    put(&mag, 4);
    put(&max, 4);
    put(&seconds, 4);
    if (version < 2) {
        const uint8_t timer[10] = {2, 0, 0, 0, 0, 0, 0, 0, 0, 0};  // units 2, elapsed 0, stopped (§7 corpus)
        put(timer, 10);
    }
    put(&nightStart, 4);
    put(&nightEnd, 4);
    return b;
}

bool HcSamePost(const HcPost& p, uint32_t type, uint32_t d2, uint32_t d3) {
    return p.type == type && p.d2 == d2 && p.d3 == d3;
}

int Clock24EnableTest(const std::string& dll, const std::string& ini, const std::string& log) {
    SetEnvironmentVariableA("FC4_DISABLE", nullptr);
    SetEnvironmentVariableA("FC4_ENABLE", nullptr);
    WriteTextFile(ini, "[subsystems]\nenable = clock24\n");
    HMODULE hd = LoadLibraryA(dll.c_str());
    CHECK(hd != nullptr);
    if (hd == nullptr) return 1;
    auto getDirector = reinterpret_cast<cIGZCOMDirector* (*)()>(GetProcAddress(hd, "GZDllGetGZCOMDirector"));
    auto seam = reinterpret_cast<FC4TestSeamFn>(GetProcAddress(hd, "FC4TestSeam"));
    cIGZCOMDirector* director = getDirector ? getDirector() : nullptr;
    CHECK(director != nullptr && seam != nullptr);
    SetGamePrecision();
    if (director != nullptr && seam != nullptr) {
        std::vector<uint32_t> classes;
        director->EnumClassObjects(&CollectClass, &classes);
        CHECK(classes.size() == 5);  // simulator (1) + budget (3) + clock (1)
        CHECK(std::count(classes.begin(), classes.end(), kCLSID_Clock24) == 1);

        static HcClockWorld w;
        const FC4HistClockHooks hooks = HcClockHooks(&w);
        CHECK(HcInstall(seam, &hooks) == 1);
        void* obj = nullptr;
        CHECK(director->GetClassObject(kCLSID_Clock24, kIID_cISC424HourClock, &obj));
        auto* c = static_cast<cFC4ISC424HourClock*>(obj);
        if (c != nullptr) {
            char* base = reinterpret_cast<char*>(c);
            // §1: the clock at +0 (0x681BD53A, 1), the message target at +4,
            // the serializable at +8, the system service at +0xC; nothing else.
            void* p = nullptr;
            CHECK(c->QueryInterface(GZIID_cIGZUnknown, &p) && p == static_cast<void*>(c));
            if (p != nullptr) static_cast<cIGZUnknown*>(p)->Release();
            p = nullptr;
            CHECK(c->QueryInterface(kIID_cISC424HourClock, &p) && p == static_cast<void*>(c));
            if (p != nullptr) static_cast<cIGZUnknown*>(p)->Release();
            p = nullptr;
            CHECK(c->QueryInterface(kHcIIDMessageTarget2, &p) && static_cast<char*>(p) == base + 4);
            if (p != nullptr) static_cast<cIGZUnknown*>(p)->Release();
            cIGZSerializable* ser = nullptr;
            CHECK(c->QueryInterface(GZIID_cIGZSerializable, reinterpret_cast<void**>(&ser)) && ser != nullptr);
            CHECK(reinterpret_cast<char*>(ser) == base + 8);
            if (ser != nullptr) CHECK(ser->GetGZCLSID() == kCLSID_Clock24);
            cIGZSystemService* svc = nullptr;
            CHECK(c->QueryInterface(GZIID_cIGZSystemService, reinterpret_cast<void**>(&svc)) && svc != nullptr);
            CHECK(reinterpret_cast<char*>(svc) == base + 0xC);
            if (svc != nullptr) {
                CHECK(svc->GetServiceID() == 0xA7E7F92A);
                CHECK(svc->GetServicePriority() == -2000000);
                CHECK(svc->OnTick(0));
                // The service's QueryInterface is the clock's (vtable 0x00A8652C slots 0-2).
                p = nullptr;
                CHECK(svc->QueryInterface(kIID_cISC424HourClock, &p) && p == static_cast<void*>(c));
                if (p != nullptr) static_cast<cIGZUnknown*>(p)->Release();
            }
            p = reinterpret_cast<void*>(0x1234);
            CHECK(!c->QueryInterface(0x12345678, &p) && p == reinterpret_cast<void*>(0x1234));
            // §2 construction: noon, magnification 100 in [0.05, 2000], night 20 -> 4.
            float f = -1.0f;
            c->GetTimePercentage(&f);  // slot 7 (the game's form: float* out)
            CHECK(HcBits(f) == 0x3F000000);
            CHECK(c->GetHour() == 12 && c->GetVisualHour() == 12 && !c->IsNight());
            CHECK(c->GetEventHour(0) == 20 && c->GetEventHour(1) == 4 && c->GetEventHour(2) == 0);
            float mn = 0, mx = 0;
            CHECK(c->GetTimeMagnification(f) == HcBits(100.0f) && f == 100.0f);  // slot 20: the bits in EAX
            CHECK(c->GetTimeMagnificationMinMax(mn, mx) == HcBits(2000.0f));
            CHECK(HcBits(mn) == 0x3D4CCCCD && mx == 2000.0f);
            // §7 / §12 test 13: a record read before Init (magnification 50,
            // max 1500, 21:00, night 22 / 5), then Init with the shipped tuning.
            if (ser != nullptr) {
                const std::vector<uint8_t> rec = HcClockRecord(2, 0, 50.0f, 1500.0f, 75600, 22, 5);
                LandfillIStream in(rec);
                CHECK(ser->Read(in));
                CHECK(c->GetHour() == 21 && c->GetVisualHour() == 12);
            }
            const uint64_t t0 = w.now;
            CHECK(w.clockReads == 0);  // Read and diagnostics leave a stopped timer alone.
            CHECK(c->Init());
            CHECK(w.clockReads == 1);  // Start only; logging must not sample it again.
            CHECK(c->GetTimeMagnification(f) == HcBits(135.0f));
            CHECK(c->GetTimeMagnificationMinMax(mn, mx) == HcBits(1500.0f));
            CHECK(c->GetEventHour(0) == 20 && c->GetEventHour(1) == 1);
            CHECK(c->GetVisualHour() == 12);
            // §4.2: AddSystemService then AddToOnIdle with the +0xC service;
            // the cheats one at a time (CK12).
            CHECK(w.framework.size() == 2);
            if (w.framework.size() == 2) {
                CHECK(w.framework[0].first == base + 0xC && w.framework[0].second == 0);
                CHECK(w.framework[1].first == base + 0xC && w.framework[1].second == 1);
            }
            const std::vector<std::array<uint32_t, 2>> wantInit = {
                {1, kHcCheatWhatTime}, {2, kHcCheatWhatTime}, {1, kHcCheatStopwatch}, {2, kHcCheatStopwatch}};
            CHECK(w.cheats == wantInit);
            CHECK(w.cheatNames.size() == 2 && w.cheatNames[0] == "WhatTimeIzIt" && w.cheatNames[1] == "Stopwatch");
            CHECK(c->Init());  // initialised: nothing again
            CHECK(w.clockReads == 1);
            CHECK(w.framework.size() == 2 && w.cheats.size() == 4);
            // §4.3 / §4.9: the step threshold at 135, then the first step
            // after the load moves the visual time from noon to 21:01:01.
            if (svc != nullptr) {
                w.now = t0 + 451851;
                CHECK(svc->OnIdle(0));
                CHECK(c->GetHour() == 21 && c->GetMinute() == 0 && w.posts.empty());
                w.now = t0 + 451852;
                CHECK(svc->OnIdle(0));
                CHECK(c->GetMinute() == 1 && c->GetSeconds() == 1);
                CHECK(c->GetVisualHour() == 21 && c->IsNight());
                CHECK(w.posts.size() == 3);
                if (w.posts.size() == 3) {
                    CHECK(HcSamePost(w.posts[0], kHcVHour, 21, 0));
                    CHECK(HcSamePost(w.posts[1], kHcVTick, 21, 0));
                    CHECK(HcSamePost(w.posts[2], kHcDayNight, 0, 0));
                    CHECK(w.posts[0].void1 == static_cast<void*>(c) && w.posts[0].unk == c);
                }
                // A long stall: at most 1200 game seconds.
                w.now += 100000000;
                CHECK(svc->OnIdle(0));
                CHECK(c->GetHour() == 21 && c->GetMinute() == 21 && c->GetSeconds() == 1);
            }
            // §4.6 setters (silent but SetHour), §4.1 getters.
            w.posts.clear();
            c->SetHour(12);
            CHECK(w.posts.size() == 2);
            if (w.posts.size() == 2) CHECK(HcSamePost(w.posts[0], kHcHour, 12, 0) && HcSamePost(w.posts[1], kHcTick, 12, 0));
            CHECK(c->SetMinute(75) == (12 * 60 + 15) * 60);
            CHECK(c->SetSeconds(61) == 15);  // EAX = GetMinute()
            CHECK(c->GetSeconds() == 1 && c->GetVisualHour() == 21);  // no setter moves the visual time
            c->SetMinute(0);
            c->SetSeconds(0);
            c->GetTimePercentage(&f);
            CHECK(HcBits(f) == 0x3F000000);
            c->GetVisualTimePercentage(&f);  // slot 9
            CHECK(f == static_cast<float>(76861 * 1.1574074e-05f));  // visual 21:21:01
            float pct = -0.5f;
            c->SetTimePercentage(pct);  // slot 8: wraps to 66,496 (§4.6)
            CHECK(c->GetHour() == 66496 / 3600);
            CHECK(c->SetTimeMagnification(5000.0f));  // clamped to the saved max 1500
            CHECK(c->GetTimeMagnification(f) == HcBits(1500.0f));
            CHECK(c->SetTimeMagnificationMinMax(10.0f, 5.0f));
            CHECK(c->GetTimeMagnificationMinMax(mn, mx) == HcBits(10.0f) && mn == 10.0f);
            CHECK(c->SetTimeMagnificationMinMax(0.05f, 2000.0f) && c->SetTimeMagnification(135.0f));
            // §4.7 pause counters.
            w.posts.clear();
            CHECK(c->Pause(true) && c->Pause(true) && c->IsPaused());
            CHECK(c->Pause(false) && c->Pause(false) && !c->IsPaused() && c->Pause(false));
            CHECK(c->HiddenPause(true) && c->IsHiddenPaused() && c->HiddenPause(false) && !c->IsHiddenPaused());
            CHECK(w.posts.size() == 4);
            if (w.posts.size() == 4) {
                CHECK(HcSamePost(w.posts[0], kHcPause, 1, 0) && HcSamePost(w.posts[1], kHcPause, 0, 0));
                CHECK(HcSamePost(w.posts[2], kHcHidden, 1, 0) && HcSamePost(w.posts[3], kHcHidden, 0, 0));
            }
            // §4.5 modes and notifications.
            w.posts.clear();
            c->SetVisualTimeMode(1);
            CHECK(c->GetVisualTimeMode() == 1 && c->GetVisualHour() == 12 && !c->IsNight());
            CHECK(w.posts.size() == 3);
            if (w.posts.size() == 3) CHECK(HcSamePost(w.posts[2], kHcDayNight, 1, 0));
            c->EnableDayNightTransitionNotifications(false);
            c->EnableDayNightTransitionNotifications(true);
            CHECK(w.posts.size() == 4);
            if (w.posts.size() == 4) CHECK(HcSamePost(w.posts[3], kHcDayNight, 1, 0));
            c->SetVisualTimeMode(0);
            // §4.8 cheats through the message target (void2 = the text).
            cIGZMessageTarget2* target = nullptr;
            CHECK(c->QueryInterface(kHcIIDMessageTarget2, reinterpret_cast<void**>(&target)) && target != nullptr);
            if (target != nullptr) {
                cRZBaseString t1("WhatTimeIzIt 7");
                HcMessage m1(kHcMsgCheat, kHcCheatWhatTime, 0, static_cast<cIGZString*>(&t1));
                CHECK(target->DoMessage(&m1));
                CHECK(c->GetHour() == 7);
                cRZBaseString t2("Stopwatch PAUSE");
                HcMessage m2(kHcMsgCheat, kHcCheatStopwatch, 0, static_cast<cIGZString*>(&t2));
                CHECK(target->DoMessage(&m2) && c->IsPaused());
                cRZBaseString t3("Stopwatch resume");
                HcMessage m3(kHcMsgCheat, kHcCheatStopwatch, 0, static_cast<cIGZString*>(&t3));
                CHECK(target->DoMessage(&m3) && !c->IsPaused());
                cRZBaseString t4("Stopwatch magnify 5000");
                HcMessage m4(kHcMsgCheat, kHcCheatStopwatch, 0, static_cast<cIGZString*>(&t4));
                CHECK(target->DoMessage(&m4) && c->GetTimeMagnification(f) == HcBits(2000.0f));
                HcMessage other(0x12345678, 0);
                CHECK(target->DoMessage(&other));
                target->Release();
            }
            // Getters from another thread while the main thread steps (the lock).
            if (svc != nullptr) {
                std::atomic<bool> stop{false};
                HcAnswerGate gate;
                std::thread th([&] {
                    while (!stop.load()) {
                        if (c->GetHour() < 24 && c->GetVisualMinute() < 60) gate.Arrive();
                        float pf = 0;
                        c->GetVisualTimePercentage(&pf);
                    }
                });
                // Every kHcThreadCheckpoint steps, wait on HcAnswerGate until
                // the getters answered again, so they run between the steps
                // throughout however late the thread starts.
                const auto deadline = std::chrono::steady_clock::now() + kHcThreadDeadline;
                int seen = 0, checkpoints = 0;
                bool answering = true;
                for (int i = 0; i < 3000; ++i) {
                    w.now += 500000;
                    svc->OnIdle(0);
                    if (answering && i % kHcThreadCheckpoint == 0) {
                        answering = gate.WaitForAnswer(seen, deadline);
                        if (answering) ++checkpoints;
                    }
                }
                stop = true;
                th.join();
                CHECK(answering && checkpoints == 3000 / kHcThreadCheckpoint);
                CHECK(gate.Answers() > 0);  // the reader thread got an answer from the plugin
            }
            // §7: version 2, 26 bytes; the §7 example at noon with the shipped values.
            if (ser != nullptr) {
                c->SetHour(12);
                c->SetMinute(0);
                c->SetTimeMagnification(135.0f);
                const std::vector<uint8_t> rec = HcWrite(ser);
                const std::vector<uint8_t> example = {0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x07,
                                                      0x43, 0x00, 0x00, 0xFA, 0x44, 0xC0, 0xA8, 0x00, 0x00,
                                                      0x14, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00};
                CHECK(rec == example);
            }
            // §4.2 Shutdown: per cheat remove the notification, then unregister
            // (CK13); RemoveFromOnIdle, then RemoveSystemService.
            w.cheats.clear();
            w.framework.clear();
            CHECK(c->Shutdown());
            const std::vector<std::array<uint32_t, 2>> wantShutdown = {
                {3, kHcCheatWhatTime}, {4, kHcCheatWhatTime}, {3, kHcCheatStopwatch}, {4, kHcCheatStopwatch}};
            CHECK(w.cheats == wantShutdown);
            CHECK(w.framework.size() == 2);
            if (w.framework.size() == 2) CHECK(w.framework[0].second == 2 && w.framework[1].second == 3);
            CHECK(c->GetHour() == 0);  // Shutdown sets seconds := 0
            CHECK(c->Shutdown() && w.framework.size() == 2);
            if (svc != nullptr) svc->Release();
            if (ser != nullptr) ser->Release();
            CHECK(c->Release() == 0);
        }
        // §7: a version 1 record (36 bytes) skips its timer record; a version
        // 3 record reads as version 2; both write back as version 2. Without
        // a cheat manager nothing is registered.
        w.manager = false;
        for (uint16_t version : {uint16_t{1}, uint16_t{3}}) {
            obj = nullptr;
            CHECK(director->GetClassObject(kCLSID_Clock24, kIID_cISC424HourClock, &obj));
            auto* fresh = static_cast<cFC4ISC424HourClock*>(obj);
            if (fresh == nullptr) continue;
            cIGZSerializable* s = nullptr;
            CHECK(fresh->QueryInterface(GZIID_cIGZSerializable, reinterpret_cast<void**>(&s)) && s != nullptr);
            if (s != nullptr) {
                const std::vector<uint8_t> rec = HcClockRecord(version, 0, 135.0f, 2000.0f, 47000, 20, 1);
                CHECK(rec.size() == (version == 1 ? 36u : 26u));
                LandfillIStream in(rec);
                CHECK(s->Read(in));
                CHECK(HcWrite(s) == HcClockRecord(2, 0, 135.0f, 2000.0f, 47000, 20, 1));
                w.cheats.clear();
                CHECK(fresh->Init() && w.cheats.empty());
                CHECK(fresh->Shutdown() && w.cheats.empty());
                s->Release();
            }
            CHECK(fresh->Release() == 0);
        }
        CHECK(HcInstall(seam, nullptr) == 1);
    }
    FreeLibrary(hd);
    const std::string text = ReadFile(log);
    const auto npos = std::string::npos;
    CHECK(text.find("subsystem 'clock24': enabled (1 class replaced) by FreeCity4.ini (enable=)") != npos);
    CHECK(text.find("clock24 Init: ok=1 cheats=yes tuning=yes") != npos);
    CHECK(text.find("clock24 Shutdown:") != npos);
    CHECK(text.find("26 bytes, read back and rewritten identical)") != npos);
    CHECK(text.find("WARNING: released last reference") == npos && text.find("WARNING: over-release") == npos);
    if (g_failures == 0) std::printf("plugin 24-hour clock replacement test: all checks passed\n");
    return g_failures == 0 ? 0 : 1;
}

// ---- the 24-hour clock shadow (FC4Clock24Shadow.h, seam op 54 op 2) ---------------------------------------------

intptr_t HcClockShadow(FC4TestSeamFn seam, FC4Clock24ShadowTestInput& in) {
    in.struct_size = sizeof(in);
    FC4HistClockTestInput t{};
    t.op = 2;
    t.arg = &in;
    return seam(kFC4SeamHistClock, &t, nullptr);
}

FC4Clock24ShadowTestInput HcClockEvent(cIGZUnknown* orig, uint32_t event, uint32_t type = 0, uint32_t d2 = 0,
                                       uint32_t d3 = 0) {
    FC4Clock24ShadowTestInput in{};
    in.event = event;
    in.original = orig;
    in.type = type;
    in.data2 = d2;
    in.data3 = d3;
    in.julian = static_cast<int32_t>(kHcDay0);
    return in;
}

// The original's posts so far, to the shadow in order; the sum of the results.
intptr_t HcClockDeliver(FC4TestSeamFn seam, cIGZUnknown* orig, HcClockWorld& w) {
    intptr_t sum = 0;
    for (const HcPost& p : w.posts) {
        FC4Clock24ShadowTestInput in = HcClockEvent(orig, 2, p.type, p.d2, p.d3);
        const intptr_t r = HcClockShadow(seam, in);
        sum += r < 0 ? 1000 : r;
    }
    w.posts.clear();
    return sum;
}

int Clock24ShadowIniTest(const std::string& dll, const std::string& ini, const std::string& log) {
    SetEnvironmentVariableA("FC4_DISABLE", nullptr);
    SetEnvironmentVariableA("FC4_ENABLE", nullptr);
    // With ours enabled the shadow is inactive in the game; the seam still
    // runs it, with FreeCity4's clock standing in for the original.
    WriteTextFile(ini, "[subsystems]\nenable = clock24\n[tools]\nclock24_shadow = 1\n");
    HMODULE hd = LoadLibraryA(dll.c_str());
    CHECK(hd != nullptr);
    if (hd == nullptr) return 1;
    auto getDirector = reinterpret_cast<cIGZCOMDirector* (*)()>(GetProcAddress(hd, "GZDllGetGZCOMDirector"));
    auto seam = reinterpret_cast<FC4TestSeamFn>(GetProcAddress(hd, "FC4TestSeam"));
    cIGZCOMDirector* director = getDirector ? getDirector() : nullptr;
    CHECK(director != nullptr && seam != nullptr);
    SetGamePrecision();
    if (director != nullptr && seam != nullptr) {
        static HcClockWorld w;
        const FC4HistClockHooks hooks = HcClockHooks(&w);
        CHECK(HcInstall(seam, &hooks) == 1);
        void* obj = nullptr;
        CHECK(director->GetClassObject(kCLSID_Clock24, kIID_cISC424HourClock, &obj));
        auto* c = static_cast<cFC4ISC424HourClock*>(obj);
        cIGZSystemService* svc = nullptr;
        if (c != nullptr) c->QueryInterface(GZIID_cIGZSystemService, reinterpret_cast<void**>(&svc));
        CHECK(svc != nullptr);
        if (c != nullptr && svc != nullptr) {
            cIGZUnknown* orig = c;
            // Before the load: no shadow core.
            FC4Clock24ShadowTestInput in = HcClockEvent(orig, 3);
            CHECK(HcClockShadow(seam, in) == -1);
            CHECK(c->Init());
            c->EnableDayNightTransitionNotifications(true);
            // City load: the record, the tuning, the values.
            in = HcClockEvent(orig, 0);
            CHECK(HcClockShadow(seam, in) == 0);
            CHECK(in.compared > 10);
            // 1000 steps of 63 s from noon (past 20:00 and 01:00), some with the
            // messages before the idle pass, a stall, a hidden pause, a mode
            // change and a cheat on the way.
            intptr_t bad = 0;
            for (int i = 0; i < 1000; ++i) {
                w.now += (i == 300) ? 50000000 : 466676;  // a stall: 1200 s
                CHECK(svc->OnIdle(0));
                if (i % 3 == 0) {
                    bad += HcClockDeliver(seam, orig, w);  // the message first: it samples itself
                    in = HcClockEvent(orig, 1);
                    bad += HcClockShadow(seam, in) == 0 ? 0 : 1;
                } else {
                    in = HcClockEvent(orig, 1);
                    bad += HcClockShadow(seam, in) == 0 ? 0 : 1;
                    bad += HcClockDeliver(seam, orig, w);
                }
                if (i == 100) {
                    c->HiddenPause(true);
                    bad += HcClockDeliver(seam, orig, w);
                    w.now += 466676;
                    svc->OnIdle(0);  // paused: no step
                    c->HiddenPause(false);
                    bad += HcClockDeliver(seam, orig, w);  // the pause and resume, between two samples
                }
                if (i == 500) {
                    c->SetVisualTimeMode(1);
                    in = HcClockEvent(orig, 1);
                    bad += HcClockShadow(seam, in) == 0 ? 0 : 1;
                    bad += HcClockDeliver(seam, orig, w);
                    c->SetVisualTimeMode(0);
                    bad += HcClockDeliver(seam, orig, w);
                }
                if (i == 700) {
                    c->SetHour(3);  // WhatTimeIzIt 3: a jump
                    bad += HcClockDeliver(seam, orig, w);
                }
            }
            CHECK(bad == 0);
            in = HcClockEvent(orig, 3);
            CHECK(HcClockShadow(seam, in) == 0);
            CHECK(in.compared > 1000);
            // A magnification the shadow did not see: found once, then mirrored.
            c->SetTimeMagnification(50.0f);
            in = HcClockEvent(orig, 3);
            CHECK(HcClockShadow(seam, in) > 0);
            in = HcClockEvent(orig, 3);
            CHECK(HcClockShadow(seam, in) == 0);
            // A message the shadow core did not post.
            in = HcClockEvent(orig, 2, 0x44778230, 5, 0);
            CHECK(HcClockShadow(seam, in) == 1);
            in = HcClockEvent(orig, 3);
            CHECK(HcClockShadow(seam, in) == 1);
            // City shutdown: the core dropped.
            in = HcClockEvent(orig, 4);
            CHECK(HcClockShadow(seam, in) == 0);
            in = HcClockEvent(orig, 3);
            CHECK(HcClockShadow(seam, in) == -1);
            CHECK(c->Shutdown());
        }
        if (svc != nullptr) svc->Release();
        if (c != nullptr) CHECK(c->Release() == 0);
        CHECK(HcInstall(seam, nullptr) == 1);
    }
    FreeLibrary(hd);
    const std::string text = ReadFile(log);
    const auto npos = std::string::npos;
    CHECK(text.find("clock shadow: requested (clock24_shadow from FreeCity4.ini) but INACTIVE") != npos);
    CHECK(text.find("load: the original's record 26 bytes (version 2") != npos);
    CHECK(text.find("written back identical") != npos);
    CHECK(text.find("month: time=") != npos && text.find("mismatches=0; comparisons=1 clean=1") != npos);
    CHECK(text.find("GetTimeMagnification (EAX): original") != npos);
    CHECK(text.find("the original posted HourChanged (5, 0)") != npos);
    CHECK(text.find("WARNING: released last reference") == npos && text.find("WARNING: over-release") == npos);
    if (g_failures == 0) std::printf("plugin 24-hour clock shadow test: all checks passed\n");
    return g_failures == 0 ? 0 : 1;
}

}  // namespace
