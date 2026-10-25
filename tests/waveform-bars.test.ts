import assert from 'node:assert/strict'
import test from 'node:test'
import { barCornerRadius, waveBars } from '../src/waveform-bars.ts'

function wave(peaks: number[]) {
  return { samples: false, lanes: 1, count: peaks.length, stride: 6,
    values: Float32Array.from(peaks.flatMap(peak => [-peak, peak, peak / 2, 1, 0, 0])) }
}

test('each discrete bar takes the maximum across its whole cell, without a sloping cap', () => {
  const bars = waveBars(wave([0.125, 0.5, 1, 0.25, 0.125, 0]), 0, 8, 1)
  assert.deepEqual(bars.map(bar => bar.peak), [0.5, 1, 0.125])
  assert.deepEqual(bars.map(bar => bar.x), [0, 3, 6])
  assert.ok(bars.every(bar => bar.width === 2))
  // An impulse in the visual gap still determines the adjacent bar's height.
  assert.equal(bars[1].index, 2)
})

test('bars keep full widths and pixel-aligned gaps at fractional display scales', () => {
  for (const dpr of [1, 1.25, 1.5, 2, 3]) {
    for (const width of [1, 2, 8, 49, 651.5, 652]) {
      const bars = waveBars(wave([0.25, 1, 0.5]), 0, width, dpr)
      assert.ok(bars.every(bar => Math.abs(bar.x * dpr - Math.round(bar.x * dpr)) < 1e-6))
      assert.ok(bars.every(bar => bar.x >= 0 && bar.x + bar.width <= width + 1e-6))
      assert.ok(bars.every(bar => bar.width === bars[0].width))
      for (let i = 1; i < bars.length; i++)
        assert.ok(bars[i].x > bars[i - 1].x + bars[i - 1].width)
    }
  }
})


test('custom widths and gaps preserve full bars and all source peaks', () => {
  const data = wave([0.125, 1, 0.25, 0.5, 0.125, 0])
  const bars = waveBars(data, 0, 14, 2, { width: 4, gap: 1 })
  assert.deepEqual(bars.map(bar => [bar.x, bar.width, bar.peak]), [[0, 4, 1], [5, 4, 0.5], [10, 4, 0.125]])
  const joined = waveBars(data, 0, 12, 1, { width: 4, gap: 0 })
  assert.deepEqual(joined.map(bar => bar.x), [0, 4, 8])
  assert.equal(Math.max(...joined.map(bar => bar.peak)), 1)
  assert.equal(barCornerRadius(4, 20, 0), 0)
  assert.equal(barCornerRadius(4, 20, 1), 2)
  assert.equal(barCornerRadius(4, 1, 1), 0.5)
})
