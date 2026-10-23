<script setup lang="ts">
import type { AcceptableValue } from 'reka-ui'
import type { SampleKind } from '@/api'
import { Check, Pause, Play } from '@lucide/vue'
import { storeToRefs } from 'pinia'
import { computed } from 'vue'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
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
import { Spinner } from '@/components/ui/spinner'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { KIND_LABELS } from '@/format'
import { useLibraryStore } from '@/stores/library'

const library = useLibraryStore()
const { kindFilter, bpmMin, bpmMax, keyFilter, hasAnalysisFilters, analysis } = storeToRefs(library)

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
      <Input
        :model-value="bpmMin ?? ''" type="number" min="1" placeholder="min" aria-label="Minimum BPM"
        class="num h-8 w-18" @change="bpmMin = parseBpm($event)"
      />
      –
      <Input
        :model-value="bpmMax ?? ''" type="number" min="1" placeholder="max" aria-label="Maximum BPM"
        class="num h-8 w-18" @change="bpmMax = parseBpm($event)"
      />
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

    <Button v-if="hasAnalysisFilters" variant="ghost" size="sm" class="text-muted-foreground" @click="library.clearAnalysisFilters()">
      Clear
    </Button>

    <div class="ml-auto min-w-0 flex items-center gap-2 text-muted-foreground tabular whitespace-nowrap">
      <template v-if="analysis.pending > 0">
        <Spinner v-if="!analysis.paused" class="text-primary" />
        <span class="truncate" title="BPM, key, loudness and one-shot/loop detection run in the background">
          {{ analysis.paused ? 'Analysis paused' : 'Analyzing' }} · {{ analysis.pending.toLocaleString() }} left
        </span>
        <Button
          variant="ghost" size="icon-sm"
          :title="analysis.paused ? 'Resume analysis' : 'Pause analysis'"
          @click="library.setAnalysisPaused(!analysis.paused)"
        >
          <component :is="analysis.paused ? Play : Pause" />
        </Button>
      </template>
      <span v-else class="flex items-center gap-1.5">
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
