import type { Settings } from '@/api'
import type { Mode, Palette } from '@/themes'
import { listen } from '@tauri-apps/api/event'
import { useMediaQuery } from '@vueuse/core'
import { acceptHMRUpdate, defineStore } from 'pinia'
import { computed, ref, shallowRef, watch, watchEffect } from 'vue'
import { toast } from 'vue-sonner'
import { api } from '@/api'
import { setLocale } from '@/i18n'
import { waveformSettings } from '@/waveform-settings'
import { applyPalette, DEFAULT_THEME, findTheme } from '@/themes'

export type SettingsSection = 'general' | 'library' | 'playback' | 'waveform' | 'themes' | 'analysis' | 'shortcuts' | 'about'

// Mirrors settings.json, which the backend owns; used until it loads.
const DEFAULTS: Settings = {
  theme: 'system',
  colorTheme: DEFAULT_THEME,
  customThemes: [],
  language: 'en',
  watchFolders: true,
  autoplay: true,
  rowPlaybackMode: 'cursor',
  volume: 1,
  meterStyle: 'bars',
  outputDevice: null,
  inputDevice: null,
  analysisThreads: null,
  stemsFolder: null,
  syncEnabled: false,
  projectBpm: null,
  projectKey: null,
  ...waveformSettings(),
  editorWaveform: waveformSettings(),
}

export const useSettingsStore = defineStore('settings', () => {
  const settings = ref<Settings>({ ...DEFAULTS })
  const dialogOpen = ref(false)
  const section = ref<SettingsSection>('general')

  // Every window follows the stored language, including changes made in another window.
  watch(() => settings.value.language, setLocale, { immediate: true })

  // The backend sets the window appearance from `theme`, so the media query already
  // answers "light, dark or the system's".
  const systemDark = useMediaQuery('(prefers-color-scheme: dark)')
  /** A palette being edited, shown on the page instead of the selected theme until cleared. */
  const preview = shallowRef<{ palette: Palette, mode: Mode } | null>(null)
  const mode = computed<Mode>(() => preview.value?.mode ?? (systemDark.value ? 'dark' : 'light'))
  const activeTheme = computed(() => findTheme(settings.value.colorTheme, settings.value.customThemes))
  /** Bumped after colours change; canvases redraw on it since they read CSS variables. */
  const themeRevision = ref(0)
  watchEffect(() => {
    applyPalette(preview.value?.palette ?? activeTheme.value[mode.value], mode.value)
    themeRevision.value++
  })

  async function init() {
    const loaded = await api.getSettings()
    settings.value = { ...loaded, ...waveformSettings(loaded), editorWaveform: waveformSettings(loaded.editorWaveform) }
    // Changes made in another window (e.g. the main one while an editor is open).
    await listen<Settings>('settings-changed', ({ payload }) => {
      settings.value = { ...payload, ...waveformSettings(payload), editorWaveform: waveformSettings(payload.editorWaveform) }
    })
  }

  /** Applies right away; the backend stores the full object and clamps values. */
  async function update(patch: Partial<Settings>) {
    const prev = settings.value
    settings.value = { ...prev, ...patch }
    try {
      const saved = await api.updateSettings(settings.value)
      settings.value = { ...saved, ...waveformSettings(saved), editorWaveform: waveformSettings(saved.editorWaveform) }
    }
    catch (e) {
      settings.value = prev
      toast.error(String(e))
    }
  }

  function openDialog(at?: SettingsSection) {
    if (at)
      section.value = at
    dialogOpen.value = true
  }

  return { settings, dialogOpen, section, mode, activeTheme, preview, themeRevision, init, update, openDialog }
})

if (import.meta.hot)
  import.meta.hot.accept(acceptHMRUpdate(useSettingsStore, import.meta.hot))
