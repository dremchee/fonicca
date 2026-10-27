<script setup lang="ts">
import type { GridMode } from '@/composables/editor/grid'
import type { Curve, Marker, Wave } from '@/stores/editor'
import {
  ArrowLeftRight,
  ArrowUpToLine,
  ClipboardPaste,
  Copy,
  Crop,
  Eraser,
  FolderOutput,
  IterationCw,
  Magnet,
  Maximize2,
  Minimize2,
  Music,
  Play,
  Redo2,
  Repeat,
  Scissors,
  Square,
  SlidersHorizontal,
  Undo2,
  Volume1,
  Volume2,
  VolumeX,
  ZoomIn,
  ZoomOut,
  X,
} from '@lucide/vue'
import { useLocalStorage, useResizeObserver } from '@vueuse/core'
import { computed, nextTick, ref, shallowRef, watch, watchEffect } from 'vue'
import { useI18n } from 'vue-i18n'
import EditorMinimap from '@/components/EditorMinimap.vue'
import EditorSaveMenu from '@/components/EditorSaveMenu.vue'
import EditorDisplayControls from '@/components/EditorDisplayControls.vue'
import { useEditorSpectrogram } from '@/composables/editor/spectrogram'
import { drawFrequencyAxis, drawSpectrogram } from '@/spectrogram'
import EditorProcessing from '@/components/EditorProcessing.vue'
import LevelMeter from '@/components/LevelMeter.vue'
import EditorSlicer from '@/components/EditorSlicer.vue'
import FadeIcon from '@/components/icons/FadeIcon.vue'
import Hint from '@/components/Hint.vue'
import { Button } from '@/components/ui/button'
import {
  ContextMenu,
  ContextMenuCheckboxItem,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuShortcut,
  ContextMenuTrigger,
} from '@/components/ui/context-menu'
import { Input } from '@/components/ui/input'
import ToolbarOverflow from '@/components/ToolbarOverflow.vue'
import {
  DropdownMenu,
  DropdownMenuCheckboxItem,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Separator } from '@/components/ui/separator'
import { Toggle } from '@/components/ui/toggle'
import { MIN_LINE_PX, useEditorGrid } from '@/composables/editor/grid'
import { useEditorViewport } from '@/composables/editor/viewport'
import { useFrameDraw } from '@/composables/editor/frameDraw'
import { formatNumber, formatRate } from '@/format'
import { canvasContext, drawWave } from '@/waveform-renderer'
import { useWaveRenderer } from '@/waveform-gl'
import { waveformLook } from '@/waveform-settings'
import { regionsOf, useEditorStore } from '@/stores/editor'
import { useSettingsStore } from '@/stores/settings'
import { useBasicPitchStore } from '@/stores/basicPitch'

const { t } = useI18n()
const props = withDefaults(defineProps<{ embedded?: boolean, expanded?: boolean }>(), { embedded: false, expanded: false })
const emit = defineEmits<{ close: [], toggleExpand: [] }>()
const toolbarTools = ['undo', 'redo', 'play', 'loop', 'loopPoints', 'crop', 'delete', 'cut', 'copy', 'paste', 'silence', 'fadeIn', 'fadeOut', 'curve', 'normalize', 'gainDown', 'gainUp', 'reverse', 'stretch', 'lufs', 'dc', 'midi', 'slice', 'regions']
const editor = useEditorStore()
const settings = useSettingsStore()

const box = ref<HTMLElement | null>(null)
const canvas = ref<HTMLCanvasElement | null>(null)
useWaveRenderer(canvas)
const width = ref(0)
const height = ref(0)
const wave = shallowRef<Wave | null>(null)
// Vertical zoom (⌥-scroll); 1 = full scale.
const vZoom = ref(1)
const RULER = 18
// Toggles look like the ghost icon buttons around them; switched on, the icon takes
// the brand colour instead of getting a background. Styled by aria-pressed: the
// tooltip trigger wrapping them overwrites their data-state.
const toggleIcon = 'size-8 px-0 hover:bg-accent hover:text-accent-foreground dark:hover:bg-accent/50 aria-pressed:bg-transparent aria-pressed:text-primary aria-pressed:hover:text-primary aria-pressed:hover:bg-accent dark:aria-pressed:hover:bg-accent/50'
// Per-window preferences, not worth fields in settings.json.
const displayMode = useLocalStorage<'waveform' | 'spectrogram'>('editor.displayMode', 'waveform')
const isSpectrogram = computed(() => displayMode.value === 'spectrogram')
const gridMode = useLocalStorage<GridMode>('editor.gridMode', 'beats')
const snap = useLocalStorage('editor.snap', true)
const zeroSnap = useLocalStorage('editor.zeroSnap', true)
const gridTempoRow = ref<HTMLElement | null>(null)
function openGridTempo() {
  gridMode.value = 'beats'
  // Let the context menu restore focus before focusing the footer field.
  setTimeout(() => {
    const input = gridTempoRow.value?.querySelector('input')
    input?.focus()
    input?.select()
  }, 0)
}
// Slices shown while the slicer is open.
const slicePreview = ref<Marker[]>([])
// Tempo of the bar grid (from the sample, editable) and where bar 1 starts, in frames.
const bpm = ref<number | null>(null)
const downbeat = ref(0)
function exportMidi() {
  if (!editor.sample || !editor.state?.frames) return
  useBasicPitchStore().show(editor.sample, editor.selection ?? { start: 0, end: editor.state.frames }, !!editor.selection, bpm.value)
}


// The View menu changes the editor profile only; library preview has its own options.
type WaveKey = 'waveStyle' | 'waveScale' | 'waveChannels' | 'waveColors'
const viewOptions: { key: WaveKey, label: string, choices: { value: string, label: string }[] }[] = [
  { key: 'waveStyle', label: 'settings.waveform.style', choices: [{ value: 'peaks', label: 'settings.waveform.peaks' }, { value: 'peaksRms', label: 'settings.waveform.peaksRms' }, { value: 'bars', label: 'settings.waveform.bars' }] },
  { key: 'waveChannels', label: 'settings.waveform.channels', choices: [{ value: 'split', label: 'settings.waveform.split' }, { value: 'mix', label: 'settings.waveform.mix' }] },
  { key: 'waveScale', label: 'settings.waveform.scale', choices: [{ value: 'linear', label: 'settings.waveform.linear' }, { value: 'db', label: 'settings.waveform.db' }] },
  { key: 'waveColors', label: 'settings.waveform.colors', choices: [{ value: 'theme', label: 'settings.waveform.theme' }, { value: 'frequency', label: 'settings.waveform.frequency' }] },
]

function setView(key: WaveKey, value: unknown) {
  if (typeof value === 'string' && value !== settings.settings.editorWaveform[key])
    settings.update({ editorWaveform: { ...settings.settings.editorWaveform, [key]: value } })
}

const frames = computed(() => editor.state?.frames ?? 0)
const rate = computed(() => editor.state?.sampleRate ?? 44100)
const { viewStart, viewEnd, fit, zoom, pan, follow, deferFollow, zoomToSelection } = useEditorViewport(frames, () => editor.selection)
watch(() => editor.playhead, (frame) => {
  if (editor.playing) follow(frame)
})
const { data: spectrum, loading: spectrumLoading, error: spectrumError, refresh: refreshSpectrum } = useEditorSpectrogram({
  enabled: isSpectrogram, revision: computed(() => editor.revision), frames,
  start: viewStart, end: viewEnd, width, read: editor.spectrogram,
})

