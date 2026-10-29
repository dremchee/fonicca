import assert from 'node:assert/strict'
import { test } from 'node:test'
import { effectScope, reactive } from 'vue'
import { useLibraryNavigation } from '../src/composables/library/navigation.ts'

test('navigation derives source, tag, favorites and collection filters without retaining the old scope', () => {
  const owner = effectScope()
  try {
    const expanded = reactive(new Set<number>([7]))
    const view = owner.run(() => useLibraryNavigation(expanded))!
    view.scope.value = { kind: 'source', id: 2, path: '/fixture/Pack' }
    assert.equal(view.filter.value.sourceId, 2)
    assert.equal(view.filter.value.directory, '/fixture/Pack')
    view.scope.value = { kind: 'tag', id: 3 }
    assert.equal(view.filter.value.tagId, 3)
    assert.equal(view.filter.value.sourceId, null)
    assert.equal(view.filter.value.directory, null)
    view.scope.value = { kind: 'favorites' }
    assert.equal(view.filter.value.favorites, true)
    assert.equal(view.filter.value.tagId, null)
    view.scope.value = { kind: 'collection', id: 4 }
    assert.equal(view.filter.value.collectionId, 4)
    assert.equal(view.filter.value.favorites, false)
    expanded.add(8)
    assert.deepEqual(view.filter.value.expandedStems, [7, 8])
    view.query.value = 'snare'
    view.bpmMin.value = 100
    view.categoryFilter.value = 'snare'
    assert.equal(view.hasAnalysisFilters.value, true)
    view.clearAnalysisFilters()
    assert.equal(view.hasAnalysisFilters.value, false)
    assert.equal(view.filter.value.query, 'snare')
    assert.equal(view.filter.value.bpmMin, null)
    assert.equal(view.filter.value.category, null)
  } finally { owner.stop() }
})

test('similar navigation restores its original scope and suspends user sorting', () => {
  const owner = effectScope()
  try {
    const view = owner.run(() => useLibraryNavigation(new Set<number>()))!
    view.scope.value = { kind: 'source', id: 2 }
    view.setSort('length')
    const sort = view.sort.value
    view.findSimilar({ id: 10, name: 'Kick' } as Parameters<typeof view.findSimilar>[0])
    assert.equal(view.filter.value.groupStems, false)
    assert.equal(view.filter.value.similarTo, 10)
    view.setSort('name')
    assert.deepEqual(view.sort.value, sort)
    view.findSimilar({ id: 11, name: 'Snare' } as Parameters<typeof view.findSimilar>[0])
    view.exitSimilar()
    assert.deepEqual(view.scope.value, { kind: 'source', id: 2 })
    assert.equal(view.filter.value.groupStems, true)
    assert.equal(view.filter.value.similarTo, null)
    view.setSort('length')
    assert.equal(view.sort.value.direction, 'desc')
  } finally { owner.stop() }
})
