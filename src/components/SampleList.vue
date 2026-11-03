<script setup lang="ts">
import { rowClickSeeks } from '@/row-playback'
import { useWaveformBackgrounds } from '@/composables/waveformBackgrounds'
import type { Sample } from '@/api'
import type { ResizableSampleColumnId } from '@/sample-columns'
import { useVirtualizer } from '@tanstack/vue-virtual'
import { AudioLines, FolderPlus, Layers, SearchX, Sparkles, Star, Tag, X } from '@lucide/vue'
import { onKeyStroke, useEventListener, usePreferredReducedMotion, useScroll } from '@vueuse/core'
import { storeToRefs } from 'pinia'
import { computed, nextTick, onScopeDispose, ref, watch } from 'vue'
import { isTauri } from '@tauri-apps/api/core'
import { useI18n } from 'vue-i18n'
import Hint from '@/components/Hint.vue'
import SampleListHints from '@/components/SampleListHints.vue'
import SampleRow from '@/components/SampleRow.vue'
import { usePinnedPlayback } from '@/composables/pinnedPlayback'
import { Tooltip } from '@/components/ui/tooltip'
import SampleContextMenu from '@/components/SampleContextMenu.vue'
import { useSampleTable } from '@/composables/sampleTable'
import SampleColumnHeader from '@/components/SampleColumnHeader.vue'
import { useSampleColumnWidths } from '@/composables/sampleColumnWidths'
import { Button } from '@/components/ui/button'
import { ContextMenu, ContextMenuTrigger } from '@/components/ui/context-menu'
import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from '@/components/ui/empty'
import { openEditor, useEditorStore } from '@/stores/editor'
import { PAGE_SIZE, useLibraryStore } from '@/stores/library'
import { usePlayerStore } from '@/stores/player'
import { useSettingsStore } from '@/stores/settings'

const ROW_HEIGHT = 36

const { t } = useI18n()
const backgrounds = useWaveformBackgrounds()
const library = useLibraryStore()
const player = usePlayerStore()
const editor = useEditorStore()
const settings = useSettingsStore()
const { total, focusIndex, selectedIds, scope } = storeToRefs(library)

const { table, visibleColumns } = useSampleTable()
const headerColumns = computed(() => table.getVisibleLeafColumns().map(column => ({
  column, id: column.id as ResizableSampleColumnId,
})))
const tableViewport = ref<HTMLElement | null>(null)
const { tableStyle, resizingColumn, preview: previewColumnWidth, commit: commitColumnWidth, cancel: cancelColumnResize, reset: resetColumnWidths } = useSampleColumnWidths(visibleColumns, tableViewport)

const similarTo = computed(() => (scope.value.kind === 'similar' ? scope.value : null))

// What to say when the list is empty depends on why it is empty.
const empty = computed(() => {
  if (!library.sources.length)
    return { icon: FolderPlus, title: t('list.empty.noSamplesYet'), text: t('list.empty.noSamplesYetText') }
  if (library.query || library.hasAnalysisFilters)
    return { icon: SearchX, title: t('list.empty.nothingMatches'), text: t('list.empty.nothingMatchesText') }
  switch (scope.value.kind) {
    case 'favorites':
      return { icon: Star, title: t('list.empty.noFavorites'), text: t('list.empty.noFavoritesText') }
    case 'collection':
      return { icon: Layers, title: t('list.empty.emptyCollection'), text: t('list.empty.emptyCollectionText') }
    case 'tag':
      return { icon: Tag, title: t('list.empty.noTagged'), text: t('list.empty.noTaggedText') }
    case 'similar':
      return { icon: Sparkles, title: t('list.empty.noSimilar'), text: t('list.empty.noSimilarText') }
    case 'source':
      return library.isScanning
        ? { icon: AudioLines, title: t('list.empty.scanning'), text: t('list.empty.scanningText') }
        : { icon: AudioLines, title: t('list.empty.emptyFolder'), text: t('list.empty.emptyFolderText') }
    default:
      return { icon: AudioLines, title: t('list.empty.noSamples'), text: '' }
  }
})

