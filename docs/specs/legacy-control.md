# Legacy control (`cGZLegacyControl`): building widgets from `LEGACY` script blocks

Status: partial, 45 of 57 owner functions (2026-10-21, lane re-legacycontrol4). Owner `cGZLegacyControl` in SimCity 4 Deluxe 1.1.641
(Windows). Covered: object layout, COM surface, lifecycle, the shared vocabulary and its two lookup maps, the
`SetParameters` build sequence, the property-list readers, and the full word tables of all 21 vocabulary
functions. Per-control interpreter contracts (`InterpretProperties*`) are specified for the common pass (§8);
the per-type interpreters are listed in §9 with their state.

Related specs: `widget-toolkit.md` (`cGZWin`, the windows this object builds), `ui-windows.md` (`cIGZWin`,
`cIGZWinGen`, the script loader). Interface slot names below come from `docs/abi/interfaces.tsv`.

## 1. Identity

| Item | Value |
|---|---|
| Object size | `0x20` bytes (factory `cGZUIScriptCOMDirector::CreateLegacyControl` `0x0094AF90` allocates `0x20` and runs the constructor `0x0095B8C4`) |
| Primary vtable | `0x00AD5360`, 11 slots |
| Second sub-object (`+0x04`) | vtable `0x00AD5344`, 8 slots: the control-area interface (§3, answers LC1) |
| Third sub-object (`+0x08`), the legacy-control interface | vtable `0x00AD5324`, 8 slots |
| IIDs answered by `QueryInterface` `0x0094C431` | `0x00000001` and `0x1A356083` → the object itself; `0x5A356159` → `this+4`; `0x9A4527CB` → `this+8` |

`QueryInterface` stores the pointer, calls `AddRef` on the primary object, and returns true; any other IID returns
false without touching the out-pointer. For the `+4`/`+8` cases a null object would yield null (the code masks
the adjusted pointer by "this is non-null"); the result is otherwise identical.

## 2. Object layout *[compat]*

| Offset | Size | Meaning | Constructor value |
|---|---|---|---|
| `+0x00` | 4 | primary vtable | `0x00AD5360` |
| `+0x04` | 4 | second vtable | `0x00AD5344` |
| `+0x08` | 4 | legacy-control interface vtable | `0x00AD5324` |
| `+0x0C` | 4 | reference count | 0 |
| `+0x10` | 4 | **the control**: the `cIGZWin` this object builds (owned reference) | null |
| `+0x14` | 4 | **pending parent** window (owned reference), consumed by `SetParameters` | null |
| `+0x18` | 4 | **window procedure** object (owned reference), set by `UseWinProc` (§7.2), handed to the generator and dropped by `DoExceptions` (§7.3) | null |
| `+0x1C` | 1 | **own-control** flag: when set, destruction detaches and shuts the control down (§5.2) | 0 |

## 3. Slot tables *[compat]*

Primary vtable `0x00AD5360`:

| Slot | Offset | Target | Method | Contract |
|---|---|---|---|---|
| 0 | `+0x00` | `0x0094C431` | `QueryInterface(iid, out)` | §1 |
| 1 | `+0x04` | `0x0094C47C` | `AddRef` (shared with `cGZWin::AddRef`) | `++[+0x0C]`, returns the new count |
| 2 | `+0x08` | `0x0094C483` | `Release` | §5.3 |
| 3 | `+0x0C` | `0x00957A9B` | `SetParameters(props)` | §6; returns false always |
| 4 | `+0x10` | `0x0094C30E` | `GetControlVocabulary()` | returns the shared vocabulary pointer (no `AddRef`) |
| 5 | `+0x14` | `0x0094C3CD` | `GetID()` | control null → 0; else the control's `GetID` (`cIGZWin` `+0xFC`), tail-called |
| 6 | `+0x18` | `0x0094C3E1` | `SetID(id)` | control null → nothing; else the control's `SetID` (`+0x100`) with the same argument |
| 7 | `+0x1C` | `0x0094C314` | `SetOwnControl(bool)` | writes the byte at `+0x1C` |
| 8 | `+0x20` | `0x00935EA0` | `ImplementInterface(4 args)` | returns true, ignores all four arguments (ICF-shared body) |
| 9 | `+0x24` | `0x0094C31E` | `SetParent(obj)` | §7.1; returns true always |
| 10 | `+0x28` | `0x0094C761` | scalar deleting destructor | §5.2 |

Control-area interface vtable `0x00AD5344` (sub-object at `+0x04`, IID `0x5A356159`; no name in the export, the
interface name is *[inferred]*). Raw slot words read from the image at `0x00AD5344`–`0x00AD535F`; the next word,
`0x00AD5360`, starts the primary vtable. Slots 0–2 are `this − 4` thunks.

| Slot | Offset | Target | Method | Contract |
|---|---|---|---|---|
| 0 | `+0x00` | `0x0094C4B8` | `QueryInterface` | thunk to `0x0094C431` |
| 1 | `+0x04` | `0x009359BD` | `AddRef` | thunk to `0x0094C47C` |
| 2 | `+0x08` | `0x0094C4C0` | `Release` | thunk to `0x0094C483` |
| 3 | `+0x0C` | `0x0091559A` | (one argument) | returns true, argument ignored (ICF-shared `mov al,1; ret 4`) |
| 4 | `+0x10` | `0x0091559A` | (one argument) | same body as slot 3 |
| 5 | `+0x14` | `0x0094C3F5` | `SetArea(rect)` *[inferred name]* | control (object `+0x10`) null → false; else control `SetArea(rect)` (`cIGZWin` `+0xD8`), true |
| 6 | `+0x18` | `0x0094C413` | `GetArea(rect)` *[inferred name]* | control null → false; else control `GetArea(rect)` (`cIGZWin` `+0xBC`), true |
| 7 | `+0x1C` | `0x0092B33A` | (two arguments) | returns true, both arguments ignored (ICF-shared `mov al,1; ret 8`) |

Slots 5 and 6 pass the caller's rectangle pointer through unchanged and return true even though the window
call's own result is discarded.

Legacy-control interface vtable `0x00AD5324` (sub-object at `+0x08`; slots 0–2 and 4 are `this − 8` thunks):

| Slot | Offset | Target | Method | Contract |
|---|---|---|---|---|
| 0 | `+0x00` | `0x0094C4C8` | `QueryInterface` | thunk to `0x0094C431` |
| 1 | `+0x04` | `0x009359D5` | `AddRef` | thunk to `0x0094C47C` |
| 2 | `+0x08` | `0x0094C4D0` | `Release` | thunk to `0x0094C483` |
| 3 | `+0x0C` | `0x009C32AC` | `GetControl()` | returns the control at object `+0x10` (no `AddRef`; ICF-shared 4-byte getter) |
| 4 | `+0x10` | `0x0094C4D8` | `SetOwnControl(bool)` | thunk to `0x0094C314` |
| 5 | `+0x14` | `0x0094C3A1` | `ForceControl(ptr)` | §7.2, stores into **`+0x10`** (object-relative, see §7.2) |
| 6 | `+0x18` | `0x0094C375` | `UseWinProc(win)` | §7.2, stores into the control slot |
| 7 | `+0x1C` | `0x0096638D` | `GetScriptFragment(out)` | §7.4 |

## 4. The shared vocabulary *[compat]*

One vocabulary object and two lookup maps are shared by every legacy control in the process.

| Global | What |
|---|---|
| `0x00B63588` | the vocabulary (a GZCOM object created from class `0xBA2E7954`, interface `0x1A2BDEBB`); refcounted |
| `0x00B6358C` | **interpreter map**: IID → interpreter function `(cIGZWin*, props, bool firstPass)` |
| `0x00B63598` | **class map**: class ID → IID |

Both maps are ordered maps keyed by unsigned 32-bit value (ascending order matters for §6 step 4).

**Construction** (`0x0095B8C4`): fields as in §2. If the vocabulary global is null, create it through
GZCOM (`CreateInstance(0xBA2E7954, 0x1A2BDEBB, &global)`); if that succeeded, run the 21 vocabulary
functions in this exact order: Common, FlatRect, Text, TextEdit, ListBox, Combo, Btn, BMP, Slider, Scrollbar,
LineInput, OptGrp, Spinner, Outline, TextTicker, FileBrowser, TreeView, Folders, Grid, Custom, SC3WinGen. If the
global was already set, `AddRef` it instead (vocabulary slot 1). Creation failure leaves the global null and the
object usable (lookups then find nothing).

**A vocabulary function** registers each word with vocabulary slot 3 (`+0x0C`, `Add(name, id)`; the name is a
temporary string). Every per-type function (all but Common) first adds the class name and the interface name
(SC3WinGen adds two class names, `SC3WinGen` and `GZWinGen`, both `0x4386D516`), then its property words, and
finally inserts `IID → interpreter` into the interpreter map and `class ID → IID` into the class map (unique
insert: a second registration of the same key keeps the first). Common adds its words only and registers no
map entries; its interpreter is called directly (§6).

**Release** (in the destructor, §5.2): call vocabulary `Release`; if it returns 0, null the global. The maps
are never cleared, so a re-created vocabulary re-inserts into maps that already hold the keys (no change).

### 4.1 Registered classes

The interpreter map key equals the interface ID in every row (verified per function from the map-insert
arguments).

