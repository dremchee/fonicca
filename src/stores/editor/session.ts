import { nextTick, watch } from 'vue'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { toast } from 'vue-sonner'
import { api } from '@/api'
import { settleAudioExport } from '@/composables/audioExport'
import { t } from '@/i18n'
import { acceptEditorTransfer } from '@/editor-window-transfer'
import { readEditorWindowView, readEditorTransferToken, type EditorWindowView } from '@/editor-window-state'
import { editorApi } from './api'
import type { Sample } from '@/api'
import type { EditedFile, EditorState } from './types'
import type { EditorStoreState } from './state'

type SessionState = Pick<EditorStoreState,
  'sample' | 'state' | 'open' | 'loading' | 'processing' | 'saving' | 'sessionVersion'
  | 'expanded' | 'windowView' | 'unsavedPrompt' | 'error' | 'revision' | 'cursor'
  | 'selection' | 'looping' | 'playhead' | 'curve'>

interface Options {
  context: SessionState
  stopLibraryPlayback: () => Promise<void>
  stopPlayback: () => Promise<void>
  save: () => Promise<boolean>
  refreshClipboard: () => Promise<void>
}

/** Owns opening intents and the prepare/accept/rollback window handshake. */
export function createEditorSession(options: Options) {
  const { sample, state, open, loading, processing, saving, sessionVersion,
    expanded, windowView, unsavedPrompt, error, revision, cursor, selection,
    looping, playhead, curve } = options.context
  const { stopLibraryPlayback, stopPlayback, save, refreshClipboard } = options
  let openingIntent = 0
  let transitioning = false
  let pendingWindowToken: number | null = null
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
    const intent = ++openingIntent
    let previous: { sample: EditedFile | null, state: EditorState | null } | null = null
    let opened = false
    try {
      if (!await allowTransition() || intent !== openingIntent) return
      previous = { sample: sample.value, state: state.value }
      loading.value = true
      // Mount the editor before decoding audio, using the known file metadata.
      sample.value = { id: next.id, name: next.name, ext: next.ext, path: next.path, bpm: next.bpm }
      state.value = null
      await stopLibraryPlayback()
      if (intent !== openingIntent) return
      await stopPlayback()
      if (intent !== openingIntent) return
      const result = await editorApi.open(next.id, !!previous.state?.dirty)
      if (intent !== openingIntent) return
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
      if (intent !== openingIntent) return
      if (previous && !opened) {
        sample.value = previous.sample
        state.value = previous.state
        revision.value++
      }
      toast.error(String(error))
    }
    finally {
      if (intent === openingIntent) { loading.value = false; transitioning = false }
    }
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
    // Late success and failure callbacks belong to the renderer state before
    // this explicit reset; neither may restore its track or clear a newer load.
    openingIntent++
    resolveUnsaved('cancel')
    settleAudioExport(null)
    loading.value = true
    try {
      await stopPlayback()
      await stopLibraryPlayback()
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

  async function detachWindow(viewport?: EditorWindowView['viewport'], attach = false): Promise<boolean> {
    if (!sample.value || !state.value || transitioning || loading.value || processing.value || saving.value || decide) return false
    transitioning = true
    loading.value = true
    try {
      const view: EditorWindowView = { cursor: playhead.value ?? cursor.value,
        selection: selection.value && { ...selection.value }, looping: looping.value, curve: curve.value, viewport }
      await stopPlayback()
      if (attach) await editorApi.attach(view)
      else await editorApi.detach(view)
      // The native session now belongs to the new window. Do not close it or
      // prompt to save: those actions belong to that window from this point on.
      sample.value = null
      state.value = null
      selection.value = null
      cursor.value = 0
      expanded.value = false
      windowView.value = null
      sessionVersion.value++
      if (attach) await getCurrentWindow().destroy()
      return true
    } catch (error) {
      toast.error(String(error))
      return false
    } finally { loading.value = false; transitioning = false }
  }
  /** Stage the destination UI before accepting native ownership. */
  async function adoptWindow(token: number, view: EditorWindowView, reportError = true): Promise<boolean> {
    // A duplicate delivery is not a competing offer and must not cancel it.
    if (pendingWindowToken === token) return false
    if (transitioning || loading.value || processing.value || saving.value || sample.value || state.value || decide) {
      await editorApi.rejectTransfer(token).catch(error => { if (reportError) toast.error(String(error)) })
      return false
    }
    transitioning = true
    loading.value = true
    pendingWindowToken = token
    try {
      await acceptEditorTransfer(token, editorApi, {
        stage: async (r) => {
          const restored = readEditorWindowView(`?detached=1&view=${encodeURIComponent(JSON.stringify(view))}`, r.state.frames)
          state.value = r.state
          sample.value = { id: r.state.sampleId, name: r.name, ext: r.ext, path: r.path, bpm: r.bpm }
          cursor.value = restored?.cursor ?? 0
          selection.value = restored?.selection ?? null
          looping.value = restored?.looping ?? false
          curve.value = restored?.curve ?? 'linear'
          windowView.value = restored
          expanded.value = false
          await nextTick()
        },
        rollback: () => {
          sample.value = null
          state.value = null
          cursor.value = 0
          selection.value = null
          looping.value = false
          windowView.value = null
        },
      })
      sessionVersion.value++
      revision.value++
      void refreshClipboard()
      return true
    } catch (error) {
      if (reportError) toast.error(String(error))
      return false
    } finally { loading.value = false; transitioning = false; pendingWindowToken = null }
  }

  /** Loads the window's file and wires up the title and close confirmation. */
  async function init(id: number) {
    const win = getCurrentWindow()
    await win.onCloseRequested((event) => {
      if (!sample.value) return
      event.preventDefault()
      void closeEmbedded().then(closed => { if (closed) void win.close() })
    })
    error.value = null
    const token = readEditorTransferToken(location.search)
    if (new URLSearchParams(location.search).has('transfer') && token === null) {
      error.value = 'Invalid editor window transfer token'
      return
    }
    // A pending offer survives a reload of its target. Accept first; only a
    // committed window uses the ordinary reload/discard behavior afterward.
    const reloading = open.value || (performance.getEntriesByType('navigation')[0] as PerformanceNavigationTiming | undefined)?.type === 'reload'
    if (!token && reloading && !await reset()) return
    let intent: number | undefined
    try {
      let transferred = false
      if (token) {
        const view = readEditorWindowView(location.search, Number.MAX_SAFE_INTEGER)
        if (!view) {
          await editorApi.rejectTransfer(token).catch(() => {})
          throw new Error('Invalid editor window transfer view')
        }
        transferred = await adoptWindow(token, view, false)
        if (!transferred) {
          if (!reloading) throw new Error('Editor window transfer was not accepted')
          // A reload can interrupt the IPC reply after native acceptance but
          // before URL cleanup. Probe ownership without decoding: only a session
          // already owned by THIS window may be reset. A pending/expired offer
          // still owned by its source fails this probe and remains untouched.
          await editorApi.open(id, false, true)
          if (!await reset()) return
        }
        // A later reload must not replay an already-consumed offer token.
        const url = new URL(location.href)
        for (const key of ['transfer', 'detached', 'view']) url.searchParams.delete(key)
        history.replaceState(history.state, '', url)
      }
      if (!transferred) {
        intent = ++openingIntent
        loading.value = true
        await api.stop()
        if (intent !== openingIntent) return
        const r = await editorApi.open(id, false, !reloading && new URLSearchParams(location.search).get('detached') === '1')
        if (intent !== openingIntent) return
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
    }
    catch (e) {
      if (intent === undefined || intent === openingIntent) error.value = String(e)
    }
    finally {
      if (intent === undefined || intent === openingIntent) loading.value = false
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
  }

  return { resolveUnsaved, openSample, closeEmbedded, reset, detachWindow, adoptWindow, init }
}
