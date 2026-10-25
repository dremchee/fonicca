<script setup lang="ts">
import type { ComponentPublicInstance } from 'vue'
import { Globe, Search, SlidersHorizontal } from '@lucide/vue'
import { useEventListener } from '@vueuse/core'
import { computed, useTemplateRef } from 'vue'
import { useI18n } from 'vue-i18n'
import Hint from '@/components/Hint.vue'
import { InputGroup, InputGroupAddon, InputGroupInput } from '@/components/ui/input-group'
import { Button } from '@/components/ui/button'
import { folderName, formatNumber } from '@/format'
import { useLibraryStore } from '@/stores/library'

const emit = defineEmits<{ openSearch: [] }>()
const { t } = useI18n()
const library = useLibraryStore()
const searchInput = useTemplateRef<ComponentPublicInstance>('searchInput')
const searchGroup = useTemplateRef<ComponentPublicInstance>('searchGroup')
const searchElement = computed(() => searchInput.value?.$el as HTMLInputElement | undefined)
// Native draggable regions may consume the click without moving DOM focus.
// Blur on pointer-down before that happens, while preserving clicks inside the field.
useEventListener(document, 'pointerdown', (event) => {
  const input = searchElement.value
  const group = searchGroup.value?.$el as HTMLElement | undefined
  if (input === document.activeElement && group && !event.composedPath().includes(group))
    input.blur()
}, { capture: true })

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

function openAdvancedSearch() {
  // The dialog restores the element focused before it opens.
  searchElement.value?.focus({ preventScroll: true })
  emit('openSearch')
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
    <InputGroup ref="searchGroup" class="search-group flex-1 max-w-sm min-w-0">
      <InputGroupAddon class="py-0">
        <Hint :text="t('list.search.entireLibrary')">
          <Button
            variant="ghost" size="icon-sm" :aria-label="t('list.search.entireLibrary')"
            :aria-pressed="library.scope.kind === 'all'"
            @mousedown.prevent @click="searchEverywhere"
          ><component :is="library.scope.kind === 'all' ? Search : Globe" class="size-[14px]" /></Button>
        </Hint>
      </InputGroupAddon>
      <InputGroupInput ref="searchInput" v-model="library.query" type="search" :placeholder="searchLabel" :aria-label="searchLabel" :title="searchLabel" />
      <InputGroupAddon align="inline-end" class="py-0">
        <Hint :text="t('palette.title')" shortcut="⌘K">
          <Button variant="ghost" size="icon-sm" :aria-label="t('palette.title')" aria-haspopup="dialog" @mousedown.prevent @click="openAdvancedSearch"><SlidersHorizontal class="size-[14px]" /></Button>
        </Hint>
      </InputGroupAddon>
    </InputGroup>
  </header>
</template>

<style scoped>
.library-toolbar {
  container-type: inline-size;
}
</style>