| Vocabulary fn | Class name = class ID | Interface name = IID | Interpreter | Property words |
|---|---|---|---|---|
| `0x0094D641` | (none: common words) | | called directly | 64 |
| `0x00951269` | `GZWinTextTicker` = `0xE32F0B31` | `IGZWinTextTicker` = `0x32F0B3C` | `0x0094CE28` | 3 |
| `0x009518A8` | `GZWinFolders` = `0x78E8055` | `IGZWinFolders` = `0xA13351B8` | `0x0094D17E` | 0 |
| `0x0095195E` | `GZWinCustom` = `0x478D1E6F` | `IGZWinCustom` = `0x678D1E84` | `0x00A6D837` | 0 |
| `0x00951A14` | `GZWinFlatRect` = `0xC2AFA76E` | `IGZWinFlatRect` = `0xC2AFA76F` | `0x0094E33B` | 9 |
| `0x00951C9F` | `GZWinText` = `0x592` | `IGZWinText` = `0x212CDC1F` | `0x0094E516` | 15 |
| `0x00952046` | `GZWinTextEdit` = `0x231A1C58` | `IGZWinTextEdit` = `0x231A1C57` | `0x0094E7C8` | 26 |
| `0x00952645` | `GZWinListBox` = `0x598` | `IGZWinListBox` = `0x4132242B` | `0x0094EE39` | 7 |
| `0x00952868` | `GZWinCombo` = `0x59B` | `IGZWinCombo` = `0x412CE496` | `0x0094F36D` | 4 |
| `0x009529EF` | `GZWinBtn` = `0x22ECFC47` | `IGZWinBtn` = `0x8810` | `0x0094F757` | 28 |
| `0x00953056` | `GZWinBMP` = `0x82FE68C4` | `IGZWinBMP` = `0xC12CEA13` | `0x0095002C` | 3 |
| `0x009531A9` | `GZWinSlider` = `0x21325208` | `IGZWinSlider` = `0x21325207` | `0x00950218` | 4 |
| `0x00953330` | `GZWinScrollbar` = `0x61325A2E` | `IGZWinScrollbar` = `0x61325A2D` | `0x009503AB` | 3 |
| `0x00953483` | `GZWinLineInput` = `0x21335C5A` | `IGZWinLineInput` = `0x21335C59` | `0x009505B6` | 12 |
| `0x009537AA` | `GZWinSpinner` = `0x612CE0C4` | `IGZWinSpinner` = `0x612CE0C3` | `0x00950AF6` | 11 |
| `0x00953A9D` | `GZWinOutline` = `0x4303E5B6` | `IGZWinOutline` = `0xE303E5AE` | `0x00951084` | 3 |
| `0x00953BF0` | `SC3WinGen` / `GZWinGen` = `0x4386D516` | `IGZWinGen` = `0x5386D516` | `0x009513BC` | 18 |
| `0x009552BE` | `GZWinFileBrowser` = `0x1AA52EA4` | `IGZWinFileBrowser` = `0x3AA52E64` | `0x00954212` | 66 |
| `0x00957C88` | `GZWinOptGrp` = `0xA1336CC1` | `IGZWinOptGrp` = `0xA1336CC0` | `0x00956C46` | 19 |
| `0x009599D2` | `GZWinTreeView` = `0x3AE8BAE1` | `IGZWinTreeView` = `0x3AE8BAD2` | `0x009586B0` | 18 |
| `0x0095B021` | `GZWinGrid` = `0xDAA6B9BF` | `IGZWinGrid` = `0xDAA6B9BE` | `0x0095A250` | 39 |

### 4.2 Word tables

Word IDs per vocabulary function, in registration order. The words `yes`/`true`/`on` = 1 and
`no`/`false`/`off` = 0 are registered by Common; style words (`left`, `flat`, `sunken`, ...) are registered with
IDs in the `0xF000` range and are compared as values, not as property keys.

**common** (`0x0094D641`): `clsid` 0xFFFFFFFF, `iid` 0xFFFFFFFE, `yes` 0x1, `true` 0x1, `no` 0x0, `false` 0x0, `on` 0x1, `off` 0x0, `GZWin` 0xE2BA00EE, `id` 0x100, `area` 0x101, `pos` 0x102, `size` 0x103, `fillcolor` 0x104, `caption` 0x105, `captionres` 0x106, `transparent` 0x107, `font` 0xF000, `bkgcolor` 0xF001, `forecolor` 0xF002, `notify` 0xF003, `gutters` 0xF004, `style` 0xF005, `left` 0xF006, `right` 0xF007, `center` 0xF008, `flat` 0xF009, `normal` 0xF00A, `sunken` 0xF00B, `raised` 0xF00C, `fill` 0xF00D, `nofill` 0xF00E, `colorfontnormal` 0xF00F, `colorfontdisabled` 0xF010, `colorfonthilited` 0xF011, `colorfontnormalbkg` 0xF012, `colorfontdisabledbkg` 0xF013, `colorfonthilitedbkg` 0xF014, `align` 0xF015, `image` 0xF016, `imagetype` 0xF017, `imagerect` 0xF018, `outline` 0xF019, `comments` 0x108, `commentslmd` 0x109, `commentsrmd` 0x10A, `commentslmu` 0x10B, `commentsrmu` 0x10C, `commentsmm` 0x10D, `commentsku` 0x10E, `commentskd` 0x10F, `winflag_visible` 0xF01A, `winflag_enabled` 0xF01B, `winflag_moveable` 0xF01C, `winflag_sizeable` 0xF01D, `winflag_sortable` 0xF01E, `winflag_pbuff` 0xF01F, `winflag_pbufftrans` 0xF020, `winflag_pbufferase` 0xF021, `winflag_pbuffvid` 0xF022, `winflag_alphablend` 0xF026, `winflag_mousetrans` 0xF023, `winflag_ignoremouse` 0xF024, `winflag_acceptfocus` 0xF025

**IGZWinTextTicker** (`0x00951269`): `speed` 0x1200, `frequency` 0x1201, `message` 0x1202

**IGZWinFlatRect** (`0x00951A14`): `colorleft` 0x200, `colortop` 0x201, `colorright` 0x202, `colorbottom` 0x203, `top` 0x204, `bottom` 0x205, `all` 0x206, `lrb` 0x207, `trb` 0x208

**IGZWinText** (`0x00951C9F`): `wrapped` 0x300, `opaque` 0x301, `defcolor` 0x302, `dbgdrawarea` 0x303, `autofit` 0x304, `lefttop` 0x0, `leftcenter` 0x1, `leftbottom` 0x2, `centertop` 0x3, `center` 0x4, `centerbottom` 0x5, `righttop` 0x6, `rightcenter` 0x7, `rightbottom` 0x8, `textoffsets` 0x305

**IGZWinTextEdit** (`0x00952046`): `editable` 0x400, `hscrollbar` 0x401, `vscrollbar` 0x402, `caretvisible` 0x403, `allowinsert` 0x404, `allowundo` 0x405, `singleline` 0x406, `caretcolor` 0x407, `highlightcolor` 0x408, `maxtext` 0x409, `initvalue` 0x40A, `outlinecolor` 0x40B, `overwrite` 0x40C, `insertindex` 0x40D, `insertpos` 0x40E, `caretperiod` 0x40F, `maxundo` 0x410, `hscrollimage` 0x413, `hscrollimageRect` 0x414, `vscrollimage` 0x411, `vscrollimageRect` 0x412, `notifyonreturn` 0x415, `notifyonchange` 0x416, `notifyonlostfocus` 0x417, `enableclipboard` 0x418, `passwordmode` 0x419

**IGZWinListBox** (`0x00952645`): `scrollbargutters` 0x500, `initselection` 0x501, `sort` 0x503, `drop` 0x504, `setlinevisible` 0x505, `setlineontop` 0x506, `listelement` 0x502

**IGZWinCombo** (`0x00952868`): `combodownarrowimage` 0x800, `combodownarrowrect` 0x801, `combodowncolor` 0x802, `buttongutter` 0x803

**IGZWinBtn** (`0x009529EF`): `toggle` 0xA00, `triggerondown` 0xA01, `showcaption` 0xA02, `autosize` 0xA03, `wrapcaption` 0xA04, `shiftcaption` 0xA05, `tips` 0xA06, `tipsdelay` 0xA07, `tipstimeout` 0xA08, `label` 0xA09, `standard` 0xA0A, `standardfixed` 0xA0B, `togglebtn` 0xA0C, `togglebtnfixed` 0xA0D, `checkbox` 0xA0E, `radiobtn` 0xA0F, `tiptext` 0xA10, `tipoffsets` 0xA11, `tipres` 0xA12, `noimage` 0xA13, `standardhilite` 0xA14, `standardhilitefixed` 0xA15, `radiocheckall` 0xA16, `radio` 0xA17, `tipflag` 0xA18, `btnclicksnd` 0xA19, `btnupsnd` 0xA1A, `radiocheck` 0xA1B

**IGZWinBMP** (`0x00953056`): `alpha` 0xB00, `transparentbkg` 0xB01, `edgeimage` 0xB02

**IGZWinSlider** (`0x009531A9`): `minmaxvalue` 0xC00, `direction` 0xC01, `horizontal` 0xC02, `vertical` 0xC03

**IGZWinScrollbar** (`0x00953330`): `pagesize` 0xD00, `linesize` 0xD01, `linepagecount` 0xD02

**IGZWinLineInput** (`0x00953483`): `filled` 0xE00, `outline3d` 0xE01, `outline3dex` 0xE02, `colorlt` 0xE03, `colorrb` 0xE04, `colorltinside` 0xE05, `colorrbinside` 0xE06, `fillcolorhilite` 0xE07, `fillalphahilite` 0xE08, `caretpos` 0xE09, `charlimit` 0xE0A, `selection` 0xE0B

