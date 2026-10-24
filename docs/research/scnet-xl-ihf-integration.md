# SCNet XL IHF in Fonicca

25 October 2026. XL IHF is available as the quality option, SCNet-large as the
balanced option, and Spleeter as the fast option. XL is the default for new
preferences; existing valid selections remain unchanged.

## Artifact and signal contract

The pinned FP32 export is from
[Ryan5453/unblend](https://huggingface.co/Ryan5453/unblend/tree/30b749fd691312b1e4b3f8fe79df0b51cde836cd):

- `scnet_xl_wide_v5_fp32.onnx`, 221,482,626 bytes (211.22 MiB).
- SHA-256 `6e2e46b669fa368247afbe867aaa61317425f3f0568309fd6e0c79671c39f0b6`.
- Inputs `spec_real`, `spec_imag`: `[1, 2, 2049, 476]` float32.
- Outputs `out_spec_real`, `out_spec_imag`: `[1, 4, 2, 2049, 476]` float32.
- Model source order drums, bass, other, vocals is mapped to Fonicca's vocals,
  drums, bass, other order.
- Rectangular normalized STFT, FFT 4096, hop 1024; 485,100-sample segments
  padded by 1,300 samples for the spectral contract. Existing overlap-add and
  bounded output streaming are shared with Large.
- External normalization is disabled, matching XL's training/inference config.
  Large retains its existing normalization. Weights and computation are FP32.

The Python ONNX reference was compared with the original PyTorch XL outputs on
all three listening fragments. Across 12 stems, output SNR was 97.29–112.82 dB
and the maximum absolute sample difference was below 0.00000386. This measures
numerical agreement, not separation quality. See
[per-file measurements](scnet-xl-ihf-onnx-parity.json) and the separate
[model comparison](scnet-xl-ihf-comparison.md).

The native runner additionally checks waveform length, finite output, source
order, and agreement with the original reference. The full-file export check
verifies four WAV files, exact frame count, cancellation cleanup, and unchanged
source bytes. The pinned artifact installation test verifies its hash and notices.

## Storage and UI

The pack lives under the application's models directory in `scnet-xl-ihf-v1`.
Download uses a temporary file and checks size/SHA-256 before publishing it.
An installed pack is reused. Model availability, real file sizes, and download
progress are shown in settings; model selection is persisted.

The architecture code is MIT, but the exporter labels these separately trained
weights as unlicensed. The original author's statements about their own weights
are not treated as a license for this checkpoint. See
[the attribution notice](../licenses/scnet-xl-NOTICE.txt).

## Run the native checks

Compile the library tests with `cargo test --manifest-path src-tauri/Cargo.toml --lib`.
For the ignored inference check set `SCNET_VARIANT=xlIhf`, `SCNET_MODELS` to the
directory containing the ONNX file, `STEM_AUDIO` to an input WAV, and
`SCNET_REFERENCE` to the directory of original XL reference stems. Run
`stems::scnet::tests::real_model_inference` with `--ignored --exact --nocapture`.

For `stems::tests::xl_file_export_keeps_original_and_cleans_cancelled_work` and
`stems::models::tests::xl_manifest_and_installation_are_verified`, set
`SCNET_XL_MODELS` to the same ONNX directory. The file-export test also needs
`STEM_AUDIO`. These checks use local weights and do not need another download.
