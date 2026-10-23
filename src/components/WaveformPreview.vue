<script setup lang="ts">
import { useResizeObserver } from '@vueuse/core'
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useSettingsStore } from '@/stores/settings'
import { demoWave, drawWave } from '@/waveform'

// Live preview of the waveform settings on a made-up groove.
const { t } = useI18n()
const settings = useSettingsStore()
const box = ref<HTMLElement | null>(null)
const canvas = ref<HTMLCanvasElement | null>(null)
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
  const s = settings.settings
  const style = getComputedStyle(el)
  drawWave(ctx, demoWave(Math.round(width * dpr), s.waveChannels === 'split'), { top: 0, width, height, dpr }, {
    style: s.waveStyle,
    scale: s.waveScale,
    colors: s.waveColors,
    zoom: 1,
    color: style.getPropertyValue('--wave-strong').trim(),
    lineColor: style.getPropertyValue('--border').trim(),
  })
}

watch([size, () => settings.settings], draw, { deep: true })
</script>

<template>
  <div ref="box" class="relative h-28 rounded-md bg-background overflow-hidden" :aria-label="t('player.wavePreview')" role="img">
    <canvas ref="canvas" class="absolute inset-0 size-full" />
  </div>
</template>
