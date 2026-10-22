// SPDX-License-Identifier: GPL-3.0-or-later
#pragma once
#include "cIGZUnknown.h"

namespace fc4::subjectverify {
// advisors.md 12.4: SDK corrections, with exact integer setter returns and
// the missing slot 13. No private object layout or constructor is used.
class Native : public cIGZUnknown {
public:
    virtual bool SetLocation(float x, float z) = 0;
    virtual float GetLocationX() = 0;
    virtual float GetLocationZ() = 0;
    virtual bool HasLocation() = 0;
    virtual bool SetObject(cIGZUnknown* object) = 0;
    virtual cIGZUnknown* GetObjectA() = 0;
    virtual uint32_t GetData1() = 0;
    virtual uint32_t SetData1(uint32_t value) = 0;
    virtual uint32_t GetData2() = 0;
    virtual uint32_t SetData2(uint32_t value) = 0;
    virtual bool Clone(Native** out) = 0;
};
}
