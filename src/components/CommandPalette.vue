<script setup lang="ts">
import type { SampleKind } from '@/api'
import type { Scope } from '@/stores/library'
import {
  AudioWaveform,
  Download,
  Folder,
  FolderPlus,
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
import { CATEGORY_LABELS, folderName, KIND_LABELS } from '@/format'
import { useLibraryStore } from '@/stores/library'
import { usePlayerStore } from '@/stores/player'
import { useSettingsStore } from '@/stores/settings'

const open = defineModel<boolean>('open', { required: true })

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
        Command palette
      </DialogTitle>
      <DialogDescription class="sr-only">
        Jump to a folder, collection or tag, filter the list or run an action.
      </DialogDescription>
      <Command>
        <CommandInput placeholder="Type a command or search…" />
        <CommandList class="max-h-[min(360px,60vh)]">
          <CommandEmpty>Nothing found.</CommandEmpty>

          <CommandGroup v-if="focused" heading="Sample">
            <CommandItem value="sample-play" @select="run(() => player.toggle(focused!))">
              <component :is="player.playingId === focused.id ? Square : Play" />
              {{ player.playingId === focused.id ? 'Stop' : 'Play' }} “{{ focused.name }}”
              <CommandShortcut>Space</CommandShortcut>
            </CommandItem>
            <CommandItem value="sample-favorite" @select="run(() => library.toggleFavorite())">
              <Star />
              {{ focused.favorite ? 'Remove from favorites' : 'Add to favorites' }}
              <CommandShortcut>F</CommandShortcut>
            </CommandItem>
            <CommandItem value="sample-tag" @select="run(library.requestTagInput, true)">
              <Tag />
              Add tag…
              <CommandShortcut>T</CommandShortcut>
            </CommandItem>
            <CommandItem value="sample-similar" @select="run(() => library.findSimilar(focused!))">
              <Sparkles />
              Find similar sounds
            </CommandItem>
          </CommandGroup>

          <CommandGroup heading="Go to">
            <CommandItem value="go-all" @select="go({ kind: 'all' })">
              <Library /> All samples
            </CommandItem>
            <CommandItem value="go-favorites" @select="go({ kind: 'favorites' })">
              <Star /> Favorites
            </CommandItem>
            <CommandItem v-for="s in library.sources" :key="`s${s.id}`" :value="`go-source-${s.id}`" @select="go({ kind: 'source', id: s.id })">
              <Folder /> {{ folderName(s.path) }}
              <CommandShortcut class="tracking-normal">
                Folder
              </CommandShortcut>
            </CommandItem>
            <CommandItem v-for="c in library.collections" :key="`c${c.id}`" :value="`go-collection-${c.id}`" @select="go({ kind: 'collection', id: c.id })">
              <Layers /> {{ c.name }}
              <CommandShortcut class="tracking-normal">
                Collection
              </CommandShortcut>
            </CommandItem>
            <CommandItem v-for="t in library.tags" :key="`t${t.id}`" :value="`go-tag-${t.id}`" @select="go({ kind: 'tag', id: t.id })">
              <Tag /> {{ t.name }}
              <CommandShortcut class="tracking-normal">
                Tag
              </CommandShortcut>
            </CommandItem>
          </CommandGroup>

          <CommandGroup heading="Filter">
            <CommandItem value="filter-kind-all" @select="run(() => (library.kindFilter = null))">
              <AudioWaveform /> Show all kinds
            </CommandItem>
            <CommandItem v-for="k in kinds" :key="k" :value="`filter-kind-${k}`" @select="run(() => (library.kindFilter = k))">
              <AudioWaveform /> Show {{ KIND_LABELS[k].toLowerCase() }}s
            </CommandItem>
            <CommandItem
              v-for="(label, id) in CATEGORY_LABELS" :key="id" :value="`filter-category-${id}`"
              @select="run(() => (library.categoryFilter = id))"
            >
              <ListFilter /> Category: {{ label }}
            </CommandItem>
            <CommandItem v-if="library.hasAnalysisFilters" value="filter-clear" @select="run(library.clearAnalysisFilters)">
              <X /> Clear filters
            </CommandItem>
          </CommandGroup>

          <CommandSeparator />

          <CommandGroup heading="Actions">
            <CommandItem value="action-add-folders" @select="run(library.addSources, true)">
              <FolderPlus /> Add folders…
            </CommandItem>
            <CommandItem v-if="pending > 0" value="action-analysis" @select="run(() => library.setAnalysisPaused(!library.analysis.paused))">
              <component :is="library.analysis.paused ? Play : Pause" />
              {{ library.analysis.paused ? 'Resume analysis' : 'Pause analysis' }}
            </CommandItem>
            <CommandItem value="action-export" @select="run(library.exportUserData, true)">
              <Download /> Export favorites, tags &amp; collections…
            </CommandItem>
            <CommandItem value="action-import" @select="run(library.importUserData, true)">
              <Upload /> Import favorites, tags &amp; collections…
            </CommandItem>
            <CommandItem value="action-settings" @select="run(() => settings.openDialog(), true)">
              <Settings /> Settings
              <CommandShortcut>⌘,</CommandShortcut>
            </CommandItem>
          </CommandGroup>
        </CommandList>
      </Command>
    </DialogContent>
  </Dialog>
</template>
