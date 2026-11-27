<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import LibraryBreadcrumbs from '@/components/LibraryBreadcrumbs.vue'
import { formatNumber } from '@/format'
import { useLibraryStore } from '@/stores/library'

const { t } = useI18n()
const library = useLibraryStore()
</script>

<template>
  <header class="library-toolbar h-16 shrink-0 px-3 flex items-center gap-3 border-b border-border" data-tauri-drag-region="deep">
    <div class="library-context min-w-0 flex-1">
      <LibraryBreadcrumbs />
      <div class="text-xs text-muted-foreground tabular truncate" aria-live="polite">
        {{ t('common.count.samples', { n: formatNumber(library.total) }, library.total) }}
        <template v-if="library.selectedIds.size"> · {{ t('list.search.selected', { n: formatNumber(library.selectedIds.size) }) }}</template>
        <template v-if="library.isScanning"> · {{ t('list.search.scanning') }}</template>
      </div>
    </div>
  </header>
</template>

<style scoped>
.library-toolbar {
  padding-left: var(--library-toolbar-inset, 0.75rem);
}
</style>
