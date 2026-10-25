import type { ApolloProgress } from '@/stores/apollo'
import { emit } from '@tauri-apps/api/event'
const initial = (): ApolloProgress => ({ sourceId: null, running: false, stage: '', progress: 0, error: null, cancelled: false, path: null, id: null, name: '' })
let progress = initial()
let generation = 0
export function apolloCommand(cmd: string, args: any, installed: boolean, didDownload: () => void): unknown {
  if (cmd === 'apollo_status') return { progress: structuredClone(progress), modelReady: installed, downloadBytes: 66541845 }
  if (cmd === 'apollo_cancel') {
    generation++; progress = { ...progress, running: false, stage: 'cancelled', cancelled: true }
    void emit('apollo:changed', progress); return null
  }
  if (cmd === 'apollo_start') {
    const version = ++generation
    progress = { ...initial(), sourceId: args.id, running: true, stage: installed ? 'processing' : 'downloading', name: 'Drum loop.wav' }
    const tick = () => {
      if (version !== generation) return
      progress.progress = Math.min(1, progress.progress + 0.1)
      if (progress.progress < 1) setTimeout(tick, 300)
      else if (progress.stage === 'downloading') { didDownload(); progress.stage = 'processing'; progress.progress = 0; setTimeout(tick, 300) }
      else {
        progress = { ...progress, running: false, stage: 'done', path: '/mock/Drum loop - Apollo.wav', id: 201 }
        void emit('apollo:changed', progress)
      }
    }
    setTimeout(tick, 300); return null
  }
  return undefined
}
export function apolloBusy() { return progress.running }
