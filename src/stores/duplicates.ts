import type { DuplicateAction, DuplicateCopy, DuplicateGroup, HiddenFile, ResolveReport, ScanProgress } from '@/api'
import { listen } from '@tauri-apps/api/event'
import { useStorage } from '@vueuse/core'
import { acceptHMRUpdate, defineStore } from 'pinia'
import { computed, reactive, ref } from 'vue'
import { api, DUPLICATES_VERIFIED_EVENT, SCAN_PROGRESS_EVENT } from '@/api'
import { useLibraryStore } from '@/stores/library'

export type KeepRule = 'shortest' | 'oldest' | 'newest'

export const useDuplicatesStore = defineStore('duplicates', () => {
  const library = useLibraryStore()

  /** Candidate groups, for the sidebar. */
  const count = ref(0)
  const groups = ref<DuplicateGroup[]>([])
  const loading = ref(false)
  /** Groups that turned out different once their files were hashed whole. */
  const falseMatches = ref(0)
  const rule = useStorage<KeepRule>('samplette-duplicates-rule', 'shortest')
  /** Folders whose copies always win, newest first. */
  const preferred = ref<string[]>([])
  /** Per-group choices made by hand (group key → copy id); they beat the rule and folders. */
  const manual = reactive(new Map<string, number>())
  const selected = reactive(new Set<string>())
  const focusedId = ref<number | null>(null)
  const hidden = ref<HiddenFile[]>([])

  const verifying = computed(() => groups.value.filter(g => !g.verified).length)

  function keepOf(g: DuplicateGroup): DuplicateCopy {
    const byHand = g.copies.find(c => c.id === manual.get(g.key))
    if (byHand)
      return byHand
    for (const folder of preferred.value) {
      const c = g.copies.find(c => c.path.startsWith(`${folder}/`))
      if (c)
        return c
    }
    const sorted = [...g.copies].sort((a, b) =>
      rule.value === 'shortest'
        ? a.path.length - b.path.length || a.path.localeCompare(b.path)
        : rule.value === 'oldest' ? a.mtime - b.mtime : b.mtime - a.mtime)
    return sorted[0]!
  }

  /** Groups an action applies to: the selection, or all of them. */
  const targets = computed(() => selected.size ? groups.value.filter(g => selected.has(g.key)) : groups.value)
  const extras = computed(() => targets.value.flatMap((g) => {
    const keep = keepOf(g)
    return g.copies.filter(c => c.id !== keep.id)
  }))
  /** Extras that may go to the Trash: only groups confirmed byte by byte. */
  const trashable = computed(() => targets.value.filter(g => g.verified).flatMap((g) => {
    const keep = keepOf(g)
    return g.copies.filter(c => c.id !== keep.id)
  }))

  async function loadCount() {
    count.value = await api.duplicateCount()
  }

  /** Group key without the content hash: what an unverified group is known by. */
  function candidateKey(key: string) {
    return key.split(':').slice(0, 2).join(':')
  }

  async function load({ afterVerify = false } = {}) {
    loading.value = true
    try {
      const [list, folders] = await Promise.all([api.duplicateGroups(), api.preferredFolders()])
      if (afterVerify) {
        // A candidate with no verified group left had files that only looked alike.
        const kept = new Set(list.map(g => candidateKey(g.key)))
        falseMatches.value += groups.value.filter(g => !g.verified && !kept.has(g.key)).length
      }
      groups.value = list
      preferred.value = folders
      count.value = list.length
      // Drop choices for groups that no longer exist.
      const keys = new Set(list.map(g => g.key))
      for (const k of [...manual.keys()]) {
        if (!keys.has(k))
          manual.delete(k)
      }
      for (const k of [...selected]) {
        if (!keys.has(k))
          selected.delete(k)
      }
      if (list.some(g => !g.verified))
        await api.verifyDuplicates()
    }
    finally {
      loading.value = false
    }
  }

  async function prefer(folder: string, on = true) {
    await api.setPreferredFolder(folder, on)
    preferred.value = await api.preferredFolders()
  }

  function setKeep(g: DuplicateGroup, copyId: number) {
    manual.set(g.key, copyId)
  }

  /** Trashes or hides `extras` of the given groups and refreshes everything they touched. */
  async function resolve(action: DuplicateAction, only?: { group: DuplicateGroup, copy: DuplicateCopy }): Promise<ResolveReport> {
    let resolutions
    if (only) {
      // Removing the copy that would be kept: another copy stays instead.
      const keep = keepOf(only.group).id !== only.copy.id ? keepOf(only.group) : only.group.copies.find(c => c.id !== only.copy.id)!
      resolutions = [{ keep: keep.id, extras: [only.copy.id] }]
    }
    else {
      resolutions = (action === 'trash' ? targets.value.filter(g => g.verified) : targets.value).map((g) => {
        const keep = keepOf(g)
        return { keep: keep.id, extras: g.copies.filter(c => c.id !== keep.id).map(c => c.id) }
      })
    }
    const report = await api.resolveDuplicates(resolutions, action)
    selected.clear()
    await Promise.all([load(), loadHidden(), library.loadSources(), library.loadTags(), library.loadCollections(), library.refresh({ keepSelection: true })])
    return report
  }

  async function loadHidden() {
    hidden.value = await api.listHidden()
  }

  async function unhide(paths: string[] | null) {
    await api.unhideSamples(paths)
    await loadHidden()
  }

  async function init() {
    await listen(DUPLICATES_VERIFIED_EVENT, async () => {
      if (library.scope.kind === 'duplicates')
        await load({ afterVerify: true })
      else
        await loadCount()
    })
    // New or removed files can create or end duplicates.
    await listen<ScanProgress>(SCAN_PROGRESS_EVENT, async ({ payload }) => {
      if (!payload.done || !payload.changes)
        return
      if (library.scope.kind === 'duplicates')
        await load()
      else
        await loadCount()
    })
    await Promise.all([loadCount(), loadHidden()])
  }

  return {
    count,
    groups,
    loading,
    falseMatches,
    rule,
    preferred,
    manual,
    selected,
    focusedId,
    hidden,
    verifying,
    targets,
    extras,
    trashable,
    keepOf,
    setKeep,
    loadCount,
    load,
    prefer,
    resolve,
    loadHidden,
    unhide,
    init,
  }
})

if (import.meta.hot)
  import.meta.hot.accept(acceptHMRUpdate(useDuplicatesStore, import.meta.hot))
