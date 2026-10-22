import type { Sample, SampleFilter, ScanProgress, Source } from '@/api'
import { listen } from '@tauri-apps/api/event'
import { open } from '@tauri-apps/plugin-dialog'
import { defineStore } from 'pinia'
import { computed, reactive, ref, shallowReactive, watch } from 'vue'
import { api, SCAN_PROGRESS_EVENT } from '@/api'

const PAGE_SIZE = 200

export const useLibraryStore = defineStore('library', () => {
  const sources = ref<Source[]>([])
  const activeSourceId = ref<number | null>(null)
  const query = ref('')
  const total = ref(0)
  const selectedIndex = ref(-1)
  const scans = reactive(new Map<number, ScanProgress>())

  // Rows are fetched lazily in pages as the list scrolls. `generation` invalidates
  // in-flight requests when the filter changes.
  const pages = shallowReactive(new Map<number, Sample[]>())
  const pending = new Map<number, Promise<void>>()
  let generation = 0

  const filter = computed<SampleFilter>(() => ({ query: query.value, sourceId: activeSourceId.value }))
  const selected = computed(() => sampleAt(selectedIndex.value))
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

  async function refresh({ keepSelection = false } = {}) {
    generation++
    pages.clear()
    pending.clear()
    if (!keepSelection)
      selectedIndex.value = -1
    total.value = await api.countSamples(filter.value)
  }

  async function loadSources() {
    sources.value = await api.listSources()
  }

  async function addSources() {
    const picked = await open({ directory: true, multiple: true, title: 'Add sample folders' })
    const paths = Array.isArray(picked) ? picked : picked ? [picked] : []
    for (const path of paths) {
      const id = await api.addSource(path)
      scans.set(id, { sourceId: id, processed: 0, total: 0, done: false, error: null })
    }
    await loadSources()
  }

  async function removeSource(id: number) {
    await api.removeSource(id)
    if (activeSourceId.value === id)
      activeSourceId.value = null
    await Promise.all([loadSources(), refresh()])
  }

  async function rescanSource(id: number) {
    scans.set(id, { sourceId: id, processed: 0, total: 0, done: false, error: null })
    await api.rescanSource(id)
  }

  function select(index: number) {
    if (total.value === 0)
      return
    selectedIndex.value = Math.max(0, Math.min(total.value - 1, index))
  }

  watch(filter, () => refresh())

  async function init() {
    await listen<ScanProgress>(SCAN_PROGRESS_EVENT, async ({ payload }) => {
      if (!payload.done) {
        scans.set(payload.sourceId, payload)
        return
      }
      scans.delete(payload.sourceId)
      if (payload.error)
        console.error(`scan of source ${payload.sourceId} failed:`, payload.error)
      await Promise.all([loadSources(), refresh({ keepSelection: true })])
    })
    await Promise.all([loadSources(), refresh()])
  }

  return {
    sources,
    activeSourceId,
    query,
    total,
    selectedIndex,
    selected,
    scans,
    isScanning,
    pages,
    sampleAt,
    ensureRange,
    refresh,
    addSources,
    removeSource,
    rescanSource,
    select,
    init,
  }
})
