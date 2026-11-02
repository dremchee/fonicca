import { computed, toValue, type MaybeRefOrGetter } from 'vue'

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
    if (id === null) return null
    for (const [page, samples] of toValue(options.pages)) {
      const offset = samples.findIndex(sample => sample.id === id)
      if (offset >= 0) return { index: page * options.pageSize + offset, sample: samples[offset]! }
    }
    return null
  })
  return {
    playingIndex: computed(() => playing.value?.index ?? -1),
    pinnedSample: computed(() => {
      const row = playing.value
      return row && toValue(options.scrollTop) >= (row.index + 1) * options.rowHeight ? row.sample : null
    }),
  }
}
