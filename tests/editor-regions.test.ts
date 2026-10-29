import assert from 'node:assert/strict'
import { test } from 'node:test'
import { regionsOf } from '../src/editor-regions.ts'

test('the first explicit marker starts region one, without an implicit region at zero', () => {
  assert.deepEqual(regionsOf([{ frame: 400, skip: false }], 1000), [
    { start: 400, end: 1000, skip: false },
  ])
  assert.deepEqual(regionsOf([{ frame: 400, skip: false }, { frame: 650, skip: true }], 1000), [
    { start: 400, end: 650, skip: false },
    { start: 650, end: 1000, skip: true },
  ])
})

test('the beginning participates only when the user explicitly places a marker there', () => {
  assert.deepEqual(regionsOf([], 1000), [])
  assert.deepEqual(regionsOf([{ frame: 0, skip: true }, { frame: 400, skip: false }], 1000), [
    { start: 0, end: 400, skip: true },
    { start: 400, end: 1000, skip: false },
  ])
})
