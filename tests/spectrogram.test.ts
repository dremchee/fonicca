import assert from 'node:assert/strict'
import { setImmediate } from 'node:timers/promises'
import { test } from 'node:test'
import { effectScope, nextTick, shallowRef } from 'vue'
import { frequencyBounds, frequencyY, parseSpectrogram } from '../src/spectrogram.ts'
import { useEditorSpectrogram } from '../src/composables/editor/spectrogram.ts'
import type { Spectrogram } from '../src/spectrogram.ts'

function packed(start = 0, end = 100, columns = 100, value = 0) {
  const buffer = new ArrayBuffer(32 + columns * 256)
  const words = [1, start, end, columns, 256, 48000, 4096, 1024]
  const view = new DataView(buffer)
  words.forEach((v, i) => view.setUint32(i * 4, v, true))
  new Uint8Array(buffer, 32).fill(value)
  return buffer
}
const spectrum = (a = 0, b = 100, columns = 100, value = 0) => parseSpectrogram(packed(a, b, columns, value))

function options(read: (a: number, b: number, columns: number) => Promise<Spectrogram>) {
  return {
    enabled: shallowRef(true), revision: shallowRef(0), frames: shallowRef(1000),
    start: shallowRef(200), end: shallowRef(400), width: shallowRef(200), read,
  }
}

test('binary viewport has the native dimensions and preserves energy bytes', () => {
  const data = parseSpectrogram(packed(10, 200, 20, 171))
  assert.equal(data.start, 10)
  assert.equal(data.end, 200)
  assert.equal(data.columns, 20)
  assert.equal(data.bins, 256)
  assert.equal(data.values.length, 20 * 256)
  assert.ok(data.values.every(v => v === 171))
  assert.throws(() => parseSpectrogram(new ArrayBuffer(8)))
  const invalid = packed()
  new DataView(invalid).setUint32(0, 99, true)
  assert.throws(() => parseSpectrogram(invalid))
  assert.throws(() => parseSpectrogram(packed().slice(0, 33)))
})

test('frequency scale is logarithmic with low frequencies at the bottom', () => {
  assert.deepEqual(frequencyBounds(48000), { min: 20, max: 24000 })
  assert.equal(frequencyY(20, 48000), 1)
  assert.equal(frequencyY(24000, 48000), 0)
  assert.ok(Math.abs(frequencyY(Math.sqrt(20 * 24000), 48000) - 0.5) < 1e-12)
  assert.ok(frequencyY(100, 48000) > frequencyY(1000, 48000))
})

test('small pans reuse overscan; changing audio invalidates it', async () => {
  let reads = 0
  const o = options(async (a, b, columns) => { reads++; return spectrum(a, b, columns) })
  const scope = effectScope()
  const state = scope.run(() => useEditorSpectrogram(o))!
  try {
    await setImmediate()
    assert.equal(reads, 1)
    assert.equal(state.data.value?.start, 150)
    assert.equal(state.data.value?.end, 450)
    o.start.value = 220
    o.end.value = 420
    await setImmediate()
    assert.equal(reads, 1)
    o.revision.value++
    await setImmediate()
    assert.equal(reads, 2)
    o.enabled.value = false
    o.start.value = 700
    o.end.value = 900
    await setImmediate()
    assert.equal(reads, 2)
  }
  finally { scope.stop() }
})

test('only one read runs and an obsolete audio revision cannot publish a result', async () => {
  const pending: { resolve: (data: Spectrogram) => void, a: number, b: number, columns: number }[] = []
  const o = options((a, b, columns) => new Promise(resolve => pending.push({ resolve, a, b, columns })))
  const scope = effectScope()
  const state = scope.run(() => useEditorSpectrogram(o))!
  try {
    assert.equal(pending.length, 1)
    o.revision.value++
    await nextTick()
    assert.equal(pending.length, 1)
    const first = pending[0]
    first.resolve(spectrum(first.a, first.b, first.columns, 1))
    await setImmediate()
    assert.equal(state.data.value, null)
    assert.equal(pending.length, 2)
    const second = pending[1]
    second.resolve(spectrum(second.a, second.b, second.columns, 2))
    await setImmediate()
    assert.equal(state.data.value?.values[0], 2)
    assert.equal(state.loading.value, false)
  }
  finally { scope.stop() }
})

test('read errors are retryable and a disposed editor drops in-flight data', async () => {
  let fail = true
  const o = options(async (a, b, columns) => {
    if (fail) throw new Error('analysis unavailable')
    return spectrum(a, b, columns)
  })
  const scope = effectScope()
  const state = scope.run(() => useEditorSpectrogram(o))!
  await setImmediate()
  assert.match(state.error.value!, /analysis unavailable/)
  assert.equal(state.loading.value, false)
  fail = false
  await state.refresh()
  assert.equal(state.error.value, null)
  assert.ok(state.data.value)
  scope.stop()

  let resolve!: (data: Spectrogram) => void
  const otherScope = effectScope()
  const other = otherScope.run(() => useEditorSpectrogram(options(() => new Promise(r => { resolve = r }))))!
  otherScope.stop()
  resolve(spectrum())
  await setImmediate()
  assert.equal(other.data.value, null)
})
