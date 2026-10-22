<script setup lang="ts">
import type { Sample } from '@/api'
import { useResizeObserver } from '@vueuse/core'
import { computed, onMounted, ref, shallowRef, watch } from 'vue'
import { api } from '@/api'
import { formatChannels, formatDuration, formatRate, formatSize } from '@/format'
import { useLibraryStore } from '@/stores/library'
import { usePlayerStore } from '@/stores/player'

const props = defineProps<{ sample: Sample | undefined }>()

const library = useLibraryStore()
const player = usePlayerStore()
const tagInput = ref<HTMLInputElement | null>(null)
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
  <section class="h-52 shrink-0 border-t border-line bg-panel flex flex-col">
    <template v-if="sample">
      <div class="h-10 px-3 flex items-center gap-3">
        <button
          class="size-7 rounded-full grid place-items-center bg-accent text-white shrink-0"
          :title="player.playingId === sample.id ? 'Stop (Space)' : 'Play (Space)'"
          @click="player.toggle(sample)"
        >
          <span :class="player.playingId === sample.id ? 'i-lucide-square' : 'i-lucide-play'" class="text-xs" />
        </button>
        <div
          class="min-w-0 flex-1 cursor-grab"
          draggable="true"
          title="Drag into your DAW"
          @dragstart="onDragStart"
        >
          <div class="truncate font-medium">
            {{ sample.name }}<span class="text-muted">.{{ sample.ext }}</span>
          </div>
          <div class="truncate text-[11px] text-muted">
            {{ sample.path }}
          </div>
        </div>
        <div class="flex gap-3 text-[11px] text-muted tabular shrink-0">
          <span>{{ formatDuration(sample.duration) }}</span>
          <span>{{ formatRate(sample.sampleRate) }}</span>
          <span v-if="sample.bitDepth">{{ sample.bitDepth }}-bit</span>
          <span>{{ formatChannels(sample.channels) }}</span>
          <span>{{ formatSize(sample.size) }}</span>
        </div>
        <label class="flex items-center gap-1.5 text-[11px] text-muted shrink-0">
          <input v-model="player.autoplay" type="checkbox" class="accent-[var(--accent)]">
          Autoplay
        </label>
      </div>
      <div class="h-8 px-3 flex items-center gap-1.5 min-w-0">
        <button
          class="size-6 shrink-0 rounded grid place-items-center hover:bg-raised"
          :title="sample.favorite ? 'Remove from favorites (F)' : 'Add to favorites (F)'"
          @click="library.toggleFavorite()"
        >
          <span :class="sample.favorite ? 'i-lucide-star text-amber-400' : 'i-lucide-star text-muted'" class="text-sm" />
        </button>
        <div class="flex gap-1 overflow-x-auto min-w-0">
          <span v-for="t in sample.tags" :key="t.id" class="tag">
            {{ t.name }}
            <button class="opacity-60 hover:opacity-100" title="Remove tag" @click="library.removeTag(t.id)">
              <span class="i-lucide-x text-[10px]" />
            </button>
          </span>
        </div>
        <input
          ref="tagInput"
          v-model="newTag"
          list="known-tags"
          :placeholder="selectionSize > 1 ? `Tag ${selectionSize} samples… (T)` : 'Add tag… (T)'"
          class="w-44 shrink-0 h-6 px-2 rounded bg-raised border border-transparent outline-none focus:border-accent placeholder:text-muted text-xs"
          @keydown="onTagKeydown"
        >
        <datalist id="known-tags">
          <option v-for="t in library.tags" :key="t.id" :value="t.name" />
        </datalist>
      </div>
      <div ref="box" class="relative flex-1 mx-3 mb-3 rounded-md bg-bg overflow-hidden">
        <canvas ref="canvas" class="absolute inset-0 size-full cursor-pointer" @mousedown="onSeek" />
        <div v-if="loading" class="absolute inset-0 grid place-items-center text-muted text-xs">
          Reading waveform…
        </div>
        <div v-else-if="error" class="absolute inset-0 grid place-items-center text-danger text-xs px-4 text-center">
          {{ error }}
        </div>
      </div>
    </template>
    <div v-else class="flex-1 grid place-items-center text-muted text-xs">
      Select a sample to preview it · ↑ ↓ to browse · Space to play
    </div>
    <div v-if="player.error" class="px-3 pb-2 text-danger text-xs truncate">
      {{ player.error }}
    </div>
  </section>
</template>

<style scoped>
.tag {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  flex-shrink: 0;
  padding: 0 4px 0 8px;
  border-radius: 4px;
  background: var(--accent-soft);
  color: var(--accent);
  font-size: 11px;
  line-height: 20px;
}
</style>
