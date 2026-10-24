import type { Adjust, Sample } from '@/api'
import { acceptHMRUpdate, defineStore } from 'pinia'
import { computed, ref, shallowRef, watch } from 'vue'
import { api } from '@/api'
import { useSettingsStore } from '@/stores/settings'

export const usePlayerStore = defineStore('player', () => {
  const playingId = ref<number | null>(null)
  const paused = ref(false)
  const position = ref(0)
  const settings = useSettingsStore()
  // Stored with the other settings so it survives a restart.
  const autoplay = computed({
    get: () => settings.settings.autoplay,
    set: (autoplay: boolean) => settings.update({ autoplay }),
  })
  const error = ref<string | null>(null)
  /** Change applied to the sample playing now (tempo/key sync). */
  const adjust = ref<Adjust | null>(null)
  // Kept across hot reloads, like the playing id, so the player stays on screen.
  const playing = shallowRef<Sample | null>(import.meta.hot?.data.playing ?? null)
  if (import.meta.hot)
    watch(playing, s => (import.meta.hot!.data.playing = s), { immediate: true })
  /** The sample playing now, so the player can keep showing it once it is no longer selected. */
  const playingSample = computed(() => (playingId.value !== null && playing.value?.id === playingId.value ? playing.value : null))
  let frame = 0
  let tracking = 0
  let needsResync = false

  function isPlaying(id: number) {
    return playingId.value === id && !paused.value
  }

  const syncTarget = computed(() => {
    const s = settings.settings
    return s.syncEnabled && (s.projectBpm || s.projectKey) ? { bpm: s.projectBpm, key: s.projectKey } : null
  })

  // Poll the backend for the playhead while something plays.
  function track() {
    cancelAnimationFrame(frame)
    const version = ++tracking
    const tick = async () => {
      const id = playingId.value
      const pos = await api.playbackPosition()
      if (id !== playingId.value || version !== tracking || paused.value)
        return
      if (pos === null) {
        playingId.value = null
        paused.value = false
        position.value = 0
        return
      }
      position.value = pos
      frame = requestAnimationFrame(tick)
    }
    frame = requestAnimationFrame(tick)
  }

  async function play(sample: Sample, start = 0) {
    error.value = null
    tracking++
    cancelAnimationFrame(frame)
    try {
      adjust.value = await api.play(sample.id, start, syncTarget.value)
      playing.value = sample
      playingId.value = sample.id
      paused.value = false
      needsResync = false
      position.value = start
      track()
    }
    catch (e) {
      playingId.value = null
      paused.value = false
      error.value = String(e)
    }
  }

  /** Applies a change made in the library to the playing sample's copy. */
  function patchPlaying(ids: Set<number>, patch: (s: Sample) => Sample) {
    if (playing.value && ids.has(playing.value.id))
      playing.value = patch(playing.value)
  }

  async function stop() {
    cancelAnimationFrame(frame)
    tracking++
    playingId.value = null
    paused.value = false
    position.value = 0
    await api.stop()
  }

  async function pause() {
    const id = playingId.value
    if (id === null || paused.value)
      return
    cancelAnimationFrame(frame)
    const version = ++tracking
    try {
      const pos = await api.pause()
      if (version !== tracking || id !== playingId.value)
        return
      if (pos === null) {
        playingId.value = null
        position.value = 0
        return
      }
      paused.value = true
      position.value = pos
    }
    catch (e) {
      error.value = String(e)
      if (version === tracking && id === playingId.value)
        track()
    }
  }

  async function resume(sample: Sample) {
    error.value = null
    if (needsResync) {
      await play(sample, position.value)
      return
    }
    const id = playingId.value
    const version = tracking
    try {
      const resumed = await api.resume()
      if (version !== tracking || id !== playingId.value)
        return
      if (!resumed) {
        await play(sample, position.value)
        return
      }
      paused.value = false
      track()
    }
    catch (e) {
      error.value = String(e)
    }
  }

  async function toggle(sample: Sample) {
    if (playingId.value === sample.id && paused.value)
      await resume(sample)
    else if (playingId.value === sample.id)
      await pause()
    else
      await play(sample)
  }

  async function seek(sample: Sample, pos: number) {
    if (playingId.value === sample.id) {
      position.value = pos
      await api.seek(pos)
    }
    else {
      await play(sample, pos)
    }
  }

  // Changing the project tempo/key restarts the playing sample where it was.
  watch(syncTarget, (next, prev) => {
    if (JSON.stringify(next) !== JSON.stringify(prev) && playingSample.value) {
      if (paused.value)
        needsResync = true
      else
        play(playingSample.value, position.value)
    }
  })

  return { playingId, paused, isPlaying, playingSample, position, autoplay, error, adjust, syncTarget, play, stop, pause, toggle, seek, patchPlaying }
})

if (import.meta.hot)
  import.meta.hot.accept(acceptHMRUpdate(usePlayerStore, import.meta.hot))
