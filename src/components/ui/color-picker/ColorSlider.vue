<script setup lang="ts">
import type { CSSProperties } from 'vue'
import { computed, ref } from 'vue'
import { cn } from 'cn'
import { CHECKERBOARD_STYLE, clamp } from './color'

const props = withDefaults(defineProps<{
  /** Normalised position 0..1. */
  modelValue: number
  /** Accessible name for the slider. */
  label: string
  /** Inline style for the gradient track. */
  trackStyle: CSSProperties
  /** Inline style for the draggable handle (usually its fill colour). */
  handleStyle?: CSSProperties
  /** Render a transparency checkerboard behind the track (alpha channel). */
  checkerboard?: boolean
  /** Keyboard arrow step, in normalised units. */
  step?: number
  class?: string
}>(), {
  step: 0.01,
})

const emit = defineEmits<{
  'update:modelValue': [value: number]
}>()

const el = ref<HTMLElement | null>(null)

function set(next: number) {
  emit('update:modelValue', clamp(next, 0, 1))
}

const { onPointerDown, onPointerMove, onPointerUp } = (() => {
  // Inline 1D variant of usePointerTrack — only the x axis matters here.
  let dragging = false
  function report(event: PointerEvent) {
    const node = el.value
    if (!node)
      return
    const rect = node.getBoundingClientRect()
    if (rect.width === 0)
      return
    set((event.clientX - rect.left) / rect.width)
  }
  return {
    onPointerDown(event: PointerEvent) {
      if (event.button !== 0)
        return
      dragging = true
      ;(event.currentTarget as HTMLElement).setPointerCapture?.(event.pointerId)
      report(event)
      event.preventDefault()
    },
    onPointerMove(event: PointerEvent) {
      if (dragging)
        report(event)
    },
    onPointerUp(event: PointerEvent) {
      if (!dragging)
        return
      dragging = false
      ;(event.currentTarget as HTMLElement).releasePointerCapture?.(event.pointerId)
    },
  }
})()

function onKeydown(event: KeyboardEvent) {
  const step = event.shiftKey ? props.step * 10 : props.step
  switch (event.key) {
    case 'ArrowLeft':
    case 'ArrowDown':
      set(props.modelValue - step)
      break
    case 'ArrowRight':
    case 'ArrowUp':
      set(props.modelValue + step)
      break
    case 'Home':
      set(0)
      break
    case 'End':
      set(1)
      break
    default:
      return
  }
  event.preventDefault()
}

const handleLeft = computed(() => `${clamp(props.modelValue, 0, 1) * 100}%`)
</script>

<template>
  <div
    ref="el"
    role="slider"
    tabindex="0"
    :aria-label="label"
    :aria-valuemin="0"
    :aria-valuemax="100"
    :aria-valuenow="Math.round(clamp(modelValue, 0, 1) * 100)"
    :class="cn(
      'relative h-3 w-full cursor-pointer touch-none rounded-full',
      'ring-1 ring-inset ring-border',
      'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring',
      props.class,
    )"
    @pointerdown="onPointerDown"
    @pointermove="onPointerMove"
    @pointerup="onPointerUp"
    @pointercancel="onPointerUp"
    @keydown="onKeydown"
  >
    <div v-if="checkerboard" class="absolute inset-0 rounded-full" :style="CHECKERBOARD_STYLE" />
    <div class="absolute inset-0 rounded-full" :style="trackStyle" />
    <div
      class="pointer-events-none absolute top-1/2 size-4 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 border-white shadow-[0_0_0_1px_rgba(0,0,0,0.35)]"
      :style="{ left: handleLeft, ...handleStyle }"
    />
  </div>
</template>
