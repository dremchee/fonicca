<script setup lang="ts">
import type { AcceptableValue } from 'reka-ui'
import type { SampleKind } from '@/api'
import { Check, ChevronDown, Download, Pause, Play } from '@lucide/vue'
import { storeToRefs } from 'pinia'
import { computed, ref, watch } from 'vue'
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
import { CATEGORY_LABELS, formatSize, KIND_LABELS } from '@/format'
import { useLibraryStore } from '@/stores/library'

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
  return 'Any'
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
        All
      </ToggleGroupItem>
      <ToggleGroupItem v-for="k in kinds" :key="k" :value="k">
        {{ KIND_LABELS[k] }}s
      </ToggleGroupItem>
    </ToggleGroup>

    <div class="flex items-center gap-2 text-muted-foreground">
      BPM
      <Popover>
        <PopoverTrigger as-child>
          <Button
            variant="outline" size="sm" aria-label="BPM range"
            class="w-28 justify-between font-normal tabular dark:bg-input/30 dark:hover:bg-input/50"
            :class="bpmMin == null && bpmMax == null ? 'text-muted-foreground' : 'text-foreground'"
          >
            {{ bpmLabel }}
            <ChevronDown class="opacity-50" />
          </Button>
        </PopoverTrigger>
        <PopoverContent align="start" class="w-64 flex flex-col gap-4">
          <div class="flex items-center justify-between text-sm">
            <span class="font-medium">BPM range</span>
            <Button
              v-if="bpmMin != null || bpmMax != null"
              variant="ghost" size="xs" class="-my-1 text-muted-foreground"
              @click="bpmMin = null; bpmMax = null"
            >
              Any
            </Button>
          </div>
          <Slider
            v-model="bpmRange" :min="BPM_FLOOR" :max="BPM_CEIL" :step="1" :min-steps-between-thumbs="0"
            aria-label="BPM range" @value-commit="commitBpmRange"
          />
          <div class="flex items-center gap-2 text-muted-foreground">
            <Input
              :model-value="bpmMin ?? ''" type="number" min="1" placeholder="Min" aria-label="Minimum BPM"
              class="num h-8" @change="bpmMin = parseBpm($event)"
            />
            –
            <Input
              :model-value="bpmMax ?? ''" type="number" min="1" placeholder="Max" aria-label="Maximum BPM"
              class="num h-8" @change="bpmMax = parseBpm($event)"
            />
          </div>
        </PopoverContent>
      </Popover>
    </div>

    <div class="flex items-center gap-2 text-muted-foreground">
      Key
      <Select v-model="key">
        <SelectTrigger size="sm" class="w-24 text-foreground" aria-label="Key">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem :value="ANY">
            Any
          </SelectItem>
          <SelectSeparator />
          <SelectGroup>
            <SelectLabel>Major</SelectLabel>
            <SelectItem v-for="n in notes" :key="n" :value="n">
              {{ n }}
            </SelectItem>
          </SelectGroup>
          <SelectSeparator />
          <SelectGroup>
            <SelectLabel>Minor</SelectLabel>
            <SelectItem v-for="n in notes" :key="`${n}m`" :value="`${n}m`">
              {{ n }}m
            </SelectItem>
          </SelectGroup>
        </SelectContent>
      </Select>
    </div>

    <div class="flex items-center gap-2 text-muted-foreground">
      Category
      <Select v-model="category">
        <SelectTrigger size="sm" class="w-32 text-foreground" aria-label="Category">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem :value="ANY">
            Any
          </SelectItem>
          <SelectSeparator />
          <SelectItem v-for="(label, id) in CATEGORY_LABELS" :key="id" :value="id">
            {{ label }}
          </SelectItem>
        </SelectContent>
      </Select>
    </div>

    <Button v-if="hasAnalysisFilters" variant="ghost" size="sm" class="text-muted-foreground" @click="library.clearAnalysisFilters()">
      Clear
    </Button>

    <div class="ml-auto min-w-0 flex items-center gap-2 text-muted-foreground tabular whitespace-nowrap">
      <template v-if="modelDownload">
        <span v-if="modelDownload.error" class="truncate text-destructive" :title="modelDownload.error">
          Download failed
        </span>
        <template v-else>
          <Spinner class="text-primary" />
          Downloading sound model · {{ downloadPercent }}%
        </template>
      </template>
      <Button
        v-else-if="soundModel && !soundModel.installed"
        variant="outline" size="sm"
        title="Classify unnamed samples (kick, bass, pad…) by how they sound and detect keys of tonal loops. Runs offline."
        @click="library.downloadSoundModel()"
      >
        <Download /> Sound model · {{ formatSize(soundModel.size) }}
      </Button>
      <template v-if="pending > 0">
        <Spinner v-if="!analysis.paused" class="text-primary" />
        <span class="truncate" title="Loudness, one-shot/loop, BPM, categories and keys are analyzed in the background">
          {{ analysis.paused ? 'Analysis paused' : 'Analyzing' }} · {{ pending.toLocaleString() }} left
        </span>
        <Button
          variant="ghost" size="icon-sm"
          :title="analysis.paused ? 'Resume analysis' : 'Pause analysis'"
          @click="library.setAnalysisPaused(!analysis.paused)"
        >
          <component :is="analysis.paused ? Play : Pause" />
        </Button>
      </template>
      <span v-else-if="!modelDownload" class="flex items-center gap-1.5">
        <Check class="size-4" /> Analyzed
      </span>
    </div>
  </div>
</template>

<style scoped>
.num::-webkit-inner-spin-button {
  display: none;
}
</style>
