<script setup lang="ts">
import type { TimingSummary } from '@/editor-performance'
import { useI18n } from 'vue-i18n'
import { debugBytes } from '@/composables/debugMonitor'

defineProps<{ timings: TimingSummary[], heapBytes: number | null }>()
const { t } = useI18n()
</script>

<template>
  <section class="performance-metrics">
    <h3 class="text-xs font-medium">{{ t('debug.timings') }}</h3>
    <p class="text-[10px] text-muted-foreground mt-1 mb-2">{{ t('debug.timingsNote') }}</p>
    <table v-if="timings.length" class="timings-table">
      <thead><tr><th>{{ t('debug.operation') }}</th><th>{{ t('debug.calls') }}</th><th>p50</th><th>p95</th></tr></thead>
      <tbody>
        <tr v-for="item in timings" :key="item.name">
          <td :title="item.name">{{ item.name.replace(/^editor[_.]/, '') }}<span v-if="item.errors" class="text-destructive"> ({{ item.errors }}!)</span></td>
          <td>{{ item.calls }}</td><td>{{ item.p50Ms.toFixed(1) }}</td><td>{{ item.p95Ms.toFixed(1) }}</td>
        </tr>
      </tbody>
    </table>
    <p v-else class="text-xs text-muted-foreground">{{ t('debug.noTimings') }}</p>
    <div class="heap-row"><span>{{ t('debug.jsHeap') }}</span><span>{{ debugBytes(heapBytes) }}</span></div>
    <p class="text-[10px] text-muted-foreground mt-1">{{ t('debug.heapNote') }}</p>
  </section>
</template>

<style scoped>
.performance-metrics { border-top: 1px solid var(--border); margin-top: 12px; padding-top: 12px; }
.timings-table { width: 100%; table-layout: fixed; font-size: 10px; font-variant-numeric: tabular-nums; }
.timings-table th, .timings-table td { text-align: right; padding: 3px 0 3px 6px; }
.timings-table th { color: var(--muted-foreground); font-weight: normal; }
.timings-table th:first-child, .timings-table td:first-child { width: 48%; text-align: left; padding-left: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.heap-row { display: flex; justify-content: space-between; margin-top: 10px; font-size: 11px; color: var(--muted-foreground); font-variant-numeric: tabular-nums; }
</style>
