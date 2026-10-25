import assert from 'node:assert/strict'
import test from 'node:test'
import { visibleTagCount } from '../src/tag-overflow.ts'

test('all tags remain visible when their complete badges and gaps fit', () => {
  assert.equal(visibleTagCount([40, 50, 60], 158, 28), 3)
  assert.equal(visibleTagCount([], 0, 28), 0)
})

test('overflow reserves room for the counter and never exposes part of a badge', () => {
  assert.equal(visibleTagCount([40, 50, 60], 150, 28), 2)
  assert.equal(visibleTagCount([40, 50, 60], 100, 28), 1)
  assert.equal(visibleTagCount([40, 50, 60], 50, 28), 0)
})

test('an oversized first tag is hidden without reordering the others', () => {
  const widths = [400, 30, 40]
  assert.equal(visibleTagCount(widths, 170, 28), 0)
  assert.deepEqual(widths, [400, 30, 40])
  assert.equal(visibleTagCount(widths, 500, 28), 3)
})
