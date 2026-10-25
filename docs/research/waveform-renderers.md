# Canvas / gl-waveform comparison

Date: 2026-10-25. Measurements of the isolated bench before application integration. The subsequent default-renderer migration is documented in [waveform-integration.md](./waveform-integration.md).

## Conclusion

The measured workload does not establish a performance reason to replace Canvas. Both renderers handled the isolated scenario smoothly. The WebGL renderer is a useful candidate for a different editor appearance, with antialiased lines, sample dots and optional density fills, and these isolated results alone do not validate application integration or native WebKit performance.

The bench is available at `/waveform-compare.html` on its separate development server, port 1425. See `experiments/waveform/README.md` for installation and use. The bench has its own dependency installation. The application now also declares the renderer as a production dependency.

## Method

- Current Fonicca `drawWave`, peaks + RMS, versus `gl-waveform@5.3.0`.
- Shared decoded PCM, a 1098 × 160 CSS-pixel viewport, DPR 1, linear amplitude, purple color, 92% lane height.
- Repository fixture `src-tauri/tests/data/ref_long.wav`: 12 seconds, mono, decoded at 48 kHz. This is a test signal, not representative music.
- The long case repeats that PCM to 300 seconds. It is a stress buffer, not a five-minute recorded song.
- Fixed pan/zoom trajectory: 20 warmup frames excluded, 180 measured frames.
- Chromium 156, ANGLE Metal Renderer: Apple M1 Pro, through the in-app browser. These are not native Tauri/WKWebView results.
- Times measure CPU-side API calls, including any synchronous work inside them. GPU completion is not measured. No `gl.finish()` or GPU timer query was used.
- Canvas receives prepared min/max/RMS columns. Their JS preparation is timed separately. Production uses Rust, so the preparation figures are not a measurement of the production backend.

## Final isolated 300-second runs

| Operation | Median | p95 | Maximum |
| --- | ---: | ---: | ---: |
| Canvas drawing, prepared overview | 0.4 ms | 0.9 ms | 1.5 ms |
| gl-waveform render call | 0.7 ms | 1.7 ms | 2.7 ms |
| Canvas overview preparation in JS | 0.7 ms | 8.1 ms | 8.9 ms |

Both isolated runs had zero measured frame intervals over 25 ms. Their median interval was 10 ms. Do not attribute the combined two-renderer page's frame intervals to either renderer separately. Sub-millisecond differences are close to the timer's observed granularity and should not be treated as a stable speedup.

For the final 12-second side-by-side run, both draw-call medians were 0.4 ms; p95 was 0.6 ms for Canvas and 0.9 ms for WebGL. WebGL construction in the isolated long run took 38.5 ms; this is not a complete end-to-end cold-start measurement.

Raw results: `waveform-12s.json`, `waveform-300s.json`, `waveform-canvas-300s.json`, `waveform-webgl-300s.json`.

## Memory

- Shared raw PCM: 2,304,000 bytes for 12 seconds, 57,600,000 bytes for 300 seconds (about 2.20 and 54.93 MiB).
- A mono Canvas overview at the tested resolution occupies 26,352 bytes in its Float32 representation (about 25.73 KiB). This excludes the backing canvas, browser overhead and backend buffers.
- The experiment retains raw PCM for both renderers. Production Canvas can receive only the overview from Rust. Full PCM in the frontend is therefore a material integration choice, not an inherent requirement of rendering with Canvas.
- gl-waveform has `peaks()` and `drop()` APIs for externally held samples and summaries. Adapting those to the Rust data path may avoid retaining full PCM in the frontend; that path was not benchmarked.
- `Runtime.getHeapUsage` snapshots are saved in `waveform-memory-snapshots.json` for transparency. Forced GC was unavailable. Snapshots may include previous documents or uncollected garbage and are not attributable per renderer. They exclude GPU memory. No total-memory winner is claimed.

## Visual and integration checks

- Both views draw the same file and respond to shared zoom, pan and cursor controls.
- Inspected the full-file overview and a zoom into individual samples. The adapter snaps the shared range to whole frames and offsets the WebGL sample range by half a sample to match Canvas cell centers. Point shape and antialiasing still differ.
- Native Canvas also supports the app's dB scale, frequency colors and bars. Those modes are not connected to gl-waveform in this experiment.
- Fonicca preview playback currently moves a clip edge rather than redrawing the waveform every frame. A pan/zoom rendering benchmark is not a playback-performance benchmark.
- Custom-file input was checked with the repository's separate 1.3-second WAV; measured cases used the 12-second fixture. JSON export is provided.

## Sources

- https://github.com/dy/gl-waveform — current upstream, API, MIT license metadata and rendering model.
- https://audiojs.dev/lab/waveform/ — waveform design examples and link to the renderer.

The earlier `audiojs/gl-waveform` URL was incorrect; the verified upstream is `dy/gl-waveform`.