watch(() => editor.sessionVersion, async (id) => {
  if (id == null)
    return
  fit()
  bpm.value = editor.sample?.bpm ?? null
  downbeat.value = 0
  await nextTick()
  canvas.value?.focus()
}, { immediate: true })

useResizeObserver(box, (entries) => {
  const r = entries[0].contentRect
  width.value = Math.floor(r.width)
  height.value = Math.floor(r.height)
})

// --- Peaks and drawing ------------------------------------------------------

// The wave is read for a range half again as wide as the view, so short pans draw
// from what is there. A new read is needed only when the view leaves that range, the
// zoom changes the resolution, or the audio or the options change. One read is in
// flight at a time; the view it lags behind meanwhile is drawn stretched.
const OVERSCAN = 1.5
/** Frames and options `wave` was read for. */
let waveRange = { start: 0, end: 0, key: '' }
let reading = false
let readAgain = false

function waveKey() {
  const s = settings.settings.editorWaveform
  return `${editor.revision}|${s.waveChannels}|${s.waveColors}`
}

/** Whether `wave` can draw the current view at about one column per pixel. */
function waveFits() {
  const data = wave.value
  const r = waveRange
  if (!data || r.key !== waveKey() || r.start > viewStart.value || r.end < viewEnd.value)
    return false
  const px = width.value * (window.devicePixelRatio || 1)
  const span = viewEnd.value - viewStart.value
  if (data.samples)
    return span <= px
  const ratio = ((r.end - r.start) / data.count) / (span / px)
  return ratio > 0.9 && ratio < 1.1
}

async function readWave() {
  if (reading) {
    readAgain = true
    return
  }
  reading = true
  try {
    do {
      readAgain = false
      if (isSpectrogram.value || !editor.open || width.value === 0 || waveFits())
        continue
      const n = editor.state?.frames ?? 0
      const span = viewEnd.value - viewStart.value
      const wide = Math.min(n, span * OVERSCAN)
      const start = Math.floor(Math.max(0, Math.min(n - wide, viewStart.value - (wide - span) / 2)))
      const end = Math.ceil(Math.min(n, start + wide))
      const columns = Math.min(16384, Math.round(width.value * (window.devicePixelRatio || 1) * (end - start) / span))
      const key = waveKey()
      const s = settings.settings.editorWaveform
      const data = await editor.wave(start, end, columns, s.waveChannels === 'split', s.waveColors === 'frequency')
      waveRange = { start, end, key }
      wave.value = data
    } while (readAgain)
  }
  finally {
    reading = false
  }
}

watch([isSpectrogram, () => editor.revision, viewStart, viewEnd, width, () => settings.settings.editorWaveform.waveChannels, () => settings.settings.editorWaveform.waveColors], readWave)

const x = (frame: number) => ((frame - viewStart.value) / (viewEnd.value - viewStart.value)) * width.value

function tickStep(span: number) {
  const steps = [0.001, 0.005, 0.01, 0.05, 0.1, 0.25, 0.5, 1, 2, 5, 10, 15, 30, 60, 120, 300]
  const seconds = span / rate.value
  return steps.find(s => (s / seconds) * width.value >= 80) ?? 600
}

// --- Grid -------------------------------------------------------------------

const { beatFrames, beatGrid, gridStep, gridOrigin, gridFrames, isMultiple, snapped } = useEditorGrid({
  bpm, rate, frames, width, viewStart, viewEnd, downbeat, gridMode, snap,
})

const scheduleDraw = useFrameDraw(draw)

function draw() {
  const el = canvas.value
  if (!el || width.value === 0)
    return
  const dpr = window.devicePixelRatio || 1
  const w = width.value
  const h = height.value
  const ctx = canvasContext(el, w, h, dpr)
  const style = getComputedStyle(el)
  const spectralColors: Record<string, string> = { '--foreground': '#eee8f5', '--muted-foreground': '#c9bfd6', '--border': '#554b64', '--card': '#080511', '--background': '#080511' }
  const color = (name: string) => (isSpectrogram.value && spectralColors[name]) || style.getPropertyValue(name).trim()
  if (isSpectrogram.value) {
    ctx.fillStyle = '#080511'
    ctx.fillRect(0, 0, w, h)
  }
  const step = tickStep(viewEnd.value - viewStart.value)
  const first = Math.ceil(viewStart.value / rate.value / step) * step
  const ticks: number[] = []
  for (let t = first; t * rate.value <= viewEnd.value; t += step)
    ticks.push(t)

  // Waveform, one lane per channel (or one mixed lane), in the full brand colour.
  // The selection is the inverse: a brand-colour block with the wave in the canvas colour.
  const sel = editor.selection
  const selX = sel ? x(sel.start) : 0
  const selW = sel ? Math.max(1, x(sel.end) - selX) : 0
  if (isSpectrogram.value) {
    const data = spectrum.value
    ctx.save()
    ctx.beginPath()
    ctx.rect(0, RULER, w, Math.max(0, h - RULER))
    ctx.clip()
    if (data)
      drawSpectrogram(ctx, data, { left: x(data.start), top: RULER, width: x(data.end) - x(data.start), height: h - RULER })
    if (sel) {
      ctx.fillStyle = 'rgba(255, 255, 255, 0.14)'
      ctx.fillRect(selX, RULER, selW, h - RULER)
      ctx.strokeStyle = '#eee8f5'
      ctx.strokeRect(selX + 0.5, RULER, selW - 1, h - RULER)
    }
    ctx.restore()
  }
  else if (wave.value) {
    const look = { ...waveformLook(settings.settings.editorWaveform), zoom: vZoom.value, lineColor: color('--border'), color: color('--wave-strong') }
    // The wave covers its own range (see readWave), placed under the view.
    const data = wave.value
    const left = x(waveRange.start)
    const area = { top: RULER, width: x(waveRange.end) - left, height: h - RULER, dpr }
    const placed = (l: typeof look) => {
      ctx.save()
      ctx.translate(left, 0)
      drawWave(ctx, data, area, l)
      ctx.restore()
    }
    ctx.save()
    ctx.beginPath()
    ctx.rect(0, 0, w, h)
    ctx.clip()
    placed(look)
    ctx.restore()
    if (sel) {
      ctx.save()
      ctx.beginPath()
      ctx.rect(selX, RULER, selW, h - RULER)
      ctx.clip()
      ctx.fillStyle = color('--primary')
      ctx.fillRect(selX, RULER, selW, h - RULER)
      // Frequency colours stay as they are; the theme colour flips to the canvas colour.
      placed({ ...look, color: color('--card'), lineColor: color('--card'), channelColors: [null, null] })
      ctx.restore()
    }
  }
  else if (sel) {
    ctx.fillStyle = color('--primary-soft')
    ctx.fillRect(selX, RULER, selW, h - RULER)
  }

  // Grid over the wave, so it reads on the wave and the selection alike: bars
  // strongest, then beats (or ruler ticks), then subdivisions.
  if (gridMode.value !== 'off') {
    ctx.fillStyle = color('--foreground')
    for (const f of gridFrames(gridStep(MIN_LINE_PX))) {
      const rel = f - gridOrigin()
      const strength = beatGrid.value
        ? isMultiple(rel, beatFrames.value * 4) ? 0.3 : isMultiple(rel, beatFrames.value) ? 0.16 : 0.07
        : isMultiple(rel, step * rate.value) ? 0.16 : 0.07
      ctx.globalAlpha = strength
      ctx.fillRect(Math.round(x(f)), RULER, 1, h - RULER)
    }
    ctx.globalAlpha = 1
  }

  // Ruler, set off from the wave by a line: bar numbers ("3", "3.2") on the bar
  // grid, otherwise time.
  ctx.fillStyle = color('--muted-foreground')
  ctx.font = '10px system-ui, sans-serif'
  if (beatGrid.value) {
    const labelStep = gridStep(48)
    for (const f of gridFrames(labelStep)) {
      const beat = Math.round((f - downbeat.value) / beatFrames.value)
      const bar = Math.floor(beat / 4) + 1
      const inBar = ((beat % 4) + 4) % 4
      const px = x(f)
      ctx.fillRect(px, RULER - 5, 1 / dpr, 5)
      ctx.fillText(inBar ? `${bar}.${inBar + 1}` : String(bar), px + 3, 11)
    }
  }
  else {
    for (const t of ticks) {
      const px = x(t * rate.value)
      ctx.fillRect(px, RULER - 5, 1 / dpr, 5)
      ctx.fillText(formatTime(t, step < 1), px + 3, 11)
    }
  }
  ctx.fillStyle = color('--border')
  ctx.fillRect(0, RULER, w, 1 / dpr)

  // Loop points: a brand-colour band at the foot of the ruler, dashed edges through the wave.
  const lp = editor.state?.loop
  if (lp) {
    const a = x(lp.start)
    const b = x(lp.end)
    ctx.fillStyle = color('--primary')
    ctx.fillRect(a, RULER - 4, Math.max(1, b - a), 4)
    ctx.strokeStyle = color('--foreground')
    ctx.globalAlpha = 0.7
    ctx.setLineDash([3, 3])
    ctx.lineWidth = 1
    ctx.beginPath()
    for (const px of [Math.round(a) + 0.5, Math.round(b) - 0.5]) {
      ctx.moveTo(px, RULER)
      ctx.lineTo(px, h)
    }
    ctx.stroke()
    ctx.setLineDash([])
    ctx.globalAlpha = 1
  }
  drawSlices(ctx, w, h, color)
  if (isSpectrogram.value)
    drawFrequencyAxis(ctx, rate.value, w, RULER, h - RULER)

  // Cursor and playhead are elements over the canvas (see cursorEl, playheadEl):
  // moving them, every frame while playing, redraws nothing here.
}

