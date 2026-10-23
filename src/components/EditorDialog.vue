<script setup lang="ts">
import type { Curve } from '@/stores/editor'
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
import { onKeyStroke, useResizeObserver } from '@vueuse/core'
import { computed, nextTick, ref, shallowRef, watch } from 'vue'
import Hint from '@/components/Hint.vue'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogTitle } from '@/components/ui/dialog'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Separator } from '@/components/ui/separator'
import { Toggle } from '@/components/ui/toggle'
import { useEditorStore } from '@/stores/editor'
import { useLibraryStore } from '@/stores/library'

const editor = useEditorStore()
const library = useLibraryStore()

// E opens the focused sample in the editor.
onKeyStroke(['e', 'у'], (e) => {
  if (editor.open || e.metaKey || e.ctrlKey || e.altKey)
    return
  if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement)
    return
  if (e.target instanceof Element && e.target.closest('[role="dialog"], [role="menu"]'))
    return
  const s = library.focused
  if (s) {
    e.preventDefault()
    editor.openSample(s)
  }
})

const box = ref<HTMLElement | null>(null)
const canvas = ref<HTMLCanvasElement | null>(null)
const width = ref(0)
const height = ref(0)
const peaks = shallowRef<Int8Array | null>(null)
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
watch([() => editor.revision, viewStart, viewEnd, width], async () => {
  if (!editor.open || width.value === 0)
    return
  const req = ++request
  const columns = Math.min(8192, Math.round(width.value * (window.devicePixelRatio || 1)))
  const data = await editor.peaks(Math.floor(viewStart.value), Math.ceil(viewEnd.value), columns)
  if (req === request)
    peaks.value = Int8Array.from(data)
})

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

  // Waveform.
  const data = peaks.value
  if (data) {
    const columns = data.length / 2
    const mid = RULER + (h - RULER) / 2
    const amp = ((h - RULER) / 2) * 0.95
    ctx.fillStyle = color('--wave')
    for (let c = 0; c < columns; c++) {
      const px = (c / columns) * w
      const top = mid - (data[c * 2 + 1] / 127) * amp
      const bottom = mid - (data[c * 2] / 127) * amp
      ctx.fillRect(px, top, Math.max(1 / dpr, w / columns), Math.max(1 / dpr, bottom - top))
    }
    ctx.fillStyle = color('--border')
    ctx.fillRect(0, mid, w, 1 / dpr)
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

  // Cursor and playhead.
  ctx.fillStyle = color('--primary')
  ctx.fillRect(x(editor.cursor), RULER, 1, h - RULER)
  if (editor.playhead !== null) {
    ctx.fillStyle = color('--foreground')
    ctx.fillRect(x(editor.playhead), RULER, 1.5, h - RULER)
  }
}

watch([peaks, () => editor.selection, () => editor.cursor, () => editor.playhead, width, height], draw)

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

