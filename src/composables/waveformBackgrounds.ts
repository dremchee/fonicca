import type { Sample } from '@/api'
import type { Wave } from '@/stores/editor'
import { createGlobalState } from '@vueuse/core'
import { shallowReactive } from 'vue'
import { api } from '@/api'
import { parseWave } from '@/waveform'
import { waveformBackground } from '@/waveform-background'

export const useWaveformBackgrounds = createGlobalState(() => {
  const cache = shallowReactive(new Map<number, { stamp: string, path: string }>())
  const versions = new Map<number, number>()
  function invalidate(id: number) {
    cache.delete(id)
    versions.set(id, (versions.get(id) ?? 0) + 1)
    misses.clear()
  }
  const stamp = (sample: Sample) => `${sample.path}:${sample.size}`
  function remember(sample: Sample, wave: Wave) {
    cache.delete(sample.id)
    cache.set(sample.id, { stamp: stamp(sample), path: waveformBackground(wave) })
    while (cache.size > 128) cache.delete(cache.keys().next().value!)
  }
  function pathFor(sample: Sample) {
    const entry = cache.get(sample.id)
    return entry?.stamp === stamp(sample) ? entry.path : undefined
  }
  const misses = new Set<string>()
  const pending = new Set<number>()
  const queue = new Map<number, Sample>()
  function pump() {
    while (pending.size < 2 && queue.size) {
      const [id, sample] = queue.entries().next().value!
      queue.delete(id)
      pending.add(id)
      const version = versions.get(id)
      // Read an existing overview only; a miss must never start decoding audio.
      void api.sampleWave(id, 256, false, true).then((bytes) => {
        if (version !== versions.get(id)) return
        if (bytes.byteLength) remember(sample, parseWave(bytes))
        else {
          misses.add(stamp(sample))
          while (misses.size > 256) misses.delete(misses.values().next().value!)
        }
      }).catch(() => {}).finally(() => {
        pending.delete(id)
        pump()
      })
    }
  }
  const generating = new Set<number>()
  async function prepare(sample: Sample) {
    if (pathFor(sample) || generating.has(sample.id)) return
    generating.add(sample.id)
    const version = versions.get(sample.id)
    try {
      const bytes = await api.sampleWave(sample.id, 256, false)
      if (bytes.byteLength && version === versions.get(sample.id)) {
        remember(sample, parseWave(bytes))
        misses.delete(stamp(sample))
      }
    }
    catch (error) { console.error('waveform preview failed', error) }
    finally { generating.delete(sample.id) }
  }
  function restore(samples: Sample[]) {
    queue.clear()
    for (const sample of samples) {
      if (!pathFor(sample) && !misses.has(stamp(sample)) && !pending.has(sample.id))
        queue.set(sample.id, sample)
    }
    pump()
  }
  return { remember, pathFor, prepare, restore, invalidate }
})
