<script setup lang="ts">
import type { Sample } from '@/api'
import { AudioLines, CircleMinus, FolderOpen, Info, Layers, Music, Pencil, Pause, Play, Star, Tag, WandSparkles } from '@lucide/vue'
import { revealItemInDir } from '@tauri-apps/plugin-opener'
import { computed, onBeforeUnmount, ref } from 'vue'
import { Menu, type MenuItemOptions } from '@tauri-apps/api/menu'
import { LogicalPosition } from '@tauri-apps/api/dpi'
import { toast } from 'vue-sonner'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter } from '@/components/ui/dialog'
import { useI18n } from 'vue-i18n'
import {
  ContextMenuContent,
  ContextMenuGroup,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuShortcut,
  ContextMenuSub,
  ContextMenuSubContent,
  ContextMenuSubTrigger,
} from '@/components/ui/context-menu'
import { Input } from '@/components/ui/input'
import { openBatch } from '@/composables/batch'
import { openTrackInfo } from '@/composables/trackInfo'
import { openEditor } from '@/stores/editor'
import { useLibraryStore } from '@/stores/library'
import { usePlayerStore } from '@/stores/player'
import { useStemsStore } from '@/stores/stems'
import { useApolloStore } from '@/stores/apollo'
import { useBasicPitchStore } from '@/stores/basicPitch'

const props = defineProps<{ sample: Sample, native?: boolean }>()
const emit = defineEmits<{ close: [] }>()

const { t } = useI18n()
const library = useLibraryStore()
const player = usePlayerStore()
const newName = ref('')

const count = computed(() => (library.selectedIds.has(props.sample.id) ? library.selectedIds.size : 1))
const favoriteLabel = computed(() => {
  if (count.value > 1)
    return t(props.sample.favorite ? 'list.favorite.removeMany' : 'list.favorite.addMany', { samples: t('common.count.samples', count.value) })
  return props.sample.favorite ? t('list.favorite.remove') : t('list.favorite.add')
})
const currentCollection = computed(() => {
  const s = library.scope
  return s.kind === 'collection' ? library.collections.find(c => c.id === s.id) : undefined
})

// Actions that move focus elsewhere run once the menu has closed,
// otherwise the menu hands focus back to the list and steals it.
let afterClose: (() => unknown) | null = null

function runAfterClose(action: () => unknown) {
  afterClose = action
}

function onCloseAutoFocus(e: Event) {
  if (!afterClose)
    return
  e.preventDefault()
  // The focus trap is released in a timeout of its own; run after it.
  setTimeout(afterClose, 0)
  afterClose = null
}

async function submitNew() {
  const name = newName.value.trim()
  if (!name)
    return
  newName.value = ''
  emit('close')
  await library.createCollection(name, true)
}

const creatingCollection = ref(false)
let nativeMenu: Menu | null = null

onBeforeUnmount(() => { void nativeMenu?.close().catch(error => console.error('menu cleanup failed', error)) })

async function openNative(position: { x: number, y: number }) {
  const sample = props.sample
  const ids = library.selectedIds.has(sample.id) ? [...library.selectedIds] : [sample.id]
  const collection = currentCollection.value
  // Keep the menu resource alive after popup resolves (it resolves on show).
  // Release it when the next menu replaces it, or the component unmounts.
  const item = (text: string, action: () => unknown, accelerator?: string, enabled = true): MenuItemOptions => ({
    text, accelerator, enabled,
    action: () => { setTimeout(() => { Promise.resolve().then(action).catch(error => toast.error(String(error))) }, 0) },
  })
  try {
    await nativeMenu?.close()
    nativeMenu = null
    nativeMenu = await Menu.new({ items: [
      item(t(player.isPlaying(sample.id) ? 'common.actions.pause' : 'common.actions.play'), () => player.toggle(sample), 'Space'),
      item(t('list.menu.edit'), () => openEditor(sample), 'E'),
      item(t('trackInfo.edit'), () => openTrackInfo(sample), 'I'),
      item(t('list.menu.findSimilar'), () => library.findSimilar(sample), 'S', !!library.soundModel?.installed),
      { text: t('list.menu.processing'), items: [
        item(ids.length > 1 ? `${t('batch.title', { n: ids.length }, ids.length)}…` : t('batch.action'), () => openBatch(ids)),
        item(t('stems.action'), () => useStemsStore().show(sample)),
        item(t('apollo.action'), () => useApolloStore().show(sample)),
        item(t('basicPitch.action'), () => useBasicPitchStore().show(sample)),
      ] },
      { item: 'Separator' },
      item(favoriteLabel.value, () => library.toggleFavorite(sample), 'F'),
      item(t('list.menu.addTag'), () => library.requestTagInput(), 'T'),
      { text: t('list.menu.addToCollection'), items: [
        ...library.collections.map(c => item(c.name, () => library.addToCollection(c.id))),
        ...(library.collections.length ? [{ item: 'Separator' as const }] : []),
        item(t('list.menu.newCollection'), () => { newName.value = ''; creatingCollection.value = true }),
      ] },
      ...(collection ? [item(t('list.menu.removeFromCollection', { name: collection.name }), () => library.removeFromCollection(collection.id))] : []),
      { item: 'Separator' },
      item(t('common.actions.showInFinder'), () => revealItemInDir(sample.path)),
    ] })
    await nativeMenu.popup(new LogicalPosition(position.x, position.y))
  }
  catch (error) {
    await nativeMenu?.close().catch(() => {})
    nativeMenu = null
    toast.error(String(error))
  }
}

