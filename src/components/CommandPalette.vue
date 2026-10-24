<script setup lang="ts">
import type { SampleKind } from '@/api'
import type { Scope } from '@/stores/library'
import {
  AudioWaveform,
  Download,
  Folder,
  FolderPlus,
  Info,
  Layers,
  Library,
  ListFilter,
  Pause,
  Play,
  Settings,
  Sparkles,
  Square,
  Star,
  Tag,
  Upload,
  X,
} from '@lucide/vue'
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
  CommandSeparator,
  CommandShortcut,
} from '@/components/ui/command'
import { Dialog, DialogContent, DialogDescription, DialogTitle } from '@/components/ui/dialog'
import { CATEGORIES, categoryLabel, folderName, kindLabel } from '@/format'
import { openTrackInfo } from '@/composables/trackInfo'
import { useLibraryStore } from '@/stores/library'
import { usePlayerStore } from '@/stores/player'
import { useSettingsStore } from '@/stores/settings'

const open = defineModel<boolean>('open', { required: true })

const { t } = useI18n()
const library = useLibraryStore()
const player = usePlayerStore()
const settings = useSettingsStore()

const kinds: SampleKind[] = ['oneshot', 'loop', 'long']
const focused = computed(() => library.focused)
const pending = computed(() => library.analysis.pending + library.analysis.pendingEmbeddings)

// Actions that move focus (a popover, another dialog, a native picker) run once the
// palette has handed focus back; the rest run right away.
let afterClose: (() => unknown) | null = null

function run(action: () => unknown, takesFocus = false) {
  if (takesFocus)
    afterClose = action
  else
    action()
  open.value = false
}

function onCloseAutoFocus(e: Event) {
  if (!afterClose)
    return
  e.preventDefault()
  // The focus trap is released in a timeout of its own; run after it.
  setTimeout(afterClose, 0)
  afterClose = null
}

function go(scope: Scope) {
  run(() => (library.scope = scope))
}
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent
      :show-close-button="false"
      class="top-[18%] translate-y-0 overflow-hidden p-0 sm:max-w-lg"
      @close-auto-focus="onCloseAutoFocus"
    >
      <DialogTitle class="sr-only">
        {{ t('palette.title') }}
      </DialogTitle>
      <DialogDescription class="sr-only">
        {{ t('palette.description') }}
      </DialogDescription>
      <Command>
        <CommandInput :placeholder="t('palette.placeholder')" />
        <CommandList class="max-h-[min(360px,60vh)]">
          <CommandEmpty>{{ t('palette.empty') }}</CommandEmpty>

          <CommandGroup v-if="focused" :heading="t('palette.groups.sample')">
            <CommandItem value="sample-play" @select="run(() => player.toggle(focused!))">
              <component :is="player.playingId === focused.id ? Square : Play" />
              {{ t(player.playingId === focused.id ? 'palette.stop' : 'palette.play', { name: focused.name }) }}
              <CommandShortcut>{{ t('palette.spaceKey') }}</CommandShortcut>
            </CommandItem>
            <CommandItem value="sample-favorite" @select="run(() => library.toggleFavorite())">
              <Star />
              {{ focused.favorite ? t('palette.removeFavorite') : t('palette.addFavorite') }}
              <CommandShortcut>F</CommandShortcut>
            </CommandItem>
            <CommandItem value="sample-tag" @select="run(library.requestTagInput, true)">
              <Tag />
              {{ t('palette.addTag') }}
              <CommandShortcut>T</CommandShortcut>
            </CommandItem>
            <CommandItem value="sample-info" @select="run(() => openTrackInfo(focused), true)">
              <Info />
              {{ t('trackInfo.edit') }}
              <CommandShortcut>I</CommandShortcut>
            </CommandItem>
            <CommandItem value="sample-similar" @select="run(() => library.findSimilar(focused!))">
              <Sparkles />
              {{ t('palette.findSimilar') }}
            </CommandItem>
          </CommandGroup>

          <CommandGroup :heading="t('palette.groups.goTo')">
            <CommandItem value="go-all" @select="go({ kind: 'all' })">
              <Library /> {{ t('palette.allSamples') }}
            </CommandItem>
            <CommandItem value="go-favorites" @select="go({ kind: 'favorites' })">
              <Star /> {{ t('palette.favorites') }}
            </CommandItem>
            <CommandItem v-for="s in library.sources" :key="`s${s.id}`" :value="`go-source-${s.id}`" @select="go({ kind: 'source', id: s.id })">
              <Folder /> {{ folderName(s.path) }}
              <CommandShortcut class="tracking-normal">
                {{ t('palette.folder') }}
              </CommandShortcut>
            </CommandItem>
            <CommandItem v-for="c in library.collections" :key="`c${c.id}`" :value="`go-collection-${c.id}`" @select="go({ kind: 'collection', id: c.id })">
              <Layers /> {{ c.name }}
              <CommandShortcut class="tracking-normal">
                {{ t('palette.collection') }}
              </CommandShortcut>
            </CommandItem>
            <CommandItem v-for="tag in library.tags" :key="`t${tag.id}`" :value="`go-tag-${tag.id}`" @select="go({ kind: 'tag', id: tag.id })">
              <Tag /> {{ tag.name }}
              <CommandShortcut class="tracking-normal">
                {{ t('palette.tag') }}
              </CommandShortcut>
            </CommandItem>
          </CommandGroup>

          <CommandGroup :heading="t('palette.groups.filter')">
            <CommandItem value="filter-kind-all" @select="run(() => (library.kindFilter = null))">
              <AudioWaveform /> {{ t('palette.showAllKinds') }}
            </CommandItem>
            <CommandItem v-for="k in kinds" :key="k" :value="`filter-kind-${k}`" @select="run(() => (library.kindFilter = k))">
              <AudioWaveform /> {{ t('palette.showKind', { kind: kindLabel(k, true).toLowerCase() }) }}
            </CommandItem>
            <CommandItem
              v-for="id in CATEGORIES" :key="id" :value="`filter-category-${id}`"
              @select="run(() => (library.categoryFilter = id))"
            >
              <ListFilter /> {{ t('palette.category', { name: categoryLabel(id) }) }}
            </CommandItem>
            <CommandItem v-if="library.hasAnalysisFilters" value="filter-clear" @select="run(library.clearAnalysisFilters)">
              <X /> {{ t('palette.clearFilters') }}
            </CommandItem>
          </CommandGroup>

          <CommandSeparator />

          <CommandGroup :heading="t('palette.groups.actions')">
            <CommandItem value="action-add-folders" @select="run(library.addSources, true)">
              <FolderPlus /> {{ t('palette.addFolders') }}
            </CommandItem>
            <CommandItem v-if="pending > 0" value="action-analysis" @select="run(() => library.setAnalysisPaused(!library.analysis.paused))">
              <component :is="library.analysis.paused ? Play : Pause" />
              {{ library.analysis.paused ? t('palette.resumeAnalysis') : t('palette.pauseAnalysis') }}
            </CommandItem>
            <CommandItem value="action-export" @select="run(library.exportUserData, true)">
              <Download /> {{ t('palette.exportData') }}
            </CommandItem>
            <CommandItem value="action-import" @select="run(library.importUserData, true)">
              <Upload /> {{ t('palette.importData') }}
            </CommandItem>
            <CommandItem value="action-settings" @select="run(() => settings.openDialog(), true)">
              <Settings /> {{ t('palette.settings') }}
              <CommandShortcut>⌘,</CommandShortcut>
            </CommandItem>
          </CommandGroup>
        </CommandList>
      </Command>
    </DialogContent>
  </Dialog>
</template>
