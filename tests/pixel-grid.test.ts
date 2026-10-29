import assert from 'node:assert/strict'
import { test } from 'node:test'
import { alignDevicePixel } from '../src/pixel-grid.ts'

test('overview coordinates align to physical pixels with fractional layout and display scaling', () => {
  for (const ratio of [1, 1.25, 1.5, 2, 3]) {
    for (const origin of [0, 0.25, 77.5, 213.328125]) {
      for (const value of [0, 2.5, 6.625, 27.5, 319.349]) {
        const absolute = (origin + alignDevicePixel(value, origin, ratio)) * ratio
        assert.ok(Math.abs(absolute - Math.round(absolute)) < 1e-9)
      }
    }
  }
})
