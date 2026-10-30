import type { Sample, StemModel, StemMode } from '@/api'
import { useLocalStorage } from '@vueuse/core'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { acceptHMRUpdate, defineStore } from 'pinia'
import { computed, ref } from 'vue'
import { isStemModel, modelsForMode } from '@/stem-models'
import { useBackgroundJob, type ModelJobStatus } from '@/composables/backgroundJob'
export interface StemProgress { mode: StemMode, model: StemModel, sourceId: number | null, running: boolean, stage: string, progress: number, error: string | null, cancelled: boolean, paths: string[], ids: number[], name: string }
const initial = (): StemProgress => ({ mode: 'four', model: 'scnetXlIhf', sourceId: null, running: false, stage: '', progress: 0, error: null, cancelled: false, paths: [], ids: [], name: '' })
export const useStemsStore = defineStore('stems', () => {
  const open = ref(false)
  const model = useLocalStorage<StemModel>('stems.model', 'scnetXlIhf')
  const mode = useLocalStorage<StemMode>('stems.mode', 'four')
  if (mode.value !== 'two' && mode.value !== 'four') mode.value = 'four'
  if (!isStemModel(model.value) || !modelsForMode(mode.value).includes(model.value)) model.value = modelsForMode(mode.value)[0]!
  const availableModels = computed(() => modelsForMode(mode.value))
  const sample = ref<Sample | null>(null)
  const job = useBackgroundJob({ initial: initial(), bytes: 221482626,
    read: () => invoke<ModelJobStatus<StemProgress>>('stem_status', { model: model.value }),
    subscribe: reload => listen('stems:changed', reload),
    onStatus(status) {
      if (status.progress.running) { model.value = status.progress.model; mode.value = status.progress.mode }
    },
  })
  const { progress, ready, bytes, error, starting, cancelling, init, load } = job
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
  function show(selected?: Sample) { if (selected) sample.value = selected; job.clearError(); open.value = true; void load() }
  function start() {
    return job.start(async () => {
      if (!sample.value) return false
      await invoke('stem_start', { id: sample.value.id, model: model.value, mode: mode.value })
    })
  }
  function cancel() { return job.cancel(() => invoke('stem_cancel')) }
  return { open, mode, setMode, availableModels, model, setModel, sample, progress, ready, bytes, error, starting, cancelling, init, show, start, cancel }
})

if (import.meta.hot)
  import.meta.hot.accept(acceptHMRUpdate(useStemsStore, import.meta.hot))
