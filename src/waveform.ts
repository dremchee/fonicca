import type { WaveStyle } from '@/api'
import type { Wave } from '@/stores/editor'

/** Range shown by the dB scale, from silence to full scale. */
const DB_RANGE = 60

export interface WaveLook {
  style: WaveStyle
  scale: 'linear' | 'db'
  colors: 'theme' | 'frequency'
  /** Vertical zoom; 1 = full scale. */
  zoom: number
  color: string
  /** Darkens the RMS body in "peaks + RMS" (usually the background colour). */
  shade: string
  /** Centre and lane separator lines. */
  lineColor: string
}

export interface WaveArea {
  top: number
  width: number
  height: number
  dpr: number
}

/** Unpacks wave data sent by the backend (`editor::wave` / `overview` layout). */
export function parseWave(buf: ArrayBuffer): Wave {
  const head = new Uint32Array(buf, 0, 4)
  return { samples: head[0] === 1, lanes: head[1], count: head[2], stride: head[3], values: new Float32Array(buf, 16) }
}

/** Amplitude (−1…1) to lane height fraction, with vertical zoom and dB scale. */
export function waveLevel(v: number, look: Pick<WaveLook, 'scale' | 'zoom'>) {
  const z = Math.max(-1, Math.min(1, v * look.zoom))
  if (look.scale !== 'db')
    return z
  const db = 20 * Math.log10(Math.abs(z) + 1e-9)
  return Math.sign(z) * Math.max(0, (db + DB_RANGE) / DB_RANGE)
}

/** Low/mid/high band energy as an RGB colour (red bass, green mids, blue highs). */
export function bandColor(low: number, mid: number, high: number) {
  const max = Math.max(low, mid, high, 1e-9)
  const c = (v: number) => Math.round(60 + 195 * (v / max))
  return `rgb(${c(low)},${c(mid)},${c(high)})`
}

/** Draws a waveform, one lane per channel, into `area` of a 2D context. */
export function drawWave(ctx: CanvasRenderingContext2D, data: Wave, area: WaveArea, look: WaveLook) {
  const { top: areaTop, width: w, dpr } = area
  const laneH = area.height / data.lanes
  for (let lane = 0; lane < data.lanes; lane++) {
    const mid = areaTop + laneH * lane + laneH / 2
    const amp = (laneH / 2) * 0.92
    const level = (v: number) => waveLevel(v, look)
    const y = (v: number) => mid - level(v) * amp
    const base = lane * data.count * data.stride
    const at = (i: number, field: number) => data.values[base + i * data.stride + field]

    if (data.samples) {
      // Zoomed in to single samples: a line through them, dots when there is room.
      const step = w / Math.max(1, data.count)
      const px = (i: number) => (i + 0.5) * step
      ctx.strokeStyle = look.color
      ctx.lineWidth = 1.25
      ctx.beginPath()
      for (let i = 0; i < data.count; i++)
        i ? ctx.lineTo(px(i), y(at(i, 0))) : ctx.moveTo(px(i), y(at(i, 0)))
      ctx.stroke()
      if (step >= 6) {
        ctx.fillStyle = look.color
        for (let i = 0; i < data.count; i++)
          ctx.fillRect(px(i) - 1.5, y(at(i, 0)) - 1.5, 3, 3)
      }
    }
    else {
      const colW = w / data.count
      const fill = (i: number) => (look.colors === 'frequency' ? bandColor(at(i, 3), at(i, 4), at(i, 5)) : look.color)
      if (look.style === 'bars') {
        // Bars 3 px wide with 1 px gaps, height from the loudest column under each.
        const per = Math.max(1, Math.round(4 / colW))
        for (let i = 0; i < data.count; i += per) {
          let peak = 0
          let pick = i
          for (let j = i; j < Math.min(i + per, data.count); j++) {
            const p = Math.max(Math.abs(at(j, 0)), Math.abs(at(j, 1)))
            if (p >= peak) {
              peak = p
              pick = j
            }
          }
          const half = Math.max(1, Math.abs(level(peak)) * amp)
          ctx.fillStyle = fill(pick)
          ctx.beginPath()
          ctx.roundRect(i * colW, mid - half, Math.max(1, per * colW - 1), half * 2, 1.5)
          ctx.fill()
        }
      }
      else {
        // Bright peaks, then the RMS body shaded darker inside them.
        for (let i = 0; i < data.count; i++) {
          const top = y(at(i, 1))
          ctx.fillStyle = fill(i)
          ctx.fillRect(i * colW, top, Math.max(1 / dpr, colW), Math.max(1 / dpr, y(at(i, 0)) - top))
        }
        if (look.style === 'peaksRms') {
          ctx.globalAlpha = 0.4
          ctx.fillStyle = look.shade
          for (let i = 0; i < data.count; i++) {
            const r = at(i, 2)
            const top = y(r)
            ctx.fillRect(i * colW, top, Math.max(1 / dpr, colW), Math.max(1 / dpr, y(-r) - top))
          }
          ctx.globalAlpha = 1
        }
      }
    }
    ctx.fillStyle = look.lineColor
    ctx.fillRect(0, mid, w, 1 / dpr)
    if (lane > 0)
      ctx.fillRect(0, areaTop + laneH * lane, w, 1)
  }
}

/**
 * A two-bar groove for previews: kicks (bass), hats (highs) and a pad (mids),
 * the right channel a little brighter than the left. Same layout as editor data.
 */
export function demoWave(columns: number, split: boolean): Wave {
  const lanes = split ? 2 : 1
  const stride = 6
  const values = new Float32Array(lanes * columns * stride)
  let seed = 7
  const noise = () => {
    seed = (seed * 16807) % 2147483647
    return seed / 2147483647
  }
  const seconds = 2
  for (let c = 0; c < columns; c++) {
    const t = (c / columns) * seconds
    const sinceKick = t % 0.5
    const sinceHat = (t + 0.25) % 0.25
    const kick = Math.exp(-sinceKick * 9) * (sinceKick < 0.004 ? sinceKick / 0.004 : 1)
    const hat = Math.exp(-sinceHat * 45)
    const pad = 0.12 + 0.05 * Math.sin(t * Math.PI)
    for (let lane = 0; lane < lanes; lane++) {
      const side = split ? (lane === 0 ? -1 : 1) : 0
      const k = kick * (0.85 - side * 0.05)
      const h = hat * (0.3 + side * 0.08)
      const jitter = 0.9 + noise() * 0.2
      const peak = Math.min(1, (k + h + pad) * jitter)
      const i = (lane * columns + c) * stride
      values[i] = -peak * (0.9 + noise() * 0.1)
      values[i + 1] = peak
      values[i + 2] = Math.min(peak, (k * 0.62 + h * 0.3 + pad * 0.7) * jitter)
      values[i + 3] = k
      values[i + 4] = pad
      values[i + 5] = h
    }
  }
  return { samples: false, lanes, count: columns, stride, values }
}
