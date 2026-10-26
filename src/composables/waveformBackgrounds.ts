import type { Sample } from '@/api'
import type { Wave } from '@/stores/editor'
import { createGlobalState } from '@vueuse/core'
import { shallowReactive } from 'vue'
import { api } from '@/api'
import { parseWave } from '@/waveform'
import { waveformBackground } from '@/waveform-background'

export const useWaveformBackgrounds = createGlobalState(() => {
  const cache = shallowReactive(new Map<number, { stamp: string, path: string }>())
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
      // Read an existing overview only; a miss must never start decoding audio.
      void api.sampleWave(id, 256, false, true).then((bytes) => {
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
  function restore(samples: Sample[]) {
    queue.clear()
    for (const sample of samples) {
      if (!pathFor(sample) && !misses.has(stamp(sample)) && !pending.has(sample.id))
        queue.set(sample.id, sample)
    }
    pump()
  }
  return { remember, pathFor, restore }
})
