import type { Wave } from './stores/editor'

/** Rendering coordinates only: a backend column covers this many virtual samples.
 * Repeated leaves retain its extrema and mean square; they are not audio PCM.
 * gl-waveform's summary-only path needs at least 1024 samples per device pixel.
 */
export function summarySpan(count: number, deviceWidth: number) {
  return 1024 * 2 ** Math.max(0, Math.ceil(Math.log2(deviceWidth / Math.max(1, count))))
}

export function glLane(data: Wave, lane: number, span: number, level: (v: number) => number) {
  const base = lane * data.count * data.stride
  if (data.samples) {
    return Float32Array.from(data.values.subarray(base, base + data.count), level)
  }
  const repeats = span / 256
  const leaves = new Float64Array(data.count * repeats * 4)
  for (let i = 0; i < data.count; i++) {
    const at = base + i * data.stride
    const lo = level(data.values[at])
    const hi = level(data.values[at + 1])
    const rms = level(data.values[at + 2])
    for (let j = 0; j < repeats; j++) {
      const k = (i * repeats + j) * 4
      leaves[k] = lo
      leaves[k + 1] = hi
      leaves[k + 2] = rms * rms * 256
      leaves[k + 3] = 256
    }
  }
  return leaves
}

/** Symmetric value range for a lane's device-pixel viewport. */
export function laneAmplitude(height: number, dpr: number, thickness: number, sampleSpacing = 0) {
  if (height <= 0)
    return 1 / 0.92
  const halfStroke = Math.max(thickness * dpr / 2, 0.5)
  // gl-waveform snaps the centre according to stroke parity. Odd-height lanes
  // can consequently have half a pixel less space on one side.
  const center = Math.round(2 * halfStroke) % 2 ? Math.floor(height / 2) + 0.5 : Math.round(height / 2)
  const t = Math.max(0, Math.min(1, (sampleSpacing - 6) / 6))
  const radius = halfStroke + 1.5 * dpr * t * t * (3 - 2 * t)
  // Include the antialias fringe and one CSS pixel of clear space on each side.
  const available = Math.min(center, height - center) - radius - 0.5 - dpr
  const halfHeight = Math.max(0.01, Math.min(height * 0.46, available))
  return height / (2 * halfHeight)
}
