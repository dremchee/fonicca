<script setup lang="ts">
import { debugMonitorOpen } from '@/composables/debugMonitor'
import { tagColorStyle } from '@/tag-colors'
import SoundModelDownload from '@/components/SoundModelDownload.vue'
import SettingsNavigation from '@/components/SettingsNavigation.vue'
import TagColorPicker from '@/components/TagColorPicker.vue'
import type { Scope } from '@/stores/library'
import { Activity, ChevronRight, FolderOpen, Pencil, Plus, RefreshCw, Settings, Trash2, X } from '@lucide/vue'
import { revealItemInDir } from '@tauri-apps/plugin-opener'
import { storeToRefs } from 'pinia'
import { useStorage } from '@vueuse/core'
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'
import { isTauri } from '@tauri-apps/api/core'
import { LogicalPosition } from '@tauri-apps/api/dpi'
import { Menu, type MenuItemOptions } from '@tauri-apps/api/menu'
import { useI18n } from 'vue-i18n'
import { toast } from 'vue-sonner'
import Hint from '@/components/Hint.vue'
import SourceDirectories from '@/components/SourceDirectories.vue'
import { Button } from '@/components/ui/button'
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from '@/components/ui/context-menu'
import { confirm } from '@/composables/confirm'
import { folderName, formatNumber } from '@/format'
import { useDuplicatesStore } from '@/stores/duplicates'
import { useLibraryStore } from '@/stores/library'
import { useSettingsStore } from '@/stores/settings'

const { t } = useI18n()
const library = useLibraryStore()
const settings = useSettingsStore()
const duplicates = useDuplicatesStore()
const { sources, tags, collections, scope, scans } = storeToRefs(library)

const sections = useStorage('fonicca-sidebar-sections', { folders: true, collections: true, tags: true }, undefined, { mergeDefaults: true })

type Editable = 'tag' | 'collection'

const editing = ref<{ kind: Editable, id: number } | null>(null)
const editName = ref('')
const editInput = ref<HTMLInputElement[]>([])
const creatingCollection = ref(false)
const newCollection = ref('')
const newCollectionInput = ref<HTMLInputElement | null>(null)
const creatingTag = ref(false)
const newTag = ref('')
const newTagInput = ref<HTMLInputElement | null>(null)
const expandedSources = ref(new Set<number>())
const loadingSources = ref(new Set<number>())

watch(scope, (current) => {
  if (current.kind === 'source' && current.path)
    expandedSources.value.add(current.id)
}, { immediate: true })

async function toggleSource(id: number) {
  if (expandedSources.value.has(id)) {
    expandedSources.value.delete(id)
    return
  }
  expandedSources.value.add(id)
  if (library.sourceFolders.has(id))
    return
  loadingSources.value.add(id)
  try {
    await library.loadSourceFolders(id)
  }
  catch (e) {
    expandedSources.value.delete(id)
    toast.error(String(e))
  }
  finally {
    loadingSources.value.delete(id)
  }
}

function isActive(s: Scope) {
  if (settings.dialogOpen) return false
  const cur = scope.value
  if (s.kind === 'source')
    return cur.kind === 'source' && cur.id === s.id && cur.path === s.path
  return cur.kind === s.kind && ('id' in s ? 'id' in cur && cur.id === s.id : true)
}

function isEditing(kind: Editable, id: number) {
  return editing.value?.kind === kind && editing.value.id === id
}

function progressLabel(id: number) {
  const s = scans.value.get(id)
  if (!s)
    return null
  return s.total ? `${s.processed}/${s.total}` : t('sidebar.scanning')
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
  sections.value.collections = true
  creatingCollection.value = true
  newCollection.value = ''
  await nextTick()
  newCollectionInput.value?.focus()
}

async function startCreateTag() {
  sections.value.tags = true
  creatingTag.value = true
  newTag.value = ''
  await nextTick()
  newTagInput.value?.focus()
}

async function commitCreateTag() {
  if (!creatingTag.value)
    return
  const name = newTag.value.trim()
  creatingTag.value = false
  if (!name)
    return
  try {
    await library.createTag(name)
  }
  catch (e) {
    toast.error(String(e))
  }
}

