import assert from 'node:assert/strict'
import { setImmediate } from 'node:timers/promises'
import { test } from 'node:test'
import { createWaveformBackgroundCache } from '../src/waveform-background-cache.ts'

const sample = (id: number, size = 100) => ({ id, path: `/mock/${id}.wav`, size })
const bytes = (peak: number) => {
  const buffer = new ArrayBuffer(16 + 12)
  new Uint32Array(buffer, 0, 4).set([0, 1, 1, 6])
  new Int16Array(buffer, 16).set([-peak, peak, peak / 2, 0, 0, 0])
  return buffer
}
const settle = () => setImmediate()
function deferredReads() {
  const calls: { id: number, cachedOnly: boolean, resolve: (value: ArrayBuffer) => void }[] = []
  const cache = createWaveformBackgroundCache((id, columns, split, cachedOnly = false) => {
    assert.equal(columns, 256)
    assert.equal(split, false)
    return new Promise(resolve => calls.push({ id, cachedOnly, resolve }))
  })
  return { cache, calls }
}

test('restoring rows reads existing overviews only, with at most two requests', async () => {
  const { cache, calls } = deferredReads()
  cache.restore([sample(1), sample(2), sample(3)])
  await settle()
  assert.equal(calls.length, 2)
  assert.ok(calls.every(call => call.cachedOnly))
  calls[0]!.resolve(new ArrayBuffer(0))
  await settle()
  assert.equal(calls.length, 3)
  calls[1]!.resolve(bytes(1000)); calls[2]!.resolve(bytes(2000))
  await settle()
  cache.restore([sample(1), sample(2), sample(3)])
  await settle()
  assert.equal(calls.length, 3)
  assert.ok(cache.pathFor(sample(2)))
})

test('a late cached-only overview cannot overwrite a newly generated silhouette', async () => {
  const { cache, calls } = deferredReads()
  cache.restore([sample(1)])
  const prepare = cache.prepare(sample(1))
  await settle()
  const generated = calls.find(call => !call.cachedOnly)!
  generated.resolve(bytes(24000))
  await prepare
  const fresh = cache.pathFor(sample(1))
  assert.ok(fresh)
  calls.find(call => call.cachedOnly)!.resolve(bytes(1000))
  await settle()
  assert.equal(cache.pathFor(sample(1)), fresh)
})

test('invalidating many idle tracks does not retain a per-track version history', () => {
  let reads = 0
  const cache = createWaveformBackgroundCache(async () => { reads++; return new ArrayBuffer(0) })
  for (let id = 0; id < 100_000; id++) cache.invalidate(id)
  assert.equal(reads, 0)
  assert.equal(cache.inspect().invalidations, 0)
})

test('restore during an invalidated read retries the edited track without another scroll', async () => {
  const { cache, calls } = deferredReads()
  cache.restore([sample(1)])
  await settle()
  cache.invalidate(1)
  cache.restore([sample(1, 200)])
  calls[0]!.resolve(bytes(1000))
  await settle()
  assert.equal(cache.pathFor(sample(1, 200)), undefined)
  assert.equal(calls.length, 2)
  calls[1]!.resolve(bytes(20000))
  await settle()
  assert.ok(cache.pathFor(sample(1, 200)))
  assert.equal(cache.pathFor(sample(1)), undefined)
})

test('rapid scrolling replaces queued lookups and never promotes a miss to a decode', async () => {
  const { cache, calls } = deferredReads()
  cache.restore([sample(1), sample(2), sample(3), sample(4)])
  await settle()
  cache.restore([sample(10), sample(11)])
  calls[0]!.resolve(new ArrayBuffer(0)); calls[1]!.resolve(new ArrayBuffer(0))
  await settle()
  assert.deepEqual(calls.map(call => call.id), [1, 2, 10, 11])
  assert.ok(calls.every(call => call.cachedOnly))
  calls[2]!.resolve(new ArrayBuffer(0)); calls[3]!.resolve(new ArrayBuffer(0))
  await settle()
  assert.equal(cache.inspect().pending, 0)
  assert.equal(cache.inspect().queued, 0)
})

