<script setup lang="ts">
import type { ComponentPublicInstance } from 'vue'
import { Globe, PanelsTopLeft, Search } from '@lucide/vue'
import { computed, useTemplateRef } from 'vue'
import { useI18n } from 'vue-i18n'
import CollectionPicker from '@/components/CollectionPicker.vue'
import Hint from '@/components/Hint.vue'
import { Button } from '@/components/ui/button'
import { InputGroup, InputGroupAddon, InputGroupInput } from '@/components/ui/input-group'
import { Kbd } from '@/components/ui/kbd'
import { folderName, formatNumber } from '@/format'
import { useLibraryStore } from '@/stores/library'
import { openProjectWindow } from '@/composables/projectWindow'

const { t } = useI18n()
const library = useLibraryStore()
const searchInput = useTemplateRef<ComponentPublicInstance>('searchInput')
const searchElement = computed(() => searchInput.value?.$el as HTMLInputElement | undefined)
const title = computed(() => {
  const scope = library.scope
  switch (scope.kind) {
    case 'source': return folderName(scope.path ?? library.sources.find(s => s.id === scope.id)?.path ?? '') || t('sidebar.folders')
    case 'collection': return library.collections.find(c => c.id === scope.id)?.name ?? t('sidebar.collections')
    case 'tag': return library.tags.find(tag => tag.id === scope.id)?.name ?? t('sidebar.tags')
    case 'favorites': return t('sidebar.favorites')
    case 'similar': return t('list.similar.soundsLike')
    default: return t('sidebar.allSamples')
  }
})
const searchLabel = computed(() => library.scope.kind === 'all'
  ? t('list.search.entireLibrary')
  : t('list.search.inScope', { name: title.value }))
const path = computed(() => {
  const scope = library.scope
  return scope.kind === 'source' ? scope.path ?? library.sources.find(s => s.id === scope.id)?.path : undefined
})

function searchEverywhere() {
  library.scope = { kind: 'all' }
  searchElement.value?.focus()
}

defineExpose({ searchElement })
</script>

<template>
  <header class="library-toolbar h-16 shrink-0 px-3 flex items-center gap-3 border-b border-border" data-tauri-drag-region>
    <div class="min-w-0 flex-1" data-tauri-drag-region>
      <h1 class="text-sm font-medium truncate" :title="path ?? title" data-tauri-drag-region>{{ title }}</h1>
      <div class="text-xs text-muted-foreground tabular truncate" aria-live="polite" data-tauri-drag-region>
        {{ t('common.count.samples', { n: formatNumber(library.total) }, library.total) }}
        <template v-if="library.selectedIds.size"> · {{ t('list.search.selected', { n: formatNumber(library.selectedIds.size) }) }}</template>
        <template v-if="library.isScanning"> · {{ t('list.search.scanning') }}</template>
      </div>
    </div>
    <InputGroup class="search-group flex-1 max-w-sm min-w-0">
      <InputGroupAddon><Search /></InputGroupAddon>
      <InputGroupInput ref="searchInput" v-model="library.query" type="search" :placeholder="searchLabel" :aria-label="searchLabel" :title="searchLabel" />
      <InputGroupAddon v-if="library.scope.kind !== 'all'" align="inline-end">
        <Hint :text="t('list.search.entireLibrary')">
          <Button variant="ghost" size="icon-xs" :aria-label="t('list.search.entireLibrary')" @click="searchEverywhere">
            <Globe />
          </Button>
        </Hint>
      </InputGroupAddon>
      <InputGroupAddon align="inline-end" class="search-shortcut"><Kbd>⌘F</Kbd></InputGroupAddon>
    </InputGroup>
    <Button variant="outline" size="sm" :title="t('project.openWindow')" @click="openProjectWindow()"><PanelsTopLeft />{{ t('project.title') }}</Button>
    <CollectionPicker />
  </header>
</template>

<style scoped>
.library-toolbar {
  container-type: inline-size;
}
@container (max-width: 650px) {
  .search-shortcut {
    display: none;
  }
}
</style>
