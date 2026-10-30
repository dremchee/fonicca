<script setup lang="ts">
import { usePixelCanvas } from '@/composables/pixelCanvas'
import { Spinner } from '@/components/ui/spinner'
import type { GridMode } from '@/composables/editor/grid'
import type { Marker } from '@/stores/editor'
import { Magnet, Maximize2, SlidersHorizontal, ZoomIn, ZoomOut } from '@lucide/vue'
import { useLocalStorage } from '@vueuse/core'
import { computed, nextTick, ref, shallowRef, watch, watchEffect } from 'vue'
import { useI18n } from 'vue-i18n'
import EditorMinimap from '@/components/EditorMinimap.vue'
import AudioExportDialog from '@/components/AudioExportDialog.vue'
import EditorToolbar from '@/components/editor/EditorToolbar.vue'
import { EDITOR_TOGGLE_CLASS, type EditorToolAction } from '@/editor-tools'
import EditorDisplayControls from '@/components/EditorDisplayControls.vue'
import { createEditorShortcuts } from '@/editor-shortcuts'
import { useEditorGestures } from '@/composables/editor/gestures'
import { useEditorWaveform } from '@/composables/editor/waveform'
import { useEditorSpectrogram } from '@/composables/editor/spectrogram'
import { drawFrequencyAxis, drawSpectrogram } from '@/spectrogram'
import LevelMeter from '@/components/LevelMeter.vue'
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
import { Separator } from '@/components/ui/separator'
import { Toggle } from '@/components/ui/toggle'
import { useEditorGrid } from '@/composables/editor/grid'
import { useEditorViewport } from '@/composables/editor/viewport'
import { drawEditorCanvas, type RegionBadge } from '@/editor-canvas'
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
const editor = useEditorStore()
const settings = useSettingsStore()

const box = ref<HTMLElement | null>(null)
const canvas = ref<HTMLCanvasElement | null>(null)
useWaveRenderer(canvas)
const { geometry: canvasGeometry, style: canvasStyle, localPixel, pixelRatio } = usePixelCanvas(box)
const width = computed(() => canvasGeometry.value.width)
const height = computed(() => canvasGeometry.value.height)
// Vertical zoom (⌥-scroll); 1 = full scale.
const vZoom = ref(1)
const RULER = 18
// Toggles look like the ghost icon buttons around them; switched on, the icon takes
// the brand colour instead of getting a background. Styled by aria-pressed: the
// tooltip trigger wrapping them overwrites their data-state.
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
watch(() => editor.windowView, view => {
  const v = view?.viewport
  if (!v) return
  viewStart.value = v.start
  viewEnd.value = v.end
  vZoom.value = v.zoom
  bpm.value = v.bpm
  downbeat.value = v.downbeat
}, { immediate: true, flush: 'post' })
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

// --- Peaks and drawing ------------------------------------------------------

const { data: wave, range: waveRange, error: waveError, refresh: readWave } = useEditorWaveform({
  enabled: computed(() => !isSpectrogram.value && !editor.loading && editor.open),
  identity: computed(() => `${editor.sample?.id}:${editor.sessionVersion}`),
  revision: computed(() => editor.revision), frames, start: viewStart, end: viewEnd, width, pixelRatio,
  split: computed(() => settings.settings.editorWaveform.waveChannels === 'split'),
  bands: computed(() => settings.settings.editorWaveform.waveColors === 'frequency'),
  read: editor.wave,
})
const openingWave = computed(() => editor.loading || (!isSpectrogram.value && !!editor.state?.frames && !wave.value && !waveError.value))

const x = (frame: number) => ((frame - viewStart.value) / (viewEnd.value - viewStart.value)) * width.value

// --- Grid -------------------------------------------------------------------

const { beatFrames, beatGrid, gridStep, gridOrigin, gridFrames, isMultiple, snapped } = useEditorGrid({
  bpm, rate, frames, width, viewStart, viewEnd, downbeat, gridMode, snap,
})

