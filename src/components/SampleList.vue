<script setup lang="ts">
import type { Sample } from '@/api'
import { useVirtualizer } from '@tanstack/vue-virtual'
import { storeToRefs } from 'pinia'
import { computed, ref, watch } from 'vue'
import { startFileDrag } from '@/api'
import { formatChannels, formatDuration, formatRate } from '@/format'
import { useLibraryStore } from '@/stores/library'
import { usePlayerStore } from '@/stores/player'

const ROW_HEIGHT = 30

const library = useLibraryStore()
const player = usePlayerStore()
const { total, selectedIndex } = storeToRefs(library)

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

watch(selectedIndex, (i) => {
  if (i >= 0)
    virtualizer.value.scrollToIndex(i, { align: 'auto' })
})

function onClick(index: number) {
  library.select(index)
  const s = library.sampleAt(index)
  if (s && player.autoplay)
    player.play(s)
}

function onDoubleClick(index: number) {
  const s = library.sampleAt(index)
  if (s)
    player.toggle(s)
}

function onDragStart(e: DragEvent, sample: Sample) {
  // Replace the HTML drag with a native file drag so DAWs and Finder accept it.
  e.preventDefault()
  startFileDrag([sample.path]).catch(err => console.error('drag failed', err))
}
</script>

<template>
  <div class="flex-1 min-h-0 flex flex-col">
    <div class="header grid items-center px-3 h-7 border-b border-line text-[11px] uppercase tracking-wide text-muted">
      <span />
      <span>Name</span>
      <span class="text-right">Length</span>
      <span class="text-right">Rate</span>
      <span class="text-right">Bits</span>
      <span class="text-right">Ch</span>
      <span>Type</span>
    </div>

    <div ref="scroller" class="flex-1 overflow-y-auto" tabindex="-1">
      <div v-if="total === 0" class="h-full grid place-items-center text-muted">
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
            :class="{ selected: item.index === selectedIndex, playing: s.id === player.playingId }"
            :style="{ height: `${item.size}px`, transform: `translateY(${item.start}px)` }"
            draggable="true"
            :title="s.path"
            @mousedown="onClick(item.index)"
            @dblclick="onDoubleClick(item.index)"
            @dragstart="onDragStart($event, s)"
          >
            <span class="grid place-items-center">
              <span v-if="s.id === player.playingId" class="i-lucide-volume-2 text-accent text-xs" />
            </span>
            <span class="truncate">{{ s.name }}</span>
            <span class="text-right text-muted">{{ formatDuration(s.duration) }}</span>
            <span class="text-right text-muted">{{ formatRate(s.sampleRate) }}</span>
            <span class="text-right text-muted">{{ s.bitDepth ?? '—' }}</span>
            <span class="text-right text-muted">{{ formatChannels(s.channels) }}</span>
            <span class="text-muted uppercase text-[11px]">{{ s.ext }}</span>
          </div>
          <div
            v-else
            class="row grid items-center px-3 absolute inset-x-0"
            :style="{ height: `${item.size}px`, transform: `translateY(${item.start}px)` }"
          >
            <span />
            <span class="h-2.5 w-40 rounded bg-raised" />
          </div>
        </template>
      </div>
    </div>
  </div>
</template>

<style scoped>
.header,
.row {
  grid-template-columns: 20px minmax(0, 1fr) 64px 72px 40px 60px 44px;
  column-gap: 12px;
}
.row:hover {
  background: var(--raised);
}
.row.selected {
  background: var(--accent-soft);
}
.row.playing {
  color: var(--accent);
}
</style>
