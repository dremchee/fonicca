<script setup lang="ts">
import type { Scope } from '@/stores/library'
import { onClickOutside } from '@vueuse/core'
import { storeToRefs } from 'pinia'
import { nextTick, ref } from 'vue'
import { folderName } from '@/format'
import { useLibraryStore } from '@/stores/library'

const library = useLibraryStore()
const { sources, tags, collections, scope, scans, notice } = storeToRefs(library)

type Editable = 'tag' | 'collection'

const editing = ref<{ kind: Editable, id: number } | null>(null)
const editName = ref('')
const editInput = ref<HTMLInputElement[]>([])
const renameError = ref<string | null>(null)
const creatingCollection = ref(false)
const newCollection = ref('')
const newCollectionInput = ref<HTMLInputElement | null>(null)
const menuOpen = ref(false)
const menuEl = ref<HTMLElement | null>(null)

onClickOutside(menuEl, () => (menuOpen.value = false))

function isActive(s: Scope) {
  const cur = scope.value
  return cur.kind === s.kind && ('id' in s ? 'id' in cur && cur.id === s.id : true)
}

function isEditing(kind: Editable, id: number) {
  return editing.value?.kind === kind && editing.value.id === id
}

function progressLabel(id: number) {
  const s = scans.value.get(id)
  if (!s)
    return null
  return s.total ? `${s.processed}/${s.total}` : 'scanning…'
}

async function startRename(kind: Editable, id: number, name: string) {
  editing.value = { kind, id }
  editName.value = name
  renameError.value = null
  await nextTick()
  editInput.value[0]?.select()
}

async function commitRename() {
  const target = editing.value
  if (!target)
    return
  editing.value = null
  const name = editName.value.trim()
  const list = target.kind === 'tag' ? tags.value : collections.value
  if (!name || name === list.find(x => x.id === target.id)?.name)
    return
  try {
    if (target.kind === 'tag')
      await library.renameTag(target.id, name)
    else
      await library.renameCollection(target.id, name)
  }
  catch (e) {
    renameError.value = String(e)
  }
}

function onRenameKeydown(e: KeyboardEvent) {
  if (e.key === 'Enter')
    commitRename()
  else if (e.key === 'Escape')
    editing.value = null
  e.stopPropagation()
}

async function startCreateCollection() {
  creatingCollection.value = true
  newCollection.value = ''
  renameError.value = null
  await nextTick()
  newCollectionInput.value?.focus()
}

async function commitCreateCollection() {
  const name = newCollection.value.trim()
  creatingCollection.value = false
  if (!name)
    return
  try {
    await library.createCollection(name)
  }
  catch (e) {
    renameError.value = String(e)
  }
}

function onCreateKeydown(e: KeyboardEvent) {
  if (e.key === 'Enter')
    commitCreateCollection()
  else if (e.key === 'Escape')
    creatingCollection.value = false
  e.stopPropagation()
}

function runMenu(action: () => unknown) {
  menuOpen.value = false
  action()
}
</script>

