# Desktop builds

The `Desktop builds` GitHub Actions workflow produces installers on each platform:

| Platform | Architecture | Runner | Packages |
| --- | --- | --- | --- |
| macOS 13.4+ | Apple Silicon (ARM64) | `macos-15` | `.dmg` |
| Windows | x64 | `windows-2022` | NSIS `.exe`, WiX `.msi` |
| Linux (Ubuntu 24.04+) | x64 | `ubuntu-24.04` | `.deb`, `.AppImage` |

## Run and download

The workflow runs on pushes to `main`, pull requests targeting `main`, and `v*` tags.
It can also be started from **Actions → Desktop builds → Run workflow**, selecting
an available branch or tag. A workflow must be on the default branch before
GitHub exposes its manual trigger.

Pushes to `main`, pull requests and manual runs on branches execute a single
Ubuntu 24.04 checks job: frontend tests, TypeScript/frontend build and Rust tests
(which compile and link the native code). They do not package or upload installers.

Only matching `v*` tags run the three-platform installer matrix. This also applies
to manual runs: select a version tag to build installers, or a branch for checks.

Open the completed tag run and download the platform archive from **Artifacts**, or
use the download link in the job summary. Artifacts are kept for 14 days. Each
matrix job is independent: failure on one platform does not cancel the others.

A `v*` tag creates or updates a **draft GitHub Release** once all three platform
builds succeed. The draft contains the five installer formats, a `SHA256SUMS.txt` checksum file, and
automatically generated release notes. Publish it manually after checking it.

These builds are not distribution-signed or notarized. Branch and pull-request
builds need no signing secrets. Release tags require `TAURI_SIGNING_PRIVATE_KEY`
to produce signed updater packages. The release job receives `contents: write`
via the built-in `GITHUB_TOKEN` to publish a draft in this repository.
Updater signatures are separate from Apple notarization and Windows code signing.

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

3. Optionally add `docs/releases/v<version>.md` before tagging. CI uses this file
   as the draft description and the in-app update notes. If it is missing, the
   same-repository release uses automatically generated notes.
4. Wait for **Desktop builds**, then open the draft from the release job summary
   or the repository's Releases page. Review the installers and notes, then choose
   **Publish release**.

The example assumes the version has already been changed to `0.1.1`; it does not
bump versions for you. CI checks all four files before starting expensive builds
and rejects a tag that does not exactly match `v<app version>`.

Prerelease versions such as `0.2.0-beta.1` use the tag `v0.2.0-beta.1`; their release
draft is marked as a prerelease. A manual run on a branch only runs checks;
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
- Branch/PR checks include frontend and Rust unit tests; frontend tests also run
  before release packaging. Tauri's `beforeBuildCommand` runs the
  TypeScript check and frontend production build on each runner.
- Rust stable compiles the native application for the explicit matrix target.
- pnpm and Cargo dependency caches speed up later runs. Native checks and release
  builds use separate Rust caches. Main checks and release builds save caches;
  pull requests only restore them.
- Missing installer output fails the job instead of silently uploading nothing.

## ONNX Runtime and macOS Intel

The pinned `ort = 2.0.0-rc.12` downloads CPU libraries for the three matrix targets
and links ONNX Runtime statically. Its prebuilt distribution does **not** include
`x86_64-apple-darwin`; Intel Mac builds need a separate compatible runtime build
and are intentionally absent from the matrix. The supported runtime requires
macOS 13.4 or later, reflected in `tauri.conf.json`.

## Linux runtime baseline

The prebuilt ONNX Runtime archive references the `__isoc23_*` conversion functions
introduced in glibc 2.38. Ubuntu 22.04 ships glibc 2.35 and cannot link that archive.
Linux installers are therefore built on Ubuntu 24.04 (glibc 2.39), with the matching
`libfuse2t64` package. The Rust cache key includes the runner image to avoid reusing
native build outputs from the older image. The resulting Linux build targets
Ubuntu 24.04 or compatible newer systems; AppImage does not bundle glibc.

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


## Automatic updates

The public `dremchee/fonicca` repository hosts both source code and releases.
Production builds check this feed at startup and from **Settings → About → Updates**:

`https://github.com/dremchee/fonicca/releases/latest/download/latest.json`

Downloads show progress and are verified with the public signing key bundled in
the app. Installation requires confirmation and rechecks native state: all editor
windows must be saved and closed, and processing, model operations and duplicate
verification must finish. Windows exits to run its installer; macOS and AppImage
restart after installation. Development builds do not check for updates. Debian
packages are updated through the package manager.

### One-time signing setup

1. In the repository’s **Settings → Secrets and variables → Actions → Secrets**,
   add `TAURI_SIGNING_PRIVATE_KEY` using the full contents of the local signing
   key. On the machine where this integration was prepared, the key is
   `~/.tauri/fonicca-updater.key`, outside the checkout. Its public sibling
   `.key.pub` matches `plugins.updater.pubkey` in `src-tauri/tauri.conf.json`.
   The key currently has an empty password. If using a password-protected key,
   also set `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.
2. Keep a secure backup: future updates must be signed by the key trusted by
   installed versions. Never commit it or paste it into a release or issue.

   With an authenticated GitHub CLI, upload it without printing it:

   ```sh
   gh secret set TAURI_SIGNING_PRIVATE_KEY --repo dremchee/fonicca < ~/.tauri/fonicca-updater.key
   ```

3. Bump the version, commit and push the matching `v*` tag as described above.
   CI signs updater packages, verifies the required artifacts and writes
   `latest.json`. The draft is created in this repository using `GITHUB_TOKEN`;
   no personal access token or `UPDATE_REPOSITORY` variable is required.
4. Review the release notes and publish the draft. Draft assets are not available
   to ordinary users. Only published stable releases enter the stable update
   channel; prereleases do not.

### Optional separate downloads repository

If source code is made private in the future, initialize a public binary-only
repository and set the Actions variable `UPDATE_REPOSITORY` to `owner/repository`.
For a different repository, CI additionally requires a `RELEASE_TOKEN` secret:
use a fine-grained token with **Contents: Read and write**, scoped only to that
public downloads repository. The built-in token cannot publish across repositories.
Using the current repository as the variable value still uses `GITHUB_TOKEN`.
The release build generates its feed URL from this variable; tokens are never
embedded in the application. A changed feed requires users to install a version
that knows the new address before the old feed is removed.

Signed releases include the usual five installer formats plus the macOS
`.app.tar.gz` updater archive and `.sig` files. The feed maps `darwin-aarch64` to
the macOS archive, `windows-x86_64` to NSIS, and `linux-x86_64` to AppImage.
Checksums cover installers, update archives, signatures and the manifest.

**The first version with an enabled feed must be installed manually.** Older
versions without the updater or with updates disabled cannot discover it. From
that version onward, publishing a higher version makes it available on the next
app launch or manual check. Users do not need GitHub accounts.

For a local signed build, set `TAURI_SIGNING_PRIVATE_KEY` to the key’s absolute
path and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` as needed, then provide the same
configuration overlay as CI:

```sh
export UPDATE_REPOSITORY=dremchee/fonicca
export TAURI_SIGNING_PRIVATE_KEY="$HOME/.tauri/fonicca-updater.key"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=''
pnpm tauri build --ci --config "$(node scripts/updater-config.mjs)" -- --locked
```

See the [Tauri updater documentation](https://v2.tauri.app/plugin/updater/) for
platform-specific installation and signing behavior.
