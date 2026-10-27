# The cheat code manager (`cGZCheatCodeManager`) (behaviour spec)

Scope: the `cGZCheatCodeManager` owner as tools/coverage.py assigns it on `bb641878` (44 functions, 3,811 bytes;
inventory in §6). The manager holds the registered cheat strings and their IDs, matches typed text against them,
notifies subscribers, handles the "enable"/"disable" password lock, keeps per-cheat on/off states and a history, and
packs that state for saving. Its callers: the cheat dialog (cheat-dialog.md), the command line (application.md) and
the subsystems that react to cheat messages (advisors.md, flora.md, …).

Related:
- cheat-dialog.md: running a typed cheat (slot 22); the manager's global `0x00B43CAC`;
- advisors.md (AD9): the cheat message `0x230E27AC` as subscribers receive it;
- dates.md §2: `cRZDate()` is today (its `+4`, the day number, is what the history stores).

Evidence: static RE only, on the instruction and data bytes of the Steam 1.1.641 exe (the same code as GOG,
`docs/lanes/re-dates.md`). Every function was read in the decompile; the enable/disable notices, the password setter,
`DoDefaultCheatCodeProcessing`'s exits and both save-data paths were checked on the instructions. The vtable
`0x00ACFCD0` (28 slots) and the strings `0x00ACFC08` (`"enable"`) and `0x00ACFC00` (`"disable"`) were read. Written
2026-10-24 (re-gzcheats7).

Conventions: "slot n" is a `cIGZCheatCodeManager` slot. "History record" is the 16-byte `{id, day, action, 0}` with
`day` = today's day number and action 0 = entered, 1 = state on, 2 = state off; records are only added while history
is enabled (`+0x24`).

## 1. Identity and fields *[compat]*

- System service `0x69D9E74B` (priority `0x1312D0`) at `+0x04`; a COM base table at `+0x08` (slot 3 the deleting
  destructor, thunk `0x00921A63`). Constructor `0x00921970`; destructor body `0x00921A6B`, deleting form
  `0x00921B02`.
- **`QueryInterface`** (`0x00920C0A`): IIDs `0xA1085722` and 1 → itself, AddRef'd; else the system service's (thunks
  `0x00921A53`, `0x00921A5B`). AddRef/Release (shared) count at `+0x20`; Release at 1 or below deletes.

