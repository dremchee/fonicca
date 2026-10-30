import type { Curve, Span } from './stores/editor'

export interface EditorWindowView {
  cursor: number
  selection: Span | null
  looping: boolean
  curve: Curve
  viewport?: { start: number, end: number, zoom: number, bpm: number | null, downbeat: number }
}

/** Window URLs contain only UI coordinates; audio/history stay in native memory. */
export function readEditorWindowView(search: string, frames: number): EditorWindowView | null {
  const params = new URLSearchParams(search)
  if (params.get('detached') !== '1') return null
  try {
    const value = JSON.parse(params.get('view') ?? 'null')
    if (!value || !Number.isFinite(value.cursor)) return null
    const frame = (n: number) => Math.max(0, Math.min(frames, Math.round(n)))
    const sel = value.selection
    const selection = sel && Number.isFinite(sel.start) && Number.isFinite(sel.end) && frame(sel.end) > frame(sel.start)
      ? { start: frame(sel.start), end: frame(sel.end) } : null
    const result: EditorWindowView = { cursor: frame(value.cursor), selection, looping: value.looping === true,
      curve: ['linear', 'smooth', 'exponential'].includes(value.curve) ? value.curve : 'linear' }
    const v = value.viewport
    if (v && [v.start, v.end, v.zoom, v.downbeat].every(Number.isFinite) && frame(v.end) > frame(v.start)) {
      result.viewport = { start: frame(v.start), end: frame(v.end), zoom: Math.max(.1, Math.min(64, v.zoom)),
        bpm: Number.isFinite(v.bpm) && v.bpm >= 20 && v.bpm <= 999 ? v.bpm : null, downbeat: Math.round(v.downbeat) }
    }
    return result
  } catch { return null }
}
