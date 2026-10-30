import type { Sample } from '@/api'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { acceptHMRUpdate, defineStore } from 'pinia'
import { ref } from 'vue'
import { useBackgroundJob, type ModelJobStatus } from '@/composables/backgroundJob'
import { modelCatalog } from '@/model-catalog.generated'
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
const initial = (): ApolloProgress => ({ sourceId: null, running: false, stage: '', progress: 0, error: null, cancelled: false, path: null, id: null, name: '' })
export const useApolloStore = defineStore('apollo', () => {
  const open = ref(false)
  const sample = ref<Sample | null>(null)
  const job = useBackgroundJob({ initial: initial(), bytes: modelCatalog.apollo.bytes,
    read: () => invoke<ModelJobStatus<ApolloProgress>>('apollo_status'), subscribe: reload => listen('apollo:changed', reload),
  })
  const { progress, ready, bytes, error, starting, cancelling, init, load } = job
  function show(selected: Sample) { sample.value = selected; job.clearError(); open.value = true; void load() }
  function start() {
    return job.start(async () => {
      if (!sample.value) return false
      await invoke('apollo_start', { id: sample.value.id })
    })
  }
  function cancel() { return job.cancel(() => invoke('apollo_cancel')) }
  return { open, sample, progress, ready, bytes, error, starting, cancelling, init, show, start, cancel }
})
if (import.meta.hot) import.meta.hot.accept(acceptHMRUpdate(useApolloStore, import.meta.hot))