| Offset | Meaning | Constructor value |
|---|---|---|
| `+0x24` | history enabled (byte) | 1 |
| `+0x28` | registered cheats: list of `{string, id, save state}` (0x24-byte nodes: string `+8`, id `+0x1C`, byte `+0x20`) | empty |
| `+0x2C` | notification targets (`cIGZMessageTarget`, AddRef'd), as `{target, id}` | empty |
| `+0x30` | notification targets (`cIGZMessageTarget2`), as `{target, id}` | empty |
| `+0x34`, `+0x38` | the IDs whose state is on (a set) and its count | empty |
| `+0x40`…`+0x48` | the history (vector of records) | empty |
| `+0x4C`, `+0x50` | the stored password (compressed, heap block) and its size | null, 0 |
| `+0x54` | cheats enabled (byte) | **1** |
| `+0x58`, `+0x6C` | the enable and disable words (`cRZString`, lower case) | `"enable"`, `"disable"` |

## 2. The slots *[compat]*

| Slot | Address | Behaviour |
|---|---|---|
| 3 `RegisterCheatCode(id, s)` | `0x00921B38` | false when a registered string already equals `s` (string slot 10 `(…, 1)`); else append `{s, id, save state 1}`; true |
| 4 `UnregisterCheatCode(id)` | `0x00921817` | remove the first entry with `id`; whether one was found |
| 5 / 6 `Add/RemoveNotification(target, id)` | `0x0092124C` / `0x00920FA0` | list `+0x2C`: add the pair when absent (AddRef) / remove it (Release); whether anything changed |
| 7 / 8 `Add/RemoveNotification2(target, id)` | `0x009212A6` / `0x00921000` | the same for list `+0x30` |
| 9 `EnableCheatCodeHistory(on)` | `0x00920C35` | `+0x24` |
| 10 `ClearCheatCodeHistory()` | `0x00921310` | |
| 11 `GetCheatCodeSaveData(out, out size)` | `0x00921060` | §4 |
| 12 `SetCheatCodeSaveData(data, size)` | `0x009214DB` | §4 |
| 13 `GetListOfAllCheatStrings(out, in/out n)` | `0x009211C3` | `n` = 0 → returns the number of registered cheats (`n` stays 0); else up to `n` string pointers, `n :=` the number written, returned |
| 14 `HasUserUsedCheat(id)` | `0x00920E7E` | the number of "entered" history records for `id` |
| 15 `GetCheatState(id)` | `0x00921215` | whether `id` is in the set |
| 16 `GetCheatsWithEnabledStates(out, in/out n)` | `0x00921150` | null `out` → `n :=` the count; else up to `n` IDs; returns `n` |
| 17 `SetCheatState(id, on)` | `0x009215D7` | add to / remove from the set; history record with action 1 / 2 |
| 18 `SetStateShouldBeSaved(id, b)` | `0x00920EA8` | the entry's save-state byte; false when not registered |
| 19 `ClearAllEnabledCheatStates()` | `0x00921300` | empty the set |
| 20 `SendCheatNotifications(text, id)` | `0x00920ECE` | §3 |
| 21 `DoesCheatCodeMatch(text, out id)` | `0x0092184B` | §3 |
| 22 `DoDefaultCheatCodeProcessing(text, out id)` | `0x00921621` | §3 |
| 23 `EnableCheatCodes(on, password)` | `0x0092131E` | §3.1 |
| 24 `GetCheatCodesEnabled()` | `0x00920C3F` | `+0x54` |
| 25 `GetCheatCodePassword(out, in/out size)` | `0x00920D52` | none → `size := 0`. Else the stored block's length (its first dword): a buffer too small → `size :=` length, false; else copy, `size :=` length. True |
| 26 `SetCheatCodePassword(data, size)` | `0x00920DA1` | free the old one; with `data`, copy `size` bytes; when the block's first dword ≠ `size`, discard and false; else **cheats enabled := 1**. True (also for null `data`, which only clears). `+0x50` is not updated |
| 27 `SetEnableDisableStrings(enable, disable)` | `0x00920C43` | assign both, lower-cased; true |

## 3. Running cheats *[compat]*

**`DoesCheatCodeMatch(text, out id)`**: `id := 0`. Only when cheats are enabled and `text` is not empty: the
lower-cased text is compared with each registered string (lower-cased) in registration order; the first one the text
**starts with** wins: `id :=` its ID, result 2 when the lengths are equal (exact) or 1 (text longer, e.g. with
arguments). Otherwise 0.

**`DoDefaultCheatCodeProcessing(text, out id)`**: `id := 0`.
1. Cheats enabled and a match → `SendCheatNotifications(text, id)`, an "entered" history record; true.
2. Otherwise, when the enable and disable words are both empty, false. The trimmed, lower-cased text starting with the
   enable word → `EnableCheatCodes(1, rest)`; with the disable word → `EnableCheatCodes(0, rest)` (`rest` = the text
   after the word, left-trimmed); true either way, whatever `EnableCheatCodes` returns. Neither → false.

**`SendCheatNotifications(text, id)`**: nothing while cheats are disabled. Every `+0x2C` target registered for `id` or
for 0 gets `DoMessage({0x230E27AC, id, text, 0})`; every `+0x30` target for `id` or 0 gets a new standard message-2
with type `0x230E27AC`, data1 `id` and void2 `text` (released after the call). The result carries no status.

### 3.1 `EnableCheatCodes(on, password)` (slot 23)

A password longer than 64 characters → false.
- **Disable** (`on` false) while enabled: a non-empty password is compressed (`cRZFastCompression2::CompressData`,
  256-byte buffer; failure → false) and stored with its size. Then `SendCheatNotifications("", 0x69D9A743)` and
  enabled := 0; true.
- **Enable** (`on` true) while disabled: with a stored password, its decompressed length must equal the given one
  (else false); when it is at most 64 the block is decompressed and compared byte for byte (mismatch → false; a failed
  decompression skips the comparison). The stored password is freed. Enabled := 1, `SendCheatNotifications("",
  0x030E27B7)`; true.
- Anything else (disable while disabled, enable while enabled) → false.

So "disable <word>" locks the cheats behind `<word>` and "enable <word>" unlocks them; without a word the lock is
free.

## 4. Save data *[compat]*

**`GetCheatCodeSaveData(out, out size)`**: `size := 0x14 + p + 16·h + 4·e`, with `p` = the password size rounded up to
a multiple of 4, `h` the history records and `e` the IDs that are on. With `out`: `[size, p, password size,
password (zero-padded to p), 16·h, h records, 4·e, e IDs]` (dwords).

**`SetCheatCodeSaveData(data, size)`**: only with `data` and `size` > 3. The password is replaced by a copy of the
stored bytes; the history records are **appended** (`16·h / 16` of them); then the next dword is read as the number of
IDs, but it holds `4·e`, so `4·e` dwords are inserted into the set: the `e` saved IDs and `3·e` dwords from beyond
the block *[quirk]* (CCM1). Nothing checks `size` beyond the first test, the password size `+0x50` is not updated and
the enabled flag is left as it is.

*[compat]* The city owns persistence of this block. `cSC4City::Write` queries its size, fills
an independent byte buffer and writes that buffer as a version-1 record with type
`0x89D9B9EE`, group `0x69D9B9F1`, instance 0 (`0x0045C5B0`, calls at `0x0045CA66` and
`0x0045CA91`). `cSC4City::Read` obtains that record's length, fills a separate byte buffer,
passes it with its length to the manager, then frees it (`0x0045CB10`,
`0x0045D184`–`0x0045D1BA`). The saved IDs end the payload (`0x00921060`); there is no
following city field within that buffer. The excess reads therefore reach beyond the
payload, not a specified following record. Their actual values depend on allocation contents
and remain unresolved (CCM1).

*[compat-limit]* A bounded replacement should consume only the state IDs represented by
that byte count and available in the payload, rather than reproducing the out-of-bounds reads.

## 5. Quirks and cases

Quirks:
1. `SetCheatCodeSaveData` reads four times too many state IDs (§4, CCM1).
2. `SetCheatCodePassword` re-enables the cheats (§2).
3. `GetListOfAllCheatStrings` with `n` = 0 returns the count but leaves `n` at 0.
4. A registered cheat matches any text that starts with it, so `"moolah"` would also match `"moolahs"` (result 1).
5. `SetCheatCodeSaveData` appends to the history and the set instead of replacing them.

Cases:
1. Registered `(M, "moolah")`, text `"Moolah 1000"`: match result 1, id `M`; subscribers get `{0x230E27AC, M,
   "Moolah 1000"}`; history `{M, today, 0, 0}`.
2. `"disable secret"`: password `"secret"` stored compressed, notice `0x69D9A743`, cheats off; `"moolah"` now does
   nothing; `"enable wrong"` → false; `"enable secret"` → on again, notice `0x030E27B7`.
3. Two cheats on (IDs a, b): the save data ends `[…, 8, a, b]`; loading it inserts a, b and the six dwords after
   them.

`M` stands for whatever ID the registering subsystem chose.

## 6. Function inventory (owner `cGZCheatCodeManager`, 44)

Slots: `0x00920C0A`, `0x00920C35`, `0x00920C3F`, `0x00920C43`, `0x00920D52`, `0x00920DA1`, `0x00920E7E`,
`0x00920EA8`, `0x00920ECE`, `0x00920FA0`, `0x00921000`, `0x00921060`, `0x00921150`, `0x009211C3`, `0x00921215`,
`0x0092124C`, `0x009212A6`, `0x00921300`, `0x00921310`, `0x0092131E`, `0x009214DB`, `0x009215D7`, `0x00921621`,
`0x00921817`, `0x0092184B`, `0x00921B38`. Helpers: `0x008FE833` (16-byte record copy), `0x00920B39` (pair equality),
`0x00920C78`, `0x00920E37` (find a pair), `0x00920D15` (the stored block's length), `0x00920E61` (vector copy),
`0x00921232` (erase a range of the set), `0x009215A3` (add a history record), `0x009217F2` (entry copy),
`0x0092194F`, `0x00921ADD`, `0x00921B1E` (list node and append), `0x00921970` constructor, `0x00921A53`,
`0x00921A5B`, `0x00921A63` thunks, `0x00921A6B` destructor body, `0x00921B02` deleting destructor.

Not covered: `cRZFastCompression2`, and who calls the save-data slots (CCM1).
