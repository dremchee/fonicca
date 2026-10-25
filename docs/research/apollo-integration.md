# Apollo whole-track restoration

Implemented 25 October 2026 as an optional library action, **Restore with Apollo**.
It produces an independent FLOAT WAV next to the source, imports it into the
library, and offers original/result playback plus editor and Finder actions.
The original is never overwritten. Repeat runs receive a numbered suffix.

Apollo is a restoration model for music degraded by lossy compression; it is
not a guarantee of better sound on every input. The earlier experiment on
separated stems remains recorded in `apollo-comparison.md`.

## Native model packaging

- Official [JusperLee/Apollo checkpoint](https://huggingface.co/JusperLee/Apollo/blob/c68bd80fdd9c0d93d2f4a833cb154624f660a561/pytorch_model.bin):
  66,541,845 bytes; SHA-256
  `99d9af7f1ff20e63c393035513a655392818d66b4d7fc23d658175c1f15e8d76`.
- The architecture is pinned to Apollo source revision
  `e84bcacc59d5455f05d86a5c97dd4aeb3c14dbb6`.
- The app bundles the approximately 1.7 MiB ONNX spectral graph, **without learned
  tensors**. All 644 initializers reference raw tensor storage byte ranges in
  the exact official checkpoint. PyTorch saves those entries uncompressed.
- The downloader validates the whole checkpoint size and SHA-256 before loading.
  Native inference does not deserialize or execute Python/Pickle metadata.
- `export-apollo-onnx.py --apollo /path/to/cache` regenerates the graph from a
  pinned source checkout in `cache/source/` and the official checkpoint. It
  validates every external tensor against its ZIP payload. The export tool
  requires Python; the installed app does not.
- Code and weight notices are copied into `models/apollo-v1`; the graph is
  materialized there from the app resource when the model pack is prepared.

## Processing

ONNX Runtime CPU, FP32, up to four threads, CPU arena allocator disabled. Audio
is mono or stereo, up to ten minutes, resampled to 44.1 kHz. Longer
recordings are rejected explicitly instead of silently truncating the output. Channels are inferred
independently, matching Apollo's batch/channel flattening. The spectral contract
is an 882-sample periodic Hann window, hop 441, centered reflection padding,
unnormalized FFT, 442 bins, separate real/imaginary inputs and outputs.

Processing uses six-second chunks with a five-second step, one-second linear
crossfades and surrounding context up to eight seconds per inference window.
At file edges the window shifts into available real audio. Repeated identical
context windows are reused. Long-track outputs are streamed to a hidden `.part`
file; no four-stem separation or mix-down is involved. No external normalization
or output gain matching is applied.

Cancellation is checked around each channel inference and before publication.
A cancelled/failed render removes its temporary file. Publication uses exclusive
creation, cleans its own failed copy, and never replaces an existing file.
Apollo restoration and stem separation share an exclusive processing lease;
model operations also keep their own lease while weights are used.

The library row shows source-ID-bound progress even when the dialog is closed.
The dialog can be closed without cancelling, and provides explicit cancellation.
Settings describe Apollo as whole-track restoration rather than a comparison-only
model. A completed restored track is independent of stem groups.

## Validation

On an M1 Pro with 16 GB RAM:

- The spectral ONNX export matched the original PyTorch/MPS model at 80.61–94.79
  dB SNR on the two channels of a one-second music probe.
- Native FFT/ONNX/WAV output matched the original reference at 74.04 dB SNR.
  That one-second stereo file took approximately 5.3 seconds including setup.
- A real 64-kbit/s MP3 decoded to 12.04245 seconds was restored across multiple
  windows in approximately 82.6 seconds. Frame count, channel count, sample rate,
  finite output, original source bytes and absence of temporary files passed.
- A one-second CoreML probe took 2.10 seconds of inference plus 11.52 seconds
  of session setup; static-shape CPU took 1.91 plus 1.08 seconds. This probe
  did not justify enabling CoreML by default.
- These checks establish numerical agreement and safe file handling, not a
  perceptual improvement claim. CPU processing is substantially slower than
  playback; it runs in the background.
- Unit tests cover Hann STFT round-trip, mono/stereo and single-sample chunk
  assembly, context/crossfades, cancellation, collision-safe names, and importing
  an independent track without duplicate source folders.
- Browser checks cover the context action, closed-dialog row progress and the
  original/result playback controls.

For a local native file check, set `APOLLO_MODELS` to the installed `apollo-v1`
folder and `APOLLO_AUDIO` to a 44.1 kHz mono/stereo source, then run the ignored
`apollo::tests::real_music_restoration_matches_reference_and_preserves_original`.
`APOLLO_REFERENCE` optionally names a matching reference WAV for SNR comparison.
