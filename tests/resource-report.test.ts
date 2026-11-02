import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
// @ts-expect-error Opt-in Node script has no separate type declarations.
import { auditResourceReport } from '../scripts/resource-profile/report.mjs'

const fixture = () => JSON.parse(readFileSync(new URL('../docs/research/resource-lifecycle-baseline-2026-11-02.json', import.meta.url), 'utf8'))

test('resource audit accepts a completed native lifecycle observation', () => {
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
