import type { Sample, StemModel, StemMode } from '@/api'
import { useLocalStorage } from '@vueuse/core'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { acceptHMRUpdate, defineStore } from 'pinia'
import { computed, ref } from 'vue'
import { isStemModel, modelsForMode } from '@/stem-models'
export interface StemProgress { mode: StemMode, model: StemModel, sourceId: number | null, running: boolean, stage: string, progress: number, error: string | null, cancelled: boolean, paths: string[], ids: number[], name: string }
interface Status { progress: StemProgress, modelReady: boolean, downloadBytes: number }
const initial = (): StemProgress => ({ mode: 'four', model: 'scnetXlIhf', sourceId: null, running: false, stage: '', progress: 0, error: null, cancelled: false, paths: [], ids: [], name: '' })
export const useStemsStore = defineStore('stems', () => {
  const open = ref(false)
  const model = useLocalStorage<StemModel>('stems.model', 'scnetXlIhf')
  const mode = useLocalStorage<StemMode>('stems.mode', 'four')
  if (mode.value !== 'two' && mode.value !== 'four') mode.value = 'four'
  if (!isStemModel(model.value) || !modelsForMode(mode.value).includes(model.value)) model.value = modelsForMode(mode.value)[0]!
  const availableModels = computed(() => modelsForMode(mode.value))
  const sample = ref<Sample | null>(null)
  const progress = ref(initial())
  const ready = ref(false)
  const bytes = ref(221482626)
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
      if (status.progress.running) { model.value = status.progress.model; mode.value = status.progress.mode }
      progress.value = status.progress; ready.value = status.modelReady; bytes.value = status.downloadBytes
      if (!progress.value.running) cancelling.value = false
      if (progress.value.running) timer = setTimeout(load, 500)
    }
    catch (e) { error.value = String(e) }
  }
  function setModel(value: unknown) {
    if (starting.value || progress.value.running || (!isStemModel(value) || !availableModels.value.includes(value))) return
    model.value = value
    void load()
  }
  function setMode(value: unknown) {
    if (starting.value || progress.value.running || (value !== 'two' && value !== 'four')) return
    mode.value = value
    if (!availableModels.value.includes(model.value)) model.value = availableModels.value[0]!
    void load()
  }
  async function init() {
    if (initialized) return
    initialized = true
    await listen('stems:changed', load)
    await load()
  }
  function show(selected?: Sample) { if (selected) sample.value = selected; error.value = ''; open.value = true; void load() }
  async function start() {
    if (!sample.value || starting.value || progress.value.running) return
    starting.value = true; error.value = ''
    try { await invoke('stem_start', { id: sample.value.id, model: model.value, mode: mode.value }); await load() }
    catch (e) { error.value = String(e) }
    finally { starting.value = false }
  }
  async function cancel() {
    cancelling.value = true
    try { await invoke('stem_cancel'); await load() }
    catch (e) { cancelling.value = false; error.value = String(e) }
  }
  return { open, mode, setMode, availableModels, model, setModel, sample, progress, ready, bytes, error, starting, cancelling, init, show, start, cancel }
})

if (import.meta.hot)
  import.meta.hot.accept(acceptHMRUpdate(useStemsStore, import.meta.hot))
