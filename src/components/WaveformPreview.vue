<script setup lang="ts">
import type { WaveformSettings } from '@/api'
import { useEventListener, useResizeObserver } from '@vueuse/core'
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useSettingsStore } from '@/stores/settings'
import { demoWave, risingWave, drawWave } from '@/waveform-renderer'
import { useWaveRenderer } from '@/waveform-gl'
import { waveformLook } from '@/waveform-settings'

// Live preview of the waveform settings on a made-up groove.
const { t } = useI18n()
const props = defineProps<{ options: WaveformSettings, accent?: boolean, pattern?: 'groove' | 'rising', zoom?: number, pan?: number }>()
const emit = defineEmits<{ pan: [value: number], viewport: [value: { zoom: number, pan: number }] }>()
const settings = useSettingsStore()
const box = ref<HTMLElement | null>(null)
const canvas = ref<HTMLCanvasElement | null>(null)
useWaveRenderer(canvas)
const size = ref({ width: 0, height: 0 })

useResizeObserver(box, (entries) => {
  const r = entries[0].contentRect
  size.value = { width: Math.floor(r.width), height: Math.floor(r.height) }
})

function draw() {
  const el = canvas.value
  const { width, height } = size.value
  if (!el || width === 0)
    return
  const dpr = window.devicePixelRatio || 1
  el.width = width * dpr
  el.height = height * dpr
  const ctx = el.getContext('2d')!
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
  ctx.clearRect(0, 0, width, height)
  const s = props.options
  const style = getComputedStyle(el)
  const makeWave = props.pattern === 'rising' ? risingWave : demoWave
  const count = Math.max(1, Math.round(width * dpr))
  const zoom = Math.max(1, Math.min(8, props.zoom ?? 1))
  const data = makeWave(Math.round(count * zoom), s.waveChannels === 'split')
  const start = Math.round(Math.max(0, Math.min(1, props.pan ?? 0)) * (data.count - count))
  const values = new Float32Array(data.lanes * count * data.stride)
  for (let lane = 0; lane < data.lanes; lane++) {
    const offset = (lane * data.count + start) * data.stride
    values.set(data.values.subarray(offset, offset + count * data.stride), lane * count * data.stride)
  }
  drawWave(ctx, { ...data, count, values }, { top: 0, width, height, dpr }, {
    ...waveformLook(s),
    zoom: 1,
    color: style.getPropertyValue(props.accent ? '--wave-strong' : '--wave').trim(),
    lineColor: style.getPropertyValue('--border').trim(),
  })
}

watch([size, () => props.options, () => props.accent, () => props.pattern, () => props.zoom, () => props.pan, () => settings.themeRevision], draw, { deep: true })
useEventListener(box, 'wheel', (event: WheelEvent) => {
  if (props.zoom == null || !box.value || !event.deltaY) return
  event.preventDefault()
  const rect = box.value.getBoundingClientRect()
  const anchor = Math.max(0, Math.min(1, (event.clientX - rect.left) / rect.width))
  const zoom = props.zoom
  const delta = event.deltaY * (event.deltaMode === 1 ? 16 : event.deltaMode === 2 ? rect.height : 1)
  const next = Math.max(1, Math.min(8, zoom * Math.exp(-Math.max(-100, Math.min(100, delta)) * 0.008)))
  const point = (props.pan ?? 0) * (1 - 1 / zoom) + anchor / zoom
  const pan = next === 1 ? 0 : Math.max(0, Math.min(1, (point - anchor / next) / (1 - 1 / next)))
  emit('viewport', { zoom: next, pan })
}, { passive: false })

let drag: { id: number, x: number, pan: number } | null = null
function startPan(event: PointerEvent) {
  if (event.button !== 0 || (props.zoom ?? 1) <= 1) return
  drag = { id: event.pointerId, x: event.clientX, pan: props.pan ?? 0 }
  ;(event.currentTarget as HTMLElement).setPointerCapture(event.pointerId)
}
function movePan(event: PointerEvent) {
  if (!drag || event.pointerId !== drag.id) return
  const delta = (event.clientX - drag.x) / Math.max(1, size.value.width * ((props.zoom ?? 1) - 1))
  emit('pan', Math.max(0, Math.min(1, drag.pan - delta)))
}
</script>

<template>
  <div ref="box" class="relative h-28 rounded-md bg-background overflow-hidden" :aria-label="t('player.wavePreview')" role="img"
    :class="{ 'cursor-grab active:cursor-grabbing touch-none': (zoom ?? 1) > 1 }"
    @pointerdown="startPan" @pointermove="movePan" @pointerup="drag = null" @pointercancel="drag = null" @lostpointercapture="drag = null"
  >
    <canvas ref="canvas" class="absolute inset-0 size-full" />
  </div>
</template>
