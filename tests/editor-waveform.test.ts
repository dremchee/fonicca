import assert from 'node:assert/strict'
import { setImmediate } from 'node:timers/promises'
import { test } from 'node:test'
import { effectScope, nextTick, shallowRef } from 'vue'
import { useEditorWaveform } from '../src/composables/editor/waveform.ts'
import type { Wave } from '../src/stores/editor.ts'

const wave = (count: number, value = 0): Wave => ({ samples: false, lanes: 1, count, stride: 6, values: new Float32Array(count * 6).fill(value) })
const settle = async () => { await nextTick(); await setImmediate() }
function setup(read: (a: number, b: number, columns: number) => Promise<Wave>) {
  const options = { enabled: shallowRef(true), identity: shallowRef('track-1'), revision: shallowRef(0),
    frames: shallowRef(10000), start: shallowRef(2000), end: shallowRef(4000), width: shallowRef(200),
    pixelRatio: shallowRef(1), split: shallowRef(true), bands: shallowRef(false), read }
  const scope = effectScope()
  const result = scope.run(() => useEditorWaveform(options))!
  return { ...result, options, scope }
}

test('waveform overscan reuses short pans and reloads for zoom, DPR and audio changes', async () => {
  const calls: number[][] = []
  const view = setup(async (a, b, columns) => { calls.push([a, b, columns]); return wave(columns) })
  try {
    await settle()
    assert.deepEqual(calls, [[1500, 4500, 300]])
    view.options.start.value += 100; view.options.end.value += 100
    await settle()
    assert.equal(calls.length, 1)
    view.options.end.value = 3100
    await settle()
    assert.equal(calls.length, 2)
    view.options.pixelRatio.value = 2
    await settle()
    assert.equal(calls.length, 3)
    view.options.revision.value++
    await settle()
    assert.equal(calls.length, 4)
  }
  finally { view.scope.stop() }
})

test('only one read runs and a replaced session cannot publish its old wave', async () => {
  let finish!: (value: Wave) => void
  let reads = 0
  const view = setup(async (_a, _b, count) => {
    reads++
    if (reads === 1) return new Promise(resolve => { finish = resolve })
    return wave(count, 2)
  })
  try {
    view.options.identity.value = 'track-2'
    view.options.revision.value++
    await settle()
    assert.equal(reads, 1)
    assert.equal(view.data.value, null)
    finish(wave(300, 1))
    await settle()
    assert.equal(reads, 2)
    assert.equal(view.data.value!.values[0], 2)
  }
  finally { view.scope.stop() }
})

test('failed reads can retry and disposal discards late data', async () => {
  let fail = true
  let finish!: (value: Wave) => void
  const view = setup(async () => {
    if (fail) throw new Error('temporary read error')
    return new Promise(resolve => { finish = resolve })
  })
  await settle()
  assert.match(view.error.value!, /temporary read error/)
  fail = false
  const pending = view.refresh()
  view.scope.stop()
  finish(wave(300))
  await pending
  assert.equal(view.data.value, null)
})


test('capped waveform density is reused for pans but refined after zoom', async () => {
  const calls: number[][] = []
  const view = setup(async (a, b, columns) => { calls.push([a, b, columns]); return wave(columns) })
  try {
    await settle()
    view.options.frames.value = 1_000_000
    view.options.start.value = 200_000
    view.options.end.value = 400_000
    view.options.width.value = 7680
    view.options.pixelRatio.value = 2
    await settle()
    assert.equal(calls.at(-1)![2], 16384)
    const initial = calls.length
    for (let i = 0; i < 20; i++) {
      view.options.start.value += 100
      view.options.end.value += 100
      await settle()
    }
    await view.refresh()
    assert.equal(calls.length, initial, 'small pans must reuse the achievable capped density')
    view.options.end.value = view.options.start.value + 100_000
    await settle()
    assert.equal(calls.length, initial + 1, 'zoom must refine a coarse capped cache')
    assert.equal(calls.at(-1)![2], 16384)
    assert.equal(calls.at(-1)![1] - calls.at(-1)![0], 150_000)
  }
  finally { view.scope.stop() }
})

test('rapid pan and zoom coalesce behind one read and closing prevents follow-up work', async () => {
  const pending: { start: number, end: number, columns: number, resolve: (value: Wave) => void }[] = []
  let active = 0, peak = 0
  const view = setup((start, end, columns) => {
    peak = Math.max(peak, ++active)
    return new Promise<Wave>(resolve => pending.push({ start, end, columns, resolve }))
      .finally(() => { active-- })
  })
  try {
    for (let i = 0; i < 50; i++) {
      view.options.start.value = 5000 + i * 10
      view.options.end.value = view.options.start.value + 1000
      await nextTick()
    }
    assert.equal(pending.length, 1)
    pending[0]!.resolve(wave(pending[0]!.columns))
    await settle()
    assert.equal(pending.length, 2)
    assert.deepEqual([pending[1]!.start, pending[1]!.end], [5240, 6740])
    assert.equal(peak, 1)
    view.options.identity.value = 'closed'
    view.options.enabled.value = false
    await nextTick()
    pending[1]!.resolve(wave(pending[1]!.columns, 7))
    await settle()
    assert.equal(view.data.value, null)
    assert.equal(pending.length, 2)
    assert.equal(active, 0)
  }
  finally { view.scope.stop() }
})

test('capped full-track coverage and individual samples do not trigger redundant requests', async () => {
  const calls: number[][] = []
  const view = setup(async (a, b, columns) => {
    calls.push([a, b, columns])
    return b - a <= columns
      ? { samples: true, lanes: 1, count: b - a, stride: 1, values: new Float32Array(b - a) }
      : wave(columns)
  })
  try {
    await settle()
    view.options.frames.value = 100_000
    view.options.start.value = 0
    view.options.end.value = 100_000
    view.options.width.value = 7680
    view.options.pixelRatio.value = 3
    await settle()
    const capped = calls.length
    await view.refresh()
    assert.equal(calls.length, capped)
    view.options.start.value = 50_000
    view.options.end.value = 50_010
    await settle()
    assert.equal(view.data.value!.samples, true)
    const sampled = calls.length
    view.options.start.value++
    view.options.end.value++
    await settle()
    await view.refresh()
    assert.equal(calls.length, sampled)
    view.options.end.value = 80_000
    await settle()
    assert.equal(view.data.value!.samples, false)
    assert.equal(calls.length, sampled + 1)
  }
  finally { view.scope.stop() }
})
