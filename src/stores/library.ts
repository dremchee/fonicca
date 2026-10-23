import type { AnalysisProgress, Collection, Sample, SampleFilter, SampleKind, ScanProgress, Source, Tag } from '@/api'
import { listen } from '@tauri-apps/api/event'
import { open, save } from '@tauri-apps/plugin-dialog'
import { acceptHMRUpdate, defineStore } from 'pinia'
import { computed, reactive, ref, shallowReactive, watch } from 'vue'
import { ANALYSIS_PROGRESS_EVENT, api, SCAN_PROGRESS_EVENT, startFileDrag } from '@/api'

const PAGE_SIZE = 200

/** What the sidebar has selected; combined with the search query into a filter. */
export type Scope
  = | { kind: 'all' }
    | { kind: 'favorites' }
    | { kind: 'source', id: number }
    | { kind: 'tag', id: number }
    | { kind: 'collection', id: number }

export const useLibraryStore = defineStore('library', () => {
  const sources = ref<Source[]>([])
  const tags = ref<Tag[]>([])
  const collections = ref<Collection[]>([])
  /** Short status message shown at the bottom of the sidebar. */
  const notice = ref<{ text: string, error?: boolean } | null>(null)
  let noticeTimer: ReturnType<typeof setTimeout> | undefined
  const scope = ref<Scope>({ kind: 'all' })
  const query = ref('')
  const kindFilter = ref<SampleKind | null>(null)
  const bpmMin = ref<number | null>(null)
  const bpmMax = ref<number | null>(null)
  const keyFilter = ref<string | null>(null)
  const analysis = ref<{ pending: number, paused: boolean }>({ pending: 0, paused: false })
  const total = ref(0)
  const scans = reactive(new Map<number, ScanProgress>())

  // Selection: `focusIndex` is the row under keyboard focus (previewed),
  // `selectedIds` the set acted on, `anchorIndex` the start of shift-ranges.
  const focusIndex = ref(-1)
  const anchorIndex = ref(-1)
  const selectedIds = reactive(new Set<number>())
  /** Bumped to ask the inspector to focus its "add tag" field. */
  const tagInputRequest = ref(0)

  // Rows are fetched lazily in pages as the list scrolls. `generation` invalidates
  // in-flight requests when the filter changes.
  const pages = shallowReactive(new Map<number, Sample[]>())
  const pending = new Map<number, Promise<void>>()
  let generation = 0

  const filter = computed<SampleFilter>(() => {
    const s = scope.value
    return {
      query: query.value,
      sourceId: s.kind === 'source' ? s.id : null,
      tagId: s.kind === 'tag' ? s.id : null,
      favorites: s.kind === 'favorites',
      collectionId: s.kind === 'collection' ? s.id : null,
      kind: kindFilter.value,
      bpmMin: bpmMin.value,
      bpmMax: bpmMax.value,
      key: keyFilter.value,
    }
  })
  const focused = computed(() => sampleAt(focusIndex.value))
  const isScanning = computed(() => scans.size > 0)

  function sampleAt(index: number): Sample | undefined {
    if (index < 0)
      return undefined
    return pages.get(Math.floor(index / PAGE_SIZE))?.[index % PAGE_SIZE]
  }

  function loadPage(page: number): Promise<void> {
    if (pages.has(page))
      return Promise.resolve()
    const inflight = pending.get(page)
    if (inflight)
      return inflight
    const gen = generation
    const load = api.listSamples(filter.value, page * PAGE_SIZE, PAGE_SIZE)
      .then((rows) => {
        if (gen === generation)
          pages.set(page, rows)
      })
      .finally(() => {
        if (gen === generation)
          pending.delete(page)
      })
    pending.set(page, load)
    return load
  }

  /** Ensures every page covering [start, end] is loaded. */
  async function ensureRange(start: number, end: number) {
    const loads = []
    for (let p = Math.floor(start / PAGE_SIZE); p <= Math.floor(end / PAGE_SIZE); p++)
      loads.push(loadPage(p))
    await Promise.all(loads)
  }

  /** Reloads rows and count. Selection is kept unless the filter itself changed. */
  async function refresh({ keepSelection = false } = {}) {
    generation++
    pages.clear()
    pending.clear()
    if (!keepSelection) {
      focusIndex.value = -1
      anchorIndex.value = -1
      selectedIds.clear()
    }
    total.value = await api.countSamples(filter.value)
    if (focusIndex.value >= total.value)
      focusIndex.value = total.value - 1
  }

  watch(filter, () => refresh())

  const hasAnalysisFilters = computed(() =>
    kindFilter.value !== null || bpmMin.value !== null || bpmMax.value !== null || keyFilter.value !== null)

  function clearAnalysisFilters() {
    kindFilter.value = null
    bpmMin.value = null
    bpmMax.value = null
    keyFilter.value = null
  }

  /** Re-fetches loaded pages in place (no flashing) after background updates. */
  let reloadTimer: ReturnType<typeof setTimeout> | undefined
  function scheduleReload() {
    if (reloadTimer)
      return
    reloadTimer = setTimeout(async () => {
      reloadTimer = undefined
      const gen = generation
      // Filters on analysis results can change which rows match.
      if (hasAnalysisFilters.value) {
        const n = await api.countSamples(filter.value)
        if (gen === generation)
          total.value = n
      }
      await Promise.all([...pages.keys()].map(async (page) => {
        const rows = await api.listSamples(filter.value, page * PAGE_SIZE, PAGE_SIZE)
        if (gen === generation)
          pages.set(page, rows)
      }))
    }, 1500)
  }

  /** Moves not-yet-analyzed samples to the front of the analysis queue. */
  function prioritizeAnalysis(samples: (Sample | undefined)[]) {
    const ids = samples.filter((s): s is Sample => !!s && !s.analyzed).map(s => s.id)
    if (ids.length && !analysis.value.paused)
      api.prioritizeAnalysis(ids)
  }

  async function setAnalysisPaused(paused: boolean) {
    analysis.value = { ...analysis.value, paused }
    await api.setAnalysisPaused(paused)
  }

  // --- Selection -----------------------------------------------------------

  async function idsInRange(a: number, b: number): Promise<number[]> {
    const [from, to] = a < b ? [a, b] : [b, a]
    await ensureRange(from, to)
    const ids = []
    for (let i = from; i <= to; i++) {
      const s = sampleAt(i)
      if (s)
        ids.push(s.id)
    }
    return ids
  }

  function setSelection(ids: Iterable<number>) {
    selectedIds.clear()
    for (const id of ids)
      selectedIds.add(id)
  }

  async function clickRow(index: number, { shift = false, toggle = false } = {}) {
    const sample = sampleAt(index)
    if (!sample)
      return
    if (shift && anchorIndex.value >= 0) {
      setSelection(await idsInRange(anchorIndex.value, index))
    }
    else if (toggle) {
      if (selectedIds.has(sample.id))
        selectedIds.delete(sample.id)
      else
        selectedIds.add(sample.id)
      anchorIndex.value = index
    }
    else {
      setSelection([sample.id])
      anchorIndex.value = index
    }
    focusIndex.value = index
  }

  /** Moves keyboard focus; with `extend` grows the selection from the anchor. */
  async function moveFocus(delta: number, extend = false) {
    if (total.value === 0)
      return
    const index = focusIndex.value < 0 ? 0 : Math.max(0, Math.min(total.value - 1, focusIndex.value + delta))
    focusIndex.value = index
    await ensureRange(index, index)
    if (extend && anchorIndex.value >= 0) {
      setSelection(await idsInRange(anchorIndex.value, index))
    }
    else {
      const s = sampleAt(index)
      setSelection(s ? [s.id] : [])
      anchorIndex.value = index
    }
  }

  async function selectAll() {
    setSelection(await api.sampleIds(filter.value))
  }

  /** Samples an action applies to: the selection, or the focused row if nothing is selected. */
  function targetIds(): number[] {
    if (selectedIds.size)
      return [...selectedIds]
    return focused.value ? [focused.value.id] : []
  }

  async function dragSamples(sample: Sample) {
    const ids = selectedIds.has(sample.id) ? [...selectedIds] : [sample.id]
    const paths = ids.length === 1 ? [sample.path] : await api.samplePaths(ids)
    await startFileDrag(paths)
  }

  // --- Favorites & tags ----------------------------------------------------

  /** Updates already-loaded rows in place, so the list does not flash while reloading. */
  function patchSamples(ids: number[], patch: (s: Sample) => Sample) {
    const set = new Set(ids)
    for (const [page, rows] of pages) {
      if (rows.some(r => set.has(r.id)))
        pages.set(page, rows.map(r => (set.has(r.id) ? patch(r) : r)))
    }
  }

  /** Whether a change to favorites/tags can move rows in or out of the current view. */
  function viewDependsOnTags() {
    return scope.value.kind === 'favorites' || scope.value.kind === 'tag' || query.value.trim() !== ''
  }

  /** Toggles favorite on `sample` (or the selection it belongs to), else on the selection. */
  async function toggleFavorite(sample?: Sample) {
    const ids = sample && !selectedIds.has(sample.id) ? [sample.id] : targetIds()
    if (!ids.length)
      return
    const favorite = !(sample ?? focused.value)?.favorite
    await api.setFavorite(ids, favorite)
    patchSamples(ids, s => ({ ...s, favorite }))
    if (scope.value.kind === 'favorites')
      await refresh({ keepSelection: true })
  }

  function requestTagInput() {
    tagInputRequest.value++
  }

  async function loadTags() {
    tags.value = await api.listTags()
  }

  async function addTag(name: string) {
    const ids = targetIds()
    if (!ids.length || !name.trim())
      return
    const tag = await api.tagSamples(ids, name)
    patchSamples(ids, s => s.tags.some(t => t.id === tag.id)
      ? s
      : { ...s, tags: [...s.tags, tag].sort((a, b) => a.name.localeCompare(b.name)) })
    await Promise.all([loadTags(), viewDependsOnTags() && refresh({ keepSelection: true })])
  }

  async function removeTag(tagId: number) {
    const ids = targetIds()
    if (!ids.length)
      return
    await api.untagSamples(ids, tagId)
    patchSamples(ids, s => ({ ...s, tags: s.tags.filter(t => t.id !== tagId) }))
    await Promise.all([loadTags(), viewDependsOnTags() && refresh({ keepSelection: true })])
  }

  async function renameTag(id: number, name: string) {
    await api.renameTag(id, name)
    await Promise.all([loadTags(), refresh({ keepSelection: true })])
  }

  async function deleteTag(id: number) {
    await api.deleteTag(id)
    if (scope.value.kind === 'tag' && scope.value.id === id)
      scope.value = { kind: 'all' }
    await Promise.all([loadTags(), refresh({ keepSelection: true })])
  }

  // --- Collections ---------------------------------------------------------

  async function loadCollections() {
    collections.value = await api.listCollections()
  }

  /** Creates a collection, filled with the current selection when `withSelection`. */
  async function createCollection(name: string, withSelection = false) {
    await api.createCollection(name, withSelection ? targetIds() : [])
    await loadCollections()
    if (withSelection)
      showNotice(`Added to “${name.trim()}”`)
  }

  async function addToCollection(id: number) {
    const ids = targetIds()
    if (!ids.length)
      return
    await api.addToCollection(ids, id)
    await loadCollections()
    const name = collections.value.find(c => c.id === id)?.name
    showNotice(`${ids.length === 1 ? 'Sample' : `${ids.length} samples`} added to “${name}”`)
  }

  async function removeFromCollection(id: number) {
    const ids = targetIds()
    if (!ids.length)
      return
    await api.removeFromCollection(ids, id)
    await loadCollections()
    if (scope.value.kind === 'collection' && scope.value.id === id)
      await refresh({ keepSelection: false })
  }

  async function renameCollection(id: number, name: string) {
    await api.renameCollection(id, name)
    await loadCollections()
  }

  async function deleteCollection(id: number) {
    await api.deleteCollection(id)
    if (scope.value.kind === 'collection' && scope.value.id === id)
      scope.value = { kind: 'all' }
    await loadCollections()
  }

  // --- Export / import -----------------------------------------------------

  function showNotice(text: string, error = false) {
    clearTimeout(noticeTimer)
    notice.value = { text, error }
    noticeTimer = setTimeout(() => (notice.value = null), error ? 8000 : 4000)
  }

  async function exportUserData() {
    const date = new Date().toISOString().slice(0, 10)
    const path = await save({
      title: 'Export favorites, tags and collections',
      defaultPath: `samplette-${date}.json`,
      filters: [{ name: 'JSON', extensions: ['json'] }],
    })
    if (!path)
      return
    try {
      const n = await api.exportUserData(path)
      showNotice(`Exported ${n.toLocaleString()} samples`)
    }
    catch (e) {
      showNotice(String(e), true)
    }
  }

  async function importUserData() {
    const path = await open({
      title: 'Import favorites, tags and collections',
      filters: [{ name: 'JSON', extensions: ['json'] }],
    })
    if (typeof path !== 'string')
      return
    try {
      const r = await api.importUserData(path)
      showNotice(r.unmatched
        ? `Imported ${r.matched.toLocaleString()} samples, ${r.unmatched.toLocaleString()} not found in the library`
        : `Imported ${r.matched.toLocaleString()} samples`)
      await Promise.all([loadTags(), loadCollections(), refresh({ keepSelection: true })])
    }
    catch (e) {
      showNotice(String(e), true)
    }
  }

  // --- Sources -------------------------------------------------------------

  async function loadSources() {
    sources.value = await api.listSources()
  }

  async function addSources() {
    const picked = await open({ directory: true, multiple: true, title: 'Add sample folders' })
    const paths = Array.isArray(picked) ? picked : picked ? [picked] : []
    for (const path of paths) {
      const id = await api.addSource(path)
      scans.set(id, { sourceId: id, processed: 0, total: 0, done: false, changes: 0, error: null })
    }
    await loadSources()
  }

  async function removeSource(id: number) {
    await api.removeSource(id)
    if (scope.value.kind === 'source' && scope.value.id === id)
      scope.value = { kind: 'all' }
    await Promise.all([loadSources(), loadTags(), loadCollections(), refresh()])
  }

  async function rescanSource(id: number) {
    scans.set(id, { sourceId: id, processed: 0, total: 0, done: false, changes: 0, error: null })
    await api.rescanSource(id)
  }

  async function init() {
    await listen<AnalysisProgress>(ANALYSIS_PROGRESS_EVENT, ({ payload }) => {
      analysis.value = { pending: payload.pending, paused: payload.paused }
      if (payload.updated.length)
        scheduleReload()
    })
    api.analysisStatus().then((s) => {
      analysis.value = { pending: s.pending, paused: s.paused }
    })
    await listen<ScanProgress>(SCAN_PROGRESS_EVENT, async ({ payload }) => {
      if (!payload.done) {
        scans.set(payload.sourceId, payload)
        return
      }
      scans.delete(payload.sourceId)
      if (payload.error)
        console.error(`scan of source ${payload.sourceId} failed:`, payload.error)
      // Background rescans (startup, file watching) usually find nothing new.
      if (payload.changes > 0)
        await Promise.all([loadSources(), loadTags(), loadCollections(), refresh({ keepSelection: true })])
    })
    await Promise.all([loadSources(), loadTags(), loadCollections(), refresh()])
  }

  return {
    sources,
    tags,
    collections,
    notice,
    scope,
    query,
    kindFilter,
    bpmMin,
    bpmMax,
    keyFilter,
    hasAnalysisFilters,
    clearAnalysisFilters,
    analysis,
    prioritizeAnalysis,
    setAnalysisPaused,
    total,
    scans,
    isScanning,
    pages,
    focusIndex,
    focused,
    selectedIds,
    tagInputRequest,
    requestTagInput,
    sampleAt,
    ensureRange,
    refresh,
    clickRow,
    moveFocus,
    selectAll,
    dragSamples,
    toggleFavorite,
    addTag,
    removeTag,
    renameTag,
    deleteTag,
    createCollection,
    addToCollection,
    removeFromCollection,
    renameCollection,
    deleteCollection,
    exportUserData,
    importUserData,
    addSources,
    removeSource,
    rescanSource,
    init,
  }
})

if (import.meta.hot)
  import.meta.hot.accept(acceptHMRUpdate(useLibraryStore, import.meta.hot))
