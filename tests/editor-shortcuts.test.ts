import assert from 'node:assert/strict'
import { test } from 'node:test'
import { createEditorShortcuts } from '../src/editor-shortcuts.ts'

test('shortcuts preserve save/undo modifiers, marker layout and sampler-pad numbering', () => {
  const calls: unknown[] = []
  const editor = { selection: null as { start: number, end: number } | null, cursor: 100, looping: false,
    togglePlay: () => calls.push('play'), undo: () => calls.push('undo'), redo: () => calls.push('redo'),
    save: (mode: string) => calls.push(mode), remove: () => calls.push('remove'), toggleLoopPoints: () => calls.push('loop') }
  const regions = Array.from({ length: 18 }, (_, i) => ({ start: i * 10, end: (i + 1) * 10, skip: false }))
  const keydown = createEditorShortcuts({ editor, frames: () => 1000, regions: () => regions,
    playRegion: region => calls.push(region.start), addMarkers: frames => calls.push(frames),
    toggleSnap: () => calls.push('snap'), zoom: (...args) => calls.push(args), fit: () => calls.push('fit') })
  let prevented = 0
  const press = (key: string, extras = {}) => keydown({ key, code: '', ctrlKey: false, metaKey: false,
    shiftKey: false, altKey: false, preventDefault: () => { prevented++ }, ...extras } as KeyboardEvent)
  press('s', { metaKey: true, shiftKey: true })
  press('z', { ctrlKey: true })
  press('z', { metaKey: true, shiftKey: true })
  press('ь')
  press('!', { code: 'Digit1', shiftKey: true })
  press('a', { metaKey: true })
  assert.deepEqual(editor.selection, { start: 0, end: 1000 })
  press('m')
  press('m', { metaKey: true })
  press('Escape')
  assert.equal(editor.selection, null)
  assert.deepEqual(calls, ['saveAs', 'undo', 'redo', [100], 90, [0, 1000]])
  assert.equal(prevented, 5)
})
