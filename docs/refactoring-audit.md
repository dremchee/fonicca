# Refactoring completion audit

Audited 2026-10-30 against the six-stage scope in [refactoring.md](refactoring.md).
Implementation source revision: `8f073f2`; that revision already includes the
public graph gate, production loading changes and architecture documentation.
No code changed during this audit. Historical measurements retain their original
scope and source information; they are not silently treated as new measurements.

**Overall status: complete for the agreed six-stage scope.** macOS became unlocked
after the initial audit. The remaining foreground scenario was then measured
with real native IPC, a private SQLite library and two private decoded audio files.
The earlier hidden-scene observation remains explicitly partial; it is not the
evidence used to close the foreground requirement.

## Requirement-to-evidence check

| Requirement | Current evidence | Finding and limits |
| --- | --- | --- |
| Short/long audio baseline | [Original workload](research/editor-performance-baseline.json), [final-code repeat](research/editor-final-workload.json), `editor/benchmarks.rs` | Native 30/180/1200-second fixtures, three observations per operation. Final extended workload passed all three durations; not a foreground UI test. |
| Bounded request/draw telemetry | `src/editor-performance.ts`, `src/composables/debugMonitor.ts`, `tests/editor-performance.test.ts` | Timings bounded by operation name and sample window, errors retained, reset rejects late results. Draw timings describe submission, not GPU completion. |
| Large library scrolling and stale search | [Browser baseline](research/library-browser-baseline.json), [renderer comparison](research/library-ui-comparison.json), `tests/library-pages.test.ts`, `tests/library-selection.test.ts` | 100k metadata fixture, bounded page/concurrency checks, stale selection/scope guards. Browser mock does not establish native SQLite or WebKit memory. |
| Separate native/renderer memory | [Foreground observation](research/webview-foreground-observation.json); earlier [hidden observation](research/webview-memory-observation.json) retained separately | Nine visible/focused, unlocked checkpoints cover library scroll/search and short/long native editors with ten fade/undo cycles each. Per-process RSS/physical footprint and cohort-exit attribution recorded. No mocks or user data; no disjoint RSS/PCM sum, leak diagnosis or before/after memory-savings claim. |
| Waveform loading and rendering boundaries | `composables/editor/waveform.ts`, `editor-canvas.ts`; `tests/editor-waveform.test.ts`, `tests/editor-canvas.test.ts` | Lifecycle/cache/request generation separate from scene painting; tests cover retries, zoom/DPR/audio invalidation and discarded session replies. |
| Gestures, markers, keyboard and toolbar | `composables/editor/gestures.ts`, `editor-shortcuts.ts`, `components/editor/EditorToolbar.vue`; corresponding Node tests | Click audition, silent drag selection, active-range update without restart, marker bounds and listener disposal covered. Mock browser flows exercise the assembled toolbar; they do not substitute for a native audio-device test. |
| View/store ownership documented | [Architecture](architecture.md), current view/composable/store imports | View composes surfaces; transient gestures/viewport/wave requests have owners; store actions retain audio/session commands. |
| Measure and optimize audio bottlenecks | [Effects](research/editor-effects-comparison.json), [processing](research/editor-processing-lock-comparison.json) | Block dispatch/scans improved measured paths; stretch and LUFS evaluated. Native copy/render/commit phases have separate timings. No blanket claim that all DSP is faster. |
| Remove avoidable paste/MIDI copies | [Paste](research/editor-paste-comparison.json), [transcription preparation](research/editor-transcription-comparison.json); native differential tests | Shared snapshots, bounded conversion/downmix reads and stale-result rejection covered. Paste format conversion is slower in the measured 20-minute case; the removed full intermediate and short lock are the benefit. MIDI inference itself was unchanged and is not claimed faster. |
| Save/export and undo behavior preserved | [Save](research/editor-save-comparison.json), [slices/export](research/editor-slices-comparison.json); `editor/saving.rs`, `paste.rs`, `slices.rs`, `exporting.rs` tests | Samples, metadata/ranges, one-step undo, captured saved anchor, overlapping saves, close/reopen and unique output directories covered. Worker extraction reduces editor lock time; disk writes/detection CPU are not claimed consistently faster. Partial export files can remain after failure, as documented. |
| Library navigation/pagination/selection split | `composables/library/navigation.ts`, `library-pages.ts`, `library-selection.ts`, related Node tests | State owners separated; default page/concurrency/retention policies tested, large range selection reads IDs. |
| Inspect and improve SQL plans | [Query baseline](research/library-query-baseline.json), [query comparison](research/library-query-comparison.json); `src-tauri/src/library.rs` tests | 32k/100k fixtures, natural-order index, collapsed-root path, three-run comparisons and differential counts/pages/IDs/sorts. Scoped results and parent/child semantics covered; this is not a browser FPS claim. |
| Shared job polling/cancellation lifetime | `composables/backgroundJob.ts`, `job-poller.ts`, `composables/modelStorage.ts`, `tests/background-job.test.ts` | Serialized/coalesced reads, bounded retry, command invalidation, start/cancel guards and disposal tests. Stores retain model-specific inputs/options; closing a view does not automatically cancel native DSP. |
| One authoritative model catalog | `model-packs/catalog.json`, generated TS/Rust, `tests/model-catalog.test.ts`, [native build-gate cases](research/model-catalog-build-gate.json) | Complete generated-content checks and native fingerprint gate; changed/missing/CRLF cases recorded. Metadata generation does not add an inference implementation. |
| Check graph distribution before packaging | `scripts/verify-model-packs.mjs`, `tests/model-packs.test.ts`, parsed workflow, [public probe](research/model-pack-verification.json) | Both current anonymous graph downloads matched size/SHA-256. Timeout/retry/corruption/CLI cases tested. Installer matrix depends on the tag gate. Hosted CI was not dispatched; public availability is time-specific, and weight downloads are excluded. |
| Measure bundle/native build costs | [Bundle/loading comparison](research/bundle-loading-comparison.json), [local release build](research/local-release-build.json) | Library/editor startup dependency bytes reduced; total JS grew slightly. Twelve production browser checks include an observed preload regression and its correction. Current ARM64 app builds and its ad-hoc signature verifies. No measured launch-time claim; no local Windows/Linux installer verification. |
| Update current architecture/dependencies | [Architecture](architecture.md), [dependencies](dependencies.md), [build guide](builds.md), README and current stack in plan | Current implementations distinguished from archived candidates; 39 package-version/license-metadata rows and 55 relative links checked. Model-weight conditions remain separate from package metadata. |
| Preserve version and keep work reviewable | Current manifests/`release:check`, Git history | Version remains 0.1.3, implementation stages are separate commits. No tag, push, published release or PR is part of this work. |