**IGZWinSpinner** (`0x009537AA`): `digits` 0x1000, `autonumber` 0x1001, `autonumbercomma` 0x1002, `autonumbercurrency` 0x1003, `minmax` 0x1004, `stepsize` 0x1005, `coloroutline` 0x1006, `coloroutlinel` 0x1007, `coloroutlinet` 0x1008, `coloroutliner` 0x1009, `coloroutlineb` 0x100A

**IGZWinOutline** (`0x00953A9D`): `colordark` 0x1100, `colorlight` 0x1101, `node` 0x1102

**IGZWinGen** (`0x00953BF0`): `userdata` 0x10000, `blttype` 0x10001, `tiled` 0x0, `edge` 0x2, `divider` 0x10002, `bluebar` 0x10003, `moveable` 0x10004, `sizeable` 0x10005, `defaultkeys` 0x10006, `closevisible` 0x10007, `gobackvisible` 0x10008, `minmaxvisible` 0x10009, `closedisabled` 0x1000A, `gobackdisabled` 0x1000B, `minmaxdisabled` 0x1000C, `titlebar` 0x1000D, `paint` 0x1000E, `sidebar` 0x1000F

**IGZWinFileBrowser** (`0x009552BE`): `defaultpath` 0x1301, `backdir` 0x1302, `dir` 0x1303, `file` 0x1304, `defaultselectiontype` 0x1305, `defaultselectionname` 0x1306, `defaultselectionextension` 0x1307, `bynameascending` 0x1308, `bynamedescending` 0x1309, `byextensionascending` 0x130A, `byextensiondescending` 0x130B, `filesortprimary` 0x130C, `filesortsecondary` 0x130D, `topascending` 0x130E, `topdescending` 0x130F, `bottomascending` 0x1310, `bottomdescending` 0x1311, `directorysort` 0x1312, `icon` 0x1313, `name` 0x1314, `extension` 0x1315, `empty` 0x1316, `unused` 0x1317, `default` 0x1318, `column1data` 0x1319, `column2data` 0x131A, `column3data` 0x131B, `filenamefont` 0x131C, `filenameforegroundcolor` 0x131D, `filenamebackgroundcolor` 0x131E, `filenameforegroundcolordisabled` 0x131F, `filenamebackgroundcolordisabled` 0x1320, `filenameforegroundcolorhilited` 0x1321, `filenamebackgroundcolorhilited` 0x1322, `filenamealignment` 0x1323, `fileextensionfont` 0x1324, `fileextensionforegroundcolor` 0x1325, `fileextensionbackgroundcolor` 0x1326, `fileextensionforegroundcolordisabled` 0x1327, `fileextensionbackgroundcolordisabled` 0x1328, `fileextensionforegroundcolorhilited` 0x1329, `fileextensionbackgroundcolorhilited` 0x132A, `fileextensionalignment` 0x132B, `drivenamefont` 0x132C, `drivenameforegroundcolor` 0x132D, `drivenamebackgroundcolor` 0x132E, `drivenameforegroundcolordisabled` 0x132F, `drivenamebackgroundcolordisabled` 0x1330, `drivenameforegroundcolorhilited` 0x1331, `drivenamebackgroundcolorhilited` 0x1332, `drivenamealignment` 0x1333, `iconcolumnwidth` 0x1334, `extensioncolumnwidth` 0x1335, `rowheight` 0x1336, `linegridenabled` 0x1337, `linegridcolor` 0x1338, `columngridenabled` 0x1339, `columngridcolor` 0x133A, `highlightcolorforeground` 0x133B, `highlightcolorbackground` 0x133C, `backdirtext` 0x133D, `drivelistsize` 0x133E, `backgroundcolor` 0x133F, `backgroundimage` 0x1340, `backgroundtransparent` 0x1341, `filter` 0x1342

**IGZWinOptGrp** (`0x00957C88`): `maincaption` 0xF00, `itemcaption` 0xF01, `drawfill` 0xF02, `labelfont` 0xF03, `buttonfont` 0xF04, `labelfontcolornormal` 0xF05, `labelfontcolordisabled` 0xF06, `buttonfontcolornormal` 0xF07, `buttonfontcolorhilited` 0xF08, `buttonfontcolordisabled` 0xF09, `captiontextresid` 0xF0A, `backimage` 0xF0B, `autofilltype` 0xF0C, `option` 0xF0D, `automatic` 0xF0E, `manual` 0xF0F, `autofillsize` 0xF10, `optionmoveto` 0xF11, `optionsetsize` 0xF12

**IGZWinTreeView** (`0x009599D2`): `displayroot` 0x1401, `displaynodeicons` 0x1402, `displaytreelines` 0x1403, `multiselect` 0x1404, `icongutter` 0x1405, `minmaxboxsize` 0x1406, `roottext` 0x1407, `rooticon` 0x1408, `rooticonselected` 0x1409, `rooticonexpanded` 0x140A, `rooticonexpandedselected` 0x140B, `rootcolorforeground` 0x140C, `rootcolorbackground` 0x140D, `rootcolorforegroundhilited` 0x140E, `rootcolorbackgroundhilited` 0x140F, `rootfont` 0x1410, `rootexpanded` 0x1411, `rootselected` 0x1412

**IGZWinGrid** (`0x0095B021`): `fontcolor` 0x1601, `textalign` 0x1602, `textwrapping` 0x1603, `olinecolor` 0x1604, `selrule` 0x1605, `maxselcount` 0x1606, `dcolwidth` 0x1607, `drowheight` 0x1608, `enablehdr` 0x1609, `colhdrsz` 0x160A, `rowhdrsz` 0x160B, `fnone` 0x160C, `fedit` 0x160D, `fhscroll` 0x160E, `fvscroll` 0x160F, `fcolheading` 0x1610, `frowheading` 0x1611, `fcolgrid` 0x1612, `frowgrid` 0x1613, `finsmode` 0x1614, `fopqbkgnd` 0x1615, `fdoutline` 0x1616, `fdgridoutline` 0x1617, `fcellwrap` 0x1618, `fallownosel` 0x1619, `ffixcolcnt` 0x161A, `ffixrowcnt` 0x161B, `fdpastlastcol` 0x161C, `fdpastlastrow` 0x161D, `fcelloverlap` 0x161E, `fdrpdnmenu` 0x161F, `fhlcelldata` 0x1620, `fdefault` 0x1621, `fall` 0x1622, `colgridclr` 0x1623, `rowgridclr` 0x1624, `wingridcell` 0x1625, `wingridrow` 0x1627, `wingridcol` 0x1626

## 5. Lifecycle *[compat]*

### 5.1 Reference count

`AddRef` increments `+0x0C`. `Release` (`0x0094C483`): if the count is greater than 1, decrement and return the
new count; otherwise (count 1 **or 0**) call the deleting destructor (slot 10 with 1) and return 0. The count
is not decremented to 0 first.

### 5.2 Destruction (`0x0094C761`, body `0x0094C288`)

In order:

1. Restore the three vtables.
2. If the control (`+0x10`) is non-null:
   1. if own-control (`+0x1C`) is set: `parent = control->GetParentWin()` (`cIGZWin` `+0x2C`); if non-null,
      `parent->ChildRemove(control)` (`+0x3C`); then `control->Shutdown()` (`+0x14`) whether or not there was a
      parent;
   2. `control->Release()`; `+0x10 := null`.
3. If the pending parent (`+0x14`) is non-null: release it, null it.
4. Vocabulary: if the global is non-null, release it; a 0 result nulls the global.
5. If `+0x18` is non-null: release it, null it.
6. Free the `0x20` bytes when the deleting flag bit 0 is set.

Without own-control the control is only released: it stays attached to its parent, which holds its own
reference (§6 step 9).

## 6. `SetParameters(props)` — building the control *[compat]*

`props` is the property list of one `LEGACY` block: an array of `(key, cIGZVariant*)` pairs (8 bytes each,
`begin`/`end` at `+0`/`+4`), already translated from words to IDs by the script reader. Keys are 32-bit IDs from
§4.2. Readers are in §7.5.

Steps (`0x00957A9B`):

1. `clsid := Uint32(props, 0xFFFFFFFF)` (`clsid`). Missing, wrong type, or 0 → **return false**, nothing changed.
2. `iid := Uint32(props, 0xFFFFFFFE)` (`iid`); if missing, `iid := classMap[clsid]` when present, else 0.
3. `+0x10 := null` (any previous control pointer is overwritten **without release**; quirk Q1).
4. Create the control: `GZCOM CreateInstance(clsid, 0x22BA0121 cIGZWin, &[+0x10])`. On failure, walk the class map
   in ascending class-ID order to the **first** entry whose IID equals `iid` and try that class once (on its
   failure, stop walking); if still no control, create class `0x478D1E6F` (`GZWinCustom`). If that fails,
   **return false** with `+0x10` as the GZCOM call left it.
5. `control->SetParam(0xE85EEBDE, new variant(uint32 clsid))` (`cIGZWin` `+0x1C8`; the 20-byte variant is
   allocated here and handed over). If `iid != 0`, also `SetParam(0x08D6E462, new variant(uint32 iid))`. Note the
   **requested** `clsid` is stored even when a fallback class was created.