// ⌘/Ctrl + wheel or pinch zooms around the pointer; plain scrolling pans.
function onWheel(e: WheelEvent) {
  e.preventDefault()
  if (e.ctrlKey || e.metaKey)
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

// Esc first clears the selection, then closes (asking about unsaved edits).
function onEscape(e: KeyboardEvent) {
  e.preventDefault()
  if (editor.selection)
    editor.selection = null
  else
    editor.closeEditor()
}

function onOpenChange(open: boolean) {
  if (!open)
    editor.closeEditor()
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
  { value: 'smooth', label: 'Smooth' },
  { value: 'linear', label: 'Linear' },
  { value: 'exponential', label: 'Exponential' },
]
</script>

<template>
  <Dialog :open="editor.open" @update:open="onOpenChange">
    <DialogContent
      class="sm:max-w-[min(96vw,1400px)] w-[96vw] h-[80vh] p-0 gap-0 flex flex-col overflow-hidden"
      :show-close-button="false"
      @escape-key-down="onEscape"
      @interact-outside.prevent
    >
      <header class="h-12 shrink-0 px-4 flex items-center gap-3 border-b border-border">
        <DialogTitle class="text-sm font-medium truncate">
          {{ editor.sample?.name }}<span class="text-muted-foreground">.{{ editor.sample?.ext }}</span>
          <span v-if="editor.state?.dirty" class="text-primary" title="Unsaved edits"> •</span>
        </DialogTitle>
        <DialogDescription class="sr-only">
          Audio editor
        </DialogDescription>
        <div class="ml-auto flex items-center gap-2">
          <Hint text="Save as a new file next to the original" shortcut="⇧⌘S">
            <Button variant="outline" size="sm" @click="editor.save(true)">
              Save as new
            </Button>
          </Hint>
          <Hint
            :text="editor.canOverwrite ? 'Save over the original (a backup is kept)' : 'Only WAV files can be saved in place'"
            shortcut="⌘S"
          >
            <Button size="sm" :disabled="!editor.canOverwrite || !editor.state?.dirty" @click="editor.save(false)">
              <Save /> Save
            </Button>
          </Hint>
          <Button variant="ghost" size="sm" @click="editor.closeEditor()">
            Close
          </Button>
        </div>
      </header>

      <div class="h-11 shrink-0 px-3 flex items-center gap-1 border-b border-border overflow-x-auto">
        <Hint text="Undo" shortcut="⌘Z">
          <Button variant="ghost" size="icon-sm" :disabled="!editor.state?.canUndo" @click="editor.undo()">
            <Undo2 />
          </Button>
        </Hint>
        <Hint text="Redo" shortcut="⇧⌘Z">
          <Button variant="ghost" size="icon-sm" :disabled="!editor.state?.canRedo" @click="editor.redo()">
            <Redo2 />
          </Button>
        </Hint>
        <Separator orientation="vertical" class="mx-1 h-5" />
        <Hint :text="editor.playing ? 'Stop' : editor.selection ? 'Play selection' : 'Play from cursor'" shortcut="Space">
          <Button variant="ghost" size="icon-sm" @click="editor.togglePlay()">
            <component :is="editor.playing ? Square : Play" />
          </Button>
        </Hint>
        <Hint text="Loop the selection" shortcut="L">
          <Toggle v-model="editor.looping" size="sm" aria-label="Loop">
            <Repeat />
          </Toggle>
        </Hint>
        <Separator orientation="vertical" class="mx-1 h-5" />
        <Hint text="Keep only the selection">
          <Button variant="ghost" size="sm" :disabled="!editor.selection" @click="editor.crop()">
            <Crop /> Crop
          </Button>
        </Hint>
        <Hint text="Delete the selection" shortcut="⌫">
          <Button variant="ghost" size="sm" :disabled="!editor.selection" @click="editor.remove()">
            <Scissors /> Delete
          </Button>
        </Hint>
        <Hint text="Silence the selection">
          <Button variant="ghost" size="icon-sm" :disabled="!editor.selection" @click="editor.silence()">
            <VolumeX />
          </Button>
        </Hint>
        <Separator orientation="vertical" class="mx-1 h-5" />
        <Hint text="Fade in the selection, or from the start to the cursor">
          <Button variant="ghost" size="sm" @click="editor.fade('in')">
            Fade in
          </Button>
        </Hint>
        <Hint text="Fade out the selection, or from the cursor to the end">
          <Button variant="ghost" size="sm" @click="editor.fade('out')">
            Fade out
          </Button>
        </Hint>
        <Select v-model="editor.curve">
          <SelectTrigger size="sm" class="w-28" aria-label="Fade curve">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem v-for="c in curves" :key="c.value" :value="c.value">
              {{ c.label }}
            </SelectItem>
          </SelectContent>
        </Select>
        <Separator orientation="vertical" class="mx-1 h-5" />
        <Hint text="Peak to −0.3 dB (selection or whole file)">
          <Button variant="ghost" size="sm" @click="editor.normalize()">
            Normalize
          </Button>
        </Hint>
        <Button variant="ghost" size="sm" class="tabular" @click="editor.gain(-3)">
          −3 dB
        </Button>
        <Button variant="ghost" size="sm" class="tabular" @click="editor.gain(3)">
          +3 dB
        </Button>
        <Hint text="Reverse (selection or whole file)">
          <Button variant="ghost" size="icon-sm" @click="editor.reverse()">
            <ArrowLeftRight />
          </Button>
        </Hint>
        <div class="ml-auto flex items-center gap-1">
          <Hint text="Zoom out" shortcut="−">
            <Button variant="ghost" size="icon-sm" @click="zoom(2)">
              <ZoomOut />
            </Button>
          </Hint>
          <Hint text="Zoom in (to the selection if any)" shortcut="+">
            <Button variant="ghost" size="icon-sm" @click="editor.selection ? zoomToSelection() : zoom(0.5, editor.cursor)">
              <ZoomIn />
            </Button>
          </Hint>
          <Hint text="Show everything" shortcut="0">
            <Button variant="ghost" size="icon-sm" @click="fit()">
              <Maximize2 />
            </Button>
          </Hint>
        </div>
      </div>

      <div ref="box" class="relative flex-1 min-h-0 bg-card">
        <canvas
          ref="canvas"
          tabindex="0"
          class="absolute inset-0 size-full outline-none cursor-text"
          aria-label="Waveform: drag to select, ⌘-scroll to zoom"
          @mousedown="onMouseDown"
          @wheel="onWheel"
          @keydown="onKeyDown"
        />
        <div v-if="editor.loading" class="absolute inset-0 grid place-items-center text-muted-foreground text-sm">
          Opening…
        </div>
      </div>

      <footer class="h-8 shrink-0 px-4 flex items-center gap-4 border-t border-border text-xs text-muted-foreground tabular">
        <span>Cursor {{ status.cursor }}</span>
        <span v-if="status.selection">Selection {{ status.selection }}</span>
        <span class="ml-auto">{{ status.length }} · {{ (rate / 1000).toLocaleString() }} kHz · {{ editor.state?.channels === 1 ? 'Mono' : 'Stereo' }}</span>
      </footer>
    </DialogContent>
  </Dialog>
</template>