watch([
  wave, spectrum, isSpectrogram, viewStart, viewEnd, width, height, vZoom, gridMode, bpm, downbeat,
  () => editor.selection?.start,
  () => editor.selection?.end,
  () => settings.settings.editorWaveform.waveStyle,
  () => settings.settings.editorWaveform.waveScale,
  () => settings.settings.editorWaveform.waveChannels,
  () => settings.settings.editorWaveform.waveColors,
  () => settings.themeRevision,
], scheduleDraw)

// Edit cursor: a thin dimmed line in the text colour. Playhead: the text colour at full
// strength over a canvas-coloured halo, with a marker on the ruler. Positioned through
// style only, so playback does not re-render the component either.
const cursorEl = ref<HTMLElement | null>(null)
const playheadEl = ref<HTMLElement | null>(null)
watchEffect(() => {
  if (cursorEl.value)
    cursorEl.value.style.transform = `translateX(${Math.round(x(editor.cursor))}px)`
})
watchEffect(() => {
  const el = playheadEl.value
  if (!el)
    return
  const p = editor.playhead
  el.style.display = p === null ? 'none' : ''
  if (p !== null)
    el.style.transform = `translateX(${Math.round(x(p))}px)`
})
watch(() => editor.state?.loop, scheduleDraw)

// Under the pointer: a faint line and its time on the ruler (and a mark on the
// minimap), moved through style like the cursor.
const hoverFrame = shallowRef<number | null>(null)
const hoverEl = ref<HTMLElement | null>(null)
const hoverLabel = ref<HTMLElement | null>(null)
watchEffect(() => {
  const el = hoverEl.value
  const label = hoverLabel.value
  if (!el || !label)
    return
  const f = hoverFrame.value
  el.style.display = f === null ? 'none' : ''
  if (f === null)
    return
  const px = Math.round(x(f))
  el.style.transform = `translateX(${px}px)`
  label.textContent = formatTime(f / rate.value)
  // Centre on the line, clamping only when the badge reaches a viewport edge.
  const labelWidth = label.offsetWidth
  const left = Math.max(2, Math.min(width.value - labelWidth - 2, px - labelWidth / 2))
  label.style.transform = `translateX(${left - px}px)`
})

// Slice markers and their regions: a line and a flag on the ruler at each marker,
// skipped regions dimmed, the regions that will be saved numbered. While the slicer
// is open, the markers it would place show dashed.
const draggedMarkers = shallowRef<Marker[] | null>(null)
const shownMarkers = computed(() => draggedMarkers.value ?? editor.state?.markers ?? [])
// Regions Save regions would write.
const playableRegions = computed(() => editor.regions.filter(r => !r.skip))
const savedRegions = computed(() => playableRegions.value.length)
const shownRegions = computed(() => regionsOf(shownMarkers.value, frames.value))

// Region number badges as last drawn, for hovering and clicking them.
let badges: { start: number, end: number, x: number, w: number }[] = []
const BADGE_H = 14

function drawSlices(ctx: CanvasRenderingContext2D, w: number, h: number, color: (name: string) => string) {
  const regions = shownRegions.value
  badges = []
  ctx.fillStyle = color('--card')
  ctx.globalAlpha = 0.65
  for (const r of regions) {
    if (r.skip)
      ctx.fillRect(x(r.start), RULER, x(r.end) - x(r.start), h - RULER)
  }
  ctx.globalAlpha = 1

  ctx.font = '600 10px system-ui, sans-serif'
  let n = 0
  let lastLabel = -Infinity
  for (const r of regions) {
    if (!r.skip)
      n++
    const px = Math.round(x(r.start))
    if (px < -40 || px > w)
      continue
    ctx.fillStyle = color('--foreground')
    // The start of the file is no marker of its own.
    if (r.start > 0 || shownMarkers.value[0]?.frame === 0) {
      ctx.fillRect(px, 0, 1, h)
      ctx.beginPath()
      ctx.moveTo(px, 0)
      ctx.lineTo(px + 6, 3)
      ctx.lineTo(px, 6)
      ctx.closePath()
      ctx.fill()
    }
    if (r.skip)
      continue
    const label = String(n)
    const lw = Math.max(BADGE_H, ctx.measureText(label).width + 6)
    if (px - lastLabel < lw + 4)
      continue
    lastLabel = px
    badges.push({ start: r.start, end: r.end, x: px, w: lw })
    ctx.fillRect(px, RULER + 1, lw, BADGE_H)
    ctx.fillStyle = color('--background')
    // Hovered: a play button.
    if (hoverBadge.value === r.start) {
      const cx = px + lw / 2
      const cy = RULER + 1 + BADGE_H / 2
      ctx.beginPath()
      ctx.moveTo(cx - 2.5, cy - 4)
      ctx.lineTo(cx + 4, cy)
      ctx.lineTo(cx - 2.5, cy + 4)
      ctx.closePath()
      ctx.fill()
    }
    else {
      ctx.fillText(label, px + (lw - ctx.measureText(label).width) / 2, RULER + 12)
    }
  }

  if (slicePreview.value.length) {
    ctx.strokeStyle = color('--foreground')
    ctx.globalAlpha = 0.6
    ctx.setLineDash([4, 3])
    ctx.beginPath()
    for (const m of slicePreview.value) {
      const px = Math.round(x(m.frame)) + 0.5
      if (px >= 0 && px <= w) {
        ctx.moveTo(px, RULER)
        ctx.lineTo(px, h)
      }
    }
    ctx.stroke()
    ctx.setLineDash([])
    ctx.globalAlpha = 1
  }
}
watch([slicePreview, shownMarkers, () => editor.state?.markers], scheduleDraw)

