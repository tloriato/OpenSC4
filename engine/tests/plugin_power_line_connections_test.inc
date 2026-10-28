// SPDX-License-Identifier: GPL-3.0-or-later
// network-occupants §5.1 / power §5.4: counted, potentially aliased outputs.
namespace {
struct PowerConnectionPole : cIGZUnknown {
    uint32_t refs = 1, peak = 1;
    bool QueryInterface(uint32_t, void**) override { return false; }
    uint32_t AddRef() override {
        ++refs;
        if (refs > peak) peak = refs;
        return refs;
    }
    uint32_t Release() override { return --refs; }
};

struct PowerConnectionLine : PowerConnectionPole {
    cIGZUnknown* first = nullptr;
    cIGZUnknown* second = nullptr;
    bool expose = true;
    uint32_t calls = 0;
    bool QueryInterface(uint32_t iid, void** out) override {
        if (!expose || iid != 0xC98F3B24 || out == nullptr) return false;
        *out = this;
        AddRef();
        return true;
    }
    virtual void GetConnectedPoles(cIGZUnknown*& a, cIGZUnknown*& b, bool& flag) {
        ++calls;
        a = first;
        b = second;
        if (a != nullptr) a->AddRef();
        if (b != nullptr) b->AddRef();
        flag = true;
    }
};

void PowerLineConnectionChecks(FC4TestSeamFn seam) {
    PowerConnectionPole a, b;
    PowerConnectionLine line;
    line.first = &a;
    line.second = &b;
    uint64_t pair[2] = {};
    CHECK(seam(kFC4SeamPowerLineConnections, &line, pair) == 1);
    CHECK(pair[0] == reinterpret_cast<uintptr_t>(&a) && pair[1] == reinterpret_cast<uintptr_t>(&b));
    CHECK(a.refs == 1 && b.refs == 1 && a.peak == 2 && b.peak == 2 && line.refs == 1);

    line.second = &a;
    CHECK(seam(kFC4SeamPowerLineConnections, &line, pair) == 1);
    CHECK(pair[0] == pair[1] && a.refs == 1 && a.peak == 3);

    line.second = nullptr;
    CHECK(seam(kFC4SeamPowerLineConnections, &line, pair) == 0);
    CHECK(pair[0] == 0 && pair[1] == 0 && a.refs == 1 && line.refs == 1);

    line.expose = false;
    pair[0] = pair[1] = 123;
    CHECK(seam(kFC4SeamPowerLineConnections, &line, pair) == 0);
    CHECK(pair[0] == 0 && pair[1] == 0 && line.calls == 3 && line.refs == 1);
}
// power §10.1 distinguishes full-EAX predicates from the AL-only slots 32-34.
void PowerQueryAbiChecks(FC4TestSeamFn seam) {
    auto* power = reinterpret_cast<cIGZUnknown*>(seam(kFC4SeamPowerCreate, nullptr, nullptr));
    CHECK(power != nullptr);
    if (power == nullptr) return;
    for (uint32_t slot : {19u, 37u, 38u}) {
        const fc4test::SlotCall result = fc4test::CallSlot(power, slot, {0u, 0u});
        CHECK(result.eax == 0u && result.espDelta == 0 && result.x87Pushed == 0);
    }
    const fc4test::SlotCall result = fc4test::CallSlot(power, 31u, {0u});
    CHECK(result.eax == 0u && result.espDelta == 0 && result.x87Pushed == 0);
    CHECK(power->Release() == 0u);
}
}  // namespace
