<script setup lang="ts">
import type { Sample } from '@/api'
import type { SAMPLE_COLUMNS } from '@/sample-columns'
import { ChevronRight, Pause, Play, Star, Volume2 } from '@lucide/vue'
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import Hint from '@/components/Hint.vue'
import CategoryIcon from '@/components/icons/CategoryIcon.vue'
import SampleStemProgress from '@/components/SampleStemProgress.vue'
import SampleTagsCell from '@/components/SampleTagsCell.vue'
import { useWaveformBackgrounds } from '@/composables/waveformBackgrounds'
import { categoryHint, categoryLabel, formatBpm, formatDuration, kindLabel, sourceHint } from '@/format'
import { relativeSampleDirectory, sampleDirectory } from '@/sample-columns'
import { activeStemProgress } from '@/stem-progress'
import { useApolloStore } from '@/stores/apollo'
import { useLibraryStore } from '@/stores/library'
import { usePlayerStore } from '@/stores/player'
import { useStemsStore } from '@/stores/stems'

const props = withDefaults(defineProps<{
  sample: Sample
  item: { index: number, start: number, size: number }
  visibleColumns: readonly typeof SAMPLE_COLUMNS[number][]
  stemChild?: boolean
  groupStart?: boolean
  groupEnd?: boolean
  grouped?: boolean
  pinned?: boolean
}>(), { stemChild: false, groupStart: false, groupEnd: false, grouped: false, pinned: false })
const emit = defineEmits<{
  press: [event: MouseEvent, index: number, sample: Sample]
  select: [index: number]
  edit: [index: number]
  drag: [event: DragEvent, sample: Sample]
  play: [sample: Sample]
  stems: [sample: Sample, index: number]
  favorite: [sample: Sample]
}>()

// Stores are read here so playback progress belongs to this row's render
// effect. The list owns all interaction side effects through emitted events.
const library = useLibraryStore()
const player = usePlayerStore()
const stems = useStemsStore()
const apollo = useApolloStore()
const backgrounds = useWaveformBackgrounds()
const { t } = useI18n()
const s = computed(() => props.sample)
const playing = computed(() => player.isPlaying(s.value.id))
const paused = computed(() => player.playingId === s.value.id && player.paused)
const waveformPath = computed(() => backgrounds.pathFor(s.value))
const similarTo = computed(() => library.scope.kind === 'similar' ? library.scope : null)
const stemProgress = computed(() => stems.progress.sourceId === s.value.id ? activeStemProgress(stems.progress) : null)
const apolloProgress = computed(() => apollo.progress.sourceId === s.value.id ? activeStemProgress(apollo.progress) : null)
</script>

<template>
  <div
    class="row grid items-center px-3 absolute inset-x-0 tabular"
    :class="{
      selected: library.selectedIds.has(s.id),
      focused: item.index === library.focusIndex,
      playing: playing,
      paused: paused,
      pinned,
      'stem-parent': library.expandedStems.has(s.id),
      'stem-child': stemChild,
      grouped,
      'group-start': groupStart,
      'group-end': groupEnd,
    }"
    :style="{ height: `${pinned ? item.size : item.size - 4}px`, top: `${pinned ? 0 : item.start + 2}px` }"
    :data-index="item.index"
    :data-pinned="pinned || undefined"
    draggable="true"
    @mousedown="emit('press', $event, item.index, s)"
    @click="emit('select', item.index)"
    @dblclick="emit('edit', item.index)"
    @dragstart="emit('drag', $event, s)"
  >
    <span class="track-icon relative grid place-items-center">
      <Volume2 v-if="playing" class="track-icon-display track-volume size-3.5 text-primary" aria-hidden="true" />
      <CategoryIcon v-else :category="s.category" :stem="s.stemKind" class="track-icon-display size-3.5 text-muted-foreground" aria-hidden="true" />
      <button
        type="button" class="track-play absolute grid place-items-center rounded-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
        :aria-label="t(playing ? 'common.actions.pause' : 'common.actions.play')"
        :aria-pressed="playing" :draggable="false"
        @mousedown.stop @dblclick.stop @click.stop="emit('play', s)"
      >
        <Pause v-if="playing" class="size-3.5 fill-current" />
        <Play v-else class="size-3.5 fill-current" />
      </button>
    </span>
    <span class="track-name relative isolate flex items-center gap-2 min-w-0 h-full overflow-hidden">
      <svg v-if="waveformPath" class="row-waveform pointer-events-none absolute inset-0 -z-1 size-full" viewBox="0 0 1000 100" preserveAspectRatio="none" aria-hidden="true">
        <path :d="waveformPath" fill="currentColor" />
      </svg>
      <svg v-if="waveformPath && player.playingId === s.id && s.duration" class="row-waveform row-waveform-played pointer-events-none absolute inset-0 -z-1 size-full" viewBox="0 0 1000 100" preserveAspectRatio="none" :style="{ clipPath: `inset(0 ${100 - Math.min(100, Math.max(0, player.position / s.duration * 100))}% 0 0)` }" aria-hidden="true">
        <path :d="waveformPath" fill="currentColor" />
      </svg>
      <span class="track-title truncate">{{ s.stemKind ? t(`stems.instruments.${s.stemKind}`) : s.name }}</span>
      <SampleStemProgress v-if="stemProgress" :progress="stemProgress" :name="s.name" />
      <SampleStemProgress v-if="apolloProgress" :progress="apolloProgress" :name="s.name" namespace="apollo" />
      <Hint v-if="s.similarity != null && s.id !== similarTo?.id" :text="t('list.similar.howSimilar')" side="top">
        <span class="shrink-0 text-[11px] text-primary">{{ Math.round(Math.max(0, s.similarity) * 100) }}%</span>
      </Hint>
      <button
        v-if="s.stemCount && !similarTo && !pinned" type="button"
        class="stem-toggle ml-auto shrink-0 inline-flex items-center gap-1 rounded px-1.5 py-0.5 text-[10px] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
        :aria-label="t(library.expandedStems.has(s.id) ? 'stems.collapse' : 'stems.expand', { name: s.name })"
        :aria-expanded="library.expandedStems.has(s.id)"
        @mousedown.stop @dblclick.stop @click.stop="emit('stems', s, item.index)"
      >
        {{ t('stems.badge', { n: s.stemCount }) }}
        <ChevronRight class="size-3 transition-transform" :class="{ 'rotate-90': library.expandedStems.has(s.id) }" />
      </button>
      <span v-else-if="s.stemCount && !pinned" class="stem-toggle ml-auto shrink-0 rounded px-1.5 py-0.5 text-[10px]">{{ t('stems.badge', { n: s.stemCount }) }}</span>
    </span>
    <template v-for="column in visibleColumns" :key="column.id">
      <span v-if="column.id === 'directory'" class="min-w-0 truncate text-muted-foreground text-xs" :title="sampleDirectory(s.path)">{{ relativeSampleDirectory(s.path, library.sources.find(source => source.id === s.sourceId)?.path) }}</span>
      <SampleTagsCell v-else-if="column.id === 'tags'" :sample="s" />
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
        @click.stop="emit('favorite', s)"
      >
        <Star class="size-3" :class="{ 'fill-current text-amber-400': s.favorite }" aria-hidden="true" />
      </button>
  </div>
