<script setup lang="ts">
import type { AcceptableValue } from 'reka-ui'
import type { SampleKind } from '@/api'
import { Check, ChevronDown, Download, Pause, Play } from '@lucide/vue'
import { storeToRefs } from 'pinia'
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import Hint from '@/components/Hint.vue'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectLabel,
  SelectSeparator,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { Slider } from '@/components/ui/slider'
import { Spinner } from '@/components/ui/spinner'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { CATEGORIES, categoryLabel, formatNumber, formatSize, kindLabel } from '@/format'
import { useLibraryStore } from '@/stores/library'

const { t } = useI18n()
const library = useLibraryStore()
const { kindFilter, bpmMin, bpmMax, keyFilter, categoryFilter, hasAnalysisFilters, analysis, soundModel, modelDownload } = storeToRefs(library)

const kinds: SampleKind[] = ['oneshot', 'loop', 'long']
const notes = ['C', 'C#', 'D', 'D#', 'E', 'F', 'F#', 'G', 'G#', 'A', 'A#', 'B']

// Reka controls cannot hold null, so "no filter" is a sentinel value.
const ANY = 'any'

const kind = computed<string>({
  get: () => kindFilter.value ?? ANY,
  set: v => (kindFilter.value = v === ANY ? null : v as SampleKind),
})

function onKindChange(v: AcceptableValue | AcceptableValue[]) {
  // A single toggle group deselects on a second click; keep one segment on.
  if (typeof v === 'string' && v)
    kind.value = v
}

const key = computed<string>({
  get: () => keyFilter.value ?? ANY,
  set: v => (keyFilter.value = v === ANY ? null : v),
})

const category = computed<string>({
  get: () => categoryFilter.value ?? ANY,
  set: v => (categoryFilter.value = v === ANY ? null : v),
})

const pending = computed(() => analysis.value.pending + analysis.value.pendingEmbeddings)
const downloadPercent = computed(() => {
  const d = modelDownload.value
  return d && d.total ? Math.round((d.received / d.total) * 100) : 0
})

// BPM range: the slider covers the usual tempos; an end at its limit means "no bound".
const BPM_FLOOR = 40
const BPM_CEIL = 220
const bpmRange = ref<number[]>([])

function syncBpmRange() {
  bpmRange.value = [
    Math.max(BPM_FLOOR, Math.min(bpmMin.value ?? BPM_FLOOR, BPM_CEIL)),
    Math.max(BPM_FLOOR, Math.min(bpmMax.value ?? BPM_CEIL, BPM_CEIL)),
  ]
}
watch([bpmMin, bpmMax], syncBpmRange, { immediate: true })

// Applied on release, not on every step, so dragging does not re-query the library.
function commitBpmRange(v: number[] | undefined) {
  if (!v)
    return
  bpmMin.value = v[0] <= BPM_FLOOR ? null : v[0]
  bpmMax.value = v[1] >= BPM_CEIL ? null : v[1]
}

const bpmLabel = computed(() => {
  const lo = bpmMin.value
  const hi = bpmMax.value
  if (lo != null && hi != null)
    return lo === hi ? `${lo}` : `${lo}–${hi}`
  if (lo != null)
    return `≥ ${lo}`
  if (hi != null)
    return `≤ ${hi}`
  return t('filters.anyBpm')
})

function parseBpm(e: Event): number | null {
  const v = Number.parseFloat((e.target as HTMLInputElement).value)
  return Number.isFinite(v) && v > 0 ? v : null
}
</script>

