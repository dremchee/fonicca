# AudioJS waveform integration

The application uses `gl-waveform@5.3.0`, the WebGL renderer linked by
[AudioJS's waveform lab](https://audiojs.dev/lab/waveform/).
It is the default geometry renderer for the library preview (including its played
portion), editor, minimap and settings previews. Audio playback and decoding
continue through the existing native backend.

`waveform-renderer.ts` keeps the existing draw contract. `waveform-gl.ts` renders
GPU geometry into a shared surface per destination canvas, then composites it into
the timeline's 2D canvas. This preserves selection clipping, grids, translation,
frequency tint and bars without changing interaction coordinates. Canvas geometry
is retained as a fallback if WebGL2 creation or rendering fails, including context
loss. Component scope disposal and conditional canvas replacement release GPU
resources. The destination canvas's `data-wave-renderer` records the active path.

The backend still sends only visible samples or compact overview statistics.
`waveform-gl-data.ts` converts each overview column to repeated 256-sample summary
leaves in a **virtual rendering coordinate system**, using at least 1024 virtual
samples per device pixel. Each leaf retains the original column's minimum,
maximum and mean square; it is not fabricated PCM and has no playback timing role.
Summary boundaries are quantized to these leaves, so the rendered outline can
differ slightly from the old rectangular envelope. Close zoom uses the actual
visible sample values. dB scaling and vertical gain are applied before upload.

The earlier `waveform-renderers.md` measurements describe the isolated comparison,
not this compositing adapter. No application performance improvement is claimed
from those results. Browser validation covers the mock app; native WKWebView
validation remains separate from Chromium checks.

## Validation

- `pnpm build`: frontend type checking and production bundle.
- `pnpm test`: 36 passing tests, including summary/sample adapter, preset isolation and dB range checks.
- Browser mock app: WebGL active in both library preview canvases, editor,
  minimap and settings preview; verified frequency tint, bars, dB mode,
  selection, keyboard seeking and zoom to sample dots. The mock backend now
  supplies library overviews and real sample mode at close zoom.
- No native playback backend or Rust data format was changed. Native WebKit
  rendering and a forced context-loss fallback are not covered by these checks.

## Appearance controls

Preview and editor profiles now store independent density (`off`, `gaussian`,
`laplace`, `flat`), peak/body opacity, 1–3 px line width, 40/60/90 dB range and
optional per-channel hex colors. Density is unavailable for bars; custom colors
are unavailable for frequency tint. Peaks/body opacity applies to the RMS or
density styles. The native settings store supplies defaults for old files and
clamps numeric/color input. The Canvas fallback retains colors, dB range,
line width and RMS opacity; density shading requires WebGL.

Three presets apply a complete drawing profile: Editing (plain peaks + RMS),
Quiet details (90 dB and soft density), Music (frequency tint and full density).
Manual changes show Custom settings. A live example previews slider/color drags;
preferences are written on release. Editor windows can open the same settings
from View → Waveform settings, with the editor profile selected.

Final checks also include `cargo test --lib settings::tests`: all five native settings tests pass, including persistence, legacy profiles and appearance clamping.


Discrete bars use their own Canvas path, rather than slicing the continuous GL
outline with gaps. Each pixel-aligned 2 CSS px bar has a constant height from the
maximum of its complete time bucket, flat caps and a 1 CSS px gap. Complete bars
are centered in the available width, and frequency color is selected from the
peak column. The center guide is omitted so it does not bridge the gaps. WebGL
resources are released when switching to bars; the continuous and sample modes
continue to use gl-waveform. Line thickness is disabled for discrete bars.


Bar appearance is configurable per profile: width 1–8 CSS px (0.5 steps), gap
0–6 CSS px (0.5 steps), and end roundness 0–100%. The example updates during
slider movement; settings are persisted on release. Native defaults preserve
the previous 2 px / 1 px / flat appearance for old settings files. Rounding is
bounded by both the width and height of each individual bar. Verified with the
browser controls, 40 frontend tests, six native settings tests and the production
frontend build.
