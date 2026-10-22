<script setup lang="ts">
import { storeToRefs } from 'pinia'
import { folderName } from '@/format'
import { useLibraryStore } from '@/stores/library'

const library = useLibraryStore()
const { sources, activeSourceId, scans } = storeToRefs(library)

function progressLabel(id: number) {
  const s = scans.value.get(id)
  if (!s)
    return null
  return s.total ? `${s.processed}/${s.total}` : 'scanning…'
}
</script>

<template>
  <aside class="w-60 shrink-0 flex flex-col border-r border-line bg-panel">
    <div class="h-11 px-3 flex items-center justify-between" data-tauri-drag-region>
      <span class="pl-16 text-xs font-semibold uppercase tracking-wide text-muted" data-tauri-drag-region>Library</span>
      <button
        class="size-7 rounded-md grid place-items-center text-muted hover:bg-raised hover:text-fg"
        title="Add folders"
        @click="library.addSources()"
      >
        <span class="i-lucide-folder-plus" />
      </button>
    </div>

    <nav class="flex-1 overflow-y-auto px-2 pb-2 flex flex-col gap-px">
      <button
        class="row" :class="{ active: activeSourceId === null }"
        @click="activeSourceId = null"
      >
        <span class="i-lucide-library shrink-0" />
        <span class="flex-1 truncate">All samples</span>
      </button>

      <div
        v-for="s in sources" :key="s.id"
        class="row group" :class="{ active: activeSourceId === s.id }"
        :title="s.path"
        @click="activeSourceId = s.id"
      >
        <span class="i-lucide-folder shrink-0" />
        <span class="flex-1 truncate">{{ folderName(s.path) }}</span>
        <span v-if="progressLabel(s.id)" class="text-[11px] text-accent tabular">{{ progressLabel(s.id) }}</span>
        <template v-else>
          <span class="text-[11px] text-muted tabular group-hover:hidden">{{ s.sampleCount.toLocaleString() }}</span>
          <span class="hidden group-hover:flex gap-0.5">
            <button class="icon-btn" title="Rescan" @click.stop="library.rescanSource(s.id)">
              <span class="i-lucide-refresh-cw text-xs" />
            </button>
            <button class="icon-btn hover:!text-danger" title="Remove from library" @click.stop="library.removeSource(s.id)">
              <span class="i-lucide-x text-xs" />
            </button>
          </span>
        </template>
      </div>

      <p v-if="!sources.length" class="px-2 py-4 text-muted text-xs leading-relaxed">
        Add a folder with samples to start. Files stay where they are — Samplette only indexes them.
      </p>
    </nav>
  </aside>
</template>

<style scoped>
.row {
  display: flex;
  align-items: center;
  gap: 8px;
  height: 28px;
  padding: 0 8px;
  border-radius: 6px;
  color: var(--fg);
  text-align: left;
}
.row:hover {
  background: var(--raised);
}
.row.active {
  background: var(--accent-soft);
  color: var(--accent);
}
.icon-btn {
  width: 20px;
  height: 20px;
  display: grid;
  place-items: center;
  border-radius: 4px;
  color: var(--muted);
}
.icon-btn:hover {
  color: var(--fg);
  background: var(--line);
}
</style>
