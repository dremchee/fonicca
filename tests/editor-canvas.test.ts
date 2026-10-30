import assert from 'node:assert/strict'
import { test } from 'node:test'
import { drawEditorCanvas, type EditorCanvasScene } from '../src/editor-canvas.ts'

function setup() {
  const calls: { name: string, args: unknown[] }[] = []
  const ctx = new Proxy({ measureText: (text: string) => ({ width: text.length * 6 }) }, {
    get: (target, name) => name in target ? target[name as keyof typeof target] : (...args: unknown[]) => calls.push({ name: String(name), args }),
    set: () => true,
  }) as unknown as CanvasRenderingContext2D
  const painted: { kind: string, color?: string }[] = []
  const painters = { drawWave: (_ctx: unknown, _data: unknown, _area: unknown, look: { color?: string }) => { painted.push({ kind: 'wave', color: look.color }) },
    drawSpectrogram: () => { painted.push({ kind: 'spectrum' }) }, drawFrequencyAxis: () => { painted.push({ kind: 'frequency-axis' }) } }
  const scene: EditorCanvasScene = {
    viewport: { width: 400, height: 200, ratio: 2, ruler: 18, start: 0, end: 4000, rate: 1000 },
    grid: { mode: 'off', beatGrid: false, beatFrames: 1000, downbeat: 0,
      gridStep: () => 1000, gridFrames: () => [0, 1000, 2000, 3000, 4000], gridOrigin: () => 0,
      isMultiple: (value, of) => value % of === 0 },
    wave: { samples: false, lanes: 1, count: 400, stride: 6, values: new Float32Array(2400) },
    waveRange: { start: 0, end: 4000 }, look: { style: 'peaks', scale: 'linear', colors: 'theme' }, zoom: 1, spectral: false, spectrum: null,
    selection: { start: 1000, end: 2000 }, loop: null, regions: [], markers: [], slicePreview: [], hoverBadge: null,
    color: name => name,
  }
  return { scene, ctx, painters, calls, painted }
}

test('wave painting keeps selection inversion and draws the requested range', () => {
  const v = setup()
  assert.deepEqual(drawEditorCanvas(v.ctx, v.scene, v.painters), [])
  assert.deepEqual(v.painted, [{ kind: 'wave', color: '--wave-strong' }, { kind: 'wave', color: '--card' }])
  assert.ok(v.calls.some(c => c.name === 'rect' && c.args[0] === 100 && c.args[2] === 100))
  assert.equal(v.calls.filter(c => c.name === 'save').length, v.calls.filter(c => c.name === 'restore').length)
})

test('explicit markers own numbered badges, skipped regions have no playable badge', () => {
  const v = setup()
  v.scene.markers = [{ frame: 1000, skip: false }, { frame: 3000, skip: true }]
  v.scene.regions = [{ start: 1000, end: 3000, skip: false }, { start: 3000, end: 4000, skip: true }]
  const badges = drawEditorCanvas(v.ctx, v.scene, v.painters)
  assert.deepEqual(badges, [{ start: 1000, end: 3000, x: 104, w: 14 }])
  assert.ok(v.calls.some(c => c.name === 'fillText' && c.args[0] === '1'))
  v.scene.hoverBadge = 1000
  v.calls.length = 0
  drawEditorCanvas(v.ctx, v.scene, v.painters)
  assert.ok(!v.calls.some(c => c.name === 'fillText' && c.args[0] === '1'))
})

test('spectrogram uses its own painter/axis and an empty viewport paints nothing', () => {
  const v = setup()
  v.scene.spectral = true
  v.scene.spectrum = { start: 0, end: 4000, columns: 1, bins: 1, sampleRate: 1000, fftSize: 256, hop: 64, values: new Uint8Array(1) }
  drawEditorCanvas(v.ctx, v.scene, v.painters)
  assert.deepEqual(v.painted.map(p => p.kind), ['spectrum', 'frequency-axis'])
  v.calls.length = 0
  v.scene.viewport.end = 0
  assert.deepEqual(drawEditorCanvas(v.ctx, v.scene, v.painters), [])
  assert.deepEqual(v.calls, [])
})

test('saved loop uses a translucent range, solid corner flags and a label without inverting the wave', () => {
  const v = setup()
  v.scene.loop = { ...v.scene.selection! }
  drawEditorCanvas(v.ctx, v.scene, v.painters)
  assert.equal(v.painted.length, 1)
  assert.ok(v.calls.some(c => c.name === 'fillText' && c.args[0] === 'Loop'))
  assert.ok(v.calls.some(c => c.name === 'lineTo' && c.args[0] === 107 && c.args[1] === 0))
  assert.ok(v.calls.some(c => c.name === 'lineTo' && c.args[0] === 193 && c.args[1] === 0))
  assert.ok(v.calls.some(c => c.name === 'lineTo' && c.args[0] === 100 && c.args[1] === 7))
  assert.ok(!v.calls.some(c => c.name === 'setLineDash' && JSON.stringify(c.args[0]) === '[3,3]'))
  v.scene.selection = { start: 2200, end: 2600 }
  v.painted.length = 0
  drawEditorCanvas(v.ctx, v.scene, v.painters)
  assert.equal(v.painted.length, 2)
})

test('slice numbers remain clickable beside a loop label at the same boundary', () => {
  const v = setup()
  v.scene.loop = { start: 1000, end: 3000 }
  v.scene.regions = [{ start: 1000, end: 3000, skip: false }]
  const badges = drawEditorCanvas(v.ctx, v.scene, v.painters)
  assert.equal(badges[0].x, 144)
})

test('playing a saved loop inverts its waveform like a selected marker region', () => {
  const v = setup()
  v.scene.loop = { ...v.scene.selection! }
  v.scene.loopPlaying = true
  drawEditorCanvas(v.ctx, v.scene, v.painters)
  assert.deepEqual(v.painted, [{ kind: 'wave', color: '--wave-strong' }, { kind: 'wave', color: '--card' }])
  v.scene.loopPlaying = false
  v.painted.length = 0
  drawEditorCanvas(v.ctx, v.scene, v.painters)
  assert.equal(v.painted.length, 1)
})
