# Downloadable model graphs

These files are distribution assets, not application resources. They are not
embedded in the executable or copied into desktop installers. Fonicca downloads
them on demand into its application-data `models` directory, together with the
separately hosted original weights. Downloads must match the size and SHA-256
in [`catalog.json`](catalog.json) before becoming available for inference.

## Catalog maintenance

`catalog.json` is the authoritative download metadata for CLAP, the seven stem
models, Apollo and Basic Pitch: names, groups, stem modes, directories, artifacts,
exact byte sizes, SHA-256 hashes and download URLs. Localized captions add their
own descriptions to the canonical names.

After editing it, run:

```sh
pnpm models:generate
pnpm models:check
```

Commit the catalog and both generated files:
`src/model-catalog.generated.ts` and `src-tauri/src/stems/catalog.generated.rs`.
The interface and Rust downloader consume those generated files. Generation is
deterministic; normal checks and builds do not rewrite the source checkout.
The Node check compares the complete generated contents, and the native build
rejects a catalog whose synchronization fingerprint is outdated. Line endings
are normalized so Windows checkouts agree with Linux and macOS.

The synchronization fingerprint is independent of the SHA-256 verification of
downloaded model bytes. Adding a model still requires a matching Rust enum variant,
inference implementation and tensor/preprocessing contract. Changing download
metadata does not add support for a new architecture.

## v1

The graph artifacts are `bs-roformer-native.onnx` and `mdx23c.onnx`. Their pinned
sizes, hashes and URLs are recorded in the catalog; tests verify the files in
this checkout against it.

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
