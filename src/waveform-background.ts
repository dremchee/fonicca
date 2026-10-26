import type { Wave } from './stores/editor'

/** A compact mono silhouette; every column and channel contributes its peak. */
export function waveformBackground(data: Wave): string {
  if (!data.count || !data.lanes) return ''
  const count = Math.min(256, data.count)
  const peaks = Array.from({ length: count }, (_, bin) => {
    const start = Math.floor(bin * data.count / count)
    const end = Math.floor((bin + 1) * data.count / count)
    let peak = 0
    for (let lane = 0; lane < data.lanes; lane++) {
      for (let i = start; i < end; i++) {
        const offset = (lane * data.count + i) * data.stride
        const value = Math.max(Math.abs(data.values[offset]), data.samples ? 0 : Math.abs(data.values[offset + 1]))
        if (Number.isFinite(value)) peak = Math.max(peak, value)
      }
    }
    return Math.min(1, peak) * 46
  })
  const x = (i: number) => (i * 1000 / Math.max(1, count - 1)).toFixed(1)
  const top = peaks.map((peak, i) => `${x(i)},${(50 - peak).toFixed(1)}`)
  const bottom = peaks.map((peak, i) => `${x(i)},${(50 + peak).toFixed(1)}`).reverse()
  return `M${top.join(' L')} L${bottom.join(' L')} Z`
}
