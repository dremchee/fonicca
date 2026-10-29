# Refactoring and performance work

Scope agreed 2026-10-29. Keep existing editing/playback/save behavior. Structural
changes and measured optimizations must remain separately reviewable. A passing
unit test alone does not establish an improvement in interactive performance.

## Completion checklist

- [ ] Baseline: short/long audio workloads, library scrolling/search, bounded
  operation telemetry, separate native/renderer memory interpretation.
- [ ] Editor: extract waveform loading/rendering, pointer selection/markers,
  shortcuts and toolbar; clarify view/store ownership; verify interaction parity.
- [ ] Audio: measure copying/locks/recomputation; optimize the demonstrated
  bottleneck, including evaluating stretch and LUFS; compare identical workloads.
- [ ] Library: separate navigation, pagination and selection; bound page retention;
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

Still pending: library baseline, full WebView-process memory measurements,
remaining editor boundaries, measured audio optimization, library/job/model
refactoring and release checks. The six-item completion checklist remains open.
