# Apollo after SCNet XL IHF

Local experiment, 25 October 2026. This does not add Apollo to the application.

Apollo's official model was applied to the existing SCNet XL IHF outputs from
the same three MUSDB18 test previews used in `scnet-xl-ihf-comparison.md`:
Cristina Vane — So Easy, Louis Cressy Band — Good Time, and Secretariat — Borderline.
All four stems per song were processed: 12 stereo files, 300,032 frames each,
44,100 Hz, 6.803446712 seconds. The original inputs remain unchanged.

## Pinned model and execution

- Source: [JusperLee/Apollo](https://github.com/JusperLee/Apollo/tree/e84bcacc59d5455f05d86a5c97dd4aeb3c14dbb6).
- Checkpoint: [official pytorch_model.bin](https://huggingface.co/JusperLee/Apollo/blob/c68bd80fdd9c0d93d2f4a833cb154624f660a561/pytorch_model.bin),
  66,541,845 bytes, SHA-256 `99d9af7f1ff20e63c393035513a655392818d66b4d7fc23d658175c1f15e8d76`.
- Official architecture: `sr=44100, win=20, feature_dim=256, layer=6`, strict
  state-dictionary loading. The checkpoint's upstream deserialization format
  includes OmegaConf metadata; the script checks its exact hash before loading.
- Apple M1 Pro, 16 GB unified memory; PyTorch 2.14.1, MPS, float32.
  CPU thread count 4, interop threads 1. `PYTORCH_ENABLE_MPS_FALLBACK=1`.
- Full-file inference, no chunking, external normalization, or trimming.
  The source's documented edge behavior is retained, including at file ends.
- FLOAT WAV output; exact frame/channel count, finite samples, unchanged
  source hashes, and lossless float32 save/readback verified for all 12 files.

## Measurements

SI-SDR uses flattened stereo with means removed, matching the preceding XL
experiment. Scores compare each output with the real separate reference stem.
They are not BSSEval SDR and are not a perceptual listening score.

| Stem | SCNet XL IHF | After Apollo | Difference |
| --- | ---: | ---: | ---: |
| Vocals | 13.65 dB | 13.43 dB | −0.23 dB |
| Drums | 10.53 dB | 9.26 dB | −1.27 dB |
| Bass | 6.63 dB | 6.52 dB | −0.12 dB |
| Other | 7.98 dB | 4.44 dB | −3.54 dB |
| All 12 | 9.70 dB | 8.41 dB | −1.29 dB |

SI-SDR decreased in 10 of 12 cases. Apollo is trained for restoration after MP3
compression, so improvement after source separation is not established by its
original task. These small-sample results do not support enabling it by default.
Listening is still needed to assess the changes in timbre and artifacts.

Total measured inference time was 47.56 seconds for 81.64 seconds of audio.
The first file took 7.12 seconds including first-run overhead inside inference;
subsequent files took approximately 3.52–4.04 seconds each. These are local
measurements, not throughput guarantees. Peak CPU RSS was 569,081,856 bytes.
The largest observed post-forward MPS driver allocation was 5,302,288,384 bytes;
this is an allocation snapshot, not an independently sampled total memory peak.

Raw output RMS increased by 0.39–1.14 dB. Maximum output amplitude was 0.61432;
none exceeded ±1. The listening page offers equal-RMS Apollo copies by default
and a toggle for raw model output. Reference audio is unmodified; measured
SI-SDR always uses raw output. Equal-RMS playback does not establish perceptual
loudness equality.

## Reproduce

Download the pinned source and checkpoint above into a cache directory with
`source/`, `pytorch_model.bin`, and a `manifest.json` containing the revisions and
checkpoint metadata in `apollo-comparison.json`. Use an isolated Python
environment with PyTorch, numpy, soundfile, huggingface-hub 1.27.0, and
omegaconf 2.3.0. Then run:

```sh
python docs/research/compare-apollo.py \
  --apollo /path/to/apollo-cache \
  --stems /path/to/scnet-xl-cache/outputs \
  --references /path/to/stems-comparison \
  --output /path/to/apollo-cache/outputs \
  --device mps
```

`--references` contains `<track>/reference/<stem>.wav`. Outputs and per-file
metrics are written separately. Detailed results, model metadata, and hashes
are recorded in `apollo-comparison.json`.

Listening artifact: `http://127.0.0.1:8766/comparison-apollo.html`, alongside
`comparison-xl.html`. The page includes all 12 before/after/reference groups,
the three source mixes, exclusive playback, and the equal-RMS toggle.
