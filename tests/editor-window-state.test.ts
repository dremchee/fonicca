import assert from 'node:assert/strict'
import { test } from 'node:test'
import { readEditorWindowView } from '../src/editor-window-state.ts'

const query = (view: unknown) => `?editor=1&detached=1&view=${encodeURIComponent(JSON.stringify(view))}`
test('a detached window restores selection, loop, curve and viewport independently of audio', () => {
  const view = { cursor: 210, selection: { start: 100, end: 400 }, looping: true, curve: 'smooth',
    viewport: { start: 50, end: 500, zoom: 2, bpm: 96, downbeat: -100 } }
  assert.deepEqual(readEditorWindowView(query(view), 1000), view)
})
test('window view input is bounded, malformed URLs do not break opening a session', () => {
  assert.equal(readEditorWindowView('?view={}', 1000), null)
  assert.equal(readEditorWindowView('?detached=1&view=broken', 1000), null)
  assert.equal(readEditorWindowView(query({}), 1000), null)
  assert.deepEqual(readEditorWindowView(query({ cursor: 4000, selection: { start: -4, end: 2000 }, looping: 'true', curve: 'bad',
    viewport: { start: -20, end: 5000, zoom: 100, bpm: 0, downbeat: 2.6 } }), 1000), {
    cursor: 1000, selection: { start: 0, end: 1000 }, looping: false, curve: 'linear',
    viewport: { start: 0, end: 1000, zoom: 64, bpm: null, downbeat: 3 },
  })
  assert.equal(readEditorWindowView(query({ cursor: 0, selection: { start: 20, end: 10 } }), 100).selection, null)
})
