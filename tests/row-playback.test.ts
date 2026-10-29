import assert from 'node:assert/strict'
import { test } from 'node:test'
import { rowClickSeeks } from '../src/row-playback.ts'

test('row playback modes distinguish new selections, repeated clicks and Alt-clicks', () => {
  for (const selected of [false, true]) {
    for (const alt of [false, true]) {
      assert.equal(rowClickSeeks('cursor', selected, alt), true)
      assert.equal(rowClickSeeks('start', selected, alt), false)
      assert.equal(rowClickSeeks('selected', selected, alt), selected)
      assert.equal(rowClickSeeks('alt', selected, alt), alt)
    }
  }
})
