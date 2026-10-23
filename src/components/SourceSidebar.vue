<script setup lang="ts">
import type { Scope } from '@/stores/library'
import { storeToRefs } from 'pinia'
import { nextTick, ref } from 'vue'
import { folderName } from '@/format'
import { useLibraryStore } from '@/stores/library'

const library = useLibraryStore()
const { sources, tags, scope, scans } = storeToRefs(library)

const editingTagId = ref<number | null>(null)
const editName = ref('')
const editInput = ref<HTMLInputElement[]>([])
const tagError = ref<string | null>(null)

function isActive(s: Scope) {
  const cur = scope.value
  return cur.kind === s.kind && ('id' in s ? 'id' in cur && cur.id === s.id : true)
}

function progressLabel(id: number) {
  const s = scans.value.get(id)
  if (!s)
    return null
  return s.total ? `${s.processed}/${s.total}` : 'scanning…'
}

async function startRename(id: number, name: string) {
  editingTagId.value = id
  editName.value = name
  tagError.value = null
  await nextTick()
  editInput.value[0]?.select()
}

async function commitRename() {
  const id = editingTagId.value
  if (id === null)
    return
  const name = editName.value.trim()
  const current = tags.value.find(t => t.id === id)
  editingTagId.value = null
  if (!name || name === current?.name)
    return
  try {
    await library.renameTag(id, name)
  }
  catch (e) {
    tagError.value = String(e)
  }
}

function onRenameKeydown(e: KeyboardEvent) {
  if (e.key === 'Enter')
    commitRename()
  else if (e.key === 'Escape')
    editingTagId.value = null
  e.stopPropagation()
}
</script>

<template>
  <aside class="w-60 shrink-0 flex flex-col border-r border-line bg-panel">
    <!-- Left part of this bar sits under the macOS traffic lights (overlay title bar). -->
    <div class="h-11 px-3 flex items-center justify-end" data-tauri-drag-region>
      <button
        class="size-7 rounded-md grid place-items-center text-muted hover:bg-raised hover:text-fg"
        title="Add folders"
        @click="library.addSources()"
      >
        <span class="i-lucide-folder-plus" />
      </button>
    </div>

    <nav class="flex-1 overflow-y-auto px-2 pb-2 flex flex-col gap-px">
      <button class="row" :class="{ active: isActive({ kind: 'all' }) }" @click="scope = { kind: 'all' }">
        <span class="i-lucide-library shrink-0" />
        <span class="flex-1 truncate">All samples</span>
      </button>
      <button class="row" :class="{ active: isActive({ kind: 'favorites' }) }" @click="scope = { kind: 'favorites' }">
        <span class="i-lucide-star shrink-0" />
        <span class="flex-1 truncate">Favorites</span>
      </button>

      <h3 class="section">
        Folders
      </h3>
      <div
        v-for="s in sources" :key="s.id"
        class="row group" :class="{ active: isActive({ kind: 'source', id: s.id }) }"
        :title="s.path"
        @click="scope = { kind: 'source', id: s.id }"
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
      <p v-if="!sources.length" class="px-2 py-2 text-muted text-xs leading-relaxed">
        Add a folder with samples to start. Files stay where they are — Samplette only indexes them.
      </p>

      <h3 class="section">
        Tags
      </h3>
      <div
        v-for="t in tags" :key="t.id"
        class="row group" :class="{ active: isActive({ kind: 'tag', id: t.id }) }"
        @click="scope = { kind: 'tag', id: t.id }"
        @dblclick="startRename(t.id, t.name)"
      >
        <span class="i-lucide-tag shrink-0" />
        <input
          v-if="editingTagId === t.id"
          ref="editInput"
          v-model="editName"
          class="flex-1 min-w-0 h-5 px-1 -mx-1 rounded bg-bg outline-none border border-accent"
          @click.stop
          @keydown="onRenameKeydown"
          @blur="commitRename"
        >
        <template v-else>
          <span class="flex-1 truncate">{{ t.name }}</span>
          <span class="text-[11px] text-muted tabular group-hover:hidden">{{ t.sampleCount.toLocaleString() }}</span>
          <span class="hidden group-hover:flex gap-0.5">
            <button class="icon-btn" title="Rename" @click.stop="startRename(t.id, t.name)">
              <span class="i-lucide-pencil text-xs" />
            </button>
            <button class="icon-btn hover:!text-danger" title="Delete tag" @click.stop="library.deleteTag(t.id)">
              <span class="i-lucide-x text-xs" />
            </button>
          </span>
        </template>
      </div>
      <p v-if="tagError" class="px-2 text-danger text-xs">
        {{ tagError }}
      </p>
      <p v-if="!tags.length" class="px-2 py-2 text-muted text-xs leading-relaxed">
        Select samples and press T to tag them.
      </p>
    </nav>
  </aside>
</template>

<style scoped>
.section {
  margin: 12px 8px 4px;
  font-size: 11px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  color: var(--muted);
}
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
