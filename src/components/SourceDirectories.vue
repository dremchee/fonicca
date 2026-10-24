<script setup lang="ts">
import type { SourceFolder } from '@/api'
import { ChevronRight, Folder, FolderOpen } from '@lucide/vue'
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import Hint from '@/components/Hint.vue'
import { isDirectoryWithin } from '@/library-session'
import { folderName, formatNumber } from '@/format'
import { useLibraryStore } from '@/stores/library'

const props = defineProps<{ sourceId: number, rootPath: string, folders: SourceFolder[] }>()
const library = useLibraryStore()
const { t } = useI18n()
const expanded = ref(new Set<string>())

watch([() => library.scope, () => props.folders], ([scope]) => {
  if (scope.kind !== 'source' || scope.id !== props.sourceId || !scope.path)
    return
  for (const folder of props.folders) {
    if (folder.path !== scope.path && isDirectoryWithin(scope.path, folder.path))
      expanded.value.add(folder.path)
  }
}, { immediate: true })

const children = computed(() => {
  const map = new Map<string, SourceFolder[]>()
  for (const folder of props.folders) {
    const parent = folder.path.slice(0, Math.max(folder.path.lastIndexOf('/'), folder.path.lastIndexOf('\\')))
    const siblings = map.get(parent) ?? []
    siblings.push(folder)
    map.set(parent, siblings)
  }
  for (const siblings of map.values())
    siblings.sort((a, b) => folderName(a.path).localeCompare(folderName(b.path), undefined, { numeric: true, sensitivity: 'base' }))
  return map
})

const rows = computed(() => {
  const result: (SourceFolder & { depth: number, hasChildren: boolean })[] = []
  function visit(parent: string, depth: number) {
    for (const folder of children.value.get(parent) ?? []) {
      result.push({ ...folder, depth, hasChildren: children.value.has(folder.path) })
      if (expanded.value.has(folder.path))
        visit(folder.path, depth + 1)
    }
  }
  visit(props.rootPath.replace(/[/\\]+$/, ''), 1)
  return result
})

function toggle(path: string) {
  if (expanded.value.has(path))
    expanded.value.delete(path)
  else
    expanded.value.add(path)
}

function isActive(path: string) {
  const scope = library.scope
  return scope.kind === 'source' && scope.id === props.sourceId && scope.path === path
}
</script>

<template>
  <div
    v-for="folder in rows" :key="folder.path"
    class="flex h-7 shrink-0 items-center gap-1 rounded-md pr-2"
    :class="isActive(folder.path) ? 'bg-primary-soft text-primary' : 'text-foreground hover:bg-accent'"
    :style="{ paddingLeft: `${8 + Math.min(folder.depth, 8) * 14}px` }"
    :data-menu="`source:${sourceId}`" :data-directory-path="folder.path"
  >
    <button
      v-if="folder.hasChildren" class="grid size-4 shrink-0 place-items-center rounded-sm text-muted-foreground hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
      :aria-expanded="expanded.has(folder.path)"
      :aria-label="t(expanded.has(folder.path) ? 'sidebar.collapseFolder' : 'sidebar.expandFolder', { name: folderName(folder.path) })"
      @click="toggle(folder.path)"
    >
      <ChevronRight class="size-3 transition-transform" :class="{ 'rotate-90': expanded.has(folder.path) }" />
    </button>
    <span v-else class="size-4 shrink-0" />
    <Hint :text="folder.path" side="right">
      <button
        class="flex h-full min-w-0 flex-1 items-center gap-2 rounded-sm text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
        :aria-current="isActive(folder.path) ? 'page' : undefined"
        @click="library.scope = { kind: 'source', id: sourceId, path: folder.path }"
        @keydown.right.prevent="folder.hasChildren && expanded.add(folder.path)"
        @keydown.left.prevent="expanded.delete(folder.path)"
      >
        <component :is="expanded.has(folder.path) ? FolderOpen : Folder" class="size-3.5 shrink-0" />
        <span class="min-w-0 flex-1 truncate">{{ folderName(folder.path) }}</span>
        <span class="text-[11px] text-muted-foreground tabular-nums">{{ formatNumber(folder.sampleCount) }}</span>
      </button>
    </Hint>
  </div>
</template>
