<script setup lang="ts">
import type { Wave } from '@/stores/editor'
import { useResizeObserver } from '@vueuse/core'
import { ref, shallowRef, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useEditorStore } from '@/stores/editor'
import { useSettingsStore } from '@/stores/settings'
import { drawWave } from '@/waveform'

// Whole file at a glance, with the visible range as a draggable window.
const props = defineProps<{ viewStart: number, viewEnd: number }>()
const emit = defineEmits<{
  view: [start: number, end: number]
  zoom: [factor: number, around: number]
  fit: []
}>()

const { t } = useI18n()
const editor = useEditorStore()
const settings = useSettingsStore()
const box = ref<HTMLElement | null>(null)
const canvas = ref<HTMLCanvasElement | null>(null)
const size = ref({ width: 0, height: 0 })
const wave = shallowRef<Wave | null>(null)

useResizeObserver(box, (entries) => {
  const r = entries[0].contentRect
  size.value = { width: Math.floor(r.width), height: Math.floor(r.height) }
})

const frames = () => editor.state?.frames ?? 0

let request = 0
watch([() => editor.revision, () => size.value.width, () => settings.settings.waveColors], async () => {
  const n = frames()
  if (!n || !size.value.width)
    return
  const req = ++request
  const columns = Math.min(4096, Math.round(size.value.width * (window.devicePixelRatio || 1)))
  const data = await editor.wave(0, n, columns, false, settings.settings.waveColors === 'frequency')
  if (req === request)
    wave.value = data
}, { immediate: true })

function draw() {
  const el = canvas.value
  const { width: w, height: h } = size.value
  if (!el || !w)
    return
  const dpr = window.devicePixelRatio || 1
  el.width = w * dpr
  el.height = h * dpr
  const ctx = el.getContext('2d')!
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
  ctx.clearRect(0, 0, w, h)
  const style = getComputedStyle(el)
  const color = (name: string) => style.getPropertyValue(name).trim()
  const n = frames()
  if (!wave.value || !n)
    return
  const x = (f: number) => (f / n) * w

  const sel = editor.selection
  if (sel) {
    ctx.fillStyle = color('--primary-soft')
    ctx.fillRect(x(sel.start), 0, Math.max(1, x(sel.end) - x(sel.start)), h)
  }
  const s = settings.settings
  drawWave(ctx, wave.value, { top: 0, width: w, height: h, dpr }, {
    style: s.waveStyle === 'bars' ? 'peaks' : s.waveStyle,
    scale: s.waveScale,
    colors: s.waveColors,
    zoom: 1,
    color: color('--wave'),
    lineColor: 'transparent',
  })

  // Visible range: everything outside it dimmed, the window outlined.
  const a = x(props.viewStart)
  const b = Math.max(a + 2, x(props.viewEnd))
  ctx.fillStyle = color('--background')
  ctx.globalAlpha = 0.55
  ctx.fillRect(0, 0, a, h)
  ctx.fillRect(b, 0, w - b, h)
  ctx.globalAlpha = 1
  ctx.strokeStyle = color('--primary')
  ctx.lineWidth = 1
  ctx.strokeRect(a + 0.5, 0.5, b - a - 1, h - 1)

  // Same marks as the main view: a dim cursor in the text colour, the playhead in
  // the brand colour.
  ctx.fillStyle = color('--foreground')
  ctx.globalAlpha = 0.6
  ctx.fillRect(x(editor.cursor), 0, 1, h)
  ctx.globalAlpha = 1
  if (editor.playhead !== null) {
    ctx.fillStyle = color('--primary')
    ctx.fillRect(x(editor.playhead), 0, 1.5, h)
  }
}

watch(
  [wave, size, () => props.viewStart, () => props.viewEnd, () => editor.selection, () => editor.cursor, () => editor.playhead, () => settings.settings],
  draw,
  { deep: true },
)

function frameAt(clientX: number) {
  const rect = canvas.value!.getBoundingClientRect()
  return Math.min(frames(), Math.max(0, ((clientX - rect.left) / rect.width) * frames()))
}

// Drag the window to pan; a press outside it first centres the window there.
function onMouseDown(e: MouseEvent) {
  if (e.button !== 0)
    return
  e.preventDefault()
  const span = props.viewEnd - props.viewStart
  const at = frameAt(e.clientX)
  const inside = at >= props.viewStart && at <= props.viewEnd
  const grab = inside ? at - props.viewStart : span / 2
  const moveTo = (clientX: number) => {
    const start = Math.max(0, Math.min(frames() - span, frameAt(clientX) - grab))
    emit('view', start, start + span)
  }
  if (!inside)
    moveTo(e.clientX)
  const move = (ev: MouseEvent) => moveTo(ev.clientX)
  const up = () => {
    window.removeEventListener('mousemove', move)
    window.removeEventListener('mouseup', up)
  }
  window.addEventListener('mousemove', move)
  window.addEventListener('mouseup', up)
}

function onWheel(e: WheelEvent) {
  e.preventDefault()
  emit('zoom', Math.exp(e.deltaY * 0.01), frameAt(e.clientX))
}
</script>

<template>
  <div ref="box" class="relative h-11 shrink-0 border-b border-border bg-background" :title="t('editor.minimap.hint')">
    <canvas
      ref="canvas"
      class="absolute inset-0 size-full cursor-grab active:cursor-grabbing"
      :aria-label="t('editor.minimap.label')"
      @mousedown="onMouseDown"
      @wheel="onWheel"
      @dblclick="emit('fit')"
    />
  </div>
</template>