const { draggedMarkers, draggedLoop, hoverEdge, hoverMarker, hoverLoopLabel, menuOnRuler, menu, menuRegion,
  frameAt, selectLoop, playRegion, addMarkers, removeMarker, toggleSkip,
  onDoubleClick, onContextMenu, onMouseDown, updateHover } = useEditorGestures({
  editor, canvas, frames, rate, viewStart, viewEnd, zeroSnap, x, snapped, badgeAt, ruler: RULER,
  identity: computed(() => `${editor.sample?.id}:${editor.sessionVersion}`),
})
const shownLoop = computed(() => draggedLoop.value ?? editor.state?.loop ?? null)

const scheduleDraw = useFrameDraw(draw)

function draw() {
  const el = canvas.value
  if (!el || width.value === 0) return
  const ctx = canvasContext(el, width.value, height.value, canvasGeometry.value.ratio)
  const style = getComputedStyle(el)
  const spectralColors: Record<string, string> = { '--foreground': '#eee8f5', '--muted-foreground': '#c9bfd6', '--border': '#554b64', '--card': '#080511', '--background': '#080511' }
  badges = drawEditorCanvas(ctx, {
    viewport: { width: width.value, height: height.value, ratio: canvasGeometry.value.ratio,
      ruler: RULER, start: viewStart.value, end: viewEnd.value, rate: rate.value },
    grid: { mode: gridMode.value, beatGrid: beatGrid.value, beatFrames: beatFrames.value,
      downbeat: downbeat.value, gridStep, gridFrames, gridOrigin, isMultiple },
    wave: wave.value, waveRange: waveRange.value, look: waveformLook(settings.settings.editorWaveform),
    zoom: vZoom.value, spectral: isSpectrogram.value, spectrum: spectrum.value,
    selection: editor.selection, loop: shownLoop.value, loopLabel: t('editor.waveform.loopLabel'), loopPlaying: editor.playing && editor.looping, regions: shownRegions.value,
    markers: shownMarkers.value, slicePreview: slicePreview.value, hoverBadge: hoverBadge.value,
    color: name => (isSpectrogram.value && spectralColors[name]) || style.getPropertyValue(name).trim(),
  }, { drawWave, drawSpectrogram, drawFrequencyAxis })
}

watch([
  wave, spectrum, isSpectrogram, viewStart, viewEnd, width, height, pixelRatio, vZoom, gridMode, bpm, downbeat,
  () => editor.playing,
  () => editor.looping,
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
    cursorEl.value.style.transform = `translateX(${localPixel(x(editor.cursor))}px)`
})
watchEffect(() => {
  const el = playheadEl.value
  if (!el)
    return
  const p = editor.playhead
  el.style.display = p === null ? 'none' : ''
  if (p !== null)
    el.style.transform = `translateX(${localPixel(x(p))}px)`
})
watch(shownLoop, scheduleDraw)

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
  const px = localPixel(x(f))
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
const shownMarkers = computed(() => draggedMarkers.value ?? editor.state?.markers ?? [])
// Regions Save regions would write.
const playableRegions = computed(() => editor.regions.filter(r => !r.skip))
const shownRegions = computed(() => regionsOf(shownMarkers.value, frames.value))

// Region number badges as last drawn, for hovering and clicking them.
let badges: RegionBadge[] = []
const BADGE_H = 14


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

function onHover(e: MouseEvent) {
  hoverBadge.value = badgeAt(e)?.start ?? null
  hoverFrame.value = frameAt(e.clientX)
  updateHover(e)
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

const onKeyDown = createEditorShortcuts({
  editor, frames: () => frames.value, regions: () => playableRegions.value,
  playRegion, addMarkers, toggleSnap: () => { snap.value = !snap.value }, zoom, fit,
})

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
  const lp = shownLoop.value
  const range = (r: { start: number, end: number }) => `${formatTime(r.start / sr)} – ${formatTime(r.end / sr)} (${formatTime((r.end - r.start) / sr)})`
  return {
    cursor: formatTime(editor.cursor / sr),
    selection: sel ? range(sel) : '',
    loop: lp ? range(lp) : '',
    length: formatTime(frames.value / sr),
  }
})

