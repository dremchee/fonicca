import { computed, shallowRef } from 'vue'
import { toast } from 'vue-sonner'
import { adjacentPlaybackIndex } from '@/playback-navigation'
import { useDuplicatesStore } from '@/stores/duplicates'
import { useEditorStore } from '@/stores/editor'
import { useLibraryStore } from '@/stores/library'
import { usePlayerStore } from '@/stores/player'

export function useMiniPlayerTransport() {
  const player = usePlayerStore()
  const editor = useEditorStore()
  const library = useLibraryStore()
  const duplicates = useDuplicatesStore()
  const sample = computed(() => player.miniPlayerSample)
  const state = computed(() => {
    if (player.playingId === null) return 'stopped'
    return player.paused || player.pausing ? 'paused' : 'playing'
  })
  const busy = shallowRef(false)

  const copies = computed(() => library.scope.kind === 'duplicates'
    ? duplicates.filteredGroups.flatMap(group => group.copies)
    : null)
  const index = computed(() => {
    const id = sample.value?.id
    if (id == null) return -1
    return copies.value?.findIndex(copy => copy.id === id) ?? library.cachedIndexOf(id)
  })
  const count = computed(() => copies.value?.length ?? library.total)
  const canPrevious = computed(() => adjacentPlaybackIndex(index.value, count.value, -1) !== null)
  const canNext = computed(() => adjacentPlaybackIndex(index.value, count.value, 1) !== null)

  async function run(action: () => Promise<void>) {
    if (busy.value) return
    busy.value = true
    try { await action() }
    catch (error) { toast.error(String(error)) }
    finally { busy.value = false }
  }

  function toggle() {
    const current = sample.value
    if (!current) return
    return run(async () => {
      if (state.value !== 'playing') await editor.stopPlayback()
      if (player.canControl(current.id)) await player.toggle(current)
    })
  }

  function stop() {
    return run(() => player.stop({ keepTrack: true }))
  }

  function move(direction: -1 | 1) {
    const current = sample.value
    if (!current) return
    return run(async () => {
      const list = copies.value
      const target = adjacentPlaybackIndex(index.value, count.value, direction)
      if (target === null) return
      const next = list ? list[target] : await library.adjacentSample(current.id, direction)
      if (!next || !player.canControl(current.id)) return
      await editor.stopPlayback()
      if (player.canControl(current.id)) await player.play(next)
    })
  }

  return { sample, state, busy, canPrevious, canNext, toggle, stop, move }
}