// Esc leaves the similar view once nothing is playing (the first Esc stops playback).
// Capture phase: checked before the app-wide Esc handler stops the player.
useEventListener(window, 'keydown', (e: KeyboardEvent) => {
  if (settings.dialogOpen || e.key !== 'Escape' || !similarTo.value || player.playingId !== null)
    return
  if (e.target instanceof Element && e.target.closest('[role="menu"], [role="dialog"], [role="alertdialog"], input'))
    return
  library.exitSimilar()
}, { capture: true })

// S: find samples that sound like the focused one (Esc or × to go back).
onKeyStroke(['s', 'ы'], (e) => {
  if (settings.dialogOpen || e.metaKey || e.ctrlKey || e.altKey || (e.target instanceof Element && e.target.closest('input, [role="menu"], [role="dialog"]')))
    return
  const s = library.focused
  if (s) {
    e.preventDefault()
    library.findSimilar(s)
  }
})

const scroller = ref<HTMLElement | null>(null)
const { y: scrollTop } = useScroll(scroller)
const reducedMotion = usePreferredReducedMotion()
const { playingIndex, pinnedSample } = usePinnedPlayback({
  pages: () => library.pages,
  playingId: () => player.playingId,
  scrollTop,
  pageSize: PAGE_SIZE,
  rowHeight: ROW_HEIGHT,
})
const listSurface = ref<HTMLElement | null>(null)
const virtualizer = useVirtualizer(computed(() => ({
  count: total.value,
  getScrollElement: () => scroller.value,
  estimateSize: () => ROW_HEIGHT,
  overscan: 20,
})))

const items = computed(() => virtualizer.value.getVirtualItems())
const rows = computed(() => items.value.map((item) => {
  const sample = library.sampleAt(item.index)
  const stemChild = sample?.stemParentId != null && library.expandedStems.has(sample.stemParentId)
  const next = library.sampleAt(item.index + 1)
  const groupStart = !!sample && library.expandedStems.has(sample.id) && next?.stemParentId === sample.id
  const groupEnd = stemChild && next?.stemParentId !== sample?.stemParentId
  const grouped = groupStart || stemChild
  return { item, sample, stemChild, groupStart, groupEnd, grouped }
}))

watch(() => rows.value.map(row => row.sample).filter((sample): sample is Sample => !!sample), samples => backgrounds.restore(samples), { immediate: true })

// A scope change may keep the same row count and virtual range while clearing pages.
watch([items, () => library.pages.size], ([list]) => {
  if (list.length)
    void library.ensureVisibleRange(list[0].index, list[list.length - 1].index)
      .catch(error => console.error('Failed to load visible library rows', error))
}, { immediate: true })

// Analyze what is on screen first.
let prioritizeTimer: ReturnType<typeof setTimeout> | undefined
onScopeDispose(() => clearTimeout(prioritizeTimer))
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
  const fromCursor = rowClickSeeks(settings.settings.rowPlaybackMode, library.focused?.id === sample.id, e.altKey)
  collapseOnClick = -1
  const toggle = e.metaKey || e.ctrlKey
  if (!e.shiftKey && !toggle && selectedIds.value.size > 1 && selectedIds.value.has(sample.id)) {
    collapseOnClick = index
    focusIndex.value = index
  }
  else {
    await library.clickRow(index, { shift: e.shiftKey, toggle })
  }
  // Row-click preferences decide whether an available waveform seeks.
  // A second press opens the editor without a second playback action.
  if (!e.shiftKey && !toggle && e.detail < 2) {
    const target = e.target instanceof Element ? e.target : null
    const name = target?.closest<HTMLElement>('.track-name')
    const wave = fromCursor && name && backgrounds.pathFor(sample) && sample.duration && !target?.closest('button')
    await editor.stopPlayback()
    if (wave && name) {
      const rect = name.getBoundingClientRect()
      const start = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width)) * sample.duration!
      if (player.playingId === sample.id && !player.paused)
        await player.seek(sample, start)
      else
        await player.play(sample, start)
    }
    else if (settings.settings.rowPlaybackMode !== 'cursor' || player.playingId !== sample.id || player.paused) {
      await player.play(sample)
    }
    void backgrounds.prepare(sample)
  }
}

function onClick(index: number) {
  if (collapseOnClick === index)
    library.clickRow(index)
  collapseOnClick = -1
}

async function toggleRowPlayback(sample: Sample) {
  await editor.stopPlayback()
  await player.toggle(sample)
  void backgrounds.prepare(sample)
}

