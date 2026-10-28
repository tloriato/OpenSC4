# Driver and UI input captures

`draw_capture = 1` enables the plugin's default-off recorder. It patches only
live, version-checked vtables; it never writes the executable on disk. All 83
`cIGZGDriver` slots of the DX7, OpenGL and software tables are tapped before
city loading. Native calls still run with their original arguments and return
registers. This is an input-call oracle, not an image or core/native equivalence
verdict. Disable replacement classes for a standalone-game reference.

The primary JSONL stream is `<LogPath()>.draw.jsonl`, beside the game's log.
The run and queue launchers preserve it as `<label>.draw.jsonl` with the copied
log. `draw_capture_export = <absolute file path>` additionally copies the
completed stream before a slot is reused, including with older launchers.

Example ini (the usual harness supplies the city path and quit timeout):

```ini
disable = simulator, budget
schedule = fixed
session_seed = 33
autotest_unpause = 0
popup_dialogs = 0
draw_capture = 1
draw_capture_scenarios = 1
draw_capture_frames = 4
draw_capture_warmup = 30
```

Each framework `OnTick` is a frame, matching simulator.md 5.8. After a city
loads and exits its hidden pause, the tool pauses it, sets a camera and waits
the configured number of warmup ticks. It requests full redraws through the
public renderer. The sweep captures zooms 1–5, each at rotations 0 and 1, at
the loaded save's view position. A single-camera capture instead uses
`draw_capture_scenarios = 0`, `draw_capture_zoom` (default 3) and
`draw_capture_rotation` (default 0). Defaults are 4 captured and 30 warmup
ticks per camera. Keep the game visible (`minimize = 0` in queue requests).
Do not move the mouse or send input during a paired run.

Camera setup calls the public UI camera wrapper, including its zoom-button
updates. Camera and refresh operations start a 2000-ms hidden pause
(view3d-window.md 6f); under the observed paused fixed schedule that timer did
not expire. This additional simulation hold does not hide the window or stop
drawing. The recorder captures its flag instead of waiting for it to expire.
An explicit simulator pause remains required. Nested modal ticks during
setup are excluded from scenario advancement.

```text
python tools/drawcheck.py build-runs/a.draw.jsonl build-runs/b.draw.jsonl
```

Exit 0 means identical validated streams; 1 prints the first difference
(frame, call index, method, scalar argument or buffer); 2 means invalid or
incomplete evidence. `--json` provides the same result as JSON. Both complete
inputs are validated even when an early difference is found. Empty frames,
missing completion records, unreadable buffers and unknown buffer extents
cannot establish equality. The tool uses only the Python standard library.

`fc4.draw/1` starts with a header, followed by `frame`, preorder `ui`, `call`
and `end_frame` records, and ends with counters in `end`. Scalars are exact
32-bit argument words, including float/double bit patterns. Buffer pointers
are presence bits; HWND values are also presence bits. Texture names remain
scalar values. Object addresses and real thread IDs are never serialized.
The thread field distinguishes the main thread from other threads.

Buffers have a byte count and 64-bit FNV-1a hash. Matrices, fog/color vectors,
combiner bytes, texture pixels, indices and vertex-format lists are captured.
Vertex bindings have no count: `InterleavedArrays` marks them deferred, and
each draw hashes its actual vertex range using the current binding. Indexed
DX7 draws hash the minimum-to-maximum range the driver copies, including
intermediate vertices. Standard vertex bytes exclude stride padding. Texture
rows exclude row padding; DXT inputs include complete blocks. Hash read
failures and buffers over 64 MiB refuse completeness. Output-only destinations
are marked `output`, never read before the game initializes them. DX7's
unsupported blits are marked `ignored-dx7`; other drivers' blit buffers and
the unspecified `Punt` payload refuse completeness if exercised.

The UI root is available through the public application interface. Each frame
records the window tree in child enumeration order: parent/node indices,
window ID, local rectangle, visibility, caption byte count and text hash.
Duplicate IDs remain distinguishable by their tree positions. Captions are
hashed in the game's byte encoding, without storing their text.

Pausing the simulator does not prove frozen rendering. A paired run may still
differ through animation, cursor state, asynchronous loading or renderer work
outside the scheduled tick. Keep such results separate from exact captures;
use the first difference and controlled reruns to identify the source. The
recorder deliberately does not modify `FC4Schedule.cpp` or `FC4Schedule.h`.

`engine/tests/scenarios/draw.tsv` supplies paused, visible camera sweeps using
the same start saves as `suite.tsv`. Record them with the pinned reference DLL:

```text
python tools/check.py record --scenarios draw --store <reference-store> --jobs 1
python tools/check.py record --scenarios draw --store <reference-store> --rerun --jobs 1
```

Repeat the second command with `GOLDEN_INI_EXTRA="heap perturb = holes"` and
`GOLDEN_INI_EXTRA="heap perturb = reverse"`. These audits retain their environment
and first difference beside the immutable reference. Draw settings and reference
DLL bytes participate in the input key, so a new pin requires new recordings.
Varying originals cannot establish exact draw credit or use save envelopes.

`check.py run`, `run --replace each`, and `credit` include the draw rows whose
recorded entry counters reach the switched owners, including required owners.
Missing draw references withhold credit. A difference reports frame, call index,
method and argument or buffer. A draw-only `run --replace each --scenarios draw`
writes `draw-owner-check.tsv`, preserving the save-check table for comparison;
the usual full check writes both save and draw verdicts into `owner-check.tsv`.
