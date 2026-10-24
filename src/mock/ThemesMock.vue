<script setup lang="ts">
import type { ColorKey, Mode, Palette, Theme } from './themes'
import { AlertTriangle, ArrowLeft, Check, Copy, Download, Ellipsis, Moon, Pencil, Plus, RotateCcw, Sun, Trash2, Upload } from '@lucide/vue'
import { computed, ref, watchEffect } from 'vue'
import { toast } from 'vue-sonner'
import ConfirmDialog from '@/components/ConfirmDialog.vue'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogTitle } from '@/components/ui/dialog'
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuSeparator, DropdownMenuTrigger } from '@/components/ui/dropdown-menu'
import { Input } from '@/components/ui/input'
import { Toaster } from '@/components/ui/sonner'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { TooltipProvider } from '@/components/ui/tooltip'
import { confirm } from '@/composables/confirm'
import ThemeThumb from './ThemeThumb.vue'
import { BUILT_IN, COLOR_INFO, COLOR_KEYS, contrastIssues, cssVars, isHex } from './themes'

// Plain JSON data: a JSON round trip also unwraps Vue's reactive proxies, which structuredClone rejects.
const clone = <T>(v: T): T => JSON.parse(JSON.stringify(v))

const open = ref(true)
// Stands in for Appearance (System / Light / Dark) and the macOS setting.
const mode = ref<Mode>(matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light')

const themes = ref<Theme[]>(clone(BUILT_IN))
const activeId = ref('samplette')
const active = computed(() => themes.value.find(t => t.id === activeId.value) ?? themes.value[0]!)

// --- Editing ------------------------------------------------------------------
const editingId = ref<string | null>(null)
const draft = ref<Theme | null>(null)
const saved = computed(() => themes.value.find(t => t.id === editingId.value))
const dirty = computed(() => !!draft.value && JSON.stringify(draft.value) !== JSON.stringify(saved.value))
const draftPal = computed(() => draft.value?.[mode.value])
const issues = computed(() => draftPal.value ? contrastIssues(draftPal.value) : [])
const issueFor = (key: ColorKey) => issues.value.filter(i => i.key === key)

// Live preview: the whole page wears the draft while it is edited, else the active theme.
watchEffect(() => {
  const pal: Palette = draft.value?.[mode.value] ?? active.value[mode.value]
  const root = document.documentElement
  for (const [k, v] of Object.entries(cssVars(pal, mode.value)))
    root.style.setProperty(k, v)
  root.style.colorScheme = mode.value
})

let nextId = 1
function newId() {
  return `custom-${Date.now().toString(36)}-${nextId++}`
}

function uniqueName(base: string) {
  const names = new Set(themes.value.map(t => t.name))
  if (!names.has(base))
    return base
  let n = 2
  while (names.has(`${base} ${n}`)) n++
  return `${base} ${n}`
}

function edit(theme: Theme) {
  editingId.value = theme.id
  draft.value = clone(theme)
  activeId.value = theme.id
}

function duplicate(theme: Theme, name = `${theme.name} copy`) {
  const copy: Theme = { ...clone(theme), id: newId(), name: uniqueName(name), builtIn: false }
  themes.value.push(copy)
  edit(copy)
}

function createNew() {
  duplicate(active.value, 'My theme')
}

function setColor(key: ColorKey, value: string) {
  if (draft.value && isHex(value))
    draft.value[mode.value][key] = value.toLowerCase()
}

function save() {
  if (!draft.value)
    return
  if (!draft.value.name.trim()) {
    toast.error('Give the theme a name')
    return
  }
  const i = themes.value.findIndex(t => t.id === draft.value!.id)
  themes.value[i] = clone(draft.value)
  toast.success(`Saved “${draft.value.name}”`)
}

function revert() {
  if (saved.value)
    draft.value = clone(saved.value)
}

async function back() {
  if (dirty.value) {
    const ok = await confirm({ title: 'Discard unsaved changes?', description: 'The theme goes back to its last saved colours.', action: 'Discard', destructive: true })
    if (!ok)
      return
  }
  editingId.value = null
  draft.value = null
}

async function remove(theme: Theme) {
  const ok = await confirm({ title: `Delete “${theme.name}”?`, description: 'The Samplette theme is used instead if it is active.', action: 'Delete', destructive: true })
  if (!ok)
    return
  themes.value = themes.value.filter(t => t.id !== theme.id)
  if (activeId.value === theme.id)
    activeId.value = 'samplette'
  if (editingId.value === theme.id) {
    editingId.value = null
    draft.value = null
  }
}

function exportTheme(theme: Theme) {
  const { id: _, builtIn: __, ...data } = theme
  const blob = new Blob([JSON.stringify({ format: 'samplette-theme', version: 1, ...data }, null, 2)], { type: 'application/json' })
  const a = document.createElement('a')
  a.href = URL.createObjectURL(blob)
  a.download = `${theme.name}.samplette-theme.json`
  a.click()
  URL.revokeObjectURL(a.href)
}

const fileInput = ref<HTMLInputElement | null>(null)
async function importTheme(e: Event) {
  const file = (e.target as HTMLInputElement).files?.[0]
  if (!file)
    return
  try {
    const data = JSON.parse(await file.text())
    const valid = (pal: unknown) => !!pal && COLOR_KEYS.every(k => typeof (pal as Palette)[k] === 'string' && isHex((pal as Palette)[k]))
    if (data.format !== 'samplette-theme' || !valid(data.light) || !valid(data.dark))
      throw new Error('not a Samplette theme')
    const theme: Theme = { id: newId(), name: uniqueName(String(data.name || 'Imported theme')), builtIn: false, light: data.light, dark: data.dark }
    themes.value.push(theme)
    activeId.value = theme.id
    toast.success(`Imported “${theme.name}”`)
  }
  catch (err) {
    toast.error(`Cannot import: ${(err as Error).message}`)
  }
  finally {
    (e.target as HTMLInputElement).value = ''
  }
}

const sections = ['General', 'Library', 'Playback', 'Waveform', 'Themes', 'Analysis', 'Shortcuts', 'About']
</script>

<template>
  <TooltipProvider :delay-duration="500">
    <!-- Stand-in for the main window, wearing the current theme. -->
    <div class="h-full flex">
      <aside class="w-56 shrink-0 border-r border-border bg-card p-2 flex flex-col gap-px">
        <div class="h-9" />
        <div class="side-row active">
          <span class="i-lucide-library" /> All samples
        </div>
        <div class="side-row">
          <span class="i-lucide-star" /> Favorites
        </div>
        <div class="side-row">
          <span class="i-lucide-copy" /> Duplicates
        </div>
        <div class="px-2 pt-4 pb-1 text-[11px] font-medium text-muted-foreground">
          Folders
        </div>
        <div class="side-row">
          <span class="i-lucide-folder" /> Source
        </div>
      </aside>
      <main class="flex-1 flex flex-col">
        <header class="h-11 px-3 flex items-center gap-3 border-b border-border">
          <div class="h-8 w-80 rounded-md border border-input px-3 flex items-center text-muted-foreground">
            Search samples
          </div>
          <ToggleGroup :model-value="mode" type="single" variant="outline" size="sm" class="ml-auto" @update:model-value="$event && (mode = $event as Mode)">
            <ToggleGroupItem value="light">
              <Sun /> Light
            </ToggleGroupItem>
            <ToggleGroupItem value="dark">
              <Moon /> Dark
            </ToggleGroupItem>
          </ToggleGroup>
          <Button size="sm" @click="open = true">
            Themes…
          </Button>
        </header>
        <div class="flex-1">
          <div v-for="(n, i) in ['Kick Rumble 01', 'Snare Crack', 'Hat Closed 909', 'Loop Dust 90 Am']" :key="n" class="h-8 px-3 flex items-center gap-3 border-b border-border" :class="{ 'bg-primary-soft': i === 1 }">
            <span class="flex-1">{{ n }}</span>
            <span class="text-xs text-muted-foreground">0.5s</span>
          </div>
        </div>
        <div class="h-24 px-3 flex items-center gap-[2px] border-t border-border">
          <b v-for="h in 90" :key="h" class="flex-1 rounded-sm" :style="{ height: `${10 + Math.abs(Math.sin(h / 3)) * 60}%`, background: h < 34 ? 'var(--primary)' : 'var(--wave)' }" />
        </div>
      </main>
    </div>

    <Dialog v-model:open="open">
      <DialogContent class="p-0 gap-0 sm:max-w-5xl h-[min(720px,calc(100vh-4rem))] grid grid-cols-[200px_1fr] overflow-hidden">
        <nav class="bg-muted/40 border-r border-border p-2 pt-4 flex flex-col gap-px">
          <div v-for="s in sections" :key="s" class="nav-row" :class="{ active: s === 'Themes' }">
            {{ s }}
          </div>
        </nav>

        <div class="min-w-0 flex flex-col">
          <header class="h-13 shrink-0 px-6 flex items-center gap-2 border-b border-border">
            <Button v-if="draft" variant="ghost" size="icon-sm" class="-ml-2" aria-label="Back to themes" @click="back">
              <ArrowLeft />
            </Button>
            <DialogTitle class="text-base font-semibold">
              {{ draft ? draft.name || 'Untitled theme' : 'Themes' }}
            </DialogTitle>
            <span v-if="dirty" class="text-xs text-muted-foreground">· unsaved</span>
            <DialogDescription class="sr-only">
              Colour themes
            </DialogDescription>
          </header>

          <!-- Theme list -->
          <div v-if="!draft" class="flex-1 overflow-y-auto px-6 py-5 flex flex-col gap-4">
            <div class="flex items-center gap-2">
              <p class="text-xs text-muted-foreground flex-1">
                Each theme has a light and a dark variant; Appearance in General picks which one you see.
              </p>
              <Button variant="outline" size="sm" @click="fileInput?.click()">
                <Upload /> Import…
              </Button>
              <Button size="sm" @click="createNew">
                <Plus /> New theme
              </Button>
              <input ref="fileInput" type="file" accept=".json,application/json" class="hidden" @change="importTheme">
            </div>
            <div class="grid grid-cols-3 gap-3">
              <div
                v-for="t in themes"
                :key="t.id"
                class="card group"
                :class="{ selected: t.id === activeId }"
                role="button"
                tabindex="0"
                @click="activeId = t.id"
                @keydown.enter="activeId = t.id"
              >
                <ThemeThumb :pal="t[mode]" />
                <div class="flex items-center gap-1.5 pt-2">
                  <Check v-if="t.id === activeId" class="size-3.5 text-primary shrink-0" />
                  <span class="font-medium truncate">{{ t.name }}</span>
                  <Badge v-if="t.builtIn" variant="secondary" class="h-4 px-1.5 text-[10px]">
                    Built-in
                  </Badge>
                  <div class="ml-auto flex opacity-0 group-hover:opacity-100 focus-within:opacity-100" @click.stop>
                    <Button v-if="!t.builtIn" variant="ghost" size="icon-xs" aria-label="Edit" @click="edit(t)">
                      <Pencil />
                    </Button>
                    <DropdownMenu>
                      <DropdownMenuTrigger as-child>
                        <Button variant="ghost" size="icon-xs" aria-label="More">
                          <Ellipsis />
                        </Button>
                      </DropdownMenuTrigger>
                      <DropdownMenuContent align="end">
                        <DropdownMenuItem @select="duplicate(t)">
                          <Copy /> Duplicate
                        </DropdownMenuItem>
                        <DropdownMenuItem @select="exportTheme(t)">
                          <Download /> Export…
                        </DropdownMenuItem>
                        <template v-if="!t.builtIn">
                          <DropdownMenuSeparator />
                          <DropdownMenuItem class="text-destructive focus:text-destructive" @select="remove(t)">
                            <Trash2 /> Delete
                          </DropdownMenuItem>
                        </template>
                      </DropdownMenuContent>
                    </DropdownMenu>
                  </div>
                </div>
              </div>
            </div>
            <p class="text-xs text-muted-foreground">
              Built-in themes can't be changed: duplicate one to make it yours.
            </p>
          </div>

          <!-- Theme editor -->
          <template v-else>
            <div class="flex-1 min-h-0 grid grid-cols-[1fr_260px]">
              <div class="overflow-y-auto px-6 py-5 flex flex-col gap-4">
                <div class="flex items-center gap-3">
                  <Input v-model="draft.name" class="h-8 max-w-64" aria-label="Theme name" placeholder="Theme name" />
                  <ToggleGroup :model-value="mode" type="single" variant="outline" size="sm" @update:model-value="$event && (mode = $event as Mode)">
                    <ToggleGroupItem value="light">
                      <Sun /> Light
                    </ToggleGroupItem>
                    <ToggleGroupItem value="dark">
                      <Moon /> Dark
                    </ToggleGroupItem>
                  </ToggleGroup>
                </div>
                <section class="group-box">
                  <div v-for="key in COLOR_KEYS" :key="key" class="row">
                    <label class="swatch" :style="{ background: draft[mode][key] }">
                      <input type="color" :value="draft[mode][key]" :aria-label="COLOR_INFO[key].label" @input="setColor(key, ($event.target as HTMLInputElement).value)">
                    </label>
                    <div class="flex-1 min-w-0">
                      <div class="font-medium">
                        {{ COLOR_INFO[key].label }}
                      </div>
                      <div class="text-xs text-muted-foreground">
                        {{ COLOR_INFO[key].hint }}
                      </div>
                      <div v-for="issue in issueFor(key)" :key="issue.text" class="text-xs text-amber-600 dark:text-amber-400 flex items-center gap-1 mt-0.5">
                        <AlertTriangle class="size-3 shrink-0" /> {{ issue.text }}
                      </div>
                    </div>
                    <Input
                      :model-value="draft[mode][key]"
                      class="h-7 w-24 font-mono text-xs"
                      :aria-label="`${COLOR_INFO[key].label} hex`"
                      @change="setColor(key, ($event.target as HTMLInputElement).value)"
                    />
                  </div>
                </section>
              </div>
              <aside class="border-l border-border p-4 flex flex-col gap-3 bg-muted/30">
                <div class="text-xs font-medium text-muted-foreground">
                  Preview
                </div>
                <ThemeThumb :pal="draft.light" />
                <ThemeThumb :pal="draft.dark" />
                <p class="text-xs text-muted-foreground">
                  The app behind this dialog shows the {{ mode }} variant as you edit.
                </p>
              </aside>
            </div>
            <footer class="h-14 shrink-0 px-5 flex items-center gap-2 border-t border-border">
              <Button variant="ghost" size="sm" class="text-destructive hover:text-destructive" @click="remove(saved!)">
                <Trash2 /> Delete
              </Button>
              <Button variant="ghost" size="sm" @click="exportTheme(draft)">
                <Download /> Export…
              </Button>
              <Button variant="outline" size="sm" class="ml-auto" :disabled="!dirty" @click="revert">
                <RotateCcw /> Revert
              </Button>
              <Button size="sm" :disabled="!dirty" @click="save">
                Save
              </Button>
            </footer>
          </template>
        </div>
      </DialogContent>
    </Dialog>

    <ConfirmDialog />
    <Toaster position="bottom-right" />
  </TooltipProvider>
</template>

<style scoped>
.side-row {
  height: 1.75rem;
  padding: 0 0.5rem;
  border-radius: calc(var(--radius) - 2px);
  display: flex;
  align-items: center;
  gap: 0.5rem;
  color: var(--muted-foreground);
}

.side-row.active,
.nav-row.active {
  background: var(--primary-soft);
  color: var(--primary);
}

.nav-row {
  height: 1.875rem;
  padding: 0 0.5rem;
  border-radius: calc(var(--radius) - 2px);
  display: flex;
  align-items: center;
  color: var(--muted-foreground);
}

.card {
  padding: 0.5rem;
  border-radius: var(--radius);
  border: 1px solid var(--border);
  background: var(--card);
  cursor: pointer;
  outline: none;
}

.card:hover {
  border-color: var(--muted-foreground);
}

.card.selected,
.card:focus-visible {
  border-color: var(--primary);
  box-shadow: 0 0 0 1px var(--primary);
}

.group-box {
  border: 1px solid var(--border);
  border-radius: var(--radius);
  background: var(--card);
}

.row {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  padding: 0.625rem 0.875rem;
}

.row + .row {
  border-top: 1px solid var(--border);
}

.swatch {
  position: relative;
  width: 1.75rem;
  height: 1.75rem;
  flex-shrink: 0;
  border-radius: 999px;
  border: 1px solid var(--border);
  box-shadow: inset 0 0 0 1px rgb(0 0 0 / 0.08);
  cursor: pointer;
}

.swatch input {
  position: absolute;
  inset: 0;
  opacity: 0;
  cursor: pointer;
}
</style>
