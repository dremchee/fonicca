<script setup lang="ts">
import { computed, ref } from 'vue'
import { cn } from 'cn'
import { useI18n } from 'vue-i18n'
import { clamp } from './color'
import { usePointerTrack } from './usePointerTrack'

const props = withDefaults(defineProps<{
  /** Current hue 0..360 — drives the background colour of the area. */
  hue: number
  /** Current saturation 0..100 (horizontal axis). */
  saturation: number
  /** Current value/brightness 0..100 (vertical axis). */
  value: number
  /** CSS colour used to fill the draggable handle. */
  handleColor: string
  class?: string
}>(), {})

const emit = defineEmits<{
  update: [payload: { s: number, v: number }]
}>()
const { t } = useI18n()

const el = ref<HTMLElement | null>(null)

const { onPointerDown, onPointerMove, onPointerUp } = usePointerTrack(el, ({ x, y }) => {
  emit('update', { s: x * 100, v: (1 - y) * 100 })
})

const background = computed(() => ({
  backgroundColor: `hsl(${props.hue}, 100%, 50%)`,
  backgroundImage:
    'linear-gradient(to top, #000, rgba(0,0,0,0)), linear-gradient(to right, #fff, rgba(255,255,255,0))',
}))

const handlePosition = computed(() => ({
  left: `${props.saturation}%`,
  top: `${100 - props.value}%`,
}))

function onKeydown(event: KeyboardEvent) {
  const step = event.shiftKey ? 10 : 1
  let s = props.saturation
  let v = props.value
  switch (event.key) {
    case 'ArrowLeft':
      s -= step
      break
    case 'ArrowRight':
      s += step
      break
    case 'ArrowUp':
      v += step
      break
    case 'ArrowDown':
      v -= step
      break
    default:
      return
  }
  event.preventDefault()
  emit('update', { s: clamp(s, 0, 100), v: clamp(v, 0, 100) })
}
</script>

<template>
  <div
    ref="el"
    role="slider"
    tabindex="0"
    :aria-label="t('common.color.saturationBrightness')"
    :aria-valuetext="t('common.color.saturationValue', { s: Math.round(saturation), v: Math.round(value) })"
    :class="cn(
      'relative h-40 w-full cursor-crosshair touch-none overflow-hidden rounded-md',
      'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring',
      props.class,
    )"
    :style="background"
    @pointerdown="onPointerDown"
    @pointermove="onPointerMove"
    @pointerup="onPointerUp"
    @pointercancel="onPointerUp"
    @keydown="onKeydown"
  >
    <div
      class="pointer-events-none absolute size-4 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 border-white shadow-[0_0_0_1px_rgba(0,0,0,0.35)]"
      :style="{ ...handlePosition, backgroundColor: handleColor }"
    />
  </div>
</template>
