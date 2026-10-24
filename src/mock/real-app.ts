// The real App on a fake backend (mockIPC), to click through the UI in a browser.
import type { DuplicateGroup, HiddenFile } from '@/api'
import { emit } from '@tauri-apps/api/event'
import { mockIPC } from '@tauri-apps/api/mocks'
import { groups as fake } from './duplicates-data'
import { batchCommand } from './batch'
import { editorCommand } from './editor'

const ROOT = '/Users/me/Projects/Sound/Source'
let groups: DuplicateGroup[] = fake.map(g => ({
  key: `${g.id.toString(16)}:${g.size}`,
  size: g.size,
  verified: !g.verifying,
  copies: g.copies.map(c => ({
    id: c.id, sourceId: 1, path: `${ROOT}/${c.path}`, name: c.path.split('/').pop()!.replace(/\.wav$/, ''), ext: 'wav',
    size: g.size, duration: g.duration, sampleRate: 44100, channels: 2, bitDepth: 24, favorite: false, tags: [],
    kind: null, bpm: null, bpmSource: null, key: null, keySource: null, peakDb: null, lufs: null, analyzed: true,
    category: null, categorySource: null, categoryConfidence: null, similarity: null,
    mtime: Math.floor(Date.now() / 1000) - c.age * 86400,
  })),
}))
let hidden: HiddenFile[] = []
let playStart: number | null = null
let playFrom = 0
let preferred: string[] = []
let settings: any = { theme: 'system', colorTheme: 'fonicca', customThemes: [], language: 'en', watchFolders: true, autoplay: true, volume: 1, analysisThreads: null, syncEnabled: false, projectBpm: null, projectKey: null, waveStyle: 'peaksRms', waveChannels: 'split', waveScale: 'linear', waveColors: 'theme' }

export function install() {
  mockIPC((cmd, args: any) => {
    switch (cmd) {
      case 'list_sources': return [{ id: 1, path: ROOT, sampleCount: 32187 }]
      case 'count_samples': return 0
      case 'list_samples': case 'sample_ids': case 'list_tags': case 'list_collections': return []
      case 'analysis_status': return { pending: 0, pendingEmbeddings: 0, modelReady: false, paused: false, updated: [] }
      case 'sound_model_status': return { installed: false, ready: false, downloading: false, size: 34301667 }
      case 'get_settings': return settings
      case 'update_settings': settings = args.settings; return settings
      case 'app_info': return { version: '0.1.0', cores: 10, defaultThreads: 5, dbPath: '/db', dbSize: 1, modelsDir: '/m', modelsSize: 0, settingsPath: '/s' }
      case 'duplicate_count': return groups.length
      case 'duplicate_groups': return structuredClone(groups)
      case 'verify_duplicates':
        // Hash in "the background": one candidate turns out different.
        setTimeout(() => {
          const pending = groups.filter(g => !g.verified)
          groups = groups.filter(g => g !== pending.at(-1)).map(g => g.verified ? g : { ...g, key: `${g.key}:abcd`, verified: true })
          emit('duplicates-verified', { hashed: pending.length, error: null })
        }, 2500)
        return null
      case 'resolve_duplicates': {
        let removed = 0
        let bytes = 0
        for (const r of args.resolutions as { keep: number, extras: number[] }[]) {
          for (const g of groups) {
            const gone = g.copies.filter(c => r.extras.includes(c.id))
            if (args.action === 'hide')
              hidden.unshift(...gone.map(c => ({ path: c.path, sourceId: 1, hiddenAt: Date.now() / 1000 })))
            removed += gone.length
            bytes += gone.length * g.size
            g.copies = g.copies.filter(c => !r.extras.includes(c.id))
          }
        }
        groups = groups.filter(g => g.copies.length > 1)
        return { removed, bytes, skipped: [] }
      }
      case 'list_hidden': return hidden
      case 'unhide_samples': hidden = args.paths ? hidden.filter(h => !args.paths.includes(h.path)) : []; return null
      case 'preferred_folders': return preferred
      case 'set_preferred_folder':
        preferred = args.preferred ? [args.path, ...preferred.filter(p => p !== args.path)] : preferred.filter(p => p !== args.path)
        return null
      // Playback: time runs from the play call; levels mimic a kick on every beat at 120 BPM.
      case 'editor_play': playStart = performance.now(); playFrom = args.start / 48000; return null
      case 'play': playStart = performance.now(); playFrom = args.start ?? 0; return { stretch: 1, semitones: 0 }
      case 'stop': playStart = null; return null
      case 'playback_position': return playStart === null ? null : playFrom + (performance.now() - playStart) / 1000
      case 'playback_levels': {
        if (playStart === null)
          return null
        const beat = ((performance.now() - playStart) / 500) % 1
        const level = 0.08 + 0.95 * Math.exp(-beat * 6)
        return { peak: [level, level * 0.92], rms: [level * 0.45, level * 0.4] }
      }
      default: return editorCommand(cmd, args) ?? batchCommand(cmd, args) ?? null
    }
  }, { shouldMockEvents: true })
}
