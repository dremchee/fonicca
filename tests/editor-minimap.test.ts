import assert from 'node:assert/strict'
import { test } from 'node:test'
import { dragMinimapRange } from '../src/editor-minimap.ts'

test('overview handles keep the opposite edge fixed and cannot cross it', () => {
  assert.deepEqual(dragMinimapRange(100, 500, 'start', 75, 1000), [175, 500])
  assert.deepEqual(dragMinimapRange(100, 500, 'end', -75, 1000), [100, 425])
  assert.deepEqual(dragMinimapRange(100, 500, 'start', 1000, 1000), [436, 500])
  assert.deepEqual(dragMinimapRange(100, 500, 'end', -1000, 1000), [100, 164])
  assert.deepEqual(dragMinimapRange(100, 500, 'start', -1000, 1000), [0, 500])
  assert.deepEqual(dragMinimapRange(100, 500, 'end', 1000, 1000), [100, 1000])
})

test('panning preserves zoom and stays inside the file, including short clips', () => {
  assert.deepEqual(dragMinimapRange(100, 500, 'move', 150, 1000), [250, 650])
  assert.deepEqual(dragMinimapRange(100, 500, 'move', 1000, 1000), [600, 1000])
  assert.deepEqual(dragMinimapRange(100, 500, 'move', -1000, 1000), [0, 400])
  for (const mode of ['move', 'start', 'end'] as const) {
    assert.deepEqual(dragMinimapRange(0, 8, mode, 1000, 8), [0, 8])
    assert.deepEqual(dragMinimapRange(0, 0, mode, 10, 0), [0, 0])
  }
})
