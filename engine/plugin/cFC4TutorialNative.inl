// SPDX-License-Identifier: GPL-3.0-or-later
// Included inside cFC4Tutorial.cpp's private namespace. TU6/TU9 native
// boundaries, resolved exclusively by compat against the supported image.
template <typename F> F WindowSlot(uint64_t token, unsigned slot) {
    auto* p = reinterpret_cast<void*>(static_cast<uintptr_t>(token));
    return reinterpret_cast<F>((*static_cast<void***>(p))[slot]);
}
void* Pointer(uint64_t token) { return reinterpret_cast<void*>(static_cast<uintptr_t>(token)); }

uint64_t CreateWindowCb(void* u) {
    fc4::CoreLock::Unlocked unlocked(Self(u)->Lock());
    const auto* native = fc4::compat::TutorialNative();
    // TU9 explicitly excludes an allocation-failure without-window fallback.
    if (native == nullptr || !Self(u)->BindNativeOwner()) std::terminate();
    using Allocate = void*(__cdecl*)(uint32_t);
    using Construct = void*(__thiscall*)(void*);
    void* block = reinterpret_cast<Allocate>(fc4::compat::DirectCall(fc4::compat::Direct::GlobalHeapAllocate))(100);
    if (block == nullptr) std::terminate();
    void* window = reinterpret_cast<Construct>(native->construct)(block);
    fc4::Log("tutorial native: constructed window=%p count=0 owner=%p", window, Self(u)->Primary());
    return static_cast<uint64_t>(reinterpret_cast<uintptr_t>(window));
}
void InitWindowCb(void* u, uint64_t w) {
    fc4::CoreLock::Unlocked unlocked(Self(u)->Lock());
    using Fn = void(__thiscall*)(void*);
    reinterpret_cast<Fn>(fc4::compat::TutorialNative()->init)(Pointer(w));
    fc4::Log("tutorial native: Init window=%p task=%d", Pointer(w), Self(u)->GetTutorialTaskNumber());
}
void CloseWindowCb(void* u, uint64_t w) {
    fc4::CoreLock::Unlocked unlocked(Self(u)->Lock());
    using Fn = void(__thiscall*)(void*);
    reinterpret_cast<Fn>(fc4::compat::TutorialNative()->close)(Pointer(w));
    fc4::Log("tutorial native: closed window=%p", Pointer(w));
}
uint64_t CurrentTaskCb(void* u, uint64_t w) {
    fc4::CoreLock::Unlocked unlocked(Self(u)->Lock());
    using Fn = void*(__thiscall*)(void*);
    return static_cast<uint64_t>(reinterpret_cast<uintptr_t>(WindowSlot<Fn>(w, 9)(Pointer(w))));
}
bool TaskEdgeCb(void* u, uint64_t task) {
    fc4::CoreLock::Unlocked unlocked(Self(u)->Lock());
    using Fn = bool(__thiscall*)(void*);
    return WindowSlot<Fn>(task, 25)(Pointer(task));
}
void FilterValueCb(void* u, uint64_t w, uint32_t value) {
    fc4::CoreLock::Unlocked unlocked(Self(u)->Lock());
    using Fn = void(__thiscall*)(void*, uint32_t);
    WindowSlot<Fn>(w, 6)(Pointer(w), value);
}
void ShowHintsCb(void* u, uint64_t w) {
    fc4::CoreLock::Unlocked unlocked(Self(u)->Lock());
    using Fn = void(__thiscall*)(void*);
    WindowSlot<Fn>(w, 7)(Pointer(w));
}
void CatalogCb(void* u, uint64_t w, uint64_t menu) {
    fc4::CoreLock::Unlocked unlocked(Self(u)->Lock());
    using Fn = void(__thiscall*)(void*, void*);
    WindowSlot<Fn>(w, 8)(Pointer(w), Pointer(menu));
}
bool LeftClickCb(void* u, uint64_t w, int32_t* x, int32_t* z, int32_t stage) {
    fc4::CoreLock::Unlocked unlocked(Self(u)->Lock());
    using Fn = bool(__thiscall*)(void*, int32_t*, int32_t*, int32_t);
    return WindowSlot<Fn>(w, 10)(Pointer(w), x, z, stage);
}
int32_t ResetWindowCb(void* u, uint64_t w, int32_t guid, int32_t index) {
    fc4::CoreLock::Unlocked unlocked(Self(u)->Lock());
    using Fn = int32_t(__thiscall*)(void*, uint32_t, int32_t);
    return WindowSlot<Fn>(w, 11)(Pointer(w), static_cast<uint32_t>(guid), index);
}
void EventCb(void* u, uint64_t w, uint32_t event) {
    fc4::CoreLock::Unlocked unlocked(Self(u)->Lock());
    using Fn = bool(__thiscall*)(void*, uint32_t);
    reinterpret_cast<Fn>(fc4::compat::TutorialNative()->event)(Pointer(w), event);
}
bool RegistrySetActiveCb(void* u, uint32_t guid, bool active) {
    fc4::CoreLock::Unlocked unlocked(Self(u)->Lock());
    const auto* native = fc4::compat::TutorialNative();
    cISC4App* app = App();
    bool changed = false;
    if (native != nullptr && app != nullptr) {
        void* registry = reinterpret_cast<void*>(app->GetTutorialRegistry()); // borrowed, already adjusted
        using Fn = bool(__thiscall*)(void*, uint32_t, uint32_t); // AL only, pop8
        if (registry) changed = reinterpret_cast<Fn>(native->setActive)(registry, guid, active ? 1u : 0u);
    }
    if (app) FC4_RELEASE(app);
    fc4::Log("tutorial registry: SetActive guid=0x%08X active=%u changed=%u", guid, active, changed);
    return changed;
}
bool RegistryEntryEnabledCb(void* u, uint32_t guid, bool* enabled) {
    fc4::CoreLock::Unlocked unlocked(Self(u)->Lock());
    const auto* native = fc4::compat::TutorialNative();
    cISC4App* app = App();
    bool found = false;
    *enabled = false;
    if (native != nullptr && app != nullptr) {
        void* registry = reinterpret_cast<void*>(app->GetTutorialRegistry());
        using Fn = const uint8_t*(__thiscall*)(void*, uint32_t);
        const uint8_t* item = registry ? reinterpret_cast<Fn>(native->getItem)(registry, guid) : nullptr;
        if (item != nullptr) { found = true; *enabled = item[0x10] != 0; }
    }
    if (app) FC4_RELEASE(app);
    fc4::Log("tutorial registry: GetItem guid=0x%08X found=%u active=%u", guid, found, *enabled);
    return found;
}

