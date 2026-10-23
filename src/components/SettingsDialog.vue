<script setup lang="ts">
import type { Component } from 'vue'
import type { AppInfo, Settings, Theme } from '@/api'
import type { SettingsSection } from '@/stores/settings'
import {
  AudioLines,
  AudioWaveform,
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
import { computed, ref, watch } from 'vue'
import { toast } from 'vue-sonner'
import { api } from '@/api'
import Hint from '@/components/Hint.vue'
import WaveformPreview from '@/components/WaveformPreview.vue'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogTitle } from '@/components/ui/dialog'
import { Kbd, KbdGroup } from '@/components/ui/kbd'
import { Select, SelectContent, SelectItem, SelectSeparator, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Slider } from '@/components/ui/slider'
import { Spinner } from '@/components/ui/spinner'
import { Switch } from '@/components/ui/switch'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { confirm } from '@/composables/confirm'
import { folderName, formatSize } from '@/format'
import { useDuplicatesStore } from '@/stores/duplicates'
import { useLibraryStore } from '@/stores/library'
import { useSettingsStore } from '@/stores/settings'

const store = useSettingsStore()
const { settings, dialogOpen, section } = storeToRefs(store)
const library = useLibraryStore()
const { sources, scans, analysis, soundModel, modelDownload } = storeToRefs(library)
const duplicates = useDuplicatesStore()

const sections: { id: SettingsSection, label: string, icon: Component }[] = [
  { id: 'general', label: 'General', icon: Settings2 },
  { id: 'library', label: 'Library', icon: Library },
  { id: 'playback', label: 'Playback', icon: AudioLines },
  { id: 'waveform', label: 'Waveform', icon: AudioWaveform },
  { id: 'analysis', label: 'Analysis', icon: Cpu },
  { id: 'shortcuts', label: 'Shortcuts', icon: Keyboard },
  { id: 'about', label: 'About', icon: Info },
]
const current = computed(() => sections.find(s => s.id === section.value)!)

// Paths and sizes change while the app runs: read them each time the dialog opens.
const info = ref<AppInfo | null>(null)
watch(dialogOpen, async (open) => {
  if (open)
    [info.value] = await Promise.all([api.appInfo(), duplicates.loadHidden()])
})

async function unhide(paths: string[] | null) {
  try {
    await duplicates.unhide(paths)
  }
  catch (e) {
    toast.error(String(e))
  }
}

function samplesLabel(n: number) {
  return `${n.toLocaleString()} ${n === 1 ? 'sample' : 'samples'}`
}

function scanLabel(id: number) {
  const s = scans.value.get(id)
  if (!s)
    return null
  return s.total ? `scanning ${s.processed.toLocaleString()}/${s.total.toLocaleString()}` : 'scanning…'
}

