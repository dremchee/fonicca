import { modelCatalog } from '@/model-catalog.generated'
import { emit } from '@tauri-apps/api/event'
const initial = () => ({ sourceId: null as number | null, range: null as { start: number, end: number } | null,
  running: false, stage: '', progress: 0, error: null, cancelled: false, path: null as string | null, noteCount: 0, name: '' })
let progress = initial()
let installed = location.hash === '#models'
let downloading: number | null = null
let generation = 0
export function basicPitchStorage() {
  return { id: 'basicPitch', path: '/m/basic-pitch-v1', installed, downloading: downloading != null || (progress.running && progress.stage === 'downloading'),
    busy: downloading != null || progress.running, size: installed ? modelCatalog.basicPitch.bytes : Math.round((downloading ?? 0) * modelCatalog.basicPitch.bytes), downloadSize: modelCatalog.basicPitch.bytes }
}
export function deleteBasicPitchModel() {
  if (progress.running || downloading != null) throw new Error('model is in use')
  installed = false
}
export function downloadBasicPitchModel() {
  if (progress.running || downloading != null) throw new Error('model is in use')
  downloading = 0
  return new Promise<void>(resolve => {
    const tick = () => { downloading = Math.min(1, (downloading ?? 0) + 0.25)
      if (downloading < 1) setTimeout(tick, 200)
      else { installed = true; downloading = null; resolve() }
    }
    setTimeout(tick, 200)
  })
}
export function basicPitchCommand(cmd: string, args: any): unknown {
  if (cmd === 'plugin:dialog|save' && JSON.stringify(args).includes('.mid')) return '/tmp/fonicca-basic-pitch-notes.mid'
  if (cmd === 'basic_pitch_status') return { progress: structuredClone(progress), modelReady: installed, downloadBytes: modelCatalog.basicPitch.bytes }
  if (cmd === 'basic_pitch_cancel') {
    generation++; progress = { ...progress, running: false, cancelled: true, stage: 'cancelled' }
    void emit('basic-pitch:changed', progress); return null
  }
  if (cmd !== 'basic_pitch_start') return undefined
  const current = ++generation
  progress = { ...initial(), running: true, sourceId: args.id, range: args.range ?? null, name: 'Audio to MIDI', stage: installed ? 'transcribing' : 'downloading' }
  const tick = () => {
    if (current !== generation) return
    progress.progress = Math.min(1, progress.progress + 0.25)
    if (progress.progress < 1) setTimeout(tick, 250)
    else if (!installed) { installed = true; progress.stage = 'transcribing'; progress.progress = 0; setTimeout(tick, 250) }
    else { progress = { ...progress, running: false, stage: 'done', path: args.path, noteCount: 16 }; void emit('basic-pitch:changed', progress) }
  }
  setTimeout(tick, 250)
  return null
}
