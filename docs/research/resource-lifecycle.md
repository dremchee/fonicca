# Native resource lifecycle audit

Run the generated-data scenario explicitly:

```sh
node scripts/profile-resources.mjs /tmp/fonicca-memory.json
node scripts/resource-profile/report.mjs /tmp/fonicca-memory.json
```

The memory run requires an unlocked macOS desktop. It launches a separately
identified, ad-hoc signed release `.app` built from a copied checkout. Keep other
builds stopped during observation: changes to the ordinary application binary
invalidate the isolation check. No user library or audio is opened.

The runner generates a private SQLite library of 100,000 metadata rows and a
20-minute stereo float32 WAV. It exercises scrolling, three editor open/edit/undo/
spectrogram/close cycles, transfer to a native window and back, and closing while
spectrogram analysis is pending. The last stage waits for the wave response before
requesting the spectrogram. Earlier observations used a fixed wait; their pending
request snapshots identify which work was outstanding.

The copied frontend records renderer ownership, canvas presence, pending animation
frames, IPC callbacks and outstanding editor requests. The controller records native
RSS separately from the newly appearing WebKit processes and their `footprint` data.
The controller terminates only its owned host and requires the attributed process
cohort to exit. The runner checks that ordinary debug/release binaries stayed
unchanged. Successful generated audio/data are deleted; failures retain their private
fixture and copy diagnostic logs beside the report. Source hashes are captured
before copying and checked against the copy before instrumentation.

These are ownership and lifecycle checks, not proof of JavaScript garbage collection
or GPU driver deallocation. RSS, physical footprint and PCM accounting overlap and
must not be added together. No heap-GC or RSS leak threshold is invented. End-to-end
request/close timing includes IPC, polling and observation overhead; it is not pure
DSP time. Device playback and model inference are outside this scenario.

## Observations on 2026-11-02

- `resource-lifecycle-baseline-2026-11-02.json`: completed release observation before
  cooperative cancellation. All lifecycle/ownership gates pass. Closing during STFT
  left the native process at 622.3 MiB with one pending spectrogram request; it drained
  at 1,455 ms and 218.6 MiB. This showed temporary audio retention by obsolete work,
  rather than establishing a permanent leak.
- `resource-lifecycle-occluded-attempt-2026-11-02.json`: incomplete first attempt,
  rejected because the detached document was hidden. This prompted explicit focus
  and visibility checks at each checkpoint.
- `resource-lifecycle-after-cancellation-2026-11-02.json`: all lifecycle stages ran,
  but the ordinary debug binary changed during the run. The strict isolation gate
  rejects it; its memory values are diagnostic evidence only.
- `resource-lifecycle-verified-2026-11-02.json`: completed repeat after cancellation;
  all 16 stages and 12 recorded gates passed, with unchanged ordinary binaries and
  no source changes. At the first post-close checkpoint (26 ms), the spectrogram
  request had already drained and native RSS was 206.0 MiB. The later drain
  checkpoint includes the intervening OS memory observation overhead, so its
  221 ms timestamp is not the cancellation duration. Closed stages had zero live
  renderer owners, lanes, canvases and RAF; callbacks returned to 45. All attributed
  processes exited. The final request explicitly awaited wave completion; baseline
  pending snapshots also show only spectrum work, but timings are observations
  from separate runs, not a controlled latency benchmark.

The production fix gives each native editor session a cooperative analysis token.
Audio edits, undo/redo, transfer and session destruction cancel old work. STFT checks
the token between columns and before publishing. Metadata-only changes retain
analysis. Frontend staging suppresses spectrum requests until the editor is ready,
and expected cancellation retries silently when the same view is still enabled.

## Packaged platform smoke

```sh
node scripts/profile-resources.mjs --smoke /tmp/fonicca-smoke.json
```

The opt-in `Native lifecycle smoke` workflow runs on pull requests affecting these
paths and by manual dispatch. macOS launches the `.app`; Windows silently installs
and uninstalls a private NSIS package on a GitHub-hosted runner; Linux extracts and
launches an AppImage under Xvfb. Smoke uses 1,000 metadata rows, a 30-second WAV and
one lifecycle cycle. Windows installation is deliberately refused outside a
GitHub-hosted Actions runner. No release is published and no updater credentials
are used. Workflow artifacts contain the report and failure logs.

Passing Node/Rust tests, YAML parsing or a macOS measurement does not establish that
Windows/Linux packaged UI smoke passed. Actual workflow results must be inspected.