## Final native repeat

[Raw final-code observations](research/editor-final-workload.json) include the
exact command, source revision/hashes, hardware/profile/power state and all 18
operations at each duration. The 20-minute fixture's medians include decode
1200.1 ms, first wave 975.3 ms, cached wave 1.47 ms, three-second fade 0.41 ms,
full fade 1105.8 ms and WAV writing 782.4 ms. Undo/redo stay below 0.002 ms in
these native samples. These exclude IPC, scheduling and UI, not full user latency.

Historical baseline values are included for context. Power state was not recorded
for that baseline, and the extended workload adds operations before saving.
Consequently this final repeat is not a new controlled causal before/after study;
small timing differences should not be presented as proven regressions/speedups.

## Verification status

The last full code checks remain 132 passing frontend tests, 215 passing Rust
tests (42 opt-in cases omitted from the normal run), production frontend/type
checks and a local ARM64 release app build. This audit additionally ran the
original opt-in native workload on the current source: one workload test passed,
with three observations for each of 18 operations at all three durations.
Current source hashes still match the graph/loading/build reports. Repeating
unchanged full suites is unnecessary for the audit-document change.

## Final foreground verification

The [foreground report](research/webview-foreground-observation.json) records the
scenario, repetitions, source fingerprints, binary hash, native diagnostics,
per-process RSS and physical footprint, visibility/focus and unlocked state at
every checkpoint. It uses the real production Vue surfaces/native backend with
an isolated SQLite database, not mockIPC or an iframe. The fixture contains 100k
rows, of which two reference private synthetic WAVs. Metadata-only paths are not
scanned; no model inference or audible playback runs. Both short and long editors
perform ten fades/undos and close cleanly, with zero recorded request errors.

All attributed native/WebContent/GPU/Networking PIDs disappeared after the test
host stopped. The earlier hidden-start timeout was discarded and the fixture
corrected to show/focus before asynchronous bootstrap. Templates are retained in
the report; their placeholders contain no one-run token. The normal debug binary
was rebuilt successfully and checked for absence of the fixture identifier and
private data root; its hash is in the report. It was not launched. The private
fixture directory and its temporary server were removed after recording evidence.
No normal app was launched against the user's data, and no unlock or visibility
state was forced or faked. No required scope item remains, subject to those
explicit measurement limits.
