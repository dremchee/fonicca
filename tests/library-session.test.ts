import assert from 'node:assert/strict'
import { test } from 'node:test'
import { findSessionSampleIndex, isDirectoryWithin, parseLibrarySession, resolveSessionScope, sessionScope } from '../src/library-session.ts'

const sources = [{ id: 1, path: '/Music/Beat', sampleCount: 900 }]

test('session parsing tolerates missing, malformed, and obsolete bookmarks', () => {
  for (const value of [null, {}, { scope: { kind: 'unknown' } }, { scope: { kind: 'source', id: '1' } }])
    assert.equal(parseLibrarySession(value), null)
  assert.deepEqual(parseLibrarySession({ scope: { kind: 'all' }, sample: { id: 1, index: -1 } }), { scope: { kind: 'all' }, sample: null })
  assert.deepEqual(parseLibrarySession({ scope: { kind: 'source', id: 1, path: '/Music/Beat/Drums/Kicks' }, sample: { id: 17, index: 450 } }), {
    scope: { kind: 'source', id: 1, path: '/Music/Beat/Drums/Kicks' }, sample: { id: 17, index: 450 },
  })
  assert.equal(sessionScope({ kind: 'similar', id: 1, name: 'Kick' }), null)
  assert.equal(sessionScope({ kind: 'duplicates' }), null)
})

test('missing directories fall back to a source root, and removed sources to the library', () => {
  const nested = { kind: 'source' as const, id: 1, path: '/Music/Beat/Drums/Kicks' }
  assert.deepEqual(resolveSessionScope(nested, sources, [], [], [{ path: nested.path, sampleCount: 12 }]), nested)
  assert.deepEqual(resolveSessionScope(nested, sources, [], [], []), { kind: 'source', id: 1 })
  assert.deepEqual(resolveSessionScope(nested, [], [], []), { kind: 'all' })
  assert.deepEqual(resolveSessionScope({ kind: 'collection', id: 3 }, sources, [], []), { kind: 'all' })
  assert.deepEqual(resolveSessionScope({ kind: 'source', id: 1, path: '/Music/Beats/Other' }, sources, [], []), { kind: 'source', id: 1 })
  assert.equal(isDirectoryWithin('C:\\Samples\\Drums\\Kicks', 'C:\\Samples'), true)
  assert.equal(isDirectoryWithin('/Music/Beats', '/Music/Beat'), false)
})

test('restoring a track beyond the first page uses the saved row without loading every ID', async () => {
  const visited: number[] = []
  const index = await findSessionSampleIndex({ id: 27, index: 450 }, 1000, async i => { visited.push(i); return 27 }, async () => { throw new Error('Unexpected full scan') })
  assert.equal(index, 450)
  assert.deepEqual(visited, [450])
})

test('a changed sort position is resolved by ID and deleted tracks stay unselected', async () => {
  assert.equal(await findSessionSampleIndex({ id: 27, index: 2 }, 4, async () => 9, async () => [1, 9, 13, 27]), 3)
  assert.equal(await findSessionSampleIndex({ id: 27, index: 450 }, 2, async () => { throw new Error('Out of range') }, async () => [1, 9]), -1)
  assert.equal(await findSessionSampleIndex({ id: 27, index: 450 }, 0, async () => 27, async () => [27]), -1)
})
