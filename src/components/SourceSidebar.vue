<script setup lang="ts">
import type { Scope } from '@/stores/library'
import { FolderOpen, FolderPlus, Pencil, Plus, RefreshCw, Settings, Trash2, X } from '@lucide/vue'
import { revealItemInDir } from '@tauri-apps/plugin-opener'
import { storeToRefs } from 'pinia'
import { computed, nextTick, ref } from 'vue'
import { toast } from 'vue-sonner'
import Hint from '@/components/Hint.vue'
import { Button } from '@/components/ui/button'
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from '@/components/ui/context-menu'
import { confirm } from '@/composables/confirm'
import { folderName } from '@/format'
import { useLibraryStore } from '@/stores/library'
import { useSettingsStore } from '@/stores/settings'

const library = useLibraryStore()
const settings = useSettingsStore()
const { sources, tags, collections, scope, scans } = storeToRefs(library)

type Editable = 'tag' | 'collection'

const editing = ref<{ kind: Editable, id: number } | null>(null)
const editName = ref('')
const editInput = ref<HTMLInputElement[]>([])
const creatingCollection = ref(false)
const newCollection = ref('')
const newCollectionInput = ref<HTMLInputElement | null>(null)

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
    toast.error(String(e))
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
    toast.error(String(e))
  }
}

function onCreateKeydown(e: KeyboardEvent) {
  if (e.key === 'Enter')
    commitCreateCollection()
  else if (e.key === 'Escape')
    creatingCollection.value = false
  e.stopPropagation()
}

// Right-click on a folder, collection or tag row opens one shared menu for it.
const menuTarget = ref<{ kind: 'source' | Editable, id: number } | null>(null)
const menuSource = computed(() => {
  const t = menuTarget.value
  return t?.kind === 'source' ? sources.value.find(s => s.id === t.id) : undefined
})
const menuItem = computed(() => {
  const t = menuTarget.value
  if (t?.kind === 'tag')
    return tags.value.find(x => x.id === t.id)
  if (t?.kind === 'collection')
    return collections.value.find(x => x.id === t.id)
  return undefined
})

function onContextMenu(e: MouseEvent) {
  const row = (e.target as HTMLElement).closest<HTMLElement>('[data-menu]')
  const [kind, id] = row?.dataset.menu?.split(':') ?? []
  if (!kind || editing.value) {
    e.preventDefault()
    return
  }
  menuTarget.value = { kind: kind as 'source' | Editable, id: Number(id) }
}

// Renaming focuses an input, so it waits until the menu has given focus back.
let afterClose: (() => unknown) | null = null

function onCloseAutoFocus(e: Event) {
  if (!afterClose)
    return
  e.preventDefault()
  // The focus trap is released in a timeout of its own; run after it.
  setTimeout(afterClose, 0)
  afterClose = null
}

function renameFromMenu() {
  const t = menuTarget.value
  const item = menuItem.value
  if (t && t.kind !== 'source' && item)
    afterClose = () => startRename(t.kind as Editable, t.id, item.name)
}

function deleteFromMenu() {
  const t = menuTarget.value
  if (t?.kind === 'tag')
    afterClose = () => confirmDeleteTag(t.id)
  else if (t?.kind === 'collection')
    afterClose = () => confirmDeleteCollection(t.id)
}

function removeSourceFromMenu() {
  const t = menuTarget.value
  if (t?.kind === 'source')
    afterClose = () => confirmRemoveSource(t.id)
}

function samplesLabel(n: number) {
  return `${n.toLocaleString()} ${n === 1 ? 'sample' : 'samples'}`
}

async function confirmRemoveSource(id: number) {
  const s = sources.value.find(x => x.id === id)
  if (!s)
    return
  const ok = await confirm({
    title: `Remove “${folderName(s.path)}” from the library?`,
    description: `${samplesLabel(s.sampleCount)} will disappear from Samplette. Files on disk are not touched.`,
    action: 'Remove',
    destructive: true,
  })
  if (ok)
    await library.removeSource(id)
}

async function confirmDeleteCollection(id: number) {
  const c = collections.value.find(x => x.id === id)
  if (!c)
    return
  const ok = await confirm({
    title: `Delete collection “${c.name}”?`,
    description: `The ${samplesLabel(c.sampleCount)} in it stay in the library.`,
    action: 'Delete',
    destructive: true,
  })
  if (ok)
    await library.deleteCollection(id)
}

