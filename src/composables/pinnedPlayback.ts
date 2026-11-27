import { computed, toValue, type MaybeRefOrGetter } from 'vue'
import { findCachedRow } from '../library-pages.ts'

/** Pin only a loaded current row after its whole virtual slot leaves the top.
 * Does not fetch pages or change the virtualizer's range (including for stems).
 */
export function usePinnedPlayback<T extends { id: number }>(options: {
  pages: MaybeRefOrGetter<ReadonlyMap<number, readonly T[]>>
  playingId: MaybeRefOrGetter<number | null>
  scrollTop: MaybeRefOrGetter<number>
  pageSize: number
  rowHeight: number
}) {
  const playing = computed(() => {
    const id = toValue(options.playingId)
    return id === null ? null : findCachedRow(toValue(options.pages), options.pageSize, id)
  })
  return {
    playingIndex: computed(() => playing.value?.index ?? -1),
    pinnedSample: computed(() => {
      const row = playing.value
      return row && toValue(options.scrollTop) >= (row.index + 1) * options.rowHeight ? row.sample : null
    }),
  }
}
