import type { Adjust, Sample } from '@/api'
import { acceptHMRUpdate, defineStore } from 'pinia'
import { computed, ref, watch } from 'vue'
import { api } from '@/api'
import { useSettingsStore } from '@/stores/settings'

export const usePlayerStore = defineStore('player', () => {
  const playingId = ref<number | null>(null)
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
  let playing: Sample | null = null
  let frame = 0

  const syncTarget = computed(() => {
    const s = settings.settings
    return s.syncEnabled && (s.projectBpm || s.projectKey) ? { bpm: s.projectBpm, key: s.projectKey } : null
  })

  // Poll the backend for the playhead while something plays.
  function track() {
    cancelAnimationFrame(frame)
    const tick = async () => {
      const id = playingId.value
      const pos = await api.playbackPosition()
      if (id !== playingId.value)
        return
      if (pos === null) {
        playingId.value = null
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
    try {
      adjust.value = await api.play(sample.id, start, syncTarget.value)
      playing = sample
      playingId.value = sample.id
      position.value = start
      track()
    }
    catch (e) {
      playingId.value = null
      error.value = String(e)
    }
  }

  async function stop() {
    cancelAnimationFrame(frame)
    playingId.value = null
    position.value = 0
    await api.stop()
  }

  async function toggle(sample: Sample) {
    if (playingId.value === sample.id)
      await stop()
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
    if (JSON.stringify(next) !== JSON.stringify(prev) && playing && playingId.value === playing.id)
      play(playing, position.value)
  })

  return { playingId, position, autoplay, error, adjust, syncTarget, play, stop, toggle, seek }
})

if (import.meta.hot)
  import.meta.hot.accept(acceptHMRUpdate(usePlayerStore, import.meta.hot))
