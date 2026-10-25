<script setup lang="ts">
import type { Component } from 'vue'
import type { AppInfo, Theme, WaveformSettings } from '@/api'
import type { Locale } from '@/i18n'
import type { SettingsSection } from '@/stores/settings'
import darkAppIcon from '../../src-tauri/icons/128x128-dark.png'
import lightAppIcon from '../../src-tauri/icons/128x128-light.png'
import {
  AudioLines,
  AudioWaveform,
  Palette,
  Cpu,
  Download,
  EyeOff,
  FolderOpen,
  FolderPlus,
  Info,
  Keyboard,
  Library,
  Monitor,
  Moon,
  Pause,
  Play,
  RefreshCw,
  Settings2,
  Sun,
  Trash2,
  Upload,
  Volume2,
} from '@lucide/vue'
import { revealItemInDir } from '@tauri-apps/plugin-opener'
import { storeToRefs } from 'pinia'
import { TabsContent, TabsList, TabsRoot, TabsTrigger } from 'reka-ui'
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { toast } from 'vue-sonner'
import { api } from '@/api'
import Hint from '@/components/Hint.vue'
import ThemesSettings from '@/components/ThemesSettings.vue'
import ModelsSettings from '@/components/ModelsSettings.vue'
import StemStorageSettings from '@/components/StemStorageSettings.vue'
import WaveformControls from '@/components/WaveformControls.vue'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogTitle } from '@/components/ui/dialog'
import { Kbd, KbdGroup } from '@/components/ui/kbd'
import { Select, SelectContent, SelectItem, SelectSeparator, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Slider } from '@/components/ui/slider'
import { Spinner } from '@/components/ui/spinner'
import { Switch } from '@/components/ui/switch'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { confirm } from '@/composables/confirm'
import { LOCALES } from '@/i18n'
import { folderName, formatNumber, formatSize } from '@/format'
import { useDuplicatesStore } from '@/stores/duplicates'
import { useLibraryStore } from '@/stores/library'
import { useSettingsStore } from '@/stores/settings'

const props = withDefaults(defineProps<{ waveTarget?: 'preview' | 'editor' }>(), { waveTarget: 'preview' })
const { t } = useI18n()
const store = useSettingsStore()
const { settings, dialogOpen, section } = storeToRefs(store)
const appIcon = computed(() => store.mode === 'dark' ? darkAppIcon : lightAppIcon)
const library = useLibraryStore()
const { sources, scans, analysis } = storeToRefs(library)
const duplicates = useDuplicatesStore()

const sections: { id: SettingsSection, icon: Component }[] = [
  { id: 'general', icon: Settings2 },
  { id: 'library', icon: Library },
  { id: 'playback', icon: AudioLines },
  { id: 'waveform', icon: AudioWaveform },
  { id: 'themes', icon: Palette },
  { id: 'analysis', icon: Cpu },
  { id: 'shortcuts', icon: Keyboard },
  { id: 'about', icon: Info },
]
const current = computed(() => sections.find(s => s.id === section.value)!)

// Paths and sizes change while the app runs: read them each time the dialog opens.
const info = ref<AppInfo | null>(null)
watch(dialogOpen, async (open) => {
  if (open)
    [info.value] = await Promise.all([api.appInfo(), duplicates.loadHidden()])
})

async function refreshInfo() {
  info.value = await api.appInfo()
}

async function unhide(paths: string[] | null) {
  try {
    await duplicates.unhide(paths)
  }
  catch (e) {
    toast.error(String(e))
  }
}

function samplesLabel(n: number) {
  return t('common.count.samples', { n: formatNumber(n) }, n)
}

function scanLabel(id: number) {
  const s = scans.value.get(id)
  if (!s)
    return null
  return s.total
    ? t('settings.library.scanningProgress', { processed: formatNumber(s.processed), total: formatNumber(s.total) })
    : t('settings.library.scanning')
}

async function removeSource(id: number) {
  const s = sources.value.find(x => x.id === id)
  if (!s)
    return
  const ok = await confirm({
    title: t('settings.library.removeTitle', { name: folderName(s.path) }),
    description: t('settings.library.removeDesc', { samples: samplesLabel(s.sampleCount) }, s.sampleCount),
    action: t('common.actions.remove'),
    destructive: true,
  })
  if (ok)
    await library.removeSource(id)
}

async function reveal(path: string) {
  try {
    await revealItemInDir(path)
  }
  catch (e) {
    toast.error(String(e))
  }
}

