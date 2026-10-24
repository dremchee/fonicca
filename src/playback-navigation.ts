/** Keyboard navigation uses smaller increments for short one-shots. */
export function playbackSeekTarget(key: string, position: number, duration: number, shift = false): number | null {
  if (!Number.isFinite(duration) || duration <= 0 || !Number.isFinite(position))
    return null
  const step = duration < 10 ? (shift ? 0.1 : 0.01) : (shift ? 5 : 1)
  let target: number
  switch (key) {
    case 'ArrowLeft': case 'ArrowDown': target = position - step; break
    case 'ArrowRight': case 'ArrowUp': target = position + step; break
    case 'PageDown': target = position - duration / 10; break
    case 'PageUp': target = position + duration / 10; break
    case 'Home': target = 0; break
    case 'End': target = duration; break
    default: return null
  }
  // Round only arithmetic noise; don't move the end of a fractional-length sample.
  return Math.min(duration, Math.max(0, Math.round(target * 1000000) / 1000000))
}
