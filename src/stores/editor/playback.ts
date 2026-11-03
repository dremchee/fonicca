import { toast } from 'vue-sonner'
import { api } from '@/api'
import { editorApi } from './api'
import type { EditorStoreState } from './state'

interface Options {
  context: Pick<EditorStoreState, 'state' | 'cursor' | 'selection' | 'looping' | 'playing' | 'playhead'>
  stopLibraryPlayback: () => Promise<void>
}

/** Owns playback request generations and cursor polling for one editor. */
export function createEditorPlayback(options: Options) {
  const { state, cursor, selection, looping, playing, playhead } = options.context
  const { stopLibraryPlayback } = options
  let frame = 0
  let playbackVersion = 0
  let startingPlayback = false

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
    startingPlayback = true
    cancelAnimationFrame(frame)
    const s = selection.value
    const [start, end] = s ? [s.start, s.end] : [cursor.value, state.value.frames]
    try {
      await stopLibraryPlayback()
      if (version !== playbackVersion) return
      await editorApi.play(start, end, looping.value && !!s)
      if (version !== playbackVersion) return
      playing.value = true
      playhead.value = start
      track(version)
    }
    catch (e) {
      if (version === playbackVersion) {
        playing.value = false
        playhead.value = null
        toast.error(String(e))
      }
    }
    finally { if (version === playbackVersion) startingPlayback = false }
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
    catch (e) { if (version === playbackVersion) toast.error(String(e)) }
  }

  function stopPlayback() {
    playbackVersion++
    cancelAnimationFrame(frame)
    const stopped = playing.value || startingPlayback ? api.stop() : Promise.resolve()
    startingPlayback = false
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

  return { play, stopPlayback, togglePlay, updatePlaybackRange }
}
