import assert from 'node:assert/strict'
import { setImmediate } from 'node:timers/promises'
import { test } from 'node:test'
import { createLibraryPages } from '../src/library-pages.ts'

const rows = (page: number) => Array.from({ length: 200 }, (_, i) => ({ id: page * 200 + i }))

test('scrolling 100k rows retains a bounded cache and preserves the focused page', async () => {
  const pages = new Map<number, { id: number }[]>()
  const cache = createLibraryPages({ pages, fetch: async page => rows(page), protectedPages: () => [0] })
  await cache.ensureRange(0, 0)
  for (let page = 1; page < 500; page++) {
    await cache.ensureVisibleRange(page * 200, page * 200 + 30)
    assert.ok(pages.size <= 12)
    assert.equal(cache.sampleAt(0)?.id, 0)
  }
  assert.equal(cache.diagnostics().retainedRows, 2400)
  assert.equal(cache.sampleAt(99999)?.id, 99999)
})

test('concurrency is bounded, duplicate requests coalesce and obsolete scroll requests are skipped', async () => {
  const finish = new Map<number, (rows: { id: number }[]) => void>()
  const called: number[] = []
  const cache = createLibraryPages({ pages: new Map<number, { id: number }[]>(), concurrency: 1,
    fetch: page => { called.push(page); return new Promise(resolve => { finish.set(page, resolve) }) } })
  const first = cache.ensureVisibleRange(0, 5)
  await setImmediate()
  const obsolete = cache.ensureVisibleRange(200, 205)
  const latest = cache.ensureVisibleRange(400, 405)
  const duplicate = cache.ensureRange(400, 405)
  assert.deepEqual(called, [0])
  assert.equal(cache.diagnostics().queued, 1)
  finish.get(0)!(rows(0))
  await setImmediate()
  assert.deepEqual(called, [0, 2])
  finish.get(2)!(rows(2))
  await Promise.all([first, obsolete, latest, duplicate])
})

test('old scope responses cannot populate or delete new pending pages; failed reads retry', async () => {
  const resolvers: ((rows: { id: number }[]) => void)[] = []
  const pages = new Map<number, { id: number }[]>()
  const cache = createLibraryPages({ pages, fetch: () => new Promise(resolve => { resolvers.push(resolve) }) })
  const old = cache.ensureRange(0, 0)
  await setImmediate()
  cache.clear()
  const current = cache.ensureRange(0, 0)
  await setImmediate()
  resolvers[0]!(rows(9))
  await old
  assert.equal(cache.sampleAt(0), undefined)
  const same = cache.ensureRange(0, 0)
  resolvers[1]!(rows(0))
  await Promise.all([current, same])
  assert.equal(cache.sampleAt(0)?.id, 0)
  let attempts = 0
  const retry = createLibraryPages({ pages: new Map<number, { id: number }[]>(), fetch: async page => {
    if (++attempts === 1) throw new Error('temporary')
    return rows(page)
  } })
  await assert.rejects(retry.ensureRange(0, 0), /temporary/)
  await retry.ensureRange(0, 0)
  assert.equal(retry.sampleAt(0)?.id, 0)
})

test('background refreshes never resurrect evicted pages and disposal drops late results', async () => {
  const pages = new Map<number, { id: number }[]>()
  const cache = createLibraryPages({ pages, maxPages: 1, protectedPages: () => [0], fetch: async page => rows(page) })
  await cache.ensureRange(0, 0)
  await cache.ensureVisibleRange(200, 201)
  assert.equal(pages.size, 1)
  assert.ok(pages.has(0))
  await cache.refreshCached(1)
  assert.equal(pages.size, 1)
  let finish!: (rows: { id: number }[]) => void
  const pending = createLibraryPages({ pages: new Map<number, { id: number }[]>(), fetch: () => new Promise(resolve => { finish = resolve }) })
  const request = pending.ensureRange(0, 0)
  await setImmediate()
  pending.dispose()
  finish(rows(0))
  await request
  assert.equal(pending.sampleAt(0), undefined)
})

test('background refresh shares request limits and does not resurrect evicted pages', async () => {
  let finish!: (rows: { id: number }[]) => void
  const pages = new Map([[0, rows(0)], [1, rows(1)]])
  const cache = createLibraryPages({ pages, maxPages: 2, concurrency: 1,
    fetch: () => new Promise(resolve => { finish = resolve }) })
  const old = cache.refreshCached(0)
  const next = cache.refreshCached(1)
  await setImmediate()
  assert.equal(cache.diagnostics().active, 1)
  assert.equal(cache.diagnostics().queued, 1)
  pages.delete(0)
  finish(rows(9))
  await old
  assert.equal(pages.has(0), false)
  await setImmediate()
  finish(rows(8))
  await next
  assert.equal(cache.sampleAt(200)?.id, 1600)
})
