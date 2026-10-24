<script setup lang="ts">
import type { Curve, Marker, Wave } from '@/stores/editor'
import {
  ArrowLeftRight,
  ArrowUpToLine,
  ClipboardPaste,
  Copy,
  Crop,
  Eraser,
  FolderOpen,
  FolderOutput,
  IterationCw,
  Magnet,
  Maximize2,
  Play,
  Redo2,
  Repeat,
  Save,
  Scissors,
  Square,
  SlidersHorizontal,
  Undo2,
  Volume1,
  Volume2,
  VolumeX,
  ZoomIn,
  ZoomOut,
} from '@lucide/vue'
import { revealItemInDir } from '@tauri-apps/plugin-opener'
import { useLocalStorage, useResizeObserver } from '@vueuse/core'
import { computed, nextTick, ref, shallowRef, watch, watchEffect } from 'vue'
import { useI18n } from 'vue-i18n'
import EditorMinimap from '@/components/EditorMinimap.vue'
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
import { formatNumber, formatRate, shortPath } from '@/format'
import { canvasContext, drawWave } from '@/waveform'
import { regionsOf, useEditorStore } from '@/stores/editor'
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
// Toggles look like the ghost icon buttons around them; switched on, the icon takes
// the brand colour instead of getting a background. Styled by aria-pressed: the
// tooltip trigger wrapping them overwrites their data-state.
const toggleIcon = 'size-8 px-0 hover:bg-accent hover:text-accent-foreground dark:hover:bg-accent/50 aria-pressed:bg-transparent aria-pressed:text-primary aria-pressed:hover:text-primary aria-pressed:hover:bg-accent dark:aria-pressed:hover:bg-accent/50'
// Per-window preferences, not worth fields in settings.json.
type GridMode = 'beats' | 'time' | 'off'
const gridMode = useLocalStorage<GridMode>('editor.gridMode', 'beats')
const snap = useLocalStorage('editor.snap', true)
const zeroSnap = useLocalStorage('editor.zeroSnap', true)
// Slices shown while the slicer is open.
const slicePreview = ref<Marker[]>([])
// Tempo of the bar grid (from the sample, editable) and where bar 1 starts, in frames.
const bpm = ref<number | null>(null)
const downbeat = ref(0)

// The View menu: the same waveform options as in settings, applied right away.
type WaveKey = 'waveStyle' | 'waveScale' | 'waveChannels' | 'waveColors'
const viewOptions: { key: WaveKey, label: string, choices: { value: string, label: string }[] }[] = [
  { key: 'waveStyle', label: 'settings.waveform.style', choices: [{ value: 'peaks', label: 'settings.waveform.peaks' }, { value: 'peaksRms', label: 'settings.waveform.peaksRms' }, { value: 'bars', label: 'settings.waveform.bars' }] },
  { key: 'waveChannels', label: 'settings.waveform.channels', choices: [{ value: 'split', label: 'settings.waveform.split' }, { value: 'mix', label: 'settings.waveform.mix' }] },
  { key: 'waveScale', label: 'settings.waveform.scale', choices: [{ value: 'linear', label: 'settings.waveform.linear' }, { value: 'db', label: 'settings.waveform.db' }] },
  { key: 'waveColors', label: 'settings.waveform.colors', choices: [{ value: 'theme', label: 'settings.waveform.theme' }, { value: 'frequency', label: 'settings.waveform.frequency' }] },
]

function setView(key: WaveKey, value: unknown) {
  if (typeof value === 'string' && value !== settings.settings[key])
    settings.update({ [key]: value })
}

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
  bpm.value = editor.sample?.bpm ?? null
  downbeat.value = 0
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
  const s = settings.settings
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
      if (!editor.open || width.value === 0 || waveFits())
        continue
      const n = editor.state?.frames ?? 0
      const span = viewEnd.value - viewStart.value
      const wide = Math.min(n, span * OVERSCAN)
      const start = Math.floor(Math.max(0, Math.min(n - wide, viewStart.value - (wide - span) / 2)))
      const end = Math.ceil(Math.min(n, start + wide))
      const columns = Math.min(16384, Math.round(width.value * (window.devicePixelRatio || 1) * (end - start) / span))
      const key = waveKey()
      const s = settings.settings
      const data = await editor.wave(start, end, columns, s.waveChannels === 'split', s.waveColors === 'frequency')
      waveRange = { start, end, key }
      wave.value = data
    } while (readAgain)
  }
  finally {
    reading = false
  }
}