6. `+0x1C := 1` (the object now owns the control).
7. `interp := interpreterMap[iid]` (null if absent). If non-null: `interp(control, props, true)`.
8. `InterpretPropertiesGZWinCommon(control, props, true)` (§8).
9. `DoExceptions(control, true)` (§7.3).
10. `control->Init()` (`+0x10`).
11. Attach: if the pending parent is null, `RZ::WM()` slot 3 (`+0x0C`, returns the manager's top window,
    *[inferred]*) `->ChildAdd(control)` (`+0x38`); else `pendingParent->ChildAdd(control)`, then release the
    pending parent and null `+0x14`.
12. `control->PullToFront()` (`+0x5C`).
13. If `interp`: `interp(control, props, false)`.
14. `InterpretPropertiesGZWinCommon(control, props, false)`.
15. **Return false** (the epilogue clears AL unconditionally, `0x00957C81`). Callers cannot detect success from
    the return value; they read the control through `GetControl`.

So every interpreter runs twice: once before `Init`/attach (`firstPass = true`) and once after attach and
`PullToFront` (`firstPass = false`). The type interpreter always runs before the common one in each pass.

Worked example: block `clsid=0x592` (GZWinText) with no `iid`. Step 2 gives `iid = 0x212CDC1F`. If GZCOM can
create `0x592`, the control gets params `0xE85EEBDE = 0x592`, `0x08D6E462 = 0x212CDC1F`, the Text interpreter
`0x0094E516` and then Common run with `true`, the control is initialised and added to the pending parent (or the
manager's window), pulled to front, then Text and Common run again with `false`.

## 7. Small methods *[compat]*

### 7.1 `SetParent(obj)` (`0x0094C31E`)

Release and null any pending parent. If `obj` is non-null, ask it for interface `0x9A4527CB` (the legacy-control
interface of §3); on success take its `GetControl()` result, `AddRef` it into `+0x14` (null stays null), and
release the temporary interface. An `obj` that is not a legacy control leaves the pending parent null. Returns
true in every case. So the parent of a nested `LEGACY` block is the **control** of the enclosing legacy object.

### 7.2 `UseWinProc(win)` / `ForceControl(ptr)`

Both are reached through the `+0x08` sub-object, so their `this` is object `+8`. `UseWinProc` (`0x0094C375`)
writes `this+0x10` = object `+0x18`; `ForceControl` (`0x0094C3A1`) writes `this+0x08` = object `+0x10`, the
control. Both do the same thing to their slot: if the new pointer differs from the stored one, `AddRef` the new
one (if non-null), release the old one (if non-null), store the new one. Equal pointers: nothing. `ForceControl` does not set own-control.

### 7.3 `DoExceptions(win, flag)` (`0x0094C77D`)

Only when `flag` is true: ask `win` for `cIGZWinGen` (`0x5386D516`). If that succeeds **and** object `+0x18` is
non-null, call generator slot 7 (`+0x1C`) with `+0x18`, release `+0x18` and null it. Release the generator
interface if obtained. So a window procedure set by `UseWinProc` is handed to the generated window once and then
dropped by this object.

### 7.4 `GetScriptFragment(out)` (`0x0096638D`)

Calls `GetControl()`; null → return false. Otherwise allocate a new empty `cRZString` (20 bytes), `AddRef` it,
and run the serialiser `0x00965F73` (1050 bytes, not owned here; §7.6) to fill it from the control. On
success store the string in `*out` (the caller receives the reference) and return true; on failure release the
string and return false. The serialiser always reports success, so with a non-null control the method always
returns true with a filled string.

### 7.5 Property-list readers (shared helpers, used by every interpreter)

All take the property list in a register (not a stack argument) and scan it **linearly from the start**.

| Helper | Address | Rule |
|---|---|---|
| `HasProperty(key)` | `0x0094C64E` | true if any pair has the key (the value is not checked, may be null) |
| `Uint32(key, out)` | `0x0094C675` | first pair with the key, a non-null value and variant type 6 → `*out` = its 32-bit value, true. Pairs with the key but another type are skipped and the scan continues. None → false, `*out` untouched |
| `Sint32(key, out)` | `0x0094C6C7` | same scan and the same type test (6) |
| `String(key, out)` | `0x0094CCD9` | first clears `out`; then the first pair with a non-null value of variant type `0x800E` → copies its text, true |

Callers pre-initialise the out value, so "missing" means "keep the default".

### 7.6 Script fragment format (serialiser `0x00965F73`) (answers LC3)

The serialiser appends to the string it is given (it does not clear it; `GetScriptFragment` passes a new empty
string). Every item below is written as `word=value` followed by **one space**. Output, in order:

1. The literal `<LEGACY ` (with its trailing space).
2. Class and interface (helper `0x00965D01`), see below: `clsid=<class text> ` then `iid=<interface text> `.
3. The common words (helper `0x00962FC2`), see below.
4. The per-type words: the resolved interface ID selects a writer (table below). The serialiser asks the control
   for that interface; on success it calls the writer with it and releases it; if the control does not answer
   it, or the ID has no writer (`IGZWinCustom` `0x678D1E84`, base `IGZWin` `0x22BA0121`, anything else), no
   per-type words are written.
5. The literal `>` and a line feed (`0x3E 0x0A`, string at `0x00AD7028`). No carriage return; the text before
   `>` ends with the space of the last item.

It always returns true.

**Class and interface resolution** (`0x00965D01`). The class table at `0x00B16FA8` has 21 rows of
(class ID, interface ID, name), in this order: GZWinFlatRect, GZWinText, GZWinTextEdit, GZWinListBox,
GZWinLineInput, GZWinCombo, GZWinBtn, GZWinBMP, GZWinScrollbar, GZWinSlider, GZWinOptGrp, GZWinSpinner,
GZWinOutline, GZWinTextTicker, GZWinGen, GZWinFileBrowser, GZWinTreeView, GZWinFolders, GZWinGrid, GZWinCustom
(the IDs of §4.1; note row 14 is named `GZWinGen`, not `SC3WinGen`), and last `GZWin` = class `0xE2BA00EE`,
interface `0x22BA0121`.

1. Class ID = the control's parameter `0xE85EEBDE`, interface ID = its parameter `0x08D6E462` (both stored by
   `SetParameters`, §6 step 5); a missing parameter or a zero value counts as 0.
2. If either is 0: for **every** row in table order, ask the control for the row's interface (and release it
   on success). The first row that answers fills whichever of the two IDs is still 0 (class from the row's class
   column, interface from its interface column); later rows change nothing. After the scan, an interface ID still
   0 becomes `0x22BA0121`. (When both parameters exist the scan is skipped.)
3. A class ID still 0 becomes `0xE2BA00EE`.
4. Class text: the name of the row whose class ID matches, else `0x` and the ID as 8 lowercase hex digits
   (`0x%08lx`). Interface text: `I` + the name of the row whose interface ID matches, else the same hex form.
5. Write `clsid=<class text> `; the interface ID is non-zero by now, so `iid=<interface text> ` is always written.

Quirk Q8: with only one of the two parameters present, the other comes from the first answering table row, so the
pair can disagree (a stored class with the interface of an earlier-listed class the control also answers).

**Common words** (`0x00962FC2`), in this order:

| Word | Written when | Value format | Source |
|---|---|---|---|
| `id` | `GetID()` (`+0xFC`) is non-zero | `0x%08x` (8 lowercase hex digits) | the window ID |
| `area` | always | `(%d,%d,%d,%d)`, no spaces | `GetL` `+0xAC`, `GetT` `+0xB0`, `GetR` `+0xB4`, `GetB` `+0xB8` |
| `fillcolor` | always | `(%u,%u,%u)` | `GetFillColor(&r,&g,&b)` (`+0x1A0`) |
| `caption` | the caption (`+0x124`) has length > 0 | `"text"`, no escaping of quotes | the caption text |
| `captionres` | parameter `0x0B31771A` reads as a 12-byte resource key and its group or instance is non-zero | `{%08x,%08x}` = group, instance (no `0x`) | stored by §8 step 7 |
| `comments` … `commentskd` | parameter `0xBACA6879` exists, its variant type has bit `0x10`, the comment list is obtained, and that string is non-empty | `"text"` | the 8-string list of §8 step 9, same word order |
| 13 flag words | always | `yes` / `no` | `GetFlag(mask)` (`+0x10C`) |

Flag words, in order: `winflag_visible` `0x1`, `winflag_enabled` `0x2`, `winflag_moveable` `0x100`,
`winflag_sizeable` `0x200`, `winflag_sortable` `0x800`, `winflag_pbuff` `0x10000`, `winflag_pbufftrans`
`0x20000`, `winflag_pbufferase` `0x40000`, `winflag_pbuffvid` `0x100000`, `winflag_alphablend` `0x4`,
`winflag_acceptfocus` `0x8000`, `winflag_mousetrans` `0x80000`, `winflag_ignoremouse` `0x200000`.

Quirk Q9: `GetFillColor` (`0x0099BFC9`) writes one byte into each of three 32-bit locals the writer never
initialised (`0x00963065`–`0x00963084`), so the original can print stack garbage in the upper 24 bits.
Implementations print the byte values 0–255.

Not written by the common part: `pos`, `size` (the `area` form is always used), `transparent`, and every word
§8 does not read.

**Per-type writers** (dispatch at `0x00965FCA`–`0x0096634C`; not owner functions, their word sets are
outside this spec and stay open as LC8):

