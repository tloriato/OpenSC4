// SPDX-License-Identifier: GPL-3.0-or-later
#pragma once

#include <cstdint>

namespace fc4 {

// Coverage accounting, not simulation behaviour. A successful observation is
// never an independent operation match unless all of that operation's inputs
// were captured. In particular, counter equality and record bootstrap do not
// establish a captured random or callback transcript (PV14/PV15).
struct PropDevEvidence {
    uint64_t nativeCalls = 0, compared = 0, equal = 0, refused = 0;
    uint64_t failures = 0, bootstraps = 0, resyncs = 0, fallbacks = 0;
    uint64_t returnCompared = 0, returnEqual = 0;
    uint64_t recordAttempts = 0, recordCompared = 0, recordEqual = 0;
    uint64_t queryAttempts = 0, queryCompared = 0, queryEqual = 0;
    uint64_t diagnosticMismatches = 0, secondaryMessages = 0;

    void ObserveCall(bool replayed, bool nativeResult, bool replayResult, bool inputsCaptured) {
        ++nativeCalls;
        if (replayed) {
            ++returnCompared;
            if (nativeResult == replayResult) ++returnEqual;
            else ++diagnosticMismatches;
        }
        if (!replayed || !inputsCaptured) {
            ++refused;
            return;
        }
        ++compared;
        if (nativeResult == replayResult) ++equal;
    }
};

}  // namespace fc4