watch([() => editor.revision, viewStart, viewEnd, width, () => settings.settings.waveChannels, () => settings.settings.waveColors], readWave)

const x = (frame: number) => ((frame - viewStart.value) / (viewEnd.value - viewStart.value)) * width.value

function tickStep(span: number) {
  const steps = [0.001, 0.005, 0.01, 0.05, 0.1, 0.25, 0.5, 1, 2, 5, 10, 15, 30, 60, 120, 300]
  const seconds = span / rate.value
  return steps.find(s => (s / seconds) * width.value >= 80) ?? 600
}

// --- Grid -------------------------------------------------------------------

const beatFrames = computed(() => (bpm.value && bpm.value > 0 ? (60 / bpm.value) * rate.value : 0))
// Bars and beats when the tempo is known, otherwise seconds. 4/4 assumed.
const beatGrid = computed(() => gridMode.value === 'beats' && beatFrames.value > 0)
const BEAT_STEPS = [1 / 8, 1 / 4, 1 / 2, 1, 4, 8, 16, 32, 64, 128, 256]
const TIME_STEPS = [0.001, 0.005, 0.01, 0.05, 0.1, 0.25, 0.5, 1, 2, 5, 10, 15, 30, 60, 120, 300, 600]
const MIN_LINE_PX = 12

/** The finest grid step, in frames, that is at least `minPx` wide on screen. */
function gridStep(minPx: number) {
  const pxPerFrame = width.value / (viewEnd.value - viewStart.value)
  const unit = beatGrid.value ? beatFrames.value : rate.value
  const steps = beatGrid.value ? BEAT_STEPS : TIME_STEPS
  return (steps.find(s => s * unit * pxPerFrame >= minPx) ?? steps.at(-1)!) * unit
}

const gridOrigin = () => (beatGrid.value ? downbeat.value : 0)

/** Visible multiples of `step` from the grid origin. */
function gridFrames(step: number) {
  const origin = gridOrigin()
  const out: number[] = []
  for (let k = Math.ceil((viewStart.value - origin) / step); origin + k * step <= viewEnd.value; k++)
    out.push(origin + k * step)
  return out
}

const isMultiple = (value: number, of: number) => Math.abs(value / of - Math.round(value / of)) < 1e-6

/** Snaps a frame to the nearest grid line (and to the end of the file) while snapping is on; ⌥ inverts it. */
function snapped(frame: number, e: MouseEvent) {
  if (gridMode.value === 'off' || snap.value === e.altKey)
    return frame
  const step = gridStep(MIN_LINE_PX)
  const origin = gridOrigin()
  const line = origin + Math.round((frame - origin) / step) * step
  const nearEnd = Math.abs(frames.value - frame) < Math.abs(line - frame)
  return Math.round(Math.min(frames.value, Math.max(0, nearEnd ? frames.value : line)))
}

function draw() {
  const el = canvas.value
  if (!el || width.value === 0)
    return
  const dpr = window.devicePixelRatio || 1
  const w = width.value
  const h = height.value
  const ctx = canvasContext(el, w, h, dpr)
  const style = getComputedStyle(el)
  const color = (name: string) => style.getPropertyValue(name).trim()
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
  if (wave.value) {
    const { waveStyle, waveScale, waveColors } = settings.settings
    const look = { style: waveStyle, scale: waveScale, colors: waveColors, zoom: vZoom.value, lineColor: color('--border'), color: color('--wave-strong') }
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
      placed({ ...look, color: color('--card'), lineColor: color('--card') })
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

  // Cursor and playhead are elements over the canvas (see cursorEl, playheadEl):
  // moving them, every frame while playing, redraws nothing here.
}

watch([wave, viewStart, viewEnd, () => editor.selection, width, height, vZoom, gridMode, bpm, downbeat, () => settings.settings, () => settings.themeRevision], draw, { deep: true })

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
watch(() => editor.state?.loop, draw)

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
  // Near the right edge the time goes to the left of the line.
  label.style.transform = px > width.value - 72 ? 'translateX(calc(-100% - 4px))' : 'translateX(4px)'
})

