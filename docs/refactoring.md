# Refactoring and performance work

Scope agreed 2026-10-29. Keep existing editing/playback/save behavior. Structural
changes and measured optimizations must remain separately reviewable. A passing
unit test alone does not establish an improvement in interactive performance.

## Completion checklist

- [ ] Baseline: short/long audio workloads, library scrolling/search, bounded
  operation telemetry, separate native/renderer memory interpretation.
- [x] Editor: extract waveform loading/rendering, pointer selection/markers,
  shortcuts and toolbar; clarify view/store ownership; verify interaction parity.
- [x] Audio: measure copying/locks/recomputation; optimize the demonstrated
  bottleneck, including evaluating stretch and LUFS; compare identical workloads.
- [x] Library: separate navigation, pagination and selection; bound page retention;
  test stale requests and inspect slow SQL query plans on a large fixture.
- [x] Jobs/models: one authoritative model catalog; share polling/cancellation
  mechanics while keeping model-specific DSP implementations separate.
- [ ] Release: verify downloadable graphs before distributing installers; measure
  build/bundle costs and update outdated dependency/architecture documentation.

## Editor boundaries

- `EditorView`: composes the editor surfaces and owns their layout.
- Wave rendering composable: owns viewport requests, cache invalidation and drawing.
- Pointer/marker composable: owns transient gestures and issues store actions.
- Shortcut composable: translates keyboard input to explicit editor actions.
- Toolbar component: receives availability/state and emits tool actions.
- `EditorPerformanceMetrics`: receives timing summaries/heap estimate, renders them;
  sampling/reset remain in the monitor owner, outside the hot rendering path.

## Native baseline command

```sh
FONICCA_BENCH_SECONDS=30,180,1200 FONICCA_BENCH_REPEATS=3 \
FONICCA_BENCH_OUTPUT=/tmp/fonicca-editor-baseline.json \
cargo test --manifest-path src-tauri/Cargo.toml --locked --lib \
  editor_workload_benchmark -- --ignored --nocapture --test-threads=1
```

Use the same machine, power mode, build profile and scenarios for comparisons.
The ignored workload creates private synthetic stereo float WAVs at 44.1 kHz.
It measures decode, cold/cached/panned/zoomed waves, short/full-file fades,
delete, undo/redo and save. It does not access user audio or require an audio
device. Temporary files are removed when the test ends. An hour-long fixture is
available through `3600`, but needs several GiB of free disk space.

These are native warm-filesystem timings, not end-to-end UI measurements. Keep
raw repetitions; p95 of three repetitions is just the largest observed value,
not a statistically strong latency estimate. Store measured results with the
source revision and environment before changing audio algorithms.

## Interactive measurement protocol

Open the debug monitor, reset, then repeat a scenario at least ten times. Compare
the same viewport width, waveform settings and file. The timings table retains
the last 60 measurements per operation and at most 32 operation names. Request
timings include native queue/work, transport and response delivery. `draw` times
CPU command submission, not GPU completion or time waiting for animation frames.

Native RSS excludes WebView processes. The optional JavaScript heap estimate is
reported separately only on browsers that expose it; it is not total WebView RSS
and must not be added to native PCM counters as if they were disjoint allocations.
Use the platform process profiler for complete renderer-process memory measurements.

Library scenario: 32k and 100k entries; scroll down and back, switch folders/search
rapidly, and verify that pending results cannot restore the old scope. Record
request count, retained pages, first-page latency and long frames.

## Progress and evidence

2026-10-29, source base `3bd5203` (0.1.3), macOS ARM64, Cargo dev-test profile:

- Native workload added and run for 30, 180 and 1200 seconds, three repetitions
  each. Raw results: [editor-performance-baseline.json](research/editor-performance-baseline.json).