async function removeSource(id: number) {
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

const modelState = computed(() => {
  if (modelDownload.value && !modelDownload.value.done)
    return 'downloading'
  return soundModel.value?.installed ? 'installed' : 'missing'
})
const downloadRatio = computed(() => {
  const d = modelDownload.value
  return d?.total ? d.received / d.total : 0
})

async function deleteModel() {
  const ok = await confirm({
    title: 'Delete the sound model?',
    description: 'Categories already found by sound stay. Samples added later get categories from their names only, until you download the model again.',
    action: 'Delete',
    destructive: true,
  })
  if (!ok)
    return
  try {
    await library.deleteSoundModel()
    info.value = await api.appInfo()
  }
  catch (e) {
    toast.error(String(e))
  }
}

type WaveKey = 'waveStyle' | 'waveScale' | 'waveChannels' | 'waveColors'
const waveOptions: { key: WaveKey, title: string, desc: string, choices: { value: Settings[WaveKey], label: string }[] }[] = [
  {
    key: 'waveStyle',
    title: 'Style',
    desc: 'Peaks + RMS shows the average level inside the peaks.',
    choices: [{ value: 'peaks', label: 'Peaks' }, { value: 'peaksRms', label: 'Peaks + RMS' }, { value: 'bars', label: 'Bars' }],
  },
  {
    key: 'waveScale',
    title: 'Amplitude scale',
    desc: 'Decibels make quiet tails and reverb visible. ⌥-scroll in the editor zooms vertically.',
    choices: [{ value: 'linear', label: 'Linear' }, { value: 'db', label: 'Decibels' }],
  },
  {
    key: 'waveChannels',
    title: 'Channels',
    desc: 'One lane per channel shows stereo width and differences between sides.',
    choices: [{ value: 'split', label: 'Separate' }, { value: 'mix', label: 'Mixed' }],
  },
  {
    key: 'waveColors',
    title: 'Colors',
    desc: 'By frequency: bass red, mids green, highs blue, so kicks and hats stand apart.',
    choices: [{ value: 'theme', label: 'Theme' }, { value: 'frequency', label: 'By frequency' }],
  },
]

const shortcuts: { keys: string[], label: string }[] = [
  { keys: ['↑', '↓'], label: 'Move focus' },
  { keys: ['⇧', '↑ ↓'], label: 'Extend selection' },
  { keys: ['Space'], label: 'Play / stop' },
  { keys: ['Esc'], label: 'Stop, then leave search' },
  { keys: ['⌘', 'A'], label: 'Select all' },
  { keys: ['F'], label: 'Toggle favorite' },
  { keys: ['T'], label: 'Add tag' },
  { keys: ['E'], label: 'Edit in a new window (or double-click)' },
  { keys: ['S'], label: 'Find similar sounds' },
  { keys: ['⌥', 'scroll'], label: 'Vertical zoom in the editor' },
  { keys: ['⌘', 'K'], label: 'Command palette' },
  { keys: ['⌘', 'F'], label: 'Search' },
  { keys: ['⌘', ','], label: 'Settings' },
]
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
          {{ s.label }}
        </button>
      </nav>

      <div class="min-w-0 flex flex-col">
        <header class="h-13 shrink-0 px-6 flex items-center border-b border-border">
          <DialogTitle class="text-base font-semibold">
            {{ current.label }}
          </DialogTitle>
          <DialogDescription class="sr-only">
            {{ current.label }} settings
          </DialogDescription>
        </header>

        <div class="flex-1 overflow-y-auto px-6 py-5 flex flex-col gap-6">
          <!-- General -->
          <template v-if="section === 'general'">
            <section class="group-box">
              <div class="row">
                <div class="row-text">
                  <div class="row-title">
                    Appearance
                  </div>
                  <div class="row-desc">
                    System follows the macOS setting.
                  </div>
                </div>
                <ToggleGroup
                  :model-value="settings.theme"
                  type="single"
                  variant="outline"
                  size="sm"
                  @update:model-value="$event && store.update({ theme: $event as Theme })"
                >
                  <ToggleGroupItem value="system" aria-label="System">
                    <Monitor /> System
                  </ToggleGroupItem>
                  <ToggleGroupItem value="light" aria-label="Light">
                    <Sun /> Light
                  </ToggleGroupItem>
                  <ToggleGroupItem value="dark" aria-label="Dark">
                    <Moon /> Dark
                  </ToggleGroupItem>
                </ToggleGroup>
              </div>
            </section>
          </template>

          <!-- Library -->
          <template v-else-if="section === 'library'">
            <div class="flex flex-col gap-2">
              <div class="flex items-center justify-between">
                <h3 class="group-title">
                  Folders
                </h3>
                <Button variant="outline" size="sm" @click="library.addSources()">
                  <FolderPlus /> Add folders…
                </Button>
              </div>
              <section class="group-box">
                <div v-if="!sources.length" class="row text-muted-foreground justify-center">
                  No folders yet
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
                    <Hint text="Show in Finder">
                      <Button variant="ghost" size="icon-sm" class="text-muted-foreground" aria-label="Show in Finder" @click="reveal(s.path)">
                        <FolderOpen />
                      </Button>
                    </Hint>
                    <Hint text="Rescan">
                      <Button variant="ghost" size="icon-sm" class="text-muted-foreground" aria-label="Rescan" :disabled="scans.has(s.id)" @click="library.rescanSource(s.id)">
                        <RefreshCw />
                      </Button>
                    </Hint>
                    <Hint text="Remove from library">
                      <Button variant="ghost" size="icon-sm" class="text-muted-foreground hover:text-destructive" aria-label="Remove from library" @click="removeSource(s.id)">
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
                  <div class="row-title">Watch folders for changes</div>
                  <div class="row-desc">New, moved and deleted files show up without a manual rescan.</div>
                </div>
                <Switch :model-value="settings.watchFolders" @update:model-value="store.update({ watchFolders: $event })" />
              </label>
            </section>

            <div class="flex flex-col gap-2">
              <div class="flex items-center justify-between">
                <h3 class="group-title">
                  Hidden files<template v-if="duplicates.hidden.length">
                    · {{ duplicates.hidden.length.toLocaleString() }}
                  </template>
                </h3>
                <Button v-if="duplicates.hidden.length > 1" variant="outline" size="sm" @click="unhide(null)">
                  Unhide all
                </Button>
              </div>
              <section class="group-box max-h-64 overflow-y-auto">
                <div v-if="!duplicates.hidden.length" class="row text-muted-foreground">
                  Nothing hidden. Extra copies of duplicates can be hidden instead of trashed.
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
                    Unhide
                  </Button>
                </div>
              </section>
            </div>

            <div class="flex flex-col gap-2">
              <h3 class="group-title">
                Your data
              </h3>
              <section class="group-box">
                <div class="row">
                  <div class="row-text">
                    <div class="row-title">
                      Favorites, tags &amp; collections
                    </div>
                    <div class="row-desc">
                      A JSON file. Import merges into the current library and finds files by path, then by content.
                    </div>
                  </div>
                  <div class="flex gap-2">
                    <Button variant="outline" size="sm" @click="library.exportUserData()">
                      <Download /> Export…
                    </Button>
                    <Button variant="outline" size="sm" @click="library.importUserData()">
                      <Upload /> Import…
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
                  <div class="row-title">Autoplay</div>
                  <div class="row-desc">Play a sample when you move to it with the arrow keys.</div>
                </div>
                <Switch :model-value="settings.autoplay" @update:model-value="store.update({ autoplay: $event })" />
              </label>
              <div class="row">
                <div class="row-text">
                  <div class="row-title">
                    Preview volume
                  </div>
                  <div class="row-desc">
                    Applies to previews only, not to files dragged into a DAW.
                  </div>
                </div>
                <div class="w-48 flex items-center gap-3">
                  <Volume2 class="size-4 text-muted-foreground shrink-0" />
                  <Slider
                    :model-value="[volumePercent]"
                    :max="100"
                    :step="1"
                    aria-label="Preview volume"
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
            <section class="group-box">
              <div v-for="opt in waveOptions" :key="opt.key" class="row">
                <div class="row-text">
                  <div class="row-title">
                    {{ opt.title }}
                  </div>
                  <div class="row-desc">
                    {{ opt.desc }}
                  </div>
                </div>
                <ToggleGroup
                  :model-value="settings[opt.key]"
                  type="single"
                  variant="outline"
                  size="sm"
                  @update:model-value="$event && store.update({ [opt.key]: $event })"
                >
                  <ToggleGroupItem v-for="c in opt.choices" :key="c.value" :value="c.value">
                    {{ c.label }}
                  </ToggleGroupItem>
                </ToggleGroup>
              </div>
            </section>
            <section class="group-box p-3 grid gap-2">
              <div class="text-xs text-muted-foreground">
                Preview
              </div>
              <WaveformPreview />
            </section>
          </template>

          <!-- Analysis -->
          <template v-else-if="section === 'analysis'">
            <section class="group-box">
              <div class="row">
                <div class="row-text">
                  <div class="row-title flex items-center gap-2">
                    <Spinner v-if="!analysis.paused && pending" class="text-primary" />
                    {{ !pending ? 'Library analyzed' : analysis.paused ? 'Analysis paused' : 'Analyzing in the background' }}
                  </div>
                  <div class="row-desc tabular">
                    <template v-if="pending">
                      {{ pending.toLocaleString() }} left ·
                    </template>
                    loudness, one-shot / loop, BPM, key, category
                  </div>
                </div>
                <Button variant="outline" size="sm" :disabled="!pending" @click="library.setAnalysisPaused(!analysis.paused)">
                  <component :is="analysis.paused ? Play : Pause" />
                  {{ analysis.paused ? 'Resume' : 'Pause' }}
                </Button>
              </div>
              <div class="row">
                <div class="row-text">
                  <div class="row-title">
                    CPU threads
                  </div>
                  <div class="row-desc">
                    More threads finish sooner but leave less for your DAW.
                  </div>
                </div>
                <Select v-model="threads">
                  <SelectTrigger size="sm" class="w-40">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="auto">
                      Auto<template v-if="info">
                        ({{ info.defaultThreads }} of {{ info.cores }})
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

            <div v-if="soundModel" class="flex flex-col gap-2">
              <h3 class="group-title">
                Sound model
              </h3>
              <section class="group-box">
                <div class="row">
                  <div class="row-text">
                    <div class="row-title">
                      CLAP audio encoder
                    </div>
                    <div class="row-desc tabular">
                      <template v-if="modelState === 'installed'">
                        Installed · {{ formatSize(soundModel.size) }} · categorizes samples whose names give no hint
                      </template>
                      <template v-else-if="modelState === 'downloading'">
                        Downloading · {{ formatSize(modelDownload!.received) }} of {{ formatSize(modelDownload!.total || soundModel.size) }}
                      </template>
                      <template v-else>
                        Not installed · {{ formatSize(soundModel.size) }} · needed to categorize samples by sound
                      </template>
                    </div>
                    <div v-if="modelState === 'downloading'" class="mt-2 h-1 rounded-full bg-muted overflow-hidden">
                      <div class="h-full bg-primary transition-[width]" :style="{ width: `${downloadRatio * 100}%` }" />
                    </div>
                    <div v-if="modelDownload?.error" class="row-desc text-destructive">
                      {{ modelDownload.error }}
                    </div>
                  </div>
                  <div v-if="modelState === 'installed'" class="flex gap-0.5">
                    <Hint v-if="info" text="Show in Finder">
                      <Button variant="ghost" size="icon-sm" class="text-muted-foreground" aria-label="Show in Finder" @click="reveal(info.modelsDir)">
                        <FolderOpen />
                      </Button>
                    </Hint>
                    <Hint text="Delete model">
                      <Button variant="ghost" size="icon-sm" class="text-muted-foreground hover:text-destructive" aria-label="Delete model" @click="deleteModel">
                        <Trash2 />
                      </Button>
                    </Hint>
                  </div>
                  <Button v-else variant="outline" size="sm" :disabled="modelState === 'downloading'" @click="library.downloadSoundModel()">
                    <Download /> Download
                  </Button>
                </div>
              </section>
            </div>
          </template>

          <!-- Shortcuts -->
          <template v-else-if="section === 'shortcuts'">
            <section class="group-box">
              <div v-for="s in shortcuts" :key="s.label" class="row py-2">
                <div class="flex-1">
                  {{ s.label }}
                </div>
                <KbdGroup>
                  <Kbd v-for="k in s.keys" :key="k">
                    {{ k }}
                  </Kbd>
                </KbdGroup>
              </div>
            </section>
          </template>

          <!-- About -->
          <template v-else>
            <div class="flex items-center gap-4">
              <div class="size-14 rounded-xl bg-primary grid place-items-center text-primary-foreground">
                <span class="i-lucide-audio-waveform text-2xl" />
              </div>
              <div>
                <div class="text-base font-semibold">
                  Samplette
                </div>
                <div v-if="info" class="text-muted-foreground tabular">
                  Version {{ info.version }}
                </div>
              </div>
            </div>

            <div v-if="info" class="flex flex-col gap-2">
              <h3 class="group-title">
                Storage
              </h3>
              <section class="group-box">
                <div
                  v-for="s in [
                    { label: 'Library database', path: info.dbPath, size: info.dbSize },
                    { label: 'Models', path: info.modelsDir, size: info.modelsSize },
                    { label: 'Settings', path: info.settingsPath, size: null },
                  ]"
                  :key="s.label"
                  class="row"
                >
                  <div class="row-text">
                    <div class="row-title">
                      {{ s.label }}
                      <span v-if="s.size" class="text-muted-foreground font-normal tabular">· {{ formatSize(s.size) }}</span>
                    </div>
                    <div class="row-desc truncate select-text" :title="s.path">
                      {{ s.path }}
                    </div>
                  </div>
                  <Hint text="Show in Finder">
                    <Button variant="ghost" size="icon-sm" class="text-muted-foreground" aria-label="Show in Finder" @click="reveal(s.path)">
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
