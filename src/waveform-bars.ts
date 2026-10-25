import type { Wave } from './stores/editor'

/** Independent, pixel-aligned bars. Every source column contributes to one bar,
 * including columns whose time falls in a visual gap, so transients survive.
 */
export function waveBars(data: Wave, lane: number, width: number, dpr: number, options: { width?: number, gap?: number } = {}) {
  const pixels = Math.floor(width * dpr)
  if (pixels < 1 || !data.count)
    return []
  const barWidth = Math.min(pixels, Math.max(1, Math.round((options.width ?? 2) * dpr)))
  const gap = Math.max(0, Math.round((options.gap ?? 1) * dpr))
  const count = Math.max(1, Math.floor((pixels + gap) / (barWidth + gap)))
  const inset = Math.floor((pixels - (count * barWidth + (count - 1) * gap)) / 2)
  const base = lane * data.count * data.stride
  return Array.from({ length: count }, (_, bar) => {
    const start = Math.floor(bar * data.count / count)
    const end = Math.min(data.count, Math.max(start + 1, Math.floor((bar + 1) * data.count / count)))
    let peak = 0
    let index = start
    for (let i = start; i < end; i++) {
      const at = base + i * data.stride
      const value = Math.max(Math.abs(data.values[at]), Math.abs(data.values[at + 1]))
      if (value >= peak) {
        peak = value
        index = i
      }
    }
    return { x: (inset + bar * (barWidth + gap)) / dpr, width: barWidth / dpr, peak, index }
  })
}


/** Roundness 0…1; even very short bars keep their complete caps. */
export function barCornerRadius(width: number, height: number, roundness: number) {
  return Math.min(width, height) / 2 * Math.max(0, Math.min(1, roundness))
}