- On the 1200-second fixture, median decode 1222.8 ms, first waveform 968.5 ms,
  cached waveform 1.3 ms, short fade 1.0 ms, full fade 1350.9 ms, save 779.4 ms.
  Undo/delete timings are recorded in the raw data; these runs do not establish
  interactive playback or UI latency on actual compressed music files.
- Bounded frontend request/draw telemetry added to the debug monitor, with error
  accounting and reset protection against late results. Browser heap availability
  is explicit; native RSS and JS heap are not conflated.
- Waveform request/cache lifecycle extracted from `EditorView` into
  `composables/editor/waveform.ts`. Overscan, zoom/DPR invalidation, stale sessions,
  retries and disposal are covered by dedicated tests.
- Browser mock: waveform displayed, fade and undo executed, operation metrics
  appeared in the monitor, browser console had no errors on the checked page.
  Screenshot: local `output/playwright/refactor-editor-monitor.png` (not shipped).

Second pass:

- Selection/marker/context-menu gestures now live in `composables/editor/gestures.ts`.
  Scope/session changes remove pending window listeners and invalidate delayed
  zero-crossing results. Tests cover audition clicks, silent drag selection,
  updating an active range without restarting, marker bounds and disposal.
- Keyboard mapping moved to `editor-shortcuts.ts`, with tests for modifier keys,
  Russian marker shortcut and shifted sampler-pad numbering.
- `EditorToolbar` receives state flags and emits typed actions. Curve, loop mode
  and slice preview use explicit two-way models. Audio operations remain in the
  editor store; dialogs continue owning their own specialized behavior.
- Browser mock: drag selection and M markers worked; extracted toolbar fade
  invoked an edit; at 680px width the overflow menu and Save remained available.
- Block effects dispatch once per block instead of per channel/sample. Constant
  gain conversions are hoisted, normalization and DC analysis scan contiguous
  pieces and compute only their required statistic. Multi-channel fragmented
  edits are checked against the contiguous reference implementation.
- [Raw before/after results](research/editor-effects-comparison.json): for the
  identical 180-second workload, median normalization 300.5 → 158.4 ms, gain
  165.6 → 139.4 ms, full fade 184.5 → 173.3 ms. These are three local repetitions,
  not a cross-platform speed guarantee. Short-selection stretch and LUFS were
  also measured (about 34 and 7 ms); no change was made to those algorithms.

Third pass, 2026-10-30:

- Canvas scene painting moved to `editor-canvas.ts`. It receives a snapshot and
  injected waveform/spectrogram painters, returning the region hit-test badges.
  Vue lifecycle, audio requests and DOM cursor overlays remain with their owners.
  Drawing tests cover selection inversion, markers, skipped regions and spectrum.
- Library navigation/filter preferences, page scheduling and selection now have
  separate owners. The store composes them and retains library mutations/session
  restoration. Public navigation and selection behavior is preserved.
- Page retention is capped at 12 × 200 rows. Four page requests can run at once;
  queued obsolete scroll requests are dropped, old-scope responses are ignored,
  and background refreshes cannot resurrect evicted pages. A focused page stays
  available. Large shift selections request only IDs in the required range instead
  of retaining all row objects. Tests also cover immediate retry, new-scope/select-all races,
  delayed keyboard focus and disposal.
- Opt-in SQL baseline on isolated 32k/100k databases:
  [raw timings and plans](research/library-query-baseline.json). On the 100k
  fixture, grouped first/middle/last pages took about 91/366/469 ms. Query plans
  use temporary sorting; this baseline has no stems, tags or embeddings and is
  one observation per offset. It is evidence for further query work, not a SQL
  performance improvement from this refactor.
- Browser fixture: `/mock.html?size=100000#library-large` (also `size=32000`).
  It constructs metadata only for requested pages and has a 25ms mock list delay;
  search/category/basic scopes are simulated, not the full SQLite semantics.
  The debug monitor receives an optional library diagnostics getter from `App`
  and samples cache counts on its existing cadence.
