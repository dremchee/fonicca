/** Project time is in seconds; changing BPM changes the grid, never source speed. */
export interface ProjectClip {
  id: string
  path: string
  name: string
  sourceDuration: number
  start: number
  offset: number
  duration: number
  repeats: number
  fadeIn: number
  fadeOut: number
}
export interface ProjectTrack {
  id: string
  name: string
  gainDb: number
  pan: number
  mute: boolean
  solo: boolean
  clips: ProjectClip[]
}
export interface ProjectDocument {
  version: 1
  name: string
  bpm: number
  masterDb: number
  tracks: ProjectTrack[]
}
export const PROJECT_LIMIT = 600
export const SAMPLE_DRAG_TYPE = 'application/x-fonicca-samples'
export const emptyProject = (name: string): ProjectDocument => ({ version: 1, name, bpm: 120, masterDb: -6, tracks: [] })
export const clipEnd = (c: ProjectClip) => c.start + c.duration * c.repeats
export const projectDuration = (p: ProjectDocument) => Math.max(0, ...p.tracks.flatMap(t => t.clips.map(clipEnd)))
export function snapProjectTime(time: number, bpm: number, snap: boolean) {
  const step = 60 / bpm / 4
  return Math.max(0, Math.min(PROJECT_LIMIT, snap ? Math.round(time / step) * step : time))
}
export function copyProject(p: ProjectDocument): ProjectDocument { return JSON.parse(JSON.stringify(p)) }
export function parseProject(value: unknown): ProjectDocument | null {
  const range = (n: unknown, min: number, max: number): n is number => typeof n === 'number' && Number.isFinite(n) && n >= min && n <= max
  const text = (s: unknown): s is string => typeof s === 'string' && s.length <= 4096
  if (!value || typeof value !== 'object') return null
  const p = value as ProjectDocument
  if (p.version !== 1 || !text(p.name) || !range(p.bpm, 30, 300) || !range(p.masterDb, -60, 0) || !Array.isArray(p.tracks) || p.tracks.length > 32) return null
  const ids = new Set<string>()
  const id = (s: unknown) => text(s) && !!s && !ids.has(s) && !!ids.add(s)
  let count = 0
  for (const t of p.tracks) {
    if (!t || !id(t.id) || !text(t.name) || !range(t.gainDb, -60, 12) || !range(t.pan, -1, 1) || typeof t.mute !== 'boolean' || typeof t.solo !== 'boolean' || !Array.isArray(t.clips)) return null
    for (const c of t.clips) {
      if (!c || ++count > 256 || !id(c.id) || !text(c.path) || !c.path || !text(c.name)
        || !range(c.sourceDuration, 1 / 48000, PROJECT_LIMIT) || !range(c.start, 0, PROJECT_LIMIT)
        || !range(c.offset, 0, c.sourceDuration) || !range(c.duration, 1 / 48000, c.sourceDuration - c.offset + 1e-8)
        || !Number.isInteger(c.repeats) || !range(c.repeats, 1, 64) || clipEnd(c) > PROJECT_LIMIT
        || !range(c.fadeIn, 0, c.duration) || !range(c.fadeOut, 0, c.duration)) return null
    }
  }
  return copyProject(p)
}
export function moveProjectClip(p: ProjectDocument, id: string, trackId: string, start: number): ProjectDocument {
  const next = copyProject(p)
  const target = next.tracks.find(t => t.id === trackId)
  const from = next.tracks.find(t => t.clips.some(c => c.id === id))
  const clip = from?.clips.find(c => c.id === id)
  if (!target || !from || !clip) return next
  from.clips = from.clips.filter(c => c.id !== id)
  clip.start = Math.max(0, Math.min(PROJECT_LIMIT - clip.duration * clip.repeats, start))
  target.clips.push(clip)
  return next
}