| Interface | Writer | Size | | Interface | Writer | Size |
|---|---|---|---|---|---|---|
| Slider `0x21325207` | `0x0095F4EC` | 375 | | FlatRect `0xC2AFA76F` | `0x0095C211` | 841 |
| Text `0x212CDC1F` | `0x0095C55A` | 1789 | | BMP `0xC12CEA13` | `0x0095F2CA` | 546 |
| TextTicker `0x032F0B3C` | `0x00960C31` | 214 | | Scrollbar `0x61325A2D` | `0x0095F663` | 536 |
| Btn `0x00008810` | `0x0095E6F0` | 3034 | | Spinner `0x612CE0C3` | `0x0096017E` | 2115 |
| LineInput `0x21335C59` | `0x0095DB1A` | 1736 | | Gen `0x5386D516` | `0x00960D07` | 1767 |
| Combo `0x412CE496` | `0x0095E1E2` | 1294 | | OptGrp `0xA1336CC0` | `0x0095F9B0` | 1998 |
| TreeView `0x3AE8BAD2` | `0x00963688` | 4467 | | Folders `0xA13351B8` | `0x00962E9F` | 291 |
| FileBrowser `0x3AA52E64` | `0x009613EE` | 6833 | | Outline `0xE303E5AE` | `0x009609C1` | 624 |
| TextEdit `0x231A1C57` | `0x0095CC7B` | 1982 | | Grid `0xDAA6B9BE` | `0x009647FB` | 5382 |
| ListBox `0x4132242B` | `0x0095D439` | 1761 | | | | |

Value helpers the writers share (each appends `word=` + value + space): hex `0x0095BE2E` (`0x%08x`), quad
`0x0095BF21` (`(%d,%d,%d,%d)`), colour `0x0095BC92` (`(%u,%u,%u)`), resource key `0x0095BE7D`
(`{%08x,%08x}`), quoted text `0x0095C028` (`"%s"`), yes/no `0x0095BFCB` (`yes` when the byte argument is
non-zero, else `no`).

Worked example: a `GZWinText` built by `SetParameters` (parameters `0xE85EEBDE = 0x592`,
`0x08D6E462 = 0x212CDC1F`), ID `0x1234`, area (0,0,100,20), fill (255,255,255), caption `OK`, no `captionres`,
empty comments, `GetFlag` true for `0x1` and `0x2` only, gives

`<LEGACY clsid=GZWinText iid=IGZWinText id=0x00001234 area=(0,0,100,20) fillcolor=(255,255,255) caption="OK" winflag_visible=yes winflag_enabled=yes winflag_moveable=no winflag_sizeable=no winflag_sortable=no winflag_pbuff=no winflag_pbufftrans=no winflag_pbufferase=no winflag_pbuffvid=no winflag_alphablend=no winflag_acceptfocus=no winflag_mousetrans=no winflag_ignoremouse=no ` + the Text writer's words + `>` + LF.

A control with neither parameter that answers only `IGZWin` gives `clsid=GZWin iid=IGZWin `; one whose stored
class is `0x12345678` (not in the table) gives `clsid=0x12345678 `.

## 8. The common interpreter (`0x0095660D`)

`InterpretPropertiesGZWinCommon(win, props, firstPass)`. `SetFlag(mask, bool)` is `cIGZWin` `+0x110`; flag
bits are those of `widget-toolkit.md` §3. A "flag word" below is read with `Uint32` and applied as
`value != 0`.

**First pass (`firstPass = true`)**, in this order:

1. Defaults: `SetFlag(0x1, 1)`, `SetFlag(0x10000, 1)`, `SetFlag(0x800, 0)`, `SetFlag(0x20000, 1)`,
   `SetFlag(0x40000, 1)`, `SetFlag(0x4, 0)`, `SetFlag(0x8000, 1)`.
2. Flag words, each only if present: `winflag_visible` (`0xF01A`) → `0x1`; `winflag_moveable` (`0xF01C`) →
   `0x100`; `winflag_sizeable` (`0xF01D`) → `0x200`; `winflag_sortable` (`0xF01E`) → `0x800`;
   `winflag_acceptfocus` (`0xF025`) → `0x8000`; `winflag_alphablend` (`0xF026`) → `0x4`; `winflag_mousetrans`
   (`0xF023`) → `0x80000`; `winflag_ignoremouse` (`0xF024`) → `0x200000`.
3. `id` (`0x100`, Uint32) → `SetID(id)` (`+0x100`).
4. Geometry. If `area` (`0x101`, String) is present: parse it with the pattern `( %d, %d, %d, %d)` into a
   4-int rectangle and call `SetArea(&rect)` (`+0xD8`). The parse count is **not checked**: fields the text
   does not supply keep indeterminate values (implementations: use 0 and file a question if a script relies on
   it). If `area` is absent: `pos` (`0x102`) and then `size` (`0x103`), each parsed by the coordinate reader
   (String, pattern `(%d,%d)`, accepted only when both numbers parse), call `GZWinMoveTo(x, y)` (`+0xE0`) and
   `SetSize(w, h)` (`+0xD4`) respectively. With `area` present, `pos` and `size` are ignored.
5. `fillcolor` (`0x104`): colour reader (String, pattern `(%d,%d,%d)`, accepted only when all three parse;
   each component truncated to its low byte) → `SetFillColor(r, g, b)` (`+0x1AC`).
6. `caption` (`0x105`, String) → `SetCaption(text)` (`+0x128`).
7. `captionres` (`0x106`, String, pattern `{%x,%x}` → group, instance; unparsed parts stay 0): load the string
   resource (helper `0x0094C58D`, through the UI-script service and the resource manager); if it loads,
   `SetCaption(loaded)` (overriding step 6). Whether or not it loaded, store the resource key as parameter
   `0x0B31771A` (a 12-byte variant of type `0x8000` holding type `0x2026960B`, group, instance; helper
   `0x0094C1A4`). Then, **always** (with or without `captionres`), store parameter `0x3ACA6906` = an 8-byte
   variant (type `0x8000`) holding group, instance ({0, 0} when `captionres` is absent).
8. `transparent` (`0x107`): if the key is present at all (any value, `HasProperty`), `SetFlag(0x20000, 1)`.
9. Comments: always (even when none are given) build a comment list of exactly 8 strings, in this order:
   `comments` `0x108`, `commentslmd` `0x109`, `commentsrmd` `0x10A`, `commentslmu` `0x10B`, `commentsrmu`
   `0x10C`, `commentsmm` `0x10D`, `commentsku` `0x10E`, `commentskd` `0x10F` (missing → empty string). The list
   object (20 bytes, vtable `0x00AD5420`) is wrapped in a new variant (variant slot `+0xF4`) and stored as
   parameter `0xBACA6879`; this function then drops its own references to both.

**Second pass (`firstPass = false`)**: only five flag words, each if present: `winflag_enabled` (`0xF01B`) →
`0x2`; `winflag_pbuff` (`0xF01F`) → `0x10000`; `winflag_pbufftrans` (`0xF020`) → `0x20000`;
`winflag_pbufferase` (`0xF021`) → `0x40000`; `winflag_pbuffvid` (`0xF022`) → `0x100000`. Nothing else.

Consequences: the private-buffer flags and `enabled` are applied after `Init`, attach and `PullToFront`
(§6 steps 10–12); everything else before. The common words `bkgcolor`, `forecolor`, `notify`, `gutters`,
`style`, `font`, the alignment and image words and the `colorfont*` words are **not** read by the common
interpreter; they are read (if at all) by the per-type interpreters (§9).

Worked example: `winflag_visible=no, area=(0,0,100,20), caption=OK, winflag_enabled=no` gives, first pass,
the seven defaults, then `SetFlag(0x1, false)`, `SetArea(0,0,100,20)`, `SetCaption("OK")`, the 8-empty-string
comment list, and parameter `0x3ACA6906` = {0, 0}; second pass `SetFlag(0x2, false)`.

## 9. Per-type interpreters

| Interpreter | Address | Size | State |
|---|---|---|---|
| GZWinCustom | `0x00A6D837` | 1 | **specified**: does nothing (a bare return, ICF-shared) |
| GZWinSlider | `0x00950218` | 403 | **specified** (§9.1), slot names on the slider interface *[inferred]* |
| GZWinTreeView | `0x009586B0` | 4857 | **specified** (§9.2) |
| GZWinFileBrowser | `0x00954212` | 3761 | **specified** (§9.3) |
| GZWinGrid | `0x0095A250` | 3537 | **specified** (§9.5) |
| GZWinFolders | `0x0094D17E` | 465 | **specified** (§9.6) |
| GZWinFlatRect | `0x0094E33B` | 475 | **specified** (§9.7) |
| GZWinOutline | `0x00951084` | 485 | **specified** (§9.8) |
| all other 12 | §4.1 | 492–2629 | open (LC4) |

Every per-type interpreter starts the same way: `QueryInterface` the control for its own IID (§4.1); on failure
it does nothing at all; on success it works through that interface and releases it at the end.

### 9.1 GZWinSlider (`0x00950218`)

Interface `IGZWinSlider` `0x21325207`, a sub-object at control `+0xD8` (vtable `0x00AE049C`; the export has no
slot names, so the names below are inferred from the arguments and are marked so).

First pass:

1. `minmaxvalue` (`0xC00`, coordinate reader `(%d,%d)`, both numbers required): order the pair so that
   `lo ≤ hi` (signed compare; equal values unchanged), then call slot `+0x20` with `hi` (*SetMaximum*) **before**
   slot `+0x18` with `lo` (*SetMinimum*).
2. `direction` (`0xC01`, Uint32): value `0xC03` (`vertical`) → slot `+0x28` with 2; **any other value**
   (including `horizontal` `0xC02`) → 1.