// Slice markers and their regions: a line and a flag on the ruler at each marker,
// skipped regions dimmed, the regions that will be saved numbered. While the slicer
// is open, the markers it would place show dashed.
const draggedMarkers = shallowRef<Marker[] | null>(null)
const shownMarkers = computed(() => draggedMarkers.value ?? editor.state?.markers ?? [])
// Regions Save regions would write.
const savedRegions = computed(() => editor.regions.filter(r => !r.skip).length)

function drawSlices(ctx: CanvasRenderingContext2D, w: number, h: number, color: (name: string) => string) {
  const regions = regionsOf(shownMarkers.value, frames.value)
  ctx.fillStyle = color('--card')
  ctx.globalAlpha = 0.65
  for (const r of regions.filter(r => r.skip))
    ctx.fillRect(x(r.start), RULER, x(r.end) - x(r.start), h - RULER)
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
    const lw = ctx.measureText(label).width + 6
    if (px - lastLabel < lw + 4)
      continue
    lastLabel = px
    ctx.fillRect(px, RULER + 1, lw, 14)
    ctx.fillStyle = color('--background')
    ctx.fillText(label, px + 3, RULER + 12)
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
watch([slicePreview, shownMarkers, () => editor.state?.markers], draw)

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
const menu = shallowRef<{ kind: 'marker', index: number } | { kind: 'selection' } | { kind: 'point', frame: number } | null>(null)
// The region starting at the right-clicked marker, for "skip / include".
const menuRegion = computed(() => {
  const m = menu.value
  const marker = m?.kind === 'marker' ? editor.state?.markers[m.index] : undefined
  return marker ? editor.regions.find(r => r.start === marker.frame) : undefined
})

function onContextMenu(e: MouseEvent) {
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
  hoverFrame.value = frameAt(e.clientX)
  hoverEdge.value = edgeAt(e)
  hoverMarker.value = markerAt(e) >= 0
}

function onMouseDown(e: MouseEvent) {
  if (e.button !== 0)
    return
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
  const up = (ev: MouseEvent) => {
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
    toZeroCrossings(ev)
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
        <Toggle v-model="editor.looping" size="sm" :class="toggleIcon" :aria-label="t('editor.toolbar.loop')">
          <Repeat />
        </Toggle>
      </Hint>
      <Hint :text="editor.selection ? t('editor.toolbar.setLoopHint') : t('editor.toolbar.clearLoopHint')" shortcut="⇧L">
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
      <Separator orientation="vertical" class="mx-1 h-5" />
      <Hint :text="t('editor.toolbar.cropHint')">
        <Button variant="ghost" size="icon-sm" :disabled="!editor.selection" :aria-label="t('editor.toolbar.crop')" @click="crop()">
          <Crop />
        </Button>
      </Hint>
      <Hint :text="t('editor.toolbar.deleteHint')" shortcut="⌫">
        <Button variant="ghost" size="icon-sm" :disabled="!editor.selection" :aria-label="t('editor.toolbar.delete')" @click="editor.remove()">
          <Eraser />
        </Button>
      </Hint>
      <Hint :text="t('editor.toolbar.cutHint')" shortcut="⌘X">
        <Button variant="ghost" size="icon-sm" :disabled="!editor.selection" :aria-label="t('editor.toolbar.cut')" @click="editor.cut()">
          <Scissors />
        </Button>
      </Hint>
      <Hint :text="t('editor.toolbar.copyHint')" shortcut="⌘C">
        <Button variant="ghost" size="icon-sm" :disabled="!editor.selection" :aria-label="t('editor.toolbar.copy')" @click="editor.copy()">
          <Copy />
        </Button>
      </Hint>
      <Hint :text="editor.selection ? t('editor.toolbar.pasteOverHint') : t('editor.toolbar.pasteHint')" shortcut="⌘V">
        <Button variant="ghost" size="icon-sm" :disabled="!editor.clipboard" :aria-label="t('editor.toolbar.paste')" @click="editor.paste()">
          <ClipboardPaste />
        </Button>
      </Hint>
      <Hint :text="t('editor.toolbar.silenceHint')">
        <Button variant="ghost" size="icon-sm" :disabled="!editor.selection" @click="editor.silence()">
          <VolumeX />
        </Button>
      </Hint>
      <Separator orientation="vertical" class="mx-1 h-5" />
      <Hint :text="t('editor.toolbar.fadeInHint')">
        <Button variant="ghost" size="icon-sm" :aria-label="t('editor.toolbar.fadeIn')" @click="editor.fade('in')">
          <FadeIcon direction="in" :curve="editor.curve" />
        </Button>
      </Hint>
      <Hint :text="t('editor.toolbar.fadeOutHint')">
        <Button variant="ghost" size="icon-sm" :aria-label="t('editor.toolbar.fadeOut')" @click="editor.fade('out')">
          <FadeIcon direction="out" :curve="editor.curve" />
        </Button>
      </Hint>
      <!-- Curve picker shown as the curve itself; the items name it too. -->
      <!-- The tooltip wraps the whole picker: inside it, it would take the popup's anchor. -->
      <Hint :text="t('editor.toolbar.fadeCurve')">
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
      <Separator orientation="vertical" class="mx-1 h-5" />
      <Hint :text="t('editor.toolbar.normalizeHint')">
        <Button variant="ghost" size="icon-sm" :aria-label="t('editor.toolbar.normalize')" @click="editor.normalize()">
          <ArrowUpToLine />
        </Button>
      </Hint>
      <Hint :text="t('editor.toolbar.gainDownHint')">
        <Button variant="ghost" size="icon-sm" :aria-label="t('editor.toolbar.gainDownHint')" @click="editor.gain(-3)">
          <Volume1 />
        </Button>
      </Hint>
      <Hint :text="t('editor.toolbar.gainUpHint')">
        <Button variant="ghost" size="icon-sm" :aria-label="t('editor.toolbar.gainUpHint')" @click="editor.gain(3)">
          <Volume2 />
        </Button>
      </Hint>
      <Hint :text="t('editor.toolbar.reverseHint')">
        <Button variant="ghost" size="icon-sm" @click="editor.reverse()">
          <ArrowLeftRight />
        </Button>
      </Hint>
      <Separator orientation="vertical" class="mx-1 h-5" />
      <EditorSlicer v-model:preview="slicePreview" :beat-frames="beatFrames" :downbeat="downbeat" :zero="zeroSnap" />
      <Hint :text="savedRegions ? t('editor.regions.saveHint', { n: formatNumber(savedRegions) }, savedRegions) : t('editor.regions.none')">
        <Button variant="ghost" size="sm" class="px-2" :disabled="!savedRegions" :aria-label="t('editor.regions.save')" @click="editor.saveRegions()">
          <FolderOutput />
          <span v-if="savedRegions" class="tabular">{{ formatNumber(savedRegions) }}</span>
        </Button>
      </Hint>
      <!-- Tempo of the bar grid (and of snapping), next to the edits it lines up. -->
      <Hint :text="t('editor.toolbar.bpmHint')">
        <Input
          :model-value="bpm ?? ''"
          type="number"
          min="20"
          max="999"
          step="0.01"
          :placeholder="t('editor.toolbar.bpm')"
          :aria-label="t('editor.toolbar.bpm')"
          class="ml-auto h-7 w-20 px-2 text-xs tabular [appearance:textfield] [&::-webkit-inner-spin-button]:appearance-none [&::-webkit-outer-spin-button]:appearance-none"
          @update:model-value="setBpm"
        />
      </Hint>
    </div>

    <EditorMinimap
      :view-start="viewStart"
      :view-end="viewEnd"
      :hover-frame="hoverFrame"
      @view="(a, b) => { viewStart = a; viewEnd = b }"
      @zoom="zoom"
      @fit="fit()"
    />
    <!-- The waveform, and the level meter at its right. -->
    <div class="flex-1 min-h-0 flex">
    <div ref="box" class="relative flex-1 min-w-0 bg-card">
      <!-- Cut, copy and paste come as DOM events: the native Edit menu takes ⌘X/⌘C/⌘V before keydown. -->
      <ContextMenu>
        <ContextMenuTrigger as-child>
          <canvas
            ref="canvas"
            tabindex="0"
            class="absolute inset-0 size-full outline-none"
            :class="hoverEdge || hoverMarker ? 'cursor-ew-resize' : 'cursor-text'"
            :aria-label="t('editor.waveform.label')"
            @mousedown="onMouseDown"
            @copy.prevent="editor.copy()"
            @cut.prevent="editor.cut()"
            @paste.prevent="editor.paste()"
            @dblclick="onDoubleClick"
            @contextmenu="onContextMenu"
            @mousemove="onHover"
            @mouseleave="hoverEdge = null; hoverMarker = false; hoverFrame = null"
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
        </ContextMenuContent>
      </ContextMenu>
      <div class="pointer-events-none absolute inset-0 overflow-hidden" aria-hidden="true">
        <div ref="cursorEl" class="absolute left-0 bottom-0 w-px bg-foreground/60" :style="{ top: `${RULER}px` }" />
        <div ref="hoverEl" class="absolute left-0 top-0 bottom-0" style="display: none">
          <div class="absolute left-0 bottom-0 w-px bg-foreground/35" :style="{ top: `${RULER}px` }" />
          <span ref="hoverLabel" class="absolute top-px px-1 rounded-sm bg-foreground text-background text-[10px] leading-4 tabular whitespace-nowrap" />
        </div>
        <div ref="playheadEl" class="absolute left-0 top-0 bottom-0 text-foreground" style="display: none">
          <div class="absolute -left-px bottom-0 w-0.5 bg-current shadow-[0_0_0_1px_var(--card)]" :style="{ top: `${RULER}px` }" />
          <svg class="absolute -left-[5px]" :style="{ top: `${RULER - 7}px` }" width="10" height="7" viewBox="0 0 10 7">
            <path d="M0 0H10L5 7Z" fill="currentColor" />
          </svg>
        </div>
      </div>
      <div v-if="editor.loading" class="absolute inset-0 grid place-items-center text-muted-foreground text-sm">
        {{ t('editor.waveform.opening') }}
      </div>
      <div v-else-if="editor.error" class="absolute inset-0 grid place-items-center text-destructive text-sm px-6 text-center">
        {{ editor.error }}
      </div>
    </div>
    <LevelMeter :active="editor.playhead !== null" />
    </div>

    <footer class="h-9 shrink-0 pl-4 pr-2 flex items-center gap-4 border-t border-border text-xs text-muted-foreground tabular">
      <span class="shrink-0">{{ t('editor.status.cursor', { time: status.cursor }) }}</span>
      <button
        v-if="vZoom > 1.01" class="shrink-0 hover:text-foreground"
        :title="t('editor.status.vZoom')"
        @click="vZoom = 1"
      >
        ↕ ×{{ formatNumber(vZoom, { fixed: 1 }) }}
      </button>
      <span v-if="status.selection" class="shrink-0">{{ t('editor.status.selection', { range: status.selection }) }}</span>
      <button v-if="status.loop" class="shrink-0 hover:text-foreground" :title="t('editor.status.selectLoop')" @click="selectLoop()">
        {{ t('editor.status.loop', { range: status.loop }) }}
      </button>
      <button
        class="group min-w-0 flex-1 flex items-center justify-end gap-1.5 hover:text-foreground"
        :title="`${editor.sample?.path}\n${t('editor.status.reveal')}`"
        @click="editor.sample && revealItemInDir(editor.sample.path)"
      >
        <FolderOpen class="size-3.5 shrink-0" />
        <span class="truncate group-hover:underline">{{ editor.sample ? shortPath(editor.sample.path) : '' }}</span>
      </button>
      <span class="shrink-0">{{ status.length }} · {{ formatRate(rate) }} · {{ editor.state?.channels === 1 ? t('common.channels.mono') : t('common.channels.stereo') }}</span>
      <Separator orientation="vertical" class="h-4" />
      <!-- View controls: beat grid, snapping, look and zoom. -->
      <div class="shrink-0 flex items-center gap-1 text-foreground">
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
              <DropdownMenuContent align="end" side="top" class="w-52">
                <template v-for="(opt, i) in viewOptions" :key="opt.key">
                  <DropdownMenuSeparator v-if="i" />
                  <DropdownMenuLabel class="text-xs text-muted-foreground font-normal">
                    {{ t(opt.label) }}
                  </DropdownMenuLabel>
                  <DropdownMenuRadioGroup :model-value="settings.settings[opt.key]" @update:model-value="setView(opt.key, $event)">
                    <DropdownMenuRadioItem v-for="c in opt.choices" :key="c.value" :value="c.value" @select.prevent>
                      {{ t(c.label) }}
                    </DropdownMenuRadioItem>
                  </DropdownMenuRadioGroup>
                </template>
                <DropdownMenuSeparator />
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
                <DropdownMenuItem :disabled="vZoom <= 1.01" @select="vZoom = 1">
                  {{ t('editor.view.resetVZoom') }}
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
          </span>
        </Hint>
        <Separator orientation="vertical" class="mx-0.5 h-4" />
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