// Dragging updates the label only; the value is stored once the thumb is released.
const volumeDrag = ref<number | null>(null)
const volumePercent = computed(() => Math.round((volumeDrag.value ?? settings.value.volume) * 100))

const pending = computed(() => analysis.value.pending + analysis.value.pendingEmbeddings)

const threads = computed({
  get: () => settings.value.analysisThreads == null ? 'auto' : String(settings.value.analysisThreads),
  set: (v: string) => store.update({ analysisThreads: v === 'auto' ? null : Number(v) }),
})

const waveTargets = [
  { value: 'preview', labelKey: 'settings.waveform.previewTab' },
  { value: 'editor', labelKey: 'settings.waveform.editorTab' },
] as const

function waveSettings(target: 'preview' | 'editor') {
  return target === 'editor' ? settings.value.editorWaveform : settings.value
}

function setWaveOptions(target: 'preview' | 'editor', patch: Partial<WaveformSettings>) {
  if (target === 'editor')
    store.update({ editorWaveform: { ...settings.value.editorWaveform, ...patch } })
  else
    store.update(patch)
}

const shortcuts: { keys: string[], labelKey: string }[] = [
  { keys: ['↑', '↓'], labelKey: 'settings.shortcuts.moveFocus' },
  { keys: ['⇧', '↑ ↓'], labelKey: 'settings.shortcuts.extendSelection' },
  { keys: ['space'], labelKey: 'settings.shortcuts.playStop' },
  { keys: ['Esc'], labelKey: 'settings.shortcuts.escape' },
  { keys: ['⌘', 'A'], labelKey: 'settings.shortcuts.selectAll' },
  { keys: ['F'], labelKey: 'settings.shortcuts.favorite' },
  { keys: ['T'], labelKey: 'settings.shortcuts.tag' },
  { keys: ['E'], labelKey: 'settings.shortcuts.edit' },
  { keys: ['I'], labelKey: 'settings.shortcuts.info' },
  { keys: ['S'], labelKey: 'settings.shortcuts.similar' },
  { keys: ['⌥', 'scroll'], labelKey: 'settings.shortcuts.verticalZoom' },
  { keys: ['1…9'], labelKey: 'settings.shortcuts.playRegion' },
  { keys: ['⇧', '1…9'], labelKey: 'settings.shortcuts.playRegion10' },
  { keys: ['⌘', 'K'], labelKey: 'settings.shortcuts.palette' },
  { keys: ['⌘', 'F'], labelKey: 'settings.shortcuts.search' },
  { keys: ['⌘', ','], labelKey: 'settings.shortcuts.settings' },
]
// Named keys are translated; glyphs and letters are shown as is.
const NAMED_KEYS = new Set(['space', 'scroll'])
function keyLabel(k: string) {
  return NAMED_KEYS.has(k) ? t(`settings.shortcuts.keys.${k}`) : k
}
</script>