3. `image` (`0xF016`, String, pattern `{%x,%x}` → group, instance; both required): load the image buffer
   (helper `0x0094C4E0`); if a buffer is returned: `imagerect` (`0xF018`, rectangle reader `0x0094D5CA`) gives a
   rectangle or, when absent, null; call slot `+0x30` (*SetImage*) with (buffer, rect-or-null); store the
   resource key as control parameter `0x6B4BCBD4` (type `0x856DDBAC`, group, instance; same helper as §8 step 7);
   store the image key as parameter `0xDAA3B860` (helper `0x0094C1DA`, n = 0, §9.4); release the
   buffer. No buffer → nothing.

Second pass: `initvalue` (`0x40A`, read with the Sint32 reader, so it must still be a type-6 variant) → slot
`+0x14` (*SetValue*). The initial value is applied after the range, so it is clamped by a range set in the
first pass if the control clamps (control behaviour, not specified here).

Worked example: `minmaxvalue=(100,0), direction=vertical, initvalue=30` → first pass `+0x20(100)`,
`+0x18(0)`, `+0x28(2)`; second pass `+0x14(30)`.

### 9.2 GZWinTreeView (`0x009586B0`)

Interface `IGZWinTreeView` `0x3AE8BAD2`. Slot names are inferred from arguments *[inferred]*; "colour(r,g,b)"
below is the graphics-system colour constructor (`RZ::GS()` slot `+0x88`, three ints). A node handle of 0 is
the root node. "Store parameter K = map" means: wrap the map pointer in a new 20-byte variant and call the
control's `SetParameter(K, variant)` (`cIGZWin` `+0x1C8`); the map is handed over, not freed here.

**First pass** (each only if present; flags are `value != 0`):

| Word | ID | Reader | Effect |
|---|---|---|---|
| `displayroot` | `0x1401` | Uint32 | slot `+0x0C` (*SetStyleBit*) (2, flag) |
| `displaynodeicons` | `0x1402` | Uint32 | `+0x0C` (4, flag) |
| `displaytreelines` | `0x1403` | Uint32 | `+0x0C` (8, flag) |
| `multiselect` | `0x1404` | Uint32 | `+0x0C` (0x10, flag) |
| `icongutter` | `0x1405` | Uint32 | `+0x1C` (value) |
| `rowheight` | `0x1336` | Uint32 | `+0x2C` (value) |
| `minmaxboxsize` | `0x1406` | Uint32 | `+0x34` (value) |
| `backgroundcolor` | `0x133F` | colour `(%d,%d,%d)` | `+0x3C` (colour) |
| `highlightcolor` | `0x408` | colour | `+0x44` (colour) |

`rowheight`, `backgroundcolor` and `highlightcolor` are words of other vocabularies (all words share one ID
space, §4), read here by ID.

**Second pass**, in this order:

1. **Image table.** Create three empty maps: *NameToIcon* (string → icon index), *IconToName* (icon index →
   string), *NameToKey* (string → resource key {group, instance}). For **every** property pair with key
   `image` (`0xF016`) whose value is a string variant (`0x800E`), in list order (repeats allowed): split the
   text at `:` into *name* `:` *key* `:` *rect* (the rect part is the rest of the text). *key* is parsed with
   `{%x,%x}`; only if both numbers parse is the image buffer loaded (group, instance). Insert *name* →
   {group, instance} into NameToKey **whether or not** it loaded (unparsed numbers 0). *rect* is parsed
   with `(%d,%d,%d,%d)` into a zeroed rectangle (count not checked). If a buffer loaded: rectangle = the
   parsed one, or, when all four fields are 0, the buffer's own full rectangle (buffer slot `+0x30`); call tree
   slot `+0x50` (*AddIcon*) (buffer, rect) → icon index; insert *name* → index into NameToIcon and index →
   *name* into IconToName; release the buffer. Then store parameters `0x46C207A5` = NameToIcon,
   `0xC6C207C8` = IconToName, `0x66C20802` = NameToKey (always, even when empty).
2. **Root node** (handle 0), each only if present:
   - `roottext` `0x1407` (String) → `+0x80` (0, text).
   - `rooticon` `0x1408`, `rooticonselected` `0x1409`, `rooticonexpanded` `0x140A`,
     `rooticonexpandedselected` `0x140B` (String: an image name) → `+0x90` (0, state, index) with state
     0, 1, 2, 3 respectively; index = NameToIcon[name], or **the state number itself** when the name is unknown.
   - `rootcolorforeground` `0x140C`, `rootcolorbackground` `0x140D`, `rootcolorforegroundhilited` `0x140E`,
     `rootcolorbackgroundhilited` `0x140F` (colour) → `+0xAC` (0, which, colour) with which 0, 1, 2, 3.
   - `rootfont` `0x1410`: if it reads as Uint32 (type 6): font = window-class manager (`RZ::WCM()`) slot
     `+0x14` (id); tree `+0x9C` (0, font); store the id as control parameter `0xFAA3BE85` (helper
     `0x0094C222`, parameter = `0xFAA3BE85` + index 0). Else if it reads as String: **only** store the text as
     parameter `0xFAA3BE85` (helper `0x0094C255`); the root font is not changed.
   - `rootexpanded` `0x1411` (Uint32) → `+0xB4` (0, value == 1).
   - `rootselected` `0x1412` (Uint32): only when value == 1 → `+0xE0` (0, 0).
3. **Always**: `+0x24` (colour constructor called with (0, 0, 0, 1)).
4. **Node records.** Create an empty map *NodeFont* (node id → font id). For every pair with key `node`
   (`0x1102`, a word of the Outline vocabulary) whose value is a string, in list order, split at `:` into 15
   fields:

   | # | Field | Rule |
   |---|---|---|
   | 1 | id | text |
   | 2 | text | text shown |
   | 3 | parent | id of the parent node; `Root` (exact, case-sensitive) = the root |
   | 4 | position | `first` → −2, `last` → −3, `sorted` → −4 (case-sensitive exact); **any other text leaves the field as the previous node record had it** (indeterminate for the first; quirk Q5) |
   | 5–8 | icon normal, selected, expanded, expanded-selected | image names → NameToIcon, unknown → 0, 1, 2, 3 |
   | 9–12 | fg, bg, fg hilited, bg hilited | `(%d,%d,%d)` (unparsed components 0) → colour |
   | 13 | font | font name → WCM slot `+0x98` (name) = font id; WCM `+0x14` (id) = font; insert id → font id into NodeFont |
   | 14 | expanded | exactly `yes` → true, else false |
   | 15 | selected | the remaining text, exactly `yes` → true |

   Records are appended to a pending list. Store parameter `0x86C21DA0` = NodeFont.
5. **Tree build (depth-first).** Create *IdToHandle* (string → handle) and *HandleToId*. A stack starts with
   one entry (`Root`, handle 0). Loop while the stack is not empty: look at the **top** entry (not popped); find
   the first pending record (list order) whose *parent* equals the top's id; if none, pop and continue.
   Otherwise: handle = tree `+0x60` (top handle, position, text, 0); then `+0x90` (handle, s, icon s) for s =
   0..3; `+0x9C` (handle, font); `+0xAC` (handle, w, colour w) for w = 0..3; `+0xB4` (handle, expanded);
   if selected, `+0xE0` (handle, 0); insert id → handle and handle → id; push (id, handle); remove the record
   from the pending list. Records whose parent is never inserted are silently dropped. Store parameters
   `0xE6C21D83` = IdToHandle and `0x26C21E35` = HandleToId.

Worked example: `image=folder:{1234,1}:(0,0,0,0)`, `node=a:Alpha:Root:last:folder:::::::::no:no`,
`node=b:Beta:a:first::::::::::no:yes`: the image loads with its full rectangle and gets index *i*; node `a`
is inserted under 0 at −3 with icons (*i*, 1, 2, 3), node `b` under `a` at −2 with icons (0, 1, 2, 3) and
is selected. Listing `b` before `a` gives the same tree (the search restarts from the list head per step).

### 9.3 GZWinFileBrowser (`0x00954212`)

Interface `IGZWinFileBrowser` `0x3AA52E64`. **First pass only**; the second pass does nothing (beyond the
QueryInterface/Release). Slot names *[inferred]*. "Alignment code": `unused` `0x1317` → 0, `default` `0x1318`
→ 1, `left` `0xF006` → 2, `center` `0xF008` → 3, `right` `0xF007` → 4; any other value → the setter is not
called. "Font word": if it reads as Uint32, font = `RZ::WCM()` slot `+0x14` (id), call the font setter, and
store the id as control parameter `0xFAA3BE85 + n`; else if it reads as String, only store the text as that
parameter (helpers `0x0094C222` / `0x0094C255`); the setter is not called. Colour words use the colour reader
and the colour constructor of §9.2; the six colour indices are 0 fg, 1 bg, 2 fg disabled, 3 bg disabled,
4 fg hilited, 5 bg hilited.

In this order:

1. `defaultpath` `0x1301` (String) → `+0x3C` (text).
2. `defaultselectionname` `0x1306` and `defaultselectionextension` `0x1307` are read as strings (empty when
   absent). `defaultselectiontype` `0x1305` (Uint32): `backdir` `0x1302` → 0, `dir` `0x1303` → 1, `file`
   `0x1304` → 2 → `+0x70` (type, name, extension); other values or absent → not called.
3. File sort: read the current pair with `+0x88` (&primary, &secondary); `filesortprimary` `0x130C` and
   `filesortsecondary` `0x130D` each override with `bynameascending` `0x1308` → 0, `bynamedescending`
   `0x1309` → 1, `byextensionascending` `0x130A` → 2, `byextensiondescending` `0x130B` → 3 (other values keep
   the current); then **always** `+0x8C` (primary, secondary).
