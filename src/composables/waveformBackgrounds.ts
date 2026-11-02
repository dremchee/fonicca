import { createGlobalState } from '@vueuse/core'
import { api } from '@/api'
import { createWaveformBackgroundCache } from '@/waveform-background-cache'

export const useWaveformBackgrounds = createGlobalState(() => {
  const { remember, pathFor, prepare, restore, invalidate } = createWaveformBackgroundCache(api.sampleWave)
  return { remember, pathFor, prepare, restore, invalidate }
})
