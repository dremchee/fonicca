import type { Ref } from 'vue'
import type { Wave } from './stores/editor'
import type { WaveArea, WaveLook } from './waveform'
import Waveform from 'gl-waveform'
import { onScopeDispose, watch } from 'vue'
import { bandColor, waveLevel } from './waveform'
import { glLane, laneAmplitude, summarySpan } from './waveform-gl-data'

/** GPU geometry, composited into the existing timeline so clips, grids and
 * selection overlays keep their coordinates. No complete audio buffer is needed.
 */
class GlWaveRenderer {
  readonly surface = document.createElement('canvas')
  readonly tinted = document.createElement('canvas')
  readonly gl = this.surface.getContext('webgl2', {
    premultipliedAlpha: true, preserveDrawingBuffer: true,
    antialias: false, depth: false, stencil: false,
  })

  private lanes: Waveform[] = []
  private data?: Wave
  private key = ''

  constructor() {
    if (!this.gl)
      throw new Error('WebGL2 unavailable')
  }

  draw(ctx: CanvasRenderingContext2D, data: Wave, area: WaveArea, look: WaveLook) {
    if (this.gl!.isContextLost())
      throw new Error('Waveform context lost')
    const { width, height, dpr, top } = area
    const bw = Math.max(1, Math.round(width * dpr))
    const bh = Math.max(1, Math.round(height * dpr))
    if (this.surface.width !== bw)
      this.surface.width = bw
    if (this.surface.height !== bh)
      this.surface.height = bh
    const span = summarySpan(data.count, bw)
    const key = `${look.scale}:${look.dbRange}:${look.zoom}:${span}`
    if (data !== this.data || key !== this.key) {
      for (const lane of this.lanes)
        lane.destroy()
      this.lanes = []
      for (let i = 0; i < data.lanes; i++) {
        const lane = new Waveform(this.gl!)
        this.lanes.push(lane)
        const values = glLane(data, i, span, v => waveLevel(v, look))
        if (data.samples)
          lane.update({ data: values })
        else
          lane.peaks(values)
      }
      this.data = data
      this.key = key
    }
    this.gl!.disable(this.gl!.SCISSOR_TEST)
    this.gl!.clearColor(0, 0, 0, 0)
    this.gl!.clear(this.gl!.COLOR_BUFFER_BIT)
    const laneH = height / data.lanes
    for (let i = 0; i < this.lanes.length; i++) {
      const deviceLaneHeight = Math.round((i + 1) * laneH * dpr) - Math.round(i * laneH * dpr)
      const amplitude = laneAmplitude(deviceLaneHeight, dpr, look.thickness ?? 2, data.samples ? bw / (dpr * data.count) : 0)
      const frequency = look.colors === 'frequency' && !data.samples
      const color = frequency ? '#fff' : look.channelColors?.[i] ?? look.color
      const opacity = (value: number) => `color-mix(in srgb, ${color} ${value * 100}%, transparent)`
      const density = !data.samples && look.style !== 'bars' && look.density !== 'off' ? look.density ?? false : false
      this.lanes[i].update({
        pixelRatio: dpr,
        viewport: [0, i * laneH, width, laneH],
        range: data.samples ? [-0.5, data.count - 0.5] : [0, data.count * span],
        amplitude: [-amplitude, amplitude],
        color: density ? opacity(look.rmsOpacity ?? 1) : color,
        rms: !data.samples && look.style === 'peaksRms' ? opacity(look.rmsOpacity ?? 1) : false,
        density,
        peaks: (density || (look.style === 'peaksRms' && !data.samples))
          ? opacity(look.peakOpacity ?? 0.4) : null,
        thickness: look.thickness ?? 2,
      }).render()
    }
    let image = this.surface
    if (!data.samples && look.colors === 'frequency') {
      if (this.tinted.width !== bw)
        this.tinted.width = bw
      if (this.tinted.height !== bh)
        this.tinted.height = bh
      const tint = this.tinted.getContext('2d')!
      tint.setTransform(1, 0, 0, 1, 0, 0)
      tint.clearRect(0, 0, bw, bh)
      tint.globalCompositeOperation = 'source-over'
      tint.scale(dpr, dpr)
      for (let lane = 0; lane < data.lanes; lane++) {
        if (look.colors === 'frequency') {
          const colW = width / data.count
          for (let i = 0; i < data.count; i++) {
            const at = (lane * data.count + i) * data.stride
            tint.fillStyle = bandColor(data.values[at + 3], data.values[at + 4], data.values[at + 5])
            tint.fillRect(i * colW, lane * laneH, colW + 0.5 / dpr, laneH)
          }
        }
        else {
          tint.fillStyle = look.channelColors?.[lane] ?? look.color
          tint.fillRect(0, lane * laneH, width, laneH)
        }
      }
      tint.globalCompositeOperation = 'destination-in'
      tint.drawImage(this.surface, 0, 0, width, height)
      image = this.tinted
    }
    ctx.drawImage(image, 0, top, width, height)
    ctx.fillStyle = look.lineColor
    for (let lane = 0; lane < data.lanes; lane++) {
      ctx.fillRect(0, top + (lane + 0.5) * laneH, width, 1 / dpr)
      if (lane)
        ctx.fillRect(0, top + lane * laneH, width, 1)
    }
  }

  destroy() {
    for (const lane of this.lanes)
      lane.destroy()
    this.gl?.getExtension('WEBGL_lose_context')?.loseContext()
    this.surface.width = this.surface.height = 0
    this.tinted.width = this.tinted.height = 0
  }
}

const renderers = new WeakMap<HTMLCanvasElement, GlWaveRenderer | null>()

/** false asks the caller to use the Canvas fallback. */
export function drawGlWave(ctx: CanvasRenderingContext2D, data: Wave, area: WaveArea, look: WaveLook) {
  if (data.count === 0 || area.width <= 0 || area.height <= 0)
    return true
  const canvas = ctx.canvas
  let renderer = renderers.get(canvas)
  try {
    if (renderer === undefined) {
      renderer = new GlWaveRenderer()
      renderers.set(canvas, renderer)
    }
    if (!renderer)
      return false
    renderer.draw(ctx, data, area, look)
    canvas.dataset.waveRenderer = 'webgl'
    return true
  }
  catch (error) {
    renderer?.destroy()
    renderers.set(canvas, null)
    canvas.dataset.waveRenderer = 'canvas'
    console.warn('Waveform: using Canvas fallback', error)
    return false
  }
}

export function releaseWaveRenderer(canvas: HTMLCanvasElement | null) {
  if (canvas) {
    renderers.get(canvas)?.destroy()
    renderers.delete(canvas)
  }
}

/** Release GPU resources on conditional canvas replacement and component removal. */
export function useWaveRenderer(...canvases: Ref<HTMLCanvasElement | null>[]) {
  for (const canvas of canvases)
    watch(canvas, (_, previous) => releaseWaveRenderer(previous), { flush: 'sync' })
  onScopeDispose(() => canvases.forEach(canvas => releaseWaveRenderer(canvas.value)))
}
