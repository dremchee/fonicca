# Development

Fonicca uses Tauri 2, Rust, Vue 3 and TypeScript. The library is stored locally in SQLite, and ONNX Runtime runs the audio models.

## Run from source {#run-from-source}

Install Node.js 26, pnpm 9.10.0, the Rust toolchain from `rust-toolchain.toml` and the [Tauri prerequisites for your platform](https://v2.tauri.app/start/prerequisites/). Then run:

```sh
git clone https://github.com/dremchee/fonicca.git
cd fonicca
pnpm install --frozen-lockfile
pnpm tauri dev
```

## Project references {#project-references}

- [Architecture and module boundaries (Russian)](https://github.com/dremchee/fonicca/blob/main/docs/architecture.md)
- [Desktop builds and release process](https://github.com/dremchee/fonicca/blob/main/docs/builds.md)
- [Dependencies and model usage terms (Russian)](https://github.com/dremchee/fonicca/blob/main/docs/dependencies.md)
- [Downloadable model graphs](https://github.com/dremchee/fonicca/blob/main/model-packs/README.md)
- [Third-party license texts and notices](https://github.com/dremchee/fonicca/tree/main/docs/licenses)
