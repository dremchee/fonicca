import type { Sample } from '@/api'
import { invoke } from '@tauri-apps/api/core'
import { acceptHMRUpdate, defineStore } from 'pinia'
import { computed, ref } from 'vue'
import { toast } from 'vue-sonner'
import { api } from '@/api'
import { confirm } from '@/composables/confirm'
import { usePlayerStore } from '@/stores/player'

export type Curve = 'linear' | 'smooth' | 'exponential'

/** One edit; ranges are frames of the current buffer, `end` exclusive. */
export type EditOp
  = | { op: 'crop' | 'delete' | 'silence' | 'reverse', start: number, end: number }
    | { op: 'fadeIn' | 'fadeOut', start: number, end: number, curve: Curve }
    | { op: 'gain', start: number, end: number, db: number }
    | { op: 'normalize', start: number, end: number, peakDb: number }

export interface EditorState {
  sampleId: number
  frames: number
  sampleRate: number
  channels: number
  canUndo: boolean
  canRedo: boolean
  dirty: boolean
}

const editorApi = {
  open: (id: number) => invoke<EditorState>('editor_open', { id }),
  close: () => invoke<void>('editor_close'),
  apply: (op: EditOp) => invoke<EditorState>('editor_apply', { op }),
  undo: () => invoke<EditorState>('editor_undo'),
  redo: () => invoke<EditorState>('editor_redo'),
  peaks: (start: number, end: number, columns: number) => invoke<number[]>('editor_peaks', { start, end, columns }),
  play: (start: number, end: number, looping: boolean) => invoke<void>('editor_play', { start, end, looping }),
  save: (asNew: boolean) =>
    invoke<{ path: string, backup: string | null, state: EditorState }>('editor_save', { asNew }),
}

export const useEditorStore = defineStore('editor', () => {
  const player = usePlayerStore()

  const sample = ref<Sample | null>(null)
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

  async function openSample(s: Sample) {
    if (open.value && !(await closeEditor()))
      return
    await player.stop()
    loading.value = true
    try {
      state.value = await editorApi.open(s.id)
      sample.value = s
      cursor.value = 0
      selection.value = null
      revision.value++
    }
    catch (e) {
      toast.error(String(e))
    }
    finally {
      loading.value = false
    }
  }

  /** Asks before discarding unsaved edits. Returns false if the user keeps editing. */
  async function closeEditor(): Promise<boolean> {
    if (state.value?.dirty) {
      const ok = await confirm({
        title: 'Discard unsaved edits?',
        description: `Changes to “${sample.value?.name}” will be lost.`,
        action: 'Discard',
        destructive: true,
      })
      if (!ok)
        return false
    }
    stopPlayback()
    await editorApi.close()
    sample.value = null
    state.value = null
    return true
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

  async function apply(op: EditOp) {
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
      toast.success(asNew ? `Saved as “${name}”` : `Saved “${name}”`, {
        description: r.backup ? 'The previous version was kept in the backups folder.' : undefined,
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
    revision,
    cursor,
    selection,
    looping,
    playing,
    playhead,
    curve,
    canOverwrite,
    openSample,
    closeEditor,
    peaks: editorApi.peaks,
    undo,
    redo,
    crop,
    remove,
    silence,
    fade,
    gain,
    normalize,
    reverse,
    play,
    stopPlayback,
    togglePlay,
    save,
  }
})

if (import.meta.hot)
  import.meta.hot.accept(acceptHMRUpdate(useEditorStore, import.meta.hot))
