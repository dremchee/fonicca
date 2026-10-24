<script setup lang="ts">
import type { ColorKey, CustomTheme, Mode, Theme } from '@/themes'
import { AlertTriangle, ArrowLeft, Check, Copy, Download, Ellipsis, Moon, Pencil, Plus, RotateCcw, Sun, Trash2, Upload } from '@lucide/vue'
import { open, save } from '@tauri-apps/plugin-dialog'
import { storeToRefs } from 'pinia'
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { toast } from 'vue-sonner'
import { api } from '@/api'
import ThemeThumb from '@/components/ThemeThumb.vue'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { ColorInput } from '@/components/ui/color-input'
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuSeparator, DropdownMenuTrigger } from '@/components/ui/dropdown-menu'
import { Input } from '@/components/ui/input'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { confirm } from '@/composables/confirm'
import { formatNumber } from '@/format'
import { useSettingsStore } from '@/stores/settings'
import { BUILT_IN, COLOR_KEYS, contrastIssues, DEFAULT_THEME, isHex } from '@/themes'

const { t } = useI18n()
const store = useSettingsStore()
const { settings, mode } = storeToRefs(store)

const themes = computed<Theme[]>(() => [...BUILT_IN, ...settings.value.customThemes.map(c => ({ ...c, builtIn: false }))])

function themeName(theme: Pick<Theme, 'id' | 'name' | 'builtIn'>) {
  return theme.builtIn ? t(`settings.themes.names.${theme.id as 'samplette'}`) : theme.name
}

// Plain JSON data: a JSON round trip also unwraps Vue's reactive proxies.
const clone = <T>(v: T): T => JSON.parse(JSON.stringify(v))

function newId() {
  return `custom-${Date.now().toString(36)}${Math.random().toString(36).slice(2, 6)}`
}

function uniqueName(base: string) {
  const names = new Set(themes.value.map(themeName))
  if (!names.has(base))
    return base
  let n = 2
  while (names.has(`${base} ${n}`)) n++
  return `${base} ${n}`
}

function select(theme: Theme) {
  if (settings.value.colorTheme !== theme.id)
    store.update({ colorTheme: theme.id })
}

// --- Editing ------------------------------------------------------------------
const draft = ref<CustomTheme | null>(null)
/** Which variant is being edited; the whole app shows it meanwhile. */
const editMode = ref<Mode>(mode.value)
const saved = computed(() => settings.value.customThemes.find(c => c.id === draft.value?.id))
const dirty = computed(() => !!draft.value && JSON.stringify(draft.value) !== JSON.stringify(saved.value))
const issues = computed(() => draft.value ? contrastIssues(draft.value[editMode.value]) : [])

watch([draft, editMode], () => {
  store.preview = draft.value ? { palette: { ...draft.value[editMode.value] }, mode: editMode.value } : null
}, { deep: true })
// Leaving the section or closing the dialog ends the preview; unsaved edits are dropped.
onBeforeUnmount(() => {
  store.preview = null
})

function edit(theme: CustomTheme) {
  draft.value = clone(theme)
  editMode.value = mode.value
  select({ ...theme, builtIn: false })
}

async function addCustom(theme: CustomTheme) {
  await store.update({ customThemes: [...settings.value.customThemes, theme], colorTheme: theme.id })
  edit(theme)
}

function duplicate(theme: Theme, name = t('settings.themes.copyName', { name: themeName(theme) })) {
  addCustom({ id: newId(), name: uniqueName(name), light: clone(theme.light), dark: clone(theme.dark) })
}

function createNew() {
  duplicate(store.activeTheme, t('settings.themes.newName'))
}

function setColor(key: ColorKey, value: string) {
  if (draft.value && isHex(value))
    draft.value[editMode.value][key] = value.trim().toLowerCase()
}

async function saveDraft() {
  const theme = draft.value
  if (!theme)
    return
  theme.name = theme.name.trim() || t('settings.themes.untitled')
  await store.update({ customThemes: settings.value.customThemes.map(c => c.id === theme.id ? clone(theme) : c) })
  toast.success(t('settings.themes.editor.saved', { name: theme.name }))
}

function revert() {
  if (saved.value)
    draft.value = clone(saved.value)
}

