import { useLibraryNavigation } from '@/composables/library/navigation'
export type { Scope } from '@/library-scope'
import { useTagColors } from '@/composables/tagColors'
import type { AnalysisProgress, Collection, DownloadProgress, Sample, SampleFilter, ScanProgress, SoundModelStatus, Source, SourceFolder, Tag } from '@/api'
import { listen } from '@tauri-apps/api/event'
import { open, save } from '@tauri-apps/plugin-dialog'
import { acceptHMRUpdate, defineStore } from 'pinia'
import { StorageSerializers, useStorage } from '@vueuse/core'
import type { LibrarySession } from '@/library-session'
import { findSessionSampleIndex, parseLibrarySession, resolveSessionScope, sessionScope } from '@/library-session'
import { computed, nextTick, onScopeDispose, reactive, ref, shallowReactive, watch } from 'vue'
import { createLibraryPages } from '@/library-pages'
import { createLibrarySelection } from '@/library-selection'
import { toast } from 'vue-sonner'
import { ANALYSIS_PROGRESS_EVENT, api, MODEL_DOWNLOAD_EVENT, SCAN_PROGRESS_EVENT, startFileDrag } from '@/api'
import { formatNumber } from '@/format'
import { t } from '@/i18n'
import { usePlayerStore } from '@/stores/player'
import { useWaveformBackgrounds } from '@/composables/waveformBackgrounds'

const PAGE_SIZE = 200

/** "1,234 samples" in the current language. */
function countSamples(n: number) {
  return t('common.count.samples', { n: formatNumber(n) }, n)
}