const toolbarActions: Record<EditorToolAction, () => unknown> = {
  undo: editor.undo, redo: editor.redo, play: editor.togglePlay, loopPoints: editor.toggleLoopPoints,
  crop, delete: editor.remove, cut: editor.cut, copy: editor.copy, paste: editor.paste,
  silence: editor.silence, fadeIn: () => editor.fade('in'), fadeOut: () => editor.fade('out'),
  normalize: editor.normalize, gainDown: () => editor.gain(-3), gainUp: () => editor.gain(3),
  reverse: editor.reverse, midi: exportMidi,
}
</script>

<template>
  <div class="h-full flex flex-col overflow-hidden bg-background text-foreground" :inert="editor.loading || editor.saving" :aria-busy="openingWave" data-editor-panel>
    <!-- Keep a drag region for the legacy standalone editor window. -->
    <header v-if="!props.embedded" class="h-11 shrink-0 pl-[84px] pr-3 flex items-center border-b border-border" data-tauri-drag-region>
      <span class="min-w-0 truncate text-sm font-medium" data-tauri-drag-region>{{ editor.sample?.name }}<span class="text-muted-foreground">.{{ editor.sample?.ext }}</span><span v-if="editor.state?.dirty" class="text-primary"> •</span></span>
    </header>

    <EditorToolbar
      :embedded="props.embedded" :expanded="props.expanded"
      :can-detach="!!editor.state && !editor.loading && !editor.processing && !editor.saving"
      :can-undo="!!editor.state?.canUndo" :can-redo="!!editor.state?.canRedo"
      :has-selection="!!editor.selection" :has-loop="!!editor.state?.loop" :has-clipboard="!!editor.clipboard"
      :playing="editor.playing" :can-export-midi="!!editor.sample && !!frames && !editor.processing"
      :beat-frames="beatFrames" :downbeat="downbeat" :zero-snap="zeroSnap"
      v-model:curve="editor.curve" v-model:looping="editor.looping" v-model:preview="slicePreview"
      @action="action => toolbarActions[action]()" @close="emit('close')" @toggle-expand="emit('toggleExpand')"
      @detach="editor.detachWindow({ start: viewStart, end: viewEnd, zoom: vZoom, bpm, downbeat })"
    />

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
            class="absolute outline-none"
            :style="canvasStyle"
            :class="hoverBadge !== null || hoverLoopLabel ? 'cursor-pointer' : hoverEdge || hoverMarker ? 'cursor-ew-resize' : 'cursor-text'"
            :aria-label="t(isSpectrogram ? 'editor.spectrogram.label' : 'editor.waveform.label')"
            @mousedown="onMouseDown"
            @copy.prevent="editor.copy()"
            @cut.prevent="editor.cut()"
            @paste.prevent="editor.paste()"
            @dblclick="onDoubleClick"
            @contextmenu="onContextMenu"
            @mousemove="onHover"
            @mouseleave="hoverEdge = null; hoverMarker = false; hoverLoopLabel = false; hoverFrame = null; hoverBadge = null"
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
      <div class="pointer-events-none absolute overflow-hidden" :style="canvasStyle" aria-hidden="true">
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
      <div v-if="openingWave" class="absolute inset-0 flex flex-col items-center justify-center gap-3 bg-background/95 text-muted-foreground text-sm" role="status" aria-live="polite">
        <Spinner class="size-6 text-primary" aria-hidden="true" />
        {{ t('editor.waveform.opening') }}
      </div>
      <div v-else-if="waveError" class="absolute inset-0 flex flex-col items-center justify-center gap-3 bg-background/95 text-destructive text-sm px-6 text-center" role="alert">
        {{ waveError }}
        <Button variant="outline" size="sm" @click="readWave">{{ t('editor.spectrogram.retry') }}</Button>
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
          <Toggle v-model="snap" size="sm" :class="EDITOR_TOGGLE_CLASS" :disabled="gridMode === 'off'" :aria-label="t('editor.toolbar.snap')">
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
    <AudioExportDialog />
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
