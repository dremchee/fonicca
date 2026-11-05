<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import LibraryBreadcrumbs from '@/components/LibraryBreadcrumbs.vue'
import LibraryNowPlaying from '@/components/LibraryNowPlaying.vue'
import { formatNumber } from '@/format'
import { useLibraryStore } from '@/stores/library'
import { usePlayerStore } from '@/stores/player'

const { t } = useI18n()
const library = useLibraryStore()
const player = usePlayerStore()
</script>

<template>
  <header class="library-toolbar h-16 shrink-0 px-3 flex items-center gap-3 border-b border-border" :class="{ 'has-player': player.playingSample }"
    data-tauri-drag-region>
    <div class="library-context min-w-0 flex-1" data-tauri-drag-region>
      <LibraryBreadcrumbs />
      <div class="text-xs text-muted-foreground tabular truncate" aria-live="polite" data-tauri-drag-region>
        {{ t('common.count.samples', { n: formatNumber(library.total) }, library.total) }}
        <template v-if="library.selectedIds.size"> · {{ t('list.search.selected', { n: formatNumber(library.selectedIds.size) }) }}</template>
        <template v-if="library.isScanning"> · {{ t('list.search.scanning') }}</template>
      </div>
    </div>
    <LibraryNowPlaying v-if="player.playingSample" />
  </header>
</template>

<style scoped>
.library-toolbar {
  position: relative;
  container-type: inline-size;
  padding-left: var(--library-toolbar-inset, 0.75rem);
}
.library-toolbar.has-player {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, min(42%, 504px));
}
</style>