async function confirmDeleteTag(id: number) {
  const t = tags.value.find(x => x.id === id)
  if (!t)
    return
  const ok = await confirm({
    title: `Delete tag “${t.name}”?`,
    description: `It will be removed from ${samplesLabel(t.sampleCount)}.`,
    action: 'Delete',
    destructive: true,
  })
  if (ok)
    await library.deleteTag(id)
}
</script>

<template>
  <aside class="h-full flex flex-col bg-card">
    <!-- Left part of this bar sits under the macOS traffic lights (overlay title bar). -->
    <div class="h-11 px-3 flex items-center justify-end gap-0.5" data-tauri-drag-region>
      <Hint text="Add folders">
        <Button variant="ghost" size="icon-sm" class="text-muted-foreground" aria-label="Add folders" @click="library.addSources()">
          <FolderPlus />
        </Button>
      </Hint>
      <Hint text="Settings" shortcut="⌘,">
        <Button variant="ghost" size="icon-sm" class="text-muted-foreground" aria-label="Settings" @click="settings.openDialog()">
          <Settings />
        </Button>
      </Hint>
    </div>

    <ContextMenu>
      <ContextMenuTrigger as-child>
        <nav class="flex-1 overflow-y-auto px-2 pb-2 flex flex-col gap-px" @contextmenu="onContextMenu">
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
            :data-menu="`source:${s.id}`"
            @click="scope = { kind: 'source', id: s.id }"
          >
            <span class="i-lucide-folder shrink-0" />
            <Hint :text="s.path" side="right">
              <span class="flex-1 truncate">{{ folderName(s.path) }}</span>
            </Hint>
            <span v-if="progressLabel(s.id)" class="text-[11px] text-primary tabular">{{ progressLabel(s.id) }}</span>
            <template v-else>
              <span class="text-[11px] text-muted-foreground tabular group-hover:hidden">{{ s.sampleCount.toLocaleString() }}</span>
              <span class="hidden group-hover:flex gap-0.5">
                <Hint text="Rescan">
                  <Button variant="ghost" size="icon-xs" class="text-muted-foreground" aria-label="Rescan" @click.stop="library.rescanSource(s.id)">
                    <RefreshCw />
                  </Button>
                </Hint>
                <Hint text="Remove from library">
                  <Button variant="ghost" size="icon-xs" class="text-muted-foreground hover:text-destructive" aria-label="Remove from library" @click.stop="confirmRemoveSource(s.id)">
                    <X />
                  </Button>
                </Hint>
              </span>
            </template>
          </div>
          <p v-if="!sources.length" class="px-2 py-2 text-muted-foreground text-xs leading-relaxed">
            Add a folder with samples to start. Files stay where they are — Samplette only indexes them.
          </p>

          <h3 class="section flex items-center justify-between">
            Collections
            <Hint text="New collection">
              <Button variant="ghost" size="icon-xs" class="text-muted-foreground -my-1" aria-label="New collection" @click="startCreateCollection">
                <Plus />
              </Button>
            </Hint>
          </h3>
          <div
            v-for="c in collections" :key="c.id"
            class="row group" :class="{ active: isActive({ kind: 'collection', id: c.id }) }"
            :data-menu="`collection:${c.id}`"
            @click="scope = { kind: 'collection', id: c.id }"
            @dblclick="startRename('collection', c.id, c.name)"
          >
            <span class="i-lucide-layers shrink-0" />
            <input
              v-if="isEditing('collection', c.id)"
              ref="editInput"
              v-model="editName"
              class="flex-1 min-w-0 h-5 px-1 -mx-1 rounded bg-background outline-none border border-primary"
              @click.stop
              @keydown="onRenameKeydown"
              @blur="commitRename"
            >
            <template v-else>
              <span class="flex-1 truncate">{{ c.name }}</span>
              <span class="text-[11px] text-muted-foreground tabular group-hover:hidden">{{ c.sampleCount.toLocaleString() }}</span>
              <span class="hidden group-hover:flex gap-0.5">
                <Hint text="Rename">
                  <Button variant="ghost" size="icon-xs" class="text-muted-foreground" aria-label="Rename" @click.stop="startRename('collection', c.id, c.name)">
                    <Pencil />
                  </Button>
                </Hint>
                <Hint text="Delete collection">
                  <Button variant="ghost" size="icon-xs" class="text-muted-foreground hover:text-destructive" aria-label="Delete collection" @click.stop="confirmDeleteCollection(c.id)">
                    <X />
                  </Button>
                </Hint>
              </span>
            </template>
          </div>
          <div v-if="creatingCollection" class="row">
            <span class="i-lucide-layers shrink-0" />
            <input
              ref="newCollectionInput"
              v-model="newCollection"
              placeholder="Collection name"
              class="flex-1 min-w-0 h-5 px-1 -mx-1 rounded bg-background outline-none border border-primary placeholder:text-muted-foreground"
              @keydown="onCreateKeydown"
              @blur="commitCreateCollection"
            >
          </div>
          <p v-else-if="!collections.length" class="px-2 py-2 text-muted-foreground text-xs leading-relaxed">
            Right-click samples → Add to collection.
          </p>

          <h3 class="section">
            Tags
          </h3>
          <div
            v-for="t in tags" :key="t.id"
            class="row group" :class="{ active: isActive({ kind: 'tag', id: t.id }) }"
            :data-menu="`tag:${t.id}`"
            @click="scope = { kind: 'tag', id: t.id }"
            @dblclick="startRename('tag', t.id, t.name)"
          >
            <span class="i-lucide-tag shrink-0" />
            <input
              v-if="isEditing('tag', t.id)"
              ref="editInput"
              v-model="editName"
              class="flex-1 min-w-0 h-5 px-1 -mx-1 rounded bg-background outline-none border border-primary"
              @click.stop
              @keydown="onRenameKeydown"
              @blur="commitRename"
            >
            <template v-else>
              <span class="flex-1 truncate">{{ t.name }}</span>
              <span class="text-[11px] text-muted-foreground tabular group-hover:hidden">{{ t.sampleCount.toLocaleString() }}</span>
              <span class="hidden group-hover:flex gap-0.5">
                <Hint text="Rename">
                  <Button variant="ghost" size="icon-xs" class="text-muted-foreground" aria-label="Rename" @click.stop="startRename('tag', t.id, t.name)">
                    <Pencil />
                  </Button>
                </Hint>
                <Hint text="Delete tag">
                  <Button variant="ghost" size="icon-xs" class="text-muted-foreground hover:text-destructive" aria-label="Delete tag" @click.stop="confirmDeleteTag(t.id)">
                    <X />
                  </Button>
                </Hint>
              </span>
            </template>
          </div>
          <p v-if="!tags.length" class="px-2 py-2 text-muted-foreground text-xs leading-relaxed">
            Select samples and press T to tag them.
          </p>
        </nav>
      </ContextMenuTrigger>
      <ContextMenuContent class="w-52" @close-auto-focus="onCloseAutoFocus">
        <template v-if="menuSource">
          <ContextMenuItem @select="library.rescanSource(menuSource.id)">
            <RefreshCw />
            Rescan
          </ContextMenuItem>
          <ContextMenuItem @select="revealItemInDir(menuSource.path)">
            <FolderOpen />
            Show in Finder
          </ContextMenuItem>
          <ContextMenuSeparator />
          <ContextMenuItem variant="destructive" @select="removeSourceFromMenu">
            <X />
            Remove from library
          </ContextMenuItem>
        </template>
        <template v-else-if="menuItem">
          <ContextMenuItem @select="renameFromMenu">
            <Pencil />
            Rename
          </ContextMenuItem>
          <ContextMenuSeparator />
          <ContextMenuItem variant="destructive" @select="deleteFromMenu">
            <Trash2 />
            Delete {{ menuTarget?.kind === 'tag' ? 'tag' : 'collection' }}
          </ContextMenuItem>
        </template>
      </ContextMenuContent>
    </ContextMenu>
  </aside>
</template>

<style scoped>
.section {
  margin: 12px 8px 4px;
  font-size: 11px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  color: var(--muted-foreground);
}
.row {
  display: flex;
  align-items: center;
  gap: 8px;
  height: 28px;
  padding: 0 8px;
  border-radius: 6px;
  color: var(--foreground);
  text-align: left;
}
.row:hover {
  background: var(--accent);
}
.row.active {
  background: var(--primary-soft);
  color: var(--primary);
}
</style>
