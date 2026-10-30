import assert from 'node:assert/strict'
import { setImmediate } from 'node:timers/promises'
import { test } from 'node:test'
import { effectScope, shallowRef } from 'vue'
import { useEditorGestures, type GestureEditor } from '../src/composables/editor/gestures.ts'

function mouse(type: string, x: number, overrides = {}) {
  return Object.assign(new Event(type), { clientX: x, clientY: 30, button: 0,
    shiftKey: false, altKey: false, detail: 1, ...overrides }) as unknown as MouseEvent
}
function setup(t: { after: (cleanup: () => void) => void }) {
  const old = Object.getOwnPropertyDescriptor(globalThis, 'window')
  const target = new EventTarget()
  Object.defineProperty(globalThis, 'window', { value: target, configurable: true })
  const calls = { play: 0, update: 0, markers: 0, loops: 0, stop: 0 }
  const editor: GestureEditor = {
    state: { loop: null, markers: [] }, regions: [], selection: null, cursor: 0, playing: false, looping: false,
    zeroSnap: async frames => frames,
    setMarkers: () => { calls.markers++ }, stopPlayback: () => { calls.stop++ },
    setLoopPoints: async range => { editor.state!.loop = { ...range }; calls.loops++; return true },
    play: () => { calls.play++ }, updatePlaybackRange: () => { calls.update++ },
  }
  const canvas = shallowRef({ getBoundingClientRect: () => ({ left: 0, top: 0, width: 100 }) } as HTMLCanvasElement)
  const identity = shallowRef('first-track')
  const autoplay = shallowRef(true)
  const zeroSnap = shallowRef(false)
  const scope = effectScope()
  const gestures = scope.run(() => useEditorGestures({ editor, canvas, identity, zeroSnap, autoplay,
    frames: shallowRef(1000), rate: shallowRef(1000), viewStart: shallowRef(0), viewEnd: shallowRef(1000),
    ruler: 18, x: frame => frame / 10, snapped: frame => frame, badgeAt: () => undefined }))!
  t.after(() => { scope.stop(); if (old) Object.defineProperty(globalThis, 'window', old); else Reflect.deleteProperty(globalThis, 'window') })
  return { gestures, editor, calls, target, identity, zeroSnap, autoplay, scope }
}

test('click auditions its position, dragging selects silently, playing range updates without restarting', async t => {
  const v = setup(t)
  v.gestures.onMouseDown(mouse('mousedown', 20))
  v.target.dispatchEvent(mouse('mouseup', 20))
  await setImmediate()
  assert.equal(v.editor.cursor, 200)
  assert.equal(v.calls.play, 1)
  v.gestures.onMouseDown(mouse('mousedown', 30))
  v.target.dispatchEvent(mouse('mousemove', 60))
  v.target.dispatchEvent(mouse('mouseup', 65))
  await setImmediate()
  assert.deepEqual(v.editor.selection, { start: 300, end: 650 })
  assert.equal(v.calls.play, 1)
  v.editor.playing = true
  v.gestures.onMouseDown(mouse('mousedown', 65))
  v.target.dispatchEvent(mouse('mousemove', 75))
  v.target.dispatchEvent(mouse('mouseup', 75))
  await setImmediate()
  assert.deepEqual(v.editor.selection, { start: 300, end: 750 })
  assert.equal(v.calls.update, 1)
  assert.equal(v.calls.play, 1)
})

test('late zero snapping cannot overwrite another session or trigger playback', async t => {
  const v = setup(t)
  let finish!: (frames: number[]) => void
  v.zeroSnap.value = true
  v.editor.zeroSnap = () => new Promise(resolve => { finish = resolve })
  v.gestures.onMouseDown(mouse('mousedown', 20))
  v.target.dispatchEvent(mouse('mouseup', 20))
  v.identity.value = 'second-track'
  v.editor.cursor = 100
  finish([199])
  await setImmediate()
  assert.equal(v.editor.cursor, 100)
  assert.equal(v.calls.play, 0)
})

test('closing the editor removes pending gesture listeners', async t => {
  const v = setup(t)
  v.gestures.onMouseDown(mouse('mousedown', 20))
  v.scope.stop()
  v.target.dispatchEvent(mouse('mousemove', 60))
  v.target.dispatchEvent(mouse('mouseup', 60))
  await setImmediate()
  assert.equal(v.editor.selection, null)
  assert.equal(v.calls.play, 0)
})

test('marker dragging stays between neighbours and a cancelled drag never commits', async t => {
  const v = setup(t)
  v.editor.state!.markers = [{ frame: 200, skip: true }, { frame: 400, skip: false }]
  v.editor.setMarkers = markers => { v.editor.state!.markers = markers; v.calls.markers++ }
  v.gestures.onMouseDown(mouse('mousedown', 20, { clientY: 5 }))
  v.target.dispatchEvent(mouse('mousemove', 70, { clientY: 5 }))
  v.target.dispatchEvent(mouse('mouseup', 70, { clientY: 5 }))
  await setImmediate()
  assert.deepEqual(v.editor.state!.markers, [{ frame: 399, skip: true }, { frame: 400, skip: false }])
  assert.equal(v.calls.markers, 1)
  v.gestures.onMouseDown(mouse('mousedown', 40, { clientY: 5 }))
  v.target.dispatchEvent(mouse('mousemove', 20, { clientY: 5 }))
  v.identity.value = 'new-track'
  v.target.dispatchEvent(mouse('mouseup', 20, { clientY: 5 }))
  await setImmediate()
  assert.equal(v.calls.markers, 1)
  assert.equal(v.gestures.draggedMarkers.value, null)
})