test('generation is deduplicated and invalidating it allows a fresh prepare after completion', async () => {
  const { cache, calls } = deferredReads()
  const first = cache.prepare(sample(1))
  const duplicate = cache.prepare(sample(1))
  await settle()
  assert.equal(calls.length, 1)
  cache.invalidate(1)
  const retry = cache.prepare(sample(1))
  await settle()
  assert.equal(calls.length, 1, 'invalidated work still occupies its slot')
  calls[0]!.resolve(bytes(1000))
  await first; await duplicate; await settle()
  assert.equal(cache.pathFor(sample(1)), undefined)
  assert.equal(calls.length, 2)
  calls[1]!.resolve(bytes(24000))
  await retry
  assert.ok(cache.pathFor(sample(1)))
  assert.equal(cache.inspect().generating, 0)
  assert.equal(cache.inspect().invalidations, 0)
})

test('editing one track does not erase unrelated cached-only misses', async () => {
  const { cache, calls } = deferredReads()
  cache.restore([sample(1), sample(2)])
  await settle()
  for (const call of calls) call.resolve(new ArrayBuffer(0))
  await settle()
  cache.invalidate(1)
  cache.restore([sample(1), sample(2)])
  await settle()
  assert.deepEqual(calls.map(call => call.id), [1, 2, 1])
  calls[2]!.resolve(new ArrayBuffer(0))
  await settle()
})

test('positive and negative caches remain bounded across large libraries', async () => {
  let reads = 0
  const cache = createWaveformBackgroundCache(async id => { reads++; return id < 200 ? bytes(10000) : new ArrayBuffer(0) })
  for (let id = 0; id < 200; id++) {
    cache.restore([sample(id)])
    await settle()
  }
  assert.equal(cache.inspect().cached, 128)
  assert.equal(cache.pathFor(sample(0)), undefined)
  assert.ok(cache.pathFor(sample(199)))
  for (let id = 200; id < 1200; id++) {
    cache.restore([sample(id)])
    await settle()
  }
  assert.equal(reads, 1200)
  assert.deepEqual(cache.inspect(), { cached: 128, misses: 256, pending: 0, generating: 0, queued: 0, invalidations: 0 })
  cache.restore([sample(1199)])
  await settle()
  assert.equal(reads, 1200)
  cache.restore([sample(200)])
  await settle()
  assert.equal(reads, 1201, 'an evicted miss must be eligible for another lookup')
})

test('an early cached-only response does not cancel the fresher generation', async () => {
  const { cache, calls } = deferredReads()
  cache.restore([sample(1)])
  const fresh = cache.prepare(sample(1))
  await settle()
  calls.find(call => call.cachedOnly)!.resolve(bytes(1000))
  await settle()
  const old = cache.pathFor(sample(1))
  calls.find(call => !call.cachedOnly)!.resolve(bytes(24000))
  await fresh
  assert.ok(cache.pathFor(sample(1)))
  assert.notEqual(cache.pathFor(sample(1)), old)
})

test('invalidating a queued row prevents its stale lookup from starting', async () => {
  const { cache, calls } = deferredReads()
  cache.restore([sample(1), sample(2), sample(3)])
  await settle()
  cache.invalidate(3)
  calls[0]!.resolve(new ArrayBuffer(0)); calls[1]!.resolve(new ArrayBuffer(0))
  await settle()
  assert.deepEqual(calls.map(call => call.id), [1, 2])
  assert.equal(cache.inspect().queued, 0)
})

test('a failed cached lookup releases its slot and can retry', async () => {
  let reads = 0
  const cache = createWaveformBackgroundCache(async () => {
    if (++reads === 1) throw Error('temporary lookup failure')
    return bytes(24000)
  })
  cache.restore([sample(1)])
  await settle()
  assert.equal(cache.inspect().pending, 0)
  assert.equal(cache.inspect().misses, 0, 'an IPC failure is not a known disk-cache miss')
  cache.restore([sample(1)])
  await settle()
  assert.equal(reads, 2)
  assert.ok(cache.pathFor(sample(1)))
})
