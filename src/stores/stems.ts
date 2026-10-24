import type { Sample, StemModel } from '@/api'
import { useLocalStorage } from '@vueuse/core'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { acceptHMRUpdate, defineStore } from 'pinia'
import { ref } from 'vue'
export interface StemProgress { model: StemModel, running: boolean, stage: string, progress: number, error: string | null, cancelled: boolean, paths: string[], ids: number[], name: string }
interface Status { progress: StemProgress, modelReady: boolean, downloadBytes: number }
const initial = (): StemProgress => ({ model: 'scnetLarge', running: false, stage: '', progress: 0, error: null, cancelled: false, paths: [], ids: [], name: '' })
export const useStemsStore = defineStore('stems', () => {
  const open = ref(false)
  const model = useLocalStorage<StemModel>('stems.model', 'scnetLarge')
  if (model.value !== 'spleeter' && model.value !== 'scnetLarge') model.value = 'scnetLarge'
  const sample = ref<Sample | null>(null)
  const progress = ref(initial())
  const ready = ref(false)
  const bytes = ref(78856560)
  const error = ref('')
  const cancelling = ref(false)
  const starting = ref(false)
  let timer: ReturnType<typeof setTimeout> | undefined
  let initialized = false
  let statusVersion = 0
  async function load() {
    clearTimeout(timer)
    const version = ++statusVersion
    try {
      const status = await invoke<Status>('stem_status', { model: model.value })
      if (version !== statusVersion) return
      if (status.progress.running) model.value = status.progress.model
      progress.value = status.progress; ready.value = status.modelReady; bytes.value = status.downloadBytes
      if (!progress.value.running) cancelling.value = false
      if (progress.value.running) timer = setTimeout(load, 500)
    }
    catch (e) { error.value = String(e) }
  }
  function setModel(value: unknown) {
    if (starting.value || progress.value.running || (value !== 'spleeter' && value !== 'scnetLarge')) return
    model.value = value
    void load()
  }
  async function init() {
    if (initialized) return
    initialized = true
    await listen('stems:changed', load)
    await load()
  }
  function show(selected?: Sample) { if (selected) sample.value = selected; error.value = ''; open.value = true; void load() }
  async function start(folder: string) {
    if (!sample.value || starting.value || progress.value.running) return
    starting.value = true; error.value = ''
    try { await invoke('stem_start', { id: sample.value.id, folder, model: model.value }); await load() }
    catch (e) { error.value = String(e) }
    finally { starting.value = false }
  }
  async function cancel() {
    cancelling.value = true
    try { await invoke('stem_cancel'); await load() }
    catch (e) { cancelling.value = false; error.value = String(e) }
  }
  return { open, model, setModel, sample, progress, ready, bytes, error, starting, cancelling, init, show, start, cancel }
})

if (import.meta.hot)
  import.meta.hot.accept(acceptHMRUpdate(useStemsStore, import.meta.hot))
