<script setup lang="ts">
import { rowClickSeeks } from '@/row-playback'
import { useWaveformBackgrounds } from '@/composables/waveformBackgrounds'
import { useTagColors } from '@/composables/tagColors'
import type { Sample } from '@/api'
import { useVirtualizer } from '@tanstack/vue-virtual'
import { AudioLines, ChevronRight, FolderPlus, Layers, Pause, Play, SearchX, Sparkles, Star, Tag, Volume2, X } from '@lucide/vue'
import { onKeyStroke, useEventListener } from '@vueuse/core'
import { storeToRefs } from 'pinia'
import { computed, nextTick, ref, watch } from 'vue'
import { isTauri } from '@tauri-apps/api/core'
import { useI18n } from 'vue-i18n'
import { categoryHint, categoryLabel, formatBpm, formatDuration, kindLabel, sourceHint } from '@/format'
import Hint from '@/components/Hint.vue'
import SampleListHints from '@/components/SampleListHints.vue'
import { Tooltip } from '@/components/ui/tooltip'
import CategoryIcon from '@/components/icons/CategoryIcon.vue'
import SampleContextMenu from '@/components/SampleContextMenu.vue'
import SampleStemProgress from '@/components/SampleStemProgress.vue'
import { activeStemProgress } from '@/stem-progress'
import { useStemsStore } from '@/stores/stems'
import { useApolloStore } from '@/stores/apollo'
import { useSampleColumns } from '@/composables/sampleColumns'
import SampleColumnHeader from '@/components/SampleColumnHeader.vue'
import { useSampleColumnWidths } from '@/composables/sampleColumnWidths'
import { relativeSampleDirectory, sampleDirectory, sampleColumnLimits, SAMPLE_COLUMNS } from '@/sample-columns'
import { Button } from '@/components/ui/button'
import { ContextMenu, ContextMenuTrigger } from '@/components/ui/context-menu'
import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from '@/components/ui/empty'
import { openEditor, useEditorStore } from '@/stores/editor'
import { useLibraryStore } from '@/stores/library'
import { usePlayerStore } from '@/stores/player'
import { useSettingsStore } from '@/stores/settings'

const ROW_HEIGHT = 36

const { t } = useI18n()
const { tagStyle } = useTagColors()
const backgrounds = useWaveformBackgrounds()
const library = useLibraryStore()
const player = usePlayerStore()
const editor = useEditorStore()
const settings = useSettingsStore()
const stems = useStemsStore()
const stemProgress = computed(() => activeStemProgress(stems.progress))
const apollo = useApolloStore()
const apolloProgress = computed(() => activeStemProgress(apollo.progress))
const { total, focusIndex, selectedIds, scope } = storeToRefs(library)

