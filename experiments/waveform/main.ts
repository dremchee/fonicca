import Waveform from 'gl-waveform'
import { canvasContext, drawWave } from '../../src/waveform'
import type { Wave } from '../../src/stores/editor'
import './style.css'
import { buildOverview } from './overview'
import fixtureUrl from '../../src-tauri/tests/data/ref_long.wav?url'

const el = <T extends HTMLElement>(id: string) => document.getElementById(id) as T
const canvas = el<HTMLCanvasElement>('canvas'), gpu = el<HTMLCanvasElement>('gl')
const zoom = el<HTMLInputElement>('zoom'), pan = el<HTMLInputElement>('pan')
const mode = el<HTMLSelectElement>('renderer'), length = el<HTMLSelectElement>('length')
const bench = el<HTMLButtonElement>('bench'), exportButton = el<HTMLButtonElement>('export')
const audio = el<HTMLAudioElement>('audio')
const query = new URLSearchParams(location.search)
mode.value = query.get('renderer') ?? 'both'
length.value = query.get('length') ?? '0'
let channels: Float32Array[] = [], rate = 48000, original: Float32Array[] = []
let glWaves: Waveform[] = [], summaries: Wave | null = null, sourceName = '', blobUrl = ''
let run = 0, width = 0, height = 160, context: AudioContext | null = null
let lastDpr = 0
let initMs = 0, result: unknown = null, selectedSample = 0
const dpr = () => window.devicePixelRatio || 1
const format = (n: number) => n.toFixed(3)
const frame = () => new Promise<number>(resolve => requestAnimationFrame(resolve))

function range(): [number, number] {
  const n = channels[0]?.length ?? 1
  const span = Math.min(n, Math.max(2, Math.round(n / 2 ** Number(zoom.value))))
  const start = Math.round(Number(pan.value) * Math.max(0, n - span))
  return [start, Math.min(n, start + span)]
}


