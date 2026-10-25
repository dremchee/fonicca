import type { Sample, StemModel } from '@/api'
import { emit } from '@tauri-apps/api/event'
const initial = () => ({ model: 'spleeter' as StemModel, sourceId: null as number | null, running: false, stage: '', progress: 0, error: null, cancelled: false, paths: [] as string[], ids: [] as number[], name: '' })
let progress = initial()
const ready: Record<StemModel, boolean> = { spleeter: location.hash === '#models', scnetLarge: location.hash === '#models', scnetXlIhf: location.hash === '#models', htdemucs: location.hash === '#models' }
const bytes = { spleeter: 78856560, scnetLarge: 45082486, scnetXlIhf: 221482626, htdemucs: 316446953 }
let generation = 0
const original = { id: 1, sourceId: 1, path: '/mock/Drum loop.wav', name: 'Drum loop', duration: 8, ext: 'wav', size: 48000, sampleRate: 44100, channels: 2, bitDepth: 24, favorite: false, tags: [], bpm: 120, key: null, kind: 'loop', category: 'drum', bpmSource: 'name', keySource: null, peakDb: -3, lufs: -16, analyzed: true, categorySource: 'name', categoryConfidence: 1, similarity: null } satisfies Sample
let children: Sample[] = []
const downloads: Partial<Record<StemModel, number>> = {}
export function deleteStemModel(model: StemModel) {
  if (downloads[model] != null || (progress.running && progress.model === model)) throw new Error('model is in use')
  ready[model] = false
}
export function downloadStemModel(model: StemModel) {
  if (downloads[model] != null || (progress.running && progress.model === model)) throw new Error('model is in use')
  downloads[model] = 0
  return new Promise<void>(resolve => {
    const tick = () => {
      downloads[model] = Math.min(1, (downloads[model] ?? 0) + 0.2)
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
      progress = { ...initial(), model, sourceId: args.id, running: true, stage: ready[model] ? 'separating' : 'downloading', name: 'Drum loop.wav' }
      const tick = () => {
        if (version !== generation) return
        progress.progress = Math.min(1, progress.progress + 0.2)
        if (progress.progress < 1) setTimeout(tick, 250)
        else if (!ready[model]) { ready[model] = true; progress.stage = 'separating'; progress.progress = 0; setTimeout(tick, 250) }
        else { progress.running = false; progress.stage = 'done'; progress.ids = [101, 102, 103, 104]; progress.paths = ['vocals', 'drums', 'bass', 'other'].map(n => `${folder || '/mock'}/Drum loop - ${n}.wav`); children = progress.paths.map((path, index) => ({ ...original, id: progress.ids[index], path, name: path.split('/').pop()!.replace(/\.wav$/, ''), stemParentId: 1, stemKind: ['vocals','drums','bass','other'][index] as Sample['stemKind'], stemCount: 0 })); localStorage.setItem('mock.stem.results', JSON.stringify(progress.paths.map((path, i) => ({ id: progress.ids[i], path, name: ['vocals', 'drums', 'bass', 'other'][i], duration: 8 })))); void emit('stems:changed', progress) }
      }
      setTimeout(tick, 250); return null
    }
    case 'stem_cancel': generation++; progress = { ...progress, running: false, stage: 'cancelled', cancelled: true }; void emit('stems:changed', progress); return null
  }
  if (location.hash === '#stems') {
    if (cmd === 'plugin:dialog|open') return '/tmp/fonicca-mock-stems'
    const parent = { ...original, stemCount: children.length }
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
