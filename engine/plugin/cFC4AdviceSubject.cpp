// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.
//
// cSC4AdviceSubject over the core subject (advisors.md §2.5, §11.4, §12.4).

#include "cFC4AdviceSubject.h"

#include "FC4Log.h"
#include "fc4.h"

// Private transport; deliberately outside the shared public ABI tables.
extern "C" fc4_adv_obj* fc4_private_adv_subject_clone(const fc4_adv_obj* source);
extern "C" bool fc4_private_adv_subject_set_location(const fc4_adv_obj*, float, float);
extern "C" bool fc4_private_adv_subject_set_object(const fc4_adv_obj*, uint64_t);
extern "C" bool fc4_private_adv_subject_set_data1(const fc4_adv_obj*, uint32_t);
extern "C" bool fc4_private_adv_subject_set_data2(const fc4_adv_obj*, uint32_t);

cFC4AdviceSubject::cFC4AdviceSubject(fc4_adv_obj* handle, NewSubjectFn, void*)
    : fc4::AdvFront(handle) {
    Register(Token());
}

bool cFC4AdviceSubject::QueryInterface(uint32_t riid, void** ppvObj) {
    if (ppvObj == nullptr) return false;
    *ppvObj = nullptr;
    if (riid == kFC4_IID_cISC4AdviceSubject || riid == 1 || riid == kFC4_CLSID_cSC4AdviceSubject) {
        *ppvObj = static_cast<cFC4ISC4AdviceSubject*>(this);
    } else if (riid == GZIID_cIGZSerializable) {
        *ppvObj = static_cast<cIGZSerializable*>(this);
    } else if (riid == kFC4_IID_CoreObject) {
        *ppvObj = static_cast<cIFC4CoreObject*>(this);
    } else {
        return false;
    }
    AddRef();
    return true;
}

uint32_t cFC4AdviceSubject::AddRef() { return AddRefFront(); }

// May delete this (FC4AdvLife.h).
uint32_t cFC4AdviceSubject::Release() { return ReleaseFront(); }

bool cFC4AdviceSubject::Get(fc4_adv_subject_state* out) {
    *out = fc4_adv_subject_state{};
    Pin h(*this);
    return h && fc4_adv_subject_get(h.get(), out);
}

bool cFC4AdviceSubject::SetLocation(float x, float z) {
    Pin h(*this);
    return h && fc4_private_adv_subject_set_location(h.get(), x, z);
}

float cFC4AdviceSubject::GetLocationX() {
    fc4_adv_subject_state st{};
    return Get(&st) && st.has_location ? st.x : 0.0f;
}

float cFC4AdviceSubject::GetLocationZ() {
    fc4_adv_subject_state st{};
    return Get(&st) && st.has_location ? st.z : 0.0f;
}

bool cFC4AdviceSubject::HasLocation() {
    fc4_adv_subject_state st{};
    return Get(&st) && st.has_location != 0;
}

bool cFC4AdviceSubject::SetObject(cIGZUnknown* object) {
    Pin h(*this);
    return h && fc4_private_adv_subject_set_object(h.get(), object != nullptr ? fc4::TokenOf(object) : 0);
}

cIGZUnknown* cFC4AdviceSubject::GetObjectA() {
    fc4_adv_subject_state st{};
    return Get(&st) ? fc4::FromToken(st.object) : nullptr;
}

uint32_t cFC4AdviceSubject::GetData1() {
    fc4_adv_subject_state st{};
    return Get(&st) ? st.data1 : 0;
}

bool cFC4AdviceSubject::SetData1(uint32_t v) {
    Pin h(*this);
    return h && fc4_private_adv_subject_set_data1(h.get(), v);
}

uint32_t cFC4AdviceSubject::GetData2() {
    fc4_adv_subject_state st{};
    return Get(&st) ? st.data2 : 0;
}

bool cFC4AdviceSubject::SetData2(uint32_t v) {
    Pin h(*this);
    return h && fc4_private_adv_subject_set_data2(h.get(), v);
}

// Slot 13: a new subject with the same fields, the object held again, AddRef'd
// for the caller.
bool cFC4AdviceSubject::Clone(cFC4ISC4AdviceSubject** out) {
    if (out == nullptr) return false;
    *out = nullptr;
    Pin source(*this);
    if (!source) return false;
    fc4_adv_obj* h = fc4_private_adv_subject_clone(source.get());
    if (h == nullptr) return false;
    const uint64_t token = fc4_adv_obj_token(h);
    auto* c = token != 0 ? static_cast<cFC4ISC4AdviceSubject*>(fc4::FromToken(token)) : nullptr;
    if (c != nullptr) c->AddRef();  // before h goes: h may be the copy's only strong handle
    fc4_adv_obj_destroy(h);
    if (c == nullptr) return false;
    *out = c;
    return true;
}

bool cFC4AdviceSubject::Write(cIGZOStream& out) {
    fc4::OStreamAdapter a(out);
    Pin h(*this);
    return h && fc4_adv_obj_write(h.get(), a.Get());
}

bool cFC4AdviceSubject::Read(cIGZIStream& in) {
    fc4::IStreamAdapter a(in);
    Pin h(*this);
    return h && fc4_adv_obj_read(h.get(), a.Get());
}
