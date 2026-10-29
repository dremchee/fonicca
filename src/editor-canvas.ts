import type { Wave } from './stores/editor.ts'
import type { WaveArea, WaveLook } from './waveform.ts'
import type { Marker, Region } from './editor-regions.ts'
import type { Spectrogram } from './spectrogram.ts'
import type { GridMode } from './composables/editor/grid.ts'
import { MIN_LINE_PX } from './composables/editor/grid.ts'

export interface RegionBadge { start: number, end: number, x: number, w: number }
export interface EditorCanvasScene {
  viewport: { width: number, height: number, ratio: number, ruler: number, start: number, end: number, rate: number }
  grid: { mode: GridMode, beatGrid: boolean, beatFrames: number, downbeat: number,
    gridStep: (pixels: number) => number, gridFrames: (step: number) => number[],
    gridOrigin: () => number, isMultiple: (value: number, of: number) => boolean }
  wave: Wave | null
  waveRange: { start: number, end: number }
  look: Omit<WaveLook, 'zoom' | 'color' | 'lineColor'>
  zoom: number
  spectral: boolean
  spectrum: Spectrogram | null
  selection: { start: number, end: number } | null
  loop: { start: number, end: number } | null
  regions: Region[]
  markers: Marker[]
  slicePreview: Marker[]
  hoverBadge: number | null
  color: (name: string) => string
}
interface Painters {
  drawWave: (ctx: CanvasRenderingContext2D, data: Wave, area: WaveArea, look: WaveLook) => void
  drawSpectrogram: (ctx: CanvasRenderingContext2D, data: Spectrogram, area: { left: number, top: number, width: number, height: number }) => void
  drawFrequencyAxis: (ctx: CanvasRenderingContext2D, rate: number, width: number, top: number, height: number) => void
}
const BADGE_H = 14
function tickStep(span: number, rate: number, width: number) {
  const steps = [0.001, 0.005, 0.01, 0.05, 0.1, 0.25, 0.5, 1, 2, 5, 10, 15, 30, 60, 120, 300]
  const seconds = span / rate
  return steps.find(step => (step / seconds) * width >= 80) ?? 600
}
function formatTime(seconds: number, precise: boolean) {
  const m = Math.floor(seconds / 60), s = seconds - m * 60
  return `${m}:${precise ? s.toFixed(3).padStart(6, '0') : String(Math.floor(s)).padStart(2, '0')}`
}

/** Draw a snapshot of the editor scene; audio requests and Vue lifecycle live elsewhere. */
export function drawEditorCanvas(ctx: CanvasRenderingContext2D, scene: EditorCanvasScene, painters: Painters): RegionBadge[] {
  if (scene.viewport.width <= 0 || scene.viewport.height <= 0 || scene.viewport.end <= scene.viewport.start) return []
  return render(ctx, scene, painters)
}

