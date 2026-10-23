<script setup lang="ts">
import type { Sample } from '@/api'
import { useVirtualizer } from '@tanstack/vue-virtual'
import { useEventListener } from '@vueuse/core'
import { storeToRefs } from 'pinia'
import { computed, ref, watch } from 'vue'
import { CATEGORY_LABELS, categoryHint, formatBpm, formatDuration, KIND_LABELS, sourceHint } from '@/format'
import SampleContextMenu from '@/components/SampleContextMenu.vue'
import { ContextMenu, ContextMenuTrigger } from '@/components/ui/context-menu'
import { useLibraryStore } from '@/stores/library'
import { usePlayerStore } from '@/stores/player'

const ROW_HEIGHT = 30

const library = useLibraryStore()
const player = usePlayerStore()
const { total, focusIndex, selectedIds } = storeToRefs(library)

const scroller = ref<HTMLElement | null>(null)

const virtualizer = useVirtualizer(computed(() => ({
  count: total.value,
  getScrollElement: () => scroller.value,
  estimateSize: () => ROW_HEIGHT,
  overscan: 20,
})))

const items = computed(() => virtualizer.value.getVirtualItems())
const rows = computed(() => items.value.map(item => ({ item, sample: library.sampleAt(item.index) })))

watch(items, (list) => {
  if (list.length)
    library.ensureRange(list[0].index, list[list.length - 1].index)
}, { immediate: true })

// Analyze what is on screen first.
let prioritizeTimer: ReturnType<typeof setTimeout> | undefined
watch(rows, (list) => {
  clearTimeout(prioritizeTimer)
  prioritizeTimer = setTimeout(() => library.prioritizeAnalysis(list.map(r => r.sample)), 300)
})

watch(focusIndex, (i) => {
  if (i >= 0)
    virtualizer.value.scrollToIndex(i, { align: 'auto' })
})

// A plain press on a row that is part of a multi-selection keeps the selection,
// so it can be dragged as a group; it collapses to that row on click instead.
let collapseOnClick = -1

async function onMouseDown(e: MouseEvent, index: number, sample: Sample) {
  if (e.button !== 0)
    return
  collapseOnClick = -1
  const toggle = e.metaKey || e.ctrlKey
  if (!e.shiftKey && !toggle && selectedIds.value.size > 1 && selectedIds.value.has(sample.id)) {
    collapseOnClick = index
    focusIndex.value = index
  }
  else {
    await library.clickRow(index, { shift: e.shiftKey, toggle })
  }
  if (player.autoplay && !e.shiftKey)
    player.play(sample)
}

function onClick(index: number) {
  if (collapseOnClick === index)
    library.clickRow(index)
  collapseOnClick = -1
}

function onDoubleClick(index: number) {
  const s = library.sampleAt(index)
  if (s)
    player.toggle(s)
}

const menuOpen = ref(false)
const menuSample = ref<Sample | null>(null)

useEventListener(window, 'blur', () => (menuOpen.value = false))

// One menu for the whole list: find the row under the cursor and skip
// the menu entirely on empty space and placeholder rows.
function onContextMenu(e: MouseEvent) {
  const row = (e.target as HTMLElement).closest<HTMLElement>('[data-index]')
  const index = row ? Number(row.dataset.index) : -1
  const sample = index >= 0 ? library.sampleAt(index) : undefined
  if (!sample) {
    e.preventDefault()
    return
  }
  menuSample.value = sample
  if (!selectedIds.value.has(sample.id))
    library.clickRow(index)
}

function onDragStart(e: DragEvent, sample: Sample) {
  // Replace the HTML drag with a native file drag so DAWs and Finder accept it.
  e.preventDefault()
  collapseOnClick = -1
  library.dragSamples(sample).catch(err => console.error('drag failed', err))
}
</script>

