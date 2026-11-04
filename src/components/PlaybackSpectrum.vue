<script setup lang="ts">
import type { PlaybackLevels } from '@/api'
import { usePreferredReducedMotion, useRafFn } from '@vueuse/core'
import { computed, onScopeDispose, shallowRef, watch } from 'vue'
import { api } from '@/api'

const props = defineProps<{ active: boolean }>()
const emit = defineEmits<{ glow: [value: { color: string, intensity: number }] }>()
const levels = shallowRef<number[]>(Array.from({ length: 16 }, () => 0))
const intensity = shallowRef(0)
const reducedMotion = usePreferredReducedMotion()
const spectrumColor = 'var(--primary)'
// A denser drawing of the same logarithmic spectrum, without extra audio reads.
const DISPLAY_BANDS = 64
const displayLevels = computed(() => Array.from({ length: DISPLAY_BANDS }, (_, index) => {
  const position = index * (levels.value.length - 1) / (DISPLAY_BANDS - 1)
  const left = Math.floor(position)
  const start = levels.value[left] ?? 0
  const end = levels.value[Math.min(left + 1, levels.value.length - 1)] ?? 0
  return start + (end - start) * (position - left)
}))
let pending = false
let generation = 0
let disposed = false

// Poll only while auditioning; keep a short release tail after a pause.
const { pause, resume } = useRafFn(async ({ delta }) => {
  if (pending) return
  pending = true
  const version = generation
  let meter: PlaybackLevels | null = null
  try {
    if (props.active) meter = await api.playbackLevels(true)
  }
  catch { /* A missing meter must not interrupt playback. */ }
  finally { pending = false }
  if (disposed || version !== generation) return
  const release = Math.min(delta, 100) / 350
  levels.value = levels.value.map((level, i) => {
    const amplitude = meter?.spectrum?.[i] ?? 0
    const next = amplitude > 0 ? Math.max(0, Math.min(1, (20 * Math.log10(amplitude) + 60) / 60)) : 0
    return Math.max(next, level - release)
  })
  // Use actual output RMS, including the player's volume, for the glow envelope.
  const rms = meter?.rms.length ? Math.sqrt(meter.rms.reduce((sum, value) => sum + value ** 2, 0) / meter.rms.length) : 0
  const target = rms > 0 ? Math.max(0, Math.min(1, (20 * Math.log10(rms) + 48) / 42)) : 0
  const smoothing = 1 - Math.exp(-Math.min(delta, 100) / (target > intensity.value ? 90 : 380))
  intensity.value = reducedMotion.value === 'reduce' ? 0 : intensity.value + (target - intensity.value) * smoothing
  if (target === 0 && intensity.value < 0.002) intensity.value = 0
  emit('glow', { color: spectrumColor, intensity: intensity.value })
  if (!props.active && levels.value.every(level => level === 0) && intensity.value === 0) pause()
}, { immediate: false, fpsLimit: 30 })

watch(() => props.active, () => {
  generation++
  resume()
}, { immediate: true })
onScopeDispose(() => { disposed = true })
</script>

<template>
  <div class="playback-spectrum" aria-hidden="true">
    <svg class="spectrum-bars" :style="{ color: spectrumColor }" :viewBox="`0 0 ${DISPLAY_BANDS * 4 - 2} 18`" preserveAspectRatio="none" fill="currentColor">
      <rect v-for="(level, index) in displayLevels" :key="index" :x="index * 4" :y="18 - Math.max(1, level * 18)" width="2" :height="Math.max(1, level * 18)" rx="0.6" />
    </svg>
  </div>
</template>

<style scoped>
.playback-spectrum { position: relative; isolation: isolate; display: block; width: 100%; height: 14px; color: var(--primary); pointer-events: none; }
.spectrum-bars { position: relative; display: block; width: 100%; height: 100%; }
</style>