// TU7: quotes suppress only space, tab and LF delimiters. No escape branch.
std::vector<std::string> TutorialTokens(const char* line) {
    std::vector<std::string> tokens;
    const char* start = nullptr;
    bool quoted = false;
    for (const char* p = line;; ++p) {
        const char c = *p;
        if (c == '"') quoted = !quoted;
        const bool delimiter = !quoted && (c == ' ' || c == '\t' || c == '\n');
        if (c == 0 || delimiter) {
            if (start != nullptr) {
                std::string token(start, p);
                if (token.size() >= 2 && token.front() == '"' && token.back() == '"')
                    token = token.substr(1, token.size() - 2);
                tokens.push_back(std::move(token));
                start = nullptr;
            }
            if (c == 0) return tokens;
        } else if (start == nullptr) start = p;
    }
}
uint32_t TutorialNumber(const std::string& token) {
    int base = 10;
    if (token.size() >= 2 && token[0] == '0' && (token[1] == 'x' || token[1] == 'X')) base = 16;
    else for (unsigned char c : token) {
        if ((c >= 'a' && c <= 'f') || (c >= 'A' && c <= 'F')) { base = 16; break; }
        if (c < '0' || c > '9') break;
    }
    return static_cast<uint32_t>(std::strtoul(token.c_str(), nullptr, base));
}