// Double-click opens the sample in the embedded editor.
function onDoubleClick(index: number) {
  const s = library.sampleAt(index)
  if (s)
    openEditor(s)
}

const nativeMenus = isTauri()
const sampleMenu = ref<InstanceType<typeof SampleContextMenu> | null>(null)
const menuOpen = ref(false)
const menuSample = ref<Sample | null>(null)

useEventListener(window, 'blur', () => (menuOpen.value = false))

// One menu for the whole list: find the row under the cursor and skip
// the menu entirely on empty space and placeholder rows.
async function onContextMenu(e: MouseEvent) {
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
  if (nativeMenus) {
    e.preventDefault()
    const position = { x: e.clientX, y: e.clientY }
    await nextTick()
    await sampleMenu.value?.openNative(position)
  }
}

function onDragStart(e: DragEvent, sample: Sample) {
  // Replace the HTML drag with a native file drag so DAWs and Finder accept it.
  e.preventDefault()
  collapseOnClick = -1
  library.dragSamples(sample).catch(err => console.error('drag failed', err))
}
</script>

<template>

  <div class="sample-list flex-1 min-w-0 min-h-0 flex flex-col">
    <div v-if="similarTo" class="h-9 shrink-0 px-3 flex items-center gap-2 border-b border-border bg-primary-soft text-sm">
      <AudioLines class="size-4 text-primary" />
      <span class="truncate">{{ t('list.similar.soundsLike') }} <span class="font-medium">{{ similarTo.name }}</span></span>
      <span class="text-muted-foreground text-xs">{{ t('list.similar.mostSimilarFirst') }}</span>
      <Hint :text="t('list.similar.back')" shortcut="Esc">
        <Button variant="ghost" size="icon-sm" class="ml-auto" :aria-label="t('list.similar.back')" @click="library.exitSimilar()">
          <X />
        </Button>
      </Hint>
    </div>
    <div ref="tableViewport" class="table-viewport flex-1 min-w-0 min-h-0 overflow-x-auto">
      <div class="table-content relative flex flex-col h-full" :style="total > 0 ? tableStyle : undefined">
        <div v-if="total > 0" class="header grid items-center px-3 h-8 border-b border-border text-[11px] uppercase tracking-wide text-muted-foreground">
          <span />
          <SampleColumnHeader
            v-for="{ column, id } in headerColumns" :key="id" :column="column"
            :label="id === 'bpm' ? 'BPM' : t(`list.columns.${id}`)"
            :align-right="id === 'bpm' || id === 'length'"
            @resize="previewColumnWidth(id, $event)" @commit="commitColumnWidth(id, $event)"
            @cancel="cancelColumnResize" @reset="resetColumnWidths(id)"
          />
          <span aria-hidden="true" />
        </div>

        <Tooltip>
        <ContextMenu v-model:open="menuOpen">
          <ContextMenuTrigger as-child :disabled="nativeMenus">
            <div ref="listSurface" class="relative isolate flex-1 min-h-0 overflow-hidden" @contextmenu="onContextMenu">
            <div ref="scroller" class="h-full min-h-0 overflow-y-auto outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring" tabindex="0" :aria-label="t('sidebar.allSamples')">
              <Empty v-if="total === 0" class="h-full">
                <EmptyHeader>
                  <EmptyMedia variant="icon">
                    <component :is="empty.icon" />
                  </EmptyMedia>
                  <EmptyTitle>{{ empty.title }}</EmptyTitle>
                  <EmptyDescription v-if="empty.text">
                    {{ empty.text }}
                  </EmptyDescription>
                </EmptyHeader>
                <EmptyContent v-if="!library.sources.length || library.query || library.hasAnalysisFilters">
                  <div class="flex gap-2">
                    <Button v-if="!library.sources.length" size="sm" @click="library.addSources()">
                      <FolderPlus /> {{ t('list.empty.addFolders') }}
                    </Button>
                    <Button v-if="library.query" variant="outline" size="sm" @click="library.query = ''">
                      {{ t('list.empty.clearSearch') }}
                    </Button>
                    <Button v-if="library.hasAnalysisFilters" variant="outline" size="sm" @click="library.clearAnalysisFilters()">
                      {{ t('list.empty.clearFilters') }}
                    </Button>
                  </div>
                </EmptyContent>
              </Empty>

              <div v-else class="relative w-full" :style="{ height: `${virtualizer.getTotalSize()}px` }">
                <template v-for="{ item, sample: s, stemChild, groupStart, groupEnd, grouped } in rows" :key="item.index">
                  <SampleRow
                    v-if="s" v-show="!pinnedSample || item.index !== playingIndex"
                    :sample="s" :item="item" :stem-child="!!stemChild" :group-start="groupStart"
                    :group-end="!!groupEnd" :grouped="!!grouped" :pinned="false" :visible-columns="visibleColumns"
                    @press="onMouseDown" @select="onClick" @edit="onDoubleClick" @drag="onDragStart"
                    @play="toggleRowPlayback" @stems="library.toggleStems" @favorite="library.toggleFavorite"
                  />
                  <div
                    v-else
                    class="row-placeholder grid items-center px-3 absolute inset-x-0"
                    :style="{ height: `${item.size - 4}px`, transform: `translateY(${item.start + 2}px)` }"
                  >
                    <span />
                    <span class="h-2.5 w-40 rounded bg-muted" />
                  </div>
                </template>
              </div>
            </div>
            <Transition name="pinned-track" :duration="reducedMotion === 'reduce' ? 0 : { enter: 140, leave: 100 }">
            <div v-if="pinnedSample" class="pinned-track absolute inset-x-0 top-0 z-10 overflow-hidden bg-primary" :style="{ height: `${ROW_HEIGHT}px` }">
              <SampleRow :sample="pinnedSample" :item="{ index: playingIndex, start: 0, size: ROW_HEIGHT }"
                :stem-child="false" :group-start="false" :group-end="false" :grouped="false" :pinned="true" :visible-columns="visibleColumns"
                    @press="onMouseDown" @select="onClick" @edit="onDoubleClick" @drag="onDragStart"
                    @play="toggleRowPlayback" @stems="library.toggleStems" @favorite="library.toggleFavorite" />
            </div>
            </Transition>
            </div>
          </ContextMenuTrigger>
          <SampleContextMenu v-if="menuSample" ref="sampleMenu" :native="nativeMenus" :sample="menuSample" @close="menuOpen = false" />
        </ContextMenu>
        <SampleListHints :viewport="listSurface" />
        </Tooltip>
        <div v-if="total > 0" class="column-guides pointer-events-none absolute inset-0 grid px-3" aria-hidden="true">
          <span />
          <span class="column-guide" :class="{ active: resizingColumn === 'name' }" />
          <span v-for="column in visibleColumns" :key="column.id" class="column-guide" :class="{ active: resizingColumn === column.id }" />
          <span />
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.header,
.row-placeholder,
.column-guides {
  grid-template-columns: var(--sample-columns);
  column-gap: 12px;
}
.row-placeholder {
  inset-inline: 6px;
  padding-inline: calc(0.75rem - 6px);
  border-radius: 6px;
  isolation: isolate;
}
.column-guide {
  position: relative;
}
.column-guide::after {
  content: '';
  position: absolute;
  inset-block: 0;
  left: calc(100% + 5px);
  width: 1px;
  background: var(--border);
}
.column-guide.active::after {
  background: var(--primary);
}
.header {
  position: relative;
  z-index: 20;
  flex-shrink: 0;
  background: var(--background);
  /* Overlap the composited scroll edge without moving rows or columns. */
  box-shadow: 0 1px 0 var(--border);
}
.pinned-track {
  transform: translateZ(0);
}
.pinned-track-enter-active .row {
  transition: opacity 140ms cubic-bezier(0.16, 1, 0.3, 1), translate 140ms cubic-bezier(0.16, 1, 0.3, 1);
}
.pinned-track-leave-active {
  pointer-events: none;
}
.pinned-track-leave-active .row {
  transition: opacity 100ms ease-out, translate 100ms ease-out;
}
.pinned-track-enter-from .row,
.pinned-track-leave-to .row {
  opacity: 0;
  translate: 0 -3px;
}
@media (prefers-reduced-motion: reduce) {
  .pinned-track-enter-active .row,
  .pinned-track-leave-active .row {
    transition: none;
  }
  .pinned-track-enter-from .row,
  .pinned-track-leave-to .row {
    opacity: 1;
    translate: none;
  }
}
</style>