<template>
  <Dialog v-model:open="dialogOpen">
    <DialogContent class="p-0 gap-0 sm:max-w-5xl h-[min(720px,calc(100vh-4rem))] grid grid-cols-[200px_1fr] overflow-hidden">
      <nav class="bg-muted/40 border-r border-border p-2 pt-4 flex flex-col gap-px">
        <button
          v-for="s in sections"
          :key="s.id"
          class="nav-row"
          :class="{ active: section === s.id }"
          @click="section = s.id"
        >
          <component :is="s.icon" class="size-4 shrink-0" />
          {{ t(`settings.nav.${s.id}`) }}
        </button>
      </nav>

      <div class="min-h-0 min-w-0 flex flex-col">
        <header class="h-13 shrink-0 px-6 flex items-center border-b border-border">
          <DialogTitle class="text-base font-semibold">
            {{ t(`settings.nav.${current.id}`) }}
          </DialogTitle>
          <DialogDescription class="sr-only">
            {{ t('settings.description', { section: t(`settings.nav.${current.id}`) }) }}
          </DialogDescription>
        </header>

        <div class="flex-1 overflow-y-auto px-6 py-5 flex flex-col gap-6">
          <!-- General -->
          <template v-if="section === 'general'">
            <section class="group-box">
              <div class="row">
                <div class="row-text">
                  <div class="row-title">
                    {{ t('settings.general.appearance') }}
                  </div>
                  <div class="row-desc">
                    {{ t('settings.general.appearanceDesc') }}
                  </div>
                </div>
                <ToggleGroup
                  :model-value="settings.theme"
                  type="single"
                  variant="outline"
                  size="sm"
                  @update:model-value="$event && store.update({ theme: $event as Theme })"
                >
                  <ToggleGroupItem value="system" :aria-label="t('settings.general.themeSystem')">
                    <Monitor /> {{ t('settings.general.themeSystem') }}
                  </ToggleGroupItem>
                  <ToggleGroupItem value="light" :aria-label="t('settings.general.themeLight')">
                    <Sun /> {{ t('settings.general.themeLight') }}
                  </ToggleGroupItem>
                  <ToggleGroupItem value="dark" :aria-label="t('settings.general.themeDark')">
                    <Moon /> {{ t('settings.general.themeDark') }}
                  </ToggleGroupItem>
                </ToggleGroup>
              </div>
              <div class="row">
                <div class="row-text">
                  <div class="row-title">
                    {{ t('settings.general.language') }}
                  </div>
                </div>
                <Select :model-value="settings.language" @update:model-value="store.update({ language: $event as Locale })">
                  <SelectTrigger size="sm" class="w-40">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem v-for="l in LOCALES" :key="l.id" :value="l.id">
                      {{ l.name }}
                    </SelectItem>
                  </SelectContent>
                </Select>
              </div>
            </section>
          </template>

          <!-- Library -->
          <template v-else-if="section === 'library'">
            <div class="flex flex-col gap-2">
              <div class="flex items-center justify-between">
                <h3 class="group-title">
                  {{ t('settings.library.folders') }}
                </h3>
                <Button variant="outline" size="sm" @click="library.addSources()">
                  <FolderPlus /> {{ t('settings.library.addFolders') }}
                </Button>
              </div>
              <section class="group-box">
                <div v-if="!sources.length" class="row text-muted-foreground justify-center">
                  {{ t('settings.library.noFolders') }}
                </div>
                <div v-for="s in sources" :key="s.id" class="row">
                  <span class="i-lucide-folder text-muted-foreground shrink-0" />
                  <div class="row-text">
                    <div class="row-title truncate">
                      {{ folderName(s.path) }}
                    </div>
                    <div class="row-desc truncate tabular" :title="s.path">
                      {{ scanLabel(s.id) ?? samplesLabel(s.sampleCount) }} · {{ s.path }}
                    </div>
                  </div>
                  <div class="flex gap-0.5">
                    <Hint :text="t('common.actions.showInFinder')">
                      <Button variant="ghost" size="icon-sm" class="text-muted-foreground" :aria-label="t('common.actions.showInFinder')" @click="reveal(s.path)">
                        <FolderOpen />
                      </Button>
                    </Hint>
                    <Hint :text="t('settings.library.rescan')">
                      <Button variant="ghost" size="icon-sm" class="text-muted-foreground" :aria-label="t('settings.library.rescan')" :disabled="scans.has(s.id)" @click="library.rescanSource(s.id)">
                        <RefreshCw />
                      </Button>
                    </Hint>
                    <Hint :text="t('settings.library.removeFromLibrary')">
                      <Button variant="ghost" size="icon-sm" class="text-muted-foreground hover:text-destructive" :aria-label="t('settings.library.removeFromLibrary')" @click="removeSource(s.id)">
                        <Trash2 />
                      </Button>
                    </Hint>
                  </div>
                </div>
              </section>
            </div>

            <section class="group-box">
              <label class="row">
                <div class="row-text">
                  <div class="row-title">{{ t('settings.library.watch') }}</div>
                  <div class="row-desc">{{ t('settings.library.watchDesc') }}</div>
                </div>
                <Switch :model-value="settings.watchFolders" @update:model-value="store.update({ watchFolders: $event })" />
              </label>
            </section>

            <div class="flex flex-col gap-2">
              <div class="flex items-center justify-between">
                <h3 class="group-title">
                  {{ t('settings.library.hidden') }}<template v-if="duplicates.hidden.length">
                    · {{ formatNumber(duplicates.hidden.length) }}
                  </template>
                </h3>
                <Button v-if="duplicates.hidden.length > 1" variant="outline" size="sm" @click="unhide(null)">
                  {{ t('settings.library.unhideAll') }}
                </Button>
              </div>
              <section class="group-box max-h-64 overflow-y-auto">
                <div v-if="!duplicates.hidden.length" class="row text-muted-foreground">
                  {{ t('settings.library.nothingHidden') }}
                </div>
                <div v-for="h in duplicates.hidden" :key="h.path" class="row py-2">
                  <EyeOff class="size-3.5 text-muted-foreground shrink-0" />
                  <div class="row-text">
                    <div class="truncate">
                      {{ folderName(h.path) }}
                    </div>
                    <div class="row-desc truncate select-text" :title="h.path">
                      {{ h.path }}
                    </div>
                  </div>
                  <Button variant="ghost" size="xs" @click="unhide([h.path])">
                    {{ t('settings.library.unhide') }}
                  </Button>
                </div>
              </section>
            </div>

            <div class="flex flex-col gap-2">
              <h3 class="group-title">
                {{ t('settings.library.yourData') }}
              </h3>
              <section class="group-box">
                <div class="row">
                  <div class="row-text">
                    <div class="row-title">
                      {{ t('settings.library.userData') }}
                    </div>
                    <div class="row-desc">
                      {{ t('settings.library.userDataDesc') }}
                    </div>
                  </div>
                  <div class="flex gap-2">
                    <Button variant="outline" size="sm" @click="library.exportUserData()">
                      <Download /> {{ t('settings.library.export') }}
                    </Button>
                    <Button variant="outline" size="sm" @click="library.importUserData()">
                      <Upload /> {{ t('settings.library.import') }}
                    </Button>
                  </div>
                </div>
              </section>
            </div>
          </template>

          <!-- Playback -->
          <template v-else-if="section === 'playback'">
            <section class="group-box">
              <label class="row">
                <div class="row-text">
                  <div class="row-title">{{ t('settings.playback.autoplay') }}</div>
                  <div class="row-desc">{{ t('settings.playback.autoplayDesc') }}</div>
                </div>
                <Switch :model-value="settings.autoplay" @update:model-value="store.update({ autoplay: $event })" />
              </label>
              <div class="row">
                <div class="row-text">
                  <div class="row-title">
                    {{ t('settings.playback.volume') }}
                  </div>
                  <div class="row-desc">
                    {{ t('settings.playback.volumeDesc') }}
                  </div>
                </div>
                <div class="w-48 flex items-center gap-3">
                  <Volume2 class="size-4 text-muted-foreground shrink-0" />
                  <Slider
                    :model-value="[volumePercent]"
                    :max="100"
                    :step="1"
                    :aria-label="t('settings.playback.volume')"
                    @update:model-value="volumeDrag = ($event?.[0] ?? 0) / 100"
                    @value-commit="store.update({ volume: ($event[0] ?? 0) / 100 }).then(() => volumeDrag = null)"
                  />
                  <span class="w-9 text-right tabular text-muted-foreground">{{ volumePercent }}%</span>
                </div>
              </div>
            </section>
          </template>

          <!-- Waveform -->
          <template v-else-if="section === 'waveform'">
            <TabsRoot :default-value="props.waveTarget" class="flex flex-col gap-4">
              <TabsList class="inline-flex h-9 w-fit items-center rounded-md bg-muted p-1 text-muted-foreground" :aria-label="t('settings.nav.waveform')">
                <TabsTrigger
                  v-for="target in waveTargets" :key="target.value" :value="target.value"
                  class="inline-flex items-center justify-center whitespace-nowrap rounded-sm px-3 py-1 text-sm font-medium transition-all focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-50 data-[state=active]:bg-background data-[state=active]:text-foreground data-[state=active]:shadow-sm"
                >
                  {{ t(target.labelKey) }}
                </TabsTrigger>
              </TabsList>
              <TabsContent v-for="target in waveTargets" :key="target.value" :value="target.value" class="flex flex-col gap-4 outline-none focus-visible:ring-2 focus-visible:ring-ring data-[state=inactive]:hidden">
                <WaveformControls :target="target.value" :options="waveSettings(target.value)" @change="setWaveOptions(target.value, $event)" />
              </TabsContent>
            </TabsRoot>
          </template>

          <!-- Analysis -->
          <ThemesSettings v-else-if="section === 'themes'" />

          <template v-else-if="section === 'analysis'">
            <section class="group-box">
              <div class="row">
                <div class="row-text">
                  <div class="row-title flex items-center gap-2">
                    <Spinner v-if="!analysis.paused && pending" class="text-primary" />
                    {{ !pending ? t('settings.analysis.done') : analysis.paused ? t('settings.analysis.paused') : t('settings.analysis.running') }}
                  </div>
                  <div class="row-desc tabular">
                    <template v-if="pending">
                      {{ t('settings.analysis.left', { n: formatNumber(pending) }) }}
                    </template>
                    {{ t('settings.analysis.what') }}
                  </div>
                </div>
                <Button variant="outline" size="sm" :disabled="!pending" @click="library.setAnalysisPaused(!analysis.paused)">
                  <component :is="analysis.paused ? Play : Pause" />
                  {{ analysis.paused ? t('settings.analysis.resume') : t('settings.analysis.pause') }}
                </Button>
              </div>
              <div class="row">
                <div class="row-text">
                  <div class="row-title">
                    {{ t('settings.analysis.threads') }}
                  </div>
                  <div class="row-desc">
                    {{ t('settings.analysis.threadsDesc') }}
                  </div>
                </div>
                <Select v-model="threads">
                  <SelectTrigger size="sm" class="w-40">
                    <SelectValue>
                      <template v-if="threads === 'auto'">
                        {{ t('settings.analysis.auto') }}<template v-if="info">
                          {{ t('settings.analysis.autoThreads', { n: info.defaultThreads, cores: info.cores }) }}
                        </template>
                      </template>
                      <template v-else>
                        {{ threads }}
                      </template>
                    </SelectValue>
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="auto">
                      {{ t('settings.analysis.auto') }}<template v-if="info">
                        {{ t('settings.analysis.autoThreads', { n: info.defaultThreads, cores: info.cores }) }}
                      </template>
                    </SelectItem>
                    <template v-if="info">
                      <SelectSeparator />
                      <SelectItem v-for="n in info.cores" :key="n" :value="String(n)">
                        {{ n }}
                      </SelectItem>
                    </template>
                  </SelectContent>
                </Select>
              </div>
            </section>

            <StemStorageSettings />
            <ModelsSettings @changed="refreshInfo" />
          </template>

          <!-- Shortcuts -->
          <template v-else-if="section === 'shortcuts'">
            <section class="group-box">
              <div v-for="s in shortcuts" :key="s.labelKey" class="row py-2">
                <div class="flex-1">
                  {{ t(s.labelKey) }}
                </div>
                <KbdGroup>
                  <Kbd v-for="k in s.keys" :key="k">
                    {{ keyLabel(k) }}
                  </Kbd>
                </KbdGroup>
              </div>
            </section>
          </template>

          <!-- About -->
          <template v-else>
            <div class="flex items-center gap-4">
              <img :src="appIcon" alt="" width="56" height="56" class="size-14 shrink-0 object-contain">
              <div>
                <div class="text-base font-semibold">
                  Fonicca
                </div>
                <div v-if="info" class="text-muted-foreground tabular">
                  {{ t('settings.about.version', { version: info.version }) }}
                </div>
              </div>
            </div>

            <div v-if="info" class="flex flex-col gap-2">
              <h3 class="group-title">
                {{ t('settings.about.storage') }}
              </h3>
              <section class="group-box">
                <div
                  v-for="s in [
                    { labelKey: 'settings.about.database', path: info.dbPath, size: info.dbSize },
                    { labelKey: 'settings.about.models', path: info.modelsDir, size: info.modelsSize },
                    { labelKey: 'settings.about.settings', path: info.settingsPath, size: null },
                  ]"
                  :key="s.labelKey"
                  class="row"
                >
                  <div class="row-text">
                    <div class="row-title">
                      {{ t(s.labelKey) }}
                      <span v-if="s.size" class="text-muted-foreground font-normal tabular">· {{ formatSize(s.size) }}</span>
                    </div>
                    <div class="row-desc truncate select-text" :title="s.path">
                      {{ s.path }}
                    </div>
                  </div>
                  <Hint :text="t('common.actions.showInFinder')">
                    <Button variant="ghost" size="icon-sm" class="text-muted-foreground" :aria-label="t('common.actions.showInFinder')" @click="reveal(s.path)">
                      <FolderOpen />
                    </Button>
                  </Hint>
                </div>
              </section>
            </div>
          </template>
        </div>
      </div>
    </DialogContent>
  </Dialog>
</template>

<style scoped>
.nav-row {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  height: 1.875rem;
  padding: 0 0.5rem;
  border-radius: calc(var(--radius) - 2px);
  color: var(--muted-foreground);
  text-align: left;
}

.nav-row:hover {
  background: var(--accent);
  color: var(--foreground);
}

.nav-row.active {
  background: var(--primary-soft);
  color: var(--primary);
  font-weight: 500;
}

.group-title {
  font-size: 12px;
  font-weight: 500;
  color: var(--muted-foreground);
}

.group-box {
  border: 1px solid var(--border);
  border-radius: var(--radius);
  background: var(--card);
}

.row {
  display: flex;
  align-items: center;
  gap: 1rem;
  padding: 0.75rem 1rem;
}

.row + .row {
  border-top: 1px solid var(--border);
}

.row-text {
  flex: 1;
  min-width: 0;
}

.row-title {
  font-weight: 500;
}

.row-desc {
  margin-top: 0.125rem;
  font-size: 12px;
  color: var(--muted-foreground);
}
</style>