<template>
  <div class="flex-1 min-h-0 flex flex-col">
    <div class="header grid items-center px-3 h-7 border-b border-border text-[11px] uppercase tracking-wide text-muted-foreground">
      <span />
      <span />
      <span>Name</span>
      <span>Tags</span>
      <span>Category</span>
      <span>Kind</span>
      <span class="text-right">BPM</span>
      <span>Key</span>
      <span class="text-right">Length</span>

      <span>Type</span>
    </div>

    <ContextMenu v-model:open="menuOpen">
      <ContextMenuTrigger as-child>
        <div ref="scroller" class="flex-1 overflow-y-auto outline-none" tabindex="-1" @contextmenu="onContextMenu">
          <div v-if="total === 0" class="h-full grid place-items-center text-muted-foreground">
            <div class="text-center">
              <span class="i-lucide-audio-waveform text-3xl opacity-50" />
              <p class="mt-2">
                {{ library.query ? 'Nothing matches your search' : 'No samples yet' }}
              </p>
            </div>
          </div>

          <div v-else class="relative w-full" :style="{ height: `${virtualizer.getTotalSize()}px` }">
            <template v-for="{ item, sample: s } in rows" :key="item.index">
              <div
                v-if="s"
                class="row grid items-center px-3 absolute inset-x-0 tabular"
                :class="{
                  selected: selectedIds.has(s.id),
                  focused: item.index === focusIndex,
                  playing: s.id === player.playingId,
                }"
                :style="{ height: `${item.size}px`, transform: `translateY(${item.start}px)` }"
                :data-index="item.index"
                draggable="true"
                :title="s.path"
                @mousedown="onMouseDown($event, item.index, s)"
                @click="onClick(item.index)"
                @dblclick="onDoubleClick(item.index)"
                @dragstart="onDragStart($event, s)"
              >
                <span class="grid place-items-center">
                  <span v-if="s.id === player.playingId" class="i-lucide-volume-2 text-primary text-xs" />
                </span>
                <button
                  class="star grid place-items-center" :class="{ on: s.favorite }"
                  :title="s.favorite ? 'Remove from favorites (F)' : 'Add to favorites (F)'"
                  @mousedown.stop
                  @click.stop="library.toggleFavorite(s)"
                >
                  <span :class="s.favorite ? 'i-lucide-star text-amber-400' : 'i-lucide-star'" class="text-xs" />
                </button>
                <span class="truncate">{{ s.name }}</span>
                <span class="flex gap-1 overflow-hidden">
                  <span v-for="t in s.tags" :key="t.id" class="chip">{{ t.name }}</span>
                </span>
                <span
                  class="truncate text-[11px]" :class="s.categorySource === 'audio' ? 'text-muted-foreground' : ''"
                  :title="categoryHint(s.categorySource, s.categoryConfidence)"
                >{{ s.category ? CATEGORY_LABELS[s.category] ?? s.category : '' }}</span>
                <span class="text-muted-foreground text-[11px]">
                  <template v-if="s.kind">{{ KIND_LABELS[s.kind] }}</template>
                  <span v-else-if="!s.analyzed" class="opacity-50">…</span>
                </span>
                <span class="text-right" :class="s.bpmSource === 'audio' ? 'text-muted-foreground' : ''" :title="sourceHint(s.bpmSource)">
                  {{ s.bpm != null ? formatBpm(s.bpm) : '' }}
                </span>
                <span :class="s.keySource === 'audio' ? 'text-muted-foreground' : ''" :title="sourceHint(s.keySource)">{{ s.key ?? '' }}</span>
                <span class="text-right text-muted-foreground">{{ formatDuration(s.duration) }}</span>
                <span class="text-muted-foreground uppercase text-[11px]">{{ s.ext }}</span>
              </div>
              <div
                v-else
                class="row grid items-center px-3 absolute inset-x-0"
                :style="{ height: `${item.size}px`, transform: `translateY(${item.start}px)` }"
              >
                <span />
                <span />
                <span class="h-2.5 w-40 rounded bg-muted" />
              </div>
            </template>
          </div>
        </div>
      </ContextMenuTrigger>
      <SampleContextMenu v-if="menuSample" :sample="menuSample" @close="menuOpen = false" />
    </ContextMenu>
  </div>
</template>

<style scoped>
.header,
.row {
  grid-template-columns: 16px 18px minmax(0, 2fr) minmax(0, 1fr) 76px 60px 44px 36px 56px 40px;
  column-gap: 12px;
}
.row:hover {
  background: var(--accent);
}
.row.selected {
  background: var(--primary-soft);
}
.row.focused {
  box-shadow: inset 2px 0 0 var(--primary);
}
.star {
  color: var(--muted-foreground);
  opacity: 0;
}
.star.on,
.row:hover .star {
  opacity: 1;
}
.chip {
  flex-shrink: 0;
  padding: 0 6px;
  border-radius: 4px;
  background: var(--accent);
  color: var(--muted-foreground);
  font-size: 11px;
  line-height: 18px;
}
.row.selected .chip {
  background: var(--card);
}
.row.playing {
  color: var(--primary);
}
</style>