- [Browser observation](research/library-browser-baseline.json): 35 scroll jumps
  retained at most 12 pages/2400 rows, with 57 DOM sample rows near row 85k and 37
  on return to the first row. A rapid search followed by Favorites left the new
  empty scope intact. Mock RSS is demo data and cannot establish memory savings.
  Long tasks of 50–205ms occurred in this development run; smooth scrolling has
  not been established and needs profiling separately from native SQL.
- Browser editor check after drawing extraction: drag selection, marker creation
  and fade rendered correctly; checked console had zero errors. Current checks:
  110 frontend tests, 194 native tests passing (37 opt-in tests ignored), and
  `pnpm build` succeeds. The build still reports a >500kB JavaScript chunk.

Repeat native library measurements with:

```sh
FONICCA_LIBRARY_BENCH_OUTPUT=/tmp/fonicca-library.json \
cargo test --manifest-path src-tauri/Cargo.toml --locked --lib \
  library_workload_benchmark -- --ignored --nocapture --test-threads=1
```

SQL follow-up:

- Added a schema migration for an index matching natural-name/path ordering.
  Collapsed groups use a direct index walk for an unfiltered library and resolve
  matching IDs first for selective scopes. Expanded groups keep their existing
  CTE. Empty metadata ordering and parent/child semantics remain unchanged.
- Differential tests compare collapsed results, counts, pages and ID ranges
  against the existing CTE for eight sorts in both directions, including matching
  children in another source, nested relations, favorites, tags and collections.
  Saved stem migration tests now select their historical migration explicitly.
- `sample_ids` accepts optional offset/limit. Shift selection makes one ID-range
  request; select-all retains its existing all-IDs behavior. This avoids hundreds
  of serial row-page requests for a large selection. Browser fixture check:
  100,000 selected, 37 DOM sample rows rendered.
- [Three-run comparison](research/library-query-comparison.json), 100k entries,
  10k stem links and about 33k tag assignments. Median grouped first/middle/last
  page: 82/344/464 → 0.75/10.4/25.0 ms. Flat list: 24/324/461 → 0.62/2.75/4.65 ms.
  Filtered cases and raw repetitions are retained too; directory differences are
  small. These synthetic local measurements do not establish browser FPS.
- For reproducibility, `FONICCA_LIBRARY_BENCH_RELATIONS=1` enables links/tags.
  `FONICCA_LIBRARY_BENCH_REFERENCE=1` drops the new index and uses the previous
  materialized-root query with no expanded parent present. Run reference and
  optimized workloads sequentially after compilation; do not run UI profiling
  or other builds concurrently with their query measurements.

Background task follow-up:

- Stem separation, Apollo and Basic Pitch share `useBackgroundJob`. Stores keep
  their source identity, model/mode validation, output dialog and DSP arguments.
  Starting/cancelling commands, pending flags and status lifetime are shared.
- `createJobPoller` serializes status reads, coalesces refreshes and invalidates
  pre-command/model responses. A read failure retries with bounded backoff;
  idle status stops polling. Disposal drops late results and releases waiters.
  Store disposal also releases its native event listener, including a listener
  whose registration completes after disposal. It does not cancel native audio
  processing merely because a component closes.
- Download badges and visible model settings share one model-storage poller,
  replacing their independent loops. Its last mounted consumer releases the
  timer; disposed results cannot replace a newly mounted consumer's state.
- Tests exercise status recovery, duplicate commands, delayed reads, retrying
  subscription registration, cancelled save dialogs and shared consumer lifetime.
  Native DSP/job implementations have not been combined.
- Browser mock: started two-track separation, stopped it, then started again and
  observed completed vocals/instrumental outputs. Current frontend verification:
  120 tests passing, type check and production build passing. Native code is
  unchanged by this follow-up; actual model inference was not rerun for it.

Model catalog follow-up:

- `model-packs/catalog.json` owns download metadata for all ten models: canonical
  names, groups, stem capabilities, storage directories, artifacts, byte sizes,
  SHA-256 and URLs. A migration audit against source `9291952` confirmed identical
  metadata for every existing model; download locations and hashes did not change.
