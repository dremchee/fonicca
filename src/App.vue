<script setup lang="ts">
import type { ComponentPublicInstance } from 'vue'
import { Search } from '@lucide/vue'
import { onKeyStroke } from '@vueuse/core'
import { storeToRefs } from 'pinia'
import { computed, onMounted, ref, useTemplateRef } from 'vue'
import CommandPalette from '@/components/CommandPalette.vue'
import ConfirmDialog from '@/components/ConfirmDialog.vue'
import FilterBar from '@/components/FilterBar.vue'
import { InputGroup, InputGroupAddon, InputGroupInput } from '@/components/ui/input-group'
import { Kbd } from '@/components/ui/kbd'
import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from '@/components/ui/resizable'
import { Toaster } from '@/components/ui/sonner'
import { TooltipProvider } from '@/components/ui/tooltip'
import SampleList from '@/components/SampleList.vue'
import SettingsDialog from '@/components/SettingsDialog.vue'
import SourceSidebar from '@/components/SourceSidebar.vue'
import WaveformView from '@/components/WaveformView.vue'
import { useLibraryStore } from '@/stores/library'
import { usePlayerStore } from '@/stores/player'
import { useSettingsStore } from '@/stores/settings'

const library = useLibraryStore()
const player = usePlayerStore()
const settings = useSettingsStore()
const paletteOpen = ref(false)
const { query, total, focused, selectedIds } = storeToRefs(library)
const searchInput = useTemplateRef<ComponentPublicInstance>('searchInput')
// InputGroupInput renders a bare <input> as its root.
const search = computed(() => searchInput.value?.$el as HTMLInputElement | undefined)

onMounted(() => Promise.all([settings.init(), library.init()]))

// Typing in a field or working in an open menu, popover or dialog: global shortcuts stay out of the way.
function isTyping(e: KeyboardEvent) {
  if (e.target instanceof Element && e.target.closest('[role="menu"], [role="dialog"], [role="alertdialog"]'))
    return true
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
onKeyStroke('Escape', (e) => {
  if (e.target instanceof Element && e.target.closest('[role="menu"], [role="dialog"], [role="alertdialog"]'))
    return
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
// Works from anywhere, the search field included.
onKeyStroke(['k', 'л'], (e) => {
  if (e.metaKey || e.ctrlKey) {
    e.preventDefault()
    paletteOpen.value = !paletteOpen.value
  }
})
onKeyStroke(',', (e) => {
  if (e.metaKey || e.ctrlKey) {
    e.preventDefault()
    settings.openDialog()
  }
})
</script>

<template>
  <TooltipProvider :delay-duration="500" :skip-delay-duration="300">
    <!-- Sidebar and player sizes are in px and saved by the splitters (localStorage). -->
    <ResizablePanelGroup direction="horizontal" auto-save-id="samplette-layout-columns" class="h-full">
      <ResizablePanel id="sidebar" :order="1" size-unit="px" :default-size="210" :min-size="170" :max-size="320">
        <SourceSidebar />
      </ResizablePanel>
      <ResizableHandle />
      <!-- The list needs ~520px for its columns; the sidebar gives way before it does. -->
      <ResizablePanel id="main" :order="2" as="main" size-unit="px" :min-size="520" class="min-w-0 flex flex-col">
        <header class="h-11 shrink-0 px-3 flex items-center gap-3 border-b border-border" data-tauri-drag-region>
          <InputGroup class="flex-1 max-w-md">
            <InputGroupAddon>
              <Search />
            </InputGroupAddon>
            <InputGroupInput
              ref="searchInput"
              v-model="query"
              type="search"
              placeholder="Search samples"
              aria-label="Search samples"
            />
            <InputGroupAddon align="inline-end">
              <Kbd>⌘F</Kbd>
            </InputGroupAddon>
          </InputGroup>
          <span class="text-xs text-muted-foreground tabular" data-tauri-drag-region>
            <template v-if="selectedIds.size > 1">{{ selectedIds.size.toLocaleString() }} selected · </template>
            {{ total.toLocaleString() }} {{ total === 1 ? 'sample' : 'samples' }}
            <template v-if="library.isScanning"> · scanning…</template>
          </span>
        </header>
        <FilterBar />
        <ResizablePanelGroup direction="vertical" auto-save-id="samplette-layout-rows" class="min-h-0 flex-1">
          <ResizablePanel id="list" :order="1" :min-size="20" class="flex flex-col">
            <SampleList />
          </ResizablePanel>
          <!-- The player only exists once a sample is selected. -->
          <template v-if="focused">
            <ResizableHandle />
            <ResizablePanel id="player" :order="2" size-unit="px" :default-size="196" :min-size="150" :max-size="480">
              <WaveformView :sample="focused" />
            </ResizablePanel>
          </template>
        </ResizablePanelGroup>
      </ResizablePanel>
    </ResizablePanelGroup>
    <SettingsDialog />
    <CommandPalette v-model:open="paletteOpen" />
    <ConfirmDialog />
    <Toaster theme="system" position="bottom-right" />
  </TooltipProvider>
</template>
