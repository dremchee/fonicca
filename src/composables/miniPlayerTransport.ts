import { computed, shallowRef } from 'vue'
import { toast } from 'vue-sonner'
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
  const active = computed(() => player.playingId !== null && !player.paused)
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
  const canPrevious = computed(() => count.value > 0 && index.value !== 0)
  const canNext = computed(() => count.value > 0 && (index.value < 0 || index.value + 1 < count.value))

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
      if (!active.value) await editor.stopPlayback()
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
      const target = index.value < 0 ? (direction === 1 ? 0 : count.value - 1) : index.value + direction
      const next = list ? list[target] : await library.adjacentSample(current.id, direction)
      if (!next || !player.canControl(current.id)) return
      await editor.stopPlayback()
      if (player.canControl(current.id)) await player.play(next)
    })
  }

  return { sample, active, busy, canPrevious, canNext, toggle, stop, move }
}
