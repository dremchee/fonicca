<script setup lang="ts">
import type { ComponentPublicInstance } from 'vue'
import type { Sample } from '@/api'
import { Play, Square, Star, X } from '@lucide/vue'
import { useResizeObserver } from '@vueuse/core'
import { computed, onMounted, ref, shallowRef, useTemplateRef, watch } from 'vue'
import { api } from '@/api'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { CATEGORY_LABELS, categoryHint, formatBpm, formatChannels, formatDb, formatDuration, formatRate, formatSize, KIND_LABELS, sourceHint } from '@/format'
import { useLibraryStore } from '@/stores/library'
import { usePlayerStore } from '@/stores/player'

const props = defineProps<{ sample: Sample | undefined }>()

const library = useLibraryStore()
const player = usePlayerStore()
const tagInputRef = useTemplateRef<ComponentPublicInstance>('tagField')
// Input renders a bare <input> as its root.
const tagInput = computed(() => tagInputRef.value?.$el as HTMLInputElement | undefined)
const newTag = ref('')

const selectionSize = computed(() => {
  const n = library.selectedIds.size
  return props.sample && (n > 1 || (n === 1 && !library.selectedIds.has(props.sample.id))) ? n : 1
})

watch(() => library.tagInputRequest, () => {
  tagInput.value?.focus()
})

async function submitTag() {
  const name = newTag.value.trim()
  if (!name)
    return
  newTag.value = ''
  await library.addTag(name)
}

function onTagKeydown(e: KeyboardEvent) {
  if (e.key === 'Enter') {
    e.preventDefault()
    submitTag()
  }
  else if (e.key === 'Escape') {
    e.stopPropagation()
    newTag.value = ''
    tagInput.value?.blur()
  }
}
const canvas = ref<HTMLCanvasElement | null>(null)
const box = ref<HTMLElement | null>(null)
const peaks = shallowRef<Int8Array | null>(null)
const loading = ref(false)
const error = ref<string | null>(null)

const progress = computed(() => {
  const s = props.sample
  if (!s?.duration || player.playingId !== s.id)
    return 0
  return Math.min(1, player.position / s.duration)
})

