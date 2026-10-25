<script setup lang="ts">
import { useTagColors } from '@/composables/tagColors'
import type { Sample, SampleKind } from '@/api'
import type { Scope } from '@/stores/library'
import {
  AudioWaveform,
  Copy,
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
  Star,
  Tag,
  Upload,
  WandSparkles,
  X,
} from '@lucide/vue'
import { computed, ref, watch } from 'vue'
import { toast } from 'vue-sonner'
import PaletteSearchResults from '@/components/PaletteSearchResults.vue'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { useDuplicatesStore } from '@/stores/duplicates'
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
import { openBatch } from '@/composables/batch'
import { openTrackInfo } from '@/composables/trackInfo'
import { useLibraryStore } from '@/stores/library'
import { usePlayerStore } from '@/stores/player'
import { useSettingsStore } from '@/stores/settings'

const open = defineModel<boolean>('open', { required: true })

const { t } = useI18n()
const { tagStyle } = useTagColors()
const library = useLibraryStore()
const player = usePlayerStore()
const settings = useSettingsStore()
const duplicates = useDuplicatesStore()
const query = ref('')
const mode = ref<'all' | 'tracks' | 'duplicates' | 'commands'>('all')
const resultState = ref({ loading: false, count: 0, error: false })
watch(open, () => { query.value = ''; mode.value = 'all' })

async function openSample(sample: Sample) {
  open.value = false
  try {
    if (await library.revealSample(sample))
      await player.play(sample)
  }
  catch (e) { toast.error(String(e)) }
}
function showTracks(text: string) {
  run(() => {
    library.scope = { kind: 'all' }
    library.clearAnalysisFilters()
    library.query = text
  })
}
function showDuplicates(text: string, groupKey?: string, copyId?: number) {
  run(() => {
    duplicates.query = text
    duplicates.selected.clear()
    if (groupKey) duplicates.selected.add(groupKey)
    duplicates.focusedId = copyId ?? null
    library.scope = { kind: 'duplicates' }
  })
}

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
  run(() => { library.query = ''; library.scope = scope })
}
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent
      :show-close-button="false"
      class="top-[18%] translate-y-0 overflow-hidden p-0 sm:max-w-xl"
      @close-auto-focus="onCloseAutoFocus"
    >
      <DialogTitle class="sr-only">
        {{ t('palette.title') }}
      </DialogTitle>
      <DialogDescription class="sr-only">
        {{ t('palette.description') }}
      </DialogDescription>
      <Command>
        <CommandInput :placeholder="t('palette.placeholder')" :aria-label="t('palette.title')" @input="query = ($event.target as HTMLInputElement).value" />
        <div class="flex flex-wrap items-center gap-2 border-b px-3 py-2">
          <ToggleGroup :model-value="mode" type="single" size="sm" :aria-label="t('palette.searchMode')" @update:model-value="value => { if (value === 'all' || value === 'tracks' || value === 'duplicates' || value === 'commands') mode = value }">
            <ToggleGroupItem v-for="tab in ['all', 'tracks', 'duplicates', 'commands'] as const" :key="tab" :value="tab">{{ t(`palette.modes.${tab}`) }}</ToggleGroupItem>
          </ToggleGroup>
          <ToggleGroup v-if="mode === 'duplicates'" :model-value="duplicates.view" type="single" size="sm" class="ml-auto" :aria-label="t('palette.duplicateView')" @update:model-value="value => { if (value === 'groups' || value === 'files') duplicates.view = value }">
            <ToggleGroupItem value="groups">{{ t('palette.grouped') }}</ToggleGroupItem>
            <ToggleGroupItem value="files">{{ t('palette.files') }}</ToggleGroupItem>
          </ToggleGroup>
        </div>
        <CommandList class="max-h-[min(360px,60vh)]">
          <CommandEmpty v-if="!resultState.loading && !resultState.count && !resultState.error">{{ t('palette.empty') }}</CommandEmpty>
          <PaletteSearchResults :open="open" :query="query" :mode="mode" :duplicate-view="duplicates.view" @state="resultState = $event" @sample="openSample" @library="showTracks" @duplicates="showDuplicates" />
          <template v-if="mode === 'all' || mode === 'commands'">

          <CommandGroup v-if="focused" :heading="t('palette.groups.sample')">
            <CommandItem value="sample-play" @select="run(() => player.toggle(focused!))">
              <component :is="player.isPlaying(focused.id) ? Pause : Play" />
              {{ t(player.isPlaying(focused.id) ? 'palette.pause' : 'palette.play', { name: focused.name }) }}
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
            <CommandItem value="sample-batch" @select="run(() => openBatch(library.selectedIds.size ? [...library.selectedIds] : [focused!.id]), true)">
              <WandSparkles />
              {{ library.selectedIds.size > 1 ? `${t('batch.title', { n: library.selectedIds.size }, library.selectedIds.size)}…` : t('batch.action') }}
            </CommandItem>
          </CommandGroup>

          <CommandGroup :heading="t('palette.groups.goTo')">
            <CommandItem value="go-all" @select="go({ kind: 'all' })">
              <Library /> {{ t('palette.allSamples') }}
            </CommandItem>
            <CommandItem value="go-duplicates" @select="showDuplicates('')">
              <Copy /> {{ t('palette.modes.duplicates') }}
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
              <span class="tag-dot shrink-0" :style="tagStyle(tag)" aria-hidden="true" /> {{ tag.name }}
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
          </template>
        </CommandList>
        <div class="flex items-center gap-4 border-t px-3 py-2 text-[11px] text-muted-foreground">
          <span>↑ ↓ {{ t('palette.navigate') }}</span><span>↵ {{ t('palette.openResult') }}</span><span class="ml-auto">Esc {{ t('palette.close') }}</span>
        </div>
      </Command>
    </DialogContent>
  </Dialog>
</template>
