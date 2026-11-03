import { invoke } from '@tauri-apps/api/core'
import type { EditorWindowView } from '@/editor-window-state'
import { editorTimings } from '@/editor-performance'
import { parseSpectrogram } from '@/spectrogram'
import { parseWave } from '@/waveform'
import type { AudioProcessing, ClipInfo, EditOp, EditorState, ProcessedAudio, SaveMode, SliceBy, Span, Wave } from './types'

function invokeEditor<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  const op = args?.op
  const name = command === 'editor_apply' && op && typeof op === 'object' && 'op' in op
    ? `${command}.${String(op.op)}` : command
  return editorTimings.measure(name, () => invoke<T>(command, args))
}

export const editorApi = {
  openWindow: (id: number) => invokeEditor<void>('open_editor_window', { id }),
  open: (id: number, discardExisting = false, reuse = false) => invokeEditor<{ state: EditorState, name: string, ext: string, path: string, bpm: number | null }>('editor_open', { id, discardExisting, reuse }),
  prepareTransfer: (token: number) => invokeEditor<{ state: EditorState, name: string, ext: string, path: string, bpm: number | null }>('editor_prepare_transfer', { token }),
  acceptTransfer: (token: number) => invokeEditor<void>('editor_accept_transfer', { token }),
  rejectTransfer: (token: number) => invokeEditor<void>('editor_reject_transfer', { token }),
  attach: (view: EditorWindowView) => invokeEditor<void>('editor_attach_window', { view }),
  detach: (view: EditorWindowView) => invokeEditor<void>('editor_detach_window', { view }),
  close: (discard: boolean) => invokeEditor<void>('editor_close', { discard }),
  apply: (op: EditOp) => invokeEditor<EditorState>('editor_apply', { op }),
  process: (start: number, end: number, processing: AudioProcessing) => invokeEditor<ProcessedAudio>('editor_process', { start, end, processing }),
  undo: () => invokeEditor<EditorState>('editor_undo'),
  redo: () => invokeEditor<EditorState>('editor_redo'),
  wave: async (start: number, end: number, columns: number, split: boolean, bands: boolean): Promise<Wave> => {
    return parseWave(await invokeEditor<ArrayBuffer>('editor_wave', { start, end, columns, split, bands }))
  },
  spectrogram: async (start: number, end: number, columns: number) => parseSpectrogram(await invokeEditor<ArrayBuffer>('editor_spectrogram', { start, end, columns })),
  slices: (start: number, end: number, by: SliceBy, zero: boolean) => invokeEditor<Span[]>('editor_slices', { start, end, by, zero }),
  exportSlices: (spans: Span[]) => invokeEditor<{ dir: string, count: number, inLibrary: boolean }>('editor_export_slices', { spans }),
  copy: (start: number, end: number) => invokeEditor<ClipInfo | null>('editor_copy', { start, end }),
  clipboard: () => invokeEditor<ClipInfo | null>('editor_clipboard'),
  paste: (start: number, end: number) => invokeEditor<{ state: EditorState, start: number, end: number }>('editor_paste', { start, end }),
  zeroSnap: (frames: number[], radius: number) => invokeEditor<number[]>('editor_zero_snap', { frames, radius }),
  play: (start: number, end: number, looping: boolean) => invokeEditor<void>('editor_play', { start, end, looping }),
  updatePlayback: (start: number, end: number, looping: boolean) => invokeEditor<boolean>('editor_update_playback', { start, end, looping }),
  save: (mode: SaveMode, destination?: string, range?: Span, sampleRate?: number | null) =>
    invokeEditor<{ path: string, backup: string | null, state: EditorState, name: string, ext: string, bpm: number | null, inLibrary: boolean, warning: string | null }>('editor_save', { mode, destination, range, sampleRate }),
}
