import assert from 'node:assert/strict'
import { test } from 'node:test'
import { createTimings } from '../src/editor-performance.ts'

test('timings preserve results/errors and bound the percentile window', async () => {
  let clock = 0
  const timings = createTimings(() => clock, 3)
  for (const duration of [1000, 1, 2, 3]) {
    const result = await timings.measure('wave', async () => { clock += duration; return 42 })
    assert.equal(result, 42)
  }
  assert.deepEqual(timings.snapshot()[0], { name: 'wave', calls: 4, errors: 0, lastMs: 3, p50Ms: 2, p95Ms: 3, maxMs: 3 })
  const failure = new Error('original error')
  await assert.rejects(timings.measure('apply', async () => { clock += 5; throw failure }), e => e === failure)
  assert.equal(timings.snapshot()[1]!.errors, 1)
  assert.equal(timings.snapshot()[1]!.lastMs, 5)
})

test('reset drops in-flight results and storage is bounded by operation names', async () => {
  const timings = createTimings(() => 0)
  let finish!: () => void
  const pending = timings.measure('old-session', () => new Promise<void>(resolve => { finish = resolve }))
  timings.reset()
  finish()
  await pending
  assert.deepEqual(timings.snapshot(), [])
  for (let i = 0; i < 100; i++) timings.measureSync(`operation-${i}`, () => i)
  assert.equal(timings.snapshot().length, 32)
})

test('synchronous drawing errors are recorded and still propagate', () => {
  let clock = 0
  const timings = createTimings(() => clock)
  const failure = new Error('draw')
  assert.throws(() => timings.measureSync('draw', () => { clock = 4; throw failure }), e => e === failure)
  assert.equal(timings.snapshot()[0]!.errors, 1)
  assert.equal(timings.snapshot()[0]!.p95Ms, 4)
})
