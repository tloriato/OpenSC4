| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| core-text-edit4: portable TextEdit owner | Codex / `core-text-edit4` (engine lane) | in progress; detached HEAD; no ABI changes | 1. Baseline gate and published contracts. 2. Implement and test owner paths. 3. Final gate and bounded review queue. |

## Handoff
- Goal: first portable TextEdit owner, published contracts only; preserve TE5/TE8, no game/native promotion.
- Done: 3d010273 substantive first source; added layout/scroll geometry/navigation, complete key routing, paint ordering/hit mapping and resource/timer adapters. TE9 records unpublished choices; TE5/TE8 preserved.
- Done: 37 independent scenarios written; all seven new Rust files passed qbuild fmt with absolute paths before final small fixes. Resource owner cannot be cloned without acquiring references. No game requests.
- Next: quick result (fc4verify-quick-test.log), stable full gate and final report/queue; final source commit follows fixes to the one clippy collapsible-match finding and sound failure-handle release. LengthFromGlyphCount is a foreign call, not an inferred parser.
- Baseline all: Rust stopped in unchanged capi occupant_manager_tests::slot42_callbacks_run_unlocked_across_threads: 536 passed, 1 failed (200.82s). Clippy still running; quick waiting on build directory lock. Do not claim a clean gate.
- Constraints: no game, clipboard OS, ABI changes, agents, main edits or branch/ref updates.
- Manifest: C:/Users/tlori/FreeCity4/build-runs/run4-core-text-edit4-r1-manifest.json.

## Per-body contract checklist

Portable entries below implement owner decisions in the named area, with foreign service inputs and explicit safe-domain boundaries. Value getters replace native pointer outputs; aliasing/GZCOM ABI is withheld. P and P/B are copied publication dispositions, never portable proof grades. Unmatched labels are withheld rather than inferred.

