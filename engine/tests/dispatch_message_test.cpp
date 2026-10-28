// SPDX-License-Identifier: GPL-3.0-or-later
#include "FC4DispatchMessage.h"
#include <cstdio>

struct Message {
    uintptr_t data1 = 0x12345678;
    uintptr_t unknown = 0x87654321;
    unsigned dataReads = 0, unknownReads = 0;
    intptr_t GetData1() { ++dataReads; return static_cast<intptr_t>(data1); }
    void* GetIGZUnknown() { ++unknownReads; return reinterpret_cast<void*>(unknown); }
};

int main() {
    Message vehicle;
    if (fc4::DispatchMessageObject(0x09D92FC3, vehicle) != vehicle.data1 ||
        vehicle.dataReads != 1 || vehicle.unknownReads != 0) return 1;
    Message removal;
    if (fc4::DispatchMessageObject(0x99EF1143, removal) != removal.data1 ||
        removal.dataReads != 1 || removal.unknownReads != 0) return 2;
    Message empty;
    empty.data1 = 0;  // An unrelated IGZUnknown must not turn a null event into a vehicle.
    if (fc4::DispatchMessageObject(0x09D92FC3, empty) != 0 || empty.unknownReads != 0) return 3;
    Message post;
    if (fc4::DispatchMessageObject(0xE94671EB, post) != post.unknown ||
        post.dataReads != 0 || post.unknownReads != 1) return 4;
    std::puts("Dispatch incoming object channel and null-event contracts passed");
    return 0;
}
