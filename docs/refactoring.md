# Refactoring and performance work

Scope agreed 2026-10-29. Keep existing editing/playback/save behavior. Structural
changes and measured optimizations must remain separately reviewable. A passing
unit test alone does not establish an improvement in interactive performance.

## Completion checklist

- [ ] Baseline: short/long audio workloads, library scrolling/search, bounded
  operation telemetry, separate native/renderer memory interpretation.
- [x] Editor: extract waveform loading/rendering, pointer selection/markers,
  shortcuts and toolbar; clarify view/store ownership; verify interaction parity.
- [ ] Audio: measure copying/locks/recomputation; optimize the demonstrated
  bottleneck, including evaluating stretch and LUFS; compare identical workloads.
- [x] Library: separate navigation, pagination and selection; bound page retention;
  test stale requests and inspect slow SQL query plans on a large fixture.
- [ ] Jobs/models: one authoritative model catalog; share polling/cancellation
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

Still pending: full WebView-process memory measurements, long-frame profiling,
shared job/model refactoring and release checks. The remaining completion items
stay open until their full evidence is present.
