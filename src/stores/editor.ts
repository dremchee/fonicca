import type { Sample } from '@/api'
import { regionsOf, type Marker } from '@/editor-regions'
export { regionsOf } from '@/editor-regions'
export type { Marker, Region } from '@/editor-regions'
import { invoke } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { revealItemInDir } from '@tauri-apps/plugin-opener'
import { save as chooseSavePath } from '@tauri-apps/plugin-dialog'
import { acceptHMRUpdate, defineStore } from 'pinia'
import { computed, ref, watch } from 'vue'
import { toast } from 'vue-sonner'
import { api } from '@/api'
import { usePlayerStore } from '@/stores/player'
import { confirm } from '@/composables/confirm'
import { formatNumber } from '@/format'
import { t } from '@/i18n'
import { parseWave } from '@/waveform'
import { audioExportFormat, audioExportPath } from '@/audio-export'
import { chooseAudioExport, settleAudioExport } from '@/composables/audioExport'
import { editorTimings } from '@/editor-performance'
import { parseSpectrogram } from '@/spectrogram'
import { readEditorWindowView, type EditorWindowView } from '@/editor-window-state'

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
interface ProcessedAudio { state: EditorState, start: number, end: number, loudness: LoudnessReport | null }

function invokeEditor<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  const op = args?.op
  const name = command === 'editor_apply' && op && typeof op === 'object' && 'op' in op
    ? `${command}.${String(op.op)}` : command
  return editorTimings.measure(name, () => invoke<T>(command, args))
}

