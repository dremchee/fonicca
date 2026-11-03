import type { Sample } from '@/api'
import type { Marker } from '@/editor-regions'
import type { EditOp, EditorState, AudioProcessing, Span } from './editor/types'
import { editorApi } from './editor/api'
import { createEditorState } from './editor/state'
import { createEditorSession } from './editor/session'
import { createEditorSaving } from './editor/saving'
import { createEditorPlayback } from './editor/playback'
import { acceptHMRUpdate, defineStore } from 'pinia'
import { toast } from 'vue-sonner'
import { usePlayerStore } from '@/stores/player'
import { formatNumber } from '@/format'
import { t } from '@/i18n'

export { regionsOf } from '@/editor-regions'
export type { Marker, Region } from '@/editor-regions'
export type { SaveMode, Curve, EditOp, EditorState, Wave, Span, SliceBy, ClipInfo, EditedFile, AudioProcessing, LoudnessReport } from './editor/types'

/** Opens a sample in the library's embedded editor. */
export async function openEditor(sample: Sample) {
  await useEditorStore().openSample(sample)
}

/** State of the editor window this page runs in. */
export const useEditorStore = defineStore('editor', () => {
  const context = createEditorState()
  const { sample, state, open, loading, processing, saving, sessionVersion,
    expanded, windowView, unsavedPrompt, revision, cursor, selection, looping,
    playing, playhead, curve, clipboard, canOverwrite, error, regions } = context
  const stopLibraryPlayback = () => usePlayerStore().stop()
  const { play, stopPlayback, togglePlay, updatePlaybackRange } = createEditorPlayback({ context, stopLibraryPlayback })
  const { save, saveRegions } = createEditorSaving({ context, stopLibraryPlayback, stopPlayback })
  const { resolveUnsaved, openSample, closeEmbedded, reset, detachWindow, adoptWindow, init } = createEditorSession({
    context, stopLibraryPlayback, stopPlayback, save, refreshClipboard,
  })

  function after(next: EditorState, audioChanged = true) {
    state.value = next
    if (audioChanged) revision.value++
    // Keep cursor and selection inside the (possibly shorter) audio.
    cursor.value = Math.min(cursor.value, next.frames)
    if (selection.value) {
      const end = Math.min(selection.value.end, next.frames)
      selection.value = end > selection.value.start ? { start: selection.value.start, end } : null
    }
  }

  async function changeAudio(action: () => Promise<EditorState>, stop = true, audioChanged = true) {
    if (loading.value || processing.value || saving.value) return
    processing.value = true
    try {
      if (stop) await stopPlayback()
      after(await action(), audioChanged)
    }
    catch (e) { toast.error(String(e)) }
    finally { processing.value = false }
  }

  function apply(op: EditOp, stop = true) {
    return changeAudio(() => editorApi.apply(op), stop, !['setMarkers', 'setLoop', 'clearLoop'].includes(op.op))
  }

  function undo() {
    if (state.value?.canUndo) return changeAudio(() => editorApi.undo())
  }

  function redo() {
    if (state.value?.canRedo) return changeAudio(() => editorApi.redo())
  }

  /** The selection, or the whole file when nothing is selected. */
  function target() {
    return selection.value ?? { start: 0, end: state.value?.frames ?? 0 }
  }

  function crop() {
    if (processing.value || saving.value)
      return
    const s = selection.value
    if (!s)
      return
    selection.value = null
    cursor.value = 0
    apply({ op: 'crop', ...s })
  }

  function remove() {
    if (processing.value || saving.value)
      return
    const s = selection.value
    if (!s)
      return
    selection.value = null
    cursor.value = s.start
    apply({ op: 'delete', ...s })
  }

  async function refreshClipboard() {
    try {
      clipboard.value = await editorApi.clipboard()
    }
    catch (e) {
      console.error(e)
    }
  }

  async function copy() {
    if (processing.value || saving.value)
      return
    const s = selection.value
    if (!s)
      return
    try {
      clipboard.value = await editorApi.copy(s.start, s.end)
    }
    catch (e) {
      toast.error(String(e))
    }
  }

  async function cut() {
    const s = selection.value && { ...selection.value }
    if (!s || s.end <= s.start) return
    await changeAudio(async () => {
      clipboard.value = await editorApi.copy(s.start, s.end)
      const next = await editorApi.apply({ op: 'delete', ...s })
      selection.value = null
      cursor.value = s.start
      return next
    })
  }

  /** Pastes over the selection, or at the cursor; the pasted audio ends up selected. */
  async function paste() {
    if (!clipboard.value) return
    const s = selection.value ?? { start: cursor.value, end: cursor.value }
    await changeAudio(async () => {
      const r = await editorApi.paste(s.start, s.end)
      selection.value = r.end > r.start ? { start: r.start, end: r.end } : null
      cursor.value = r.start
      return r.state
    })
  }

  function silence() {
    if (processing.value || saving.value)
      return
    if (selection.value)
      apply({ op: 'silence', ...selection.value })
  }

  /** Fades the selection; without one, from the start to the cursor (in) or the cursor to the end (out). */
  function fade(direction: 'in' | 'out') {
    if (processing.value || saving.value)
      return
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

  async function processAudio(options: AudioProcessing): Promise<boolean> {
    if (processing.value || saving.value || !state.value)
      return false
    const range = target()
    stopPlayback()
    processing.value = true
    try {
      const result = await editorApi.process(range.start, range.end, options)
      after(result.state)
      selection.value = { start: result.start, end: result.end }
      cursor.value = result.start
      if (result.loudness) {
        const report = result.loudness
        toast.success(t(report.limited ? 'editor.processing.limited' : 'editor.processing.normalized', {
          before: formatNumber(report.before, { fixed: 1 }),
          after: formatNumber(report.after, { fixed: 1 }),
        }))
      }
      return true
    }
    catch (e) {
      toast.error(String(e))
      return false
    }
    finally {
      processing.value = false
    }
  }

  function removeDcOffset() {
    const range = target()
    if (range.end > range.start)
      return apply({ op: 'dcOffset', ...range })
  }

  function reverse() {
    apply({ op: 'reverse', ...target() })
  }

  /** Replaces the markers (one undoable edit); the backend sorts and cleans them up. */
  function setMarkers(markers: Marker[]) {
    return apply({ op: 'setMarkers', markers }, false)
  }

  /** One undoable loop-bound edit, preserving the current playback stream. */
  async function setLoopPoints(range: Span): Promise<boolean> {
    if (!state.value || loading.value || processing.value || saving.value) return false
    if (range.start < 0 || range.end > state.value.frames || range.end <= range.start) return false
    if (state.value.loop?.start === range.start && state.value.loop.end === range.end) return true
    await apply({ op: 'setLoop', ...range }, false)
    return state.value?.loop?.start === range.start && state.value.loop.end === range.end
  }

  /** Loop points from the selection; without one, clears them. Playback goes on. */
  async function toggleLoopPoints() {
    if (selection.value)
      await setLoopPoints(selection.value)
    else if (state.value?.loop)
      await apply({ op: 'clearLoop' }, false)
  }

  return {
    sample,
    state,
    expanded,
    windowView,
    detachWindow,
    adoptWindow,
    unsavedPrompt,
    resolveUnsaved,
    openSample,
    closeEmbedded,
    reset,
    open,
    loading,
    processing,
    saving,
    sessionVersion,
    processAudio,
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
    spectrogram: editorApi.spectrogram,
    undo,
    redo,
    crop,
    remove,
    silence,
    fade,
    gain,
    normalize,
    removeDcOffset,
    reverse,
    toggleLoopPoints,
    setLoopPoints,
    regions,
    setMarkers,
    saveRegions,
    clipboard,
    copy,
    cut,
    paste,
    zeroSnap: editorApi.zeroSnap,
    slices: editorApi.slices,
    exportSlices: editorApi.exportSlices,
    play,
    stopPlayback,
    togglePlay,
    updatePlaybackRange,
    save,
  }
})

if (import.meta.hot)
  import.meta.hot.accept(acceptHMRUpdate(useEditorStore, import.meta.hot))
