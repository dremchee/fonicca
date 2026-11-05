<script setup lang="ts">
import type { ComponentPublicInstance } from 'vue'
import { Search } from '@lucide/vue'
import { useEventListener } from '@vueuse/core'
import { computed, useTemplateRef } from 'vue'
import { InputGroup, InputGroupAddon, InputGroupInput } from '@/components/ui/input-group'
import { useLibraryScopeLabel } from '@/composables/libraryScopeLabel'
import { useLibraryStore } from '@/stores/library'

const library = useLibraryStore()
const { searchLabel } = useLibraryScopeLabel()
const searchInput = useTemplateRef<ComponentPublicInstance>('searchInput')
const searchGroup = useTemplateRef<ComponentPublicInstance>('searchGroup')
const searchElement = computed(() => searchInput.value?.$el as HTMLInputElement | undefined)
// Native draggable regions may consume the click without moving DOM focus.
useEventListener(document, 'pointerdown', (event) => {
  const input = searchElement.value
  const group = searchGroup.value?.$el as HTMLElement | undefined
  if (input === document.activeElement && group && !event.composedPath().includes(group))
    input.blur()
}, { capture: true })

function focusSearch() {
  searchElement.value?.focus()
  searchElement.value?.select()
}

defineExpose({ searchElement, focusSearch })
</script>

<template>
  <InputGroup ref="searchGroup" class="library-search" @keydown.esc.stop.prevent="searchElement?.blur()">
    <InputGroupAddon><Search class="size-[14px]" aria-hidden="true" /></InputGroupAddon>
    <InputGroupInput ref="searchInput" v-model="library.query" type="search" :placeholder="searchLabel" :aria-label="searchLabel" :title="searchLabel" aria-keyshortcuts="Meta+F Control+F" />
  </InputGroup>
</template>

<style scoped>
.library-search {
  width: clamp(180px, 18vw, 240px);
  min-width: 140px;
}
</style>