test('saved loop boundaries preview live and commit once with the opposite edge fixed', async t => {
  const v = setup(t)
  v.editor.state!.loop = { start: 200, end: 600 }
  v.gestures.updateHover(mouse('mousemove', 20))
  assert.equal(v.gestures.hoverEdge.value, 'start')
  v.gestures.onMouseDown(mouse('mousedown', 20))
  v.target.dispatchEvent(mouse('mousemove', 30))
  v.target.dispatchEvent(mouse('mousemove', 35))
  assert.deepEqual(v.gestures.draggedLoop.value, { start: 350, end: 600 })
  assert.deepEqual(v.editor.state!.loop, { start: 200, end: 600 })
  assert.equal(v.calls.loops, 0)
  v.target.dispatchEvent(mouse('mouseup', 40))
  await setImmediate()
  assert.deepEqual(v.editor.state!.loop, { start: 400, end: 600 })
  assert.deepEqual(v.editor.selection, { start: 400, end: 600 })
  assert.equal(v.calls.loops, 1)
  assert.equal(v.calls.play, 0)
  assert.equal(v.gestures.draggedLoop.value, null)
})

test('resizing a playing saved loop updates its audible range without stopping or restarting', async t => {
  const v = setup(t)
  v.editor.state!.loop = { start: 200, end: 600 }
  v.editor.selection = { start: 200, end: 600 }
  v.editor.playing = true
  v.gestures.onMouseDown(mouse('mousedown', 60, { clientY: 17 }))
  v.target.dispatchEvent(mouse('mousemove', 80))
  assert.ok(v.calls.update > 0)
  assert.deepEqual(v.editor.selection, { start: 200, end: 800 })
  v.target.dispatchEvent(mouse('mouseup', 85))
  await setImmediate()
  assert.deepEqual(v.editor.state!.loop, { start: 200, end: 850 })
  assert.equal(v.calls.loops, 1)
  assert.equal(v.calls.play, 0)
  assert.equal(v.calls.stop, 0)
})

test('loop resize zero-snaps only the moved edge and never crosses its anchor', async t => {
  const v = setup(t)
  v.editor.state!.loop = { start: 200, end: 600 }
  v.zeroSnap.value = true
  const snapped: number[][] = []
  v.editor.zeroSnap = async frames => { snapped.push(frames); return [0] }
  v.gestures.onMouseDown(mouse('mousedown', 60))
  v.target.dispatchEvent(mouse('mousemove', 5))
  v.target.dispatchEvent(mouse('mouseup', 5))
  await setImmediate()
  assert.deepEqual(snapped, [[201]])
  assert.deepEqual(v.editor.state!.loop, { start: 200, end: 201 })
})

test('late loop snapping cannot commit to another track', async t => {
  const v = setup(t)
  v.editor.state!.loop = { start: 200, end: 600 }
  v.zeroSnap.value = true
  let finish!: (frames: number[]) => void
  v.editor.zeroSnap = () => new Promise(resolve => { finish = resolve })
  v.gestures.onMouseDown(mouse('mousedown', 60))
  v.target.dispatchEvent(mouse('mousemove', 80))
  v.target.dispatchEvent(mouse('mouseup', 80))
  v.identity.value = 'new-track'
  v.editor.state!.loop = { start: 10, end: 100 }
  finish([799])
  await setImmediate()
  assert.deepEqual(v.editor.state!.loop, { start: 10, end: 100 })
  assert.equal(v.calls.loops, 0)
  assert.equal(v.gestures.draggedLoop.value, null)
})

test('a distinct selection retains its own resize edge instead of changing saved loop metadata', async t => {
  const v = setup(t)
  v.editor.state!.loop = { start: 200, end: 600 }
  v.editor.selection = { start: 400, end: 600 }
  v.gestures.onMouseDown(mouse('mousedown', 60))
  v.target.dispatchEvent(mouse('mousemove', 80))
  v.target.dispatchEvent(mouse('mouseup', 80))
  await setImmediate()
  assert.deepEqual(v.editor.selection, { start: 400, end: 800 })
  assert.deepEqual(v.editor.state!.loop, { start: 200, end: 600 })
  assert.equal(v.calls.loops, 0)
})

