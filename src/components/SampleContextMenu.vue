<script setup lang="ts">
import type { Sample } from '@/api'
import { AudioLines, CircleMinus, FolderOpen, Info, Layers, Pencil, Play, Square, Star, Tag } from '@lucide/vue'
import { revealItemInDir } from '@tauri-apps/plugin-opener'
import { computed, ref } from 'vue'
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
import { openTrackInfo } from '@/composables/trackInfo'
import { openEditor } from '@/stores/editor'
import { useLibraryStore } from '@/stores/library'
import { usePlayerStore } from '@/stores/player'

const props = defineProps<{ sample: Sample }>()
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
</script>

<template>
  <ContextMenuContent class="w-56" @close-auto-focus="onCloseAutoFocus">
    <ContextMenuItem @select="player.toggle(sample)">
      <component :is="player.playingId === sample.id ? Square : Play" />
      {{ player.playingId === sample.id ? t('common.actions.stop') : t('common.actions.play') }}
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
</template>