function onCreateTagKeydown(e: KeyboardEvent) {
  if (e.key === 'Enter')
    commitCreateTag()
  else if (e.key === 'Escape')
    creatingTag.value = false
  e.stopPropagation()
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
const menuDirectory = ref<string | null>(null)
const menuSource = computed(() => {
  const m = menuTarget.value
  return m?.kind === 'source' ? sources.value.find(s => s.id === m.id) : undefined
})
const menuItem = computed(() => {
  const m = menuTarget.value
  if (m?.kind === 'tag')
    return tags.value.find(x => x.id === m.id)
  if (m?.kind === 'collection')
    return collections.value.find(x => x.id === m.id)
  return undefined
})

async function onContextMenu(e: MouseEvent) {
  const row = (e.target as HTMLElement).closest<HTMLElement>('[data-menu]')
  const [kind, id] = row?.dataset.menu?.split(':') ?? []
  if (!kind || editing.value) {
    e.preventDefault()
    return
  }
  menuTarget.value = { kind: kind as 'source' | Editable, id: Number(id) }
  menuDirectory.value = row?.dataset.directoryPath ?? null
  if (nativeMenus) {
    e.preventDefault()
    await openNativeMenu({ x: e.clientX, y: e.clientY })
  }
}

const nativeMenus = isTauri()
let nativeMenu: Menu | null = null
onBeforeUnmount(() => { void nativeMenu?.close().catch(error => console.error('menu cleanup failed', error)) })

async function openNativeMenu(position: { x: number, y: number }) {
  const target = menuTarget.value
  const source = menuSource.value
  const entry = menuItem.value
  const directory = menuDirectory.value
  const item = (text: string, action: () => unknown): MenuItemOptions => ({
    text,
    // Let the system menu dismiss before showing a dialog or focusing an input.
    action: () => { setTimeout(() => { Promise.resolve().then(action).catch(error => toast.error(String(error))) }, 0) },
  })
  if (!target) return
  const items = source
    ? [
        item(t('sidebar.rescan'), () => library.rescanSource(source.id)),
        item(t('common.actions.showInFinder'), () => revealItemInDir(directory ?? source.path)),
        ...(!directory ? [
          { item: 'Separator' as const },
          item(t('sidebar.removeFromLibrary'), () => confirmRemoveSource(source.id)),
        ] : []),
      ]
    : entry && target.kind !== 'source'
      ? [
          item(t('common.actions.rename'), () => startRename(target.kind as Editable, entry.id, entry.name)),
          { item: 'Separator' as const },
          item(t(target.kind === 'tag' ? 'sidebar.deleteTag' : 'sidebar.deleteCollection'), () => target.kind === 'tag' ? confirmDeleteTag(entry.id) : confirmDeleteCollection(entry.id)),
        ]
      : []
  if (!items.length) return
  try {
    await nativeMenu?.close()
    nativeMenu = null
    nativeMenu = await Menu.new({ items })
    await nativeMenu.popup(new LogicalPosition(position.x, position.y))
  }
  catch (error) {
    await nativeMenu?.close().catch(() => {})
    nativeMenu = null
    toast.error(String(error))
  }
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
  const m = menuTarget.value
  const item = menuItem.value
  if (m && m.kind !== 'source' && item)
    afterClose = () => startRename(m.kind as Editable, m.id, item.name)
}

function deleteFromMenu() {
  const m = menuTarget.value
  if (m?.kind === 'tag')
    afterClose = () => confirmDeleteTag(m.id)
  else if (m?.kind === 'collection')
    afterClose = () => confirmDeleteCollection(m.id)
}

function removeSourceFromMenu() {
  const m = menuTarget.value
  if (m?.kind === 'source')
    afterClose = () => confirmRemoveSource(m.id)
}

async function confirmRemoveSource(id: number) {
  const s = sources.value.find(x => x.id === id)
  if (!s)
    return
  const ok = await confirm({
    title: t('sidebar.confirmRemoveSource.title', { name: folderName(s.path) }),
    description: t('sidebar.confirmRemoveSource.description', { n: formatNumber(s.sampleCount) }, s.sampleCount),
    action: t('common.actions.remove'),
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
    title: t('sidebar.confirmDeleteCollection.title', { name: c.name }),
    description: t('sidebar.confirmDeleteCollection.description', { n: formatNumber(c.sampleCount) }, c.sampleCount),
    action: t('common.actions.delete'),
    destructive: true,
  })
  if (ok)
    await library.deleteCollection(id)
}

async function confirmDeleteTag(id: number) {
  const tag = tags.value.find(x => x.id === id)
  if (!tag)
    return
  if (tag.sampleCount === 0) {
    await library.deleteTag(id)
    return
  }
  const ok = await confirm({
    title: t('sidebar.confirmDeleteTag.title', { name: tag.name }),
    description: t('sidebar.confirmDeleteTag.description', { n: formatNumber(tag.sampleCount) }, tag.sampleCount),
    action: t('common.actions.delete'),
    destructive: true,
  })
  if (ok)
    await library.deleteTag(id)
}
</script>

<template>
  <aside class="h-full min-h-0 flex flex-col overflow-hidden bg-card">
    <!-- Left part of this bar sits under the macOS traffic lights (overlay title bar). -->
    <div class="h-11 shrink-0" data-tauri-drag-region />

    <SettingsNavigation v-if="settings.dialogOpen" show-back class="flex-1" />
    <div v-show="!settings.dialogOpen" class="min-h-0 flex-1 flex flex-col">
    <ContextMenu>
      <ContextMenuTrigger as-child :disabled="nativeMenus">
        <nav class="min-h-0 flex-1 overflow-y-auto overscroll-contain px-2 pb-2 flex flex-col gap-px [&>*]:shrink-0" @contextmenu="onContextMenu">
          <button class="row" :class="{ active: isActive({ kind: 'all' }) }" @click="settings.dialogOpen = false; scope = { kind: 'all' }">
            <span class="i-lucide-library shrink-0" />
            <span class="flex-1 truncate">{{ t('sidebar.allSamples') }}</span>
          </button>
          <button class="row" :class="{ active: isActive({ kind: 'favorites' }) }" @click="settings.dialogOpen = false; scope = { kind: 'favorites' }">
            <span class="i-lucide-star shrink-0" />
            <span class="flex-1 truncate">{{ t('sidebar.favorites') }}</span>
          </button>
          <!-- Shows up only while there is something to clean. -->
          <button
            v-if="duplicates.count || isActive({ kind: 'duplicates' })"
            class="row"
            :class="{ active: isActive({ kind: 'duplicates' }) }"
            @click="settings.dialogOpen = false; scope = { kind: 'duplicates' }"
          >
            <span class="i-lucide-copy shrink-0" />
            <span class="flex-1 truncate">{{ t('sidebar.duplicates') }}</span>
            <span class="text-[11px] text-muted-foreground tabular">{{ formatNumber(duplicates.count) }}</span>
          </button>

          <h3 class="section flex items-center justify-between">
            <button type="button" class="section-toggle" :aria-expanded="sections.folders" @click="sections.folders = !sections.folders">
              <ChevronRight class="size-6 shrink-0 p-1.5 transition-transform" :class="{ 'rotate-90': sections.folders }" />
              {{ t('sidebar.folders') }}
            </button>
            <Hint :text="t('sidebar.addFolders')">
              <Button variant="ghost" size="icon-xs" class="text-muted-foreground -my-1" :aria-label="t('sidebar.addFolders')" @click="library.addSources()">
                <Plus />
              </Button>
            </Hint>
          </h3>
          <template v-if="sections.folders">
            <template v-for="s in sources" :key="s.id">
              <div
                class="row group" :class="{ active: isActive({ kind: 'source', id: s.id }) }"
                :data-menu="`source:${s.id}`"
                @click="settings.dialogOpen = false; scope = { kind: 'source', id: s.id }"
              >
                <button
                  v-if="!library.sourceFolders.has(s.id) || library.sourceFolders.get(s.id)?.length"
                  class="grid size-4 shrink-0 place-items-center rounded-sm text-muted-foreground hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                  :aria-expanded="expandedSources.has(s.id)" :aria-busy="loadingSources.has(s.id)"
                  :aria-label="t(expandedSources.has(s.id) ? 'sidebar.collapseFolder' : 'sidebar.expandFolder', { name: folderName(s.path) })"
                  @click.stop="toggleSource(s.id)"
                >
                  <ChevronRight class="size-3 transition-transform" :class="{ 'rotate-90': expandedSources.has(s.id) }" />
                </button>
                <span v-else class="size-4 shrink-0" />
                <span class="i-lucide-folder shrink-0" />
                <Hint :text="s.path" side="right">
                  <button
                    class="min-w-0 flex-1 truncate rounded-sm text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                    :aria-current="isActive({ kind: 'source', id: s.id }) ? 'page' : undefined"
                    @click.stop="settings.dialogOpen = false; scope = { kind: 'source', id: s.id }"
                  >
                    {{ folderName(s.path) }}
                  </button>
                </Hint>
                <span v-if="progressLabel(s.id)" class="text-[11px] text-primary tabular">{{ progressLabel(s.id) }}</span>
                <template v-else>
                  <span class="text-[11px] text-muted-foreground tabular group-hover:hidden">{{ formatNumber(s.sampleCount) }}</span>
                  <span class="hidden group-hover:flex gap-0.5">
                    <Hint :text="t('sidebar.rescan')">
                      <Button variant="ghost" size="icon-xs" class="text-muted-foreground" :aria-label="t('sidebar.rescan')" @click.stop="library.rescanSource(s.id)">
                        <RefreshCw />
                      </Button>
                    </Hint>
                    <Hint :text="t('sidebar.removeFromLibrary')">
                      <Button variant="ghost" size="icon-xs" class="text-muted-foreground hover:text-destructive" :aria-label="t('sidebar.removeFromLibrary')" @click.stop="confirmRemoveSource(s.id)">
                        <X />
                      </Button>
                    </Hint>
                  </span>
                </template>
              </div>
              <SourceDirectories
                v-if="expandedSources.has(s.id)" :source-id="s.id" :root-path="s.path"
                :folders="library.sourceFolders.get(s.id) ?? []"
              />
            </template>
            <p v-if="!sources.length" class="px-2 py-2 text-muted-foreground text-xs leading-relaxed">
              {{ t('sidebar.empty.folders') }}
            </p>

          </template>
          <h3 class="section flex items-center justify-between">
            <button type="button" class="section-toggle" :aria-expanded="sections.collections" @click="sections.collections = !sections.collections">
              <ChevronRight class="size-6 shrink-0 p-1.5 transition-transform" :class="{ 'rotate-90': sections.collections }" />
              {{ t('sidebar.collections') }}
            </button>
            <Hint :text="t('sidebar.newCollection')">
              <Button variant="ghost" size="icon-xs" class="text-muted-foreground -my-1" :aria-label="t('sidebar.newCollection')" @click="startCreateCollection">
                <Plus />
              </Button>
            </Hint>
          </h3>
          <template v-if="sections.collections">
            <div
              v-for="c in collections" :key="c.id"
              class="row group" :class="{ active: isActive({ kind: 'collection', id: c.id }) }"
              :data-menu="`collection:${c.id}`"
              @click="settings.dialogOpen = false; scope = { kind: 'collection', id: c.id }"
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
                <span class="text-[11px] text-muted-foreground tabular group-hover:hidden">{{ formatNumber(c.sampleCount) }}</span>
                <span class="hidden group-hover:flex gap-0.5">
                  <Hint :text="t('common.actions.rename')">
                    <Button variant="ghost" size="icon-xs" class="text-muted-foreground" :aria-label="t('common.actions.rename')" @click.stop="startRename('collection', c.id, c.name)">
                      <Pencil />
                    </Button>
                  </Hint>
                  <Hint :text="t('sidebar.deleteCollection')">
                    <Button variant="ghost" size="icon-xs" class="text-muted-foreground hover:text-destructive" :aria-label="t('sidebar.deleteCollection')" @click.stop="confirmDeleteCollection(c.id)">
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
                :placeholder="t('sidebar.collectionName')"
                class="flex-1 min-w-0 h-5 px-1 -mx-1 rounded bg-background outline-none border border-primary placeholder:text-muted-foreground"
                @keydown="onCreateKeydown"
                @blur="commitCreateCollection"
              >
            </div>
            <div v-else-if="!collections.length" class="px-2 py-2 flex flex-col items-start gap-2">
              <p class="text-muted-foreground text-xs leading-relaxed">{{ t('sidebar.empty.collections') }}</p>
              <Button variant="outline" size="sm" class="max-w-full" @click="startCreateCollection">
                <Plus /> <span class="truncate">{{ t('sidebar.newCollection') }}</span>
              </Button>
            </div>
          </template>
          <h3 class="section flex items-center justify-between">
            <button type="button" class="section-toggle" :aria-expanded="sections.tags" @click="sections.tags = !sections.tags">
              <ChevronRight class="size-6 shrink-0 p-1.5 transition-transform" :class="{ 'rotate-90': sections.tags }" />
              {{ t('sidebar.tags') }}
            </button>
            <Hint :text="t('sidebar.newTag')">
              <Button variant="ghost" size="icon-xs" class="text-muted-foreground -my-1" :aria-label="t('sidebar.newTag')" @click="startCreateTag">
                <Plus />
              </Button>
            </Hint>
          </h3>
          <template v-if="sections.tags">
            <div
              v-for="tag in tags" :key="tag.id"
              class="row group" :class="{ active: isActive({ kind: 'tag', id: tag.id }) }"
              :data-menu="`tag:${tag.id}`"
              @click="settings.dialogOpen = false; scope = { kind: 'tag', id: tag.id }"
              @dblclick="startRename('tag', tag.id, tag.name)"
            >
              <TagColorPicker :tag="tag" />
              <input
                v-if="isEditing('tag', tag.id)"
                ref="editInput"
                v-model="editName"
                class="flex-1 min-w-0 h-5 px-1 -mx-1 rounded bg-background outline-none border border-primary"
                @click.stop
                @keydown="onRenameKeydown"
                @blur="commitRename"
              >
              <template v-else>
                <span class="flex-1 truncate">{{ tag.name }}</span>
                <span class="text-[11px] text-muted-foreground tabular group-hover:hidden">{{ formatNumber(tag.sampleCount) }}</span>
                <span class="hidden group-hover:flex gap-0.5">
                  <Hint :text="t('common.actions.rename')">
                    <Button variant="ghost" size="icon-xs" class="text-muted-foreground" :aria-label="t('common.actions.rename')" @click.stop="startRename('tag', tag.id, tag.name)">
                      <Pencil />
                    </Button>
                  </Hint>
                  <Hint :text="t('sidebar.deleteTag')">
                    <Button variant="ghost" size="icon-xs" class="text-muted-foreground hover:text-destructive" :aria-label="t('sidebar.deleteTag')" @click.stop="confirmDeleteTag(tag.id)">
                      <X />
                    </Button>
                  </Hint>
                </span>
              </template>
            </div>
            <div v-if="creatingTag" class="row">
              <span class="tag-dot mx-0.5 shrink-0" :style="tagColorStyle(newTag)" />
              <input
                ref="newTagInput" v-model="newTag"
                :placeholder="t('sidebar.tagName')" :aria-label="t('sidebar.tagName')"
                class="flex-1 min-w-0 h-5 px-1 -mx-1 rounded bg-background outline-none border border-primary placeholder:text-muted-foreground"
                @keydown="onCreateTagKeydown" @blur="commitCreateTag"
              >
            </div>
            <p v-if="!tags.length && !creatingTag" class="px-2 py-2 text-muted-foreground text-xs leading-relaxed">
              {{ t('sidebar.empty.tags') }}
            </p>
          </template>
        </nav>
      </ContextMenuTrigger>
      <ContextMenuContent v-if="!nativeMenus" class="w-52" @close-auto-focus="onCloseAutoFocus">
        <template v-if="menuSource">
          <ContextMenuItem @select="library.rescanSource(menuSource.id)">
            <RefreshCw />
            {{ t('sidebar.rescan') }}
          </ContextMenuItem>
          <ContextMenuItem @select="revealItemInDir(menuDirectory ?? menuSource.path)">
            <FolderOpen />
            {{ t('common.actions.showInFinder') }}
          </ContextMenuItem>
          <ContextMenuSeparator v-if="!menuDirectory" />
          <ContextMenuItem v-if="!menuDirectory" variant="destructive" @select="removeSourceFromMenu">
            <X />
            {{ t('sidebar.removeFromLibrary') }}
          </ContextMenuItem>
        </template>
        <template v-else-if="menuItem">
          <ContextMenuItem @select="renameFromMenu">
            <Pencil />
            {{ t('common.actions.rename') }}
          </ContextMenuItem>
          <ContextMenuSeparator />
          <ContextMenuItem variant="destructive" @select="deleteFromMenu">
            <Trash2 />
            {{ menuTarget?.kind === 'tag' ? t('sidebar.deleteTag') : t('sidebar.deleteCollection') }}
          </ContextMenuItem>
        </template>
      </ContextMenuContent>
    </ContextMenu>
    </div>

    <footer v-if="!settings.dialogOpen" class="shrink-0 border-t border-border px-2 pt-1 pb-3">
      <SoundModelDownload />

      <button type="button" class="row w-full" :class="{ active: debugMonitorOpen }" :aria-pressed="debugMonitorOpen" @click="debugMonitorOpen = !debugMonitorOpen">
        <Activity class="size-4 shrink-0" />
        <span class="truncate">{{ t('debug.title') }}</span>
      </button>
      <button type="button" class="row w-full" :class="{ active: settings.dialogOpen }" :aria-current="settings.dialogOpen ? 'page' : undefined" @click="settings.openDialog()">
        <Settings class="size-4 shrink-0" />
        <span>{{ t('sidebar.settings') }}</span>
      </button>
    </footer>
  </aside>
</template>

<style scoped>
.section-toggle {
  display: flex;
  align-items: center;
  gap: 0;
  flex: 1;
  min-width: 0;
  text-align: left;
  font: inherit;
  text-transform: inherit;
  letter-spacing: inherit;
  border-radius: 4px;
}
.section-toggle:hover {
  color: var(--foreground);
}
.section-toggle:focus-visible {
  outline: 2px solid var(--ring);
  outline-offset: 2px;
}
.section {
  margin: 12px 0 4px;
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
