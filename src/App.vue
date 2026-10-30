<script setup lang="ts">
import { debugMonitorOpen } from '@/composables/debugMonitor'
import { onKeyStroke } from '@vueuse/core'
import { isTauri } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { storeToRefs } from 'pinia'
import { computed, defineAsyncComponent, onMounted, ref, useTemplateRef, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import BatchDialog from '@/components/BatchDialog.vue'
import StemDialog from '@/components/StemDialog.vue'
import ApolloDialog from '@/components/ApolloDialog.vue'
import BasicPitchDialog from '@/components/BasicPitchDialog.vue'
import { useBasicPitchStore } from '@/stores/basicPitch'
import { useApolloStore } from '@/stores/apollo'
import { useStemsStore } from '@/stores/stems'
import CommandPalette from '@/components/CommandPalette.vue'
import ConfirmDialog from '@/components/ConfirmDialog.vue'
import FilterBar from '@/components/FilterBar.vue'
import LibraryToolbar from '@/components/LibraryToolbar.vue'
import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from '@/components/ui/resizable'
import { Toaster } from '@/components/ui/sonner'
import { TooltipProvider } from '@/components/ui/tooltip'
import SampleList from '@/components/SampleList.vue'
import SettingsHost from '@/components/SettingsHost.vue'
import TrackInfoDialog from '@/components/TrackInfoDialog.vue'
import SourceSidebar from '@/components/SourceSidebar.vue'
import EditorUnsavedDialog from '@/components/EditorUnsavedDialog.vue'
import { installAppMenu } from '@/menu'
import { useDuplicatesStore } from '@/stores/duplicates'
import { openEditor, useEditorStore } from '@/stores/editor'
import { openTrackInfo } from '@/composables/trackInfo'
import { useWaveformBackgrounds } from '@/composables/waveformBackgrounds'
import { useLibraryStore } from '@/stores/library'
import { usePlayerStore } from '@/stores/player'
import { useUpdatesStore } from '@/stores/updates'
import { useSettingsStore } from '@/stores/settings'

const EditorView = defineAsyncComponent(() => import('@/components/EditorView.vue'))
const DuplicatesView = defineAsyncComponent(() => import('@/components/DuplicatesView.vue'))
const DebugMonitor = defineAsyncComponent(() => import('@/components/DebugMonitor.vue'))

const { locale } = useI18n()
const library = useLibraryStore()
const player = usePlayerStore()
const editor = useEditorStore()
const backgrounds = useWaveformBackgrounds()
const settings = useSettingsStore()
const duplicates = useDuplicatesStore()
const paletteOpen = ref(false)
const { focused } = storeToRefs(library)
const toolbar = useTemplateRef<InstanceType<typeof LibraryToolbar>>('toolbar')
const search = computed(() => toolbar.value?.searchElement)

onMounted(async () => {
  await Promise.all([settings.init(), library.init(), duplicates.init(), useStemsStore().init(), useApolloStore().init(), useBasicPitchStore().init()])
  void useUpdatesStore().init()
  if (isTauri()) {
    const win = getCurrentWindow()
    await win.onCloseRequested((event) => {
      if (!editor.open) return
      event.preventDefault()
      void editor.closeEmbedded().then((closed) => { if (closed) void win.close() })
    })
  }
})

// The native menu is rebuilt whenever the language changes.
watch(locale, () => installAppMenu(() => settings.openDialog()).catch(console.error), { immediate: true })

watch(() => library.scope, () => { settings.dialogOpen = false })
watch(() => player.playingSample, sample => { if (sample) void backgrounds.prepare(sample) })

// DuplicatesView replaces the list and handles its own arrows and Space.
const inDuplicates = computed(() => library.scope.kind === 'duplicates')

// Typing in a field or working in an open menu, popover or dialog: global shortcuts stay out of the way.
function isTyping(e: KeyboardEvent) {
  if (settings.dialogOpen || inDuplicates.value || e.defaultPrevented)
    return true
  if (e.target instanceof Element && e.target.closest('[data-editor-panel], button, a, [role="slider"], [role="checkbox"], [role="combobox"], [role="menu"], [role="dialog"], [role="alertdialog"]'))
    return true
  return e.target instanceof HTMLInputElement && e.target.type !== 'checkbox'
}

async function move(delta: number, extend: boolean) {
  await library.moveFocus(delta, extend)
  const s = library.focused
  if (s && player.autoplay) {
    await editor.stopPlayback()
    await player.play(s)
  }
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
    void editor.stopPlayback().then(() => { if (focused.value) return player.toggle(focused.value) })
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
  if (settings.dialogOpen) return
  if (e.target instanceof Element && e.target.closest('[data-editor-panel], [role="menu"], [role="dialog"], [role="alertdialog"]'))
    return
  if (player.playingId !== null)
    player.stop()
  else
    search.value?.blur()
})
onKeyStroke(['f', 'а'], (e) => {
  if (e.metaKey || e.ctrlKey) {
    e.preventDefault()
    settings.dialogOpen = false
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
        <SettingsHost embedded />
        <div v-show="!settings.dialogOpen" class="min-h-0 flex-1 flex flex-col">
          <template v-if="!inDuplicates && !editor.expanded">
            <LibraryToolbar ref="toolbar" @open-search="paletteOpen = true" />
            <FilterBar />
          </template>
          <ResizablePanelGroup direction="vertical" auto-save-id="fonicca-layout-rows" class="min-h-0 flex-1">
            <ResizablePanel v-if="!editor.expanded" id="list" :order="1" :min-size="20" class="flex flex-col">
              <DuplicatesView v-if="inDuplicates" />
              <SampleList v-else />
            </ResizablePanel>
            <template v-if="editor.open">
              <ResizableHandle v-if="!editor.expanded" />
              <ResizablePanel id="editor" :order="2" size-unit="px" :default-size="320" :min-size="200">
                <EditorView embedded :expanded="editor.expanded" @close="editor.closeEmbedded()" @toggle-expand="editor.expanded = !editor.expanded" />
              </ResizablePanel>
            </template>
          </ResizablePanelGroup>
        </div>
      </ResizablePanel>
    </ResizablePanelGroup>
    <TrackInfoDialog />
    <BatchDialog />
    <StemDialog />
    <ApolloDialog />
    <BasicPitchDialog />
    <CommandPalette v-model:open="paletteOpen" />
    <DebugMonitor v-if="debugMonitorOpen" :library-diagnostics="library.pageDiagnostics" />
    <ConfirmDialog />
    <EditorUnsavedDialog />
    <Toaster theme="system" position="bottom-right" />
  </TooltipProvider>
</template>
