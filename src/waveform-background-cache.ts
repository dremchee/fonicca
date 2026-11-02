import type { Sample } from './api.ts'
import type { Wave } from './stores/editor.ts'
import { shallowReactive } from 'vue'
import { parseWave } from './waveform.ts'
import { waveformBackground } from './waveform-background.ts'

type OverviewSample = Pick<Sample, 'id' | 'path' | 'size'>
type ReadOverview = (id: number, columns: number, split: boolean, cachedOnly?: boolean) => Promise<ArrayBuffer>
interface Request { stamp: string, valid: boolean, done: Promise<void> }

/** Shared row silhouettes; reads are injected so scheduling can be checked without IPC. */
export function createWaveformBackgroundCache(read: ReadOverview) {
  const cache = shallowReactive(new Map<number, { stamp: string, path: string }>())
  const misses = new Map<number, string>()
  const pending = new Map<number, Request>()
  const generating = new Map<number, Request>()
  const queue = new Map<number, OverviewSample>()
  const stamp = (sample: OverviewSample) => `${sample.path}:${sample.size}`

  function invalidateRequests(id: number) {
    for (const pool of [pending, generating]) {
      const request = pool.get(id)
      if (request) request.valid = false
    }
  }
  function invalidate(id: number) {
    cache.delete(id)
    misses.delete(id)
    queue.delete(id)
    // Only live requests need a cancellation flag; idle IDs retain no tombstone.
    invalidateRequests(id)
  }
  function supersede(sample: OverviewSample) {
    for (const pool of [pending, generating]) {
      const request = pool.get(sample.id)
      if (request && request.stamp !== stamp(sample)) request.valid = false
    }
  }
  function store(sample: OverviewSample, wave: Wave) {
    queue.delete(sample.id)
    misses.delete(sample.id)
    cache.delete(sample.id)
    cache.set(sample.id, { stamp: stamp(sample), path: waveformBackground(wave) })
    while (cache.size > 128) cache.delete(cache.keys().next().value!)
  }
  function remember(sample: OverviewSample, wave: Wave) {
    // A generated/directly supplied wave wins over any older disk lookup.
    invalidateRequests(sample.id)
    store(sample, wave)
  }
  function pathFor(sample: OverviewSample) {
    const entry = cache.get(sample.id)
    return entry?.stamp === stamp(sample) ? entry.path : undefined
  }
  function request(sample: OverviewSample, cachedOnly: boolean) {
    const pool = cachedOnly ? pending : generating
    const task: Request = { stamp: stamp(sample), valid: true, done: Promise.resolve() }
    pool.set(sample.id, task)
    task.done = Promise.resolve().then(() => read(sample.id, 256, false, cachedOnly)).then((bytes) => {
      if (!task.valid) return
      if (bytes.byteLength) {
        // Disk restoration is lower priority: it may display while generation
        // runs, but must never cancel the newer generated result.
        if (cachedOnly) store(sample, parseWave(bytes))
        else remember(sample, parseWave(bytes))
      }
      else if (cachedOnly) {
        misses.delete(sample.id)
        misses.set(sample.id, task.stamp)
        while (misses.size > 256) misses.delete(misses.keys().next().value!)
      }
    }).catch((error) => {
      if (!cachedOnly && task.valid) console.error('waveform preview failed', error)
    }).finally(() => {
      if (pool.get(sample.id) === task) pool.delete(sample.id)
      pump()
    })
    return task.done
  }
  function pump() {
    while (pending.size < 2 && queue.size) {
      // Invalidated requests still occupy their slot until IPC really finishes.
      let next: [number, OverviewSample] | undefined
      for (const entry of queue) {
        if (!pending.has(entry[0]) && !generating.has(entry[0])) { next = entry; break }
      }
      if (!next) break
      const [id, sample] = next
      queue.delete(id)
      if (!pathFor(sample) && misses.get(id) !== stamp(sample)) void request(sample, true)
    }
  }
  async function prepare(sample: OverviewSample): Promise<void> {
    if (pathFor(sample)) return
    supersede(sample)
    const current = generating.get(sample.id)
    if (current) {
      if (current.valid) return current.done
      await current.done
      return prepare(sample)
    }
    return request(sample, false)
  }
  function restore(samples: OverviewSample[]) {
    queue.clear()
    for (const sample of samples) {
      supersede(sample)
      if (!pathFor(sample) && misses.get(sample.id) !== stamp(sample)
        && !pending.get(sample.id)?.valid && !generating.get(sample.id)?.valid)
        queue.set(sample.id, sample)
    }
    pump()
  }
  return { remember, pathFor, prepare, restore, invalidate,
    // Counts only: diagnostics must not retain request, sample or waveform objects.
    inspect: () => ({ cached: cache.size, misses: misses.size, pending: pending.size,
      generating: generating.size, queued: queue.size,
      invalidations: [...pending.values(), ...generating.values()].filter(task => !task.valid).length }),
  }
}
