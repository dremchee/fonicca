import assert from 'node:assert/strict'
import { test } from 'node:test'
import { buildOverview } from '../experiments/waveform/overview.ts'

test('comparison overview preserves an isolated one-sample peak and its RMS', () => {
  const pcm = new Float32Array(10000)
  pcm[5299] = 1
  const wave = buildOverview([pcm], 0, pcm.length, 10)
  assert.equal(wave.samples, false)
  assert.equal(wave.values[5 * 6 + 1], 1)
  assert.ok(Math.abs(wave.values[5 * 6 + 2] - Math.sqrt(1 / 1000)) < 1e-7)
  assert.equal(wave.values[4 * 6 + 1], 0)
})
test('comparison keeps stereo channels separate and exposes original samples at close zoom', () => {
  const left = Float32Array.from([0, .2, .5, 1, 0, -.5, -1, 0])
  const right = Float32Array.from(left, x => -x)
  const coarse = buildOverview([left, right], 0, 8, 2)
  assert.equal(coarse.lanes, 2)
  assert.equal(coarse.values[1], 1)
  assert.equal(coarse.values[12], -1)
  const fine = buildOverview([left], 2, 6, 100)
  assert.equal(fine.samples, true)
  assert.deepEqual([...fine.values], [.5, 1, 0, -.5])
})
