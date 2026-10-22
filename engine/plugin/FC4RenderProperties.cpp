// SPDX-License-Identifier: GPL-3.0-or-later
// Behavior source: render-properties §§1-3, 7. Original remains installed (RP7).
#include "FC4RenderProperties.h"
#include "FC4Log.h"
#include "FC4RefAudit.h"
#include "FC4Subsystems.h"
#include "compat/FC4Compat.h"
#include "fc4.h"
#include "GZServPtrs.h"
#include "cIGZApp.h"
#include "cIGZFrameWork.h"
#include "cISC4App.h"
#include "cISC4RenderProperties.h"
#include <algorithm>
#include <array>
#include <cstring>
#include <string>
#include <vector>

namespace fc4 {
namespace {
using Bytes = std::vector<uint8_t>;
cISC4RenderProperties* original = nullptr;
fc4_renderprops* ours = nullptr;
compat::RenderTextPrimitive upper = nullptr, lower = nullptr;
unsigned rounds = 0;

// Independent mixed-case spellings from spec §3, not read from either owner.
const char* const boolNames[] = {
    "ForceSoftwareDriver", "RenderDynamicView", "RenderStaticView", "RenderWaterEffects",
    "RenderCloudsAndFog", "ForcePointSampling", "ForceBilinear", "ForceNoAlphaBlend",
    "ForceNoAlphaTest", "RenderShadows", "RenderDecals", "UseSecondStage", "ForceDebugTexture",
    "RenderParticleEffects", "RenderFloraShadows", "RenderTerrain", "ShowUpdateRegions",
    "UseCursorEffects", "DrawLotTextures", "DrawLotFoundations", "DrawLotWalls", "ForceTextureRepeat",
    "UseScreenShake", "RenderUI", "UseUpdateRectClipping", "UseVertexBuffers", "ShowBoundingBoxes",
    "UseOverlayCulling", "RenderSnapshot", "NoAddSigned", "NoPartialBackingStoreCopies",
    "UseScreenFlash", "RenderAutomata", "PreferOpaque", "DisplayAlertBorders", "DrawLotOpaqueTextures",
    "UseViewCleanup", "DoAutoZoomAndRotationChanges"};
const char* const intNames[] = {
    "ViewOffsetY", "ViewOffsetX", "TextureBits", "LODOffset", "ShadowDebug", "MaxParticlesTarget",
    "ParticleLODOffset", "EffectPriorityLevel", "BackingStoreMaxZoomAll", "BackingStoreMaxZoomBuildingsProps",
    "BackingStoreMaxZoomTerrain", "BackingStoreMaxZoomFlora", "ShadowQuality", "MinVehicleZoom", "MinPedZoom",
    "CursorType", "DepthOffset", "DebugTexture", "DirtyRectMergeFrames", "SoundDetail", "BuildingVariety",
    "BuildingLoaderSpeed", "ModelLODOffset", "StringTextureCacheSize"};
const char* const floatNames[] = {
    "ParticleEffectsLOD", "SplatTextureScale", "TexBindMaxFree", "ParticleDensity", "ParticleDamping",
    "ParticleScale", "LightingUpdateDelta", "ShadowModelThreshold", "DemolishModelThreshold",
    "RenderModelThreshold", "RenderModelThresholdScroll", "ParticleSizeThreshold", "MaxVehiclePct",
    "MaxPedPct", "OpaqueUIAlphaValue"};
const char* const* const source[] = {boolNames, intNames, floatNames};
constexpr int counts[] = {38, 24, 15};

int NativeCount(unsigned k) {
    return k == 0 ? original->NumBoolProperties() : k == 1 ? original->NumIntProperties() : original->NumFloatProperties();
}
const char* NativeName(unsigned k, int i) {
    return k == 0 ? original->NameFromBoolPropertyID(i) : k == 1 ? original->NameFromIntPropertyID(i) : original->NameFromFloatPropertyID(i);
}
int NativeFind(unsigned k, const char* s) {
    return k == 0 ? original->BoolPropertyIDFromName(s) : k == 1 ? original->IntPropertyIDFromName(s) : original->FloatPropertyIDFromName(s);
}
uint32_t NativeValue(unsigned k, int i) {
    if (k == 0) return original->BoolValue(i) ? 1u : 0u;
    if (k == 1) return static_cast<uint32_t>(original->IntValue(i));
    float v = original->FloatValue(i);
    uint32_t bits;
    std::memcpy(&bits, &v, 4);
    return bits;
}
void NativeSet(unsigned k, int i, uint32_t bits) {
    if (k == 0) original->SetBoolValue(i, bits != 0);
    else if (k == 1) original->SetIntValue(i, static_cast<int32_t>(bits));
    else {
        float v;
        std::memcpy(&v, &bits, 4);
        original->SetFloatValue(i, v);
    }
}
Bytes Fold(const char* s, bool abbreviation) {
    Bytes result;
    for (; *s; ++s) {
        const int32_t b = static_cast<int8_t>(*s);
        if (!abbreviation || upper(b) != 0) result.push_back(static_cast<uint8_t>(lower(b)));
    }
    return result;
}
bool Enabled() {
    size_t n;
    const Subsystem* list = Subsystems(&n);
    for (size_t i = 0; i < n; ++i)
        if (std::strcmp(list[i].key, "renderprops") == 0) return SubsystemEnabled(list[i]);
    return false;
}

void Interfaces() {
    // QueryInterface(1) gives U, not P. Only release references actually acquired.
    const uint32_t a = FC4_ADDREF(original), b = FC4_ADDREF(original);
    const uint32_t c = FC4_RELEASE(original), d = FC4_RELEASE(original);
    void* unknown = nullptr;
    const bool ok = original->QueryInterface(1, &unknown);
    bool views = false, unsupported = false;
    if (ok && unknown) {
        auto* u = static_cast<cIGZUnknown*>(unknown);
        FC4_ACQUIRED(u, "QueryInterface");
        void* again = nullptr;
        if (u->QueryInterface(1, &again) && again) {
            auto* u2 = static_cast<cIGZUnknown*>(again);
            FC4_ACQUIRED(u2, "QueryInterface");
            views = again == unknown && reinterpret_cast<uintptr_t>(unknown) == reinterpret_cast<uintptr_t>(original) + 4;
            FC4_RELEASE(u2);
        }
        void* sentinel = reinterpret_cast<void*>(uintptr_t(0x1234));
        unsupported = !u->QueryInterface(0x26CE01C0, &sentinel) && sentinel == reinterpret_cast<void*>(uintptr_t(0x1234));
        FC4_RELEASE(u);
    }
    void* sentinel = reinterpret_cast<void*>(uintptr_t(0x4321));
    const bool pReject = !original->QueryInterface(0x26CE01C0, &sentinel) && sentinel == reinterpret_cast<void*>(uintptr_t(0x4321));
    Log("renderprops: interfaces balanced=%u P_U=%u unsupported_untouched=%u native_delete_test=unavailable_RP7",
        b == a + 1 && c == a && d + 1 == a, views, unsupported && pReject);
}
} // namespace

void RenderPropertiesCheck(const char* phase, bool setters) {
    if (!original || !ours) return;
    unsigned nameTests = 0, nameDiff = 0, findTests = 0, findDiff = 0, oracleDiff = 0;
    unsigned valueTests = 0, valueDiff = 0, setTests = 0, setDiff = 0, restoreDiff = 0;
    for (unsigned k = 0; k < 3; ++k) {
        const int nativeCount = NativeCount(k), rustCount = fc4_renderprops_count(ours, k);
        if (nativeCount != counts[k] || rustCount != counts[k]) {
            Log("renderprops: shape unavailable kind=%u native=%d rust=%d expected=%d", k, nativeCount, rustCount, counts[k]);
            return; // Never perform an unchecked native indexed access outside its table.
        }
        std::vector<Bytes> full, shortNames;
        for (int i = 0; i < counts[k]; ++i) {
            const char* n = NativeName(k, i);
            if (!n) return;
            full.emplace_back(n, n + std::strlen(n)); // Copy before any owner release.
            shortNames.push_back(Fold(source[k][i], true));
            uint8_t buffer[128]{};
            int len = fc4_renderprops_name(ours, k, i, buffer, sizeof(buffer));
            const Bytes expected = Fold(source[k][i], false);
            ++nameTests;
            if (len < 0 || Bytes(buffer, buffer + std::max(0, len)) != full.back() || full.back() != expected) ++nameDiff;
            uint32_t rust = 0, native = NativeValue(k, i);
            ++valueTests;
            if (!fc4_renderprops_get(ours, k, i, &rust) || rust != native) {
                ++valueDiff;
                Log("renderprops: live-default difference phase=%s kind=%u index=%d native=0x%08X rust=0x%08X", phase, k, i, native, rust);
            }
        }
        auto query = [&](Bytes bytes) {
            // Counted copy includes arbitrary bytes after first NUL; append readable terminator for native.
            bytes.push_back(0);
            Bytes folded;
            for (uint8_t b : bytes) {
                if (!b) break;
                folded.push_back(static_cast<uint8_t>(lower(static_cast<int8_t>(b))));
            }
            int expected = -1;
            for (int i = 0; i < counts[k]; ++i) {
                if (folded == full[i] || folded == shortNames[i]) { expected = i; break; }
            }
            const int native = NativeFind(k, reinterpret_cast<const char*>(bytes.data()));
            const int rust = fc4_renderprops_find(ours, k, bytes.data(), bytes.size());
            ++findTests;
            if (native != rust) ++findDiff;
            if (native != expected) ++oracleDiff;
        };
        // Reverse registration order distinguishes lookup order from query order.
        for (int i = counts[k] - 1; i >= 0; --i) {
            const char* s = source[k][i];
            Bytes mixed(s, s + std::strlen(s));
            query(mixed);
            query(shortNames[i]);
            Bytes nul = mixed;
            nul.insert(nul.end(), {0, 0x80, 0xd0, 0xff});
            query(nul);
            mixed.insert(mixed.begin(), 'k');
            query(mixed);
        }
        query({}); query({0, 'R', 'S'}); query({'R', 'S', ' '});
        query({'R', 'S', 0, 0x80, 0xd0, 0xff}); query({'R', 'S', 0xff});
        query({0xc4, 0xb0}); query({'I'}); query({'i'});
        for (unsigned b = 0x80; b <= 0xff; ++b) {
            query({static_cast<uint8_t>(b)});
            query({'R', 'S', static_cast<uint8_t>(b)});
        }
        if (setters) {
            for (int i = 0; i < counts[k]; ++i) {
                const uint32_t savedNative = NativeValue(k, i);
                uint32_t savedRust = 0;
                fc4_renderprops_get(ours, k, i, &savedRust);
                // Independently supplied inputs; never seed the Rust owner from native values.
                const uint32_t values[] = {k == 0 ? 0u : k == 1 ? 0x80000000u : 0x80000000u,
                                           k == 0 ? 1u : k == 1 ? 0x7fffffffu : 0xc2150000u};
                for (uint32_t input : values) {
                    NativeSet(k, i, input);
                    uint32_t output = 0;
                    const bool set = fc4_renderprops_set(ours, k, i, input) != 0;
                    ++setTests;
                    if (!set || !fc4_renderprops_get(ours, k, i, &output) || output != input || NativeValue(k, i) != input) ++setDiff;
                }
                NativeSet(k, i, savedNative);
                fc4_renderprops_set(ours, k, i, savedRust);
                uint32_t restored = 0;
                fc4_renderprops_get(ours, k, i, &restored);
                if (NativeValue(k, i) != savedNative || restored != savedRust) ++restoreDiff;
            }
        }
    }
    ++rounds;
    Log("renderprops: phase=%s names=%u diff=%u finds=%u diff=%u oracle_diff=%u live_defaults=%u diff=%u setters=%u diff=%u restore_diff=%u resync=0 replacement=0",
        phase, nameTests, nameDiff, findTests, findDiff, oracleDiff, valueTests, valueDiff, setTests, setDiff, restoreDiff);
}

void RenderPropertiesStart() {
    if (!Enabled() || original || ours) return;
    if (!compat::RenderTextPrimitives(&upper, &lower)) { Log("renderprops: unavailable native text binding"); return; }
    cIGZFrameWork* fw = RZGetFrameWork();
    cIGZApp* app = fw ? fw->Application() : nullptr;
    cISC4App* sc4 = nullptr;
    if (!app || !app->QueryInterface(GZIID_cISC4App, reinterpret_cast<void**>(&sc4)) || !sc4) {
        Log("renderprops: unavailable application binding"); return;
    }
    FC4_ACQUIRED(sc4, "QueryInterface");
    original = sc4->GetRenderProperties(); // Borrowed: retain before releasing the app interface.
    if (original) FC4_ADDREF(original);
    FC4_RELEASE(sc4);
    if (!original) { Log("renderprops: unavailable properties binding"); return; }
    ours = fc4_renderprops_create(upper, lower);
    if (!ours) { FC4_RELEASE(original); original = nullptr; return; }
    Log("renderprops: native-text bound; independent Rust owner; verification only RP7; locale=uncontrolled RP6");
    for (int x : {0x49, 0x69, -128, -48, -1})
        Log("renderprops: scalar signed=%d upper=%d lower=%d low=%u", x, upper(x), lower(x), static_cast<uint8_t>(lower(x)));
    Interfaces();
    RenderPropertiesCheck("post-app", false);
}

void RenderPropertiesStop() {
    if (!original && !ours) return;
    RenderPropertiesCheck("pre-shutdown", false);
    fc4_renderprops_destroy(ours);
    ours = nullptr;
    if (original) FC4_RELEASE(original);
    original = nullptr;
    upper = lower = nullptr; // Borrowed executable callbacks: never free them.
    Log("renderprops: shutdown rounds=%u retained_reference_released=1 rust_destroyed=1 replacement=0", rounds);
}
} // namespace fc4
