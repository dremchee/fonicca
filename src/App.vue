<script setup lang="ts">
import { onKeyStroke } from '@vueuse/core'
import { storeToRefs } from 'pinia'
import { computed, onMounted, ref, useTemplateRef, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import BatchDialog from '@/components/BatchDialog.vue'
import CommandPalette from '@/components/CommandPalette.vue'
import ConfirmDialog from '@/components/ConfirmDialog.vue'
import DuplicatesView from '@/components/DuplicatesView.vue'
import FilterBar from '@/components/FilterBar.vue'
import LibraryToolbar from '@/components/LibraryToolbar.vue'
import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from '@/components/ui/resizable'
import { Toaster } from '@/components/ui/sonner'
import { TooltipProvider } from '@/components/ui/tooltip'
import SampleList from '@/components/SampleList.vue'
import SettingsDialog from '@/components/SettingsDialog.vue'
import TrackInfoDialog from '@/components/TrackInfoDialog.vue'
import SourceSidebar from '@/components/SourceSidebar.vue'
import WaveformView from '@/components/WaveformView.vue'
import { installAppMenu } from '@/menu'
import { useDuplicatesStore } from '@/stores/duplicates'
import { openEditor } from '@/stores/editor'
import { openTrackInfo } from '@/composables/trackInfo'
import { useLibraryStore } from '@/stores/library'
import { usePlayerStore } from '@/stores/player'
import { useSettingsStore } from '@/stores/settings'

const { locale } = useI18n()
const library = useLibraryStore()
const player = usePlayerStore()
const settings = useSettingsStore()
const duplicates = useDuplicatesStore()
const paletteOpen = ref(false)
const { focused } = storeToRefs(library)
// The selected sample, or the one playing once nothing is selected.
const shownSample = computed(() => focused.value ?? player.playingSample ?? undefined)
const toolbar = useTemplateRef<InstanceType<typeof LibraryToolbar>>('toolbar')
const search = computed(() => toolbar.value?.searchElement)

onMounted(() => Promise.all([settings.init(), library.init(), duplicates.init()]))

// The native menu is rebuilt whenever the language changes.
watch(locale, () => installAppMenu(() => settings.openDialog()).catch(console.error), { immediate: true })

// DuplicatesView replaces the list and handles its own arrows and Space.
const inDuplicates = computed(() => library.scope.kind === 'duplicates')

// Typing in a field or working in an open menu, popover or dialog: global shortcuts stay out of the way.
function isTyping(e: KeyboardEvent) {
  if (inDuplicates.value || e.defaultPrevented)
    return true
  if (e.target instanceof Element && e.target.closest('button, a, [role="slider"], [role="checkbox"], [role="combobox"], [role="menu"], [role="dialog"], [role="alertdialog"]'))
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
onKeyStroke(['i', 'ш'], (e) => {
  if (isTyping(e) || e.metaKey || e.ctrlKey || e.altKey || !library.focused)
    return
  e.preventDefault()
  openTrackInfo(library.focused)
})
onKeyStroke(['e', 'у'], (e) => {
  if (isTyping(e) || e.metaKey || e.ctrlKey || e.altKey || !library.focused)
    return
  e.preventDefault()
  openEditor(library.focused)
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
    <ResizablePanelGroup direction="horizontal" auto-save-id="fonicca-layout-columns" class="h-full">
      <ResizablePanel id="sidebar" :order="1" size-unit="px" :default-size="210" :min-size="170" :max-size="320">
        <SourceSidebar />
      </ResizablePanel>
      <ResizableHandle />
      <!-- The list needs ~520px for its columns; the sidebar gives way before it does. -->
      <ResizablePanel id="main" :order="2" as="main" size-unit="px" :min-size="520" class="min-w-0 flex flex-col">
        <DuplicatesView v-if="inDuplicates" />
        <template v-else>
          <LibraryToolbar ref="toolbar" />
          <FilterBar />
          <ResizablePanelGroup direction="vertical" auto-save-id="fonicca-layout-rows" class="min-h-0 flex-1">
            <ResizablePanel id="list" :order="1" :min-size="20" class="flex flex-col">
              <SampleList />
            </ResizablePanel>
            <!-- The player exists while a sample is selected, or one is still playing. -->
            <template v-if="shownSample">
              <ResizableHandle />
              <ResizablePanel id="player" :order="2" size-unit="px" :default-size="196" :min-size="150" :max-size="480">
                <WaveformView :sample="shownSample" />
              </ResizablePanel>
            </template>
          </ResizablePanelGroup>
        </template>
      </ResizablePanel>
    </ResizablePanelGroup>
    <SettingsDialog />
    <TrackInfoDialog />
    <BatchDialog />
    <CommandPalette v-model:open="paletteOpen" />
    <ConfirmDialog />
    <Toaster theme="system" position="bottom-right" />
  </TooltipProvider>
</template>
