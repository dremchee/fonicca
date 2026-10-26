# Desktop builds

The `Desktop builds` GitHub Actions workflow produces installers on each platform:

| Platform | Architecture | Runner | Packages |
| --- | --- | --- | --- |
| macOS 13.4+ | Apple Silicon (ARM64) | `macos-15` | `.dmg` |
| Windows | x64 | `windows-2022` | NSIS `.exe`, WiX `.msi` |
| Linux | x64 | `ubuntu-22.04` | `.deb`, `.AppImage` |

## Run and download

The workflow runs on pushes to `main`, pull requests targeting `main`, and `v*` tags.
It can also be started from **Actions → Desktop builds → Run workflow**, selecting
an available branch or tag. A workflow must be on the default branch before
GitHub exposes its manual trigger.

Open the completed run and download the platform archive from **Artifacts**, or
use the download link in the job summary. Artifacts are kept for 14 days. Each
matrix job is independent: failure on one platform does not cancel the others.

Branch and pull-request builds stay in Actions artifacts. A `v*` tag also creates
or updates a **draft GitHub Release** once all three platform builds succeed. The
draft contains the five installer formats, a `SHA256SUMS.txt` checksum file, and
automatically generated release notes. Publish it manually after checking it.

These builds are not distribution-signed or notarized. No custom repository
secrets are required: only the release job receives `contents: write` via the
built-in `GITHUB_TOKEN`.

## Release by tag

1. Set the same version in `package.json`, `[package].version` in
   `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, and the `fonicca` package
   entry in `src-tauri/Cargo.lock` (Cargo can synchronize its lockfile with
   `cargo update --workspace --manifest-path src-tauri/Cargo.toml`).
2. Check the intended tag locally, then commit and push the changes:

   ```sh
   pnpm release:check v0.1.1
   git add package.json src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/tauri.conf.json
   git commit -m "Release 0.1.1"
   git push origin main
   git tag -a v0.1.1 -m "Fonicca 0.1.1"
   git push origin v0.1.1
   ```

3. Wait for **Desktop builds**, then open the draft from the release job summary
   or the repository's Releases page. Review the installers and notes, then choose
   **Publish release**.

The example assumes the version has already been changed to `0.1.1`; it does not
bump versions for you. CI checks all four files before starting expensive builds
and rejects a tag that does not exactly match `v<app version>`.

Prerelease versions such as `0.2.0-beta.1` use the tag `v0.2.0-beta.1`; their release
draft is marked as a prerelease. A manual run on a branch only builds artifacts;
a manual run on a matching `v*` tag can recreate or update its draft.

Re-running a successful tag build replaces files only while its release is still
a draft. The workflow refuses to change an already published release. For a new
published build, use a new version and tag instead of moving an existing tag.

The workflow files and release scripts must be committed and pushed before
creating the first tag. Configuring the workflow does not create a tag or publish
a release by itself.

## Reproducibility and checks

- Node.js 26 and the package manager recorded in `package.json` (pnpm 9.10.0).
- `pnpm install --frozen-lockfile` and the committed `src-tauri/Cargo.lock`.
- Frontend tests run before packaging. Tauri's `beforeBuildCommand` runs the
  TypeScript check and frontend production build on each runner.
- Rust stable compiles the native application for the explicit matrix target.
- pnpm and Cargo dependency caches speed up later runs. Only pushes to `main`
  save the Rust cache.
- Missing installer output fails the job instead of silently uploading nothing.

## ONNX Runtime and macOS Intel

The pinned `ort = 2.0.0-rc.12` downloads CPU libraries for the three matrix targets
and links ONNX Runtime statically. Its prebuilt distribution does **not** include
`x86_64-apple-darwin`; Intel Mac builds need a separate compatible runtime build
and are intentionally absent from the matrix. The supported runtime requires
macOS 13.4 or later, reflected in `tauri.conf.json`.

## Local equivalent

```sh
pnpm install --frozen-lockfile
pnpm test
pnpm tauri build --ci -- --locked
```

Install the platform prerequisites before building locally. Bundles are generated
by that platform's native tools; a macOS machine cannot validate Linux or Windows
installers. The three hosted jobs provide that verification.

References: [Tauri GitHub Actions guide](https://v2.tauri.app/distribute/pipelines/github/),
[ort release notes](https://github.com/pykeio/ort/releases/tag/v2.0.0-rc.11).
