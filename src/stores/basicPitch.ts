import type { Sample } from '@/api'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { save } from '@tauri-apps/plugin-dialog'
import { acceptHMRUpdate, defineStore } from 'pinia'
import { computed, shallowRef } from 'vue'
import { t } from '@/i18n'
export interface MidiRange { start: number, end: number }
type MidiSource = Pick<Sample, 'id' | 'name' | 'path' | 'bpm'>
interface Progress {
  sourceId: number | null, range: MidiRange | null, running: boolean, stage: string, progress: number,
  error: string | null, cancelled: boolean, path: string | null, noteCount: number, name: string,
}
interface Status { progress: Progress, modelReady: boolean, downloadBytes: number }
const initial = (): Progress => ({ sourceId: null, range: null, running: false, stage: '', progress: 0, error: null, cancelled: false, path: null, noteCount: 0, name: '' })
export const useBasicPitchStore = defineStore('basicPitch', () => {
  const open = shallowRef(false)
  const sample = shallowRef<MidiSource | null>(null)
  const range = shallowRef<MidiRange | null>(null)
  const sourceKind = shallowRef<'file' | 'selection' | 'editor'>('file')
  const tempo = shallowRef<number | null>(null)
  const progress = shallowRef(initial())
  const ready = shallowRef(false)
  const bytes = shallowRef(230444)
  const error = shallowRef('')
  const starting = shallowRef(false)
  const cancelling = shallowRef(false)
  const current = computed(() => progress.value.sourceId === sample.value?.id
    && progress.value.range?.start === range.value?.start && progress.value.range?.end === range.value?.end)
  const result = computed(() => {
    const p = progress.value
    const r = range.value
    return !p.running && p.path && p.sourceId === sample.value?.id
      && p.range?.start === r?.start && p.range?.end === r?.end ? p : null
  })
  let initialized = false
  let version = 0
  let timer: ReturnType<typeof setTimeout> | undefined
  async function load() {
    clearTimeout(timer)
    const current = ++version
    try {
      const state = await invoke<Status>('basic_pitch_status')
      if (current !== version) return
      progress.value = state.progress; ready.value = state.modelReady; bytes.value = state.downloadBytes
      if (!state.progress.running) cancelling.value = false
      else timer = setTimeout(load, 500)
    }
    catch (e) { error.value = String(e) }
  }
  async function init() {
    if (initialized) return
    initialized = true
    await listen('basic-pitch:changed', load)
    await load()
  }
  function show(source: MidiSource, selected: MidiRange | null = null, selection = false, bpm: number | null = source.bpm) {
    sample.value = { ...source }; range.value = selected ? { ...selected } : null; tempo.value = bpm
    sourceKind.value = selected ? selection ? 'selection' : 'editor' : 'file'
    error.value = ''; open.value = true; void load()
  }
  async function start() {
    if (!sample.value || starting.value || progress.value.running) return
    const source = sample.value
    const selected = range.value
    starting.value = true; error.value = ''
    try {
      const destination = await save({ title: t('basicPitch.saveTitle'),
        defaultPath: `${source.path.replace(/\.[^/\\.]+$/, '')}${sourceKind.value === 'selection' ? '-selection' : ''}-notes.mid`,
        filters: [{ name: 'MIDI', extensions: ['mid'] }],
      })
      if (!destination) return
      const path = /\.mid$/i.test(destination) ? destination : `${destination}.mid`
      await invoke('basic_pitch_start', { id: source.id, range: selected, path, bpm: tempo.value })
      await load()
    }
    catch (e) { error.value = String(e) }
    finally { starting.value = false }
  }
  async function cancel() {
    cancelling.value = true
    try { await invoke('basic_pitch_cancel'); await load() }
    catch (e) { cancelling.value = false; error.value = String(e) }
  }
  return { open, sample, range, sourceKind, progress, ready, bytes, error, starting, cancelling, result, current, init, show, start, cancel }
})
if (import.meta.hot) import.meta.hot.accept(acceptHMRUpdate(useBasicPitchStore, import.meta.hot))
