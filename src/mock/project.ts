import { emit } from '@tauri-apps/api/event'
import type { ProjectDocument } from '@/project'
import { copyProject, projectDuration } from '@/project'
const samples = [
  { id: 1, path: '/mock/Kick.wav', name: 'Kick', duration: 0.5 },
  { id: 2, path: '/mock/Drum loop.wav', name: 'Drum loop', duration: 2 },
  { id: 3, path: '/mock/Pad.wav', name: 'Pad', duration: 4 },
].map(s => ({ ...s, sourceId: 1, ext: 'wav', size: 48000, sampleRate: 48000, channels: 2, bitDepth: 24, favorite: false, tags: [], bpm: 120, key: null, kind: 'loop', category: 'drum', peakDb: -6, lufs: -18 }))
let current: ProjectDocument | null = null
let began = 0
let offset = 0
let active = false
const files = new Map<string, ProjectDocument>()
window.addEventListener('message', event => {
  if (event.origin === location.origin && event.data === 'project:samples-ready') void emit('project:samples-ready')
})
export function projectCommand(cmd: string, args: any): unknown {
  switch (cmd) {
    case 'open_project_window': {
      const pending = JSON.parse(localStorage.getItem('mock.project.requests') ?? '[]')
      if (args.ids.length) pending.push(args.ids)
      localStorage.setItem('mock.project.requests', JSON.stringify(pending))
      const popup = window.open('', 'fonicca-project')
      if (popup?.location.href === 'about:blank') popup.location.href = '/mock.html#project'
      popup?.focus()
      popup?.postMessage('project:samples-ready', location.origin)
      return null
    }
    case 'project_take_samples': {
      const pending = JSON.parse(localStorage.getItem('mock.project.requests') ?? '[]')
      localStorage.removeItem('mock.project.requests')
      return pending
    }
    case 'project_sources': return args.paths.map((path: string) => ({ path, name: path.split('/').pop(), duration: 2 }))
    case 'project_levels': return active ? {peak:[0.4,0.3],rms:[0.2,0.15]} : null

    case 'project_samples': return args.ids.map((id: number) => samples.find(s => s.id === id)).filter(Boolean)
    case 'project_wave': return { duration: args.path.includes('Kick') ? 0.5 : args.path.includes('Pad') ? 4 : 2, peaks: Array.from({ length: 512 }, (_, i) => 0.2 + 0.5 * Math.abs(Math.sin(i * 0.13)) * Math.exp(-(i % 64) / 30)) }
    case 'project_play': current = args.project; offset = args.start; began = performance.now(); active = true; return null
    case 'project_stop': active = false; return null
    case 'project_position': {
      const pos = offset + (performance.now() - began) / 1000
      return active && current && pos < projectDuration(current) ? pos : null
    }
    case 'project_save': files.set(args.path, copyProject(args.project)); return null
    case 'project_load': return files.get(args.path) ?? null
    case 'project_export': return { peakDb: -6, seconds: projectDuration(args.project) }
  }
  if (location.hash === '#project' || location.hash === '#library') {
    if (cmd === 'list_samples') return samples.filter(s => s.name.toLowerCase().includes((args.filter?.query ?? '').toLowerCase()))
    if (cmd === 'count_samples') return samples.length
    if (cmd === 'sample_ids') return samples.map(s => s.id)
    if (cmd === 'plugin:dialog|save') return args.options?.defaultPath?.endsWith('.wav') ? '/mock/mix.wav' : '/mock/test.fonicca.json'
    if (cmd === 'plugin:dialog|open') return '/mock/test.fonicca.json'
  }
  return undefined
}
