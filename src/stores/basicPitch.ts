import type { Sample } from '@/api'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { save } from '@tauri-apps/plugin-dialog'
import { acceptHMRUpdate, defineStore } from 'pinia'
import { computed, shallowRef } from 'vue'
import { t } from '@/i18n'
import { useBackgroundJob, type ModelJobStatus } from '@/composables/backgroundJob'
export interface MidiRange { start: number, end: number }
type MidiSource = Pick<Sample, 'id' | 'name' | 'path' | 'bpm'>
interface Progress {
  sourceId: number | null, range: MidiRange | null, running: boolean, stage: string, progress: number,
  error: string | null, cancelled: boolean, path: string | null, noteCount: number, name: string,
}
const initial = (): Progress => ({ sourceId: null, range: null, running: false, stage: '', progress: 0, error: null, cancelled: false, path: null, noteCount: 0, name: '' })
export const useBasicPitchStore = defineStore('basicPitch', () => {
  const open = shallowRef(false)
  const sample = shallowRef<MidiSource | null>(null)
  const range = shallowRef<MidiRange | null>(null)
  const sourceKind = shallowRef<'file' | 'selection' | 'editor'>('file')
  const tempo = shallowRef<number | null>(null)
  const job = useBackgroundJob({ initial: initial(), bytes: 230444,
    read: () => invoke<ModelJobStatus<Progress>>('basic_pitch_status'), subscribe: reload => listen('basic-pitch:changed', reload),
  })
  const { progress, ready, bytes, error, starting, cancelling, init, load } = job
  const current = computed(() => progress.value.sourceId === sample.value?.id
    && progress.value.range?.start === range.value?.start && progress.value.range?.end === range.value?.end)
  const result = computed(() => {
    const p = progress.value
    const r = range.value
    return !p.running && p.path && p.sourceId === sample.value?.id
      && p.range?.start === r?.start && p.range?.end === r?.end ? p : null
  })
  function show(source: MidiSource, selected: MidiRange | null = null, selection = false, bpm: number | null = source.bpm) {
    sample.value = { ...source }; range.value = selected ? { ...selected } : null; tempo.value = bpm
    sourceKind.value = selected ? selection ? 'selection' : 'editor' : 'file'
    job.clearError(); open.value = true; void load()
  }
  function start() {
    return job.start(async () => {
      const source = sample.value
      if (!source) return false
      const selected = range.value
      const destination = await save({ title: t('basicPitch.saveTitle'),
        defaultPath: `${source.path.replace(/\.[^/\\.]+$/, '')}${sourceKind.value === 'selection' ? '-selection' : ''}-notes.mid`,
        filters: [{ name: 'MIDI', extensions: ['mid'] }],
      })
      if (!destination) return false
      const path = /\.mid$/i.test(destination) ? destination : `${destination}.mid`
      await invoke('basic_pitch_start', { id: source.id, range: selected, path, bpm: tempo.value })
    })
  }
  function cancel() { return job.cancel(() => invoke('basic_pitch_cancel')) }
  return { open, sample, range, sourceKind, progress, ready, bytes, error, starting, cancelling, result, current, init, show, start, cancel }
})
if (import.meta.hot) import.meta.hot.accept(acceptHMRUpdate(useBasicPitchStore, import.meta.hot))