<template>
  <!-- Wraps onto a second line in narrow windows instead of overflowing. -->
  <div class="min-h-11 shrink-0 px-3 py-1.5 flex flex-wrap items-center gap-x-4 gap-y-1.5 border-b border-border text-sm">
    <ToggleGroup type="single" variant="outline" size="sm" :model-value="kind" @update:model-value="onKindChange">
      <ToggleGroupItem :value="ANY">
        {{ t('filters.all') }}
      </ToggleGroupItem>
      <ToggleGroupItem v-for="k in kinds" :key="k" :value="k">
        {{ kindLabel(k, true) }}
      </ToggleGroupItem>
    </ToggleGroup>

    <div class="flex items-center gap-2 text-muted-foreground">
      BPM
      <Popover>
        <PopoverTrigger as-child>
          <Button
            variant="outline" size="sm" :aria-label="t('filters.bpmRange')"
            class="w-28 justify-between font-normal tabular dark:bg-input/30 dark:hover:bg-input/50"
            :class="bpmMin == null && bpmMax == null ? 'text-muted-foreground' : 'text-foreground'"
          >
            {{ bpmLabel }}
            <ChevronDown class="opacity-50" />
          </Button>
        </PopoverTrigger>
        <PopoverContent align="start" class="w-64 flex flex-col gap-4">
          <div class="flex items-center justify-between text-sm">
            <span class="font-medium">{{ t('filters.bpmRange') }}</span>
            <Button
              v-if="bpmMin != null || bpmMax != null"
              variant="ghost" size="xs" class="-my-1 text-muted-foreground"
              @click="bpmMin = null; bpmMax = null"
            >
              {{ t('filters.anyBpm') }}
            </Button>
          </div>
          <Slider
            v-model="bpmRange" :min="BPM_FLOOR" :max="BPM_CEIL" :step="1" :min-steps-between-thumbs="0"
            :aria-label="t('filters.bpmRange')" @value-commit="commitBpmRange"
          />
          <div class="flex items-center gap-2 text-muted-foreground">
            <Input
              :model-value="bpmMin ?? ''" type="number" min="1" :placeholder="t('filters.min')" :aria-label="t('filters.minBpm')"
              class="num h-8" @change="bpmMin = parseBpm($event)"
            />
            –
            <Input
              :model-value="bpmMax ?? ''" type="number" min="1" :placeholder="t('filters.max')" :aria-label="t('filters.maxBpm')"
              class="num h-8" @change="bpmMax = parseBpm($event)"
            />
          </div>
        </PopoverContent>
      </Popover>
    </div>

    <div class="flex items-center gap-2 text-muted-foreground">
      {{ t('filters.key') }}
      <!-- SelectValue keeps the text it was mounted with: spell labels out so they follow a language switch. -->
      <Select v-model="key">
        <SelectTrigger size="sm" class="w-24 text-foreground" :aria-label="t('filters.key')">
          <SelectValue>{{ key === ANY ? t('filters.anyKey') : key }}</SelectValue>
        </SelectTrigger>
        <SelectContent>
          <SelectItem :value="ANY">
            {{ t('filters.anyKey') }}
          </SelectItem>
          <SelectSeparator />
          <SelectGroup>
            <SelectLabel>{{ t('filters.major') }}</SelectLabel>
            <SelectItem v-for="n in notes" :key="n" :value="n">
              {{ n }}
            </SelectItem>
          </SelectGroup>
          <SelectSeparator />
          <SelectGroup>
            <SelectLabel>{{ t('filters.minor') }}</SelectLabel>
            <SelectItem v-for="n in notes" :key="`${n}m`" :value="`${n}m`">
              {{ n }}m
            </SelectItem>
          </SelectGroup>
        </SelectContent>
      </Select>
    </div>

    <div class="flex items-center gap-2 text-muted-foreground">
      {{ t('filters.category') }}
      <Select v-model="category">
        <SelectTrigger size="sm" class="w-32 text-foreground" :aria-label="t('filters.category')">
          <SelectValue>{{ category === ANY ? t('filters.anyCategory') : categoryLabel(category) }}</SelectValue>
        </SelectTrigger>
        <SelectContent>
          <SelectItem :value="ANY">
            {{ t('filters.anyCategory') }}
          </SelectItem>
          <SelectSeparator />
          <SelectItem v-for="id in CATEGORIES" :key="id" :value="id">
            {{ categoryLabel(id) }}
          </SelectItem>
        </SelectContent>
      </Select>
    </div>

    <Button v-if="hasAnalysisFilters" variant="ghost" size="sm" class="text-muted-foreground" @click="library.clearAnalysisFilters()">
      {{ t('filters.clear') }}
    </Button>

    <div class="ml-auto min-w-0 flex items-center gap-2 text-muted-foreground tabular whitespace-nowrap">
      <template v-if="modelDownload">
        <Hint v-if="modelDownload.error" :text="modelDownload.error">
          <span class="truncate text-destructive">{{ t('filters.downloadFailed') }}</span>
        </Hint>
        <template v-else>
          <Spinner class="text-primary" />
          {{ t('filters.downloadingModel', { percent: downloadPercent }) }}
        </template>
      </template>
      <Hint
        v-else-if="soundModel && !soundModel.installed"
        :text="t('filters.soundModelHint')"
      >
        <Button variant="outline" size="sm" @click="library.downloadSoundModel()">
          <Download /> {{ t('filters.soundModel', { size: formatSize(soundModel.size) }) }}
        </Button>
      </Hint>
      <template v-if="pending > 0">
        <Spinner v-if="!analysis.paused" class="text-primary" />
        <Hint :text="t('filters.analysisHint')">
          <span class="truncate">
            {{ t(analysis.paused ? 'filters.analysisPaused' : 'filters.analyzing', { n: formatNumber(pending) }) }}
          </span>
        </Hint>
        <Hint :text="analysis.paused ? t('filters.resumeAnalysis') : t('filters.pauseAnalysis')">
          <Button
            variant="ghost" size="icon-sm" :aria-label="analysis.paused ? t('filters.resumeAnalysis') : t('filters.pauseAnalysis')"
            @click="library.setAnalysisPaused(!analysis.paused)"
          >
            <component :is="analysis.paused ? Play : Pause" />
          </Button>
        </Hint>
      </template>
      <span v-else-if="!modelDownload" class="flex items-center gap-1.5">
        <Check class="size-4" /> {{ t('filters.analyzed') }}
      </span>
    </div>
  </div>
</template>

<style scoped>
.num::-webkit-inner-spin-button {
  display: none;
}
</style>
