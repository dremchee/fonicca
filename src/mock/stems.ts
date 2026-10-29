import type { Sample, StemModel, StemMode } from '@/api'
import { stemModelBytes, stemModels, stemNames } from '@/stem-models'
import { emit } from '@tauri-apps/api/event'
const initial = () => ({ mode: 'four' as StemMode, model: 'spleeter' as StemModel, sourceId: null as number | null, running: false, stage: '', progress: 0, error: null, cancelled: false, paths: [] as string[], ids: [] as number[], name: '' })
let progress = initial()
const ready = Object.fromEntries(stemModels.map(model => [model, location.hash === '#models' || location.hash === '#stem-processing'])) as Record<StemModel, boolean>
const bytes = stemModelBytes
let generation = 0
export const stemSample = { id: 1, sourceId: 1, path: '/mock/Drum loop.wav', name: 'Drum loop', duration: 8, ext: 'wav', size: 48000, sampleRate: 44100, channels: 2, bitDepth: 24, favorite: false, tags: [], bpm: 120, key: null, kind: 'loop', category: 'drum', bpmSource: 'name', keySource: null, peakDb: -3, lufs: -16, analyzed: true, categorySource: 'name', categoryConfidence: 1, similarity: null } satisfies Sample
let children: Sample[] = []
const downloads: Partial<Record<StemModel, number>> = {}
export function deleteStemModel(model: StemModel) {
  if (downloads[model] != null || (progress.running && progress.model === model)) throw new Error('model is in use')
  ready[model] = false
}
export function downloadStemModel(model: StemModel) {
  if (downloads[model] != null || (progress.running && progress.model === model)) throw new Error('model is in use')
  downloads[model] = location.hash === '#model-downloads' ? 0.3 : 0
  return new Promise<void>(resolve => {
    const tick = () => {
      downloads[model] = Math.min(1, (downloads[model] ?? 0) + (location.hash === '#model-downloads' ? 0.001 : 0.2))
      if (downloads[model]! < 1) setTimeout(tick, 400)
      else { ready[model] = true; delete downloads[model]; resolve() }
    }
    setTimeout(tick, 400)
  })
}
export function stemModelStorage(model: StemModel) {
  const downloading = downloads[model] != null || (progress.running && progress.stage === 'downloading' && progress.model === model)
  const busy = downloading || (progress.running && progress.model === model)
  return { id: model, path: `/m/${model}`, installed: ready[model], downloading, busy, size: ready[model] ? bytes[model] : downloading ? Math.round((downloads[model] ?? progress.progress) * bytes[model]) : 0, downloadSize: bytes[model] }
}
export function stemsCommand(cmd: string, args: any, folder?: string | null): unknown {
  switch (cmd) {
    case 'stem_status': { const model: StemModel = progress.running ? progress.model : args.model ?? 'spleeter'; return { progress: structuredClone(progress), modelReady: ready[model], downloadBytes: bytes[model] } }
    case 'stem_start': {
      const version = ++generation
      const model: StemModel = args.model ?? 'spleeter'
      const mode: StemMode = args.mode ?? 'four'
      const names = stemNames(mode)
      progress = { ...initial(), model, mode, sourceId: args.id, running: true, stage: ready[model] ? 'separating' : 'downloading', name: 'Drum loop.wav', progress: location.hash === '#stem-processing' ? 0.3 : 0 }
      const tick = () => {
        if (version !== generation) return
        progress.progress = Math.min(1, progress.progress + (location.hash === '#stem-processing' ? 0.001 : 0.2))
        if (progress.progress < 1) setTimeout(tick, 250)
        else if (!ready[model]) { ready[model] = true; progress.stage = 'separating'; progress.progress = 0; setTimeout(tick, 250) }
        else { progress.running = false; progress.stage = 'done'; progress.ids = names.map((_, i) => 101 + i); progress.paths = names.map(n => `${folder || '/mock'}/Drum loop - ${n}.wav`); children = progress.paths.map((path, index) => ({ ...stemSample, id: progress.ids[index], path, name: path.split('/').pop()!.replace(/\.wav$/, ''), stemParentId: 1, stemKind: names[index] as Sample['stemKind'], stemCount: 0 })); localStorage.setItem('mock.stem.results', JSON.stringify(progress.paths.map((path, i) => ({ id: progress.ids[i], path, name: names[i], duration: 8 })))); void emit('stems:changed', progress) }
      }
      setTimeout(tick, 250); return null
    }
    case 'stem_cancel': generation++; progress = { ...progress, running: false, stage: 'cancelled', cancelled: true }; void emit('stems:changed', progress); return null
  }
  if (location.hash === '#stems' || location.hash === '#stem-processing') {
    if (cmd === 'plugin:dialog|open') return '/tmp/fonicca-mock-stems'
    const parent = { ...stemSample, stemCount: children.length }
    const expanded = args.filter?.expandedStems?.includes(1)
    const visible = args.filter?.groupStems ? [parent, ...(expanded ? children : [])] : [parent, ...children]
    const query = args.filter?.query?.toLowerCase() || ''
    const matching = !query || [parent,...children].some(s=>s.name.toLowerCase().includes(query))
    const rows = matching ? visible : []
    if (cmd === 'count_samples') return rows.length
    if (cmd === 'list_samples') return rows.slice(args.offset || 0, (args.offset || 0) + (args.limit || 200))
    if (cmd === 'sample_ids') return rows.map(s=>s.id)
  }
  return undefined
}
