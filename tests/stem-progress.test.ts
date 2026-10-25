import assert from 'node:assert/strict'
import { test } from 'node:test'
import { activeStemProgress } from '../src/stem-progress.ts'

test('progress belongs to the job source even when another sample is selected', () => {
  const progress = activeStemProgress({ sourceId: 42, running: true, stage: 'separating', progress: 0.456 })!
  assert.equal(progress.sourceId, 42)
  assert.equal(progress.percent, 46)
  assert.equal(progress.indeterminate, false)
})

test('finished, cancelled and unidentified jobs leave no row progress behind', () => {
  for (const stage of ['done', 'cancelled', 'failed'])
    assert.equal(activeStemProgress({ sourceId: 42, running: false, stage, progress: 1 }), null)
  assert.equal(activeStemProgress({ sourceId: null, running: true, stage: 'separating', progress: 0.5 }), null)
})

test('preparing and importing do not pretend to have measurable progress', () => {
  for (const stage of ['preparing', 'importing'])
    assert.equal(activeStemProgress({ sourceId: 42, running: true, stage, progress: 0 })?.indeterminate, true)
  for (const [input, expected] of [[-1, 0], [2, 100], [NaN, 0], [Infinity, 0]])
    assert.equal(activeStemProgress({ sourceId: 42, running: true, stage: 'downloading', progress: input })?.percent, expected)
})
