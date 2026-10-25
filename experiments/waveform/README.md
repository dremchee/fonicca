# Waveform comparison

A development-only comparison of Fonicca's existing `drawWave` Canvas renderer and `gl-waveform@5.3.0` (MIT). The bench remains independent; the application's subsequent integration is documented in `docs/research/waveform-integration.md`.

From the repository root:

```sh
npm ci --prefix experiments/waveform --ignore-scripts
npm --prefix experiments/waveform run dev
```

Open http://localhost:1425/waveform-compare.html. The server uses a separate port and dependency cache. The default input is the repository's 12-second mono test WAV, not a song. You can load a local audio file; its bytes stay in the browser. The 5-minute option repeats the test PCM for a visual stress test; the audio player still plays the original file.

Both views share PCM, zoom, pan, cursor, dimensions, color and RMS settings. There are presets for a full file, one second and individual samples. Drag to pan, scroll to zoom, click to place the shared cursor. The benchmark follows a fixed range path: 20 warmup frames and 180 measured frames. Results can be downloaded as JSON.

Select Canvas or WebGL alone for an isolated timing run. These measurements time JavaScript calls, not GPU completion. The Canvas summary is produced in JS in the bench and measured separately; production Fonicca produces it in Rust. Do not interpret that JS reduction time as the performance of the production backend.

```sh
npm --prefix experiments/waveform run check
npm --prefix experiments/waveform run build
```

The standalone build goes to `experiments/waveform/dist/`, including its fixture asset. It is excluded from the normal app build. Research results and limitations are in `docs/research/waveform-renderers.md`.
