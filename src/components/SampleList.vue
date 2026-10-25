<script setup lang="ts">
import type { Sample } from '@/api'
import { useVirtualizer } from '@tanstack/vue-virtual'
import { AudioLines, ChevronRight, FolderPlus, Layers, SearchX, Sparkles, Star, Tag, X } from '@lucide/vue'
import { onKeyStroke, useEventListener, useStorage } from '@vueuse/core'
import { storeToRefs } from 'pinia'
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { categoryHint, categoryLabel, formatBpm, formatDuration, kindLabel, sourceHint } from '@/format'
import Hint from '@/components/Hint.vue'
import SampleContextMenu from '@/components/SampleContextMenu.vue'
import SampleStemProgress from '@/components/SampleStemProgress.vue'
import { activeStemProgress } from '@/stem-progress'
import { useStemsStore } from '@/stores/stems'
import SampleColumnsMenu from '@/components/SampleColumnsMenu.vue'
import SampleColumnHeader from '@/components/SampleColumnHeader.vue'
import { useSampleColumnWidths } from '@/composables/sampleColumnWidths'
import { defaultSampleColumns, sampleColumnLimits, SAMPLE_COLUMNS } from '@/sample-columns'
import { Button } from '@/components/ui/button'
import { ContextMenu, ContextMenuTrigger } from '@/components/ui/context-menu'
import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from '@/components/ui/empty'
import { openEditor } from '@/stores/editor'
import { useLibraryStore } from '@/stores/library'
import { usePlayerStore } from '@/stores/player'

const ROW_HEIGHT = 32

const { t } = useI18n()
const library = useLibraryStore()
const player = usePlayerStore()
const stems = useStemsStore()
const stemProgress = computed(() => activeStemProgress(stems.progress))
const { total, focusIndex, selectedIds, scope } = storeToRefs(library)

// Keep the former full/compact preference when upgrading to individual columns.
const legacyDetails = useStorage('fonicca-list-details', false)
const columnVisibility = useStorage('fonicca-list-columns-v1', defaultSampleColumns(legacyDetails.value), undefined, { mergeDefaults: true })
const visibleColumns = computed(() => SAMPLE_COLUMNS.filter(column => columnVisibility.value[column.id]))
const { tableStyle, resizingColumn, preview: previewColumnWidth, commit: commitColumnWidth, cancel: cancelColumnResize, reset: resetColumnWidths } = useSampleColumnWidths(visibleColumns)

const similarTo = computed(() => (scope.value.kind === 'similar' ? scope.value : null))

// Lets a tooltip wrap a long path at its slashes rather than inside a folder name.
function breakablePath(path: string) {
  return path.replaceAll('/', '/\u200B')
}

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
  if (e.key !== 'Escape' || !similarTo.value || player.playingId !== null)
    return
  if (e.target instanceof Element && e.target.closest('[role="menu"], [role="dialog"], [role="alertdialog"], input'))
    return
  library.exitSimilar()
}, { capture: true })

// S: find samples that sound like the focused one (Esc or × to go back).
onKeyStroke(['s', 'ы'], (e) => {
  if (e.metaKey || e.ctrlKey || e.altKey || (e.target instanceof Element && e.target.closest('input, [role="menu"], [role="dialog"]')))
    return
  const s = library.focused
  if (s) {
    e.preventDefault()
    library.findSimilar(s)
  }
})

const scroller = ref<HTMLElement | null>(null)

const virtualizer = useVirtualizer(computed(() => ({
  count: total.value,
  getScrollElement: () => scroller.value,
  estimateSize: () => ROW_HEIGHT,
  overscan: 20,
})))

const items = computed(() => virtualizer.value.getVirtualItems())
const rows = computed(() => items.value.map(item => ({ item, sample: library.sampleAt(item.index) })))