- `pnpm models:generate` produces TypeScript metadata/types and Rust constants,
  artifact tables and enum adapters. Captions, byte defaults, mocks, mode choices,
  mode validation and downloads consume these outputs. Specialized tensor/DSP
  code remains separate. Maintenance instructions: `model-packs/README.md`.
- `pnpm models:check`, the frontend build and release-version CLI reject stale or
  hand-edited outputs. Native builds also reject an outdated catalog fingerprint
  without requiring Node during Cargo builds. LF/CRLF normalization is tested.
- [Native build-gate evidence](research/model-catalog-build-gate.json): the actual
  build script was compiled with a no-op bundler adapter and run in isolated
  temporary checkouts. Valid LF and CRLF inputs passed; changed source and missing
  generated stamps failed. This verifies the catalog guard, not desktop packaging.
- Node tests verify generated content, mode lists, totals, malformed metadata
  rejection and both checked-in graph files against the pinned SHA-256. Rust tests
  verify enum serialization IDs and retain the existing model/mode constraints.
  Browser mock showed four models in four-track mode and three specialized vocal
  models in two-track mode, with the existing names and sizes.
- Verification: 124 frontend tests and 194 native tests pass (37 opt-in native
  cases remain ignored); frontend production build, native `cargo check`,
  `models:check` and `release:check` pass. Download/inference of the large upstream
  weights was not repeated: the metadata migration was checked against the old
  definitions, and existing native inference code keeps its tensor contracts.

Library rendering follow-up:

- Repeated the 100k-row scroll workload on a production mock, separate from the
  earlier development measurement. A CPU profile identified frequent tooltip
  mounting/unmounting, Reka forwarding helpers and associated reactive allocations.
  This is renderer work, not a SQLite query or audio decode delay.
- The virtualized list now delegates metadata/favorite hints to one shared tooltip.
  Cells stay plain DOM nodes. Existing tooltip placement, hoverable content,
  keyboard focus, Escape, provenance/confidence text and favorite shortcut remain.
  Scroll, context menus, changed analysis and removed rows dismiss the old anchor;
  `aria-describedby` is attached only while the corresponding hint is open.
- Reproducible browser workload: `scripts/profile-library.mjs`. It needs a local
  production mock and an already open Playwright CLI session. It reloads each run,
  records actual viewport/browser, 35 scroll jumps, frame intervals, long tasks and
  CDP main-thread task time. Optional `--cpu` captures a separate CPU profile;
  profiler-enabled timings must not be mixed with ordinary measurement runs.
- Browser regression scenario: `tests/browser/library-hints.mjs` exports
  `checkLibraryHints(page)` for Playwright `run-code`. The checked cases include
  hover onto content, keyboard access, Escape suppression, scroll/context-menu
  dismissal, changed analysis and filtering away the anchor. It is an opt-in
  browser check, not part of the Node-only `pnpm test` command.
- Raw observations, source/build hashes and measurement limits are in
  [library-ui-comparison.json](research/library-ui-comparison.json). These local
  Chromium results do not establish Tauri/WebKit memory usage or performance on
  every OS. The mock has metadata and cached-wave misses, not 100k decoded files.
- Three alternating runs per build: median main-thread task time 2192.6 → 762.7 ms
  (65.2% less busy time for this workload), with 57 rendered sample rows in every
  observation. Median maximum frame interval was 50.1 → 16.8 ms; the three observed
  baseline long tasks were 50–58 ms, and none occurred in the three candidate runs.

To build a production mock without including it in the distributed application:

```sh
node --input-type=module -e 'import {build} from "vite"; await build({build:{outDir:"/tmp/fonicca-library-profile",emptyOutDir:true,sourcemap:true,rolldownOptions:{input:"mock.html"}}})'
python3 -m http.server 1434 --bind 127.0.0.1 --directory /tmp/fonicca-library-profile
```