const editorApi = {
  openWindow: (id: number) => invokeEditor<void>('open_editor_window', { id }),
  open: (id: number, discardExisting = false, reuse = false) => invokeEditor<{ state: EditorState, name: string, ext: string, path: string, bpm: number | null }>('editor_open', { id, discardExisting, reuse }),
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

/** Opens a sample in the library's embedded editor. */
export async function openEditor(sample: Sample) {
  await useEditorStore().openSample(sample)
}

/** State of the editor window this page runs in. */
export const useEditorStore = defineStore('editor', () => {
  const sample = ref<EditedFile | null>(null)
  const state = ref<EditorState | null>(null)
  const open = computed(() => sample.value !== null)
  const loading = ref(false)
  const processing = ref(false)
  const saving = ref(false)
  const sessionVersion = ref(0)
  const expanded = ref(false)
  const windowView = ref<EditorWindowView | null>(null)
  const unsavedPrompt = ref(false)
  let transitioning = false
  let decide: ((choice: 'save' | 'discard' | 'cancel') => void) | null = null

  function resolveUnsaved(choice: 'save' | 'discard' | 'cancel') {
    unsavedPrompt.value = false
    const resolve = decide
    decide = null
    resolve?.(choice)
  }

  async function allowTransition(): Promise<boolean> {
    if (loading.value || processing.value || saving.value || decide)
      return false
    if (!state.value?.dirty)
      return true
    unsavedPrompt.value = true
    const choice = await new Promise<'save' | 'discard' | 'cancel'>(resolve => { decide = resolve })
    if (choice === 'cancel') return false
    if (choice === 'discard') return true
    loading.value = true
    try { return await save() }
    finally { loading.value = false }
  }

  async function openSample(next: Sample) {
    if ((sample.value?.id === next.id && sample.value.path === next.path) || transitioning) return
    transitioning = true
    let previous: { sample: EditedFile | null, state: EditorState | null } | null = null
    let opened = false
    try {
      if (!await allowTransition()) return
      previous = { sample: sample.value, state: state.value }
      loading.value = true
      // Mount the editor before decoding audio, using the known file metadata.
      sample.value = { id: next.id, name: next.name, ext: next.ext, path: next.path, bpm: next.bpm }
      state.value = null
      await usePlayerStore().stop()
      await stopPlayback()
      const result = await editorApi.open(next.id, !!previous.state?.dirty)
      opened = true
      state.value = result.state
      sample.value = { id: next.id, name: result.name, ext: result.ext, path: result.path, bpm: result.bpm }
      sessionVersion.value++
      cursor.value = 0
      selection.value = null
      looping.value = false
      revision.value++
      await refreshClipboard()
    }
    catch (error) {
      if (previous && !opened) {
        sample.value = previous.sample
        state.value = previous.state
        revision.value++
      }
      toast.error(String(error))
    }
    finally { loading.value = false; transitioning = false }
  }

  async function closeEmbedded(): Promise<boolean> {
    if (!sample.value) return true
    if (transitioning) return false
    transitioning = true
    try {
      if (!await allowTransition()) return false
      loading.value = true
      await stopPlayback()
      await editorApi.close(!!state.value?.dirty)
      sample.value = null
      state.value = null
      selection.value = null
      cursor.value = 0
      expanded.value = false
      return true
    }
    catch (error) { toast.error(String(error)); return false }
    finally { loading.value = false; transitioning = false }
  }

  /** A UI reload deliberately starts clean, including the surviving native session. */
  async function reset(): Promise<boolean> {
    resolveUnsaved('cancel')
    settleAudioExport(null)
    loading.value = true
    try {
      await stopPlayback()
      await usePlayerStore().stop()
      await editorApi.close(true)
      sample.value = null
      state.value = null
      selection.value = null
      cursor.value = 0
      looping.value = false
      expanded.value = false
      windowView.value = null
      error.value = null
      sessionVersion.value++
      revision.value++
      return true
    } catch (e) {
      toast.error(String(e))
      return false
    } finally {
      loading.value = false
      processing.value = false
      saving.value = false
      transitioning = false
    }
  }

  async function detachWindow(viewport?: EditorWindowView['viewport']): Promise<boolean> {
    if (!sample.value || !state.value || transitioning || loading.value || processing.value || saving.value || decide) return false
    transitioning = true
    loading.value = true
    try {
      const view: EditorWindowView = { cursor: playhead.value ?? cursor.value,
        selection: selection.value && { ...selection.value }, looping: looping.value, curve: curve.value, viewport }
      await stopPlayback()
      await editorApi.detach(view)
      // The native session now belongs to the new window. Do not close it or
      // prompt to save: those actions belong to that window from this point on.
      sample.value = null
      state.value = null
      selection.value = null
      cursor.value = 0
      expanded.value = false
      windowView.value = null
      sessionVersion.value++
      return true
    } catch (error) {
      toast.error(String(error))
      return false
    } finally { loading.value = false; transitioning = false }
  }
  /** Bumped after every change of the audio, so the view re-reads peaks. */
  const revision = ref(0)

  // Selection and cursor, in frames.
  const cursor = ref(0)
  const selection = ref<{ start: number, end: number } | null>(null)
  const looping = ref(false)
  const playing = ref(false)
  const playhead = ref<number | null>(null)
  const curve = ref<Curve>('smooth')
  /** The clipboard shared by all editor windows; re-read when this one gets focus. */
  const clipboard = ref<ClipInfo | null>(null)
  let frame = 0
  let playbackVersion = 0

  const canOverwrite = computed(() => !!state.value?.canReplaceOriginal)

  const error = ref<string | null>(null)

  /** Loads the window's file and wires up the title and close confirmation. */
  async function init(id: number) {
    const win = getCurrentWindow()
    // A newly detached window must adopt its transferred session. A reload of
    // that window discards it and opens the file afresh, like the main UI reset.
    const reloading = open.value || (performance.getEntriesByType('navigation')[0] as PerformanceNavigationTiming | undefined)?.type === 'reload'
    if (reloading && !await reset()) return
    loading.value = true
    try {
      await api.stop()
      const r = await editorApi.open(id, false, !reloading && new URLSearchParams(location.search).get('detached') === '1')
      state.value = r.state
      sample.value = { id, name: r.name, ext: r.ext, path: r.path, bpm: r.bpm }
      sessionVersion.value++
      revision.value++
      const view = reloading ? null : readEditorWindowView(location.search, r.state.frames)
      if (view) {
        cursor.value = view.cursor
        selection.value = view.selection
        looping.value = view.looping
        curve.value = view.curve
        windowView.value = view
      }
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

    refreshClipboard()
    await win.onFocusChanged(({ payload: focused }) => {
      if (focused)
        refreshClipboard()
    })

    await win.onCloseRequested((event) => {
      if (!sample.value) return
      event.preventDefault()
      void closeEmbedded().then(closed => { if (closed) void win.close() })
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

  async function changeAudio(action: () => Promise<EditorState>, stop = true) {
    if (loading.value || processing.value || saving.value) return
    processing.value = true
    try {
      if (stop) await stopPlayback()
      after(await action())
    }
    catch (e) { toast.error(String(e)) }
    finally { processing.value = false }
  }

  function apply(op: EditOp, stop = true) {
    return changeAudio(() => editorApi.apply(op), stop)
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

  const regions = computed(() => regionsOf(state.value?.markers ?? [], state.value?.frames ?? 0))

  /** Replaces the markers (one undoable edit); the backend sorts and cleans them up. */
  function setMarkers(markers: Marker[]) {
    return apply({ op: 'setMarkers', markers }, false)
  }

  /** Saves each region not skipped as a new sample, into a folder next to the file. */
  async function saveRegions() {
    if (processing.value || saving.value)
      return
    const spans = regions.value.filter(r => !r.skip && r.end > r.start).map(({ start, end }) => ({ start, end }))
    if (!spans.length)
      return
    try {
      const r = await editorApi.exportSlices(spans)
      toast.success(t('editor.regions.saved', { n: formatNumber(r.count) }, r.count), {
        description: r.inLibrary ? undefined : t('editor.regions.notInLibrary'),
        action: { label: t('editor.regions.reveal'), onClick: () => revealItemInDir(r.dir) },
      })
    }
    catch (e) {
      toast.error(String(e))
    }
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

  // --- Playback ------------------------------------------------------------

  function track(version: number) {
    cancelAnimationFrame(frame)
    const tick = async () => {
      const pos = await api.playbackPosition()
      if (version !== playbackVersion || !playing.value)
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
    const version = ++playbackVersion
    cancelAnimationFrame(frame)
    const s = selection.value
    const [start, end] = s ? [s.start, s.end] : [cursor.value, state.value.frames]
    try {
      await usePlayerStore().stop()
      if (version !== playbackVersion) return
      await editorApi.play(start, end, looping.value && !!s)
      if (version !== playbackVersion) return
      playing.value = true
      playhead.value = start
      track(version)
    }
    catch (e) {
      if (version === playbackVersion) toast.error(String(e))
    }
  }

  async function updatePlaybackRange() {
    const range = selection.value
    if (!playing.value || !range) return
    const version = playbackVersion
    try {
      const active = await editorApi.updatePlayback(range.start, range.end, looping.value)
      if (version !== playbackVersion) return
      if (!active) {
        cancelAnimationFrame(frame)
        playing.value = false
        playhead.value = null
      }
    }
    catch (e) { toast.error(String(e)) }
  }

  function stopPlayback() {
    playbackVersion++
    cancelAnimationFrame(frame)
    const stopped = playing.value ? api.stop() : Promise.resolve()
    playing.value = false
    playhead.value = null
    return stopped
  }

  function togglePlay() {
    if (playing.value)
      stopPlayback()
    else
      play()
  }

  // --- Saving --------------------------------------------------------------

  async function save(mode: SaveMode = 'save') {
    if (!sample.value || processing.value || saving.value)
      return false
    if (mode === 'save' && !state.value?.dirty)
      return true
    const selected = selection.value ? { ...selection.value } : undefined
    if (mode === 'selection' && !selected)
      return false
    saving.value = true
    try {
      let destination: string | undefined
      let exportRate: number | null | undefined
      const actualMode = mode === 'save' && !state.value?.hasSavedTarget ? 'saveAs' : mode
      if (actualMode === 'saveAs' || actualMode === 'selection') {
        const options = await chooseAudioExport({ format: audioExportFormat(sample.value.path), sampleRate: state.value?.exportSampleRate ?? null }, state.value?.sampleRate ?? 44100)
        if (!options) return false
        const format = options.format
        exportRate = options.sampleRate
        const suffix = `${mode === 'selection' ? ' — loop' : ' — edited'}.${format}`
        const picked = await chooseSavePath({
          title: t(mode === 'selection' ? 'editor.save.selection' : 'editor.save.as'),
          defaultPath: sample.value.path.replace(/\.[^/\\.]+$/, '') + suffix,
          filters: [{ name: format === 'ogg' ? 'OGG Vorbis' : format.toUpperCase(), extensions: [format] }],
        })
        if (!picked) return false
        destination = audioExportPath(picked, format)
      }
      if (mode === 'replaceOriginal') {
        if (!canOverwrite.value || !await confirm({ title: t('editor.save.replace'),
          description: t('editor.save.replaceConfirm', { path: state.value?.originalPath ?? '' }),
          action: t('editor.save.replace'), destructive: true })) return false
      }
      await usePlayerStore().stop()
      await stopPlayback()
      const r = await editorApi.save(actualMode, destination, mode === 'selection' ? selected : undefined, exportRate)
      if (mode !== 'selection') {
        state.value = r.state
        sample.value = { id: r.state.sampleId, name: r.name, ext: r.ext, path: r.path, bpm: r.bpm ?? sample.value.bpm }
      }
      toast.success(t(mode === 'selection' ? 'editor.save.selectionSaved' : 'editor.save.saved', { name: r.name }), {
        description: r.backup ? t('editor.save.backupKept') : undefined,
      })
      if (r.warning) toast.warning(t('editor.save.indexWarning'), { description: r.warning })
      return true
    }
    catch (e) {
      toast.error(String(e))
      return false
    }
    finally { saving.value = false }
  }

  return {
    sample,
    state,
    expanded,
    windowView,
    detachWindow,
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