| Published body | Label | Spec disposition | Portable implementation / withheld scope |
|---|---|---|---|
| `0x0091880E` | GetTextLength | P | byte copy exists; output-string virtual-call result/alias ABI withheld |
| `0x0099357F` | GetTextGutterX | P | layout/scroll |
| `0x00993586` | GetTextGutterY | P | layout/scroll |
| `0x00994A39` | GetText | P | text/copy/maximum |
| `0x00994E6A` | GetMaximumTextToAllow | P | text/copy/maximum |
| `0x00994E7E` | GetFirstVisibleLine | P | layout/scroll |
| `0x00994E85` | GetLineHeight | P | layout/scroll |
| `0x00994E8C` | SetText | P | text/copy/maximum |
| `0x00994EB9` | GetCaption | P | text/copy/maximum |
| `0x00994EC0` | SetCaption | P | text/copy/maximum |
| `0x00994ED7` | SetMaxUndoLevelCount | P | text/copy/maximum |
| `0x00994FF2` | GetLineCount | P | layout/scroll |
| `0x009BC5E1` | QueryInterface | P | withheld: no matching portable entry |
| `0x009BC60E` | ToggleCaretVisibility | P | caret/timers |
| `0x009BC635` | MakeCaretVisible | P | caret/timers |
| `0x009BC65C` | StartCaretTimer | P | caret/timers |
| `0x009BC6B5` | GZOnKeyUp | P | input/messages |
| `0x009BC6D8` | GZOnMouseDownR | P/B | input/messages |
| `0x009BC705` | GZOnMouseWheel | P/B | input/messages |
| `0x009BC75D` | GZOnSetFocus | P/B | input/messages |
| `0x009BC79D` | GZOnKillFocus | P/B | input/messages |
| `0x009BC7DD` | SetFlag | P/B | mode/flags |
| `0x009BC81C` | SetFlag | P/B | mode/flags |
| `0x009BC837` | ClearFlag | P | mode/flags |
| `0x009BC854` | IsFlagSet | P | mode/flags |
| `0x009BC864` | GetScrollBarImageVertical | P | resources/colors |
| `0x009BC88E` | GetScrollBarImageHorizontal | P | resources/colors |
| `0x009BC8DB` | SetFillColor | P | resources/colors |
| `0x009BC904` | SetFontStyle | P/B | resources/colors |
| `0x009BC94E` | GetFontStyle | P | resources/colors |
| `0x009BC967` | SetColor | P | resources/colors |
| `0x009BC97B` | GetColor | P | resources/colors |
| `0x009BC991` | GetBackgroundImage | P | resources/colors |
| `0x009BC9AC` | SetBackgroundImage | P | resources/colors |
| `0x009BCA09` | SetTextGutterY | P/B | layout/scroll |
| `0x009BCA39` | InsertText | P/B | withheld wrapper; raw/high-level entries exist, exact wrapper parameters unpublished (TE9) |
| `0x009BCA6B` | RemoveText | P/B | withheld wrapper; raw/high-level entries exist, exact wrapper parameters unpublished (TE9) |
| `0x009BCA9D` | RemoveText | P/B | withheld wrapper; raw/high-level entries exist, exact wrapper parameters unpublished (TE9) |
| `0x009BCAFE` | SetInsertMode | P | mode/flags |
| `0x009BCB14` | GetInsertMode | P | mode/flags |
| `0x009BCB2E` | GetInsertionLineAndColumn | P | position/byte navigation |
| `0x009BCB45` | SetCaretPeriod | P | caret/timers |
| `0x009BCB9D` | GetCaretPeriod | P | caret/timers |
| `0x009BCBAE` | GetSelection | P | position/byte navigation |
| `0x009BCBC5` | CalculateVisibleWidth | P/B | layout/scroll |
| `0x009BCC0E` | CalculateVisibleHeight | P/B | layout/scroll |
| `0x009BCC57` | SetSoundID | P | resources/colors |
| `0x009BCC74` | StartScrollTimer | P | caret/timers |
| `0x009BCC93` | StopScrollTimer | P | caret/timers |
| `0x009BCCAF` | ScrollTimerCallback | P | caret/timers |
| `0x009BCCCE` | DoCursorScroll | P | input/messages |
| `0x009BCDBA` | RemoveScrollBar | P | layout/scroll |
| `0x009BCE3D` | SetScrollBarSizes | P | layout/scroll |
| `0x009BD003` | Shutdown | P/B | resources/colors |
| `0x009BD0E1` | DoMessage | P/B | input/messages |
| `0x009BD16C` | CaretTimerCallback | P | caret/timers |
| `0x009BD19B` | GetTextArea | P/B | layout/scroll |
| `0x009BD1CD` | GZOnMouseUpL | P/B | input/messages |
| `0x009BD1F6` | GZOnMouseMove | P/B | input/messages |
| `0x009BD278` | SetScrollBarImageVertical | P | resources/colors |
| `0x009BD2F9` | SetScrollBarImageHorizontal | P | resources/colors |
| `0x009BD37A` | SetCaretColor | P | resources/colors |
| `0x009BD399` | GetCaretColor | P | resources/colors |
| `0x009BD3B9` | GetFont | P | resources/colors |
| `0x009BD3C0` | SetFontColor | P | resources/colors |
| `0x009BD3E1` | SetFontColor | P | resources/colors |
| `0x009BD406` | GetFontColor | P | resources/colors |
| `0x009BD447` | ClearFontColor | P | resources/colors |
| `0x009BD47F` | GetHighlightFillColor | P | resources/colors |
| `0x009BD495` | SetHighlightFillColor | P | resources/colors |
| `0x009BD4B4` | SetOutlineUse | P | resources/colors |
| `0x009BD4E4` | GetOutlineUse | P | resources/colors |
| `0x009BD510` | GetIndexFromPosition | P | position/byte navigation |
| `0x009BD554` | GetCharacterCountOfLine | P | position/byte navigation |
| `0x009BD593` | GetLineAndColumnFromIndex | P | position/byte navigation |
| `0x009BD610` | GetIndexFromLineAndColumn | P | position/byte navigation |
| `0x009BD66E` | SetInsertionIndex | P | position/byte navigation |
| `0x009BD6B4` | SetInsertionLineAndColumn | P | position/byte navigation |
| `0x009BD71A` | IncrementInsertionPositionByComposedCharacter | P | increment_position(composed=true); foreign composed lengths TE5 |
| `0x009BD7FA` | IncrementInsertionPosition | P | position/byte navigation |
| `0x009BD89D` | IncrementInsertionPositionByWord | P | increment_word: space/LF byte scans |
| `0x009BD997` | SetSelection | P | position/byte navigation |
| `0x009BDA65` | FixInsertionPositionIfBroken | P | position/byte navigation |
| `0x009BDB00` | GetPixelWidthOfLongestLine | P | layout/scroll |
| `0x009BDB83` | GetHorizontalLineMetrics | P | layout/scroll |
| `0x009BDCD1` | SetScrollBarValues | P | layout/scroll |
| `0x009BDDC2` | GetHorizontalScrollPixels | P | layout/scroll |
| `0x009BDE57` | ScrollLineUp | P | layout/scroll |
| `0x009BDE87` | ScrollLineDown | P | layout/scroll |
| `0x009BDEC9` | ScrollLeft | P | layout/scroll |
| `0x009BDF39` | ScrollRight | P | layout/scroll |
| `0x009BDFC7` | ScrollToStart | P | layout/scroll |
| `0x009BE009` | ScrollToEnd | P | layout/scroll |
| `0x009BE063` | ScrollPageUp | P | layout/scroll |
| `0x009BE0A9` | ScrollPageDown | P | layout/scroll |
| `0x009BE111` | ResizeWindowForExactLineHeights | P | layout/scroll |
| `0x009BE179` | GetColumnFromPosition | P/B | layout/scroll |
| `0x009BE239` | GetPositionFromLineAndColumn | P/B | layout/scroll |
| `0x009BE2DB` | GetText | P | text/copy/maximum |
| `0x009BE302` | PutInsertionPositionInView | P | layout/scroll |
| `0x009BE477` | AddScrollBar | P | layout/scroll |
| `0x009BE614` | FixBadCharacterComposition | P/B | foreign repair/search/paint |
| `0x009BE6CE` | SelectCurrentWord | P | position/byte navigation |
| `0x009BE7AF` | TestForPossibleScrollBarAdditionOrRemoval | P/B | layout/scroll |
| `0x009BE862` | GZOnMouseDownL | P/B | input/messages |
| `0x009BE945` | UpdateScrollBarsAndWindowPosition | P | layout/scroll |
| `0x009BE96A` | FindText | P/B | foreign repair/search/paint |
| `0x009BE99F` | ShowWindow | P | layout/scroll |
| `0x009BEA28` | GZPaint | P/B | foreign repair/search/paint |
| `0x009BF162` | GetLineAndColumnFromPosition | P | layout/scroll |
| `0x009BF2E5` | AdjustLinePositionArray | P | layout/scroll |
| `0x009BF335` | RemoveBadCharacters | P/B | foreign repair/search/paint |
| `0x009BF3E0` | CalculateAllLineLengths | P/B | layout/scroll |
| `0x009BF578` | RemoveBadCharacters | P | foreign repair/search/paint |
| `0x009BF5B7` | InsertText | P | mutation/history |
| `0x009BF776` | RemoveChar | P | mutation/history |
| `0x009BF8A8` | RemoveText | P/B | mutation/history |
| `0x009BF98B` | RecalculateAll | P/B | layout/scroll |
| `0x009BFB9C` | Init | P/B | init: font fallback, cursor, timers, code page, clipboard; foreign failures TE8 |
| `0x009BFCA5` | SetArea | P | layout/scroll |
| `0x009BFCEF` | SetFlags | P/B | mode/flags |
| `0x009BFDC6` | SetFont | P/B | resources/colors |
| `0x009BFE03` | SetTextGutterX | P/B | layout/scroll |
| `0x009BFF52` | ~cGZWinTextEdit | P/B | destroy/shutdown: owner order; native deleting wrapper and allocation unwind withheld |
| `0x009BFF6E` | InitializeMemberVariables | P | new: published defaults; unspecified words TE9; native construction ABI withheld |
| `0x009C01AF` | cGZWinTextEdit | P | new: published defaults; unspecified words TE9; native construction ABI withheld |
| `0x009C027E` | Undo | P/B | mutation/history |
| `0x009C0396` | Redo | P/B | mutation/history |
| `0x009C04FE` | SetText | P | text/copy/maximum |
| `0x009C068D` | AddUndoRecordForTextInsertion | P | mutation/history |
| `0x009C075F` | AddUndoRecordForTextTextDeletion | P | mutation/history |
| `0x009C07E1` | InsertTextHighLevel | P | mutation/history |
| `0x009C0996` | GZOnCharacter | P | input/messages |
| `0x009C0A95` | GZOnKeyDown | P/B | input/messages |

Additional attribution: space/LF, preceding-LF correction and sound adapter are implemented as separator/correct_preceding_lf/reason. Initial line entry and action scalar/string assignment, forward suffix retention and range destruction are represented by owned Rust vectors/actions. Native 60-byte allocation, copy-construction/null destination tests, deleting destruction, generated failure/unwind and pointer-adjustment thunks remain withheld (TE8). The folded maximum setter is set_maximum, outside the 150-body attribution. No inherited window owner or extra text-owner methods are claimed.
