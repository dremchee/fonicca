<script setup lang="ts">
defineProps<{ levels: readonly number[], clipped: boolean }>()
</script>

<template>
  <svg class="spectrum-display" viewBox="0 0 108 36" fill="none" aria-hidden="true">
    <path class="spectrum-base" d="M 2 27 H 106" />
    <rect v-for="(level, index) in levels" :key="index" class="spectrum-band" :x="4 + index * 6.4" :y="26 - level * 23" width="3.5" :height="Math.max(1, level * 23)" rx="1" />
    <circle v-if="clipped" class="spectrum-clip" cx="105" cy="3" r="1.5" />
  </svg>
</template>

<style scoped>
.spectrum-display { display: block; width: 108px; height: 36px; flex-shrink: 0; }
.spectrum-base { stroke: var(--muted-foreground); stroke-opacity: 0.15; }
.spectrum-band { fill: var(--primary); }
.spectrum-clip { fill: var(--destructive); }
</style>