function drawCanvas() {
  if (!summaries) return
  const ctx = canvasContext(canvas, width, height, dpr())
  drawWave(ctx, summaries, { top: 0, width, height, dpr: dpr() }, {
    style: 'peaksRms', scale: 'linear', colors: 'theme', zoom: 1, color: '#8b6dff', lineColor: '#292933',
  })
}
function drawGl(view: [number, number]) {
  // Canvas places discrete samples at each sample cell's center.
  const offset = view[1] - view[0] <= width * dpr() ? .5 : 0
  glWaves.forEach(wave => wave.update({ range: [view[0] - offset, view[1] - offset] }).clear().render())
}
function draw() {
  if (!channels.length) return
  const view = range()
  if (mode.value !== 'webgl') { summaries = buildOverview(channels, ...view, width * dpr()); drawCanvas() }
  if (mode.value !== 'canvas') drawGl(view)
  el('range').textContent = `${(view[0] / rate).toFixed(3)} — ${(view[1] / rate).toFixed(3)} с`
  document.querySelectorAll<HTMLElement>('.cursor').forEach(cursor => {
    cursor.style.left = `${100 * (selectedSample - view[0]) / (view[1] - view[0])}%`
    cursor.hidden = selectedSample < view[0] || selectedSample > view[1]
  })
}
function build() {
  run++
  result = null; exportButton.disabled = true
  el('summary').replaceChildren()
  el('results').textContent = 'Нажмите «Замерить 180 кадров».'
  glWaves.forEach(wave => wave.destroy()); glWaves = []
  el('canvas-card').hidden = mode.value === 'webgl'
  el('gl-card').hidden = mode.value === 'canvas'
  width = Math.round(el('canvas-card').hidden ? gpu.clientWidth : canvas.clientWidth)
  lastDpr = dpr()
  gpu.width = Math.round(width * dpr()); gpu.height = Math.round(height * dpr())
  const start = performance.now()
  if (mode.value !== 'canvas') {
    try {
      channels.forEach((data, i) => glWaves.push(new Waveform(gpu, {
        data, pixelRatio: dpr(), viewport: [0, i * height / channels.length, width, height / channels.length],
        amplitude: [-1 / .92, 1 / .92], color: '#8b6dff', rms: '#8b6dff', peaks: 'rgba(139,109,255,.4)', thickness: 2,
      })))
    }
    catch (error) {
      glWaves.forEach(wave => wave.destroy()); glWaves = []
      el('status').textContent = `WebGL2 недоступен: ${String(error)}`
      bench.disabled = true
      return
    }
  }
  initMs = performance.now() - start
  draw(); bench.disabled = false
  el('status').textContent = `${sourceName} · ${(channels[0].length / rate).toFixed(1)} с · ${rate} Гц · каналов: ${channels.length} · ${width} × ${height} CSS px · DPR ${dpr()}`
}
function resize() {
  const nextWidth = Math.round(mode.value === 'webgl' ? gpu.clientWidth : canvas.clientWidth)
  if (channels.length && (nextWidth !== width || dpr() !== lastDpr)) build()
}
function useLength() {
  const seconds = Number(length.value)
  channels = original.map(data => {
    if (!seconds) return data
    const repeated = new Float32Array(Math.ceil(seconds * rate))
    for (let i = 0; i < repeated.length; i += data.length) repeated.set(data.subarray(0, Math.min(data.length, repeated.length - i)), i)
    return repeated
  })
  zoom.value = '0'; pan.value = '0'; build()
}
async function load(bytes: ArrayBuffer, name: string) {
  run++; bench.disabled = true
  el('status').textContent = 'Декодирование файла…'
  if (blobUrl) URL.revokeObjectURL(blobUrl)
  blobUrl = URL.createObjectURL(new Blob([bytes])); audio.src = blobUrl
  context = new AudioContext()
  try {
    const decoded = await context.decodeAudioData(bytes.slice(0))
    rate = decoded.sampleRate
    original = Array.from({ length: decoded.numberOfChannels }, (_, i) => decoded.getChannelData(i))
    sourceName = name; selectedSample = 0; useLength()
  }
  finally { await context.close(); context = null }
}
function stats(values: number[]) {
  const sorted = [...values].sort((a, b) => a - b)
  return { medianMs: sorted[Math.floor(sorted.length / 2)], p95Ms: sorted[Math.floor(sorted.length * .95)], maxMs: sorted.at(-1) }
}
async function benchmark() {
  const token = ++run, n = channels[0].length
  const canvasTimes: number[] = [], prepareTimes: number[] = [], glTimes: number[] = [], intervals: number[] = []
  bench.disabled = true; exportButton.disabled = true
  let previous = await frame()
  const start = performance.now()
  for (let i = 0; i < 200; i++) {
    const now = await frame()
    if (token !== run) { bench.disabled = false; return }
    if (i >= 20) intervals.push(now - previous)
    previous = now
    const span = Math.min(n, Math.max(32, Math.round(n / 2 ** (1 + 8 * (.5 - .5 * Math.cos(i / 199 * Math.PI * 2))))))
    const from = Math.round((n - span) * (.5 + .45 * Math.sin(i * .065)))
    if (mode.value !== 'webgl') {
      const t = performance.now(); summaries = buildOverview(channels, from, from + span, width * dpr())
      if (i >= 20) prepareTimes.push(performance.now() - t)
      const drawStart = performance.now(); drawCanvas()
      if (i >= 20) canvasTimes.push(performance.now() - drawStart)
    }
    if (mode.value !== 'canvas') {
      const t = performance.now(); drawGl([from, from + span])
      if (i >= 20) glTimes.push(performance.now() - t)
    }
    el('status').textContent = `Измерение: ${Math.max(0, i - 19)} / 180 кадров`
  }
  const gl = glWaves[0]?.gl
  const debug = gl?.getExtension('WEBGL_debug_renderer_info')
  result = {
    graphicsRenderer: gl ? gl.getParameter(debug ? debug.UNMASKED_RENDERER_WEBGL : gl.RENDERER) : null,
    source: sourceName, durationSeconds: n / rate, repeated: Number(length.value) > 0,
    mode: mode.value, library: 'gl-waveform@5.3.0', viewport: { width, height, dpr: dpr() },
    userAgent: navigator.userAgent, warmupFrames: 20, measuredFrames: 180,
    canvasPreparation: prepareTimes.length ? stats(prepareTimes) : null,
    canvasDrawCpu: canvasTimes.length ? stats(canvasTimes) : null,
    webglDrawCpu: glTimes.length ? stats(glTimes) : null,
    frameIntervals: stats(intervals), framesOver25ms: intervals.filter(t => t > 25).length,
    elapsedMs: performance.now() - start, webglInitializationMs: initMs,
    pcmBytes: channels.reduce((s, c) => s + c.byteLength, 0), canvasOverviewBytes: summaries?.values.byteLength ?? 0,
    gpuMemoryBytes: null, note: 'CPU submission timings. GPU completion and total GPU memory are not measured. Canvas reduction is JS in this bench, Rust in production.',
  }
  const table = document.createElement('table')
  const header = table.insertRow()
  for (const title of ['Операция', 'Медиана, мс', 'p95, мс']) { const cell = document.createElement('th'); cell.textContent = title; header.append(cell) }
  for (const [label, times] of [['Canvas · рисование', canvasTimes], ['WebGL · вызов render', glTimes], ['Canvas · сводка в JS', prepareTimes]] as const) {
    if (!times.length) continue
    const data = stats([...times]), row = table.insertRow()
    for (const value of [label, data.medianMs.toFixed(2), data.p95Ms.toFixed(2)]) row.insertCell().textContent = value
  }
  el('summary').replaceChildren(table)
  el('results').textContent = JSON.stringify(result, (_, value) => typeof value === 'number' ? Number(format(value)) : value, 2)
  bench.disabled = false; exportButton.disabled = false; draw()
  el('status').textContent = 'Измерение завершено. 20 прогревочных кадров исключены.'
}

