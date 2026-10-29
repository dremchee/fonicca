<script setup lang="ts">
import { dragMinimapRange, type MinimapDrag } from '@/editor-minimap'
import type { Wave } from '@/stores/editor'
import { usePixelCanvas } from '@/composables/pixelCanvas'
import { computed, onBeforeUnmount, ref, shallowRef, watch, watchEffect } from 'vue'
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
const wave = shallowRef<Wave | null>(null)
const { geometry, style: canvasStyle, snapX, pixelRatio } = usePixelCanvas(box)
const line = computed(() => 1 / geometry.value.ratio)
const dotSize = computed(() => Math.round(5 * geometry.value.ratio) / geometry.value.ratio)

const frames = () => editor.state?.frames ?? 0
const minimumSpan = computed(() => Math.min(64, frames()))
const startX = computed(() => snapX(geometry.value.left + props.viewStart / Math.max(1, frames()) * geometry.value.width))
const endX = computed(() => snapX(geometry.value.left + props.viewEnd / Math.max(1, frames()) * geometry.value.width))
const hitSize = computed(() => Math.round(12 * geometry.value.ratio) / geometry.value.ratio)
const windowStyle = computed(() => {
  const ratio = geometry.value.ratio
  const dotPixels = Math.round(5 * ratio)
  // Matching parity keeps a CSS-centred dot on physical pixels as well.
  let heightPixels = Math.max(dotPixels, Math.round(geometry.value.height * ratio) - 2)
  if ((heightPixels - dotPixels) % 2) heightPixels--
  const width = Math.min(geometry.value.width, Math.max(2 * line.value, endX.value - startX.value))
  const left = Math.max(geometry.value.left, Math.min(startX.value, geometry.value.left + geometry.value.width - width))
  return {
    transform: `translate(${left}px, ${geometry.value.top + line.value}px)`,
    width: `${width}px`,
    height: `${heightPixels / ratio}px`,
    '--window-line': `${line.value}px`,
    '--dot-size': `${dotSize.value}px`,
    '--hit-size': `${hitSize.value}px`,
    '--edge-phase': `${(dotPixels % 2) / (2 * ratio)}px`,
  }
})
const timeAt = (frame: number) => {
  const seconds = frame / Math.max(1, editor.state?.sampleRate ?? 1)
  return `${Math.floor(seconds / 60)}:${(seconds % 60).toFixed(3).padStart(6, '0')}`
}

let request = 0
watch([() => editor.loading, () => editor.revision, () => geometry.value.width, pixelRatio, () => settings.settings.editorWaveform.waveColors], async () => {
  const req = ++request
  const n = frames()
  if (editor.loading || !n || !geometry.value.width) { wave.value = null; return }
  const columns = Math.min(4096, Math.round(geometry.value.width * geometry.value.ratio))
  try {
    const data = await editor.wave(0, n, columns, false, settings.settings.editorWaveform.waveColors === 'frequency')
    if (req === request) wave.value = data
  }
  catch { if (req === request) wave.value = null }
}, { immediate: true })

function draw() {
  const el = canvas.value
  const { width: w, height: h, ratio: dpr } = geometry.value
  if (!el || !w)
    return
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
}

watch(
  [wave, geometry, () => props.viewStart, () => props.viewEnd, () => editor.selection, () => settings.settings, () => settings.themeRevision],
  draw,
  { deep: true, flush: 'post' },
)

// Same marks as the main view, as elements over the canvas so playback redraws
// nothing: a dim cursor and a full-strength playhead, both in the text colour.
const cursorEl = ref<HTMLElement | null>(null)
const playheadEl = ref<HTMLElement | null>(null)
const hoverEl = ref<HTMLElement | null>(null)
const at = (f: number) => `${snapX(geometry.value.left + (f / Math.max(1, frames())) * geometry.value.width)}px`
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
  const rect = canvas.value?.getBoundingClientRect()
  if (!rect?.width) return 0
  return Math.min(frames(), Math.max(0, ((clientX - rect.left) / rect.width) * frames()))
}

const dragging = shallowRef<MinimapDrag | null>(null)
let drag: { pointer: number, origin: number, start: number, end: number } | null = null
function stopDrag() {
  const pointer = drag?.pointer
  drag = null
  dragging.value = null
  if (pointer !== undefined && box.value?.hasPointerCapture(pointer)) box.value.releasePointerCapture(pointer)
}
function onPointerDown(e: PointerEvent) {
  if (e.button !== 0 || !e.isPrimary || !frames() || editor.loading) return
  const edge = (e.target as Element).closest<HTMLElement>('[data-edge]')?.dataset.edge
  const mode: MinimapDrag = edge === 'start' || edge === 'end' ? edge : 'move'
  const point = frameAt(e.clientX)
  let start = props.viewStart
  let end = props.viewEnd
  // Clicking outside the visible range centres it there before the drag starts.
  if (mode === 'move' && (point < start || point > end)) {
    const span = end-start
    start = Math.max(0, Math.min(frames()-span, point-span/2))
    end = start+span
    emit('view', start, end)
  }
  drag = { pointer: e.pointerId, origin: point, start, end }
  dragging.value = mode
  box.value?.setPointerCapture(e.pointerId)
}
function onPointerMove(e: PointerEvent) {
  if (!drag || e.pointerId !== drag.pointer || !dragging.value) return
  emit('view', ...dragMinimapRange(drag.start, drag.end, dragging.value, frameAt(e.clientX)-drag.origin, frames()))
}
function onKey(e: KeyboardEvent, mode: MinimapDrag) {
  const direction = ['ArrowLeft', 'ArrowDown'].includes(e.key) ? -1 : ['ArrowRight', 'ArrowUp'].includes(e.key) ? 1 : 0
  if (!direction && e.key !== 'Home' && e.key !== 'End') return
  e.preventDefault()
  e.stopPropagation()
  const step = Math.max(1, (props.viewEnd-props.viewStart) * (e.shiftKey ? 0.1 : 0.01))
  const delta = e.key === 'Home' ? -frames() : e.key === 'End' ? frames() : step*direction
  emit('view', ...dragMinimapRange(props.viewStart, props.viewEnd, mode, delta, frames()))
}
function onDoubleClick(e: MouseEvent) {
  if (!(e.target as Element).closest('[data-edge]')) emit('fit')
}
watch([() => editor.sample?.id, () => editor.state?.frames], stopDrag)
onBeforeUnmount(() => { request++; stopDrag() })