// A scope change may keep the same row count and virtual range while clearing pages.
watch([items, () => library.pages.size], ([list]) => {
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

// Double-click opens the sample in its editor window.
function onDoubleClick(index: number) {
  const s = library.sampleAt(index)
  if (s)
    openEditor(s)
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
    <div class="table-viewport flex-1 min-w-0 min-h-0 overflow-x-auto">
      <div class="table-content relative flex flex-col h-full" :style="total > 0 ? tableStyle : undefined">
        <div v-if="total > 0" class="header grid items-center px-3 h-8 border-b border-border text-[11px] uppercase tracking-wide text-muted-foreground">
          <span />
          <SampleColumnHeader
            :label="t('list.columns.name')" :min-width="sampleColumnLimits('name').min" :max-width="sampleColumnLimits('name').max"
            @resize="previewColumnWidth('name', $event)" @commit="commitColumnWidth('name', $event)"
            @cancel="cancelColumnResize" @reset="resetColumnWidths('name')"
          />
          <SampleColumnHeader
            v-for="column in visibleColumns" :key="column.id"
            :label="column.id === 'bpm' ? 'BPM' : t(`list.columns.${column.id}`)"
            :min-width="sampleColumnLimits(column.id).min" :max-width="sampleColumnLimits(column.id).max"
            :align-right="column.id === 'bpm' || column.id === 'length'"
            @resize="previewColumnWidth(column.id, $event)" @commit="commitColumnWidth(column.id, $event)"
            @cancel="cancelColumnResize" @reset="resetColumnWidths(column.id)"
          />
          <SampleColumnsMenu v-model="columnVisibility" @reset-widths="resetColumnWidths()" />
        </div>

        <ContextMenu v-model:open="menuOpen">
          <ContextMenuTrigger as-child>
            <div ref="scroller" class="flex-1 min-h-0 overflow-y-auto outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring" tabindex="0" :aria-label="t('sidebar.allSamples')" @contextmenu="onContextMenu">
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
                <template v-for="{ item, sample: s } in rows" :key="item.index">
                  <div
                    v-if="s"
                    class="row grid items-center px-3 absolute inset-x-0 tabular"
                    :class="{
                      selected: selectedIds.has(s.id),
                      focused: item.index === focusIndex,
                      playing: player.isPlaying(s.id),
                    }"
                    :style="{ height: `${item.size}px`, transform: `translateY(${item.start}px)` }"
                    :data-index="item.index"
                    draggable="true"
                    @mousedown="onMouseDown($event, item.index, s)"
                    @click="onClick(item.index)"
                    @dblclick="onDoubleClick(item.index)"
                    @dragstart="onDragStart($event, s)"
                  >
                    <span class="grid place-items-center">
                      <span v-if="player.isPlaying(s.id)" class="i-lucide-volume-2 text-primary text-xs" aria-hidden="true" />
                    </span>
                    <span class="flex items-center gap-2 min-w-0" :class="{ 'pl-5': s.stemParentId }">
                      <Hint :text="breakablePath(s.path)" side="top">
                        <span class="truncate">{{ s.stemKind ? t(`stems.instruments.${s.stemKind}`) : s.name }}</span>
                      </Hint>
                      <SampleStemProgress v-if="stemProgress?.sourceId === s.id" :progress="stemProgress" :name="s.name" />
                      <Hint v-if="s.similarity != null && s.id !== similarTo?.id" :text="t('list.similar.howSimilar')" side="top">
                        <span class="shrink-0 text-[11px] text-primary">{{ Math.round(Math.max(0, s.similarity) * 100) }}%</span>
                      </Hint>
                      <button
                        v-if="s.stemCount && !similarTo" type="button"
                        class="stem-toggle ml-auto shrink-0 inline-flex items-center gap-1 rounded px-1.5 py-0.5 text-[10px] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                        :aria-label="t(library.expandedStems.has(s.id) ? 'stems.collapse' : 'stems.expand', { name: s.name })"
                        :aria-expanded="library.expandedStems.has(s.id)"
                        @mousedown.stop @dblclick.stop @click.stop="library.toggleStems(s, item.index)"
                      >
                        {{ t('stems.badge', { n: s.stemCount }) }}
                        <ChevronRight class="size-3 transition-transform" :class="{ 'rotate-90': library.expandedStems.has(s.id) }" />
                      </button>
                      <span v-else-if="s.stemCount" class="stem-toggle ml-auto shrink-0 rounded px-1.5 py-0.5 text-[10px]">{{ t('stems.badge', { n: s.stemCount }) }}</span>
                    </span>
                    <template v-for="column in visibleColumns" :key="column.id">
                      <span v-if="column.id === 'tags'" class="flex gap-1 overflow-hidden">
                        <span v-for="tag in s.tags" :key="tag.id" class="chip">{{ tag.name }}</span>
                      </span>
                      <Hint v-else-if="column.id === 'category'" :text="categoryHint(s.categorySource, s.categoryConfidence)" side="top">
                        <span class="truncate text-xs" :class="s.categorySource === 'audio' ? 'text-muted-foreground' : ''">{{ s.category ? categoryLabel(s.category) : '' }}</span>
                      </Hint>
                      <span v-else-if="column.id === 'kind'" class="text-muted-foreground text-xs">
                        <template v-if="s.kind">{{ kindLabel(s.kind) }}</template>
                        <span v-else-if="!s.analyzed" class="opacity-50">…</span>
                      </span>
                      <Hint v-else-if="column.id === 'bpm'" :text="sourceHint(s.bpmSource)" side="top">
                        <span class="text-right" :class="s.bpmSource === 'audio' ? 'text-muted-foreground' : ''">
                          {{ s.bpm != null ? formatBpm(s.bpm) : '' }}
                        </span>
                      </Hint>
                      <Hint v-else-if="column.id === 'key'" :text="sourceHint(s.keySource)" side="top">
                        <span :class="s.keySource === 'audio' ? 'text-muted-foreground' : ''">{{ s.key ?? '' }}</span>
                      </Hint>
                      <span v-else-if="column.id === 'length'" class="text-right text-muted-foreground">{{ formatDuration(s.duration) }}</span>
                      <span v-else-if="column.id === 'type'" class="text-muted-foreground uppercase text-xs">{{ s.ext }}</span>
                    </template>
                    <Hint :text="s.favorite ? t('list.favorite.remove') : t('list.favorite.add')" shortcut="F">
                      <button
                        class="star grid place-items-center" :class="{ on: s.favorite }"
                        :aria-label="s.favorite ? t('list.favorite.remove') : t('list.favorite.add')"
                        @mousedown.stop
                        @click.stop="library.toggleFavorite(s)"
                      >
                        <span :class="s.favorite ? 'i-lucide-star text-amber-400' : 'i-lucide-star'" class="text-xs" />
                      </button>
                    </Hint>
                  </div>
                  <div
                    v-else
                    class="row grid items-center px-3 absolute inset-x-0"
                    :style="{ height: `${item.size}px`, transform: `translateY(${item.start}px)` }"
                  >
                    <span />
                    <span class="h-2.5 w-40 rounded bg-muted" />
                  </div>
                </template>
              </div>
            </div>
          </ContextMenuTrigger>
          <SampleContextMenu v-if="menuSample" :sample="menuSample" @close="menuOpen = false" />
        </ContextMenu>
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
.row,
.column-guides {
  grid-template-columns: var(--sample-columns);
  column-gap: 12px;
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
  flex-shrink: 0;
}
.row:hover {
  background: var(--accent);
}
.row.selected {
  background: color-mix(in srgb, var(--foreground) 8%, var(--background));
}
.row.focused {
  box-shadow: inset 2px 0 0 var(--muted-foreground);
}
.star {
  justify-self: end;
  width: 28px;
  color: var(--muted-foreground);
  opacity: 0;
}
.star.on,
.row:hover .star,
.row.selected .star,
.star:focus-visible {
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
  box-shadow: inset 3px 0 0 var(--primary);
}
.row.playing > span:nth-child(2) {
  color: var(--primary);
}
.stem-toggle {
  background: var(--primary-soft);
  color: var(--primary);
}
button.stem-toggle:hover {
  filter: brightness(0.95);
}
</style>
