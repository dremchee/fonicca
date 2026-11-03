import type { Marker } from '@/editor-regions'

export type SaveMode = 'save' | 'saveAs' | 'replaceOriginal' | 'selection'

export type Curve = 'linear' | 'smooth' | 'exponential'

/** One edit; ranges are frames of the current buffer, `end` exclusive. */
export type EditOp
  = | { op: 'crop' | 'delete' | 'silence' | 'reverse' | 'dcOffset', start: number, end: number }
    | { op: 'fadeIn' | 'fadeOut', start: number, end: number, curve: Curve }
    | { op: 'gain', start: number, end: number, db: number }
    | { op: 'normalize', start: number, end: number, peakDb: number }
    | { op: 'setLoop', start: number, end: number }
    | { op: 'clearLoop' }
    | { op: 'setMarkers', markers: Marker[] }

export interface EditorState {
  sampleId: number
  frames: number
  sampleRate: number
  channels: number
  canUndo: boolean
  canRedo: boolean
  dirty: boolean
  hasSavedTarget: boolean
  /** Output rate of the saved target; live editing stays at sampleRate. */
  exportSampleRate?: number | null
  canReplaceOriginal: boolean
  originalPath: string
  /** Loop points for samplers, saved in the WAV's `smpl` chunk. */
  loop: { start: number, end: number } | null
  /** Slice markers, sorted; each starts a region, saved as cue points in the WAV. */
  markers: Marker[]
}

/**
 * Waveform of the visible range. With `samples` each lane has one value per
 * frame; otherwise per column `min, max, rms, low, mid, high` (stride 6).
 */
export interface Wave {
  samples: boolean
  lanes: number
  count: number
  stride: number
  values: Float32Array
}

/** A slice, `[start, end)` frames. */
export interface Span { start: number, end: number }

/** How to cut into slices: on the bar grid, at attacks, or around silence. */
export type SliceBy
  = | { mode: 'grid', origin: number, step: number }
    | { mode: 'transients', sensitivity: number, minMs: number }
    | { mode: 'silence', thresholdDb: number, minSilenceMs: number }

/** What the clipboard shared by the editor windows holds. */
export interface ClipInfo { frames: number, channels: number, sampleRate: number }

/** What an editor window shows about its file. */
export interface EditedFile {
  id: number
  name: string
  ext: string
  path: string
  bpm: number | null
}

export type AudioProcessing
  = | { mode: 'stretch', semitones: number, ratio: number }
    | { mode: 'lufs', target: number, ceiling: number }

export interface LoudnessReport { before: number, after: number, gainDb: number, limited: boolean }
export interface ProcessedAudio { state: EditorState, start: number, end: number, loudness: LoudnessReport | null }
