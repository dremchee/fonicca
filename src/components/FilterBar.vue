<script setup lang="ts">
import type { SampleKind } from '@/api'
import { storeToRefs } from 'pinia'
import { KIND_LABELS } from '@/format'
import { useLibraryStore } from '@/stores/library'

const library = useLibraryStore()
const { kindFilter, bpmMin, bpmMax, keyFilter, hasAnalysisFilters, analysis } = storeToRefs(library)

const kinds: (SampleKind | null)[] = [null, 'oneshot', 'loop', 'long']
const notes = ['C', 'C#', 'D', 'D#', 'E', 'F', 'F#', 'G', 'G#', 'A', 'A#', 'B']

function parseBpm(e: Event): number | null {
  const v = Number.parseFloat((e.target as HTMLInputElement).value)
  return Number.isFinite(v) && v > 0 ? v : null
}
</script>

<template>
  <div class="h-9 shrink-0 px-3 flex items-center gap-3 border-b border-line text-xs">
    <div class="flex rounded-md bg-raised p-0.5">
      <button
        v-for="k in kinds" :key="k ?? 'all'"
        class="seg" :class="{ on: kindFilter === k }"
        @click="kindFilter = k"
      >
        {{ k ? `${KIND_LABELS[k]}s` : 'All' }}
      </button>
    </div>

    <label class="flex items-center gap-1.5 text-muted">
      BPM
      <input
        :value="bpmMin ?? ''" type="number" min="1" placeholder="min"
        class="num" @change="bpmMin = parseBpm($event)"
      >
      –
      <input
        :value="bpmMax ?? ''" type="number" min="1" placeholder="max"
        class="num" @change="bpmMax = parseBpm($event)"
      >
    </label>

    <label class="flex items-center gap-1.5 text-muted">
      Key
      <select v-model="keyFilter" class="h-6 px-1 rounded bg-raised outline-none text-fg">
        <option :value="null">Any</option>
        <optgroup label="Major">
          <option v-for="n in notes" :key="n" :value="n">{{ n }}</option>
        </optgroup>
        <optgroup label="Minor">
          <option v-for="n in notes" :key="`${n}m`" :value="`${n}m`">{{ n }}m</option>
        </optgroup>
      </select>
    </label>

    <button v-if="hasAnalysisFilters" class="text-muted hover:text-fg" @click="library.clearAnalysisFilters()">
      Clear
    </button>

    <div class="ml-auto flex items-center gap-1.5 text-muted tabular">
      <template v-if="analysis.pending > 0">
        <span v-if="!analysis.paused" class="i-lucide-loader-circle animate-spin text-accent" />
        <span :title="'BPM, key, loudness and one-shot/loop detection run in the background'">
          {{ analysis.paused ? 'Analysis paused' : 'Analyzing' }} · {{ analysis.pending.toLocaleString() }} left
        </span>
        <button
          class="size-6 rounded grid place-items-center hover:bg-raised hover:text-fg"
          :title="analysis.paused ? 'Resume analysis' : 'Pause analysis'"
          @click="library.setAnalysisPaused(!analysis.paused)"
        >
          <span :class="analysis.paused ? 'i-lucide-play' : 'i-lucide-pause'" />
        </button>
      </template>
      <span v-else class="flex items-center gap-1">
        <span class="i-lucide-check" /> Analyzed
      </span>
    </div>
  </div>
</template>

<style scoped>
.seg {
  height: 22px;
  padding: 0 10px;
  border-radius: 5px;
  color: var(--muted);
}
.seg.on {
  background: var(--panel);
  color: var(--fg);
  box-shadow: 0 1px 2px rgb(0 0 0 / 0.15);
}
.num {
  width: 52px;
  height: 24px;
  padding: 0 6px;
  border-radius: 4px;
  background: var(--raised);
  color: var(--fg);
  outline: none;
  border: 1px solid transparent;
}
.num:focus {
  border-color: var(--accent);
}
.num::-webkit-inner-spin-button {
  display: none;
}
</style>
