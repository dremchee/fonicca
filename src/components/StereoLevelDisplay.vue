<script setup lang="ts">
import { computed } from 'vue'

const props = defineProps<{
  variant: 'arcs' | 'dots'
  channels: readonly { level: number, peak: number, hold: number }[]
  clipped: boolean
}>()
const dots = Array.from({ length: 12 }, (_, i) => (i + 1) / 12)
const rows = computed(() => props.channels.map(channel => ({
  ...channel,
  marker: {
    x: 26 - 19 * Math.cos(Math.PI * channel.hold),
    y: 27 - 19 * Math.sin(Math.PI * channel.hold),
  },
  heldDot: channel.hold > 0 ? Math.max(0, Math.ceil(channel.hold * dots.length) - 1) : -1,
})))
</script>

<template>
  <svg class="stereo-display" :class="`display-${variant}`" viewBox="0 0 108 36" fill="none" aria-hidden="true">
    <template v-if="variant === 'arcs'">
      <g v-for="(channel, index) in rows" :key="index" :transform="`translate(${index * 54} 0)`">
        <path class="arc-track" d="M 7 27 A 19 19 0 0 1 45 27" />
        <path class="arc-peak" d="M 7 27 A 19 19 0 0 1 45 27" pathLength="100" :stroke-dasharray="`${channel.peak * 100} 100`" />
        <path class="arc-level" d="M 7 27 A 19 19 0 0 1 45 27" pathLength="100" :stroke-dasharray="`${channel.level * 100} 100`" />
        <circle v-if="channel.hold > 0" class="peak-marker" :class="{ overload: clipped }" :cx="channel.marker.x" :cy="channel.marker.y" r="1.5" />
      </g>
    </template>
    <template v-else>
      <g v-for="(channel, index) in rows" :key="index" :transform="`translate(0 ${index * 14})`">
        <circle
          v-for="(threshold, i) in dots" :key="i" :cx="14 + i * 8" cy="11" r="2.3"
          class="level-dot" :class="{ lit: channel.level >= threshold, peak: channel.peak >= threshold, held: channel.heldDot === i, overload: clipped && i === dots.length - 1 }"
        />
      </g>
    </template>
  </svg>
</template>

<style scoped>
.stereo-display { display: block; width: 108px; height: 36px; flex-shrink: 0; }
.arc-track { stroke: var(--muted-foreground); stroke-opacity: 0.15; stroke-width: 2; }
.arc-peak, .arc-level { stroke: var(--primary); stroke-width: 2; }
.arc-peak { stroke-opacity: 0.3; }
.peak-marker { fill: var(--foreground); }
.level-dot { fill: var(--muted-foreground); fill-opacity: 0.18; transition: fill 100ms, fill-opacity 100ms; }
.level-dot.peak { fill: var(--primary); fill-opacity: 0.35; }
.level-dot.lit { fill: var(--primary); fill-opacity: 1; }
.level-dot.held { fill: var(--foreground); fill-opacity: 0.8; }
.overload { fill: var(--destructive) !important; fill-opacity: 1 !important; }
@media (prefers-reduced-motion: reduce) { .level-dot { transition: none; } }
</style>
