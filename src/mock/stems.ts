import { emit } from '@tauri-apps/api/event'
const initial = () => ({ running: false, stage: '', progress: 0, error: null, cancelled: false, paths: [] as string[], ids: [] as number[], name: '' })
let progress = initial()
let ready = false
let generation = 0
export function stemsCommand(cmd: string, args: any): unknown {
  switch (cmd) {
    case 'stem_status': return { progress: structuredClone(progress), modelReady: ready, downloadBytes: 78856560 }
    case 'stem_start': {
      const version = ++generation
      progress = { ...initial(), running: true, stage: ready ? 'separating' : 'downloading', name: 'Drum loop.wav' }
      const tick = () => {
        if (version !== generation) return
        progress.progress = Math.min(1, progress.progress + 0.2)
        if (progress.progress < 1) setTimeout(tick, 250)
        else if (!ready) { ready = true; progress.stage = 'separating'; progress.progress = 0; setTimeout(tick, 250) }
        else { progress.running = false; progress.stage = 'done'; progress.ids = [101, 102, 103, 104]; progress.paths = ['vocals', 'drums', 'bass', 'other'].map(n => `${args.folder}/stems/${n}.wav`); localStorage.setItem('mock.stem.results', JSON.stringify(progress.paths.map((path, i) => ({ id: progress.ids[i], path, name: ['vocals', 'drums', 'bass', 'other'][i], duration: 8 })))); void emit('stems:changed', progress) }
      }
      setTimeout(tick, 250); return null
    }
    case 'stem_cancel': generation++; progress = { ...progress, running: false, stage: 'cancelled', cancelled: true }; void emit('stems:changed', progress); return null
  }
  if (location.hash === '#stems') {
    if (cmd === 'plugin:dialog|open') return '/tmp/fonicca-mock-stems'
    if (cmd === 'count_samples') return 1
    if (cmd === 'list_samples') return [{ id: 1, sourceId: 1, path: '/mock/Drum loop.wav', name: 'Drum loop', duration: 8, ext: 'wav', size: 48000, sampleRate: 44100, channels: 2, bitDepth: 24, favorite: false, tags: [], bpm: 120, key: null, kind: 'loop', category: 'drum' }]
  }
  return undefined
}
