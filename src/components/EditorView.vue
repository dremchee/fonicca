<script setup lang="ts">
import type { Curve, Wave } from '@/stores/editor'
import {
  ArrowLeftRight,
  Crop,
  Maximize2,
  Play,
  Redo2,
  Repeat,
  Save,
  Scissors,
  Square,
  Undo2,
  VolumeX,
  ZoomIn,
  ZoomOut,
} from '@lucide/vue'
import { revealItemInDir } from '@tauri-apps/plugin-opener'
import { useResizeObserver } from '@vueuse/core'
import { computed, nextTick, ref, shallowRef, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import EditorMinimap from '@/components/EditorMinimap.vue'
import Hint from '@/components/Hint.vue'
import { Button } from '@/components/ui/button'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Separator } from '@/components/ui/separator'
import { Toggle } from '@/components/ui/toggle'
import { formatNumber, formatRate, shortPath } from '@/format'
import { drawWave } from '@/waveform'
import { useEditorStore } from '@/stores/editor'
import { useSettingsStore } from '@/stores/settings'

const { t } = useI18n()
const editor = useEditorStore()
const settings = useSettingsStore()

const box = ref<HTMLElement | null>(null)
const canvas = ref<HTMLCanvasElement | null>(null)
const width = ref(0)
const height = ref(0)
const wave = shallowRef<Wave | null>(null)
// Vertical zoom (⌥-scroll); 1 = full scale.
const vZoom = ref(1)
const RULER = 18

// Visible range in frames.
const viewStart = ref(0)
const viewEnd = ref(1)
const frames = computed(() => editor.state?.frames ?? 0)
const rate = computed(() => editor.state?.sampleRate ?? 44100)

function fit() {
  viewStart.value = 0
  viewEnd.value = Math.max(1, frames.value)
}

watch(() => editor.sample?.id, async (id) => {
  if (id == null)
    return
  fit()
  await nextTick()
  canvas.value?.focus()
})

// After an edit shortens the audio, keep the view inside it.
watch(frames, (n) => {
  if (viewEnd.value > n || viewEnd.value - viewStart.value <= 0) {
    const span = Math.min(viewEnd.value - viewStart.value, n)
    viewEnd.value = Math.max(1, n)
    viewStart.value = Math.max(0, n - span)
  }
})

useResizeObserver(box, (entries) => {
  const r = entries[0].contentRect
  width.value = Math.floor(r.width)
  height.value = Math.floor(r.height)
})

// --- Peaks and drawing ------------------------------------------------------

let request = 0
watch(
  [() => editor.revision, viewStart, viewEnd, width, () => settings.settings.waveChannels, () => settings.settings.waveColors],
  async () => {
    if (!editor.open || width.value === 0)
      return
    const req = ++request
    const columns = Math.min(8192, Math.round(width.value * (window.devicePixelRatio || 1)))
    const data = await editor.wave(
      Math.floor(viewStart.value),
      Math.ceil(viewEnd.value),
      columns,
      settings.settings.waveChannels === 'split',
      settings.settings.waveColors === 'frequency',
    )
    if (req === request)
      wave.value = data
  },
)

const x = (frame: number) => ((frame - viewStart.value) / (viewEnd.value - viewStart.value)) * width.value

function tickStep(span: number) {
  const steps = [0.001, 0.005, 0.01, 0.05, 0.1, 0.25, 0.5, 1, 2, 5, 10, 15, 30, 60, 120, 300]
  const seconds = span / rate.value
  return steps.find(s => (s / seconds) * width.value >= 80) ?? 600
}

function draw() {
  const el = canvas.value
  if (!el || width.value === 0)
    return
  const dpr = window.devicePixelRatio || 1
  el.width = width.value * dpr
  el.height = height.value * dpr
  const ctx = el.getContext('2d')!
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
  const w = width.value
  const h = height.value
  const style = getComputedStyle(el)
  const color = (name: string) => style.getPropertyValue(name).trim()
  ctx.clearRect(0, 0, w, h)

  // Selection.
  const sel = editor.selection
  if (sel) {
    ctx.fillStyle = color('--primary-soft')
    ctx.fillRect(x(sel.start), RULER, Math.max(1, x(sel.end) - x(sel.start)), h - RULER)
  }

  // Waveform, one lane per channel (or one mixed lane).
  if (wave.value) {
    const { waveStyle, waveScale, waveColors } = settings.settings
    drawWave(ctx, wave.value, { top: RULER, width: w, height: h - RULER, dpr }, {
      style: waveStyle,
      scale: waveScale,
      colors: waveColors,
      zoom: vZoom.value,
      color: color('--wave-strong'),
      lineColor: color('--border'),
    })
  }

  // Ruler.
  ctx.fillStyle = color('--muted-foreground')
  ctx.font = '10px system-ui, sans-serif'
  const step = tickStep(viewEnd.value - viewStart.value)
  const first = Math.ceil(viewStart.value / rate.value / step) * step
  for (let t = first; t * rate.value <= viewEnd.value; t += step) {
    const px = x(t * rate.value)
    ctx.fillRect(px, RULER - 5, 1 / dpr, 5)
    ctx.fillText(formatTime(t, step < 1), px + 3, 11)
  }

  // Cursor (text colour, the wave is the brand colour) and playhead (amber).
  ctx.fillStyle = color('--foreground')
  ctx.fillRect(x(editor.cursor), RULER, 1, h - RULER)
  if (editor.playhead !== null) {
    ctx.fillStyle = '#f59e0b'
    ctx.fillRect(x(editor.playhead), RULER, 1.5, h - RULER)
  }
}