// A region's number badge turns into a play button under the pointer; a click plays
// the region from its start (selecting it, as a click on the ruler does), every time,
// like a sampler pad (so do the number keys, see onKeyDown). Space or the toolbar stops it.
const hoverBadge = ref<number | null>(null)
watch(hoverBadge, scheduleDraw)

function badgeAt(e: MouseEvent) {
  const rect = canvas.value?.getBoundingClientRect()
  if (!rect)
    return undefined
  const px = e.clientX - rect.left
  const py = e.clientY - rect.top
  if (py < RULER + 1 || py > RULER + 1 + BADGE_H)
    return undefined
  return badges.find(b => px >= b.x && px <= b.x + b.w)
}

function playRegion(badge: { start: number, end: number }) {
  editor.stopPlayback()
  editor.selection = { start: badge.start, end: badge.end }
  editor.cursor = badge.start
  editor.play()
}

// --- Mouse ------------------------------------------------------------------

function frameAt(clientX: number) {
  const rect = canvas.value!.getBoundingClientRect()
  const f = viewStart.value + ((clientX - rect.left) / rect.width) * (viewEnd.value - viewStart.value)
  return Math.round(Math.min(frames.value, Math.max(0, f)))
}

// Edges move to the nearest zero crossing, so cuts and loops don't click. Within
// 10 ms (a 50 Hz period); ⌥ inverts it, like grid snapping.
async function toZeroCrossings(e: MouseEvent) {
  if (zeroSnap.value === e.altKey)
    return
  const sel = editor.selection
  const cursor = editor.cursor
  try {
    const radius = Math.round(rate.value * 0.01)
    const [a, b] = await editor.zeroSnap(sel ? [sel.start, sel.end] : [cursor], radius)
    // Unless the user has moved on in the meantime.
    if (editor.selection !== sel || editor.cursor !== cursor)
      return
    if (sel && b > a)
      editor.selection = { start: a, end: b }
    editor.cursor = a
  }
  catch (err) {
    console.error(err)
  }
}

function selectLoop() {
  const lp = editor.state?.loop
  if (lp) {
    editor.selection = { ...lp }
    editor.cursor = lp.start
  }
}

// A selection edge under the pointer (within a few pixels, in the wave area): dragging it resizes the selection.
const EDGE_PX = 5
const hoverEdge = ref<'start' | 'end' | null>(null)

function edgeAt(e: MouseEvent): 'start' | 'end' | null {
  const sel = editor.selection
  const rect = canvas.value?.getBoundingClientRect()
  if (!sel || !rect || e.clientY - rect.top < RULER)
    return null
  const px = e.clientX - rect.left
  const toStart = Math.abs(px - x(sel.start))
  const toEnd = Math.abs(px - x(sel.end))
  if (Math.min(toStart, toEnd) > EDGE_PX)
    return null
  return toStart < toEnd ? 'start' : 'end'
}

// --- Markers ----------------------------------------------------------------

const hoverMarker = ref(false)

/** The marker whose flag is under the pointer (on the ruler, within a few pixels), or -1. */
function markerAt(e: MouseEvent) {
  const rect = canvas.value?.getBoundingClientRect()
  const markers = editor.state?.markers ?? []
  if (!rect || e.clientY - rect.top >= RULER)
    return -1
  const px = e.clientX - rect.left
  let best = -1
  let bestDistance = EDGE_PX + 1
  markers.forEach((m, i) => {
    const d = Math.abs(px - x(m.frame))
    if (d < bestDistance) {
      best = i
      bestDistance = d
    }
  })
  return best
}

const onRuler = (e: MouseEvent) => e.clientY - canvas.value!.getBoundingClientRect().top < RULER

/** Moves frames to zero crossings when that snapping is on (⌥ inverts it). */
async function zeroSnapped(at: number[], e: MouseEvent | KeyboardEvent) {
  if (zeroSnap.value === e.altKey)
    return at
  try {
    return await editor.zeroSnap(at, Math.round(rate.value * 0.01))
  }
  catch {
    return at
  }
}

/** New markers split regions; each half keeps the region's skip. */
function addMarkers(at: number[]) {
  const markers = editor.state?.markers ?? []
  const added = at
    .filter(f => f > 0 && f < frames.value && !markers.some(m => m.frame === f))
    .map(frame => ({ frame, skip: editor.regions.find(r => frame >= r.start && frame < r.end)?.skip ?? false }))
  if (added.length)
    editor.setMarkers([...markers, ...added])
}

function removeMarker(i: number) {
  editor.setMarkers((editor.state?.markers ?? []).filter((_, j) => j !== i))
}

function toggleSkip(region: { start: number, skip: boolean }) {
  const markers = editor.state?.markers ?? []
  // The part before the first marker gets a marker of its own to carry the flag.
  if (!markers.some(m => m.frame === region.start))
    editor.setMarkers([{ frame: region.start, skip: !region.skip }, ...markers])
  else
    editor.setMarkers(markers.map(m => (m.frame === region.start ? { ...m, skip: !m.skip } : m)))
}

/** Drags marker `i` between its neighbours, snapping like the selection. */
function dragMarker(e: MouseEvent, i: number) {
  const markers = editor.state?.markers ?? []
  const lo = i > 0 ? markers[i - 1].frame + 1 : 0
  const hi = (markers[i + 1]?.frame ?? frames.value) - 1
  const startX = e.clientX
  let moved = false
  let at = markers[i].frame
  const place = (f: number) => markers.map((m, j) => (j === i ? { ...m, frame: f } : m))
  const move = (ev: MouseEvent) => {
    if (!moved && Math.abs(ev.clientX - startX) < 3)
      return
    moved = true
    at = Math.min(hi, Math.max(lo, snapped(frameAt(ev.clientX), ev)))
    draggedMarkers.value = place(at)
  }
  const up = async (ev: MouseEvent) => {
    window.removeEventListener('mousemove', move)
    window.removeEventListener('mouseup', up)
    if (moved) {
      const [f] = await zeroSnapped([at], ev)
      await editor.setMarkers(place(Math.min(hi, Math.max(lo, f))))
    }
    draggedMarkers.value = null
  }
  window.addEventListener('mousemove', move)
  window.addEventListener('mouseup', up)
}

async function onDoubleClick(e: MouseEvent) {
  if (onRuler(e) && markerAt(e) < 0)
    addMarkers(await zeroSnapped([snapped(frameAt(e.clientX), e)], e))
}

// --- Context menu -----------------------------------------------------------

// What was right-clicked: a marker's flag, the selection, or any other point,
// which moves the cursor there like a click.
const menuOnRuler = ref(false)
const menu = shallowRef<{ kind: 'marker', index: number } | { kind: 'selection' } | { kind: 'point', frame: number } | null>(null)
// The region starting at the right-clicked marker, for "skip / include".
const menuRegion = computed(() => {
  const m = menu.value
  const marker = m?.kind === 'marker' ? editor.state?.markers[m.index] : undefined
  return marker ? editor.regions.find(r => r.start === marker.frame) : undefined
})

function onContextMenu(e: MouseEvent) {
  menuOnRuler.value = onRuler(e)
  const i = markerAt(e)
  if (i >= 0) {
    menu.value = { kind: 'marker', index: i }
    return
  }
  const f = frameAt(e.clientX)
  const s = editor.selection
  if (s && f >= s.start && f <= s.end) {
    menu.value = { kind: 'selection' }
    return
  }
  editor.selection = null
  editor.cursor = f
  menu.value = { kind: 'point', frame: f }
}