function onWheel(e: WheelEvent) {
  e.preventDefault()
  if (frames()) emit('zoom', Math.exp(e.deltaY * 0.01), frameAt(e.clientX))
}
</script>

<template>
  <div
    ref="box"
    class="minimap relative h-7 shrink-0 border-b border-border bg-background"
    :class="{ 'is-dragging': dragging }"
    :title="t('editor.minimap.hint')"
    @pointerdown="onPointerDown"
    @pointermove="onPointerMove"
    @pointerup="stopDrag"
    @pointercancel="stopDrag"
    @lostpointercapture="stopDrag"
    @wheel="onWheel"
    @dblclick="onDoubleClick"
  >
    <canvas ref="canvas" class="absolute" :style="canvasStyle" :aria-label="t('editor.minimap.label')" />
    <div
      v-if="frames() && geometry.width"
      class="view-window"
      :class="{ narrow: endX-startX < hitSize*2, 'at-start': startX-geometry.left < hitSize*1.5, 'at-end': geometry.left+geometry.width-endX < hitSize*1.5 }"
      :style="windowStyle"
      role="group" :aria-label="t('editor.minimap.label')"
    >
      <div
        class="view-pan"
        tabindex="0" role="slider" aria-orientation="horizontal"
        :aria-label="t('editor.minimap.move')"
        :aria-valuemin="0" :aria-valuemax="Math.max(0, frames()-(viewEnd-viewStart))" :aria-valuenow="viewStart"
        :aria-valuetext="`${timeAt(viewStart)} – ${timeAt(viewEnd)}`"
        @keydown="onKey($event, 'move')"
      />
      <button
        v-for="edge in (['start', 'end'] as const)" :key="edge"
        type="button" class="resize-handle" :data-edge="edge" :class="{ active: dragging === edge }"
        role="slider" aria-orientation="horizontal"
        :aria-label="t(`editor.minimap.${edge}`)"
        :aria-valuemin="edge === 'start' ? 0 : viewStart+minimumSpan"
        :aria-valuemax="edge === 'start' ? viewEnd-minimumSpan : frames()"
        :aria-valuenow="edge === 'start' ? viewStart : viewEnd"
        :aria-valuetext="timeAt(edge === 'start' ? viewStart : viewEnd)"
        @keydown="onKey($event, edge)"
      ><span aria-hidden="true" /></button>
    </div>
    <div class="pointer-events-none absolute inset-0 overflow-hidden" aria-hidden="true">
      <div ref="cursorEl" class="absolute inset-y-0 w-px bg-foreground/60" />
      <div ref="hoverEl" class="absolute inset-y-0 w-px bg-foreground/35" style="display: none" />
      <div ref="playheadEl" class="absolute inset-y-0 w-[1.5px] bg-foreground" style="display: none" />
    </div>
  </div>
</template>

<style scoped>
.minimap { touch-action: none; user-select: none; cursor: grab; }
.minimap.is-dragging { cursor: grabbing; }
.view-window { position: absolute; left: 0; top: 0; background: color-mix(in srgb, var(--primary) 2%, transparent); }
.view-window::before { content: ''; position: absolute; inset: 0; border: var(--window-line) solid var(--primary); border-radius: 2px; pointer-events: none; }
.view-pan { position: absolute; inset: 0; }
.view-pan:focus-visible { outline: 2px solid var(--ring); outline-offset: -3px; }
.resize-handle { position: absolute; z-index: 2; top: 0; bottom: 0; width: var(--hit-size); display: grid; place-items: center; border: 0; padding: 0; background: transparent; color: var(--primary); cursor: ew-resize; outline: none; }
.resize-handle[data-edge="start"] { left: calc(var(--edge-phase) - var(--hit-size) / 2); }
.resize-handle[data-edge="end"] { right: calc(var(--edge-phase) - var(--hit-size) / 2); }
.resize-handle > span { width: var(--dot-size); height: var(--dot-size); border-radius: 50%; background: currentColor; opacity: 0.8; pointer-events: none; }
.resize-handle:hover > span, .resize-handle.active > span, .resize-handle:focus-visible > span { opacity: 1; }
.resize-handle:focus-visible > span { outline: 2px solid var(--ring); outline-offset: 2px; }
/* Tiny zoom windows keep separate grab areas; file edges keep dots inside the map. */
.view-window.narrow .resize-handle[data-edge="start"] { left: calc(var(--edge-phase) - var(--hit-size) * 1.5); }
.view-window.narrow .resize-handle[data-edge="end"] { right: calc(var(--edge-phase) - var(--hit-size) * 1.5); }
.view-window.at-start .resize-handle[data-edge="start"] { left: 0; justify-items: start; }
.view-window.at-end .resize-handle[data-edge="end"] { right: 0; justify-items: end; }
.view-window.narrow.at-start .resize-handle[data-edge="end"] { left: var(--hit-size); right: auto; justify-items: start; }
.view-window.narrow.at-end .resize-handle[data-edge="start"] { right: var(--hit-size); left: auto; justify-items: end; }
</style>