export const useLibraryStore = defineStore('library', () => {
  const tagColors = useTagColors()
  const sources = ref<Source[]>([])
  const sourceFolders = shallowReactive(new Map<number, SourceFolder[]>())
  const tags = ref<Tag[]>([])
  const collections = ref<Collection[]>([])
  const lastSession = useStorage<LibrarySession | null>('fonicca-library-session-v1', null, undefined, {
    serializer: StorageSerializers.object, flush: 'sync', listenToStorageChanges: false,
  })
  let sessionReady = false
  let restoringSession = true
  let navigationRevision = 0
  const analysis = ref({ pending: 0, pendingEmbeddings: 0, modelReady: false, paused: false })
  const soundModel = ref<SoundModelStatus | null>(null)
  const modelDownload = ref<DownloadProgress | null>(null)
  const expandedStems = reactive(new Set<number>())
  const { scope, query, sort, kindFilter, bpmMin, bpmMax, keyFilter, categoryFilter,
    filter, hasAnalysisFilters, clearAnalysisFilters, setSort, findSimilar, exitSimilar } = useLibraryNavigation(expandedStems)
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
  // Not store state, so a hot reload would drop the loaded rows (and with them the
  // focused sample, until the list fetched its page again): kept in the HMR data.
  const pages: Map<number, Sample[]> = import.meta.hot?.data.pages ?? shallowReactive(new Map<number, Sample[]>())
  if (import.meta.hot)
    import.meta.hot.data.pages = pages
  let generation = 0

  const focused = computed(() => sampleAt(focusIndex.value))
  const isScanning = computed(() => scans.size > 0)

  const pageCache = createLibraryPages({
    pages, pageSize: PAGE_SIZE,
    fetch: page => api.listSamples(filter.value, page * PAGE_SIZE, PAGE_SIZE),
    protectedPages: () => focusIndex.value < 0 ? [] : [Math.floor(focusIndex.value / PAGE_SIZE)],
  })
  const { sampleAt, ensureRange, ensureVisibleRange } = pageCache
  onScopeDispose(pageCache.dispose)

  /** Reloads rows and count. Selection is kept unless the filter itself changed. */
  async function refresh({ keepSelection = false } = {}) {
    const gen = ++generation
    pageCache.clear()
    if (!keepSelection) {
      focusIndex.value = -1
      anchorIndex.value = -1
      selectedIds.clear()
    }
    try {
      const count = await api.countSamples(filter.value)
      if (gen !== generation)
        return
      total.value = count
    }
    catch (e) {
      // E.g. "find similar" without the sound model: go back to where we were.
      if (scope.value.kind !== 'similar')
        throw e
      toast.error(String(e))
      exitSimilar()
      return
    }
    if (focusIndex.value >= total.value)
      focusIndex.value = total.value - 1
  }

  watch(filter, () => navigationRevision++, { flush: 'sync' })
  watch(filter, (next, previous) => {
    // Expansion reloads below while preserving selection and keyboard focus.
    if (JSON.stringify({ ...next, expandedStems: [] }) === JSON.stringify({ ...previous, expandedStems: [] })) return
    const withoutSort = (f: SampleFilter) => JSON.stringify({ ...f, sortBy: null, sortDirection: null })
    if (!restoringSession && withoutSort(next) === withoutSort(previous)) {
      void refreshSorted()
      return
    }
    if (!restoringSession) refresh()
  }, { flush: 'sync' })

  async function refreshSorted() {
    const id = focused.value?.id
    const revision = navigationRevision
    try {
      await refresh({ keepSelection: true })
      if (revision !== navigationRevision) return
      const gen = generation
      const ids = id == null ? [] : await api.sampleIds(filter.value)
      if (revision !== navigationRevision || gen !== generation) return
      focusIndex.value = id == null ? -1 : ids.indexOf(id)
      anchorIndex.value = focusIndex.value
      if (focusIndex.value >= 0) {
        await ensureRange(focusIndex.value, focusIndex.value)
        if (revision !== navigationRevision || gen !== generation) return
        const sample = sampleAt(focusIndex.value)
        if (sample) rememberSelection(sample, focusIndex.value)
      }
    }
    catch (e) { toast.error(String(e)) }
  }

  let expanding = false
  async function toggleStems(sample: Sample, index: number) {
    if (expanding || !sample.stemCount || scope.value.kind === 'similar') return
    expanding = true
    const collapse = expandedStems.has(sample.id)
    const previousFocus = focused.value
    const previousIndex = focusIndex.value
    if (collapse) expandedStems.delete(sample.id)
    else expandedStems.add(sample.id)
    try {
      await refresh({ keepSelection: true })
      let next = previousIndex
      if (collapse && previousFocus?.stemParentId === sample.id) {
        next = index
        selectedIds.clear(); selectedIds.add(sample.id)
      }
      else if (previousIndex > index) next += (collapse ? -1 : 1) * sample.stemCount
      focusIndex.value = Math.max(-1, Math.min(next, total.value - 1))
      anchorIndex.value = focusIndex.value
      if (focusIndex.value >= 0) await ensureRange(focusIndex.value, focusIndex.value)
    }
    finally { expanding = false }
  }

  /** Reveal a global search result without relying on the current folder or filters. */
  async function revealSample(sample: Sample) {
    scope.value = { kind: 'all' }
    clearAnalysisFilters()
    query.value = sample.name
    if (sample.stemParentId != null)
      expandedStems.add(sample.stemParentId)
    const navigation = navigationRevision
    await nextTick()
    if (navigation !== navigationRevision)
      return false
    await refresh()
    if (navigation !== navigationRevision)
      return false
    const gen = generation
    const ids = await api.sampleIds(filter.value)
    const index = ids.indexOf(sample.id)
    if (index < 0 || gen !== generation)
      return false
    await ensureRange(index, index)
    if (gen !== generation || sampleAt(index)?.id !== sample.id)
      return false
    await clickRow(index)
    return true
  }

  function rememberSelection(sample: Sample, index: number) {
    const current = sessionScope(scope.value)
    if (sessionReady && current)
      lastSession.value = { scope: current, sample: { id: sample.id, index } }
  }

  watch(scope, (value) => {
    const current = sessionScope(value)
    if (!sessionReady || !current)
      return
    const previous = parseLibrarySession(lastSession.value)
    lastSession.value = {
      scope: current,
      sample: JSON.stringify(previous?.scope) === JSON.stringify(current) ? previous?.sample ?? null : null,
    }
  }, { flush: 'sync', deep: true })

  /** Re-fetches loaded pages in place (no flashing) after background updates. */
  let reloadTimer: ReturnType<typeof setTimeout> | undefined
  function scheduleReload() {
    if (reloadTimer)
      return
    reloadTimer = setTimeout(async () => {
      reloadTimer = undefined
      const gen = generation
      try {
        // Filters on analysis results can change which rows match.
        if (hasAnalysisFilters.value) {
          const n = await api.countSamples(filter.value)
          if (gen !== generation) return
          total.value = n
        }
        await Promise.all([...pages.keys()].map(page => pageCache.refreshCached(page)))
      }
      catch (error) { console.error('Failed to refresh library rows', error) }
    }, 1500)
  }

  /** Moves not-yet-analyzed samples to the front of the analysis queue. */
  function prioritizeAnalysis(samples: (Sample | undefined)[]) {
    const ids = samples.filter((s): s is Sample => !!s && !s.analyzed).map(s => s.id)
    if (ids.length && !analysis.value.paused)
      api.prioritizeAnalysis(ids)
  }

  async function loadSoundModel() {
    soundModel.value = await api.soundModelStatus()
  }

  async function downloadSoundModel() {
    modelDownload.value = { received: 0, total: soundModel.value?.size ?? 0, done: false, error: null }
    await api.downloadSoundModel()
  }

  async function deleteSoundModel() {
    await api.deleteSoundModel()
    await loadSoundModel()
  }

  async function setAnalysisPaused(paused: boolean) {
    analysis.value = { ...analysis.value, paused }
    await api.setAnalysisPaused(paused)
  }

  // --- Selection -----------------------------------------------------------

  const selection = createLibrarySelection({
    focusIndex, anchorIndex, selectedIds,
    total: () => total.value, identity: () => `${navigationRevision}:${generation}`,
    sampleAt, ensureRange,
    rangeIds: (a, b) => api.sampleIds(filter.value, Math.min(a, b), Math.abs(b - a) + 1),
    allIds: () => api.sampleIds(filter.value), remember: rememberSelection,
  })
  const { clickRow, moveFocus, selectAll, targetIds } = selection

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
    // The player may be showing it after it left the selection.
    usePlayerStore().patchPlaying(set, patch)
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

  async function createTag(name: string) {
    if (!name.trim())
      return
    await api.tagSamples([], name.trim())
    await loadTags()
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
    tagColors.setColor(id)
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
      toast.success(t('list.toasts.addedTo', { name: name.trim() }))
  }

  async function addToCollection(id: number) {
    const ids = targetIds()
    if (!ids.length)
      return
    await api.addToCollection(ids, id)
    await loadCollections()
    const name = collections.value.find(c => c.id === id)?.name
    toast.success(ids.length === 1
      ? t('list.toasts.sampleAddedTo', { name })
      : t('list.toasts.samplesAddedTo', { n: formatNumber(ids.length), name }, ids.length))
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

  async function exportUserData() {
    const date = new Date().toISOString().slice(0, 10)
    const path = await save({
      title: t('list.dialogs.exportTitle'),
      defaultPath: `fonicca-${date}.json`,
      filters: [{ name: 'JSON', extensions: ['json'] }],
    })
    if (!path)
      return
    try {
      const n = await api.exportUserData(path)
      toast.success(t('list.toasts.exported', { samples: countSamples(n) }))
    }
    catch (e) {
      toast.error(String(e))
    }
  }

  async function importUserData() {
    const path = await open({
      title: t('list.dialogs.importTitle'),
      filters: [{ name: 'JSON', extensions: ['json'] }],
    })
    if (typeof path !== 'string')
      return
    try {
      const r = await api.importUserData(path)
      toast.success(r.unmatched
        ? t('list.toasts.importedPartly', { samples: countSamples(r.matched), missing: formatNumber(r.unmatched) })
        : t('list.toasts.imported', { samples: countSamples(r.matched) }))
      await Promise.all([loadTags(), loadCollections(), refresh({ keepSelection: true })])
    }
    catch (e) {
      toast.error(String(e))
    }
  }

  // --- Sources -------------------------------------------------------------

  async function loadSources() {
    sources.value = await api.listSources()
    for (const id of sourceFolders.keys()) {
      if (!sources.value.some(s => s.id === id))
        sourceFolders.delete(id)
    }
    await Promise.all([...sourceFolders.keys()].map(loadSourceFolders))
  }

  async function loadSourceFolders(id: number) {
    const folders = await api.sourceFolders(id)
    if (sources.value.some(s => s.id === id))
      sourceFolders.set(id, folders)
  }

  async function addSources() {
    const picked = await open({ directory: true, multiple: true, title: t('list.dialogs.addFoldersTitle') })
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
    await listen<number>('editor:saved', async ({ payload: id }) => {
      useWaveformBackgrounds().invalidate(id)
      await Promise.all([loadSources(), refresh({ keepSelection: true })])
    })
    await listen<{ id: number | null }>('apollo:changed', async ({ payload }) => {
      if (payload.id != null) await Promise.all([loadSources(), loadTags(), refresh({ keepSelection: true })])
    })
    await listen<{ ids: number[] }>('stems:changed', async ({ payload }) => {
      if (payload.ids.length) await Promise.all([loadSources(), loadTags(), loadCollections(), refresh({ keepSelection: true })])
    })
    const initialSelectionRevision = selection.version()
    await listen<AnalysisProgress>(ANALYSIS_PROGRESS_EVENT, ({ payload }) => {
      const { updated, ...status } = payload
      analysis.value = status
      if (updated.length)
        scheduleReload()
    })
    await listen<DownloadProgress>(MODEL_DOWNLOAD_EVENT, async ({ payload }) => {
      modelDownload.value = payload.done && !payload.error ? null : payload
      if (payload.done)
        await loadSoundModel()
    })
    api.analysisStatus().then(({ updated: _, ...status }) => {
      analysis.value = status
    })
    loadSoundModel()
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
    await Promise.all([loadSources(), loadTags(), loadCollections()])
    const saved = parseLibrarySession(lastSession.value)
    const canRestore = navigationRevision === 0 && selection.version() === initialSelectionRevision
    let restoreSelection = false
    if (saved && canRestore) {
      let restored = resolveSessionScope(saved.scope, sources.value, tags.value, collections.value)
      if (restored.kind === 'source' && restored.path) {
        const sourceScope = restored
        try {
          await loadSourceFolders(sourceScope.id)
          restored = resolveSessionScope(sourceScope, sources.value, tags.value, collections.value, sourceFolders.get(sourceScope.id))
        }
        catch {
          restored = { kind: 'source', id: sourceScope.id }
        }
      }
      // User navigation during folder loading takes precedence over the saved view.
      if (navigationRevision === 0 && selection.version() === initialSelectionRevision) {
        scope.value = restored
        restoreSelection = true
      }
    }
    const restoreRevision = navigationRevision
    const restoreSelectionRevision = selection.version()
    await nextTick()
    restoringSession = false
    const loading = refresh()
    const gen = generation
    await loading
    try {
      if (saved?.sample && restoreSelection && navigationRevision === restoreRevision && selection.version() === restoreSelectionRevision && gen === generation) {
        const currentFilter = filter.value
        const index = await findSessionSampleIndex(saved.sample, total.value, async (index) => {
          await ensureRange(index, index)
          return gen === generation ? sampleAt(index)?.id : undefined
        }, () => gen === generation ? api.sampleIds(currentFilter) : Promise.resolve([]))
        if (index >= 0 && gen === generation && navigationRevision === restoreRevision && selection.version() === restoreSelectionRevision) {
          await ensureRange(index, index)
          if (gen === generation && navigationRevision === restoreRevision && selection.version() === restoreSelectionRevision && sampleAt(index)?.id === saved.sample.id)
            await clickRow(index)
        }
      }
    }
    catch (error) {
      // A stale bookmark must never prevent the library from opening.
      console.warn('Could not restore the last sample', error)
    }
    finally {
      sessionReady = true
      const current = sessionScope(scope.value)
      if (current)
        lastSession.value = { scope: current, sample: focused.value ? { id: focused.value.id, index: focusIndex.value } : null }
    }
  }

  return {
    expandedStems, toggleStems,
    sources,
    sourceFolders,
    loadSourceFolders,
    tags,
    collections,
    scope,
    query,
    kindFilter,
    bpmMin,
    bpmMax,
    keyFilter,
    categoryFilter,
    hasAnalysisFilters,
    clearAnalysisFilters,
    analysis,
    soundModel,
    modelDownload,
    downloadSoundModel,
    prioritizeAnalysis,
    setAnalysisPaused,
    deleteSoundModel,
    total,
    scans,
    isScanning,
    pages,
    focusIndex,
    focused,
    selectedIds,
    tagInputRequest,
    requestTagInput,
    sort,
    setSort,
    sampleAt,
    ensureRange,
    ensureVisibleRange,
    pageDiagnostics: pageCache.diagnostics,
    refresh,
    clickRow,
    revealSample,
    moveFocus,
    selectAll,
    dragSamples,
    findSimilar,
    exitSimilar,
    toggleFavorite,
    addTag,
    createTag,
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
    loadSources,
    loadTags,
    loadCollections,
    init,
  }
})

if (import.meta.hot)
  import.meta.hot.accept(acceptHMRUpdate(useLibraryStore, import.meta.hot))
