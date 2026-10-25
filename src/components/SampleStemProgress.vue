<script setup lang="ts">
import type { ActiveStemProgress } from '@/stem-progress'
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

const props = defineProps<{ progress: ActiveStemProgress, name: string, namespace?: 'stems' | 'apollo' }>()
const { t } = useI18n()
const stage = computed(() => t(`${props.namespace ?? 'stems'}.stages.${props.progress.stage}`))
</script>

<template>
  <span class="min-w-0 max-w-[55%] ml-auto flex items-center gap-1.5 text-[11px] text-primary" :title="stage">
    <span class="truncate">{{ stage }}</span>
    <span v-if="!progress.indeterminate" class="shrink-0 tabular">{{ progress.percent }}%</span>
    <span
      role="progressbar" :aria-label="`${name}: ${stage}`"
      :aria-valuenow="progress.indeterminate ? undefined : progress.percent"
      :aria-valuetext="progress.indeterminate ? stage : `${stage} ${progress.percent}%`"
      aria-valuemin="0" aria-valuemax="100"
      class="pointer-events-none absolute inset-x-0 bottom-0 h-0.5 overflow-hidden bg-primary/10"
    >
      <span class="block h-full bg-primary transition-[width] duration-300 motion-reduce:transition-none" :class="{ 'animate-pulse motion-reduce:animate-none': progress.indeterminate }" :style="{ width: `${progress.indeterminate ? 100 : progress.percent}%` }" />
    </span>
  </span>
</template>
