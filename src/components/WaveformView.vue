<script setup lang="ts">
import type { Sample, TrackTags } from '@/api'
import type { Wave } from '@/stores/editor'
import { FolderOpen, Pause, Play, Star, Volume2, VolumeX } from '@lucide/vue'
import { revealItemInDir } from '@tauri-apps/plugin-opener'
import { useResizeObserver } from '@vueuse/core'
import { computed, onBeforeUnmount, onMounted, ref, shallowRef, useId, watch, watchEffect } from 'vue'
import { useI18n } from 'vue-i18n'
import { api } from '@/api'
import Hint from '@/components/Hint.vue'
import LevelMeter from '@/components/LevelMeter.vue'
import ProjectSync from '@/components/ProjectSync.vue'
import SampleInfoHover from '@/components/SampleInfoHover.vue'
import PreviewTags from '@/components/PreviewTags.vue'
import WaveformPreview from '@/components/WaveformPreview.vue'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import { Label } from '@/components/ui/label'
import { Slider } from '@/components/ui/slider'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { useWaveformBackgrounds } from '@/composables/waveformBackgrounds'
import { trackInfoRevision } from '@/composables/trackInfo'
import { formatDuration, shortPath } from '@/format'
import { playbackSeekTarget } from '@/playback-navigation'
import { useLibraryStore } from '@/stores/library'
import { usePlayerStore } from '@/stores/player'
import { useSettingsStore } from '@/stores/settings'
import { canvasContext, drawWave, parseWave } from '@/waveform-renderer'
import { useWaveRenderer } from '@/waveform-gl'
import { waveformLook } from '@/waveform-settings'

const props = defineProps<{ sample: Sample | undefined }>()

const { t } = useI18n()
const library = useLibraryStore()
const backgrounds = useWaveformBackgrounds()
const player = usePlayerStore()
const settings = useSettingsStore()
const selectionSize = computed(() => {
  const n = library.selectedIds.size
  return props.sample && (n > 1 || (n === 1 && !library.selectedIds.has(props.sample.id))) ? n : 1
})

// Tags and cover art from the file itself, for songs and tagged packs.
const tags = shallowRef<TrackTags | null>(null)
const coverUrl = ref<string | null>(null)
let tagsRequest = 0

function setCover(url: string | null) {
  if (coverUrl.value)
    URL.revokeObjectURL(coverUrl.value)
  coverUrl.value = url
}

watch([() => props.sample?.id, trackInfoRevision], async ([id]) => {
  const req = ++tagsRequest
  tags.value = null
  setCover(null)
  if (id == null)
    return
  try {
    const found = await api.sampleTags(id)
    if (req !== tagsRequest)
      return
    tags.value = found
    if (found?.hasCover) {
      const bytes = await api.sampleCover(id)
      if (req === tagsRequest && bytes.byteLength)
        setCover(URL.createObjectURL(new Blob([bytes])))
    }
  }
  catch {
    // A file whose tags cannot be read just shows without them.
  }
}, { immediate: true })

onBeforeUnmount(() => setCover(null))

const title = computed(() => tags.value?.title ?? props.sample?.name ?? '')
const byline = computed(() => {
  const g = tags.value
  return g ? [g.artist, g.album, g.year, g.genre].filter(Boolean).join(' · ') : ''
})

const canvas = ref<HTMLCanvasElement | null>(null)
useWaveRenderer(canvas)
const playedCanvas = ref<HTMLCanvasElement | null>(null)
useWaveRenderer(playedCanvas)
const dimEl = ref<HTMLElement | null>(null)
const box = ref<HTMLElement | null>(null)
const width = ref(0)
const height = ref(0)
const wave = shallowRef<Wave | null>(null)
const loading = ref(false)
const error = ref<string | null>(null)

