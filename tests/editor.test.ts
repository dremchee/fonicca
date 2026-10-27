import assert from 'node:assert/strict'
import { test } from 'node:test'
import { effectScope, nextTick, shallowRef } from 'vue'
import { useFrameDraw } from '../src/composables/editor/frameDraw.ts'
import { useEditorGrid } from '../src/composables/editor/grid.ts'
import { useEditorViewport } from '../src/composables/editor/viewport.ts'

function viewport() {
  const frames = shallowRef(10000)
  const selection = shallowRef<{ start: number, end: number } | null>(null)
  const scope = effectScope()
  const view = scope.run(() => useEditorViewport(frames, () => selection.value))!
  view.fit()
  return { ...view, frames, selection, scope }
}

test('zoom keeps the pointer anchor and panning stays inside audio', () => {
  const v = viewport()
  try {
    v.zoom(0.5, 2500)
    assert.deepEqual([v.viewStart.value, v.viewEnd.value], [1250, 6250])
    v.pan(-10000)
    assert.deepEqual([v.viewStart.value, v.viewEnd.value], [0, 5000])
    v.pan(10000)
    assert.deepEqual([v.viewStart.value, v.viewEnd.value], [5000, 10000])
    v.zoom(0.0001)
    assert.equal(v.viewEnd.value - v.viewStart.value, 64)
  }
  finally { v.scope.stop() }
})

test('crop clamps the view and selection zoom uses the selected range', async () => {
  const v = viewport()
  try {
    v.selection.value = { start: 3000, end: 6000 }
    v.zoomToSelection()
    assert.deepEqual([v.viewStart.value, v.viewEnd.value], [3000, 6000])
    v.frames.value = 2000
    await nextTick()
    assert.deepEqual([v.viewStart.value, v.viewEnd.value], [0, 2000])
    v.frames.value = 0
    await nextTick()
    assert.deepEqual([v.viewStart.value, v.viewEnd.value], [0, 1])
  }
  finally { v.scope.stop() }
})

test('playback follows offscreen frames while preserving zoom and file bounds', (t) => {
  let now = 0
  t.mock.method(Date, 'now', () => now)
  const v = viewport()
  try {
    v.zoom(0.2, 0)
    now = 3000
    v.follow(1800)
    assert.deepEqual([v.viewStart.value, v.viewEnd.value], [0, 2000])
    v.follow(2000)
    assert.deepEqual([v.viewStart.value, v.viewEnd.value], [1800, 3800])
    v.follow(3700)
    assert.deepEqual([v.viewStart.value, v.viewEnd.value], [1800, 3800])
    v.follow(9950)
    assert.deepEqual([v.viewStart.value, v.viewEnd.value], [8000, 10000])
    v.follow(50) // A loop or a backwards seek brings the start back into view.
    assert.deepEqual([v.viewStart.value, v.viewEnd.value], [0, 2000])
    v.follow(null)
    v.follow(Number.NaN)
    assert.deepEqual([v.viewStart.value, v.viewEnd.value], [0, 2000])
    v.fit()
    v.follow(6000)
    assert.deepEqual([v.viewStart.value, v.viewEnd.value], [0, 10000])
  }
  finally { v.scope.stop() }
})

test('manual navigation delays following for three seconds after the latest interaction', (t) => {
  let now = 0
  t.mock.method(Date, 'now', () => now)
  const v = viewport()
  try {
    v.zoom(0.2, 0)
    now = 2999
    v.follow(6000)
    assert.deepEqual([v.viewStart.value, v.viewEnd.value], [0, 2000])
    v.pan(1000)
    now = 5998
    v.follow(6000)
    assert.deepEqual([v.viewStart.value, v.viewEnd.value], [1000, 3000])
    now = 5999
    v.follow(6000)
    assert.deepEqual([v.viewStart.value, v.viewEnd.value], [5800, 7800])
    v.deferFollow() // Dragging the overview uses the same delay.
    now = 8998
    v.follow(1000)
    assert.deepEqual([v.viewStart.value, v.viewEnd.value], [5800, 7800])
    now = 8999
    v.follow(1000)
    assert.deepEqual([v.viewStart.value, v.viewEnd.value], [800, 2800])
  }
  finally { v.scope.stop() }
})

test('beat grid follows tempo, origin, time fallback and Alt inversion', () => {
  const options = {
    bpm: shallowRef<number | null>(120), rate: shallowRef(1000), frames: shallowRef(1900),
    width: shallowRef(1000), viewStart: shallowRef(0), viewEnd: shallowRef(2000),
    downbeat: shallowRef(100), gridMode: shallowRef<'beats' | 'time' | 'off'>('beats'),
    snap: shallowRef(true),
  }
  const grid = useEditorGrid(options)
  const mouse = (altKey: boolean) => ({ altKey }) as MouseEvent
  assert.equal(grid.beatFrames.value, 500)
  assert.deepEqual(grid.gridFrames(500), [100, 600, 1100, 1600])
  assert.equal(grid.snapped(580, mouse(false)), 600)
  assert.equal(grid.snapped(580, mouse(true)), 580)
  options.snap.value = false
  assert.equal(grid.snapped(580, mouse(false)), 580)
  assert.equal(grid.snapped(580, mouse(true)), 600)
  options.snap.value = true
  assert.equal(grid.snapped(1899, mouse(false)), 1900)
  options.bpm.value = null
  assert.equal(grid.beatGrid.value, false)
  assert.equal(grid.gridOrigin(), 0)
  options.gridMode.value = 'off'
  assert.equal(grid.snapped(580, mouse(false)), 580)
})

test('canvas invalidations draw once per frame, use latest state and cancel on disposal', (t) => {
  const callbacks = new Map<number, FrameRequestCallback>()
  let id = 0
  const originalRequest = globalThis.requestAnimationFrame
  const originalCancel = globalThis.cancelAnimationFrame
  t.after(() => {
    globalThis.requestAnimationFrame = originalRequest
    globalThis.cancelAnimationFrame = originalCancel
  })
  globalThis.requestAnimationFrame = (callback: FrameRequestCallback) => {
    callbacks.set(++id, callback)
    return id
  }
  globalThis.cancelAnimationFrame = (handle: number) => { callbacks.delete(handle) }
  const scope = effectScope()
  let state = 0
  const drawn: number[] = []
  const invalidate = scope.run(() => useFrameDraw(() => drawn.push(state)))!
  for (let i = 0; i < 100; i++) {
    state = i
    invalidate()
  }
  assert.equal(callbacks.size, 1)
  const [handle, callback] = [...callbacks][0]
  callbacks.delete(handle)
  callback(0)
  assert.deepEqual(drawn, [99])
  invalidate()
  assert.equal(callbacks.size, 1)
  scope.stop()
  assert.equal(callbacks.size, 0)
  invalidate()
  assert.equal(callbacks.size, 0)
})