In another terminal, using an installed Playwright CLI:

```sh
playwright-cli --session fonicca-profile open 'http://127.0.0.1:1434/mock.html?size=100000#library-large'
node scripts/profile-library.mjs --url 'http://127.0.0.1:1434/mock.html?size=100000#library-large' --output /tmp/library-ui.json
```

`PLAYWRIGHT_CLI=/absolute/path/to/playwright_cli.sh` also accepts a shell wrapper.
Run at least three observations per version, alternate versions, keep the same
viewport and avoid simultaneous builds. To repeat the hint regression scenario in
the existing session:

```sh
node --input-type=module <<'JS'
import { checkLibraryHints } from './tests/browser/library-hints.mjs'
import { spawnSync } from 'node:child_process'
const cli = process.env.PLAYWRIGHT_CLI ?? 'playwright-cli'
const shell = cli.endsWith('.sh')
const args = [...(shell ? [cli] : []), '--session', 'fonicca-profile', 'run-code', checkLibraryHints.toString()]
const result = spawnSync(shell ? 'bash' : cli, args, { stdio: 'inherit' })
process.exitCode = result.status ?? 1
JS
```

The browser regression starts on the first page of the English 100k mock and ends
with a no-match search. Reload the mock before another UI check. Temporary builds
and raw CPU profiles belong outside the checkout; only the compact evidence report
is versioned.

Native processing lock follow-up:

- A command-equivalent workload exposed two long critical sections in LUFS:
  the target PCM was copied while holding the editor mutex, and the returned
  gain operation was rendered under it after loudness analysis. The existing
  worker covered loudness analysis but not these two phases.
- `processing::Snapshot` captures immutable audio/revision under the mutex.
  Memory probing, copying, analysis and rendering the output all finish on the
  blocking worker. Commit checks the original revision and installs the rendered
  audio through the existing snapshot-history path. New edits, undo and reopening
  a session invalidate a late result. No IPC payload or DSP algorithm changed.
- Differential tests compare fragmented mono/stereo LUFS and stretch against the
  previous render/apply sequence, including loops, markers and undo/redo. They
  also check invalid ranges, source sharing and results arriving after reopen.
- [Raw native measurements](research/editor-processing-lock-comparison.json),
  three observations at 30/180/1200 seconds: for full-file 1200-second LUFS, median
  preparation hold 50.8 ms → 0.000042 ms and audio commit hold 859.0 ms → 0.0024 ms.
  These are uncontended core lock phases; state DTO assembly and IPC are excluded.
  Combined phase time remained about 3.84 seconds. The gain render moved into the
  worker, so comparing worker time alone would incorrectly imply slower DSP.
- The same owned PCM input still exists, now allocated outside the mutex. This
  change does not establish lower peak memory. The benchmark records native
  process RSS at phase boundaries, separately from deduplicated PCM storage.
  macOS `sysinfo` reads `proc_taskinfo.pti_resident_size`; mapped PCM pages and
  allocator retention are part of RSS, and phase samples can miss transient peaks.

Repeat the lock/RSS workload after compilation, without simultaneous UI profiling:

```sh
FONICCA_BENCH_SECONDS=30,180,1200 FONICCA_BENCH_REPEATS=3 \
FONICCA_LOCK_BENCH_OUTPUT=/tmp/editor-lock-current.json \
cargo test --manifest-path src-tauri/Cargo.toml --locked --lib \
  editor_processing_lock_benchmark -- --ignored --nocapture --test-threads=1
```

Add `FONICCA_LOCK_BENCH_REFERENCE=1` to run the former command sequence in the
same test binary. Both modes use private generated audio, a real session mutex
and no user library or audio device. Tests measure rather than assert timing
thresholds; use the full repetitions when interpreting the result.

WebView memory observation:

- [Partial native WebView observation](research/webview-memory-observation.json)
  records separate `ps` RSS and OS `footprint` measurements for the Rust host,
  WebContent, GPU and Networking processes. The test application used a different
  identifier, a private temporary app-data root and an incognito WebView; its
  native database remained empty. All four attributed processes exited after
  the test host was stopped.
- The production 100k metadata mock ran in a same-origin iframe inside a minimal
  Tauri page. This is necessary because the browser SDK mocks try to replace
  native read-only Tauri callback globals in the main frame. The native host's
  backend was not replaced, and its real data directories were not accessed.
- Only the loaded library checkpoint (37 DOM rows) was captured. macOS reported
  `IOConsoleLocked=Yes` / `CGSSessionScreenIsLocked=Yes`, and WebKit reported
  `document.visibilityState=hidden`. Further checkpoints did not arrive while
  the desktop remained locked.
  These measurements do not complete the foreground scroll/editor memory scenario.
  JavaScript heap estimation was unavailable, so it was not substituted for OS
  process accounting. Repeat the scenario when the desktop is unlocked.
- Native verification after this follow-up: 196 tests pass, 38 opt-in cases remain
  ignored; the normal native debug build and `cargo check` pass. The temporary
  fixture host was stopped and the normal debug binary was rebuilt without its
  test identifier or temporary app-data configuration.

Clipboard/paste follow-up:

- Clipboard snapshots now share immutable samples. A same-format paste reuses
  that allocation across its audio pieces, operations and undo history. Capturing
  a paste no longer holds either the clipboard or session mutex during conversion.
  Later clipboard copies cannot change an in-flight paste. The old clip is freed
  after releasing the clipboard mutex.
- Rate/channel conversion retains the final output and at most 16,385 frames of
  remapping scratch, instead of a full second remapped clip. Global interpolation
  positions preserve the previous float samples, including block boundaries,
  one-frame inputs, endpoint clamping and multi-channel mixing.
- Paste runs on a blocking worker and commits against the captured session
  revision. Newer edits, undo or reopening invalidate a late result. The frontend
  uses its existing processing guard during paste; errors preserve selection,
  cursor and state and release that guard for retry.
- [Three alternating observations per mode](research/editor-paste-comparison.json),
  30/180/1200-second fixtures: same-format 1200-second insertion reduced median
  core phases from 338.0 ms to 0.034 ms and retained unique PCM storage including
  the clipboard from 1,270,080,000 to 846,720,000 bytes. This storage includes the
  mapped source; it is not a reduction in process RSS by that amount.
- Conversion has a measured tradeoff: stereo 44.1 kHz to mono 48 kHz took median
  467.5 → 555.7 ms for the 1200-second clip. Its full remapped intermediate was
  removed and work no longer holds the editor mutex (median capture + commit
  0.0033 ms in this fixture), but conversion itself did not become faster.
  Measurements exclude IPC, worker queueing, state DTO assembly and freeing an
  existing redo branch; they do not prove UI latency or peak RSS savings.
- Differential native tests check samples, range clamping, loops/markers, single
  undo/redo, sharing across repeated pastes and stale sessions. The allocation
  sharing regression was observed failing against the original clipboard code.
  `tests/browser/editor-paste.mjs` checks the real store against delayed/rejected
  mock IPC: duplicate edits, session transitions, selection preservation and retry.
  It is an opt-in browser check for `/mock.html#editor`, not a native audio test.
- Verification: 201 native tests pass (39 opt-in cases ignored), `cargo check`,
  124 frontend tests and production build pass. Eight paste integration checks
  pass in the development browser mock. The >500 kB chunk warning remains.

Repeat the paste workload with the same duration/repetition variables as the
processing workload, but filter on `editor_paste_lock_benchmark`. Set
`FONICCA_LOCK_BENCH_REFERENCE=1` for the old sequence. To alternate runs, use
`FONICCA_BENCH_REPEATS=1` and retain three runs per mode.

Transcription preparation follow-up:

