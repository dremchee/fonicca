<script setup lang="ts">
import type { BatchLoudness } from '@/composables/batch'
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { formatNumber } from '@/format'

const props = defineProps<{ reports: BatchLoudness[] }>()
const { t } = useI18n()
const limited = computed(() => props.reports.filter(r => r.limited).length)
</script>

<template>
  <details v-if="reports.length" class="mt-2 text-xs" :open="limited > 0">
    <summary class="cursor-pointer text-muted-foreground">
      {{ t('batch.done.loudness', { n: formatNumber(reports.length) }, reports.length) }}
      <template v-if="limited"> · {{ t('batch.done.limitedCount', { n: formatNumber(limited) }, limited) }}</template>
    </summary>
    <ul class="mt-2 max-h-32 overflow-y-auto flex flex-col gap-1.5">
      <li v-for="r in reports" :key="r.path" class="flex flex-wrap items-baseline gap-x-2" :title="r.path">
        <span class="min-w-0 max-w-48 truncate font-medium">{{ r.name }}</span>
        <span class="tabular text-muted-foreground">{{ formatNumber(r.before, { fixed: 1 }) }} → {{ formatNumber(r.after, { fixed: 1 }) }} LUFS</span>
        <span v-if="r.limited" class="text-muted-foreground">{{ t('batch.done.limited', { target: formatNumber(r.target, { fixed: 1 }) }) }}</span>
      </li>
    </ul>
  </details>
</template>