watch([wave, () => editor.selection, () => editor.cursor, () => editor.playhead, width, height, vZoom, () => settings.settings], draw, { deep: true })

// --- Mouse ------------------------------------------------------------------

function frameAt(clientX: number) {
  const rect = canvas.value!.getBoundingClientRect()
  const f = viewStart.value + ((clientX - rect.left) / rect.width) * (viewEnd.value - viewStart.value)
  return Math.round(Math.min(frames.value, Math.max(0, f)))
}

function onMouseDown(e: MouseEvent) {
  if (e.button !== 0)
    return
  const anchor = e.shiftKey ? editor.cursor : frameAt(e.clientX)
  const startX = e.clientX
  let dragged = false
  const move = (ev: MouseEvent) => {
    if (Math.abs(ev.clientX - startX) < 3 && !dragged)
      return
    dragged = true
    const f = frameAt(ev.clientX)
    editor.selection = f === anchor ? null : { start: Math.min(f, anchor), end: Math.max(f, anchor) }
  }
  const up = (ev: MouseEvent) => {
    window.removeEventListener('mousemove', move)
    window.removeEventListener('mouseup', up)
    if (!dragged) {
      const f = frameAt(ev.clientX)
      if (e.shiftKey && f !== anchor)
        editor.selection = { start: Math.min(f, anchor), end: Math.max(f, anchor) }
      else {
        editor.selection = null
        editor.cursor = f
      }
    }
    else if (editor.selection) {
      editor.cursor = editor.selection.start
    }
  }
  window.addEventListener('mousemove', move)
  window.addEventListener('mouseup', up)
}

function zoom(factor: number, around = (viewStart.value + viewEnd.value) / 2) {
  const span = Math.min(frames.value, Math.max(64, (viewEnd.value - viewStart.value) * factor))
  const ratio = (around - viewStart.value) / (viewEnd.value - viewStart.value)
  let start = around - span * ratio
  start = Math.max(0, Math.min(frames.value - span, start))
  viewStart.value = start
  viewEnd.value = start + span
}

function pan(deltaFrames: number) {
  const span = viewEnd.value - viewStart.value
  const start = Math.max(0, Math.min(frames.value - span, viewStart.value + deltaFrames))
  viewStart.value = start
  viewEnd.value = start + span
}

// ⌘/Ctrl + wheel or pinch zooms around the pointer, ⌥ + wheel zooms vertically; plain scrolling pans.
function onWheel(e: WheelEvent) {
  e.preventDefault()
  if (e.altKey)
    vZoom.value = Math.min(32, Math.max(1, vZoom.value * Math.exp(-e.deltaY * 0.01)))
  else if (e.ctrlKey || e.metaKey)
    zoom(Math.exp(e.deltaY * 0.01), frameAt(e.clientX))
  else
    pan(((e.deltaX || e.deltaY) / width.value) * (viewEnd.value - viewStart.value))
}

function zoomToSelection() {
  const s = editor.selection
  if (s) {
    viewStart.value = s.start
    viewEnd.value = Math.max(s.end, s.start + 64)
  }
}

// --- Keyboard ---------------------------------------------------------------

function onKeyDown(e: KeyboardEvent) {
  const mod = e.metaKey || e.ctrlKey
  const key = e.key.toLowerCase()
  const handled = () => e.preventDefault()
  if (e.key === ' ') {
    handled()
    editor.togglePlay()
  }
  else if (mod && key === 'z') {
    handled()
    e.shiftKey ? editor.redo() : editor.undo()
  }
  else if (mod && key === 's') {
    handled()
    editor.save(e.shiftKey || !editor.canOverwrite)
  }
  else if (mod && key === 'a') {
    handled()
    editor.selection = { start: 0, end: frames.value }
  }
  else if (e.key === 'Delete' || e.key === 'Backspace') {
    handled()
    editor.remove()
  }
  else if (e.key === 'Escape') {
    editor.selection = null
  }
  else if (!mod && key === 'l') {
    editor.looping = !editor.looping
  }
  else if (!mod && (e.key === '=' || e.key === '+')) {
    zoom(0.5, editor.selection ? (editor.selection.start + editor.selection.end) / 2 : editor.cursor)
  }
  else if (!mod && e.key === '-') {
    zoom(2)
  }
  else if (!mod && e.key === '0') {
    fit()
  }
}