async function back() {
  if (dirty.value) {
    const ok = await confirm({
      title: t('settings.themes.confirmDiscard.title'),
      description: t('settings.themes.confirmDiscard.description'),
      action: t('settings.themes.confirmDiscard.action'),
      destructive: true,
    })
    if (!ok)
      return
  }
  draft.value = null
}

async function remove(theme: CustomTheme) {
  const ok = await confirm({
    title: t('settings.themes.confirmDelete.title', { name: theme.name }),
    description: t('settings.themes.confirmDelete.description'),
    action: t('settings.themes.delete'),
    destructive: true,
  })
  if (!ok)
    return
  if (draft.value?.id === theme.id)
    draft.value = null
  await store.update({
    customThemes: settings.value.customThemes.filter(c => c.id !== theme.id),
    colorTheme: settings.value.colorTheme === theme.id ? DEFAULT_THEME : settings.value.colorTheme,
  })
}

const fileFilters = () => [{ name: t('settings.themes.fileFilter'), extensions: ['json'] }]

async function exportTheme(theme: Theme | CustomTheme) {
  const name = 'builtIn' in theme ? themeName(theme) : theme.name
  const path = await save({ title: t('settings.themes.exportTitle'), defaultPath: `${name}.samplette-theme.json`, filters: fileFilters() })
  if (!path)
    return
  try {
    await api.exportTheme(path, { id: theme.id, name, light: theme.light, dark: theme.dark })
  }
  catch (e) {
    toast.error(String(e))
  }
}

async function importTheme() {
  const path = await open({ title: t('settings.themes.importTitle'), filters: fileFilters() })
  if (typeof path !== 'string')
    return
  try {
    const theme = await api.importTheme(path)
    const imported = { ...theme, id: newId(), name: uniqueName(theme.name) }
    await store.update({ customThemes: [...settings.value.customThemes, imported], colorTheme: imported.id })
    toast.success(t('settings.themes.imported', { name: imported.name }))
  }
  catch (e) {
    toast.error(String(e))
  }
}
</script>

