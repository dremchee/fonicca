<script setup lang="ts">
import type { ProjectClip } from '@/project'
import { computed, ref, useTemplateRef, watch } from 'vue'
import { useResizeObserver } from '@vueuse/core'
import { useI18n } from 'vue-i18n'
import { projectWave } from '@/composables/projectWave'
import { snapProjectTime } from '@/project'

const props = defineProps<{ clip: ProjectClip, trackId: string, scale: number, bpm: number, snap: boolean, selected: boolean }>()
const emit = defineEmits<{ select: [id: string], move: [id: string, track: string, start: number], remove: [], duplicate: [] }>()
const { t } = useI18n()
const canvas = useTemplateRef<HTMLCanvasElement>('wave')
const preview = ref<number | null>(null)
const width = computed(() => Math.max(6, props.clip.duration * props.clip.repeats * props.scale))
let peaks: { duration: number, peaks: number[] } | undefined
watch(() => props.clip.path, async (path, _, onCleanup) => {
  let stale = false; onCleanup(() => stale = true)
  peaks = undefined
  try { const result = await projectWave(path); if (!stale) { peaks = result; draw() } }
  catch { if (!stale) draw() }
}, { immediate: true })
function draw() {
  const el = canvas.value; if (!el) return
  const w = Math.min(2048, Math.ceil(width.value)), h = 36
  el.width = w; el.height = h
  const ctx = el.getContext('2d'); if (!ctx) return
  ctx.clearRect(0, 0, w, h)
  if (!peaks?.duration || !peaks.peaks.length) return
  ctx.fillStyle = 'currentColor'
  for (let x = 0; x < w; x++) {
    const time = (x / w * props.clip.duration * props.clip.repeats) % props.clip.duration
    const index = Math.min(peaks.peaks.length - 1, Math.floor((props.clip.offset + time) / peaks.duration * peaks.peaks.length))
    const fade = Math.min(1, props.clip.fadeIn ? time / props.clip.fadeIn : 1, props.clip.fadeOut ? (props.clip.duration - time) / props.clip.fadeOut : 1)
    const amp = (peaks.peaks[index] ?? 0) * fade * 16
    ctx.fillRect(x, h / 2 - amp, 1, Math.max(1, amp * 2))
  }
}
useResizeObserver(canvas, draw)
watch(() => [props.clip.offset, props.clip.duration, props.clip.repeats, props.clip.fadeIn, props.clip.fadeOut, props.scale], draw)
let drag: { x: number, origin: number } | null = null
function down(e: PointerEvent) {
  if (e.button !== 0) return
  emit('select', props.clip.id)
  drag = { x: e.clientX, origin: props.clip.start }
  ;(e.currentTarget as HTMLElement).setPointerCapture(e.pointerId)
}
function move(e: PointerEvent) {
  if (!drag) return
  const start = snapProjectTime(drag.origin + (e.clientX - drag.x) / props.scale, props.bpm, props.snap !== e.altKey)
  preview.value = Math.min(600 - props.clip.duration * props.clip.repeats, start)
}
function up(e: PointerEvent) {
  if (!drag) return
  const lane = document.elementFromPoint(e.clientX, e.clientY)?.closest<HTMLElement>('[data-project-track]')
  if (preview.value !== null) emit('move', props.clip.id, lane?.dataset.projectTrack ?? props.trackId, preview.value)
  drag = null; preview.value = null
}
function key(e: KeyboardEvent) {
  if (e.key === 'ArrowLeft' || e.key === 'ArrowRight') {
    e.preventDefault(); e.stopPropagation()
    const step = (e.shiftKey ? 4 : props.snap ? 0.25 : 0.1) * 60 / props.bpm
    emit('move', props.clip.id, props.trackId, Math.max(0, props.clip.start + (e.key === 'ArrowLeft' ? -step : step)))
  }
  if (e.key === 'Delete' || e.key === 'Backspace') { e.preventDefault(); emit('remove') }
  if ((e.metaKey || e.ctrlKey) && e.key === 'd') { e.preventDefault(); emit('duplicate') }
}
</script>

<template>
  <button
    class="project-clip absolute top-2 bottom-2 rounded-md border text-left overflow-hidden select-none touch-none"
    :class="selected ? 'border-primary ring-1 ring-primary bg-primary/20' : 'border-primary/35 bg-primary/10 hover:bg-primary/15'"
    :style="{ left: `${(preview ?? clip.start) * scale}px`, width: `${width}px` }"
    :aria-label="`${clip.name} · ${clip.start.toFixed(2)} s · ${clip.repeats}×`" :aria-pressed="selected" :title="`${clip.name}\n${t('project.clipHint')}`"
    @pointerdown.stop="down" @pointermove="move" @pointerup="up" @pointercancel="drag = null; preview = null" @keydown="key"
    @click.stop="emit('select', clip.id)"
  >
    <span class="absolute top-1 left-2 right-1 truncate text-xs font-medium">{{ clip.name }}<template v-if="clip.repeats > 1"> · {{ clip.repeats }}×</template></span>
    <canvas ref="wave" class="absolute left-0 bottom-1 w-full h-9 text-primary opacity-65 pointer-events-none" aria-hidden="true" />
  </button>
</template>
