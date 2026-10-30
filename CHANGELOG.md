# Changelog

## [0.1.4] - 2026-10-30

- Measure editor operation latency and add reproducible audio benchmarks (2034b0e)
- Extract editor waveform request lifecycle with stale-session protection (a3fdebc)
- Separate editor toolbar, pointer gestures and keyboard commands (072617e)
- Speed up block effects and contiguous normalization scans (3fd4d89)
- Separate editor canvas painting from view lifecycle (cf38561)
- Bound library page retention and separate navigation and selection (221ef63)
- Speed up library ordering and fetch selection IDs by range (529fb10)
- Share background job polling, command state and model storage reads (9291952)
- Generate UI and native model metadata from one catalog (6457fc6)
- Reduce library scroll rendering cost with shared cell tooltips (67b93dc)
- Move editor processing copies and gain rendering outside session locks (ba2ac2e)
- Prepare pastes on workers and share immutable clipboard samples (e9daaa1)
- Prepare transcription from shared editor blocks outside session locks (94f470f)
- Save immutable editor versions on workers with guarded session commits (4b9ebc1)
- Run slice detection and export on workers and discard stale previews (b27faf8)
- Verify public model graph integrity before release packaging (67d8d8f)
- Load editor and settings UI on demand and measure production bundles (0ebf821)
- Document current architecture dependencies and refactoring evidence (8f073f2)
- Verify final native workloads and audit refactoring completion (6e8c893)
- Complete isolated foreground WebView and native editor measurements (8d573cd)
- Remove unused UI components and obsolete icon concepts (407add7)
- Add WAV MP3 and Ogg export with sample rate conversion (fbc60ce)
- Detach the live editor session into a separate window (7bce042)
- Resize saved loop boundaries with live playback and undo (af81725)
- Reset playback and discard stale editor sessions after UI reload (6ab9f92)
- feat(editor): add loop flags, labels and direct marker dragging (2091355)
- fix(editor): connect marker stems to flags and remove loop shading (9f1722f)
- fix(editor): preserve waveform cache for marker and loop edits (3408197)
- feat(editor): refine badges, toolbar menus and autoplay controls (80ba9b3)
- refactor(settings): remove obsolete preview waveform controls (ae5c746)

## [0.1.3] - 2026-10-29

- Format updater release notes as readable sections and lists (8aaf849)
- Fit tooltip width to centered text lines (bf38130)
- Prevent popover autofocus from opening tooltips on mouse clicks (f7f42ce)
- Add configurable row playback and align stem rows (06ea593)
- Share disk-backed editor audio and reuse waveform caches (11ed86b)
- Add draggable performance monitor and sidebar model download (edaef92)
- Document editor optimizations and defer the sound map design (a4f1577)
- Align editor canvases and timeline drawing to device pixels (c08b336)
- Add minimal waveform overview resize handles (2e23d1b)
- Limit bounded decoder helper to test builds (9b79d33)
- Refine waveform overview controls and marker region saving (b0cd2df)
- Add native two-track vocal and instrumental separation (484f73a)
- Show model downloads as compact sidebar badges (97880f9)
- Download vocal model graphs instead of embedding them (a96fab4)
- Document external model graph distribution and publishing (458b544)
- Add patch release command with automatic push (c1c8f63)

## [0.1.2] - 2026-10-28

- Sign macOS app bundles and verify release signatures (648fc4f)
- Add release command for version bumps and changelog generation (00d34b8)