// --- Labels -----------------------------------------------------------------

function formatTime(seconds: number, precise = true) {
  const m = Math.floor(seconds / 60)
  const s = seconds - m * 60
  const sec = precise ? s.toFixed(3).padStart(6, '0') : String(Math.floor(s)).padStart(2, '0')
  return `${m}:${sec}`
}

const status = computed(() => {
  const sr = rate.value
  const sel = editor.selection
  return {
    cursor: formatTime(editor.cursor / sr),
    selection: sel ? `${formatTime(sel.start / sr)} – ${formatTime(sel.end / sr)} (${formatTime((sel.end - sel.start) / sr)})` : '',
    length: formatTime(frames.value / sr),
  }
})

const curves: { value: Curve, label: string }[] = [
  { value: 'smooth', label: 'editor.curves.smooth' },
  { value: 'linear', label: 'editor.curves.linear' },
  { value: 'exponential', label: 'editor.curves.exponential' },
]
</script>

<template>
  <div class="h-full flex flex-col overflow-hidden bg-background text-foreground">
    <!-- Drag region under the macOS traffic lights (overlay title bar), like the main window. -->
    <header class="h-11 shrink-0 pl-[84px] pr-3 flex items-center gap-3 border-b border-border" data-tauri-drag-region>
      <div class="min-w-0 text-sm font-medium truncate" data-tauri-drag-region>
        {{ editor.sample?.name }}<span class="text-muted-foreground">.{{ editor.sample?.ext }}</span>
        <span v-if="editor.state?.dirty" class="text-primary"> •</span>
      </div>
      <div class="ml-auto flex items-center gap-2">
        <Hint :text="t('editor.save.asNewHint')" shortcut="⇧⌘S">
          <Button variant="outline" size="sm" @click="editor.save(true)">
            {{ t('editor.save.asNew') }}
          </Button>
        </Hint>
        <Hint
          :text="editor.canOverwrite ? t('editor.save.overwriteHint') : t('editor.save.wavOnly')"
          shortcut="⌘S"
        >
          <Button size="sm" :disabled="!editor.canOverwrite || !editor.state?.dirty" @click="editor.save(false)">
            <Save /> {{ t('common.actions.save') }}
          </Button>
        </Hint>
      </div>
    </header>

    <div class="h-11 shrink-0 px-3 flex items-center gap-1 border-b border-border overflow-x-auto">
      <Hint :text="t('editor.toolbar.undo')" shortcut="⌘Z">
        <Button variant="ghost" size="icon-sm" :disabled="!editor.state?.canUndo" @click="editor.undo()">
          <Undo2 />
        </Button>
      </Hint>
      <Hint :text="t('editor.toolbar.redo')" shortcut="⇧⌘Z">
        <Button variant="ghost" size="icon-sm" :disabled="!editor.state?.canRedo" @click="editor.redo()">
          <Redo2 />
        </Button>
      </Hint>
      <Separator orientation="vertical" class="mx-1 h-5" />
      <Hint :text="editor.playing ? t('editor.toolbar.stop') : editor.selection ? t('editor.toolbar.playSelection') : t('editor.toolbar.playFromCursor')" :shortcut="t('editor.toolbar.space')">
        <Button variant="ghost" size="icon-sm" @click="editor.togglePlay()">
          <component :is="editor.playing ? Square : Play" />
        </Button>
      </Hint>
      <Hint :text="t('editor.toolbar.loopHint')" shortcut="L">
        <Toggle v-model="editor.looping" size="sm" :aria-label="t('editor.toolbar.loop')">
          <Repeat />
        </Toggle>
      </Hint>
      <Separator orientation="vertical" class="mx-1 h-5" />
      <Hint :text="t('editor.toolbar.cropHint')">
        <Button variant="ghost" size="sm" :disabled="!editor.selection" @click="editor.crop()">
          <Crop /> {{ t('editor.toolbar.crop') }}
        </Button>
      </Hint>
      <Hint :text="t('editor.toolbar.deleteHint')" shortcut="⌫">
        <Button variant="ghost" size="sm" :disabled="!editor.selection" @click="editor.remove()">
          <Scissors /> {{ t('editor.toolbar.delete') }}
        </Button>
      </Hint>
      <Hint :text="t('editor.toolbar.silenceHint')">
        <Button variant="ghost" size="icon-sm" :disabled="!editor.selection" @click="editor.silence()">
          <VolumeX />
        </Button>
      </Hint>
      <Separator orientation="vertical" class="mx-1 h-5" />
      <Hint :text="t('editor.toolbar.fadeInHint')">
        <Button variant="ghost" size="sm" @click="editor.fade('in')">
          {{ t('editor.toolbar.fadeIn') }}
        </Button>
      </Hint>
      <Hint :text="t('editor.toolbar.fadeOutHint')">
        <Button variant="ghost" size="sm" @click="editor.fade('out')">
          {{ t('editor.toolbar.fadeOut') }}
        </Button>
      </Hint>
      <Select v-model="editor.curve">
        <SelectTrigger size="sm" class="w-28" :aria-label="t('editor.toolbar.fadeCurve')">
          <SelectValue>{{ t(curves.find(c => c.value === editor.curve)?.label ?? 'editor.curves.smooth') }}</SelectValue>
        </SelectTrigger>
        <SelectContent>
          <SelectItem v-for="c in curves" :key="c.value" :value="c.value">
            {{ t(c.label) }}
          </SelectItem>
        </SelectContent>
      </Select>
      <Separator orientation="vertical" class="mx-1 h-5" />
      <Hint :text="t('editor.toolbar.normalizeHint')">
        <Button variant="ghost" size="sm" @click="editor.normalize()">
          {{ t('editor.toolbar.normalize') }}
        </Button>
      </Hint>
      <Button variant="ghost" size="sm" class="tabular" @click="editor.gain(-3)">
        −3 {{ t('common.units.dB') }}
      </Button>
      <Button variant="ghost" size="sm" class="tabular" @click="editor.gain(3)">
        +3 {{ t('common.units.dB') }}
      </Button>
      <Hint :text="t('editor.toolbar.reverseHint')">
        <Button variant="ghost" size="icon-sm" @click="editor.reverse()">
          <ArrowLeftRight />
        </Button>
      </Hint>
      <div class="ml-auto flex items-center gap-1">
        <Hint :text="t('editor.toolbar.zoomOut')" shortcut="−">
          <Button variant="ghost" size="icon-sm" @click="zoom(2)">
            <ZoomOut />
          </Button>
        </Hint>
        <Hint :text="t('editor.toolbar.zoomIn')" shortcut="+">
          <Button variant="ghost" size="icon-sm" @click="editor.selection ? zoomToSelection() : zoom(0.5, editor.cursor)">
            <ZoomIn />
          </Button>
        </Hint>
        <Hint :text="t('editor.toolbar.fit')" shortcut="0">
          <Button variant="ghost" size="icon-sm" @click="fit()">
            <Maximize2 />
          </Button>
        </Hint>
      </div>
    </div>

    <EditorMinimap
      :view-start="viewStart"
      :view-end="viewEnd"
      @view="(a, b) => { viewStart = a; viewEnd = b }"
      @zoom="zoom"
      @fit="fit()"
    />
    <div ref="box" class="relative flex-1 min-h-0 bg-card">
      <canvas
        ref="canvas"
        tabindex="0"
        class="absolute inset-0 size-full outline-none cursor-text"
        :aria-label="t('editor.waveform.label')"
        @mousedown="onMouseDown"
        @wheel="onWheel"
        @keydown="onKeyDown"
      />
      <div v-if="editor.loading" class="absolute inset-0 grid place-items-center text-muted-foreground text-sm">
        {{ t('editor.waveform.opening') }}
      </div>
      <div v-else-if="editor.error" class="absolute inset-0 grid place-items-center text-destructive text-sm px-6 text-center">
        {{ editor.error }}
      </div>
    </div>

    <footer class="h-8 shrink-0 px-4 flex items-center gap-4 border-t border-border text-xs text-muted-foreground tabular">
      <span class="shrink-0">{{ t('editor.status.cursor', { time: status.cursor }) }}</span>
      <button
        v-if="vZoom > 1.01" class="shrink-0 hover:text-foreground"
        :title="t('editor.status.vZoom')"
        @click="vZoom = 1"
      >
        ↕ ×{{ formatNumber(vZoom, { fixed: 1 }) }}
      </button>
      <span v-if="status.selection" class="shrink-0">{{ t('editor.status.selection', { range: status.selection }) }}</span>
      <button
        class="min-w-0 flex-1 truncate text-right hover:text-foreground hover:underline"
        :title="`${editor.sample?.path}\n${t('editor.status.reveal')}`"
        @click="editor.sample && revealItemInDir(editor.sample.path)"
      >
        {{ editor.sample ? shortPath(editor.sample.path) : '' }}
      </button>
      <span class="shrink-0">{{ status.length }} · {{ formatRate(rate) }} · {{ editor.state?.channels === 1 ? t('common.channels.mono') : t('common.channels.stereo') }}</span>
    </footer>
  </div>
</template>
