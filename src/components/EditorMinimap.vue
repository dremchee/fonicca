<script setup lang="ts">
import type { Wave } from '@/stores/editor'
import { useResizeObserver } from '@vueuse/core'
import { ref, shallowRef, watch, watchEffect } from 'vue'
import { useI18n } from 'vue-i18n'
import { useEditorStore } from '@/stores/editor'
import { useSettingsStore } from '@/stores/settings'
import { canvasContext, drawWave } from '@/waveform-renderer'
import { useWaveRenderer } from '@/waveform-gl'
import { waveformLook } from '@/waveform-settings'

// Whole file at a glance, with the visible range as a draggable window.
const props = defineProps<{
  viewStart: number
  viewEnd: number
  /** Frame under the pointer in the waveform, marked here too. */
  hoverFrame?: number | null
}>()
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
useWaveRenderer(canvas)
const size = ref({ width: 0, height: 0 })
const wave = shallowRef<Wave | null>(null)

useResizeObserver(box, (entries) => {
  const r = entries[0].contentRect
  size.value = { width: Math.floor(r.width), height: Math.floor(r.height) }
})

const frames = () => editor.state?.frames ?? 0

let request = 0
watch([() => editor.revision, () => size.value.width, () => settings.settings.editorWaveform.waveColors], async () => {
  const n = frames()
  if (!n || !size.value.width)
    return
  const req = ++request
  const columns = Math.min(4096, Math.round(size.value.width * (window.devicePixelRatio || 1)))
  const data = await editor.wave(0, n, columns, false, settings.settings.editorWaveform.waveColors === 'frequency')
  if (req === request)
    wave.value = data
}, { immediate: true })

function draw() {
  const el = canvas.value
  const { width: w, height: h } = size.value
  if (!el || !w)
    return
  const dpr = window.devicePixelRatio || 1
  const ctx = canvasContext(el, w, h, dpr)
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
  const s = settings.settings.editorWaveform
  drawWave(ctx, wave.value, { top: 0, width: w, height: h, dpr }, {
    ...waveformLook(s),
    style: s.waveStyle === 'bars' ? 'peaks' : s.waveStyle,
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
}

watch(
  [wave, size, () => props.viewStart, () => props.viewEnd, () => editor.selection, () => settings.settings, () => settings.themeRevision],
  draw,
  { deep: true },
)

// Same marks as the main view, as elements over the canvas so playback redraws
// nothing: a dim cursor and a full-strength playhead, both in the text colour.
const cursorEl = ref<HTMLElement | null>(null)
const playheadEl = ref<HTMLElement | null>(null)
const hoverEl = ref<HTMLElement | null>(null)
const at = (f: number) => `${(f / Math.max(1, frames())) * 100}%`
watchEffect(() => {
  const el = hoverEl.value
  if (!el)
    return
  const f = props.hoverFrame ?? null
  el.style.display = f === null ? 'none' : ''
  if (f !== null)
    el.style.left = at(f)
})
watchEffect(() => {
  if (cursorEl.value)
    cursorEl.value.style.left = at(editor.cursor)
})
watchEffect(() => {
  const el = playheadEl.value
  if (!el)
    return
  const p = editor.playhead
  el.style.display = p === null ? 'none' : ''
  if (p !== null)
    el.style.left = at(p)
})

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
    <div class="pointer-events-none absolute inset-0 overflow-hidden" aria-hidden="true">
      <div ref="cursorEl" class="absolute inset-y-0 w-px bg-foreground/60" />
      <div ref="hoverEl" class="absolute inset-y-0 w-px bg-foreground/35" style="display: none" />
      <div ref="playheadEl" class="absolute inset-y-0 w-[1.5px] bg-foreground" style="display: none" />
    </div>
  </div>
</template>