const seekHintId = useId()
const seekPreview = ref<number | null>(null)
const elapsed = computed(() => seekPreview.value ?? (player.playingId === props.sample?.id ? player.position : 0))
watch(() => props.sample?.id, () => seekPreview.value = null)
const elapsedLabel = computed(() => {
  if ((props.sample?.duration ?? 0) < 10)
    return formatDuration(elapsed.value)
  const seconds = Math.max(0, Math.floor(elapsed.value))
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, '0')}`
})
const volumeDrag = ref<number | null>(null)
const volumePercent = computed(() => Math.round((volumeDrag.value ?? settings.settings.volume) * 100))

const progress = computed(() => {
  const s = props.sample
  if (!s?.duration)
    return 0
  return Math.min(1, Math.max(0, elapsed.value / s.duration))
})

useResizeObserver(box, (entries) => {
  width.value = Math.floor(entries[0].contentRect.width)
  height.value = Math.floor(entries[0].contentRect.height)
})

// Re-read when the sample, the width or the channel layout changes; the backend
// keeps an overview of recent samples, so only the first read decodes the file.
let requestId = 0
watch([() => props.sample?.id, width, () => settings.settings.waveChannels], async ([id], [prevId]) => {
  const req = ++requestId
  if (id !== prevId) {
    wave.value = null
    error.value = null
  }
  if (id == null || width.value === 0)
    return
  loading.value = wave.value === null
  try {
    const columns = Math.min(8192, Math.round(width.value * (window.devicePixelRatio || 1)))
    const data = parseWave(await api.sampleWave(id, columns, settings.settings.waveChannels === 'split'))
    if (req === requestId) {
      wave.value = data
      if (props.sample) backgrounds.remember(props.sample, data)
    }
  }
  catch (e) {
    if (req === requestId)
      error.value = String(e)
  }
  finally {
    if (req === requestId)
      loading.value = false
  }
}, { immediate: true })

// The wave is drawn twice, once in each colour, on stacked canvases; playback only
// moves the clip of the played one (see below), so the wave is not redrawn per frame.
function draw() {
  const el = canvas.value
  const played = playedCanvas.value
  if (!el || !played)
    return
  const dpr = window.devicePixelRatio || 1
  const w = el.clientWidth
  const h = el.clientHeight
  const ctx = canvasContext(el, w, h, dpr)
  const playedCtx = canvasContext(played, w, h, dpr)
  const data = wave.value
  if (!data)
    return

  const style = getComputedStyle(el)
  const color = (name: string) => style.getPropertyValue(name).trim()
  const { waveColors } = settings.settings
  const look = { ...waveformLook(settings.settings), zoom: 1, color: color('--wave'), lineColor: color('--border') }
  const area = { top: 0, width: w, height: h, dpr }
  drawWave(ctx, data, area, look)
  if (waveColors !== 'frequency')
    drawWave(playedCtx, data, area, { ...look, color: color('--wave-played'), channelColors: [null, null] })
}

watch([wave, () => settings.settings, () => settings.themeRevision, width, height], draw, { deep: true })
onMounted(draw)

// The part already played: in the accent colour, or (frequency colours) the rest
// dimmed so the colours stay readable. Both only move a clip edge.
watchEffect(() => {
  const base = canvas.value
  const played = playedCanvas.value
  const dim = dimEl.value
  if (!base || !played || !dim)
    return
  const frequency = settings.settings.waveColors === 'frequency'
  const p = progress.value
  // The base is cut away under the played part: its dimmed peaks would show through.
  base.style.clipPath = frequency ? '' : `inset(0 0 0 ${p * 100}%)`
  played.style.clipPath = `inset(0 ${(1 - p) * 100}% 0 0)`
  played.style.display = frequency ? 'none' : ''
  dim.style.left = `${p * 100}%`
  dim.style.display = frequency && p > 0 ? '' : 'none'
})

async function commitSeek(position: number) {
  const s = props.sample
  if (!s?.duration)
    return
  seekPreview.value = Math.min(s.duration, Math.max(0, position))
  const preview = seekPreview.value
  try {
    await player.seek(s, preview)
  }
  catch (error) {
    player.error = String(error)
  }
  finally {
    // A newer drag or another sample must keep its own preview.
    if (props.sample?.id === s.id && seekPreview.value === preview)
      seekPreview.value = null
  }
}

function onSeekKeydown(e: KeyboardEvent) {
  if (e.key === ' ') {
    e.preventDefault()
    if (props.sample)
      player.toggle(props.sample)
    return
  }
  if (e.metaKey || e.ctrlKey || e.altKey)
    return
  const target = playbackSeekTarget(e.key, elapsed.value, props.sample?.duration ?? 0, e.shiftKey)
  if (target == null)
    return
  e.preventDefault()
  commitSeek(target)
}

function onDragStart(e: DragEvent) {
  e.preventDefault()
  if (props.sample)
    library.dragSamples(props.sample).catch(err => console.error('drag failed', err))
}
</script>

<template>
  <!-- Hidden until a sample is selected, so the list gets the whole height. -->
  <section v-if="sample" class="player-panel h-full bg-card flex flex-col">
    <div class="player-toolbar shrink-0 px-3">
      <Hint :text="player.isPlaying(sample.id) ? t('common.actions.pause') : t('common.actions.play')" :shortcut="t('player.space')">
        <Button size="icon-sm" class="rounded-full shrink-0" :aria-label="player.isPlaying(sample.id) ? t('common.actions.pause') : t('common.actions.play')" @click="player.toggle(sample)">
          <component :is="player.isPlaying(sample.id) ? Pause : Play" class="fill-current" />
        </Button>
      </Hint>
      <div
        class="preview-track min-w-0 flex items-center gap-2.5 cursor-grab"
        draggable="true"
        :title="t('player.dragToDaw')"
        @dragstart="onDragStart"
      >
        <img
          v-if="coverUrl" :src="coverUrl" :alt="t('player.cover')" draggable="false"
          class="size-8 shrink-0 rounded-sm object-cover ring-1 ring-border"
        >
        <div class="min-w-0">
          <div class="truncate font-medium">
            {{ title }}<span v-if="!tags?.title" class="text-muted-foreground">.{{ sample.ext }}</span>
          </div>
          <div v-if="byline" class="truncate text-[11px] text-muted-foreground">
            {{ byline }}
          </div>
        </div>
      </div>
      <div class="preview-audio flex items-center gap-2">
        <LevelMeter horizontal :variant="settings.settings.meterStyle" :active="player.playingId !== null && !player.paused" />
        <Popover>
          <PopoverTrigger as-child>
            <Button
              variant="ghost" size="icon-sm" class="text-muted-foreground"
              :aria-label="`${t('player.volume')}: ${volumePercent}%`"
              :title="`${t('player.volume')}: ${volumePercent}%`"
            >
              <component :is="volumePercent === 0 ? VolumeX : Volume2" />
            </Button>
          </PopoverTrigger>
          <PopoverContent side="top" align="center" class="w-56 p-3">
            <div class="mb-3 flex items-center justify-between gap-3 text-xs">
              <span>{{ t('player.volume') }}</span>
              <span class="tabular text-muted-foreground">{{ volumePercent }}%</span>
            </div>
            <Slider
              :model-value="[volumePercent]" :min="0" :max="100" :step="1"
              :aria-label="t('player.volume')"
              @update:model-value="volumeDrag = ($event?.[0] ?? 0) / 100"
              @value-commit="settings.update({ volume: ($event[0] ?? 0) / 100 }).then(() => volumeDrag = null)"
            />
          </PopoverContent>
        </Popover>
      </div>
      <div class="preview-time shrink-0 tabular text-xs text-muted-foreground whitespace-nowrap">
        <span class="text-foreground">{{ elapsedLabel }}</span>
        <span class="mx-1">/</span>{{ formatDuration(sample.duration) }}
      </div>
      <!-- Actions on the sample. -->
      <div class="preview-actions shrink-0 flex items-center gap-0.5">
        <Hint :text="sample.favorite ? t('list.favorite.remove') : t('list.favorite.add')" shortcut="F">
          <Button
            variant="ghost" size="icon-sm"
            :aria-label="sample.favorite ? t('list.favorite.remove') : t('list.favorite.add')"
            :aria-pressed="sample.favorite" @click="library.toggleFavorite(sample)"
          >
            <Star :class="sample.favorite ? 'fill-amber-400 text-amber-400' : 'text-muted-foreground'" />
          </Button>
        </Hint>
        <SampleInfoHover :key="sample.id" :sample="sample" :tags="tags" :cover-url="coverUrl" />
      </div>

    </div>
    <p :id="seekHintId" class="sr-only">{{ t('player.seekHint') }}</p>
    <div class="preview-signal flex flex-1 min-h-0 gap-2 mx-3">
      <div :aria-busy="loading" ref="box" class="seek-surface relative flex-1 min-w-0 rounded-md bg-background overflow-hidden cursor-pointer">
        <canvas ref="canvas" class="wave-canvas absolute inset-0 size-full" :class="{ ready: wave && !loading }" />
        <canvas ref="playedCanvas" class="wave-canvas absolute inset-0 size-full" :class="{ ready: wave && !loading }" aria-hidden="true" />
        <div ref="dimEl" class="pointer-events-none absolute inset-y-0 right-0 bg-background/50" style="display: none" />
        <div
          v-if="player.playingId === sample.id || seekPreview !== null"
          class="pointer-events-none absolute inset-y-0 w-px bg-primary"
          :style="{ left: `clamp(0px, ${progress * 100}%, calc(100% - 1px))` }"
        />
        <input
          :key="sample.id" type="range" class="seek-input absolute inset-0 size-full opacity-0 cursor-pointer"
          min="0" :max="sample.duration || 0" step="any" :value="elapsed" :disabled="!sample.duration"
          :aria-label="t('player.seek')" :aria-describedby="seekHintId" :title="t('player.seekHint')"
          :aria-valuetext="t('player.position', { elapsed: elapsedLabel, duration: formatDuration(sample.duration) })"
          @input="seekPreview = Number(($event.target as HTMLInputElement).value)"
          @change="commitSeek(Number(($event.target as HTMLInputElement).value))"
          @keydown="onSeekKeydown" @pointercancel="seekPreview = null"
        >
        <div v-if="loading" class="wave-placeholder pointer-events-none absolute inset-0" role="status" :aria-label="t('player.readingWaveform')">
          <WaveformPreview pattern="rising" :options="settings.settings" class="placeholder-preview absolute inset-0" aria-hidden="true" />
          <span class="sr-only">{{ t('player.readingWaveform') }}</span>
        </div>
        <div v-else-if="error" class="pointer-events-none absolute inset-0 grid place-items-center text-destructive text-xs px-4 text-center">
          {{ error }}
        </div>
      </div>
    </div>
    <!-- Tags and preview actions stay visible; technical details live in the info card. -->
    <div class="h-8 shrink-0 px-4 flex items-center gap-3 text-xs text-muted-foreground tabular">
      <PreviewTags :tags="sample.tags" :count="selectionSize" />
      <span class="min-w-0 flex-1" aria-hidden="true" />
      <button
        class="group min-w-0 max-w-[25%] flex items-center gap-1.5 hover:text-foreground"
        :title="`${sample.path}\n${t('player.clickToReveal')}`"
        @click="revealItemInDir(sample.path)"
      >
        <FolderOpen class="size-3.5 shrink-0" />
        <span class="truncate group-hover:underline">{{ shortPath(sample.path) }}</span>
      </button>
      <div class="preview-options shrink-0 flex items-center gap-3 border-l border-border pl-3">
        <ProjectSync />
        <Checkbox
          id="autoplay"
          :aria-label="t('player.autoplay')"
          :model-value="player.autoplay"
          @update:model-value="player.autoplay = $event === true"
        />
        <Label for="autoplay" class="text-muted-foreground font-normal" :title="t('player.autoplay')">
          <span class="autoplay-long">{{ t('player.autoplay') }}</span>
          <span class="autoplay-short">{{ t('player.autoplayShort') }}</span>
        </Label>
      </div>
    </div>
    <div v-if="player.error" class="px-3 pb-2 text-destructive text-xs truncate">
      {{ player.error }}
    </div>
  </section>
</template>

<style scoped>
.wave-canvas {
  opacity: 0;
}
.wave-canvas.ready {
  opacity: 1;
  animation: wave-reveal 180ms ease-out;
}
.placeholder-preview {
  height: 100%;
  opacity: 0.24;
  transform-origin: center;
  animation: placeholder-pulse 1100ms ease-in-out infinite alternate;
}
@keyframes placeholder-pulse {
  from { transform: scaleY(0.55); opacity: 0.16; }
  to { transform: scaleY(1); opacity: 0.34; }
}
@keyframes wave-reveal {
  from { opacity: 0; transform: scaleY(0.96); }
  to { opacity: 1; transform: scaleY(1); }
}
@media (prefers-reduced-motion: reduce) {
  .placeholder-preview,
  .wave-canvas.ready {
    animation: none;
  }
}

.player-panel {
  container-type: inline-size;
}
.preview-options :deep([data-slot="button"]) {
  height: 22px;
  padding-inline: 8px;
  gap: 6px;
  font-size: 12px;
}
.preview-options :deep([data-slot="button"] svg) {
  width: 12px;
  height: 12px;
}
.preview-options :deep([data-slot="checkbox"]) {
  width: 12px;
  height: 12px;
}
.preview-options :deep([data-slot="checkbox"] svg) {
  width: 10px;
  height: 10px;
}
.player-toolbar {
  display: grid;
  grid-template-columns: auto minmax(120px, 1fr) auto auto auto;
  align-items: center;
  gap: 12px;
  min-height: 42px;
  padding-block: 6px;
}
@container (max-width: 650px) {
  .player-toolbar {
    grid-template-columns: auto minmax(0, 1fr) auto auto;
    gap: 6px 10px;
  }
  .preview-track { grid-column: 2; grid-row: 1; }
  .preview-time { grid-column: 3; grid-row: 1; }
  .preview-actions { grid-column: 4; grid-row: 1; }
  .preview-audio { grid-column: 1 / 5; grid-row: 2; justify-self: end; }
}
.seek-input {
  margin: 0;
  touch-action: none;
}
.seek-surface:has(.seek-input:focus-visible) {
  outline: 2px solid var(--ring);
  outline-offset: 2px;
}
.autoplay-short {
  display: none;
}
@container (max-width: 680px) {
  .autoplay-long {
    display: none;
  }
  .autoplay-short {
    display: inline;
  }
}
</style>
