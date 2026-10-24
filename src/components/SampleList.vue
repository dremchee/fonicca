<script setup lang="ts">
import type { Sample } from '@/api'
import { useVirtualizer } from '@tanstack/vue-virtual'
import { AudioLines, FolderPlus, Layers, SearchX, Sparkles, Star, Tag, X } from '@lucide/vue'
import { onKeyStroke, useEventListener } from '@vueuse/core'
import { storeToRefs } from 'pinia'
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { categoryHint, categoryLabel, formatBpm, formatDuration, kindLabel, sourceHint } from '@/format'
import Hint from '@/components/Hint.vue'
import SampleContextMenu from '@/components/SampleContextMenu.vue'
import { Button } from '@/components/ui/button'
import { ContextMenu, ContextMenuTrigger } from '@/components/ui/context-menu'
import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from '@/components/ui/empty'
import { openEditor } from '@/stores/editor'
import { useLibraryStore } from '@/stores/library'
import { usePlayerStore } from '@/stores/player'

const ROW_HEIGHT = 30

const { t } = useI18n()
const library = useLibraryStore()
const player = usePlayerStore()
const { total, focusIndex, selectedIds, scope } = storeToRefs(library)

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
  if (e.metaKey || e.ctrlKey || e.altKey || e.target instanceof HTMLInputElement)
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
  <div class="flex-1 min-h-0 flex flex-col">
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
    <div class="header grid items-center px-3 h-7 border-b border-border text-[11px] uppercase tracking-wide text-muted-foreground">
      <span />
      <span />
      <span>{{ t('list.columns.name') }}</span>
      <span>{{ t('list.columns.tags') }}</span>
      <span>{{ t('list.columns.category') }}</span>
      <span>{{ t('list.columns.kind') }}</span>
      <span class="text-right">BPM</span>
      <span>{{ t('list.columns.key') }}</span>
      <span class="text-right">{{ t('list.columns.length') }}</span>

      <span>{{ t('list.columns.type') }}</span>
    </div>

    <ContextMenu v-model:open="menuOpen">
      <ContextMenuTrigger as-child>
        <div ref="scroller" class="flex-1 overflow-y-auto outline-none" tabindex="-1" @contextmenu="onContextMenu">
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
                  playing: s.id === player.playingId,
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
                  <span v-if="s.id === player.playingId" class="i-lucide-volume-2 text-primary text-xs" />
                </span>
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
                <span class="flex items-center gap-2 min-w-0">
                  <Hint :text="breakablePath(s.path)" side="top">
                    <span class="truncate">{{ s.name }}</span>
                  </Hint>
                  <Hint v-if="s.similarity != null && s.id !== similarTo?.id" :text="t('list.similar.howSimilar')" side="top">
                    <span class="shrink-0 text-[11px] text-primary">{{ Math.round(Math.max(0, s.similarity) * 100) }}%</span>
                  </Hint>
                </span>
                <span class="flex gap-1 overflow-hidden">
                  <span v-for="tag in s.tags" :key="tag.id" class="chip">{{ tag.name }}</span>
                </span>
                <Hint :text="categoryHint(s.categorySource, s.categoryConfidence)" side="top">
                  <span class="truncate text-[11px]" :class="s.categorySource === 'audio' ? 'text-muted-foreground' : ''">{{ s.category ? categoryLabel(s.category) : '' }}</span>
                </Hint>
                <span class="text-muted-foreground text-[11px]">
                  <template v-if="s.kind">{{ kindLabel(s.kind) }}</template>
                  <span v-else-if="!s.analyzed" class="opacity-50">…</span>
                </span>
                <Hint :text="sourceHint(s.bpmSource)" side="top">
                  <span class="text-right" :class="s.bpmSource === 'audio' ? 'text-muted-foreground' : ''">
                    {{ s.bpm != null ? formatBpm(s.bpm) : '' }}
                  </span>
                </Hint>
                <Hint :text="sourceHint(s.keySource)" side="top">
                  <span :class="s.keySource === 'audio' ? 'text-muted-foreground' : ''">{{ s.key ?? '' }}</span>
                </Hint>
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
