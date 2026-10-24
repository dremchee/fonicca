<script setup lang="ts">
import type { Sample, TrackTags } from '@/api'
import type { Wave } from '@/stores/editor'
import { FolderOpen, Info, Pencil, Play, Square, Star, X } from '@lucide/vue'
import { revealItemInDir } from '@tauri-apps/plugin-opener'
import { useResizeObserver } from '@vueuse/core'
import { computed, onBeforeUnmount, onMounted, ref, shallowRef, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { api } from '@/api'
import Hint from '@/components/Hint.vue'
import ProjectSync from '@/components/ProjectSync.vue'
import TagPicker from '@/components/TagPicker.vue'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import { Label } from '@/components/ui/label'
import { Separator } from '@/components/ui/separator'
import { openTrackInfo, trackInfoRevision } from '@/composables/trackInfo'
import { categoryHint, categoryLabel, formatBpm, formatChannels, formatDb, formatDuration, formatRate, formatSize, kindLabel, shortPath, sourceHint } from '@/format'
import { openEditor } from '@/stores/editor'
import { useLibraryStore } from '@/stores/library'
import { usePlayerStore } from '@/stores/player'
import { useSettingsStore } from '@/stores/settings'
import { drawWave, parseWave } from '@/waveform'

const props = defineProps<{ sample: Sample | undefined }>()

const { t } = useI18n()
const library = useLibraryStore()
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
const box = ref<HTMLElement | null>(null)
const width = ref(0)
const wave = shallowRef<Wave | null>(null)
const loading = ref(false)
const error = ref<string | null>(null)

const progress = computed(() => {
  const s = props.sample
  if (!s?.duration || player.playingId !== s.id)
    return 0
  return Math.min(1, player.position / s.duration)
})

useResizeObserver(box, (entries) => {
  width.value = Math.floor(entries[0].contentRect.width)
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
    if (req === requestId)
      wave.value = data
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

function draw() {
  const el = canvas.value
  if (!el)
    return
  const dpr = window.devicePixelRatio || 1
  const w = el.clientWidth
  const h = el.clientHeight
  el.width = Math.round(w * dpr)
  el.height = Math.round(h * dpr)
  const ctx = el.getContext('2d')!
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
  ctx.clearRect(0, 0, w, h)
  const data = wave.value
  if (!data)
    return

  const style = getComputedStyle(el)
  const color = (name: string) => style.getPropertyValue(name).trim()
  const { waveStyle, waveScale, waveColors } = settings.settings
  const look = { style: waveStyle, scale: waveScale, colors: waveColors, zoom: 1, color: color('--wave'), lineColor: color('--border') }
  const area = { top: 0, width: w, height: h, dpr }
  drawWave(ctx, data, area, look)

  // The part already played: in the accent colour, or (frequency colours) the
  // rest dimmed so the colours stay readable.
  const playedX = progress.value * w
  if (playedX <= 0)
    return
  ctx.save()
  ctx.beginPath()
  if (waveColors === 'frequency') {
    ctx.rect(playedX, 0, w - playedX, h)
    ctx.clip()
    ctx.globalAlpha = 0.5
    ctx.fillStyle = color('--background')
    ctx.fillRect(playedX, 0, w - playedX, h)
  }
  else {
    ctx.rect(0, 0, playedX, h)
    ctx.clip()
    ctx.clearRect(0, 0, playedX, h)
    drawWave(ctx, data, area, { ...look, color: color('--wave-played') })
  }
  ctx.restore()
}

watch([wave, progress, () => settings.settings, () => settings.themeRevision, width], draw, { deep: true })
onMounted(draw)

function onSeek(e: MouseEvent) {
  const s = props.sample
  if (!s?.duration || !canvas.value)
    return
  const rect = canvas.value.getBoundingClientRect()
  const ratio = Math.min(1, Math.max(0, (e.clientX - rect.left) / rect.width))
  player.seek(s, ratio * s.duration)
}

function onDragStart(e: DragEvent) {
  e.preventDefault()
  if (props.sample)
    library.dragSamples(props.sample).catch(err => console.error('drag failed', err))
}
</script>

<template>
  <!-- Hidden until a sample is selected, so the list gets the whole height. -->
  <section v-if="sample" class="h-full bg-card flex flex-col">
    <div class="h-12 px-3 flex items-center gap-3">
      <Hint :text="player.playingId === sample.id ? t('common.actions.stop') : t('common.actions.play')" :shortcut="t('player.space')">
        <Button size="icon-sm" class="rounded-full shrink-0" @click="player.toggle(sample)">
          <component :is="player.playingId === sample.id ? Square : Play" class="fill-current" />
        </Button>
      </Hint>
      <div
        class="min-w-0 flex-1 flex items-center gap-2.5 cursor-grab"
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
      <!-- Actions on the sample, then playback options. -->
      <div class="shrink-0 flex items-center gap-0.5">
        <Hint :text="sample.favorite ? t('list.favorite.remove') : t('list.favorite.add')" shortcut="F">
          <Button variant="ghost" size="icon-sm" @click="library.toggleFavorite()">
            <Star :class="sample.favorite ? 'fill-amber-400 text-amber-400' : 'text-muted-foreground'" />
          </Button>
        </Hint>
        <Hint :text="t('trackInfo.edit')" shortcut="I">
          <Button variant="ghost" size="icon-sm" class="text-muted-foreground" :aria-label="t('trackInfo.edit')" @click="openTrackInfo(sample)">
            <Info />
          </Button>
        </Hint>
        <Hint :text="t('player.edit')" shortcut="E">
          <Button variant="ghost" size="icon-sm" class="text-muted-foreground" :aria-label="t('player.edit')" @click="openEditor(sample)">
            <Pencil />
          </Button>
        </Hint>
      </div>
      <Separator orientation="vertical" class="h-5" />
      <div class="shrink-0 flex items-center gap-2">
        <ProjectSync />
        <Checkbox
          id="autoplay"
          :model-value="player.autoplay"
          @update:model-value="player.autoplay = $event === true"
        />
        <Label for="autoplay" class="text-muted-foreground font-normal">
          {{ t('player.autoplay') }}
        </Label>
      </div>
    </div>
    <div ref="box" class="relative flex-1 mx-3 rounded-md bg-background overflow-hidden">
      <canvas ref="canvas" class="absolute inset-0 size-full cursor-pointer" @mousedown="onSeek" />
      <div v-if="loading" class="absolute inset-0 grid place-items-center text-muted-foreground text-xs">
        {{ t('player.readingWaveform') }}
      </div>
      <div v-else-if="error" class="absolute inset-0 grid place-items-center text-destructive text-xs px-4 text-center">
        {{ error }}
      </div>
    </div>
    <!-- Status bar. One line tall: fields that do not fit wrap onto the hidden second line
         and drop out whole, least important (the last ones) first. -->
    <div class="h-8 shrink-0 px-3 flex items-center gap-3 text-[11px] text-muted-foreground tabular">
      <!-- The sample's own tags and the picker to add more; badges that do not fit drop out. -->
      <div class="shrink-0 max-w-[40%] min-w-0 flex items-center gap-1">
        <div v-if="sample.tags.length" class="min-w-0 h-5 flex flex-wrap items-center gap-1 overflow-hidden">
          <Badge v-for="tag in sample.tags" :key="tag.id" variant="secondary" class="h-5 pr-1 text-[11px]">
            {{ tag.name }}
            <button
              class="rounded-full opacity-60 hover:opacity-100 focus-visible:opacity-100 outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
              :aria-label="t('player.removeTag', { name: tag.name })"
              @click="library.removeTag(tag.id)"
            >
              <X class="size-3" />
            </button>
          </Badge>
        </div>
        <TagPicker :assigned="sample.tags" :count="selectionSize" />
      </div>
      <div class="min-w-0 flex-1 h-8 flex flex-wrap items-center gap-x-3 leading-8 overflow-hidden">
        <span v-if="sample.category" :title="categoryHint(sample.categorySource, sample.categoryConfidence)">{{ categoryLabel(sample.category) }}</span>
        <span v-if="sample.kind">{{ kindLabel(sample.kind) }}</span>
        <span v-if="sample.bpm != null" :title="sourceHint(sample.bpmSource)">{{ t('common.units.bpm', { n: formatBpm(sample.bpm) }) }}</span>
        <span v-if="sample.key" :title="sourceHint(sample.keySource)">{{ sample.key }}</span>
        <span v-if="sample.peakDb != null" :title="t('player.samplePeak')">{{ formatDb(sample.peakDb) }}</span>
        <span v-if="sample.lufs != null" :title="t('player.integratedLoudness')">{{ formatDb(sample.lufs, 'LUFS') }}</span>
        <span>{{ formatDuration(sample.duration) }}</span>
        <span>{{ formatRate(sample.sampleRate) }}</span>
        <span v-if="sample.bitDepth">{{ t('common.units.bit', { n: sample.bitDepth }) }}</span>
        <span>{{ formatChannels(sample.channels) }}</span>
        <span>{{ formatSize(sample.size) }}</span>
      </div>
      <button
        class="group min-w-0 max-w-[50%] flex items-center gap-1.5 hover:text-foreground"
        :title="`${sample.path}\n${t('player.clickToReveal')}`"
        @click="revealItemInDir(sample.path)"
      >
        <FolderOpen class="size-3.5 shrink-0" />
        <span class="truncate group-hover:underline">{{ shortPath(sample.path) }}</span>
      </button>
    </div>
    <div v-if="player.error" class="px-3 pb-2 text-destructive text-xs truncate">
      {{ player.error }}
    </div>
  </section>
</template>
