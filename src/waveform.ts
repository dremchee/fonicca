import type { WaveStyle } from '@/api'
import type { Wave } from '@/stores/editor'
import { barCornerRadius, waveBars } from './waveform-bars.ts'

/** Range shown by the dB scale, from silence to full scale. */
const DB_RANGE = 60

export interface WaveLook {
  density?: 'off' | 'gaussian' | 'laplace' | 'flat'
  peakOpacity?: number
  rmsOpacity?: number
  thickness?: number
  barWidth?: number
  barGap?: number
  barRadius?: number
  dbRange?: number
  channelColors?: [string | null, string | null]
  style: WaveStyle
  scale: 'linear' | 'db'
  colors: 'theme' | 'frequency'
  /** Vertical zoom; 1 = full scale. */
  zoom: number
  color: string
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
  // Values come as i16 fractions of full scale.
  const packed = new Int16Array(buf, 16)
  const values = new Float32Array(packed.length)
  for (let i = 0; i < packed.length; i++)
    values[i] = packed[i] / 32767
  return { samples: head[0] === 1, lanes: head[1], count: head[2], stride: head[3], values }
}

/**
 * A cleared 2D context scaled to CSS pixels. The backing store is resized only when
 * the size changes: reallocating it on every draw is the costly part of a redraw.
 */
export function canvasContext(el: HTMLCanvasElement, width: number, height: number, dpr: number) {
  const bw = Math.round(width * dpr)
  const bh = Math.round(height * dpr)
  if (el.width !== bw)
    el.width = bw
  if (el.height !== bh)
    el.height = bh
  const ctx = el.getContext('2d')!
  ctx.setTransform(1, 0, 0, 1, 0, 0)
  ctx.clearRect(0, 0, bw, bh)
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
  return ctx
}

/** Amplitude (−1…1) to lane height fraction, with vertical zoom and dB scale. */
export function waveLevel(v: number, look: Pick<WaveLook, 'scale' | 'zoom' | 'dbRange'>) {
  const z = Math.max(-1, Math.min(1, v * look.zoom))
  if (look.scale !== 'db')
    return z
  const range = look.dbRange ?? DB_RANGE
  const db = 20 * Math.log10(Math.max(Math.abs(z), 1e-9))
  return Math.sign(z) * Math.max(0, (db + range) / range)
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
    const laneColor = look.colors === 'theme' ? look.channelColors?.[lane] ?? look.color : look.color
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
      ctx.strokeStyle = laneColor
      // Thick enough to read as the wave itself, not a hairline over the lane.
      ctx.lineWidth = look.thickness ?? 2
      ctx.lineJoin = 'round'
      ctx.lineCap = 'round'
      ctx.beginPath()
      for (let i = 0; i < data.count; i++)
        i ? ctx.lineTo(px(i), y(at(i, 0))) : ctx.moveTo(px(i), y(at(i, 0)))
      ctx.stroke()
      if (step >= 6) {
        ctx.fillStyle = laneColor
        for (let i = 0; i < data.count; i++)
          ctx.fillRect(px(i) - 2, y(at(i, 0)) - 2, 4, 4)
      }
    }
    else {
      const colW = w / data.count
      const fill = (i: number) => (look.colors === 'frequency' ? bandColor(at(i, 3), at(i, 4), at(i, 5)) : laneColor)
      if (look.style === 'bars') {
        for (const bar of waveBars(data, lane, w, dpr, { width: look.barWidth, gap: look.barGap })) {
          const half = Math.max(1 / dpr, Math.abs(level(bar.peak)) * amp)
          ctx.fillStyle = fill(bar.index)
          const radius = barCornerRadius(bar.width, half * 2, look.barRadius ?? 0)
          if (radius > 0) {
            ctx.beginPath()
            ctx.roundRect(bar.x, mid - half, bar.width, half * 2, radius)
            ctx.fill()
          }
          else {
            ctx.fillRect(bar.x, mid - half, bar.width, half * 2)
          }
        }
      }
      else {
        // Peaks dimmed around a full-strength RMS body (or full strength alone). Zoomed
        // in, a column spans few frames: each one reaches over to its neighbour so the
        // wave stays one stroke, and gets a little thickness so it is not a hairline.
        ctx.globalAlpha = look.style === 'peaksRms' ? look.peakOpacity ?? 0.4 : 1
        const pad = 0.75
        // Narrow columns also widen a little, so steep slopes get the same thickness.
        const padX = colW < 2 ? 0.75 : 0
        let prevLo = at(0, 0)
        let prevHi = at(0, 1)
        for (let i = 0; i < data.count; i++) {
          let lo = at(i, 0)
          let hi = at(i, 1)
          if (hi < prevLo)
            hi = prevLo
          if (lo > prevHi)
            lo = prevHi
          prevLo = at(i, 0)
          prevHi = at(i, 1)
          const top = y(hi) - pad
          ctx.fillStyle = fill(i)
          ctx.fillRect(i * colW - padX, top, Math.max(1 / dpr, colW) + 2 * padX, y(lo) + pad - top)
        }
        ctx.globalAlpha = 1
        if (look.style === 'peaksRms') {
          ctx.globalAlpha = look.rmsOpacity ?? 1
          for (let i = 0; i < data.count; i++) {
            const r = at(i, 2)
            const top = y(r)
            ctx.fillStyle = fill(i)
            ctx.fillRect(i * colW, top, Math.max(1 / dpr, colW), Math.max(1 / dpr, y(-r) - top))
          }
        }
      }
    }
    ctx.globalAlpha = 1
    ctx.fillStyle = look.lineColor
    if (look.style !== 'bars' || data.samples)
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

/** Abstract crescendo for loading; independent of the file being decoded. */
export function risingWave(columns: number, split: boolean): Wave {
  const lanes = split ? 2 : 1
  const stride = 6
  const values = new Float32Array(lanes * columns * stride)
  for (let lane = 0; lane < lanes; lane++) {
    for (let c = 0; c < columns; c++) {
      const x = c / Math.max(1, columns - 1)
      const peak = (0.04 + 0.86 * x ** 1.25) * (0.94 + 0.06 * Math.sin(x * 70 + lane * 0.7))
      const i = (lane * columns + c) * stride
      values[i] = -peak
      values[i + 1] = peak
      values[i + 2] = peak * 0.6
      values[i + 3] = peak * 0.6
      values[i + 4] = peak * 0.3
      values[i + 5] = peak * 0.1
    }
  }
  return { samples: false, lanes, count: columns, stride, values }
}