test('a rejected loop edit restores the previous selection and cursor', async t => {
  const v = setup(t)
  v.editor.state!.loop = { start: 200, end: 600 }
  v.editor.cursor = 50
  v.editor.setLoopPoints = async () => false
  v.gestures.onMouseDown(mouse('mousedown', 60))
  v.target.dispatchEvent(mouse('mousemove', 80))
  v.target.dispatchEvent(mouse('mouseup', 80))
  await setImmediate()
  assert.deepEqual(v.editor.state!.loop, { start: 200, end: 600 })
  assert.equal(v.editor.selection, null)
  assert.equal(v.editor.cursor, 50)
  assert.equal(v.gestures.draggedLoop.value, null)
})

test('inward corner flags can be grabbed away from the boundary without a jump', async t => {
  const v = setup(t)
  v.editor.state!.loop = { start: 200, end: 900 }
  v.gestures.updateHover(mouse('mousemove', 27, { clientY: 3 }))
  assert.equal(v.gestures.hoverEdge.value, 'start')
  v.gestures.onMouseDown(mouse('mousedown', 27, { clientY: 3 }))
  v.target.dispatchEvent(mouse('mousemove', 37, { clientY: 3 }))
  v.target.dispatchEvent(mouse('mouseup', 37, { clientY: 3 }))
  await setImmediate()
  assert.deepEqual(v.editor.state!.loop, { start: 300, end: 900 })
  assert.equal(v.calls.loops, 1)
})

test('Loop label plays the saved range from its start with repeat enabled', t => {
  const v = setup(t)
  v.editor.state!.loop = { start: 200, end: 900 }
  v.gestures.updateHover(mouse('mousemove', 50, { clientY: 25 }))
  assert.equal(v.gestures.hoverLoopLabel.value, true)
  v.gestures.onMouseDown(mouse('mousedown', 50, { clientY: 25 }))
  assert.deepEqual(v.editor.selection, { start: 200, end: 900 })
  assert.equal(v.editor.cursor, 200)
  assert.equal(v.editor.looping, true)
  assert.equal(v.calls.play, 1)
  assert.equal(v.calls.loops, 0)
})

test('Loop label does not start playback while processing or saving', t => {
  const v = setup(t)
  v.editor.state!.loop = { start: 200, end: 900 }
  v.editor.processing = true
  v.gestures.onMouseDown(mouse('mousedown', 50, { clientY: 25 }))
  v.editor.processing = false
  v.editor.saving = true
  v.gestures.onMouseDown(mouse('mousedown', 50, { clientY: 25 }))
  assert.equal(v.calls.play, 0)
  assert.equal(v.editor.looping, false)
})

test('a marker can be dragged by its line while playback and selection remain unchanged', async t => {
  const v = setup(t)
  v.editor.state!.markers = [{ frame: 200, skip: true }, { frame: 800, skip: false }]
  v.editor.selection = { start: 300, end: 500 }
  v.editor.cursor = 350
  v.editor.playing = true
  v.editor.setMarkers = markers => { v.editor.state!.markers = markers; v.calls.markers++ }
  v.gestures.updateHover(mouse('mousemove', 22, { clientY: 160 }))
  assert.equal(v.gestures.hoverMarker.value, true)
  v.gestures.onMouseDown(mouse('mousedown', 22, { clientY: 160 }))
  v.target.dispatchEvent(mouse('mousemove', 42, { clientY: 160 }))
  assert.deepEqual(v.gestures.draggedMarkers.value![0], { frame: 400, skip: true })
  assert.equal(v.calls.markers, 0)
  v.gestures.updateHover(mouse('mousemove', 42, { clientY: 160 }))
  assert.equal(v.gestures.hoverMarker.value, true)
  v.target.dispatchEvent(mouse('mouseup', 44, { clientY: 160 }))
  await setImmediate()
  assert.deepEqual(v.editor.state!.markers[0], { frame: 420, skip: true })
  assert.deepEqual(v.editor.selection, { start: 300, end: 500 })
  assert.equal(v.editor.cursor, 350)
  assert.equal(v.calls.markers, 1)
  assert.equal(v.calls.play, 0)
  assert.equal(v.calls.stop, 0)
})


test('ruler selects a marker region inside an existing loop for setting new loop bounds', async t => {
  const v = setup(t)
  v.editor.state!.loop = { start: 100, end: 900 }
  v.editor.regions = [{ start: 200, end: 500, skip: false }, { start: 500, end: 1000, skip: false }]
  v.gestures.onMouseDown(mouse('mousedown', 35, { clientY: 12 }))
  assert.deepEqual(v.editor.selection, { start: 200, end: 500 })
  await v.editor.setLoopPoints(v.editor.selection!)
  assert.deepEqual(v.editor.state!.loop, { start: 200, end: 500 })
})


test('autoplay can disable cursor audition without disabling explicit playback', async t => {
  const v = setup(t)
  v.autoplay.value = false
  v.gestures.onMouseDown(mouse('mousedown', 20))
  v.target.dispatchEvent(mouse('mouseup', 20))
  await setImmediate()
  assert.equal(v.editor.cursor, 200)
  assert.equal(v.calls.play, 0)
  v.gestures.playRegion({ start: 200, end: 400 })
  assert.equal(v.calls.play, 1)
})