- Basic Pitch captures an immutable selected range of edited audio, sharing
  its existing pieces. The selection survives later edits and closing its source.
  The 10-minute/mono-stereo/finite-input rules and model input format are unchanged.
- Finite checks and downmix read at most 16,384 source frames at a time. A range
  crossing pieces may materialize that small block; contiguous pieces are borrowed.
  The owned full interleaved selection was removed. The worker still holds a mono
  input, mono output and the existing FFT resampler's scratch while converting.
  Owned-file preparation retains its previous move-only path for mono input.
- Tests compare every model input sample with the previous preparation for
  mono/stereo, five rates, tiny inputs and block boundaries. They cover captured
  fragmented edits, later source disposal, invalid ranges/formats/duration and a
  non-finite sample in a later block. Model inference was not repeated: prediction
  and note decoding are unchanged, and their prepared input is checked directly.
- [Three alternating native observations per mode](research/editor-transcription-comparison.json):
  for a 600-second stereo 44.1 kHz selection, median capture hold 37.08 → 0.00125 ms;
  copied interleaved bytes under that lock 211,680,000 → 0. Median combined core
  preparation phases 605.6 → 585.9 ms. This is not a measured reduction of peak RSS
  by 211 MB or an inference speed improvement. Timings exclude UI/IPC/job startup.
- Verification: 204 native tests pass (40 opt-in cases ignored), `cargo check`
  passes. Frontend code did not change in this follow-up.

Repeat this workload with `editor_transcription_lock_benchmark`, durations
`30,180,600` and the same reference/output/repetition environment variables as
the other lock workloads. No model or audio device is required.

Save follow-up:

- Saving captures immutable audio, loops/markers and the requested operation
  history. File validation, unique first-save naming, WAV output, recoverable
  backup, rename and library indexing run on a blocking worker. The live editor
  mutex covers capture and the final saved-anchor/state update, not file I/O or
  the database probe/index query. Existing IPC response fields are unchanged.
- Each open session has an RAII save lease: another save of that session is
  rejected while pending, and the lease is released after failures or completion.
  The lease also identifies the session after close/reopen. A stale result leaves
  the replacement session intact and reports where the file was already saved.
  Successfully indexed output still emits the library update in that case.
- The saved history anchor is the captured version. Backend edits made during
  the write stay dirty; undoing back to the written version reaches a clean state.
  The frontend retains its existing save guard and controls. Selection export
  does not switch source ID/path or mark the whole file saved.
- A separate file-writer mutex serializes destination choice, backup/replace and
  completion across editor windows. It protects competing first-save names
  without holding the editor map while another file is written. Temporary output
  has RAII cleanup; existing file and backup-failure behavior remains covered.
- [Native file workload](research/editor-save-comparison.json), three alternating
  observations per mode at 30/180/1200 seconds. For the 1200-second first save,
  median session hold 1151.3 → 0.0183 ms; updating the version with backup,
  928.1 → 0.0038 ms. Disk writing itself is not consistently faster: total first
  save 1151.3 → 1325.9 ms; version update 928.1 → 940.6 ms in this observation.
  The reference wraps the same writing policy in the session mutex to isolate
  lock scope. Measurements include state DTO assembly but exclude DB indexing,
  IPC, scheduling and UI. They are not a peak-memory or durability guarantee.
- Filesystem tests additionally cover concurrent editor access, newer audio and
  metadata during save, the exact undo/redo saved anchor, replaced sessions,
  overlapping-save rejection, retry after failure, two windows saving one source
  and selection export indexed under a different ID.
- Verification: 209 native tests pass (41 opt-in workloads/inference cases
  ignored); `cargo check` passes without warnings. Frontend code is unchanged.

Repeat the file workload using `editor_save_lock_benchmark`, durations
`30,180,1200` and the other lock workloads' reference/output/repetition variables.
All source/output/backup files are private temporary fixtures removed on exit.

