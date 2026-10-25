// A fake editor session for /mock.html#editor: four bars of a 120 BPM loop
// (kick on every beat, hats in between), so the bar grid has something to line up with.
import type { AudioProcessing, ClipInfo, EditorState, Marker, SliceBy, Span } from '@/stores/editor'

const RATE = 44100
const BPM = 120
const FRAMES = Math.round((60 / BPM) * 16 * RATE)
const BEAT = (60 / BPM) * RATE

// Envelope of the loop at a frame, 0…1.
function level(frame: number) {
  const inBeat = frame % BEAT
  const kick = Math.exp(-inBeat / (RATE * 0.08))
  const hat = 0.35 * Math.exp(-Math.abs(inBeat - BEAT / 2) / (RATE * 0.02))
  return Math.min(1, 0.06 + kick * 0.9 + hat)
}

let loop: EditorState['loop'] = null
let clip: ClipInfo | null = null
let markers: Marker[] = []
let currentFrames = FRAMES
let processed = false
const capture = () => ({ frames: currentFrames, loop: loop && { ...loop }, markers: markers.map(m => ({ ...m })), processed })
type Snapshot = ReturnType<typeof capture>
const history: Snapshot[] = []
const future: Snapshot[] = []
function restore(s: Snapshot) {
  currentFrames = s.frames
  loop = s.loop
  markers = s.markers
  processed = s.processed
}
function remember() {
  history.push(capture())
  future.length = 0
}

function state(): EditorState {
  return { sampleId: 1, frames: currentFrames, sampleRate: RATE, channels: 2, canUndo: history.length > 0, canRedo: future.length > 0, dirty: processed || loop !== null || markers.length > 0, loop, markers }
}

// Same packing as the backend: u32 mode, lanes, count, stride, then lane-major columns
// of i16 fractions of full scale.
function wave(start: number, end: number, columns: number, split: boolean) {
  const lanes = split ? 2 : 1
  const count = Math.max(0, Math.floor(end) - Math.floor(start))
  if (count <= columns) {
    const buf = new ArrayBuffer(16 + lanes * count * 2)
    new Uint32Array(buf, 0, 4).set([1, lanes, count, 1])
    const values = new Int16Array(buf, 16)
    for (let lane = 0; lane < lanes; lane++) {
      for (let i = 0; i < count; i++) {
        const frame = Math.floor(start) + i
        values[lane * count + i] = Math.round(level(frame) * Math.sin(frame * 2 * Math.PI * (110 + lane * 5) / RATE) * 32767)
      }
    }
    return buf
  }
  const stride = 6
  const buf = new ArrayBuffer(16 + lanes * columns * stride * 2)
  new Uint32Array(buf, 0, 4).set([0, lanes, columns, stride])
  const values = new Int16Array(buf, 16)
  for (let lane = 0; lane < lanes; lane++) {
    for (let c = 0; c < columns; c++) {
      const a = start + ((end - start) * c) / columns
      const b = start + ((end - start) * (c + 1)) / columns
      let peak = 0
      for (let s = 0; s < 4; s++)
        peak = Math.max(peak, level(a + ((b - a) * s) / 4))
      const i = (lane * columns + c) * stride
      values.set([-peak, peak, peak * 0.6, peak * 0.8, 0.3, 0.2].map(v => Math.round(v * 32767)), i)
    }
  }
  return buf
}

// Slices of the fake loop: grid lines, every kick and hat (hits), or each kick's
// decay (silence), within [start, end).
function slices(start: number, end: number, by: SliceBy): Span[] {
  const cuts: number[] = []
  const every = (step: number, origin = 0) => {
    for (let f = origin + Math.ceil((start - origin) / step) * step; f < end; f += step)
      cuts.push(Math.round(f))
  }
  if (by.mode === 'silence') {
    const out: Span[] = []
    for (let f = Math.ceil(start / BEAT) * BEAT; f < end; f += BEAT)
      out.push({ start: Math.round(f), end: Math.min(end, Math.round(f + RATE * 0.3)) })
    return out
  }
  if (by.mode === 'grid')
    every(by.step, by.origin)
  else
    every(by.sensitivity > 0.6 ? BEAT / 2 : BEAT)
  const points = [start, ...cuts.filter(c => c > start), end]
  return points.slice(1).map((e, i) => ({ start: points[i], end: e }))
}

/** Editor commands for the mock backend; `undefined` for anything else. */
export function editorCommand(cmd: string, args: any): unknown {
  switch (cmd) {
    case 'editor_open': return { state: state(), name: 'Drum Loop 120', ext: 'wav', path: '/Users/me/Projects/Sound/Loops/Drum Loop 120.wav', bpm: BPM }
    case 'sample_wave': return wave(0, FRAMES, args.columns, args.split)
    case 'editor_wave': return wave(args.start, args.end, args.columns, args.split)
    case 'editor_apply':
      remember()
      if (args.op.op === 'setLoop')
        loop = { start: args.op.start, end: args.op.end }
      else if (args.op.op === 'clearLoop')
        loop = null
      // Sorted, one per frame (the last wins), inside the file: as the backend does.
      else if (args.op.op === 'setMarkers')
        markers = [...new Map((args.op.markers as Marker[]).filter(m => m.frame < currentFrames).map(m => [m.frame, m])).values()].sort((a, b) => a.frame - b.frame)
      return state()
    case 'editor_undo': {
      const previous = history.pop()
      if (previous) {
        future.push(capture())
        restore(previous)
      }
      return state()
    }
    case 'editor_redo': {
      const next = future.pop()
      if (next) {
        history.push(capture())
        restore(next)
      }
      return state()
    }
    case 'editor_process': {
      // The mock changes metadata only; actual DSP is exercised by Rust tests.
      const processing = args.processing as AudioProcessing
      remember()
      processed = true
      let end = args.end
      if (processing.mode === 'stretch') {
        const inserted = Math.round((args.end - args.start) * processing.ratio)
        const map = (p: number) => p <= args.start ? p : p >= args.end
          ? p + inserted - (args.end - args.start)
          : args.start + Math.round((p - args.start) * processing.ratio)
        loop = loop && { start: map(loop.start), end: map(loop.end) }
        markers = markers.map(m => ({ ...m, frame: map(m.frame) }))
        currentFrames += inserted - (args.end - args.start)
        end = args.start + inserted
      }
      const result = { state: state(), start: args.start, end, loudness: processing.mode === 'lufs'
        ? { before: -20, after: Math.min(-12, processing.target), gainDb: Math.min(8, processing.target + 20), limited: processing.target > -12 } : null }
      return new Promise(resolve => setTimeout(() => resolve(result), 350))
    }
    case 'editor_copy':
      clip = { frames: args.end - args.start, channels: 2, sampleRate: RATE }
      return clip
    case 'editor_clipboard': return clip
    // The fake audio stays as it is; only the selection follows the paste.
    case 'editor_paste': return { state: state(), start: args.start, end: args.start + (clip?.frames ?? 0) }
    case 'editor_slices': return slices(args.start, args.end, args.by)
    case 'editor_export_slices': return { dir: '/Users/me/Projects/Sound/Loops/Drum Loop 120 slices', count: args.spans.length, inLibrary: true }
    // Every beat starts on a zero crossing; snap to the nearest within reach.
    case 'editor_zero_snap': return args.frames.map((f: number) => {
      const beat = Math.round(f / BEAT) * BEAT
      return Math.abs(beat - f) <= args.radius ? Math.round(beat) : f
    })
    default: return undefined
  }
}
