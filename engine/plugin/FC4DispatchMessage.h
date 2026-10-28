// SPDX-License-Identifier: GPL-3.0-or-later
#pragma once
#include <cstdint>

namespace fc4 {

// dispatch.md 3: removal and direct vehicle events carry their object in
// Data1. The IGZUnknown field is used by the manager's outgoing posts instead.
template<class Message>
uint64_t DispatchMessageObject(uint32_t type, Message& message) {
    if (type == 0x99EF1143 || type == 0x09D92FC3) {
        return static_cast<uintptr_t>(message.GetData1());
    }
    return reinterpret_cast<uintptr_t>(message.GetIGZUnknown());
}

}  // namespace fc4
