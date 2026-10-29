# Downloadable model graphs

These files are distribution assets, not application resources. They are not
embedded in the executable or copied into desktop installers. Fonicca downloads
them on demand into its application-data `models` directory, together with the
separately hosted original weights. Downloads must match the size and SHA-256
in `src-tauri/src/stems/models.rs` before becoming available for inference.

## v1

| Graph | Bytes | SHA-256 |
| --- | ---: | --- |
| `bs-roformer-native.onnx` | 4,871,339 | `320e61bbb30a3bb0cba7b88c013b02b0acef444c3341667158b1bc1611664af9` |
| `mdx23c.onnx` | 221,595 | `eb6f147119a27b8dd7be4ee8069ef19de115f03656cc054693bccf23c15fa599` |

Download base: `https://raw.githubusercontent.com/dremchee/fonicca/main/model-packs/v1/`.
Publish this directory to the public `main` branch **before distributing a build
that uses these URLs**. Until then, fresh installations cannot download these two
graphs. Existing valid graphs in the models cache are reused offline.

Treat version directories as immutable. Publish changed graphs in a new directory
and update the pinned download sizes/hashes/URLs together. A changed remote file
is rejected by the current application's hash check.

Reproduction and attribution:

- `docs/research/prepare-bs-roformer-onnx.py`
- `docs/research/export-mdx23c-onnx.py`
- `docs/research/mdx23c-onnx-manifest.json`
- `docs/licenses/vocal-models-NOTICE.txt`
- `docs/licenses/vocal-models-MIT.txt`

The graph files contain structure and small constants, not learned float weights.
The license notes distinguish code/export licensing from pretrained weight terms.
