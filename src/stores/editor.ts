import type { Sample } from '@/api'
import { invoke } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { acceptHMRUpdate, defineStore } from 'pinia'
import { computed, ref, watch } from 'vue'
import { toast } from 'vue-sonner'
import { api } from '@/api'
import { confirm } from '@/composables/confirm'
import { t } from '@/i18n'
import { parseWave } from '@/waveform'

export type Curve = 'linear' | 'smooth' | 'exponential'

/** One edit; ranges are frames of the current buffer, `end` exclusive. */
export type EditOp
  = | { op: 'crop' | 'delete' | 'silence' | 'reverse', start: number, end: number }
    | { op: 'fadeIn' | 'fadeOut', start: number, end: number, curve: Curve }
    | { op: 'gain', start: number, end: number, db: number }
    | { op: 'normalize', start: number, end: number, peakDb: number }
    | { op: 'setLoop', start: number, end: number }
    | { op: 'clearLoop' }

export interface EditorState {
  sampleId: number
  frames: number
  sampleRate: number
  channels: number
  canUndo: boolean
  canRedo: boolean
  dirty: boolean
  /** Loop points for samplers, saved in the WAV's `smpl` chunk. */
  loop: { start: number, end: number } | null
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

/** What an editor window shows about its file. */
export interface EditedFile {
  id: number
  name: string
  ext: string
  path: string
  bpm: number | null
}

const editorApi = {
  openWindow: (id: number) => invoke<void>('open_editor_window', { id }),
  open: (id: number) => invoke<{ state: EditorState, name: string, ext: string, path: string, bpm: number | null }>('editor_open', { id }),
  apply: (op: EditOp) => invoke<EditorState>('editor_apply', { op }),
  undo: () => invoke<EditorState>('editor_undo'),
  redo: () => invoke<EditorState>('editor_redo'),
  wave: async (start: number, end: number, columns: number, split: boolean, bands: boolean): Promise<Wave> => {
    return parseWave(await invoke<ArrayBuffer>('editor_wave', { start, end, columns, split, bands }))
  },
  slices: (start: number, end: number, by: SliceBy, zero: boolean) => invoke<Span[]>('editor_slices', { start, end, by, zero }),
  exportSlices: (spans: Span[]) => invoke<{ dir: string, count: number, inLibrary: boolean }>('editor_export_slices', { spans }),
  zeroSnap: (frames: number[], radius: number) => invoke<number[]>('editor_zero_snap', { frames, radius }),
  play: (start: number, end: number, looping: boolean) => invoke<void>('editor_play', { start, end, looping }),
  save: (asNew: boolean) =>
    invoke<{ path: string, backup: string | null, state: EditorState }>('editor_save', { asNew }),
}

/** Opens a sample in its own editor window (from the library window). */
export async function openEditor(sample: Sample) {
  try {
    await editorApi.openWindow(sample.id)
  }
  catch (e) {
    toast.error(String(e))
  }
}

/** State of the editor window this page runs in. */
export const useEditorStore = defineStore('editor', () => {
  const sample = ref<EditedFile | null>(null)
  const state = ref<EditorState | null>(null)
  const open = computed(() => sample.value !== null)
  const loading = ref(false)
  /** Bumped after every change of the audio, so the view re-reads peaks. */
  const revision = ref(0)

  // Selection and cursor, in frames.
  const cursor = ref(0)
  const selection = ref<{ start: number, end: number } | null>(null)
  const looping = ref(false)
  const playing = ref(false)
  const playhead = ref<number | null>(null)
  const curve = ref<Curve>('smooth')
  let frame = 0

  const canOverwrite = computed(() => sample.value?.ext === 'wav')

  const error = ref<string | null>(null)

  /** Loads the window's file and wires up the title and close confirmation. */
  async function init(id: number) {
    const win = getCurrentWindow()
    loading.value = true
    try {
      await api.stop()
      const r = await editorApi.open(id)
      state.value = r.state
      sample.value = { id, name: r.name, ext: r.ext, path: r.path, bpm: r.bpm }
      revision.value++
    }
    catch (e) {
      error.value = String(e)
    }
    finally {
      loading.value = false
    }

    // "Kick • — Edit" while there are unsaved edits; re-titled on a language switch too.
    watch(() => {
      const name = sample.value?.name
      return name ? t(state.value?.dirty ? 'editor.window.titleDirty' : 'editor.window.title', { name }) : null
    }, (title) => {
      if (title)
        win.setTitle(title)
    }, { immediate: true })

    await win.onCloseRequested(async (event) => {
      if (state.value?.dirty) {
        const ok = await confirm({
          title: t('editor.discard.title'),
          description: t('editor.discard.description', { name: sample.value?.name ?? '' }),
          action: t('editor.discard.action'),
          destructive: true,
        })
        if (!ok) {
          event.preventDefault()
          return
        }
      }
      stopPlayback()
    })
  }

  function after(next: EditorState) {
    state.value = next
    revision.value++
    // Keep cursor and selection inside the (possibly shorter) audio.
    cursor.value = Math.min(cursor.value, next.frames)
    if (selection.value) {
      const end = Math.min(selection.value.end, next.frames)
      selection.value = end > selection.value.start ? { start: selection.value.start, end } : null
    }
  }

  async function apply(op: EditOp, stop = true) {
    if (stop)
      stopPlayback()
    try {
      after(await editorApi.apply(op))
    }
    catch (e) {
      toast.error(String(e))
    }
  }

  async function undo() {
    if (state.value?.canUndo)
      after(await editorApi.undo())
  }

  async function redo() {
    if (state.value?.canRedo)
      after(await editorApi.redo())
  }

  /** The selection, or the whole file when nothing is selected. */
  function target() {
    return selection.value ?? { start: 0, end: state.value?.frames ?? 0 }
  }

  function crop() {
    const s = selection.value
    if (!s)
      return
    selection.value = null
    cursor.value = 0
    apply({ op: 'crop', ...s })
  }

  function remove() {
    const s = selection.value
    if (!s)
      return
    selection.value = null
    cursor.value = s.start
    apply({ op: 'delete', ...s })
  }

  function silence() {
    if (selection.value)
      apply({ op: 'silence', ...selection.value })
  }

  /** Fades the selection; without one, from the start to the cursor (in) or the cursor to the end (out). */
  function fade(direction: 'in' | 'out') {
    const frames = state.value?.frames ?? 0
    const range = selection.value
      ?? (direction === 'in' ? { start: 0, end: cursor.value } : { start: cursor.value, end: frames })
    if (range.end - range.start < 2)
      return
    apply({ op: direction === 'in' ? 'fadeIn' : 'fadeOut', ...range, curve: curve.value })
  }

  function gain(db: number) {
    apply({ op: 'gain', ...target(), db })
  }

  function normalize(peakDb = -0.3) {
    apply({ op: 'normalize', ...target(), peakDb })
  }

  function reverse() {
    apply({ op: 'reverse', ...target() })
  }

  /** Loop points from the selection; without one, clears them. Playback goes on. */
  function toggleLoopPoints() {
    if (selection.value)
      apply({ op: 'setLoop', ...selection.value }, false)
    else if (state.value?.loop)
      apply({ op: 'clearLoop' }, false)
  }

  // --- Playback ------------------------------------------------------------

  function track() {
    cancelAnimationFrame(frame)
    const tick = async () => {
      const pos = await api.playbackPosition()
      if (!playing.value)
        return
      if (pos === null) {
        stopPlayback()
        return
      }
      playhead.value = Math.round(pos * (state.value?.sampleRate ?? 1))
      frame = requestAnimationFrame(tick)
    }
    frame = requestAnimationFrame(tick)
  }

  /** Plays the selection (looping if enabled), or from the cursor to the end. */
  async function play() {
    if (!state.value)
      return
    const s = selection.value
    const [start, end] = s ? [s.start, s.end] : [cursor.value, state.value.frames]
    try {
      await editorApi.play(start, end, looping.value && !!s)
      playing.value = true
      track()
    }
    catch (e) {
      toast.error(String(e))
    }
  }

  function stopPlayback() {
    cancelAnimationFrame(frame)
    if (playing.value)
      api.stop()
    playing.value = false
    playhead.value = null
  }

  function togglePlay() {
    if (playing.value)
      stopPlayback()
    else
      play()
  }

  // --- Saving --------------------------------------------------------------

  async function save(asNew = !canOverwrite.value) {
    stopPlayback()
    try {
      const r = await editorApi.save(asNew)
      state.value = r.state
      const name = r.path.split('/').pop()
      toast.success(t(asNew ? 'editor.save.savedAs' : 'editor.save.saved', { name: name ?? '' }), {
        description: r.backup ? t('editor.save.backupKept') : undefined,
      })
    }
    catch (e) {
      toast.error(String(e))
    }
  }

  return {
    sample,
    state,
    open,
    loading,
    error,
    revision,
    cursor,
    selection,
    looping,
    playing,
    playhead,
    curve,
    canOverwrite,
    init,
    wave: editorApi.wave,
    undo,
    redo,
    crop,
    remove,
    silence,
    fade,
    gain,
    normalize,
    reverse,
    toggleLoopPoints,
    zeroSnap: editorApi.zeroSnap,
    slices: editorApi.slices,
    exportSlices: editorApi.exportSlices,
    play,
    stopPlayback,
    togglePlay,
    save,
  }
})

if (import.meta.hot)
  import.meta.hot.accept(acceptHMRUpdate(useEditorStore, import.meta.hot))
