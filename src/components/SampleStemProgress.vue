<script setup lang="ts">
import type { ActiveStemProgress } from '@/stem-progress'
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

const props = defineProps<{ progress: ActiveStemProgress, name: string, namespace?: 'stems' | 'apollo' }>()
const { t } = useI18n()
const stage = computed(() => t(`${props.namespace ?? 'stems'}.stages.${props.progress.stage}`))
</script>

<template>
  <span class="sample-progress min-w-0 max-w-[55%] ml-auto flex items-center gap-1.5 text-[11px] font-medium" :title="stage">
    <span class="truncate">{{ stage }}</span>
    <span v-if="!progress.indeterminate" class="shrink-0 tabular">{{ progress.percent }}%</span>
    <span
      role="progressbar" :aria-label="`${name}: ${stage}`"
      :aria-valuenow="progress.indeterminate ? undefined : progress.percent"
      :aria-valuetext="progress.indeterminate ? stage : `${stage} ${progress.percent}%`"
      aria-valuemin="0" aria-valuemax="100"
      class="progress-track pointer-events-none absolute inset-x-0 bottom-0 h-[3px] overflow-hidden"
    >
      <span class="progress-fill block h-full transition-[width] duration-300 motion-reduce:transition-none" :class="{ 'animate-pulse motion-reduce:animate-none': progress.indeterminate }" :style="{ width: `${progress.indeterminate ? 100 : progress.percent}%` }" />
    </span>
  </span>
</template>

<style scoped>
.sample-progress {
  color: var(--sample-progress-color, var(--primary));
}
.progress-track {
  background: color-mix(in srgb, var(--sample-progress-color, var(--primary)) 25%, transparent);
}
.progress-fill {
  background: var(--sample-progress-color, var(--primary));
}
</style>
