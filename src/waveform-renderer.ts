import type { Wave } from './stores/editor'
import type { WaveArea, WaveLook } from './waveform'
import { drawWave as drawCanvasWave } from './waveform'
import { drawGlWave, releaseWaveRenderer } from './waveform-gl'

export { canvasContext, demoWave, parseWave } from './waveform'

/** WebGL geometry with Canvas overlays; fallback for unavailable/lost contexts. */
export function drawWave(ctx: CanvasRenderingContext2D, data: Wave, area: WaveArea, look: WaveLook) {
  // Discrete bars need constant heights and flat caps. The continuous WebGL
  // envelope would give them diagonal tops when cut with a stripe mask.
  if (look.style === 'bars' && !data.samples) {
    releaseWaveRenderer(ctx.canvas)
    drawCanvasWave(ctx, data, area, look)
    ctx.canvas.dataset.waveRenderer = 'canvas-bars'
    return
  }
  if (!drawGlWave(ctx, data, area, look))
    drawCanvasWave(ctx, data, area, look)
}