let requestId = 0
watch(() => props.sample?.id, async (id) => {
  const req = ++requestId
  peaks.value = null
  error.value = null
  if (id == null)
    return
  loading.value = true
  try {
    const data = await api.getPeaks(id)
    if (req === requestId)
      peaks.value = Int8Array.from(data)
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

  const data = peaks.value
  if (!data)
    return
  const style = getComputedStyle(el)
  const base = style.getPropertyValue('--wave').trim()
  const played = style.getPropertyValue('--wave-played').trim()
  const buckets = data.length / 2
  const mid = h / 2
  const playedX = progress.value * w

  // One vertical bar per device pixel column, aggregating the buckets under it.
  const columns = Math.floor(w)
  for (let x = 0; x < columns; x++) {
    const from = Math.floor((x / columns) * buckets)
    const to = Math.max(from + 1, Math.floor(((x + 1) / columns) * buckets))
    let lo = 0
    let hi = 0
    for (let b = from; b < to && b < buckets; b++) {
      lo = Math.min(lo, data[b * 2])
      hi = Math.max(hi, data[b * 2 + 1])
    }
    const top = mid - (hi / 127) * mid * 0.95
    const bottom = mid - (lo / 127) * mid * 0.95
    ctx.fillStyle = x < playedX ? played : base
    ctx.fillRect(x, top, 1, Math.max(1, bottom - top))
  }
}

watch([peaks, progress], draw)
onMounted(draw)
useResizeObserver(box, draw)

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
  <section v-if="sample" class="h-60 shrink-0 border-t border-border bg-card flex flex-col">
    <div class="h-10 px-3 flex items-center gap-3">
      <Button
        size="icon-sm" class="rounded-full"
        :title="player.playingId === sample.id ? 'Stop (Space)' : 'Play (Space)'"
        @click="player.toggle(sample)"
      >
        <component :is="player.playingId === sample.id ? Square : Play" class="fill-current" />
      </Button>
      <div
        class="min-w-0 flex-1 cursor-grab"
        draggable="true"
        title="Drag into your DAW"
        @dragstart="onDragStart"
      >
        <div class="truncate font-medium">
          {{ sample.name }}<span class="text-muted-foreground">.{{ sample.ext }}</span>
        </div>
        <div class="truncate text-[11px] text-muted-foreground">
          {{ sample.path }}
        </div>
      </div>
      <div class="flex items-center gap-2 shrink-0">
        <Checkbox
          id="autoplay"
          :model-value="player.autoplay"
          @update:model-value="player.autoplay = $event === true"
        />
        <Label for="autoplay" class="text-muted-foreground font-normal">
          Autoplay
        </Label>
      </div>
    </div>
    <div class="h-10 px-3 flex items-center gap-2 min-w-0">
      <Button
        variant="ghost" size="icon-sm"
        :title="sample.favorite ? 'Remove from favorites (F)' : 'Add to favorites (F)'"
        @click="library.toggleFavorite()"
      >
        <Star :class="sample.favorite ? 'fill-amber-400 text-amber-400' : 'text-muted-foreground'" />
      </Button>
      <div class="flex gap-1 overflow-x-auto min-w-0">
        <Badge v-for="t in sample.tags" :key="t.id" variant="secondary" class="pr-1">
          {{ t.name }}
          <button
            class="rounded-full opacity-60 hover:opacity-100 focus-visible:opacity-100 outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
            :aria-label="`Remove tag ${t.name}`"
            @click="library.removeTag(t.id)"
          >
            <X class="size-3" />
          </button>
        </Badge>
      </div>
      <Input
        ref="tagField"
        v-model="newTag"
        list="known-tags"
        :placeholder="selectionSize > 1 ? `Tag ${selectionSize} samples… (T)` : 'Add tag… (T)'"
        class="w-48 shrink-0 h-8"
        @keydown="onTagKeydown"
      />
      <datalist id="known-tags">
        <option v-for="t in library.tags" :key="t.id" :value="t.name" />
      </datalist>
    </div>
    <div ref="box" class="relative flex-1 mx-3 rounded-md bg-background overflow-hidden">
      <canvas ref="canvas" class="absolute inset-0 size-full cursor-pointer" @mousedown="onSeek" />
      <div v-if="loading" class="absolute inset-0 grid place-items-center text-muted-foreground text-xs">
        Reading waveform…
      </div>
      <div v-else-if="error" class="absolute inset-0 grid place-items-center text-destructive text-xs px-4 text-center">
        {{ error }}
      </div>
    </div>
    <!-- Status bar. One line tall: fields that do not fit wrap onto the hidden second line
         and drop out whole, least important (the last ones) first. -->
    <div class="h-7 shrink-0 px-3 flex flex-wrap items-center gap-x-3 overflow-hidden text-[11px] leading-7 text-muted-foreground tabular">
      <span v-if="sample.category" :title="categoryHint(sample.categorySource, sample.categoryConfidence)">{{ CATEGORY_LABELS[sample.category] ?? sample.category }}</span>
      <span v-if="sample.kind">{{ KIND_LABELS[sample.kind] }}</span>
      <span v-if="sample.bpm != null" :title="sourceHint(sample.bpmSource)">{{ formatBpm(sample.bpm) }} BPM</span>
      <span v-if="sample.key" :title="sourceHint(sample.keySource)">{{ sample.key }}</span>
      <span v-if="sample.peakDb != null" title="Sample peak">{{ formatDb(sample.peakDb) }}</span>
      <span v-if="sample.lufs != null" title="Integrated loudness">{{ formatDb(sample.lufs, 'LUFS') }}</span>
      <span>{{ formatDuration(sample.duration) }}</span>
      <span>{{ formatRate(sample.sampleRate) }}</span>
      <span v-if="sample.bitDepth">{{ sample.bitDepth }}-bit</span>
      <span>{{ formatChannels(sample.channels) }}</span>
      <span>{{ formatSize(sample.size) }}</span>
    </div>
    <div v-if="player.error" class="px-3 pb-2 text-destructive text-xs truncate">
      {{ player.error }}
    </div>
  </section>
</template>
