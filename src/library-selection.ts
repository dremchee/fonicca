import type { Ref } from 'vue'

interface Options<Row extends { id: number }> {
  focusIndex: Ref<number>
  anchorIndex: Ref<number>
  selectedIds: Set<number>
  total: () => number
  identity: () => string
  sampleAt: (index: number) => Row | undefined
  ensureRange: (start: number, end: number) => Promise<void>
  rangeIds: (a: number, b: number) => Promise<number[]>
  allIds: () => Promise<number[]>
  remember: (sample: Row, index: number) => void
}

/** Selection owns IDs, not cached pages. Late requests never change a new scope
 * or overwrite a newer keyboard/mouse selection. */
export function createLibrarySelection<Row extends { id: number }>(options: Options<Row>) {
  const { focusIndex, anchorIndex, selectedIds, total, identity, sampleAt, ensureRange, rangeIds, allIds, remember } = options
  let revision = 0
  function request() {
    const current = ++revision, scope = identity()
    return () => current === revision && scope === identity()
  }
  function setSelection(ids: Iterable<number>) {
    selectedIds.clear()
    for (const id of ids) selectedIds.add(id)
  }
  async function clickRow(index: number, { shift = false, toggle = false } = {}) {
    const current = request(), sample = sampleAt(index)
    if (!sample) return
    if (shift && anchorIndex.value >= 0) {
      const ids = await rangeIds(anchorIndex.value, index)
      if (!current()) return
      setSelection(ids)
    } else if (toggle) {
      if (selectedIds.has(sample.id)) selectedIds.delete(sample.id)
      else selectedIds.add(sample.id)
      anchorIndex.value = index
    } else {
      setSelection([sample.id])
      anchorIndex.value = index
    }
    focusIndex.value = index
    remember(sample, index)
  }
  async function moveFocus(delta: number, extend = false) {
    const current = request()
    if (total() === 0) return
    const index = focusIndex.value < 0 ? 0 : Math.max(0, Math.min(total() - 1, focusIndex.value + delta))
    focusIndex.value = index
    await ensureRange(index, index)
    if (!current()) return
    if (extend && anchorIndex.value >= 0) {
      const ids = await rangeIds(anchorIndex.value, index)
      if (!current()) return
      setSelection(ids)
    } else {
      const sample = sampleAt(index)
      setSelection(sample ? [sample.id] : [])
      anchorIndex.value = index
    }
    const sample = sampleAt(index)
    if (sample) remember(sample, index)
  }
  async function selectAll() {
    const current = request(), ids = await allIds()
    if (current()) setSelection(ids)
  }
  function targetIds() {
    if (selectedIds.size) return [...selectedIds]
    const focused = sampleAt(focusIndex.value)
    return focused ? [focused.id] : []
  }
  return { clickRow, moveFocus, selectAll, setSelection, targetIds, version: () => revision }
}
