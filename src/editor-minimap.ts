export type MinimapDrag = 'move' | 'start' | 'end'

/** Resize one viewport edge without crossing the other; the editor's minimum zoom is 64 frames. */
export function dragMinimapRange(start: number, end: number, mode: MinimapDrag, delta: number, frames: number): [number, number] {
  if (frames <= 0) return [0, 0]
  const minSpan = Math.min(64, frames)
  if (mode === 'start') return [Math.max(0, Math.min(end - minSpan, start + delta)), end]
  if (mode === 'end') return [start, Math.max(start + minSpan, Math.min(frames, end + delta))]
  const span = Math.min(frames, end - start)
  const next = Math.max(0, Math.min(frames - span, start + delta))
  return [next, next + span]
}