4. `directorysort` `0x1312`: `topascending` `0x130E` → 0, `topdescending` `0x130F` → 1, `bottomascending`
   `0x1310` → 2, `bottomdescending` `0x1311` → 3 → `+0x94` (code); other values → not called.
5. Columns: read `+0x98` (&c1, &c2, &c3); `column1data` `0x1319`, `column2data` `0x131A`, `column3data`
   `0x131B` each override with `icon` `0x1313` → 0, `name` `0x1314` → 1, `extension` `0x1315` → 2, `empty`
   `0x1316` → **5**; then **always** `+0xA0` (c1, c2, c3).
6. File name: font word `filenamefont` `0x131C` (n = 0, setter `+0xB4` (font)); colours `0x131D`–`0x1322`
   → `+0xB8` (index 0–5, colour); `filenamealignment` `0x1323` → `+0xBC` (alignment code).
7. File extension: `fileextensionfont` `0x1324` (n = 1, setter `+0xCC`); colours `0x1325`–`0x132A` → `+0xD0`;
   `fileextensionalignment` `0x132B` → `+0xD4`.
8. Drive name: `drivenamefont` `0x132C` (n = 2, setter `+0xE4`); colours `0x132D`–`0x1332` → `+0xE8`;
   `drivenamealignment` `0x1333` → `+0xEC`.
9. `iconcolumnwidth` `0x1334` → `+0xF4`; `extensioncolumnwidth` `0x1335` → `+0xFC`; `rowheight` `0x1336` →
   `+0x104` (Uint32 values).
10. Line grid: read `+0x108` (&enabled, &colour); `linegridenabled` `0x1337` (flag) and `linegridcolor`
    `0x1338` override; **always** `+0x10C` (enabled, colour). Column grid likewise: `+0x110`, `columngridenabled`
    `0x1339`, `columngridcolor` `0x133A`, always `+0x114`.
11. Highlight: read `+0x118` (&fg, &bg); `highlightcolorforeground` `0x133B`, `highlightcolorbackground`
    `0x133C` override; always `+0x11C` (fg, bg).
12. `backdirtext` `0x133D` (String) → `+0x134`; `drivelistsize` `0x133E` → `+0x144`; `backgroundcolor`
    `0x133F` → `+0x14C` (colour); `backgroundtransparent` `0x1341` → `+0x15C` (flag); `outline` `0xF019` →
    `+0x13C` (flag).
13. `backgroundimage` `0x1340` (String `{%x,%x}`, both required): load the buffer; if loaded, `+0x154`
    (buffer), store image key parameter n = 0 (§9.4 helper), release.