function render(ctx: CanvasRenderingContext2D, scene: EditorCanvasScene, painters: Painters) {
  const { width: w, height: h, ratio: dpr, ruler: RULER, start, end, rate } = scene.viewport
  const pixel = (value: number) => Math.round(value * dpr) / dpr
  const x = (frame: number) => ((frame - start) / (end - start)) * w
  const color = scene.color
  const { drawWave, drawSpectrogram, drawFrequencyAxis } = painters
  const { gridStep, gridFrames, gridOrigin, isMultiple } = scene.grid
  if (scene.spectral) { ctx.fillStyle = '#080511'; ctx.fillRect(0, 0, w, h) }
  const step = tickStep(end - start, rate, w)
  const first = Math.ceil(start / rate / step) * step
  const ticks: number[] = []
  for (let t = first; t * rate <= end; t += step)
    ticks.push(t)

  // Waveform, one lane per channel (or one mixed lane), in the full brand colour.
  // The selection is the inverse: a brand-colour block with the wave in the canvas colour.
  const sel = scene.selection
  const selX = sel ? pixel(x(sel.start)) : 0
  const selW = sel ? Math.max(1 / dpr, pixel(x(sel.end)) - selX) : 0
  if (scene.spectral) {
    const data = scene.spectrum
    ctx.save()
    ctx.beginPath()
    ctx.rect(0, RULER, w, Math.max(0, h - RULER))
    ctx.clip()
    if (data)
      drawSpectrogram(ctx, data, { left: x(data.start), top: RULER, width: x(data.end) - x(data.start), height: h - RULER })
    if (sel) {
      ctx.fillStyle = 'rgba(255, 255, 255, 0.14)'
      ctx.fillRect(selX, RULER, selW, h - RULER)
      ctx.strokeStyle = '#eee8f5'
      ctx.strokeRect(selX + 0.5, RULER, selW - 1, h - RULER)
    }
    ctx.restore()
  }
  else if (scene.wave) {
    const look = { ...scene.look, zoom: scene.zoom, lineColor: color('--border'), color: color('--wave-strong') }
    // The wave covers its own range (see readWave), placed under the view.
    const data = scene.wave
    const left = pixel(x(scene.waveRange.start))
    const area = { top: pixel(RULER), width: pixel(x(scene.waveRange.end)) - left, height: h - pixel(RULER), dpr }
    const placed = (l: typeof look) => {
      ctx.save()
      ctx.translate(left, 0)
      drawWave(ctx, data, area, l)
      ctx.restore()
    }
    ctx.save()
    ctx.beginPath()
    ctx.rect(0, 0, w, h)
    ctx.clip()
    placed(look)
    ctx.restore()
    if (sel) {
      ctx.save()
      ctx.beginPath()
      ctx.rect(selX, RULER, selW, h - RULER)
      ctx.clip()
      ctx.fillStyle = color('--primary')
      ctx.fillRect(selX, RULER, selW, h - RULER)
      // Frequency colours stay as they are; the theme colour flips to the canvas colour.
      placed({ ...look, color: color('--card'), lineColor: color('--card'), channelColors: [null, null] })
      ctx.restore()
    }
  }
  else if (sel) {
    ctx.fillStyle = color('--primary-soft')
    ctx.fillRect(selX, RULER, selW, h - RULER)
  }

  // Grid over the wave, so it reads on the wave and the selection alike: bars
  // strongest, then beats (or ruler ticks), then subdivisions.
  if (scene.grid.mode !== 'off') {
    ctx.fillStyle = color('--foreground')
    for (const f of gridFrames(gridStep(MIN_LINE_PX))) {
      const rel = f - gridOrigin()
      const strength = scene.grid.beatGrid
        ? isMultiple(rel, scene.grid.beatFrames * 4) ? 0.3 : isMultiple(rel, scene.grid.beatFrames) ? 0.16 : 0.07
        : isMultiple(rel, step * rate) ? 0.16 : 0.07
      ctx.globalAlpha = strength
      ctx.fillRect(pixel(x(f)), pixel(RULER), pixel(1), h - pixel(RULER))
    }
    ctx.globalAlpha = 1
  }

  // Ruler, set off from the wave by a line: bar numbers ("3", "3.2") on the bar
  // grid, otherwise time.
  ctx.fillStyle = color('--muted-foreground')
  ctx.font = '10px system-ui, sans-serif'
  if (scene.grid.beatGrid) {
    const labelStep = gridStep(48)
    for (const f of gridFrames(labelStep)) {
      const beat = Math.round((f - scene.grid.downbeat) / scene.grid.beatFrames)
      const bar = Math.floor(beat / 4) + 1
      const inBar = ((beat % 4) + 4) % 4
      const px = pixel(x(f))
      ctx.fillRect(px, pixel(RULER - 5), 1 / dpr, pixel(5))
      ctx.fillText(inBar ? `${bar}.${inBar + 1}` : String(bar), pixel(px + 3), pixel(11))
    }
  }
  else {
    for (const t of ticks) {
      const px = pixel(x(t * rate))
      ctx.fillRect(px, pixel(RULER - 5), 1 / dpr, pixel(5))
      ctx.fillText(formatTime(t, step < 1), pixel(px + 3), pixel(11))
    }
  }
  ctx.fillStyle = color('--border')
  ctx.fillRect(0, pixel(RULER), w, 1 / dpr)

  // Loop points: a brand-colour band at the foot of the ruler, dashed edges through the wave.
  const lp = scene.loop
  if (lp) {
    const a = x(lp.start)
    const b = x(lp.end)
    ctx.fillStyle = color('--primary')
    ctx.fillRect(a, RULER - 4, Math.max(1, b - a), 4)
    ctx.strokeStyle = color('--foreground')
    ctx.globalAlpha = 0.7
    ctx.setLineDash([3, 3])
    ctx.lineWidth = 1
    ctx.beginPath()
    for (const px of [Math.round(a) + 0.5, Math.round(b) - 0.5]) {
      ctx.moveTo(px, RULER)
      ctx.lineTo(px, h)
    }
    ctx.stroke()
    ctx.setLineDash([])
    ctx.globalAlpha = 1
  }
  const badges = drawSlices(ctx, scene, x, color)
  if (scene.spectral)
    drawFrequencyAxis(ctx, rate, w, RULER, h - RULER)

  return badges
}


