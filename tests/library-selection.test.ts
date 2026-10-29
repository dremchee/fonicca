import assert from 'node:assert/strict'
import { test } from 'node:test'
import { createLibrarySelection } from '../src/library-selection.ts'

function setup() {
  const focusIndex = { value: -1 }, anchorIndex = { value: -1 }, selectedIds = new Set<number>()
  let scope = 'first'
  const options = { focusIndex, anchorIndex, selectedIds, total: () => 100000, identity: () => scope,
    sampleAt: (i: number) => i >= 0 ? { id: i + 10 } : undefined,
    ensureRange: async () => {}, rangeIds: async (a: number, b: number) => [a + 10, b + 10],
    allIds: async () => [10, 11, 12], remember: () => {} }
  const selection = createLibrarySelection(options)
  return { options, selection, focusIndex, anchorIndex, selectedIds, changeScope: () => { scope = 'second' } }
}
test('click, toggle, shift range and keyboard focus preserve selection semantics', async () => {
  const v = setup()
  await v.selection.clickRow(1)
  await v.selection.clickRow(3, { toggle: true })
  assert.deepEqual([...v.selectedIds], [11, 13])
  await v.selection.clickRow(5, { shift: true })
  assert.deepEqual([...v.selectedIds], [13, 15])
  await v.selection.moveFocus(1)
  assert.equal(v.focusIndex.value, 6)
  assert.deepEqual(v.selection.targetIds(), [16])
  await v.selection.selectAll()
  assert.deepEqual([...v.selectedIds], [10, 11, 12])
})
test('late range selection cannot overwrite a newer click', async () => {
  const v = setup()
  let finish!: (ids: number[]) => void
  v.options.rangeIds = () => new Promise(resolve => { finish = resolve })
  // A second engine shares the state but uses a deferred range provider.
  const selection = createLibrarySelection(v.options)
  await selection.clickRow(0)
  const old = selection.clickRow(20, { shift: true })
  await selection.clickRow(5)
  finish([10, 30])
  await old
  assert.deepEqual([...v.selectedIds], [15])
  assert.equal(v.focusIndex.value, 5)
})
test('late select-all cannot alter a new scope', async () => {
  const v = setup()
  let finish!: (ids: number[]) => void
  v.options.allIds = () => new Promise(resolve => { finish = resolve })
  const selection = createLibrarySelection(v.options)
  const pending = selection.selectAll()
  v.changeScope()
  v.selectedIds.add(999)
  finish([10, 11])
  await pending
  assert.deepEqual([...v.selectedIds], [999])
})

test('late keyboard fetch cannot alter a new scope or overwrite a newer click', async () => {
  for (const changeScope of [true, false]) {
    const v = setup()
    let finish!: () => void
    v.options.ensureRange = () => new Promise(resolve => { finish = resolve })
    const selection = createLibrarySelection(v.options)
    const pending = selection.moveFocus(1)
    if (changeScope) {
      v.changeScope()
      v.focusIndex.value = -1
      v.selectedIds.add(999)
    } else await selection.clickRow(5)
    finish()
    await pending
    assert.equal(v.focusIndex.value, changeScope ? -1 : 5)
    assert.deepEqual([...v.selectedIds], changeScope ? [999] : [15])
  }
})
