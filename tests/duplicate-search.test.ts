import assert from 'node:assert/strict'
import test from 'node:test'
import { matchingDuplicateGroups } from '../src/duplicate-search.ts'
import type { DuplicateGroup } from '../src/api.ts'

const groups = [
  { key: 'a', copies: [{ id: 1, path: '/Packs/Café/Kick.wav' }, { id: 2, path: '/Archive/Copy.wav' }] },
  { key: 'b', copies: [{ id: 3, path: '/Packs/Drums/Snare.wav' }, { id: 4, path: '/Backup/Snare.wav' }] },
] as DuplicateGroup[]

test('duplicate search keeps a complete matching group and ignores case and accents', () => {
  assert.deepEqual(matchingDuplicateGroups(groups, 'CAFE kick'), [groups[0]])
  assert.equal(matchingDuplicateGroups(groups, 'archive')[0].copies.length, 2)
  assert.deepEqual(matchingDuplicateGroups(groups, 'snare backup'), [groups[1]])
})

test('empty queries preserve all groups and every term must match one copy path', () => {
  assert.equal(matchingDuplicateGroups(groups, '  '), groups)
  assert.deepEqual(matchingDuplicateGroups(groups, 'kick archive'), [])
  assert.deepEqual(matchingDuplicateGroups(groups, 'missing'), [])
})
