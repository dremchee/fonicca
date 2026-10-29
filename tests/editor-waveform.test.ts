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
