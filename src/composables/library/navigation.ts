import type { Sample, SampleFilter, SampleKind } from '../../api.ts'
import type { Scope } from '../../library-scope.ts'
import type { SampleSort } from '../../sample-columns.ts'
import { normalizeSampleSort } from '../../sample-columns.ts'
import { useStorage } from '@vueuse/core'
import { computed, ref } from 'vue'

/** Navigation preferences and the derived query; fetching rows is owned by pagination. */
export function useLibraryNavigation(expandedStems: Set<number>) {
  const scope = ref<Scope>({ kind: 'all' })
  const query = ref('')
  const sortPreference = useStorage<SampleSort>('fonicca-sample-sort-v1', { by: 'name', direction: 'asc' })
  const sort = computed(() => normalizeSampleSort(sortPreference.value))
  const kindFilter = ref<SampleKind | null>(null)
  const bpmMin = ref<number | null>(null)
  const bpmMax = ref<number | null>(null)
  const keyFilter = ref<string | null>(null)
  const categoryFilter = ref<string | null>(null)
  const filter = computed<SampleFilter>(() => {
    const s = scope.value
    return {
      sortBy: sort.value.by,
      sortDirection: sort.value.direction,
      groupStems: s.kind !== 'similar',
      expandedStems: [...expandedStems],
      query: query.value,
      sourceId: s.kind === 'source' ? s.id : null,
      directory: s.kind === 'source' ? s.path ?? null : null,
      tagId: s.kind === 'tag' ? s.id : null,
      favorites: s.kind === 'favorites',
      collectionId: s.kind === 'collection' ? s.id : null,
      similarTo: s.kind === 'similar' ? s.id : null,
      kind: kindFilter.value,
      bpmMin: bpmMin.value,
      bpmMax: bpmMax.value,
      key: keyFilter.value,
      category: categoryFilter.value,
    }
  })
  /** Where to return when leaving a "find similar" view. */
  let scopeBeforeSimilar: Scope = { kind: 'all' }

  function findSimilar(sample: Sample) {
    if (scope.value.kind !== 'similar')
      scopeBeforeSimilar = scope.value
    scope.value = { kind: 'similar', id: sample.id, name: sample.name }
  }

  function exitSimilar() {
    if (scope.value.kind === 'similar')
      scope.value = scopeBeforeSimilar
  }

  function setSort(by: SampleSort['by']) {
    if (scope.value.kind === 'similar') return
    sortPreference.value = { by, direction: sort.value.by === by && sort.value.direction === 'asc' ? 'desc' : 'asc' }
  }

  const hasAnalysisFilters = computed(() =>
    kindFilter.value !== null || bpmMin.value !== null || bpmMax.value !== null || keyFilter.value !== null
    || categoryFilter.value !== null)

  function clearAnalysisFilters() {
    kindFilter.value = null
    bpmMin.value = null
    bpmMax.value = null
    keyFilter.value = null
    categoryFilter.value = null
  }

  return { scope, query, sort, kindFilter, bpmMin, bpmMax, keyFilter, categoryFilter,
    filter, hasAnalysisFilters, clearAnalysisFilters, setSort, findSimilar, exitSimilar }
}