<template>
  <aside class="w-60 shrink-0 flex flex-col border-r border-line bg-panel">
    <!-- Left part of this bar sits under the macOS traffic lights (overlay title bar). -->
    <div class="h-11 px-3 flex items-center justify-end gap-0.5" data-tauri-drag-region>
      <button
        class="size-7 rounded-md grid place-items-center text-muted hover:bg-raised hover:text-fg"
        title="Add folders"
        @click="library.addSources()"
      >
        <span class="i-lucide-folder-plus" />
      </button>
      <div ref="menuEl" class="relative">
        <button
          class="size-7 rounded-md grid place-items-center text-muted hover:bg-raised hover:text-fg"
          title="More"
          @click="menuOpen = !menuOpen"
        >
          <span class="i-lucide-ellipsis" />
        </button>
        <div v-if="menuOpen" class="dropdown">
          <button class="dropdown-item" @click="runMenu(library.exportUserData)">
            <span class="i-lucide-download" />
            Export favorites, tags &amp; collections…
          </button>
          <button class="dropdown-item" @click="runMenu(library.importUserData)">
            <span class="i-lucide-upload" />
            Import…
          </button>
        </div>
      </div>
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

      <h3 class="section flex items-center justify-between">
        Collections
        <button class="icon-btn -my-1" title="New collection" @click="startCreateCollection">
          <span class="i-lucide-plus text-xs" />
        </button>
      </h3>
      <div
        v-for="c in collections" :key="c.id"
        class="row group" :class="{ active: isActive({ kind: 'collection', id: c.id }) }"
        @click="scope = { kind: 'collection', id: c.id }"
        @dblclick="startRename('collection', c.id, c.name)"
      >
        <span class="i-lucide-layers shrink-0" />
        <input
          v-if="isEditing('collection', c.id)"
          ref="editInput"
          v-model="editName"
          class="flex-1 min-w-0 h-5 px-1 -mx-1 rounded bg-bg outline-none border border-accent"
          @click.stop
          @keydown="onRenameKeydown"
          @blur="commitRename"
        >
        <template v-else>
          <span class="flex-1 truncate">{{ c.name }}</span>
          <span class="text-[11px] text-muted tabular group-hover:hidden">{{ c.sampleCount.toLocaleString() }}</span>
          <span class="hidden group-hover:flex gap-0.5">
            <button class="icon-btn" title="Rename" @click.stop="startRename('collection', c.id, c.name)">
              <span class="i-lucide-pencil text-xs" />
            </button>
            <button class="icon-btn hover:!text-danger" title="Delete collection" @click.stop="library.deleteCollection(c.id)">
              <span class="i-lucide-x text-xs" />
            </button>
          </span>
        </template>
      </div>
      <div v-if="creatingCollection" class="row">
        <span class="i-lucide-layers shrink-0" />
        <input
          ref="newCollectionInput"
          v-model="newCollection"
          placeholder="Collection name"
          class="flex-1 min-w-0 h-5 px-1 -mx-1 rounded bg-bg outline-none border border-accent placeholder:text-muted"
          @keydown="onCreateKeydown"
          @blur="commitCreateCollection"
        >
      </div>
      <p v-else-if="!collections.length" class="px-2 py-2 text-muted text-xs leading-relaxed">
        Right-click samples → Add to collection.
      </p>

      <h3 class="section">
        Tags
      </h3>
      <div
        v-for="t in tags" :key="t.id"
        class="row group" :class="{ active: isActive({ kind: 'tag', id: t.id }) }"
        @click="scope = { kind: 'tag', id: t.id }"
        @dblclick="startRename('tag', t.id, t.name)"
      >
        <span class="i-lucide-tag shrink-0" />
        <input
          v-if="isEditing('tag', t.id)"
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
            <button class="icon-btn" title="Rename" @click.stop="startRename('tag', t.id, t.name)">
              <span class="i-lucide-pencil text-xs" />
            </button>
            <button class="icon-btn hover:!text-danger" title="Delete tag" @click.stop="library.deleteTag(t.id)">
              <span class="i-lucide-x text-xs" />
            </button>
          </span>
        </template>
      </div>
      <p v-if="!tags.length" class="px-2 py-2 text-muted text-xs leading-relaxed">
        Select samples and press T to tag them.
      </p>
    </nav>
    <p v-if="renameError" class="px-4 py-2 text-danger text-xs border-t border-line">
      {{ renameError }}
    </p>
    <p
      v-if="notice" class="px-4 py-2 text-xs border-t border-line"
      :class="notice.error ? 'text-danger' : 'text-muted'"
    >
      {{ notice.text }}
    </p>
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
.dropdown {
  position: absolute;
  right: 0;
  top: calc(100% + 4px);
  z-index: 40;
  width: 260px;
  padding: 4px;
  border-radius: 8px;
  border: 1px solid var(--line);
  background: var(--panel);
  box-shadow: 0 8px 28px rgb(0 0 0 / 0.25);
}
.dropdown-item {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  height: 28px;
  padding: 0 8px;
  border-radius: 5px;
  text-align: left;
}
.dropdown-item:hover {
  background: var(--accent);
  color: white;
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
