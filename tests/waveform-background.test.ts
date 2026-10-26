import assert from 'node:assert/strict'
import { test } from 'node:test'
import { waveformBackground } from '../src/waveform-background.ts'

test('row backgrounds preserve peaks from either channel when reducing columns', () => {
  const values = new Float32Array(2 * 1024 * 6)
  values[(1024 + 1023) * 6 + 1] = 1
  const path = waveformBackground({ samples: false, lanes: 2, count: 1024, stride: 6, values })
  assert.match(path, /1000\.0,4\.0/)
  assert.match(path, /1000\.0,96\.0/)
  assert.equal((path.match(/ L/g) ?? []).length, 511)
})

test('empty and invalid wave data does not produce invalid SVG coordinates', () => {
  assert.equal(waveformBackground({ samples: false, lanes: 1, count: 0, stride: 6, values: new Float32Array() }), '')
  const path = waveformBackground({ samples: true, lanes: 1, count: 3, stride: 1, values: new Float32Array([NaN, -1, Infinity]) })
  assert.doesNotMatch(path, /NaN|Infinity/)
  assert.match(path, /500\.0,4\.0/)
})
