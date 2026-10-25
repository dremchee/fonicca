import assert from 'node:assert/strict'
import test from 'node:test'
import { glLane, laneAmplitude, summarySpan } from '../src/waveform-gl-data.ts'

const wave = {
  samples: false, lanes: 2, count: 2, stride: 6,
  values: new Float32Array([
    -0.5, 1, 0.25, 1, 0, 0, -0.125, 0.5, 0.125, 0, 1, 0,
    -1, 0.5, 0.5, 0, 0, 1, -0.25, 0.125, 0.125, 1, 1, 0,
  ]),
}

test('summary coordinates keep the renderer above its summary-only zoom threshold', () => {
  for (const [columns, width] of [[8192, 2048], [400, 1200], [1, 4000]]) {
    const span = summarySpan(columns, width)
    assert.ok(columns * span / width >= 1024)
    assert.equal(span % 256, 0)
  }
})

test('overview leaves retain extrema, mean square and independent stereo lanes', () => {
  const span = summarySpan(wave.count, 2)
  const left = glLane(wave, 0, span, x => x)
  const right = glLane(wave, 1, span, x => x)
  assert.deepEqual([...left.slice(0, 4)], [-0.5, 1, 16, 256])
  assert.deepEqual([...right.slice(0, 4)], [-1, 0.5, 64, 256])
  let sum = 0
  let count = 0
  for (let i = 0; i < left.length / 2; i += 4) {
    sum += left[i + 2]
    count += left[i + 3]
  }
  assert.equal(Math.sqrt(sum / count), 0.25)
  assert.deepEqual([...left.slice(span / 256 * 4, span / 256 * 4 + 4)], [-0.125, 0.5, 4, 256])
})

test('amplitude transforms apply independently to both extrema and RMS', () => {
  const leaves = glLane(wave, 0, 1024, x => x / 2)
  assert.deepEqual([...leaves.slice(0, 4)], [-0.25, 0.5, 4, 256])
})

test('sample mode passes exact lane samples through the amplitude transform', () => {
  const samples = { samples: true, lanes: 2, count: 3, stride: 1, values: new Float32Array([0, 0.5, -1, 1, 0.25, -0.5]) }
  assert.deepEqual([...glLane(samples, 1, 1024, x => x / 2)], [0.5, 0.125, -0.25])
  assert.equal(samples.values[3], 1)
})


test('full-scale peaks, strokes and sample dots fit inside short channel lanes', () => {
  for (const height of [16, 17, 37, 49, 98, 151, 287]) {
    for (const dpr of [1, 1.25, 1.5, 2, 3]) {
      for (const thickness of [1, 2, 3]) {
        for (const spacing of [0, 6, 9, 12, 24]) {
          const h = Math.round(height * dpr)
          const hw = Math.max(thickness * dpr / 2, 0.5)
          const center = Math.round(2 * hw) % 2 ? Math.floor(h / 2) + 0.5 : Math.round(h / 2)
          const t = Math.max(0, Math.min(1, (spacing - 6) / 6))
          const radius = hw + 1.5 * dpr * t * t * (3 - 2 * t)
          const extent = h / (2 * laneAmplitude(h, dpr, thickness, spacing)) + radius + 0.5
          assert.ok(center - extent >= dpr - 1e-6, `bottom clipped: h=${h}, dpr=${dpr}, stroke=${thickness}, spacing=${spacing}`)
          assert.ok(center + extent <= h - dpr + 1e-6, `top clipped: h=${h}, dpr=${dpr}, stroke=${thickness}, spacing=${spacing}`)
        }
      }
    }
  }
  assert.equal(laneAmplitude(600, 2, 2), 1 / 0.92)
})