// Keep the former full/compact preference when upgrading to individual columns.
const { visibility: columnVisibility } = useSampleColumns()
const visibleColumns = computed(() => SAMPLE_COLUMNS.filter(column => columnVisibility.value[column.id]))
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
            :direction="similarTo ? null : library.sort.by === 'name' ? library.sort.direction : null"
            :sort-disabled="!!similarTo" @sort="library.setSort('name')"
            :label="t('list.columns.name')" :min-width="sampleColumnLimits('name').min" :max-width="sampleColumnLimits('name').max"
            @resize="previewColumnWidth('name', $event)" @commit="commitColumnWidth('name', $event)"
            @cancel="cancelColumnResize" @reset="resetColumnWidths('name')"
          />
          <SampleColumnHeader
            v-for="column in visibleColumns" :key="column.id"
            :direction="similarTo ? null : library.sort.by === column.id ? library.sort.direction : null"
            :sort-disabled="!!similarTo || column.id === 'directory'" @sort="column.id !== 'directory' && library.setSort(column.id)"
            :label="column.id === 'bpm' ? 'BPM' : t(`list.columns.${column.id}`)"
            :min-width="sampleColumnLimits(column.id).min" :max-width="sampleColumnLimits(column.id).max"
            :align-right="column.id === 'bpm' || column.id === 'length'"
            @resize="previewColumnWidth(column.id, $event)" @commit="commitColumnWidth(column.id, $event)"
            @cancel="cancelColumnResize" @reset="resetColumnWidths(column.id)"
          />
          <span aria-hidden="true" />
        </div>

        <Tooltip>
        <ContextMenu v-model:open="menuOpen">
          <ContextMenuTrigger as-child :disabled="nativeMenus">
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
                <template v-for="{ item, sample: s, stemChild, groupStart, groupEnd, grouped } in rows" :key="item.index">
                  <div
                    v-if="s"
                    class="row grid items-center px-3 absolute inset-x-0 tabular"
                    :class="{
                      selected: selectedIds.has(s.id),
                      focused: item.index === focusIndex,
                      playing: player.isPlaying(s.id),
                      'stem-parent': library.expandedStems.has(s.id),
                      'stem-child': stemChild,
                      grouped,
                      'group-start': groupStart,
                      'group-end': groupEnd,
                    }"
                    :style="{ height: `${item.size - 4}px`, transform: `translateY(${item.start + 2}px)` }"
                    :data-index="item.index"
                    draggable="true"
                    @mousedown="onMouseDown($event, item.index, s)"
                    @click="onClick(item.index)"
                    @dblclick="onDoubleClick(item.index)"
                    @dragstart="onDragStart($event, s)"
                  >
                    <span class="track-icon relative grid place-items-center">
                      <Volume2 v-if="player.isPlaying(s.id)" class="track-icon-display track-volume size-3.5 text-primary" aria-hidden="true" />
                      <CategoryIcon v-else :category="s.category" :stem="s.stemKind" class="track-icon-display size-3.5 text-muted-foreground" aria-hidden="true" />
                      <button
                        type="button" class="track-play absolute grid place-items-center rounded-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                        :aria-label="t(player.isPlaying(s.id) ? 'common.actions.pause' : 'common.actions.play')"
                        :aria-pressed="player.isPlaying(s.id)" :draggable="false"
                        @mousedown.stop @dblclick.stop @click.stop="toggleRowPlayback(s)"
                      >
                        <Pause v-if="player.isPlaying(s.id)" class="size-3.5 fill-current" />
                        <Play v-else class="size-3.5 fill-current" />
                      </button>
                    </span>
                    <span class="track-name relative isolate flex items-center gap-2 min-w-0 h-full overflow-hidden">
                      <svg v-if="backgrounds.pathFor(s)" class="row-waveform pointer-events-none absolute inset-0 -z-1 size-full" viewBox="0 0 1000 100" preserveAspectRatio="none" aria-hidden="true">
                        <path :d="backgrounds.pathFor(s)" fill="currentColor" />
                      </svg>
                      <svg v-if="backgrounds.pathFor(s) && player.playingId === s.id && s.duration" class="row-waveform row-waveform-played pointer-events-none absolute inset-0 -z-1 size-full" viewBox="0 0 1000 100" preserveAspectRatio="none" :style="{ clipPath: `inset(0 ${100 - Math.min(100, Math.max(0, player.position / s.duration * 100))}% 0 0)` }" aria-hidden="true">
                        <path :d="backgrounds.pathFor(s)" fill="currentColor" />
                      </svg>
                      <span class="truncate">{{ s.stemKind ? t(`stems.instruments.${s.stemKind}`) : s.name }}</span>
                      <SampleStemProgress v-if="stemProgress?.sourceId === s.id" :progress="stemProgress" :name="s.name" />
                      <SampleStemProgress v-if="apolloProgress?.sourceId === s.id" :progress="apolloProgress" :name="s.name" namespace="apollo" />
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
                      <span v-if="column.id === 'directory'" class="min-w-0 truncate text-muted-foreground text-xs" :title="sampleDirectory(s.path)">{{ relativeSampleDirectory(s.path, library.sources.find(source => source.id === s.sourceId)?.path) }}</span>
                      <span v-else-if="column.id === 'tags'" class="flex gap-1 overflow-hidden">
                        <span v-for="tag in s.tags" :key="tag.id" class="chip tag-color" :style="tagStyle(tag)">{{ tag.name }}</span>
                      </span>
                      <span v-else-if="column.id === 'category'" :data-sample-hint="categoryHint(s.categorySource, s.categoryConfidence)" data-grace-area-trigger class="truncate text-xs" :class="s.categorySource === 'audio' ? 'text-muted-foreground' : ''">{{ s.category ? categoryLabel(s.category) : '' }}</span>
                      <span v-else-if="column.id === 'kind'" class="text-muted-foreground text-xs">
                        <template v-if="s.kind">{{ kindLabel(s.kind) }}</template>
                        <span v-else-if="!s.analyzed" class="opacity-50">…</span>
                      </span>
                      <span v-else-if="column.id === 'bpm'" :data-sample-hint="sourceHint(s.bpmSource)" data-grace-area-trigger class="text-right" :class="s.bpmSource === 'audio' ? 'text-muted-foreground' : ''">
                          {{ s.bpm != null ? formatBpm(s.bpm) : '' }}
                      </span>
                      <span v-else-if="column.id === 'key'" :data-sample-hint="sourceHint(s.keySource)" data-grace-area-trigger :class="s.keySource === 'audio' ? 'text-muted-foreground' : ''">{{ s.key ?? '' }}</span>
                      <span v-else-if="column.id === 'length'" class="text-right text-muted-foreground">{{ formatDuration(s.duration) }}</span>
                      <span v-else-if="column.id === 'type'" class="text-muted-foreground uppercase text-xs">{{ s.ext }}</span>
                    </template>
                      <button
                        class="star grid place-items-center" :class="{ on: s.favorite }"
                        :data-sample-hint="s.favorite ? t('list.favorite.remove') : t('list.favorite.add')" data-grace-area-trigger data-hint-shortcut="F" data-hint-side="bottom"
                        :aria-label="s.favorite ? t('list.favorite.remove') : t('list.favorite.add')"
                        @mousedown.stop
                        @click.stop="library.toggleFavorite(s)"
                      >
                        <span :class="s.favorite ? 'i-lucide-star text-amber-400' : 'i-lucide-star'" class="text-xs" />
                      </button>
                  </div>
                  <div
                    v-else
                    class="row grid items-center px-3 absolute inset-x-0"
                    :style="{ height: `${item.size - 4}px`, transform: `translateY(${item.start + 2}px)` }"
                  >
                    <span />
                    <span class="h-2.5 w-40 rounded bg-muted" />
                  </div>
                </template>
              </div>
            </div>
          </ContextMenuTrigger>
          <SampleContextMenu v-if="menuSample" ref="sampleMenu" :native="nativeMenus" :sample="menuSample" @close="menuOpen = false" />
        </ContextMenu>
        <SampleListHints :viewport="scroller" />
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
.row-waveform { opacity: 0.1; }
.row.playing .row-waveform { opacity: 0.14; }
.row-waveform-played { color: var(--primary); opacity: 0.35; }
.row.playing .row-waveform-played { color: var(--primary-foreground); opacity: 0.45; }
.track-volume :deep(path:first-child) { fill: currentColor; }
.track-play { width: 24px; height: 24px; opacity: 0; }
.row:hover .track-play, .track-play:focus-visible { opacity: 1; }
.row:hover .track-icon-display, .track-icon:focus-within .track-icon-display { opacity: 0; }
@media (hover: none) {
  .track-play { opacity: 1; }
  .track-icon-display { opacity: 0; }
}
.header,
.row,
.column-guides {
  grid-template-columns: var(--sample-columns);
  column-gap: 12px;
}
.row {
  inset-inline: 6px;
  padding-inline: calc(0.75rem - 6px);
  border-radius: 6px;
  isolation: isolate;
}
.row.grouped {
  border-radius: 0;
}
.row.grouped::before {
  content: '';
  position: absolute;
  inset: -2px 0;
  z-index: -1;
  border-radius: inherit;
  background: inherit;
  pointer-events: none;
}
.row.group-start::before {
  top: 0;
}
.row.group-end::before {
  bottom: 0;
}
.row.group-start {
  border-start-start-radius: 6px;
  border-start-end-radius: 6px;
}
.row.group-end {
  border-end-start-radius: 6px;
  border-end-end-radius: 6px;
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
.row.stem-parent,
.row.stem-child {
  background: color-mix(in srgb, var(--primary) 5%, var(--background));
}
.row:hover {
  background: var(--accent);
}
.row.selected {
  background: color-mix(in srgb, var(--foreground) 8%, var(--background));
}
.row.focused:not(.selected):not(.playing) {
  background: var(--accent);
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
  font-size: 11px;
  line-height: 18px;
}
.stem-toggle {
  background: var(--primary);
  color: var(--primary-foreground);
  font-weight: 500;
}
button.stem-toggle:hover {
  filter: brightness(0.95);
}
.row.playing {
  --sample-progress-color: var(--primary-foreground);
  z-index: 1;
  background: var(--primary);
  color: var(--primary-foreground);
}
.row.playing :deep(.text-muted-foreground) {
  color: color-mix(in srgb, var(--primary-foreground) 80%, transparent);
}
.row.playing :deep(.text-primary),
.row.playing :deep(.text-amber-400),
.row.playing .star {
  color: var(--primary-foreground);
}
.row.playing .star {
  opacity: 1;
}
.row.playing .stem-toggle {
  background: var(--primary-foreground);
  color: var(--primary);
}
</style>