</template>

<style scoped>
.track-title { transition: opacity 140ms ease-out; }
@media (hover: hover) {
  .track-title:hover { opacity: 0.25; }
}
@media (prefers-reduced-motion: reduce) {
  .track-title { transition: none; }
}
.row-waveform { opacity: 0.1; }
.row:is(.playing, .paused) .row-waveform { opacity: 0.14; }
.row-waveform-played { color: var(--primary); opacity: 0.35; }
.row:is(.playing, .paused) .row-waveform-played { color: var(--primary-foreground); opacity: 0.45; }
.track-volume :deep(path:first-child) { fill: currentColor; }
.track-play { width: 24px; height: 24px; opacity: 0; }
.row:hover .track-play, .track-play:focus-visible { opacity: 1; }
.row:hover .track-icon-display, .track-icon:focus-within .track-icon-display { opacity: 0; }
.row :deep(.tag-add) { opacity: 0; pointer-events: none; }
.row:hover :deep(.tag-add),
.row :deep(.tag-add:focus-visible),
.row :deep(.tag-add[data-state="open"]) { opacity: 1; pointer-events: auto; }
@media (hover: none) {
  .track-play { opacity: 1; }
  .track-icon-display { opacity: 0; }
  .row :deep(.tag-add) { opacity: 1; pointer-events: auto; }
}
.row {
  --sample-row-surface: var(--background);
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
.row.stem-parent,
.row.stem-child {
  --sample-row-surface: color-mix(in srgb, var(--primary) 5%, var(--background));
  background: var(--sample-row-surface);
}
.row:hover {
  --sample-row-surface: var(--accent);
  background: var(--sample-row-surface);
}
.row.selected {
  --sample-row-surface: color-mix(in srgb, var(--foreground) 8%, var(--background));
  background: var(--sample-row-surface);
}
.row.focused:not(.selected):not(.playing):not(.paused) {
  --sample-row-surface: var(--accent);
  background: var(--sample-row-surface);
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
.stem-toggle {
  background: var(--primary);
  color: var(--primary-foreground);
  font-weight: 500;
}
button.stem-toggle:hover {
  filter: brightness(0.95);
}
.row.pinned {
  inset-inline: 0;
  padding-inline: 0.75rem;
  border-radius: 0;
  z-index: 3;
}
.row.paused .track-play { opacity: 1; }
.row.paused .track-icon-display { opacity: 0; }
.row:is(.playing, .paused) {
  --sample-progress-color: var(--primary-foreground);
  --sample-row-surface: var(--primary);
  z-index: 1;
  background: var(--sample-row-surface);
  color: var(--primary-foreground);
}
.row:is(.playing, .paused) :deep(.text-muted-foreground) {
  color: color-mix(in srgb, var(--primary-foreground) 80%, transparent);
}
.row:is(.playing, .paused) :deep(.text-primary),
.row:is(.playing, .paused) :deep(.text-amber-400),
.row:is(.playing, .paused) .star {
  color: var(--primary-foreground);
}
.row:is(.playing, .paused) .star {
  opacity: 1;
}
.row:is(.playing, .paused) .stem-toggle {
  background: var(--primary-foreground);
  color: var(--primary);
}
.row:is(.playing, .paused).pinned { z-index: 3; }
</style>