function drawSlices(ctx: CanvasRenderingContext2D, scene: EditorCanvasScene, x: (frame: number) => number, color: (name: string) => string) {
  const { width: w, height: h, ruler: RULER } = scene.viewport
  const regions = scene.regions
  const badges: RegionBadge[] = []
  ctx.fillStyle = color('--card')
  ctx.globalAlpha = 0.65
  for (const r of regions) {
    if (r.skip)
      ctx.fillRect(x(r.start), RULER, x(r.end) - x(r.start), h - RULER)
  }
  ctx.globalAlpha = 1

  ctx.font = '600 10px system-ui, sans-serif'
  let n = 0
  let lastLabel = -Infinity
  for (const r of regions) {
    if (!r.skip)
      n++
    const px = Math.round(x(r.start))
    if (px < -40 || px > w)
      continue
    ctx.fillStyle = color('--foreground')
    // The start of the file is no marker of its own.
    if (r.start > 0 || scene.markers[0]?.frame === 0) {
      ctx.fillRect(px, 0, 1, h)
      ctx.beginPath()
      ctx.moveTo(px, 0)
      ctx.lineTo(px + 6, 3)
      ctx.lineTo(px, 6)
      ctx.closePath()
      ctx.fill()
    }
    if (r.skip)
      continue
    const label = String(n)
    const lw = Math.max(BADGE_H, ctx.measureText(label).width + 6)
    if (px - lastLabel < lw + 4)
      continue
    lastLabel = px
    badges.push({ start: r.start, end: r.end, x: px, w: lw })
    ctx.fillRect(px, RULER + 1, lw, BADGE_H)
    ctx.fillStyle = color('--background')
    // Hovered: a play button.
    if (scene.hoverBadge === r.start) {
      const cx = px + lw / 2
      const cy = RULER + 1 + BADGE_H / 2
      ctx.beginPath()
      ctx.moveTo(cx - 2.5, cy - 4)
      ctx.lineTo(cx + 4, cy)
      ctx.lineTo(cx - 2.5, cy + 4)
      ctx.closePath()
      ctx.fill()
    }
    else {
      ctx.fillText(label, px + (lw - ctx.measureText(label).width) / 2, RULER + 12)
    }
  }

  if (scene.slicePreview.length) {
    ctx.strokeStyle = color('--foreground')
    ctx.globalAlpha = 0.6
    ctx.setLineDash([4, 3])
    ctx.beginPath()
    for (const m of scene.slicePreview) {
      const px = Math.round(x(m.frame)) + 0.5
      if (px >= 0 && px <= w) {
        ctx.moveTo(px, RULER)
        ctx.lineTo(px, h)
      }
    }
    ctx.stroke()
    ctx.setLineDash([])
    ctx.globalAlpha = 1
  }
  return badges
}
