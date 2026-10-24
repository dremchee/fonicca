<script setup lang="ts">
import type { Curve } from '@/stores/editor'

// Lucide-style fade glyph: the gain curve over a baseline, mirrored for a fade out.
const props = withDefaults(defineProps<{ direction?: 'in' | 'out', curve?: Curve }>(), { direction: 'in', curve: 'smooth' })

const PATHS: Record<Curve, string> = {
  linear: 'M3 19 L21 5',
  smooth: 'M3 19 C12 19 12 5 21 5',
  exponential: 'M3 19 C14 19 19 15 21 5',
}
</script>

<template>
  <svg
    xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24"
    fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"
    aria-hidden="true"
  >
    <g :transform="props.direction === 'out' ? 'matrix(-1 0 0 1 24 0)' : undefined">
      <path :d="PATHS[props.curve]" />
    </g>
    <path d="M3 21h18" opacity="0.5" />
  </svg>
</template>
