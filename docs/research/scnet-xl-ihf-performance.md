# SCNet XL IHF CPU tuning — 25 October 2026

XL now uses up to six performance cores on Apple Silicon, bounded by the
available CPU count. The performance-core count comes from
`sysctl -n hw.perflevel0.physicalcpu`, with a conservative four-worker fallback.
Other platforms and SCNet-large retain the four-worker limit. XL's CPU worker
spinning is disabled; the CPU memory arena remains disabled.

The model file, FP32 computation, STFT, normalization, 11-second windows,
75% overlap, fades, source order, and output streaming are unchanged.

## Measurement

M1 Pro, six performance cores plus two efficiency cores, 16 GB RAM.
ONNX Runtime 1.31.0, same installed ONNX file and the same reflected/padded
Cristina Vane 6.8034467-second test excerpt. These times measure inference only,
with no build running concurrently:

| CPU configuration | Inference |
| --- | ---: |
| Four workers, spinning enabled (previous configuration) | 30.42 s |
| Four workers, spinning disabled | 29.53 s |
| Six workers, spinning disabled, first / second run | 23.43 / 25.20 s |

This is approximately 17–23% less elapsed inference time than the previous
configuration (1.21–1.30 times the throughput). It is a small local benchmark,
not a guarantee for other machines or long tracks. A one-worker trial was
stopped after exceeding the four-worker latency substantially.

Core ML was also tested, but is not enabled in the application. MLProgram
covered only 206 of 1819 optimized graph nodes and took 28.78 / 28.56 s;
NeuralNetwork covered 80 nodes and took 28.83 / 27.98 s. Most computation
remained on CPU, so these results do not justify an additional runtime backend.

## Signal validation

The native ignored `stems::scnet::tests::real_model_inference` check compared
the tuned runner with four WAV files saved by the previous native runner.
All four outputs preserved length, finite samples, and source order. SNR
against the previous result was 227–235 dB, effectively numerical agreement.
The first check ran during a background rebuild and was excluded from timing.
A subsequent native check without a rebuild took 26.38 seconds including load
and DSP, compared with 37.16 seconds before tuning on the same excerpt.
This is a local end-to-end observation; the inference-only comparison above
uses repeated runs and does not promise that ratio on every track.

To reproduce the signal check, set `SCNET_VARIANT=xlIhf`, `SCNET_MODELS` to
the installed XL directory, `STEM_AUDIO` to the test WAV, and `SCNET_REFERENCE`
to the saved baseline stems, then run:

```sh
cargo test --manifest-path src-tauri/Cargo.toml --lib \
  stems::scnet::tests::real_model_inference -- --ignored --exact --nocapture
```

The change applies to newly loaded separation sessions. Existing in-flight
sessions cannot change their worker count.
