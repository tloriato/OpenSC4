// SPDX-License-Identifier: GPL-3.0-or-later
#include "FC4DispatchPylon.h"
#include <cstdio>

namespace {
struct Native {
    void** table;
    uint32_t slot = 0, argument = 0;
    void* pointer = nullptr;
    cS3DVector3 position;
};

bool __fastcall Shutdown(Native* n, void*) { n->slot = 5; return true; }
void __fastcall UI(Native* n, void*, uint32_t v) { n->slot = 6; n->argument = v; }
void __fastcall Type(Native* n, void*, uint32_t v) { n->slot = 9; n->argument = v; }
uint32_t __fastcall GetType(Native* n, void*) { n->slot = 10; return 6; }
void __fastcall State(Native* n, void*, uint32_t v) { n->slot = 11; n->argument = v; }
void __fastcall Number(Native* n, void*, uint32_t v) { n->slot = 13; n->argument = v; }
uint32_t __fastcall GetNumber(Native* n, void*) { n->slot = 14; return 0xCAFE1234; }
void __fastcall Source(Native* n, void*, void* p) { n->slot = 17; n->pointer = p; }
void* __fastcall GetSource(Native* n, void*) { n->slot = 18; return n->pointer; }
void* __fastcall Occupant(Native* n, void*) { n->slot = 3; return n->pointer; }
bool __fastcall Position(Native* n, void*, const cS3DVector3* p) {
    n->slot = 9; n->position = *p; return true;
}
}

#define CHECK(x) do { if (!(x)) { std::printf("line %d: %s\n", __LINE__, #x); return 1; } } while (0)

int main() {
    // Independent native slot table: unused slots stay null, so shifted SDK
    // numbering cannot silently agree with a fixture deriving from our prefix.
    void* table[21]{};
    table[3] = reinterpret_cast<void*>(&Occupant);
    table[5] = reinterpret_cast<void*>(&Shutdown);
    table[6] = reinterpret_cast<void*>(&UI);
    table[9] = reinterpret_cast<void*>(&Type);
    table[10] = reinterpret_cast<void*>(&GetType);
    table[11] = reinterpret_cast<void*>(&State);
    table[13] = reinterpret_cast<void*>(&Number);
    table[14] = reinterpret_cast<void*>(&GetNumber);
    table[17] = reinterpret_cast<void*>(&Source);
    table[18] = reinterpret_cast<void*>(&GetSource);
    Native p{table};
    const uint64_t token = reinterpret_cast<uintptr_t>(&p);
    fc4_dispatch_host h{};
    fc4::WireDispatchPylon(h);
    h.pylon_shutdown(nullptr, token); CHECK(p.slot == 5);
    h.pylon_set_ui_state(nullptr, token, 2); CHECK(p.slot == 6 && p.argument == 2);
    h.pylon_set_dispatch_type(nullptr, token, 1); CHECK(p.slot == 9 && p.argument == 1);
    CHECK(h.pylon_dispatch_type(nullptr, token) == 6 && p.slot == 10);
    h.pylon_set_dispatch_state(nullptr, token, 5); CHECK(p.slot == 11 && p.argument == 5);
    h.pylon_set_call_number(nullptr, token, 0xABCD9876); CHECK(p.slot == 13 && p.argument == 0x9876);
    CHECK(h.pylon_call_number(nullptr, token) == 0x1234 && p.slot == 14);
    h.pylon_set_source(nullptr, token, 0x12345678); CHECK(p.slot == 17);
    CHECK(h.pylon_source(nullptr, token) == 0x12345678 && p.slot == 18);
    void* occupantTable[10]{};
    occupantTable[9] = reinterpret_cast<void*>(&Position);
    Native occupant{occupantTable};
    p.pointer = &occupant;
    h.pylon_move_to(nullptr, token, 1.0f, 2.0f, 3.0f);
    CHECK(p.slot == 3 && occupant.slot == 9);
    CHECK(occupant.position.fX == 1.0f && occupant.position.fY == 2.0f && occupant.position.fZ == 3.0f);
    CHECK(h.pylon_source(nullptr, 0) == 0 && h.pylon_call_number(nullptr, 0) == 0);
    h.pylon_shutdown(nullptr, 0);
    std::puts("Dispatch pylon native slot, borrowed pointer and low-word contracts passed");
    return 0;
}
