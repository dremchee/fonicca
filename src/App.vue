<script setup lang="ts">
import { onKeyStroke } from '@vueuse/core'
import { storeToRefs } from 'pinia'
import { onMounted, ref } from 'vue'
import FilterBar from '@/components/FilterBar.vue'
import SampleList from '@/components/SampleList.vue'
import SourceSidebar from '@/components/SourceSidebar.vue'
import WaveformView from '@/components/WaveformView.vue'
import { useLibraryStore } from '@/stores/library'
import { usePlayerStore } from '@/stores/player'

const library = useLibraryStore()
const player = usePlayerStore()
const { query, total, focused, selectedIds } = storeToRefs(library)
const search = ref<HTMLInputElement | null>(null)

onMounted(() => library.init())

function isTyping(e: KeyboardEvent) {
  return e.target instanceof HTMLInputElement && e.target.type !== 'checkbox'
}

async function move(delta: number, extend: boolean) {
  await library.moveFocus(delta, extend)
  const s = library.focused
  if (s && player.autoplay)
    player.play(s)
}

onKeyStroke('ArrowDown', (e) => {
  if (isTyping(e) && e.target !== search.value)
    return
  e.preventDefault()
  move(1, e.shiftKey)
})
onKeyStroke('ArrowUp', (e) => {
  if (isTyping(e) && e.target !== search.value)
    return
  e.preventDefault()
  move(-1, e.shiftKey)
})
onKeyStroke(' ', (e) => {
  if (isTyping(e))
    return
  e.preventDefault()
  if (focused.value)
    player.toggle(focused.value)
})
onKeyStroke('a', (e) => {
  if ((e.metaKey || e.ctrlKey) && !isTyping(e)) {
    e.preventDefault()
    library.selectAll()
  }
})
onKeyStroke(['f', 'а'], (e) => {
  if (isTyping(e) || e.metaKey || e.ctrlKey || e.altKey)
    return
  e.preventDefault()
  library.toggleFavorite()
})
onKeyStroke(['t', 'е'], (e) => {
  if (isTyping(e) || e.metaKey || e.ctrlKey || e.altKey)
    return
  e.preventDefault()
  library.requestTagInput()
})
onKeyStroke('Escape', () => {
  if (player.playingId !== null)
    player.stop()
  else
    search.value?.blur()
})
onKeyStroke(['f', 'а'], (e) => {
  if (e.metaKey || e.ctrlKey) {
    e.preventDefault()
    search.value?.focus()
    search.value?.select()
  }
})
</script>

<template>
  <div class="h-full flex">
    <SourceSidebar />
    <main class="flex-1 min-w-0 flex flex-col">
      <header class="h-11 shrink-0 px-3 flex items-center gap-3 border-b border-line" data-tauri-drag-region>
        <div class="relative flex-1 max-w-md">
          <span class="i-lucide-search absolute left-2.5 top-1/2 -translate-y-1/2 text-muted text-sm" />
          <input
            ref="search"
            v-model="query"
            type="search"
            placeholder="Search samples (⌘F)"
            class="w-full h-7 pl-8 pr-2 rounded-md bg-raised border border-transparent outline-none focus:border-accent placeholder:text-muted"
          >
        </div>
        <span class="text-xs text-muted tabular" data-tauri-drag-region>
          <template v-if="selectedIds.size > 1">{{ selectedIds.size.toLocaleString() }} selected · </template>
          {{ total.toLocaleString() }} {{ total === 1 ? 'sample' : 'samples' }}
          <template v-if="library.isScanning"> · scanning…</template>
        </span>
      </header>
      <FilterBar />
      <SampleList />
      <WaveformView :sample="focused" />
    </main>
  </div>
</template>