<template>
  <!-- Theme list -->
  <template v-if="!draft">
    <div class="flex items-center gap-2">
      <p class="text-xs text-muted-foreground flex-1">
        {{ t('settings.themes.intro') }}
      </p>
      <Button variant="outline" size="sm" @click="importTheme">
        <Upload /> {{ t('settings.themes.import') }}
      </Button>
      <Button size="sm" @click="createNew">
        <Plus /> {{ t('settings.themes.newTheme') }}
      </Button>
    </div>
    <div class="grid grid-cols-3 gap-3">
      <div
        v-for="theme in themes"
        :key="theme.id"
        class="card group"
        :class="{ selected: theme.id === store.activeTheme.id }"
        role="button"
        tabindex="0"
        :aria-pressed="theme.id === store.activeTheme.id"
        @click="select(theme)"
        @keydown.enter="select(theme)"
      >
        <ThemeThumb :pal="theme[mode]" />
        <div class="flex items-center gap-1.5 pt-2">
          <Check v-if="theme.id === store.activeTheme.id" class="size-3.5 text-primary shrink-0" />
          <span class="font-medium truncate">{{ themeName(theme) }}</span>
          <Badge v-if="theme.builtIn" variant="secondary" class="h-4 px-1.5 text-[10px]">
            {{ t('settings.themes.builtIn') }}
          </Badge>
          <div class="ml-auto flex opacity-0 group-hover:opacity-100 focus-within:opacity-100" @click.stop>
            <Button v-if="!theme.builtIn" variant="ghost" size="icon-xs" :aria-label="t('settings.themes.edit')" @click="edit(theme)">
              <Pencil />
            </Button>
            <DropdownMenu>
              <DropdownMenuTrigger as-child>
                <Button variant="ghost" size="icon-xs" :aria-label="t('settings.themes.more')">
                  <Ellipsis />
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end">
                <DropdownMenuItem @select="duplicate(theme)">
                  <Copy /> {{ t('settings.themes.duplicate') }}
                </DropdownMenuItem>
                <DropdownMenuItem @select="exportTheme(theme)">
                  <Download /> {{ t('settings.themes.export') }}
                </DropdownMenuItem>
                <template v-if="!theme.builtIn">
                  <DropdownMenuSeparator />
                  <DropdownMenuItem class="text-destructive focus:text-destructive" @select="remove(theme)">
                    <Trash2 /> {{ t('settings.themes.delete') }}
                  </DropdownMenuItem>
                </template>
              </DropdownMenuContent>
            </DropdownMenu>
          </div>
        </div>
      </div>
    </div>
    <p class="text-xs text-muted-foreground">
      {{ t('settings.themes.builtInNote') }}
    </p>
  </template>

  <!-- Theme editor -->
  <template v-else>
    <div class="flex items-center gap-2 -mt-1">
      <Button variant="ghost" size="icon-sm" class="-ml-2" :aria-label="t('settings.themes.editor.back')" @click="back">
        <ArrowLeft />
      </Button>
      <Input v-model="draft.name" class="h-8 max-w-64" :aria-label="t('settings.themes.editor.name')" :placeholder="t('settings.themes.editor.name')" />
      <span v-if="dirty" class="text-xs text-muted-foreground">· {{ t('settings.themes.editor.unsaved') }}</span>
      <ToggleGroup :model-value="editMode" type="single" variant="outline" size="sm" class="ml-auto" @update:model-value="$event && (editMode = $event as Mode)">
        <ToggleGroupItem value="light">
          <Sun /> {{ t('settings.themes.editor.light') }}
        </ToggleGroupItem>
        <ToggleGroupItem value="dark">
          <Moon /> {{ t('settings.themes.editor.dark') }}
        </ToggleGroupItem>
      </ToggleGroup>
    </div>

    <div class="grid grid-cols-[1fr_220px] gap-4 items-start">
      <section class="group-box">
        <div v-for="key in COLOR_KEYS" :key="key" class="color-row">
          <div class="flex-1 min-w-0">
            <div class="font-medium">
              {{ t(`settings.themes.colors.${key}.label`) }}
            </div>
            <div class="text-xs text-muted-foreground">
              {{ t(`settings.themes.colors.${key}.hint`) }}
            </div>
            <div v-for="issue in issues.filter(i => i.key === key)" :key="issue.check" class="text-xs text-amber-600 dark:text-amber-400 flex items-center gap-1 mt-0.5">
              <AlertTriangle class="size-3 shrink-0" />
              {{ t('settings.themes.contrast.message', { what: t(`settings.themes.contrast.${issue.check}`), ratio: formatNumber(issue.ratio, { fixed: 1 }), min: formatNumber(issue.min, { max: 1 }) }) }}
            </div>
          </div>
          <!-- Themes store #rrggbb: hex only, no transparency. -->
          <ColorInput
            :model-value="draft[editMode][key]"
            class="h-8 w-32 font-mono text-xs"
            :formats="['hex']"
            :show-alpha="false"
            popover-side="left"
            :aria-label="t('settings.themes.editor.hex', { color: t(`settings.themes.colors.${key}.label`) })"
            @update:model-value="setColor(key, $event)"
          />
        </div>
      </section>
      <aside class="flex flex-col gap-3">
        <div class="text-xs font-medium text-muted-foreground">
          {{ t('settings.themes.editor.preview') }}
        </div>
        <ThemeThumb :pal="draft.light" />
        <ThemeThumb :pal="draft.dark" />
        <p class="text-xs text-muted-foreground">
          {{ t(editMode === 'light' ? 'settings.themes.editor.previewLight' : 'settings.themes.editor.previewDark') }}
        </p>
      </aside>
    </div>

    <div class="sticky bottom-0 -mx-6 -mb-5 mt-auto px-6 py-3 flex items-center gap-2 border-t border-border bg-background">
      <Button v-if="saved" variant="ghost" size="sm" class="text-destructive hover:text-destructive" @click="remove(saved)">
        <Trash2 /> {{ t('settings.themes.delete') }}
      </Button>
      <Button variant="ghost" size="sm" @click="exportTheme(draft)">
        <Download /> {{ t('settings.themes.export') }}
      </Button>
      <Button variant="outline" size="sm" class="ml-auto" :disabled="!dirty" @click="revert">
        <RotateCcw /> {{ t('settings.themes.editor.revert') }}
      </Button>
      <Button size="sm" :disabled="!dirty" @click="saveDraft">
        {{ t('common.actions.save') }}
      </Button>
    </div>
  </template>
</template>

<style scoped>
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

.color-row {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  padding: 0.625rem 0.875rem;
}

.color-row + .color-row {
  border-top: 1px solid var(--border);
}
</style>
