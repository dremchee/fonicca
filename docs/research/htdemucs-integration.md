# Standard HTDemucs in Fonicca — 25 October 2026

HTDemucs is an additional four-stem option. SCNet XL IHF remains the default
for existing and new preferences. This is the standard single-network model,
not the previously removed HTDemucs FT ensemble. Installed FL Studio refers to
Demucs, but its exact checkpoint has not been identified; equal output or equal
performance with FL Studio is not claimed.

## Pinned artifact

Source: [StemSplitio/htdemucs-onnx](https://huggingface.co/StemSplitio/htdemucs-onnx/tree/d54ed9eb60e258ea82131c6ee14578628816456a).
`htdemucs.onnx`: 316,446,953 bytes (301.79 MiB), SHA-256
`68d0bf16428ef66e692cdff8a9ccf28f1ef3f69440d57e58605a4cc55fcc5e74`.
The FP32 ONNX export includes signal-transform kernels, so its size differs
from the author's original checkpoint. It is installed in `htdemucs-v1`.
The application checks size and hash before reuse or publishing a download.

Input: float32 `mix` `[1,2,343980]`. Output: float32 `stems`
`[1,4,2,343980]`, ordered drums/bass/other/vocals. All four predictions come
from one invocation per window and are reordered to the application's
vocals/drums/bass/other order.

Framing follows upstream `apply_model` / `TensorChunk`: 7.8-second windows,
25% overlap, positive triangular windows, centred padding with neighbouring
track context, and no random shifts. Whole-track mono mean and sample standard
deviation normalize the input. Scale is restored; DC is not added to each
stem, matching Fonicca's normalized SCNet pipeline. Outputs stream through a
single window-sized overlap buffer per stem. Original files are unchanged;
output names, publication and cleanup use the shared stem export pipeline.

## Validation and performance

M1 Pro, 16 GB, ONNX Runtime CPU, four workers, spinning and CPU arena disabled.
Three MUSDB18 previews, 6.8034467 seconds each, ran through the actual native
runner. Inference including DSP took 5.87 / 5.18 / 5.18 seconds. Complete ignored
test executions, including model load and reference verification, took
8.67 / 7.52 / 7.43 seconds. The tuned XL runner took 26.38 seconds including
load and DSP on the first preview; different models and framing are being
compared, so this does not establish equal quality or long-track throughput.

All twelve native stems matched the independent whole-track NumPy/ORT
reference at 119.67–128.83 dB SNR, with exact frame counts and finite output.
`compare-htdemucs.py` generates that reference; measured SI-SDR against the
original dataset stems is in `htdemucs-comparison.json`. Three short previews
are insufficient for a general quality ranking. The all-stem overlap test
also covers short mono, stereo, boundaries crossing multiple windows, source
order, gain, cancellation and invalid predictions.

The normal Rust suite and frontend build pass. Ignored checks additionally
validate the installed pack's SHA-256 and license notices, actual model
inference, and full file export / cancellation cleanup. The real 12-second file exported
in 13.05 seconds including model load and WAV writing; the subsequent cancel
check preserved the original bytes and left exactly four completed outputs.

## Licensing

Architecture and code are MIT; the original author's separate statement
restricts pretrained weights to scientific use. The third-party ONNX card's
MIT tag does not supersede that statement. The selector and settings expose
this restriction; installation writes code attribution and a weight notice.
See `../licenses/htdemucs-NOTICE.txt`. No claim of permission for a commercial
release is made.