zoom.oninput = pan.oninput = () => { run++; draw(); bench.disabled = false }
mode.onchange = () => { const params = new URLSearchParams(location.search); params.set('renderer', mode.value); params.set('length', length.value); history.replaceState(null, '', `?${params}`); build() }
length.onchange = () => { const params = new URLSearchParams(location.search); params.set('length', length.value); history.replaceState(null, '', `?${params}`); useLength() }
bench.onclick = () => benchmark().catch(error => { el('status').textContent = String(error); bench.disabled = false })
exportButton.onclick = () => { const url = URL.createObjectURL(new Blob([JSON.stringify(result, null, 2)], { type: 'application/json' })); const a = document.createElement('a'); a.href = url; a.download = 'waveform-comparison.json'; a.click(); setTimeout(() => URL.revokeObjectURL(url), 1000) }
el('whole').onclick = () => { run++; zoom.value = '0'; pan.value = '0'; draw() }
el('second').onclick = () => { run++; zoom.value = String(Math.max(0, Math.log2((channels[0]?.length ?? 1) / rate))); pan.value = '.5'; draw() }
el('samples').onclick = () => { run++; zoom.value = String(Math.min(16, Math.max(0, Math.log2((channels[0]?.length ?? 1) / 128)))); pan.value = '.5'; draw() }
el<HTMLInputElement>('file').onchange = async event => { const file = (event.target as HTMLInputElement).files?.[0]; if (file) try { await load(await file.arrayBuffer(), file.name) } catch (error) { el('status').textContent = String(error) } }
el<HTMLButtonElement>('fixture').onclick = async () => { try { const response = await fetch(fixtureUrl); if (!response.ok) throw new Error(`HTTP ${response.status}`); await load(await response.arrayBuffer(), 'ref_long.wav · тестовый сигнал') } catch (error) { el('status').textContent = String(error) } }
document.querySelectorAll<HTMLElement>('.plot').forEach(plot => {
  let drag: { x: number, pan: number, moved: boolean } | null = null
  plot.onpointerdown = event => { if (event.button || !channels.length) return; drag = { x: event.clientX, pan: Number(pan.value), moved: false }; plot.setPointerCapture(event.pointerId) }
  plot.onpointermove = event => { if (!drag) return; const view = range(), available = channels[0].length - (view[1] - view[0]); const dx = event.clientX - drag.x; drag.moved ||= Math.abs(dx) > 3; if (available > 0) { pan.value = String(Math.min(1, Math.max(0, drag.pan - dx / width * (view[1] - view[0]) / available))); run++; draw() } }
  plot.onpointerup = event => { if (drag && !drag.moved) { const view = range(); selectedSample = view[0] + (event.clientX - plot.getBoundingClientRect().left) / width * (view[1] - view[0]); draw() }; drag = null }
  plot.onpointercancel = () => drag = null
  plot.onwheel = event => { event.preventDefault(); run++; zoom.value = String(Math.min(16, Math.max(0, Number(zoom.value) - Math.sign(event.deltaY) * .3))); draw() }
})
new ResizeObserver(resize).observe(el('canvas-card').parentElement!)
window.addEventListener('beforeunload', () => { run++; glWaves.forEach(wave => wave.destroy()); if (blobUrl) URL.revokeObjectURL(blobUrl); context?.close() })
el<HTMLButtonElement>('fixture').click()
