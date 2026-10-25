import type { Sample } from '@/api'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { acceptHMRUpdate, defineStore } from 'pinia'
import { ref } from 'vue'
export interface ApolloProgress {
  sourceId: number | null
  running: boolean
  stage: string
  progress: number
  error: string | null
  cancelled: boolean
  path: string | null
  id: number | null
  name: string
}
interface Status { progress: ApolloProgress, modelReady: boolean, downloadBytes: number }
const initial = (): ApolloProgress => ({ sourceId: null, running: false, stage: '', progress: 0, error: null, cancelled: false, path: null, id: null, name: '' })
export const useApolloStore = defineStore('apollo', () => {
  const open = ref(false)
  const sample = ref<Sample | null>(null)
  const progress = ref(initial())
  const ready = ref(false)
  const bytes = ref(66541845)
  const error = ref('')
  const starting = ref(false)
  const cancelling = ref(false)
  let initialized = false
  let version = 0
  let timer: ReturnType<typeof setTimeout> | undefined
  async function load() {
    clearTimeout(timer)
    const current = ++version
    try {
      const status = await invoke<Status>('apollo_status')
      if (current !== version) return
      progress.value = status.progress; ready.value = status.modelReady; bytes.value = status.downloadBytes
      if (!status.progress.running) cancelling.value = false
      else timer = setTimeout(load, 500)
    }
    catch (e) { error.value = String(e) }
  }
  async function init() {
    if (initialized) return
    initialized = true
    await listen('apollo:changed', load)
    await load()
  }
  function show(selected: Sample) { sample.value = selected; error.value = ''; open.value = true; void load() }
  async function start() {
    if (!sample.value || starting.value || progress.value.running) return
    starting.value = true; error.value = ''
    try { await invoke('apollo_start', { id: sample.value.id }); await load() }
    catch (e) { error.value = String(e) }
    finally { starting.value = false }
  }
  async function cancel() {
    cancelling.value = true
    try { await invoke('apollo_cancel'); await load() }
    catch (e) { cancelling.value = false; error.value = String(e) }
  }
  return { open, sample, progress, ready, bytes, error, starting, cancelling, init, show, start, cancel }
})
if (import.meta.hot) import.meta.hot.accept(acceptHMRUpdate(useApolloStore, import.meta.hot))