function onHover(e: MouseEvent) {
  hoverBadge.value = badgeAt(e)?.start ?? null
  hoverFrame.value = frameAt(e.clientX)
  hoverEdge.value = edgeAt(e)
  hoverMarker.value = markerAt(e) >= 0
}

let mouseGesture = 0
function onMouseDown(e: MouseEvent) {
  if (e.button !== 0)
    return
  const gesture = ++mouseGesture
  const badge = badgeAt(e)
  if (badge) {
    playRegion(badge)
    return
  }
  // A marker's flag on the ruler drags it.
  const marker = markerAt(e)
  if (marker >= 0) {
    dragMarker(e, marker)
    return
  }
  // The loop band on the ruler selects the loop.
  const lp = editor.state?.loop
  if (lp && e.clientY - canvas.value!.getBoundingClientRect().top < RULER) {
    const f = frameAt(e.clientX)
    if (f >= lp.start && f <= lp.end) {
      selectLoop()
      return
    }
  }
  // A region on the ruler: a click selects it, ⌥-click skips it or takes it back.
  if (editor.regions.length && onRuler(e)) {
    const f = frameAt(e.clientX)
    const region = editor.regions.find(r => f >= r.start && f < r.end)
    if (region) {
      if (e.altKey) {
        toggleSkip(region)
      }
      else {
        editor.selection = { start: region.start, end: region.end }
        editor.cursor = region.start
      }
      return
    }
  }
  // Grabbing an edge keeps the other one as the anchor; otherwise a new selection starts here.
  const edge = e.shiftKey ? null : edgeAt(e)
  const sel = editor.selection
  const anchor = edge && sel
    ? (edge === 'start' ? sel.end : sel.start)
    : e.shiftKey ? editor.cursor : snapped(frameAt(e.clientX), e)
  const startX = e.clientX
  let dragged = false
  const move = (ev: MouseEvent) => {
    if (Math.abs(ev.clientX - startX) < 3 && !dragged)
      return
    dragged = true
    const f = snapped(frameAt(ev.clientX), ev)
    editor.selection = f === anchor ? null : { start: Math.min(f, anchor), end: Math.max(f, anchor) }
  }
  const up = async (ev: MouseEvent) => {
    window.removeEventListener('mousemove', move)
    window.removeEventListener('mouseup', up)
    // The last mousemove may lag behind the release point.
    if (dragged)
      move(ev)
    // A click on an edge without moving leaves the selection as it is.
    else if (edge)
      return
    if (!dragged) {
      const f = snapped(frameAt(ev.clientX), ev)
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
    await toZeroCrossings(ev)
    if (gesture !== mouseGesture)
      return
    const selected = editor.selection
    const changedRange = selected && (!sel || selected.start !== sel.start || selected.end !== sel.end)
    // An active audition follows the edited selection, including its new end and loop bounds.
    if (editor.playing && changedRange)
      void editor.updatePlaybackRange()
    // Creating a selection while stopped remains silent; a plain click auditions the cursor.
    else if (!dragged && !e.shiftKey && e.detail < 2 && !selected && editor.cursor < frames.value)
      void editor.play()
  }
  window.addEventListener('mousemove', move)
  window.addEventListener('mouseup', up)
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

// --- Keyboard ---------------------------------------------------------------

function onKeyDown(e: KeyboardEvent) {
  const mod = e.metaKey || e.ctrlKey
  const key = e.key.toLowerCase()
  const handled = () => e.preventDefault()
  if (e.key === ' ') {
    handled()
    editor.togglePlay()
  }
  // 1…9 play regions 1–9 like pads, ⇧1…⇧9 regions 10–18 (by key position, so any layout).
  else if (!mod && !e.altKey && /^Digit[1-9]$/.test(e.code)) {
    const n = Number(e.code.slice(5)) + (e.shiftKey ? 9 : 0)
    const region = playableRegions.value[n - 1]
    if (region) {
      handled()
      playRegion(region)
    }
  }
  else if (mod && key === 'z') {
    handled()
    e.shiftKey ? editor.redo() : editor.undo()
  }
  else if (mod && key === 's') {
    handled()
    editor.save(e.shiftKey ? 'saveAs' : 'save')
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
  else if (!mod && e.shiftKey && key === 'l') {
    editor.toggleLoopPoints()
  }
  else if (!mod && key === 'l') {
    editor.looping = !editor.looping
  }
  else if (!mod && (key === 'm' || key === 'ь')) {
    const sel = editor.selection
    addMarkers(sel ? [sel.start, sel.end] : [editor.cursor])
  }
  else if (!mod && key === 's') {
    snap.value = !snap.value
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

// Cropping moves the selection start to 0; the bars move with the audio.
function crop() {
  if (editor.selection)
    downbeat.value -= editor.selection.start
  editor.crop()
}

function setBpm(value: string | number) {
  const n = Number(value)
  bpm.value = value !== '' && Number.isFinite(n) && n >= 20 && n <= 999 ? n : null
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
  const lp = editor.state?.loop
  const range = (r: { start: number, end: number }) => `${formatTime(r.start / sr)} – ${formatTime(r.end / sr)} (${formatTime((r.end - r.start) / sr)})`
  return {
    cursor: formatTime(editor.cursor / sr),
    selection: sel ? range(sel) : '',
    loop: lp ? range(lp) : '',
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
  <div class="h-full flex flex-col overflow-hidden bg-background text-foreground" :inert="editor.loading || editor.saving" data-editor-panel>
    <!-- Keep a drag region for the legacy standalone editor window. -->
    <header v-if="!props.embedded" class="h-11 shrink-0 pl-[84px] pr-3 flex items-center border-b border-border" data-tauri-drag-region>
      <span class="min-w-0 truncate text-sm font-medium" data-tauri-drag-region>{{ editor.sample?.name }}<span class="text-muted-foreground">.{{ editor.sample?.ext }}</span><span v-if="editor.state?.dirty" class="text-primary"> •</span></span>
    </header>

    <div class="h-11 shrink-0 px-3 flex items-center gap-1 border-b border-border">
      <ToolbarOverflow :items="toolbarTools">
        <template #default="{ item }">
          <Hint v-if="item === 'undo'" :text="t('editor.toolbar.undo')" shortcut="⌘Z">
              <Button variant="ghost" size="icon-sm" :disabled="!editor.state?.canUndo" :aria-label="t('editor.toolbar.undo')" @click="editor.undo()">
                <Undo2 />
              </Button>
          </Hint>
          <Hint v-if="item === 'redo'" :text="t('editor.toolbar.redo')" shortcut="⇧⌘Z">
              <Button variant="ghost" size="icon-sm" :disabled="!editor.state?.canRedo" :aria-label="t('editor.toolbar.redo')" @click="editor.redo()">
                <Redo2 />
              </Button>
          </Hint>
          <Hint v-if="item === 'play'" :text="editor.playing ? t('editor.toolbar.stop') : editor.selection ? t('editor.toolbar.playSelection') : t('editor.toolbar.playFromCursor')" :shortcut="t('editor.toolbar.space')">
              <Button variant="ghost" size="icon-sm" :aria-label="t(editor.playing ? 'editor.toolbar.stop' : 'common.actions.play')" @click="editor.togglePlay()">
                <component :is="editor.playing ? Square : Play" />
              </Button>
          </Hint>
          <Hint v-if="item === 'loop'" :text="t('editor.toolbar.loopHint')" shortcut="L">
              <Toggle v-model="editor.looping" size="sm" :class="toggleIcon" :aria-label="t('editor.toolbar.loop')">
                <Repeat />
              </Toggle>
          </Hint>
          <Hint v-if="item === 'loopPoints'" :text="editor.selection ? t('editor.toolbar.setLoopHint') : t('editor.toolbar.clearLoopHint')" shortcut="⇧L">
              <Button
                variant="ghost" size="icon-sm"
                :class="editor.state?.loop && 'text-primary hover:text-primary'"
                :disabled="!editor.selection && !editor.state?.loop"
                :aria-label="t('editor.toolbar.loopPoints')"
                @click="editor.toggleLoopPoints()"
              >
                <IterationCw />
              </Button>
          </Hint>
          <Hint v-if="item === 'crop'" :text="t('editor.toolbar.cropHint')">
              <Button variant="ghost" size="icon-sm" :disabled="!editor.selection" :aria-label="t('editor.toolbar.crop')" @click="crop()">
                <Crop />
              </Button>
          </Hint>
          <Hint v-if="item === 'delete'" :text="t('editor.toolbar.deleteHint')" shortcut="⌫">
              <Button variant="ghost" size="icon-sm" :disabled="!editor.selection" :aria-label="t('editor.toolbar.delete')" @click="editor.remove()">
                <Eraser />
              </Button>
          </Hint>
          <Hint v-if="item === 'cut'" :text="t('editor.toolbar.cutHint')" shortcut="⌘X">
              <Button variant="ghost" size="icon-sm" :disabled="!editor.selection" :aria-label="t('editor.toolbar.cut')" @click="editor.cut()">
                <Scissors />
              </Button>
          </Hint>
          <Hint v-if="item === 'copy'" :text="t('editor.toolbar.copyHint')" shortcut="⌘C">
              <Button variant="ghost" size="icon-sm" :disabled="!editor.selection" :aria-label="t('editor.toolbar.copy')" @click="editor.copy()">
                <Copy />
              </Button>
          </Hint>
          <Hint v-if="item === 'paste'" :text="editor.selection ? t('editor.toolbar.pasteOverHint') : t('editor.toolbar.pasteHint')" shortcut="⌘V">
              <Button variant="ghost" size="icon-sm" :disabled="!editor.clipboard" :aria-label="t('editor.toolbar.paste')" @click="editor.paste()">
                <ClipboardPaste />
              </Button>
          </Hint>
          <Hint v-if="item === 'silence'" :text="t('editor.toolbar.silenceHint')">
              <Button variant="ghost" size="icon-sm" :disabled="!editor.selection" @click="editor.silence()">
                <VolumeX />
              </Button>
          </Hint>
          <Hint v-if="item === 'fadeIn'" :text="t('editor.toolbar.fadeInHint')">
              <Button variant="ghost" size="icon-sm" :aria-label="t('editor.toolbar.fadeIn')" @click="editor.fade('in')">
                <FadeIcon direction="in" :curve="editor.curve" />
              </Button>
          </Hint>
          <Hint v-if="item === 'fadeOut'" :text="t('editor.toolbar.fadeOutHint')">
              <Button variant="ghost" size="icon-sm" :aria-label="t('editor.toolbar.fadeOut')" @click="editor.fade('out')">
                <FadeIcon direction="out" :curve="editor.curve" />
              </Button>
          </Hint>
          <Hint v-if="item === 'curve'" :text="t('editor.toolbar.fadeCurve')">
              <span class="inline-flex">
                <Select v-model="editor.curve">
                  <SelectTrigger size="sm" class="gap-1 px-2 border-transparent bg-transparent shadow-none dark:bg-transparent hover:bg-accent" :aria-label="t('editor.toolbar.fadeCurve')">
                    <SelectValue>
                      <FadeIcon :curve="editor.curve" class="size-4 text-foreground" />
                    </SelectValue>
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem v-for="c in curves" :key="c.value" :value="c.value">
                      <FadeIcon :curve="c.value" class="size-4" /> {{ t(c.label) }}
                    </SelectItem>
                  </SelectContent>
                </Select>
              </span>
          </Hint>
          <Hint v-if="item === 'normalize'" :text="t('editor.toolbar.normalizeHint')">
              <Button variant="ghost" size="icon-sm" :aria-label="t('editor.toolbar.normalize')" @click="editor.normalize()">
                <ArrowUpToLine />
              </Button>
          </Hint>
          <Hint v-if="item === 'gainDown'" :text="t('editor.toolbar.gainDownHint')">
              <Button variant="ghost" size="icon-sm" :aria-label="t('editor.toolbar.gainDownHint')" @click="editor.gain(-3)">
                <Volume1 />
              </Button>
          </Hint>
          <Hint v-if="item === 'gainUp'" :text="t('editor.toolbar.gainUpHint')">
              <Button variant="ghost" size="icon-sm" :aria-label="t('editor.toolbar.gainUpHint')" @click="editor.gain(3)">
                <Volume2 />
              </Button>
          </Hint>
          <Hint v-if="item === 'reverse'" :text="t('editor.toolbar.reverseHint')">
              <Button variant="ghost" size="icon-sm" @click="editor.reverse()">
                <ArrowLeftRight />
              </Button>
          </Hint>
          <EditorProcessing v-if="item === 'stretch'" tool="stretch" />
          <EditorProcessing v-if="item === 'lufs'" tool="lufs" />
          <EditorProcessing v-if="item === 'dc'" tool="dc" />
          <Hint v-if="item === 'midi'" :text="t('basicPitch.action')">
              <Button variant="ghost" size="sm" class="px-2" :disabled="!editor.sample || !frames || editor.processing" :aria-label="t('basicPitch.action')" @click="exportMidi">
                <Music /> MIDI
              </Button>
          </Hint>
          <EditorSlicer v-if="item === 'slice'" v-model:preview="slicePreview" :beat-frames="beatFrames" :downbeat="downbeat" :zero="zeroSnap" />
          <Hint v-if="item === 'regions'" :text="savedRegions ? t('editor.regions.saveHint', { n: formatNumber(savedRegions) }, savedRegions) : t('editor.regions.none')">
              <Button variant="ghost" size="sm" class="px-2" :disabled="!savedRegions" :aria-label="t('editor.regions.save')" @click="editor.saveRegions()">
                <FolderOutput />
                <span v-if="savedRegions" class="tabular">{{ formatNumber(savedRegions) }}</span>
              </Button>
          </Hint>
        </template>
      </ToolbarOverflow>
      <div class="shrink-0 flex items-center gap-1 pl-2 border-l border-border">
        <EditorSaveMenu />
        <Button v-if="props.embedded" variant="ghost" size="icon-sm" :aria-label="t(props.expanded ? 'editor.panel.collapse' : 'editor.panel.expand')" @click="emit('toggleExpand')">
          <Minimize2 v-if="props.expanded" /><Maximize2 v-else />
        </Button>
        <Button v-if="props.embedded" variant="ghost" size="icon-sm" :aria-label="t('common.actions.close')" @click="emit('close')"><X /></Button>
      </div>
    </div>

    <EditorMinimap
      :view-start="viewStart"
      :view-end="viewEnd"
      :hover-frame="hoverFrame"
      @view="(a, b) => { deferFollow(); viewStart = a; viewEnd = b }"
      @zoom="zoom"
      @fit="fit()"
    />
    <!-- Both displays share the canvas interactions and overlays. -->
    <div class="flex-1 min-h-0 flex">
    <div ref="box" class="relative flex-1 min-w-0 bg-card">
      <!-- Cut, copy and paste come as DOM events: the native Edit menu takes ⌘X/⌘C/⌘V before keydown. -->
      <ContextMenu>
        <ContextMenuTrigger as-child>
          <canvas
            ref="canvas"
            tabindex="0"
            class="absolute inset-0 size-full outline-none"
            :class="hoverBadge !== null ? 'cursor-pointer' : hoverEdge || hoverMarker ? 'cursor-ew-resize' : 'cursor-text'"
            :aria-label="t(isSpectrogram ? 'editor.spectrogram.label' : 'editor.waveform.label')"
            @mousedown="onMouseDown"
            @copy.prevent="editor.copy()"
            @cut.prevent="editor.cut()"
            @paste.prevent="editor.paste()"
            @dblclick="onDoubleClick"
            @contextmenu="onContextMenu"
            @mousemove="onHover"
            @mouseleave="hoverEdge = null; hoverMarker = false; hoverFrame = null; hoverBadge = null"
            @wheel="onWheel"
            @keydown="onKeyDown"
          />
        </ContextMenuTrigger>
        <ContextMenuContent class="min-w-56">
          <!-- On a marker's flag -->
          <template v-if="menu?.kind === 'marker'">
            <ContextMenuItem v-if="menuRegion" @select="toggleSkip(menuRegion)">
              {{ menuRegion.skip ? t('editor.menu.includeRegion') : t('editor.menu.skipRegion') }}
            </ContextMenuItem>
            <ContextMenuItem class="text-destructive focus:text-destructive" @select="removeMarker(menu.index)">
              {{ t('editor.menu.deleteMarker') }}
            </ContextMenuItem>
          </template>
          <!-- Inside the selection -->
          <template v-else-if="menu?.kind === 'selection'">
            <ContextMenuItem @select="editor.play()">
              {{ t('editor.toolbar.playSelection') }}<ContextMenuShortcut>{{ t('editor.toolbar.space') }}</ContextMenuShortcut>
            </ContextMenuItem>
            <ContextMenuCheckboxItem :model-value="editor.looping" @update:model-value="editor.looping = $event === true">
              {{ t('editor.toolbar.loop') }}<ContextMenuShortcut>L</ContextMenuShortcut>
            </ContextMenuCheckboxItem>
            <ContextMenuSeparator />
            <ContextMenuItem @select="editor.cut()">
              {{ t('editor.toolbar.cut') }}<ContextMenuShortcut>⌘X</ContextMenuShortcut>
            </ContextMenuItem>
            <ContextMenuItem @select="editor.copy()">
              {{ t('editor.toolbar.copy') }}<ContextMenuShortcut>⌘C</ContextMenuShortcut>
            </ContextMenuItem>
            <ContextMenuItem :disabled="!editor.clipboard" @select="editor.paste()">
              {{ t('editor.menu.pasteOver') }}<ContextMenuShortcut>⌘V</ContextMenuShortcut>
            </ContextMenuItem>
            <ContextMenuItem @select="editor.remove()">
              {{ t('editor.toolbar.delete') }}<ContextMenuShortcut>⌫</ContextMenuShortcut>
            </ContextMenuItem>
            <ContextMenuItem @select="crop()">
              {{ t('editor.toolbar.cropHint') }}
            </ContextMenuItem>
            <ContextMenuItem @select="editor.silence()">
              {{ t('editor.menu.silence') }}
            </ContextMenuItem>
            <ContextMenuSeparator />
            <ContextMenuItem @select="editor.fade('in')">
              {{ t('editor.toolbar.fadeIn') }}
            </ContextMenuItem>
            <ContextMenuItem @select="editor.fade('out')">
              {{ t('editor.toolbar.fadeOut') }}
            </ContextMenuItem>
            <ContextMenuItem @select="editor.gain(3)">
              {{ t('editor.menu.gainUp') }}
            </ContextMenuItem>
            <ContextMenuItem @select="editor.gain(-3)">
              {{ t('editor.menu.gainDown') }}
            </ContextMenuItem>
            <ContextMenuItem @select="editor.normalize()">
              {{ t('editor.toolbar.normalize') }}
            </ContextMenuItem>
            <ContextMenuItem @select="editor.removeDcOffset()">
              {{ t('editor.processing.dcOffset') }}
            </ContextMenuItem>
            <ContextMenuItem @select="editor.reverse()">
              {{ t('editor.menu.reverse') }}
            </ContextMenuItem>
            <ContextMenuSeparator />
            <ContextMenuItem @select="addMarkers([editor.selection!.start, editor.selection!.end])">
              {{ t('editor.menu.markEdges') }}<ContextMenuShortcut>M</ContextMenuShortcut>
            </ContextMenuItem>
            <ContextMenuItem @select="editor.toggleLoopPoints()">
              {{ t('editor.menu.setLoopPoints') }}<ContextMenuShortcut>⇧L</ContextMenuShortcut>
            </ContextMenuItem>
            <ContextMenuItem @select="zoomToSelection()">
              {{ t('editor.menu.zoomToSelection') }}
            </ContextMenuItem>
            <ContextMenuItem @select="editor.selection = null">
              {{ t('editor.menu.deselect') }}<ContextMenuShortcut>Esc</ContextMenuShortcut>
            </ContextMenuItem>
          </template>
          <!-- Anywhere else: the cursor moved to the click -->
          <template v-else-if="menu?.kind === 'point'">
            <ContextMenuItem @select="editor.play()">
              {{ t('editor.menu.playFromHere') }}<ContextMenuShortcut>{{ t('editor.toolbar.space') }}</ContextMenuShortcut>
            </ContextMenuItem>
            <ContextMenuItem :disabled="!editor.clipboard" @select="editor.paste()">
              {{ t('editor.menu.pasteHere') }}<ContextMenuShortcut>⌘V</ContextMenuShortcut>
            </ContextMenuItem>
            <ContextMenuItem @select="addMarkers([menu.frame])">
              {{ t('editor.menu.addMarker') }}<ContextMenuShortcut>M</ContextMenuShortcut>
            </ContextMenuItem>
            <ContextMenuSeparator />
            <ContextMenuItem v-if="editor.state?.loop" @select="selectLoop()">
              {{ t('editor.menu.selectLoop') }}
            </ContextMenuItem>
            <ContextMenuItem @select="editor.selection = { start: 0, end: frames }">
              {{ t('editor.menu.selectAll') }}<ContextMenuShortcut>⌘A</ContextMenuShortcut>
            </ContextMenuItem>
            <ContextMenuItem @select="fit()">
              {{ t('editor.toolbar.fit') }}<ContextMenuShortcut>0</ContextMenuShortcut>
            </ContextMenuItem>
          </template>
          <template v-if="menuOnRuler">
            <ContextMenuSeparator />
            <ContextMenuItem @select="openGridTempo()">{{ t('editor.view.tempo') }}…</ContextMenuItem>
          </template>
        </ContextMenuContent>
      </ContextMenu>
      <div class="pointer-events-none absolute inset-0 overflow-hidden" aria-hidden="true">
        <div ref="cursorEl" class="absolute left-0 bottom-0 w-px" :class="isSpectrogram ? 'bg-white/60' : 'bg-foreground/60'" :style="{ top: `${RULER}px` }" />
        <div ref="hoverEl" class="absolute left-0 top-0 bottom-0" style="display: none">
          <div class="absolute left-0 bottom-0 w-px" :class="isSpectrogram ? 'bg-white/35' : 'bg-foreground/35'" :style="{ top: `${RULER}px` }" />
          <span ref="hoverLabel" class="absolute top-px px-1 rounded-sm bg-foreground text-background text-[10px] leading-4 tabular whitespace-nowrap" />
        </div>
        <div ref="playheadEl" class="absolute left-0 top-0 bottom-0" :class="isSpectrogram ? 'text-white' : 'text-foreground'" style="display: none">
          <div class="absolute -left-px bottom-0 w-0.5 bg-current shadow-[0_0_0_1px_var(--card)]" :style="{ top: `${RULER}px` }" />
          <svg class="absolute -left-[5px]" :style="{ top: `${RULER - 7}px` }" width="10" height="7" viewBox="0 0 10 7">
            <path d="M0 0H10L5 7Z" fill="currentColor" />
          </svg>
        </div>
      </div>
      <div v-if="editor.sample" class="pointer-events-none absolute bottom-2 left-2 max-w-[min(70%,28rem)] truncate rounded bg-background/80 px-2 py-1 text-xs font-medium text-foreground" :title="`${editor.sample.name}.${editor.sample.ext}`">
        {{ editor.sample.name }}<span class="text-muted-foreground">.{{ editor.sample.ext }}</span><span v-if="editor.state?.dirty" class="text-primary"> •</span>
      </div>
      <div
        v-if="isSpectrogram && spectrum && !spectrumError"
        class="pointer-events-none absolute bottom-2 right-14 flex items-center gap-2 rounded bg-black/70 px-2 py-1 text-[10px] text-white/75 tabular"
        :aria-label="t('editor.spectrogram.legend')"
      >
        <span>−100 dB</span>
        <span class="w-12 h-1.5 rounded-full" style="background: linear-gradient(90deg, #080511, #6f2687, #e44f55, #fc9d39, #fcfabe)" />
        <span>0 dB</span>
      </div>
      <div v-if="editor.loading" class="absolute inset-0 grid place-items-center text-muted-foreground text-sm">
        {{ t('editor.waveform.opening') }}
      </div>
      <div v-else-if="editor.error" class="absolute inset-0 grid place-items-center text-destructive text-sm px-6 text-center">
        {{ editor.error }}
      </div>
    </div>
    <LevelMeter :active="editor.playhead !== null" :split-channels="!isSpectrogram && settings.settings.editorWaveform.waveChannels === 'split' && (editor.state?.channels ?? 1) > 1" :top-inset="RULER" />
    </div>

    <footer class="h-9 shrink-0 px-4 flex items-center gap-4 border-t border-border text-xs text-muted-foreground tabular">
      <div class="min-w-0 flex-1 flex items-center gap-4 overflow-hidden">
        <span class="shrink-0">{{ t('editor.status.cursor', { time: status.cursor }) }}</span>
        <label v-if="gridMode === 'beats'" ref="gridTempoRow" class="shrink-0 inline-flex items-center gap-2">
          <span>{{ t('editor.view.tempo') }}</span>
          <Input
            :model-value="bpm ?? ''" type="number" min="20" max="999" step="0.01"
            :placeholder="t('editor.toolbar.bpm')" :aria-label="t('editor.view.tempo')"
            class="h-6 w-16 px-1.5 text-xs tabular" @update:model-value="setBpm"
          />
        </label>
        <button
          v-if="vZoom > 1.01" class="shrink-0 hover:text-foreground"
          :title="t('editor.status.vZoom')"
          @click="vZoom = 1"
        >
          ↕ ×{{ formatNumber(vZoom, { fixed: 1 }) }}
        </button>
        <span v-if="status.selection" class="min-w-0 truncate" :title="t('editor.status.selection', { range: status.selection })">{{ t('editor.status.selection', { range: status.selection }) }}</span>
        <button v-if="status.loop" class="shrink-0 hover:text-foreground" :title="t('editor.status.selectLoop')" @click="selectLoop()">
          {{ t('editor.status.loop', { range: status.loop }) }}
        </button>
        <span class="ml-auto min-w-0 truncate">{{ status.length }} · {{ formatRate(rate) }} · {{ editor.state?.channels === 1 ? t('common.channels.mono') : t('common.channels.stereo') }}</span>
      </div>
      <Separator orientation="vertical" class="h-3" />
      <!-- View controls: beat grid, snapping, look and zoom. -->
      <div class="footer-controls shrink-0 flex items-center gap-1.5 text-foreground">
        <EditorDisplayControls v-model="displayMode" :loading="spectrumLoading" :error="spectrumError" @retry="refreshSpectrum" />
        <Separator orientation="vertical" class="mx-0.5 h-3" />
        <Hint :text="t('editor.toolbar.snapHint')" shortcut="S">
          <Toggle v-model="snap" size="sm" :class="toggleIcon" :disabled="gridMode === 'off'" :aria-label="t('editor.toolbar.snap')">
            <Magnet />
          </Toggle>
        </Hint>
        <!-- The tooltip wraps the whole picker: inside it, it would take the popup's anchor. -->
        <Hint :text="t('editor.toolbar.view')">
          <span class="inline-flex">
            <DropdownMenu>
              <DropdownMenuTrigger as-child>
                <Button variant="ghost" size="icon-sm" class="data-[state=open]:bg-accent" :aria-label="t('editor.toolbar.view')">
                  <SlidersHorizontal />
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end" side="top" class="w-60">
                <DropdownMenuLabel class="text-xs text-muted-foreground font-normal">
                  {{ t('editor.view.grid') }}
                </DropdownMenuLabel>
                <DropdownMenuRadioGroup v-model="gridMode">
                  <DropdownMenuRadioItem value="beats" @select.prevent>
                    {{ t('editor.view.gridBeats') }}
                  </DropdownMenuRadioItem>
                  <DropdownMenuRadioItem value="time" @select.prevent>
                    {{ t('editor.view.gridTime') }}
                  </DropdownMenuRadioItem>
                  <DropdownMenuRadioItem value="off" @select.prevent>
                    {{ t('editor.view.gridOff') }}
                  </DropdownMenuRadioItem>
                </DropdownMenuRadioGroup>
                <DropdownMenuItem :disabled="!beatGrid" @select="downbeat = editor.cursor">
                  {{ t('editor.view.downbeat') }}
                </DropdownMenuItem>
                <DropdownMenuCheckboxItem v-model="zeroSnap" @select.prevent>
                  {{ t('editor.view.zeroSnap') }}
                </DropdownMenuCheckboxItem>
                <DropdownMenuSeparator />
                <template v-for="(opt, i) in viewOptions" :key="opt.key">
                  <DropdownMenuSeparator v-if="i" />
                  <DropdownMenuLabel class="text-xs text-muted-foreground font-normal">
                    {{ t(opt.label) }}
                  </DropdownMenuLabel>
                  <DropdownMenuRadioGroup :model-value="settings.settings.editorWaveform[opt.key]" @update:model-value="setView(opt.key, $event)">
                    <DropdownMenuRadioItem v-for="c in opt.choices" :key="c.value" :value="c.value" @select.prevent>
                      {{ t(c.label) }}
                    </DropdownMenuRadioItem>
                  </DropdownMenuRadioGroup>
                </template>
                <DropdownMenuSeparator />
                <DropdownMenuItem @select="settings.openDialog('waveform')">
                  {{ t('settings.waveform.moreSettings') }}
                </DropdownMenuItem>
                <DropdownMenuItem :disabled="vZoom <= 1.01" @select="vZoom = 1">
                  {{ t('editor.view.resetVZoom') }}
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
          </span>
        </Hint>
        <Separator orientation="vertical" class="mx-0.5 h-3" />
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
    </footer>
  </div>
</template>

<style scoped>
.footer-controls :deep(button) {
  width: 28px;
  min-width: 28px;
  height: 24px;
  padding: 0;
}
.footer-controls :deep(button svg) {
  width: 12px;
  height: 12px;
}
</style>
