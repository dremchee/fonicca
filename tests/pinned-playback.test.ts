import assert from 'node:assert/strict'
import { test } from 'node:test'
import { ref, shallowReactive } from 'vue'
import { usePinnedPlayback } from '../src/composables/pinnedPlayback.ts'

test('loaded playing row pins only when its entire virtual slot leaves the top', () => {
  const row = { id: 9 }
  const pages = shallowReactive(new Map([[2, [{ id: 8 }, row]]]))
  const playingId = ref<number | null>(9), scrollTop = ref(0)
  const pin = usePinnedPlayback({ pages, playingId, scrollTop, pageSize: 200, rowHeight: 36 })
  assert.equal(pin.playingIndex.value, 401)
  assert.equal(pin.pinnedSample.value, null)
  scrollTop.value = 402 * 36 - .01
  assert.equal(pin.pinnedSample.value, null)
  scrollTop.value += .01
  assert.equal(pin.pinnedSample.value, row)
  scrollTop.value = 0
  assert.equal(pin.pinnedSample.value, null)
  playingId.value = null
  assert.equal(pin.playingIndex.value, -1)
})

test('stems pin as their own row, updates and reordered pages stay current', () => {
  const child = { id: 2, stemParentId: 1 }
  const pages = shallowReactive(new Map([[0, [{ id: 1 }, child]]]))
  const playingId = ref<number | null>(child.id), scrollTop = ref(1000)
  const pin = usePinnedPlayback({ pages: () => pages, playingId: () => playingId.value, scrollTop, pageSize: 200, rowHeight: 36 })
  assert.equal(pin.pinnedSample.value, child)
  const refreshed = { ...child, name: 'Updated stem' }
  pages.set(0, [refreshed, { id: 1 }])
  assert.equal(pin.playingIndex.value, 0)
  assert.equal(pin.pinnedSample.value, refreshed)
  pages.clear()
  assert.equal(pin.playingIndex.value, -1)
  assert.equal(pin.pinnedSample.value, null)
  // An unloaded/filtered playing track never creates a synthetic row or fetch.
  playingId.value = 99
  assert.equal(pin.pinnedSample.value, null)
  assert.equal(pages.size, 0)
})
