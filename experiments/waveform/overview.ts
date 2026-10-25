import type { Wave } from '../../src/stores/editor'

// Same [min, max, RMS, low, mid, high] layout that the Rust overview sends.
// This reduction is deliberately measured separately from Canvas drawing.
export function buildOverview(channels: readonly Float32Array[], from: number, to: number, pixelColumns: number): Wave {
  const span = to - from
  const count = span <= pixelColumns ? Math.max(1, Math.ceil(span)) : Math.max(1, Math.round(pixelColumns))
  const samples = span <= pixelColumns, stride = samples ? 1 : 6
  const values = new Float32Array(channels.length * count * stride)
  channels.forEach((data, lane) => {
    for (let x = 0; x < count; x++) {
      const a = Math.min(data.length - 1, Math.floor(from + x / count * span))
      const b = Math.min(data.length, Math.max(a + 1, Math.floor(from + (x + 1) / count * span)))
      const at = (lane * count + x) * stride
      if (samples) { values[at] = data[a]; continue }
      let min = Infinity, max = -Infinity, sum = 0
      for (let i = a; i < b; i++) { const v = data[i]; min = Math.min(min, v); max = Math.max(max, v); sum += v * v }
      values[at] = min; values[at + 1] = max; values[at + 2] = Math.sqrt(sum / (b - a))
    }
  })
  return { samples, lanes: channels.length, count, stride, values }
}
