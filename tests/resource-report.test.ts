import assert from 'node:assert/strict'
import { test } from 'node:test'
// @ts-expect-error Opt-in Node script has no separate type declarations.
import { auditResourceReport } from '../scripts/resource-profile/report.mjs'

function fixture() {
  const nativeHostPid = 100
  const checkpoint = (stage: string, open = false, window = 'main') => ({
    stage,
    window,
    visibility: 'visible',
    screenLocked: false,
    processes: [{ pid: nativeHostPid, rssBytes: 1024 }],
    callbacks: 4,
    native: { editor: open ? {} : null },
    editor: { open },
    canvases: open ? 1 : 0,
    resources: { live: open ? 1 : 0, lanes: open ? 1 : 0 },
    pendingRaf: 0,
    library: { retainedPages: 2, maxPages: 4 },
  })
  return {
    mode: 'memory',
    status: 'measured',
    finished: { ok: true },
    nativeHostPid,
    normalBinary: { unchanged: true },
    sourceChangedDuringRun: [],
    cohortRemainingAfterHostExit: [],
    checkpoints: [
      checkpoint('baseline'),
      checkpoint('library-scrolled'),
      checkpoint('cycle-1-opened', true),
      checkpoint('cycle-1-spectrum', true),
      checkpoint('cycle-1-closed'),
      checkpoint('cycle-2-opened', true),
      checkpoint('cycle-2-spectrum', true),
      checkpoint('cycle-2-closed'),
      checkpoint('cycle-3-opened', true),
      checkpoint('cycle-3-spectrum', true),
      checkpoint('detached-rendered', true, 'editor-detached-1'),
      checkpoint('cycle-3-returned', true),
      checkpoint('cycle-3-closed'),
      checkpoint('closed-with-pending-spectrum'),
      checkpoint('pending-work-drained'),
      checkpoint('settled-after-close'),
    ],
  }
}

test('resource audit accepts a completed synthetic lifecycle observation', () => {
  assert.equal(auditResourceReport(fixture()).passed, true)
})

test('resource audit rejects incomplete evidence and retained native/UI resources', () => {
  const mutations = [
    (r: any) => { r.finished.ok = false },
    (r: any) => { r.checkpoints = r.checkpoints.filter((p: any) => p.stage !== 'detached-rendered') },
    (r: any) => { r.normalBinary.unchanged = false },
    (r: any) => { r.sourceChangedDuringRun = ['src/main.ts'] },
    (r: any) => { delete r.checkpoints[0].processes },
    (r: any) => { r.cohortRemainingAfterHostExit = [123] },
    (r: any) => { r.checkpoints.find((p: any) => p.stage === 'cycle-1-closed').resources.live = 1 },
    (r: any) => { r.checkpoints.find((p: any) => p.stage === 'cycle-1-closed').pendingRaf = 1 },
    (r: any) => { r.checkpoints.find((p: any) => p.stage === 'pending-work-drained').pendingEditor = { editor_spectrogram: 1 } },
    (r: any) => { r.checkpoints.find((p: any) => p.stage === 'cycle-1-closed').callbacks += 2 },
  ]
  for (const mutate of mutations) {
    const report = fixture()
    mutate(report)
    assert.equal(auditResourceReport(report).passed, false)
  }
})