Slice detection/export follow-up:

- Grid/transient/silence detection captures shared edited audio and runs outside
  the editor map on a blocking worker. Return validates the audio revision, so
  later edits, undo or reopening cannot deliver obsolete boundaries. Loop/marker
  metadata changes alone do not invalidate unchanged source audio.
- Region export captures edited pieces, path, depth and spans. Writing happens
  on a worker and does not switch the source or mutate history. An atomically
  created output directory prevents two windows from sharing/overwriting a set.
  Numbering, range clamping, WAV depth and metadata policy remain unchanged.
  As before, a failed multi-file export can leave its already-written partial set.
- Slicer preview now invalidates existing results and marker placement immediately
  when its target/settings/editor revision changes. The next analysis remains
  debounced. This closes a window where an older reply could enable obsolete
  markers before a new request started. A disposed component cannot start a
  delayed read or restore its preview.
- [Native observations and browser regression evidence](research/editor-slices-comparison.json):
  three alternating measurements per mode at 30/180/1200 seconds. For the
  1200-second fixture, median session holds: transient detection 2179.7 → 0.000166
  ms, silence detection 1061.7 → 0.000166 ms, four-region export 935.0 → 0.000583 ms.
  Detection algorithms and their total computation time remain about 2.2/1.1
  seconds respectively. Measurements exclude worker queueing, IPC and rescanning;
  the short native holds do not establish end-to-end UI latency or peak memory.
- Native tests verify all detection modes on fragmented edits, metadata-only
  changes, stale audio/session rejection, exact exported samples, no source
  mutation, concurrent unique directories, numbering and failure cases.
- `tests/browser/editor-slices.mjs` runs against a real compiled mock UI. A build
  with the previous component fails on a response arriving inside the debounce
  gap. The candidate passes seven target/settings/revision/placement/disposal
  checks. Eight existing paste integration checks pass on that production mock
  too. Temporary mock builds were not included in the application's `dist`.
- Verification: 215 native tests pass (42 opt-in workloads/inference cases
  ignored), 124 frontend tests, type check, frontend production build, native
  `cargo check` and normal native debug build pass. The debug binary was rebuilt
  without the earlier isolated performance-fixture identifier; it was not launched.
  The existing frontend chunk-size warning remains.

Repeat detection/export timings with `editor_slices_lock_benchmark`, durations
`30,180,1200` and the reference/output/repetition variables used above. The browser
regression takes a Playwright Page on `/mock.html#editor`; it unmounts that test
application at the end, so reload before another UI check.

Release graph follow-up:

- Added `pnpm models:verify`: validate catalog synchronization, then verify the
  checkout's versioned graph files and stream their public URLs without credentials.
  Both byte size and SHA-256 must match. Large weight packs are excluded.
- The existing version job runs this gate for version tags; the installer matrix
  depends on that job, so unavailable or changed public graphs prevent packaging.
  Requests have a deadline covering fetch and body reads, transient failures retry
  at most three times, corrupt/missing artifacts fail immediately, and stalled
  cancellation cannot hold the verifier indefinitely.
- Tests cover local corruption, streaming hash/size checks, HTTP/network failures,
  retries, timeouts, local-first gating, exclusion of weights and CLI failure codes.
  Offline mode is explicitly local-only. An optional JSON report records results.
- [Current public probe and source fingerprints](research/model-pack-verification.json):
  both graphs downloaded anonymously and matched the catalog. Parsed workflow
  YAML confirms the tag condition and installer dependency. Hosted CI and new
  installers were not run for this check; public availability is time-specific.
- Verification: 132 frontend tests and frontend production build pass;
  `pnpm release:check` reports 0.1.3. Updated model distribution instructions and
  corrected obsolete release commands in `docs/builds.md`. No version/tag/push.

Still pending: foreground WebView/editor memory measurements, build/bundle costs
and current dependency/architecture documentation.
Completion items remain open until their full evidence exists.