async function createFromDialog() {
  if (!newName.value.trim()) return
  try {
    await submitNew()
    creatingCollection.value = false
  }
  catch (error) { toast.error(String(error)) }
}

defineExpose({ openNative })

</script>

<template>
  <ContextMenuContent v-if="!native" class="w-56" @close-auto-focus="onCloseAutoFocus">
    <ContextMenuItem @select="player.toggle(sample)">
      <component :is="player.isPlaying(sample.id) ? Pause : Play" />
      {{ player.isPlaying(sample.id) ? t('common.actions.pause') : t('common.actions.play') }}
      <ContextMenuShortcut>{{ t('list.menu.space') }}</ContextMenuShortcut>
    </ContextMenuItem>
    <ContextMenuItem @select="openEditor(sample)">
      <Pencil />
      {{ t('list.menu.edit') }}
      <ContextMenuShortcut>E</ContextMenuShortcut>
    </ContextMenuItem>
    <ContextMenuItem @select="runAfterClose(() => openTrackInfo(sample))">
      <Info />
      {{ t('trackInfo.edit') }}
      <ContextMenuShortcut>I</ContextMenuShortcut>
    </ContextMenuItem>
    <ContextMenuItem
      :disabled="!library.soundModel?.installed"
      :title="library.soundModel?.installed ? undefined : t('list.menu.needsSoundModel')"
      @select="library.findSimilar(sample)"
    >
      <AudioLines />
      {{ t('list.menu.findSimilar') }}
      <ContextMenuShortcut>S</ContextMenuShortcut>
    </ContextMenuItem>
    <!-- The selection when the row is in it, otherwise this row. -->
    <ContextMenuItem @select="runAfterClose(() => openBatch(library.selectedIds.has(sample.id) ? [...library.selectedIds] : [sample.id]))">
      <WandSparkles />
      <span class="truncate">{{ count > 1 ? `${t('batch.title', { n: count }, count)}…` : t('batch.action') }}</span>
    </ContextMenuItem>
    <ContextMenuItem @select="runAfterClose(() => useStemsStore().show(sample))"><Layers />{{ t('stems.action') }}</ContextMenuItem>
    <ContextMenuItem @select="runAfterClose(() => useApolloStore().show(sample))"><WandSparkles />{{ t('apollo.action') }}</ContextMenuItem>
    <ContextMenuItem @select="runAfterClose(() => useBasicPitchStore().show(sample))"><Music />{{ t('basicPitch.action') }}</ContextMenuItem>
    <ContextMenuSeparator />
    <ContextMenuGroup>
      <ContextMenuItem @select="library.toggleFavorite(sample)">
        <Star />
        {{ favoriteLabel }}
        <ContextMenuShortcut>F</ContextMenuShortcut>
      </ContextMenuItem>
      <ContextMenuItem @select="runAfterClose(library.requestTagInput)">
        <Tag />
        {{ t('list.menu.addTag') }}
        <ContextMenuShortcut>T</ContextMenuShortcut>
      </ContextMenuItem>
      <ContextMenuSub>
        <ContextMenuSubTrigger class="gap-2">
          <Layers class="text-muted-foreground" />
          {{ t('list.menu.addToCollection') }}
        </ContextMenuSubTrigger>
        <ContextMenuSubContent class="w-52 max-h-72 overflow-y-auto">
          <ContextMenuItem v-for="c in library.collections" :key="c.id" @select="library.addToCollection(c.id)">
            <span class="truncate">{{ c.name }}</span>
          </ContextMenuItem>
          <ContextMenuSeparator v-if="library.collections.length" />
          <Input
            v-model="newName"
            :placeholder="t('list.menu.newCollection')"
            class="h-8"
            @keydown.stop
            @keydown.enter="submitNew"
          />
        </ContextMenuSubContent>
      </ContextMenuSub>
      <ContextMenuItem v-if="currentCollection" @select="library.removeFromCollection(currentCollection.id)">
        <CircleMinus />
        <span class="truncate">{{ t('list.menu.removeFromCollection', { name: currentCollection.name }) }}</span>
      </ContextMenuItem>
    </ContextMenuGroup>
    <ContextMenuSeparator />
    <ContextMenuItem @select="revealItemInDir(sample.path)">
      <FolderOpen />
      {{ t('common.actions.showInFinder') }}
    </ContextMenuItem>
  </ContextMenuContent>
  <Dialog v-if="native" v-model:open="creatingCollection">
    <DialogContent class="sm:max-w-sm">
      <DialogHeader><DialogTitle>{{ t('sidebar.newCollection') }}</DialogTitle></DialogHeader>
      <form class="space-y-4" @submit.prevent="createFromDialog">
        <Input v-model="newName" :placeholder="t('sidebar.collectionName')" :aria-label="t('sidebar.collectionName')" />
        <DialogFooter>
          <Button type="button" variant="ghost" @click="creatingCollection = false">{{ t('common.actions.cancel') }}</Button>
          <Button type="submit" :disabled="!newName.trim()">{{ t('sidebar.newCollection') }}</Button>
        </DialogFooter>
      </form>
    </DialogContent>
  </Dialog>
</template>