14. Filters: for every pair at list position *k* (0-based) with key `filter` `0x1342` and a string value, in
    order: split at `:` into *text* `:` *number* `:` *image*; *number* parsed with `%d` (default 0); if
    *image* parses as `{%x,%x}` (both), load the buffer (may fail → null) and store the image key parameter
    with n = **k + 1** (the pair's list position plus one, not a filter counter); then **always**
    `+0x54` (text, buffer-or-null, number, 0); release the buffer if any.

### 9.5 GZWinGrid (`0x0095A250`)

Interface `IGZWinGrid` `0xDAA6B9BE`; slot names *[inferred]*. Colours here are built by the grid's own window
(grid slot `+0x14` returns it) slot `+0x1B0` (r, g, b[, 0]), not by `RZ::GS()`.

**Second pass**: only grid-line colours. `colgridclr` `0x1623` and `rowgridclr` `0x1624` (colour reader;
missing → (0,0,0)) → **always** `+0xE8` (column colour, row colour). Nothing else.

**First pass**, in this order:

1. `fontcolor` `0x1601` (colour) → `+0x44` (colour).
2. `font` `0xF000`: Uint32 → `+0x3C` (WCM font(id)) and store parameter `0xFAA3BE85`; else String → only the
   parameter (§9.4); **absent → `+0x3C` (WCM font(`0x68963C4C`))**, a default font.
3. `textalign` `0x1602` (Sint32) → `+0xF0`. `textwrapping` `0x1603` → **always** `+0x54` (present and ≠ 0).
4. `gutters` `0xF004` (rectangle reader) → `+0x60` (l, t, r, b), **absent → `+0x60` (0, 0, 0, 0)**.
5. `selrule` `0x1605` (Sint32, default 2), `maxselcount` `0x1606` (default 1) → always `+0x9C` (rule, max).
6. Outline: `fdoutline` `0x1616` read as Uint32 (default 0) and `olinecolor` `0x1604` (default black) →
   always `+0x6C` (value == 1, colour).
7. `dcolwidth` `0x1607` (Sint32, default 100) → always `+0x100`; `drowheight` `0x1608` (default 20) →
   always `+0x108`.
8. `enablehdr` `0x1609` (Uint32) is read **into the column-width variable**, so when absent it holds the
   `dcolwidth` value (quirk Q6). Always `+0x128` (a, b): value 0 → (1, 1); 1 → (1, 0); 2 → (0, 1); any other →
   (0, 0).
9. `colhdrsz` `0x160A`, `rowhdrsz` `0x160B` (Sint32, default 0) → always `+0x130` (col, row).
10. Style flags, each only if present (Sint32): value ≠ 0 → `+0x28` (bit) (*SetStyle*), value 0 → `+0x30`
    (bit) (*ClearStyle*). Bits: `fnone` 0, `fedit` 1, `fhscroll` 2, `fvscroll` 4, `fcolheading` 8,
    `frowheading` 0x10, `fcolgrid` 0x20, `frowgrid` 0x40, `finsmode` 0x80, `fopqbkgnd` 0x100, `fdoutline`
    0x200, `fdgridoutline` 0x400, `fcellwrap` 0x800, `fallownosel` 0x1000, `ffixcolcnt` 0x2000, `ffixrowcnt`
    0x4000, `fdpastlastcol` 0x8000, `fdpastlastrow` 0x10000, `fcelloverlap` 0x20000, `fdrpdnmenu` 0x40000,
    `fhlcelldata` 0x80000, `fdefault` 0x19BFF, `fall` 0xFFFFFFFF (IDs `0x160C`–`0x1622` in that order).
    Clearing `fvscroll` additionally calls the grid window's slot `+0x170` (no arguments) after `+0x30`.
11. Cells: every pair with key `wingridcell` `0x1625` and a string value, in list order. The text is split
    at `,` into at most 11 fields (parser `0x00956464`); **fewer than 4 fields → the entry is skipped**.
    Fields: 1 column, 2 row (decimal), 3 cell flags (hex), 4–5 data (rejoined as `f4,f5`), 6 font name
    (→ WCM `+0x98` font id), 7–11 five numbers (cell style values). Before the first accepted cell, a
    cell-notifier object (16 bytes, vtable `0x00AD538C`, wrapping a new 36-byte object from `0x009ABB63`) is
    stored as parameter `0x287A7136`. Per accepted cell, with F = field 3 (byte-checked against the
    instructions `0x0095AA02`–`0x0095AC59`; answers LC7):
    1. Data scan: if bit `0x40000` of the **style-word scratch** is clear (Q10), the data text is scanned as
       `%08x,%08x` into (group, instance). The two values are not reset per cell: fields that do not parse keep
       the previous cell's values.
    2. A fresh cell descriptor is built each cell (all zero, empty text) and its third word is set to F.
    3. If F has `0x40000`: the data text itself is the cell text; call `+0x1C0` (col, row, text, length,
       1, wrap) with wrap = 1 when F has `0x80000`, else 0. Steps 4–5 are skipped; the descriptor's group and
       instance stay 0.
    4. Else the descriptor takes (group, instance). If F has `0x2000`: load the image buffer of key
       (type `0x856DDBAC`, group, instance) (helper `0x0094C4E0`); if it loads, `+0x1C8` (col, row, buffer, 1),
       then release the buffer. A failed load sets nothing.
    5. Else (no `0x2000`) if F has `0x10000`: load the string resource (group, instance) (helper `0x0094C58D`);
       if it loads, `+0x1C0` (col, row, text, its length, 1, wrap as in 3). Neither `0x2000` nor `0x10000`:
       no content call.
    6. Independently of 3–5, if F has `0x1000`: a cell-data record is built from its default constructor
       (`0x009A5BF2`): font slot `+0x00` = WCM `+0x14` (field 6 font id) only when that id is non-zero; `+0x04` =
       field 7 (32-bit); `+0x14` = alignment byte, 1 when F has `0x100000`, else −1 (`0xFF`); `+0x15`, `+0x16`,
       `+0x17` = low bytes of fields 8, 9, 10; `+0x18` = field 11 (32-bit); then `+0x154` (col, row, &record).
    7. Always: helper `0x0095A0F3` on the cell notifier (col, row, &descriptor, 0).
    Col and row are fields 1 and 2 in that argument order. The meaning of record fields 7–11 beyond these
    offsets belongs to the grid control spec.
12. Columns: every `wingridcol` `0x1626` string `%d,%d,%d` (all three required; else skipped): before the
    first, a notifier (vtable `0x00AD5398`) is stored as parameter `0xC88CCB6D`; helper `0x009583C5`
    (first, 0, …) then `+0x104` (first, **1**, third): only one column is sized, the second number is
    ignored (quirk Q7).
13. Rows: every `wingridrow` `0x1627` `%d,%d,%d`: notifier (vtable `0x00AD53A4`) as parameter `0xC88CCB6E`;
    helper `0x009584E0` (0, first, …) then `+0x10C` (first, second − first + 1, third): rows first..second
    get height third.

Worked example: `wingridrow=2,4,18` → `+0x10C(2, 3, 18)`; `wingridcol=2,4,60` → `+0x104(2, 1, 60)`.

### 9.4 Image-key parameter helper (`0x0094C1DA`) (answers LC6)

`StoreImageKey(win, group, instance, n)` (group in a register, instance and n on the stack): wraps
{group, instance} in a new 8-byte variant of type `0x8000` and calls `win.SetParameter(0xDAA3B860 + n,
variant)` (`+0x1C8`); returns true. It does not depend on whether the image loaded. The Slider (§9.1) calls it
with n = 0 after a successful image load; FileBrowser uses n = 0 for the background and k + 1 for filters.
The font counterpart (`0x0094C222` Uint32 / `0x0094C255` String) stores the value as parameter
`0xFAA3BE85 + n`.

### 9.6 GZWinFolders (`0x0094D17E`)

Interface `IGZWinFolders` `0xA13351B8`. It has no words of its own (§4.1); it reads two common words and the
ListBox word `listelement`. Slot names on the folders interface are *[inferred]*.

**First pass:**

1. `font` (`0xF000`): if it reads as Uint32 (id): font = `RZ::WCM()` slot `+0x14` (id); call interface `+0x28`
   (font); store the id as control parameter `0xFAA3BE85` (helper `0x0094C222`, index 0). Else if it reads as
   String: only store the text as parameter `0xFAA3BE85` (helper `0x0094C255`); `+0x28` is not called.
2. `image` (`0xF016`, String): scan `{%x,%x}` into (group, instance) **without checking the count**; load the
   image buffer (helper `0x0094C4E0`, type `0x856DDBAC`); if it loads, interface `+0x2C` (buffer). Whether or
   not it loaded, store parameter `0xDAA3B860` = 8-byte type-`0x8000` variant {group, instance} (the §9.4 key
   rule, index 0); then release the buffer if any. Quirk Q11: an unparsed group keeps the value of the
   variable it shares (the font id from step 1, else the window argument pointer) and an unparsed instance is
   uninitialised; implementations use 0.

**Second pass:** call interface `+0x1C` (no arguments; a reset/clear *[inferred]*) once, then for every pair with
key `listelement` (`0x502`) and a string value (`0x800E`), in list order (repeats allowed), copy the text and
call interface `+0x10` (text, 0).

Worked example: `font=3, image={1abe787d,cafe0001}, listelement=A, listelement=B` → first pass `+0x28`(WCM
font 3), parameter `0xFAA3BE85` = 3, `+0x2C`(buffer) if the image loads, parameter `0xDAA3B860` =
{`0x1ABE787D`, `0xCAFE0001`}; second pass `+0x1C`(), `+0x10`("A", 0), `+0x10`("B", 0).

### 9.7 GZWinFlatRect (`0x0094E33B`)

Interface `IGZWinFlatRect` `0xC2AFA76F`. **First pass only**; the second pass does nothing beyond the
QueryInterface/Release. Slot names *[inferred]*.

1. Style byte: start at 0; for every pair with key `style` (`0xF005`), a non-null value and variant type 6, in
   list order, OR in the bits of its value (byte-checked at `0x0094E3DF`–`0x0094E428`): `left` `0xF006` → `0x01`,
   `top` `0x204` → `0x02`, `right` `0xF007` → `0x04`, `bottom` `0x205` → `0x08`, `all` `0x206` → `0x0F`, `lrb`
   `0x207` → `0x0D`, `trb` `0x208` → `0x0E`, `raised` `0xF00C` → `0x10`, `sunken` `0xF00B` → `0x20`, `nofill`
   `0xF00E` → `0x40`; any other value adds nothing. Then **always** call interface `+0x30` (style byte,
   zero-extended), even with no `style` pair.
2. Edge colours, each with the colour reader of §8 step 5 and only when it parses: `colorleft` `0x200`,
   `colortop` `0x201`, `colorright` `0x202`, `colorbottom` `0x203` → interface `+0x28` (edge, r, g, b) with edge
   0, 1, 2, 3 respectively.

Worked example: `style=top, style=bottom, style=sunken, colortop=(10,20,30)` → `+0x30`(`0x2A`), `+0x28`(1, 10,
20, 30).

### 9.8 GZWinOutline (`0x00951084`)

Interface `IGZWinOutline` `0xE303E5AE`. **First pass only**; the second pass does nothing beyond the
QueryInterface/Release. Slot names *[inferred]*.

1. Style: start at 0; for every `style` (`0xF005`) pair with a non-null type-6 value, OR in: `flat` `0xF009` →
   `0x1`, `sunken` `0xF00B` → `0x2`, `raised` `0xF00C` → `0x4`, `fill` `0xF00D` → `0x20`; other values add
   nothing. Only if the result is non-zero, call interface `+0x0C` (style). (Unlike FlatRect, no call for 0.)
2. `colordark` `0x1100`, then `colorlight` `0x1101`: colour reader of §8 step 5; when it parses, interface
   `+0x14` (which, r, g, b) with which = 1 for dark, 2 for light.
3. Points: every `node` (`0x1102`) pair with a non-null string value (`0x800E`), in list order, is scanned as
   `(%d,%d)` into one (x, y) pair of a 32-entry array; **at most 32** are taken (later `node` pairs are
   ignored). The scan count is not checked: unparsed numbers keep the previous values of the two shared
   variables (the last colour components read in step 2, else uninitialised; Q12; implementations use 0).
   If at least one point was taken, call interface `+0x34` (count, &points, 1, 1).

Worked example: `style=raised, style=fill, colordark=(0,0,0), node=(0,0), node=(10,0), node=(10,10)` →
`+0x0C`(`0x24`), `+0x14`(1, 0, 0, 0), `+0x34`(3, [(0,0),(10,0),(10,10)], 1, 1).

## 10. Quirks (keep for compatibility)

- Q1. `SetParameters` overwrites `+0x10` with null without releasing a previous control; calling it twice on one
  object leaks the first control (and its own-control detach is lost).
- Q2. `SetParameters` always returns false.
- Q3. A failed exact create falls back to the lowest-numbered registered class with the same interface, then to
  `GZWinCustom`; the stored `0xE85EEBDE` parameter still holds the requested class ID.
- Q4. `Release` with a count of 0 destroys the object.
- Q5. TreeView `node` records: a position field other than `first`/`last`/`sorted` is not reset, so it keeps the
  previous record's value (stack garbage for the first record). Implementations: use −3 (`last`) for the first.
- Q6. Grid `enablehdr` shares its variable with `dcolwidth`: when absent, the header mode is the column width
  (100 by default → (0, 0)).
- Q7. Grid `wingridcol=a,b,w` sizes only column a; b is ignored (rows use the a..b range).
- Q8. Script fragment: with only one of the class/interface parameters stored, the other comes from the first
  table row the control answers (§7.6).
- Q9. Script fragment `fillcolor`: upper 24 bits of each component are uninitialised stack in the original (§7.6).
- Q10. Grid cells: the "scan the data as a resource key" test reads bit `0x40000` of the shared style-word scratch
  (the value of the last present grid style word, `0x0095A932`), not of the cell's flags; style values are
  normally 0/1, so the scan effectively always runs. Uninitialised when no style word is present.
- Q11. Folders `image`: the `{%x,%x}` scan count is not checked; unparsed parts are stale or uninitialised (§9.6).
- Q12. Outline `node`: the `(%d,%d)` scan count is not checked; unparsed numbers are stale (§9.8).

## 11. Open questions

- LC8. Word sets of the 19 per-type script-fragment writers (§7.6 table; not owner functions).
- LC4. Interpreter effect tables (§8, §9).
- LC5. Window manager slot 3 (`+0x0C`) used as the default parent: confirm it is the root/main window.

## 12. Coverage of owner functions (honest)

Owner functions: 57 in `re/names_windows.tsv` (20 `InterpretProperties*`, 21 `ControlVocabulary*`, 16 others including
the constructor, the `SetOwnControl` thunk and the ICF-folded `ImplementInterface`; `AddRef`/`GetControl` and the
Custom interpreter are shared bodies owned elsewhere).

| Group | Functions | State |
|---|---|---|
| COM surface, accessors, lifecycle (`QueryInterface`, `Release`, destructor, `GetControlVocabulary`, `SetOwnControl` + thunk, `SetParent`, `UseWinProc`, `ForceControl`, `GetID`, `SetID`, `DoExceptions`, constructor, `ImplementInterface`) | 14 | specified (§3, §5, §7) |
| `SetParameters` | 1 | specified (§6) |
| `GetScriptFragment` | 1 | specified (§7.4; serialiser frame, class/interface and common words §7.6; per-type writer word sets are non-owner functions, LC8) |
| `ControlVocabulary*` | 21 | specified (§4: every word, ID and map entry) |
| `InterpretProperties*` | 21 | Common (§8), Slider (§9.1), TreeView (§9.2), FileBrowser (§9.3), Grid (§9.5), Folders (§9.6), FlatRect (§9.7), Outline (§9.8), Custom (shared stub) specified; 12 owned interpreters open (LC4) |

Count: 45 of 57 owner functions complete (14 COM/lifecycle + `SetParameters` + `GetScriptFragment` + 21 vocabulary +
Common, Slider, TreeView, FileBrowser, Grid, Folders, FlatRect, Outline); 12 interpreters open (LC4). No wildcard credit.
