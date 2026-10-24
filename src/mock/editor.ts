// A fake editor session for /mock.html#editor: four bars of a 120 BPM loop
// (kick on every beat, hats in between), so the bar grid has something to line up with.
import type { EditorState, SliceBy, Span } from '@/stores/editor'

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

function state(): EditorState {
  return { sampleId: 1, frames: FRAMES, sampleRate: RATE, channels: 2, canUndo: false, canRedo: false, dirty: loop !== null, loop }
}

// Same packing as the backend: u32 mode, lanes, count, stride, then lane-major columns.
function wave(start: number, end: number, columns: number, split: boolean) {
  const lanes = split ? 2 : 1
  const stride = 6
  const buf = new ArrayBuffer(16 + lanes * columns * stride * 4)
  new Uint32Array(buf, 0, 4).set([0, lanes, columns, stride])
  const values = new Float32Array(buf, 16)
  for (let lane = 0; lane < lanes; lane++) {
    for (let c = 0; c < columns; c++) {
      const a = start + ((end - start) * c) / columns
      const b = start + ((end - start) * (c + 1)) / columns
      let peak = 0
      for (let s = 0; s < 4; s++)
        peak = Math.max(peak, level(a + ((b - a) * s) / 4))
      const i = (lane * columns + c) * stride
      values.set([-peak, peak, peak * 0.6, peak * 0.8, 0.3, 0.2], i)
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
    case 'editor_wave': return wave(args.start, args.end, args.columns, args.split)
    case 'editor_apply':
      if (args.op.op === 'setLoop')
        loop = { start: args.op.start, end: args.op.end }
      else if (args.op.op === 'clearLoop')
        loop = null
      return state()
    case 'editor_undo': case 'editor_redo': return state()
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
